//! Ordinary lifetime/accounting seams; these fixtures mint no native root or IO
//! authority. The reservation helper uses genuine metadata only.
use std::path::Path;

use super::super::native_tests::reservation;
use super::super::tests::resources;
use super::super::tests::scope;
use super::*;

fn make_ledger(bank: &Arc<RepositoryRootResources>) -> Arc<NativeBodyLedger> {
    NativeBodyLedger::new(
        bank,
        &scope(),
        &reservation(Path::new("/body-test"), 19),
        &|| true,
    )
    .unwrap()
}
fn commit(
    ledger: &NativeBodyLedger,
    candidates: &mut NativeBodyCandidates,
    bodies: &[RepositoryWorkspaceBody],
) {
    ledger
        .commit_canonical(
            candidates,
            |visit| {
                for body in bodies {
                    visit(body)?;
                }
                Ok(())
            },
            &|| true,
        )
        .unwrap();
}

#[test]
fn canonical_reuse_foreign_coexistence_and_filtered_last_owner() {
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let ledger = make_ledger(&bank);
    let mut candidates = ledger.candidates().unwrap();
    let a: RepositoryWorkspaceBody =
        RepositoryWorkspaceBody::from_arc(Arc::from(&b"private-body-marker"[..]));
    let equal_foreign: RepositoryWorkspaceBody =
        RepositoryWorkspaceBody::from_arc(Arc::from(a.raw_arc().unwrap().as_ref()));
    assert!(!RepositoryWorkspaceBody::ptr_eq(&a, &equal_foreign));
    let wa = RepositoryWorkspaceBody::downgrade(&a);
    let wf = RepositoryWorkspaceBody::downgrade(&equal_foreign);
    let la = candidates.retain(&a, &|| true).unwrap();
    let admitted = bank.snapshot().retained_bytes;
    let reuse = candidates.retain(&a, &|| true).unwrap();
    assert_eq!(bank.snapshot().retained_bytes, admitted);
    assert!(RepositoryWorkspaceBody::ptr_eq(la.body(), reuse.body()));
    let foreign = candidates.retain(&equal_foreign, &|| true).unwrap();
    assert!(bank.snapshot().retained_bytes >= admitted + body_bytes(equal_foreign.len()).unwrap());
    commit(
        &ledger,
        &mut candidates,
        &[a.clone(), equal_foreign.clone()],
    );
    let set = ledger.final_set(|visit| visit(&a), &|| true).unwrap();
    assert!(set.lookup(&a, &|| true).unwrap().is_some());
    assert!(set.lookup(&equal_foreign, &|| true).unwrap().is_none());
    for debug in [
        format!("{ledger:?}"),
        format!("{candidates:?}"),
        format!("{set:?}"),
        format!("{la:?}"),
    ] {
        assert!(!debug.contains("private-body-marker"));
        assert!(!debug.contains("0x"));
    }
    drop((a, equal_foreign, candidates, ledger, la, foreign, reuse));
    assert!(wa.upgrade().is_some());
    assert!(
        wf.upgrade().is_none(),
        "filtered data must not retain history"
    );
    drop(set);
    assert!(wa.upgrade().is_none());
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    assert_eq!(bank.snapshot().native_fds, 0);
    assert_eq!(bank.snapshot().backing_bytes, 0);
}

#[test]
fn commit_failure_preserves_candidates_and_only_canonical_survivors_enter_ledger() {
    let bank = resources();
    let ledger = make_ledger(&bank);
    let mut candidates = ledger.candidates().unwrap();
    let used: RepositoryWorkspaceBody = RepositoryWorkspaceBody::from_arc(Arc::from(&b"used"[..]));
    let unused: RepositoryWorkspaceBody =
        RepositoryWorkspaceBody::from_arc(Arc::from(&b"unused"[..]));
    drop(candidates.retain(&used, &|| true).unwrap());
    drop(candidates.retain(&unused, &|| true).unwrap());
    let missing: RepositoryWorkspaceBody =
        RepositoryWorkspaceBody::from_arc(Arc::from(&b"unknown"[..]));
    assert!(
        ledger
            .commit_canonical(
                &mut candidates,
                |visit| {
                    visit(&used)?;
                    visit(&missing)
                },
                &|| true
            )
            .is_err()
    );
    assert!(
        ledger.lookup(&used, &|| true).unwrap().is_none(),
        "failed visitor must not partially append"
    );
    assert!(candidates.lookup(&used, &|| true).unwrap().is_some());
    let mut incoming = ledger.candidates().unwrap();
    let transferred: RepositoryWorkspaceBody =
        RepositoryWorkspaceBody::from_arc(Arc::from(&b"transferred"[..]));
    let transfer = incoming.retain(&transferred, &|| true).unwrap();
    candidates.absorb(&mut incoming, &|| true).unwrap();
    assert!(std::ptr::eq(
        candidates
            .lookup(&transferred, &|| true)
            .unwrap()
            .unwrap()
            .0
            .get(),
        transfer.0.get()
    ));
    drop((incoming, transfer));
    commit(&ledger, &mut candidates, &[used.clone(), transferred]);
    assert!(ledger.lookup(&used, &|| true).unwrap().is_some());
    assert!(ledger.lookup(&unused, &|| true).unwrap().is_none());
    let w = RepositoryWorkspaceBody::downgrade(&unused);
    drop(unused);
    assert!(w.upgrade().is_some());
    drop(candidates);
    assert!(w.upgrade().is_none());
}

#[test]
fn foreign_witness_origin_and_cancellation_refuse_without_fallback() {
    let bank = resources();
    let origin_scope = scope();
    let reservation = reservation(Path::new("/body-test"), 19);
    let ledger = NativeBodyLedger::new(&bank, &origin_scope, &reservation, &|| true).unwrap();
    let mut candidates = ledger.candidates().unwrap();
    let body: RepositoryWorkspaceBody = RepositoryWorkspaceBody::from_arc(Arc::from(&b"same"[..]));
    let witness = candidates.retain(&body, &|| true).unwrap();
    let alien_bank = resources();
    let alien_ledger = make_ledger(&alien_bank);
    let mut alien = alien_ledger.candidates().unwrap();
    let alien_witness = alien.retain(&body, &|| true).unwrap();
    let other = make_ledger(&bank);
    let mut other_candidates = other.candidates().unwrap();
    let other_witness = other_candidates.retain(&body, &|| true).unwrap();
    let before = bank.snapshot().retained_bytes;
    assert!(
        candidates
            .retain_claimed(&body, &other_witness, &|| true)
            .is_err()
    );
    assert!(candidates.absorb(&mut other_candidates, &|| true).is_err());
    assert!(
        candidates
            .retain_claimed(&body, &alien_witness, &|| true)
            .is_err()
    );
    let equal: RepositoryWorkspaceBody =
        RepositoryWorkspaceBody::from_arc(Arc::from(body.raw_arc().unwrap().as_ref()));
    assert!(
        candidates
            .retain_claimed(&equal, &witness, &|| true)
            .is_err()
    );
    assert!(
        ledger
            .check_origin(&alien_bank, &origin_scope, &reservation, &|| true)
            .is_err()
    );
    assert!(
        ledger
            .check_origin(&bank, &scope(), &reservation, &|| true)
            .is_err()
    );
    assert!(candidates.reserve_new(1, &|| false).is_err());
    assert_eq!(bank.snapshot().retained_bytes, before);
    origin_scope.cancel();
    assert!(candidates.retain(&body, &|| true).is_err());
    assert_eq!(bank.snapshot().retained_bytes, before);
}

#[test]
fn fresh_reservation_empty_body_and_failed_finish_retire_exactly() {
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let ledger = make_ledger(&bank);
    let mut candidates = ledger.candidates().unwrap();
    let pending = candidates.reserve_new(0, &|| true).unwrap();
    assert!(pending.credit.charged_bytes() > 0);
    let body: RepositoryWorkspaceBody = RepositoryWorkspaceBody::from_arc(Arc::from([]));
    let weak = RepositoryWorkspaceBody::downgrade(&body);
    let lease = candidates.finish(pending, body, &|| true).unwrap();
    assert!(lease.body().is_empty());
    let pending = candidates.reserve_new(5, &|| true).unwrap();
    let wrong: RepositoryWorkspaceBody = RepositoryWorkspaceBody::from_arc(Arc::from(&b"bad"[..]));
    let ww = RepositoryWorkspaceBody::downgrade(&wrong);
    assert!(candidates.finish(pending, wrong, &|| true).is_err());
    assert!(ww.upgrade().is_none());
    drop((lease, candidates, ledger));
    assert!(weak.upgrade().is_none());
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}

#[test]
fn map_growth_is_prepaid_across_index_threshold_and_bank_refuses_before_growth() {
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let mut map = BodyMap::new(&bank).unwrap();
    for n in 0..130usize {
        let body: RepositoryWorkspaceBody =
            RepositoryWorkspaceBody::from_arc(Arc::from(n.to_ne_bytes()));
        let lease = NativeBodyLease(
            bank.reserve_body_bytes(body_bytes(body.len()).unwrap())
                .unwrap()
                .freeze(body)
                .unwrap(),
        );
        map.insert(lease).unwrap();
        assert!(
            map.bodies.retained_storage_bytes_upper_bound().unwrap()
                <= map_bound(map.bodies.capacity()).unwrap()
        );
        assert!(
            map.credit.charged_bytes() >= map.bodies.retained_storage_bytes_upper_bound().unwrap()
        );
    }
    assert!(map.bodies.capacity() >= 130);
    drop(map);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    // Keep exactly all remaining bank capacity in another positive reservation.
    let mut refused = BodyMap::new(&bank).unwrap();
    let intrinsic = refused.credit.charged_bytes();
    let hold = bank
        .reserve_body_bytes(8 * 1024 * 1024 - baseline - intrinsic)
        .unwrap();
    let peak = bank.snapshot().peak_retained_bytes;
    assert!(refused.ensure(1).is_err());
    assert_eq!(refused.bodies.capacity(), 0);
    assert_eq!(refused.credit.charged_bytes(), intrinsic);
    assert_eq!(bank.snapshot().peak_retained_bytes, peak);
    drop((refused, hold));
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}

#[test]
fn interleaved_commit_preserves_first_published_allocation_witness() {
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let ledger = make_ledger(&bank);
    let mut a = ledger.candidates().unwrap();
    let mut b = ledger.candidates().unwrap();
    let body: RepositoryWorkspaceBody = RepositoryWorkspaceBody::from_arc(Arc::from(
        &b"one allocation, two admitted temporary owners"[..],
    ));
    let weak = RepositoryWorkspaceBody::downgrade(&body);
    let lease_a = a.retain(&body, &|| true).unwrap();
    let lease_b = b.retain(&body, &|| true).unwrap();
    assert!(!std::ptr::eq(lease_a.0.get(), lease_b.0.get()));
    assert!(ledger.lookup(&body, &|| true).unwrap().is_none());
    ledger
        .commit_canonical(
            &mut a,
            |visit| {
                // A has selected its temporary witness, but has not published it.
                visit(&body)?;
                // A second complete canonical publisher wins before A acquires the
                // append lock. This is deterministic callback interleaving, not a
                // fabricated ledger row or a native authority claim.
                ledger.commit_canonical(&mut b, |visit| visit(&body), &|| true)
            },
            &|| true,
        )
        .unwrap();
    let published = ledger.lookup(&body, &|| true).unwrap().unwrap();
    assert!(std::ptr::eq(published.0.get(), lease_b.0.get()));
    let mut fresh = ledger.candidates().unwrap();
    let claimed = fresh.retain_claimed(&body, &lease_b, &|| true).unwrap();
    assert!(
        std::ptr::eq(claimed.0.get(), lease_b.0.get()),
        "later publisher must not invalidate an issued canonical witness"
    );
    drop((
        body, a, b, fresh, ledger, lease_a, lease_b, published, claimed,
    ));
    assert!(weak.upgrade().is_none());
    let final_usage = bank.snapshot();
    assert_eq!(
        (
            final_usage.retained_bytes,
            final_usage.native_fds,
            final_usage.backing_bytes
        ),
        (baseline, 0, 0)
    );
}

#[test]
fn packed_body_controls_and_block_allocations_remain_covered_until_last_data_owner() {
    // Allocation-coverage utility evidence only; this constructs no native root.
    let bank = resources();
    let scope = scope();
    let reservation = reservation(Path::new("/packed-body-test"), 1901);
    let baseline = bank.snapshot().retained_bytes;
    let ledger = NativeBodyLedger::new(&bank, &scope, &reservation, &|| true).unwrap();
    let mut candidates = ledger.candidates().unwrap();
    let length = 3 * 64 * 1024 + 7;
    let mut pending = candidates.reserve_packed(length, &|| true).unwrap();
    let scratch = bank
        .reserve(
            RepositoryWorkspaceBodyBuilder::encoding_scratch_bytes(),
            0,
            0,
        )
        .unwrap();
    let mut builder = RepositoryWorkspaceBodyBuilder::new(length).unwrap();
    let block = [0x6d; 64 * 1024];
    for bytes in [
        block.as_slice(),
        block.as_slice(),
        block.as_slice(),
        &block[..7],
    ] {
        builder
            .append(bytes, &mut |layout| {
                pending.admit_packed_arc(layout, &|| true)
            })
            .unwrap();
    }
    let body = builder.finish().unwrap();
    assert!(body.raw_arc().is_none());
    assert_eq!(body.len(), length);
    assert!(body.native_storage_bytes() < length);
    assert!(pending.credit.charged_bytes() >= body.native_storage_bytes());
    let weak = body.downgrade();
    let lease = candidates.finish(pending, body.clone(), &|| true).unwrap();
    drop(scratch);
    commit(&ledger, &mut candidates, &[body.clone()]);
    drop((body, candidates, ledger));
    assert!(
        weak.upgrade().is_some(),
        "data-only lease retains every packed block"
    );
    assert!(bank.snapshot().retained_bytes > baseline);
    drop(lease);
    assert!(weak.upgrade().is_none());
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}
