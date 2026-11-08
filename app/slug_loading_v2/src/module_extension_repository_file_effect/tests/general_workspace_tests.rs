//! DICE policy/freshness fences around the actual original-evaluation route.
use slug_bzlmod_v2::RepositoryWorkspaceEntry;
use slug_bzlmod_v2::RepositoryWorkspaceEpoch;
use slug_bzlmod_v2::RepositoryWorkspaceEpochKey;
use slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicy;
use slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicyKey;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceReservation;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits;

use super::*;
async fn update(
    transaction: dice::DiceTransaction,
    policy: RepositoryWorkspaceExecutionPolicy,
    epoch: RepositoryWorkspaceEpoch,
) -> dice::DiceTransaction {
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            RepositoryWorkspaceExecutionPolicyKey::new(workspace.clone()),
            policy,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            RepositoryWorkspaceEpochKey::new(workspace.clone()),
            epoch,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace.clone(),
            },
            RepositoryMaterializationResultEpoch::new(workspace, []).unwrap(),
        )])
        .unwrap();
    updater.commit().await
}
fn empty(lease: u64) -> RepositoryWorkspaceEpoch {
    RepositoryWorkspaceEpoch::fresh(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        lease,
        RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024).unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn original_general_policy_invalidates_disabled_prepared_result_and_fresh_zero_execute_commands()
 {
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut transaction = transaction_untracked(&dice, MODULE, EXTENSION, true).await;
    let owner = owner(&mut transaction).await;
    let key = HostSelectedRepositoryFileEffectKey::new(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        owner,
        0,
    );
    let SourcePreparationOutcome::Complete(legacy) = transaction.compute(&key).await.unwrap()
    else {
        panic!("Disabled original must complete")
    };
    assert_eq!(legacy.as_ref().as_ref().unwrap().plan().effects().len(), 2);
    let mut epoch = empty(71);
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        epoch.clone(),
    )
    .await;
    let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
        panic!("Enabled must not reuse old authored plan")
    };
    let demand = needs.workspace_reservations().values().next().unwrap();
    assert!(needs.repository_executes().is_empty());
    let reservation = Arc::new(
        RepositoryWorkspaceReservation::new(
            demand.key().workspace().clone(),
            demand.key().owner().clone(),
            demand.key().repository().clone(),
            demand.key().ordinal(),
            demand.repo_spec().clone(),
            NormalizedAbsolutePath::new("/__slug_repositories__/original-policy").unwrap(),
            PathObservationInstanceId::new(81),
            71,
            RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024)
                .unwrap(),
        )
        .unwrap(),
    );
    epoch = epoch.reserve(reservation, 0o755).unwrap();
    for index in 0..2 {
        transaction = update(
            transaction,
            RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
            epoch.clone(),
        )
        .await;
        let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
            panic!("new original File must suspend")
        };
        assert!(
            needs.repository_executes().is_empty()
                && needs.repository_filesystem_operations().is_empty()
        );
        let demand = needs.workspace_operations().values().next().unwrap();
        let request = demand.request();
        assert_eq!(request.index(), index);
        let RepositoryWorkspaceOperation::File { path, body, mode } = request.operation() else {
            panic!("actual original File operation required")
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
        entries.sort_by(|a, b| a.path().cmp(b.path()));
        epoch = epoch
            .complete(
                request.as_ref().clone(),
                RepositoryWorkspaceResult::Unit,
                request.predecessor().mode(),
                entries,
            )
            .unwrap();
    }
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        epoch,
    )
    .await;
    let SourcePreparationOutcome::Complete(general) = transaction.compute(&key).await.unwrap()
    else {
        panic!("exact observed prefix must complete")
    };
    let general = general.as_ref().as_ref().unwrap();
    assert_eq!(
        general
            .workspace_realization()
            .unwrap()
            .transcript()
            .records()
            .len(),
        2
    );
    assert!(general.plan().effects().is_empty());
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        empty(72),
    )
    .await;
    assert!(
        matches!(transaction.compute(&key).await.unwrap(),SourcePreparationOutcome::Need(needs) if needs.workspace_reservations().len()==1)
    );
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        RepositoryWorkspaceEpoch::revoked(),
    )
    .await;
    assert!(
        matches!(transaction.compute(&key).await.unwrap(),SourcePreparationOutcome::Complete(result) if result.is_err())
    );
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::Disabled,
        RepositoryWorkspaceEpoch::revoked(),
    )
    .await;
    let SourcePreparationOutcome::Complete(restored) = transaction.compute(&key).await.unwrap()
    else {
        panic!("Disabled semantic policy should restore old path")
    };
    assert_eq!(
        restored.as_ref().as_ref().unwrap().plan().effects().len(),
        2
    );
}
