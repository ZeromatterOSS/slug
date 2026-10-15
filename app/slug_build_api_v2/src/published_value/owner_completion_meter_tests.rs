//! Isolated meter/poison controls; genuine constructor witnesses live next door.
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use super::*;

#[derive(Debug, Allocative)]
struct ObservedLengths {
    #[allocative(skip)]
    reads: AtomicUsize,
}
impl ObservedLengths {
    fn new() -> Self {
        Self {
            reads: AtomicUsize::new(0),
        }
    }
    fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}
impl StructuralFunctionOwnerPart for ObservedLengths {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().is::<Self>()
    }
    fn hash_part(&self, h: &mut dyn Hasher) {
        h.write_u8(0);
    }
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        self.reads.fetch_add(1, Ordering::SeqCst);
        &[]
    }
    fn retained_owners(&self) -> &[FunctionEvaluationOwner] {
        self.reads.fetch_add(1, Ordering::SeqCst);
        &[]
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        self.reads.fetch_add(1, Ordering::SeqCst);
        &[]
    }
}
fn read_cost(b: &PublishedValueArenaBuilder) -> usize {
    SETUP_WORK
        + completion_read_work(
            b.owner_seen.capacity(),
            b.shared_seen.capacity(),
            b.owner_seen.len(),
            b.shared_seen.len(),
        )
        .unwrap()
}
fn fill_work(b: &mut PublishedValueArenaBuilder, target: usize) {
    let visits = target.checked_sub(b.charged_visits()).unwrap();
    b.charge_storage(crate::DepsetStorageCharge {
        visits,
        ..Default::default()
    })
    .unwrap();
}

#[test]
fn exact_completion_work_and_plus_one_refusal_precede_all_root_length_reads() {
    for with_owner in [false, true] {
        for extra in [0, 1] {
            let roots = ObservedLengths::new();
            let mut b = PublishedValueArenaBuilder::new_strict_shared_audit();
            if with_owner {
                b.audit_owner_shape(&owner(81)).unwrap();
                assert_eq!(b.owner_seen.len(), 1);
                assert!(
                    b.owner_seen.capacity() > b.owner_seen.len(),
                    "actual spare buckets must be charged"
                );
            }
            let added = read_cost(&b);
            if with_owner {
                assert!(added > SETUP_WORK + completion_read_work(1, 0, 1, 0).unwrap());
            } else {
                assert_eq!(added, 22);
            }
            fill_work(&mut b, NODE_LIMIT - added + extra);
            let before = b.visits;
            let result = b.complete_owner_shape(&roots, &roots, 0);
            assert_eq!(b.charged_visits(), NODE_LIMIT);
            if extra == 0 {
                let completed = result.unwrap();
                assert_eq!(completed.declaration_references, 0);
                assert_eq!(completed.shape.visits, if with_owner { 2 } else { 0 });
                assert_eq!(roots.reads(), 6, "both original root positions count");
                assert_eq!(b.visits, before + added);
                assert!(!b.failed_budget);
            } else {
                assert_eq!(result.err().unwrap(), PublishedValueError::Limit("visits"));
                assert_eq!(
                    roots.reads(),
                    0,
                    "full count cost admitted before accessors"
                );
                assert!(b.failed_budget);
                assert_eq!(b.charge_visit(), Err(PublishedValueError::Limit("visits")));
            }
            assert_eq!(b.certified_shared_visits, 0);
        }
    }
}

#[test]
fn completion_scratch_exact_bytes_and_plus_one_preserve_primitive_failure_and_snapshot() {
    for extra in [0, 1] {
        let roots = ObservedLengths::new();
        let mut b = PublishedValueArenaBuilder::new_strict_shared_audit();
        let scratch = completion_scratch_bytes();
        assert!(
            scratch >= std::mem::size_of::<PublicationShape>() + 16 * std::mem::size_of::<usize>()
        );
        b.charge_bytes(BYTE_LIMIT - scratch + extra).unwrap();
        let prior_bytes = b.bytes;
        let result = b.complete_owner_shape(&roots, &roots, 0);
        if extra == 0 {
            let completed = result.unwrap();
            assert_eq!(
                completed.shape.bytes, BYTE_LIMIT,
                "snapshot follows positive prepay"
            );
            assert_eq!(b.bytes, BYTE_LIMIT);
            assert_eq!(b.producer_scratch_bytes, scratch);
            assert_eq!(roots.reads(), 6);
        } else {
            let PublishedValueError::ByteBudget(report) = result.err().unwrap() else {
                panic!("expected primitive byte refusal")
            };
            assert_eq!(
                (
                    report.used_bytes(),
                    report.requested_bytes(),
                    report.checked_total_bytes()
                ),
                (prior_bytes, scratch, Some(BYTE_LIMIT + 1))
            );
            assert_eq!(report.producer_work(), Some(SETUP_WORK));
            assert_eq!(report.producer_scratch_bytes(), 0);
            assert!(!report.prior_poisoned());
            assert_eq!(b.bytes, prior_bytes);
            assert_eq!(b.producer_scratch_bytes, 0);
            assert_eq!(roots.reads(), 0);
            assert!(b.failed_budget);
        }
    }
}

#[test]
fn raw_active_draft_and_poisoned_states_cannot_complete_or_read_declarations() {
    for kind in 0..6 {
        let roots = ObservedLengths::new();
        let mut b = if kind == 0 {
            PublishedValueArenaBuilder::new()
        } else {
            PublishedValueArenaBuilder::new_strict_shared_audit()
        };
        match kind {
            1 => {
                b.reserve().unwrap();
            }
            2 => {
                b.owner_active.insert(7);
            }
            3 => {
                b.shared_active.insert(7);
            }
            4 => {
                b.charge_storage(crate::DepsetStorageCharge {
                    visits: NODE_LIMIT + 1,
                    ..Default::default()
                })
                .unwrap_err();
            }
            5 => {
                b.certified_shared_visits = 1;
                b.shared_payload.visits = 1;
                b.visits = 1;
            }
            _ => {}
        }
        let error = b.complete_owner_shape(&roots, &roots, 0).err().unwrap();
        assert_eq!(roots.reads(), 0);
        assert!(b.failed_budget);
        if kind == 4 {
            assert_eq!(error, PublishedValueError::Limit("visits"));
        } else {
            assert_eq!(
                error,
                PublishedValueError::Invalid("owner completion state")
            );
        }
    }
}

#[test]
fn completion_checked_totals_and_capacity_envelopes_fail_closed_without_credit() {
    assert_eq!(completion_read_work(usize::MAX, 0, 0, 0), None);
    assert_eq!(completion_read_work(0, usize::MAX, 0, 0), None);
    assert_eq!(completion_read_work(0, 0, usize::MAX, 0), None);
    assert_eq!(completion_read_work(0, 0, 0, usize::MAX), None);
    let roots = ObservedLengths::new();
    let mut b = PublishedValueArenaBuilder::new_strict_shared_audit();
    b.audit_owner_shape(&owner(82)).unwrap();
    let before = b.visits;
    let added = read_cost(&b);
    let error = b
        .complete_owner_shape(&roots, &roots, usize::MAX)
        .err()
        .unwrap();
    assert_eq!(error, PublishedValueError::Limit("owner complete visits"));
    assert_eq!(
        b.visits,
        before + added,
        "all old and new work remains debited"
    );
    assert_eq!(b.owner_payload.visits, 2);
    assert_eq!(b.owner_seen.len(), 1);
    assert!(b.failed_budget);
    assert_eq!(roots.reads(), 6);
}
