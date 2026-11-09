//! Allocation/cursor mechanism tests only; a returned lease never proves FD
//! contents or native acquisition authority. Snapshot integration supplies that.
use super::super::super::native_tests::reservation;
use super::super::super::tests::resources;
use super::super::super::tests::scope;
use super::*;

const BANK_BYTES: usize = 8 * 1024 * 1024;
fn ledger(bank: &Arc<RepositoryRootResources>) -> Arc<NativeBodyLedger> {
    NativeBodyLedger::new(
        bank,
        &scope(),
        &reservation(std::path::Path::new("/body-recovery"), 191),
        &|| true,
    )
    .unwrap()
}
fn retain(
    candidates: &mut NativeBodyCandidates,
    byte: u8,
    length: usize,
) -> RepositoryWorkspaceBody {
    let body: RepositoryWorkspaceBody = vec![byte; length].into();
    drop(candidates.retain(&body, &|| true).unwrap());
    body
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
fn refuse(
    candidates: &mut NativeBodyCandidates,
    bank: &Arc<RepositoryRootResources>,
    length: usize,
    spare: usize,
) -> (NativeBodyRefusal, ResourceCredit) {
    // Pay map growth before filling the bank: only the final payload debit may
    // issue this token. The retained sentinel is an actual same-bank credit.
    candidates.storage.ensure(1).unwrap();
    let held = bank
        .reserve(BANK_BYTES - bank.snapshot().retained_bytes - spare, 0, 0)
        .unwrap();
    let NativeBodyAdmission::Refused(token) = candidates
        .reserve_new_for_capture(length, &|| true)
        .unwrap()
    else {
        panic!("expected actual payload-budget refusal")
    };
    (token, held)
}

#[test]
fn recovery_selects_pending_then_committed_with_exact_length_and_full_refunds() {
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let ledger = ledger(&bank);
    let mut committed = ledger.candidates().unwrap();
    let old = retain(&mut committed, 1, 4096);
    let old_weak = RepositoryWorkspaceBody::downgrade(&old);
    commit(&ledger, &mut committed, &[old.clone()]);
    drop(committed);
    let mut pending = ledger.candidates().unwrap();
    let _wrong_length = retain(&mut pending, 2, 7);
    let new = retain(&mut pending, 3, 4096);
    let new_weak = RepositoryWorkspaceBody::downgrade(&new);
    let (token, held) = refuse(&mut pending, &bank, 4096, control_bytes().unwrap());
    let unlocked = || ledger.storage.try_lock().is_ok();
    let mut cursor = pending.recovery(token, &unlocked).unwrap().unwrap();
    let first = cursor.next(&unlocked).unwrap().unwrap();
    let second = cursor.next(&unlocked).unwrap().unwrap();
    assert!(RepositoryWorkspaceBody::ptr_eq(first.body(), &new));
    assert!(RepositoryWorkspaceBody::ptr_eq(second.body(), &old));
    assert!(cursor.next(&|| true).unwrap().is_none());
    assert_eq!(cursor.rows, 3);
    assert_eq!(cursor.comparisons, 2);
    assert_eq!(cursor.compared_bytes, 8192);
    assert!(pending.retain_claimed(&new, &first, &|| true).is_ok());
    assert!(pending.retain_claimed(&old, &second, &|| true).is_ok());
    drop((first, second));
    drop(cursor);
    drop((new, old, _wrong_length, pending, ledger, held));
    assert!(new_weak.upgrade().is_none() && old_weak.upgrade().is_none());
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    assert_eq!(bank.snapshot().native_fds, 0);
    assert_eq!(bank.snapshot().backing_bytes, 0);
}

#[test]
fn recovery_total_row_limit_and_fixed_endpoints_ignore_later_appends() {
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    {
        let ledger = ledger(&bank);
        let mut existing = ledger.candidates().unwrap();
        let matching = retain(&mut existing, 9, 4096);
        commit(&ledger, &mut existing, &[matching.clone()]);
        let mut pending = ledger.candidates().unwrap();
        // All pending rows count, even though none has the requested length.
        for byte in 0..128 {
            drop(retain(&mut pending, byte, 1));
        }
        let (token, held) = refuse(&mut pending, &bank, 4096, control_bytes().unwrap());
        let mut cursor = pending.recovery(token, &|| true).unwrap().unwrap();
        assert!(cursor.next(&|| true).unwrap().is_none());
        assert_eq!(cursor.rows, MAX_ROWS);
        assert_eq!(cursor.comparisons, 0);
        drop((cursor, held));
    }
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    {
        let ledger = ledger(&bank);
        let mut pending = ledger.candidates().unwrap();
        let first = retain(&mut pending, 1, 4096);
        let (token, held) = refuse(&mut pending, &bank, 4096, control_bytes().unwrap());
        let mut cursor = pending.recovery(token, &|| true).unwrap().unwrap();
        drop(held);
        let mut later = ledger.candidates().unwrap();
        let appended = retain(&mut later, 2, 4096);
        commit(&ledger, &mut later, &[appended]);
        assert!(RepositoryWorkspaceBody::ptr_eq(
            cursor.next(&|| true).unwrap().unwrap().body(),
            &first
        ));
        assert!(
            cursor.next(&|| true).unwrap().is_none(),
            "committed endpoint captured before append"
        );
        assert_eq!(cursor.rows, 1);
    }
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}

#[test]
fn recovery_comparison_and_byte_budgets_are_debited_before_returned_lease() {
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let ledger = ledger(&bank);
    let mut pending = ledger.candidates().unwrap();
    for byte in 0..6 {
        drop(retain(&mut pending, byte, 4096));
    }
    let (token, held) = refuse(&mut pending, &bank, 4096, control_bytes().unwrap());
    let mut cursor = pending.recovery(token, &|| true).unwrap().unwrap();
    for index in 1..=MAX_COMPARISONS {
        assert!(cursor.next(&|| true).unwrap().is_some());
        assert_eq!(cursor.comparisons, index);
        assert_eq!(cursor.compared_bytes, index * 4096);
    }
    assert!(cursor.next(&|| true).unwrap().is_none());
    assert_eq!(cursor.rows, MAX_COMPARISONS);
    // Scalar near-boundary fixture, not a claim of performing 512MiB IO.
    cursor.comparisons = 0;
    cursor.compared_bytes = MAX_BYTES - 4096;
    assert!(cursor.next(&|| true).unwrap().is_some());
    assert_eq!(cursor.compared_bytes, MAX_BYTES);
    assert!(cursor.next(&|| true).unwrap().is_none());
    assert_eq!(
        cursor.rows, 5,
        "unused sixth row cannot escape the byte budget"
    );
    cursor.compared_bytes = MAX_BYTES - 4095;
    assert!(cursor.next(&|| true).unwrap().is_none());
    assert_eq!(
        cursor.rows, 5,
        "one-byte-over forecast refuses before selecting another lease"
    );
    drop(cursor);
    drop((held, pending, ledger));
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}

#[test]
fn recovery_refuses_foreign_origin_inactive_scope_and_missing_control_room() {
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let origin_scope = scope();
    let selected = reservation(std::path::Path::new("/body-recovery"), 191);
    let ledger = NativeBodyLedger::new(&bank, &origin_scope, &selected, &|| true).unwrap();
    let mut pending = ledger.candidates().unwrap();
    drop(retain(&mut pending, 1, 4096));
    let foreign = NativeBodyLedger::new(&bank, &scope(), &selected, &|| true).unwrap();
    let foreign_candidates = foreign.candidates().unwrap();
    let (token, held) = refuse(&mut pending, &bank, 4096, control_bytes().unwrap());
    assert!(matches!(
        foreign_candidates.recovery(token, &|| true),
        Err(PhysicalRootError::Reservation)
    ));
    drop(held);
    let alien_bank = resources();
    let alien_baseline = alien_bank.snapshot().retained_bytes;
    let alien = self::ledger(&alien_bank);
    let alien_candidates = alien.candidates().unwrap();
    let (token, held) = refuse(&mut pending, &bank, 4096, control_bytes().unwrap());
    assert!(matches!(
        alien_candidates.recovery(token, &|| true),
        Err(PhysicalRootError::Reservation)
    ));
    drop((held, alien_candidates, alien));
    assert_eq!(alien_bank.snapshot().retained_bytes, alien_baseline);
    let (token, held) = refuse(&mut pending, &bank, 4096, control_bytes().unwrap() - 1);
    assert!(pending.recovery(token, &|| true).unwrap().is_none());
    drop(held);
    let (token, held) = refuse(&mut pending, &bank, 4096, control_bytes().unwrap());
    assert!(matches!(
        pending.recovery(token, &|| false),
        Err(PhysicalRootError::Inactive)
    ));
    drop(held);
    let (token, held) = refuse(&mut pending, &bank, 4096, control_bytes().unwrap());
    let mut cursor = pending.recovery(token, &|| true).unwrap().unwrap();
    let lease = cursor.next(&|| true).unwrap().unwrap();
    let live = bank.snapshot().retained_bytes;
    origin_scope.cancel();
    assert!(matches!(
        cursor.next(&|| true),
        Err(PhysicalRootError::SourceService(
            SourceServiceError::Cancelled
        ))
    ));
    assert_eq!(cursor.comparisons, 1);
    assert_eq!(
        bank.snapshot().retained_bytes,
        live,
        "Source error cannot retire live lease/cursor coverage"
    );
    drop(lease);
    drop(cursor);
    drop((held, pending, ledger, foreign_candidates, foreign));
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}

#[test]
fn recovery_only_follows_resource_limit_not_map_layout_or_arithmetic_refusals() {
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    assert!(
        bank.try_reserve_body_bytes(BANK_BYTES + 1)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        bank.try_reserve_body_bytes(usize::MAX),
        Err(PhysicalRootError::Resource)
    ));
    let ledger = ledger(&bank);
    let mut candidates = ledger.candidates().unwrap();
    assert!(matches!(
        candidates.reserve_new_for_capture(usize::MAX, &|| true),
        Err(PhysicalRootError::Resource)
    ));
    let mut empty = ledger.candidates().unwrap();
    let held = bank
        .reserve(BANK_BYTES - bank.snapshot().retained_bytes, 0, 0)
        .unwrap();
    assert!(
        matches!(
            empty.reserve_new_for_capture(4096, &|| true),
            Err(PhysicalRootError::Resource)
        ),
        "map control refusal must not create a recovery token"
    );
    drop((held, empty));
    let (token, held) = refuse(
        &mut candidates,
        &bank,
        MAX_BYTES + 1,
        control_bytes().unwrap(),
    );
    assert!(candidates.recovery(token, &|| true).unwrap().is_none());
    drop(held);
    let (token, held) = refuse(&mut candidates, &bank, 4096, control_bytes().unwrap());
    assert!(
        candidates.recovery(token, &|| true).unwrap().is_none(),
        "empty endpoints contribute no candidates"
    );
    drop((held, candidates, ledger));
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}
