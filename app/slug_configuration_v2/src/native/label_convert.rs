use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
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
    HostPlatform,
    CoreLabel,
    CoreEmptyToNull,
    LabelToStringEntry,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative, Dupe)]
pub(super) struct LabelValues(pub(super) Arc<[ResolvedOptionLabel]>);

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) struct LabelToStringEntry {
    pub(super) label: ResolvedOptionLabel,
    pub(super) value: CompactString,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) enum LabelValue {
    Label(ResolvedOptionLabel),
    Labels(LabelValues),
    LabelToStringEntry(LabelToStringEntry),
}

struct LiteralDefault {
    class_name: &'static str,
    canonical_name: &'static str,
    raw_default: &'static str,
    literal: &'static str,
}

const LITERAL_DEFAULTS: [LiteralDefault; 6] = [
    LiteralDefault {
        class_name: "com.google.devtools.build.lib.analysis.PlatformOptions",
        canonical_name: "host_platform",
        raw_default: "DEFAULT_HOST_PLATFORM",
        literal: "@bazel_tools//tools:host_platform",
    },
    LiteralDefault {
        class_name: "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options",
        canonical_name: "proto_compiler",
        raw_default: "ProtoConstants.DEFAULT_PROTOC_LABEL",
        literal: "@bazel_tools//tools/proto:protoc",
    },
    LiteralDefault {
        class_name: "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options",
        canonical_name: "proto_toolchain_for_cc",
        raw_default: "ProtoConstants.DEFAULT_CC_PROTO_LABEL",
        literal: "@bazel_tools//tools/proto:cc_toolchain",
    },
    LiteralDefault {
        class_name: "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options",
        canonical_name: "proto_toolchain_for_j2objc",
        raw_default: "ProtoConstants.DEFAULT_J2OBJC_PROTO_LABEL",
        literal: "@bazel_tools//tools/j2objc:j2objc_proto_toolchain",
    },
    LiteralDefault {
        class_name: "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options",
        canonical_name: "proto_toolchain_for_java",
        raw_default: "ProtoConstants.DEFAULT_JAVA_PROTO_LABEL",
        literal: "@bazel_tools//tools/proto:java_toolchain",
    },
    LiteralDefault {
        class_name: "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options",
        canonical_name: "proto_toolchain_for_javalite",
        raw_default: "ProtoConstants.DEFAULT_JAVA_LITE_PROTO_LABEL",
        literal: "@bazel_tools//tools/proto:javalite_toolchain",
    },
];

pub(super) fn classify(option: &NativeOptionDescriptor) -> Option<LabelFamily> {
    match option.converter {
        Some("LabelConverter.class") => Some(LabelFamily::Label),
        Some("EmptyToNullLabelConverter.class") => Some(LabelFamily::EmptyToNull),
        Some("LabelListConverter.class") => Some(LabelFamily::List),
        Some("LabelOrderedSetConverter.class") => Some(LabelFamily::OrderedSet),
        Some("LibcTopLabelConverter.class") => Some(LabelFamily::LibcTop),
        Some("HostPlatformConverter.class") if literal_default(option).is_some() => {
            Some(LabelFamily::HostPlatform)
        }
        Some("CoreOptionConverters.LabelConverter.class") if literal_default(option).is_some() => {
            Some(LabelFamily::CoreLabel)
        }
        Some("CoreOptionConverters.EmptyToNullLabelConverter.class")
            if literal_default(option).is_some() =>
        {
            Some(LabelFamily::CoreEmptyToNull)
        }
        Some("LabelToStringEntryConverter.class")
            if option.class_name == "com.google.devtools.build.lib.analysis.config.CoreOptions"
                && option.canonical_name == "experimental_override_platform_cpu_name" =>
        {
            Some(LabelFamily::LabelToStringEntry)
        }
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
        LabelFamily::HostPlatform => {
            let input = if input.is_empty() {
                literal_default(option)
                    .expect("classified host platform has a literal default")
                    .literal
            } else {
                input
            };
            label(input, context).map(|value| Some(LabelValue::Label(value)))
        }
        LabelFamily::CoreLabel => label(input, context).map(|value| Some(LabelValue::Label(value))),
        LabelFamily::CoreEmptyToNull => {
            if input.is_empty() {
                Ok(None)
            } else {
                label(input, context).map(|value| Some(LabelValue::Label(value)))
            }
        }
        LabelFamily::LabelToStringEntry => label_to_string_entry(input, context)
            .map(|value| Some(LabelValue::LabelToStringEntry(value))),
    }
}

pub(super) fn materialize_label_default(
    option: &NativeOptionDescriptor,
    context: OptionLabelContext<'_>,
) -> Result<Option<LabelValue>, LabelConvertError> {
    let family = classify(option).ok_or(LabelConvertError::Unsupported)?;
    if let Some(default) = literal_default(option) {
        return label(default.literal, context).map(|value| Some(LabelValue::Label(value)));
    }
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
        LabelFamily::LabelToStringEntry if input == "null" && option.allow_multiple => Ok(None),
        LabelFamily::List | LabelFamily::OrderedSet | LabelFamily::LabelToStringEntry => {
            Err(LabelConvertError::Invalid)
        }
        _ => convert_label_occurrence(option, input, context),
    }
}

fn literal_default(option: &NativeOptionDescriptor) -> Option<&'static LiteralDefault> {
    LITERAL_DEFAULTS.iter().find(|default| {
        option.class_name == default.class_name
            && option.canonical_name == default.canonical_name
            && option.raw_default == default.raw_default
    })
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

fn label_to_string_entry(
    input: &str,
    context: OptionLabelContext<'_>,
) -> Result<LabelToStringEntry, LabelConvertError> {
    if input.bytes().filter(|byte| *byte == b'=').count() != 1
        || input.starts_with('=')
        || input.ends_with('=')
    {
        return Err(LabelConvertError::Invalid);
    }
    let (name, value) = input
        .split_once('=')
        .expect("exactly one delimiter has already been verified");
    Ok(LabelToStringEntry {
        label: label(name, context)?,
        value: CompactString::new(value),
    })
}

fn empty() -> LabelValues {
    LabelValues(Arc::from([]))
}
