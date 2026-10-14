use super::*;

fn local(index: usize) -> PublishedEdge {
    PublishedEdge::Local(PublishedNodeId::new(index).unwrap())
}
fn empty() -> PublishedNode {
    PublishedNode::Struct {
        names: Arc::from([]),
        values: PublishedEdgeRange { start: 0, len: 0 },
    }
}
fn depset(depth: usize, input: PublishedDepsetInput) -> PublishedNode {
    PublishedNode::Depset(PublishedDepset {
        order: crate::DepsetOrder::Default,
        element_type: AnalysisValueType::Struct,
        depth,
        occurrence: crate::AnalysisDepsetOccurrence::new(),
        successors: Arc::from([input]),
    })
}
fn list(rows: &mut Vec<PublishedNode>, edges: &mut Vec<PublishedEdge>, child: usize) {
    let start = edges.len() as u32;
    edges.push(local(child));
    rows.push(PublishedNode::List(PublishedEdgeRange { start, len: 1 }));
}
fn chain(depth: usize) -> Vec<PublishedNode> {
    // Root-first numbering forces the full typed chain onto the explicit
    // traversal stack; completed bottom-up memo entries cannot mask recursion.
    let mut rows = Vec::new();
    for index in 0..depth {
        rows.push(depset(
            depth - index,
            if index + 1 == depth {
                PublishedDepsetInput::Direct(local(depth))
            } else {
                PublishedDepsetInput::Transitive(local(index + 1))
            },
        ));
    }
    rows.push(empty());
    rows
}

#[test]
fn typed_transitive_depth_is_iterative_and_keeps_ordinary_payload_height() {
    let mut rows = chain(crate::depset::MAX_DEPTH);
    let mut edges = Vec::new();
    assert_eq!(validate(&rows, &edges, &[]).unwrap(), (1, 1));
    // A completed zero-weight suffix must still compose with every enclosing
    // ordinary container, even when numbered before those containers.
    let mut child = 0;
    for _ in 0..DEPTH_LIMIT - 1 {
        list(&mut rows, &mut edges, child);
        child = rows.len() - 1;
    }
    assert_eq!(
        validate(&rows, &edges, &[]).unwrap(),
        (DEPTH_LIMIT, DEPTH_LIMIT)
    );
    let child = rows.len() - 1;
    list(&mut rows, &mut edges, child);
    assert_eq!(
        validate(&rows, &edges, &[]),
        Err(PublishedValueError::Limit("projection depth"))
    );
}

#[test]
fn typed_zero_weight_edges_validate_class_and_detect_active_cycles() {
    let cyclic = vec![
        depset(1, PublishedDepsetInput::Transitive(local(1))),
        depset(1, PublishedDepsetInput::Transitive(local(0))),
    ];
    assert_eq!(
        validate(&cyclic, &[], &[]),
        Err(PublishedValueError::Unsupported("cyclic native projection"))
    );
    let wrong_class = vec![
        empty(),
        depset(1, PublishedDepsetInput::Transitive(local(0))),
    ];
    assert_eq!(
        validate(&wrong_class, &[], &[]),
        Err(PublishedValueError::Invalid("transitive depset"))
    );
    let too_deep = vec![
        empty(),
        depset(
            crate::depset::MAX_DEPTH + 1,
            PublishedDepsetInput::Direct(local(0)),
        ),
    ];
    assert_eq!(
        validate(&too_deep, &[], &[]),
        Err(PublishedValueError::Limit("depset depth"))
    );
}

#[derive(Debug)]
struct Originals(usize);
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        self.0
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &[]
    }
}
fn imported_chain(depth: usize) -> (PublishedValueArena, PublishedNodeId) {
    let mut b = PublishedValueArenaBuilder::new();
    let mut prior = b.reserve().unwrap();
    b.set_node(prior, empty()).unwrap();
    b.set_original_slot(prior, PublishedRootSlot::new(prior.index()).unwrap())
        .unwrap();
    for _ in 0..depth {
        let next = b.reserve().unwrap();
        let range = b.append_edges(&[PublishedEdge::Local(prior)]).unwrap();
        b.set_node(next, PublishedNode::List(range)).unwrap();
        b.set_original_slot(next, PublishedRootSlot::new(next.index()).unwrap())
            .unwrap();
        prior = next;
    }
    (b.seal(Arc::new(Originals(depth + 1))).unwrap(), prior)
}

#[test]
fn weighted_complete_height_preserves_imported_closure_and_scratch_overflow() {
    let (prior, top) = imported_chain(DEPTH_LIMIT);
    let imported = PublishedEdge::Imported {
        arena: PublishedImportId::new(0).unwrap(),
        node: top,
    };
    let rows = [PublishedNode::List(PublishedEdgeRange { start: 0, len: 1 })];
    assert_eq!(
        validate(&rows, &[imported], &[prior]),
        Err(PublishedValueError::Limit("projection depth"))
    );
    assert!(scratch_bytes(usize::MAX, 0).is_none());
    assert!(scratch_bytes(0, usize::MAX).is_none());
    let rows = 3501usize;
    let refs = 3500usize;
    assert_eq!(
        scratch_bytes(rows, refs).unwrap(),
        refs * std::mem::size_of::<WeightedEdge>()
            + (rows + 1) * std::mem::size_of::<usize>()
            + rows * std::mem::size_of::<(u16, u16)>()
            + rows * std::mem::size_of::<Frame>()
            + 4 * std::mem::size_of::<Vec<usize>>()
    );
}

#[test]
fn expanded_adjacency_forecast_counts_shared_range_references_before_growth() {
    let range = PublishedEdgeRange { start: 0, len: 1 };
    let rows = [
        Some(empty()),
        Some(PublishedNode::List(range)),
        Some(PublishedNode::List(range)),
    ];
    let bytes = draft_scratch_bytes(&rows, &[local(0)]).unwrap();
    assert_eq!(bytes, scratch_bytes(3, 2).unwrap());
    assert!(bytes > scratch_bytes(3, 1).unwrap());
    assert!(matches!(
        draft_scratch_bytes(&[None], &[]),
        Err(PublishedValueError::Invalid("incomplete row"))
    ));
}
