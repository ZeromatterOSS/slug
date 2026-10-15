use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::super::tests::request;
use super::*;

fn need(root: &NormalizedAbsolutePath, body: &[u8], executable: bool) -> SourcePreparationNeeds {
    SourcePreparationNeeds::repository(request(root, "ordinary+", &[("marker", body, executable)]))
}
fn immutable(
    entry: &RepositoryMaterializationEpochEntry,
) -> (
    &Arc<str>,
    &std::path::PathBuf,
    slug_workspace_v2::PathObservationInstanceId,
) {
    let RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
        source_identity,
        generation_root,
        observation_instance,
    }) = &entry.result
    else {
        panic!("actual immutable writer result")
    };
    (source_identity, generation_root, *observation_instance)
}
async fn again(
    dice: &Arc<dice::Dice>,
    workspace: &Path,
    need: &SourcePreparationNeeds,
    current: PathObservationEpoch,
) -> (DiceTransaction, RepositoryMaterializationEpochEntry) {
    let mut updater = dice.updater();
    let SavedInputs {
        mut prior,
        mut budget,
        floor,
        ..
    } = capture(&updater, workspace).await.unwrap();
    let root = NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap();
    updater
        .changed_to(slug_workspace_v2::path_observation_shards(&current))
        .unwrap();
    updater
        .changed_to([(PathObservationEpochKey, current)])
        .unwrap();
    updater
        .changed_to([(
            RepositoryMaterializationResultEpochKey {
                workspace: root.clone(),
            },
            RepositoryMaterializationResultEpoch::new(root.clone(), []).unwrap(),
        )])
        .unwrap();
    let mut entries = Vec::new();
    let tx = super::super::materialize(
        updater.commit().await,
        workspace,
        &root,
        need,
        &mut entries,
        &mut budget,
        &mut prior,
        floor,
        &[],
    )
    .await
    .unwrap();
    assert_eq!(entries.len(), 1);
    (tx, entries.remove(0))
}
fn host(workspace: &Path) -> (PathObservationEpoch, PathObservationDemand) {
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(workspace.join("caller-absent")).unwrap(),
        PathObservationOperation::Lstat,
    );
    (
        PathObservationEpoch::from_shared([(
            demand.clone(),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing)),
        )])
        .unwrap(),
        demand,
    )
}

#[tokio::test]
async fn initial_injected_absence_is_precise_and_other_cancellations_propagate() {
    let workspace = crate::scratch();
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let saved = capture(&dice.updater(), &workspace).await.unwrap();
    assert!(saved.prior.is_none());
    assert_eq!(saved.floor, 0);
    assert!(
        injected::<()>(Err(DiceError::cancelled(CancellationReason::Rejected)))
            .unwrap()
            .is_none()
    );
    for reason in [
        CancellationReason::ByTest,
        CancellationReason::TransactionCancelled,
        CancellationReason::OutdatedEpoch,
    ] {
        assert!(injected::<()>(Err(DiceError::cancelled(reason))).is_err());
    }
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let (tx, entry) = again(
        &dice,
        &workspace,
        &need(&root, b"A", false),
        host(&workspace).0,
    )
    .await;
    assert_eq!(
        std::fs::read(immutable(&entry).1.join("marker")).unwrap(),
        b"A"
    );
    drop(tx);
    drop(saved);
    drop(dice);
    std::fs::remove_dir_all(workspace).unwrap();
}

#[tokio::test]
async fn repeated_foreign_equal_requests_retain_actual_namespace_arcs_without_new_directories() {
    let workspace = crate::scratch();
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let (current, _) = host(&workspace);
    let (mut tx, first) = again(
        &dice,
        &workspace,
        &need(&root, b"A", false),
        current.clone(),
    )
    .await;
    let original = tx.compute(&PathObservationEpochKey).await.unwrap();
    let count = std::fs::read_dir(&workspace).unwrap().count();
    for _ in 0..70 {
        // A genuinely fresh equal plan/request allocation, not an Arc hit.
        let next = need(&root, b"A", false);
        let foreign = next.repository_materializations().values().next().unwrap();
        assert!(!Arc::ptr_eq(&first.request, foreign));
        let (mut next_tx, entry) = again(&dice, &workspace, &next, current.clone()).await;
        assert_eq!(immutable(&entry), immutable(&first));
        let epoch = next_tx.compute(&PathObservationEpochKey).await.unwrap();
        for (demand, value) in original.observations() {
            if matches!(
                demand.namespace(),
                PathObservationNamespace::Materialization(_)
            ) {
                assert!(Arc::ptr_eq(value, epoch.get(demand).unwrap()));
            }
        }
        assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), count);
        tx = next_tx;
    }
    drop(tx);
    drop(dice);
    std::fs::remove_dir_all(workspace).unwrap();
}

#[tokio::test]
async fn current_missing_and_host_withholding_override_saved_namespace_inputs() {
    let workspace = crate::scratch();
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let (current, withheld) = host(&workspace);
    let request = need(&root, b"A", false);
    let (tx, first) = again(&dice, &workspace, &request, current).await;
    drop(tx);
    let (_, generation, instance) = immutable(&first);
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Materialization(instance),
        NormalizedAbsolutePath::new(generation.join("marker")).unwrap(),
        PathObservationOperation::FileBytes,
    );
    let missing = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Missing,
    ));
    let explicit = PathObservationEpoch::from_shared([(demand.clone(), missing.clone())]).unwrap();
    let (mut tx, reused) = again(&dice, &workspace, &request, explicit).await;
    assert_eq!(immutable(&reused), immutable(&first));
    let epoch = tx.compute(&PathObservationEpochKey).await.unwrap();
    assert!(Arc::ptr_eq(epoch.get(&demand).unwrap(), &missing));
    assert!(
        epoch.get(&withheld).is_none(),
        "saved Host input backfilled a caller omission"
    );
    drop(tx);
    drop(dice);
    std::fs::remove_dir_all(workspace).unwrap();
}

#[tokio::test]
async fn changed_plan_and_attributes_never_reuse_stale_source_and_a_b_a_keeps_current_inputs() {
    let workspace = crate::scratch();
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let current = host(&workspace).0;
    let (tx, a) = again(
        &dice,
        &workspace,
        &need(&root, b"A", false),
        current.clone(),
    )
    .await;
    drop(tx);
    let (tx, b) = again(&dice, &workspace, &need(&root, b"B", true), current.clone()).await;
    assert_ne!(immutable(&a).1, immutable(&b).1);
    assert!(immutable(&b).2.value() > immutable(&a).2.value());
    assert_eq!(std::fs::read(immutable(&b).1.join("marker")).unwrap(), b"B");
    drop(tx);
    let mut changed = request(&root, "ordinary+", &[("marker", b"B", true)]);
    changed.repo_spec.attributes = Arc::new(starlark_map::small_map::SmallMap::from_iter([(
        compact_str::CompactString::from("last"),
        slug_bzlmod_v2::OverrideAttributeValue::String("changed".into()),
    )]));
    let (tx, attrs) = again(
        &dice,
        &workspace,
        &SourcePreparationNeeds::repository(changed),
        current.clone(),
    )
    .await;
    assert_ne!(immutable(&b).1, immutable(&attrs).1);
    drop(tx);
    let (tx, restored) = again(&dice, &workspace, &need(&root, b"A", false), current).await;
    assert_eq!(immutable(&a).0, immutable(&restored).0);
    assert_ne!(immutable(&attrs).1, immutable(&restored).1);
    assert_eq!(
        std::fs::read(immutable(&restored).1.join("marker")).unwrap(),
        b"A"
    );
    drop(tx);
    drop(dice);
    std::fs::remove_dir_all(workspace).unwrap();
}

#[path = "tests/refusal.rs"]
mod refusal;

#[path = "tests/initial.rs"]
mod initial;
