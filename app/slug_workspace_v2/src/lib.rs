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

/// Immutable, externally observed workspace state for one DICE revision.
///
/// A snapshot lets `WorkspaceFileKey` answer for any requested path: files not
/// represented in the observed state are explicitly absent rather than an
/// uninitialized injected key. `Arc` keeps the long-lived input cheap to clone.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct WorkspaceSnapshot {
    pub files: Arc<SortedMap<PathBuf, WorkspaceFileValue>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceSnapshotKey {
    pub workspace: PathBuf,
}

impl fmt::Display for WorkspaceSnapshotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-snapshot:{}", self.workspace.display())
    }
}

impl InjectedKey for WorkspaceSnapshotKey {
    type Value = Arc<WorkspaceSnapshot>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceFileKey {
    pub workspace: PathBuf,
    pub path: PathBuf,
}

impl fmt::Display for WorkspaceFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-file:{}", self.path.display())
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
