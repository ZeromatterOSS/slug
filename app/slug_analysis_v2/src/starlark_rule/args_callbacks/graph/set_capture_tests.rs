//! Sets are original capture data; no outward AnalysisValue set is synthesized.
use starlark::values::none::NoneType;

use super::*;

#[starlark::starlark_module]
fn gc_helpers(builder: &mut GlobalsBuilder) {
    fn collect(eval: &mut Evaluator<'_, '_, '_>) -> anyhow::Result<NoneType> {
        // Run from an initialized bytecode frame, with module roots traced.
        unsafe {
            eval.garbage_collect()
                .map_err(|error| error.into_anyhow())?
        };
        Ok(NoneType)
    }
}

fn collect(module: &Module) {
    let mut globals = GlobalsBuilder::new();
    gc_helpers(&mut globals);
    Evaluator::new(module)
        .eval_module(
            AstModule::parse("gc.star", "collect()\n".into(), &Dialect::Standard).unwrap(),
            &globals.build(),
        )
        .unwrap();
}

#[tokio::test]
async fn mutable_set_capture_keeps_final_membership_aliases_and_original_after_gc_and_drop() {
    let loaded = support::LoadedSource::new(
        "def entry(ctx):\n    members = set(['first', 'removed'])\n    members.remove('removed')\n    members.add('last')\n    members.add('first')\n    def callback(value): return list(members) + [value]\n    return (callback, members, members, set(['first', 'last']))\n",
    )
    .await;
    let module = Module::new();
    module.set("_result", loaded.invoke(&module, Value::new_none()));
    // Only module-rooted values survive this collection; reacquire every handle.
    collect(&module);
    let values = TupleRef::from_value(module.get("_result").unwrap()).unwrap();
    let mut lowerer = AnalysisValueLowerer::default();
    let mut publisher = super::super::super::Publisher::new(&module);
    let function = publisher
        .capture(values.content()[0], &mut lowerer)
        .unwrap();
    let set = publisher
        .capture(values.content()[1], &mut lowerer)
        .unwrap();
    assert_eq!(
        set,
        publisher
            .capture(values.content()[2], &mut lowerer)
            .unwrap()
    );
    let split = publisher
        .capture(values.content()[3], &mut lowerer)
        .unwrap();
    assert_ne!(set, split);
    let builder = publisher.seal(&mut lowerer).unwrap();
    drop(lowerer);
    let module = module.freeze().unwrap();
    let frozen =
        super::super::super::FrozenPublisher::new(builder, module.owned_extra_value().unwrap())
            .unwrap();
    let row = frozen.graph.value(set).unwrap();
    assert_eq!(row.value_type(), slug_build_api_v2::AnalysisValueType::Set);
    let PublishedNode::Set(range) = row.node() else {
        panic!("set descriptor")
    };
    assert_eq!(frozen.graph.edges(*range).unwrap().len(), 2);
    drop(row);
    assert!(
        frozen
            .value(set)
            .unwrap_err()
            .contains("outward set projection")
    );
    let function = frozen.value(function).unwrap();
    drop(loaded);
    drop(module);
    drop(frozen);
    let consumer = Module::new();
    let original = crate::analysis_value::published::materialize_original(
        function.original_origin().unwrap(),
        consumer.frozen_heap(),
    )
    .unwrap();
    consumer.set("_callback", original.to_value());
    collect(&consumer);
    let result = Evaluator::new(&consumer)
        .eval_function(
            consumer.get("_callback").unwrap(),
            &[consumer.heap().alloc("input")],
            &[],
        )
        .unwrap();
    assert_eq!(result.to_repr(), "[\"first\", \"last\", \"input\"]");
}

#[tokio::test]
async fn frozen_module_set_capture_executes_original_and_plain_set_inputs_remain_unsupported() {
    let loaded = support::LoadedSource::new(
        "_members = set(['z', 'a', 'z'])\ndef callback(value): return list(_members)\ndef entry(ctx): return (callback, _members)\n",
    )
    .await;
    let module = Module::new();
    let values = loaded.invoke(&module, Value::new_none());
    let values = TupleRef::from_value(values).unwrap();
    assert!(values.content()[1].unpack_frozen().is_some());
    let mut lowerer = AnalysisValueLowerer::default();
    assert!(
        lowerer
            .lower(values.content()[1], "plain set input")
            .is_err()
    );
    let mut publisher = super::super::super::Publisher::new(&module);
    let function = publisher
        .capture(values.content()[0], &mut lowerer)
        .unwrap();
    let builder = publisher.seal(&mut lowerer).unwrap();
    drop(lowerer);
    let module = module.freeze().unwrap();
    let frozen =
        super::super::super::FrozenPublisher::new(builder, module.owned_extra_value().unwrap())
            .unwrap();
    let function = frozen.value(function).unwrap();
    drop(loaded);
    drop(module);
    drop(frozen);
    let consumer = Module::new();
    let original = crate::analysis_value::published::materialize_original(
        function.original_origin().unwrap(),
        consumer.frozen_heap(),
    )
    .unwrap();
    let result = Evaluator::new(&consumer)
        .eval_function(original.to_value(), &[Value::new_none()], &[])
        .unwrap();
    assert_eq!(result.to_repr(), "[\"z\", \"a\"]");
}
