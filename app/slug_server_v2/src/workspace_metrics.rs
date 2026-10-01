//! Advisory file-change counts, independent of semantic DICE observations.
//!
//! The retained map contains metadata only. Touches may count, and content
//! changes with indistinguishable metadata may be missed. Neither outcome
//! supplies semantic bytes or controls cache validity.

use std::collections::BTreeMap;
use std::fs::Metadata;
use std::fs::ReadDir;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::Context;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FileMetadata {
    length: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
}

impl FileMetadata {
    fn new(metadata: &Metadata) -> Self {
        Self {
            length: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(unix)]
            changed_seconds: metadata.ctime(),
            #[cfg(unix)]
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

#[derive(Default)]
pub(super) struct FilesystemObservationAdapter {
    pub(super) previous: Option<BTreeMap<PathBuf, FileMetadata>>,
}

impl FilesystemObservationAdapter {
    pub(super) fn observe(&mut self, workspace: &Path) -> anyhow::Result<usize> {
        let workspace = workspace
            .canonicalize()
            .with_context(|| format!("canonicalizing workspace {}", workspace.display()))?;
        let current = scan(&workspace);
        let invalidated = self.previous.as_ref().map_or(0, |previous| {
            previous
                .iter()
                .filter(|(path, value)| current.get(*path) != Some(*value))
                .count()
                + current
                    .keys()
                    .filter(|path| !previous.contains_key(*path))
                    .count()
        });
        // A successful root observation replaces the old reporting state;
        // local scan errors never become semantic command errors.
        self.previous = Some(current);
        Ok(invalidated)
    }
}

fn scan(workspace: &Path) -> BTreeMap<PathBuf, FileMetadata> {
    let mut files = BTreeMap::new();
    let mut directories: Vec<(PathBuf, ReadDir)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(workspace) {
        directories.push((workspace.to_path_buf(), entries));
    }
    while let Some((directory, entries)) = directories.last_mut() {
        let entry = match entries.next() {
            Some(Ok(entry)) => entry,
            Some(Err(_)) => continue,
            None => {
                directories.pop();
                continue;
            }
        };
        if directory.as_path() == workspace && entry.file_name() == "bazel-out" {
            continue;
        }
        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_file() {
            files.insert(path, FileMetadata::new(&metadata));
        } else if metadata.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                directories.push((path, entries));
            }
        }
        // Symlinks and special files are not followed or opened. The iterator
        // stack retains directory state only along the current DFS ancestry.
    }
    files
}

#[cfg(test)]
mod tests;
