use super::*;

fn maximum() -> PublicationNativeRetentionCensus {
    let stat = PublicationNativeStatistics {
        count: usize::MAX,
        visits: usize::MAX,
        maximum: usize::MAX,
    };
    PublicationNativeRetentionCensus {
        channels: [stat; 3],
        families: [stat; 10],
        row_kinds: [usize::MAX; 16],
        rows: usize::MAX,
        maximum_rows: usize::MAX,
        edges: usize::MAX,
        maximum_edges: usize::MAX,
        slots: usize::MAX,
        maximum_slots: usize::MAX,
        maximum_local_visits: usize::MAX,
        maximum_arena_certificates: usize::MAX,
        maximum_arena_native_visits: usize::MAX,
        native_visits: usize::MAX,
        other_visits: usize::MAX,
        feature_selected: false,
        feature_ambiguous: false,
        feature_maximum_wrapper_visits: usize::MAX,
        feature_occurrences: usize::MAX,
        feature_arenas: usize::MAX,
        feature_wrapper_visits: usize::MAX,
    }
}
#[test]
fn real_final_refusal_retains_only_scalars_and_complete_format_layout_is_bounded() {
    let (channel, _, _, _) = channel_arena();
    let source_weak = Arc::downgrade(&channel.sources()[0].data().root.repository_mapping);
    let f = features("feature".into());
    let model_weak = Arc::downgrade(f.model());
    let (feature, _) = native_arena(vec![AnalysisValue::cc_toolchain_features(f)]);
    let arena_weak = Arc::downgrade(&feature.0);
    // Intrinsic2 +four explicit arenas +one local source encounter.
    let work = 7usize + 22; // paid constructor completion
    let facts = channel.0.local_shape.visits + feature.0.local_shape.visits;
    let padding = RETAINED_FACT_LIMIT + 1 - 2 - 4 - facts - (NODE_LIMIT - 1);
    let key = part(40, 1, vec![], vec![]);
    let validity = part(
        41,
        1,
        vec![],
        vec![channel, feature, leaf(NODE_LIMIT - 1), leaf(padding)],
    );
    let key_weak = Arc::downgrade(&key);
    let validity_weak = Arc::downgrade(&validity);
    let error = FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredRule, key, validity)
        .unwrap_err();
    let PublishedValueError::OwnerRetainedVisitBudget(mut report) = error else {
        panic!("expected final refusal: {error:?}")
    };
    assert_eq!(report.complete_visits(), RETAINED_FACT_LIMIT + 1);
    assert_eq!(report.producer_work(), Some(work));
    assert_eq!(report.declaration_reference_visits(), 4);
    let native = report.closure_census().unwrap().native_retention().unwrap();
    assert!(native.feature_selected());
    assert_eq!(native.feature_occurrences(), 1);
    assert_eq!(native.feature_arenas(), 1);
    let cloned = report.clone();
    assert_eq!(report, cloned);
    assert!(model_weak.upgrade().is_none() && arena_weak.upgrade().is_none());
    assert!(source_weak.upgrade().is_none());
    assert!(key_weak.upgrade().is_none() && validity_weak.upgrade().is_none());
    assert_eq!(
        std::mem::align_of::<PublicationNativeRetentionCensus>(),
        std::mem::align_of::<usize>()
    );
    // Seventy usize plus two bools, rounded to the supported scalar alignment.
    let fields = 70 * std::mem::size_of::<usize>() + 2 * std::mem::size_of::<bool>();
    let alignment = std::mem::align_of::<usize>();
    assert_eq!(
        std::mem::size_of::<PublicationNativeRetentionCensus>(),
        fields.div_ceil(alignment) * alignment
    );
    // Worst-case formatting of every old and new fixed scalar; never a certificate.
    report.domain = FunctionOwnerDomain::ConfiguredAspect;
    report.stable_key_visits = usize::MAX;
    report.input_validity_visits = usize::MAX;
    report.intrinsic_visits = usize::MAX;
    report.declaration_reference_visits = usize::MAX;
    report.legacy_local_visits = usize::MAX;
    report.owner_visits = usize::MAX;
    report.imported_visits = usize::MAX;
    report.shared_visits = usize::MAX;
    report.certified_shared_visits = usize::MAX;
    report.producer_work = Some(usize::MAX);
    report.complete_visits = usize::MAX;
    report.limit_visits = usize::MAX;
    let closure = report.closure_census.as_mut().unwrap();
    let owner = PublicationOwnerDomainCensus {
        count: usize::MAX,
        intrinsic_visits: usize::MAX,
        maximum_intrinsic_visits: usize::MAX,
    };
    closure.bzl_load = owner.clone();
    closure.configured_rule = owner.clone();
    closure.configured_aspect = owner;
    closure.imported_arenas = usize::MAX;
    closure.imported_local_visits = usize::MAX;
    closure.source_references = usize::MAX;
    closure.imported_source_body_facts = usize::MAX;
    closure.unique_sources = usize::MAX;
    closure.unique_source_body_facts = usize::MAX;
    closure.native_retention = Some(maximum());
    // Maximum new fixed fields; original scalar-only max fixture remains <1024.
    let error = PublishedValueError::OwnerRetainedVisitBudget(report);
    assert!(error.to_string().len() <= 4096);
    assert!(format!("{error:?}").len() <= 4096);
    assert!(error.is_limit());
    assert_eq!(error, error.clone());
}
