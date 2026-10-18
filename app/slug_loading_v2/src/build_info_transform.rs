//! Original build-info registration into the existing traced callable collector.
use allocative::Allocative;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::BuildInfoTransformKind;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::Value;

use crate::builtin_restriction::check_default_allowlist;
use crate::builtin_restriction::source_identities_for_evaluator;
use crate::subrule_invocation::AnalysisArtifactValue;

/// Distinct family token; GC storage and the one poisoned ledger are shared.
#[derive(Debug, Allocative)]
pub struct BuildInfoCallbackRegistration(crate::resource_set::ResourceSetRegistration);
#[derive(Debug)]
pub struct BuildInfoTransformRequest<'a> {
    pub module: &'a Module,
    pub kind: BuildInfoTransformKind,
    pub callback: BuildInfoCallbackRegistration,
    pub template: AnalysisArtifact,
    pub output_file_name: &'a str,
}
pub(crate) fn prepare<'v>(
    kind: BuildInfoTransformKind,
    transform_func: Value<'v>,
    template: Value<'v>,
    output_file_name: &'v str,
    eval: &Evaluator<'v, '_, '_>,
) -> anyhow::Result<BuildInfoTransformRequest<'v>> {
    crate::resource_set::prepay_action_storage(
        eval.module(),
        8,
        std::mem::size_of::<BuildInfoTransformRequest<'v>>() + 8 * std::mem::size_of::<usize>(),
    )?;
    // No original-global-name rule: actual Starlark closures are admitted.
    anyhow::ensure!(
        transform_func.parameters_spec().is_some(),
        "build-info transform_func must be a Starlark function"
    );
    let template = AnalysisArtifactValue::from_starlark(template)
        .ok_or_else(|| anyhow::anyhow!("build-info template must be a File"))?;
    let sources = source_identities_for_evaluator(eval)?;
    check_default_allowlist(eval, &sources)?;
    let callback = register_callback(eval.module(), transform_func)?;
    Ok(BuildInfoTransformRequest {
        module: eval.module(),
        kind,
        callback,
        template: template.artifact().clone(),
        output_file_name,
    })
}
/// Trusted Rust registration used by the Starlark binder after DEFAULT access.
/// This validates the original function, not source identity or expected effects.
pub fn register_callback<'v>(
    module: &'v Module,
    value: Value<'v>,
) -> anyhow::Result<BuildInfoCallbackRegistration> {
    crate::resource_set::prepay_action_storage(
        module,
        8,
        std::mem::size_of::<BuildInfoCallbackRegistration>() + 8 * std::mem::size_of::<usize>(),
    )?;
    anyhow::ensure!(
        value.parameters_spec().is_some(),
        "build-info transform_func must be a Starlark function"
    );
    Ok(BuildInfoCallbackRegistration(
        crate::resource_set::register_resource_set(module, value)?,
    ))
}
/// Posttransfer lookup spends only the receiving Publisher's same ledger.
pub fn callback_value<'v>(
    module: &'v Module,
    token: &BuildInfoCallbackRegistration,
    charge: impl FnMut(usize, usize) -> Result<(), String>,
) -> anyhow::Result<Value<'v>> {
    crate::resource_set::resource_set_value(module, &token.0, charge)
}
#[cfg(test)]
mod tests;
