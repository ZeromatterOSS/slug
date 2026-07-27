/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License found in the LICENSE-APACHE file in the root directory of this
 * source tree. You may select the license that applies to you.
 */

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use starlark_map::sorted_map::SortedMap;

mod path_observation;
mod path_resolution;

pub use path_observation::EmptyPathObservationNeed;
pub use path_observation::InvalidPathDirectoryName;
pub use path_observation::NeedPathObservations;
pub use path_observation::NormalizedAbsolutePath;
pub use path_observation::PathDirectoryEntries;
pub use path_observation::PathDirectoryEntry;
pub use path_observation::PathDirectoryEntryKind;
pub use path_observation::PathDirectoryName;
pub use path_observation::PathIoErrorKind;
pub use path_observation::PathLstat;
pub use path_observation::PathNodeKind;
pub use path_observation::PathNormalizationError;
pub use path_observation::PathObservationDemand;
pub use path_observation::PathObservationEpoch;
pub use path_observation::PathObservationEpochError;
pub use path_observation::PathObservationEpochKey;
pub use path_observation::PathObservationError;
pub use path_observation::PathObservationInstanceId;
pub use path_observation::PathObservationKey;
pub use path_observation::PathObservationNamespace;
pub use path_observation::PathObservationOperation;
pub use path_observation::PathObservationResult;
pub use path_observation::PathOperationResult;
pub use path_observation::PathOutcome;
pub use path_observation::PathResult;
pub use path_resolution::PathDirectoryListing;
pub use path_resolution::PathDirectoryListingError;
pub use path_resolution::PathDirectoryListingKey;
pub use path_resolution::PathFileBytes;
pub use path_resolution::PathFileBytesError;
pub use path_resolution::PathFileBytesKey;
pub use path_resolution::PathResolutionChain;
pub use path_resolution::PathResolutionError;
pub use path_resolution::ResolvedPath;
pub use path_resolution::ResolvedPathKey;
pub use path_resolution::ResolvedPathState;
pub use path_resolution::ResolvedSymlink;

/// An observation supplied to the workspace DICE graph before loading begins.
///
/// The value deliberately distinguishes a file that does not exist from a
/// read failure. A missing BUILD file is meaningful to package loading, while
/// a permission or I/O failure must be reported instead of being cached as an
/// absence.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum WorkspaceFileValue {
    Present(Arc<String>),
    Absent,
    ReadError(Arc<String>),
}

/// The exact raw-byte state of an observed workspace file.
///
/// This stays separate from [`WorkspaceFileValue`]: a Starlark source reader
/// needs UTF-8 text, whereas repository materialization must preserve bytes
/// without treating non-UTF-8 input as a read failure.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum WorkspaceRawFileValue {
    Present(Arc<[u8]>),
    Absent,
    ReadError(Arc<String>),
}

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

/// Immutable, externally observed workspace state for one DICE revision.
///
/// A snapshot lets `WorkspaceFileKey` answer for any requested path: files not
/// represented in the observed state are explicitly absent rather than an
/// uninitialized injected key. `Arc` keeps the long-lived input cheap to clone.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct WorkspaceSnapshot {
    pub files: Arc<SortedMap<PathBuf, WorkspaceFileValue>>,
}

/// One raw-byte workspace snapshot injected for a request transaction.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct WorkspaceRawSnapshot {
    pub files: Arc<SortedMap<PathBuf, WorkspaceRawFileValue>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceSnapshotKey {
    pub workspace: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceRawSnapshotKey {
    pub workspace: PathBuf,
}

impl fmt::Display for WorkspaceSnapshotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-snapshot:{}", self.workspace.display())
    }
}

impl fmt::Display for WorkspaceRawSnapshotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-raw-snapshot:{}", self.workspace.display())
    }
}

impl InjectedKey for WorkspaceSnapshotKey {
    type Value = Arc<WorkspaceSnapshot>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

impl InjectedKey for WorkspaceRawSnapshotKey {
    type Value = Arc<WorkspaceRawSnapshot>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceFileKey {
    pub workspace: PathBuf,
    pub path: PathBuf,
}

/// Computes the raw observation for one exact workspace/path identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceRawFileKey {
    pub workspace: PathBuf,
    pub path: PathBuf,
}

impl fmt::Display for WorkspaceFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-file:{}", self.path.display())
    }
}

impl fmt::Display for WorkspaceRawFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-raw-file:{}", self.path.display())
    }
}

#[async_trait]
impl Key for WorkspaceFileKey {
    type Value = WorkspaceFileValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match ctx
            .compute(&WorkspaceSnapshotKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(snapshot) => snapshot
                .files
                .get(&self.path)
                .cloned()
                .unwrap_or(WorkspaceFileValue::Absent),
            Err(error) => WorkspaceFileValue::ReadError(Arc::new(format!(
                "reading workspace snapshot for {}: {error}",
                self.path.display()
            ))),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for WorkspaceRawFileKey {
    type Value = WorkspaceRawFileValue;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match ctx
            .compute(&WorkspaceRawSnapshotKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(snapshot) => snapshot
                .files
                .get(&self.path)
                .cloned()
                .unwrap_or(WorkspaceRawFileValue::Absent),
            Err(error) => WorkspaceRawFileValue::ReadError(Arc::new(format!(
                "reading raw workspace snapshot for {}: {error}",
                self.path.display()
            ))),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for WorkspaceDirectoryKey {
    type Value = WorkspaceDirectoryValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match ctx
            .compute(&WorkspaceDirectorySnapshotKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(snapshot) => snapshot
                .directories
                .get(&self.directory)
                .cloned()
                .unwrap_or(WorkspaceDirectoryValue::Absent),
            Err(error) => WorkspaceDirectoryValue::ReadError(Arc::new(format!(
                "reading workspace directory snapshot for {}: {error}",
                self.directory.display()
            ))),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use dice::DetectCycles;
    use dice::Dice;
    use dice::InjectedKey;
    use dice::Key;
    use starlark_map::sorted_map::SortedMap;

    use super::WorkspaceDirectoryEntry;
    use super::WorkspaceDirectoryEntryKind;
    use super::WorkspaceDirectoryKey;
    use super::WorkspaceDirectorySnapshot;
    use super::WorkspaceDirectorySnapshotKey;
    use super::WorkspaceDirectoryValue;
    use super::WorkspaceFileKey;
    use super::WorkspaceFileValue;
    use super::WorkspaceRawFileKey;
    use super::WorkspaceRawFileValue;
    use super::WorkspaceRawSnapshot;
    use super::WorkspaceRawSnapshotKey;
    use super::WorkspaceSnapshot;
    use super::WorkspaceSnapshotKey;

    #[test]
    fn snapshot_equality_is_structural() {
        let path = PathBuf::from("/workspace/MODULE.bazel");
        let present = Arc::new(WorkspaceSnapshot {
            files: Arc::new(SortedMap::from_iter([(
                path.clone(),
                WorkspaceFileValue::Present(Arc::new("module(name = \"root\")\n".to_owned())),
            )])),
        });
        let same = Arc::new(WorkspaceSnapshot {
            files: Arc::new(SortedMap::from_iter([(
                path,
                WorkspaceFileValue::Present(Arc::new("module(name = \"root\")\n".to_owned())),
            )])),
        });
        let absent = Arc::new(WorkspaceSnapshot {
            files: Arc::new(SortedMap::new()),
        });

        assert!(<WorkspaceSnapshotKey as InjectedKey>::equality(
            &present, &same
        ));
        assert!(!<WorkspaceSnapshotKey as InjectedKey>::equality(
            &present, &absent
        ));
        assert!(WorkspaceFileKey::equality(
            &WorkspaceFileValue::Absent,
            &WorkspaceFileValue::Absent
        ));
        assert!(!WorkspaceFileKey::equality(
            &WorkspaceFileValue::Absent,
            &WorkspaceFileValue::ReadError(Arc::new("permission denied".to_owned()))
        ));

        let raw_present = Arc::new(WorkspaceRawSnapshot {
            files: Arc::new(SortedMap::from_iter([(
                PathBuf::from("/workspace/raw.bin"),
                WorkspaceRawFileValue::Present(Arc::from(&b"\x00raw"[..])),
            )])),
        });
        let raw_same = Arc::new(WorkspaceRawSnapshot {
            files: Arc::new(SortedMap::from_iter([(
                PathBuf::from("/workspace/raw.bin"),
                WorkspaceRawFileValue::Present(Arc::from(&b"\x00raw"[..])),
            )])),
        });
        assert!(<WorkspaceRawSnapshotKey as InjectedKey>::equality(
            &raw_present,
            &raw_same
        ));
        assert!(!WorkspaceRawFileKey::equality(
            &WorkspaceRawFileValue::Absent,
            &WorkspaceRawFileValue::ReadError(Arc::new("permission denied".to_owned()))
        ));
    }

    #[tokio::test]
    async fn missing_path_propagates_as_absent() {
        let workspace = PathBuf::from("/workspace");
        let requested = workspace.join("missing.bzl");
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                (WorkspaceSnapshotKey {
                    workspace: workspace.clone(),
                }),
                Arc::new(WorkspaceSnapshot {
                    files: Arc::new(SortedMap::new()),
                }),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;

        assert_eq!(
            transaction
                .compute(&WorkspaceFileKey {
                    workspace,
                    path: requested,
                })
                .await
                .unwrap(),
            WorkspaceFileValue::Absent
        );
    }

    #[test]
    fn directory_values_sort_entries_and_preserve_structural_states() {
        let present = WorkspaceDirectoryValue::present(vec![
            WorkspaceDirectoryEntry {
                name: "zeta".into(),
                kind: WorkspaceDirectoryEntryKind::Symlink,
            },
            WorkspaceDirectoryEntry {
                name: "alpha".into(),
                kind: WorkspaceDirectoryEntryKind::RegularFile,
            },
        ]);
        let entries = present.entries().expect("present directory entries");
        assert_eq!(entries[0].name.as_str(), "alpha");
        assert_eq!(entries[1].name.as_str(), "zeta");
        assert_eq!(WorkspaceDirectoryValue::Absent.entries(), None);
        assert_eq!(
            WorkspaceDirectoryValue::ReadError(Arc::new("denied".to_owned())).entries(),
            None
        );

        let missing = PathBuf::from("/workspace/missing");
        assert_eq!(
            WorkspaceDirectorySnapshot::empty().value(&missing),
            WorkspaceDirectoryValue::Absent
        );
        let snapshot = Arc::new(WorkspaceDirectorySnapshot {
            directories: Arc::new(SortedMap::from_iter([(
                missing.clone(),
                WorkspaceDirectoryValue::Absent,
            )])),
        });
        let same = Arc::new(WorkspaceDirectorySnapshot {
            directories: Arc::new(SortedMap::from_iter([(
                missing,
                WorkspaceDirectoryValue::Absent,
            )])),
        });
        assert!(<WorkspaceDirectorySnapshotKey as InjectedKey>::equality(
            &snapshot, &same
        ));
    }

    #[tokio::test]
    async fn directory_key_reads_present_absent_and_read_error_from_one_snapshot() {
        let workspace = PathBuf::from("/workspace");
        let present = workspace.join("present");
        let denied = workspace.join("denied");
        let missing = workspace.join("missing");
        let present_value = WorkspaceDirectoryValue::present(vec![WorkspaceDirectoryEntry {
            name: "BUILD.bazel".into(),
            kind: WorkspaceDirectoryEntryKind::RegularFile,
        }]);
        let denied_value = WorkspaceDirectoryValue::ReadError(Arc::new("denied".to_owned()));
        let snapshot = Arc::new(WorkspaceDirectorySnapshot {
            directories: Arc::new(SortedMap::from_iter([
                (present.clone(), present_value.clone()),
                (denied.clone(), denied_value.clone()),
            ])),
        });
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                WorkspaceDirectorySnapshotKey {
                    workspace: workspace.clone(),
                },
                snapshot,
            )])
            .unwrap();
        let mut transaction = updater.commit().await;

        for (directory, expected) in [
            (present, present_value),
            (denied, denied_value),
            (missing, WorkspaceDirectoryValue::Absent),
        ] {
            assert_eq!(
                transaction
                    .compute(&WorkspaceDirectoryKey {
                        workspace: workspace.clone(),
                        directory,
                    })
                    .await
                    .unwrap(),
                expected
            );
        }
    }
}
