use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use starlark_derive::starlark_value;

use crate as starlark;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::Heap;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;

fn eval<'v>(module: &'v Module, source: &str) -> crate::Result<Value<'v>> {
    let ast = AstModule::parse("operators.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new(module).eval_module(ast, &Globals::standard())
}

fn strict_budget(work: usize, bytes: usize) -> Arc<ExecutionBudget> {
    Arc::new(ExecutionBudget::new(work, bytes, 16).with_bounded_operations())
}

fn assert_limit(error: crate::Error, budget: &ExecutionBudget, expected: ExecutionLimit) {
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
fn strict_string_operators_work_in_deferred_and_ordinary_frozen_bytecode() {
    let ordinary = Module::new();
    eval(
        &ordinary,
        r#"
def render(left, right):
    return '<%s>' % ('%s/%s' % (left + right, right))
"#,
    )
    .unwrap();
    let frozen = ordinary.freeze().unwrap();

    let module = Module::new_with_execution_budget(strict_budget(50_000, 1 << 20));
    module.import_public_symbols(&frozen);
    module.set("left", module.heap().alloc("é"));
    module.set("right", module.heap().alloc("雪"));
    for source in [
        "'<%s>' % ('%s/%s' % (left + right, right))",
        "render(left, right)",
    ] {
        assert_eq!(
            eval(&module, source).unwrap().unpack_str(),
            Some("<é雪/雪>")
        );
    }
}

#[test]
fn strict_string_add_preserves_empty_aliases_and_prepays_the_result() {
    let budget = strict_budget(2, 0);
    let module = Module::new_with_execution_budget(budget.clone());
    let heap = module.heap();
    let value = heap.alloc("existing allocation");
    let empty = heap.alloc("");
    assert!(empty.add(value, heap).unwrap().ptr_eq(value));
    assert!(value.add(empty, heap).unwrap().ptr_eq(value));
    assert_eq!(budget.check(), Ok(()));

    let left = "a".repeat(64);
    let right = "雪";
    let ordinary = Module::new();
    let heap = ordinary.heap();
    let left_value = heap.alloc(left.as_str());
    let right_value = heap.alloc(right);
    left_value.add(right_value, heap).unwrap();
    let (reserved, available) = (heap.allocated_bytes(), heap.available_bytes());
    let expected = left_value.add(right_value, heap).unwrap().unpack_str();
    assert_eq!(heap.allocated_bytes(), reserved);
    let result_bytes = available - heap.available_bytes();
    assert!(result_bytes > 1);

    for available in [result_bytes - 1, result_bytes] {
        let budget = strict_budget(50_000, available);
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        let left = heap.alloc(left.as_str());
        let right = heap.alloc(right);
        let before = (heap.allocated_bytes(), heap.available_bytes());
        let result = left.add(right, heap);
        if available == result_bytes {
            assert_eq!(result.unwrap().unpack_str(), expected);
            assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
        } else {
            assert_limit(
                result.unwrap_err(),
                &budget,
                ExecutionLimit::AllocationBytes,
            );
            assert_eq!((heap.allocated_bytes(), heap.available_bytes()), before);
        }
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct OperatorCanary;

starlark::starlark_simple_value!(OperatorCanary);

impl fmt::Display for OperatorCanary {
    fn fmt(&self, _writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("rendered an unadmitted operator operand");
    }
}

#[starlark_value(type = "operator_canary")]
impl<'v> StarlarkValue<'v> for OperatorCanary {
    fn percent(&self, _other: Value<'v>, _heap: Heap<'v>) -> crate::Result<Value<'v>> {
        panic!("invoked an unadmitted percent callback");
    }

    fn add(&self, _other: Value<'v>, _heap: Heap<'v>) -> Option<crate::Result<Value<'v>>> {
        panic!("invoked an unadmitted add callback");
    }

    fn radd(&self, _other: Value<'v>, _heap: Heap<'v>) -> Option<crate::Result<Value<'v>>> {
        panic!("invoked an unadmitted reverse add callback");
    }
}

#[test]
fn strict_string_operators_refuse_native_operands_before_dispatch() {
    let ordinary = Module::new();
    eval(
        &ordinary,
        r#"
def interpolate(left, right):
    return left % right

def concatenate(left, right):
    return left + right
"#,
    )
    .unwrap();
    let frozen = ordinary.freeze().unwrap();

    for (source, operation) in [
        ("canary % 'a'", "percent operator"),
        ("interpolate(canary, 'a')", "percent operator"),
        ("canary + 'a'", "addition"),
        ("'a' + canary", "addition"),
        ("concatenate('a', canary)", "addition"),
    ] {
        let budget = strict_budget(50_000, 1 << 20);
        let module = Module::new_with_execution_budget(budget.clone());
        module.import_public_symbols(&frozen);
        module.set("canary", module.heap().alloc(OperatorCanary));
        assert_limit(
            eval(&module, source).unwrap_err(),
            &budget,
            ExecutionLimit::UnsupportedOperation {
                operation,
                value_type: "operator_canary",
            },
        );
    }
}

#[test]
fn legacy_execution_budget_still_allows_numeric_operators() {
    let budget = Arc::new(ExecutionBudget::new(50_000, 1 << 20, 16));
    let module = Module::new_with_execution_budget(budget);
    module.set("number", module.heap().alloc(7));
    assert_eq!(
        eval(&module, "number % 3 + number").unwrap().unpack_i32(),
        Some(8)
    );
}
