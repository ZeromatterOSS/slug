use std::sync::Arc;

use dice::DetectCycles;
use dice::Dice;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationInputKey;
use slug_workspace_v2::PathObservationKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::PathOutcome;

use super::*;

fn demand(name: &str) -> PathObservationDemand {
    PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(format!("/workspace/{name}")).unwrap(),
        PathObservationOperation::FileBytes,
    )
}

fn epoch(demand: PathObservationDemand, bytes: &[u8]) -> PathObservationEpoch {
    PathObservationEpoch::new([(
        demand,
        PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(bytes))),
    )])
    .unwrap()
}

#[tokio::test]
async fn discarded_selected_updater_keeps_union_for_abort_revocation_and_restoration() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let original = demand("original");
    let transient = demand("transient");
    let never_committed = demand("staged-but-discarded");
    let prior = epoch(original.clone(), b"old");
    let ledger = NativePathInputLedger::new(&prior);
    let mut initial = dice.updater();
    ledger.inject(&mut initial, &prior).unwrap();
    drop(initial.commit().await);

    let provisional = epoch(transient.clone(), b"temporary");
    let mut attempt = dice.updater();
    ledger.inject(&mut attempt, &provisional).unwrap();
    drop(attempt.commit().await);

    // Selected staging may be dropped by source/version retry before commit.
    let mut abandoned = dice.updater();
    ledger
        .inject(&mut abandoned, &epoch(never_committed.clone(), b"unused"))
        .unwrap();
    drop(abandoned);
    assert_eq!(ledger.staged.lock().unwrap().len(), 3);

    let mut restore = dice.updater();
    ledger.inject(&mut restore, &prior).unwrap();
    let mut restored = restore.commit().await;
    assert!(matches!(
        restored
            .compute(&PathObservationKey::new(original.clone()))
            .await
            .unwrap(),
        PathOutcome::Complete(result) if result == *prior.get(&original).unwrap()
    ));
    for absent in [transient, never_committed] {
        assert!(
            restored
                .compute(&PathObservationInputKey::new(absent.clone()))
                .await
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            restored
                .compute(&PathObservationKey::new(absent))
                .await
                .unwrap(),
            PathOutcome::Need(_)
        ));
    }
    assert!(restored.compute(&PathObservationEpochKey).await.is_err());
}

#[tokio::test]
async fn selected_subset_revokes_surplus_and_successor_does_not_resurrect_it() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let selected = demand("selected");
    let surplus = demand("surplus");
    let full = PathObservationEpoch::from_shared([
        (
            selected.clone(),
            epoch(selected.clone(), b"a")
                .get(&selected)
                .unwrap()
                .clone(),
        ),
        (
            surplus.clone(),
            epoch(surplus.clone(), b"b").get(&surplus).unwrap().clone(),
        ),
    ])
    .unwrap();
    let ledger = NativePathInputLedger::new(&PathObservationEpoch::empty());
    let mut attempt = dice.updater();
    ledger.inject(&mut attempt, &full).unwrap();
    drop(attempt.commit().await);
    let retained = epoch(selected.clone(), b"a");
    let mut acceptance = dice.updater();
    ledger.inject(&mut acceptance, &retained).unwrap();
    drop(acceptance.commit().await);
    drop(ledger);

    let successor = NativePathInputLedger::new(&retained);
    let mut next = dice.updater();
    successor.inject(&mut next, &retained).unwrap();
    let mut next = next.commit().await;
    assert!(matches!(
        next.compute(&PathObservationKey::new(selected))
            .await
            .unwrap(),
        PathOutcome::Complete(_)
    ));
    assert!(matches!(
        next.compute(&PathObservationKey::new(surplus))
            .await
            .unwrap(),
        PathOutcome::Need(_)
    ));
}
