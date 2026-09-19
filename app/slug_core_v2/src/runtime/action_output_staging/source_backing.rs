//! Platform boundary for private durable runfiles source stages.
use slug_workspace_v2::FileContentDigest;

use super::*;

pub(in crate::runtime) fn source_backing_path(
    workspace: &Path,
    digest: FileContentDigest,
    permissions: i32,
) -> io::Result<PathBuf> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        linux::SourceBacking::target_path(workspace, digest, permissions)
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        let _ = (workspace, digest, permissions);
        Err(unsupported())
    }
}

#[derive(Debug)]
pub(in crate::runtime) struct SourceBacking {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    inner: linux::SourceBacking,
}
impl SourceBacking {
    pub(in crate::runtime) fn new(
        workspace: &Path,
        digest: FileContentDigest,
        permissions: i32,
        source: &mut File,
    ) -> io::Result<Self> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            Ok(Self {
                inner: linux::SourceBacking::new(workspace, digest, permissions, source)?,
            })
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (workspace, digest, permissions, source);
            Err(unsupported())
        }
    }
    pub(super) fn seal(&mut self) -> io::Result<()> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            self.inner.seal()
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            Err(unsupported())
        }
    }
    pub(super) fn preflight(&self) -> io::Result<()> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            self.inner.preflight()
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            Err(unsupported())
        }
    }
    pub(super) fn publish(&mut self) -> io::Result<()> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            self.inner.publish()
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            Err(unsupported())
        }
    }
}
