use super::*;

#[test]
fn preread_bounds_and_unavailable_native_detail_preserve_old_census_and_ledgers() {
    let (arena, _, sources, _) = channel_arena();
    let mut b = draft(&[arena]);
    let closure = summary(&mut b);
    let shape = |s: PublicationShape| (s.nodes, s.edges, s.bytes, s.visits, s.depth);
    let ledgers = (
        b.visits,
        b.import_visits,
        shape(b.owner_payload),
        shape(b.shared_payload),
        b.certified_shared_visits,
        b.failed_budget,
    );
    let guards = b
        .closure_seen
        .iter()
        .map(|i| i.0.identity())
        .collect::<Vec<_>>();
    let rows = closure.native_retention().unwrap().rows();
    // One arena header +four rows +two provider entries +three certificates.
    assert!(observe(&b, rows, sources, Observer { remaining: 10 }).is_some());
    for remaining in 0..10 {
        assert!(observe(&b, rows, sources, Observer { remaining }).is_none());
    }
    assert!(observe(&b, rows, usize::MAX, Observer { remaining: 10 }).is_none());
    assert!(allowance(&b, None).is_none());
    assert!(allowance(&b, Some(b.import_visits + 1)).is_none());
    // Native unavailability is optional; the original O/I/P/U section survives.
    let mut preserved = closure.clone();
    preserved.native_retention = collect(&b, None, sources);
    assert!(preserved.native_retention().is_none());
    assert_eq!(
        preserved.imported_local_visits(),
        closure.imported_local_visits()
    );
    assert_eq!(
        preserved.imported_source_body_facts(),
        closure.imported_source_body_facts()
    );
    assert_eq!(
        preserved.unique_source_body_facts(),
        closure.unique_source_body_facts()
    );
    let mut cursor = Cursor {
        reads: Observer { remaining: 0 },
        rows: 1,
        providers: 1,
    };
    assert!(cursor.row().is_none());
    assert_eq!(cursor.rows, 1);
    assert!(cursor.provider().is_none());
    assert_eq!(cursor.providers, 1);
    assert!(cursor.certificate().is_none());
    let mut cursor = Cursor {
        reads: Observer { remaining: 10 },
        rows,
        providers: 0,
    };
    let mut partial = PublicationNativeRetentionCensus::default();
    assert!(walk::first(&b, &mut cursor, &mut partial).is_none());
    assert_eq!(
        partial
            .channel(PublicationNativeChannel::TargetProvider)
            .count(),
        0
    );
    assert_eq!(
        (
            b.visits,
            b.import_visits,
            shape(b.owner_payload),
            shape(b.shared_payload),
            b.certified_shared_visits,
            b.failed_budget
        ),
        ledgers
    );
    assert_eq!(
        b.closure_seen
            .iter()
            .map(|i| i.0.identity())
            .collect::<Vec<_>>(),
        guards
    );
    let mut only_code = PublishedValueArenaBuilder::new();
    code(&mut only_code, source(0));
    let mut no_native = draft(&[seal(only_code, 0)]);
    let only_source = summary(&mut no_native);
    assert_eq!(only_source.native_retention().unwrap().native_visits(), 0);
    assert!(!only_source.native_retention().unwrap().feature_selected());
    let (origin, _) = native_arena(vec![AnalysisValue::string("origin")]);
    let projected = AnalysisValue::with_original_origin(
        AnalysisValue::string("origin"),
        origin.value(PublishedNodeId::new(0).unwrap()).unwrap(),
    )
    .unwrap();
    assert!(
        classifier::atom(&projected).is_none(),
        "origin barrier precedes potentially delegating kind"
    );
    let f = features("feature".into());
    let (arena, _) = native_arena(vec![AnalysisValue::cc_toolchain_features(f)]);
    let b = draft(&[arena]);
    // Two passes: each has one arena, one row and one certificate read.
    assert!(observe(&b, 1, 0, Observer { remaining: 6 }).is_some());
    for remaining in 0..6 {
        assert!(observe(&b, 1, 0, Observer { remaining }).is_none());
    }
    let mut overflow = PublicationNativeStatistics {
        count: usize::MAX,
        ..Default::default()
    };
    assert!(overflow.add(1).is_none());
    let mut overflow = PublicationNativeStatistics {
        visits: usize::MAX,
        ..Default::default()
    };
    assert!(overflow.add(1).is_none());
    // Impossible certificate family cannot cause a payload walk or fallback.
    assert!(classifier::atom(&AnalysisValue::tuple(vec![])).is_none());
    let mut b = b;
    b.owner_seen.reserve(4 * NODE_LIMIT + 1);
    assert!(allowance(&b, Some(1)).is_none());
}
