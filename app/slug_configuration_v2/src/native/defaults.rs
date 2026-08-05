use std::sync::Arc;

use crate::native::convert::*;
use crate::native::registry::NativeOptionDescriptor;
use crate::native::value::*;
pub(super) fn materialize_default(
    o: &NativeOptionDescriptor,
) -> Result<Option<NativeValue>, ConvertError> {
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
