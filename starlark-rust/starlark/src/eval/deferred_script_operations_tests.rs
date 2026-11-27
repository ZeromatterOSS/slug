use std::cell::Cell;

use crate as starlark;
use crate::environment::Globals;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::compiler::def::FrozenDef;
use crate::starlark_module;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::syntax::DialectTypes;
use crate::values::ValueLike;

thread_local! {
    static NATIVE_CALLS: Cell<usize> = const { Cell::new(0) };
}

#[starlark_module]
fn testing_globals(builder: &mut GlobalsBuilder) {
    #[starlark(speculative_exec_safe)]
    fn tick(value: i32) -> anyhow::Result<i32> {
        NATIVE_CALLS.with(|calls| calls.set(calls.get() + 1));
        Ok(value)
    }
}

fn parse(source: &str) -> AstModule {
    AstModule::parse("operations.star", source.to_owned(), &Dialect::Standard).unwrap()
}

#[test]
fn prepared_values_and_frozen_definitions_defer_runtime_operations() {
    NATIVE_CALLS.set(0);
    let globals = GlobalsBuilder::standard().with(testing_globals).build();
    let caller = Module::new();
    caller.set("label", caller.heap().alloc("caller"));
    let deferred = Module::new_with_deferred_script_operations();
    let mut prepare_eval = Evaluator::new(&caller);
    let prepared = prepare_eval
        .prepare_module_in(
            &deferred,
            parse(
                r#"
label = "original"
alias = tick
ready = tick(3)
def render():
    items = {"label": label, "nested": ((), ())}
    prefix = "pre" + "fix"
    return "%s:%s" % (prefix, "{}:{}:{}".format(alias(7), items["label"], items["nested"]))
"#,
            ),
            &globals,
        )
        .unwrap();
    assert_eq!(NATIVE_CALLS.get(), 0);
    prepare_eval.eval_prepared_module(&prepared).unwrap();
    drop(prepared);
    drop(prepare_eval);
    assert_eq!(NATIVE_CALLS.get(), 1);
    assert_eq!(deferred.get("ready").unwrap().unpack_i32(), Some(3));

    let frozen = deferred.freeze().unwrap();
    assert_eq!(NATIVE_CALLS.get(), 1);
    let render = frozen
        .get("render")
        .unwrap()
        .owned_value(caller.frozen_heap());
    let bytecode = render.downcast_ref::<FrozenDef>().unwrap().dump_debug();
    assert!(bytecode.contains("Add"), "{bytecode}");
    assert!(bytecode.contains("Percent"), "{bytecode}");
    assert!(bytecode.contains("DictNPop"), "{bytecode}");
    assert_eq!(
        Evaluator::new(&caller)
            .eval_function(render, &[], &[])
            .unwrap()
            .unpack_str(),
        Some("prefix:7:original:((), ())")
    );
    assert_eq!(NATIVE_CALLS.get(), 2);
}

#[test]
fn populated_alias_and_refrozen_closure_keep_defining_mode() {
    NATIVE_CALLS.set(0);
    let globals = GlobalsBuilder::standard().with(testing_globals).build();
    let producer = Module::new_with_deferred_script_operations();
    Evaluator::new(&producer)
        .eval_module(
            parse(
                r#"
alias = tick
def factory(prefix):
    def render():
        return prefix + ("val" + "ue") + str(alias(3))
    return render
"#,
            ),
            &globals,
        )
        .unwrap();
    assert_eq!(NATIVE_CALLS.get(), 0);
    let producer = producer.freeze().unwrap();
    assert_eq!(NATIVE_CALLS.get(), 0);

    let consumer = Module::new();
    let factory = producer
        .get("factory")
        .unwrap()
        .owned_value(consumer.frozen_heap());
    let prefix = consumer.heap().alloc("captured:");
    let closure = Evaluator::new(&consumer)
        .eval_function(factory, &[prefix], &[])
        .unwrap();
    consumer.set("closure", closure);
    let consumer = consumer.freeze().unwrap();
    assert_eq!(NATIVE_CALLS.get(), 0);

    let closure = consumer.get("closure").unwrap();
    let def = closure.value().downcast_ref::<FrozenDef>().unwrap();
    let bytecode = def.dump_debug();
    assert!(bytecode.contains("Add"), "{bytecode}");
    let runtime = Module::new();
    let result = Evaluator::new(&runtime)
        .eval_function(closure.value(), &[], &[])
        .unwrap();
    assert_eq!(result.unpack_str(), Some("captured:value3"));
    assert_eq!(NATIVE_CALLS.get(), 1);
}

#[test]
fn ordinary_compilation_still_speculates() {
    NATIVE_CALLS.set(0);
    let globals = GlobalsBuilder::standard().with(testing_globals).build();
    let module = Module::new();
    Evaluator::new(&module)
        .eval_module(parse("def dormant(): return tick(9)"), &globals)
        .unwrap();
    assert_eq!(NATIVE_CALLS.get(), 1);
}

#[test]
fn deferred_compilation_refuses_typechecking_controls() {
    let globals = Globals::standard();
    let module = Module::new_with_deferred_script_operations();
    let mut eval = Evaluator::new(&module);
    let diagnostic = "requires an untyped dialect without static typechecking";
    let dialect = Dialect {
        enable_types: DialectTypes::ParseOnly,
        ..Dialect::Standard
    };
    let annotated = AstModule::parse("types.star", "x: int = 1".to_owned(), &dialect).unwrap();
    let Err(error) = eval.prepare_module(annotated, &globals) else {
        panic!("accepted the typed dialect");
    };
    assert!(error.to_string().contains(diagnostic));
    assert!(
        eval.eval_module(parse("# @starlark-rust: typecheck\nx = 1"), &globals)
            .unwrap_err()
            .to_string()
            .contains(diagnostic)
    );
    eval.enable_static_typechecking(true);
    assert!(
        eval.eval_module(parse("x = 1"), &globals)
            .unwrap_err()
            .to_string()
            .contains(diagnostic)
    );
    assert!(module.get("x").is_none());
}
