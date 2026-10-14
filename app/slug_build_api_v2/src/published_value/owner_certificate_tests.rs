//! Private immutable owner facts preserve legacy shapes but do not replay work.
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Weak;

use super::*;
use crate::AnalysisValue;
use crate::published_value::builder::import_accounting::tests::finish;
use crate::published_value::builder::import_accounting::tests::leaf;

#[derive(Debug, allocative::Allocative)]
struct Fields {
    words: Box<[u64]>,
    shape: PublicationShape,
}
impl Fields {
    fn new(count: usize) -> Arc<Self> {
        assert!(count < NODE_LIMIT);
        let words: Box<[u64]> = (0..count as u64).collect();
        let shape = PublicationShape {
            bytes: std::mem::size_of::<Self>() + words.len() * std::mem::size_of::<u64>(),
            visits: words.len() + 1, // complete header plus every immutable field
            ..PublicationShape::default()
        };
        Arc::new(Self { words, shape })
    }
}
impl StructuralFunctionOwnerPart for Fields {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|p| self.words == p.words)
    }
    fn hash_part(&self, mut state: &mut dyn Hasher) {
        self.words.hash(&mut state);
    }
    fn retained_bytes(&self) -> usize {
        self.shape.bytes
    }
    fn retained_shape(&self) -> PublicationShape {
        self.shape
    }
}
fn owner(count: usize) -> (FunctionEvaluationOwner, Weak<Fields>) {
    let stable = Fields::new(count);
    let weak = Arc::downgrade(&stable);
    let owner = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::ConfiguredRule,
        stable,
        Fields::new(count),
    )
    .unwrap();
    assert_eq!(owner.intrinsic_shape().visits, 2 * (count + 1));
    (owner, weak)
}
fn pad(b: &mut PublishedValueArenaBuilder, remaining: usize) {
    while b.charged_visits() < NODE_LIMIT - remaining {
        b.charge_visit().unwrap();
    }
}
fn hash(owner: &FunctionEvaluationOwner) -> u64 {
    let mut h = fxhash::FxHasher::default();
    owner.hash_fields(&mut h);
    h.finish()
}

#[test]
fn immutable_owner_facts_keep_full_shapes_eq_hash_and_actual_guard_deduplication() {
    let (a, weak) = owner(2);
    let (equal, _) = owner(2);
    assert_eq!(a, equal);
    assert_eq!(hash(&a), hash(&equal));
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_owner_shape(&a).unwrap();
    assert_eq!(
        (b.charged_visits(), b.visits, b.owner_payload.visits),
        (2, 8, 6)
    );
    assert_eq!(b.local_shape().visits, 8);
    assert_eq!(b.intrinsic_shape().visits, 2);
    assert_eq!(b.complete_shape().unwrap().visits, 8);
    let before = b.charged_visits();
    let atom = b.native_atom(AnalysisValue::none()).unwrap();
    assert_eq!(
        atom.retained_visits(),
        1,
        "raw native delta excludes prior owner count"
    );
    assert_eq!(b.charged_visits() - before, 1);
    b.audit_owner_shape(&a.clone()).unwrap();
    assert_eq!(b.owner_payload.visits, 6);
    b.audit_owner_shape(&equal).unwrap();
    assert_eq!(
        b.owner_payload.visits, 12,
        "equal separately allocated owners stay separate"
    );
    assert_eq!(b.owners().len(), 2);
    assert_eq!(b.visits, b.charged_visits() + 12);
    let sealed = finish(b).unwrap();
    assert_eq!(
        sealed.0.local_shape.visits, 2,
        "two retained owning-root references"
    );
    assert_eq!(sealed.shape().visits, 14);
    drop(a);
    assert!(weak.upgrade().is_some());
    drop(sealed);
    assert!(weak.upgrade().is_none());
}

#[test]
fn joint_owner_import_facts_are_capped_before_body_growth_in_both_orders() {
    let (owner, _) = owner(2); // six actual immutable owner fields/header facts
    for owner_first in [false, true] {
        for extra in [0, 1] {
            let prior = leaf(NODE_LIMIT - 5 + extra);
            let mut b = PublishedValueArenaBuilder::new();
            b.import(leaf(NODE_LIMIT - 1)).unwrap();
            if owner_first {
                b.audit_owner_shape(&owner).unwrap();
            } else {
                b.import(prior.clone()).unwrap();
            }
            let before_bytes = b.charged_bytes();
            let before_o = b.owner_payload.visits;
            let before_i = b.import_visits;
            let result = if owner_first {
                b.import(prior).map(|_| ())
            } else {
                b.audit_owner_shape(&owner)
            };
            assert_eq!(result.is_ok(), extra == 0);
            if extra == 0 {
                assert_eq!(
                    b.owner_payload.visits + b.import_visits,
                    RETAINED_FACT_LIMIT
                );
                assert_eq!(b.charged_visits(), 6);
                assert_eq!(b.complete_shape().unwrap().visits, RETAINED_FACT_LIMIT + 6);
            } else {
                assert_eq!(
                    (b.owner_payload.visits, b.import_visits),
                    (before_o, before_i)
                );
                assert_eq!(
                    b.charged_bytes(),
                    before_bytes,
                    "joint refusal precedes active/body storage"
                );
                assert!(b.owner_active.is_empty());
                assert!(b.arena_active.is_empty());
                assert_eq!(b.owners().len(), usize::from(owner_first));
                assert_eq!(b.imports().len(), 1 + usize::from(!owner_first));
                assert!(b.charge_bytes(0).is_err());
            }
        }
    }
}

#[test]
fn raw_combined_visits_above_work_cap_seal_with_same_loan_and_full_canonical_cap() {
    let (middle, _) = owner(14_999); // C=30000, genuine immutable field storage
    for (remaining, succeeds) in [(2, true), (1, false)] {
        let mut b = PublishedValueArenaBuilder::new();
        b.audit_owner_shape(&middle).unwrap();
        pad(&mut b, remaining);
        assert!(b.visits > NODE_LIMIT);
        assert_eq!(b.visits - b.owner_payload.visits, NODE_LIMIT - remaining);
        let result = finish(b);
        assert_eq!(
            result.is_ok(),
            succeeds,
            "Meter uses actual W, not combined V"
        );
        if let Ok(arena) = result {
            assert_eq!(arena.0.local_shape.visits, 1);
            assert_eq!(arena.shape().visits, 30_001);
        }
    }
    let (full, _) = owner(32_767); // constructor complete exactly65536
    assert_eq!(full.retained_shape().visits, NODE_LIMIT);
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_owner_shape(&full).unwrap();
    assert_eq!(b.charged_visits(), 2);
    assert_eq!(b.owner_payload.visits, NODE_LIMIT);
    let arena = finish(b).unwrap();
    assert_eq!(arena.shape().visits, NODE_LIMIT + 1);
    let mut imported = PublishedValueArenaBuilder::new();
    imported.import(arena).unwrap();
    assert_eq!(imported.owner_payload.visits, NODE_LIMIT);
    assert_eq!(imported.import_visits, 1);
    assert_eq!(finish(imported).unwrap().shape().visits, NODE_LIMIT + 2);
    // Constructor cap remains paid even though publication can reuse its private proof.
    assert!(
        FunctionEvaluationOwner::new(
            FunctionOwnerDomain::ConfiguredRule,
            Fields::new(32_767),
            Fields::new(32_768)
        )
        .is_err()
    );
}

#[test]
fn owner_lookup_depth_and_body_bytes_still_refuse_and_poison_without_new_guards() {
    let (owner, weak) = owner(2);
    let mut work = PublishedValueArenaBuilder::new();
    work.audit_owner_shape(&owner).unwrap();
    pad(&mut work, 0);
    assert!(work.audit_owner_shape(&owner).is_err());
    assert_eq!(work.owner_payload.visits, 6);
    assert_eq!(work.owner_seen.len(), 1);
    assert!(work.charge_bytes(0).is_err());
    let mut depth = PublishedValueArenaBuilder::new();
    depth
        .audit_owner_shape_at_depth(&owner, DEPTH_LIMIT)
        .unwrap();
    assert!(
        depth
            .audit_owner_shape_at_depth(&owner, DEPTH_LIMIT + 1)
            .is_err()
    );
    assert_eq!(depth.owner_seen.len(), 1);
    let mut bytes = PublishedValueArenaBuilder::new();
    bytes
        .charge_bytes(BYTE_LIMIT - 3 * std::mem::size_of::<usize>())
        .unwrap();
    assert!(bytes.audit_owner_shape(&owner).is_err());
    assert_eq!(bytes.owner_payload.visits, 0);
    assert_eq!(bytes.visits, 1, "failed body does not commit cached V");
    assert!(bytes.owner_seen.is_empty());
    assert!(bytes.owners().is_empty());
    assert!(bytes.charge_visit().is_err());
    drop((owner, work, depth));
    assert!(
        weak.upgrade().is_none(),
        "failed byte admission retained no owner guard"
    );
}
