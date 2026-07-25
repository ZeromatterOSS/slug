#![allow(dead_code)]
// This private neutral kernel is deliberately callerless until the Unix and
// Windows adapter packets land.

/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 */

use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochError;
use slug_workspace_v2::PathObservationError;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

#[derive(Debug, PartialEq, Eq, Allocative)]
struct RetainedMaterializationRoot {
    instance: PathObservationInstanceId,
    root: NormalizedAbsolutePath,
}

/// A compact authority snapshot whose lifetime is tied to the real owner.
///
/// This intentionally has no `Clone`, `Dupe`, map, or interner. The future
/// caller must retain the materializer owner for the entire observation call.
#[derive(Debug, Allocative)]
struct RetainedMaterializationRoots<'owner> {
    entries: Arc<[RetainedMaterializationRoot]>,
    owner: PhantomData<&'owner ()>,
}

#[derive(Debug, PartialEq, Eq)]
enum PathObservationKernelError {
    ZeroRetainedInstance,
    DuplicateRetainedInstance(PathObservationInstanceId),
    DuplicateDemand(PathObservationDemand),
    ZeroDemandInstance(PathObservationDemand),
    UnknownDemandInstance(PathObservationDemand),
    UnsupportedLstat,
    Epoch(PathObservationEpochError),
}

impl<'owner> RetainedMaterializationRoots<'owner> {
    fn new<T>(
        owner: &'owner T,
        entries: impl IntoIterator<Item = (PathObservationInstanceId, NormalizedAbsolutePath)>,
    ) -> Result<Self, PathObservationKernelError> {
        let _ = owner;
        let mut entries = entries
            .into_iter()
            .map(|(instance, root)| RetainedMaterializationRoot { instance, root })
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.instance);
        if entries.iter().any(|entry| entry.instance.value() == 0) {
            return Err(PathObservationKernelError::ZeroRetainedInstance);
        }
        if let Some(instance) = entries
            .windows(2)
            .find(|pair| pair[0].instance == pair[1].instance)
            .map(|pair| pair[0].instance)
        {
            return Err(PathObservationKernelError::DuplicateRetainedInstance(
                instance,
            ));
        }
        Ok(Self {
            entries: Arc::from(entries),
            owner: PhantomData,
        })
    }

    fn authorizes(&self, instance: PathObservationInstanceId) -> bool {
        self.entries
            .binary_search_by_key(&instance, |entry| entry.instance)
            .is_ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrimaryFailure {
    Refine(PathObservationError),
    Final(PathObservationError),
}

trait ObservationOperations {
    fn supports_lstat(&mut self) -> bool;

    fn lstat(&mut self, path: &NormalizedAbsolutePath) -> PathOperationResult<PathLstat>;

    fn read_link(&mut self, path: &NormalizedAbsolutePath) -> Result<Arc<PathBuf>, PrimaryFailure>;

    fn file_bytes(&mut self, path: &NormalizedAbsolutePath) -> Result<Arc<[u8]>, PrimaryFailure>;

    fn directory_entries(
        &mut self,
        path: &NormalizedAbsolutePath,
    ) -> Result<PathDirectoryEntries, PrimaryFailure>;
}

fn observe_with(
    retained: &RetainedMaterializationRoots<'_>,
    demands: impl IntoIterator<Item = PathObservationDemand>,
    operations: &mut impl ObservationOperations,
) -> Result<PathObservationEpoch, PathObservationKernelError> {
    let mut demands = demands.into_iter().collect::<Vec<_>>();
    demands.sort_unstable();

    if let Some(duplicate) = demands
        .windows(2)
        .find(|pair| pair[0] == pair[1])
        .map(|pair| pair[0].clone())
    {
        return Err(PathObservationKernelError::DuplicateDemand(duplicate));
    }

    for demand in &demands {
        if let PathObservationNamespace::Materialization(instance) = demand.namespace() {
            if instance.value() == 0 {
                return Err(PathObservationKernelError::ZeroDemandInstance(
                    demand.clone(),
                ));
            }
            if !retained.authorizes(instance) {
                return Err(PathObservationKernelError::UnknownDemandInstance(
                    demand.clone(),
                ));
            }
        }
    }

    if demands
        .iter()
        .any(|demand| demand.operation() == PathObservationOperation::Lstat)
        && !operations.supports_lstat()
    {
        return Err(PathObservationKernelError::UnsupportedLstat);
    }

    let observations = demands.into_iter().map(|demand| {
        let result = observe_one(&demand, operations);
        (demand, result)
    });
    PathObservationEpoch::new(observations).map_err(PathObservationKernelError::Epoch)
}

fn observe_one(
    demand: &PathObservationDemand,
    operations: &mut impl ObservationOperations,
) -> PathObservationResult {
    match demand.operation() {
        PathObservationOperation::Lstat => {
            PathObservationResult::Lstat(operations.lstat(demand.path()))
        }
        PathObservationOperation::ReadLink => {
            PathObservationResult::ReadLink(match operations.read_link(demand.path()) {
                Ok(target) => PathOperationResult::Present(target),
                Err(PrimaryFailure::Final(error)) => PathOperationResult::Error(error),
                Err(PrimaryFailure::Refine(error)) => {
                    refine_read_link(error, operations.lstat(demand.path()))
                }
            })
        }
        PathObservationOperation::FileBytes => {
            PathObservationResult::FileBytes(match operations.file_bytes(demand.path()) {
                Ok(bytes) => PathOperationResult::Present(bytes),
                Err(PrimaryFailure::Final(error)) => PathOperationResult::Error(error),
                Err(PrimaryFailure::Refine(error)) => {
                    refine_file_bytes(error, operations.lstat(demand.path()))
                }
            })
        }
        PathObservationOperation::DirectoryEntries => PathObservationResult::DirectoryEntries(
            match operations.directory_entries(demand.path()) {
                Ok(entries) => PathOperationResult::Present(entries),
                Err(PrimaryFailure::Final(error)) => PathOperationResult::Error(error),
                Err(PrimaryFailure::Refine(error)) => {
                    refine_directory_entries(error, operations.lstat(demand.path()))
                }
            },
        ),
    }
}

fn refine_read_link(
    original: PathObservationError,
    auxiliary: PathOperationResult<PathLstat>,
) -> PathOperationResult<Arc<PathBuf>> {
    match auxiliary {
        PathOperationResult::Missing => PathOperationResult::Missing,
        PathOperationResult::Present(lstat) if lstat.kind() != PathNodeKind::Symlink => {
            PathOperationResult::Error(PathObservationError::WrongKind {
                expected: PathNodeKind::Symlink,
                actual: lstat.kind(),
            })
        }
        PathOperationResult::Present(_) | PathOperationResult::Error(_) => {
            PathOperationResult::Error(original)
        }
    }
}

fn refine_file_bytes(
    original: PathObservationError,
    auxiliary: PathOperationResult<PathLstat>,
) -> PathOperationResult<Arc<[u8]>> {
    match auxiliary {
        PathOperationResult::Missing => PathOperationResult::Missing,
        PathOperationResult::Present(lstat) if lstat.kind() == PathNodeKind::Directory => {
            PathOperationResult::Error(PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: PathNodeKind::Directory,
            })
        }
        PathOperationResult::Present(_) | PathOperationResult::Error(_) => {
            PathOperationResult::Error(original)
        }
    }
}

fn refine_directory_entries(
    original: PathObservationError,
    auxiliary: PathOperationResult<PathLstat>,
) -> PathOperationResult<PathDirectoryEntries> {
    match auxiliary {
        PathOperationResult::Missing => PathOperationResult::Missing,
        PathOperationResult::Present(lstat) if lstat.kind() != PathNodeKind::Directory => {
            PathOperationResult::Error(PathObservationError::WrongKind {
                expected: PathNodeKind::Directory,
                actual: lstat.kind(),
            })
        }
        PathOperationResult::Present(_) | PathOperationResult::Error(_) => {
            PathOperationResult::Error(original)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::path::Path;

    use slug_workspace_v2::PathDirectoryName;
    use slug_workspace_v2::PathIoErrorKind;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        Lstat(NormalizedAbsolutePath),
        ReadLink(NormalizedAbsolutePath),
        FileBytes(NormalizedAbsolutePath),
        DirectoryEntries(NormalizedAbsolutePath),
    }

    struct ScriptedOperations {
        supports_lstat: bool,
        support_queries: usize,
        calls: Vec<Call>,
        lstats: VecDeque<PathOperationResult<PathLstat>>,
        read_links: VecDeque<Result<Arc<PathBuf>, PrimaryFailure>>,
        file_bytes: VecDeque<Result<Arc<[u8]>, PrimaryFailure>>,
        directory_entries: VecDeque<Result<PathDirectoryEntries, PrimaryFailure>>,
    }

    impl ScriptedOperations {
        fn supported() -> Self {
            Self {
                supports_lstat: true,
                support_queries: 0,
                calls: Vec::new(),
                lstats: VecDeque::new(),
                read_links: VecDeque::new(),
                file_bytes: VecDeque::new(),
                directory_entries: VecDeque::new(),
            }
        }

        fn unsupported() -> Self {
            Self {
                supports_lstat: false,
                ..Self::supported()
            }
        }
    }

    impl ObservationOperations for ScriptedOperations {
        fn supports_lstat(&mut self) -> bool {
            self.support_queries += 1;
            self.supports_lstat
        }

        fn lstat(&mut self, path: &NormalizedAbsolutePath) -> PathOperationResult<PathLstat> {
            self.calls.push(Call::Lstat(path.clone()));
            self.lstats
                .pop_front()
                .expect("script must supply an lstat result")
        }

        fn read_link(
            &mut self,
            path: &NormalizedAbsolutePath,
        ) -> Result<Arc<PathBuf>, PrimaryFailure> {
            self.calls.push(Call::ReadLink(path.clone()));
            self.read_links
                .pop_front()
                .expect("script must supply a read-link result")
        }

        fn file_bytes(
            &mut self,
            path: &NormalizedAbsolutePath,
        ) -> Result<Arc<[u8]>, PrimaryFailure> {
            self.calls.push(Call::FileBytes(path.clone()));
            self.file_bytes
                .pop_front()
                .expect("script must supply a file-bytes result")
        }

        fn directory_entries(
            &mut self,
            path: &NormalizedAbsolutePath,
        ) -> Result<PathDirectoryEntries, PrimaryFailure> {
            self.calls.push(Call::DirectoryEntries(path.clone()));
            self.directory_entries
                .pop_front()
                .expect("script must supply a directory result")
        }
    }

    fn path(root: &Path, suffix: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(root.join(suffix)).unwrap()
    }

    fn demand(
        namespace: PathObservationNamespace,
        path: NormalizedAbsolutePath,
        operation: PathObservationOperation,
    ) -> PathObservationDemand {
        PathObservationDemand::new(namespace, path, operation)
    }

    fn lstat(kind: PathNodeKind) -> PathLstat {
        PathLstat::new(kind, 1, 2, 3, 4, 0o755)
    }

    fn io(kind: PathIoErrorKind, raw_os_error: Option<i32>) -> PathObservationError {
        PathObservationError::Io { kind, raw_os_error }
    }

    fn empty_entries() -> PathDirectoryEntries {
        PathDirectoryEntries::new([]).unwrap()
    }

    fn roots<'a>(owner: &'a (), root: &NormalizedAbsolutePath) -> RetainedMaterializationRoots<'a> {
        RetainedMaterializationRoots::new(
            owner,
            [(PathObservationInstanceId::new(1), root.clone())],
        )
        .unwrap()
    }

    fn assert_allocative<T: Allocative>() {}

    #[test]
    fn retained_roots_are_one_sorted_arc_slice_with_borrowed_lifetime() {
        assert_allocative::<RetainedMaterializationRoot>();
        assert_allocative::<RetainedMaterializationRoots<'_>>();
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let shared = path(temp.path(), "shared");
        let other = path(temp.path(), "other");
        let retained = RetainedMaterializationRoots::new(
            &owner,
            [
                (PathObservationInstanceId::new(3), shared.clone()),
                (PathObservationInstanceId::new(1), shared.clone()),
                (PathObservationInstanceId::new(2), other),
            ],
        )
        .unwrap();
        assert_eq!(Arc::strong_count(&retained.entries), 1);
        assert_eq!(
            retained
                .entries
                .iter()
                .map(|entry| entry.instance.value())
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(retained.entries[0].root, retained.entries[2].root);
    }

    #[test]
    fn retained_roots_reject_zero_and_duplicate_instances() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let first = path(temp.path(), "first");
        let second = path(temp.path(), "second");
        assert!(matches!(
            RetainedMaterializationRoots::new(
                &owner,
                [(PathObservationInstanceId::new(0), first.clone())]
            ),
            Err(PathObservationKernelError::ZeroRetainedInstance)
        ));
        assert!(matches!(
            RetainedMaterializationRoots::new(
                &owner,
                [
                    (PathObservationInstanceId::new(4), first),
                    (PathObservationInstanceId::new(4), second)
                ]
            ),
            Err(PathObservationKernelError::DuplicateRetainedInstance(
                PathObservationInstanceId { .. }
            ))
        ));
    }

    #[test]
    fn invalid_batches_preflight_before_support_or_operation_calls() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let root = path(temp.path(), "root");
        let retained = roots(&owner, &root);
        let host = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "host"),
            PathObservationOperation::Lstat,
        );
        let zero = demand(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(0)),
            path(temp.path(), "zero"),
            PathObservationOperation::ReadLink,
        );
        let unknown = demand(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(9)),
            path(temp.path(), "unknown"),
            PathObservationOperation::FileBytes,
        );

        for (demands, expected) in [
            (
                vec![host.clone(), host.clone()],
                PathObservationKernelError::DuplicateDemand(host.clone()),
            ),
            (
                vec![host.clone(), zero.clone()],
                PathObservationKernelError::ZeroDemandInstance(zero),
            ),
            (
                vec![host.clone(), unknown.clone()],
                PathObservationKernelError::UnknownDemandInstance(unknown),
            ),
        ] {
            let mut operations = ScriptedOperations::unsupported();
            assert_eq!(
                observe_with(&retained, demands, &mut operations),
                Err(expected)
            );
            assert_eq!(operations.support_queries, 0);
            assert!(operations.calls.is_empty());
        }

        let mut operations = ScriptedOperations::unsupported();
        assert_eq!(
            observe_with(&retained, [host], &mut operations),
            Err(PathObservationKernelError::UnsupportedLstat)
        );
        assert_eq!(operations.support_queries, 1);
        assert!(operations.calls.is_empty());

        let earlier = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "a-earlier"),
            PathObservationOperation::FileBytes,
        );
        let later_lstat = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "z-later"),
            PathObservationOperation::Lstat,
        );
        let mut operations = ScriptedOperations::unsupported();
        assert_eq!(
            observe_with(&retained, [earlier, later_lstat], &mut operations),
            Err(PathObservationKernelError::UnsupportedLstat)
        );
        assert_eq!(operations.support_queries, 1);
        assert!(operations.calls.is_empty());
    }

    #[test]
    fn empty_and_non_lstat_batches_do_not_query_lstat_support() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let mut operations = ScriptedOperations::unsupported();
        let epoch = observe_with(&retained, [], &mut operations).unwrap();
        assert!(epoch.observations().is_empty());
        assert_eq!(operations.support_queries, 0);

        let read = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "read"),
            PathObservationOperation::ReadLink,
        );
        let unsupported = io(PathIoErrorKind::Unsupported, None);
        operations
            .read_links
            .push_back(Err(PrimaryFailure::Final(unsupported)));
        let epoch = observe_with(&retained, [read.clone()], &mut operations).unwrap();
        assert_eq!(operations.support_queries, 0);
        assert!(matches!(
            epoch.get(&read).unwrap().as_ref(),
            PathObservationResult::ReadLink(PathOperationResult::Error(error))
                if *error == unsupported
        ));
    }

    #[test]
    fn shuffled_demands_execute_in_exact_ord_order_and_errors_continue() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let root = path(temp.path(), "root");
        let retained = roots(&owner, &root);
        let a = path(temp.path(), "a");
        let b = path(temp.path(), "b");
        let c = path(temp.path(), "c");
        let demands = vec![
            demand(
                PathObservationNamespace::Host,
                c.clone(),
                PathObservationOperation::FileBytes,
            ),
            demand(
                PathObservationNamespace::Host,
                a.clone(),
                PathObservationOperation::ReadLink,
            ),
            demand(
                PathObservationNamespace::Host,
                b.clone(),
                PathObservationOperation::DirectoryEntries,
            ),
        ];
        let mut sorted = demands.clone();
        sorted.sort_unstable();
        let mut operations = ScriptedOperations::supported();
        operations
            .read_links
            .push_back(Err(PrimaryFailure::Final(io(
                PathIoErrorKind::PermissionDenied,
                Some(13),
            ))));
        operations.directory_entries.push_back(Ok(empty_entries()));
        operations
            .file_bytes
            .push_back(Ok(Arc::from(&b"bytes"[..])));
        let epoch = observe_with(&retained, demands, &mut operations).unwrap();
        assert_eq!(epoch.observations().len(), 3);
        assert_eq!(
            operations.calls,
            vec![
                Call::ReadLink(a),
                Call::DirectoryEntries(b),
                Call::FileBytes(c)
            ]
        );
        assert_eq!(
            epoch.observations().keys().cloned().collect::<Vec<_>>(),
            sorted
        );
    }

    #[test]
    fn exact_path_is_not_rewritten_and_namespaces_remain_distinct() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let root = path(temp.path(), "retained-root");
        let retained = roots(&owner, &root);
        let escaped = path(temp.path(), "outside/escaped");
        let host = demand(
            PathObservationNamespace::Host,
            escaped.clone(),
            PathObservationOperation::FileBytes,
        );
        let materialized = demand(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
            escaped.clone(),
            PathObservationOperation::FileBytes,
        );
        let mut operations = ScriptedOperations::supported();
        operations.file_bytes.extend([
            Ok(Arc::from(&b"host"[..])),
            Ok(Arc::from(&b"materialized"[..])),
        ]);
        let epoch = observe_with(
            &retained,
            [materialized.clone(), host.clone()],
            &mut operations,
        )
        .unwrap();
        assert_eq!(
            operations.calls,
            vec![Call::FileBytes(escaped.clone()), Call::FileBytes(escaped)]
        );
        assert!(epoch.get(&host).is_some());
        assert!(epoch.get(&materialized).is_some());
        assert_ne!(host, materialized);
    }

    #[test]
    fn lstat_and_present_primary_results_match_epoch_variants_without_aux() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let base = path(temp.path(), "base");
        let demands = [
            demand(
                PathObservationNamespace::Host,
                path(temp.path(), "lstat"),
                PathObservationOperation::Lstat,
            ),
            demand(
                PathObservationNamespace::Host,
                path(temp.path(), "link"),
                PathObservationOperation::ReadLink,
            ),
            demand(
                PathObservationNamespace::Host,
                path(temp.path(), "bytes"),
                PathObservationOperation::FileBytes,
            ),
            demand(
                PathObservationNamespace::Host,
                path(temp.path(), "dir"),
                PathObservationOperation::DirectoryEntries,
            ),
        ];
        let mut operations = ScriptedOperations::supported();
        operations
            .lstats
            .push_back(PathOperationResult::Present(lstat(
                PathNodeKind::RegularFile,
            )));
        operations
            .read_links
            .push_back(Ok(Arc::new(base.as_path().to_path_buf())));
        operations.file_bytes.push_back(Ok(Arc::from(&b"x"[..])));
        operations.directory_entries.push_back(Ok(empty_entries()));
        let epoch = observe_with(&retained, demands.clone(), &mut operations).unwrap();
        assert_eq!(operations.support_queries, 1);
        assert_eq!(operations.calls.len(), 4);
        assert!(matches!(
            epoch.get(&demands[0]).unwrap().as_ref(),
            PathObservationResult::Lstat(PathOperationResult::Present(_))
        ));
        assert!(matches!(
            epoch.get(&demands[1]).unwrap().as_ref(),
            PathObservationResult::ReadLink(PathOperationResult::Present(_))
        ));
        assert!(matches!(
            epoch.get(&demands[2]).unwrap().as_ref(),
            PathObservationResult::FileBytes(PathOperationResult::Present(bytes))
                if bytes.as_ref() == b"x"
        ));
        assert!(matches!(
            epoch.get(&demands[3]).unwrap().as_ref(),
            PathObservationResult::DirectoryEntries(PathOperationResult::Present(_))
        ));
    }

    #[test]
    fn lstat_directly_preserves_missing_and_io_error() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let missing = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "a-missing"),
            PathObservationOperation::Lstat,
        );
        let failed = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "b-failed"),
            PathObservationOperation::Lstat,
        );
        let original = io(PathIoErrorKind::PermissionDenied, Some(13));
        let mut operations = ScriptedOperations::supported();
        operations.lstats.extend([
            PathOperationResult::Missing,
            PathOperationResult::Error(original),
        ]);
        let epoch = observe_with(
            &retained,
            [failed.clone(), missing.clone()],
            &mut operations,
        )
        .unwrap();
        assert!(matches!(
            epoch.get(&missing).unwrap().as_ref(),
            PathObservationResult::Lstat(PathOperationResult::Missing)
        ));
        assert!(matches!(
            epoch.get(&failed).unwrap().as_ref(),
            PathObservationResult::Lstat(PathOperationResult::Error(error))
                if *error == original
        ));
    }

    #[test]
    fn final_primary_failures_never_run_auxiliary_lstat() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        for operation in [
            PathObservationOperation::ReadLink,
            PathObservationOperation::FileBytes,
            PathObservationOperation::DirectoryEntries,
        ] {
            let demand = demand(
                PathObservationNamespace::Host,
                path(temp.path(), &format!("{operation:?}")),
                operation,
            );
            let original = io(PathIoErrorKind::PermissionDenied, Some(13));
            let mut operations = ScriptedOperations::supported();
            match operation {
                PathObservationOperation::ReadLink => operations
                    .read_links
                    .push_back(Err(PrimaryFailure::Final(original))),
                PathObservationOperation::FileBytes => operations
                    .file_bytes
                    .push_back(Err(PrimaryFailure::Final(original))),
                PathObservationOperation::DirectoryEntries => operations
                    .directory_entries
                    .push_back(Err(PrimaryFailure::Final(original))),
                PathObservationOperation::Lstat => unreachable!(),
            }
            let epoch = observe_with(&retained, [demand.clone()], &mut operations).unwrap();
            assert_eq!(operations.calls.len(), 1);
            let result = epoch.get(&demand).unwrap();
            assert!(matches!(
                result.as_ref(),
                PathObservationResult::ReadLink(PathOperationResult::Error(error))
                    | PathObservationResult::FileBytes(PathOperationResult::Error(error))
                    | PathObservationResult::DirectoryEntries(PathOperationResult::Error(error))
                    if *error == original
            ));
        }
    }

    #[test]
    fn read_link_refinement_exhausts_every_auxiliary_kind_and_race() {
        let original = PathObservationError::NotALink;
        assert_eq!(
            refine_read_link(original, PathOperationResult::Missing),
            PathOperationResult::Missing
        );
        for kind in [
            PathNodeKind::RegularFile,
            PathNodeKind::Directory,
            PathNodeKind::SpecialFile,
        ] {
            assert_eq!(
                refine_read_link(original, PathOperationResult::Present(lstat(kind))),
                PathOperationResult::Error(PathObservationError::WrongKind {
                    expected: PathNodeKind::Symlink,
                    actual: kind
                })
            );
        }
        assert_eq!(
            refine_read_link(
                original,
                PathOperationResult::Present(lstat(PathNodeKind::Symlink))
            ),
            PathOperationResult::Error(original)
        );
        assert_eq!(
            refine_read_link(
                original,
                PathOperationResult::Error(io(PathIoErrorKind::PermissionDenied, Some(13)))
            ),
            PathOperationResult::Error(original)
        );

        let original_io = io(PathIoErrorKind::NotFound, Some(2));
        assert_eq!(
            refine_read_link(
                original_io,
                PathOperationResult::Present(lstat(PathNodeKind::Symlink))
            ),
            PathOperationResult::Error(original_io)
        );
        assert_eq!(
            refine_read_link(
                original_io,
                PathOperationResult::Error(io(PathIoErrorKind::PermissionDenied, Some(13)))
            ),
            PathOperationResult::Error(original_io)
        );
    }

    #[test]
    fn file_bytes_refinement_exhausts_every_auxiliary_kind_and_race() {
        let original = io(PathIoErrorKind::IsADirectory, Some(21));
        assert_eq!(
            refine_file_bytes(original, PathOperationResult::Missing),
            PathOperationResult::Missing
        );
        for kind in [
            PathNodeKind::RegularFile,
            PathNodeKind::SpecialFile,
            PathNodeKind::Symlink,
        ] {
            assert_eq!(
                refine_file_bytes(original, PathOperationResult::Present(lstat(kind))),
                PathOperationResult::Error(original)
            );
        }
        assert_eq!(
            refine_file_bytes(
                original,
                PathOperationResult::Present(lstat(PathNodeKind::Directory))
            ),
            PathOperationResult::Error(PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: PathNodeKind::Directory
            })
        );
        assert_eq!(
            refine_file_bytes(
                original,
                PathOperationResult::Error(io(PathIoErrorKind::PermissionDenied, Some(13)))
            ),
            PathOperationResult::Error(original)
        );
    }

    #[test]
    fn directory_refinement_exhausts_every_auxiliary_kind_and_race() {
        let original = io(PathIoErrorKind::NotADirectory, None);
        assert_eq!(
            refine_directory_entries(original, PathOperationResult::Missing),
            PathOperationResult::Missing
        );
        for kind in [
            PathNodeKind::RegularFile,
            PathNodeKind::Symlink,
            PathNodeKind::SpecialFile,
        ] {
            assert_eq!(
                refine_directory_entries(original, PathOperationResult::Present(lstat(kind))),
                PathOperationResult::Error(PathObservationError::WrongKind {
                    expected: PathNodeKind::Directory,
                    actual: kind
                })
            );
        }
        assert_eq!(
            refine_directory_entries(
                original,
                PathOperationResult::Present(lstat(PathNodeKind::Directory))
            ),
            PathOperationResult::Error(original)
        );
        assert_eq!(
            refine_directory_entries(
                original,
                PathOperationResult::Error(io(PathIoErrorKind::PermissionDenied, Some(13)))
            ),
            PathOperationResult::Error(original)
        );
    }

    #[test]
    fn refine_calls_primary_before_auxiliary_and_special_bytes_can_succeed() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let link_path = path(temp.path(), "link");
        let bytes_path = path(temp.path(), "special");
        let link = demand(
            PathObservationNamespace::Host,
            link_path.clone(),
            PathObservationOperation::ReadLink,
        );
        let bytes = demand(
            PathObservationNamespace::Host,
            bytes_path.clone(),
            PathObservationOperation::FileBytes,
        );
        let mut operations = ScriptedOperations::supported();
        operations
            .read_links
            .push_back(Err(PrimaryFailure::Refine(PathObservationError::NotALink)));
        operations
            .lstats
            .push_back(PathOperationResult::Present(lstat(PathNodeKind::Symlink)));
        operations
            .file_bytes
            .push_back(Ok(Arc::from(&b"special"[..])));
        let epoch =
            observe_with(&retained, [bytes.clone(), link.clone()], &mut operations).unwrap();
        assert_eq!(
            operations.calls,
            vec![
                Call::ReadLink(link_path.clone()),
                Call::Lstat(link_path),
                Call::FileBytes(bytes_path)
            ]
        );
        assert!(matches!(
            epoch.get(&link).unwrap().as_ref(),
            PathObservationResult::ReadLink(PathOperationResult::Error(
                PathObservationError::NotALink
            ))
        ));
        assert!(matches!(
            epoch.get(&bytes).unwrap().as_ref(),
            PathObservationResult::FileBytes(PathOperationResult::Present(value))
                if value.as_ref() == b"special"
        ));
    }

    #[test]
    fn every_refinable_operation_calls_primary_before_its_auxiliary() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let link_path = path(temp.path(), "a-link");
        let bytes_path = path(temp.path(), "b-bytes");
        let directory_path = path(temp.path(), "c-directory");
        let demands = [
            demand(
                PathObservationNamespace::Host,
                link_path.clone(),
                PathObservationOperation::ReadLink,
            ),
            demand(
                PathObservationNamespace::Host,
                bytes_path.clone(),
                PathObservationOperation::FileBytes,
            ),
            demand(
                PathObservationNamespace::Host,
                directory_path.clone(),
                PathObservationOperation::DirectoryEntries,
            ),
        ];
        let mut operations = ScriptedOperations::supported();
        operations
            .read_links
            .push_back(Err(PrimaryFailure::Refine(PathObservationError::NotALink)));
        operations
            .file_bytes
            .push_back(Err(PrimaryFailure::Refine(io(
                PathIoErrorKind::NotFound,
                None,
            ))));
        operations
            .directory_entries
            .push_back(Err(PrimaryFailure::Refine(io(
                PathIoErrorKind::NotADirectory,
                None,
            ))));
        operations.lstats.extend([
            PathOperationResult::Missing,
            PathOperationResult::Present(lstat(PathNodeKind::SpecialFile)),
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ]);
        observe_with(&retained, demands, &mut operations).unwrap();
        assert_eq!(
            operations.calls,
            vec![
                Call::ReadLink(link_path.clone()),
                Call::Lstat(link_path),
                Call::FileBytes(bytes_path.clone()),
                Call::Lstat(bytes_path),
                Call::DirectoryEntries(directory_path.clone()),
                Call::Lstat(directory_path),
            ]
        );
    }

    #[test]
    fn directory_entry_value_can_flow_through_matching_epoch_variant() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let demand = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "directory"),
            PathObservationOperation::DirectoryEntries,
        );
        let entries =
            PathDirectoryEntries::new([PathDirectoryName::new("entry").unwrap()]).unwrap();
        let mut operations = ScriptedOperations::supported();
        operations.directory_entries.push_back(Ok(entries));
        let epoch = observe_with(&retained, [demand.clone()], &mut operations).unwrap();
        assert!(matches!(
            epoch.get(&demand).unwrap().as_ref(),
            PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries))
                if entries.names()[0].as_os_str() == "entry"
        ));
    }
}
