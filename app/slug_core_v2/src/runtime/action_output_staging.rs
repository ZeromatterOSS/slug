//! Core-owned selected-output staging. Transports cannot publish these capabilities.

use std::fs::File;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use slug_build_api_v2::ActionOutput;
use slug_configuration_v2::SlugConfiguration;
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
        configuration: &SlugConfiguration,
        outputs: &[ActionOutput],
    ) -> io::Result<Self> {
        let reserved = outputs
            .iter()
            .flat_map(|output| output.path().split('/'))
            .collect();
        Self::new_reserved(workspace, configuration, outputs, &reserved)
    }

    pub(super) fn new_reserved(
        workspace: &Path,
        configuration: &SlugConfiguration,
        outputs: &[ActionOutput],
        reserved: &SmallSet<&str>,
    ) -> io::Result<Self> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            let inner = linux::Staging::new(workspace, configuration, outputs, reserved)?;
            Ok(Self {
                outputs: outputs.to_vec().into(),
                root: super::configured_output::configured_output_root(workspace, configuration),
                inner,
            })
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (workspace, configuration, outputs, reserved);
            Err(unsupported())
        }
    }
    pub(super) fn new_runfiles(
        workspace: &Path,
        configuration: &SlugConfiguration,
        output: &ActionOutput,
        workspace_name: &str,
        entries: &[(String, Option<String>)],
        reserved: &SmallSet<&str>,
    ) -> io::Result<Self> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            let inner = linux::Staging::new_runfiles(
                workspace,
                configuration,
                output,
                workspace_name,
                entries,
                reserved,
            )?;
            Ok(Self {
                outputs: Arc::from([output.clone()]),
                root: super::configured_output::configured_output_root(workspace, configuration),
                inner,
            })
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (
                workspace,
                configuration,
                output,
                workspace_name,
                entries,
                reserved,
            );
            Err(unsupported())
        }
    }

    pub(super) fn new_alias(
        workspace: &Path,
        configuration: &SlugConfiguration,
        output: &ActionOutput,
        target: &str,
        reserved: &SmallSet<&str>,
    ) -> io::Result<Self> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            Ok(Self {
                outputs: Arc::from([output.clone()]),
                root: super::configured_output::configured_output_root(workspace, configuration),
                inner: linux::Staging::new_alias(
                    workspace,
                    configuration,
                    output,
                    target,
                    reserved,
                )?,
            })
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (workspace, configuration, output, target, reserved);
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
