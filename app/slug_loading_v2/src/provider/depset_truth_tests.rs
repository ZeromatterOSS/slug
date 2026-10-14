//! Depset truth uses type emptiness metadata, never depth or leaf truth.
use slug_build_api_v2::AnalysisDepsetInput;
use slug_build_api_v2::AnalysisValue;
use starlark::environment::FrozenModule;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::FrozenHeap;
use starlark::values::none::NoneType;

use super::*;

#[starlark_module]
fn gc_helpers(builder: &mut GlobalsBuilder) {
    fn collect(eval: &mut Evaluator<'_, '_, '_>) -> anyhow::Result<NoneType> {
        // Called by actual bytecode with an initialized frame; the depset graph
        // and empty cache are owned by traced evaluator/module roots.
        unsafe {
            eval.garbage_collect()
                .map_err(|error| error.into_anyhow())?
        };
        Ok(NoneType)
    }
}

fn globals() -> Globals {
    GlobalsBuilder::standard()
        .with(|builder| {
            builder.set("depset", AnalysisBuiltinCallable::new("depset"));
            gc_helpers(builder);
        })
        .build()
}

fn evaluate(source: &str) -> FrozenModule {
    let module = Module::new();
    let mut evaluator = Evaluator::new(&module);
    evaluator
        .eval_module(
            AstModule::parse(
                "unrelated_truth_consumer.bzl",
                source.to_owned(),
                &Dialect::Standard,
            )
            .unwrap(),
            &globals(),
        )
        .unwrap();
    drop(evaluator);
    module.freeze().unwrap()
}

#[test]
fn evaluator_truth_control_flow_covers_all_orders_and_false_valued_leaves() {
    let frozen = evaluate(
        r#"
def check(order):
    empty = depset(order = order)
    empty_child = depset(transitive = [empty], order = order)
    full = depset([False], order = order)
    transitive = depset(transitive = [full], order = order)
    if empty or empty_child:
        fail("empty depset entered branch")
    if not full or not transitive:
        fail("nonempty depset skipped branch")
    return (bool(empty), not empty, bool(empty_child), bool(full),
            bool(depset([0], order = order)), bool(depset([""], order = order)),
            bool(transitive), (empty or full) == full, (full and empty) == empty)
RESULT = [check(order) for order in ["default", "postorder", "preorder", "topological"]]
"#,
    );
    assert_eq!(
        frozen.get("RESULT").unwrap().value().to_repr(),
        "[(False, True, False, True, True, True, True, True, True), (False, True, False, True, True, True, True, True, True), (False, True, False, True, True, True, True, True, True), (False, True, False, True, True, True, True, True, True)]"
    );
}

#[test]
fn empty_cache_frozen_import_and_shared_gc_dag_preserve_truth_and_occurrence() {
    let frozen = evaluate("EMPTY = depset()\nALIAS = depset(direct = [])\nFULL = depset([''])\n");
    assert!(
        frozen
            .get("EMPTY")
            .unwrap()
            .value()
            .ptr_eq(frozen.get("ALIAS").unwrap().value())
    );
    let consumer = Module::new();
    consumer.import_public_symbols(&frozen);
    let mut evaluator = Evaluator::new(&consumer);
    evaluator.eval_module(AstModule::parse("other_consumer.bzl",
        "RESULT = (bool(EMPTY), bool(ALIAS), bool(FULL), not FULL, (EMPTY or FULL) == FULL)\nCAPTURED_FULL = FULL\n".to_owned(), &Dialect::Standard).unwrap(), &globals()).unwrap();
    assert_eq!(
        consumer.get("RESULT").unwrap().to_repr(),
        "(False, False, True, False, True)"
    );
    assert_eq!(
        frozen
            .get("FULL")
            .unwrap()
            .value()
            .get_hashed()
            .unwrap()
            .hash(),
        consumer
            .get("CAPTURED_FULL")
            .unwrap()
            .get_hashed()
            .unwrap()
            .hash()
    );

    drop(evaluator);
    let frozen = evaluate(
        r#"
def build():
    empty = depset()
    shared = depset([""])
    for i in range(80):
        left = depset([str(i)], transitive = [shared])
        right = depset([str(-i)], transitive = [shared])
        shared = depset(transitive = [left, right])
        collect()
        if not shared or empty or depset():
            fail("truth changed across collection")
    return (empty, shared)
GRAPH = build()
RESULT = (bool(GRAPH[0]), bool(GRAPH[1]))
"#,
    );
    assert_eq!(
        frozen.get("RESULT").unwrap().value().to_repr(),
        "(False, True)"
    );
    let owned_graph = frozen.get("GRAPH").unwrap();
    let graph = TupleRef::from_value(owned_graph.value().to_value()).unwrap();
    assert!(!graph.content()[0].to_bool());
    assert!(graph.content()[1].to_bool());
}

#[test]
fn native_views_use_empty_type_facts_even_with_opaque_successors_or_supplied_depth() {
    let heap = FrozenHeap::new();
    for order in [
        DepsetOrder::Default,
        DepsetOrder::Postorder,
        DepsetOrder::Preorder,
        DepsetOrder::Topological,
    ] {
        for retained in [
            AnalysisDepset::empty(order),
            AnalysisDepset::new(order, vec![AnalysisValue::string("")], vec![]).unwrap(),
        ] {
            let expected = !retained.is_empty();
            let view = alloc_starlark_depset(&heap, retained.clone(), Vec::new());
            assert_eq!(view.to_value().to_bool(), expected);
            let (_, _, occurrence, native, _, successors) =
                StarlarkDepset::parts_from_value(view.to_value()).unwrap();
            assert!(successors.is_empty());
            assert_eq!(occurrence, retained.occurrence());
            assert_eq!(native.unwrap().is_empty(), !expected);
        }
    }

    let nonempty = AnalysisDepset::from_canonical_successors(
        AnalysisDepsetOccurrence::new(),
        DepsetOrder::Default,
        0,
        vec![AnalysisDepsetInput::Direct(AnalysisValue::string("actual"))],
    )
    .unwrap();
    assert_eq!(nonempty.depth(), 0);
    assert!(!nonempty.is_empty());
    assert!(
        alloc_starlark_depset(&heap, nonempty, Vec::new())
            .to_value()
            .to_bool()
    );
    let empty = AnalysisDepset::from_canonical_successors(
        AnalysisDepsetOccurrence::new(),
        DepsetOrder::Default,
        17,
        vec![AnalysisDepsetInput::Transitive(AnalysisDepset::empty(
            DepsetOrder::Default,
        ))],
    )
    .unwrap();
    assert_eq!(empty.depth(), 17);
    assert!(empty.is_empty());
    assert!(
        !alloc_starlark_depset(&heap, empty, Vec::new())
            .to_value()
            .to_bool()
    );
}

#[path = "depset_lifecycle_tests.rs"]
mod lifecycle_tests;
