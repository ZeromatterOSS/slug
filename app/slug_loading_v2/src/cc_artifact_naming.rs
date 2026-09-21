//! Native artifact-name query. Provider schema and the existing owned model are
//! the authority; constructor provenance is only relevant to callable recipes.
use compact_str::CompactString;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use starlark::eval::Evaluator;
use starlark::values::Value;

use crate::cc_features::cc_toolchain_features_value;
use crate::provider::starlark_user_provider_fields;
use crate::provider::starlark_user_provider_has_schema;
use crate::subrule_invocation::AnalysisEvaluationContext;

pub(crate) fn artifact_name_for_category(
    toolchain: Value<'_>,
    category: &str,
    output_name: &str,
    eval: &Evaluator<'_, '_, '_>,
) -> anyhow::Result<CompactString> {
    anyhow::ensure!(
        starlark_user_provider_has_schema(toolchain) == Some(true),
        "cc_toolchain requires a schema-backed CcToolchainInfo"
    );
    let (id, fields) = starlark_user_provider_fields(toolchain)
        .ok_or_else(|| anyhow::anyhow!("cc_toolchain requires CcToolchainInfo"))?;
    anyhow::ensure!(
        id.exported_name() == "CcToolchainInfo"
            && matches!(
                id.source_label(),
                "@@rules_cc+//cc/private/rules_impl:cc_toolchain_info.bzl"
                    | "@@//cc/private/rules_impl:cc_toolchain_info.bzl"
                    | "@@//third_party/bazel_rules/rules_cc/cc/private/rules_impl:cc_toolchain_info.bzl"
            ),
        "cc_toolchain requires the native CcToolchainInfo provider key"
    );
    let model = fields
        .iter()
        .find(|(name, _)| name == "_toolchain_features")
        .and_then(|(_, value)| cc_toolchain_features_value(*value))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "CcToolchainInfo._toolchain_features requires native CcToolchainFeatures"
            )
        })?;
    let context = eval
        .extra
        .and_then(|extra| extra.downcast_ref::<AnalysisEvaluationContext>())
        .ok_or_else(|| anyhow::anyhow!("artifact naming requires observed analysis Host"))?;
    anyhow::ensure!(
        context.observed_host_os()? != ActionEnvironmentHostOs::Windows,
        "Windows artifact naming is not admitted"
    );
    Ok(model.artifact_name_for_category_posix(category, output_name)?)
}

#[cfg(test)]
mod tests;
