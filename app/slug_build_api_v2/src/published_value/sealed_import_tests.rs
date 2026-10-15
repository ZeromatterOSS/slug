//! Sealed facts and producer work have independent caps; complete closures keep both.
use std::any::Any;
use std::sync::Arc;

use super::*;
use crate::AnalysisValue;

#[derive(Debug)]
struct Originals {
    roots: Box<[AnalysisValue]>,
    imports: Vec<PublishedValueArena>,
}
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        self.roots.len()
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
}
fn originals(b: &mut PublishedValueArenaBuilder, count: usize) -> Arc<Originals> {
    assert!(count <= NODE_LIMIT);
    // Forecast the owned binding, Arc counters, Vec/Box transfer peak and import guards
    // before allocating either array. All entries are real retained original values.
    b.charge_bytes(
        std::mem::size_of::<Originals>()
            + 2 * std::mem::size_of::<usize>()
            + 2 * count * std::mem::size_of::<AnalysisValue>()
            + 2 * b.imports.len() * std::mem::size_of::<PublishedValueArena>(),
    )
    .unwrap();
    Arc::new(Originals {
        roots: vec![AnalysisValue::none(); count].into_boxed_slice(),
        imports: b.imports.clone(),
    })
}
pub(in crate::published_value::builder) fn leaf(facts: usize) -> PublishedValueArena {
    assert!((4..=NODE_LIMIT + 3).contains(&facts));
    let mut b = PublishedValueArenaBuilder::new();
    let id = b.reserve().unwrap();
    let atom = b.native_atom(AnalysisValue::none()).unwrap();
    b.set_node(id, PublishedNode::Native(atom)).unwrap();
    b.set_original_slot(id, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    let originals = originals(&mut b, facts - 3);
    let arena = b.seal(originals).unwrap();
    assert_eq!(arena.0.local_shape.visits, facts);
    assert_eq!(arena.shape().visits, facts);
    arena
}
pub(in crate::published_value::builder) fn finish(
    mut b: PublishedValueArenaBuilder,
) -> Result<PublishedValueArena, PublishedValueError> {
    let roots = b.rows.len();
    let originals = originals(&mut b, roots);
    b.seal(originals)
}
fn branch(prior: &PublishedValueArena) -> PublishedValueArena {
    let mut b = PublishedValueArenaBuilder::new();
    let imported = b.import(prior.clone()).unwrap();
    let edges = b
        .append_edges(&[PublishedEdge::Imported {
            arena: imported,
            node: PublishedNodeId::new(0).unwrap(),
        }])
        .unwrap();
    let id = b.reserve().unwrap();
    b.set_node(id, PublishedNode::Tuple(edges)).unwrap();
    b.set_original_slot(id, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    // This binding really owns the original tuple, not an invented root count.
    let mut originals = originals(&mut b, 1);
    Arc::get_mut(&mut originals).unwrap().roots[0] =
        AnalysisValue::tuple(vec![AnalysisValue::none()]);
    b.seal(originals).unwrap()
}
fn pad(b: &mut PublishedValueArenaBuilder, remaining: usize) {
    while b.charged_visits() < NODE_LIMIT - remaining {
        b.charge_visit().unwrap();
    }
}

#[test]
fn sealed_diamond_admits_each_body_once_and_preserves_aliases_and_lifetime() {
    let prior = leaf(1027);
    let weak = Arc::downgrade(&prior.0);
    let a = branch(&prior);
    let b = branch(&prior);
    assert!(
        a.value(PublishedNodeId::new(0).unwrap())
            .unwrap()
            .publication_eq(&b.value(PublishedNodeId::new(0).unwrap()).unwrap())
    );
    let expected = prior.0.local_shape.visits + a.0.local_shape.visits + b.0.local_shape.visits;
    let mut builder = PublishedValueArenaBuilder::new();
    let a_id = builder.import(a.clone()).unwrap();
    let b_id = builder.import(b.clone()).unwrap();
    assert_ne!(a_id, b_id);
    assert_eq!(builder.import(a.clone()).unwrap(), a_id);
    assert_eq!(builder.import_visits, expected);
    assert_eq!(builder.closure_seen.len(), 3);
    assert!(
        builder.charged_visits() < 16,
        "only actual DAG/registration work"
    );
    let arena = finish(builder).unwrap();
    assert_eq!(
        arena.shape().visits,
        expected + 2,
        "two stored import references"
    );
    assert!(Arc::ptr_eq(
        &arena.imports()[0].imports()[0].0,
        &arena.imports()[1].imports()[0].0
    ));
    drop((prior, a, b));
    assert!(weak.upgrade().is_some());
    drop(arena);
    assert!(weak.upgrade().is_none());
}

#[test]
fn sealed_import_work_and_fact_limits_are_independent_and_body_failure_commits_no_facts() {
    let prior = leaf(1027);
    for remaining in [2, 1] {
        let mut b = PublishedValueArenaBuilder::new();
        pad(&mut b, remaining);
        let result = b.import(prior.clone());
        assert_eq!(result.is_ok(), remaining == 2);
        assert_eq!(b.charged_visits(), NODE_LIMIT);
        assert_eq!(
            b.import_visits, 1027,
            "body success precedes registration failure"
        );
        if remaining == 1 {
            assert!(b.imports.is_empty());
            assert!(b.charge_visit().is_err());
        }
    }
    for extra in [0, 1] {
        let other = leaf(NODE_LIMIT + extra);
        let mut b = PublishedValueArenaBuilder::new();
        b.import(leaf(NODE_LIMIT)).unwrap();
        let before_bytes = b.charged_bytes();
        let before_active = b.arena_active.len();
        let result = b.import(other);
        assert_eq!(result.is_ok(), extra == 0);
        if extra == 0 {
            assert_eq!(b.import_visits, RETAINED_FACT_LIMIT);
        } else {
            assert!(matches!(
                result,
                Err(PublishedValueError::Limit("import visits"))
            ));
            assert_eq!(b.import_visits, NODE_LIMIT);
            assert_eq!(
                b.charged_bytes(),
                before_bytes,
                "I refusal precedes body/key growth"
            );
            assert_eq!(b.arena_active.len(), before_active);
            assert_eq!(b.imports.len(), 1);
            assert!(b.charge_bytes(0).is_err());
        }
    }
    let mut b = PublishedValueArenaBuilder::new();
    // The temporary active-key forecast fits, but the immutable body does not.
    let key_bytes = 3 * std::mem::size_of::<PublishedArenaIdentity>();
    b.charge_bytes(BYTE_LIMIT - key_bytes).unwrap();
    assert!(b.import(prior).is_err());
    assert_eq!(b.import_visits, 0);
    assert!(b.imports.is_empty());
    assert!(b.closure_seen.is_empty());
    assert!(b.charge_bytes(0).is_err());
}

#[derive(Debug, allocative::Allocative)]
struct Leaf;
impl ImmutablePublicationInput for Leaf {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        PublicationShape {
            nodes: 0,
            edges: 0,
            bytes: 2 * std::mem::size_of::<usize>(),
            visits: 1,
            depth: 0,
        }
    }
}
#[derive(Debug, allocative::Allocative)]
struct Part {
    id: u8,
    imports: Vec<PublishedValueArena>,
    shared: Vec<SharedPublicationInput>,
}
impl StructuralFunctionOwnerPart for Part {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        // These fixture input sets are closed by id; no two equal ids have differing inputs.
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|p| p.id == self.id)
    }
    fn hash_part(&self, h: &mut dyn std::hash::Hasher) {
        h.write_u8(self.id);
    }
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.imports.capacity() * std::mem::size_of::<PublishedValueArena>()
            + self.shared.capacity() * std::mem::size_of::<SharedPublicationInput>()
    }
    fn retained_shape(&self) -> PublicationShape {
        PublicationShape {
            nodes: 0,
            edges: self.imports.len() + self.shared.len(),
            bytes: self.retained_bytes(),
            visits: 1,
            depth: 0,
        }
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &self.shared
    }
}
fn part(
    id: u8,
    imports: Vec<PublishedValueArena>,
    shared: Vec<SharedPublicationInput>,
) -> Arc<Part> {
    Arc::new(Part {
        id,
        imports,
        shared,
    })
}

#[test]
fn seal_caps_full_import_owner_shared_and_local_facts_independently_of_work() {
    for extra in [0, 1] {
        let prior = pair(RETAINED_FACT_LIMIT - 9 + extra);
        let owner = FunctionEvaluationOwner::new(
            FunctionOwnerDomain::ConfiguredRule,
            part(1, vec![], vec![]),
            part(2, vec![], vec![SharedPublicationInput::new(Arc::new(Leaf))]),
        )
        .unwrap();
        let mut b = PublishedValueArenaBuilder::new();
        b.import(prior).unwrap();
        b.audit_owner_shape(&owner).unwrap();
        assert_eq!(b.owner_payload.visits, 2);
        assert_eq!(b.shared_payload.visits, 1);
        let id = b.reserve().unwrap();
        let atom = b.native_atom(AnalysisValue::none()).unwrap();
        b.set_node(id, PublishedNode::Native(atom)).unwrap();
        b.set_original_slot(id, PublishedRootSlot::new(0).unwrap())
            .unwrap();
        assert!(b.charged_visits() < 32);
        let result = finish(b);
        if extra == 0 {
            assert_eq!(result.unwrap().shape().visits, RETAINED_FACT_LIMIT);
        } else {
            assert!(matches!(
                result,
                Err(PublishedValueError::RetainedFactBudget(ref report))
                    if report.checked_total_facts() == Some(RETAINED_FACT_LIMIT + 1)
                        && report.limit_facts() == RETAINED_FACT_LIMIT
            ));
        }
    }
}

#[test]
fn owner_constructor_checks_complete_work_plus_import_facts_before_retaining_parts() {
    for extra in [0, 1] {
        // Constructor policy counts original D2 +one root reference R1;
        // all three real arena bodies remain complete I. Work stays separate.
        let prior = pair(RETAINED_FACT_LIMIT - 3 + extra);
        let prior_weak = Arc::downgrade(&prior.0);
        let a = part(3, vec![], vec![]);
        let b = part(4, vec![prior], vec![]);
        let before = (Arc::strong_count(&a), Arc::strong_count(&b));
        let parts_weak = (Arc::downgrade(&a), Arc::downgrade(&b));
        let result =
            FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredRule, a.clone(), b.clone());
        if extra == 0 {
            assert_eq!(
                result.as_ref().unwrap().retained_shape().visits,
                RETAINED_FACT_LIMIT
            );
            assert_eq!(Arc::strong_count(&a), before.0 + 1);
        } else {
            let Err(PublishedValueError::OwnerRetainedVisitBudget(report)) = result else {
                panic!("expected final owner refusal, got {result:?}");
            };
            assert_eq!(
                (
                    report.domain(),
                    report.stable_key_visits(),
                    report.input_validity_visits(),
                    report.intrinsic_visits(),
                    report.legacy_local_visits(),
                    report.owner_visits(),
                    report.imported_visits(),
                    report.shared_visits(),
                    report.certified_shared_visits(),
                    report.producer_work(),
                    report.complete_visits(),
                    report.limit_visits()
                ),
                (
                    FunctionOwnerDomain::ConfiguredRule,
                    1,
                    1,
                    2,
                    27,
                    0,
                    RETAINED_FACT_LIMIT - 3 + 1,
                    0,
                    0,
                    Some(27),
                    RETAINED_FACT_LIMIT + 1,
                    RETAINED_FACT_LIMIT
                )
            );
            assert_eq!(report.declaration_reference_visits(), 1);
            assert_eq!((Arc::strong_count(&a), Arc::strong_count(&b)), before);
            drop((a, b));
            let cloned = report.clone();
            assert_eq!(report, cloned);
            assert!(parts_weak.0.upgrade().is_none() && parts_weak.1.upgrade().is_none());
            assert!(
                prior_weak.upgrade().is_none(),
                "numeric report retains no imported arena"
            );
        }
    }
}

// Compose genuine sealed leaves; two actual stored import references are part
// of the requested certificate. No shape/count is overwritten.
fn pair(facts: usize) -> PublishedValueArena {
    let mut b = PublishedValueArenaBuilder::new();
    b.import(leaf(NODE_LIMIT - 1)).unwrap();
    b.import(leaf(facts - (NODE_LIMIT - 1) - 2)).unwrap();
    let arena = finish(b).unwrap();
    assert_eq!(arena.shape().visits, facts);
    arena
}
#[path = "sealed_import_tests/retained_limit_tests.rs"]
mod retained_limit_tests;
