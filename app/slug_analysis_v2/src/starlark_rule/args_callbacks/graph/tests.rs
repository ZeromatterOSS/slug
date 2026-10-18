use starlark::environment::GlobalsBuilder;
use starlark::environment::LibraryExtension;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::ValueLike;

use super::*;

fn globals() -> starlark::environment::Globals {
    let mut globals = GlobalsBuilder::new();
    slug_starlark_v2::populate_universe(&mut globals);
    LibraryExtension::StructType.add(&mut globals);
    globals.build()
}
#[test]
fn actual_native_wrapper_rows_preserve_parameter_layout_and_reject_unknown_def_source() {
    let module = Module::new();
    let globals = globals();
    let actual = globals
        .iter()
        .find(|(name, _)| *name == "list")
        .unwrap()
        .1
        .to_value();
    let native = inspect_native_function(actual).unwrap();
    let mut lowerer = AnalysisValueLowerer::default();
    let mut builder = PublicationBuilder::new(&module);
    let root = builder.add_callable(actual, &mut lowerer).unwrap();
    let (graph, _) = builder.finish();
    let PublishedNode::Intrinsic(row) = graph.node(root).unwrap() else {
        panic!("actual native callable must retain an intrinsic row")
    };
    assert_eq!(row.parameters.len(), native.parameter_count());
    assert_eq!(row.api.canonical(), "starlark.universe.list");
    let ast = AstModule::parse(
        "unowned.bzl",
        "def independent(x):\n    return x\n".into(),
        &Dialect::Standard,
    )
    .unwrap();
    Evaluator::new(&module).eval_module(ast, &globals).unwrap();
    let mut builder = PublicationBuilder::new(&module);
    assert!(
        builder
            .add_callable(module.get("independent").unwrap(), &mut lowerer)
            .unwrap_err()
            .contains("source metadata")
    );
}
#[test]
fn module_slot_audit_reaches_private_values_and_refuses_unknown_capabilities() {
    let module = Module::new();
    module.set("_private", module.heap().alloc("safe"));
    let mut lowerer = AnalysisValueLowerer::default();
    let mut builder = PublicationBuilder::new(&module);
    builder.audit_module(&mut lowerer).unwrap();
    let _ = builder.finish();
    let bad = Module::new();
    // An opaque native object is not admitted by an arbitrary representation.
    bad.set("_private", bad.heap().alloc_simple(UnknownCapability));
    let mut builder = PublicationBuilder::new(&bad);
    assert!(
        builder
            .audit_module(&mut AnalysisValueLowerer::default())
            .unwrap_err()
            .contains("unsupported value/capability")
    );
    let mut calls = 0;
    bad.visit_assigned_values(&mut |_| {
        calls += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(calls, 1);
}

#[derive(
    Debug,
    allocative::Allocative,
    starlark::any::ProvidesStaticType,
    starlark::values::NoSerialize
)]
struct UnknownCapability;
impl std::fmt::Display for UnknownCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<opaque capability>")
    }
}
starlark::starlark_simple_value!(UnknownCapability);
#[starlark::values::starlark_value(type = "test_unknown_capability")]
impl<'v> starlark::values::StarlarkValue<'v> for UnknownCapability {}

#[test]
fn native_inputs_preserve_aliases_and_admit_actual_callable_descriptors() {
    let module = Module::new();
    let actual = module
        .heap()
        .alloc(starlark::values::list::AllocList(["item"]));
    let mut lowerer = AnalysisValueLowerer::default();
    let mut publisher = super::super::Publisher::new(&module);
    let a = publisher.capture(actual, &mut lowerer).unwrap();
    let b = publisher.capture(actual, &mut lowerer).unwrap();
    assert_eq!(a, b);
    let globals = globals();
    let function = globals
        .iter()
        .find(|(name, _)| *name == "str")
        .unwrap()
        .1
        .to_value();
    let callable = publisher.capture(function, &mut lowerer).unwrap();
    let builder = publisher.seal(&mut lowerer).unwrap();
    drop(lowerer);
    let module = module.freeze().unwrap();
    let frozen =
        super::super::FrozenPublisher::new(builder, module.owned_extra_value().unwrap()).unwrap();
    let a = frozen.value(a).unwrap();
    let b = frozen.value(b).unwrap();
    assert!(a.shares_storage_with(&b));
    let slug_build_api_v2::AnalysisValueKind::List(items) = a.kind() else {
        panic!("list input")
    };
    assert_eq!(items[0].as_str(), Some("item"));
    assert!(
        frozen
            .value(callable)
            .unwrap_err()
            .contains("native API descriptors")
    );
}

#[test]
fn bounded_cc_variable_inputs_preserve_semantic_seed_and_aliases() {
    use slug_build_api_v2::cc_variables::CcVariableCacheEntry;
    use slug_build_api_v2::cc_variables::CcVariablesKind;
    use slug_build_api_v2::cc_variables::CcVariablesOccurrence;
    let module = Module::new();
    let heap = module.frozen_heap();
    let child = heap.alloc("cached");
    let value = slug_loading_v2::alloc_frozen_cc_variables(
        heap,
        CcVariablesKind::Map,
        CcVariablesOccurrence::new(),
        None,
        vec![("local".into(), child)],
        vec![
            (
                "seed".into(),
                CcVariableCacheEntry::Sequence(vec![child, child].into_boxed_slice()),
            ),
            ("missing".into(), CcVariableCacheEntry::Missing),
            (
                "error".into(),
                CcVariableCacheEntry::Error("semantic error".into()),
            ),
        ],
    );
    let mut lowerer = AnalysisValueLowerer::default();
    let mut publisher = super::super::Publisher::new(&module);
    let id = publisher.capture(value.to_value(), &mut lowerer).unwrap();
    assert_eq!(
        id,
        publisher.capture(value.to_value(), &mut lowerer).unwrap()
    );
    let builder = publisher.seal(&mut lowerer).unwrap();
    drop(lowerer);
    let module = module.freeze().unwrap();
    let frozen =
        super::super::FrozenPublisher::new(builder, module.owned_extra_value().unwrap()).unwrap();
    let value = frozen.value(id).unwrap();
    let slug_build_api_v2::AnalysisValueKind::CcToolchainVariables(value) = value.kind() else {
        panic!("variables")
    };
    let model = value.model();
    assert_eq!(model.locals[0].1.as_str(), Some("cached"));
    assert!(
        model.cache.iter().any(
            |(name, value)| name == "missing" && matches!(value, CcVariableCacheEntry::Missing)
        )
    );
    assert!(model.cache.iter().any(|(name, value)| name == "error"
        && matches!(value,CcVariableCacheEntry::Error(message) if message=="semantic error")));
    let (_, CcVariableCacheEntry::Sequence(sequence)) =
        model.cache.iter().find(|(name, _)| name == "seed").unwrap()
    else {
        panic!("sequence seed")
    };
    assert_eq!(sequence[0], sequence[1]);
}

#[test]
fn actual_loading_builtin_rows_certify_state_and_preserve_instance_aliases() {
    use slug_loading_v2::AnalysisBuiltinCallableApi;
    use slug_loading_v2::alloc_frozen_analysis_builtin_callable;
    let module = Module::new();
    let heap = starlark::values::FrozenHeap::new();
    let depset = alloc_frozen_analysis_builtin_callable(&heap, AnalysisBuiltinCallableApi::Depset);
    let other = alloc_frozen_analysis_builtin_callable(&heap, AnalysisBuiltinCallableApi::Depset);
    let default =
        alloc_frozen_analysis_builtin_callable(&heap, AnalysisBuiltinCallableApi::DefaultInfo);
    let mut lowerer = AnalysisValueLowerer::default();
    let mut builder = PublicationBuilder::new(&module);
    let a = builder
        .add_callable(depset.to_value(), &mut lowerer)
        .unwrap();
    let alias = builder
        .add_callable(depset.to_value(), &mut lowerer)
        .unwrap();
    let b = builder
        .add_callable(other.to_value(), &mut lowerer)
        .unwrap();
    let c = builder
        .add_callable(default.to_value(), &mut lowerer)
        .unwrap();
    assert_eq!(a, alias);
    assert_ne!(
        a, b,
        "equal API certificates cannot collapse actual instances"
    );
    let (graph, roots) = builder.finish();
    let PublishedNode::Intrinsic(row) = graph.node(a).unwrap() else {
        panic!("intrinsic row")
    };
    assert_eq!(row.api.canonical(), "slug.loading.analysis_builtin.depset");
    assert_eq!(row.api.version(), 1);
    assert!(
        row.parameters.is_empty(),
        "custom class stores no parameter metadata, not zero arity"
    );
    assert!(row.receiver.is_none());
    assert!(row.return_annotation.is_none());
    module.set_extra_value(
        module
            .heap()
            .alloc(starlark::values::tuple::AllocTuple(roots)),
    );
    drop(lowerer);
    let module = module.freeze().unwrap();
    let frozen =
        super::super::FrozenPublisher::new(graph, module.owned_extra_value().unwrap()).unwrap();
    assert!(
        frozen
            .graph
            .value(a)
            .unwrap()
            .publication_eq(&frozen.graph.value(b).unwrap())
    );
    assert!(
        !frozen
            .graph
            .value(a)
            .unwrap()
            .publication_eq(&frozen.graph.value(c).unwrap())
    );
    assert!(
        frozen
            .value(a)
            .unwrap_err()
            .contains("native API descriptors")
    );
}

#[test]
fn run_environment_inputs_and_module_roots_roundtrip_through_borrowed_native_audit() {
    let module = Module::new();
    let value = slug_loading_v2::provider::alloc_frozen_run_environment_info(
        module.frozen_heap(),
        slug_build_api_v2::RunEnvironmentInfo::empty(),
    );
    module.set("_environment", value.to_value());
    let mut lowerer = AnalysisValueLowerer::default();
    let mut publisher = super::super::Publisher::new(&module);
    let id = publisher.capture(value.to_value(), &mut lowerer).unwrap();
    let builder = publisher.seal(&mut lowerer).unwrap();
    drop(lowerer);
    let module = module.freeze().unwrap();
    let frozen =
        super::super::FrozenPublisher::new(builder, module.owned_extra_value().unwrap()).unwrap();
    let value = frozen.value(id).unwrap();
    assert!(matches!(
        value.kind(),
        slug_build_api_v2::AnalysisValueKind::RunEnvironmentInfo(_)
    ));
}

use super::loaded_source as support;

#[path = "set_capture_tests.rs"]
mod set_capture;

#[tokio::test]
async fn genuine_function_provider_inputs_retain_originals_and_refuse_captured_capabilities() {
    let loaded=support::LoadedSource::new("def transform(value): return 'original:' + value\nP = provider(fields=['callable'])\ndef entry(ctx): return (transform, P(callable=transform))\n").await;
    let module = Module::new();
    let values = loaded.invoke(&module, Value::new_none());
    let values = TupleRef::from_value(values).unwrap();
    let mut lowerer = AnalysisValueLowerer::default();
    let mut publisher = super::super::Publisher::new(&module);
    let function = publisher
        .capture(values.content()[0], &mut lowerer)
        .unwrap();
    let provider = publisher
        .capture(values.content()[1], &mut lowerer)
        .unwrap();
    let builder = publisher.seal(&mut lowerer).unwrap();
    drop(lowerer);
    let module = module.freeze().unwrap();
    let frozen =
        super::super::FrozenPublisher::new(builder, module.owned_extra_value().unwrap()).unwrap();
    drop(loaded);
    drop(module);
    let function = frozen.value(function).unwrap();
    assert!(matches!(
        function.kind(),
        slug_build_api_v2::AnalysisValueKind::Function(_)
    ));
    let provider = frozen.value(provider).unwrap();
    let slug_build_api_v2::AnalysisValueKind::Provider(provider) = provider.kind() else {
        panic!("provider input")
    };
    assert_eq!(provider.field("callable").unwrap(), &function);
    let consumer = Module::new();
    let original = crate::analysis_value::published::materialize_original(
        function.original_origin().unwrap(),
        consumer.frozen_heap(),
    )
    .unwrap();
    let result = Evaluator::new(&consumer)
        .eval_function(original.to_value(), &[consumer.heap().alloc("input")], &[])
        .unwrap();
    assert_eq!(result.unpack_str(), Some("original:input"));
    let loaded = support::LoadedSource::new(
        "def entry(ctx):\n    def captured(value): return ctx\n    return captured\n",
    )
    .await;
    let module = Module::new();
    let callback = loaded.invoke(&module, module.heap().alloc_simple(UnknownCapability));
    let error = PublicationBuilder::new(&module)
        .add_value(callback, &mut AnalysisValueLowerer::default())
        .unwrap_err();
    assert!(error.contains("unsupported value/capability"), "{error}");
}

#[tokio::test]
async fn transient_function_publication_refuses_with_genuine_lexical_source_metadata() {
    let loaded = support::LoadedSource::new(
        "def entry(ctx):\n    def transient(value): return value\n    return transient\n",
    )
    .await;
    let module = Module::new();
    let callback = loaded.invoke_transient(&module, Value::new_none());
    let error = PublicationBuilder::new(&module)
        .add_value(callback, &mut AnalysisValueLowerer::default())
        .unwrap_err();
    assert!(error.contains("structural evaluation owner"), "{error}");
}
