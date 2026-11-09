//! Pure scheduling bounds with actual tracked PathObservationKey hint lookups.
//! The helper key supplies no native scope, data, capture or IO authority.
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering as AtomicOrdering;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::path_observation_shards;

use super::*;

fn demand(index: usize, operation: PathObservationOperation) -> PathObservationDemand {
    PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(format!("/bytes/{index}")).unwrap(),
        operation,
    )
}
fn need(indices: impl IntoIterator<Item = usize>) -> SourcePreparationNeeds {
    SourcePreparationNeeds::path(
        NeedPathObservations::try_from_iter(
            indices
                .into_iter()
                .map(|i| demand(i, PathObservationOperation::FileBytes)),
        )
        .unwrap(),
    )
}
fn stat(size: i64, kind: PathNodeKind) -> Arc<PathObservationResult> {
    Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
        PathLstat::new(kind, size, 1, 1, 1, 0o644),
    )))
}
fn epoch(sizes: impl IntoIterator<Item = (usize, i64)>) -> PathObservationEpoch {
    PathObservationEpoch::from_shared(sizes.into_iter().map(|(i, size)| {
        (
            demand(i, PathObservationOperation::Lstat),
            stat(size, PathNodeKind::RegularFile),
        )
    }))
    .unwrap()
}
async fn tx(epoch: &PathObservationEpoch) -> dice::DiceTransaction {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut update = dice.updater();
    update.changed_to(path_observation_shards(epoch)).unwrap();
    update
        .changed_to([(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    update.commit().await
}
async fn update(tx: dice::DiceTransaction, epoch: &PathObservationEpoch) -> dice::DiceTransaction {
    let mut update = tx.into_updater();
    update.changed_to(path_observation_shards(epoch)).unwrap();
    update
        .changed_to([(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    update.commit().await
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HintSum {
    right: Arc<[PathObservationDemand]>,
    pending: Option<Arc<[PathObservationDemand]>>,
    maximum: u64,
}
impl std::fmt::Display for HintSum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("test-known-byte-hints")
    }
}
#[async_trait]
impl Key for HintSum {
    type Value = Option<u64>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        added_known_bytes(ctx, self.pending.as_deref(), &self.right, self.maximum).await
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
}
fn sum(indices: impl IntoIterator<Item = usize>, maximum: u64) -> HintSum {
    HintSum {
        right: indices
            .into_iter()
            .map(|i| demand(i, PathObservationOperation::FileBytes))
            .collect(),
        pending: None,
        maximum,
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum Stop {
    Need(SourcePreparationNeeds),
    Semantic,
}
impl PathNeedBreak for Stop {
    fn into_need(self) -> Result<SourcePreparationNeeds, Self> {
        match self {
            Self::Need(need) => Ok(need),
            other => Err(other),
        }
    }
    fn from_need(need: SourcePreparationNeeds) -> Self {
        Self::Need(need)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct Plan {
    steps: Arc<[Result<(), Stop>]>,
}
impl Hash for Plan {
    fn hash<H: Hasher>(&self, h: &mut H) {
        0u8.hash(h)
    }
}
impl std::fmt::Display for Plan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("test-byte-window")
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct Report {
    result: Result<(), Stop>,
    inspected: usize,
}
#[async_trait]
impl Key for Plan {
    type Value = Arc<Report>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let mut window = HomogeneousPathLookahead::new();
        let mut inspected = 0;
        for step in self.steps.iter() {
            if !window.can_inspect_next() {
                break;
            }
            inspected += 1;
            if let ControlFlow::Break(stop) = window.observe(ctx, step.clone()).await {
                return Arc::new(Report {
                    result: Err(stop),
                    inspected,
                });
            }
        }
        Arc::new(Report {
            result: window.finish(),
            inspected,
        })
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
    fn validity(_: &Self::Value) -> bool {
        false
    }
}
async fn run(
    tx: &mut dice::DiceTransaction,
    steps: impl IntoIterator<Item = Result<(), Stop>>,
) -> Arc<Report> {
    tx.compute(&Plan {
        steps: steps.into_iter().collect(),
    })
    .await
    .unwrap()
}
fn pending(report: &Report) -> &SourcePreparationNeeds {
    let Err(Stop::Need(need)) = &report.result else {
        panic!("expected whole pending Need: {report:?}")
    };
    need
}

#[tokio::test]
async fn tracked_regular_size_hints_have_exact_byte_boundary_and_current_revoke_semantics() {
    let mut current = tx(&epoch([(0, MAX_KNOWN_BYTES as i64), (1, 0), (2, 1)])).await;
    assert_eq!(
        current
            .compute(&sum([0, 1], MAX_KNOWN_BYTES))
            .await
            .unwrap(),
        Some(MAX_KNOWN_BYTES)
    );
    assert_eq!(
        current
            .compute(&sum([0, 2], MAX_KNOWN_BYTES))
            .await
            .unwrap(),
        None
    );
    let mut duplicate = sum([0, 1], MAX_KNOWN_BYTES);
    duplicate.pending = Some(Arc::from([demand(0, PathObservationOperation::FileBytes)]));
    assert_eq!(current.compute(&duplicate).await.unwrap(), Some(0));
    let key = sum([0], MAX_KNOWN_BYTES);
    let mut current = update(current, &epoch([(0, 7)])).await;
    assert_eq!(current.compute(&key).await.unwrap(), Some(7));
    let mut current = update(current, &PathObservationEpoch::empty()).await;
    assert_eq!(
        current.compute(&key).await.unwrap(),
        None,
        "a missing current fact cannot reuse its old size"
    );
    let mut current = update(current, &epoch([(0, 0)])).await;
    assert_eq!(
        current.compute(&key).await.unwrap(),
        Some(0),
        "known empty regular files are eligible"
    );
    for result in [
        stat(-1, PathNodeKind::RegularFile),
        stat(1, PathNodeKind::Directory),
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing)),
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Error(
            slug_workspace_v2::PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: PathNodeKind::Symlink,
            },
        ))),
    ] {
        let e = PathObservationEpoch::from_shared([(
            demand(0, PathObservationOperation::Lstat),
            result,
        )])
        .unwrap();
        current = update(current, &e).await;
        assert_eq!(current.compute(&key).await.unwrap(), None);
    }
    // Explicit per-demand revocation must not fall back to the still-positive
    // legacy epoch. The mode and current input are injected before querying.
    let legacy = epoch([(0, 11)]);
    let hint = demand(0, PathObservationOperation::Lstat);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to([(PathObservationEpochKey, legacy.dupe())])
        .unwrap();
    slug_workspace_v2::inject_path_observation_inputs(&mut updater, &legacy, [hint.dupe()])
        .unwrap();
    let mut per_demand = updater.commit().await;
    assert_eq!(per_demand.compute(&key).await.unwrap(), Some(11));
    let mut updater = per_demand.into_updater();
    slug_workspace_v2::inject_path_observation_inputs(
        &mut updater,
        &PathObservationEpoch::empty(),
        [hint],
    )
    .unwrap();
    let mut per_demand = updater.commit().await;
    assert_eq!(per_demand.compute(&key).await.unwrap(), None);
    let host = epoch([(0, 3)]);
    let mut current = tx(&host).await;
    let mut foreign = sum([0], MAX_KNOWN_BYTES);
    foreign.right = Arc::from([PathObservationDemand::new(
        PathObservationNamespace::Materialization(
            slug_workspace_v2::PathObservationInstanceId::new(1),
        ),
        demand(0, PathObservationOperation::FileBytes).path().dupe(),
        PathObservationOperation::FileBytes,
    )]);
    assert_eq!(
        current.compute(&foreign).await.unwrap(),
        None,
        "same path in another namespace is not a hint"
    );
    let mut current = tx(&epoch([(0, i64::MAX), (1, i64::MAX), (2, i64::MAX)])).await;
    assert_eq!(
        current.compute(&sum([0, 1, 2], u64::MAX)).await.unwrap(),
        None,
        "hint arithmetic overflow refuses"
    );
}

#[tokio::test]
async fn byte_windows_keep_whole_barriers_class_lock_and_first_ineligible_owners() {
    let mut current = tx(&epoch([(0, 1), (1, 1), (2, MAX_KNOWN_BYTES as i64 + 1)])).await;
    for first in [need([2]), need([3]), need(0..33)] {
        let retained = first.dupe();
        let report = run(
            &mut current,
            [Err(Stop::Need(first)), Err(Stop::Need(need([1])))],
        )
        .await;
        assert_eq!(report.inspected, 1);
        assert!(std::ptr::eq(
            pending(&report).path_observations().unwrap().demands(),
            retained.path_observations().unwrap().demands()
        ));
    }
    let lstat = SourcePreparationNeeds::path(NeedPathObservations::singleton(demand(
        1,
        PathObservationOperation::Lstat,
    )));
    let digest = SourcePreparationNeeds::path(NeedPathObservations::singleton(demand(
        1,
        PathObservationOperation::FileDigest,
    )));
    let mixed = SourcePreparationNeeds::path(
        NeedPathObservations::try_from_iter([
            demand(1, PathObservationOperation::FileBytes),
            demand(1, PathObservationOperation::ReadLink),
        ])
        .unwrap(),
    );
    let environment = SourcePreparationNeeds::environment(
        slug_bzlmod_v2::NeedRepositoryEnvironmentNames::new(
            NormalizedAbsolutePath::new("/bytes").unwrap(),
            slug_bzlmod_v2::RepositoryEnvironmentNameFrontier::from_unsorted(["DECLARED".into()]),
        )
        .unwrap(),
    );
    let mixed_nonpath = need([1]).try_union(&environment).unwrap();
    for later in [
        Err(Stop::Need(environment)),
        Err(Stop::Need(mixed_nonpath)),
        Err(Stop::Need(lstat.dupe())),
        Err(Stop::Need(digest)),
        Err(Stop::Need(mixed)),
        Err(Stop::Need(need([2]))),
        Err(Stop::Need(need([3]))),
        Err(Stop::Semantic),
    ] {
        let first = need([0]);
        let report = run(
            &mut current,
            [
                Err(Stop::Need(first.dupe())),
                later,
                Err(Stop::Need(need([1]))),
            ],
        )
        .await;
        assert_eq!(report.inspected, 2);
        assert_eq!(
            pending(&report),
            &first,
            "later barriers never widen earlier pending bytes"
        );
    }
    let report = run(
        &mut current,
        [Err(Stop::Need(lstat.dupe())), Err(Stop::Need(need([0])))],
    )
    .await;
    assert_eq!(
        pending(&report),
        &lstat,
        "the original Lstat window never mixes content"
    );
}

#[tokio::test]
async fn byte_windows_enforce_row_demand_path_and_actual_control_overlap_bounds() {
    let mut current = tx(&epoch((0..33).map(|i| (i, 1)))).await;
    let mut steps = vec![Err(Stop::Need(need([0])))];
    steps.extend((1..32).map(|_| Ok(())));
    steps.push(Err(Stop::Need(need([1]))));
    let report = run(&mut current, steps).await;
    assert_eq!(report.inspected, MAX_ROWS);
    assert_eq!(pending(&report), &need([0]));
    let report = run(
        &mut current,
        [
            Err(Stop::Need(need(0..16))),
            Err(Stop::Need(need(16..32))),
            Err(Stop::Need(need([32]))),
        ],
    )
    .await;
    assert_eq!(pending(&report), &need(0..32));
    assert_eq!(report.inspected, 3);
    current = update(
        current,
        &epoch([
            (0, (MAX_KNOWN_BYTES / 2) as i64),
            (1, (MAX_KNOWN_BYTES / 2) as i64),
            (2, 1),
        ]),
    )
    .await;
    let report = run(
        &mut current,
        [
            Err(Stop::Need(need([0]))),
            Err(Stop::Need(need([1]))),
            Err(Stop::Need(need([2]))),
        ],
    )
    .await;
    assert_eq!(
        pending(&report),
        &need([0, 1]),
        "exact declared-byte union is retained; +1 stops before extension"
    );
    assert_eq!(report.inspected, 3);
    for name in [
        format!("/{}", "x".repeat(MAX_PATH)),
        format!("/{}/../short", "x".repeat(MAX_METADATA + 1)),
    ] {
        let original = SourcePreparationNeeds::path(NeedPathObservations::singleton(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(name).unwrap(),
                PathObservationOperation::FileBytes,
            ),
        ));
        let report = run(
            &mut current,
            [Err(Stop::Need(original.dupe())), Err(Stop::Need(need([0])))],
        )
        .await;
        assert_eq!(report.inspected, 1);
        assert_eq!(pending(&report), &original);
    }
    let original = need([0]);
    let candidate = byte_candidate(&original).unwrap();
    let (mut low, mut high) = (0, MAX_METADATA);
    while low < high {
        let middle = (low + high + 1) / 2;
        if byte_forecast(&candidate, None, middle).is_some() {
            low = middle
        } else {
            high = middle - 1
        }
    }
    assert_eq!(byte_forecast(&candidate, None, low), Some(1));
    assert_eq!(byte_forecast(&candidate, None, low + 1), None);
    assert_eq!(byte_forecast(&candidate, None, usize::MAX), None);
    assert!(byte_forecast(&candidate, None, 0).is_some());
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
struct MarkerKey;
impl std::fmt::Display for MarkerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("test-byte-extension-marker")
    }
}
#[async_trait]
impl Key for MarkerKey {
    type Value = (bool, bool, bool, u64, usize);
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let marker = AtomicBool::new(false);
        let first = ByteWindow::first(ctx, need([0])).await.unwrap();
        let first_flag = marker.load(AtomicOrdering::Relaxed);
        let retained = first.pending.dupe();
        let (duplicate, extended) = first.extend(ctx, need([0])).await.unwrap();
        assert!(!extended);
        mark_extended(&marker, extended);
        assert!(std::ptr::eq(
            retained.path_observations().unwrap().demands(),
            duplicate.pending.path_observations().unwrap().demands()
        ));
        let duplicate_flag = marker.load(AtomicOrdering::Relaxed);
        let (unique, extended) = duplicate.extend(ctx, need([1])).await.unwrap();
        assert!(extended);
        // The window has already installed the new unique union before marking.
        assert_eq!(
            unique
                .pending
                .only_path_observations()
                .unwrap()
                .demands()
                .len(),
            2
        );
        mark_extended(&marker, extended);
        (
            first_flag,
            duplicate_flag,
            marker.load(AtomicOrdering::Relaxed),
            unique.known_bytes,
            unique
                .pending
                .only_path_observations()
                .unwrap()
                .demands()
                .len(),
        )
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
}
#[tokio::test]
async fn local_extension_marker_stays_false_for_first_and_duplicate_and_marks_installed_union() {
    let mut current = tx(&epoch([(0, MAX_KNOWN_BYTES as i64), (1, 0)])).await;
    assert_eq!(
        current.compute(&MarkerKey).await.unwrap(),
        (false, false, true, MAX_KNOWN_BYTES, 2)
    );
}
