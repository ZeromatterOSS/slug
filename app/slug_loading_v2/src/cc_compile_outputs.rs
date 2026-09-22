//! Paired native compile-output helpers. No action or independent artifact owner.
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_identity_v2::CanonicalLabel;
use starlark::eval::Evaluator;
use starlark::values::Value;

pub(crate) mod configuration;
pub use configuration::alloc_configuration;

use crate::cc_context::CcCompileOutputContext;
use crate::subrule_invocation::AnalysisEvaluationContext;

pub(crate) fn output_name_prefix(
    configuration: Value<'_>,
    purpose: &str,
) -> anyhow::Result<&'static str> {
    configuration::unpack(configuration)?;
    // CcStarlarkInternal: the longer suffix must be considered first.
    Ok(if purpose.ends_with("_non_objc_arc") {
        "non_arc"
    } else if purpose.ends_with("_objc_arc") {
        "arc"
    } else {
        ""
    })
}

pub(crate) fn declare<'v>(
    ctx: Value<'v>,
    label: Value<'v>,
    output_name: &str,
    configuration: Value<'v>,
    eval: &Evaluator<'v, '_, '_>,
) -> anyhow::Result<Value<'v>> {
    let ctx = ctx
        .request_value::<&dyn CcCompileOutputContext>()
        .ok_or_else(|| anyhow::anyhow!("declare_compile_output_file requires a rule ctx"))?;
    let label = crate::provider::starlark_label(label)
        .ok_or_else(|| anyhow::anyhow!("declare_compile_output_file requires a Label"))?;
    let configuration = configuration::unpack(configuration)?;
    let evaluation = eval
        .extra
        .and_then(|extra| extra.downcast_ref::<AnalysisEvaluationContext>())
        .ok_or_else(|| {
            anyhow::anyhow!("compile output declaration requires observed analysis Host")
        })?;
    anyhow::ensure!(
        evaluation.observed_host_os()? != ActionEnvironmentHostOs::Windows,
        "Windows compile output paths are not admitted"
    );
    Ok(eval
        .heap()
        .alloc(ctx.declare_compile_output(&label, output_name, configuration)?))
}

/// POSIX PathFragment joining and RuleContext package containment. The returned
/// bin-relative coordinate is scratch; the existing action sink retains it.
pub fn compile_output_path(
    active_label: &CanonicalLabel,
    label: &CanonicalLabel,
    output_name: &str,
) -> anyhow::Result<String> {
    anyhow::ensure!(
        !output_name.starts_with('/'),
        "compile output path must be relative"
    );
    let package = |label: &CanonicalLabel| {
        slug_build_api_v2::repository_relative_output_path(
            label.package().repo(),
            label.package().package().as_str(),
        )
    };
    let active_package = package(active_label);
    let joined = format!(
        "{}/_objs/{}/{}",
        package(label),
        label.target().as_str(),
        output_name
    );
    let mut segments = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                anyhow::ensure!(
                    segments.pop().is_some(),
                    "compile output path escapes its root"
                );
            }
            _ => segments.push(segment),
        }
    }
    let path = segments.join("/");
    // Empty external packages have a trailing slash in the projection helper.
    let active_package = active_package.trim_end_matches('/');
    anyhow::ensure!(
        active_package.is_empty()
            || path == active_package
            || path
                .strip_prefix(active_package)
                .is_some_and(|rest| rest.starts_with('/')),
        "compile output path is outside the active ctx package"
    );
    Ok(path)
}

#[cfg(test)]
mod tests;
