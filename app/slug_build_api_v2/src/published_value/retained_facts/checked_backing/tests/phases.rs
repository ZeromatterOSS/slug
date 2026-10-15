use super::*;

#[test]
fn closed_phases_and_raw_invalid_edges_preserve_failure_order() {
    let row = [Some(PublishedNode::Cell { contents: None })];
    for (remaining, expected) in [
        (0, CertificationPhase::SourceSetup),
        (1, CertificationPhase::Rows),
        (2, CertificationPhase::BackingEdges),
        (3, CertificationPhase::RootSlots),
    ] {
        let mut work = NODE_LIMIT - remaining;
        let mut failed = false;
        let mut phase = CertificationPhase::ViewPrepay;
        let result = audit::<Checked>(
            &row,
            &[],
            &[None],
            0,
            0,
            &mut work,
            &mut 0,
            &mut 0,
            &mut failed,
            &mut phase,
        );
        assert!(matches!(
            result,
            Err(PublishedValueError::Limit("retained certificate work"))
        ));
        assert_eq!(phase, expected);
        assert_eq!(work, NODE_LIMIT);
        assert!(failed);
    }
    let mut phase = CertificationPhase::ViewPrepay;
    let result = audit::<Checked>(
        &[],
        &[],
        &[],
        1,
        0,
        &mut (NODE_LIMIT - 3),
        &mut 0,
        &mut 0,
        &mut false,
        &mut phase,
    );
    assert!(matches!(
        result,
        Err(PublishedValueError::Limit("retained certificate work"))
    ));
    assert_eq!(phase, CertificationPhase::StoredReferences);
    let source = crate::published_value::builder::retained_facts::tests::source(0);
    let mut builder = PublishedValueArenaBuilder::new();
    crate::published_value::builder::retained_facts::tests::code(&mut builder, source);
    let mut total = 0;
    audit::<Checked>(
        &builder.rows,
        &builder.edges,
        &builder.slots,
        0,
        0,
        &mut total,
        &mut 0,
        &mut 0,
        &mut false,
        &mut phase,
    )
    .unwrap();
    let mut work = NODE_LIMIT - total + 1;
    let mut failed = false;
    let result = audit::<Checked>(
        &builder.rows,
        &builder.edges,
        &builder.slots,
        0,
        0,
        &mut work,
        &mut 0,
        &mut 0,
        &mut failed,
        &mut phase,
    );
    assert!(matches!(
        result,
        Err(PublishedValueError::Limit("retained certificate work"))
    ));
    assert_eq!(phase, CertificationPhase::SourceCount);
    assert!(failed);
    for edge in [
        PublishedEdge::Local(PublishedNodeId::new(1).unwrap()),
        PublishedEdge::Imported {
            arena: PublishedImportId::new(0).unwrap(),
            node: PublishedNodeId::new(0).unwrap(),
        },
    ] {
        let mut failed = false;
        let result = local_visits(
            &row,
            &[edge],
            &[None],
            &[],
            0,
            0,
            &mut 0,
            &mut 0,
            &mut 0,
            &mut failed,
        );
        assert!(matches!(
            result,
            Err(PublishedValueError::Invalid(
                "retained edge" | "retained import"
            ))
        ));
        assert!(failed, "raw unused backing is still validated");
    }
    let labels = [
        CertificationPhase::ViewPrepay,
        CertificationPhase::SourceSetup,
        CertificationPhase::Rows,
        CertificationPhase::BackingEdges,
        CertificationPhase::RootSlots,
        CertificationPhase::StoredReferences,
        CertificationPhase::SourceCount,
    ]
    .map(CertificationPhase::label);
    assert_eq!(
        labels,
        [
            "viewPrepay",
            "sourceSetup",
            "rows",
            "backingEdges",
            "rootSlots",
            "storedReferences",
            "sourceCount"
        ]
    );
}

#[test]
fn deferred_slot_accounting_is_not_validity_authority_and_raw_order_stays_strict() {
    let rows = [Some(PublishedNode::Cell { contents: None })];
    let bad = [Some(PublishedRootSlot::new(0).unwrap())];
    for raw in [true, false] {
        let mut work = 0;
        let mut failed = false;
        let mut phase = CertificationPhase::ViewPrepay;
        let result = if raw {
            audit::<Raw>(
                &rows,
                &[],
                &bad,
                0,
                0,
                &mut work,
                &mut 0,
                &mut 0,
                &mut failed,
                &mut phase,
            )
        } else {
            audit::<Checked>(
                &rows,
                &[],
                &bad,
                0,
                0,
                &mut work,
                &mut 0,
                &mut 0,
                &mut failed,
                &mut phase,
            )
        };
        if raw {
            assert!(matches!(
                result,
                Err(PublishedValueError::Invalid("original slot"))
            ));
            assert_eq!(
                (work, failed, phase),
                (3, true, CertificationPhase::RootSlots)
            );
        } else {
            let (facts, sources) = result.unwrap();
            assert_eq!((facts.total, facts.source, facts.native), (2, 0, 0));
            assert!(sources.is_empty());
            assert_eq!((work, failed), (4, false));
            // These are private draft facts, never an accepted arena. Genuine
            // consuming-seal tests below require the final validator to refuse.
        }
    }
    // Same malformed draft: raw rejects its invalid slot BEFORE stored refs;
    // consuming accounting reaches that genuine later resource boundary first.
    let mut raw_failed = false;
    let raw = audit::<Raw>(
        &rows,
        &[],
        &bad,
        usize::MAX,
        0,
        &mut 0,
        &mut 0,
        &mut 0,
        &mut raw_failed,
        &mut CertificationPhase::ViewPrepay,
    );
    // Use root_count0 for invalidity while owner_count forces later overflow.
    assert!(matches!(
        raw,
        Err(PublishedValueError::Invalid("original slot"))
    ));
    assert!(raw_failed);
    let mut failed = false;
    let mut phase = CertificationPhase::ViewPrepay;
    let result = audit::<Checked>(
        &rows,
        &[],
        &bad,
        usize::MAX,
        0,
        &mut 0,
        &mut 0,
        &mut 0,
        &mut failed,
        &mut phase,
    );
    assert!(matches!(
        result,
        Err(PublishedValueError::Limit("retained facts"))
    ));
    assert!(failed);
    assert_eq!(phase, CertificationPhase::StoredReferences);
    for raw in [true, false] {
        let mut failed = false;
        let mut phase = CertificationPhase::ViewPrepay;
        let result = if raw {
            audit::<Raw>(
                &rows,
                &[],
                &[],
                0,
                0,
                &mut 0,
                &mut 0,
                &mut 0,
                &mut failed,
                &mut phase,
            )
        } else {
            audit::<Checked>(
                &rows,
                &[],
                &[],
                0,
                0,
                &mut 0,
                &mut 0,
                &mut 0,
                &mut failed,
                &mut phase,
            )
        };
        assert!(matches!(
            result,
            Err(PublishedValueError::Invalid("retained root slots"))
        ));
        assert!(failed);
        assert_eq!(phase, CertificationPhase::RootSlots);
    }
}
