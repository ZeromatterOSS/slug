//! Execute the retained original callable in a fresh, bounded batch evaluator.
use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use compact_str::CompactString;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::ArgsDirectoryContext;
use slug_build_api_v2::ArgsExpansionError;
use slug_build_api_v2::RetainedArgsInputs;
use slug_build_api_v2::RuntimeMapper;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::inspect_def;
use starlark::values::OwnedFrozenValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::list::ListRef;
use starlark::values::tuple::AllocTuple;
use starlark::values::tuple::TupleRef;

use super::directory::DirectoryExpander;
use super::directory::DirectoryRuntime;
use crate::analysis_value::AnalysisValueMaterializer;

#[derive(Debug, Clone, Copy)]
pub(super) struct ArgsRuntimeLimits {
    pub(super) input_count: usize,
    pub(super) output_count: usize,
    pub(super) output_bytes: usize,
    pub(super) heap_bytes: usize,
    pub(super) timeout: Duration,
    pub(super) call_stack: usize,
}
impl Default for ArgsRuntimeLimits {
    fn default() -> Self {
        Self {
            input_count: 65_536,
            output_count: 65_536,
            output_bytes: 64 * 1024 * 1024,
            heap_bytes: 64 * 1024 * 1024,
            timeout: Duration::from_secs(30),
            call_stack: 50,
        }
    }
}

pub(super) struct StarlarkRuntimeMapper {
    callable: OwnedFrozenValue,
    limits: ArgsRuntimeLimits,
    cancellation: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
}
impl fmt::Debug for StarlarkRuntimeMapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StarlarkRuntimeMapper")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}
impl StarlarkRuntimeMapper {
    pub(super) fn new(callable: OwnedFrozenValue) -> Self {
        Self {
            callable,
            limits: ArgsRuntimeLimits::default(),
            cancellation: None,
        }
    }
    pub(super) fn with_limits(mut self, limits: ArgsRuntimeLimits) -> Self {
        self.limits = limits;
        self
    }
    pub(super) fn with_cancellation(
        mut self,
        cancellation: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        self.cancellation = Some(cancellation);
        self
    }
}
struct MutedPrint;
impl PrintHandler for MutedPrint {
    fn println(&self, _: PrintLocation, _: &str) -> starlark::Result<()> {
        Ok(())
    }
}
static MUTED_PRINT: MutedPrint = MutedPrint;
#[derive(Clone, Copy)]
enum Stop {
    Cancelled,
    Time,
    Heap,
}
impl Stop {
    fn error(self) -> ArgsExpansionError {
        match self {
            Self::Cancelled => ArgsExpansionError::Cancelled,
            Self::Time => ArgsExpansionError::Limit("callback cooperative wall time".into()),
            Self::Heap => ArgsExpansionError::Limit("callback observed heap bytes".into()),
        }
    }
}

impl RuntimeMapper for StarlarkRuntimeMapper {
    fn map_batch(
        &self,
        values: &[AnalysisValue],
        context: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<CompactString>, ArgsExpansionError> {
        if values.len() > self.limits.input_count {
            return Err(ArgsExpansionError::Limit("input count".into()));
        }
        // Reuse the native graph audit before copying or materializing inputs.
        let values = RetainedArgsInputs::Sequence(Arc::from(values)).values()?;
        let module = Module::new();
        let start = Instant::now();
        let stopped = Cell::new(None);
        let check = || {
            let reason = if context.is_cancelled()
                || self.cancellation.as_ref().is_some_and(|cancel| cancel())
            {
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
        let callable = self.callable.owned_value(module.frozen_heap());
        let with_directory =
            inspect_def(callable, None).is_some_and(|def| def.parameter_count() >= 2);
        let expander = module.frozen_heap().alloc(DirectoryExpander).to_value();
        let mut materializer = AnalysisValueMaterializer::new(module.frozen_heap());
        let mut inputs = Vec::with_capacity(values.len());
        for value in &values {
            if check() {
                return Err(stopped.get().unwrap().error());
            }
            inputs.push(
                materializer
                    .value(value)
                    .map_err(|error| ArgsExpansionError::Unsupported(error.into()))?,
            );
        }
        // All inputs are frozen and owned by this module; root them before any
        // evaluator GC. No moving Value is retained in borrowed host state.
        let mut roots = Vec::with_capacity(inputs.len() + 2);
        roots.push(callable);
        roots.push(expander);
        roots.extend(inputs.iter().map(|value| value.to_value()));
        // Loading builtins own extra_value's scratch-root ABI. Keep this batch
        // in a traced transient-module slot so their cache updates cannot
        // reinterpret or replace the callback, expander and input roots.
        module.set(
            "__slug_args_batch_roots",
            module.heap().alloc(AllocTuple(roots)),
        );
        let directory = DirectoryRuntime {
            context,
            maximum_children: self.limits.output_count,
            maximum_bytes: self.limits.output_bytes,
            error: RefCell::new(None),
        };
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&directory);
        eval.set_print_handler(&MUTED_PRINT);
        eval.set_max_callstack_size(self.limits.call_stack)
            .map_err(|error| ArgsExpansionError::Limit(error.to_string().into()))?;
        eval.set_check_cancelled(Box::new(&check));
        let mut output = Vec::new();
        let mut output_bytes = 0usize;
        for input in inputs {
            if check() {
                return Err(stopped.get().unwrap().error());
            }
            let arguments = [input.to_value(), expander];
            let result = eval
                .eval_function(
                    callable,
                    &arguments[..if with_directory { 2 } else { 1 }],
                    &[],
                )
                .map_err(|error| {
                    directory
                        .error
                        .borrow()
                        .clone()
                        .or_else(|| stopped.get().map(Stop::error))
                        .unwrap_or_else(|| ArgsExpansionError::Callback(error.to_string().into()))
                })?;
            if check() {
                return Err(stopped.get().unwrap().error());
            }
            if result.is_none() {
                continue;
            }
            if let Some(string) = result.unpack_str() {
                append(string, &mut output, &mut output_bytes, self.limits)?;
            } else if let Some(list) = ListRef::from_value(result) {
                for value in list.iter() {
                    append_value(value, &mut output, &mut output_bytes, self.limits)?;
                }
            } else if let Some(tuple) = TupleRef::from_value(result) {
                for value in tuple.iter() {
                    append_value(value, &mut output, &mut output_bytes, self.limits)?;
                }
            } else {
                return Err(ArgsExpansionError::InvalidReturn(
                    format!(
                        "expected None, string, list or tuple of strings, got {}",
                        result.get_type()
                    )
                    .into(),
                ));
            }
        }
        Ok(output)
    }
}
fn append_value(
    value: Value<'_>,
    output: &mut Vec<CompactString>,
    bytes: &mut usize,
    limits: ArgsRuntimeLimits,
) -> Result<(), ArgsExpansionError> {
    let string = value.unpack_str().ok_or_else(|| {
        ArgsExpansionError::InvalidReturn(
            format!("expected string element, got {}", value.get_type()).into(),
        )
    })?;
    append(string, output, bytes, limits)
}
fn append(
    string: &str,
    output: &mut Vec<CompactString>,
    bytes: &mut usize,
    limits: ArgsRuntimeLimits,
) -> Result<(), ArgsExpansionError> {
    let next = bytes
        .checked_add(string.len())
        .ok_or_else(|| ArgsExpansionError::Limit("output bytes".into()))?;
    if output.len() >= limits.output_count || next > limits.output_bytes {
        return Err(ArgsExpansionError::Limit("output count or bytes".into()));
    }
    *bytes = next;
    output.push(string.into());
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cancellation_tests;
