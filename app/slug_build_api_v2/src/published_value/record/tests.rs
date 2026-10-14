use std::sync::Arc;

use super::*;
use crate::published_value::*;

#[derive(Debug)]
struct Originals {
    roots: usize,
    imports: Vec<PublishedValueArena>,
}
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn root_count(&self) -> usize {
        self.roots
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
}
fn seal(b: PublishedValueArenaBuilder) -> Result<PublishedValueArena, PublishedValueError> {
    let originals = Originals {
        roots: b.rows.len(),
        imports: b.imports.clone(),
    };
    b.seal(Arc::new(originals))
}
fn arena(record: bool) -> (PublishedValueArena, PublishedNodeId, usize) {
    let mut b = PublishedValueArenaBuilder::new();
    let id = if record {
        b.begin_record(0).unwrap().reserve().unwrap()
    } else {
        b.observe_depth(0).unwrap();
        b.charge_visit().unwrap();
        b.reserve().unwrap()
    };
    let empty = b.append_edges(&[]).unwrap();
    b.set_node(id, PublishedNode::Tuple(empty)).unwrap();
    b.set_original_slot(id, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    let paid = b.charged_visits();
    (seal(b).unwrap(), id, paid)
}
fn shape(s: PublicationShape) -> [usize; 5] {
    [s.nodes, s.edges, s.bytes, s.visits, s.depth]
}

#[test]
fn record_paid_unit_preserves_last_visit_and_sealed_import_certificate() {
    let mut last = PublishedValueArenaBuilder::new();
    last.visits = NODE_LIMIT - 1;
    assert_eq!(last.begin_record(0).unwrap().reserve().unwrap().index(), 0);
    assert_eq!(last.charged_visits(), NODE_LIMIT);
    assert!(matches!(
        last.begin_record(0),
        Err(PublishedValueError::Limit("visits"))
    ));
    assert_eq!(last.rows.len(), 1);

    let (record, a, record_paid) = arena(true);
    let (ordinary, b, ordinary_paid) = arena(false);
    assert_eq!((record_paid, ordinary_paid), (1, 2));
    assert_eq!(shape(record.shape()), shape(ordinary.shape()));
    assert!(
        record
            .value(a)
            .unwrap()
            .publication_eq(&ordinary.value(b).unwrap())
    );
    let mut imported_record = PublishedValueArenaBuilder::new();
    let mut imported_ordinary = PublishedValueArenaBuilder::new();
    imported_record.import(record.clone()).unwrap();
    imported_ordinary.import(ordinary.clone()).unwrap();
    assert_eq!(imported_record.import_visits, record.shape().visits);
    assert_eq!(imported_ordinary.import_visits, ordinary.shape().visits);
    assert_eq!(
        imported_record.charged_visits(),
        imported_ordinary.charged_visits()
    );
}

#[test]
fn public_reserve_order_and_record_depth_abort_keep_failed_state_closed() {
    let mut ordinary = PublishedValueArenaBuilder::new();
    ordinary.owner_nodes = NODE_LIMIT;
    ordinary.visits = NODE_LIMIT;
    assert_eq!(ordinary.reserve(), Err(PublishedValueError::Limit("nodes")));
    assert_eq!(ordinary.charged_visits(), NODE_LIMIT);
    assert!(ordinary.rows.is_empty());

    let mut record = PublishedValueArenaBuilder::new();
    record.owner_nodes = NODE_LIMIT;
    assert_eq!(
        record.begin_record(0).unwrap().reserve(),
        Err(PublishedValueError::Limit("nodes"))
    );
    assert_eq!(record.charged_visits(), 1);
    assert!(record.rows.is_empty());
    assert!(record.reserve().is_err());

    let mut depth = PublishedValueArenaBuilder::new();
    assert!(matches!(
        depth.begin_record(DEPTH_LIMIT + 1),
        Err(PublishedValueError::Limit("depth"))
    ));
    assert_eq!(depth.charged_visits(), 0);
    assert!(depth.rows.is_empty());
    assert!(depth.begin_record(0).is_err());
    assert!(depth.reserve().is_err());
    assert!(seal(depth).is_err());

    let mut aborted = PublishedValueArenaBuilder::new();
    aborted.begin_record(0).unwrap().abort();
    assert_eq!(aborted.charged_visits(), 1);
    assert!(aborted.rows.is_empty());
    assert!(aborted.begin_record(0).is_err());
    assert!(aborted.reserve().is_err());
    assert!(seal(aborted).is_err());
}

#[test]
fn record_byte_refusal_and_successful_drop_preserve_storage_and_owner() {
    let mut hit = PublishedValueArenaBuilder::new();
    {
        let mut encounter = hit.begin_record(2).unwrap();
        encounter.charge_bytes(7).unwrap();
    }
    assert_eq!((hit.charged_visits(), hit.charged_bytes()), (1, 7));
    assert_eq!(hit.maximum_depth, 2);
    assert!(hit.rows.is_empty());
    assert_eq!(hit.reserve().unwrap().index(), 0);
    assert_eq!(hit.charged_visits(), 2);

    let bytes = std::mem::size_of::<PublishedNode>()
        + std::mem::size_of::<Option<PublishedRootSlot>>()
        + crate::analysis_value::published_projection_forecast();
    let mut exact = PublishedValueArenaBuilder::new();
    exact.charge_bytes(BYTE_LIMIT - bytes).unwrap();
    exact.begin_record(0).unwrap().reserve().unwrap();
    assert_eq!(exact.charged_bytes(), BYTE_LIMIT);
    assert_eq!((exact.rows.len(), exact.slots.len()), (1, 1));

    let mut short = PublishedValueArenaBuilder::new();
    short.charge_bytes(BYTE_LIMIT - bytes + 1).unwrap();
    let error = short.begin_record(0).unwrap().reserve().unwrap_err();
    let PublishedValueError::ByteBudget(refusal) = error else {
        panic!("expected byte admission refusal");
    };
    assert_eq!(refusal.context(), PublicationByteContext::Local);
    assert_eq!(refusal.requested_bytes(), bytes);
    assert_eq!(refusal.checked_total_bytes(), Some(BYTE_LIMIT + 1));
    assert!(!refusal.prior_poisoned());
    assert_eq!((short.rows.capacity(), short.slots.capacity()), (0, 0));
    assert_eq!(short.charged_visits(), 1);
    assert!(short.begin_record(0).is_err());
    assert!(short.reserve().is_err());
    assert!(seal(short).is_err());

    let mut refused = PublishedValueArenaBuilder::new();
    let mut encounter = refused.begin_record(0).unwrap();
    let error = encounter.charge_bytes(BYTE_LIMIT + 1).unwrap_err();
    let PublishedValueError::ByteBudget(refusal) = error else {
        panic!("expected byte admission refusal");
    };
    assert_eq!(refusal.used_bytes(), 0);
    assert_eq!(refusal.requested_bytes(), BYTE_LIMIT + 1);
    assert_eq!(refusal.checked_total_bytes(), Some(BYTE_LIMIT + 1));
    assert!(!refusal.prior_poisoned());
    encounter.abort();
    assert!(refused.rows.is_empty());
    assert!(refused.reserve().is_err());
}
