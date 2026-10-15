use super::*;

fn report(error: PublishedValueError) -> Box<PublicationOwnerRetainedVisitRefusal> {
    assert!(error.is_limit());
    let PublishedValueError::OwnerRetainedVisitBudget(report) = error else {
        panic!("expected final owner refusal, got {error:?}");
    };
    report
}

#[test]
fn completed_counter_snapshot_preserves_domain_checked_work_and_total_equations() {
    for domain in [
        FunctionOwnerDomain::BzlLoad,
        FunctionOwnerDomain::ConfiguredRule,
        FunctionOwnerDomain::ConfiguredAspect,
    ] {
        let mut b = PublishedValueArenaBuilder::new_strict_shared_audit();
        b.visits = 101;
        b.owner_payload.visits = 29;
        b.import_visits = 41;
        b.shared_payload.visits = 17;
        let result = report(b.owner_retained_visit_refusal(domain, 7, 11, 18, 5, 110));
        assert_eq!(result.domain(), domain);
        assert_eq!(
            (
                result.stable_key_visits(),
                result.input_validity_visits(),
                result.intrinsic_visits()
            ),
            (7, 11, 18)
        );
        assert_eq!(
            (
                result.legacy_local_visits(),
                result.owner_visits(),
                result.imported_visits(),
                result.shared_visits(),
                result.certified_shared_visits()
            ),
            (101, 29, 41, 17, 0)
        );
        assert_eq!(result.producer_work(), Some(72));
        assert_eq!(result.declaration_reference_visits(), 5);
        // The legacy audit/work equation remains independently observable.
        assert_eq!(result.legacy_local_visits() + result.imported_visits(), 142);
        assert_eq!(
            result.producer_work().unwrap() + result.owner_visits() + result.imported_visits(),
            142
        );
        assert_eq!(
            result.complete_visits(),
            result.intrinsic_visits()
                + result.owner_visits()
                + result.shared_visits()
                + result.imported_visits()
                + result.declaration_reference_visits()
        );
        assert_eq!(result.limit_visits(), RETAINED_FACT_LIMIT);
        assert_eq!(result, result.clone());
        assert_eq!(
            (
                b.visits,
                b.owner_payload.visits,
                b.import_visits,
                b.shared_payload.visits,
                b.certified_shared_visits,
                b.failed_budget
            ),
            (101, 29, 41, 17, 0, false)
        );
    }
    // Private scalar reporting only: malformed partial state cannot introduce
    // a panic or a new admission error. Production calls only after strict success.
    let mut b = PublishedValueArenaBuilder::new_strict_shared_audit();
    b.owner_payload.visits = 1;
    assert_eq!(
        report(b.owner_retained_visit_refusal(FunctionOwnerDomain::BzlLoad, 0, 0, 0, 0, 0))
            .producer_work(),
        None
    );
    b.visits = usize::MAX;
    b.import_visits = 1;
    assert_eq!(
        b.complete_shape().unwrap_err(),
        PublishedValueError::Limit("complete visits")
    );
    b.certified_shared_visits = 1;
    assert_eq!(
        b.complete_shape().unwrap_err(),
        PublishedValueError::Invalid("strict shared audit")
    );
}

#[test]
fn numeric_report_error_and_hot_result_layout_and_display_remain_bounded() {
    // Exact pre-change enum payloads, including the earlier boxed reports.
    #[allow(dead_code)]
    enum Before {
        Limit(&'static str),
        ByteBudget(Box<PublicationByteRefusal>),
        EdgeBudget(Box<PublicationEdgeRefusal>),
        RetainedFactBudget(Box<PublicationRetainedFactRefusal>),
        Invalid(&'static str),
        Unsupported(&'static str),
    }
    use std::mem::align_of;
    use std::mem::size_of;
    assert_eq!(
        (
            size_of::<PublishedValueError>(),
            align_of::<PublishedValueError>()
        ),
        (size_of::<Before>(), align_of::<Before>())
    );
    assert_eq!(
        (
            size_of::<Result<FunctionEvaluationOwner, PublishedValueError>>(),
            align_of::<Result<FunctionEvaluationOwner, PublishedValueError>>()
        ),
        (
            size_of::<Result<FunctionEvaluationOwner, Before>>(),
            align_of::<Result<FunctionEvaluationOwner, Before>>()
        )
    );
    let alignment = align_of::<usize>()
        .max(align_of::<Option<usize>>())
        .max(align_of::<FunctionOwnerDomain>());
    let fields = 11 * size_of::<usize>()
        + size_of::<Option<usize>>()
        + size_of::<FunctionOwnerDomain>()
        + size_of::<Option<PublicationClosureCensus>>();
    assert_eq!(
        align_of::<PublicationOwnerRetainedVisitRefusal>(),
        alignment
    );
    assert_eq!(
        size_of::<PublicationOwnerRetainedVisitRefusal>(),
        fields.div_ceil(alignment) * alignment
    );
    let mut b = PublishedValueArenaBuilder::new_strict_shared_audit();
    b.visits = usize::MAX;
    b.import_visits = usize::MAX;
    b.shared_payload.visits = usize::MAX;
    for domain in [
        FunctionOwnerDomain::BzlLoad,
        FunctionOwnerDomain::ConfiguredRule,
        FunctionOwnerDomain::ConfiguredAspect,
    ] {
        let error = b.owner_retained_visit_refusal(
            domain,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        );
        assert!(error.is_limit());
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
        assert!(text.starts_with("published value: owner retained visits admission refused:"));
        assert!(text.contains(&format!("domain: {domain:?}")));
        assert!(text.contains(&format!("complete_visits: {}", usize::MAX)));
        assert!(text.len() < 1024);
        assert!(format!("{error:?}").len() < 1024);
        assert!(!text.contains("/workspace"));
    }
    for error in [
        PublishedValueError::Limit("owner certificate visits"),
        PublishedValueError::Invalid("owner shape bytes"),
        PublishedValueError::Unsupported("kind"),
    ] {
        assert_eq!(error.to_string(), format!("published value: {error:?}"));
    }
}

#[derive(Debug, Allocative)]
struct PartialPart {
    shape: PublicationShape,
    declared_bytes: usize,
}
impl StructuralFunctionOwnerPart for PartialPart {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().is::<Self>()
    }
    fn hash_part(&self, h: &mut dyn std::hash::Hasher) {
        h.write_u8(0);
    }
    fn retained_bytes(&self) -> usize {
        self.declared_bytes
    }
    fn retained_shape(&self) -> PublicationShape {
        self.shape
    }
}
#[test]
fn earlier_constructor_shape_work_byte_and_depth_errors_keep_original_families() {
    let part = |visits, bytes, depth| {
        Arc::new(PartialPart {
            shape: PublicationShape {
                visits,
                bytes,
                depth,
                ..PublicationShape::default()
            },
            declared_bytes: bytes,
        })
    };
    let empty = || part(0, 0, 0);
    let work = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::BzlLoad,
        part(NODE_LIMIT + 1, 0, 0),
        empty(),
    )
    .unwrap_err();
    assert_eq!(work, PublishedValueError::Limit("storage visits"));
    let byte = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::BzlLoad,
        part(0, BYTE_LIMIT, 0),
        empty(),
    )
    .unwrap_err();
    assert!(matches!(byte, PublishedValueError::ByteBudget(_)));
    let depth = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::BzlLoad,
        part(0, 0, DEPTH_LIMIT + 1),
        empty(),
    )
    .unwrap_err();
    assert_eq!(depth, PublishedValueError::Limit("depth"));
    let shape = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::BzlLoad,
        Arc::new(PartialPart {
            shape: PublicationShape::default(),
            declared_bytes: 1,
        }),
        empty(),
    )
    .unwrap_err();
    assert_eq!(shape, PublishedValueError::Invalid("owner shape bytes"));
}
