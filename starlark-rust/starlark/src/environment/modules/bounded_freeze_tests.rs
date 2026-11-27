use std::mem::size_of;
use std::sync::Arc;

use allocative::Allocative;
use starlark_derive::starlark_value;

use super::Module;
use crate as starlark;
use crate::environment::Globals;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::compiler::def::Def;
use crate::eval::compiler::def::FrozenDef;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenValue;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLike;

fn eval<'v>(module: &'v Module, source: &str) -> Value<'v> {
    let ast = AstModule::parse("freeze.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new(module)
        .eval_module(ast, &Globals::standard())
        .unwrap()
}

fn assert_refusal(error: crate::Error, budget: &ExecutionBudget, expected: ExecutionLimit) {
    assert!(
        matches!(
            error.kind(),
            crate::ErrorKind::Other(cause)
                if cause.downcast_ref::<ExecutionLimit>() == Some(&expected)
        ),
        "{error}"
    );
    assert_eq!(budget.check(), Err(expected));
}

#[test]
fn bounded_freeze_keeps_two_deferred_definitions_and_aliases() {
    let budget = Arc::new(ExecutionBudget::new(40_000, 1 << 20, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    eval(
        &module,
        r#"
def _helper(target): return target
def format(target): return _helper(target)
format_alias = format
"#,
    );
    let before = module
        .get("format")
        .unwrap()
        .downcast_ref::<Def>()
        .unwrap()
        .dump_debug();

    let frozen = module.freeze_bounded().unwrap();
    let format = frozen.get("format").unwrap();
    let alias = frozen.get("format_alias").unwrap();
    assert!(format.value().to_value().ptr_eq(alias.value().to_value()));
    let after = format
        .value()
        .downcast_ref::<FrozenDef>()
        .unwrap()
        .dump_debug();
    assert_eq!(before, after);
    assert!(frozen.get_option("_helper").is_err());

    let row = Module::new_with_execution_budget(budget.clone());
    let function = format.owned_value(row.frozen_heap());
    let argument = Value::testing_new_int(27);
    let result = Evaluator::new(&row)
        .eval_function(function, &[argument], &[])
        .unwrap();
    assert!(result.ptr_eq(argument));
    assert_eq!(budget.check(), Ok(()));
}

const CLOSURE_CYCLE: &str = r#"
def make():
    captured = None
    def format(): return captured
    captured = format
    return format
format = make()
"#;

#[test]
fn bounded_freeze_keeps_capture_cycles_and_refuses_excess_live_depth() {
    for depth in [1, 2] {
        let budget = Arc::new(ExecutionBudget::new(40_000, 1 << 20, depth));
        let module = Module::new_with_execution_budget(budget.clone());
        eval(&module, CLOSURE_CYCLE);
        let result = module.freeze_bounded();
        if depth == 1 {
            assert_refusal(result.unwrap_err(), &budget, ExecutionLimit::FreezeDepth);
        } else {
            let frozen = result.unwrap();
            let value = frozen.get("format").unwrap();
            let row = Module::new_with_execution_budget(budget.clone());
            let function = value.owned_value(row.frozen_heap());
            let output = Evaluator::new(&row)
                .eval_function(function, &[], &[])
                .unwrap();
            assert!(output.ptr_eq(function));
            assert_eq!(budget.check(), Ok(()));
        }
    }
}

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("unadmitted_freeze")]
struct UnadmittedFreeze;

#[starlark_value(type = "unadmitted_freeze")]
impl<'v> StarlarkValue<'v> for UnadmittedFreeze {
    fn try_freeze_directly(&self, _freezer: &Freezer<'_>) -> Option<FreezeResult<FrozenValue>> {
        panic!("bounded freeze called an unadmitted native freeze hook");
    }
}

#[test]
fn bounded_freeze_refuses_before_the_native_hook() {
    let budget = Arc::new(ExecutionBudget::new(1000, 4096, 8));
    let module = Module::new_with_execution_budget(budget.clone());
    module.set("value", module.heap().alloc_simple(UnadmittedFreeze));
    assert_refusal(
        module.freeze_bounded().unwrap_err(),
        &budget,
        ExecutionLimit::UnsupportedOperation {
            operation: "freeze",
            value_type: "unadmitted_freeze",
        },
    );
}

#[test]
fn bounded_freeze_refuses_live_definitions_from_eager_compilation() {
    let producer = Module::new();
    eval(
        &producer,
        "def make():\n    def inner(x): return x\n    return inner",
    );
    let producer = producer.freeze().unwrap();
    let make = producer.get("make").unwrap();
    let budget = Arc::new(ExecutionBudget::new(40_000, 1 << 20, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    let function = make.owned_value(module.frozen_heap());
    let live = Evaluator::new(&module)
        .eval_function(function, &[], &[])
        .unwrap();
    module.set("format", live);
    assert_refusal(
        module.freeze_bounded().unwrap_err(),
        &budget,
        ExecutionLimit::UnsupportedOperation {
            operation: "freeze",
            value_type: "function",
        },
    );
}

#[test]
fn bounded_freeze_admits_anonymous_slot_storage_before_reading_later_roots() {
    const SLOTS: usize = 65_536;
    let slot_bytes = SLOTS * size_of::<Option<FrozenValue>>();
    let budget = Arc::new(ExecutionBudget::new(SLOTS + 10, slot_bytes - 1, 8));
    let module = Module::new_with_execution_budget(budget.clone());
    module.slots().ensure_slots(SLOTS as u32);
    module.set_extra_value(module.heap().alloc_simple(UnadmittedFreeze));
    assert_refusal(
        module.freeze_bounded().unwrap_err(),
        &budget,
        ExecutionLimit::AllocationBytes,
    );
}
