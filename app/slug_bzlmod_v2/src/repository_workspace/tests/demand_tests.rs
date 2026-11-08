use super::*;
use crate::RepositoryMaterializationEpochEntry;
use crate::RepositoryMaterializationKind;
use crate::RepositoryMaterializationRequest;
use crate::RepositoryMaterializationRequestId;
use crate::RepositoryMaterializationResult;
use crate::SourcePreparationNeeds;
use crate::SourcePreparationNeedsError;
use crate::source_preparation::RepositoryWorkspaceStableKey;

fn demand(
    reservation: &Arc<RepositoryWorkspaceReservation>,
) -> RepositoryWorkspaceReservationDemand {
    RepositoryWorkspaceReservationDemand::new(
        Arc::new(
            RepositoryWorkspaceStableKey::new(
                reservation.workspace().clone(),
                reservation.owner().clone(),
                reservation.repository().clone(),
                reservation.ordinal(),
            )
            .unwrap(),
        ),
        reservation.expected_repo_spec().clone(),
    )
    .unwrap()
}
fn selected(
    reservation: &Arc<RepositoryWorkspaceReservation>,
    repo: &str,
) -> RepositoryMaterializationEpochEntry {
    RepositoryMaterializationEpochEntry {
        request: Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: reservation.workspace().clone(),
                canonical_repo: CanonicalRepoName::new(repo).unwrap(),
            },
            repo_spec: reservation.expected_repo_spec().as_ref().clone(),
            kind: RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/selected").unwrap(),
            },
        }),
        result: RepositoryMaterializationResult::SpecError("not a native process view".into()),
    }
}
#[test]
fn reservation_demand_requires_current_full_binding_and_conflicting_needs_refuse() {
    let r = reservation_with(7, 11, limits());
    let d = demand(&r);
    let empty = RepositoryWorkspaceEpoch::fresh(r.workspace().clone(), 7, limits()).unwrap();
    assert!(d.find_in(&empty).unwrap().is_none());
    let bound = empty.reserve(r.clone(), 0o755).unwrap();
    assert_eq!(d.find_in(&bound).unwrap().unwrap().reservation(), &r);
    let mut spec = r.expected_repo_spec().as_ref().clone();
    let mut attrs = (*spec.attributes).clone();
    attrs.insert(
        "version".into(),
        crate::OverrideAttributeValue::String("changed".into()),
    );
    spec.attributes = Arc::new(attrs);
    let changed = Arc::new(
        RepositoryWorkspaceReservationDemand::new(d.key().clone(), Arc::new(spec)).unwrap(),
    );
    assert_eq!(
        changed.find_in(&bound).unwrap_err(),
        RepositoryWorkspaceError::ForeignReservation
    );
    assert_eq!(
        d.find_in(&RepositoryWorkspaceEpoch::revoked()).unwrap_err(),
        RepositoryWorkspaceError::Revoked
    );
    let foreign = RepositoryWorkspaceEpoch::fresh(
        NormalizedAbsolutePath::new("/foreign").unwrap(),
        7,
        limits(),
    )
    .unwrap();
    assert_eq!(
        d.find_in(&foreign).unwrap_err(),
        RepositoryWorkspaceError::ForeignReservation
    );
    let need = SourcePreparationNeeds::repository_workspace_reservation(Arc::new(d));
    assert_eq!(need.try_union(&need).unwrap(), need);
    let changed = SourcePreparationNeeds::repository_workspace_reservation(changed);
    assert!(matches!(
        need.try_union(&changed),
        Err(SourcePreparationNeedsError::ConflictingWorkspaceReservation { .. })
    ));
}
#[test]
fn workspace_operation_need_preserves_independent_unsupported_rows_and_full_conflicts() {
    let t = transcript();
    let request = Arc::new(
        t.request(RepositoryWorkspaceOperation::Exists("".into()))
            .unwrap(),
    );
    let a = selected(t.reservation(), "a");
    let z = selected(t.reservation(), "z");
    let first = Arc::new(
        RepositoryWorkspaceOperationDemand::new(request.clone(), Arc::from([z.clone(), a.clone()]))
            .unwrap(),
    );
    let second = Arc::new(
        RepositoryWorkspaceOperationDemand::new(request.clone(), Arc::from([a.clone(), z.clone()]))
            .unwrap(),
    );
    assert_eq!(first, second);
    assert_eq!(first.selected_entries().len(), 2);
    assert_eq!(first.selected_entries()[0], a);
    let need = SourcePreparationNeeds::repository_workspace_operation(first.clone());
    let equal = SourcePreparationNeeds::repository_workspace_operation(second);
    assert_eq!(need.try_union(&equal).unwrap(), need);
    let changed = SourcePreparationNeeds::repository_workspace_operation(Arc::new(
        RepositoryWorkspaceOperationDemand::new(request.clone(), Arc::from([z])).unwrap(),
    ));
    assert!(matches!(
        need.try_union(&changed),
        Err(SourcePreparationNeedsError::ConflictingWorkspaceOperation { .. })
    ));
    let other_request = Arc::new(
        t.request(RepositoryWorkspaceOperation::Read("data".into()))
            .unwrap(),
    );
    let other = SourcePreparationNeeds::repository_workspace_operation(Arc::new(
        RepositoryWorkspaceOperationDemand::new(other_request, Arc::from([])).unwrap(),
    ));
    assert!(matches!(
        need.try_union(&other),
        Err(SourcePreparationNeedsError::ConflictingWorkspaceOperation { .. })
    ));
    assert!(
        RepositoryWorkspaceOperationDemand::new(request.clone(), Arc::from([a.clone(), a]))
            .is_err()
    );
    let too_many = (0..129)
        .map(|i| selected(t.reservation(), &format!("repo{i}")))
        .collect::<Vec<_>>();
    assert_eq!(
        RepositoryWorkspaceOperationDemand::new(request, too_many.into()).unwrap_err(),
        RepositoryWorkspaceError::RetentionLimit
    );
}
#[tokio::test]
async fn workspace_policy_default_and_cached_switches_are_dice_observed() {
    use dice::DetectCycles;
    use dice::Dice;
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = RepositoryWorkspaceExecutionPolicyKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    let mut tx = dice.updater().commit().await;
    assert_eq!(
        tx.compute(&key).await.unwrap(),
        RepositoryWorkspaceExecutionPolicy::Disabled
    );
    drop(tx);
    for policy in [
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        RepositoryWorkspaceExecutionPolicy::Disabled,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
    ] {
        let mut updater = dice.updater();
        updater.changed_to([(key.clone(), policy)]).unwrap();
        let mut tx = updater.commit().await;
        assert_eq!(tx.compute(&key).await.unwrap(), policy);
    }
}
