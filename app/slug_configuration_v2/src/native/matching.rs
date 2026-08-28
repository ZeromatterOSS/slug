use std::fmt;

use compact_str::CompactString;
use slug_identity_v2::OptionLabelContext;

use super::configuration::OptionRecord;
use super::configuration::OptionValue;
use super::convert;
use super::convert::ConvertError;
use super::label_convert;
use super::label_convert::LabelConvertError;
use super::registry::NATIVE_OPTION_DESCRIPTORS;
use super::registry::NativeOptionDescriptor;
use super::value::NativeOccurrence;
use super::value::NativeValue;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeConfigSettingMatchError {
    UnknownOption(CompactString),
    NonConfigurableOption(CompactString),
    InvalidValue(CompactString),
    UnsupportedOption(CompactString),
}

impl fmt::Display for NativeConfigSettingMatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownOption(name) => write!(formatter, "unknown option: '{name}'"),
            Self::NonConfigurableOption(name) => {
                write!(formatter, "select() on '{name}' is not allowed")
            }
            Self::InvalidValue(name) => {
                write!(formatter, "invalid value for native option '{name}'")
            }
            Self::UnsupportedOption(name) => {
                write!(
                    formatter,
                    "unsupported native option converter for '{name}'"
                )
            }
        }
    }
}

impl std::error::Error for NativeConfigSettingMatchError {}

const NON_CONFIGURABLE: &[&str] = &[
    "platform_mappings",
    "incompatible_disable_select_on",
    "check_visibility",
    "verbose_visibility_errors",
    "experimental_allow_map_directory",
    "flag_alias",
];

pub(super) fn matches(
    options: &[OptionRecord],
    values: &[(CompactString, CompactString)],
    define_values: &[(CompactString, CompactString)],
) -> Result<bool, NativeConfigSettingMatchError> {
    let mut matched = true;
    for (name, expected) in values {
        matched &= matches_one(options, name, expected)?;
    }
    for (key, value) in define_values {
        let expected = format!("{key}={value}");
        matched &= matches_one(options, "define", &expected)?;
    }
    Ok(matched)
}

fn matches_one(
    options: &[OptionRecord],
    requested_name: &str,
    expected: &str,
) -> Result<bool, NativeConfigSettingMatchError> {
    if disabled_by_configuration(options, requested_name) {
        return Err(NativeConfigSettingMatchError::NonConfigurableOption(
            requested_name.into(),
        ));
    }
    let (descriptor, record) = find_option(options, requested_name)
        .ok_or_else(|| NativeConfigSettingMatchError::UnknownOption(requested_name.into()))?;
    if NON_CONFIGURABLE.contains(&descriptor.canonical_name) {
        return Err(NativeConfigSettingMatchError::NonConfigurableOption(
            requested_name.into(),
        ));
    }
    match &record.value {
        OptionValue::Native(actual) => {
            let expected = convert::convert_occurrence(descriptor, expected)
                .map_err(|error| conversion_error(requested_name, error))?;
            Ok(native_occurrence_matches(
                actual,
                &expected,
                descriptor.allow_multiple,
            ))
        }
        OptionValue::Label(actual) => {
            let expected = label_convert::convert_label_occurrence(
                descriptor,
                expected,
                OptionLabelContext::FirstRoundCanonical,
            )
            .map_err(|error| label_conversion_error(requested_name, error))?;
            if descriptor.allow_multiple {
                Ok(label_multiple_matches(actual.as_ref(), expected.as_ref()))
            } else {
                Ok(actual == &expected)
            }
        }
        OptionValue::Mixed(actual) => {
            let expected = label_convert::convert_mixed_occurrence(
                descriptor,
                expected,
                OptionLabelContext::FirstRoundCanonical,
            )
            .map_err(|error| label_conversion_error(requested_name, error))?;
            Ok(actual == &expected)
        }
    }
}

fn find_option<'a>(
    options: &'a [OptionRecord],
    requested_name: &str,
) -> Option<(&'static NativeOptionDescriptor, &'a OptionRecord)> {
    NATIVE_OPTION_DESCRIPTORS
        .iter()
        .zip(options)
        .find(|(descriptor, record)| {
            debug_assert_eq!(descriptor.class_name, record.class_name);
            debug_assert_eq!(descriptor.canonical_name, record.canonical_name);
            usize::try_from(record.ordinal)
                .ok()
                .is_some_and(|ordinal| NATIVE_OPTION_DESCRIPTORS.get(ordinal) == Some(*descriptor))
                && !is_internal(descriptor)
                && (descriptor.canonical_name == requested_name
                    || old_name(descriptor) == Some(requested_name))
        })
}

fn is_internal(descriptor: &NativeOptionDescriptor) -> bool {
    // Every INTERNAL FragmentOptions row in the pinned Bazel 9.2 registry has
    // a deliberately non-command-line name containing a space. Conversely,
    // every retained command-line option name is space-free.
    descriptor.canonical_name.contains(' ')
}

fn disabled_by_configuration(options: &[OptionRecord], requested_name: &str) -> bool {
    let Some((_, record)) = find_option(options, "incompatible_disable_select_on") else {
        debug_assert!(false, "pinned disabled-select option is missing");
        return false;
    };
    let OptionValue::Native(NativeOccurrence::Scalar(NativeValue::List(names))) = &record.value
    else {
        debug_assert!(
            false,
            "disabled-select option has an unexpected typed value"
        );
        return false;
    };
    names
        .iter()
        .any(|name| matches!(name, NativeValue::Text(name) if name == requested_name))
}

fn old_name(descriptor: &NativeOptionDescriptor) -> Option<&str> {
    descriptor
        .old_name?
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
}

fn conversion_error(name: &str, error: ConvertError) -> NativeConfigSettingMatchError {
    match error {
        ConvertError::Invalid => NativeConfigSettingMatchError::InvalidValue(name.into()),
        ConvertError::Unsupported => NativeConfigSettingMatchError::UnsupportedOption(name.into()),
    }
}

fn label_conversion_error(name: &str, error: LabelConvertError) -> NativeConfigSettingMatchError {
    match error {
        LabelConvertError::Invalid => NativeConfigSettingMatchError::InvalidValue(name.into()),
        LabelConvertError::Unsupported => {
            NativeConfigSettingMatchError::UnsupportedOption(name.into())
        }
    }
}

pub(super) fn native_occurrence_matches(
    actual: &NativeOccurrence,
    expected: &NativeOccurrence,
    allow_multiple: bool,
) -> bool {
    if !allow_multiple {
        return actual == expected;
    }
    let actual = OccurrenceValues::new(actual);
    let expected = OccurrenceValues::new(expected);
    if actual.is_empty() || expected.is_empty() {
        return actual.is_empty() && expected.is_empty();
    }
    if matches!(actual.first(), Some(NativeValue::Entry(_, _))) {
        let [NativeValue::Entry(expected_key, expected_value)] = expected.as_slice() else {
            return false;
        };
        return actual.iter().rev().find_map(|value| match value {
            NativeValue::Entry(key, value) if key == expected_key => Some(value == expected_value),
            _ => None,
        }) == Some(true);
    }
    expected
        .iter()
        .all(|expected| actual.as_slice().contains(expected))
}

enum OccurrenceValues<'a> {
    Empty,
    One(&'a NativeValue),
    Many(&'a [NativeValue]),
}

impl<'a> OccurrenceValues<'a> {
    fn new(value: &'a NativeOccurrence) -> Self {
        match value {
            NativeOccurrence::Absent => Self::Empty,
            NativeOccurrence::Scalar(value) => Self::One(value),
            NativeOccurrence::List(values) => Self::Many(values),
        }
    }

    fn as_slice(&self) -> &[NativeValue] {
        match self {
            Self::Empty => &[],
            Self::One(value) => std::slice::from_ref(value),
            Self::Many(values) => values,
        }
    }

    fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }

    fn first(&self) -> Option<&NativeValue> {
        self.as_slice().first()
    }

    fn iter(&self) -> std::slice::Iter<'_, NativeValue> {
        self.as_slice().iter()
    }
}

fn label_multiple_matches(
    actual: Option<&label_convert::LabelValue>,
    expected: Option<&label_convert::LabelValue>,
) -> bool {
    use label_convert::LabelValue;
    match (actual, expected) {
        (None, None) => true,
        (Some(LabelValue::Labels(actual)), Some(LabelValue::Labels(expected))) => expected
            .0
            .iter()
            .all(|expected| actual.0.contains(expected)),
        (Some(actual), Some(expected)) => actual == expected,
        _ => false,
    }
}
