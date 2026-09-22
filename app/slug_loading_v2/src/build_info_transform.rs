//! Finite, source-authenticated build-info callback admission. No callback is
//! invoked or retained here; the immutable recipe belongs to the action owner.
use sha2::Digest;
use sha2::Sha256;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::BuildInfoCallbackRecipe;
use slug_build_api_v2::BuildInfoTransformKind;
use starlark::eval::Evaluator;
use starlark::values::Value;

use crate::builtin_restriction::check_default_allowlist;
use crate::builtin_restriction::source_identities_for_evaluator;
use crate::subrule_invocation::AnalysisArtifactValue;

const LABEL: &str = BuildInfoCallbackRecipe::SOURCE_LABEL;
const SHA: [u8; 32] = BuildInfoCallbackRecipe::SOURCE_DIGEST;

/// The unchanged module has no imports, but its attr defaults still require
/// the selected builtin repository's complete mapping. Only the builtin route
/// may call this finite gate; it grants no callback authentication authority.
pub(crate) fn needs_builtin_mapping(
    source: &slug_identity_v2::CanonicalLabel,
    loads: &[String],
) -> bool {
    loads.is_empty() && source.to_string() == LABEL
}

#[derive(Debug)]
pub struct BuildInfoTransformRequest<'a> {
    pub callback: BuildInfoCallbackRecipe,
    pub template: AnalysisArtifact,
    pub output_file_name: &'a str,
}

pub(crate) fn prepare<'a>(
    kind: BuildInfoTransformKind,
    transform_func: Value<'_>,
    template: Value<'_>,
    output_file_name: &'a str,
    eval: &Evaluator<'_, '_, '_>,
) -> anyhow::Result<BuildInfoTransformRequest<'a>> {
    // parameters_spec downcasts concrete Def/FrozenDef only. A callable object,
    // native function or name/type-spoofing value is not a Starlark function.
    anyhow::ensure!(
        transform_func.parameters_spec().is_some(),
        "build-info transform_func must be a Starlark function"
    );
    let template = AnalysisArtifactValue::from_starlark(template)
        .ok_or_else(|| anyhow::anyhow!("build-info template must be a File"))?;
    let sources = source_identities_for_evaluator(eval)?;
    check_default_allowlist(eval, &sources)?;
    let span = eval
        .call_stack_top_location()
        .ok_or_else(|| anyhow::anyhow!("build-info callback requires pinned source authority"))?;
    let mut matching = sources
        .iter()
        .filter(|(filename, _)| filename.as_str() == span.filename());
    let (_, source) = matching
        .next()
        .ok_or_else(|| anyhow::anyhow!("build-info callback requires pinned source authority"))?;
    let evaluated: [u8; 32] = Sha256::digest(span.file.source().as_bytes()).into();
    let site = span.resolve_span();
    // Pinned bazel_cc_build_info.bzl36/37 select immutable module-global defs.
    // Both defs are capture-free and the full file has no imports. Their names
    // alone carry no authority; these exact callsites fix callback expressions.
    let line = match kind {
        BuildInfoTransformKind::Version => 35,
        BuildInfoTransformKind::Info => 36,
    };
    anyhow::ensure!(
        matching.next().is_none()
            && source.identity.label.to_string() == LABEL
            && source.source_digest == SHA
            && evaluated == SHA
            && (site.begin.line, site.end.line) == (line, line),
        "build-info callback requires the unchanged pinned source and transform callsite"
    );
    let callback = BuildInfoCallbackRecipe::new(kind, source.identity.label.clone(), SHA)
        .map_err(anyhow::Error::msg)?;
    Ok(BuildInfoTransformRequest {
        callback,
        template: template.artifact().clone(),
        output_file_name,
    })
}

#[cfg(test)]
mod tests;
