use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use derive_more::Display;

use super::Evaluator;
use super::EvaluatorError;
use super::GC_THRESHOLD;
use super::INFREQUENT_INSTRUCTION_CHECK_PERIOD;
use crate as starlark;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::is_starlark_def;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::starlark_value;

fn eval<'v>(evaluator: &mut Evaluator<'v, '_, '_>, source: &str) -> crate::Result<Value<'v>> {
    let ast = AstModule::parse("module.star", source.to_owned(), &Dialect::Standard).unwrap();
    evaluator.eval_module(ast, &Globals::standard())
}

fn strict_budget() -> Arc<ExecutionBudget> {
    Arc::new(ExecutionBudget::new(10_000, 1 << 20, 16).with_bounded_operations())
}

#[test]
fn strict_module_skips_due_gc_and_keeps_the_cancellation_poll() {
    let ordinary = Module::new();
    let budget = strict_budget();
    let strict = Module::new_with_execution_budget(budget.clone());
    for (module, bounded) in [(&ordinary, false), (&strict, true)] {
        let mut evaluator = Evaluator::new(module);
        evaluator.trigger_gc();
        let value = eval(&mut evaluator, "answer = 42\nanswer").unwrap();
        assert_eq!(value.unpack_i32(), Some(42));
        if bounded {
            assert_eq!(evaluator.next_gc_level, 0);
        } else {
            assert!(evaluator.next_gc_level >= GC_THRESHOLD);
        }
    }

    let new_budget = strict_budget();
    let module = Module::new_with_execution_budget(new_budget.clone());
    let mut evaluator = Evaluator::new(&module);
    evaluator.trigger_gc();
    evaluator.infrequent_instr_check_counter = INFREQUENT_INSTRUCTION_CHECK_PERIOD - 1;
    evaluator.set_check_cancelled(Box::new(|| true));
    let error = eval(&mut evaluator, "answer = 42").unwrap_err();
    assert!(matches!(error.kind(), crate::ErrorKind::Other(cause)
        if matches!(cause.downcast_ref::<EvaluatorError>(), Some(EvaluatorError::Cancelled))));
    assert!(module.get("answer").is_none());
    assert_eq!(evaluator.next_gc_level, 0);
    assert_eq!(new_budget.check(), Ok(()));
    assert_eq!(budget.check(), Ok(()));
}

#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
#[display("export_probe")]
struct ExportProbe {
    trap: bool,
    #[allocative(skip)]
    calls: Arc<AtomicUsize>,
}

starlark::starlark_simple_value!(ExportProbe);

#[starlark_value(type = "export_probe")]
impl<'v> StarlarkValue<'v> for ExportProbe {
    fn export_as(&self, name: &str, _evaluator: &mut Evaluator<'v, '_, '_>) -> crate::Result<()> {
        assert!(!self.trap, "ran an unadmitted export hook");
        assert_eq!(name, "published");
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

#[test]
fn strict_module_export_refuses_native_hooks_before_both_store_routes() {
    for host_store in [false, true] {
        let ordinary = Module::new();
        let budget = strict_budget();
        let strict = Module::new_with_execution_budget(budget.clone());
        for (module, bounded) in [(&ordinary, false), (&strict, true)] {
            let calls = Arc::new(AtomicUsize::new(0));
            let probe = ExportProbe {
                trap: bounded,
                calls: calls.clone(),
            };
            let mut evaluator = Evaluator::new(module);
            let value = if host_store {
                module.frozen_heap().alloc(probe).to_value()
            } else {
                module.heap().alloc(probe)
            };
            let result = if host_store {
                evaluator.set_module_variable_at_some_point("published", value)
            } else {
                module.set("subject", value);
                eval(&mut evaluator, "published = subject").map(|_| ())
            };
            if bounded {
                let limit = ExecutionLimit::UnsupportedOperation {
                    operation: "module export",
                    value_type: "export_probe",
                };
                let error = result.unwrap_err();
                assert!(matches!(error.kind(), crate::ErrorKind::Other(cause)
                    if cause.downcast_ref::<ExecutionLimit>() == Some(&limit)));
                assert_eq!(budget.check(), Err(limit));
                assert_eq!(calls.load(Ordering::Relaxed), 0);
                assert!(module.get("published").is_none());
            } else {
                result.unwrap();
                assert_eq!(calls.load(Ordering::Relaxed), 1);
                let expected = if host_store {
                    value
                } else {
                    module.get("subject").unwrap()
                };
                assert!(module.get("published").unwrap().ptr_eq(expected));
            }
        }
    }
}

#[test]
fn strict_module_exports_and_calls_moving_and_imported_frozen_defs() {
    let producer = Module::new();
    eval(
        &mut Evaluator::new(&producer),
        "def frozen(value): return value",
    )
    .unwrap();
    let producer = producer.freeze().unwrap();
    let budget = strict_budget();
    let module = Module::new_with_execution_budget(budget.clone());
    module.import_public_symbols(&producer);
    let result = eval(
        &mut Evaluator::new(&module),
        "def moving(value): return value\nalias = frozen\nalias(moving(42))",
    )
    .unwrap();
    assert_eq!(result.unpack_i32(), Some(42));
    assert!(is_starlark_def(module.get("moving").unwrap()));
    assert!(
        module
            .get("alias")
            .unwrap()
            .ptr_eq(producer.get("frozen").unwrap().value())
    );
    assert_eq!(budget.check(), Ok(()));
}
