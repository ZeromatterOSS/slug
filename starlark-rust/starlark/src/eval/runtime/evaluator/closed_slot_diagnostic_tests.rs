use std::sync::Arc;

use crate::environment::FrozenModule;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::Value;

fn modes() -> [(Option<Arc<ExecutionBudget>>, bool); 4] {
    let budget = || ExecutionBudget::new(50_000, 1 << 20, 16);
    [
        (None, false),
        (Some(Arc::new(budget())), false),
        (Some(Arc::new(budget().with_bounded_operations())), false),
        (
            Some(Arc::new(budget().with_closed_formatter_bytecode())),
            true,
        ),
    ]
}

fn module(budget: &Option<Arc<ExecutionBudget>>) -> Module {
    match budget {
        Some(budget) => Module::new_with_execution_budget(budget.clone()),
        None => Module::new(),
    }
}

fn eval<'v>(module: &'v Module, source: &str) -> crate::Result<Value<'v>> {
    let dialect = Dialect {
        enable_top_level_stmt: true,
        ..Dialect::Standard
    };
    let ast = AstModule::parse("slot_diagnostics.star", source.to_owned(), &dialect).unwrap();
    Evaluator::new_with_reused_call_storage_for_test(module).eval_module(ast, &Globals::standard())
}

fn invoke<'v>(
    module: &'v Module,
    frozen: &FrozenModule,
    function: &str,
    arguments: &[Value<'v>],
) -> crate::Result<Value<'v>> {
    let function = frozen
        .get(function)
        .unwrap()
        .owned_value(module.frozen_heap());
    Evaluator::new_with_reused_call_storage_for_test(module).eval_function(function, arguments, &[])
}

fn assert_missing(error: crate::Error, budget: &Option<Arc<ExecutionBudget>>, expected: &str) {
    assert!(
        matches!(error.kind(), crate::ErrorKind::Other(_)),
        "{error}"
    );
    assert_eq!(error.without_diagnostic().to_string(), expected);
    if let Some(budget) = budget {
        assert_eq!(budget.check(), Ok(()));
    }
}

#[test]
fn closed_profile_keeps_a_fixed_local_miss_and_reads_assigned_locals() {
    let symbol = "local_".to_owned() + &"x".repeat(1024);
    let source = Module::new_with_deferred_script_operations();
    eval(
        &source,
        &format!(
            "def run(initialize):\n    if initialize:\n        {symbol} = True\n    return {symbol}"
        ),
    )
    .unwrap();
    let frozen = source.freeze().unwrap();
    let ordinary = format!("Local variable `{symbol}` referenced before assignment");

    for (budget, closed) in modes() {
        let module = module(&budget);
        let assigned = invoke(&module, &frozen, "run", &[Value::new_bool(true)]).unwrap();
        assert_eq!(assigned.unpack_bool(), Some(true));
        let missing = invoke(&module, &frozen, "run", &[Value::new_bool(false)]).unwrap_err();
        let expected = if closed {
            "Local variable referenced before assignment"
        } else {
            ordinary.as_str()
        };
        assert_missing(missing, &budget, expected);
    }
}

#[test]
fn closed_profile_keeps_a_fixed_module_miss_for_moving_and_frozen_modules() {
    let symbol = "module_".to_owned() + &"x".repeat(1024);
    let moving = format!("def run(): return {symbol}\nrun()\n{symbol} = True");
    let freezer = |initialize| {
        let source = Module::new_with_deferred_script_operations();
        source.set("initialize", Value::new_bool(initialize));
        eval(
            &source,
            &format!("def run(): return {symbol}\nif initialize:\n    {symbol} = True"),
        )
        .unwrap();
        source.freeze().unwrap()
    };
    let assigned = freezer(true);
    let missing = freezer(false);
    let ordinary = format!("Local variable `{symbol}` referenced before assignment");

    for (budget, closed) in modes() {
        let caller = module(&budget);
        let result = invoke(&caller, &assigned, "run", &[]).unwrap();
        assert_eq!(result.unpack_bool(), Some(true));
        let expected = if closed {
            "Module variable referenced before assignment"
        } else {
            ordinary.as_str()
        };
        assert_missing(
            invoke(&caller, &missing, "run", &[]).unwrap_err(),
            &budget,
            expected,
        );
        let moving_caller = module(&budget);
        assert_missing(
            eval(&moving_caller, &moving).unwrap_err(),
            &budget,
            expected,
        );
    }
}
