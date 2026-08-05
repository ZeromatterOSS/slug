use std::sync::Arc;

use crate::native::convert::*;
use crate::native::registry::NativeOptionDescriptor;
use crate::native::value::*;
pub(super) fn materialize_default(
    o: &NativeOptionDescriptor,
) -> Result<Option<NativeValue>, ConvertError> {
    if let Some(seed) = regex_filter_default_seed(o) {
        return Ok(Some(NativeValue::RegexFilterDefault(seed)));
    }
    if o.canonical_name == "runs_per_test"
        && o.converter == Some("RunsPerTestConverter.class")
        && o.raw_default == "\"1\""
    {
        return Ok(Some(NativeValue::List(NativeValues(Arc::from([
            NativeValue::Runs(RunsPerTestSeed::one()),
        ])))));
    }
    let f = classify(o).ok_or(ConvertError::Unsupported)?;
    let x = o
        .raw_default
        .strip_prefix('"')
        .and_then(|raw| raw.strip_suffix('"'))
        .ok_or(ConvertError::Invalid)?;
    if x == "null" {
        return Ok(match f {
            NativeFamily::Bool => Some(NativeValue::Bool(false)),
            NativeFamily::Tri => Some(NativeValue::Tri(TriState::Auto)),
            _ if o.allow_multiple => Some(NativeValue::List(NativeValues(Arc::from([])))),
            _ => None,
        });
    }
    Ok(Some(scalar(f, x)?))
}
fn regex_filter_default_seed(o: &NativeOptionDescriptor) -> Option<RegexFilterDefaultSeed> {
    let semantic = match (
        o.class_name,
        o.canonical_name,
        o.field_type,
        o.converter,
        o.raw_default,
        o.allow_multiple,
    ) {
        (
            "com.google.devtools.build.lib.analysis.PlatformOptions",
            "toolchain_resolution_debug",
            "RegexFilter",
            Some("RegexFilter.RegexFilterConverter.class"),
            "\"-.*\"",
            false,
        )
        | (
            "com.google.devtools.build.lib.analysis.config.CoreOptions",
            "archived_tree_artifact_mnemonics_filter",
            "RegexFilter",
            Some("RegexFilter.RegexFilterConverter.class"),
            "\"-.*\"",
            false,
        ) => RegexFilterDefaultSemantic::ExcludeAll,
        (
            "com.google.devtools.build.lib.analysis.config.CoreOptions",
            "instrumentation_filter",
            "RegexFilter",
            Some("RegexFilter.RegexFilterConverter.class"),
            "\"-/javatests[/:],-/test/java[/:]\"",
            false,
        ) => RegexFilterDefaultSemantic::InstrumentationDefault,
        _ => return None,
    };
    let original_input = o.raw_default.strip_prefix('"')?.strip_suffix('"')?;
    Some(RegexFilterDefaultSeed::new(original_input, semantic))
}
