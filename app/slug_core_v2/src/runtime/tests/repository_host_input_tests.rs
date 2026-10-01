use std::hash::Hash;
use std::hash::Hasher;

use slug_bzlmod_v2::NeedRepositoryEnvironmentNames;
use slug_bzlmod_v2::RepositoryEnvironmentCell;
use slug_bzlmod_v2::RepositoryEnvironmentCellKey;
use slug_bzlmod_v2::RepositoryEnvironmentEntry;
use slug_bzlmod_v2::RepositoryEnvironmentNameFrontier;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshotKey;
use slug_bzlmod_v2::RepositoryHostInputTransaction;
use slug_bzlmod_v2::RepositoryPlatformKey;
use slug_bzlmod_v2::SourcePreparationNeeds;

use super::*;

fn environment_snapshot(entries: &[(&str, &str)]) -> RepositoryEnvironmentSnapshot {
    let mut entries = entries
        .iter()
        .map(|(name, value)| RepositoryEnvironmentEntry::new(*name, *value))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.name().cmp(right.name()));
    RepositoryEnvironmentSnapshot::from_canonical(Arc::from(entries)).unwrap()
}

fn environment_frontier(names: &[&str]) -> RepositoryEnvironmentNameFrontier {
    RepositoryEnvironmentNameFrontier::from_unsorted(names.iter().copied().map(Into::into))
}

#[derive(Debug, Clone, Allocative)]
struct RepositoryEnvironmentProbeKey {
    cell: RepositoryEnvironmentCellKey,
    #[allocative(skip)]
    evaluations: Arc<AtomicUsize>,
}

impl PartialEq for RepositoryEnvironmentProbeKey {
    fn eq(&self, other: &Self) -> bool {
        self.cell == other.cell && Arc::ptr_eq(&self.evaluations, &other.evaluations)
    }
}

impl Eq for RepositoryEnvironmentProbeKey {}

impl Hash for RepositoryEnvironmentProbeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.cell.hash(state);
        Arc::as_ptr(&self.evaluations).hash(state);
    }
}

impl fmt::Display for RepositoryEnvironmentProbeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-environment-probe:{}:{:p}",
            self.cell,
            Arc::as_ptr(&self.evaluations)
        )
    }
}

#[async_trait]
impl Key for RepositoryEnvironmentProbeKey {
    type Value = RepositoryEnvironmentCell;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        self.evaluations.fetch_add(1, Ordering::SeqCst);
        ctx.compute(&self.cell).await.unwrap()
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Allocative)]
struct RepositoryEnvironmentSnapshotProbeKey {
    snapshot: RepositoryEnvironmentSnapshotKey,
    #[allocative(skip)]
    evaluations: Arc<AtomicUsize>,
}

impl PartialEq for RepositoryEnvironmentSnapshotProbeKey {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot == other.snapshot && Arc::ptr_eq(&self.evaluations, &other.evaluations)
    }
}

impl Eq for RepositoryEnvironmentSnapshotProbeKey {}

impl Hash for RepositoryEnvironmentSnapshotProbeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.snapshot.hash(state);
        Arc::as_ptr(&self.evaluations).hash(state);
    }
}

impl fmt::Display for RepositoryEnvironmentSnapshotProbeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-environment-snapshot-probe:{}:{:p}",
            self.snapshot,
            Arc::as_ptr(&self.evaluations)
        )
    }
}

#[async_trait]
impl Key for RepositoryEnvironmentSnapshotProbeKey {
    type Value = RepositoryEnvironmentSnapshot;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        self.evaluations.fetch_add(1, Ordering::SeqCst);
        ctx.compute(&self.snapshot).await.unwrap()
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
struct RepositoryHostTransactionProbeKey(u64);

impl fmt::Display for RepositoryHostTransactionProbeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-host-transaction-probe:{}", self.0)
    }
}

#[async_trait]
impl Key for RepositoryHostTransactionProbeKey {
    type Value = RepositoryHostInputTransaction;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.per_transaction_data()
            .data
            .get::<RepositoryHostInputTransaction>()
            .expect("every core transaction installs repository Host inputs")
            .clone()
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[test]
fn repository_environment_cells_cut_off_unrelated_changes_and_track_all_states() {
    let workspace = tempfile::tempdir().unwrap();
    let runtime = test_runtime(workspace.path()).unwrap();
    let normalized = NormalizedAbsolutePath::new(runtime.workspace.clone()).unwrap();
    let evaluations = Arc::new(AtomicUsize::new(0));
    let probe = RepositoryEnvironmentProbeKey {
        cell: RepositoryEnvironmentCellKey::new(normalized.clone(), "A"),
        evaluations: evaluations.clone(),
    };

    let inject = |snapshot: RepositoryEnvironmentSnapshot,
                  desired: RepositoryEnvironmentNameFrontier,
                  replaced: RepositoryEnvironmentNameFrontier| {
        runtime.runtime.block_on(async {
            let data = runtime
                .user_computation_data_with_repository_host_inputs(
                    None,
                    snapshot.clone(),
                    desired.clone(),
                )
                .unwrap();
            let carrier = data.data.get::<RepositoryHostInputTransaction>().unwrap();
            assert_eq!(carrier.snapshot(), &snapshot);
            assert_eq!(carrier.frontier(), &desired);
            let mut updater = runtime.dice.updater_with_data(data);
            crate::runtime::repository_host_input::inject_repository_host_inputs(
                &mut updater,
                &normalized,
                runtime.process_host.repository_platform().unwrap(),
                &snapshot,
                &desired,
                &replaced,
            )
            .unwrap();
            let mut transaction = updater.commit().await;
            runtime.request_revision.bind_cycle_detector(&transaction);
            let value = transaction.compute(&probe).await.unwrap();
            let platform = transaction
                .compute(&RepositoryPlatformKey::new(normalized.clone()))
                .await
                .unwrap();
            assert!(!platform.os_name().is_empty());
            assert!(!platform.arch().is_empty());
            value
        })
    };

    let first = inject(
        environment_snapshot(&[("A", "one"), ("B", "")]),
        environment_frontier(&["A", "B"]),
        RepositoryEnvironmentNameFrontier::empty(),
    );
    assert_eq!(first.value().unwrap().as_deref(), Some("one"));
    assert_eq!(evaluations.load(Ordering::SeqCst), 1);

    let unrelated = inject(
        environment_snapshot(&[("A", "one"), ("B", "changed"), ("D", "new")]),
        environment_frontier(&["A", "B", "D"]),
        environment_frontier(&["A", "B"]),
    );
    assert_eq!(unrelated, first);
    assert_eq!(
        evaluations.load(Ordering::SeqCst),
        1,
        "an equal A cell must cut off changes to B and D"
    );

    let changed = inject(
        environment_snapshot(&[("A", "two")]),
        environment_frontier(&["A", "B", "D"]),
        environment_frontier(&["A", "B", "D"]),
    );
    assert_eq!(changed.value().unwrap().as_deref(), Some("two"));
    assert_eq!(evaluations.load(Ordering::SeqCst), 2);

    let absent = inject(
        RepositoryEnvironmentSnapshot::empty(),
        environment_frontier(&["A"]),
        environment_frontier(&["A", "B", "D"]),
    );
    assert_eq!(absent, RepositoryEnvironmentCell::observed(None));
    assert_eq!(evaluations.load(Ordering::SeqCst), 3);

    let unauthorized = inject(
        RepositoryEnvironmentSnapshot::empty(),
        RepositoryEnvironmentNameFrontier::empty(),
        environment_frontier(&["A"]),
    );
    assert_eq!(unauthorized, RepositoryEnvironmentCell::Unauthorized);
    assert_eq!(evaluations.load(Ordering::SeqCst), 4);

    let restored = inject(
        environment_snapshot(&[("A", "one")]),
        environment_frontier(&["A"]),
        RepositoryEnvironmentNameFrontier::empty(),
    );
    assert_eq!(restored.value().unwrap().as_deref(), Some("one"));
    assert_eq!(evaluations.load(Ordering::SeqCst), 5);
}

#[test]
fn repository_environment_snapshot_tracks_full_a_b_a_and_empty_revisions() {
    let workspace = tempfile::tempdir().unwrap();
    let runtime = test_runtime(workspace.path()).unwrap();
    let normalized = NormalizedAbsolutePath::new(runtime.workspace.clone()).unwrap();
    let evaluations = Arc::new(AtomicUsize::new(0));
    let probe = RepositoryEnvironmentSnapshotProbeKey {
        snapshot: RepositoryEnvironmentSnapshotKey::new(normalized.clone()),
        evaluations: evaluations.clone(),
    };
    let inject = |snapshot: RepositoryEnvironmentSnapshot| {
        runtime.runtime.block_on(async {
            let data = runtime
                .user_computation_data_with_repository_host_inputs(
                    None,
                    snapshot.clone(),
                    snapshot.present_name_frontier(),
                )
                .unwrap();
            let mut updater = runtime.dice.updater_with_data(data);
            crate::runtime::repository_host_input::inject_repository_host_inputs(
                &mut updater,
                &normalized,
                runtime.process_host.repository_platform().unwrap(),
                &snapshot,
                &snapshot.present_name_frontier(),
                &RepositoryEnvironmentNameFrontier::empty(),
            )
            .unwrap();
            let mut transaction = updater.commit().await;
            runtime.request_revision.bind_cycle_detector(&transaction);
            let observed = transaction.compute(&probe).await.unwrap();
            assert_eq!(observed, snapshot);
            observed
        })
    };

    let a = environment_snapshot(&[("HOME", "/one"), ("NETRC", "sentinel-secret")]);
    let first = inject(a.clone());
    assert_eq!(first.get("HOME").unwrap().as_ref(), "/one");
    assert_eq!(evaluations.load(Ordering::SeqCst), 1);
    let equal = inject(a.clone());
    assert_eq!(equal, first);
    assert_eq!(evaluations.load(Ordering::SeqCst), 1);

    let b = inject(environment_snapshot(&[
        ("HOME", "/two"),
        ("NETRC", "sentinel-secret"),
    ]));
    assert_eq!(b.get("HOME").unwrap().as_ref(), "/two");
    assert_eq!(evaluations.load(Ordering::SeqCst), 2);
    let added = inject(environment_snapshot(&[
        ("HOME", "/two"),
        ("NETRC", "sentinel-secret"),
        ("OTHER", "added"),
    ]));
    assert_eq!(added.get("OTHER").unwrap().as_ref(), "added");
    assert_eq!(evaluations.load(Ordering::SeqCst), 3);
    let removed = inject(environment_snapshot(&[("HOME", "/two")]));
    assert!(removed.get("NETRC").is_none());
    assert_eq!(evaluations.load(Ordering::SeqCst), 4);
    let empty = inject(RepositoryEnvironmentSnapshot::empty());
    assert!(empty.is_empty());
    assert_eq!(evaluations.load(Ordering::SeqCst), 5);
    let restored = inject(a);
    assert_eq!(restored, first);
    assert_eq!(evaluations.load(Ordering::SeqCst), 6);
    assert!(!format!("{restored:?} {probe:?}").contains("sentinel-secret"));
}

#[test]
fn repository_environment_need_retry_and_rejection_restore_the_accepted_snapshot() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let normalized = NormalizedAbsolutePath::new(root.clone()).unwrap();
    let runtime = test_runtime(&root).unwrap();
    let accepted_environment = environment_snapshot(&[("A", "accepted-secret")]);
    let mut accepted_request = NativeDemandRequestInputBundle::normalized_initial();
    accepted_request.repository_environment = accepted_environment.clone();
    let plan = synthetic_plan(
        9100,
        &normalized,
        [],
        [],
        Ok(SyntheticCommandValue::Build("accepted".into())),
    );
    runtime
        .drive_synthetic_command(
            accepted_request,
            SyntheticCommandRoot::Build(SyntheticBuildRootKey { plan }),
        )
        .unwrap();

    let accepted = accepted_native_snapshot(&runtime);
    assert_eq!(
        accepted.inputs.request.repository_environment,
        accepted_environment
    );
    assert!(accepted.repository_environment_frontier.contains("A"));

    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command_with_inputs(
                NativeDemandRequestInputBundle::normalized_initial(),
            )
            .unwrap()
            .into_command(),
    );
    let need = SourcePreparationNeeds::environment(
        NeedRepositoryEnvironmentNames::new(normalized.clone(), environment_frontier(&["C"]))
            .unwrap(),
    );
    assert_eq!(
        guard.progress(&need).unwrap(),
        NativeDemandProgress::Environment
    );
    assert!(matches!(
        guard.progress(&need),
        Err(NativeDemandSessionError::EnvironmentInternalNonProgress)
    ));
    let foreign = SourcePreparationNeeds::environment(
        NeedRepositoryEnvironmentNames::new(
            NormalizedAbsolutePath::new(root.join("foreign")).unwrap(),
            environment_frontier(&["D"]),
        )
        .unwrap(),
    );
    assert!(matches!(
        guard.progress(&foreign),
        Err(NativeDemandSessionError::ForeignEnvironmentWorkspace { .. })
    ));

    let cold_evaluations = Arc::new(AtomicUsize::new(0));
    let cold_probe = RepositoryEnvironmentProbeKey {
        cell: RepositoryEnvironmentCellKey::new(normalized.clone(), "C"),
        evaluations: cold_evaluations.clone(),
    };
    guard.begin_attempt().unwrap();
    runtime.runtime.block_on(async {
        let data = guard.attempt_user_computation_data().unwrap();
        let carrier = data.data.get::<RepositoryHostInputTransaction>().unwrap();
        assert!(carrier.snapshot().is_empty());
        assert!(carrier.frontier().contains("A"));
        assert!(carrier.frontier().contains("C"));
        let mut updater = runtime.dice.updater_with_data(data);
        guard.inject_attempt(&mut updater).unwrap();
        let mut transaction = updater.commit().await;
        runtime.request_revision.bind_cycle_detector(&transaction);
        assert_eq!(
            transaction
                .compute(&RepositoryEnvironmentCellKey::new(normalized.clone(), "A",))
                .await
                .unwrap(),
            RepositoryEnvironmentCell::observed(None)
        );
        assert_eq!(
            transaction.compute(&cold_probe).await.unwrap(),
            RepositoryEnvironmentCell::observed(None)
        );
        assert!(
            transaction
                .compute(&RepositoryEnvironmentSnapshotKey::new(normalized.clone()))
                .await
                .unwrap()
                .is_empty()
        );
    });
    assert_eq!(cold_evaluations.load(Ordering::SeqCst), 1);
    let restored_host_inputs = guard.discard().unwrap();
    assert_eq!(restored_host_inputs.snapshot(), &accepted_environment);
    assert_eq!(
        restored_host_inputs.frontier(),
        &environment_frontier(&["A"])
    );

    assert_current_native_snapshot(&runtime, &accepted);
    runtime.runtime.block_on(async {
        let data = runtime.user_computation_data(None).unwrap();
        let mut transaction = runtime.dice.updater_with_data(data).existing_state().await;
        runtime.request_revision.bind_cycle_detector(&transaction);
        let carrier = transaction
            .compute(&RepositoryHostTransactionProbeKey(1))
            .await
            .unwrap();
        assert!(carrier.snapshot().is_empty());
        assert!(carrier.frontier().is_empty());
        assert_eq!(
            transaction
                .compute(&RepositoryEnvironmentCellKey::new(normalized.clone(), "A",))
                .await
                .unwrap()
                .value()
                .unwrap()
                .as_deref(),
            Some("accepted-secret")
        );
        assert_eq!(
            transaction.compute(&cold_probe).await.unwrap(),
            RepositoryEnvironmentCell::Unauthorized
        );
        assert_eq!(
            transaction
                .compute(&RepositoryEnvironmentSnapshotKey::new(normalized.clone()))
                .await
                .unwrap(),
            accepted_environment
        );
    });
    assert_eq!(
        cold_evaluations.load(Ordering::SeqCst),
        2,
        "restoration to unauthorized must invalidate a completed cold-absence dependent"
    );
}

#[test]
fn cancelled_command_revokes_a_current_only_present_name() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let normalized = NormalizedAbsolutePath::new(root.clone()).unwrap();
    let runtime = test_runtime(&root).unwrap();
    let mut request = NativeDemandRequestInputBundle::normalized_initial();
    request.repository_environment = environment_snapshot(&[("X", "cancelled-secret")]);
    let mut plan = synthetic_plan(
        9101,
        &normalized,
        [],
        [],
        Ok(SyntheticCommandValue::Build("unreachable".into())),
    );
    Arc::make_mut(&mut plan).behavior = SyntheticRootBehavior::PendForCancellation;

    assert!(matches!(
        runtime.drive_synthetic_command(
            request,
            SyntheticCommandRoot::Build(SyntheticBuildRootKey { plan }),
        ),
        Err(NativeDemandSessionError::Computation(_))
    ));
    assert!(
        accepted_native_snapshot(&runtime)
            .repository_environment_frontier
            .is_empty()
    );
    runtime.runtime.block_on(async {
        let data = runtime.user_computation_data(None).unwrap();
        let mut transaction = runtime.dice.updater_with_data(data).existing_state().await;
        runtime.request_revision.bind_cycle_detector(&transaction);
        assert_eq!(
            transaction
                .compute(&RepositoryEnvironmentCellKey::new(normalized.clone(), "X"))
                .await
                .unwrap(),
            RepositoryEnvironmentCell::Unauthorized
        );
        assert!(
            transaction
                .compute(&RepositoryEnvironmentSnapshotKey::new(normalized))
                .await
                .unwrap()
                .is_empty()
        );
    });
}
