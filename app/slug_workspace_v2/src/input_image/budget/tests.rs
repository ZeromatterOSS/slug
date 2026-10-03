use std::sync::Barrier;

use super::*;

#[test]
fn reservation_highwater_records_retired_overlap_and_refuses_without_growth() {
    let budget = ImageDescriptorBudget::new(32);
    assert_eq!(budget.peak_reserved_bytes(), 0);
    let mut first = budget.try_reserve(8).unwrap();
    let second = budget.try_reserve(16).unwrap();
    first.try_grow(8).unwrap();
    assert_eq!(budget.used_bytes(), 32);
    assert_eq!(budget.peak_reserved_bytes(), 32);
    assert_eq!(
        first.try_grow(1),
        Err(ImageDescriptorBudgetError::ResourceLimit)
    );
    assert_eq!(
        first.try_grow(usize::MAX),
        Err(ImageDescriptorBudgetError::ChargeOverflow)
    );
    assert_eq!(first.charged_bytes(), 16);
    assert_eq!(budget.used_bytes(), 32);
    assert_eq!(budget.peak_reserved_bytes(), 32);
    drop(second);
    drop(first);
    let mut smaller = budget.try_reserve(3).unwrap();
    smaller.try_grow(0).unwrap();
    drop(budget.try_reserve(0).unwrap());
    assert_eq!(budget.used_bytes(), 3);
    assert_eq!(budget.peak_reserved_bytes(), 32);
    drop(smaller);
    assert_eq!(budget.used_bytes(), 0);
    let overflow = ImageDescriptorBudget::new(usize::MAX);
    // Credits only: no allocation of a usize::MAX payload is attempted.
    let full = overflow.try_reserve(usize::MAX).unwrap();
    assert!(matches!(
        overflow.try_reserve(1),
        Err(ImageDescriptorBudgetError::ChargeOverflow)
    ));
    assert_eq!(overflow.peak_reserved_bytes(), usize::MAX);
    drop(full);
    assert_eq!(overflow.used_bytes(), 0);
}

#[test]
fn concurrent_reservations_share_highwater_and_foreign_banks_stay_separate() {
    let budget = ImageDescriptorBudget::new(32);
    let clone = budget.dupe();
    let foreign = ImageDescriptorBudget::new(32);
    let ready = Barrier::new(3);
    let release = Barrier::new(3);
    let (used, peak) = std::thread::scope(|threads| {
        for bytes in [7, 11] {
            let bank = budget.dupe();
            let ready = &ready;
            let release = &release;
            threads.spawn(move || {
                let credit = bank.try_reserve(bytes);
                ready.wait();
                release.wait();
                drop(credit);
            });
        }
        ready.wait();
        let observed = (clone.used_bytes(), clone.peak_reserved_bytes());
        // Always release before assertions, including a broken admission case.
        release.wait();
        observed
    });
    assert_eq!((used, peak), (18, 18));
    assert_eq!(budget.used_bytes(), 0);
    assert_eq!(clone.peak_reserved_bytes(), 18);
    drop(foreign.try_reserve(5).unwrap());
    assert_eq!(foreign.peak_reserved_bytes(), 5);
    assert_eq!(budget.peak_reserved_bytes(), 18);
}

#[test]
fn freeze_headers_shared_clones_and_owner_drop_preserve_highwater() {
    struct Owner(ImageDescriptorBudget);
    impl Drop for Owner {
        fn drop(&mut self) {
            assert!(
                self.0.used_bytes() >= 64,
                "owner must precede credit retirement"
            );
        }
    }
    let header = std::mem::size_of::<BlockInner<Owner>>()
        + 2 * std::mem::size_of::<usize>()
        + std::mem::align_of::<BlockInner<Owner>>();
    let budget = ImageDescriptorBudget::new(64 + header);
    let block = budget
        .try_reserve(64)
        .unwrap()
        .freeze(Owner(budget.dupe()))
        .unwrap();
    let clone = block.dupe();
    assert_eq!(budget.used_bytes(), 64 + header);
    assert_eq!(budget.peak_reserved_bytes(), 64 + header);
    drop(block);
    assert_eq!(budget.used_bytes(), 64 + header);
    drop(clone);
    assert_eq!(budget.used_bytes(), 0);
    assert_eq!(budget.peak_reserved_bytes(), 64 + header);
    let small = ImageDescriptorBudget::new(64);
    let (error, owner, credit) = small
        .try_reserve(64)
        .unwrap()
        .freeze_preserving(Owner(small.dupe()))
        .err()
        .unwrap();
    assert_eq!(error, ImageDescriptorBudgetError::ResourceLimit);
    assert_eq!(small.peak_reserved_bytes(), 64);
    drop(owner);
    assert_eq!(small.used_bytes(), 64);
    drop(credit);
    assert_eq!(small.used_bytes(), 0);
    assert_eq!(small.peak_reserved_bytes(), 64);
}

#[tokio::test]
async fn normal_image_history_retires_credit_without_erasing_highwater() {
    use dice::DetectCycles;
    use dice::Dice;

    use super::super::tests::empty_epoch;
    use super::super::tests::path;
    use super::super::tests::user_data;
    use crate::InputImagePolicy;
    use crate::ObservedInputImageKey;
    use crate::PathObservationEpochKey;
    use crate::PathObservationNamespace;
    use crate::PathOutcome;

    let budget = ImageDescriptorBudget::new(1 << 20);
    let root = path("/image", &budget);
    let key = ObservedInputImageKey::from_charged(
        PathObservationNamespace::Host,
        root.dupe(),
        root,
        InputImagePolicy::plain(),
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater_with_data(user_data(&budget)).commit().await;
    // The normal key issues source observations; no completed image is injected.
    assert!(matches!(
        tx.compute(&key).await.unwrap(),
        PathOutcome::Need(_)
    ));
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            empty_epoch(&key, &budget, 1),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let PathOutcome::Complete(Ok(image)) = tx.compute(&key).await.unwrap() else {
        panic!("normal observations must produce a complete image");
    };
    assert!(image.same_budget(&budget));
    let retained = image.dupe();
    dice.wait_for_idle().await;
    let held = budget.used_bytes();
    let prior_peak = budget.peak_reserved_bytes();
    let overlap = budget.try_reserve(prior_peak).unwrap();
    let expected_peak = held + prior_peak;
    assert_eq!(budget.peak_reserved_bytes(), expected_peak);
    drop(overlap);
    // Instrumentation-only history does not change full certificate semantics.
    let PathOutcome::Complete(Ok(warm)) = tx.compute(&key).await.unwrap() else {
        panic!("same normal producer must remain available");
    };
    assert_eq!(warm, image);
    assert_eq!(warm.content(), image.content());
    drop(warm);
    drop(image);
    drop(tx);
    dice.wait_for_idle().await;
    assert!(budget.used_bytes() > 0);
    drop(retained);
    drop(key);
    // Real DICE key/result history remains an owner until engine retirement.
    assert!(budget.used_bytes() > 0);
    drop(dice);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while budget.used_bytes() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual engine history must retire within the bounded gate");
    assert_eq!(budget.peak_reserved_bytes(), expected_peak);
}
