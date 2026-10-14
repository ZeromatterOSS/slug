use super::*;

fn ordinary(kind: usize) -> AnalysisValue {
    let value = AnalysisValue::string("retained");
    match kind {
        0 => AnalysisValue::list(vec![value]),
        1 => AnalysisValue::tuple(vec![value]),
        2 => AnalysisValue::dictionary([(AnalysisValue::string("key"), value)]).unwrap(),
        3 => AnalysisValue::strukt([("field", value)]),
        _ => unreachable!(),
    }
}

#[test]
fn ordinary_container_aliases_survive_materialization_across_roots_and_calls() {
    for kind in 0..4 {
        let heap = FrozenHeap::new();
        let mut materializer = AnalysisValueMaterializer::new(&heap);
        let shared = ordinary(kind);
        let independent = ordinary(kind);
        let first = materializer.value(&shared).unwrap();
        assert_eq!(
            first.to_value().identity(),
            materializer
                .value(&shared.dupe())
                .unwrap()
                .to_value()
                .identity()
        );
        assert_ne!(
            first.to_value().identity(),
            materializer
                .value(&independent)
                .unwrap()
                .to_value()
                .identity()
        );
        let graph = AnalysisValue::tuple(vec![shared.dupe(), shared, independent]);
        let frozen = materializer.value(&graph).unwrap();
        let restored = AnalysisValueLowerer::default()
            .lower(frozen.to_value(), "containers")
            .unwrap();
        assert!(graph.publication_eq(&restored), "container kind {kind}");
        let split = AnalysisValue::tuple(vec![ordinary(kind), ordinary(kind), ordinary(kind)]);
        assert!(
            !graph.publication_eq(&split),
            "alias bijection remains required"
        );

        // A temporary native root remains owned by the memo after value() ends.
        let temporary = materializer.value(&ordinary(kind)).unwrap();
        for _ in 0..32 {
            let different = ordinary(kind);
            assert_ne!(
                temporary.to_value().identity(),
                materializer
                    .value(&different)
                    .unwrap()
                    .to_value()
                    .identity()
            );
        }
    }
}

#[test]
fn shared_and_split_empty_lists_survive_generic_materialization() {
    let heap = FrozenHeap::new();
    let mut materializer = AnalysisValueMaterializer::new(&heap);
    let shared = AnalysisValue::list(Vec::new());
    let split = AnalysisValue::list(Vec::new());
    let a = materializer.value(&shared).unwrap();
    let alias = materializer.value(&shared.dupe()).unwrap();
    let b = materializer.value(&split).unwrap();
    assert_eq!(a.to_value().identity(), alias.to_value().identity());
    assert_ne!(a.to_value().identity(), b.to_value().identity());
    let graph = AnalysisValue::tuple(vec![shared.dupe(), shared, split]);
    let frozen = materializer.value(&graph).unwrap();
    let restored = AnalysisValueLowerer::default()
        .lower(frozen.to_value(), "empty lists")
        .unwrap();
    assert!(graph.publication_eq(&restored));
    let merged = AnalysisValue::list(Vec::new());
    assert!(!graph.publication_eq(&AnalysisValue::tuple(vec![
        merged.dupe(),
        merged.dupe(),
        merged
    ])));
}

#[test]
fn frozen_empty_tuple_interning_remains_an_explicit_materialization_boundary() {
    let heap = FrozenHeap::new();
    let mut materializer = AnalysisValueMaterializer::new(&heap);
    let left = AnalysisValue::tuple(Vec::new());
    let right = AnalysisValue::tuple(Vec::new());
    assert_ne!(left.container_identity(), right.container_identity());
    let a = materializer.value(&left).unwrap();
    let alias = materializer.value(&left.dupe()).unwrap();
    let b = materializer.value(&right).unwrap();
    assert_eq!(a.to_value().identity(), alias.to_value().identity());
    // Ordinary AllocTuple's existing canonical empty behavior is unchanged.
    assert_eq!(a.to_value().identity(), b.to_value().identity());
    let graph = AnalysisValue::tuple(vec![left.dupe(), left]);
    let frozen = materializer.value(&graph).unwrap();
    let restored = AnalysisValueLowerer::default()
        .lower(frozen.to_value(), "empty tuple")
        .unwrap();
    assert!(graph.publication_eq(&restored));
}
