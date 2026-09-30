//! Disabled coverage construction; all supplied inputs validate before EMPTY.
use std::fmt;
use std::sync::LazyLock;

use allocative::Allocative;
use starlark::any::ProvidesStaticType;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::eval::ParametersSpec;
use starlark::eval::ParametersSpecParam;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::range::Range;
use starlark::values::starlark_value;
use starlark::values::tuple::TupleRef;

use crate::provider::StarlarkDepset;
use crate::subrule_invocation::AnalysisArtifactValue;

const NAME: &str = "coverage_common.instrumented_files_info";
static PARAMETERS: LazyLock<ParametersSpec<FrozenValue>> = LazyLock::new(|| {
    ParametersSpec::new_parts(
        NAME,
        [],
        [("ctx", ParametersSpecParam::Required)],
        false,
        [
            "source_attributes",
            "dependency_attributes",
            "coverage_support_files",
            "coverage_environment",
            "extensions",
            "metadata_files",
            "reported_to_actual_sources",
            "baseline_coverage_files",
        ]
        .map(|name| (name, ParametersSpecParam::Optional)),
        false,
    )
});

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct InstrumentedFilesInfoFunction;
impl fmt::Display for InstrumentedFilesInfoFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<function {NAME}>")
    }
}
starlark::starlark_simple_value!(InstrumentedFilesInfoFunction);
#[starlark_value(type = "function")]
impl<'v> StarlarkValue<'v> for InstrumentedFilesInfoFunction {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        construct(args, eval).map_err(starlark::Error::new_other)
    }
}

fn sequence<'v>(
    value: Value<'v>,
    name: &str,
    heap: Heap<'v>,
    valid: impl Fn(Value<'v>) -> bool,
) -> anyhow::Result<bool> {
    anyhow::ensure!(
        ListRef::from_value(value).is_some()
            || TupleRef::from_value(value).is_some()
            || Range::from_value(value).is_some(),
        "{name} requires a sequence"
    );
    let mut nonempty = false;
    for item in value.iterate(heap).map_err(starlark::Error::into_anyhow)? {
        anyhow::ensure!(valid(item), "{name} has an invalid element");
        nonempty = true;
    }
    Ok(nonempty)
}
fn file(value: Value<'_>) -> bool {
    AnalysisArtifactValue::from_starlark(value).is_some()
}
fn text(value: Value<'_>) -> bool {
    value.unpack_str().is_some()
}
fn depset(value: Value<'_>, name: &str, valid: impl Fn(Value<'_>) -> bool) -> anyhow::Result<bool> {
    anyhow::ensure!(
        StarlarkDepset::parts_from_value(value).is_some(),
        "{name} requires a depset"
    );
    let elements = StarlarkDepset::flatten(value).map_err(starlark::Error::into_anyhow)?;
    anyhow::ensure!(
        elements.iter().copied().all(valid),
        "{name} has an invalid element"
    );
    Ok(!elements.is_empty())
}
fn optional_sequence<'v>(
    value: Option<Value<'v>>,
    name: &str,
    heap: Heap<'v>,
    valid: impl Fn(Value<'v>) -> bool,
) -> anyhow::Result<()> {
    if let Some(value) = value {
        sequence(value, name, heap, valid)?;
    }
    Ok(())
}

fn construct<'v>(
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<Value<'v>> {
    let [
        ctx,
        source,
        dependency,
        support,
        environment,
        extensions,
        metadata,
        reported,
        baseline,
    ] = PARAMETERS
        .collect_into::<9>(args, eval.heap())
        .map_err(starlark::Error::into_anyhow)?;
    // CoverageCommon.java:47-110 conversions precede Collector's off shortcut.
    optional_sequence(
        extensions.filter(|value| !value.is_none()),
        "extensions",
        eval.heap(),
        text,
    )?;
    let reported_nonempty = match reported.filter(|value| !value.is_none()) {
        Some(value) => depset(value, "reported_to_actual_sources", |item| {
            TupleRef::from_value(item).is_some()
        })?,
        None => false,
    };
    optional_sequence(
        baseline.filter(|value| !value.is_none()),
        "baseline_coverage_files",
        eval.heap(),
        file,
    )?;
    let mut environment_nonempty = false;
    if let Some(value) = environment {
        let dictionary = DictRef::from_value(value)
            .ok_or_else(|| anyhow::anyhow!("coverage_environment requires a dict"))?;
        for (key, value) in dictionary.iter() {
            anyhow::ensure!(
                text(key) && text(value),
                "coverage_environment requires string keys and values"
            );
            environment_nonempty = true;
        }
    }
    let support_nonempty = match support {
        Some(value) if StarlarkDepset::parts_from_value(value).is_some() => {
            depset(value, "coverage_support_files", file)?
        }
        Some(value) => {
            // Validate every nested depset before applying the private gate.
            anyhow::ensure!(
                ListRef::from_value(value).is_some()
                    || TupleRef::from_value(value).is_some()
                    || Range::from_value(value).is_some(),
                "coverage_support_files requires a sequence or depset"
            );
            let mut nonempty = false;
            for item in value
                .iterate(eval.heap())
                .map_err(starlark::Error::into_anyhow)?
            {
                if StarlarkDepset::parts_from_value(item).is_some() {
                    nonempty |= depset(item, "coverage_support_files", file)?;
                } else {
                    anyhow::ensure!(file(item), "coverage_support_files has an invalid element");
                    nonempty = true;
                }
            }
            nonempty
        }
        None => false,
    };
    if reported_nonempty || environment_nonempty || support_nonempty {
        crate::builtin_restriction::check_default_allowlist(
            eval,
            &crate::builtin_restriction::source_identities_for_evaluator(eval)?,
        )?;
    }
    optional_sequence(source, "source_attributes", eval.heap(), text)?;
    optional_sequence(dependency, "dependency_attributes", eval.heap(), text)?;
    optional_sequence(metadata, "metadata_files", eval.heap(), file)?;
    crate::subrule_invocation::instrumented_files_info_empty(
        ctx.expect("required ctx collected"),
        eval,
    )
}
