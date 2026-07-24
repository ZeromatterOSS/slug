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
use dice::DiceComputations;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use starlark_map::sorted_map::SortedMap;

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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use dice::DetectCycles;
    use dice::Dice;
    use dice::InjectedKey;
    use dice::Key;
    use starlark_map::sorted_map::SortedMap;

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
}
