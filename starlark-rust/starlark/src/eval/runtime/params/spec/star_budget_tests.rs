use std::mem::size_of;
use std::sync::Arc;

use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::Value;
use crate::values::tuple::TupleRef;

fn run<'v>(module: &'v Module, source: &str, spread: Value<'v>) -> crate::Result<Value<'v>> {
    let ast = AstModule::parse("call.star", source.to_owned(), &Dialect::Standard).unwrap();
    module.set("spread", spread);
    Evaluator::new(module).eval_module(ast, &Globals::standard())
}

fn spread(module: &Module, count: i32) -> Value<'_> {
    let values: Vec<_> = (1..=count).map(Value::testing_new_int).collect();
    module.heap().alloc_tuple(&values)
}

#[test]
fn bounded_signature_keeps_star_capture_order_and_reserves_storage() {
    let source = "def capture(head, *tail):\n    return tail\ncapture(0, *spread)";
    let budget = Arc::new(ExecutionBudget::new(10_000, 1_024, 16));
    for module in [
        Module::new(),
        Module::new_with_execution_budget(budget.clone()),
    ] {
        let result = run(&module, source, spread(&module, 3)).unwrap();
        assert!(
            TupleRef::from_value(result)
                .unwrap()
                .iter()
                .map(|value| value.unpack_i32().unwrap())
                .eq([1, 2, 3])
        );
    }
    assert_eq!(budget.check(), Ok(()));

    let budget = Arc::new(ExecutionBudget::new(10_000, 0, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    assert!(run(&module, source, spread(&module, 3)).is_err());
    assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));

    // A single scratch entry fits, but copying it into the tuple also costs credits.
    let budget = Arc::new(ExecutionBudget::new(10_000, size_of::<Value>(), 16));
    let module = Module::new_with_execution_budget(budget.clone());
    assert!(run(&module, source, spread(&module, 1)).is_err());
    assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
}

#[test]
fn bounded_signature_counts_excess_star_arguments_without_scratch() {
    let source = "def one(arg):\n    return arg\none(*spread)";
    let budget = Arc::new(ExecutionBudget::new(100, 0, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    let error = run(&module, source, spread(&module, 4)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Found 3 extra positional argument(s)")
    );
    assert_eq!(budget.check(), Ok(()));

    let error = run(&module, source, Value::new_bool(true)).unwrap_err();
    assert!(error.to_string().contains("*args is not iterable"));
    assert_eq!(budget.check(), Ok(()));

    assert!(run(&module, source, spread(&module, 256)).is_err());
    assert_eq!(budget.check(), Err(ExecutionLimit::Work));
}
