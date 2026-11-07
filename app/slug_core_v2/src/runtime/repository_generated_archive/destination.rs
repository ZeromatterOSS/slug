//! Destination adapters share one decoder without ambient IO for rooted trees.
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;

use super::ArchiveMaterializationError;
use super::materialization;

pub(super) trait ArchiveDestination {
    fn validate_root(&self) -> Result<(), ArchiveMaterializationError>;
    fn ensure_directory(&self, relative: &str) -> Result<(), ArchiveMaterializationError>;
    fn open_regular(&self, relative: &str) -> Result<File, ArchiveMaterializationError>;
    fn set_mode(
        &self,
        relative: &str,
        file: &File,
        mode: u32,
    ) -> Result<(), ArchiveMaterializationError>;
    fn admit_file_size(&self, _size: u64) -> Result<(), ArchiveMaterializationError> {
        Ok(())
    }
}

pub(super) struct PathDestination<'a>(pub(super) &'a Path);

impl ArchiveDestination for PathDestination<'_> {
    fn validate_root(&self) -> Result<(), ArchiveMaterializationError> {
        let metadata = fs::symlink_metadata(self.0).map_err(|error| {
            materialization(format!("inspecting generated archive root: {error}"))
        })?;
        if !metadata.file_type().is_dir() {
            return Err(materialization(
                "generated archive root is not a real directory",
            ));
        }
        Ok(())
    }

    fn ensure_directory(&self, relative: &str) -> Result<(), ArchiveMaterializationError> {
        let parent = ensure_parent(self.0, relative)?;
        ensure_directory(
            &parent.join(relative.rsplit('/').next().unwrap_or(relative)),
            relative,
        )
    }

    fn open_regular(&self, relative: &str) -> Result<File, ArchiveMaterializationError> {
        let parent = ensure_parent(self.0, relative)?;
        let path = parent.join(relative.rsplit('/').next().unwrap_or(relative));
        admit_regular_target(&path, relative)?;
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .map_err(|error| materialization(format!("opening generated file {relative}: {error}")))
    }

    fn set_mode(
        &self,
        relative: &str,
        _file: &File,
        mode: u32,
    ) -> Result<(), ArchiveMaterializationError> {
        set_mode(&self.0.join(relative), mode)
    }
}

#[cfg(target_os = "linux")]
impl ArchiveDestination for crate::runtime::repository_root_io::RootIo<'_> {
    fn validate_root(&self) -> Result<(), ArchiveMaterializationError> {
        self.lstat(Path::new(""))
            .map(|_| ())
            .map_err(|error| materialization(format!("inspecting rooted archive root: {error}")))
    }

    fn ensure_directory(&self, relative: &str) -> Result<(), ArchiveMaterializationError> {
        self.ensure_directory(Path::new(relative), 0o777)
            .map_err(|error| {
                materialization(format!(
                    "creating rooted archive directory {relative}: {error}"
                ))
            })
    }

    fn open_regular(&self, relative: &str) -> Result<File, ArchiveMaterializationError> {
        let path = Path::new(relative);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            self.ensure_directory(parent, 0o777).map_err(|error| {
                materialization(format!(
                    "creating rooted archive parent {relative}: {error}"
                ))
            })?;
        }
        self.open_write_regular(
            path,
            crate::runtime::repository_root_io::WriteMode::ReplaceRegular,
        )
        .map_err(|error| {
            materialization(format!("opening rooted archive file {relative}: {error}"))
        })
    }

    fn set_mode(
        &self,
        relative: &str,
        file: &File,
        mode: u32,
    ) -> Result<(), ArchiveMaterializationError> {
        self.chmod_regular(file, mode).map_err(|error| {
            materialization(format!("setting rooted archive mode {relative}: {error}"))
        })
    }

    fn admit_file_size(&self, size: u64) -> Result<(), ArchiveMaterializationError> {
        self.admit_file_size(size)
            .map_err(|error| materialization(format!("rooted archive file size: {error}")))
    }
}

fn ensure_parent(root: &Path, path: &str) -> Result<PathBuf, ArchiveMaterializationError> {
    let mut parent = root.to_path_buf();
    if let Some((parents, _)) = path.rsplit_once('/') {
        for component in parents.split('/') {
            parent.push(component);
            ensure_directory(&parent, path)?;
        }
    }
    Ok(parent)
}

fn ensure_directory(path: &Path, subject: &str) -> Result<(), ArchiveMaterializationError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(materialization(format!(
            "generated tar file/directory collision at {subject}"
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|error| {
                materialization(format!("creating generated directory {subject}: {error}"))
            })
        }
        Err(error) => Err(materialization(format!(
            "inspecting generated directory {subject}: {error}"
        ))),
    }
}

fn admit_regular_target(path: &Path, subject: &str) -> Result<(), ArchiveMaterializationError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(materialization(format!(
            "generated tar file/directory collision at {subject}"
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(materialization(format!(
            "inspecting generated file {subject}: {error}"
        ))),
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), ArchiveMaterializationError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| {
        materialization(format!(
            "setting generated archive mode at {}: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), ArchiveMaterializationError> {
    Err(materialization(
        "generated archive executable modes are unsupported on this platform",
    ))
}
