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

fn builtin_source_state(lease: u64, digest: u8) -> Arc<crate::RepositorySourceSnapshotState> {
    let route = Arc::new(crate::HostCanonicalRepositoryRoute::builtin(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        crate::HostBuiltinBazelToolsRepositoryMapping::testing(),
    ));
    let input = crate::host_canonical_repository_source_input(route, None).unwrap();
    let request =
        Arc::new(crate::RepositorySourceSnapshotRequest::from_canonical(&input, lease).unwrap());
    let reservation = Arc::new(
        crate::RepositorySourceSnapshotReservation::new(
            request,
            NormalizedAbsolutePath::new("/__slug_repositories__/builtin").unwrap(),
            PathObservationInstanceId::new(100),
        )
        .unwrap(),
    );
    Arc::new(
        crate::RepositorySourceSnapshotState::new(
            reservation,
            vec![
                crate::RepositorySourceSnapshotEntry::new(
                    "".into(),
                    0o755,
                    crate::RepositorySourceSnapshotNode::Directory,
                )
                .unwrap(),
                crate::RepositorySourceSnapshotEntry::new(
                    "data".into(),
                    0o644,
                    crate::RepositorySourceSnapshotNode::File(
                        slug_workspace_v2::FileContentDigest::new([digest; 32], 1).unwrap(),
                    ),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    )
}

#[test]
fn operation_demand_authenticates_independent_source_declarations_against_execute_manifest() {
    let t = transcript();
    let source = builtin_source_state(7, 1);
    let with = |sources: Vec<Arc<crate::RepositorySourceSnapshotState>>| {
        let RepositoryWorkspaceOperation::Execute(command) = execute() else {
            panic!("execute fixture")
        };
        let manifest = RepositoryWorkspaceReadViewManifest::new(
            t.reservation(),
            sources
                .into_iter()
                .map(|s| RepositoryWorkspaceReadView::from_source(s).unwrap())
                .collect(),
        )
        .unwrap();
        Arc::new(
            t.request(RepositoryWorkspaceOperation::Execute(
                command.with_read_views(manifest).unwrap(),
            ))
            .unwrap(),
        )
    };
    let request = with(vec![source.clone()]);
    assert_eq!(
        RepositoryWorkspaceOperationDemand::new(request.clone(), Arc::from([])).unwrap_err(),
        RepositoryWorkspaceError::ReplayConflict
    );
    assert_eq!(
        RepositoryWorkspaceOperationDemand::new_with_sources(
            with(vec![]),
            Arc::from([]),
            Arc::from([source.clone()])
        )
        .unwrap_err(),
        RepositoryWorkspaceError::ReplayConflict
    );
    assert_eq!(
        RepositoryWorkspaceOperationDemand::new_with_sources(
            request.clone(),
            Arc::from([]),
            Arc::from([builtin_source_state(7, 2)])
        )
        .unwrap_err(),
        RepositoryWorkspaceError::ReplayConflict
    );
    let need = RepositoryWorkspaceOperationDemand::new_with_sources(
        request,
        Arc::from([]),
        Arc::from([source.clone()]),
    )
    .unwrap();
    assert_eq!(need.declared_sources(), &[source]);
}

#[test]
fn source_declarations_keep_nonexecute_prefixes_and_refuse_alias_foreign_limit_and_need_conflicts()
{
    let t = transcript();
    let source = builtin_source_state(7, 1);
    let request = Arc::new(
        t.request(RepositoryWorkspaceOperation::Exists("".into()))
            .unwrap(),
    );
    let unsupported = selected(t.reservation(), "unsupported");
    let need = Arc::new(
        RepositoryWorkspaceOperationDemand::new_with_sources(
            request.clone(),
            Arc::from([unsupported.clone()]),
            Arc::from([source.clone()]),
        )
        .unwrap(),
    );
    assert_eq!(need.selected_entries(), &[unsupported]);
    assert_eq!(need.declared_sources(), &[source.clone()]);
    let original = SourcePreparationNeeds::repository_workspace_operation(need);
    let changed = SourcePreparationNeeds::repository_workspace_operation(Arc::new(
        RepositoryWorkspaceOperationDemand::new_with_sources(
            request.clone(),
            Arc::from([selected(t.reservation(), "unsupported")]),
            Arc::from([builtin_source_state(7, 2)]),
        )
        .unwrap(),
    ));
    assert!(matches!(
        original.try_union(&changed),
        Err(SourcePreparationNeedsError::ConflictingWorkspaceOperation { .. })
    ));
    let refused_sources: [Arc<[Arc<crate::RepositorySourceSnapshotState>]>; 2] = [
        Arc::from([builtin_source_state(8, 1)]),
        Arc::from([source.clone(), source.clone()]),
    ];
    for sources in refused_sources {
        assert_eq!(
            RepositoryWorkspaceOperationDemand::new_with_sources(
                request.clone(),
                Arc::from([]),
                sources
            )
            .unwrap_err(),
            RepositoryWorkspaceError::ForeignReservation
        );
    }
    assert_eq!(
        RepositoryWorkspaceOperationDemand::new_with_sources(
            request.clone(),
            Arc::from([selected(t.reservation(), "bazel_tools")]),
            Arc::from([source.clone()])
        )
        .unwrap_err(),
        RepositoryWorkspaceError::ForeignReservation
    );
    let entries = (0..128)
        .map(|i| selected(t.reservation(), &format!("repo{i}")))
        .collect::<Vec<_>>();
    assert_eq!(
        RepositoryWorkspaceOperationDemand::new_with_sources(
            request,
            entries.into(),
            Arc::from([source])
        )
        .unwrap_err(),
        RepositoryWorkspaceError::RetentionLimit
    );
}
