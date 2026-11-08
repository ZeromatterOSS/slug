//! The admitted BCR parser selects every operation; destinations confer no
//! source authority. Rooted writes never join/reopen an ambient workspace path.
use std::fs::File;
use std::path::Path;

use super::ArchiveMaterializationError;
use super::materialization;

pub(super) trait Destination {
    fn directory(&self, path: &str) -> Result<(), ArchiveMaterializationError>;
    fn parent(&self, path: &str) -> Result<(), ArchiveMaterializationError>;
    fn regular(&self, path: &str) -> Result<File, ArchiveMaterializationError>;
    fn mode(&self, path: &str, file: &File, mode: u32) -> Result<(), ArchiveMaterializationError>;
    fn link(&self, path: &str, target: &str) -> Result<(), ArchiveMaterializationError>;
}
pub(super) struct PathDestination<'a>(pub(super) &'a Path);
impl Destination for PathDestination<'_> {
    fn directory(&self, path: &str) -> Result<(), ArchiveMaterializationError> {
        let destination = self.0.join(path);
        std::fs::create_dir_all(&destination)
            .map_err(|e| materialization(format!("creating selected BCR directory {path}: {e}")))?;
        super::set_mode(&destination, 0o755)
    }
    fn parent(&self, path: &str) -> Result<(), ArchiveMaterializationError> {
        let destination = self.0.join(path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                materialization(format!("creating selected BCR parent for {path}: {e}"))
            })?;
        }
        Ok(())
    }
    fn regular(&self, path: &str) -> Result<File, ArchiveMaterializationError> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.0.join(path))
            .map_err(|e| materialization(format!("creating selected BCR file {path}: {e}")))
    }
    fn mode(&self, path: &str, _: &File, mode: u32) -> Result<(), ArchiveMaterializationError> {
        super::set_mode(&self.0.join(path), mode)
    }
    fn link(&self, path: &str, target: &str) -> Result<(), ArchiveMaterializationError> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, self.0.join(path)).map_err(|e| {
                materialization(format!("creating selected BCR symbolic link {path}: {e}"))
            })
        }
        #[cfg(not(unix))]
        {
            let _ = (path, target);
            Err(materialization(
                "selected BCR symbolic links are unsupported on this platform",
            ))
        }
    }
}

#[cfg(target_os = "linux")]
impl Destination for crate::runtime::repository_root_io::RootIo<'_> {
    fn directory(&self, path: &str) -> Result<(), ArchiveMaterializationError> {
        self.ensure_directory(Path::new(path), 0o755)
            .and_then(|_| self.set_mode(Path::new(path), 0o755))
            .map_err(|e| materialization(format!("creating selected BCR directory {path}: {e}")))
    }
    fn parent(&self, path: &str) -> Result<(), ArchiveMaterializationError> {
        let parent = Path::new(path).parent().unwrap_or(Path::new(""));
        self.ensure_directory(parent, 0o777)
            .map_err(|e| materialization(format!("creating selected BCR parent for {path}: {e}")))
    }
    fn regular(&self, path: &str) -> Result<File, ArchiveMaterializationError> {
        self.open_write_regular(
            Path::new(path),
            crate::runtime::repository_root_io::WriteMode::CreateNew,
        )
        .map_err(|e| materialization(format!("creating selected BCR file {path}: {e}")))
    }
    fn mode(&self, path: &str, file: &File, mode: u32) -> Result<(), ArchiveMaterializationError> {
        self.chmod_regular(file, mode)
            .map_err(|e| materialization(format!("setting selected BCR mode {path}: {e}")))
    }
    fn link(&self, path: &str, target: &str) -> Result<(), ArchiveMaterializationError> {
        self.symlink_relative(Path::new(path), Path::new(target))
            .map_err(|e| {
                materialization(format!("creating selected BCR symbolic link {path}: {e}"))
            })
    }
}
