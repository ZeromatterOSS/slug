//! Genuine consuming-seal witnesses. No completed facts or validity token are
//! fabricated; every successful arena comes from ordinary builder methods.
use super::*;

fn cells(count: usize) -> PublishedValueArenaBuilder {
    let mut b = PublishedValueArenaBuilder::new();
    for _ in 0..count {
        cell(&mut b);
    }
    b
}

#[test]
fn real_slot_array_crosses_old_work_limit_and_seals_with_every_full_fact() {
    let count = NODE_LIMIT / 3 + 1;
    assert_eq!(count, 21846);
    let mut probe = cells(count);
    assert_eq!(
        probe.charged_visits(),
        count,
        "actual reserve work only, no padding"
    );
    assert_eq!(
        (probe.rows.len(), probe.slots.len(), probe.edges.len()),
        (count, count, 0)
    );
    let mut old_work = probe.charged_visits();
    let mut raw_failed = false;
    let old = retained_facts::local_visits(
        &probe.rows,
        &probe.edges,
        &probe.slots,
        &probe.imports,
        0,
        0,
        &mut old_work,
        &mut 0,
        &mut 0,
        &mut raw_failed,
    );
    assert!(matches!(
        old,
        Err(PublishedValueError::Limit("retained certificate work"))
    ));
    assert_eq!(old_work, NODE_LIMIT);
    assert!(raw_failed);
    assert!(
        3 * count + 1 > NODE_LIMIT,
        "independent old setup+rows+slots oracle"
    );
    let originals = binding(&probe, 0);
    let mut work = probe.charged_visits();
    let (facts, sources) = probe.certify_local(originals.as_ref(), &mut work).unwrap();
    assert_eq!(
        work,
        2 * count + 11,
        "reserve R + view8 + source setup1 + rows R + backing1 + slots1"
    );
    assert_eq!(work, 43703);
    assert_eq!((facts.total, facts.source, facts.native), (2 * count, 0, 0));
    assert!(sources.is_empty());
    drop((probe, originals)); // Never seal an already certified test probe twice.

    let b = cells(count);
    let before_bytes = b.charged_bytes();
    let originals = binding(&b, 0);
    let originals_weak = Arc::downgrade(&originals);
    let arena = b.seal(originals).unwrap();
    assert_eq!(
        (
            arena.shape().nodes,
            arena.shape().edges,
            arena.shape().depth
        ),
        (count, 0, 0)
    );
    assert_eq!(arena.shape().visits, 43692);
    assert_eq!(arena.0.local_shape.visits, 2 * count);
    assert!(arena.shape().bytes > before_bytes);
    assert!(arena.shape().bytes <= BYTE_LIMIT);
    assert_eq!(arena.0.rows.len(), count);
    assert_eq!(arena.0.original_slots.len(), count);
    assert!(arena.0.original_slots.iter().all(Option::is_none));
    assert!(
        arena
            .0
            .rows
            .iter()
            .all(|r| matches!(r, PublishedNode::Cell { contents: None }))
    );
    assert!(arena.imports().is_empty() && arena.owners().is_empty() && arena.sources().is_empty());
    assert_eq!(arena.originals().root_count(), 0);
    assert!(originals_weak.upgrade().is_some());
    drop(arena);
    assert!(originals_weak.upgrade().is_none());
}

#[derive(Debug, allocative::Allocative)]
struct Guard(Arc<()>);
impl StructuralFunctionOwnerPart for Guard {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().is::<Self>()
    }
    fn hash_part(&self, state: &mut dyn std::hash::Hasher) {
        state.write_u8(1);
    }
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>()
    }
}

#[test]
fn final_slot_validator_refuses_first_last_and_missing_with_all_guards_released() {
    use crate::published_value::builder::retained_facts::tests::code;
    use crate::published_value::builder::retained_facts::tests::source;
    for bad_position in [0usize, 2, 3] {
        let mut b = cells(2);
        let authority = source(0);
        let source_weak = Arc::downgrade(&authority.data().root.repository_mapping);
        code(&mut b, authority); // Actual Code row, native source certificate stays complete.
        let prior = seal(builder(0));
        let import_weak = Arc::downgrade(&prior.0);
        b.import(prior.clone()).unwrap();
        let token = Arc::new(());
        let owner_weak = Arc::downgrade(&token);
        let owner = FunctionEvaluationOwner::new(
            FunctionOwnerDomain::ConfiguredRule,
            Arc::new(Guard(token.clone())),
            Arc::new(Guard(token)),
        )
        .unwrap();
        b.audit_owner_shape(&owner).unwrap();
        let expected = if bad_position == 3 {
            let id = b.reserve().unwrap();
            let range = b.append_edges(&[]).unwrap();
            b.set_node(id, PublishedNode::List(range)).unwrap();
            "missing original slot"
        } else {
            b.set_original_slot(
                PublishedNodeId::new(bad_position).unwrap(),
                PublishedRootSlot::new(1).unwrap(),
            )
            .unwrap();
            "original slot"
        };
        let originals = binding(&b, 1);
        let binding_weak = Arc::downgrade(&originals);
        drop((prior, owner));
        let result = b.seal(originals);
        assert!(
            matches!(result, Err(PublishedValueError::Invalid(message)) if message == expected)
        );
        assert!(
            source_weak.upgrade().is_none(),
            "private source facts cannot escape late failure"
        );
        assert!(import_weak.upgrade().is_none());
        assert!(owner_weak.upgrade().is_none());
        assert!(binding_weak.upgrade().is_none());
    }
}

#[test]
fn slots_keep_exact_original_projection_foreign_imports_and_failed_setter_state() {
    let mut left = cells(3);
    let first = PublishedNodeId::new(0).unwrap();
    let last = PublishedNodeId::new(2).unwrap();
    let slot = PublishedRootSlot::new(0).unwrap();
    left.set_original_slot(first, slot).unwrap();
    let before = (
        left.slots.clone(),
        left.charged_visits(),
        left.charged_bytes(),
    );
    assert!(matches!(
        left.set_original_slot(first, slot),
        Err(PublishedValueError::Invalid("duplicate original slot"))
    ));
    assert!(matches!(
        left.set_original_slot(PublishedNodeId::new(3).unwrap(), slot),
        Err(PublishedValueError::Invalid("slot node"))
    ));
    assert_eq!(
        (
            left.slots.clone(),
            left.charged_visits(),
            left.charged_bytes()
        ),
        before
    );
    left.set_original_slot(last, slot).unwrap();
    let originals = binding(&left, 1);
    let weak = Arc::downgrade(&originals);
    let arena = left.seal(originals).unwrap();
    assert_eq!(
        arena.shape().visits,
        7,
        "3 rows +3 slots +1 binding reference"
    );
    assert_eq!(arena.original_slot(first), Some(slot));
    assert_eq!(arena.original_slot(last), Some(slot));
    assert_eq!(arena.original_slot(PublishedNodeId::new(1).unwrap()), None);
    let mut same = cells(3);
    same.set_original_slot(first, slot).unwrap();
    same.set_original_slot(last, slot).unwrap();
    let originals = binding(&same, 1);
    let foreign = same.seal(originals).unwrap();
    assert!(!Arc::ptr_eq(&arena.0, &foreign.0));
    assert!(
        arena
            .value(first)
            .unwrap()
            .publication_eq(&foreign.value(first).unwrap())
    );
    assert_eq!(
        (
            arena.shape().nodes,
            arena.shape().edges,
            arena.shape().bytes,
            arena.shape().visits,
            arena.shape().depth
        ),
        (
            foreign.shape().nodes,
            foreign.shape().edges,
            foreign.shape().bytes,
            foreign.shape().visits,
            foreign.shape().depth
        )
    );
    let mut b = cells(1);
    b.import(arena.clone()).unwrap();
    // Same facts and shape never replace the ordered original imported guard.
    let wrong = Arc::new(Originals {
        roots: 0,
        imports: vec![foreign.clone()],
    });
    assert!(matches!(
        b.seal(wrong),
        Err(PublishedValueError::Invalid("original imports"))
    ));
    let mut b = cells(1);
    b.import(arena.clone()).unwrap();
    b.import(foreign.clone()).unwrap();
    let originals = binding(&b, 0);
    let retained = b.seal(originals).unwrap();
    assert_eq!(retained.imports().len(), 2);
    assert_eq!(
        retained.0.local_shape.visits, 4,
        "row +slot +both imported references"
    );
    assert_eq!(
        retained.shape().visits,
        18,
        "local4 +both full foreign bodies7+7"
    );
    assert!(Arc::ptr_eq(&retained.imports()[0].0, &arena.0));
    assert!(Arc::ptr_eq(&retained.imports()[1].0, &foreign.0));
    drop((arena, foreign));
    assert!(weak.upgrade().is_some());
    drop(retained);
    assert!(weak.upgrade().is_none());
}
