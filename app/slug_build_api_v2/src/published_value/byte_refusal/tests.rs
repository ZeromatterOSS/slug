use super::*;

fn refusal(error: PublishedValueError) -> Box<PublicationByteRefusal> {
    assert!(error.is_limit());
    let PublishedValueError::ByteBudget(refusal) = error else {
        panic!("expected byte admission refusal");
    };
    refusal
}

#[derive(Debug, Allocative)]
struct Part {
    imports: Vec<PublishedValueArena>,
    inputs: Vec<SharedPublicationInput>,
}
impl StructuralFunctionOwnerPart for Part {
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
        std::mem::size_of::<Self>()
            + self.imports.len() * std::mem::size_of::<PublishedValueArena>()
            + self.inputs.len() * std::mem::size_of::<SharedPublicationInput>()
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &self.inputs
    }
}
fn owner(part: Arc<Part>) -> FunctionEvaluationOwner {
    FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, part.clone(), part).unwrap()
}
#[derive(Debug, Allocative)]
struct Fact;
impl ImmutablePublicationInput for Fact {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        PublicationShape {
            bytes: 128,
            visits: 1,
            ..PublicationShape::default()
        }
    }
}

#[test]
fn exact_byte_boundary_and_overflow_preserve_the_refused_request_and_poison() {
    let mut b = PublishedValueArenaBuilder::new();
    let former_limit = 64 * 1024 * 1024;
    assert_eq!(BYTE_LIMIT, 2 * former_limit);
    assert_eq!(
        crate::callable_graph::CALLABLE_GRAPH_BYTE_LIMIT,
        former_limit
    );
    b.charge_bytes(former_limit + 1).unwrap();
    assert_eq!(b.charged_bytes(), former_limit + 1);
    assert!(b.rows.is_empty());
    b.charge_bytes(BYTE_LIMIT - former_limit - 1).unwrap();
    b.charge_bytes(0).unwrap();
    let error = b.charge_bytes(1).unwrap_err();
    let snapshot = refusal(error.clone());
    assert_eq!(snapshot.context(), PublicationByteContext::Local);
    assert_eq!(snapshot.used_bytes(), BYTE_LIMIT);
    assert_eq!(snapshot.requested_bytes(), 1);
    assert_eq!(snapshot.limit_bytes(), BYTE_LIMIT);
    assert_eq!(snapshot.checked_total_bytes(), Some(BYTE_LIMIT + 1));
    assert!(!snapshot.prior_poisoned());
    assert_eq!(snapshot.producer_work(), Some(0));
    assert_eq!(snapshot.import_bytes(), 0);
    assert_eq!(snapshot.owner_bytes(), 0);
    assert_eq!(snapshot.shared_bytes(), 0);
    assert_eq!(error, error.clone());
    assert!(error.to_string().contains("bytes"));
    assert!(error.to_string().len() < 1024);
    assert_eq!(b.charged_bytes(), BYTE_LIMIT);
    assert!(b.rows.is_empty());
    let poisoned = refusal(b.charge_bytes(0).unwrap_err());
    assert!(poisoned.prior_poisoned());
    assert_eq!(poisoned.checked_total_bytes(), Some(BYTE_LIMIT));
    assert!(b.reserve().is_err());

    let mut overflow = PublishedValueArenaBuilder::new();
    overflow.charge_bytes(1).unwrap();
    let snapshot = refusal(overflow.charge_bytes(usize::MAX).unwrap_err());
    assert_eq!(snapshot.checked_total_bytes(), None);
    assert!(!snapshot.prior_poisoned());
    assert_eq!(snapshot.used_bytes(), 1);
    assert_eq!(snapshot.requested_bytes(), usize::MAX);
    assert_eq!(overflow.charged_bytes(), 1);
    assert!(overflow.charge_visit().is_err());
}

#[test]
fn byte_error_layout_and_static_decoration_preserve_other_error_families() {
    #[allow(dead_code)]
    enum LegacyError {
        Limit(&'static str),
        Invalid(&'static str),
        Unsupported(&'static str),
    }
    assert_eq!(
        std::mem::size_of::<PublishedValueError>(),
        std::mem::size_of::<LegacyError>()
    ); // Current compiler/layout discriminator, not a Rust ABI guarantee.
    for error in [
        PublishedValueError::Limit("bytes"),
        PublishedValueError::Invalid("row"),
        PublishedValueError::Unsupported("kind"),
    ] {
        assert_eq!(
            error
                .clone()
                .with_byte_context(PublicationByteContext::OwnerBody),
            error
        );
        assert_eq!(error.to_string(), format!("published value: {error:?}"));
    }
    let mut b = PublishedValueArenaBuilder::new();
    let error = b
        .charge_bytes(BYTE_LIMIT + 1)
        .unwrap_err()
        .with_byte_context(PublicationByteContext::ImportBody)
        .with_byte_context(PublicationByteContext::OwnerBody);
    assert_eq!(refusal(error).context(), PublicationByteContext::ImportBody);
}

#[test]
fn record_and_atomic_scratch_refusals_keep_full_paid_bytes_and_classification() {
    let mut b = PublishedValueArenaBuilder::new();
    b.producer_scratch()
        .charge_parts(3, BYTE_LIMIT - 4)
        .unwrap();
    let scratch = b.producer_scratch_bytes;
    let mut record = b.begin_record(0).unwrap();
    let snapshot = refusal(record.producer_scratch().charge_parts(1, 1).unwrap_err());
    assert_eq!(snapshot.used_bytes(), BYTE_LIMIT - 1);
    assert_eq!(snapshot.requested_bytes(), 2);
    assert_eq!(snapshot.checked_total_bytes(), Some(BYTE_LIMIT + 1));
    assert_eq!(snapshot.producer_scratch_bytes(), scratch);
    assert_eq!(snapshot.producer_work(), Some(1));
    record.abort();
    assert_eq!(b.producer_scratch_bytes, scratch);
    assert_eq!(b.charged_bytes(), BYTE_LIMIT - 1);
    assert_eq!((b.rows.len(), b.slots.len()), (0, 0));
    assert!(b.begin_record(0).is_err());
}

#[test]
fn owner_and_shared_body_refusals_preserve_uncommitted_certificates_and_guards() {
    let fact = Arc::new(Fact);
    let weak_fact = Arc::downgrade(&fact);
    let part = Arc::new(Part {
        imports: vec![],
        inputs: vec![SharedPublicationInput::new(fact.clone())],
    });
    let owner = owner(part.clone());
    let body = owner.intrinsic_shape().bytes;
    let active = 3 * std::mem::size_of::<usize>();
    // The completed leaf reference measures the actual traversal prefix, now
    // including its prepaid frame, active-table growth and completed entry.
    // Leave exactly 127 bytes for the same 128-byte SharedBody admission.
    let mut reference = PublishedValueArenaBuilder::new();
    reference
        .audit_shared_input(&SharedPublicationInput::new(fact.clone()), 0)
        .unwrap();
    assert_eq!(reference.shared_payload.bytes, 128);
    let shared_prefix = reference.charged_bytes() - reference.shared_payload.bytes;
    drop(reference);
    for (remaining, context) in [
        (active + body - 1, PublicationByteContext::OwnerBody),
        (
            active + body + shared_prefix + 127,
            PublicationByteContext::SharedBody,
        ),
    ] {
        let mut b = PublishedValueArenaBuilder::new();
        b.charge_bytes(BYTE_LIMIT - remaining).unwrap();
        let before = Arc::strong_count(&fact);
        let snapshot = refusal(b.audit_owner_shape(&owner).unwrap_err());
        assert_eq!(snapshot.context(), context);
        assert_eq!(
            snapshot.requested_bytes(),
            if context == PublicationByteContext::OwnerBody {
                body
            } else {
                128
            }
        );
        assert_eq!(snapshot.checked_total_bytes(), Some(BYTE_LIMIT + 1));
        assert_eq!(
            snapshot.owner_bytes(),
            if context == PublicationByteContext::OwnerBody {
                0
            } else {
                body
            }
        );
        assert_eq!(snapshot.shared_bytes(), 0);
        assert_eq!(b.owner_payload.bytes, snapshot.owner_bytes());
        assert_eq!(b.shared_payload.bytes, 0);
        assert!(b.shared_seen.is_empty());
        assert!(b.owner_seen.is_empty());
        assert!(b.owner_roots.is_empty());
        assert_eq!(Arc::strong_count(&fact), before);
        assert!(b.audit_owner_shape(&owner).is_err());
    }
    drop((owner, part, fact));
    assert!(weak_fact.upgrade().is_none());
}

#[test]
fn nested_import_body_refusal_keeps_its_context_and_commits_no_import_certificate() {
    let prior = import_accounting::tests::leaf(4);
    let part = Arc::new(Part {
        imports: vec![prior.clone()],
        inputs: vec![],
    });
    let owner = owner(part);
    let body = prior.0.local_shape.bytes;
    let active = 3 * std::mem::size_of::<usize>();
    let remaining = active + owner.intrinsic_shape().bytes + active + body - 1;
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_bytes(BYTE_LIMIT - remaining).unwrap();
    let snapshot = refusal(b.audit_owner_shape(&owner).unwrap_err());
    assert_eq!(snapshot.context(), PublicationByteContext::ImportBody);
    assert_eq!(snapshot.requested_bytes(), body);
    assert_eq!(snapshot.checked_total_bytes(), Some(BYTE_LIMIT + 1));
    assert_eq!(snapshot.import_bytes(), 0);
    assert_eq!(snapshot.owner_bytes(), owner.intrinsic_shape().bytes);
    assert_eq!(b.import_visits, 0);
    assert_eq!(b.import_bytes, 0);
    assert!(b.imports.is_empty());
    assert!(b.closure_seen.is_empty());
    assert!(b.owner_seen.is_empty());
    assert!(b.owner_roots.is_empty());
    assert!(b.charge_bytes(0).is_err());
    let weak = Arc::downgrade(&prior.0);
    drop((prior, owner));
    assert!(weak.upgrade().is_some()); // Existing active import guard remains paid.
    drop(b);
    assert!(weak.upgrade().is_none());
}
