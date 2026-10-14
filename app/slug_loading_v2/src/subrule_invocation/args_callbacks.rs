//! Record actual Args callbacks; publication owns capture/source admissibility.
use compact_str::CompactString;
use starlark::eval::Evaluator;
use starlark::eval::inspect_def;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::typing::StarlarkCallable;

use super::EvaluatorVectorMapEachGen;

pub(super) fn register_callback<'v>(
    map_each: Option<Value<'v>>,
    allow_closure: bool,
    eval: &Evaluator<'v, '_, '_>,
) -> anyhow::Result<Option<EvaluatorVectorMapEachGen<Value<'v>>>> {
    let Some(callback) = map_each.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    let callable: Option<StarlarkCallable<'v>> =
        StarlarkCallable::unpack_value(callback).map_err(starlark::Error::into_anyhow)?;
    anyhow::ensure!(callable.is_some(), "Args map_each must be callable or None");
    if !allow_closure && let Some(function) = inspect_def(callback, Some(eval.module())) {
        anyhow::ensure!(
            function.is_global().map_err(starlark::Error::into_anyhow)?,
            "to avoid unintended retention of analysis data structures, the map_each function (declared at {}) must be declared by a top-level def statement",
            function.signature_span()
        );
    }
    Ok(Some(EvaluatorVectorMapEachGen::Callable(callback)))
}

pub(super) fn registration_location(eval: &Evaluator<'_, '_, '_>) -> Option<CompactString> {
    eval.call_stack_top_location()
        .map(|location| location.to_string().into())
}
