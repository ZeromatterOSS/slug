//! Real mapping producers and committed DICE updates exercise the preference.
use dice::DetectCycles;
use dice::Dice;
use slug_bzlmod_v2::RepositorySourceSnapshotEntry;
use slug_bzlmod_v2::RepositorySourceSnapshotEpoch;
use slug_bzlmod_v2::RepositorySourceSnapshotNode;
use slug_bzlmod_v2::RepositorySourceSnapshotReservation;
use slug_bzlmod_v2::RepositoryWorkspaceEpoch;
use slug_workspace_v2::AnchoredPathFileDigestObservationKey;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::PathFileDigest;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::*;

fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/registration-errors").unwrap()
}
fn label() -> CanonicalLabel {
    CanonicalLabel::parse("@@//:template").unwrap()
}
async fn fixture(dice: &Arc<Dice>, renamed: bool) -> dice::DiceTransaction {
    super::super::super::registration_error_tests::registration_transaction(
        dice,
        if renamed {
            "module(name='bazel_tools',repo_name='alternate')\n"
        } else {
            "module(name='bazel_tools')\n"
        }
        .to_owned(),
    )
    .await
}
async fn request(
    tx: &mut dice::DiceTransaction,
    lease: u64,
) -> Arc<RepositorySourceSnapshotRequest> {
    let SourcePreparationOutcome::Complete(Ok(observed)) = tx
        .compute(&HostRootRepositoryMappingObservationKey::new(workspace()))
        .await
        .unwrap()
    else {
        panic!("genuine root mapping producer must complete")
    };
    Arc::new(
        RepositorySourceSnapshotRequest::from_workspace(
            workspace(),
            observed.result().as_ref().as_ref().unwrap(),
            lease,
        )
        .unwrap(),
    )
}
fn state(
    request: Arc<RepositorySourceSnapshotRequest>,
    file: bool,
) -> Arc<RepositorySourceSnapshotState> {
    let reservation = Arc::new(
        RepositorySourceSnapshotReservation::new(
            request,
            NormalizedAbsolutePath::new("/__slug_repositories__/v1/source").unwrap(),
            PathObservationInstanceId::new(101),
        )
        .unwrap(),
    );
    let mut entries = vec![
        RepositorySourceSnapshotEntry::new(
            "".into(),
            0o755,
            RepositorySourceSnapshotNode::Directory,
        )
        .unwrap(),
    ];
    if file {
        entries.push(
            RepositorySourceSnapshotEntry::new(
                "template".into(),
                0o644,
                RepositorySourceSnapshotNode::File(FileContentDigest::new([7; 32], 3).unwrap()),
            )
            .unwrap(),
        );
    }
    Arc::new(RepositorySourceSnapshotState::new(reservation, entries).unwrap())
}
async fn publish(
    tx: dice::DiceTransaction,
    lease: u64,
    sources: RepositorySourceSnapshotEpoch,
) -> dice::DiceTransaction {
    let mut update = tx.into_updater();
    update
        .changed_to(vec![(
            RepositoryWorkspaceExecutionPolicyKey::new(workspace()),
            RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        )])
        .unwrap();
    update
        .changed_to(vec![(
            RepositoryWorkspaceEpochKey::new(workspace()),
            RepositoryWorkspaceEpoch::fresh(
                workspace(),
                lease,
                slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits::new(
                    128,
                    16 * 1024 * 1024,
                    1024 * 1024 * 1024,
                )
                .unwrap(),
            )
            .unwrap(),
        )])
        .unwrap();
    update
        .changed_to(vec![(
            RepositorySourceSnapshotEpochKey::new(workspace()),
            Arc::new(sources),
        )])
        .unwrap();
    update.commit().await
}
fn key() -> CapturedSourceSnapshotKey {
    CapturedSourceSnapshotKey::new(workspace(), label().package().repo().clone())
}
fn selected(
    value: Arc<LoadingPreparationOutcome<Result<CapturedSourceSnapshot, AnalysisError>>>,
) -> CapturedSourceSnapshot {
    let LoadingPreparationOutcome::Complete(Ok(value)) = value.as_ref() else {
        panic!("expected successful selection: {value:?}")
    };
    value.clone()
}

#[tokio::test]
async fn committed_uncaptured_to_captured_source_replaces_host_path_and_preserves_anchor_digest() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut tx = fixture(&dice, false).await;
    let request = request(&mut tx, 11).await;
    let sources = RepositorySourceSnapshotEpoch::fresh(workspace(), 11).unwrap();
    let mut tx = publish(tx, 11, sources.clone()).await;
    let absent = selected(tx.compute(&key()).await.unwrap());
    assert!(absent.state.is_none());
    assert!(
        !absent.observations.observations().is_empty(),
        "None still retains natural mapping observations"
    );
    assert!(matches!(
        resolve_source_input_observed(&mut tx, &workspace(), &label()).await,
        LoadingPreparationOutcome::Need(_)
    ));

    let source = state(request, true);
    let mut tx = publish(tx, 11, sources.with_snapshot(source.clone()).unwrap()).await;
    let present = selected(tx.compute(&key()).await.unwrap());
    assert!(Arc::ptr_eq(present.state.as_ref().unwrap(), &source));
    let LoadingPreparationOutcome::Need(needs) =
        resolve_source_input_observed(&mut tx, &workspace(), &label()).await
    else {
        panic!("captured root needs its own lstat")
    };
    let root = source.reservation().display_root().clone();
    let namespace = PathObservationNamespace::Materialization(source.reservation().instance());
    assert!(
        needs
            .path_observations()
            .unwrap()
            .demands()
            .iter()
            .all(|d| d.namespace() == namespace && d.path() == &root)
    );

    let mut rows = present
        .observations
        .observations()
        .iter()
        .map(|(d, r)| (d.dupe(), r.dupe()))
        .collect::<Vec<_>>();
    for (path, kind, size) in [
        (root.clone(), PathNodeKind::Directory, 0),
        (
            NormalizedAbsolutePath::new(root.as_path().join("template")).unwrap(),
            PathNodeKind::RegularFile,
            3,
        ),
    ] {
        rows.push((
            PathObservationDemand::new(namespace, path, PathObservationOperation::Lstat),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(kind, size, 2, 3, 4, 0o644),
            ))),
        ));
    }
    let digest = FileContentDigest::new([7; 32], 3).unwrap();
    rows.push((
        PathObservationDemand::new(
            namespace,
            NormalizedAbsolutePath::new(root.as_path().join("template")).unwrap(),
            PathObservationOperation::FileDigest,
        ),
        Arc::new(PathObservationResult::FileDigest(
            PathOperationResult::Present(digest),
        )),
    ));
    let mut update = tx.into_updater();
    update
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::from_shared(rows).unwrap(),
        )])
        .unwrap();
    let mut tx = update.commit().await;
    let LoadingPreparationOutcome::Complete(Ok(observed)) =
        resolve_source_input_observed(&mut tx, &workspace(), &label()).await
    else {
        panic!("captured source must resolve")
    };
    let resolved = observed.result().as_ref().unwrap();
    assert_eq!(resolved.namespace(), namespace);
    assert_eq!(resolved.anchor().unwrap().root(), &root);
    assert!(
        observed
            .observations()
            .observations()
            .iter()
            .filter(|(d, _)| d.namespace() == namespace)
            .all(|(d, _)| d.path().as_path().starts_with(root.as_path()))
    );
    let PathOutcome::Complete(Ok(observed_digest)) = tx
        .compute(
            &AnchoredPathFileDigestObservationKey::new(
                resolved.anchor().unwrap().clone(),
                resolved.requested_path().as_path(),
            )
            .unwrap(),
        )
        .await
        .unwrap()
    else {
        panic!("actual anchored digest observation")
    };
    assert_eq!(
        observed_digest.result(),
        &Ok(PathFileDigest::Present(digest))
    );

    let mut tx = publish(
        tx,
        12,
        RepositorySourceSnapshotEpoch::fresh(workspace(), 12).unwrap(),
    )
    .await;
    assert!(
        selected(tx.compute(&key()).await.unwrap()).state.is_none(),
        "new command cannot reuse an old Source instance"
    );
    let mut tx = publish(tx, 12, RepositorySourceSnapshotEpoch::revoked()).await;
    assert!(matches!(
        tx.compute(&key()).await.unwrap().as_ref(),
        LoadingPreparationOutcome::Complete(Err(_))
    ));
}

#[tokio::test]
async fn source_preference_refuses_foreign_epoch_and_changed_natural_mapping_without_host_fallback()
{
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut tx = fixture(&dice, false).await;
    let old_request = request(&mut tx, 21).await;
    let source = state(old_request, false);
    let foreign =
        RepositorySourceSnapshotEpoch::fresh(NormalizedAbsolutePath::new("/foreign").unwrap(), 21)
            .unwrap();
    let mut tx = publish(tx, 21, foreign).await;
    assert!(matches!(
        tx.compute(&key()).await.unwrap().as_ref(),
        LoadingPreparationOutcome::Complete(Err(_))
    ));
    drop(tx);
    let mut tx = fixture(&dice, true).await;
    let changed = request(&mut tx, 21).await;
    assert_ne!(
        &changed,
        source.reservation().request(),
        "actual MODULE repo_name changes the natural mapping"
    );
    let sources = RepositorySourceSnapshotEpoch::fresh(workspace(), 21)
        .unwrap()
        .with_snapshot(source)
        .unwrap();
    let mut tx = publish(tx, 21, sources).await;
    assert!(
        matches!(resolve_source_input_observed(&mut tx,&workspace(),&label()).await, LoadingPreparationOutcome::Complete(Ok(observed)) if observed.result().is_err()),
        "same-slot conflict must not issue a Host Need"
    );
}

#[tokio::test]
async fn disabled_and_catalog_only_do_not_require_command_epochs() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    assert!(selected(tx.compute(&key()).await.unwrap()).state.is_none());
    let mut update = tx.into_updater();
    update
        .changed_to(vec![(
            RepositoryWorkspaceExecutionPolicyKey::new(workspace()),
            RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        )])
        .unwrap();
    let mut tx = update.commit().await;
    let LoadingPreparationOutcome::Complete(Ok(value)) =
        resolve_catalog_source_input_observed(&mut tx, &workspace(), &label()).await
    else {
        panic!("CatalogOnly root refusal must not depend on Source epochs")
    };
    assert!(value.result().is_err());
    assert!(value.observations().observations().is_empty());
}

#[tokio::test]
async fn ordinary_route_keeps_uncaptured_producer_but_refuses_an_occupied_source_slot() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut tx = fixture(&dice, false).await;
    let request = request(&mut tx, 31).await;
    let observed = selected(
        publish(
            tx,
            31,
            RepositorySourceSnapshotEpoch::fresh(workspace(), 31).unwrap(),
        )
        .await
        .compute(&key())
        .await
        .unwrap(),
    );
    let empty = RepositorySourceSnapshotEpoch::fresh(workspace(), 31).unwrap();
    let ordinary =
        uncaptured_ordinary(&empty, request.repository(), observed.observations.clone()).unwrap();
    assert!(ordinary.state.is_none());
    assert_eq!(ordinary.observations, observed.observations);
    let occupied = empty.with_snapshot(state(request.clone(), true)).unwrap();
    assert!(uncaptured_ordinary(&occupied, request.repository(), observed.observations).is_err());
    assert!(
        uncaptured_ordinary(
            &RepositorySourceSnapshotEpoch::revoked(),
            request.repository(),
            PathObservationEpoch::empty()
        )
        .is_err()
    );
}
