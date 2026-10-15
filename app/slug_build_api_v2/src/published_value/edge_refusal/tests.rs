use super::*;

fn refusal(error: PublishedValueError) -> Box<PublicationEdgeRefusal> {
    assert!(error.is_limit());
    let PublishedValueError::EdgeBudget(refusal) = error else {
        panic!("edge refusal");
    };
    refusal
}

// Real two-row descriptors and corresponding native originals, reused by both
// the mixed ledger boundary and genuine nested-import body discriminator.
fn tuple_builder() -> PublishedValueArenaBuilder {
    let mut b = PublishedValueArenaBuilder::new();
    let leaf = b.reserve().unwrap();
    let atom = b.native_atom(AnalysisValue::none()).unwrap();
    b.set_node(leaf, PublishedNode::Native(atom)).unwrap();
    b.set_original_slot(leaf, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    let range = b.append_edges(&[PublishedEdge::Local(leaf)]).unwrap();
    let tuple = b.reserve().unwrap();
    b.set_node(tuple, PublishedNode::Tuple(range)).unwrap();
    b.set_original_slot(tuple, PublishedRootSlot::new(1).unwrap())
        .unwrap();
    b
}
#[derive(Debug)]
struct Originals([AnalysisValue; 2]);
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        self.0.len()
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &[]
    }
}
fn prior() -> PublishedValueArena {
    let mut b = tuple_builder();
    b.charge_bytes(
        std::mem::size_of::<Originals>()
            + 2 * std::mem::size_of::<usize>()
            + 4 * std::mem::size_of::<AnalysisValue>(),
    )
    .unwrap();
    b.seal(Arc::new(Originals([
        AnalysisValue::none(),
        AnalysisValue::tuple(vec![AnalysisValue::none()]),
    ])))
    .unwrap()
}

#[test]
fn exact_mixed_edge_boundary_keeps_all_components_and_prior_poison() {
    let mut b = tuple_builder();
    b.import(prior()).unwrap();
    let (direct, inline, imported) = (b.edges.len(), b.inline_edges, b.imported_edges);
    assert!(direct > 0 && inline > 0 && imported > 0);
    let left = EDGE_LIMIT - direct - inline - imported - b.owner_edges;
    b.charge_owner_edges(left).unwrap();
    b.charge_owner_edges(0).unwrap();
    let before = b.owner_edges;
    let error = b.charge_owner_edge().unwrap_err();
    let s = refusal(error.clone());
    assert_eq!(s.context(), PublicationEdgeContext::Local);
    assert_eq!(
        (s.used_owner_edges(), s.requested_edges(), s.limit_edges()),
        (before, 1, EDGE_LIMIT)
    );
    assert_eq!(
        (s.direct_edges(), s.inline_edges(), s.imported_edges()),
        (direct, inline, imported)
    );
    assert_eq!(s.checked_owner_total(), Some(before + 1));
    assert_eq!(s.checked_joint_before(), Some(EDGE_LIMIT));
    assert_eq!(s.checked_joint_total(), Some(EDGE_LIMIT + 1));
    assert_eq!((s.owner_body_edges(), s.shared_body_edges()), (0, 0));
    assert!(!s.prior_poisoned());
    assert_eq!(b.owner_edges, before);
    let poisoned = refusal(b.charge_owner_edges(0).unwrap_err());
    assert!(poisoned.prior_poisoned());
    assert_eq!(poisoned.checked_joint_total(), Some(EDGE_LIMIT));
    assert_eq!(b.owner_edges, before);
    assert!(b.reserve().is_err());
    assert_eq!(error, error.clone());
}

#[test]
fn scalar_overflow_is_distinct_from_threshold_and_stays_poisoned() {
    let mut b = PublishedValueArenaBuilder::new();
    b.owner_edges = usize::MAX; // Explicit test-only impossible-size scalar state.
    let s = refusal(b.charge_owner_edge().unwrap_err());
    assert_eq!(s.checked_owner_total(), None);
    assert_eq!(s.checked_joint_before(), Some(usize::MAX));
    assert_eq!(s.checked_joint_total(), None);
    assert!(!s.prior_poisoned());
    assert_eq!(b.owner_edges, usize::MAX);
    assert!(refusal(b.charge_owner_edges(0).unwrap_err()).prior_poisoned());
    let mut b = PublishedValueArenaBuilder::new();
    b.owner_edges = 1;
    b.inline_edges = usize::MAX;
    let s = refusal(b.charge_owner_edges(0).unwrap_err());
    assert_eq!(s.checked_owner_total(), Some(1));
    assert_eq!(s.checked_joint_before(), None);
    assert_eq!(s.checked_joint_total(), None);
    assert!(!s.prior_poisoned());
    assert_eq!((b.owner_edges, b.inline_edges), (1, usize::MAX));
    assert!(b.charge_visit().is_err());
}

#[derive(Debug, Allocative)]
struct Part {
    id: u8,
    members: [u8; 2],
    imports: Vec<PublishedValueArena>,
    inputs: Vec<SharedPublicationInput>,
}
impl StructuralFunctionOwnerPart for Part {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        // This finite fixture closes each id over one immutable input set.
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|o| o.id == self.id)
    }
    fn hash_part(&self, h: &mut dyn std::hash::Hasher) {
        h.write_u8(self.id);
    }
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.imports.capacity() * std::mem::size_of::<PublishedValueArena>()
            + self.inputs.capacity() * std::mem::size_of::<SharedPublicationInput>()
    }
    fn retained_shape(&self) -> PublicationShape {
        PublicationShape {
            edges: self.members.len() + self.imports.len() + self.inputs.len(),
            bytes: self.retained_bytes(),
            visits: 1 + self.members.len() + self.imports.len() + self.inputs.len(),
            ..PublicationShape::default()
        }
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &self.inputs
    }
}
fn owner(
    id: u8,
    imports: Vec<PublishedValueArena>,
    inputs: Vec<SharedPublicationInput>,
) -> FunctionEvaluationOwner {
    let part = Arc::new(Part {
        id,
        members: [1, 2],
        imports,
        inputs,
    });
    FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, part.clone(), part).unwrap()
}
#[derive(Debug, Allocative)]
struct Fact([u8; 3]);
impl ImmutablePublicationInput for Fact {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        PublicationShape {
            edges: self.0.len(),
            bytes: std::mem::size_of::<Self>(),
            visits: 1,
            ..PublicationShape::default()
        }
    }
}

#[test]
fn actual_owner_import_and_nested_shared_bodies_keep_first_context_and_guards() {
    let o = owner(0, vec![], vec![]);
    let body = o.intrinsic_shape().edges;
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_owner_edges(EDGE_LIMIT - body).unwrap();
    let bytes = b.bytes;
    let s = refusal(b.audit_owner_shape(&o).unwrap_err());
    assert_eq!(s.context(), PublicationEdgeContext::OwnerBody);
    assert_eq!(s.requested_edges(), body);
    assert_eq!(s.checked_joint_total(), Some(EDGE_LIMIT + 1));
    assert_eq!(s.owner_body_edges(), 0);
    assert!(b.bytes > bytes); // Earlier successful byte/active charges stay paid.
    assert_eq!(b.owner_payload.edges, 0);
    assert!(b.owner_seen.is_empty() && b.owner_roots.is_empty());

    let fact = Arc::new(Fact([1, 2, 3]));
    let weak = Arc::downgrade(&fact);
    let o = owner(1, vec![], vec![SharedPublicationInput::new(fact.clone())]);
    let body = o.intrinsic_shape().edges;
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_owner_edges(EDGE_LIMIT - 1 - body - 1 - 2).unwrap();
    let strong = Arc::strong_count(&fact);
    let s = refusal(b.audit_owner_shape(&o).unwrap_err());
    assert_eq!(s.context(), PublicationEdgeContext::SharedBody);
    assert_eq!(s.requested_edges(), 3);
    assert_eq!(s.checked_joint_total(), Some(EDGE_LIMIT + 1));
    assert_eq!((s.owner_body_edges(), s.shared_body_edges()), (body, 0));
    assert_eq!(b.owner_payload.edges, body);
    assert!(b.shared_seen.is_empty() && b.owner_seen.is_empty() && b.owner_roots.is_empty());
    assert_eq!(Arc::strong_count(&fact), strong);
    drop((o, fact, b));
    assert!(weak.upgrade().is_none());

    let arena = prior();
    let weak = Arc::downgrade(&arena.0);
    let o = owner(2, vec![arena.clone()], vec![]);
    let body = o.intrinsic_shape().edges;
    let imported = arena.0.local_shape.edges;
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_owner_edges(EDGE_LIMIT - 1 - body - 1 - imported + 1)
        .unwrap();
    let s = refusal(b.audit_owner_shape(&o).unwrap_err());
    assert_eq!(s.context(), PublicationEdgeContext::ImportBody);
    assert_eq!(s.requested_edges(), imported);
    assert_eq!(s.checked_joint_total(), Some(EDGE_LIMIT + 1));
    assert_eq!((s.owner_body_edges(), s.shared_body_edges()), (body, 0));
    assert_eq!(s.imported_edges(), 0);
    assert_eq!(b.import_visits, 0);
    assert!(
        b.imports.is_empty()
            && b.closure_seen.is_empty()
            && b.owner_seen.is_empty()
            && b.owner_roots.is_empty()
    );
    assert!(b.charge_visit().is_err());
    drop((o, arena));
    assert!(weak.upgrade().is_some()); // Existing active import guard stays paid.
    drop(b);
    assert!(weak.upgrade().is_none());
}

#[test]
fn decoration_classification_display_and_current_enum_layout_stay_bounded() {
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
    let mut b = PublishedValueArenaBuilder::new();
    let error = b.charge_owner_edges(EDGE_LIMIT + 1).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("published value: owner edges admission refused:")
    );
    assert!(error.to_string().len() < 1024);
    let error = error
        .with_edge_context(PublicationEdgeContext::ImportBody)
        .with_edge_context(PublicationEdgeContext::OwnerBody)
        .with_byte_context(PublicationByteContext::SharedBody);
    assert_eq!(refusal(error).context(), PublicationEdgeContext::ImportBody);
    for error in [
        PublishedValueError::Limit("owner edges"),
        PublishedValueError::Invalid("row"),
        PublishedValueError::Unsupported("kind"),
    ] {
        assert_eq!(
            error
                .clone()
                .with_edge_context(PublicationEdgeContext::OwnerBody),
            error
        );
        assert_eq!(error.to_string(), format!("published value: {error:?}"));
    }
    let mut b = PublishedValueArenaBuilder::new();
    let byte = b.charge_bytes(BYTE_LIMIT + 1).unwrap_err();
    assert!(byte.is_limit());
    assert_eq!(
        byte.clone()
            .with_edge_context(PublicationEdgeContext::SharedBody),
        byte
    );
    assert!(PublishedValueError::Limit("other").is_limit());
    assert!(!PublishedValueError::Invalid("row").is_limit());
    assert!(!PublishedValueError::Unsupported("kind").is_limit());
}
