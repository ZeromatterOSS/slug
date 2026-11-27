use std::sync::Arc;

use allocative::Allocative;
use starlark_derive::starlark_value;

use crate as starlark;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::compiler::def::FrozenDef;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::FrozenValue;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLike;

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("predicate_canary")]
struct PredicateCanary {
    deny_legacy: bool,
}

starlark::starlark_simple_value!(PredicateCanary);

#[starlark_value(type = "predicate_canary")]
impl<'v> StarlarkValue<'v> for PredicateCanary {
    fn equals(&self, _other: Value<'v>) -> crate::Result<bool> {
        assert!(!self.deny_legacy, "called an unadmitted native equality");
        Ok(false)
    }

    fn to_bool(&self) -> bool {
        assert!(
            !self.deny_legacy,
            "called an unadmitted native truth conversion"
        );
        true
    }
}

fn eval<'v>(module: &'v Module, source: &str) -> crate::Result<Value<'v>> {
    let ast = AstModule::parse("predicates.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new(module).eval_module(ast, &Globals::standard())
}

fn strict_budget(work: usize) -> Arc<ExecutionBudget> {
    Arc::new(ExecutionBudget::new(work, 1 << 20, 16).with_bounded_operations())
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
fn strict_deferred_predicates_refuse_native_overrides_before_dispatch() {
    for (source, operation) in [
        ("subject == None", "equality"),
        ("subject != 1", "equality"),
        ("not subject", "truth testing"),
        ("1 if subject else 2", "truth testing"),
        ("subject or True", "truth testing"),
    ] {
        let budget = strict_budget(50_000);
        let module = Module::new_with_execution_budget(budget.clone());
        module.set(
            "subject",
            module.heap().alloc(PredicateCanary { deny_legacy: true }),
        );
        assert_limit(
            eval(&module, source).unwrap_err(),
            &budget,
            ExecutionLimit::UnsupportedOperation {
                operation,
                value_type: "predicate_canary",
            },
        );
    }
}

#[test]
fn strict_frozen_operands_cover_regular_specialized_and_branch_bytecode() {
    let producer = Module::new();
    producer.set(
        "opaque_constant",
        producer.heap().alloc(PredicateCanary { deny_legacy: true }),
    );
    eval(
        &producer,
        r#"
def equal(x, y):
    return x == y
def constant_equal(x):
    return x == opaque_constant
def integer_different(x):
    return x != 1
def string_equal(x):
    return x == "constant"
def pointer_equal(x):
    return x == None
def negate(x):
    return not x
def branch(x):
    return 1 if x else 2
def inverted_branch(x):
    if not x:
        return 1
    return 2
"#,
    )
    .unwrap();
    let producer = producer.freeze().unwrap();

    for (name, opcode, other, refusal) in [
        (
            "equal",
            "Eq",
            Some(FrozenValue::new_none()),
            Some("equality"),
        ),
        ("constant_equal", "EqConst", None, Some("equality")),
        ("integer_different", "EqInt", None, Some("equality")),
        ("string_equal", "EqStr", None, None),
        ("pointer_equal", "EqPtr", None, None),
        ("negate", "Not", None, Some("truth testing")),
        ("branch", "IfNotBr", None, Some("truth testing")),
        ("inverted_branch", "IfBr", None, Some("truth testing")),
    ] {
        let program = producer.get(name).unwrap();
        let bytecode = program
            .value()
            .downcast_ref::<FrozenDef>()
            .unwrap()
            .dump_debug();
        assert!(
            bytecode.lines().any(|line| {
                line.split_once(": ")
                    .and_then(|(_, instruction)| instruction.split_ascii_whitespace().next())
                    == Some(opcode)
            }),
            "{name} did not use {opcode}: {bytecode}"
        );
        let budget = strict_budget(50_000);
        let module = Module::new_with_execution_budget(budget.clone());
        let subject = module.heap().alloc(PredicateCanary { deny_legacy: true });
        let program = program.owned_value(module.frozen_heap());
        let mut eval = Evaluator::new(&module);
        let result = match other {
            Some(other) => eval.eval_function(program, &[subject, other.to_value()], &[]),
            None => eval.eval_function(program, &[subject], &[]),
        };
        match refusal {
            Some(operation) => assert_limit(
                result.unwrap_err(),
                &budget,
                ExecutionLimit::UnsupportedOperation {
                    operation,
                    value_type: "predicate_canary",
                },
            ),
            None => {
                assert_eq!(result.unwrap().unpack_bool(), Some(false));
                assert_eq!(budget.check(), Ok(()));
            }
        }
    }
}

#[test]
fn strict_equality_charges_string_comparison_before_scanning() {
    let left = "x".repeat(513);
    let right = format!("{}y", "x".repeat(512));
    for work in [4, 3] {
        let budget = strict_budget(work);
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        let left = heap.alloc(left.as_str());
        let right = heap.alloc(right.as_str());
        let result = left.equals_in_heap(right, heap);
        if work == 4 {
            assert!(!result.unwrap());
            assert_eq!(budget.charge(1, 0), Err(ExecutionLimit::Work));
        } else {
            assert_limit(result.unwrap_err(), &budget, ExecutionLimit::Work);
        }
    }
}

#[test]
fn strict_deferred_predicates_allow_numeric_count_and_boolean_filter() {
    let budget = strict_budget(50_000);
    let module = Module::new_with_execution_budget(budget.clone());
    module.set("keys", module.heap().alloc(vec!["edition"]));
    let source = r#"
def check(keys):
    if len(keys) != 1:
        return False
    flags = [flag for flag in [True, False] if flag]
    return len(flags) == 1 and flags[0]
check(keys)
"#;
    assert_eq!(eval(&module, source).unwrap().unpack_bool(), Some(true));
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn regular_and_cooperative_budget_predicates_keep_legacy_dispatch() {
    let budget = Arc::new(ExecutionBudget::new(50_000, 1 << 20, 16));
    for module in [
        Module::new(),
        Module::new_with_execution_budget(budget.clone()),
    ] {
        module.set(
            "subject",
            module.heap().alloc(PredicateCanary { deny_legacy: false }),
        );
        assert_eq!(
            eval(&module, "subject != None and not (not subject)")
                .unwrap()
                .unpack_bool(),
            Some(true)
        );
    }
    assert_eq!(budget.check(), Ok(()));
}
