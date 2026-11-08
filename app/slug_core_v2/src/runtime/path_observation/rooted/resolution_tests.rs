use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::symlink;
use std::time::Duration;
use std::time::Instant;

use dice::DetectCycles;
use dice::Dice;
use dice::Key;
use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::AnchoredPathResolutionError;
use slug_workspace_v2::AnchoredResolvedPathKey;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationKey;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::ResolvedPath;
use slug_workspace_v2::ResolvedPathState;

use super::tests::anchor;
use super::tests::limits;
use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;

fn scope() -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(5);
    SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/rooted-resolution-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap()
}

fn populate(backing: &Path, display: &Path, body: &[u8]) {
    std::fs::create_dir_all(backing.join("dir")).unwrap();
    std::fs::write(backing.join("body"), body).unwrap();
    symlink("dir", backing.join("alias")).unwrap();
    symlink(display.join("body"), backing.join("dir/leaf")).unwrap();
}

fn merge(epoch: &mut PathObservationEpoch, incoming: PathObservationEpoch) {
    *epoch = PathObservationEpoch::from_shared(
        epoch
            .observations()
            .iter()
            .chain(incoming.observations().iter())
            .map(|(demand, result)| (demand.clone(), result.clone())),
    )
    .unwrap();
}

async fn resolve_native(
    dice: &Arc<Dice>,
    root: &RootIo<'_>,
    anchor: &MaterializationPathAnchor,
    epoch: &mut PathObservationEpoch,
    scope: &Arc<SourceServiceScope>,
    logical: &Path,
) -> Result<ResolvedPath, AnchoredPathResolutionError> {
    let key = AnchoredResolvedPathKey::new(anchor.clone(), logical).unwrap();
    for _ in 0..16 {
        let mut updater = dice.updater();
        updater
            .changed_to([(PathObservationEpochKey, epoch.clone())])
            .unwrap();
        let mut transaction = updater.commit().await;
        let outcome = transaction.compute(&key).await.unwrap();
        drop(transaction);
        dice.wait_for_idle().await;
        match outcome {
            PathOutcome::Complete(result) => return result,
            PathOutcome::Need(need) => {
                assert!(!AnchoredResolvedPathKey::validity(&PathOutcome::Need(
                    need.clone()
                )));
                assert!(!need.demands().is_empty());
                for demand in need.demands() {
                    assert_eq!(demand.namespace(), anchor.namespace());
                    assert!(demand.path().as_path().starts_with(anchor.root().as_path()));
                    assert!(epoch.get(demand).is_none());
                }
                let attempts = scope.snapshot().source_attempts;
                let incoming =
                    observe_rooted(root, anchor, need.demands(), scope, &|| true).unwrap();
                assert!(need.demands().iter().all(|demand| matches!(
                    demand.operation(),
                    PathObservationOperation::Lstat
                        | PathObservationOperation::ReadLink
                        | PathObservationOperation::DirectoryEntries
                )));
                // Resolution metadata is still actually observed, but no
                // longer spends the content-acquisition SourceAttempts domain.
                assert_eq!(scope.snapshot().source_attempts, attempts);
                assert_eq!(scope.snapshot().live_buffer_bytes, 0);
                merge(epoch, incoming);
            }
        }
    }
    panic!("bounded real-root resolution did not complete")
}

async fn read_resolved(
    dice: &Arc<Dice>,
    root: &RootIo<'_>,
    anchor: &MaterializationPathAnchor,
    resolved: &ResolvedPath,
    epoch: &mut PathObservationEpoch,
    scope: &Arc<SourceServiceScope>,
) -> (Arc<[u8]>, FileContentDigest) {
    assert_eq!(resolved.namespace(), anchor.namespace());
    let mut values = Vec::new();
    for operation in [
        PathObservationOperation::FileBytes,
        PathObservationOperation::FileDigest,
    ] {
        let demand =
            PathObservationDemand::new(anchor.namespace(), resolved.real_path().clone(), operation);
        let key = PathObservationKey::new(demand.clone());
        let mut updater = dice.updater();
        updater
            .changed_to([(PathObservationEpochKey, epoch.clone())])
            .unwrap();
        let mut transaction = updater.commit().await;
        let PathOutcome::Need(need) = transaction.compute(&key).await.unwrap() else {
            panic!("unobserved file operation incorrectly reused another instance")
        };
        assert_eq!(need.demands(), &[demand.clone()]);
        drop(transaction);
        dice.wait_for_idle().await;
        let attempts = scope.snapshot().source_attempts;
        merge(
            epoch,
            observe_rooted(root, anchor, need.demands(), scope, &|| true).unwrap(),
        );
        assert_eq!(scope.snapshot().source_attempts, attempts + 1);
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        let mut updater = dice.updater();
        updater
            .changed_to([(PathObservationEpochKey, epoch.clone())])
            .unwrap();
        let mut transaction = updater.commit().await;
        let PathOutcome::Complete(value) = transaction.compute(&key).await.unwrap() else {
            panic!("native observation did not satisfy actual DICE Need")
        };
        assert!(Arc::ptr_eq(&value, epoch.get(&demand).unwrap()));
        values.push(value);
        drop(transaction);
        dice.wait_for_idle().await;
    }
    let PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) = values[0].as_ref()
    else {
        panic!("rooted byte result missing")
    };
    let PathObservationResult::FileDigest(PathOperationResult::Present(digest)) =
        values[1].as_ref()
    else {
        panic!("rooted digest result missing")
    };
    (bytes.clone(), *digest)
}

#[tokio::test]
async fn actual_dice_needs_resolve_rooted_symlinks_and_distinct_same_display_generations() {
    let fixture = tempfile::tempdir().unwrap();
    let display = fixture.path().join("display");
    let backing_a = fixture.path().join("backing-a");
    let backing_b = fixture.path().join("backing-b");
    populate(&display, &display, b"generation-a\0\xff");
    let descriptor_a = File::open(&display).unwrap();
    std::fs::rename(&display, &backing_a).unwrap();
    assert!(!display.exists());
    populate(&backing_b, &display, b"generation-b");
    let descriptor_b = File::open(&backing_b).unwrap();
    let root_a = RootIo::new(&descriptor_a, limits()).unwrap();
    let root_b = RootIo::new(&descriptor_b, limits()).unwrap();
    assert_ne!(
        descriptor_a.metadata().unwrap().ino(),
        descriptor_b.metadata().unwrap().ino()
    );
    let anchor_a = anchor(&display, 501);
    let anchor_b = anchor(&display, 502);
    let scope = scope();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut epoch = PathObservationEpoch::empty();
    let logical = display.join("alias/leaf");
    let resolved_a = resolve_native(&dice, &root_a, &anchor_a, &mut epoch, &scope, &logical)
        .await
        .unwrap();
    assert!(
        matches!(resolved_a.state(), ResolvedPathState::Present(stat) if stat.kind() == PathNodeKind::RegularFile)
    );
    assert_eq!(resolved_a.symlinks().len(), 2);
    assert_eq!(resolved_a.real_path().as_path(), display.join("body"));
    let root_demand = PathObservationDemand::new(
        anchor_a.namespace(),
        anchor_a.root().clone(),
        PathObservationOperation::Lstat,
    );
    assert!(
        matches!(epoch.get(&root_demand).unwrap().as_ref(), PathObservationResult::Lstat(PathOperationResult::Present(stat)) if stat.node_id() == descriptor_a.metadata().unwrap().ino() as i64)
    );
    let (bytes_a, digest_a) =
        read_resolved(&dice, &root_a, &anchor_a, &resolved_a, &mut epoch, &scope).await;
    std::fs::create_dir_all(display.join("alias")).unwrap();
    std::fs::write(display.join("body"), b"foreign-host").unwrap();
    std::fs::write(display.join("alias/leaf"), b"foreign-host").unwrap();
    let resolved_b = resolve_native(&dice, &root_b, &anchor_b, &mut epoch, &scope, &logical)
        .await
        .unwrap();
    assert_eq!(resolved_b.symlinks().len(), 2);
    let (bytes_b, digest_b) =
        read_resolved(&dice, &root_b, &anchor_b, &resolved_b, &mut epoch, &scope).await;
    assert_eq!(bytes_a.as_ref(), b"generation-a\0\xff");
    assert_eq!(bytes_b.as_ref(), b"generation-b");
    assert_eq!(
        digest_a.sha256(),
        &<[u8; 32]>::from(Sha256::digest(bytes_a.as_ref()))
    );
    assert_eq!(
        digest_b.sha256(),
        &<[u8; 32]>::from(Sha256::digest(bytes_b.as_ref()))
    );
    assert_eq!(digest_a.size_bytes(), bytes_a.len() as u64);
    assert_eq!(digest_b.size_bytes(), bytes_b.len() as u64);
    assert_ne!(digest_a, digest_b);
    assert_eq!(
        scope.snapshot().decoded_bytes,
        2 * (bytes_a.len() + bytes_b.len()) as u64
    );
    assert_eq!(scope.snapshot().source_attempts, 4);
    // B's complete same-spelling epoch cannot answer A's root request.
    epoch = PathObservationEpoch::from_shared(
        epoch
            .observations()
            .iter()
            .filter(|(demand, _)| demand.namespace() == anchor_b.namespace())
            .map(|(demand, result)| (demand.clone(), result.clone())),
    )
    .unwrap();
    let mut updater = dice.updater();
    updater
        .changed_to([(PathObservationEpochKey, epoch)])
        .unwrap();
    let mut transaction = updater.commit().await;
    let PathOutcome::Need(need) = transaction
        .compute(&AnchoredResolvedPathKey::new(anchor_a.clone(), &logical).unwrap())
        .await
        .unwrap()
    else {
        panic!("foreign generation satisfied A after its epoch was removed")
    };
    assert_eq!(need.demands(), &[root_demand]);
    drop(transaction);
    dice.wait_for_idle().await;
    scope.cancel();
    assert!(matches!(
        observe_rooted(&root_a, &anchor_a, &[], &scope, &|| true),
        Err(RootedObservationError::SourceService(
            SourceServiceError::Cancelled
        ))
    ));
    assert_eq!(scope.snapshot().source_attempts, 4);
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
}

#[tokio::test]
async fn actual_dice_external_literal_link_refuses_before_any_host_fallback() {
    let fixture = tempfile::tempdir().unwrap();
    let display = fixture.path().join("display");
    let backing = fixture.path().join("backing");
    let outside = fixture.path().join("outside");
    std::fs::write(&outside, b"host-secret").unwrap();
    std::fs::create_dir(&backing).unwrap();
    symlink(&outside, backing.join("link")).unwrap();
    let descriptor = File::open(&backing).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let anchor = anchor(&display, 503);
    let scope = scope();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut epoch = PathObservationEpoch::empty();
    let logical = display.join("link");
    assert!(matches!(
        resolve_native(&dice, &root, &anchor, &mut epoch, &scope, &logical).await,
        Err(AnchoredPathResolutionError::EscapesAnchor { .. })
    ));
    assert_eq!(scope.snapshot().source_attempts, 0);
    assert_eq!(scope.snapshot().decoded_bytes, 0);
    assert_eq!(epoch.observations().len(), 3);
    assert!(
        epoch
            .observations()
            .keys()
            .all(|demand| demand.namespace() == anchor.namespace()
                && demand.path().as_path().starts_with(&display))
    );
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
}
