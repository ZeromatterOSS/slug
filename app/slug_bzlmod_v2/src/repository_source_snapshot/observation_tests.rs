use super::*;
use crate::RepositoryWorkspaceEpoch;
use crate::RepositoryWorkspaceRetentionLimits;

fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/workspace").unwrap()
}
fn command_context(lease: u64) -> Arc<RepositoryWorkspaceContext> {
    RepositoryWorkspaceContext::from_epoch(
        &RepositoryWorkspaceEpoch::fresh(
            workspace(),
            lease,
            RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024)
                .unwrap(),
        )
        .unwrap(),
        &workspace(),
    )
    .unwrap()
}
fn request() -> Arc<RepositorySourceSnapshotRequest> {
    let route = Arc::new(crate::HostCanonicalRepositoryRoute::builtin(
        workspace(),
        crate::HostBuiltinBazelToolsRepositoryMapping::testing(),
    ));
    let input = crate::host_canonical_repository_source_input(route, None).unwrap();
    Arc::new(RepositorySourceSnapshotRequest::from_canonical(&input, 7).unwrap())
}
#[tokio::test]
async fn source_projection_uses_exact_request_state_and_context_without_absence_fallback() {
    use dice::DetectCycles;
    use dice::Dice;
    let request = request();
    let context = command_context(7);
    let key = RepositorySourceSnapshotProjection::new(context.clone(), request.clone()).unwrap();
    let empty = RepositorySourceSnapshotEpoch::fresh(workspace(), 7).unwrap();
    let missing =
        RepositorySourceSnapshotObservation::from_epoch(&empty, context.clone(), request.clone())
            .unwrap();
    assert!(missing.state().is_none());
    assert!(RepositorySourceSnapshotProjection::validity(&Ok(
        missing.clone()
    )));
    let reservation = Arc::new(
        RepositorySourceSnapshotReservation::new(
            request.clone(),
            NormalizedAbsolutePath::new("/__slug_repositories__/source").unwrap(),
            PathObservationInstanceId::new(31),
        )
        .unwrap(),
    );
    let state = Arc::new(
        RepositorySourceSnapshotState::new(
            reservation,
            vec![
                RepositorySourceSnapshotEntry::new(
                    "".into(),
                    0o755,
                    RepositorySourceSnapshotNode::Directory,
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let published = empty.with_snapshot(state.clone()).unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = RepositorySourceSnapshotEpochKey::new(workspace());
    let mut update = dice.updater();
    update
        .changed_to([(base.clone(), Arc::new(empty))])
        .unwrap();
    let mut tx = update.commit().await;
    let opaque = tx.compute_opaque(&base).await.unwrap();
    assert_eq!(tx.projection(&opaque, &key).unwrap().unwrap(), missing);
    drop(tx);
    let mut update = dice.updater();
    update
        .changed_to([(base.clone(), Arc::new(published.clone()))])
        .unwrap();
    let mut tx = update.commit().await;
    let opaque = tx.compute_opaque(&base).await.unwrap();
    let found = tx.projection(&opaque, &key).unwrap().unwrap();
    assert!(Arc::ptr_eq(found.state().unwrap(), &state));
    assert_ne!(found, missing);
    assert_eq!(found.request(), &request);
    let different_mode = Arc::new(
        RepositorySourceSnapshotState::new(
            state.reservation().clone(),
            vec![
                RepositorySourceSnapshotEntry::new(
                    "".into(),
                    0o700,
                    RepositorySourceSnapshotNode::Directory,
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let changed_epoch = RepositorySourceSnapshotEpoch::fresh(workspace(), 7)
        .unwrap()
        .with_snapshot(different_mode)
        .unwrap();
    let changed = RepositorySourceSnapshotObservation::from_epoch(
        &changed_epoch,
        context.clone(),
        request.clone(),
    )
    .unwrap();
    assert_ne!(
        found, changed,
        "full image mode is part of observation equality"
    );
    assert_eq!(
        RepositorySourceSnapshotProjection::new(command_context(8), request.clone()),
        Err(RepositoryWorkspaceError::ForeignReservation)
    );
    assert_eq!(
        RepositorySourceSnapshotObservation::from_epoch(
            &RepositorySourceSnapshotEpoch::fresh(workspace(), 8).unwrap(),
            context.clone(),
            request.clone()
        ),
        Err(RepositoryWorkspaceError::ForeignReservation)
    );
    assert_eq!(
        RepositorySourceSnapshotObservation::from_epoch(
            &RepositorySourceSnapshotEpoch::revoked(),
            context.clone(),
            request.clone()
        ),
        Err(RepositoryWorkspaceError::Revoked)
    );
    // Deliberately foreign pure metadata exercises full Eq beyond the coarse
    // request Hash. It is never a natural source publication or native owner.
    let mut foreign = request.as_ref().clone();
    foreign.selected_registry = !foreign.selected_registry;
    let foreign = Arc::new(foreign);
    assert_eq!(
        RepositorySourceSnapshotObservation::from_epoch(
            &published,
            context.clone(),
            foreign.clone()
        ),
        Err(RepositoryWorkspaceError::ReplayConflict)
    );
    assert_ne!(
        key,
        RepositorySourceSnapshotProjection::new(context, foreign).unwrap()
    );
    assert!(!RepositorySourceSnapshotProjection::validity(&Err(
        RepositoryWorkspaceError::Revoked
    )));
}
