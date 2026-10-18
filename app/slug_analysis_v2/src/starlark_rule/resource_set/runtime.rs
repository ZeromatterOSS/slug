//! One original two-argument call; no analysis request or estimate cache retained.
use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::mem::size_of;
use std::time::Duration;
use std::time::Instant;

use slug_build_api_v2::actions::ResourceEstimate;
use slug_build_api_v2::actions::ResourceSetError;
use slug_build_api_v2::actions::ResourceSetEvaluationContext;
use slug_build_api_v2::actions::ResourceSetInputs;
use slug_build_api_v2::actions::ResourceSetRuntime;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::OwnedFrozenValue;
use starlark::values::ValueLike;
use starlark::values::tuple::AllocTuple;
mod result;
use result::ResultBudget;

#[derive(Debug, Clone, Copy)]
pub(super) struct ResourceRuntimeLimits {
    pub(super) heap_bytes: usize,
    pub(super) timeout: Duration,
    pub(super) call_stack: usize,
}
impl Default for ResourceRuntimeLimits {
    fn default() -> Self {
        Self {
            heap_bytes: 64 * 1024 * 1024,
            timeout: Duration::from_secs(30),
            call_stack: 50,
        }
    }
}
pub(in crate::starlark_rule) struct StarlarkResourceSetRuntime {
    callable: OwnedFrozenValue,
    limits: ResourceRuntimeLimits,
}
impl fmt::Debug for StarlarkResourceSetRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StarlarkResourceSetRuntime")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}
impl StarlarkResourceSetRuntime {
    pub(super) fn new(callable: OwnedFrozenValue) -> Self {
        Self {
            callable,
            limits: ResourceRuntimeLimits::default(),
        }
    }
}
#[derive(Clone, Copy)]
enum Stop {
    Cancelled,
    Time,
    Heap,
}
impl Stop {
    fn error(self) -> ResourceSetError {
        match self {
            Self::Cancelled => ResourceSetError::Cancelled,
            Self::Time => ResourceSetError::Limit("callback cooperative wall time".into()),
            Self::Heap => ResourceSetError::Limit("callback observed heap bytes".into()),
        }
    }
}
struct ForwardPrint<'a> {
    context: &'a dyn ResourceSetEvaluationContext,
    budget: &'a RefCell<ResultBudget>,
    error: RefCell<Option<ResourceSetError>>,
}
impl PrintHandler for ForwardPrint<'_> {
    fn println(&self, _: PrintLocation, text: &str) -> starlark::Result<()> {
        let paid = self.budget.borrow_mut().pay(1, text.len());
        // The budget borrow ends before invoking the consumer's borrowed sink.
        let result = paid.and_then(|()| self.context.print(text));
        result.map_err(|error| {
            *self.error.borrow_mut() = Some(error);
            starlark::Error::new_other(anyhow::anyhow!("resource_set print failed"))
        })
    }
}
impl ResourceSetRuntime for StarlarkResourceSetRuntime {
    fn evaluate(
        &self,
        inputs: ResourceSetInputs,
        context: &dyn ResourceSetEvaluationContext,
    ) -> Result<ResourceEstimate, ResourceSetError> {
        let budget = RefCell::new(ResultBudget::default());
        budget
            .borrow_mut()
            .pay(8, size_of::<Module>() + 8 * size_of::<usize>())?;
        let module = Module::new();
        let start = Instant::now();
        let stopped = Cell::new(None);
        let check = || {
            let reason = if context.is_cancelled() {
                Some(Stop::Cancelled)
            } else if start.elapsed() >= self.limits.timeout {
                Some(Stop::Time)
            } else if module
                .heap()
                .allocated_bytes()
                .saturating_add(module.frozen_heap().allocated_bytes())
                > self.limits.heap_bytes
            {
                Some(Stop::Heap)
            } else {
                None
            };
            if let Some(reason) = reason {
                stopped.set(Some(reason));
            }
            stopped.get().is_some()
        };
        if check() {
            return Err(stopped.get().unwrap().error());
        }
        budget
            .borrow_mut()
            .pay(6, 8 * size_of::<starlark::values::Value>())?;
        let callable = self.callable.owned_value(module.frozen_heap());
        let os = module.heap().alloc(inputs.os.canonical_name());
        let count = module.heap().alloc(inputs.parent_input_count);
        module.set(
            "__slug_resource_call_roots",
            module.heap().alloc(AllocTuple([callable, os, count])),
        );
        let printer = ForwardPrint {
            context,
            budget: &budget,
            error: RefCell::new(None),
        };
        let mut evaluator = Evaluator::new(&module);
        evaluator.set_print_handler(&printer);
        evaluator
            .set_max_callstack_size(self.limits.call_stack)
            .map_err(|e| result::message(&budget, context, e))?;
        evaluator.set_check_cancelled(Box::new(&check));
        let value = evaluator
            .eval_function(callable, &[os, count], &[])
            .map_err(|error| {
                let print_error = printer.error.borrow_mut().take();
                print_error
                    .or_else(|| stopped.get().map(Stop::error))
                    .unwrap_or_else(|| result::message(&budget, context, error))
            })?;
        if check() {
            return Err(stopped.get().unwrap().error());
        }
        let check_result = || {
            if check() {
                Err(stopped.get().expect("check records its reason").error())
            } else {
                Ok(())
            }
        };
        let estimate = result::parse(value, &budget, &check_result);
        estimate.map_err(|error| result::contextualize(&budget, context, error))
    }
}

#[cfg(test)]
mod tests;
