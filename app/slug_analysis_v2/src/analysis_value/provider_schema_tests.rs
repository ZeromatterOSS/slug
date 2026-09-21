//! Native provider classification must survive the owned publication boundary.
use slug_loading_v2::provider::starlark_user_provider_has_schema;

use super::*;

fn retained(schemaful: bool) -> AnalysisValue {
    // Constructor classification is covered by Loading and the same-DICE
    // source-mutation integration test; here isolate both transfer directions.
    let heap = FrozenHeap::new();
    let value = alloc_starlark_user_provider(
        &heap,
        slug_build_api_v2::ProviderId::new("@@//:schema.bzl", "Info").unwrap(),
        schemaful,
        [("value".into(), heap.alloc_str("same").to_frozen_value())],
    )
    .to_value();
    assert_eq!(starlark_user_provider_has_schema(value), Some(schemaful));
    AnalysisValueLowerer::default()
        .lower(value, "result")
        .unwrap()
}

#[test]
fn provider_schema_classification_survives_publication_without_field_inference() {
    let with_schema = retained(true);
    let without_schema = retained(false);
    let AnalysisValueKind::Provider(left) = with_schema.kind() else {
        panic!("provider")
    };
    let AnalysisValueKind::Provider(right) = without_schema.kind() else {
        panic!("provider")
    };
    assert_eq!(left.identity(), right.identity());
    assert_eq!(left.fields(), right.fields());
    assert!(!with_schema.publication_eq(&without_schema));
    assert!(!without_schema.publication_eq(&with_schema));
    for (value, expected) in [(&with_schema, true), (&without_schema, false)] {
        let heap = FrozenHeap::new();
        let restored = AnalysisValueMaterializer::new(&heap).value(value).unwrap();
        assert_eq!(
            starlark_user_provider_has_schema(restored.to_value()),
            Some(expected)
        );
        let again = AnalysisValueLowerer::default()
            .lower(restored.to_value(), "again")
            .unwrap();
        assert!(value.publication_eq(&again));
        let heap2 = FrozenHeap::new();
        let twice = AnalysisValueMaterializer::new(&heap2)
            .value(&again)
            .unwrap();
        assert_eq!(
            starlark_user_provider_has_schema(twice.to_value()),
            Some(expected)
        );
    }
    assert!(with_schema.publication_eq(&retained(true)));
}
