use std::mem::size_of;
use std::sync::Arc;

use starlark_map::hashbrown_table;

use super::Def;
use crate::collections::symbol::symbol::Symbol;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::runtime::params::spec::ParameterKind;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::Heap;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::function::FUNCTION_TYPE;

const UNSUPPORTED: ExecutionLimit = ExecutionLimit::UnsupportedOperation {
    operation: "function definition",
    value_type: FUNCTION_TYPE,
};
const SOURCE_NAME: &str = "bounded_def.star";

fn eval<'v>(eval: &mut Evaluator<'v, '_, '_>, source: &str) -> crate::Result<Value<'v>> {
    let dialect = Dialect {
        enable_keyword_only_arguments: true,
        enable_positional_only_arguments: true,
        ..Dialect::Standard
    };
    let ast = AstModule::parse(SOURCE_NAME, source.to_owned(), &dialect).unwrap();
    eval.eval_module(ast, &Globals::standard())
}

fn assert_refusal(error: crate::Error, budget: &ExecutionBudget, expected: ExecutionLimit) {
    assert!(
        matches!(
            error.kind(),
            crate::ErrorKind::Other(cause) if cause.downcast_ref::<ExecutionLimit>() == Some(&expected)
        ),
        "{error}"
    );
    assert_eq!(budget.check(), Err(expected));
}

#[test]
fn bounded_creation_admits_exact_signature_and_arena_allocation() {
    let parameter = "a".repeat(513);
    let source = format!("def format({parameter}=17): return {parameter}");
    let arena_bytes = Heap::complex_allocation_bytes::<Def<'static>>();
    let (_, table_bytes) = hashbrown_table::fresh_allocation::<(Symbol, u32)>(1).unwrap();
    let bytes = arena_bytes
        + table_bytes
        + SOURCE_NAME.len()
        + ".format".len()
        + size_of::<String>()
        + size_of::<ParameterKind<Value>>()
        + parameter.len()
        + parameter.len().next_multiple_of(size_of::<usize>());
    for available in [bytes, bytes - 1] {
        let budget =
            Arc::new(ExecutionBudget::new(50_000, available, 16).with_bounded_operations());
        let module = Module::new_with_execution_budget(budget.clone());
        let mut evaluator = Evaluator::new_with_reused_call_storage_for_test(&module);
        let result = eval(&mut evaluator, &source);
        if available == bytes {
            result.unwrap();
            let format = module.get("format").unwrap().downcast_ref::<Def>().unwrap();
            let (names, kinds, _) = format.parameters.inspection_parts();
            assert_eq!(names, &[parameter.as_str()]);
            assert!(
                matches!(kinds, [ParameterKind::Defaulted(value)] if value.unpack_i32() == Some(17))
            );
            assert_eq!(
                module
                    .heap()
                    .allocated_summary()
                    .summary()
                    .get(FUNCTION_TYPE)
                    .map(|&(instances, _)| instances),
                Some(1)
            );
            assert!(evaluator.function_symbol_session().started());
            assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
        } else {
            assert_refusal(
                result.unwrap_err(),
                &budget,
                ExecutionLimit::AllocationBytes,
            );
            assert!(module.get("format").is_none());
            assert!(
                !module
                    .heap()
                    .allocated_summary()
                    .summary()
                    .contains_key(FUNCTION_TYPE)
            );
            assert!(!evaluator.function_symbol_session().started());
        }
    }
}

#[test]
fn deferred_signature_keeps_defaults_and_parameter_styles_across_freeze() {
    let budget = Arc::new(ExecutionBudget::new(50_000, 1 << 20, 16).with_bounded_operations());
    let module = Module::new_with_execution_budget(budget.clone());
    eval(
        &mut Evaluator::new_with_reused_call_storage_for_test(&module),
        r#"
def helper(target):
    return target
def format(pos, /, target=17, *, named=23):
    return helper((pos, target, named))
"#,
    )
    .unwrap();
    let frozen = module.freeze_bounded().unwrap();
    let row = Module::new_with_execution_budget(budget.clone());
    let function = frozen.get("format").unwrap().owned_value(row.frozen_heap());
    let result = Evaluator::new_with_reused_call_storage_for_test(&row)
        .eval_function(
            function,
            &[Value::testing_new_int(11)],
            &[("named", Value::testing_new_int(37))],
        )
        .unwrap();
    let tuple = crate::values::tuple::TupleRef::from_value(result).unwrap();
    assert!(tuple.content().iter().map(|value| value.unpack_i32()).eq([
        Some(11),
        Some(17),
        Some(37)
    ]));
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn strict_definition_refuses_variadics_and_captures_before_publishing() {
    for (source, prior_defs) in [
        ("def format(*args, **kwargs): return None", 0),
        (
            "def format(target):\n    def capture(): return target\n    return capture",
            0,
        ),
        (
            r#"
def make():
    def capture(value=17):
        return pending
    pending = 13
    return capture
format = make()
"#,
            1,
        ),
    ] {
        let budget = Arc::new(ExecutionBudget::new(50_000, 1 << 20, 16).with_bounded_operations());
        let module = Module::new_with_execution_budget(budget.clone());
        let mut evaluator = Evaluator::new_with_reused_call_storage_for_test(&module);
        assert_refusal(
            eval(&mut evaluator, source).unwrap_err(),
            &budget,
            UNSUPPORTED,
        );
        let allocated = module.heap().allocated_summary().summary();
        assert_eq!(
            allocated.get(FUNCTION_TYPE).map_or(0, |entry| entry.0),
            prior_defs
        );
        assert!(!allocated.contains_key("value_captured"));
        assert!(module.get("format").is_none());
    }
}

#[test]
fn strict_caller_refuses_runtime_definition_from_eager_frozen_bytecode() {
    let producer = Module::new();
    eval(
        &mut Evaluator::new(&producer),
        "def make():\n    def nested(value=17): return value\n    return nested",
    )
    .unwrap();
    let producer = producer.freeze().unwrap();
    let make = producer.get("make").unwrap();

    let budget = Arc::new(ExecutionBudget::new(50_000, 1 << 20, 16).with_bounded_operations());
    let module = Module::new_with_execution_budget(budget.clone());
    let result = Evaluator::new_with_reused_call_storage_for_test(&module).eval_function(
        make.owned_value(module.frozen_heap()),
        &[],
        &[],
    );
    assert_refusal(result.unwrap_err(), &budget, UNSUPPORTED);
    assert!(
        !module
            .heap()
            .allocated_summary()
            .summary()
            .contains_key(FUNCTION_TYPE)
    );
}
