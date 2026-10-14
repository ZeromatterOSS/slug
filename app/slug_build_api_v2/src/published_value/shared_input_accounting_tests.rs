use super::*;

#[derive(Debug, Allocative)]
struct Fact {
    semantic_id: u8,
    shape: PublicationShape,
}
impl ImmutablePublicationInput for Fact {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        self.shape
    }
}
fn fact(id: u8) -> Arc<Fact> {
    Arc::new(Fact {
        semantic_id: id,
        shape: PublicationShape {
            nodes: 1,
            edges: 2,
            bytes: 128,
            visits: 100,
            depth: 3,
        },
    })
}
#[derive(Debug, Allocative)]
struct Inputs {
    semantic_ids: Vec<u8>,
    inputs: Vec<SharedPublicationInput>,
}
impl StructuralFunctionOwnerPart for Inputs {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|o| self.semantic_ids == o.semantic_ids)
    }
    fn hash_part(&self, h: &mut dyn std::hash::Hasher) {
        h.write(&self.semantic_ids);
    }
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.semantic_ids.len()
            + self.inputs.len() * std::mem::size_of::<SharedPublicationInput>()
            + 4 * std::mem::size_of::<usize>()
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &self.inputs
    }
}
fn owner(f: &Arc<Fact>) -> FunctionEvaluationOwner {
    let part = Arc::new(Inputs {
        semantic_ids: vec![f.semantic_id],
        inputs: vec![SharedPublicationInput::new(f.clone())],
    });
    let empty = Arc::new(Inputs {
        semantic_ids: vec![],
        inputs: vec![],
    });
    FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, empty, part).unwrap()
}
#[derive(Debug)]
struct Originals(Vec<PublishedValueArena>);
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        0
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.0
    }
}
fn seal(b: PublishedValueArenaBuilder) -> PublishedValueArena {
    let imports = b.imports().to_vec();
    b.seal(Arc::new(Originals(imports))).unwrap()
}
#[test]
fn shared_leaf_payload_is_counted_once_across_distinct_owners_in_both_orders() {
    let f = fact(1);
    let a = owner(&f);
    let b = owner(&f);
    assert_eq!(a, b);
    for (first, second) in [(&a, &b), (&b, &a)] {
        let mut audit = PublishedValueArenaBuilder::new();
        audit.audit_owner_shape(first).unwrap();
        audit.audit_owner_shape(second).unwrap();
        assert_eq!(audit.shared_seen.len(), 1);
        assert_eq!(audit.shared_payload.visits, 100);
        assert_eq!(audit.local_shape().nodes - audit.intrinsic_shape().nodes, 1);
        let separately_allocated = fact(1);
        let c = owner(&separately_allocated);
        assert_eq!(a, c);
        audit.audit_owner_shape(&c).unwrap();
        assert_eq!(audit.shared_seen.len(), 2);
        assert_eq!(audit.shared_payload.visits, 200);
    }
}
#[test]
fn shared_leaf_guards_and_imported_owner_roots_use_the_same_memo() {
    let f = fact(2);
    let weak = Arc::downgrade(&f);
    let a = owner(&f);
    let mut prior = PublishedValueArenaBuilder::new();
    prior.audit_owner_shape(&a).unwrap();
    let prior = seal(prior);
    let b = owner(&f);
    for import_first in [true, false] {
        let mut audit = PublishedValueArenaBuilder::new();
        if import_first {
            audit.import(prior.clone()).unwrap();
        }
        audit.audit_owner_shape(&b).unwrap();
        audit.import(prior.clone()).unwrap();
        assert_eq!(audit.shared_seen.len(), 1);
        assert_eq!(audit.shared_payload.visits, 100);
        assert_eq!(audit.shared_payload.bytes, 128);
    }
    let mut audit = PublishedValueArenaBuilder::new();
    audit.audit_owner_shape(&b).unwrap();
    drop(a);
    drop(b);
    drop(prior);
    drop(f);
    assert!(weak.upgrade().is_some());
    drop(audit);
    assert!(weak.upgrade().is_none());
}
#[test]
fn shared_leaf_quota_and_deeper_memo_hits_refuse_before_guard_growth() {
    let f = fact(3);
    let input = SharedPublicationInput::new(f.clone());
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_shared_input(&input, 0).unwrap();
    b.audit_shared_input(&input, DEPTH_LIMIT - f.shape.depth)
        .unwrap();
    assert_eq!(b.shared_seen.len(), 1);
    assert!(
        b.audit_shared_input(&input, DEPTH_LIMIT - f.shape.depth + 1)
            .is_err()
    );
    assert!(b.seal(Arc::new(Originals(vec![]))).is_err());
    let huge = SharedPublicationInput::new(Arc::new(Fact {
        semantic_id: 4,
        shape: PublicationShape {
            bytes: BYTE_LIMIT + 1,
            ..PublicationShape::default()
        },
    }));
    let mut b = PublishedValueArenaBuilder::new();
    assert!(b.audit_shared_input(&huge, 0).is_err());
    assert!(b.shared_seen.is_empty());
    assert!(b.audit_shared_input(&input, 0).is_err());
    assert!(b.shared_seen.is_empty());
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_shared_input(&input, 0).unwrap();
    while b.charged_visits() < NODE_LIMIT {
        b.charge_visit().unwrap();
    }
    assert!(b.audit_shared_input(&input, 0).is_err());
    assert_eq!(b.shared_seen.len(), 1); // A hit still consumes reference work.
}
#[test]
fn shared_leaf_constructor_preflight_keeps_full_isolated_closure_bounds() {
    let giant = fact(5);
    let giant = Arc::new(Fact {
        semantic_id: giant.semantic_id,
        shape: PublicationShape {
            visits: NODE_LIMIT,
            ..giant.shape
        },
    });
    let part = Arc::new(Inputs {
        semantic_ids: vec![5],
        inputs: vec![SharedPublicationInput::new(giant)],
    });
    let empty = Arc::new(Inputs {
        semantic_ids: vec![],
        inputs: vec![],
    });
    assert!(FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, empty, part).is_err());
}
