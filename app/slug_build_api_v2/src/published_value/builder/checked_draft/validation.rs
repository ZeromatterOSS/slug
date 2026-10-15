use super::*;

#[test]
fn invalid_unused_edges_never_commit_and_growth_preserves_original_domains() {
    let mut b = builder(1);
    let before = (b.edges.clone(), b.charged_bytes(), b.charged_visits());
    let valid = PublishedEdge::Local(PublishedNodeId::new(0).unwrap());
    let invalid = PublishedEdge::Local(PublishedNodeId::new(1).unwrap());
    assert!(matches!(
        b.append_edges(&[valid, invalid]),
        Err(PublishedValueError::Invalid("local edge"))
    ));
    assert_eq!(
        (b.edges.clone(), b.charged_bytes(), b.charged_visits()),
        before
    );
    assert!(!b.failed_budget, "unchanged invalid-append contract");
    assert!(
        b.append_edges(&[PublishedEdge::Imported {
            arena: PublishedImportId::new(0).unwrap(),
            node: PublishedNodeId::new(0).unwrap()
        }])
        .is_err()
    );
    assert_eq!(b.edges, before.0);
    let prior = seal(builder(0));
    let foreign = seal(builder(0));
    let weak = Arc::downgrade(&prior.0);
    let import = b.import(prior.clone()).unwrap();
    let count = b.edges.len();
    assert!(
        b.append_edges(&[PublishedEdge::Imported {
            arena: import,
            node: PublishedNodeId::new(1).unwrap()
        }])
        .is_err()
    );
    assert_eq!(b.edges.len(), count);
    let imported = PublishedEdge::Imported {
        arena: import,
        node: PublishedNodeId::new(0).unwrap(),
    };
    b.append_edges(&[imported]).unwrap();
    let another = b.import(foreign.clone()).unwrap();
    assert_ne!(another, import);
    cell(&mut b);
    b.append_edges(&[valid]).unwrap();
    let arena = seal(b);
    assert_eq!(arena.0.edges.as_ref(), &[valid, imported, valid]);
    assert!(Arc::ptr_eq(&arena.imports()[import.index()].0, &prior.0));
    drop((prior, foreign));
    assert!(weak.upgrade().is_some());
    drop(arena);
    assert!(weak.upgrade().is_none());
}

#[test]
fn incomplete_rows_late_slots_and_projection_cycles_still_refuse_and_release() {
    let prior = seal(builder(0));
    let weak = Arc::downgrade(&prior.0);
    let mut b = builder(0);
    b.import(prior.clone()).unwrap();
    let unfinished = b.reserve().unwrap();
    b.append_edges(&[PublishedEdge::Local(unfinished)]).unwrap();
    let originals = binding(&b, 0);
    let binding_weak = Arc::downgrade(&originals);
    drop(prior);
    assert!(matches!(
        b.seal(originals),
        Err(PublishedValueError::Invalid("incomplete row"))
    ));
    assert!(weak.upgrade().is_none() && binding_weak.upgrade().is_none());

    let mut b = builder(0);
    b.set_original_slot(
        PublishedNodeId::new(0).unwrap(),
        PublishedRootSlot::new(0).unwrap(),
    )
    .unwrap();
    let originals = binding(&b, 0);
    assert!(matches!(
        b.seal(originals),
        Err(PublishedValueError::Invalid("original slot"))
    ));

    let mut b = PublishedValueArenaBuilder::new();
    let a = b.reserve().unwrap();
    let z = b.reserve().unwrap();
    let ar = b.append_edges(&[PublishedEdge::Local(z)]).unwrap();
    let zr = b.append_edges(&[PublishedEdge::Local(a)]).unwrap();
    b.set_node(a, PublishedNode::List(ar)).unwrap();
    b.set_node(z, PublishedNode::List(zr)).unwrap();
    b.set_original_slot(a, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    b.set_original_slot(z, PublishedRootSlot::new(1).unwrap())
        .unwrap();
    let originals = binding(&b, 2);
    assert!(matches!(
        b.seal(originals),
        Err(PublishedValueError::Unsupported("cyclic native projection"))
    ));
}
