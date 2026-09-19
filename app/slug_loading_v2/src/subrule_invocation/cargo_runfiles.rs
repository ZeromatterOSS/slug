//! Authenticate one pinned Cargo closure and snapshot only its semantic captures.

use starlark::codemap::FileSpan;

use super::*;

pub(super) const SOURCE_LABEL: &str = "@@rules_rust+//cargo/private:cargo_build_script.bzl";
const SOURCE_SHA256: [u8; 32] = [
    0x61, 0x47, 0xdf, 0x93, 0x87, 0x23, 0xec, 0x58, 0xce, 0xf6, 0x46, 0x16, 0x98, 0x14, 0xc8, 0x5a,
    0x72, 0xfa, 0x0f, 0x74, 0x08, 0x04, 0x71, 0x75, 0x6e, 0x3c, 0x18, 0x91, 0xd0, 0x2f, 0x46, 0x30,
];

pub(super) fn capture<'v>(
    source: &BzlModuleSourceProvenance,
    evaluated_digest: [u8; 32],
    span: &FileSpan,
    callback: Value<'v>,
    allow_closure: bool,
    eval: &Evaluator<'v, '_, '_>,
) -> anyhow::Result<EvaluatorVectorMapEachGen<Value<'v>>> {
    let callsite = span.resolve_span();
    if source.identity.label.to_string() != SOURCE_LABEL
        || source.source_digest != SOURCE_SHA256
        || evaluated_digest != SOURCE_SHA256
        || (callsite.begin.line, callsite.end.line) != (356, 356)
        || !allow_closure
        || callback.to_repr() != format!("<function _runfiles_map from {}>", span.filename())
    {
        anyhow::bail!("Cargo runfiles Args requires the pinned callback source and callsite");
    }
    let context = eval
        .native_call_context("workspace_name")
        .filter(|context| context.function_name == "_create_runfiles_dir")
        .ok_or_else(|| {
            anyhow::anyhow!("Cargo runfiles Args requires its immediate helper caller")
        })?;
    let workspace_name = context
        .local_value
        .ok_or_else(|| anyhow::anyhow!("Cargo runfiles Args workspace_name must be a string"))?;
    let fake_exe = eval
        .native_call_local_value("fake_exe")
        .filter(|value| AnalysisArtifactValue::from_starlark(*value).is_some())
        .ok_or_else(|| anyhow::anyhow!("Cargo runfiles Args fake_exe must be a File"))?;
    // Trace this Value in the Args owner; never retain the callback or its frame.
    Ok(EvaluatorVectorMapEachGen::CargoRunfiles {
        fake_exe,
        workspace_name: workspace_name.into(),
    })
}

#[cfg(test)]
mod tests;
