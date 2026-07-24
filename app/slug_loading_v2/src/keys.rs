/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dice::InjectedKey;
use dupe::Dupe;
use slug_identity_v2::ApparentLabel;
pub use slug_workspace_v2::WorkspaceFileKey;
pub use slug_workspace_v2::WorkspaceFileValue;
pub use slug_workspace_v2::WorkspaceSnapshot;
pub use slug_workspace_v2::WorkspaceSnapshotKey;
use starlark_map::sorted_map::SortedMap;

/// The direct kind of a directory entry observed before a DICE request.
///
/// This mirrors only the compact, portable part of Buck2's `FileType`: it
/// identifies symlinks rather than resolving them, and keeps special files
/// distinct from regular files and directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub enum WorkspaceDirectoryEntryKind {
    RegularFile,
    Directory,
    Symlink,
    Other,
}

/// One direct directory entry, sorted by `name` in a present directory value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub struct WorkspaceDirectoryEntry {
    pub name: CompactString,
    pub kind: WorkspaceDirectoryEntryKind,
}

/// An observed direct directory listing. Read failures are explicit rather
/// than being collapsed into absence.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum WorkspaceDirectoryValue {
    Present(Arc<[WorkspaceDirectoryEntry]>),
    Absent,
    ReadError(Arc<String>),
}

impl WorkspaceDirectoryValue {
    pub fn present(mut entries: Vec<WorkspaceDirectoryEntry>) -> Self {
        entries.sort_unstable_by(|left, right| left.name.cmp(&right.name));
        Self::Present(entries.into())
    }

    pub fn entries(&self) -> Option<&[WorkspaceDirectoryEntry]> {
        match self {
            Self::Present(entries) => Some(entries),
            Self::Absent | Self::ReadError(_) => None,
        }
    }
}

/// Immutable compact directory observations for one request revision.
///
/// The sorted map gives a deterministic snapshot while the `Arc` slices make
/// unchanged directory values cheap to retain and compare.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct WorkspaceDirectorySnapshot {
    pub directories: Arc<SortedMap<PathBuf, WorkspaceDirectoryValue>>,
}

impl WorkspaceDirectorySnapshot {
    pub fn empty() -> Self {
        Self {
            directories: Arc::new(SortedMap::new()),
        }
    }

    pub fn value(&self, directory: &std::path::Path) -> WorkspaceDirectoryValue {
        self.directories
            .get(directory)
            .cloned()
            .unwrap_or(WorkspaceDirectoryValue::Absent)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceDirectorySnapshotKey {
    pub workspace: PathBuf,
}

impl fmt::Display for WorkspaceDirectorySnapshotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "workspace-directory-snapshot:{}",
            self.workspace.display()
        )
    }
}

impl InjectedKey for WorkspaceDirectorySnapshotKey {
    type Value = Arc<WorkspaceDirectorySnapshot>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

/// The DICE propagation boundary for one normalized absolute directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceDirectoryKey {
    pub workspace: PathBuf,
    pub directory: PathBuf,
}

impl fmt::Display for WorkspaceDirectoryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-directory:{}", self.directory.display())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct BzlParseKey {
    pub workspace: std::path::PathBuf,
    pub path: std::path::PathBuf,
}

impl fmt::Display for BzlParseKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bzl-parse:{}", self.path.display())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct LoadLabelResolutionKey {
    pub workspace: std::path::PathBuf,
    pub requesting_package: std::path::PathBuf,
    pub load: String,
}

impl fmt::Display for LoadLabelResolutionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bzl-load-resolution:{}:{}",
            self.requesting_package.display(),
            self.load
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct BzlModuleEvalKey {
    pub workspace: std::path::PathBuf,
    pub path: std::path::PathBuf,
}

impl fmt::Display for BzlModuleEvalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bzl-module-eval:{}", self.path.display())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct PackageLoadKey {
    pub workspace: std::path::PathBuf,
    pub package: std::path::PathBuf,
}

impl fmt::Display for PackageLoadKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "package-load:{}", self.package.display())
    }
}

/// DICE identity for the direct-directory-derived contents of one root package.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct PackageListingKey {
    pub workspace: PathBuf,
    pub package: PathBuf,
}

impl fmt::Display for PackageListingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "package-listing:{}", self.package.display())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildFileTargetStub {
    pub label: ApparentLabel,
}
