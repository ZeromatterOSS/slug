use std::fmt;
use std::sync::Arc;

use allocative::Allocative;

use super::endswith_bounded;
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
use crate::values::starlark_value;

fn eval<'v>(module: &'v Module, source: &str) -> crate::Result<Value<'v>> {
    let ast = AstModule::parse("endswith.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new_with_reused_call_storage_for_test(module).eval_module(ast, &Globals::standard())
}

#[test]
fn endswith_preserves_values_and_full_named_type_errors() {
    let ordinary = Module::new();
    let budget = Arc::new(ExecutionBudget::new(4096, 4096, 16));
    let bounded = Module::new_with_execution_budget(budget.clone());
    for (source, expected) in [
        ("'café雪'.endswith('é雪')", true),
        ("'abc'.endswith('abcd')", false),
        ("'abc'.endswith(('a', 'bc'))", true),
        ("'abc'.endswith(('a', 'b'))", false),
        ("'abc'.endswith(('', 'x'))", true),
        ("'abc'.endswith(())", false),
    ] {
        for module in [&ordinary, &bounded] {
            assert_eq!(eval(module, source).unwrap().unpack_bool(), Some(expected));
        }
    }
    for source in ["'abc'.endswith(['abc'])", "'abc'.endswith(('abc', False))"] {
        let ordinary = eval(&ordinary, source).unwrap_err();
        let bounded = eval(&bounded, source).unwrap_err();
        assert!(ordinary.to_string().contains("Type of parameter `suffix`"));
        assert_eq!(
            bounded.without_diagnostic().to_string(),
            ordinary.without_diagnostic().to_string(),
        );
    }
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn endswith_prepays_tuple_scan_and_comparison_without_scratch() {
    Heap::temp(|heap| {
        let text = "é".repeat(128) + "x";
        let suffix = heap.alloc((text.as_str(), "no match", ""));
        // One dispatch, three validations, one visit, one comparison and two
        // comparison byte units. The final two already validated items need no compare.
        let exact = ExecutionBudget::new(8, 0, 8);
        assert!(endswith_bounded(&text, suffix, &exact).unwrap());
        assert_eq!(exact.charge(1, 0), Err(ExecutionLimit::Work));
        for short in [3, 7] {
            let budget = ExecutionBudget::new(short, 0, 8);
            assert!(endswith_bounded(&text, suffix, &budget).is_err());
            assert_eq!(budget.check(), Err(ExecutionLimit::Work));
        }

        let too_long = ExecutionBudget::new(2, 0, 8);
        assert!(!endswith_bounded("x", heap.alloc(text.as_str()), &too_long).unwrap());
        assert_eq!(too_long.charge(1, 0), Err(ExecutionLimit::Work));
    });
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct UnmodeledSuffix;

impl fmt::Display for UnmodeledSuffix {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("unmodeled suffix representation ran");
    }
}

starlark::starlark_simple_value!(UnmodeledSuffix);

#[starlark_value(type = "endswith_test_operand")]
impl<'v> StarlarkValue<'v> for UnmodeledSuffix {}

#[test]
fn endswith_rejects_an_unmodeled_repr_even_after_a_match() {
    let budget = Arc::new(ExecutionBudget::new(1024, 4096, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    module.set("unmodeled", module.heap().alloc(UnmodeledSuffix));
    assert!(eval(&module, "'abc'.endswith(('abc', unmodeled))").is_err());
    assert_eq!(
        budget.check(),
        Err(ExecutionLimit::UnsupportedText("endswith_test_operand")),
    );
}

#[test]
fn frozen_ordinary_endswith_uses_the_callers_work_budget() {
    let producer = Module::new();
    eval(
        &producer,
        "def run(text, suffix): return text.endswith(suffix)",
    )
    .unwrap();
    let producer = producer.freeze().unwrap();
    let function = producer.get("run").unwrap();
    let text = "x".repeat(32_769);
    for (suffix, should_match) in [("x", true), (text.as_str(), false)] {
        let budget = Arc::new(ExecutionBudget::new(128, 0, 8));
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        let function = function.owned_value(module.frozen_heap());
        let result = Evaluator::new_with_reused_call_storage_for_test(&module).eval_function(
            function,
            &[heap.alloc(text.as_str()), heap.alloc(suffix)],
            &[],
        );
        if should_match {
            assert_eq!(result.unwrap().unpack_bool(), Some(true));
            assert_eq!(budget.check(), Ok(()));
        } else {
            assert!(result.is_err());
            assert_eq!(budget.check(), Err(ExecutionLimit::Work));
        }
    }
}
