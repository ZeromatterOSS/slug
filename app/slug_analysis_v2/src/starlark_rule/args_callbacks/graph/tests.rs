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
    let mut builder = ArgsCallableGraphBuilder::new(&module);
    let root = builder.add_callable(actual, &mut lowerer).unwrap();
    let graph = builder.finish().unwrap();
    let CallableNode::Intrinsic(row) = graph.node(root).unwrap() else {
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
    let mut builder = ArgsCallableGraphBuilder::new(&module);
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
    let mut builder = ArgsCallableGraphBuilder::new(&module);
    builder.audit_module(&mut lowerer).unwrap();
    builder.finish().unwrap();
    let bad = Module::new();
    // An opaque native object is not admitted by an arbitrary representation.
    bad.set("_private", bad.heap().alloc_simple(UnknownCapability));
    let mut builder = ArgsCallableGraphBuilder::new(&bad);
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
fn native_input_preaudit_preserves_aliases_and_refuses_function_inputs() {
    let module = Module::new();
    let actual = module
        .heap()
        .alloc(starlark::values::list::AllocList(["item"]));
    let mut lowerer = AnalysisValueLowerer::default();
    let mut graph = ArgsCallableGraphBuilder::new(&module);
    let a = graph.lower_input(actual, &mut lowerer).unwrap();
    let b = graph.lower_input(actual, &mut lowerer).unwrap();
    assert!(a.shares_storage_with(&b));
    let globals = globals();
    let function = globals
        .iter()
        .find(|(name, _)| *name == "str")
        .unwrap()
        .1
        .to_value();
    assert!(
        graph
            .lower_input(function, &mut lowerer)
            .unwrap_err()
            .contains("unsupported callable")
    );
}

#[test]
fn opaque_cc_variables_fail_before_unbounded_projection() {
    let module = Module::new();
    let heap = starlark::values::FrozenHeap::new();
    let value = slug_loading_v2::alloc_frozen_cc_variables(
        &heap,
        slug_build_api_v2::cc_variables::CcVariablesKind::Map,
        slug_build_api_v2::cc_variables::CcVariablesOccurrence::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut builder = ArgsCallableGraphBuilder::new(&module);
    assert!(
        builder
            .lower_input(value.to_value(), &mut AnalysisValueLowerer::default())
            .unwrap_err()
            .contains("borrowed bounded capture audit")
    );
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
    let mut builder = ArgsCallableGraphBuilder::new(&module);
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
    let graph = builder.finish().unwrap();
    let CallableNode::Intrinsic(row) = graph.node(a).unwrap() else {
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
    assert!(graph.publication_eq(a, &graph, b));
    assert!(!graph.publication_eq(a, &graph, c));
    let mut input = ArgsCallableGraphBuilder::new(&module);
    assert!(
        input
            .lower_input(depset.to_value(), &mut lowerer)
            .unwrap_err()
            .contains("unsupported callable")
    );
}

#[test]
fn run_environment_inputs_and_captures_refuse_before_native_projection() {
    let module = Module::new();
    let value = slug_loading_v2::provider::alloc_frozen_run_environment_info(
        module.frozen_heap(),
        slug_build_api_v2::RunEnvironmentInfo::empty(),
    );
    let mut builder = ArgsCallableGraphBuilder::new(&module);
    let mut lowerer = AnalysisValueLowerer::default();
    assert!(
        builder
            .lower_input(value.to_value(), &mut lowerer)
            .unwrap_err()
            .contains("no admitted borrowed capture audit")
    );
    module.set("_environment", value.to_value());
    let mut builder = ArgsCallableGraphBuilder::new(&module);
    assert!(
        builder
            .audit_module(&mut lowerer)
            .unwrap_err()
            .contains("no admitted borrowed capture audit")
    );
}
