use std::sync::Arc;

use allocative::Allocative;
use starlark_derive::starlark_value;

use super::*;
use crate as starlark;
use crate::eval::ExecutionBudget;
use crate::eval::compiler::add_lazy_span_to_expr_error;
use crate::eval::compiler::add_span_to_expr_error;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("unadmitted_frame_name")]
struct UnadmittedFrameName;

starlark::starlark_simple_value!(UnadmittedFrameName);

#[starlark_value(type = "unadmitted_frame_name")]
impl<'v> StarlarkValue<'v> for UnadmittedFrameName {
    fn name_for_call_stack(&self, _me: Value<'v>) -> String {
        panic!("a refused evaluation reached native diagnostic rendering");
    }
}

fn assert_refusal(error: crate::Error, limit: ExecutionLimit) {
    assert!(!error.has_diagnostic());
    let crate::ErrorKind::Other(cause) = error.kind() else {
        panic!(
            "expected a typed refusal, got {}",
            error.without_diagnostic()
        );
    };
    assert_eq!(cause.downcast_ref(), Some(&limit));
}

#[test]
fn resource_refusal_skips_native_diagnostics_while_unwinding() {
    let budget = Arc::new(ExecutionBudget::new(100, 4096, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    let mut eval = Evaluator::new(&module);
    eval.call_stack.alloc_if_needed(2, Some(&budget)).unwrap();
    eval.call_stack.push(Value::new_none(), None).unwrap();
    let canary = module.heap().alloc(UnadmittedFrameName);
    let limit = ExecutionLimit::Work;
    let error = eval
        .with_call_stack(canary, None, |eval| {
            budget.refuse(limit);
            let child =
                crate::Error::new_other(anyhow::anyhow!("native returned a different error"));
            Err::<(), _>(
                add_lazy_span_to_expr_error(
                    child,
                    || panic!("a refused evaluation accessed span metadata"),
                    eval,
                )
                .into_error(),
            )
        })
        .unwrap_err();
    assert_refusal(error, limit);
    assert_eq!(eval.call_stack.count(), 1);
    eval.call_stack.pop();
    assert_eq!(eval.call_stack.count(), 0);
}

#[test]
fn typed_refusal_skips_diagnostics_after_its_ledger_is_restored() {
    let budget = Arc::new(ExecutionBudget::new(100, 4096, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    let inner_budget = Arc::new(ExecutionBudget::new(100, 4096, 16));
    let inner = Module::new_with_execution_budget(inner_budget.clone());
    let mut eval = Evaluator::new(&module);
    eval.call_stack.alloc_if_needed(2, Some(&budget)).unwrap();
    eval.call_stack.push(Value::new_none(), None).unwrap();
    let canary = module.heap().alloc(UnadmittedFrameName);
    let limit = ExecutionLimit::AllocationBytes;
    let error = eval
        .with_call_stack(canary, None, |eval| {
            let result = eval.with_module(&inner, false, |eval| {
                let refusal = eval.heap().execution_budget().unwrap().refuse(limit);
                Err::<(), _>(crate::Error::new_kind(crate::ErrorKind::Native(
                    anyhow::Error::new(refusal).context("prepared call crossed a module boundary"),
                )))
            });
            assert!(std::ptr::eq(eval.module_env, &module));
            result
        })
        .unwrap_err();
    assert_refusal(error, limit);
    assert_eq!(budget.check(), Ok(()));
    assert_eq!(inner_budget.check(), Err(limit));
    assert_eq!(eval.call_stack.count(), 1);
    eval.call_stack.pop();
    assert_eq!(eval.call_stack.count(), 0);
}

#[test]
fn ordinary_failure_keeps_diagnostics_in_budgeted_evaluation() {
    let budget = Arc::new(ExecutionBudget::new(100, 4096, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    let mut eval = Evaluator::new(&module);
    eval.call_stack.alloc_if_needed(2, Some(&budget)).unwrap();
    eval.call_stack.push(Value::new_none(), None).unwrap();
    let error = eval
        .with_call_stack(Value::new_none(), None, |eval| {
            let child = crate::Error::new_value(anyhow::anyhow!("ordinary error"));
            Err::<(), _>(add_span_to_expr_error(child, FrameSpan::default(), eval).into_error())
        })
        .unwrap_err();
    assert!(error.span().is_some());
    assert!(!error.call_stack().is_empty());
    assert_eq!(budget.check(), Ok(()));
    assert_eq!(eval.call_stack.count(), 1);
    eval.call_stack.pop();
    assert_eq!(eval.call_stack.count(), 0);
}
