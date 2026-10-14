//! Capture descriptors retain insertion order, not a replacement set evaluator.
use super::*;
fn set(
    b: &mut PublishedValueArenaBuilder,
    members: &[PublishedNodeId],
    is_list: bool,
) -> PublishedNodeId {
    let id = b.reserve().unwrap();
    let edges = members
        .iter()
        .copied()
        .map(PublishedEdge::Local)
        .collect::<Vec<_>>();
    let range = b.append_edges(&edges).unwrap();
    b.set_node(
        id,
        if is_list {
            PublishedNode::List(range)
        } else {
            PublishedNode::Set(range)
        },
    )
    .unwrap();
    b.set_original_slot(id, PublishedRootSlot::new(id.index()).unwrap())
        .unwrap();
    id
}
fn captured(reverse: bool, split: bool, as_list: bool) -> PublishedValueRef {
    let mut b = PublishedValueArenaBuilder::new();
    let one = atom(&mut b, 1);
    let two = atom(&mut b, 2);
    let members = if reverse { [two, one] } else { [one, two] };
    let a = set(&mut b, &members, as_list);
    let other = if split {
        set(&mut b, &members, as_list)
    } else {
        a
    };
    let root = b.reserve().unwrap();
    let range = b
        .append_edges(&[PublishedEdge::Local(a), PublishedEdge::Local(other)])
        .unwrap();
    b.set_node(root, PublishedNode::Tuple(range)).unwrap();
    b.set_original_slot(root, PublishedRootSlot::new(root.index()).unwrap())
        .unwrap();
    seal(b, root.index() + 1).value(root).unwrap()
}
#[test]
fn captured_sets_preserve_order_type_and_shared_occurrence_bijection() {
    let a = captured(false, false, false);
    assert!(a.publication_eq(&captured(false, false, false)));
    assert!(!a.publication_eq(&captured(true, false, false)));
    assert!(!a.publication_eq(&captured(false, true, false)));
    assert!(!a.publication_eq(&captured(false, false, true)));
    let PublishedNode::Tuple(range) = a.node() else {
        panic!("tuple");
    };
    let child = a
        .arena()
        .resolve(a.arena().edges(*range).unwrap()[0])
        .unwrap();
    assert_eq!(child.value_type(), AnalysisValueType::Set);
    assert_eq!(child.value_type().as_str(), "set");
    assert!(AnalysisValue::with_original_origin(AnalysisValue::list([]), child).is_err());
}
#[test]
fn captured_set_edges_containment_cycles_and_depth_are_bounded() {
    let mut b = PublishedValueArenaBuilder::new();
    let root = b.reserve().unwrap();
    assert!(
        b.append_edges(&[PublishedEdge::Local(PublishedNodeId::new(5).unwrap())])
            .is_err()
    );
    let edges = b.append_edges(&[PublishedEdge::Local(root)]).unwrap();
    b.set_node(root, PublishedNode::Set(edges)).unwrap();
    b.set_original_slot(root, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    assert!(
        b.seal(Arc::new(Originals {
            count: 1,
            imports: vec![]
        }))
        .is_err()
    );
    for reverse_ids in [false, true] {
        assert!(deep_set(DEPTH_LIMIT - 1, reverse_ids).is_ok());
        assert!(deep_set(DEPTH_LIMIT, reverse_ids).is_err());
    }
    let mut b = PublishedValueArenaBuilder::new();
    let root = b.reserve().unwrap();
    assert!(
        b.set_node(
            root,
            PublishedNode::Set(PublishedEdgeRange { start: 0, len: 1 })
        )
        .is_err()
    );
}

fn deep_set(
    tuple_count: usize,
    reverse_ids: bool,
) -> Result<PublishedValueArena, PublishedValueError> {
    let mut b = PublishedValueArenaBuilder::new();
    let mut ids = (0..=tuple_count)
        .map(|_| b.reserve().unwrap())
        .collect::<Vec<_>>();
    if reverse_ids {
        ids.reverse();
    }
    let mut child = atom(&mut b, 0);
    for (position, id) in ids.iter().enumerate().rev() {
        let range = b.append_edges(&[PublishedEdge::Local(child)]).unwrap();
        let row = if position == 0 {
            PublishedNode::Set(range)
        } else {
            PublishedNode::Tuple(range)
        };
        b.set_node(*id, row).unwrap();
        b.set_original_slot(*id, PublishedRootSlot::new(id.index()).unwrap())
            .unwrap();
        child = *id;
    }
    b.seal(Arc::new(Originals {
        count: ids.len() + 1,
        imports: vec![],
    }))
}

#[test]
fn imported_capture_containment_depth_preserves_intrinsic_and_complete_shapes() {
    let prior = deep_set(DEPTH_LIMIT - 2, false).unwrap();
    assert_eq!(prior.shape().depth, DEPTH_LIMIT - 1);
    for parent_count in [1, 2] {
        let mut b = PublishedValueArenaBuilder::new();
        let import = b.import(prior.clone()).unwrap();
        let mut edge = PublishedEdge::Imported {
            arena: import,
            node: PublishedNodeId::new(0).unwrap(),
        };
        for _ in 0..parent_count {
            let id = b.reserve().unwrap();
            let range = b.append_edges(&[edge]).unwrap();
            b.set_node(id, PublishedNode::Tuple(range)).unwrap();
            b.set_original_slot(id, PublishedRootSlot::new(id.index()).unwrap())
                .unwrap();
            edge = PublishedEdge::Local(id);
        }
        let result = b.seal(Arc::new(Originals {
            count: parent_count,
            imports: vec![prior.clone()],
        }));
        if parent_count == 1 {
            let arena = result.unwrap();
            assert_eq!(arena.shape().depth, DEPTH_LIMIT);
            assert_eq!(arena.0.local_shape.depth, 1);
        } else {
            assert!(matches!(
                result,
                Err(PublishedValueError::Limit("projection depth"))
            ));
        }
    }
}
