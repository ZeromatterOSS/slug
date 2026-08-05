use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_identity_v2::OptionLabelContext;
use slug_identity_v2::ResolvedOptionLabel;

use crate::native::registry::NativeOptionDescriptor;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LabelConvertError {
    Invalid,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LabelFamily {
    Label,
    EmptyToNull,
    List,
    OrderedSet,
    LibcTop,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative, Dupe)]
pub(super) struct LabelValues(pub(super) Arc<[ResolvedOptionLabel]>);

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) enum LabelValue {
    Label(ResolvedOptionLabel),
    Labels(LabelValues),
}

pub(super) fn classify(option: &NativeOptionDescriptor) -> Option<LabelFamily> {
    match option.converter {
        Some("LabelConverter.class") => Some(LabelFamily::Label),
        Some("EmptyToNullLabelConverter.class") => Some(LabelFamily::EmptyToNull),
        Some("LabelListConverter.class") => Some(LabelFamily::List),
        Some("LabelOrderedSetConverter.class") => Some(LabelFamily::OrderedSet),
        Some("LibcTopLabelConverter.class") => Some(LabelFamily::LibcTop),
        _ => None,
    }
}

pub(super) fn convert_label_occurrence(
    option: &NativeOptionDescriptor,
    input: &str,
    context: OptionLabelContext<'_>,
) -> Result<Option<LabelValue>, LabelConvertError> {
    match classify(option).ok_or(LabelConvertError::Unsupported)? {
        LabelFamily::Label => label(input, context).map(|value| Some(LabelValue::Label(value))),
        LabelFamily::EmptyToNull => {
            if input.is_empty() {
                Ok(None)
            } else {
                label(input, context).map(|value| Some(LabelValue::Label(value)))
            }
        }
        LabelFamily::List => list(input, context).map(|values| Some(LabelValue::Labels(values))),
        LabelFamily::OrderedSet => {
            ordered_set(input, context).map(|values| Some(LabelValue::Labels(values)))
        }
        LabelFamily::LibcTop => libc_top(input, context),
    }
}

pub(super) fn materialize_label_default(
    option: &NativeOptionDescriptor,
    context: OptionLabelContext<'_>,
) -> Result<Option<LabelValue>, LabelConvertError> {
    let family = classify(option).ok_or(LabelConvertError::Unsupported)?;
    let Some(input) = option
        .raw_default
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Err(LabelConvertError::Unsupported);
    };
    match family {
        LabelFamily::List if input.is_empty() || (input == "null" && option.allow_multiple) => {
            Ok(Some(LabelValue::Labels(empty())))
        }
        LabelFamily::OrderedSet if input.is_empty() => Ok(Some(LabelValue::Labels(empty()))),
        LabelFamily::Label | LabelFamily::EmptyToNull | LabelFamily::LibcTop if input == "null" => {
            Ok(None)
        }
        LabelFamily::List | LabelFamily::OrderedSet => Err(LabelConvertError::Invalid),
        _ => convert_label_occurrence(option, input, context),
    }
}

fn label(
    input: &str,
    context: OptionLabelContext<'_>,
) -> Result<ResolvedOptionLabel, LabelConvertError> {
    context.parse(input).map_err(|_| LabelConvertError::Invalid)
}

fn list(input: &str, context: OptionLabelContext<'_>) -> Result<LabelValues, LabelConvertError> {
    input
        .split(',')
        .filter(|part| !part.is_empty())
        .map(|part| label(part, context))
        .collect::<Result<Vec<_>, _>>()
        .map(|values| LabelValues(Arc::from(values)))
}

fn ordered_set(
    input: &str,
    context: OptionLabelContext<'_>,
) -> Result<LabelValues, LabelConvertError> {
    let parsed = input
        .split(',')
        .filter(|part| !part.is_empty())
        .map(|part| label(part, context))
        .collect::<Result<Vec<_>, _>>()?;
    let mut values = Vec::with_capacity(parsed.len());
    for value in parsed {
        if !values.contains(&value) {
            values.push(value);
        }
    }
    Ok(LabelValues(Arc::from(values)))
}

fn libc_top(
    input: &str,
    context: OptionLabelContext<'_>,
) -> Result<Option<LabelValue>, LabelConvertError> {
    if input == "default" {
        return Ok(None);
    }
    let Some(package) = input.strip_prefix("//") else {
        return Err(LabelConvertError::Invalid);
    };
    let package = package
        .split_once(':')
        .map_or(package, |(package, _)| package);
    label(&format!("//{package}:everything"), context).map(|value| Some(LabelValue::Label(value)))
}

fn empty() -> LabelValues {
    LabelValues(Arc::from([]))
}
