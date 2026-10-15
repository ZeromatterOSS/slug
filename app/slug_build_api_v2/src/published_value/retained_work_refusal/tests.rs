use super::*;
use crate::published_value::builder::import_accounting::tests::finish;
use crate::published_value::builder::retained_facts::tests::code;
use crate::published_value::builder::retained_facts::tests::source;

fn report(error: PublishedValueError) -> Box<PublicationRetainedCertificateWorkRefusal> {
    assert!(error.is_limit());
    let PublishedValueError::RetainedCertificateWork(r) = error else {
        panic!("expected seal-local work, got {error:?}")
    };
    r
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
fn seal_refusal_distinguishes_entry_work_from_local_walk_and_releases_all_guards() {
    for remaining in [0usize, 2] {
        let authority = source(3);
        let source_weak = Arc::downgrade(&authority.data().root.repository_mapping);
        let token = Arc::new(());
        let owner_weak = Arc::downgrade(&token);
        let owner = FunctionEvaluationOwner::new(
            FunctionOwnerDomain::ConfiguredRule,
            Arc::new(Guard(token.clone())),
            Arc::new(Guard(token)),
        )
        .unwrap();
        let mut b = PublishedValueArenaBuilder::new();
        code(&mut b, authority);
        b.audit_owner_shape(&owner).unwrap();
        drop(owner);
        while b.charged_visits() < NODE_LIMIT - remaining {
            b.charge_visit().unwrap();
        }
        let entry = b.charged_visits();
        let expected = [
            b.visits,
            b.owner_payload.visits,
            b.certified_shared_visits,
            b.shared_payload.visits,
            b.rows.len(),
            b.edges.len(),
            b.slots.len(),
            b.owner_roots.len(),
            b.shared_seen.len(),
            b.imports.len(),
        ];
        assert!(source_weak.upgrade().is_some());
        assert!(owner_weak.upgrade().is_some());
        let r = report(finish(b).unwrap_err());
        assert_eq!(r.context(), "sealLocalCertification");
        assert_eq!(r.phase(), "viewPrepay");
        assert_eq!(r.entry_work(), entry);
        assert_eq!(r.work_at_refusal(), NODE_LIMIT);
        assert_eq!(r.checked_added_work(), Some(remaining));
        assert_eq!(r.limit_work(), NODE_LIMIT);
        assert_eq!(
            [
                r.legacy_visits(),
                r.owner_facts(),
                r.certified_shared_facts(),
                r.shared_facts(),
                r.rows_len(),
                r.edges_len(),
                r.slots_len(),
                r.owner_roots_len(),
                r.shared_seen_len(),
                r.imports_len()
            ],
            expected
        );
        assert_eq!(
            r.legacy_visits() - r.owner_facts() - r.certified_shared_facts(),
            entry,
            "legacy counters describe entry, not restored post-walk W"
        );
        assert!(
            source_weak.upgrade().is_none(),
            "scalar error cannot own source storage"
        );
        assert!(
            owner_weak.upgrade().is_none(),
            "scalar error cannot own evaluation parts"
        );
        assert_eq!(r, r.clone());
    }
}

#[test]
fn wrapper_preserves_direct_work_nonwork_and_earlier_poison_error_boundaries() {
    let mut b = PublishedValueArenaBuilder::new();
    b.reserve().unwrap();
    assert_eq!(
        finish(b).unwrap_err(),
        PublishedValueError::Invalid("incomplete row")
    );
    #[derive(Debug)]
    struct NeverInspect;
    impl PublishedOriginalBinding for NeverInspect {
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn root_count(&self) -> usize {
            panic!("poison must precede original callbacks")
        }
        fn imported_arenas(&self) -> &[PublishedValueArena] {
            panic!("poison must precede original callbacks")
        }
    }
    let mut b = PublishedValueArenaBuilder::new();
    for _ in 0..NODE_LIMIT {
        b.charge_visit().unwrap();
    }
    b.charge_visit().unwrap_err();
    let error = b.seal(Arc::new(NeverInspect)).unwrap_err();
    assert!(matches!(error, PublishedValueError::ByteBudget(ref r) if r.prior_poisoned()));
}

#[test]
fn scalar_report_layout_classification_and_maximum_format_are_bounded() {
    #[allow(dead_code)]
    enum Before {
        Limit(&'static str),
        ByteBudget(Box<PublicationByteRefusal>),
        EdgeBudget(Box<PublicationEdgeRefusal>),
        RetainedFactBudget(Box<PublicationRetainedFactRefusal>),
        OwnerRetainedVisitBudget(Box<PublicationOwnerRetainedVisitRefusal>),
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
        size_of::<Result<PublishedValueArena, PublishedValueError>>(),
        size_of::<Result<PublishedValueArena, Before>>()
    );
    assert_eq!(
        size_of::<Result<FunctionEvaluationOwner, PublishedValueError>>(),
        size_of::<Result<FunctionEvaluationOwner, Before>>()
    );
    assert_eq!(
        size_of::<Result<PurePublishedAtom, PublishedValueError>>(),
        size_of::<Result<PurePublishedAtom, Before>>()
    );
    assert_eq!(
        size_of::<PublicationRetainedCertificateWorkRefusal>(),
        13 * size_of::<usize>()
    );
    assert_eq!(
        align_of::<PublicationRetainedCertificateWorkRefusal>(),
        align_of::<usize>()
    );
    let r = PublicationRetainedCertificateWorkRefusal {
        phase: retained_facts::CertificationPhase::StoredReferences,
        entry_work: usize::MAX,
        work_at_refusal: usize::MAX,
        legacy_visits: usize::MAX,
        owner_facts: usize::MAX,
        certified_shared_facts: usize::MAX,
        shared_facts: usize::MAX,
        rows_len: usize::MAX,
        edges_len: usize::MAX,
        slots_len: usize::MAX,
        owner_roots_len: usize::MAX,
        shared_seen_len: usize::MAX,
        imports_len: usize::MAX,
    };
    assert!(format!("{r:?}").len() < 2048);
    let error = PublishedValueError::RetainedCertificateWork(Box::new(r.clone()));
    assert!(error.is_limit());
    assert!(error.to_string().contains("sealLocalCertification"));
    assert!(error.to_string().len() < 2048);
    assert_eq!(
        error
            .clone()
            .with_byte_context(PublicationByteContext::OwnerBody),
        error
    );
    assert_eq!(
        error
            .clone()
            .with_edge_context(PublicationEdgeContext::SharedBody),
        error
    );
    assert_eq!(r.phase(), "storedReferences");
    assert_eq!(r.checked_added_work(), Some(0));
    let invalid = PublicationRetainedCertificateWorkRefusal {
        work_at_refusal: 0,
        ..r
    };
    assert_eq!(invalid.checked_added_work(), None);
}
