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
        consumer.shape().visits + 3,
        "original D2 plus full imported arena facts and declaration reference R1"
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
                    Err(PublishedValueError::RetainedFactBudget(ref report))
                    if report.checked_total_facts() == Some(RETAINED_FACT_LIMIT + 1)
                        && report.limit_facts() == RETAINED_FACT_LIMIT
                ),
                "one newly stored import reference still needs fact headroom"
            );
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
fn owner_full_fact_boundary_is_checked_before_arc_with_same_small_work_prefix() {
    for extra in [0, 1] {
        // Actual repeated references stay paid, even though one immutable
        // shared allocation is retained. This differs from the simple constructor fixture.
        let shared = SharedPublicationInput::new(Arc::new(Leaf));
        // New counting work29: setup8 +two capacity scan units per3
        // buckets +scan controls2 +one shared read +root6 +finish6.
        let constructor_work = 2 + 2 * 128 + 1 + 3 + 29;
        let declaration_references = 128 + 1;
        let canonical_body_and_references = 2 + 1 + declaration_references;
        let prior = pair(RETAINED_FACT_LIMIT - canonical_body_and_references + extra);
        let prior_weak = Arc::downgrade(&prior.0);
        let a = part(31, vec![], vec![]);
        let b = part(32, vec![prior], vec![shared; 128]);
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
                    constructor_work,
                    0,
                    RETAINED_FACT_LIMIT - canonical_body_and_references + 1,
                    1,
                    0,
                    Some(constructor_work),
                    RETAINED_FACT_LIMIT + 1,
                    RETAINED_FACT_LIMIT
                )
            );
            assert_eq!(
                report.declaration_reference_visits(),
                declaration_references
            );
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

#[test]
fn larger_canonical_facts_do_not_expand_real_work_or_original_root_limits() {
    // Original five non-backing operations + view setup8 + empty backing1.
    for (remaining, succeeds) in [(14, true), (13, false)] {
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
