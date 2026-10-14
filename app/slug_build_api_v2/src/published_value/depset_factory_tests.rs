use super::*;

#[derive(Debug)]
struct Originals;
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        1
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &[]
    }
}
fn retained(value: AnalysisValue) -> crate::AnalysisDepset {
    crate::AnalysisDepset::new(crate::DepsetOrder::Default, vec![value], vec![]).unwrap()
}

#[test]
fn borrowed_depset_factory_caches_full_strict_audit_and_restores_mode() {
    let retained = retained(AnalysisValue::string("actual retained leaf"));
    let mut ordinary = PublishedValueArenaBuilder::new();
    let owned = ordinary
        .native_atom(AnalysisValue::depset(retained.clone()))
        .unwrap();
    let mut borrowed = PublishedValueArenaBuilder::new();
    borrowed.owner_audit = true;
    let atom = borrowed.depset_atom_borrowed(&retained).unwrap();
    assert!(borrowed.owner_audit);
    assert_eq!(atom.retained_visits(), owned.retained_visits());
    assert!(atom.retained_visits() > 1);
    assert_eq!(
        borrowed.bytes - ordinary.bytes,
        crate::analysis_value::published_projection_forecast()
    );
    match atom.value().kind() {
        crate::AnalysisValueKind::Depset(actual) => assert_eq!(actual, &retained),
        _ => panic!("typed factory changed class"),
    }
}

#[test]
fn borrowed_depset_factory_refuses_before_allocation_and_poison_cannot_smuggle_origins() {
    let mut exhausted = PublishedValueArenaBuilder::new();
    exhausted.owner_audit = true;
    exhausted
        .charge_bytes(BYTE_LIMIT - std::mem::size_of::<NativeCertificate>())
        .unwrap();
    assert!(
        exhausted
            .depset_atom_borrowed(&retained(AnalysisValue::string("leaf")))
            .is_err()
    );
    assert_eq!(
        exhausted.charged_visits(),
        0,
        "allocation admission precedes the full audit"
    );
    assert!(exhausted.owner_audit);
    assert!(exhausted.charge_bytes(0).is_err());

    let mut producer = PublishedValueArenaBuilder::new();
    let id = producer.reserve().unwrap();
    let atom = producer
        .native_atom(AnalysisValue::string("original"))
        .unwrap();
    producer.set_node(id, PublishedNode::Native(atom)).unwrap();
    producer
        .set_original_slot(id, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    let arena = producer.seal(Arc::new(Originals)).unwrap();
    let value = AnalysisValue::with_original_origin(
        AnalysisValue::string("original"),
        arena.value(id).unwrap(),
    )
    .unwrap();
    let mut strict = PublishedValueArenaBuilder::new();
    strict.owner_audit = true;
    assert!(matches!(
        strict.depset_atom_borrowed(&retained(value)),
        Err(PublishedValueError::Invalid("arena-bearing native atom"))
    ));
    assert!(strict.owner_audit);
    assert!(strict.imports().is_empty());
    assert!(strict.charge_visit().is_err());
}

#[test]
fn weighted_validator_scratch_refuses_before_seal_arrays_are_allocated() {
    let mut b = PublishedValueArenaBuilder::new();
    let id = b.reserve().unwrap();
    let range = b.append_edges(&[]).unwrap();
    b.set_node(id, PublishedNode::List(range)).unwrap();
    b.set_original_slot(id, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    let outer = std::mem::size_of::<PublishedValueArenaData>() + 2 * std::mem::size_of::<usize>();
    // The old single packed-color allowance fits exactly; the simultaneous
    // weighted adjacency/offset/frame allowance must refuse before allocation.
    let old_colors = std::mem::size_of::<(u16, u16)>();
    b.charge_bytes(BYTE_LIMIT - b.bytes - outer - old_colors)
        .unwrap();
    assert!(matches!(
        b.seal(Arc::new(Originals)),
        Err(PublishedValueError::ByteBudget(refusal))
            if !refusal.prior_poisoned()
                && refusal.limit_bytes() == BYTE_LIMIT
                && refusal.checked_total_bytes().is_some_and(|total| total > BYTE_LIMIT)
    ));
}
