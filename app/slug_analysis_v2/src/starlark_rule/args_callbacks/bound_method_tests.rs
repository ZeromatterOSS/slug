//! Actual bound callables retain receivers and occurrence aliases across consumers.
use slug_build_api_v2::published_value::PublishedNode;
use slug_build_api_v2::published_value::PublishedNodeId;
use slug_build_api_v2::published_value::PublishedValueArena;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::list::AllocList;
use starlark::values::tuple::AllocTuple;

use super::graph::PublicationBuilder;
use super::runtime::ArgsRuntimeLimits;
use super::runtime::StarlarkRuntimeMapper;
use super::*;

fn evaluate(module: &Module, source: &str, globals: &Globals) {
    Evaluator::new(module)
        .eval_module(
            AstModule::parse(
                "unrelated_bound_methods.bzl",
                source.to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            globals,
        )
        .unwrap();
}

fn words(left: &str, right: &str) -> AnalysisValue {
    AnalysisValue::list(vec![
        AnalysisValue::string(left),
        AnalysisValue::string(right),
    ])
}

#[test]
fn first_class_bound_string_methods_execute_original_callable_after_producer_drop() {
    let producer = Module::new();
    evaluate(
        &producer,
        "joiner = '-'.join\nformatter = '{}'.format\n",
        &Globals::standard(),
    );
    let frozen = producer.freeze().unwrap();
    let joiner = StarlarkRuntimeMapper::new(frozen.get("joiner").unwrap());
    let formatter = StarlarkRuntimeMapper::new(frozen.get("formatter").unwrap());
    drop(frozen);
    for _ in 0..2 {
        assert_eq!(
            joiner
                .map_batch(
                    &[words("a", "b"), words("c", "d")],
                    &NoopArgsDirectoryContext
                )
                .unwrap(),
            ["a-b", "c-d"],
        );
        assert_eq!(
            formatter
                .map_batch(
                    &[AnalysisValue::integer(17), AnalysisValue::string("word")],
                    &NoopArgsDirectoryContext
                )
                .unwrap(),
            ["17", "word"],
        );
    }
    let error = joiner
        .map_batch(&[AnalysisValue::integer(17)], &NoopArgsDirectoryContext)
        .unwrap_err();
    assert!(
        matches!(error, ArgsExpansionError::Callback(_)),
        "original join argument validation must remain active"
    );
}

#[starlark::starlark_module]
fn gc_helpers(builder: &mut GlobalsBuilder) {
    fn collect(eval: &mut Evaluator<'_, '_, '_>) -> anyhow::Result<bool> {
        // Allocation and measurement happen inside the initialized native frame.
        eval.heap().alloc(AllocList(vec![
            "unreachable-for-bound-method".to_owned();
            512
        ]));
        let before = eval.heap().allocated_bytes();
        unsafe {
            eval.garbage_collect()
                .map_err(|error| error.into_anyhow())?
        };
        Ok(eval.heap().allocated_bytes() < before)
    }
}

#[test]
fn bound_method_closure_keeps_original_receiver_across_gc_and_fresh_batches() {
    let producer = Module::new();
    evaluate(
        &producer,
        r#"
def make():
    joiner = '-'.join
    formatter = '{}'.format
    def callback(value):
        if not collect():
            fail('GC must reclaim test garbage')
        return formatter(joiner(value))
    return callback
mapper = make()
"#,
        &GlobalsBuilder::standard().with(gc_helpers).build(),
    );
    let runtime = StarlarkRuntimeMapper::new(producer.freeze().unwrap().get("mapper").unwrap())
        .with_limits(ArgsRuntimeLimits {
            input_count: 2,
            output_count: 2,
            output_bytes: 16,
            heap_bytes: 128 * 1024,
            timeout: std::time::Duration::from_secs(1),
            call_stack: 32,
        });
    for _ in 0..2 {
        assert_eq!(
            runtime
                .map_batch(
                    &[words("a", "b"), words("c", "d")],
                    &NoopArgsDirectoryContext
                )
                .unwrap(),
            ["a-b", "c-d"]
        );
    }
}

fn recipe(
    graph: &PublishedValueArena,
    nodes: [PublishedNodeId; 2],
    callables: [OwnedFrozenValue; 2],
) -> RetainedArgsRecipe {
    let input = words("a", "b");
    let calls = nodes
        .into_iter()
        .zip(callables)
        .map(|(node, callable)| {
            let mapper = RetainedArgsMapper::from_published(
                graph.value(node).unwrap(),
                Arc::new(StarlarkRuntimeMapper::new(callable)),
            )
            .unwrap();
            RetainedArgCall::AddAll(RetainedVectorArg::new(
                RetainedVectorSource::Mapped {
                    inputs: RetainedArgsInputs::Sequence(Arc::from([input.clone()])),
                    mapper,
                    registration_location: None,
                },
                RetainedVectorOptions {
                    arg_name: None,
                    format_each: None,
                    before_each: None,
                    join_with: None,
                    format_joined: None,
                    omit_if_empty: true,
                    uniquify: false,
                    expand_directories: false,
                    terminate_with: None,
                },
            ))
        })
        .collect::<Vec<_>>();
    RetainedArgsRecipe::new(calls, RetainedParamFileFormat::Multiline)
}

#[test]
fn bound_method_publication_preserves_wrapper_occurrences_receiver_aliases_and_changes() {
    let producer = Module::new();
    let receiver = producer.heap().alloc_str("-").to_value();
    let first = receiver.get_attr_error("join", producer.heap()).unwrap();
    let second = receiver.get_attr_error("join", producer.heap()).unwrap();
    let changed = producer
        .heap()
        .alloc_str("|")
        .to_value()
        .get_attr_error("join", producer.heap())
        .unwrap();
    let formatter = producer
        .heap()
        .alloc_str("{}")
        .to_value()
        .get_attr_error("format", producer.heap())
        .unwrap();
    assert!(
        !first.ptr_eq(second),
        "separate property lookups create separate wrappers"
    );
    producer.set("first", first);
    producer.set("second", second);
    producer.set("changed", changed);
    let mut lowerer = AnalysisValueLowerer::default();
    let mut builder = PublicationBuilder::new(&producer);
    let a = builder.add_callable(first, &mut lowerer).unwrap();
    assert_eq!(a, builder.add_callable(first, &mut lowerer).unwrap());
    let b = builder.add_callable(second, &mut lowerer).unwrap();
    let c = builder.add_callable(changed, &mut lowerer).unwrap();
    let format = builder.add_callable(formatter, &mut lowerer).unwrap();
    assert_ne!(a, b);
    let (graph, roots) = builder.finish();
    let receiver_node = |id| {
        let PublishedNode::Intrinsic(row) = graph.node(id).unwrap() else {
            panic!("bound native method row")
        };
        row.receiver
            .expect("actual receiver must be part of the graph")
    };
    assert_eq!(
        receiver_node(a),
        receiver_node(b),
        "distinct wrappers share the original receiver occurrence"
    );
    assert_ne!(receiver_node(a), receiver_node(c));
    producer.set_extra_value(producer.heap().alloc(AllocTuple(roots)));
    drop(lowerer);
    let frozen = producer.freeze().unwrap();
    let published = FrozenPublisher::new(graph, frozen.owned_extra_value().unwrap()).unwrap();
    assert!(
        published
            .value(a)
            .unwrap_err()
            .contains("native API descriptors")
    );
    let graph = &published.graph;
    assert!(
        graph
            .value(a)
            .unwrap()
            .publication_eq(&graph.value(b).unwrap())
    );
    assert!(
        !graph
            .value(a)
            .unwrap()
            .publication_eq(&graph.value(c).unwrap()),
        "changing the receiver changes callable publication"
    );
    assert!(
        !graph
            .value(a)
            .unwrap()
            .publication_eq(&graph.value(format).unwrap()),
        "method metadata also participates"
    );
    let aliased = recipe(
        &graph,
        [a, a],
        [frozen.get("first").unwrap(), frozen.get("first").unwrap()],
    );
    let independent = recipe(
        &graph,
        [a, b],
        [frozen.get("first").unwrap(), frozen.get("second").unwrap()],
    );
    let changed = recipe(
        &graph,
        [c, c],
        [
            frozen.get("changed").unwrap(),
            frozen.get("changed").unwrap(),
        ],
    );
    drop(frozen);
    assert!(
        aliased != independent,
        "joint recipe equality must preserve repeated wrapper aliases"
    );
    assert!(aliased != changed);
    assert_eq!(aliased.render().unwrap(), ["a-b", "a-b"]);
    assert_eq!(independent.render().unwrap(), ["a-b", "a-b"]);
    assert_eq!(changed.render().unwrap(), ["a|b", "a|b"]);
}

#[test]
fn bound_list_dict_methods_keep_mutated_receiver_snapshot_aliases_and_frozen_refusals() {
    let producer = Module::new();
    evaluate(
        &producer,
        "values = ['before', 'kept']\nlookup = {'key': 'before'}\nindexer = values.index\nother_indexer = values.index\ngetter = lookup.get\nother_getter = lookup.get\n",
        &Globals::standard(),
    );
    let baseline = Module::new();
    evaluate(
        &baseline,
        "values = ['before', 'kept']\nlookup = {'key': 'before'}\nindexer = values.index\nother_indexer = values.index\ngetter = lookup.get\nother_getter = lookup.get\n",
        &Globals::standard(),
    );
    let mut before_lowerer = AnalysisValueLowerer::default();
    let mut before_builder = PublicationBuilder::new(&baseline);
    let before_index = before_builder
        .add_callable(baseline.get("indexer").unwrap(), &mut before_lowerer)
        .unwrap();
    let before_get = before_builder
        .add_callable(baseline.get("getter").unwrap(), &mut before_lowerer)
        .unwrap();
    let (before, before_roots) = before_builder.finish();
    baseline.set_extra_value(baseline.heap().alloc(AllocTuple(before_roots)));
    drop(before_lowerer);
    let baseline = baseline.freeze().unwrap();
    let before = FrozenPublisher::new(before, baseline.owned_extra_value().unwrap()).unwrap();
    let indexer = producer.get("indexer").unwrap();
    let getter = producer.get("getter").unwrap();
    // Binding retains the actual receiver, including mutations before freeze.
    evaluate(
        &producer,
        "values.clear()\nvalues.extend(['kept', 'after'])\nlookup['key'] = 'after'\nlookup['other'] = 'new'\n",
        &Globals::standard(),
    );
    let mut lowerer = AnalysisValueLowerer::default();
    let mut builder = PublicationBuilder::new(&producer);
    let index = builder.add_callable(indexer, &mut lowerer).unwrap();
    assert_eq!(index, builder.add_callable(indexer, &mut lowerer).unwrap());
    let other_index = builder
        .add_callable(producer.get("other_indexer").unwrap(), &mut lowerer)
        .unwrap();
    let get = builder.add_callable(getter, &mut lowerer).unwrap();
    assert_eq!(get, builder.add_callable(getter, &mut lowerer).unwrap());
    let other_get = builder
        .add_callable(producer.get("other_getter").unwrap(), &mut lowerer)
        .unwrap();
    let (graph, roots) = builder.finish();
    let receiver = |id| {
        let PublishedNode::Intrinsic(row) = graph.node(id).unwrap() else {
            panic!("bound method")
        };
        row.receiver.unwrap()
    };
    assert_ne!(index, other_index);
    assert_ne!(get, other_get);
    assert_eq!(receiver(index), receiver(other_index));
    assert_eq!(receiver(get), receiver(other_get));
    producer.set_extra_value(producer.heap().alloc(AllocTuple(roots)));
    drop(lowerer);
    let frozen = producer.freeze().unwrap();
    let published = FrozenPublisher::new(graph, frozen.owned_extra_value().unwrap()).unwrap();
    let graph = &published.graph;
    assert!(
        !before
            .graph
            .value(before_index)
            .unwrap()
            .publication_eq(&graph.value(index).unwrap())
    );
    assert!(
        !before
            .graph
            .value(before_get)
            .unwrap()
            .publication_eq(&graph.value(get).unwrap())
    );
    let indexer = frozen.get("indexer").unwrap();
    let getter = frozen.get("getter").unwrap();
    let values = frozen.get("values").unwrap();
    let lookup = frozen.get("lookup").unwrap();
    drop(frozen);
    let consumer = Module::new();
    let actual_index = indexer.owned_value(consumer.frozen_heap());
    assert_eq!(
        Evaluator::new(&consumer)
            .eval_function(
                actual_index,
                &[consumer.heap().alloc_str("kept").to_value()],
                &[]
            )
            .unwrap()
            .unpack_i32(),
        Some(0)
    );
    let get_runtime = StarlarkRuntimeMapper::new(getter);
    for _ in 0..2 {
        assert_eq!(
            get_runtime
                .map_batch(
                    &[AnalysisValue::string("key"), AnalysisValue::string("other")],
                    &NoopArgsDirectoryContext
                )
                .unwrap(),
            ["after", "new"]
        );
    }
    assert!(
        matches!(
            StarlarkRuntimeMapper::new(indexer)
                .map_batch(&[AnalysisValue::string("kept")], &NoopArgsDirectoryContext),
            Err(ArgsExpansionError::InvalidReturn(_))
        ),
        "original integer result is not an Args string result"
    );
    consumer.set("values", values.owned_value(consumer.frozen_heap()));
    consumer.set("lookup", lookup.owned_value(consumer.frozen_heap()));
    for source in [
        "values.append('illegal')",
        "lookup.update({'key': 'illegal'})",
    ] {
        let error = Evaluator::new(&consumer)
            .eval_module(
                AstModule::parse("frozen_receiver.bzl", source.to_owned(), &Dialect::Bazel)
                    .unwrap(),
                &Globals::standard(),
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains("Immutable"), "{error}");
    }
    assert_eq!(
        get_runtime
            .map_batch(&[AnalysisValue::string("key")], &NoopArgsDirectoryContext)
            .unwrap(),
        ["after"]
    );
}

#[path = "native/package_specification_tests.rs"]
mod package_specification_tests;
