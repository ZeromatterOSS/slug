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
        // Drain old rows while scanning instead of retaining two complete maps.
        // Canonicalization above must succeed before taking the reporting state.
        let (current, invalidated) = scan(&workspace, self.previous.take());
        // A successful root observation replaces the old reporting state;
        // local scan errors never become semantic command errors.
        self.previous = Some(current);
        Ok(invalidated)
    }
}

fn scan(
    workspace: &Path,
    mut previous: Option<BTreeMap<PathBuf, FileMetadata>>,
) -> (BTreeMap<PathBuf, FileMetadata>, usize) {
    let mut files = BTreeMap::new();
    let mut invalidated = 0;
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
            // A racing directory iterator may repeat a path. Advisory metrics
            // observe each file once; they never authorize semantic cache hits.
            if files.contains_key(&path) {
                continue;
            }
            let value = FileMetadata::new(&metadata);
            let old = previous
                .as_mut()
                .and_then(|files| files.remove_entry(&path));
            if previous.is_some() && old.as_ref().map(|(_, value)| value) != Some(&value) {
                invalidated += 1;
            }
            // Reuse the retained path allocation for overlapping entries too.
            let path = old.map_or(path, |(path, _)| path);
            files.insert(path, value);
        } else if metadata.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                directories.push((path, entries));
            }
        }
        // Symlinks and special files are not followed or opened. The iterator
        // stack retains directory state only along the current DFS ancestry.
    }
    invalidated += previous.map_or(0, |files| files.len());
    (files, invalidated)
}

#[cfg(test)]
mod tests;
