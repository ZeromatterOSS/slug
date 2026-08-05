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
    LabelMap,
    FlagAlias,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative, Dupe)]
pub(super) struct LabelValues(pub(super) Arc<[ResolvedOptionLabel]>);

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) struct LabelToStringEntry {
    pub(super) label: ResolvedOptionLabel,
    pub(super) value: CompactString,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative, Dupe)]
pub(super) struct LabelMapValues(pub(super) Arc<[(CompactString, Option<ResolvedOptionLabel>)]>);

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) struct FlagAliasEntry {
    pub(super) alias: CompactString,
    pub(super) label: ResolvedOptionLabel,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative, Dupe)]
pub(super) struct RunUnderSuffix(pub(super) Arc<[CompactString]>);

#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
pub(super) enum RunUnder {
    Label {
        original: CompactString,
        suffix: RunUnderSuffix,
        label: ResolvedOptionLabel,
    },
    Command {
        original: CompactString,
        suffix: RunUnderSuffix,
        command: CompactString,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
pub(super) enum MixedValue {
    RunUnder(RunUnder),
    CustomFlag(CompactString),
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) enum LabelValue {
    Label(ResolvedOptionLabel),
    Labels(LabelValues),
    LabelToStringEntry(LabelToStringEntry),
    LabelMap(LabelMapValues),
    FlagAlias(FlagAliasEntry),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MixedFamily {
    RunUnder,
    CustomFlag,
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
        Some("LabelMapConverter.class")
            if option.class_name == "com.google.devtools.build.lib.rules.java.JavaOptions"
                && option.canonical_name == "bytecode_optimizers" =>
        {
            Some(LabelFamily::LabelMap)
        }
        Some("CoreOptionConverters.FlagAliasConverter.class")
            if option.class_name == "com.google.devtools.build.lib.analysis.config.CoreOptions"
                && option.canonical_name == "flag_alias" =>
        {
            Some(LabelFamily::FlagAlias)
        }
        _ => None,
    }
}

pub(super) fn classify_mixed(option: &NativeOptionDescriptor) -> Option<MixedFamily> {
    match option.converter {
        Some("RunUnderConverter.class")
            if option.class_name == "com.google.devtools.build.lib.analysis.config.CoreOptions"
                && option.canonical_name == "run_under" =>
        {
            Some(MixedFamily::RunUnder)
        }
        Some("CoreOptionConverters.CustomFlagConverter.class")
            if option.class_name == "com.google.devtools.build.lib.analysis.config.CoreOptions"
                && option.canonical_name == "experimental_propagate_custom_flag" =>
        {
            Some(MixedFamily::CustomFlag)
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
        LabelFamily::LabelMap => {
            label_map(input, context).map(|value| Some(LabelValue::LabelMap(value)))
        }
        LabelFamily::FlagAlias => {
            flag_alias(input, context).map(|value| Some(LabelValue::FlagAlias(value)))
        }
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
        LabelFamily::FlagAlias if input == "null" && option.allow_multiple => Ok(None),
        LabelFamily::List
        | LabelFamily::OrderedSet
        | LabelFamily::LabelToStringEntry
        | LabelFamily::FlagAlias => Err(LabelConvertError::Invalid),
        _ => convert_label_occurrence(option, input, context),
    }
}

pub(super) fn convert_mixed_occurrence(
    option: &NativeOptionDescriptor,
    input: &str,
    context: OptionLabelContext<'_>,
) -> Result<Option<MixedValue>, LabelConvertError> {
    match classify_mixed(option).ok_or(LabelConvertError::Unsupported)? {
        MixedFamily::RunUnder => {
            run_under(input, context).map(|value| Some(MixedValue::RunUnder(value)))
        }
        MixedFamily::CustomFlag => {
            custom_flag(input, context).map(|value| Some(MixedValue::CustomFlag(value)))
        }
    }
}

pub(super) fn materialize_mixed_default(
    option: &NativeOptionDescriptor,
    context: OptionLabelContext<'_>,
) -> Result<Option<MixedValue>, LabelConvertError> {
    let family = classify_mixed(option).ok_or(LabelConvertError::Unsupported)?;
    let Some(input) = option
        .raw_default
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Err(LabelConvertError::Unsupported);
    };
    match family {
        MixedFamily::RunUnder | MixedFamily::CustomFlag if input == "null" => Ok(None),
        _ => convert_mixed_occurrence(option, input, context),
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

fn label_map(
    input: &str,
    context: OptionLabelContext<'_>,
) -> Result<LabelMapValues, LabelConvertError> {
    let mut entries: Vec<(CompactString, Option<ResolvedOptionLabel>)> = Vec::new();
    for raw_entry in input.split(',') {
        let entry = trim_guava_whitespace(raw_entry);
        if entry.is_empty() {
            continue;
        }
        let (key, value) = entry
            .split_once('=')
            .map_or((entry, None), |(key, value)| (key, Some(value)));
        let label = value
            .filter(|value| !value.is_empty())
            .map(|value| label(value, context))
            .transpose()?;
        if entries.iter().any(|(existing, _)| existing.as_str() == key) {
            return Err(LabelConvertError::Invalid);
        }
        entries.push((CompactString::new(key), label));
    }
    Ok(LabelMapValues(Arc::from(entries)))
}

fn trim_guava_whitespace(input: &str) -> &str {
    input.trim_matches(is_guava_whitespace)
}

fn is_guava_whitespace(character: char) -> bool {
    matches!(
        character,
        '\u{0009}'..='\u{000d}'
            | '\u{0020}'
            | '\u{0085}'
            | '\u{00a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
    )
}

fn flag_alias(
    input: &str,
    context: OptionLabelContext<'_>,
) -> Result<FlagAliasEntry, LabelConvertError> {
    let Some(position) = input.find('=') else {
        return Err(LabelConvertError::Invalid);
    };
    if position == 0 {
        return Err(LabelConvertError::Invalid);
    }
    let short_form = &input[..position];
    let long_form = &input[position + 1..];
    if !short_form
        .bytes()
        .all(|character| character.is_ascii_alphanumeric() || character == b'_')
    {
        return Err(LabelConvertError::Invalid);
    }
    if long_form.contains('=') {
        return Err(LabelConvertError::Invalid);
    }
    if !(long_form.starts_with("//")
        || long_form.starts_with("no//")
        || long_form.starts_with('@')
        || long_form.starts_with("no@"))
    {
        return Err(LabelConvertError::Invalid);
    }
    Ok(FlagAliasEntry {
        alias: CompactString::new(short_form),
        label: label(long_form, context)?,
    })
}

fn run_under(input: &str, context: OptionLabelContext<'_>) -> Result<RunUnder, LabelConvertError> {
    let (command, suffix) = shell_tokens(input)?;
    let command = command.ok_or(LabelConvertError::Invalid)?;
    let suffix = RunUnderSuffix(Arc::from(suffix));
    let original = CompactString::new(input);
    if command.starts_with("//") || command.starts_with('@') {
        Ok(RunUnder::Label {
            original,
            suffix,
            label: label(&command, context)?,
        })
    } else {
        Ok(RunUnder::Command {
            original,
            suffix,
            command,
        })
    }
}

fn shell_tokens(
    input: &str,
) -> Result<(Option<CompactString>, Vec<CompactString>), LabelConvertError> {
    let mut first = None;
    let mut suffix = Vec::new();
    let mut token = CompactString::new("");
    let mut force_token = false;
    let mut quotation = None;
    let mut characters = input.chars();
    while let Some(character) = characters.next() {
        if let Some(quote) = quotation {
            if character == quote {
                quotation = None;
            } else if character == '\\' && quote == '"' {
                let escaped = characters.next().ok_or(LabelConvertError::Invalid)?;
                if escaped != '\\' && escaped != '"' {
                    token.push('\\');
                }
                token.push(escaped);
            } else {
                token.push(character);
            }
        } else if character == '\'' || character == '"' {
            quotation = Some(character);
            force_token = true;
        } else if character == ' ' || character == '\t' {
            if force_token || !token.is_empty() {
                push_shell_token(
                    &mut first,
                    &mut suffix,
                    std::mem::replace(&mut token, CompactString::new("")),
                );
                force_token = false;
            }
        } else if character == '\\' {
            token.push(characters.next().ok_or(LabelConvertError::Invalid)?);
        } else {
            token.push(character);
        }
    }
    if quotation.is_some() {
        return Err(LabelConvertError::Invalid);
    }
    if force_token || !token.is_empty() {
        push_shell_token(&mut first, &mut suffix, token);
    }
    Ok((first, suffix))
}

fn push_shell_token(
    first: &mut Option<CompactString>,
    suffix: &mut Vec<CompactString>,
    token: CompactString,
) {
    if first.is_none() {
        *first = Some(token);
    } else {
        suffix.push(token);
    }
}

fn custom_flag(
    input: &str,
    context: OptionLabelContext<'_>,
) -> Result<CompactString, LabelConvertError> {
    if !input.starts_with("//") && !input.starts_with('@') {
        return Ok(CompactString::new(input));
    }
    let mut unambiguous = if let Some(prefix) = input.strip_suffix("/...") {
        label(&format!("{prefix}:__subpackages__"), context)?.unambiguous_form()
    } else {
        label(input, context)?.unambiguous_form()
    };
    if unambiguous.ends_with(":__subpackages__") {
        unambiguous.truncate(unambiguous.len() - ":__subpackages__".len());
        unambiguous.push_str("/...");
    }
    Ok(CompactString::from(unambiguous))
}

fn empty() -> LabelValues {
    LabelValues(Arc::from([]))
}
