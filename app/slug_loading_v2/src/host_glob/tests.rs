/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

use std::ffi::OsString;
use std::fmt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
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
use dupe::Dupe;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::*;

use super::traversal::HostGlobTraversalKey;
use super::traversal::HostGlobTraversalOperation;
use super::*;
use crate::glob::GlobPattern;

fn path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(value).unwrap()
}

fn pattern(value: &str) -> HostGlobSegmentPattern {
    GlobPattern::include(value)
        .unwrap()
        .segment(0)
        .expect("test segment pattern must contain one ordinary segment")
}

#[test]
fn shared_segment_view_preserves_kind_bytes_and_cross_pattern_identity() {
    let literal = pattern("é.txt");
    assert_eq!(literal.kind(), HostGlobSegmentPatternKind::Literal);
    assert_eq!(literal.bytes(), "é.txt".as_bytes());
    let wildcard = pattern("a*b*c.txt");
    assert_eq!(wildcard.kind(), HostGlobSegmentPatternKind::Wildcard);
    assert_eq!(wildcard.bytes(), b"a*b*c.txt");

    let same = GlobPattern::include("other/a*b*c.txt")
        .unwrap()
        .segment(1)
        .unwrap();
    let different = pattern("a*b*d.txt");
    assert_eq!(wildcard, same);
    assert_ne!(wildcard, different);
}

#[test]
fn simple_matcher_preserves_raw_dot_and_nonadjacent_star_semantics() {
    assert!(glob_segment_matches(&pattern("*"), b".hidden.txt"));
    assert!(!glob_segment_matches(&pattern("*.txt"), b".hidden.txt"));
    assert!(glob_segment_matches(&pattern(".h*.txt"), b".hidden.txt"));
    assert!(glob_segment_matches(
        &pattern("m*id*end.txt"),
        b"m-left-id-right-end.txt"
    ));
    assert!(!glob_segment_matches(&pattern("é*.txt"), b"\xe9.txt"));
    assert!(glob_segment_matches(&pattern("é*.txt"), "é.txt".as_bytes()));
    assert!(!glob_segment_matches(&pattern("a*b*c"), b"a-x-X-c"));
    assert!(!glob_segment_matches(&pattern("*"), b""));
}

#[test]
fn candidate_value_sorts_raw_bytes_and_preserves_equal_name_order() {
    let candidates = HostGlobSegmentCandidates::from_vec(vec![
        HostGlobSegmentCandidate {
            component: Arc::from(&b"\xe9"[..]),
            kind: HostGlobSegmentCandidateKind::Directory,
        },
        HostGlobSegmentCandidate {
            component: Arc::from(&b"same"[..]),
            kind: HostGlobSegmentCandidateKind::Directory,
        },
        HostGlobSegmentCandidate {
            component: Arc::from(&b"\xc3\xa9"[..]),
            kind: HostGlobSegmentCandidateKind::NonDirectory,
        },
        HostGlobSegmentCandidate {
            component: Arc::from(&b"same"[..]),
            kind: HostGlobSegmentCandidateKind::NonDirectory,
        },
    ]);
    let projected = candidates
        .candidates()
        .iter()
        .map(|candidate| (candidate.component.as_ref(), candidate.kind))
        .collect::<Vec<_>>();
    assert_eq!(
        projected,
        vec![
            (b"same".as_slice(), HostGlobSegmentCandidateKind::Directory),
            (
                b"same".as_slice(),
                HostGlobSegmentCandidateKind::NonDirectory
            ),
            (
                b"\xc3\xa9".as_slice(),
                HostGlobSegmentCandidateKind::NonDirectory
            ),
            (b"\xe9".as_slice(), HostGlobSegmentCandidateKind::Directory),
        ]
    );
}

#[test]
fn key_complete_only_equality_and_need_validity_are_exact() {
    let complete: HostGlobSegmentOutcome =
        SourcePreparationOutcome::Complete(Arc::new(Ok(HostGlobSegmentCandidates::empty())));
    let separately_allocated_equal: HostGlobSegmentOutcome =
        SourcePreparationOutcome::Complete(Arc::new(Ok(HostGlobSegmentCandidates::empty())));
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        path("/missing"),
        PathObservationOperation::Lstat,
    );
    let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
        NeedPathObservations::singleton(demand),
    ));
    assert!(HostGlobSegmentCandidatesKey::validity(&complete));
    assert!(HostGlobSegmentCandidatesKey::equality(
        &complete,
        &separately_allocated_equal
    ));
    assert!(!HostGlobSegmentCandidatesKey::validity(&need));
    assert!(!HostGlobSegmentCandidatesKey::equality(&need, &need));
    assert!(!HostGlobSegmentCandidatesKey::equality(&complete, &need));

    let observed_complete =
        SourcePreparationOutcome::Complete(Ok(ObservedHostGlobSegmentCandidates {
            result: Arc::new(Ok(HostGlobSegmentCandidates::empty())),
            observations: PathObservationEpoch::empty(),
        }));
    let observed_equal = observed_complete.dupe();
    let observed_need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
        NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/observed-missing"),
            PathObservationOperation::Lstat,
        )),
    ));
    assert!(HostGlobSegmentCandidatesObservationKey::validity(
        &observed_complete
    ));
    assert!(HostGlobSegmentCandidatesObservationKey::equality(
        &observed_complete,
        &observed_equal
    ));
    assert!(!HostGlobSegmentCandidatesObservationKey::validity(
        &observed_need
    ));
    assert!(!HostGlobSegmentCandidatesObservationKey::equality(
        &observed_need,
        &observed_need
    ));
}

#[test]
fn semantic_error_equality_strips_operational_routes_and_namespaces() {
    let error = PathObservationError::Io {
        kind: PathIoErrorKind::PermissionDenied,
        raw_os_error: Some(13),
    };
    let first = PathResolutionError::Observation {
        namespace: PathObservationNamespace::Host,
        requested_path: path("/requested-one"),
        demand: PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/physical-one"),
            PathObservationOperation::Lstat,
        ),
        error,
    };
    let second = PathResolutionError::Observation {
        namespace: PathObservationNamespace::Materialization(PathObservationInstanceId::new(7)),
        requested_path: path("/requested-two"),
        demand: PathObservationDemand::new(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(8)),
            path("/physical-two"),
            PathObservationOperation::Lstat,
        ),
        error,
    };

    assert_eq!(
        resolution_error(&path("/pkg"), Arc::from(&b"entry"[..]), first),
        resolution_error(&path("/pkg"), Arc::from(&b"entry"[..]), second)
    );
}

type ScriptEntry = (PathObservationDemand, PathObservationResult);

fn lstat(kind: PathNodeKind) -> PathLstat {
    PathLstat::new(kind, 1, 2, 3, 4, 0o755)
}

fn demand(
    path: NormalizedAbsolutePath,
    operation: PathObservationOperation,
) -> PathObservationDemand {
    PathObservationDemand::new(PathObservationNamespace::Host, path, operation)
}

fn lstat_result(
    value: NormalizedAbsolutePath,
    result: PathOperationResult<PathLstat>,
) -> ScriptEntry {
    (
        demand(value, PathObservationOperation::Lstat),
        PathObservationResult::Lstat(result),
    )
}

fn present(value: &str, kind: PathNodeKind) -> ScriptEntry {
    lstat_result(path(value), PathOperationResult::Present(lstat(kind)))
}

fn missing(value: &str) -> ScriptEntry {
    lstat_result(path(value), PathOperationResult::Missing)
}

fn directory_entries(entries: Vec<(OsString, PathDirectoryEntryKind)>) -> ScriptEntry {
    directory_entries_at("/pkg", entries)
}

fn directory_entries_at(
    value: &str,
    entries: Vec<(OsString, PathDirectoryEntryKind)>,
) -> ScriptEntry {
    let entries =
        PathDirectoryEntries::new(entries.into_iter().map(|(name, kind)| {
            PathDirectoryEntry::new(PathDirectoryName::new(name).unwrap(), kind)
        }));
    (
        demand(path(value), PathObservationOperation::DirectoryEntries),
        PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries)),
    )
}

fn read_link(value: &str, target: &str) -> ScriptEntry {
    (
        demand(path(value), PathObservationOperation::ReadLink),
        PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(PathBuf::from(
            target,
        )))),
    )
}

async fn compute(
    pattern_bytes: &[u8],
    script: impl IntoIterator<Item = ScriptEntry>,
) -> HostGlobSegmentOutcome {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(script).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    transaction
        .compute(&HostGlobSegmentCandidatesKey::new(
            path("/pkg"),
            pattern(std::str::from_utf8(pattern_bytes).unwrap()),
        ))
        .await
        .unwrap()
}

async fn compute_observed(
    pattern_bytes: &[u8],
    script: impl IntoIterator<Item = ScriptEntry>,
) -> (ObservedHostGlobSegmentOutcome, PathObservationEpoch) {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let epoch = PathObservationEpoch::new(script).unwrap();
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    let mut transaction = updater.commit().await;
    let outcome = transaction
        .compute(&HostGlobSegmentCandidatesObservationKey::new(
            path("/pkg"),
            pattern(std::str::from_utf8(pattern_bytes).unwrap()),
        ))
        .await
        .unwrap();
    (outcome, epoch)
}

fn observed_value(outcome: &ObservedHostGlobSegmentOutcome) -> &ObservedHostGlobSegmentCandidates {
    let SourcePreparationOutcome::Complete(Ok(value)) = outcome else {
        panic!("expected complete observed segment: {outcome:?}")
    };
    value
}

fn base_listing(entries: Vec<(OsString, PathDirectoryEntryKind)>) -> Vec<ScriptEntry> {
    vec![
        present("/", PathNodeKind::Directory),
        present("/pkg", PathNodeKind::Directory),
        directory_entries(entries),
    ]
}

fn unwrap_ok(outcome: HostGlobSegmentOutcome) -> HostGlobSegmentCandidates {
    let SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("expected complete outcome")
    };
    value.as_ref().as_ref().unwrap().dupe()
}

static CONSUMER_EVALUATIONS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
struct HostGlobConsumerKey {
    host: HostGlobSegmentCandidatesKey,
}

impl fmt::Display for HostGlobConsumerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "test-host-glob-consumer:{}", self.host)
    }
}

#[async_trait]
impl Key for HostGlobConsumerKey {
    type Value = HostGlobSegmentOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let value = ctx.compute(&self.host).await.unwrap();
        CONSUMER_EVALUATIONS.fetch_add(1, Ordering::SeqCst);
        value
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Default)]
struct HostGlobTracker {
    evaluated: AtomicUsize,
    evaluated_with_data: AtomicUsize,
    observed_evaluated: AtomicUsize,
    observed_with_data: AtomicUsize,
    observed_direct_resolutions: AtomicUsize,
    legacy_resolutions: AtomicUsize,
    legacy_listings: AtomicUsize,
}

impl ActivationTracker for HostGlobTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        dependencies: &mut dyn Iterator<Item = &DynKey>,
        activation: ActivationData,
    ) {
        if key
            .downcast_ref::<HostGlobSegmentCandidatesObservationKey>()
            .is_some()
        {
            if let ActivationData::Evaluated(data) = activation {
                self.observed_evaluated.fetch_add(1, Ordering::SeqCst);
                if data.is_some() {
                    self.observed_with_data.fetch_add(1, Ordering::SeqCst);
                }
                self.observed_direct_resolutions.fetch_add(
                    dependencies
                        .filter(|dependency| {
                            dependency
                                .downcast_ref::<ResolvedPathObservationKey>()
                                .is_some()
                        })
                        .count(),
                    Ordering::SeqCst,
                );
            }
            return;
        }
        if key.downcast_ref::<ResolvedPathKey>().is_some()
            && matches!(activation, ActivationData::Evaluated(_))
        {
            self.legacy_resolutions.fetch_add(1, Ordering::SeqCst);
        }
        if key.downcast_ref::<PathDirectoryListingKey>().is_some()
            && matches!(activation, ActivationData::Evaluated(_))
        {
            self.legacy_listings.fetch_add(1, Ordering::SeqCst);
        }
        if key.downcast_ref::<HostGlobSegmentCandidatesKey>().is_some()
            && let ActivationData::Evaluated(data) = activation
        {
            self.evaluated.fetch_add(1, Ordering::SeqCst);
            if data.is_some() {
                self.evaluated_with_data.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}

#[test]
fn pending_prefix_ignores_later_outer_and_full_batch_outer_wins_need() {
    let base_epoch =
        PathObservationEpoch::new([present("/base", PathNodeKind::Directory)]).unwrap();
    let base_demand = demand(path("/base"), PathObservationOperation::Lstat);
    let base_arc = base_epoch.get(&base_demand).unwrap().dupe();
    let semantic_demand = demand(path("/pkg/b-link"), PathObservationOperation::Lstat);
    let semantic_error = PathObservationError::Io {
        kind: PathIoErrorKind::PermissionDenied,
        raw_os_error: Some(13),
    };
    let semantic_epoch = PathObservationEpoch::new([
        present("/base", PathNodeKind::Directory),
        lstat_result(
            path("/pkg/b-link"),
            PathOperationResult::Error(semantic_error),
        ),
    ])
    .unwrap();
    let semantic_arc = semantic_epoch.get(&semantic_demand).unwrap().dupe();
    let need_demand = demand(path("/pkg/a-link"), PathObservationOperation::Lstat);
    let outer_demand = demand(path("/pkg/c-link"), PathObservationOperation::Lstat);
    let outer = ObservedPathFrontierError::from(PathObservationEpochError::OperationMismatch {
        demand: outer_demand.dupe(),
        result_operation: PathObservationOperation::DirectoryEntries,
    });
    let pending = |slot, name: &'static [u8]| PendingSymlink {
        slot,
        component: Arc::from(name),
        logical_path: path(&format!("/pkg/{}", String::from_utf8_lossy(name))),
    };
    let key = HostGlobSegmentCandidatesKey::new(path("/pkg"), pattern("*"));
    let outcomes = vec![
        (
            pending(0, b"a-link"),
            PathOutcome::Need(NeedPathObservations::singleton(need_demand.dupe())),
        ),
        (
            pending(1, b"b-link"),
            PathOutcome::Complete(Ok(SegmentInput {
                result: Err(PathResolutionError::Observation {
                    namespace: PathObservationNamespace::Host,
                    requested_path: path("/pkg/b-link"),
                    demand: semantic_demand.dupe(),
                    error: semantic_error,
                }),
                observations: semantic_epoch,
            })),
        ),
        (
            pending(2, b"c-link"),
            PathOutcome::Complete(Err(outer.clone())),
        ),
    ];
    let outcome = key.finish_pending_symlinks(
        PendingSymlinkBatch {
            slots: vec![None, None, None],
            pending: Vec::new(),
            directory_real_path: path("/pkg"),
            observations: base_epoch.dupe(),
        },
        outcomes,
    );
    let SourcePreparationOutcome::Complete(Ok((result, observations))) = outcome else {
        panic!("the first semantic terminal must bound the prefix")
    };
    assert!(matches!(
        result.as_ref(),
        Err(HostGlobSegmentError::Observation { component, .. })
            if component.as_ref() == b"b-link"
    ));
    assert_eq!(observations.observations().len(), 2);
    assert!(Arc::ptr_eq(
        observations.get(&base_demand).unwrap(),
        &base_arc
    ));
    assert!(Arc::ptr_eq(
        observations.get(&semantic_demand).unwrap(),
        &semantic_arc
    ));
    assert!(observations.get(&outer_demand).is_none());

    let outcome = key.finish_pending_symlinks(
        PendingSymlinkBatch {
            slots: vec![None, None],
            pending: Vec::new(),
            directory_real_path: path("/pkg"),
            observations: base_epoch,
        },
        vec![
            (
                pending(0, b"a-link"),
                PathOutcome::Need(NeedPathObservations::singleton(need_demand)),
            ),
            (pending(1, b"c-link"), PathOutcome::Complete(Err(outer))),
        ],
    );
    assert!(matches!(
        outcome,
        SourcePreparationOutcome::Complete(Err(_))
    ));
}

#[tokio::test]
async fn observed_terminal_matrix_and_matched_symlinks_preserve_exact_arcs() {
    let denied = PathObservationError::Io {
        kind: PathIoErrorKind::PermissionDenied,
        raw_os_error: Some(13),
    };
    for (glob, script, expected) in [
        (
            b"literal".as_slice(),
            vec![
                present("/", PathNodeKind::Directory),
                present("/pkg", PathNodeKind::Directory),
                present("/pkg/literal", PathNodeKind::SpecialFile),
            ],
            "present",
        ),
        (
            b"*.txt".as_slice(),
            base_listing(vec![(
                OsString::from("entry.txt"),
                PathDirectoryEntryKind::File,
            )]),
            "present",
        ),
        (
            b"literal".as_slice(),
            vec![
                present("/", PathNodeKind::Directory),
                present("/pkg", PathNodeKind::Directory),
                missing("/pkg/literal"),
            ],
            "empty",
        ),
        (
            b"literal".as_slice(),
            vec![
                present("/", PathNodeKind::Directory),
                present("/pkg", PathNodeKind::Directory),
                lstat_result(path("/pkg/literal"), PathOperationResult::Error(denied)),
            ],
            "error",
        ),
        (
            b"*".as_slice(),
            vec![present("/", PathNodeKind::Directory), missing("/pkg")],
            "error",
        ),
    ] {
        let legacy = compute(glob, script.clone()).await;
        let (outcome, epoch) = compute_observed(glob, script).await;
        let value = observed_value(&outcome);
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("legacy must complete")
        };
        assert_eq!(value.result.as_ref(), legacy.as_ref());
        assert_eq!(value.result.is_err(), expected == "error");
        if expected == "empty" {
            assert!(
                value
                    .result
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .candidates()
                    .is_empty()
            );
        }
        for (demand, result) in value.observations.observations() {
            assert!(Arc::ptr_eq(result, epoch.get(demand).unwrap()));
        }
    }
    for target in [Some(("/file", PathNodeKind::RegularFile)), None] {
        let mut script = base_listing(vec![(
            OsString::from("match"),
            PathDirectoryEntryKind::Symlink,
        )]);
        script.extend([
            present("/pkg/match", PathNodeKind::Symlink),
            read_link("/pkg/match", target.map_or("/missing", |value| value.0)),
        ]);
        script.push(target.map_or_else(|| missing("/missing"), |(path, kind)| present(path, kind)));
        let legacy = compute(b"m*tch", script.clone()).await;
        let (observed, epoch) = compute_observed(b"m*tch", script).await;
        let SourcePreparationOutcome::Complete(legacy) = &legacy else {
            panic!("legacy must complete")
        };
        assert_eq!(observed_value(&observed).result.as_ref(), legacy.as_ref());
        for (demand, result) in epoch.observations() {
            assert!(Arc::ptr_eq(
                observed_value(&observed).observations.get(demand).unwrap(),
                result
            ));
        }
    }
    let (need, _) = compute_observed(b"literal", []).await;
    assert!(matches!(need, SourcePreparationOutcome::Need(_)));
}

#[tokio::test]
async fn observed_family_isolated_and_traversal_remains_legacy() {
    let tracker = Arc::new(HostGlobTracker::default());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        ..Default::default()
    });
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(base_listing(vec![(
                OsString::from("file"),
                PathDirectoryEntryKind::File,
            )]))
            .unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let observed_key = HostGlobSegmentCandidatesObservationKey::new(path("/pkg"), pattern("*"));
    let mut cancelled = Box::pin(transaction.compute(&observed_key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(cancelled.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(cancelled);
    assert_eq!(tracker.observed_evaluated.load(Ordering::SeqCst), 0);
    drop(transaction);
    let mut transaction = dice
        .updater_with_data(UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        })
        .commit()
        .await;
    assert!(matches!(
        transaction.compute(&observed_key).await.unwrap(),
        SourcePreparationOutcome::Complete(Ok(_))
    ));
    assert_eq!(
        tracker.observed_direct_resolutions.load(Ordering::SeqCst),
        0
    );
    assert_eq!(tracker.legacy_resolutions.load(Ordering::SeqCst), 0);
    assert_eq!(tracker.legacy_listings.load(Ordering::SeqCst), 0);
    assert_eq!(tracker.observed_with_data.load(Ordering::SeqCst), 0);
    let observed_count = tracker.observed_evaluated.load(Ordering::SeqCst);
    transaction
        .compute(&HostGlobSegmentCandidatesKey::new(
            path("/pkg"),
            pattern("*"),
        ))
        .await
        .unwrap();
    assert!(tracker.legacy_resolutions.load(Ordering::SeqCst) > 0);
    assert!(tracker.legacy_listings.load(Ordering::SeqCst) > 0);

    let mut updater = transaction.into_updater();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            path("/workspace"),
            vec![path("/workspace")],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new([
                present("/", PathNodeKind::Directory),
                present("/workspace", PathNodeKind::Directory),
                present("/workspace/pkg", PathNodeKind::Directory),
                present("/workspace/pkg/entry", PathNodeKind::RegularFile),
            ])
            .unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let traversal = HostGlobTraversalKey::new(
        path("/workspace"),
        super::traversal::HostGlobBoundaryScope::Root,
        path("/workspace"),
        PackagePath::parse("pkg").unwrap(),
        GlobPattern::include("entry").unwrap(),
        HostGlobTraversalOperation::Files,
    )
    .unwrap();
    assert!(matches!(
        transaction.compute(&traversal).await.unwrap(),
        SourcePreparationOutcome::Complete(_)
    ));
    assert_eq!(
        tracker.observed_evaluated.load(Ordering::SeqCst),
        observed_count
    );
}

#[tokio::test]
async fn wildcard_uses_raw_typed_listing_and_ignores_unmatched_symlinks() {
    let entries = vec![
        (
            OsString::from_vec(b"\xe9.txt".to_vec()),
            PathDirectoryEntryKind::File,
        ),
        (
            OsString::from_vec(b"\xc3\xa9.txt".to_vec()),
            PathDirectoryEntryKind::Directory,
        ),
        (
            OsString::from("unknown.txt"),
            PathDirectoryEntryKind::Unknown,
        ),
        (
            OsString::from("unmatched-cycle"),
            PathDirectoryEntryKind::Symlink,
        ),
    ];
    let value = unwrap_ok(compute(b"*.txt", base_listing(entries)).await);
    let projected = value
        .candidates()
        .iter()
        .map(|candidate| (candidate.component.as_ref(), candidate.kind))
        .collect::<Vec<_>>();
    assert_eq!(
        projected,
        vec![
            (
                b"\xc3\xa9.txt".as_slice(),
                HostGlobSegmentCandidateKind::Directory
            ),
            (
                b"\xe9.txt".as_slice(),
                HostGlobSegmentCandidateKind::NonDirectory
            ),
        ]
    );
}

#[tokio::test]
async fn wildcard_propagates_stable_duplicate_names() {
    let value = unwrap_ok(
        compute(
            b"*",
            base_listing(vec![
                (OsString::from("same"), PathDirectoryEntryKind::Directory),
                (OsString::from("same"), PathDirectoryEntryKind::File),
            ]),
        )
        .await,
    );
    assert_eq!(
        value
            .candidates()
            .iter()
            .map(|candidate| candidate.kind)
            .collect::<Vec<_>>(),
        vec![
            HostGlobSegmentCandidateKind::Directory,
            HostGlobSegmentCandidateKind::NonDirectory,
        ]
    );
}

#[tokio::test]
async fn literal_bypasses_listing_and_classifies_special_directory_and_missing() {
    for (kind, expected) in [
        (
            PathNodeKind::SpecialFile,
            Some(HostGlobSegmentCandidateKind::NonDirectory),
        ),
        (
            PathNodeKind::Directory,
            Some(HostGlobSegmentCandidateKind::Directory),
        ),
    ] {
        let value = unwrap_ok(
            compute(
                b"literal",
                [
                    present("/", PathNodeKind::Directory),
                    present("/pkg", PathNodeKind::Directory),
                    present("/pkg/literal", kind),
                ],
            )
            .await,
        );
        assert_eq!(
            value.candidates().first().map(|candidate| candidate.kind),
            expected
        );
    }
    let missing = unwrap_ok(
        compute(
            b"literal",
            [
                present("/", PathNodeKind::Directory),
                present("/pkg", PathNodeKind::Directory),
                missing("/pkg/literal"),
            ],
        )
        .await,
    );
    assert!(missing.candidates().is_empty());
}

#[tokio::test]
async fn matched_symlink_needs_are_unioned_and_complete_error_wins() {
    let entries = vec![
        (OsString::from("a-link"), PathDirectoryEntryKind::Symlink),
        (OsString::from("b-link"), PathDirectoryEntryKind::Symlink),
    ];
    let outcome = compute(b"*-link", base_listing(entries.clone())).await;
    let SourcePreparationOutcome::Need(needs) = outcome else {
        panic!("expected matched symlink needs")
    };
    let demands = needs.path_observations().unwrap().demands();
    assert_eq!(demands.len(), 2);
    assert_eq!(
        demands[0].path().as_path().as_os_str().as_bytes(),
        b"/pkg/a-link"
    );
    assert_eq!(
        demands[1].path().as_path().as_os_str().as_bytes(),
        b"/pkg/b-link"
    );

    let mut script = base_listing(entries);
    script.push(lstat_result(
        path("/pkg/a-link"),
        PathOperationResult::Error(PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        }),
    ));
    let outcome = compute(b"*-link", script).await;
    let SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("complete error must win over sibling Need")
    };
    assert!(matches!(
        value.as_ref(),
        Err(HostGlobSegmentError::Observation {
            component,
            operation: PathObservationOperation::Lstat,
            ..
        }) if component.as_ref() == b"a-link"
    ));
}

#[tokio::test]
async fn matched_symlink_classifies_file_directory_and_dangling() {
    async fn resolve(target: &str, target_kind: Option<PathNodeKind>) -> HostGlobSegmentCandidates {
        let entries = vec![(OsString::from("match"), PathDirectoryEntryKind::Symlink)];
        let mut script = base_listing(entries);
        script.push(present("/pkg/match", PathNodeKind::Symlink));
        script.push(read_link("/pkg/match", target));
        script.push(match target_kind {
            Some(kind) => present(target, kind),
            None => missing(target),
        });
        unwrap_ok(compute(b"m*tch", script).await)
    }

    let file = resolve("/file", Some(PathNodeKind::RegularFile)).await;
    assert_eq!(
        file.candidates()[0].kind,
        HostGlobSegmentCandidateKind::NonDirectory
    );
    let directory = resolve("/directory", Some(PathNodeKind::Directory)).await;
    assert_eq!(
        directory.candidates()[0].kind,
        HostGlobSegmentCandidateKind::Directory
    );
    assert!(resolve("/missing", None).await.candidates().is_empty());
}

#[tokio::test]
async fn matched_child_symlink_under_symlinked_directory_uses_physical_identity() {
    let value = unwrap_ok(
        compute(
            b"m*tch",
            [
                present("/", PathNodeKind::Directory),
                present("/pkg", PathNodeKind::Symlink),
                read_link("/pkg", "/real"),
                present("/real", PathNodeKind::Directory),
                directory_entries_at(
                    "/real",
                    vec![(OsString::from("match"), PathDirectoryEntryKind::Symlink)],
                ),
                present("/real/match", PathNodeKind::Symlink),
                read_link("/real/match", "/file"),
                present("/file", PathNodeKind::RegularFile),
            ],
        )
        .await,
    );
    assert_eq!(
        value.candidates(),
        &[HostGlobSegmentCandidate {
            component: Arc::from(&b"match"[..]),
            kind: HostGlobSegmentCandidateKind::NonDirectory,
        }]
    );
}

#[tokio::test]
async fn matched_symlink_cycle_is_a_semantic_error() {
    let mut script = base_listing(vec![(
        OsString::from("match"),
        PathDirectoryEntryKind::Symlink,
    )]);
    script.extend([
        present("/pkg/match", PathNodeKind::Symlink),
        read_link("/pkg/match", "match"),
    ]);

    let SourcePreparationOutcome::Complete(value) = compute(b"m*tch", script).await else {
        panic!("cycle must be complete")
    };
    assert!(matches!(
        value.as_ref(),
        Err(HostGlobSegmentError::Cycle {
            logical_directory,
            component,
        }) if logical_directory == &path("/pkg") && component.as_ref() == b"match"
    ));
}

#[tokio::test]
async fn matched_directory_with_ancestor_expansion_is_a_semantic_error() {
    let mut script = base_listing(vec![(
        OsString::from("match"),
        PathDirectoryEntryKind::Symlink,
    )]);
    script.extend([
        present("/pkg/match", PathNodeKind::Symlink),
        read_link("/pkg/match", "/a/a"),
        present("/a", PathNodeKind::Directory),
        present("/a/a", PathNodeKind::Symlink),
        read_link("/a/a", "../a"),
    ]);

    let SourcePreparationOutcome::Complete(value) = compute(b"m*tch", script).await else {
        panic!("completed ancestor expansion must be complete")
    };
    assert!(matches!(
        value.as_ref(),
        Err(HostGlobSegmentError::InfiniteExpansion {
            logical_directory,
            component,
        }) if logical_directory == &path("/pkg") && component.as_ref() == b"match"
    ));
}

#[tokio::test]
async fn ancestor_expansion_accepts_wildcard_files_and_literal_directories() {
    let expansion_script = |listing| {
        let mut script = base_listing(listing);
        script.extend([
            present("/pkg/match", PathNodeKind::Symlink),
            read_link("/pkg/match", "/a/a/leaf"),
            present("/a", PathNodeKind::Directory),
            present("/a/a", PathNodeKind::Symlink),
            read_link("/a/a", "../a"),
            present("/a/leaf", PathNodeKind::RegularFile),
        ]);
        script
    };
    let wildcard = unwrap_ok(
        compute(
            b"m*tch",
            expansion_script(vec![(
                OsString::from("match"),
                PathDirectoryEntryKind::Symlink,
            )]),
        )
        .await,
    );
    assert_eq!(
        wildcard.candidates()[0].kind,
        HostGlobSegmentCandidateKind::NonDirectory
    );

    let literal = unwrap_ok(
        compute(
            b"match",
            [
                present("/", PathNodeKind::Directory),
                present("/pkg", PathNodeKind::Directory),
                present("/pkg/match", PathNodeKind::Symlink),
                read_link("/pkg/match", "/a/a"),
                present("/a", PathNodeKind::Directory),
                present("/a/a", PathNodeKind::Symlink),
                read_link("/a/a", "../a"),
            ],
        )
        .await,
    );
    assert_eq!(
        literal.candidates()[0].kind,
        HostGlobSegmentCandidateKind::Directory
    );
}

#[tokio::test]
async fn listing_symlink_resolution_disagreement_is_a_semantic_error() {
    let mut script = base_listing(vec![(
        OsString::from("match"),
        PathDirectoryEntryKind::Symlink,
    )]);
    script.push(present("/pkg/match", PathNodeKind::RegularFile));

    let SourcePreparationOutcome::Complete(value) = compute(b"m*tch", script).await else {
        panic!("listing mismatch must be complete")
    };
    assert!(matches!(
        value.as_ref(),
        Err(HostGlobSegmentError::ListingSymlinkResolutionMismatch {
            logical_directory,
            component,
        }) if logical_directory == &path("/pkg") && component.as_ref() == b"match"
    ));
}

#[tokio::test]
async fn same_dice_create_delete_recreate_and_kind_changes_restore_values() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = HostGlobSegmentCandidatesObservationKey::new(path("/pkg"), pattern("*"));
    let scripts = [
        (
            base_listing(vec![(
                OsString::from("entry"),
                PathDirectoryEntryKind::File,
            )]),
            Some(HostGlobSegmentCandidateKind::NonDirectory),
        ),
        (
            base_listing(vec![(
                OsString::from("entry"),
                PathDirectoryEntryKind::Directory,
            )]),
            Some(HostGlobSegmentCandidateKind::Directory),
        ),
        (base_listing(Vec::new()), None),
        (
            base_listing(vec![(
                OsString::from("entry"),
                PathDirectoryEntryKind::File,
            )]),
            Some(HostGlobSegmentCandidateKind::NonDirectory),
        ),
    ];
    let mut transaction = dice.updater().commit().await;
    let mut first = None;

    for (index, (script, expected)) in scripts.into_iter().enumerate() {
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(script).unwrap(),
            )])
            .unwrap();
        transaction = updater.commit().await;

        let outcome = transaction.compute(&key).await.unwrap();
        let value = observed_value(&outcome).result.as_ref().as_ref().unwrap();
        assert_eq!(
            value.candidates().first().map(|candidate| candidate.kind),
            expected
        );
        if index == 0 {
            first = Some(outcome.dupe());
            assert!(HostGlobSegmentCandidatesObservationKey::equality(
                first.as_ref().unwrap(),
                &transaction.compute(&key).await.unwrap()
            ));
        }
        if index == 3 {
            assert!(HostGlobSegmentCandidatesObservationKey::equality(
                first.as_ref().unwrap(),
                &outcome
            ));
        }
    }
}

#[tokio::test]
async fn same_dice_symlink_retarget_error_recovery_and_restoration() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = HostGlobSegmentCandidatesKey::new(path("/pkg"), pattern("*"));
    let listing = || {
        base_listing(vec![(
            OsString::from("entry"),
            PathDirectoryEntryKind::Symlink,
        )])
    };
    let resolved = |target: &'static str, kind| {
        let mut script = listing();
        script.extend([
            present("/pkg/entry", PathNodeKind::Symlink),
            read_link("/pkg/entry", target),
            present(target, kind),
        ]);
        script
    };
    let mut denied = listing();
    denied.push(lstat_result(
        path("/pkg/entry"),
        PathOperationResult::Error(PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        }),
    ));
    let scripts = [
        (
            resolved("/first", PathNodeKind::RegularFile),
            Ok(HostGlobSegmentCandidateKind::NonDirectory),
        ),
        (
            resolved("/second", PathNodeKind::Directory),
            Ok(HostGlobSegmentCandidateKind::Directory),
        ),
        (denied, Err(PathObservationOperation::Lstat)),
        (
            resolved("/first", PathNodeKind::RegularFile),
            Ok(HostGlobSegmentCandidateKind::NonDirectory),
        ),
    ];
    let mut transaction = dice.updater().commit().await;

    for (script, expected) in scripts {
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(script).unwrap(),
            )])
            .unwrap();
        transaction = updater.commit().await;

        let outcome = transaction.compute(&key).await.unwrap();
        match expected {
            Ok(kind) => assert_eq!(unwrap_ok(outcome).candidates()[0].kind, kind),
            Err(operation) => {
                let SourcePreparationOutcome::Complete(value) = outcome else {
                    panic!("observation error must be complete")
                };
                assert!(matches!(
                    value.as_ref(),
                    Err(HostGlobSegmentError::Observation {
                        operation: actual,
                        ..
                    }) if actual == &operation
                ));
            }
        }
    }
}

#[tokio::test]
async fn equal_complete_prunes_consumer_and_host_key_stores_no_evaluation_data() {
    fn script(target: &str) -> Vec<ScriptEntry> {
        let mut script = base_listing(vec![(
            OsString::from("entry"),
            PathDirectoryEntryKind::Symlink,
        )]);
        script.extend([
            present("/pkg/entry", PathNodeKind::Symlink),
            read_link("/pkg/entry", target),
            present(target, PathNodeKind::RegularFile),
        ]);
        script
    }

    CONSUMER_EVALUATIONS.store(0, Ordering::SeqCst);
    let tracker = Arc::new(HostGlobTracker::default());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let consumer = HostGlobConsumerKey {
        host: HostGlobSegmentCandidatesKey::new(path("/pkg"), pattern("*")),
    };
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        ..Default::default()
    });
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(script("/first")).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    assert_eq!(
        unwrap_ok(transaction.compute(&consumer).await.unwrap()).candidates()[0].kind,
        HostGlobSegmentCandidateKind::NonDirectory
    );
    assert_eq!(CONSUMER_EVALUATIONS.load(Ordering::SeqCst), 1);

    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(script("/second")).unwrap(),
        )])
        .unwrap();
    transaction = updater.commit().await;
    assert_eq!(
        unwrap_ok(transaction.compute(&consumer).await.unwrap()).candidates()[0].kind,
        HostGlobSegmentCandidateKind::NonDirectory
    );
    assert_eq!(CONSUMER_EVALUATIONS.load(Ordering::SeqCst), 1);
    assert!(tracker.evaluated.load(Ordering::SeqCst) >= 2);
    assert_eq!(tracker.evaluated_with_data.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn wildcard_missing_directory_is_a_semantic_error() {
    let outcome = compute(
        b"*",
        [present("/", PathNodeKind::Directory), missing("/pkg")],
    )
    .await;
    let SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("missing reached directory is complete")
    };
    assert!(matches!(
        value.as_ref(),
        Err(HostGlobSegmentError::DirectoryDisappeared { logical_directory })
            if logical_directory == &path("/pkg")
    ));
}
