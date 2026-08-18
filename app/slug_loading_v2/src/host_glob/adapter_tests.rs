/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use async_trait::async_trait;
use dice::ActivationData;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::DynKey;
use dice::Key;
use dice::UserComputationData;
use dice_futures::cancellation::CancellationContext;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryName;
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

use super::*;
use crate::host_glob::HostGlobInvalidPattern;
use crate::host_glob::traversal::HostGlobTraversal;
use crate::host_glob::traversal::HostGlobTraversalKey;
use crate::host_glob::traversal::HostGlobTraversalObservationKey;

type ScriptEntry = (PathObservationDemand, PathObservationResult);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct AdapterConsumerKey {
    package: PackagePath,
    pattern: Arc<[u8]>,
    operation: HostGlobTraversalOperation,
}

impl std::fmt::Display for AdapterConsumerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "test-host-glob-loading-adapter:{}", self.package)
    }
}

#[async_trait]
impl Key for AdapterConsumerKey {
    type Value = Result<HostGlobLoadingOutcome, HostGlobLoadingInputError>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        compute_host_glob_for_loading(
            ctx,
            path("/workspace"),
            path("/workspace"),
            self.package.clone(),
            self.pattern.clone(),
            self.operation,
        )
        .await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.complete_eq(y),
            (Err(x), Err(y)) => x == y,
            _ => false,
        }
    }

    fn validity(value: &Self::Value) -> bool {
        match value {
            Ok(value) => value.is_complete(),
            Err(_) => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct RequestAdapterConsumerKey {
    observed: bool,
    pattern: Arc<[u8]>,
}

impl std::fmt::Display for RequestAdapterConsumerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "test-host-glob-request-adapter:observed={}",
            self.observed
        )
    }
}

#[async_trait]
impl Key for RequestAdapterConsumerKey {
    type Value = Result<HostGlobRequestOutcome, HostGlobRequestInputError>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        compute_host_glob_request(
            ctx,
            path("/workspace"),
            path("/workspace"),
            PackagePath::parse("pkg").unwrap(),
            HostGlobLoadingRequest::new(self.pattern.clone(), HostGlobLoadingOperation::Files),
            self.observed,
        )
        .await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.complete_eq(y),
            (Err(x), Err(y)) => x == y,
            _ => false,
        }
    }

    fn validity(value: &Self::Value) -> bool {
        match value {
            Ok(value) => value.is_complete(),
            Err(_) => true,
        }
    }
}

#[derive(Default)]
struct AdapterTracker {
    traversal_evaluated: AtomicUsize,
    observed_traversal_evaluated: AtomicUsize,
}

impl ActivationTracker for AdapterTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        _dependencies: &mut dyn Iterator<Item = &DynKey>,
        activation: ActivationData,
    ) {
        if key.downcast_ref::<HostGlobTraversalKey>().is_some()
            && matches!(activation, ActivationData::Evaluated(_))
        {
            self.traversal_evaluated.fetch_add(1, Ordering::SeqCst);
        }
        if key
            .downcast_ref::<HostGlobTraversalObservationKey>()
            .is_some()
            && matches!(activation, ActivationData::Evaluated(_))
        {
            self.observed_traversal_evaluated
                .fetch_add(1, Ordering::SeqCst);
        }
    }
}

fn path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(value).unwrap()
}

fn demand(value: &str, operation: PathObservationOperation) -> PathObservationDemand {
    PathObservationDemand::new(PathObservationNamespace::Host, path(value), operation)
}

fn present(value: &str, kind: PathNodeKind) -> ScriptEntry {
    (
        demand(value, PathObservationOperation::Lstat),
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            kind, 1, 2, 3, 4, 0o755,
        ))),
    )
}

fn missing(value: &str) -> ScriptEntry {
    (
        demand(value, PathObservationOperation::Lstat),
        PathObservationResult::Lstat(PathOperationResult::Missing),
    )
}

fn listing(value: &str, entries: Vec<(&[u8], PathDirectoryEntryKind)>) -> ScriptEntry {
    let entries = PathDirectoryEntries::new(entries.into_iter().map(|(name, kind)| {
        PathDirectoryEntry::new(
            PathDirectoryName::new(OsString::from_vec(name.to_vec())).unwrap(),
            kind,
        )
    }));
    (
        demand(value, PathObservationOperation::DirectoryEntries),
        PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries)),
    )
}

fn prelude() -> Vec<ScriptEntry> {
    vec![
        present("/", PathNodeKind::Directory),
        present("/workspace", PathNodeKind::Directory),
        missing("/workspace/REPO.bazel"),
        missing("/workspace/.bazelignore"),
    ]
}

fn policy() -> RootPackagePolicyInputs {
    RootPackagePolicyInputs::new(
        path("/workspace"),
        vec![path("/workspace")],
        std::iter::empty::<&str>(),
        None,
        Some("warning"),
    )
    .unwrap()
}

fn consumer(
    package: &str,
    pattern: &[u8],
    operation: HostGlobTraversalOperation,
) -> AdapterConsumerKey {
    AdapterConsumerKey {
        package: PackagePath::parse(package).unwrap(),
        pattern: Arc::from(pattern),
        operation,
    }
}

fn complete_paths(value: &HostGlobLoadingOutcome) -> &[Arc<[u8]>] {
    let SourcePreparationOutcome::Complete(value) = value else {
        panic!("expected complete adapter outcome: {value:?}")
    };
    value
        .as_ref()
        .as_ref()
        .expect("expected successful adapter outcome")
        .paths()
}

fn complete_request(
    value: &Result<HostGlobRequestOutcome, HostGlobRequestInputError>,
) -> (&HostGlobPrepared, &PathObservationEpoch) {
    let Ok(SourcePreparationOutcome::Complete(Ok((prepared, observations)))) = value else {
        panic!("expected complete request adapter outcome: {value:?}")
    };
    (prepared, observations)
}

#[tokio::test]
async fn rejects_pattern_and_key_before_traversal_activation() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(AdapterTracker::default());
    let mut transaction = dice
        .updater_with_data(UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        })
        .commit()
        .await;

    let invalid = transaction
        .compute(&consumer("pkg", b"?", HostGlobTraversalOperation::Files))
        .await
        .unwrap();
    assert!(matches!(
        invalid,
        Err(HostGlobLoadingInputError::Pattern(
            HostGlobPatternError::Invalid {
                reason: HostGlobInvalidPattern::QuestionMarkForbidden,
                ..
            }
        ))
    ));

    let non_latin1 = transaction
        .compute(&consumer(
            "pkg/\u{100}",
            b"*",
            HostGlobTraversalOperation::Files,
        ))
        .await
        .unwrap();
    assert!(matches!(
        non_latin1,
        Err(HostGlobLoadingInputError::Key(
            HostGlobTraversalKeyError::NonLatin1PackagePathScalar { scalar: '\u{100}' }
        ))
    ));
    assert_eq!(tracker.traversal_evaluated.load(Ordering::SeqCst), 0);
}

#[test]
fn projection_preserves_sorted_deduplicated_raw_path_arcs_and_typed_error() {
    let raw = Arc::<[u8]>::from(&b"\xff.bin"[..]);
    let ascii = Arc::<[u8]>::from(&b"a.txt"[..]);
    let traversal = HostGlobTraversal::from_paths(vec![raw.clone(), ascii.clone(), raw.clone()]);
    let projected =
        project_traversal_outcome(SourcePreparationOutcome::Complete(Arc::new(Ok(traversal))));
    let paths = complete_paths(&projected);
    assert_eq!(
        paths.iter().map(|path| path.as_ref()).collect::<Vec<_>>(),
        vec![b"a.txt".as_slice(), b"\xff.bin".as_slice()]
    );
    assert!(Arc::ptr_eq(&paths[0], &ascii));
    assert!(Arc::ptr_eq(&paths[1], &raw));

    let error = HostGlobTraversalError::UnsupportedHost;
    let projected = project_traversal_outcome(SourcePreparationOutcome::Complete(Arc::new(Err(
        error.clone(),
    ))));
    assert!(matches!(
        projected,
        SourcePreparationOutcome::Complete(value)
            if matches!(value.as_ref(), Err(actual) if actual == &error)
    ));
}

#[tokio::test]
async fn forwards_need_without_turning_it_into_a_complete_value() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(&mut updater, policy()).unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new([]).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let value = transaction
        .compute(&consumer(
            "pkg",
            b"entry",
            HostGlobTraversalOperation::Files,
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(value, SourcePreparationOutcome::Need(_)));
}

#[tokio::test]
async fn projects_one_pattern_operation_and_same_graph_restoration() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(AdapterTracker::default());
    let script = |entry_present| {
        let mut entries = prelude();
        entries.push(present("/workspace/pkg", PathNodeKind::Directory));
        entries.push(if entry_present {
            present("/workspace/pkg/entry", PathNodeKind::RegularFile)
        } else {
            missing("/workspace/pkg/entry")
        });
        entries
    };
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
        ..Default::default()
    });
    inject_root_package_policy_inputs(&mut updater, policy()).unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(script(true)).unwrap(),
        )])
        .unwrap();
    let key = consumer("pkg", b"entry", HostGlobTraversalOperation::Files);
    let mut transaction = updater.commit().await;
    assert_eq!(
        complete_paths(&transaction.compute(&key).await.unwrap().unwrap()),
        &[Arc::<[u8]>::from(&b"entry"[..])]
    );

    for (entry_present, expected) in [(false, Vec::new()), (true, vec![b"entry".to_vec()])] {
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(script(entry_present)).unwrap(),
            )])
            .unwrap();
        transaction = updater.commit().await;
        assert_eq!(
            complete_paths(&transaction.compute(&key).await.unwrap().unwrap())
                .iter()
                .map(|path| path.to_vec())
                .collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(tracker.traversal_evaluated.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn files_and_directories_operation_reaches_the_adapter_exactly() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![
                (b"dir", PathDirectoryEntryKind::Directory),
                (b"file", PathDirectoryEntryKind::File),
            ],
        ),
        present("/workspace/pkg/dir", PathNodeKind::Directory),
        present("/workspace/pkg/file", PathNodeKind::RegularFile),
        missing("/workspace/pkg/dir/BUILD.bazel"),
        missing("/workspace/pkg/dir/BUILD"),
    ]);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(&mut updater, policy()).unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(entries).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;

    let files = transaction
        .compute(&consumer("pkg", b"*", HostGlobTraversalOperation::Files))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        complete_paths(&files)
            .iter()
            .map(|path| path.as_ref())
            .collect::<Vec<_>>(),
        vec![b"file".as_slice()]
    );

    let all = transaction
        .compute(&consumer(
            "pkg",
            b"*",
            HostGlobTraversalOperation::FilesAndDirs,
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        complete_paths(&all)
            .iter()
            .map(|path| path.as_ref())
            .collect::<Vec<_>>(),
        vec![b"dir".as_slice(), b"file".as_slice()]
    );
}

#[tokio::test]
async fn request_adapter_preserves_semantics_exact_epoch_arcs_and_family_isolation() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(AdapterTracker::default());
    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![(b"entry.txt", PathDirectoryEntryKind::File)],
        ),
    ]);
    let epoch = PathObservationEpoch::new(entries).unwrap();
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
        ..Default::default()
    });
    inject_root_package_policy_inputs(&mut updater, policy()).unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    let mut transaction = updater.commit().await;

    let key = |observed| RequestAdapterConsumerKey {
        observed,
        pattern: Arc::from(&b"*"[..]),
    };
    let legacy = transaction.compute(&key(false)).await.unwrap();
    let observed = transaction.compute(&key(true)).await.unwrap();
    let (legacy, legacy_epoch) = complete_request(&legacy);
    let (observed, observed_epoch) = complete_request(&observed);
    assert!(legacy_epoch.observations().is_empty());
    assert_eq!(legacy.as_ref(), observed.as_ref());
    assert_eq!(
        observed
            .as_ref()
            .as_ref()
            .unwrap()
            .paths()
            .iter()
            .map(|path| path.as_ref())
            .collect::<Vec<_>>(),
        [b"entry.txt".as_slice()]
    );
    assert!(!observed_epoch.observations().is_empty());
    for (demand, result) in observed_epoch.observations() {
        assert!(Arc::ptr_eq(result, epoch.get(demand).unwrap()));
    }
    assert_eq!(tracker.traversal_evaluated.load(Ordering::SeqCst), 1);
    assert_eq!(
        tracker.observed_traversal_evaluated.load(Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn observed_request_adapter_preserves_all_terminal_polarities() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(AdapterTracker::default());
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
        ..Default::default()
    });
    inject_root_package_policy_inputs(&mut updater, policy()).unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::empty(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;

    let invalid = transaction
        .compute(&RequestAdapterConsumerKey {
            observed: true,
            pattern: Arc::from(&b"?"[..]),
        })
        .await
        .unwrap();
    assert!(matches!(invalid, Err(HostGlobRequestInputError(_))));
    assert_eq!(
        tracker.observed_traversal_evaluated.load(Ordering::SeqCst),
        0
    );

    let need = transaction
        .compute(&RequestAdapterConsumerKey {
            observed: true,
            pattern: Arc::from(&b"entry"[..]),
        })
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(need, SourcePreparationOutcome::Need(_)));

    let literal = demand("/workspace/pkg/entry", PathObservationOperation::Lstat);
    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        (
            literal.dupe(),
            PathObservationResult::Lstat(PathOperationResult::Error(PathObservationError::Io {
                kind: PathIoErrorKind::PermissionDenied,
                raw_os_error: Some(13),
            })),
        ),
    ]);
    let epoch = PathObservationEpoch::new(entries).unwrap();
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    let mut transaction = updater.commit().await;
    let semantic = transaction
        .compute(&RequestAdapterConsumerKey {
            observed: true,
            pattern: Arc::from(&b"entry"[..]),
        })
        .await
        .unwrap();
    let (prepared, observations) = complete_request(&semantic);
    assert!(prepared.is_err());
    assert!(Arc::ptr_eq(
        observations.get(&literal).unwrap(),
        epoch.get(&literal).unwrap()
    ));

    let mut data = UserComputationData::default();
    data.data.set(ForceHostGlobRequestOuter(
        PathObservationEpoch::from_shared([(
            demand("/mismatch", PathObservationOperation::Lstat),
            Arc::new(PathObservationResult::FileBytes(
                PathOperationResult::Missing,
            )),
        )])
        .unwrap_err()
        .into(),
    ));
    let outer_dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = outer_dice.updater_with_data(data);
    inject_root_package_policy_inputs(&mut updater, policy()).unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    let mut outer = updater.commit().await;
    assert!(matches!(
        outer
            .compute(&RequestAdapterConsumerKey {
                observed: true,
                pattern: Arc::from(&b"entry"[..]),
            })
            .await
            .unwrap(),
        Ok(SourcePreparationOutcome::Complete(Err(_)))
    ));
}
