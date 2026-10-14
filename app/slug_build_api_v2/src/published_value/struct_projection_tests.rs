//! Named struct projection retains descriptor order and original alias edges.
use super::*;

#[test]
fn struct_projection_matches_names_without_reordering_descriptor_or_splitting_aliases() {
    let mut b = PublishedValueArenaBuilder::new();
    let scalar = atom(&mut b, 7);
    let mut lists = vec![];
    for _ in 0..3 {
        let id = b.reserve().unwrap();
        let edges = b.append_edges(&[PublishedEdge::Local(scalar)]).unwrap();
        b.set_node(id, PublishedNode::List(edges)).unwrap();
        b.set_original_slot(id, PublishedRootSlot::new(id.index()).unwrap())
            .unwrap();
        lists.push(id);
    }
    let root = b.reserve().unwrap();
    let edges = b
        .append_edges(
            &lists
                .iter()
                .copied()
                .map(PublishedEdge::Local)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    b.set_node(
        root,
        PublishedNode::Struct {
            names: Arc::from(["zeta".into(), "alpha".into(), "middle".into()]),
            values: edges,
        },
    )
    .unwrap();
    b.set_original_slot(root, PublishedRootSlot::new(root.index()).unwrap())
        .unwrap();
    let arena = seal(b, 5);
    let root = arena.value(root).unwrap();
    let scalar = AnalysisValue::with_original_origin(
        AnalysisValue::integer(7),
        arena.value(scalar).unwrap(),
    )
    .unwrap();
    let values = lists
        .into_iter()
        .map(|id| {
            AnalysisValue::with_original_origin(
                AnalysisValue::list(vec![scalar.clone()]),
                arena.value(id).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(values[0], values[1]); // Equal contents still have distinct origins.
    let fields = |a, b, c| AnalysisValue::strukt([("alpha", a), ("middle", b), ("zeta", c)]);
    assert!(
        AnalysisValue::with_original_origin(
            fields(values[1].clone(), values[2].clone(), values[0].clone()),
            root.clone()
        )
        .is_ok()
    );
    assert!(
        AnalysisValue::with_original_origin(
            fields(values[0].clone(), values[2].clone(), values[1].clone()),
            root.clone()
        )
        .is_err()
    );
    assert!(
        AnalysisValue::with_original_origin(
            AnalysisValue::strukt([
                ("alpha", values[1].clone()),
                ("middle", values[2].clone()),
                ("other", values[0].clone())
            ]),
            root.clone()
        )
        .is_err()
    );
    assert!(
        AnalysisValue::with_original_origin(
            AnalysisValue::strukt([("alpha", values[1].clone()), ("zeta", values[0].clone())]),
            root.clone()
        )
        .is_err()
    );
    let PublishedNode::Struct { names, .. } = root.node() else {
        panic!("struct descriptor");
    };
    assert_eq!(
        names.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        ["zeta", "alpha", "middle"]
    );
}
