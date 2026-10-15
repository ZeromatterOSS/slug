//! Independently sum every declaration domain, including arena-only bodies.
use super::*;

#[derive(Debug, Allocative)]
struct Shared {
    children: Vec<SharedPublicationInput>,
    intrinsic: PublicationShape,
    full: PublicationShape,
}
impl ImmutablePublicationInput for Shared {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        self.full
    }
    fn intrinsic_shape(&self) -> PublicationShape {
        self.intrinsic
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &self.children
    }
}
fn shared(children: Vec<SharedPublicationInput>) -> SharedPublicationInput {
    let intrinsic = PublicationShape {
        bytes: std::mem::size_of::<Shared>()
            + 2 * std::mem::size_of::<usize>()
            + children.capacity() * std::mem::size_of::<SharedPublicationInput>(),
        edges: children.len(),
        visits: 1,
        ..PublicationShape::default()
    };
    let mut full = intrinsic;
    for child in &children {
        let shape = child.retained_shape();
        full.bytes += shape.bytes;
        full.nodes += shape.nodes;
        full.edges += shape.edges;
        full.visits += shape.visits;
        full.depth = full.depth.max(shape.depth + 1);
    }
    SharedPublicationInput::new(Arc::new(Shared {
        children,
        intrinsic,
        full,
    }))
}
#[test]
fn positive_references_cover_root_prior_owner_shared_children_and_arena_only_bodies() {
    let leaf = shared(vec![]);
    let foreign = shared(vec![]);
    assert_ne!(leaf.storage_identity(), foreign.storage_identity());
    assert_eq!(
        leaf.retained_shape().visits,
        foreign.retained_shape().visits
    );
    let left = shared(vec![leaf.clone(), leaf.clone()]);
    let right = shared(vec![leaf, foreign.clone()]);
    let child_a = part(11, vec![], vec![], vec![left.clone()]);
    let child_b = part(12, vec![], vec![], vec![right.clone()]);
    let child = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::BzlLoad,
        child_a.clone(),
        child_b.clone(),
    )
    .unwrap();
    let equal =
        FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, child_a, child_b).unwrap();
    assert_eq!(child, equal);
    assert_eq!(child.retained_shape().visits, 12); // D2+S4+R6
    let hidden_shared = shared(vec![shared(vec![])]);
    let hidden = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::ConfiguredRule,
        part(13, vec![], vec![], vec![hidden_shared]),
        empty(14),
    )
    .unwrap();
    assert_eq!(hidden.retained_shape().visits, 6); // D2+S2+R2
    let mut inner = PublishedValueArenaBuilder::new();
    inner.audit_owner_shape(&hidden).unwrap();
    let inner = finish_builder(inner);
    assert_eq!(inner.0.local_shape.visits, 1);
    let mut outer = PublishedValueArenaBuilder::new();
    outer.import(inner).unwrap();
    let outer = finish_builder(outer);
    assert_eq!(outer.0.local_shape.visits, 1);
    assert!(outer.sources().is_empty());
    let a = part(
        15,
        vec![child.clone(), child],
        vec![outer.clone()],
        vec![left],
    );
    let b = part(
        16,
        vec![equal],
        vec![outer.clone(), outer],
        vec![right, foreign],
    );
    let mut audit = raw_audit(&*a, &*b);
    assert_eq!(
        (
            audit.owner_seen.len(),
            audit.shared_seen.len(),
            audit.closure_seen.len()
        ),
        (3, 6, 2)
    );
    assert_eq!(
        (
            audit.owner_payload.visits,
            audit.shared_payload.visits,
            audit.import_visits
        ),
        (6, 6, 2)
    );
    // Root cells9 +three prior-owner part declarations5 +shared child cells5.
    // Arena local certificates already include their own stored cells (I2).
    let completion = audit.complete_owner_shape(&*a, &*b, 2).unwrap();
    assert_eq!(completion.declaration_references, 9 + 5 + 5);
    assert_eq!(completion.shape.visits, 2 + 6 + 6 + 2 + 19);
    assert_eq!(audit.certified_shared_visits, 0);
    drop(audit);
    let mut reversed = raw_audit(&*b, &*a);
    let reversed_completion = reversed.complete_owner_shape(&*b, &*a, 2).unwrap();
    assert_eq!(reversed_completion.declaration_references, 19);
    assert_eq!(reversed_completion.shape.visits, 35);
    drop(reversed);
    let actual = FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredAspect, a, b).unwrap();
    assert_eq!(actual.intrinsic_shape().visits, 2);
    assert_eq!(actual.retained_shape().visits, 35);
    // Reimport retains intrinsic D, not the child's complete cached T/R again.
    let mut consumer = PublishedValueArenaBuilder::new();
    consumer.audit_owner_shape(&actual).unwrap();
    assert_eq!(consumer.owner_payload.visits, 8);
    assert_eq!(consumer.shared_payload.visits, 6);
    assert_eq!(consumer.import_visits, 2);
    let resealed = finish_builder(consumer);
    assert_eq!(resealed.0.local_shape.visits, 2); // actual owner +import roots
    assert_eq!(resealed.shape().visits, 8 + 6 + 2 + 2);
    let next = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::ConfiguredRule,
        empty(17),
        part(18, vec![], vec![resealed], vec![]),
    )
    .unwrap();
    // D2 +O8 +S6 +I4 +R20: prior complete35 was NOT substituted for O.
    assert_eq!(next.intrinsic_shape().visits, 2);
    assert_eq!(next.retained_shape().visits, 40);
}
