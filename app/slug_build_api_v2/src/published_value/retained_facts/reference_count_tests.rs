use super::*;

#[test]
fn large_stored_root_count_keeps_full_facts_and_independent_import_work_limit() {
    // Original five non-backing operations + view setup8 + empty backing1.
    for (remaining, succeeds) in [(14, true), (13, false)] {
        let mut builder = PublishedValueArenaBuilder::new();
        atom(&mut builder, AnalysisValue::integer(7));
        while builder.charged_visits() < NODE_LIMIT - remaining {
            builder.charge_visit().unwrap();
        }
        let result = builder.seal(Arc::new(Originals {
            count: 7000,
            imports: vec![],
        }));
        assert_eq!(result.is_ok(), succeeds, "remaining seal work {remaining}");
        if let Ok(prior) = result {
            assert_eq!(prior.0.local_shape.visits, 7003);
            assert_eq!(prior.shape().visits, 7003);
            assert_eq!(
                prior
                    .original_slot(PublishedNodeId::new(0).unwrap())
                    .unwrap()
                    .index(),
                0
            );
            let mut importer = PublishedValueArenaBuilder::new();
            for _ in 0..NODE_LIMIT - 2 {
                importer.charge_visit().unwrap();
            }
            importer.import(prior.clone()).unwrap();
            assert_eq!(importer.charged_visits(), NODE_LIMIT);
            assert_eq!(importer.import_visits, 7003);
            assert_eq!(importer.complete_shape().unwrap().visits, NODE_LIMIT + 7003);
            let mut refused = PublishedValueArenaBuilder::new();
            for _ in 0..NODE_LIMIT - 1 {
                refused.charge_visit().unwrap();
            }
            assert!(refused.import(prior).is_err());
            assert_eq!(
                refused.import_visits, 7003,
                "body admitted before registration work fails"
            );
            assert!(refused.charge_bytes(0).is_err());
        }
    }
}

#[test]
fn empty_groups_cost_no_aggregate_and_all_lengths_keep_full_counts() {
    let mut work = NODE_LIMIT - 1;
    let mut bytes = 0;
    let mut failed = false;
    let (count, sources) = local_visits(
        &[],
        &[],
        &[],
        &[],
        0,
        0,
        &mut work,
        &mut bytes,
        &mut 0,
        &mut failed,
    )
    .unwrap();
    assert_eq!(work, NODE_LIMIT, "only source-table setup is inspected");
    assert_eq!(count.total, 0);
    assert!(sources.is_empty());
    assert!(!failed);
    let mut work = 0;
    let mut bytes = 0;
    let mut failed = false;
    let (count, _) = local_visits(
        &[],
        &[],
        &[],
        &[],
        4,
        5,
        &mut work,
        &mut bytes,
        &mut 0,
        &mut failed,
    )
    .unwrap();
    assert_eq!(
        work, 2,
        "setup plus one complete nonempty reference aggregate"
    );
    assert_eq!(count.total, 9);
    // An actual imported arena contributes its stored reference even when it
    // has no local rows; imported payload admission remains a separate walk.
    let prior = seal(PublishedValueArenaBuilder::new(), 0);
    let mut work = 0;
    let (count, _) = local_visits(
        &[],
        &[],
        &[],
        &[prior],
        4,
        5,
        &mut work,
        &mut 0,
        &mut 0,
        &mut false,
    )
    .unwrap();
    assert_eq!(work, 2);
    assert_eq!(count.total, 10);
}

#[test]
fn reference_sum_overflow_and_retained_limit_poison_without_partial_count() {
    for (owners, roots) in [(usize::MAX, 1), (0, RETAINED_FACT_LIMIT + 1)] {
        let mut work = 0;
        let mut bytes = 0;
        let mut failed = false;
        assert!(
            local_visits(
                &[],
                &[],
                &[],
                &[],
                owners,
                roots,
                &mut work,
                &mut bytes,
                &mut 0,
                &mut failed
            )
            .is_err()
        );
        assert!(failed);
        assert!(
            local_visits(
                &[],
                &[],
                &[],
                &[],
                0,
                0,
                &mut work,
                &mut bytes,
                &mut 0,
                &mut failed
            )
            .is_err()
        );
    }
    // Primitive count arithmetic admits the exact fact boundary; real original
    // array lengths retain their independent NODE_LIMIT check in seal.
    let (full, _) = local_visits(
        &[],
        &[],
        &[],
        &[],
        0,
        RETAINED_FACT_LIMIT,
        &mut 0,
        &mut 0,
        &mut 0,
        &mut false,
    )
    .unwrap();
    assert_eq!(full.total, RETAINED_FACT_LIMIT);
    let prior = seal(PublishedValueArenaBuilder::new(), 0);
    let mut failed = false;
    assert!(
        local_visits(
            &[],
            &[],
            &[],
            &[prior],
            usize::MAX,
            0,
            &mut 0,
            &mut 0,
            &mut 0,
            &mut failed
        )
        .is_err()
    );
    assert!(failed, "the import-plus-owner addition is checked too");
}

#[test]
fn aggregate_scalar_bytes_are_prepaid_and_slot_integrity_stays_strict() {
    let mut scratch = 0;
    local_visits(
        &[],
        &[],
        &[],
        &[],
        0,
        0,
        &mut 0,
        &mut scratch,
        &mut 0,
        &mut false,
    )
    .unwrap();
    for (allowance, succeeds) in [
        (3 * std::mem::size_of::<usize>(), true),
        (3 * std::mem::size_of::<usize>() - 1, false),
    ] {
        let mut bytes = BYTE_LIMIT - scratch - allowance;
        let mut work = 0;
        let mut failed = false;
        let result = local_visits(
            &[],
            &[],
            &[],
            &[],
            0,
            7000,
            &mut work,
            &mut bytes,
            &mut 0,
            &mut failed,
        );
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(failed, !succeeds);
        if let Ok((count, _)) = result {
            assert_eq!(count.total, 7000);
            assert_eq!(bytes, BYTE_LIMIT);
        }
    }
    let row = Some(PublishedNode::Cell { contents: None });
    let slot = PublishedRootSlot::new(0).unwrap();
    let mut failed = false;
    assert!(
        local_visits(
            &[row],
            &[],
            &[Some(slot)],
            &[],
            0,
            0,
            &mut 0,
            &mut 0,
            &mut 0,
            &mut failed
        )
        .is_err()
    );
    assert!(failed, "invalid original slot is still visited and refused");
}
