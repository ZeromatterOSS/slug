use super::*;

fn cyclic(reverse: bool, split: bool, default: i64) -> (CallableGraph, CallableNodeId) {
    let mut b = CallableGraphBuilder::new();
    let first = b.reserve().unwrap();
    let second = b.reserve().unwrap();
    let third = b.reserve().unwrap();
    let (root, cell) = if reverse {
        (second, first)
    } else {
        (first, second)
    };
    let values = b
        .append_edges(&[cell, if split { third } else { cell }])
        .unwrap();
    b.set_node(root, CallableNode::Tuple(values)).unwrap();
    b.set_node(
        cell,
        CallableNode::Cell {
            contents: Some(root),
        },
    )
    .unwrap();
    b.set_node(third, CallableNode::Native(AnalysisValue::integer(default)))
        .unwrap();
    (b.finish().unwrap(), root)
}
#[test]
fn cyclic_graph_equality_preserves_aliases_independent_of_node_numbering() {
    let (a, ar) = cyclic(false, false, 1);
    let (b, br) = cyclic(true, false, 2);
    assert!(a.publication_eq(ar, &b, br)); // unreachable metadata is not a root input
    let (c, cr) = cyclic(true, true, 1);
    assert!(!a.publication_eq(ar, &c, cr));
}
fn intrinsic(default: i64) -> (CallableGraph, CallableNodeId) {
    let mut b = CallableGraphBuilder::new();
    let root = b.reserve().unwrap();
    let d = b.reserve().unwrap();
    b.set_node(d, CallableNode::Native(AnalysisValue::integer(default)))
        .unwrap();
    b.set_node(
        root,
        CallableNode::Intrinsic(CallableIntrinsic {
            api: NativeCallableApiId::new("native.len", 1).unwrap(),
            parameters: Arc::from([CallableParameter {
                name: "x".into(),
                mode: CallableParameterMode::PositionalOrNamed,
                requirement: CallableParameterRequirement::Defaulted(d),
                annotation: None,
            }]),
            return_annotation: None,
            receiver: None,
        }),
    )
    .unwrap();
    (b.finish().unwrap(), root)
}
#[test]
fn native_parameter_defaults_are_semantic_and_graph_limits_are_checked() {
    let (a, ar) = intrinsic(1);
    let (b, br) = intrinsic(1);
    let (c, cr) = intrinsic(2);
    assert!(a.publication_eq(ar, &b, br));
    assert!(!a.publication_eq(ar, &c, cr));
    let mut builder = CallableGraphBuilder::new();
    let id = builder.reserve().unwrap();
    assert!(matches!(
        builder.append_edges(&[CallableNodeId::new(100).unwrap()]),
        Err(CallableGraphError::InvalidNode(100))
    ));
    assert!(matches!(
        builder.observe_depth(CALLABLE_GRAPH_DEPTH_LIMIT + 1),
        Err(CallableGraphError::Limit("depth"))
    ));
    assert!(matches!(
        builder.charge_bytes(CALLABLE_GRAPH_BYTE_LIMIT),
        Err(CallableGraphError::Limit("bytes"))
    ));
    assert!(
        matches!(builder.finish(),Err(CallableGraphError::UninitializedNode(n)) if n==id.index())
    );
}
