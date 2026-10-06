//! Core-owned selected-output staging. Transports cannot publish these capabilities.

use std::fs::File;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::DerivedArtifactRoot;
use starlark_map::small_set::SmallSet;

mod plan;
mod source_backing;
pub use plan::PlannedActionOutputStaging;
pub use plan::PublishedPlannedActionOutputs;
pub(in crate::runtime) use source_backing::SourceBacking;
pub(in crate::runtime) use source_backing::source_backing_path;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod linux;

pub(in crate::runtime) fn validate_runfiles_layout(
    workspace: &str,
    entries: &[(String, Option<String>)],
) -> io::Result<()> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        linux::validate_runfiles(workspace, entries)
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        let _ = (workspace, entries);
        Err(unsupported())
    }
}

/// Accepted publication metadata; owns no transfer buffers or open handles.
#[derive(Clone, Debug)]
pub struct PublishedActionOutputs {
    root: PathBuf,
    outputs: Arc<[ActionOutput]>,
}
impl PublishedActionOutputs {
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn outputs(&self) -> &[ActionOutput] {
        &self.outputs
    }
}

/// One request's private output group. File bytes are provisional until sealed
/// and published by Core under native source validation. No path is exposed.
#[derive(Debug)]
pub struct ActionOutputStaging {
    outputs: Arc<[ActionOutput]>,
    root: PathBuf,
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    inner: linux::Staging,
}
impl ActionOutputStaging {
    #[cfg(test)]
    pub(super) fn new(
        workspace: &Path,
        root: &DerivedArtifactRoot,
        outputs: &[ActionOutput],
    ) -> io::Result<Self> {
        let reserved = outputs
            .iter()
            .flat_map(|output| output.root_relative_path().split('/'))
            .collect();
        Self::new_reserved(workspace, root, outputs, &reserved)
    }

    pub(super) fn new_reserved(
        workspace: &Path,
        root: &DerivedArtifactRoot,
        outputs: &[ActionOutput],
        reserved: &SmallSet<&str>,
    ) -> io::Result<Self> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            let inner = linux::Staging::new(workspace, root, outputs, reserved)?;
            Ok(Self {
                outputs: outputs.to_vec().into(),
                root: super::configured_output::configured_output_root(workspace, root),
                inner,
            })
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (workspace, root, outputs, reserved);
            Err(unsupported())
        }
    }
    pub(super) fn new_runfiles(
        workspace: &Path,
        root: &DerivedArtifactRoot,
        output: &ActionOutput,
        workspace_name: &str,
        entries: &[(String, Option<String>)],
        reserved: &SmallSet<&str>,
    ) -> io::Result<Self> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            let inner = linux::Staging::new_runfiles(
                workspace,
                root,
                output,
                workspace_name,
                entries,
                reserved,
            )?;
            Ok(Self {
                outputs: Arc::from([output.clone()]),
                root: super::configured_output::configured_output_root(workspace, root),
                inner,
            })
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (workspace, root, output, workspace_name, entries, reserved);
            Err(unsupported())
        }
    }

    pub(super) fn new_alias(
        workspace: &Path,
        root: &DerivedArtifactRoot,
        output: &ActionOutput,
        target: &str,
        reserved: &SmallSet<&str>,
    ) -> io::Result<Self> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            Ok(Self {
                outputs: Arc::from([output.clone()]),
                root: super::configured_output::configured_output_root(workspace, root),
                inner: linux::Staging::new_alias(workspace, root, output, target, reserved)?,
            })
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (workspace, root, output, target, reserved);
            Err(unsupported())
        }
    }

    pub fn outputs(&self) -> &[ActionOutput] {
        &self.outputs
    }

    /// Empty relative path denotes a declared File; nonempty paths are allowed
    /// only within a declared Directory. The returned file is newly created.
    pub fn create_file(&self, index: usize, relative: &str) -> io::Result<File> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            self.inner.create_file(&self.outputs, index, relative)
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (index, relative);
            Err(unsupported())
        }
    }
    pub fn create_directory(&self, index: usize, relative: &str) -> io::Result<()> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            self.inner.create_directory(&self.outputs, index, relative)
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (index, relative);
            Err(unsupported())
        }
    }
    pub(super) fn seal(&mut self) -> io::Result<()> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            self.inner.seal(&self.outputs)
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            Err(unsupported())
        }
    }
    /// Private singleton header policy; generic/tree/runfiles sealing stays executable.
    pub(super) fn seal_nonexecutable(
        &mut self,
        workspace: &slug_workspace_v2::NormalizedAbsolutePath,
        expected: &[u8],
    ) -> io::Result<()> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            self.inner
                .seal_nonexecutable(&self.outputs, workspace, expected)
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (workspace, expected);
            Err(unsupported())
        }
    }
    pub(super) fn publish_nonexecutable(
        &mut self,
        expected: &[u8],
    ) -> io::Result<PublishedActionOutputs> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            self.inner.check_scalar_bytes(&self.outputs, expected)?;
            self.publish()
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = expected;
            Err(unsupported())
        }
    }
    #[cfg(all(test, target_os = "linux", target_env = "gnu"))]
    pub(super) fn scalar_raw_fds(&self) -> [i32; 6] {
        self.inner.scalar_raw_fds()
    }
    #[cfg(all(test, target_os = "linux", target_env = "gnu"))]
    pub(super) fn scalar_stage_name(&self) -> &str {
        self.inner.scalar_stage_name()
    }
    fn preflight(&self) -> io::Result<()> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            self.inner.preflight_publication(&self.outputs)
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            Err(unsupported())
        }
    }
    pub(super) fn publish(&mut self) -> io::Result<PublishedActionOutputs> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            self.inner.publish(&self.outputs).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "publishing action outputs (earlier outputs may have changed): {error}"
                    ),
                )
            })?;
            Ok(PublishedActionOutputs {
                root: self.root.clone(),
                outputs: self.outputs.clone(),
            })
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            Err(unsupported())
        }
    }
}
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "action output publication requires Linux GNU",
    )
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod tests;
