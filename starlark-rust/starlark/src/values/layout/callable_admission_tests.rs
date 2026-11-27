use std::fmt;
use std::sync::Arc;

use allocative::Allocative;

use crate as starlark;
use crate::environment::Globals;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Arguments;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::compiler::def::FrozenDef;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::starlark_value;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct OpaqueCallable {
    trap: bool,
}

starlark::starlark_simple_value!(OpaqueCallable);

impl fmt::Display for OpaqueCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("opaque_callable")
    }
}

// A native type may print as a function without owning a trusted wrapper.
#[starlark_value(type = "function")]
impl<'v> StarlarkValue<'v> for OpaqueCallable {
    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        assert!(!self.trap, "ran an opaque native callable");
        Ok(Value::new_bool(true))
    }
}

fn eval<'v>(module: &'v Module, globals: &Globals, source: &str) -> crate::Result<Value<'v>> {
    let ast = AstModule::parse("callable.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new(module).eval_module(ast, globals)
}

fn strict_budget() -> Arc<ExecutionBudget> {
    Arc::new(ExecutionBudget::new(10_000, 1 << 20, 16).with_bounded_operations())
}

fn assert_limit(error: crate::Error, budget: &ExecutionBudget, limit: ExecutionLimit) {
    assert!(
        matches!(error.kind(), crate::ErrorKind::Other(cause)
        if cause.downcast_ref::<ExecutionLimit>() == Some(&limit)),
        "{error}"
    );
    assert_eq!(budget.check(), Err(limit));
}

fn call_refusal(value_type: &'static str) -> ExecutionLimit {
    ExecutionLimit::UnsupportedOperation {
        operation: "callable invocation",
        value_type,
    }
}

#[test]
fn strict_invocation_rejects_opaque_and_noncallable_values_before_the_hook() {
    let globals = Globals::standard();
    let ordinary = Module::new();
    ordinary.set(
        "callee",
        ordinary.heap().alloc(OpaqueCallable { trap: false }),
    );
    assert_eq!(
        eval(&ordinary, &globals, "callee()").unwrap().unpack_bool(),
        Some(true)
    );

    for opaque in [true, false] {
        let budget = strict_budget();
        let module = Module::new_with_execution_budget(budget.clone());
        let (callee, value_type) = if opaque {
            (
                module.heap().alloc(OpaqueCallable { trap: true }),
                "function",
            )
        } else {
            (Value::new_none(), "NoneType")
        };
        module.set("callee", callee);
        assert_limit(
            eval(&module, &globals, "callee()").unwrap_err(),
            &budget,
            call_refusal(value_type),
        );
    }

    let mut builder = GlobalsBuilder::standard();
    builder.set("opaque", OpaqueCallable { trap: true });
    let globals = builder.build();
    let producer = Module::new();
    eval(&producer, &globals, "def relay(): return opaque()").unwrap();
    let producer = producer.freeze().unwrap();
    let relay = producer.get("relay").unwrap();
    let bytecode = relay
        .value()
        .downcast_ref::<FrozenDef>()
        .unwrap()
        .dump_debug();
    assert!(bytecode.contains(": CallFrozenPos "), "{bytecode}");

    let budget = strict_budget();
    let module = Module::new_with_execution_budget(budget.clone());
    let relay = relay.owned_value(module.frozen_heap());
    assert_limit(
        Evaluator::new(&module)
            .eval_function(relay, &[], &[])
            .unwrap_err(),
        &budget,
        call_refusal("function"),
    );
}

#[test]
fn strict_invocation_preserves_real_defs_and_native_wrappers() {
    let globals = Globals::standard();
    let producer = Module::new();
    eval(
        &producer,
        &globals,
        "def frozen_identity(value): return value\nfrozen_suffix = 'fixture.rs'.endswith",
    )
    .unwrap();
    let producer = producer.freeze().unwrap();

    let budget = strict_budget();
    let module = Module::new_with_execution_budget(budget.clone());
    let heap = module.heap();
    let moving_def = eval(&module, &globals, "def moving(value): return value\nmoving").unwrap();
    let frozen_def = producer
        .get("frozen_identity")
        .unwrap()
        .owned_value(module.frozen_heap());
    let native = globals
        .get("str")
        .unwrap()
        .unpack_frozen()
        .unwrap()
        .to_value();
    let input = heap.alloc("same argument value");
    let mut evaluator = Evaluator::new(&module);
    for callable in [moving_def, frozen_def, native] {
        assert!(
            evaluator
                .eval_function(callable, &[input], &[])
                .unwrap()
                .ptr_eq(input)
        );
    }

    let live_method = heap
        .alloc("fixture.rs")
        .get_attr("endswith", heap)
        .unwrap()
        .unwrap();
    let frozen_method = producer
        .get("frozen_suffix")
        .unwrap()
        .owned_value(module.frozen_heap());
    for callable in [live_method, frozen_method] {
        let result = evaluator
            .eval_function(callable, &[heap.alloc(".rs")], &[])
            .unwrap();
        assert_eq!(result.unpack_bool(), Some(true));
    }
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn strict_invocation_preserves_the_first_refusal_before_call_stack_entry() {
    let budget = strict_budget();
    let module = Module::new_with_execution_budget(budget.clone());
    budget.refuse(ExecutionLimit::TextDepth);
    assert_limit(
        Value::new_none()
            .invoke_pos(&[], &mut Evaluator::new(&module))
            .unwrap_err(),
        &budget,
        ExecutionLimit::TextDepth,
    );
}
