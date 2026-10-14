use super::*;

#[test]
fn low_leaf_high_node_store_refuses_before_any_node_or_payload_visit() {
    let rows = (0..64)
        .map(|_| DenseDepsetNodeInput::<u8, ()> {
            order: DepsetOrder::Default,
            row: DenseDepsetRowSource::Successors(vec![]),
            depth: 0,
            metadata: (),
        })
        .collect();
    let root = Depset::from_dense_nodes(rows).unwrap().pop().unwrap();
    let mut charges = 0;
    let mut payload = 0;
    let result = root.visit_storage_bounded(|event| match event {
        DepsetStorageEvent::Charge(c) => {
            charges += 1;
            if c.nodes > 8 { Err("nodes") } else { Ok(()) }
        }
        _ => {
            payload += 1;
            Ok(())
        }
    });
    assert_eq!(result, Err("nodes"));
    assert_eq!(charges, 1);
    assert_eq!(payload, 0);
}

// Retained foreign rows need physical closure inspection even when they are not
// selected by the empty logical root. Empty foreign ranges remain valid rows.
fn foreign_chain(count: usize) -> Depset<u8> {
    let mut root = Depset::empty();
    for _ in 0..count {
        root = Depset {
            root: 0,
            store: Arc::new(DenseDepsetStore {
                nodes: vec![DenseNode {
                    order: DepsetOrder::Default,
                    row: DenseRowRef::Local(DenseRange { start: 0, len: 0 }),
                    depth: 0,
                    metadata: (),
                }]
                .into_boxed_slice(),
                successors: Box::new([]),
                leaves: Box::new([]),
                external_depsets: Box::new([]),
                external_rows: vec![DenseExternalRow {
                    store: root.store.clone(),
                    range: DenseRange { start: 0, len: 0 },
                }]
                .into_boxed_slice(),
            }),
        };
    }
    root
}
#[test]
fn foreign_store_growth_is_charged_before_pending_or_seen_insertion() {
    let root = foreign_chain(64);
    let mut nodes = 0;
    let mut inspected = 0;
    let mut charges = 0;
    let result = root.visit_storage_bounded(|event| match event {
        DepsetStorageEvent::Charge(c) => {
            charges += 1;
            nodes += c.nodes;
            if nodes > 8 { Err("nodes") } else { Ok(()) }
        }
        DepsetStorageEvent::Node { .. } => {
            inspected += 1;
            Ok(())
        }
        DepsetStorageEvent::Value { .. } => panic!("empty physical store has no leaf"),
    });
    assert_eq!(result, Err("nodes"));
    assert_eq!(charges, 9);
    assert_eq!(inspected, 8);
}
#[test]
fn physical_unselected_leaves_and_depth_metadata_are_not_hidden() {
    let mut rows = vec![DenseDepsetNodeInput {
        order: DepsetOrder::Default,
        row: DenseDepsetRowSource::Successors(vec![DenseDepsetInput::Direct("hidden")]),
        depth: 1,
        metadata: (),
    }];
    for i in 1..17 {
        rows.push(DenseDepsetNodeInput {
            order: DepsetOrder::Default,
            row: DenseDepsetRowSource::Successors(vec![DenseDepsetInput::Local(i - 1)]),
            depth: i as usize + 1,
            metadata: (),
        });
    }
    rows.push(DenseDepsetNodeInput {
        order: DepsetOrder::Default,
        row: DenseDepsetRowSource::Successors(vec![]),
        depth: 0,
        metadata: (),
    });
    let root = Depset::from_dense_nodes(rows).unwrap().pop().unwrap();
    assert!(root.is_empty());
    let mut values = vec![];
    root.visit_storage_bounded(|event| {
        if let DepsetStorageEvent::Value { value, .. } = event {
            values.push(*value);
        }
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(values, ["hidden"]);
    let mut payload = 0;
    let result = root.visit_storage_bounded(|event| match event {
        DepsetStorageEvent::Node { depth, .. } if depth > 8 => Err("depth"),
        DepsetStorageEvent::Value { .. } => {
            payload += 1;
            Ok(())
        }
        _ => Ok(()),
    });
    assert_eq!(result, Err("depth"));
    assert_eq!(payload, 0);
}

#[test]
fn shared_session_retains_stores_deduplicates_roots_and_replays_deeper_context() {
    let leaf = Arc::new(());
    let weak = Arc::downgrade(&leaf);
    let roots = Depset::from_dense_nodes(vec![
        DenseDepsetNodeInput {
            order: DepsetOrder::Default,
            row: DenseDepsetRowSource::Successors(vec![DenseDepsetInput::Direct(leaf)]),
            depth: 1,
            metadata: (),
        },
        DenseDepsetNodeInput {
            order: DepsetOrder::Default,
            row: DenseDepsetRowSource::Successors(vec![DenseDepsetInput::Local(0)]),
            depth: 2,
            metadata: (),
        },
    ])
    .unwrap();
    let mut seen = DepsetStorageVisitSet::new();
    let mut nodes = 0;
    let mut values = 0;
    roots[0]
        .visit_storage_bounded_with(&mut seen, 0, |event| {
            match event {
                DepsetStorageEvent::Charge(c) => nodes += c.nodes,
                DepsetStorageEvent::Value { .. } => values += 1,
                _ => {}
            }
            Ok::<_, &str>(())
        })
        .unwrap();
    assert_eq!((nodes, values, seen.len()), (2, 1, 1));
    roots[1]
        .visit_storage_bounded_with(&mut seen, 0, |event| {
            match event {
                DepsetStorageEvent::Charge(c) => nodes += c.nodes,
                DepsetStorageEvent::Value { .. } => values += 1,
                _ => {}
            }
            Ok::<_, &str>(())
        })
        .unwrap();
    assert_eq!((nodes, values, seen.len()), (2, 1, 1));
    let error = roots[1].visit_storage_bounded_with(&mut seen, 4, |event| match event {
        DepsetStorageEvent::Charge(c) => {
            assert_eq!(c.nodes, 0);
            Ok(())
        }
        DepsetStorageEvent::Value { depth, .. } => {
            assert_eq!(depth, 6);
            Err("nested payload depth")
        }
        _ => Ok(()),
    });
    assert_eq!(
        error,
        Err(DepsetStorageVisitError::Visitor("nested payload depth"))
    );
    let error = roots[0].visit_storage_bounded_with(&mut seen, 0, |_| -> Result<(), &str> {
        panic!("poisoned session must not call visitor")
    });
    assert_eq!(error, Err(DepsetStorageVisitError::Poisoned));
    drop(roots);
    assert!(weak.upgrade().is_some());
    drop(seen);
    assert!(weak.upgrade().is_none());
}
