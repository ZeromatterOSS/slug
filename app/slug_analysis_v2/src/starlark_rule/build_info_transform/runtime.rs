//! Original one-argument callback, one deadline and ledger through final encoding.
use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::mem::size_of;
use std::time::Duration;
use std::time::Instant;

use slug_build_api_v2::BuildInfoEvaluationContext;
use slug_build_api_v2::BuildInfoTransformError as Error;
use slug_build_api_v2::BuildInfoTransformRuntime;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::OwnedFrozenValue;
use starlark::values::tuple::AllocTuple;
mod budget;
mod render;
use budget::Budget;
#[derive(Debug, Clone, Copy)]
struct Limits {
    heap: usize,
    timeout: Duration,
    stack: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            heap: 64 * 1024 * 1024,
            timeout: Duration::from_secs(30),
            stack: 50,
        }
    }
}
pub(in crate::starlark_rule) struct StarlarkBuildInfoRuntime {
    original: OwnedFrozenValue,
    limits: Limits,
}
impl fmt::Debug for StarlarkBuildInfoRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StarlarkBuildInfoRuntime")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}
impl StarlarkBuildInfoRuntime {
    pub(super) fn new(original: OwnedFrozenValue) -> Self {
        Self {
            original,
            limits: Limits::default(),
        }
    }
}
struct Printer<'a> {
    context: &'a dyn BuildInfoEvaluationContext,
    budget: &'a RefCell<Budget>,
    error: RefCell<Option<Error>>,
}
impl PrintHandler for Printer<'_> {
    fn println(&self, _: PrintLocation, text: &str) -> starlark::Result<()> {
        let paid = self.budget.borrow_mut().pay(1, text.len());
        paid.and_then(|()| self.context.print(text))
            .map_err(|error| {
                *self.error.borrow_mut() = Some(error);
                starlark::Error::new_other(anyhow::anyhow!("build-info print refused"))
            })
    }
}
impl BuildInfoTransformRuntime for StarlarkBuildInfoRuntime {
    fn render(
        &self,
        status: &[u8],
        template: &[u8],
        context: &dyn BuildInfoEvaluationContext,
    ) -> Result<Vec<u8>, Error> {
        let ledger = RefCell::new(Budget::default());
        ledger.borrow_mut().pay(
            16,
            size_of::<Module>()
                + size_of::<Evaluator<'_, '_, '_>>()
                + size_of::<Budget>()
                + 32 * size_of::<usize>(),
        )?;
        let start = Instant::now();
        let module = Module::new();
        let stopped = Cell::new(None);
        let check = || {
            let reason = if context.is_cancelled() {
                Some(0)
            } else if start.elapsed() >= self.limits.timeout {
                Some(1)
            } else if module
                .heap()
                .allocated_bytes()
                .saturating_add(module.frozen_heap().allocated_bytes())
                > self.limits.heap
            {
                Some(2)
            } else {
                None
            };
            if let Some(reason) = reason {
                stopped.set(Some(reason));
            }
            match stopped.get() {
                Some(0) => Err(Error::Cancelled),
                Some(1) => Err(Error::Limit("cooperative wall time".into())),
                Some(_) => Err(Error::Limit("cooperative observed heap".into())),
                None => Ok(()),
            }
        };
        let run = || -> Result<Vec<u8>, Error> {
            check()?;
            let input = render::input_dict(status, &module, &ledger, &check)?;
            ledger
                .borrow_mut()
                .pay(6, 8 * size_of::<starlark::values::Value>())?;
            let original = self.original.owned_value(module.frozen_heap());
            module.set(
                "__slug_build_info_roots",
                module.heap().alloc(AllocTuple([original, input])),
            );
            let printer = Printer {
                context,
                budget: &ledger,
                error: RefCell::new(None),
            };
            let mut evaluator = Evaluator::new(&module);
            evaluator.set_print_handler(&printer);
            evaluator
                .set_max_callstack_size(self.limits.stack)
                .map_err(|e| budget::callback_error(&ledger, context, e))?;
            evaluator.set_check_cancelled(Box::new(|| check().is_err()));
            let result = evaluator
                .eval_function(original, &[input], &[])
                .map_err(|e| {
                    let print_error = printer.error.borrow_mut().take();
                    print_error
                        .or_else(|| check().err())
                        .unwrap_or_else(|| budget::callback_error(&ledger, context, e))
                })?;
            check()?;
            render::output(result, template, &ledger, &check)
        };
        match run() {
            Ok(bytes) => Ok(bytes),
            Err(error) => {
                let error = budget::contextualize(&ledger, context, error);
                ledger.borrow_mut().failed = true;
                Err(error)
            }
        }
    }
}
#[cfg(test)]
mod tests;
