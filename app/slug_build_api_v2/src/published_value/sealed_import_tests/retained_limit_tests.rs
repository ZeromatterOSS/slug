//! Cross-generation retained counts use one bound; actual work stays independently capped.
use super::*;

#[test]
fn local_facts_above_work_limit_survive_import_reseal_owner_and_guard_release() {
    let original = leaf(NODE_LIMIT + 1);
    assert_eq!(original.originals().root_count(), NODE_LIMIT - 2);
    assert_eq!(original.0.local_shape.visits, NODE_LIMIT + 1);
    let weak = Arc::downgrade(&original.0);
    let mut consumer = PublishedValueArenaBuilder::new();
    let first = consumer.import(original.clone()).unwrap();
    assert_eq!(consumer.import(original.clone()).unwrap(), first);
    assert_eq!(consumer.import_visits, NODE_LIMIT + 1);
    assert_eq!(
        consumer.charged_visits(),
        4,
        "both incoming lookups remain paid"
    );
    let consumer = finish(consumer).unwrap();
    assert_eq!(consumer.shape().visits, NODE_LIMIT + 2);
    assert!(
        consumer.imports()[0]
            .value(PublishedNodeId::new(0).unwrap())
            .unwrap()
            .publication_eq(&original.value(PublishedNodeId::new(0).unwrap()).unwrap())
    );
    let owner = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::ConfiguredRule,
        part(21, vec![], vec![]),
        part(22, vec![consumer.clone()], vec![]),
    )
    .unwrap();
    assert_eq!(
        owner.retained_shape().visits,
        consumer.shape().visits + 4,
        "two part records plus two actual closure visits"
    );
    drop((original, consumer));
    assert!(weak.upgrade().is_some());
    drop(owner);
    assert!(weak.upgrade().is_none());
}

#[test]
fn full_fact_limit_seals_exactly_and_refuses_the_next_real_stored_reference() {
    for extra in [0, 1] {
        let mut b = PublishedValueArenaBuilder::new();
        b.import(leaf(NODE_LIMIT - 1)).unwrap();
        b.import(leaf(NODE_LIMIT - 1 + extra)).unwrap();
        assert_eq!(b.import_visits, RETAINED_FACT_LIMIT - 2 + extra);
        assert_eq!(b.charged_visits(), 4);
        let result = finish(b);
        if extra == 0 {
            let arena = result.unwrap();
            assert_eq!(arena.shape().visits, RETAINED_FACT_LIMIT);
            let mut next = PublishedValueArenaBuilder::new();
            next.import(arena).unwrap();
            assert_eq!(next.import_visits, RETAINED_FACT_LIMIT);
            assert_eq!(next.charged_visits(), 4);
            assert!(
                matches!(
                    finish(next),
                    Err(PublishedValueError::Limit("retained closure visits"))
                ),
                "one newly stored import reference still needs fact headroom"
            );
        } else {
            assert!(matches!(
                result,
                Err(PublishedValueError::Limit("retained closure visits"))
            ));
        }
    }
}

#[test]
fn owner_full_fact_boundary_is_checked_before_arc_with_same_small_work_prefix() {
    for extra in [0, 1] {
        // Actual repeated references stay paid, even though one immutable
        // shared allocation is retained. This differs from the simple constructor fixture.
        let shared = SharedPublicationInput::new(Arc::new(Leaf));
        let constructor_work = 2 + 128 + 1 + 3; // parts, references, body, arena visits
        let prior = pair(RETAINED_FACT_LIMIT - constructor_work + extra);
        let a = part(31, vec![], vec![]);
        let b = part(32, vec![prior], vec![shared; 128]);
        let before = (Arc::strong_count(&a), Arc::strong_count(&b));
        let result =
            FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredRule, a.clone(), b.clone());
        if extra == 0 {
            assert_eq!(
                result.as_ref().unwrap().retained_shape().visits,
                RETAINED_FACT_LIMIT
            );
            assert_eq!(Arc::strong_count(&a), before.0 + 1);
        } else {
            assert!(matches!(
                result,
                Err(PublishedValueError::Limit("owner retained visits"))
            ));
            assert_eq!((Arc::strong_count(&a), Arc::strong_count(&b)), before);
        }
    }
}

#[test]
fn larger_canonical_facts_do_not_expand_real_work_or_original_root_limits() {
    for (remaining, succeeds) in [(5, true), (4, false)] {
        let mut b = PublishedValueArenaBuilder::new();
        let id = b.reserve().unwrap();
        let atom = b.native_atom(AnalysisValue::none()).unwrap();
        b.set_node(id, PublishedNode::Native(atom)).unwrap();
        b.set_original_slot(id, PublishedRootSlot::new(0).unwrap())
            .unwrap();
        let originals = originals(&mut b, NODE_LIMIT);
        pad(&mut b, remaining);
        let result = b.seal(originals);
        assert_eq!(result.is_ok(), succeeds);
        if let Ok(arena) = result {
            assert_eq!(arena.shape().visits, NODE_LIMIT + 3);
        }
    }
    let mut work = PublishedValueArenaBuilder::new();
    pad(&mut work, 0);
    assert!(work.charge_visit().is_err());
    assert!(work.charge_bytes(0).is_err());
    let mut roots = PublishedValueArenaBuilder::new();
    // Own an actual bounded array first, then one real additional root. This
    // low-level invalid fixture exercises seal's unchanged independent check.
    let mut binding = originals(&mut roots, NODE_LIMIT);
    roots
        .charge_bytes(2 * (NODE_LIMIT + 1) * std::mem::size_of::<AnalysisValue>())
        .unwrap();
    Arc::get_mut(&mut binding).unwrap().roots = vec![AnalysisValue::none(); NODE_LIMIT + 1].into();
    assert!(matches!(
        roots.seal(binding),
        Err(PublishedValueError::Invalid("original imports"))
    ));
}
