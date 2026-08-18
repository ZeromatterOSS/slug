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
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use dice::ActivationData;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DynKey;
use dice::Key;
use dice::UserComputationData;
use dupe::Dupe;
use slug_bzlmod_v2::RootPackagePolicyInputs;
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
use slug_workspace_v2::PathObservationEpochError;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationError;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::*;

type ScriptEntry = (PathObservationDemand, PathObservationResult);

#[derive(Default)]
struct TraversalTracker {
    evaluated: AtomicUsize,
    segment_evaluated: AtomicUsize,
    observed_evaluated: AtomicUsize,
    observed_with_data: AtomicUsize,
    observed_segment_evaluated: AtomicUsize,
    legacy_boundary_evaluated: AtomicUsize,
    observed_boundary_evaluated: AtomicUsize,
    activated: Mutex<Vec<String>>,
}

impl ActivationTracker for TraversalTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        _dependencies: &mut dyn Iterator<Item = &DynKey>,
        activation: ActivationData,
    ) {
        self.activated.lock().unwrap().push(key.to_string());
        if key.downcast_ref::<HostGlobTraversalKey>().is_some()
            && matches!(activation, ActivationData::Evaluated(_))
        {
            self.evaluated.fetch_add(1, Ordering::SeqCst);
        }
        if key
            .downcast_ref::<HostGlobTraversalObservationKey>()
            .is_some()
            && let ActivationData::Evaluated(ref data) = activation
        {
            self.observed_evaluated.fetch_add(1, Ordering::SeqCst);
            if data.is_some() {
                self.observed_with_data.fetch_add(1, Ordering::SeqCst);
            }
        }
        if key
            .downcast_ref::<super::super::HostGlobSegmentCandidatesKey>()
            .is_some()
            && matches!(activation, ActivationData::Evaluated(_))
        {
            self.segment_evaluated.fetch_add(1, Ordering::SeqCst);
        }
        if key
            .downcast_ref::<HostGlobSegmentCandidatesObservationKey>()
            .is_some()
            && matches!(activation, ActivationData::Evaluated(_))
        {
            self.observed_segment_evaluated
                .fetch_add(1, Ordering::SeqCst);
        }
        if key.downcast_ref::<HostRootPackageBoundaryKey>().is_some()
            && matches!(activation, ActivationData::Evaluated(_))
        {
            self.legacy_boundary_evaluated
                .fetch_add(1, Ordering::SeqCst);
        }
        if key
            .downcast_ref::<HostRootPackageBoundaryObservationKey>()
            .is_some()
            && matches!(activation, ActivationData::Evaluated(_))
        {
            self.observed_boundary_evaluated
                .fetch_add(1, Ordering::SeqCst);
        }
    }
}

fn path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(value).unwrap()
}

fn pattern(value: &[u8]) -> HostGlobPattern {
    HostGlobPattern::new(Arc::<[u8]>::from(value)).unwrap()
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

fn lstat_error(value: &str) -> ScriptEntry {
    (
        demand(value, PathObservationOperation::Lstat),
        PathObservationResult::Lstat(PathOperationResult::Error(PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        })),
    )
}

fn bytes(value: &str, contents: &'static [u8]) -> ScriptEntry {
    (
        demand(value, PathObservationOperation::FileBytes),
        PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(contents))),
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

fn policy(deleted: &[&str]) -> RootPackagePolicyInputs {
    RootPackagePolicyInputs::new(
        path("/workspace"),
        vec![path("/workspace")],
        deleted,
        None,
        Some("warning"),
    )
    .unwrap()
}

fn prelude() -> Vec<ScriptEntry> {
    vec![
        present("/", PathNodeKind::Directory),
        present("/workspace", PathNodeKind::Directory),
        missing("/workspace/REPO.bazel"),
        missing("/workspace/.bazelignore"),
    ]
}

fn key(pattern: &[u8], operation: HostGlobTraversalOperation) -> HostGlobTraversalKey {
    HostGlobTraversalKey::new(
        path("/workspace"),
        path("/workspace"),
        PackagePath::parse("pkg").unwrap(),
        self::pattern(pattern),
        operation,
    )
    .unwrap()
}

fn observed_key(
    pattern: &[u8],
    operation: HostGlobTraversalOperation,
) -> HostGlobTraversalObservationKey {
    HostGlobTraversalObservationKey::new(
        path("/workspace"),
        path("/workspace"),
        PackagePath::parse("pkg").unwrap(),
        self::pattern(pattern),
        operation,
    )
    .unwrap()
}

async fn compute(
    key: &HostGlobTraversalKey,
    entries: Vec<ScriptEntry>,
    deleted: &[&str],
) -> HostGlobTraversalOutcome {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(&mut updater, policy(deleted)).unwrap();
    updater
        .changed_to(vec![(
            (PathObservationEpochKey),
            PathObservationEpoch::new(entries).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    transaction.compute(key).await.unwrap()
}

async fn compute_observed(
    key: &HostGlobTraversalObservationKey,
    entries: Vec<ScriptEntry>,
    deleted: &[&str],
) -> (ObservedHostGlobTraversalOutcome, PathObservationEpoch) {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let epoch = PathObservationEpoch::new(entries).unwrap();
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(&mut updater, policy(deleted)).unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    let mut transaction = updater.commit().await;
    (transaction.compute(key).await.unwrap(), epoch)
}

fn observed_value(outcome: &ObservedHostGlobTraversalOutcome) -> &ObservedHostGlobTraversal {
    let SourcePreparationOutcome::Complete(Ok(value)) = outcome else {
        panic!("expected complete observed traversal: {outcome:?}")
    };
    value
}

fn observed_matches(outcome: &ObservedHostGlobTraversalOutcome) -> Vec<Vec<u8>> {
    observed_value(outcome)
        .result
        .as_ref()
        .as_ref()
        .expect("expected successful observed traversal")
        .matches()
        .iter()
        .map(|entry| entry.relative_path.to_vec())
        .collect()
}

fn matches(outcome: HostGlobTraversalOutcome) -> Vec<Vec<u8>> {
    let SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("expected complete traversal: {outcome:?}")
    };
    value
        .as_ref()
        .as_ref()
        .expect("expected successful traversal")
        .matches()
        .iter()
        .map(|entry| entry.relative_path.to_vec())
        .collect()
}

#[test]
fn checked_pattern_and_key_construction_preserve_raw_identity() {
    assert!(matches!(
        HostGlobPattern::new(Arc::<[u8]>::from(&b"dir/**/leaf"[..])),
        Ok(value) if value.fragments().len() == 3
    ));
    assert!(matches!(
        HostGlobPattern::new(Arc::<[u8]>::from(&b"dir/**x"[..])),
        Err(HostGlobPatternError::Invalid {
            fragment_index: Some(1),
            reason: HostGlobInvalidPattern::EmbeddedRecursiveWildcard,
            ..
        })
    ));
    assert!(matches!(
        HostGlobPattern::new(Arc::<[u8]>::from(&b"dir/[x]"[..])),
        Err(HostGlobPatternError::Deferred {
            fragment_index: 1,
            reason: HostGlobDeferredPattern::Bracket,
            ..
        })
    ));
    let latin = PackagePath::parse("\u{e9}").unwrap();
    let non_latin = PackagePath::parse("\u{100}").unwrap();
    assert!(
        HostGlobTraversalKey::new(
            path("/workspace"),
            path("/workspace"),
            latin,
            pattern(b"*"),
            HostGlobTraversalOperation::Files,
        )
        .is_ok()
    );
    assert!(matches!(
        HostGlobTraversalKey::new(
            path("/workspace"),
            path("/workspace"),
            non_latin,
            pattern(b"*"),
            HostGlobTraversalOperation::Files,
        ),
        Err(HostGlobTraversalKeyError::NonLatin1PackagePathScalar { scalar: '\u{100}' })
    ));
}

#[test]
fn traversal_key_identity_includes_only_semantic_fields() {
    let first = key(b"a/*", HostGlobTraversalOperation::Files);
    let same = key(b"a/*", HostGlobTraversalOperation::Files);
    let workspace = HostGlobTraversalKey::new(
        path("/other-workspace"),
        path("/workspace"),
        PackagePath::parse("pkg").unwrap(),
        pattern(b"a/*"),
        HostGlobTraversalOperation::Files,
    )
    .unwrap();
    let logical_package_root = HostGlobTraversalKey::new(
        path("/workspace"),
        path("/other-root"),
        PackagePath::parse("pkg").unwrap(),
        pattern(b"a/*"),
        HostGlobTraversalOperation::Files,
    )
    .unwrap();
    let package = HostGlobTraversalKey::new(
        path("/workspace"),
        path("/workspace"),
        PackagePath::parse("other").unwrap(),
        pattern(b"a/*"),
        HostGlobTraversalOperation::Files,
    )
    .unwrap();
    let operation = key(b"a/*", HostGlobTraversalOperation::FilesAndDirs);
    let pattern = key(b"b/*", HostGlobTraversalOperation::Files);
    assert_eq!(first, same);
    assert_ne!(first, workspace);
    assert_ne!(first, logical_package_root);
    assert_ne!(first, package);
    assert_ne!(first, operation);
    assert_ne!(first, pattern);
}

#[test]
fn package_path_latin1_lifting_keeps_distinct_raw_byte_names_distinct() {
    let one_byte = HostGlobTraversalKey::new(
        path("/workspace"),
        path("/workspace"),
        PackagePath::parse("\u{e9}").unwrap(),
        pattern(b"*"),
        HostGlobTraversalOperation::Files,
    )
    .unwrap();
    let utf8_bytes_as_scalars = HostGlobTraversalKey::new(
        path("/workspace"),
        path("/workspace"),
        PackagePath::parse("\u{c3}\u{a9}").unwrap(),
        pattern(b"*"),
        HostGlobTraversalOperation::Files,
    )
    .unwrap();
    assert_eq!(one_byte.package_bytes.as_ref(), b"\xe9");
    assert_eq!(utf8_bytes_as_scalars.package_bytes.as_ref(), b"\xc3\xa9");
    assert_ne!(one_byte, utf8_bytes_as_scalars);
}

#[test]
fn complete_only_equality_and_validity_are_exact() {
    let complete_value = |path: &'static [u8]| {
        SourcePreparationOutcome::Complete(Arc::new(Ok(HostGlobTraversal::from_paths(vec![
            Arc::from(path),
        ]))))
    };
    let complete = complete_value(b"a");
    let equal = complete_value(b"a");
    let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
        slug_workspace_v2::NeedPathObservations::singleton(demand(
            "/workspace/pkg",
            PathObservationOperation::DirectoryEntries,
        )),
    ));
    assert!(HostGlobTraversalKey::validity(&complete));
    assert!(HostGlobTraversalKey::equality(&complete, &equal));
    assert!(!HostGlobTraversalKey::validity(&need));
    assert!(!HostGlobTraversalKey::equality(&need, &need));

    let observed = SourcePreparationOutcome::Complete(Ok(ObservedHostGlobTraversal {
        result: Arc::new(Ok(HostGlobTraversal::from_paths(Vec::new()))),
        observations: PathObservationEpoch::empty(),
    }));
    let observed_equal = observed.clone();
    let observed_need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
        slug_workspace_v2::NeedPathObservations::singleton(demand(
            "/workspace/pkg",
            PathObservationOperation::DirectoryEntries,
        )),
    ));
    assert!(HostGlobTraversalObservationKey::validity(&observed));
    assert!(HostGlobTraversalObservationKey::equality(
        &observed,
        &observed_equal
    ));
    assert!(!HostGlobTraversalObservationKey::validity(&observed_need));
    assert!(!HostGlobTraversalObservationKey::equality(
        &observed_need,
        &observed_need
    ));
}

#[test]
fn final_raw_paths_sort_and_deduplicate_once() {
    let traversal = HostGlobTraversal::from_paths(vec![
        Arc::from(&b"\xe9"[..]),
        Arc::from(&b"same"[..]),
        Arc::from(&b"\xc3\xa9"[..]),
        Arc::from(&b"same"[..]),
    ]);
    assert_eq!(
        traversal
            .matches()
            .iter()
            .map(|entry| entry.relative_path.as_ref())
            .collect::<Vec<_>>(),
        vec![
            b"same".as_slice(),
            b"\xc3\xa9".as_slice(),
            b"\xe9".as_slice()
        ]
    );
}

#[test]
fn observed_prefix_terminals_preserve_ordered_arcs_and_outer_precedence() {
    let base = PathObservationEpoch::new([present("/base", PathNodeKind::Directory)]).unwrap();
    let duplicate = PathObservationEpoch::new([present("/base", PathNodeKind::Directory)]).unwrap();
    let semantic =
        PathObservationEpoch::new([present("/semantic", PathNodeKind::Directory)]).unwrap();
    let later = PathObservationEpoch::new([present("/later", PathNodeKind::Directory)]).unwrap();
    let base_demand = demand("/base", PathObservationOperation::Lstat);
    let semantic_demand = demand("/semantic", PathObservationOperation::Lstat);
    let later_demand = demand("/later", PathObservationOperation::Lstat);
    let base_arc = base.get(&base_demand).unwrap().dupe();
    let semantic_arc = semantic.get(&semantic_demand).unwrap().dupe();
    let need = SourcePreparationNeeds::path(slug_workspace_v2::NeedPathObservations::singleton(
        demand("/need", PathObservationOperation::Lstat),
    ));
    let outer = slug_workspace_v2::ObservedPathFrontierError::from(
        PathObservationEpochError::OperationMismatch {
            demand: demand("/outer", PathObservationOperation::Lstat),
            result_operation: PathObservationOperation::DirectoryEntries,
        },
    );

    let mut terminals = TraversalTerminals::new();
    terminals.add_need(need.clone());
    terminals.merge_completed(&base);
    terminals.merge_completed(&duplicate);
    terminals.merge_completed(&semantic);
    terminals.record_error(
        (1, 0),
        HostGlobTraversalError::Segment {
            logical_directory: path("/semantic"),
            fragment_index: 1,
            error: HostGlobSegmentError::DirectoryDisappeared {
                logical_directory: path("/semantic"),
            },
        },
    );
    terminals.record_outer(outer.clone());
    terminals.merge_completed(&later);
    let SourcePreparationOutcome::Complete(Ok((result, observations))) =
        terminals.finish(Vec::new())
    else {
        panic!("the first semantic terminal must bound the retained prefix")
    };
    assert!(result.is_err());
    assert_eq!(observations.observations().len(), 2);
    assert!(Arc::ptr_eq(
        observations.get(&base_demand).unwrap(),
        &base_arc
    ));
    assert!(Arc::ptr_eq(
        observations.get(&semantic_demand).unwrap(),
        &semantic_arc
    ));
    assert!(observations.get(&later_demand).is_none());

    let mut terminals = TraversalTerminals::new();
    terminals.add_need(need);
    terminals.record_outer(outer);
    assert!(matches!(
        terminals.finish(Vec::new()),
        SourcePreparationOutcome::Complete(Err(_))
    ));
}

#[tokio::test]
async fn observed_traversal_matches_literal_wildcard_recursive_and_operations() {
    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![(b"literal", PathDirectoryEntryKind::Directory)],
        ),
        present("/workspace/pkg/literal", PathNodeKind::Directory),
        missing("/workspace/pkg/literal/BUILD.bazel"),
        missing("/workspace/pkg/literal/BUILD"),
        listing(
            "/workspace/pkg/literal",
            vec![
                (b"leaf.txt", PathDirectoryEntryKind::File),
                (b"deep", PathDirectoryEntryKind::Directory),
            ],
        ),
        present("/workspace/pkg/literal/deep", PathNodeKind::Directory),
        missing("/workspace/pkg/literal/deep/BUILD.bazel"),
        missing("/workspace/pkg/literal/deep/BUILD"),
        listing(
            "/workspace/pkg/literal/deep",
            vec![(b"deep.txt", PathDirectoryEntryKind::File)],
        ),
    ]);
    for (pattern, operation) in [
        (b"literal".as_slice(), HostGlobTraversalOperation::Files),
        (b"literal/*".as_slice(), HostGlobTraversalOperation::Files),
        (b"literal/**".as_slice(), HostGlobTraversalOperation::Files),
        (
            b"literal/**".as_slice(),
            HostGlobTraversalOperation::FilesAndDirs,
        ),
    ] {
        let legacy = compute(&key(pattern, operation), entries.clone(), &[]).await;
        let (observed, epoch) =
            compute_observed(&observed_key(pattern, operation), entries.clone(), &[]).await;
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("legacy traversal must complete")
        };
        assert_eq!(observed_value(&observed).result.as_ref(), legacy.as_ref());
        for (demand, result) in observed_value(&observed).observations.observations() {
            assert!(Arc::ptr_eq(result, epoch.get(demand).unwrap()));
        }
    }
}

#[tokio::test]
async fn observed_boundary_stop_retains_boundary_but_not_descendant_observations() {
    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![
                (b"keep.txt", PathDirectoryEntryKind::File),
                (b"nested", PathDirectoryEntryKind::Directory),
            ],
        ),
        present("/workspace/pkg/nested", PathNodeKind::Directory),
        present(
            "/workspace/pkg/nested/BUILD.bazel",
            PathNodeKind::RegularFile,
        ),
        listing(
            "/workspace/pkg/nested",
            vec![(b"hidden.txt", PathDirectoryEntryKind::File)],
        ),
    ]);
    let (outcome, epoch) = compute_observed(
        &observed_key(b"**", HostGlobTraversalOperation::Files),
        entries,
        &[],
    )
    .await;
    assert_eq!(observed_matches(&outcome), vec![b"keep.txt".to_vec()]);
    let value = observed_value(&outcome);
    let marker = demand(
        "/workspace/pkg/nested/BUILD.bazel",
        PathObservationOperation::Lstat,
    );
    assert!(Arc::ptr_eq(
        value.observations.get(&marker).unwrap(),
        epoch.get(&marker).unwrap()
    ));
    assert!(
        value
            .observations
            .get(&demand(
                "/workspace/pkg/nested",
                PathObservationOperation::DirectoryEntries,
            ))
            .is_none()
    );
}

#[tokio::test]
async fn observed_segment_and_boundary_terminals_keep_carriers_but_need_does_not() {
    let (need, _) = compute_observed(
        &observed_key(b"literal", HostGlobTraversalOperation::Files),
        Vec::new(),
        &[],
    )
    .await;
    assert!(matches!(need, SourcePreparationOutcome::Need(_)));

    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        lstat_error("/workspace/pkg/literal"),
    ]);
    let (segment, epoch) = compute_observed(
        &observed_key(b"literal", HostGlobTraversalOperation::Files),
        entries,
        &[],
    )
    .await;
    assert!(matches!(
        observed_value(&segment).result.as_ref(),
        Err(HostGlobTraversalError::Segment { .. })
    ));
    let literal = demand("/workspace/pkg/literal", PathObservationOperation::Lstat);
    assert!(Arc::ptr_eq(
        observed_value(&segment).observations.get(&literal).unwrap(),
        epoch.get(&literal).unwrap()
    ));

    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![(b"nested", PathDirectoryEntryKind::Directory)],
        ),
        present("/workspace/pkg/nested", PathNodeKind::Directory),
        lstat_error("/workspace/pkg/nested/BUILD.bazel"),
    ]);
    let (boundary, epoch) = compute_observed(
        &observed_key(b"*", HostGlobTraversalOperation::Files),
        entries,
        &[],
    )
    .await;
    assert!(matches!(
        observed_value(&boundary).result.as_ref(),
        Err(HostGlobTraversalError::Boundary { .. })
    ));
    let marker = demand(
        "/workspace/pkg/nested/BUILD.bazel",
        PathObservationOperation::Lstat,
    );
    assert!(Arc::ptr_eq(
        observed_value(&boundary).observations.get(&marker).unwrap(),
        epoch.get(&marker).unwrap()
    ));
}

#[tokio::test]
async fn observed_cancellation_recovery_and_family_activation_are_isolated() {
    let tracker = Arc::new(TraversalTracker::default());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![(b"nested", PathDirectoryEntryKind::Directory)],
        ),
        present("/workspace/pkg/nested", PathNodeKind::Directory),
        missing("/workspace/pkg/nested/BUILD.bazel"),
        missing("/workspace/pkg/nested/BUILD"),
        present("/workspace/pkg/nested/leaf", PathNodeKind::RegularFile),
    ]);
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        ..Default::default()
    });
    inject_root_package_policy_inputs(&mut updater, policy(&[])).unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(entries).unwrap(),
        )])
        .unwrap();
    let observed = observed_key(b"*/leaf", HostGlobTraversalOperation::Files);
    let mut transaction = updater.commit().await;
    let mut cancelled = Box::pin(transaction.compute(&observed));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(cancelled.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(cancelled);
    assert_eq!(tracker.observed_evaluated.load(Ordering::SeqCst), 0);
    tracker.activated.lock().unwrap().clear();
    drop(transaction);

    let mut transaction = dice
        .updater_with_data(UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        })
        .commit()
        .await;
    assert_eq!(
        observed_matches(&transaction.compute(&observed).await.unwrap()),
        vec![b"nested/leaf".to_vec()]
    );
    assert_eq!(tracker.observed_evaluated.load(Ordering::SeqCst), 1);
    assert_eq!(tracker.observed_with_data.load(Ordering::SeqCst), 0);
    assert!(tracker.observed_segment_evaluated.load(Ordering::SeqCst) > 0);
    assert!(tracker.observed_boundary_evaluated.load(Ordering::SeqCst) > 0);
    assert_eq!(tracker.segment_evaluated.load(Ordering::SeqCst), 0);
    assert_eq!(tracker.legacy_boundary_evaluated.load(Ordering::SeqCst), 0);
    let activated = tracker.activated.lock().unwrap();
    let ordered = activated
        .iter()
        .filter(|key| {
            key.starts_with("observed-host-glob-segment-candidates:")
                || key.starts_with("observed-host-root-package-boundary:")
        })
        .collect::<Vec<_>>();
    assert_eq!(ordered.len(), 3);
    assert!(ordered[0].contains("/workspace/pkg"));
    assert!(ordered[1].ends_with("//pkg/nested"));
    assert!(ordered[2].contains("/workspace/pkg/nested"));
    drop(activated);

    let observed_segments = tracker.observed_segment_evaluated.load(Ordering::SeqCst);
    let observed_boundaries = tracker.observed_boundary_evaluated.load(Ordering::SeqCst);
    transaction
        .compute(&key(b"*/leaf", HostGlobTraversalOperation::Files))
        .await
        .unwrap();
    assert!(tracker.segment_evaluated.load(Ordering::SeqCst) > 0);
    assert!(tracker.legacy_boundary_evaluated.load(Ordering::SeqCst) > 0);
    assert_eq!(
        tracker.observed_segment_evaluated.load(Ordering::SeqCst),
        observed_segments
    );
    assert_eq!(
        tracker.observed_boundary_evaluated.load(Ordering::SeqCst),
        observed_boundaries
    );
}

#[tokio::test]
async fn traversal_composes_segments_boundaries_operations_and_recursive_states() {
    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![(b"literal", PathDirectoryEntryKind::Directory)],
        ),
        present("/workspace/pkg/literal", PathNodeKind::Directory),
        missing("/workspace/pkg/literal/BUILD.bazel"),
        missing("/workspace/pkg/literal/BUILD"),
        listing(
            "/workspace/pkg/literal",
            vec![
                (b"leaf.txt", PathDirectoryEntryKind::File),
                (b"deep", PathDirectoryEntryKind::Directory),
            ],
        ),
        present("/workspace/pkg/literal/deep", PathNodeKind::Directory),
        missing("/workspace/pkg/literal/deep/BUILD.bazel"),
        missing("/workspace/pkg/literal/deep/BUILD"),
        listing(
            "/workspace/pkg/literal/deep",
            vec![(b"deep.txt", PathDirectoryEntryKind::File)],
        ),
        missing("/workspace/pkg/literal/literal"),
        missing("/workspace/pkg/literal/deep/literal"),
    ]);
    assert_eq!(
        matches(
            compute(
                &key(b"literal/**", HostGlobTraversalOperation::Files),
                entries.clone(),
                &[]
            )
            .await
        ),
        vec![
            b"literal/deep/deep.txt".to_vec(),
            b"literal/leaf.txt".to_vec()
        ]
    );
    assert_eq!(
        matches(
            compute(
                &key(b"**/literal/*.txt", HostGlobTraversalOperation::Files),
                entries.clone(),
                &[]
            )
            .await
        ),
        vec![b"literal/leaf.txt".to_vec()]
    );
    assert_eq!(
        matches(
            compute(
                &key(b"literal/**", HostGlobTraversalOperation::FilesAndDirs),
                entries.clone(),
                &[]
            )
            .await
        ),
        vec![
            b"literal".to_vec(),
            b"literal/deep".to_vec(),
            b"literal/deep/deep.txt".to_vec(),
            b"literal/leaf.txt".to_vec(),
        ]
    );
    assert_eq!(
        matches(
            compute(
                &key(b"literal/*", HostGlobTraversalOperation::FilesAndDirs),
                entries,
                &[]
            )
            .await
        ),
        vec![b"literal/deep".to_vec(), b"literal/leaf.txt".to_vec()]
    );
}

#[tokio::test]
async fn boundary_stops_packages_before_matching_or_descent() {
    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![
                (b"keep.txt", PathDirectoryEntryKind::File),
                (b"nested", PathDirectoryEntryKind::Directory),
            ],
        ),
        present("/workspace/pkg/nested", PathNodeKind::Directory),
        present(
            "/workspace/pkg/nested/BUILD.bazel",
            PathNodeKind::RegularFile,
        ),
        missing("/workspace/pkg/nested/BUILD"),
    ]);
    assert_eq!(
        matches(compute(&key(b"*", HostGlobTraversalOperation::Files), entries, &[]).await),
        vec![b"keep.txt".to_vec()]
    );
}

#[tokio::test]
async fn double_recursive_wildcards_activate_each_candidate_key_once() {
    let tracker = Arc::new(TraversalTracker::default());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![(b"a", PathDirectoryEntryKind::Directory)],
        ),
        present("/workspace/pkg/a", PathNodeKind::Directory),
        missing("/workspace/pkg/a/BUILD.bazel"),
        missing("/workspace/pkg/a/BUILD"),
        listing(
            "/workspace/pkg/a",
            vec![(b"leaf.txt", PathDirectoryEntryKind::File)],
        ),
        missing("/workspace/pkg/leaf.txt"),
        present("/workspace/pkg/a/leaf.txt", PathNodeKind::RegularFile),
    ]);
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        ..Default::default()
    });
    inject_root_package_policy_inputs(&mut updater, policy(&[])).unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(entries).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    assert_eq!(
        matches(
            transaction
                .compute(&key(b"**/**/leaf.txt", HostGlobTraversalOperation::Files))
                .await
                .unwrap()
        ),
        vec![b"a/leaf.txt".to_vec()]
    );
    // The two `**` routes both reach the root and `a` candidate keys.  The
    // visited state set prevents a second evaluation of either route.
    assert_eq!(tracker.segment_evaluated.load(Ordering::SeqCst), 4);

    let duplicate = TraversalState {
        logical_directory: path("/workspace/pkg/a"),
        package: PackagePath::parse("pkg/a").unwrap(),
        relative_path: Arc::from(&b"a"[..]),
        fragment_index: 1,
        ordinal: 0,
    };
    let mut frontier = std::collections::VecDeque::new();
    let mut visited = Some(SmallSet::new());
    let mut next_ordinal = 0;
    enqueue(
        &mut frontier,
        &mut visited,
        &mut next_ordinal,
        duplicate.clone(),
    );
    enqueue(&mut frontier, &mut visited, &mut next_ordinal, duplicate);
    assert_eq!(frontier.len(), 1);
    assert_eq!(next_ordinal, 1);
}

#[tokio::test]
async fn boundary_kinds_continue_or_stop_and_ignore_precedes_lookup() {
    let tracker = Arc::new(TraversalTracker::default());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut entries = prelude();
    entries.pop();
    entries.extend([
        present("/workspace/.bazelignore", PathNodeKind::RegularFile),
        bytes("/workspace/.bazelignore", b"pkg/ignored\n"),
    ]);
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![
                (b"ordinary", PathDirectoryEntryKind::Directory),
                (b"deleted", PathDirectoryEntryKind::Directory),
                (b"package", PathDirectoryEntryKind::Directory),
                (b"ignored", PathDirectoryEntryKind::Directory),
            ],
        ),
        present("/workspace/pkg/ordinary", PathNodeKind::Directory),
        present("/workspace/pkg/deleted", PathNodeKind::Directory),
        present("/workspace/pkg/package", PathNodeKind::Directory),
        present("/workspace/pkg/ignored", PathNodeKind::Directory),
        missing("/workspace/pkg/ordinary/BUILD.bazel"),
        missing("/workspace/pkg/ordinary/BUILD"),
        present(
            "/workspace/pkg/package/BUILD.bazel",
            PathNodeKind::RegularFile,
        ),
        missing("/workspace/pkg/package/BUILD"),
        present("/workspace/pkg/ordinary/leaf", PathNodeKind::RegularFile),
        present("/workspace/pkg/deleted/leaf", PathNodeKind::RegularFile),
    ]);
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        ..Default::default()
    });
    inject_root_package_policy_inputs(&mut updater, policy(&["pkg/deleted"])).unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(entries).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    assert_eq!(
        matches(
            transaction
                .compute(&key(b"*/leaf", HostGlobTraversalOperation::Files))
                .await
                .unwrap()
        ),
        vec![b"deleted/leaf".to_vec(), b"ordinary/leaf".to_vec()]
    );
    let activated = tracker.activated.lock().unwrap();
    assert!(
        !activated
            .iter()
            .any(|key| key == "host-root-package-lookup:\"/workspace\"//pkg/ignored")
    );
    assert!(!activated.iter().any(|key| {
        key.contains("/workspace/pkg/ignored/BUILD.bazel")
            || key.contains("/workspace/pkg/ignored/BUILD\"")
    }));
}

#[tokio::test]
async fn reached_complete_segment_error_wins_over_an_earlier_sibling_need() {
    let mut entries = prelude();
    entries.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![
                (b"a", PathDirectoryEntryKind::Directory),
                (b"b", PathDirectoryEntryKind::Directory),
            ],
        ),
        present("/workspace/pkg/a", PathNodeKind::Directory),
        present("/workspace/pkg/b", PathNodeKind::Directory),
        missing("/workspace/pkg/a/BUILD.bazel"),
        missing("/workspace/pkg/a/BUILD"),
        missing("/workspace/pkg/b/BUILD.bazel"),
        missing("/workspace/pkg/b/BUILD"),
        lstat_error("/workspace/pkg/b/leaf"),
    ]);
    let outcome = compute(
        &key(b"*/leaf", HostGlobTraversalOperation::Files),
        entries,
        &[],
    )
    .await;
    let SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("the reached complete error must beat the sibling Need")
    };
    assert!(matches!(
        value.as_ref(),
        Err(HostGlobTraversalError::Segment {
            logical_directory,
            fragment_index: 1,
            ..
        }) if logical_directory == &path("/workspace/pkg/b")
    ));
}

#[tokio::test]
async fn boundary_error_beats_sibling_need_and_uses_fifo_candidate_rank() {
    let script = |error_a| {
        let mut entries = prelude();
        entries.extend([
            present("/workspace/pkg", PathNodeKind::Directory),
            listing(
                "/workspace/pkg",
                vec![
                    (b"a", PathDirectoryEntryKind::Directory),
                    (b"b", PathDirectoryEntryKind::Directory),
                ],
            ),
            present("/workspace/pkg/a", PathNodeKind::Directory),
            present("/workspace/pkg/b", PathNodeKind::Directory),
        ]);
        if error_a {
            entries.push(lstat_error("/workspace/pkg/b/BUILD.bazel"));
            entries.push(lstat_error("/workspace/pkg/a/BUILD.bazel"));
        } else {
            entries.push(lstat_error("/workspace/pkg/b/BUILD.bazel"));
        }
        entries
    };
    let boundary_error = |outcome: HostGlobTraversalOutcome, candidate: &str| {
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("the reached complete boundary error must beat a sibling Need")
        };
        assert!(matches!(
            value.as_ref(),
            Err(HostGlobTraversalError::Boundary { candidate_package, .. })
                if candidate_package == &PackagePath::parse(candidate).unwrap()
        ));
    };
    boundary_error(
        compute(
            &key(b"*", HostGlobTraversalOperation::Files),
            script(false),
            &[],
        )
        .await,
        "pkg/b",
    );
    // Both errors are complete. Candidate listing order, rather than the order
    // in which their underlying observations are supplied, selects `a`.
    boundary_error(
        compute(
            &key(b"*", HostGlobTraversalOperation::Files),
            script(true),
            &[],
        )
        .await,
        "pkg/a",
    );
}

#[tokio::test]
async fn same_graph_marker_deleted_and_ignore_transitions_restore_equal_value() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(TraversalTracker::default());
    let traversal = key(b"*", HostGlobTraversalOperation::FilesAndDirs);
    let script = |marker, ignored| {
        let mut entries = prelude();
        entries.pop();
        if ignored {
            entries.extend([
                present("/workspace/.bazelignore", PathNodeKind::RegularFile),
                bytes("/workspace/.bazelignore", b"pkg/entry\n"),
            ]);
        } else {
            entries.push(missing("/workspace/.bazelignore"));
        }
        entries.extend([
            present("/workspace/pkg", PathNodeKind::Directory),
            listing(
                "/workspace/pkg",
                vec![(b"entry", PathDirectoryEntryKind::Directory)],
            ),
            present("/workspace/pkg/entry", PathNodeKind::Directory),
            if marker {
                present(
                    "/workspace/pkg/entry/BUILD.bazel",
                    PathNodeKind::RegularFile,
                )
            } else {
                missing("/workspace/pkg/entry/BUILD.bazel")
            },
            missing("/workspace/pkg/entry/BUILD"),
        ]);
        entries
    };
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        ..Default::default()
    });
    inject_root_package_policy_inputs(&mut updater, policy(&[])).unwrap();
    updater
        .changed_to(vec![(
            (PathObservationEpochKey),
            PathObservationEpoch::new(script(false, false)).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    assert_eq!(
        matches(transaction.compute(&traversal).await.unwrap()),
        vec![b"entry".to_vec()]
    );

    for (marker, ignored, deleted, expected) in [
        (true, false, Vec::new(), Vec::new()),
        (true, false, vec!["pkg/entry"], vec![b"entry".to_vec()]),
        (true, true, vec!["pkg/entry"], Vec::new()),
        (false, false, Vec::new(), vec![b"entry".to_vec()]),
    ] {
        let mut updater = transaction.into_updater();
        inject_root_package_policy_inputs(&mut updater, policy(&deleted)).unwrap();
        updater
            .changed_to(vec![(
                (PathObservationEpochKey),
                PathObservationEpoch::new(script(marker, ignored)).unwrap(),
            )])
            .unwrap();
        transaction = updater.commit().await;
        assert_eq!(
            matches(transaction.compute(&traversal).await.unwrap()),
            expected
        );
    }
    assert_eq!(tracker.evaluated.load(Ordering::SeqCst), 5);
}

#[tokio::test]
async fn observed_warm_and_create_delete_restore_reactivate_and_restore_value() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(TraversalTracker::default());
    let key = observed_key(b"entry", HostGlobTraversalOperation::Files);
    let script = |present_entry| {
        let mut entries = prelude();
        entries.push(present("/workspace/pkg", PathNodeKind::Directory));
        entries.push(if present_entry {
            present("/workspace/pkg/entry", PathNodeKind::RegularFile)
        } else {
            missing("/workspace/pkg/entry")
        });
        entries
    };
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        ..Default::default()
    });
    inject_root_package_policy_inputs(&mut updater, policy(&[])).unwrap();
    updater
        .changed_to(vec![(
            (PathObservationEpochKey),
            PathObservationEpoch::new(script(true)).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    assert_eq!(
        observed_matches(&transaction.compute(&key).await.unwrap()),
        vec![b"entry".to_vec()]
    );
    assert_eq!(
        observed_matches(&transaction.compute(&key).await.unwrap()),
        vec![b"entry".to_vec()]
    );
    assert_eq!(tracker.observed_evaluated.load(Ordering::SeqCst), 1);

    for (present_entry, expected) in [(false, Vec::new()), (true, vec![b"entry".to_vec()])] {
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                (PathObservationEpochKey),
                PathObservationEpoch::new(script(present_entry)).unwrap(),
            )])
            .unwrap();
        transaction = updater.commit().await;
        assert_eq!(
            observed_matches(&transaction.compute(&key).await.unwrap()),
            expected
        );
    }
    assert_eq!(tracker.observed_evaluated.load(Ordering::SeqCst), 3);
}
