//! Real declaration storage and positive completion, never padded main witnesses.
use super::*;
use crate::published_value::builder::owner_accounting::owner_completion::SETUP_WORK;
use crate::published_value::builder::owner_accounting::owner_completion::completion_read_work;
use crate::published_value::builder::owner_accounting::owner_completion::completion_scratch_bytes;

#[derive(Debug, Allocative)]
struct Opaque {
    tag: u8,
    owners: Vec<FunctionEvaluationOwner>,
    arenas: Vec<PublishedValueArena>,
    shared: Vec<SharedPublicationInput>,
}
impl StructuralFunctionOwnerPart for Opaque {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().downcast_ref::<Self>().is_some_and(|p| {
            self.tag == p.tag
                && self.owners == p.owners
                && self
                    .arenas
                    .iter()
                    .map(PublishedValueArena::identity)
                    .eq(p.arenas.iter().map(PublishedValueArena::identity))
                && self
                    .shared
                    .iter()
                    .map(SharedPublicationInput::storage_identity)
                    .eq(p
                        .shared
                        .iter()
                        .map(SharedPublicationInput::storage_identity))
        })
    }
    fn hash_part(&self, h: &mut dyn Hasher) {
        h.write_u8(self.tag);
    }
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.owners.capacity() * std::mem::size_of::<FunctionEvaluationOwner>()
            + self.arenas.capacity() * std::mem::size_of::<PublishedValueArena>()
            + self.shared.capacity() * std::mem::size_of::<SharedPublicationInput>()
    }
    // Deliberately use the legitimate default opaque visits1: R must cover cells.
    fn retained_owners(&self) -> &[FunctionEvaluationOwner] {
        &self.owners
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.arenas
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &self.shared
    }
}
fn part(
    tag: u8,
    owners: Vec<FunctionEvaluationOwner>,
    arenas: Vec<PublishedValueArena>,
    shared: Vec<SharedPublicationInput>,
) -> Arc<Opaque> {
    Arc::new(Opaque {
        tag,
        owners,
        arenas,
        shared,
    })
}
fn empty(tag: u8) -> Arc<Opaque> {
    part(tag, vec![], vec![], vec![])
}
fn owner(tag: u8) -> FunctionEvaluationOwner {
    FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredRule, empty(tag), empty(tag))
        .unwrap()
}
#[derive(Debug)]
struct Bindings {
    imports: Vec<PublishedValueArena>,
}
impl PublishedOriginalBinding for Bindings {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn root_count(&self) -> usize {
        0
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
}
fn finish_builder(b: PublishedValueArenaBuilder) -> PublishedValueArena {
    let originals = Arc::new(Bindings {
        imports: b.imports().to_vec(),
    });
    b.seal(originals).unwrap()
}
fn arena(edges: usize) -> PublishedValueArena {
    let mut b = PublishedValueArenaBuilder::new();
    let id = b.reserve().unwrap();
    b.set_node(id, PublishedNode::Cell { contents: None })
        .unwrap();
    b.append_edges(&vec![PublishedEdge::Local(id); edges])
        .unwrap();
    let arena = finish_builder(b);
    assert_eq!(arena.0.local_shape.visits, edges + 2);
    arena
}
fn raw_audit(
    a: &dyn StructuralFunctionOwnerPart,
    b: &dyn StructuralFunctionOwnerPart,
) -> PublishedValueArenaBuilder {
    let mut audit = PublishedValueArenaBuilder::new_strict_shared_audit();
    // Complete part bodies are real. OwnerData's extra byte-only header cannot
    // affect this independent old work/visits oracle; no OwnerData is allocated.
    for p in [a, b] {
        let s = p.retained_shape();
        audit
            .charge_storage(crate::DepsetStorageCharge {
                nodes: s.nodes,
                edges: s.edges,
                bytes: s.bytes,
                visits: s.visits,
            })
            .unwrap();
        audit.observe_depth(s.depth).unwrap();
    }
    for p in [a, b] {
        for input in p.shared_inputs() {
            audit.audit_shared_input(input, 1).unwrap();
        }
        for owner in p.retained_owners() {
            audit.audit_owner_shape_at_depth(owner, 1).unwrap();
        }
        for arena in p.imported_arenas() {
            audit.audit_arena_closure(arena, 1).unwrap();
        }
    }
    audit
}

#[test]
fn genuine_alias_storage_old_work_total_refuses_and_positive_constructor_retains_all_facts() {
    let prior = arena(3 * NODE_LIMIT / 2);
    let prior_weak = Arc::downgrade(&prior.0);
    let child = owner(1);
    let child_guard = child.clone();
    let a = part(2, vec![child; NODE_LIMIT / 3], vec![], vec![]);
    let b = part(3, vec![], vec![prior], vec![]);
    assert_eq!(
        (a.retained_shape().visits, b.retained_shape().visits),
        (1, 1)
    );
    let mut raw = raw_audit(&*a, &*b);
    assert_eq!(
        (
            raw.charged_visits(),
            raw.visits,
            raw.import_visits,
            raw.owner_payload.visits
        ),
        (43693, 43695, 98306, 2)
    );
    assert_eq!(raw.complete_shape().unwrap().visits, 142001);
    assert!(raw.complete_shape().unwrap().visits > RETAINED_FACT_LIMIT);
    assert!(raw.import_visits + raw.owner_payload.visits < RETAINED_FACT_LIMIT);
    let before = raw.charged_visits();
    let added = SETUP_WORK
        + completion_read_work(raw.owner_seen.capacity(), raw.shared_seen.capacity(), 1, 0)
            .unwrap();
    let completion = raw.complete_owner_shape(&*a, &*b, 2).unwrap();
    assert_eq!(completion.declaration_references, NODE_LIMIT / 3 + 1);
    assert_eq!(completion.shape.visits, 120156);
    assert_eq!(raw.charged_visits(), before + added);
    assert_eq!(raw.certified_shared_visits, 0);
    let weak = (Arc::downgrade(&a), Arc::downgrade(&b));
    drop(raw);
    let actual = FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredRule, a, b).unwrap();
    assert_eq!(actual.intrinsic_shape().visits, 2);
    assert_eq!(actual.retained_shape().visits, 120156);
    assert!(actual.retained_shape().bytes < BYTE_LIMIT);
    let mut consumer = PublishedValueArenaBuilder::new();
    consumer.audit_owner_shape(&actual).unwrap();
    assert_eq!(consumer.import_visits, 98306);
    drop(actual);
    assert!(weak.0.upgrade().is_some() && prior_weak.upgrade().is_some());
    drop(consumer);
    assert!(weak.0.upgrade().is_none() && weak.1.upgrade().is_none());
    assert!(prior_weak.upgrade().is_none());
    assert_eq!(child_guard.intrinsic_shape().visits, 2);
    drop(child_guard);
}

#[path = "owner_completion_tests/boundaries.rs"]
mod boundaries;
#[path = "owner_completion_tests/coverage.rs"]
mod coverage;

#[path = "owner_completion_meter_tests.rs"]
mod meter_tests;
