//! Causal incremental proof over two actual original repository evaluations.
//! Records/receipts here are pure observed protocol fixtures, not native roots.
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use dice::DiceEvent;
use dice::DiceEventListener;
use slug_bzlmod_v2::RepositorySourceSnapshotEpoch;
use slug_bzlmod_v2::RepositorySourceSnapshotEpochKey;
use slug_bzlmod_v2::RepositoryWorkspaceRealization;
use slug_bzlmod_v2::RepositoryWorkspaceReservationDemand;

use super::*;

const TWO_REPOSITORIES: &str = "module(name='consumer')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, first='first', second='second')\n";
const ORIGINAL: &str = r#"
def write(ctx):
    ctx.file('out', 'stable original bytes', executable=False)
repo=repository_rule(implementation=write)
def impl(ctx):
    repo(name='first')
    repo(name='second')
ext=module_extension(implementation=impl)
"#;

#[derive(Default, Allocative)]
struct ObservedEffectComputes {
    #[allocative(skip)]
    count: AtomicUsize,
}
impl DiceEventListener for ObservedEffectComputes {
    fn event(&self, event: DiceEvent) {
        if matches!(event, DiceEvent::ComputeStarted { key_type }
            if key_type == "HostSelectedRepositoryFileEffectObservationKey")
        {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }
}

async fn install_counter(
    transaction: dice::DiceTransaction,
    counter: Arc<ObservedEffectComputes>,
) -> dice::DiceTransaction {
    // Preserve every userdata input installed by with_host_inputs_for. Only
    // the non-semantic DiceEventListener changes; nothing is held across await.
    let data = {
        let old = transaction.per_transaction_data();
        let mut data = UserComputationData {
            tracker: counter,
            spawner: old.spawner.clone(),
            cycle_detector: old.cycle_detector.clone(),
            activation_tracker: old.activation_tracker.clone(),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        data.data.set(
            old.data
                .get::<RepositoryHostInputTransaction>()
                .unwrap()
                .clone(),
        );
        data.data.set(
            old.data
                .get::<SelectedExtensionDownloadTransaction>()
                .unwrap()
                .clone(),
        );
        data.data
            .set(old.data.get::<RepositoryExecuteEpoch>().unwrap().clone());
        data
    };
    transaction.into_updater().commit_with_data(data).await
}

async fn observed(
    transaction: &mut dice::DiceTransaction,
    key: &HostSelectedRepositoryFileEffectObservationKey,
) -> SourcePreparationOutcome<EffectResult> {
    match transaction.compute(key).await.unwrap() {
        SourcePreparationOutcome::Need(needs) => SourcePreparationOutcome::Need(needs),
        SourcePreparationOutcome::Complete(result) => {
            SourcePreparationOutcome::Complete(result.unwrap().result().clone())
        }
    }
}

fn reserve(
    epoch: &RepositoryWorkspaceEpoch,
    demand: &RepositoryWorkspaceReservationDemand,
    name: &str,
    instance: u64,
) -> RepositoryWorkspaceEpoch {
    epoch
        .reserve(
            Arc::new(
                RepositoryWorkspaceReservation::new(
                    demand.key().workspace().clone(),
                    demand.key().owner().clone(),
                    demand.key().repository().clone(),
                    demand.key().ordinal(),
                    demand.repo_spec().clone(),
                    NormalizedAbsolutePath::new(format!("/__slug_repositories__/{name}")).unwrap(),
                    PathObservationInstanceId::new(instance),
                    epoch.lease().unwrap(),
                    RepositoryWorkspaceRetentionLimits::new(
                        128,
                        16 * 1024 * 1024,
                        1024 * 1024 * 1024,
                    )
                    .unwrap(),
                )
                .unwrap(),
            ),
            0o755,
        )
        .unwrap()
}

fn complete_file(
    epoch: &RepositoryWorkspaceEpoch,
    needs: &SourcePreparationNeeds,
) -> RepositoryWorkspaceEpoch {
    assert_eq!(needs.workspace_operations().len(), 1);
    let request = needs
        .workspace_operations()
        .values()
        .next()
        .unwrap()
        .request();
    let RepositoryWorkspaceOperation::File { path, body, mode } = request.operation() else {
        panic!("actual original must emit File")
    };
    let mut entries = request.predecessor().entries().to_vec();
    entries.push(
        RepositoryWorkspaceEntry::new(
            path.clone(),
            *mode,
            RepositoryWorkspaceNode::File(body.clone()),
        )
        .unwrap(),
    );
    entries.sort_unstable_by(|a, b| a.path().cmp(b.path()));
    epoch
        .complete(
            request.as_ref().clone(),
            RepositoryWorkspaceResult::Unit,
            request.predecessor().mode(),
            entries,
        )
        .unwrap()
}

async fn replace_workspace(
    transaction: dice::DiceTransaction,
    epoch: RepositoryWorkspaceEpoch,
) -> dice::DiceTransaction {
    let mut updater = transaction.into_updater();
    updater
        .changed_to([(
            RepositoryWorkspaceEpochKey::new(NormalizedAbsolutePath::new(WORKSPACE).unwrap()),
            epoch,
        )])
        .unwrap();
    updater.commit().await
}

async fn unchanged_a(
    transaction: &mut dice::DiceTransaction,
    key: &HostSelectedRepositoryFileEffectObservationKey,
    counter: &ObservedEffectComputes,
    expected: &RepositoryWorkspaceRealization,
    stage: &'static str,
) -> (&'static str, usize, usize) {
    let before = counter.count.load(Ordering::Relaxed);
    let SourcePreparationOutcome::Complete(result) = observed(transaction, key).await else {
        panic!("A must remain complete")
    };
    assert_eq!(
        result.as_ref().as_ref().unwrap().workspace_realization(),
        Some(expected)
    );
    (stage, before, counter.count.load(Ordering::Relaxed))
}

async fn builtin_state(lease: u64) -> Arc<slug_bzlmod_v2::RepositorySourceSnapshotState> {
    // Same genuine mapping/catalog producer pattern as the existing repository
    // context Source snapshot fixture. No native owner/authority is constructed.
    use slug_bzlmod_v2::RepositorySourceSnapshotEntry as Entry;
    use slug_bzlmod_v2::RepositorySourceSnapshotNode as Node;

    use crate::canonical_repository_route_tests::tests as fixture;
    let dice = fixture::builtin_graph_dice();
    let mut transaction = fixture::transaction(
        &dice,
        &fixture::builtin_graph_module(),
        fixture::EXTENSION_A,
        true,
        None,
    )
    .await;
    let SourcePreparationOutcome::Complete(mapping) = transaction
        .compute(
            &slug_bzlmod_v2::HostBuiltinBazelToolsRepositoryMappingKey::new(
                NormalizedAbsolutePath::new(fixture::WORKSPACE).unwrap(),
            ),
        )
        .await
        .unwrap()
    else {
        panic!("actual builtin mapping")
    };
    let route = Arc::new(slug_bzlmod_v2::HostCanonicalRepositoryRoute::builtin(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        mapping.as_ref().as_ref().unwrap().clone(),
    ));
    let input = slug_bzlmod_v2::host_canonical_repository_source_input(route, None).unwrap();
    let request = Arc::new(
        slug_bzlmod_v2::RepositorySourceSnapshotRequest::from_canonical(&input, lease).unwrap(),
    );
    let reservation = Arc::new(
        slug_bzlmod_v2::RepositorySourceSnapshotReservation::new(
            request,
            NormalizedAbsolutePath::new("/__slug_repositories__/unrelated-builtin").unwrap(),
            PathObservationInstanceId::new(403),
        )
        .unwrap(),
    );
    let mut entries = vec![Entry::new("".into(), 0o755, Node::Directory).unwrap()];
    let catalog = slug_bzlmod_v2::BuiltinBazelToolsSnapshot::CURRENT;
    catalog
        .visit_validated_catalog(
            catalog.route_identity(),
            |path, bytes, executable, digest| {
                let mut parent = path;
                while let Some((directory, _)) = parent.rsplit_once('/') {
                    if !entries.iter().any(|e| e.path() == directory) {
                        entries.push(Entry::new(directory.into(), 0o755, Node::Directory).unwrap());
                    }
                    parent = directory;
                }
                entries.push(
                    Entry::new(
                        path.into(),
                        if executable { 0o755 } else { 0o644 },
                        Node::File(
                            slug_workspace_v2::FileContentDigest::new(digest, bytes.len() as u64)
                                .unwrap(),
                        ),
                    )
                    .unwrap(),
                );
                Ok::<_, slug_bzlmod_v2::BuiltinBazelToolsSourceFileError>(())
            },
        )
        .unwrap();
    entries.sort_unstable_by(|a, b| a.path().cmp(b.path()));
    Arc::new(slug_bzlmod_v2::RepositorySourceSnapshotState::new(reservation, entries).unwrap())
}

#[tokio::test]
async fn observed_original_a_stays_cached_while_unrelated_b_and_source_progress() {
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut transaction = transaction_untracked(&dice, TWO_REPOSITORIES, ORIGINAL, true).await;
    let selected_owner = owner(&mut transaction).await;
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let a = HostSelectedRepositoryFileEffectObservationKey::new(
        workspace.clone(),
        selected_owner.clone(),
        0,
    );
    let b =
        HostSelectedRepositoryFileEffectObservationKey::new(workspace.clone(), selected_owner, 1);
    let counter = Arc::new(ObservedEffectComputes::default());
    transaction = install_counter(transaction, counter.clone()).await;
    let mut epoch = empty(71);
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        epoch.clone(),
    )
    .await;
    let source_epoch = RepositorySourceSnapshotEpoch::fresh(workspace.clone(), 71).unwrap();
    let mut updater = transaction.into_updater();
    updater
        .changed_to([(
            RepositorySourceSnapshotEpochKey::new(workspace.clone()),
            Arc::new(source_epoch.clone()),
        )])
        .unwrap();
    transaction = updater.commit().await;
    let SourcePreparationOutcome::Need(needs) = observed(&mut transaction, &a).await else {
        panic!("actual A reservation")
    };
    epoch = reserve(
        &epoch,
        needs.workspace_reservations().values().next().unwrap(),
        "a",
        401,
    );
    transaction = replace_workspace(transaction, epoch.clone()).await;
    let SourcePreparationOutcome::Need(needs) = observed(&mut transaction, &a).await else {
        panic!("actual A File")
    };
    epoch = complete_file(&epoch, &needs);
    transaction = replace_workspace(transaction, epoch.clone()).await;
    let SourcePreparationOutcome::Complete(result) = observed(&mut transaction, &a).await else {
        panic!("A fully replayed")
    };
    let expected = result
        .as_ref()
        .as_ref()
        .unwrap()
        .workspace_realization()
        .unwrap()
        .clone();
    assert_eq!(expected.transcript().records().len(), 1);
    assert!(
        counter.count.load(Ordering::Relaxed) > 0,
        "listener must observe actual effect compute"
    );
    let SourcePreparationOutcome::Need(needs) = observed(&mut transaction, &b).await else {
        panic!("actual B reservation")
    };
    epoch = reserve(
        &epoch,
        needs.workspace_reservations().values().next().unwrap(),
        "b",
        402,
    );
    transaction = replace_workspace(transaction, epoch.clone()).await;
    let mut counts =
        vec![unchanged_a(&mut transaction, &a, &counter, &expected, "b_reserved").await];
    let SourcePreparationOutcome::Need(needs) = observed(&mut transaction, &b).await else {
        panic!("actual B File")
    };
    epoch = complete_file(&epoch, &needs);
    transaction = replace_workspace(transaction, epoch.clone()).await;
    counts.push(unchanged_a(&mut transaction, &a, &counter, &expected, "b_completed").await);
    let SourcePreparationOutcome::Complete(result) = observed(&mut transaction, &b).await else {
        panic!("B fully replayed")
    };
    let realized = result
        .as_ref()
        .as_ref()
        .unwrap()
        .workspace_realization()
        .unwrap()
        .clone();
    let reservation = realized.reservation().clone();
    let entry = RepositoryMaterializationEpochEntry {
        request: Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: workspace.clone(),
                canonical_repo: reservation.repository().clone(),
            },
            repo_spec: (**reservation.expected_repo_spec()).clone(),
            kind: RepositoryMaterializationKind::GeneralWorkspace(realized.clone()),
        }),
        result: RepositoryMaterializationResult::Success(
            RepositoryMaterializationSuccess::Immutable {
                source_identity: realized.source_identity().clone(),
                generation_root: reservation.display_root().as_path().to_owned(),
                observation_instance: reservation.instance(),
                path_policy: slug_bzlmod_v2::RepositoryMaterializationPathPolicy::GenerationRoot,
            },
        ),
    };
    let mut updater = transaction.into_updater();
    updater
        .changed_to([(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace.clone(),
            },
            RepositoryMaterializationResultEpoch::new(workspace.clone(), [entry]).unwrap(),
        )])
        .unwrap();
    transaction = updater.commit().await;
    counts.push(unchanged_a(&mut transaction, &a, &counter, &expected, "b_materialized").await);
    let captured = source_epoch.with_snapshot(builtin_state(71).await).unwrap();
    let mut updater = transaction.into_updater();
    updater
        .changed_to([(
            RepositorySourceSnapshotEpochKey::new(workspace),
            Arc::new(captured),
        )])
        .unwrap();
    transaction = updater.commit().await;
    counts.push(
        unchanged_a(
            &mut transaction,
            &a,
            &counter,
            &expected,
            "unrelated_source",
        )
        .await,
    );
    assert!(
        counts.iter().all(|(_, before, after)| before == after),
        "unrelated updates recomputed queried A: {counts:?}"
    );
    // An additional observed own-root record is relevant even when its tree is
    // unchanged: this original no longer consumes the complete transcript.
    let request = epoch
        .request_at(
            expected.reservation(),
            1,
            RepositoryWorkspaceOperation::Exists("out".into()),
        )
        .unwrap();
    epoch = epoch
        .complete(
            request,
            RepositoryWorkspaceResult::Exists(true),
            expected.final_state().mode(),
            expected.final_state().entries().to_vec(),
        )
        .unwrap();
    transaction = replace_workspace(transaction, epoch).await;
    let before = counter.count.load(Ordering::Relaxed);
    assert!(
        matches!(observed(&mut transaction,&a).await,SourcePreparationOutcome::Complete(result) if result.is_err())
    );
    assert!(counter.count.load(Ordering::Relaxed) > before);
    // Fresh command identity invalidates a prior completion; the new missing
    // reservation is a Need, not a cached or fabricated empty transcript.
    transaction = replace_workspace(transaction, empty(72)).await;
    let before = counter.count.load(Ordering::Relaxed);
    let value = transaction.compute(&a).await.unwrap();
    assert!(
        matches!(&value,SourcePreparationOutcome::Need(needs) if needs.workspace_reservations().len()==1)
    );
    assert!(!HostSelectedRepositoryFileEffectObservationKey::validity(
        &value
    ));
    assert!(counter.count.load(Ordering::Relaxed) > before);
    transaction = replace_workspace(transaction, RepositoryWorkspaceEpoch::revoked()).await;
    let before = counter.count.load(Ordering::Relaxed);
    assert!(
        matches!(observed(&mut transaction,&a).await,SourcePreparationOutcome::Complete(result) if result.is_err())
    );
    assert!(counter.count.load(Ordering::Relaxed) > before);
}

#[tokio::test]
async fn retained_original_execute_request_stays_unadmitted_after_other_root_spends_budget() {
    const EXECUTING: &str = r#"
def run(ctx):
    ctx.execute(['/bin/true'])
repo=repository_rule(implementation=run)
def impl(ctx):
    repo(name='first')
    repo(name='second')
ext=module_extension(implementation=impl)
"#;
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut transaction = transaction_untracked(&dice, TWO_REPOSITORIES, EXECUTING, true).await;
    let selected_owner = owner(&mut transaction).await;
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let a = HostSelectedRepositoryFileEffectObservationKey::new(
        workspace.clone(),
        selected_owner.clone(),
        0,
    );
    let b = HostSelectedRepositoryFileEffectObservationKey::new(workspace, selected_owner, 1);
    let mut epoch = empty(71);
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        epoch.clone(),
    )
    .await;
    let SourcePreparationOutcome::Need(needs) = observed(&mut transaction, &a).await else {
        panic!("actual A reservation")
    };
    epoch = reserve(
        &epoch,
        needs.workspace_reservations().values().next().unwrap(),
        "held-a",
        411,
    );
    transaction = replace_workspace(transaction, epoch.clone()).await;
    let SourcePreparationOutcome::Need(needs) = observed(&mut transaction, &a).await else {
        panic!("actual A Execute")
    };
    let retained = needs
        .workspace_operations()
        .values()
        .next()
        .unwrap()
        .request()
        .clone();
    assert!(matches!(
        retained.operation(),
        RepositoryWorkspaceOperation::Execute(_)
    ));
    let SourcePreparationOutcome::Need(needs) = observed(&mut transaction, &b).await else {
        panic!("actual B reservation")
    };
    epoch = reserve(
        &epoch,
        needs.workspace_reservations().values().next().unwrap(),
        "held-b",
        412,
    );
    transaction = replace_workspace(transaction, epoch.clone()).await;
    let SourcePreparationOutcome::Need(needs) = observed(&mut transaction, &b).await else {
        panic!("actual B Execute")
    };
    let request = needs
        .workspace_operations()
        .values()
        .next()
        .unwrap()
        .request();
    let reservation = request.reservation().clone();
    let operation = request.operation().clone();
    // Pure admission fixture: no children/quota transitions/native authority.
    // B's first operation is genuine original; later records exercise the
    // independent command-wide protocol counter, not original B completion.
    for index in 0..128 {
        let request = epoch
            .request_at(&reservation, index, operation.clone())
            .unwrap();
        epoch = epoch
            .complete(
                request,
                RepositoryWorkspaceResult::Execute(slug_bzlmod_v2::RepositoryExecuteResult::new(
                    0,
                    Arc::from([]),
                    Arc::from([]),
                )),
                slug_bzlmod_v2::RepositoryWorkspaceMode::Process,
                vec![
                    RepositoryWorkspaceEntry::new(
                        "".into(),
                        0o755,
                        RepositoryWorkspaceNode::Directory,
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
    }
    assert_eq!(
        epoch
            .request_at(
                retained.reservation(),
                retained.index(),
                retained.operation().clone()
            )
            .unwrap_err(),
        slug_bzlmod_v2::RepositoryWorkspaceError::StateLimit
    );
    transaction = replace_workspace(transaction, epoch).await;
    let value = transaction.compute(&a).await.unwrap();
    assert!(
        !HostSelectedRepositoryFileEffectObservationKey::validity(&value),
        "Need remains intentionally non-cacheable"
    );
    let SourcePreparationOutcome::Need(needs) = value else {
        panic!("local replay still emits an unadmitted request")
    };
    let current = needs
        .workspace_operations()
        .values()
        .next()
        .unwrap()
        .request();
    assert_eq!(retained.as_ref(), current.as_ref());
}
