use std::sync::Arc;

use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::cc_toolchain_callable::CcToolchainCallable;
use slug_build_api_v2::cc_toolchain_callable::CcToolchainCallableKind;
use slug_build_api_v2::cc_toolchain_callable::CcToolchainCallableModel;
use slug_build_api_v2::cc_toolchain_callable::CcToolchainCallableSource;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::GlobalsBuilder;
use starlark::environment::LibraryExtension;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

#[test]
fn callable_capture_and_alias_survive_producer_heap_disposal_and_republication() {
    let capture = AnalysisValue::depset(
        AnalysisDepset::new(
            DepsetOrder::Default,
            vec![AnalysisValue::artifact(AnalysisArtifact::Source(
                CanonicalLabel::parse("@@//:runtime.a").unwrap(),
            ))],
            vec![],
        )
        .unwrap(),
    );
    let callable = CcToolchainCallable::new(CcToolchainCallableModel {
        kind: CcToolchainCallableKind::StaticRuntime,
        source: CcToolchainCallableSource {
            label: CanonicalLabel::parse(
                "@@rules_cc+//cc/private/rules_impl:cc_toolchain_info.bzl",
            )
            .unwrap(),
            digest: [9; 32],
        },
        identity: Arc::new(()),
        name_hash: 42,
        capture: capture.clone(),
    })
    .unwrap();
    let retained = {
        let producer = FrozenHeap::new();
        let callable = AnalysisValue::cc_toolchain_callable(callable);
        let graph = AnalysisValue::tuple([callable.clone(), callable, capture]);
        let value = AnalysisValueMaterializer::new(&producer)
            .value(&graph)
            .unwrap();
        AnalysisValueLowerer::default()
            .lower(value.to_value(), "producer")
            .unwrap()
    };
    let consumer = Module::new();
    let restored = AnalysisValueMaterializer::new(consumer.frozen_heap())
        .value(&retained)
        .unwrap();
    consumer.set("values", restored.to_value());
    let mut evaluator = Evaluator::new(&consumer);
    evaluator
        .eval_module(
            AstModule::parse(
                "consumer.bzl",
                r#"
def enabled(name):
    return [name]
def disabled(name):
    return []
def check():
    a, b, capture = values
    result = a(feature_configuration = struct(is_enabled = enabled))
    if a != b or {a: 1}[b] != 1 or result != capture:
        fail("callable or capture occurrence was replaced")
    if result.to_list()[0].basename != "runtime.a":
        fail("File capture was not restored")
    if b(feature_configuration = struct(is_enabled = disabled)).to_list() != []:
        fail("disabled runtime did not return an empty depset")
check()
"#
                .to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &GlobalsBuilder::extended_by(&[LibraryExtension::StructType]).build(),
        )
        .unwrap();
    drop(evaluator);
    let again = AnalysisValueLowerer::from_module(&consumer)
        .lower(restored.to_value(), "consumer")
        .unwrap();
    assert_eq!(retained, again);
    assert!(retained.publication_eq(&again));
}
