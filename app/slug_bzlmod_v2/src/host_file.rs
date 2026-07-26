/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the later Host root and registry packets.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationError;
use slug_workspace_v2::PathObservationKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::PathResolutionError;
use slug_workspace_v2::PathResult;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathState;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostFileBytes {
    Present(Arc<[u8]>),
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostFileError {
    Observation {
        logical_path: NormalizedAbsolutePath,
        operation: PathObservationOperation,
        error: PathObservationError,
    },
    InconsistentState {
        logical_path: NormalizedAbsolutePath,
        operation: PathObservationOperation,
        before: Option<PathLstat>,
        after: Option<PathLstat>,
    },
    WrongKind {
        logical_path: NormalizedAbsolutePath,
        actual: PathNodeKind,
    },
    Cycle {
        logical_path: NormalizedAbsolutePath,
    },
    InfiniteExpansion {
        logical_path: NormalizedAbsolutePath,
    },
}

impl HostFileError {
    fn from_resolution(logical_path: NormalizedAbsolutePath, error: PathResolutionError) -> Self {
        match error {
            PathResolutionError::Observation { demand, error, .. } => Self::Observation {
                logical_path,
                operation: demand.operation(),
                error,
            },
            PathResolutionError::InconsistentState {
                demand,
                before,
                after,
                ..
            } => Self::InconsistentState {
                logical_path,
                operation: demand.operation(),
                before,
                after,
            },
            PathResolutionError::Cycle { .. } => Self::Cycle { logical_path },
            PathResolutionError::InfiniteExpansion { .. } => {
                Self::InfiniteExpansion { logical_path }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostFileBytesKey {
    logical_path: NormalizedAbsolutePath,
}

impl HostFileBytesKey {
    pub(crate) fn new(logical_path: NormalizedAbsolutePath) -> Self {
        Self { logical_path }
    }

    pub(crate) fn logical_path(&self) -> &NormalizedAbsolutePath {
        &self.logical_path
    }
}

impl fmt::Display for HostFileBytesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bzlmod-host-file-bytes:{:?}",
            self.logical_path.as_path()
        )
    }
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("bzlmod Host-file DICE invariant failed: {error:?}"))
}

#[async_trait]
impl Key for HostFileBytesKey {
    type Value = PathResult<HostFileBytes, HostFileError>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let resolved = dice_invariant(
            ctx.compute(&ResolvedPathKey::new(
                PathObservationNamespace::Host,
                self.logical_path.dupe(),
            ))
            .await,
        );
        let resolved = match resolved {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(Err(error)) => {
                return PathOutcome::Complete(Err(HostFileError::from_resolution(
                    self.logical_path.dupe(),
                    error,
                )));
            }
            PathOutcome::Complete(Ok(resolved)) => resolved,
        };
        let lstat = match resolved.state() {
            ResolvedPathState::Missing => {
                return PathOutcome::Complete(Ok(HostFileBytes::Missing));
            }
            ResolvedPathState::Present(lstat)
                if matches!(
                    lstat.kind(),
                    PathNodeKind::RegularFile | PathNodeKind::SpecialFile
                ) =>
            {
                lstat
            }
            ResolvedPathState::Present(lstat) => {
                return PathOutcome::Complete(Err(HostFileError::WrongKind {
                    logical_path: self.logical_path.dupe(),
                    actual: lstat.kind(),
                }));
            }
        };

        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            resolved.real_path().dupe(),
            PathObservationOperation::FileBytes,
        );
        let observed = dice_invariant(ctx.compute(&PathObservationKey::new(demand.dupe())).await);
        match observed {
            PathOutcome::Need(need) => PathOutcome::Need(need),
            PathOutcome::Complete(result) => match result.as_ref() {
                PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) => {
                    PathOutcome::Complete(Ok(HostFileBytes::Present(bytes.dupe())))
                }
                PathObservationResult::FileBytes(PathOperationResult::Missing) => {
                    PathOutcome::Complete(Err(HostFileError::InconsistentState {
                        logical_path: self.logical_path.dupe(),
                        operation: demand.operation(),
                        before: Some(lstat),
                        after: None,
                    }))
                }
                PathObservationResult::FileBytes(PathOperationResult::Error(error)) => {
                    PathOutcome::Complete(Err(HostFileError::Observation {
                        logical_path: self.logical_path.dupe(),
                        operation: demand.operation(),
                        error: *error,
                    }))
                }
                PathObservationResult::Lstat(_)
                | PathObservationResult::ReadLink(_)
                | PathObservationResult::DirectoryEntries(_) => {
                    unreachable!("FileBytes demand must return a FileBytes observation")
                }
            },
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use allocative::Allocative;
    use async_trait::async_trait;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DiceComputations;
    use dice::DiceProjectionComputations;
    use dice::DiceTransaction;
    use dice::Key;
    use dice::ProjectionKey;
    use dice_futures::cancellation::CancellationContext;
    use dupe::Dupe;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathIoErrorKind;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationError;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::PathOutcome;
    use slug_workspace_v2::PathResult;

    use super::HostFileBytes;
    use super::HostFileBytesKey;
    use super::HostFileError;
    use super::dice_invariant;

    type ScriptEntry = (PathObservationDemand, PathObservationResult);

    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }

    fn lstat_variant(kind: PathNodeKind, variant: i64) -> PathLstat {
        PathLstat::new(kind, variant, variant + 1, variant + 2, variant + 3, 0o755)
    }

    fn demand(value: &str, operation: PathObservationOperation) -> PathObservationDemand {
        PathObservationDemand::new(PathObservationNamespace::Host, path(value), operation)
    }

    fn lstat_result(value: &str, result: PathOperationResult<PathLstat>) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(result),
        )
    }

    fn present(value: &str, kind: PathNodeKind, variant: i64) -> ScriptEntry {
        lstat_result(
            value,
            PathOperationResult::Present(lstat_variant(kind, variant)),
        )
    }

    fn missing(value: &str) -> ScriptEntry {
        lstat_result(value, PathOperationResult::Missing)
    }

    fn read_link(value: &str, target: &str) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::ReadLink),
            PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(PathBuf::from(
                target,
            )))),
        )
    }

    fn file_bytes(value: &str, result: PathOperationResult<Arc<[u8]>>) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(result),
        )
    }

    fn direct_script(
        kind: PathNodeKind,
        variant: i64,
        bytes: Option<PathOperationResult<Arc<[u8]>>>,
    ) -> Vec<ScriptEntry> {
        let mut script = vec![
            present("/", PathNodeKind::Directory, 0),
            present("/file", kind, variant),
        ];
        if let Some(bytes) = bytes {
            script.push(file_bytes("/file", bytes));
        }
        script
    }

    fn linked_script(
        target: &str,
        kind: PathNodeKind,
        variant: i64,
        bytes: PathOperationResult<Arc<[u8]>>,
    ) -> Vec<ScriptEntry> {
        vec![
            present("/", PathNodeKind::Directory, 0),
            present("/entry", PathNodeKind::Symlink, 1),
            read_link("/entry", target),
            present(target, kind, variant),
            file_bytes(target, bytes),
        ]
    }

    fn epoch(script: &[ScriptEntry]) -> PathObservationEpoch {
        PathObservationEpoch::new(
            script
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap()
    }

    async fn cumulative(
        logical_path: &str,
        script: &[ScriptEntry],
    ) -> Result<HostFileBytes, HostFileError> {
        let key = HostFileBytesKey::new(path(logical_path));
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater().commit().await;
        for prefix_len in 0..=script.len() {
            let mut updater = transaction.into_updater();
            updater
                .changed_to(vec![(
                    PathObservationEpochKey,
                    epoch(&script[..prefix_len]),
                )])
                .unwrap();
            transaction = updater.commit().await;
            let outcome = transaction.compute(&key).await.unwrap();
            if prefix_len < script.len() {
                let PathOutcome::Need(need) = &outcome else {
                    panic!("Host byte projection completed before script prefix {prefix_len}");
                };
                assert_eq!(need.demands(), &[script[prefix_len].0.dupe()]);
                assert!(!HostFileBytesKey::validity(&outcome));
                assert!(!HostFileBytesKey::equality(&outcome, &outcome));
            } else {
                let PathOutcome::Complete(result) = outcome else {
                    panic!("complete Host byte script still needs observations");
                };
                assert!(HostFileBytesKey::validity(&PathOutcome::Complete(
                    result.dupe()
                )));
                assert!(HostFileBytesKey::equality(
                    &PathOutcome::Complete(result.dupe()),
                    &PathOutcome::Complete(result.dupe())
                ));
                return result;
            }
        }
        unreachable!("inclusive prefix loop reaches the complete script")
    }

    #[derive(Debug, Clone, Allocative, Dupe)]
    struct HostFileCounterKey {
        file: HostFileBytesKey,
        #[allocative(skip)]
        counter: Arc<AtomicUsize>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
    struct HostFileProjectionKey;

    impl fmt::Display for HostFileProjectionKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("bzlmod-host-file-value-projection")
        }
    }

    impl ProjectionKey for HostFileProjectionKey {
        type DeriveFromKey = HostFileBytesKey;
        type Value = PathResult<HostFileBytes, HostFileError>;

        fn compute(
            &self,
            derive_from: &<Self::DeriveFromKey as Key>::Value,
            _ctx: &DiceProjectionComputations,
        ) -> Self::Value {
            derive_from.dupe()
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            HostFileBytesKey::equality(x, y)
        }

        fn validity(value: &Self::Value) -> bool {
            HostFileBytesKey::validity(value)
        }
    }

    impl PartialEq for HostFileCounterKey {
        fn eq(&self, other: &Self) -> bool {
            self.file == other.file && Arc::ptr_eq(&self.counter, &other.counter)
        }
    }

    impl Eq for HostFileCounterKey {}

    impl Hash for HostFileCounterKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.file.hash(state);
            Arc::as_ptr(&self.counter).hash(state);
        }
    }

    impl fmt::Display for HostFileCounterKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "bzlmod-host-file-counter:{}:{:p}",
                self.file,
                Arc::as_ptr(&self.counter)
            )
        }
    }

    #[async_trait]
    impl Key for HostFileCounterKey {
        type Value = PathOutcome<usize>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            let opaque = dice_invariant(ctx.compute_opaque(&self.file).await);
            dice_invariant(ctx.projection(&opaque, &HostFileProjectionKey))
                .map(|_| self.counter.fetch_add(1, Ordering::SeqCst) + 1)
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }

        fn validity(value: &Self::Value) -> bool {
            value.is_complete()
        }
    }

    async fn update(transaction: DiceTransaction, script: &[ScriptEntry]) -> DiceTransaction {
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch(script))])
            .unwrap();
        updater.commit().await
    }

    async fn complete(
        transaction: &mut DiceTransaction,
        key: &HostFileBytesKey,
    ) -> Result<HostFileBytes, HostFileError> {
        let PathOutcome::Complete(result) = transaction.compute(key).await.unwrap() else {
            panic!("complete epoch returned a Host observation Need");
        };
        result
    }

    #[tokio::test]
    async fn host_file_bytes_cumulative_projection_accepts_bazel_file_kinds() {
        let raw: Arc<[u8]> = Arc::from(&b"\xff\0MODULE"[..]);
        assert_eq!(
            cumulative(
                "/file",
                &direct_script(
                    PathNodeKind::RegularFile,
                    10,
                    Some(PathOperationResult::Present(raw.dupe())),
                ),
            )
            .await,
            Ok(HostFileBytes::Present(raw.dupe()))
        );
        assert_eq!(
            cumulative(
                "/file",
                &direct_script(
                    PathNodeKind::SpecialFile,
                    20,
                    Some(PathOperationResult::Present(raw.dupe())),
                ),
            )
            .await,
            Ok(HostFileBytes::Present(raw.dupe()))
        );
        assert_eq!(
            cumulative(
                "/entry",
                &linked_script(
                    "/target",
                    PathNodeKind::SpecialFile,
                    30,
                    PathOperationResult::Present(raw.dupe()),
                ),
            )
            .await,
            Ok(HostFileBytes::Present(raw.dupe()))
        );
        assert_eq!(
            cumulative("/file", &direct_script(PathNodeKind::Directory, 40, None),).await,
            Err(HostFileError::WrongKind {
                logical_path: path("/file"),
                actual: PathNodeKind::Directory,
            })
        );
        assert_eq!(
            cumulative(
                "/file",
                &[present("/", PathNodeKind::Directory, 0), missing("/file")],
            )
            .await,
            Ok(HostFileBytes::Missing)
        );

        let lstat = lstat_variant(PathNodeKind::RegularFile, 50);
        assert_eq!(
            cumulative(
                "/file",
                &direct_script(
                    PathNodeKind::RegularFile,
                    50,
                    Some(PathOperationResult::Missing),
                ),
            )
            .await,
            Err(HostFileError::InconsistentState {
                logical_path: path("/file"),
                operation: PathObservationOperation::FileBytes,
                before: Some(lstat),
                after: None,
            })
        );
        let io = PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        };
        assert_eq!(
            cumulative(
                "/file",
                &direct_script(
                    PathNodeKind::SpecialFile,
                    60,
                    Some(PathOperationResult::Error(io)),
                ),
            )
            .await,
            Err(HostFileError::Observation {
                logical_path: path("/file"),
                operation: PathObservationOperation::FileBytes,
                error: io,
            })
        );
        assert_eq!(
            cumulative(
                "/self",
                &[
                    present("/", PathNodeKind::Directory, 0),
                    present("/self", PathNodeKind::Symlink, 70),
                    read_link("/self", "self"),
                ],
            )
            .await,
            Err(HostFileError::Cycle {
                logical_path: path("/self"),
            })
        );
        assert_eq!(
            cumulative(
                "/prefix",
                &[
                    present("/", PathNodeKind::Directory, 0),
                    present("/prefix", PathNodeKind::Symlink, 80),
                    read_link("/prefix", "a"),
                    present("/a", PathNodeKind::Symlink, 81),
                    read_link("/a", "a/child"),
                ],
            )
            .await,
            Err(HostFileError::InfiniteExpansion {
                logical_path: path("/prefix"),
            })
        );
    }

    #[tokio::test]
    async fn host_file_bytes_semantic_lifecycle_prunes_physical_identity_and_restores() {
        let bytes_a: Arc<[u8]> = Arc::from(&b"A\xff"[..]);
        let bytes_b: Arc<[u8]> = Arc::from(&b"B\0"[..]);
        let key = HostFileBytesKey::new(path("/entry"));
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_key = HostFileCounterKey {
            file: key.dupe(),
            counter: counter.dupe(),
        };
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater().commit().await;

        let state_a = linked_script(
            "/physical-a",
            PathNodeKind::RegularFile,
            100,
            PathOperationResult::Present(bytes_a.dupe()),
        );
        transaction = update(transaction, &state_a).await;
        let initial = complete(&mut transaction, &key).await;
        assert_eq!(initial, Ok(HostFileBytes::Present(bytes_a.dupe())));
        assert!(matches!(
            transaction.compute(&counter_key).await.unwrap(),
            PathOutcome::Complete(1)
        ));

        let equivalent_a = linked_script(
            "/physical-equivalent",
            PathNodeKind::SpecialFile,
            200,
            PathOperationResult::Present(bytes_a.dupe()),
        );
        transaction = update(transaction, &equivalent_a).await;
        assert_eq!(complete(&mut transaction, &key).await, initial);
        assert!(matches!(
            transaction.compute(&counter_key).await.unwrap(),
            PathOutcome::Complete(1)
        ));

        let state_b = linked_script(
            "/physical-b",
            PathNodeKind::RegularFile,
            300,
            PathOperationResult::Present(bytes_b.dupe()),
        );
        transaction = update(transaction, &state_b).await;
        assert_eq!(
            complete(&mut transaction, &key).await,
            Ok(HostFileBytes::Present(bytes_b))
        );
        assert!(matches!(
            transaction.compute(&counter_key).await.unwrap(),
            PathOutcome::Complete(2)
        ));

        let missing_state = vec![present("/", PathNodeKind::Directory, 0), missing("/entry")];
        transaction = update(transaction, &missing_state).await;
        assert_eq!(
            complete(&mut transaction, &key).await,
            Ok(HostFileBytes::Missing)
        );
        assert!(matches!(
            transaction.compute(&counter_key).await.unwrap(),
            PathOutcome::Complete(3)
        ));

        let io = PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        };
        let error_state = linked_script(
            "/physical-error",
            PathNodeKind::SpecialFile,
            400,
            PathOperationResult::Error(io),
        );
        transaction = update(transaction, &error_state).await;
        assert!(matches!(
            complete(&mut transaction, &key).await,
            Err(HostFileError::Observation {
                logical_path,
                operation: PathObservationOperation::FileBytes,
                error,
            }) if logical_path == path("/entry") && error == io
        ));
        assert!(matches!(
            transaction.compute(&counter_key).await.unwrap(),
            PathOutcome::Complete(4)
        ));

        let equivalent_error_state = linked_script(
            "/physical-error-equivalent",
            PathNodeKind::RegularFile,
            500,
            PathOperationResult::Error(io),
        );
        transaction = update(transaction, &equivalent_error_state).await;
        assert!(matches!(
            complete(&mut transaction, &key).await,
            Err(HostFileError::Observation {
                logical_path,
                operation: PathObservationOperation::FileBytes,
                error,
            }) if logical_path == path("/entry") && error == io
        ));
        assert!(matches!(
            transaction.compute(&counter_key).await.unwrap(),
            PathOutcome::Complete(4)
        ));

        transaction = update(transaction, &state_a).await;
        let restored = complete(&mut transaction, &key).await;
        assert_eq!(restored, initial);
        assert!(matches!(
            transaction.compute(&counter_key).await.unwrap(),
            PathOutcome::Complete(5)
        ));
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }
}
