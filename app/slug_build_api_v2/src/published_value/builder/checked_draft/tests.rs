use super::*;

#[derive(Debug)]
struct Originals {
    roots: usize,
    imports: Vec<PublishedValueArena>,
}
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        self.roots
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
}
fn binding(b: &PublishedValueArenaBuilder, roots: usize) -> Arc<Originals> {
    Arc::new(Originals {
        roots,
        imports: b.imports().to_vec(),
    })
}
fn cell(b: &mut PublishedValueArenaBuilder) -> PublishedNodeId {
    let id = b.reserve().unwrap();
    b.set_node(id, PublishedNode::Cell { contents: None })
        .unwrap();
    id
}
fn builder(count: usize) -> PublishedValueArenaBuilder {
    let mut b = PublishedValueArenaBuilder::new();
    let id = cell(&mut b);
    b.append_edges(&vec![PublishedEdge::Local(id); count])
        .unwrap();
    b
}
fn seal(b: PublishedValueArenaBuilder) -> PublishedValueArena {
    let originals = binding(&b, 0);
    b.seal(originals).unwrap()
}
fn raw(
    b: &PublishedValueArenaBuilder,
) -> Result<retained_facts::LocalRetainedFacts, PublishedValueError> {
    retained_facts::local_visits(
        &b.rows,
        &b.edges,
        &b.slots,
        &b.imports,
        b.owner_roots.len(),
        0,
        &mut 0,
        &mut 0,
        &mut 0,
        &mut false,
    )
    .map(|(facts, _)| facts)
}

#[test]
fn real_unused_backing_crosses_raw_work_limit_and_seals_with_every_fact() {
    let b = builder(NODE_LIMIT);
    assert_eq!(
        b.charged_visits(),
        1,
        "one real reserved Cell, no work padding"
    );
    assert_eq!(b.rows.len(), 1);
    assert_eq!(b.edges.len(), NODE_LIMIT);
    assert!(matches!(
        raw(&b),
        Err(PublishedValueError::Limit("retained certificate work"))
    ));
    let arena = seal(b);
    assert_eq!(
        arena.shape().visits,
        NODE_LIMIT + 2,
        "row + ALL backing + None slot"
    );
    assert_eq!(arena.0.local_shape.visits, NODE_LIMIT + 2);
    assert_eq!(arena.shape().nodes, 1);
    assert_eq!(arena.shape().edges, NODE_LIMIT);
    assert_eq!(arena.shape().depth, 0);
    assert!(arena.shape().bytes < 64 * 1024 * 1024);
    assert_eq!(arena.0.edges.len(), NODE_LIMIT);
    assert!(
        arena
            .0
            .edges
            .iter()
            .all(|e| *e == PublishedEdge::Local(PublishedNodeId::new(0).unwrap()))
    );
    assert!(arena.0.original_slots[0].is_none());
    assert!(arena.imports().is_empty() && arena.owners().is_empty() && arena.sources().is_empty());
    assert_eq!(arena.originals().root_count(), 0);
}

#[test]
fn small_raw_full_counts_and_semantic_equality_survive_checked_seal() {
    for count in [0, 1, 17] {
        let b = builder(count);
        let full = raw(&b).unwrap();
        assert_eq!((full.total, full.source, full.native), (2 + count, 0, 0));
        let left = seal(b);
        let right = seal(builder(count));
        assert_eq!(left.shape().visits, full.total);
        assert_ne!(
            left.identity(),
            right.identity(),
            "foreign actual arenas remain distinct"
        );
        let id = PublishedNodeId::new(0).unwrap();
        assert!(
            left.value(id)
                .unwrap()
                .publication_eq(&right.value(id).unwrap())
        );
    }
    let mut b = builder(3);
    let id = b.reserve().unwrap();
    b.set_node(
        id,
        PublishedNode::Cell {
            contents: Some(PublishedEdge::Local(PublishedNodeId::new(0).unwrap())),
        },
    )
    .unwrap();
    let full = raw(&b).unwrap();
    assert_eq!(
        full.total, 8,
        "two rows, three stored edges, one inline, two slots"
    );
    let arena = seal(b);
    assert_eq!(arena.shape().visits, 8);
    assert!(
        !arena
            .value(PublishedNodeId::new(0).unwrap())
            .unwrap()
            .publication_eq(&arena.value(id).unwrap())
    );
}

#[test]
fn fixed_view_reads_are_prepaid_and_actual_type_storage_is_bounded() {
    use std::mem::size_of;
    assert_eq!(size_of::<CheckedDraft<'_>>(), 6 * size_of::<usize>());
    let expected = 3 * size_of::<CheckedDraft<'_>>() + 8 * size_of::<usize>();
    let (mut work, mut bytes, mut scratch, mut failed) = (0, 0, 0, false);
    retained_facts::prepare_checked_draft(&mut work, &mut bytes, &mut scratch, &mut failed)
        .unwrap();
    assert_eq!(
        (work, bytes, scratch, failed),
        (8, expected, expected, false)
    );
    for (remaining, ok) in [(8, true), (7, false)] {
        let (mut work, mut bytes, mut scratch, mut failed) = (NODE_LIMIT - remaining, 0, 0, false);
        let result =
            retained_facts::prepare_checked_draft(&mut work, &mut bytes, &mut scratch, &mut failed);
        assert_eq!(result.is_ok(), ok);
        assert_eq!(work, NODE_LIMIT);
        assert_eq!(failed, !ok);
        assert_eq!(
            (bytes, scratch),
            if ok { (expected, expected) } else { (0, 0) }
        );
    }
}

#[path = "validation.rs"]
mod validation;

#[path = "slot_count.rs"]
mod slot_count;
