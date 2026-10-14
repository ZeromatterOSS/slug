use super::super::import_accounting::tests::finish;
use super::*;
use crate::AnalysisValue;

fn native_builder() -> PublishedValueArenaBuilder {
    let mut b = PublishedValueArenaBuilder::new();
    let id = b.reserve().unwrap();
    let value = b.native_atom(AnalysisValue::none()).unwrap();
    b.set_node(id, PublishedNode::Native(value)).unwrap();
    b.set_original_slot(id, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    b
}
fn shape(s: PublicationShape) -> [usize; 5] {
    [s.nodes, s.edges, s.bytes, s.visits, s.depth]
}
fn with_dropped_scratch(extra: bool) -> (PublishedValueArena, usize) {
    let mut b = native_builder();
    if extra {
        let bytes = std::mem::size_of::<Vec<u64>>() + 8 * std::mem::size_of::<u64>();
        b.producer_scratch().charge_bytes(bytes).unwrap();
        let mut temporary = Vec::with_capacity(8);
        temporary.extend(0u64..8);
        assert_eq!(temporary.len(), 8);
        drop(temporary);
    }
    let live = b.charged_bytes();
    (finish(b).unwrap(), live)
}

#[test]
fn scratch_is_fully_paid_and_mixed_admission_is_atomic_before_growth() {
    for (remaining, succeeds) in [(16, true), (15, false)] {
        let mut b = PublishedValueArenaBuilder::new();
        b.charge_bytes(BYTE_LIMIT - remaining).unwrap();
        let mut values = Vec::<u8>::new();
        let result = b.producer_scratch().charge_parts(8, 8);
        if result.is_ok() {
            values.reserve_exact(8);
        }
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(b.producer_scratch_bytes, if succeeds { 8 } else { 0 });
        assert_eq!(
            b.charged_bytes(),
            if succeeds {
                BYTE_LIMIT
            } else {
                BYTE_LIMIT - 15
            }
        );
        if !succeeds {
            assert_eq!(values.capacity(), 0);
        }
        let prior = (b.charged_bytes(), b.producer_scratch_bytes);
        assert!(b.producer_scratch().charge_bytes(1).is_err());
        assert_eq!((b.charged_bytes(), b.producer_scratch_bytes), prior);
        assert!(b.producer_scratch().charge_bytes(0).is_err());
        assert!(b.charge_bytes(0).is_err());
    }
}

#[test]
fn scratch_overflow_and_poison_cannot_commit_or_refund_classification() {
    let mut b = PublishedValueArenaBuilder::new();
    assert_eq!(
        b.producer_scratch().charge_parts(usize::MAX, 1),
        Err(PublishedValueError::Limit("producer scratch bytes"))
    );
    assert_eq!((b.charged_bytes(), b.producer_scratch_bytes), (0, 0));
    assert!(b.charge_bytes(0).is_err());
    assert!(b.producer_scratch().charge_parts(0, 0).is_err());

    let mut b = PublishedValueArenaBuilder::new();
    b.producer_scratch().charge_bytes(BYTE_LIMIT).unwrap();
    assert_eq!(b.charged_bytes(), BYTE_LIMIT);
    assert_eq!(b.producer_scratch_bytes, BYTE_LIMIT);
    // Reuse a genuine empty original binding: the helper's own prepayment
    // unwrap must not run against this deliberately exhausted builder.
    let originals = finish(PublishedValueArenaBuilder::new())
        .unwrap()
        .0
        .originals
        .clone();
    assert!(
        b.seal(originals).is_err(),
        "paid scratch cannot fund seal's new storage"
    );
}

#[test]
fn record_scratch_reborrow_preserves_encounter_reservation_and_abort() {
    let mut b = PublishedValueArenaBuilder::new();
    {
        let mut record = b.begin_record(0).unwrap();
        record.producer_scratch().charge_parts(5, 7).unwrap();
    }
    assert_eq!((b.charged_visits(), b.rows.len()), (1, 0));
    assert_eq!((b.charged_bytes(), b.producer_scratch_bytes), (12, 7));
    let id = {
        let mut record = b.begin_record(0).unwrap();
        record.producer_scratch().charge_bytes(3).unwrap();
        record.reserve().unwrap()
    };
    assert_eq!((b.charged_visits(), b.rows.len()), (2, 1));
    b.set_node(id, PublishedNode::Cell { contents: None })
        .unwrap();
    assert!(finish(b).is_ok());

    let mut b = PublishedValueArenaBuilder::new();
    let mut record = b.begin_record(0).unwrap();
    record.producer_scratch().charge_bytes(9).unwrap();
    record.abort();
    assert_eq!((b.charged_bytes(), b.producer_scratch_bytes), (9, 9));
    assert!(b.producer_scratch().charge_bytes(0).is_err());
    assert!(b.rows.is_empty());
}

#[test]
fn dropped_real_scratch_keeps_identical_full_retained_shapes_and_values() {
    let (plain, plain_live) = with_dropped_scratch(false);
    let (extra, extra_live) = with_dropped_scratch(true);
    assert_eq!(
        extra_live - plain_live,
        std::mem::size_of::<Vec<u64>>() + 64
    );
    assert_eq!(shape(plain.shape()), shape(extra.shape()));
    assert_eq!(shape(plain.0.local_shape), shape(extra.0.local_shape));
    let root = PublishedNodeId::new(0).unwrap();
    assert!(
        plain
            .value(root)
            .unwrap()
            .publication_eq(&extra.value(root).unwrap())
    );
    let mut a = PublishedValueArenaBuilder::new();
    let mut b = PublishedValueArenaBuilder::new();
    a.import(plain.clone()).unwrap();
    b.import(extra.clone()).unwrap();
    assert_eq!(a.import_bytes, plain.0.local_shape.bytes);
    assert_eq!(b.import_bytes, extra.0.local_shape.bytes);
    assert_eq!(a.charged_bytes(), b.charged_bytes());
    assert_eq!(
        shape(finish(a).unwrap().shape()),
        shape(finish(b).unwrap().shape())
    );
}

#[test]
fn corrected_certificate_survives_diamonds_and_actual_guard_release() {
    let (prior, _) = with_dropped_scratch(true);
    let weak = Arc::downgrade(&prior.0);
    let branch = |prior: &PublishedValueArena| {
        let mut b = PublishedValueArenaBuilder::new();
        b.import(prior.clone()).unwrap();
        finish(b).unwrap()
    };
    let a = branch(&prior);
    let c = branch(&prior);
    let mut b = PublishedValueArenaBuilder::new();
    b.import(a.clone()).unwrap();
    b.import(c.clone()).unwrap();
    assert_eq!(b.closure_seen.len(), 3);
    assert_eq!(
        b.import_bytes,
        prior.0.local_shape.bytes + a.0.local_shape.bytes + c.0.local_shape.bytes
    );
    let joined = finish(b).unwrap();
    assert!(
        joined.imports()[0].imports()[0].identity() == joined.imports()[1].imports()[0].identity()
    );
    drop(prior);
    drop(a);
    drop(c);
    assert!(weak.upgrade().is_some());
    drop(joined);
    assert!(weak.upgrade().is_none());
}

#[test]
fn default_native_and_mixed_payload_bytes_remain_retained() {
    let mut b = PublishedValueArenaBuilder::new();
    let value = AnalysisValue::string("full retained native payload ".repeat(128));
    let atom = b.native_atom_borrowed(&value).unwrap();
    let native_bytes = b.charged_bytes();
    assert!(native_bytes >= 128 * "full retained native payload ".len());
    assert_eq!(b.producer_scratch_bytes, 0);
    b.producer_scratch().charge_parts(31, 101).unwrap();
    assert_eq!(b.charged_bytes(), native_bytes + 132);
    assert_eq!(b.intrinsic_shape().bytes, native_bytes + 132);
    assert_eq!(b.producer_scratch_bytes, 101);
    b.charge_bytes(17).unwrap();
    assert_eq!(
        b.charged_bytes() - b.producer_scratch_bytes,
        native_bytes + 48
    );
    assert!(atom.value() == &value);
}
