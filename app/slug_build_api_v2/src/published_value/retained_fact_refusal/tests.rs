use super::*;

fn report(error: PublishedValueError) -> Box<PublicationRetainedFactRefusal> {
    assert!(error.is_limit());
    let PublishedValueError::RetainedFactBudget(report) = error else {
        panic!("expected complete final refusal, got {error:?}");
    };
    report
}

#[test]
fn checked_final_sum_preserves_order_boundary_and_overflow() {
    for (i, o, s, l) in [
        (0, 0, 0, 0),
        (17, 29, 41, 53),
        (RETAINED_FACT_LIMIT, 0, 0, 1),
    ] {
        assert_eq!(
            checked_total(i, o, s, l),
            i.checked_add(o)
                .and_then(|n| n.checked_add(s))
                .and_then(|n| n.checked_add(l))
        );
    }
    for values in [
        (usize::MAX, 1, 0, 0),
        (0, usize::MAX, 1, 0),
        (0, 0, usize::MAX, 1),
    ] {
        assert_eq!(checked_total(values.0, values.1, values.2, values.3), None);
    }
    assert_eq!(
        checked_total(RETAINED_FACT_LIMIT - 3, 1, 1, 1),
        Some(RETAINED_FACT_LIMIT)
    );
}

#[test]
fn final_refusal_keeps_exact_import_local_counts_and_no_payload_guards() {
    use super::super::import_accounting::tests::finish;
    use super::super::import_accounting::tests::leaf;
    let prior = leaf(NODE_LIMIT - 1);
    let weak = Arc::downgrade(&prior.0);
    let mut b = PublishedValueArenaBuilder::new();
    b.import(prior).unwrap();
    b.import(leaf(NODE_LIMIT)).unwrap();
    let report = report(finish(b).unwrap_err());
    assert_eq!(
        (
            report.imported_facts(),
            report.owner_facts(),
            report.shared_facts(),
            report.local_facts()
        ),
        (RETAINED_FACT_LIMIT - 1, 0, 0, 2)
    );
    assert_eq!(
        (
            report.local_source_facts(),
            report.local_native_facts(),
            report.local_other_facts()
        ),
        (0, 0, 2)
    );
    assert_eq!(report.checked_total_facts(), Some(RETAINED_FACT_LIMIT + 1));
    assert_eq!(report.limit_facts(), RETAINED_FACT_LIMIT);
    assert_eq!(report.certified_shared_facts(), 0);
    // Import entry4 + view8 + source setup1 + empty backing1 + empty slots1 + refs1.
    // R=0 adds1 work without changing the full import/local fact refusal.
    assert_eq!(report.producer_work(), 16);
    let cloned = report.clone();
    assert_eq!(report, cloned);
    assert!(
        weak.upgrade().is_none(),
        "keeping numeric errors cannot retain imported arenas"
    );
}

#[test]
fn fixed_numeric_error_layout_decoration_and_partial_failures_stay_separate() {
    #[allow(dead_code)]
    enum Legacy {
        Limit(&'static str),
        Invalid(&'static str),
        Unsupported(&'static str),
    }
    assert_eq!(
        std::mem::size_of::<PublishedValueError>(),
        std::mem::size_of::<Legacy>()
    );
    assert_eq!(
        std::mem::size_of::<PublicationRetainedFactRefusal>(),
        10 * std::mem::size_of::<usize>() + std::mem::size_of::<Option<usize>>()
    );
    let mut b = PublishedValueArenaBuilder::new();
    b.failed_budget = true;
    let error = b.retained_fact_refusal(
        retained_facts::LocalRetainedFacts {
            total: 31,
            source: 7,
            native: 11,
        },
        43,
        None,
    );
    assert_eq!(report(error.clone()).local_other_facts(), 13);
    assert_eq!(report(error.clone()).producer_work(), 43);
    assert_eq!(report(error.clone()).checked_total_facts(), None);
    assert_eq!(
        error
            .clone()
            .with_byte_context(PublicationByteContext::OwnerBody),
        error
    );
    assert_eq!(
        error
            .clone()
            .with_edge_context(PublicationEdgeContext::ImportBody),
        error
    );
    let text = error.to_string();
    assert!(text.starts_with("published value: retained closure visits admission refused:"));
    assert!(text.len() < 1024);
    assert!(!text.contains("/workspace"));
    assert!(b.charge_visit().is_err());
    for error in [
        PublishedValueError::Limit("retained facts"),
        PublishedValueError::Invalid("row"),
        PublishedValueError::Unsupported("kind"),
    ] {
        assert_eq!(error.to_string(), format!("published value: {error:?}"));
    }
    use super::super::import_accounting::tests::finish;
    let originals = finish(PublishedValueArenaBuilder::new())
        .unwrap()
        .0
        .originals
        .clone();
    let mut poisoned = PublishedValueArenaBuilder::new();
    assert!(poisoned.charge_bytes(BYTE_LIMIT + 1).is_err());
    assert!(!matches!(
        poisoned.seal(originals),
        Err(PublishedValueError::RetainedFactBudget(_))
    ));
}
