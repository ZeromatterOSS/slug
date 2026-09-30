//! A finite registration descriptor for one unchanged module-global callback.
//! No Starlark callback or resource estimate is evaluated or retained.
use sha2::Digest;
use sha2::Sha256;
use slug_build_api_v2::actions::RetainedSpawnResourceSet;
use starlark::eval::Evaluator;
use starlark::values::Value;

use crate::builtin_restriction::source_identities_for_evaluator;

pub(crate) fn authenticate(
    callback: Value<'_>,
    eval: &Evaluator<'_, '_, '_>,
) -> Option<RetainedSpawnResourceSet> {
    // This downcasts concrete Def/FrozenDef, excluding native/callable spoofs.
    callback.parameters_spec()?;
    let span = eval.call_stack_top_location()?;
    let sources = source_identities_for_evaluator(eval).ok()?;
    let mut matches = sources
        .iter()
        .filter(|(filename, _)| filename.as_str() == span.filename());
    let (_, source) = matches.next()?;
    let site = span.resolve_span();
    let evaluated: [u8; 32] = Sha256::digest(span.file.source().as_bytes()).into();
    // The full pinned file fixes the literal module-global _resource_set argument
    // and capture-free definition. Names/allowlists alone provide no authority.
    if matches.next().is_some()
        || source.identity.label.to_string() != RetainedSpawnResourceSet::SOURCE_LABEL
        || source.source_digest != RetainedSpawnResourceSet::SOURCE_DIGEST
        || evaluated != RetainedSpawnResourceSet::SOURCE_DIGEST
        // The retained compiler records args.lparen, not the full call expression.
        || (site.begin.line, site.end.line) != (380, 380)
    {
        return None;
    }
    RetainedSpawnResourceSet::new(source.identity.label.clone(), evaluated).ok()
}

#[cfg(test)]
mod tests;
