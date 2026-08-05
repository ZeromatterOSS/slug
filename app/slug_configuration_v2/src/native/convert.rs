use std::sync::Arc;

use compact_str::CompactString;

use crate::native::registry::NativeOptionDescriptor;
use crate::native::value::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConvertError {
    Invalid,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeFamily {
    Bool,
    Int,
    Text,
    Tri,
    Void,
    Duration,
    Comma,
    Set,
    Entry,
    Env,
    Dotted,
    Timeout,
    Shard,
    Fission,
    Platform,
    Empty,
    Enum(EnumFamily),
}

impl NativeFamily {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "F-Bool",
            Self::Int => "F-Int",
            Self::Text => "F-Text",
            Self::Tri => "F-Tri",
            Self::Void => "F-Void",
            Self::Duration => "F-Duration",
            Self::Comma => "F-AllowCommaList",
            Self::Set => "F-StringSet",
            Self::Entry => "F-Entry",
            Self::Env => "F-Env",
            Self::Dotted => "F-Dotted",
            Self::Timeout => "F-Timeout",
            Self::Shard => "F-Shard",
            Self::Fission => "F-Fission",
            Self::Platform => "F-Platform",
            Self::Empty => "F-EmptyList",
            Self::Enum(family) => enum_family_name(family),
        }
    }
}

fn enum_family_name(family: EnumFamily) -> &'static str {
    match family {
        EnumFamily::StrictDeps => "F-Enum-StrictDeps",
        EnumFamily::Exec => "F-Enum-ExecConfigurationDistinguisher",
        EnumFamily::OutputName => "F-Enum-OutputDirectoryNaming",
        EnumFamily::OutputPaths => "F-Enum-OutputPaths",
        EnumFamily::Include => "F-Enum-IncludeConfigFragments",
        EnumFamily::Android => "F-Enum-AndroidConfigurationDistinguisher",
        EnumFamily::Apk => "F-Enum-ApkSigningMethod",
        EnumFamily::Merger => "F-Enum-AndroidManifestMerger",
        EnumFamily::MergerOrder => "F-Enum-ManifestMergerOrder",
        EnumFamily::Apple => "F-Enum-AppleConfigurationDistinguisher",
        EnumFamily::Dynamic => "F-Enum-DynamicMode",
        EnumFamily::Classpath => "F-Enum-JavaClasspathMode",
        EnumFamily::OneVersion => "F-Enum-JavaOneVersionLevel",
        EnumFamily::Cancel => "F-Enum-Cancel",
        EnumFamily::Compilation => "F-Enum-CompilationMode",
        EnumFamily::Strip => "F-Enum-StripMode",
    }
}

pub(super) fn classify(option: &NativeOptionDescriptor) -> Option<NativeFamily> {
    let converter = option.converter.unwrap_or("");
    Some(match option.field_type {
        "boolean" => NativeFamily::Bool,
        "int" => NativeFamily::Int,
        "TriState" => NativeFamily::Tri,
        "Void" => NativeFamily::Void,
        "CompilationMode" => NativeFamily::Enum(EnumFamily::Compilation),
        "StrictDepsMode" => NativeFamily::Enum(EnumFamily::StrictDeps),
        "ExecConfigurationDistinguisherScheme" => NativeFamily::Enum(EnumFamily::Exec),
        "OutputDirectoryNamingScheme" => NativeFamily::Enum(EnumFamily::OutputName),
        "OutputPathsMode" => NativeFamily::Enum(EnumFamily::OutputPaths),
        "IncludeConfigFragmentsEnum" => NativeFamily::Enum(EnumFamily::Include),
        "DynamicMode" => NativeFamily::Enum(EnumFamily::Dynamic),
        "JavaClasspathMode" => NativeFamily::Enum(EnumFamily::Classpath),
        "OneVersionEnforcementLevel" => NativeFamily::Enum(EnumFamily::OneVersion),
        "CancelConcurrentTests" => NativeFamily::Enum(EnumFamily::Cancel),
        "StripMode" => NativeFamily::Enum(EnumFamily::Strip),
        "ApkSigningMethod" => NativeFamily::Enum(EnumFamily::Apk),
        "AndroidManifestMerger" => NativeFamily::Enum(EnumFamily::Merger),
        "ManifestMergerOrder" => NativeFamily::Enum(EnumFamily::MergerOrder),
        "ConfigurationDistinguisher" if option.class_name.contains(".rules.android.") => {
            NativeFamily::Enum(EnumFamily::Android)
        }
        "ConfigurationDistinguisher" => NativeFamily::Enum(EnumFamily::Apple),
        _ if converter.ends_with("DurationConverter.class") => NativeFamily::Duration,
        _ if converter.ends_with("CommaSeparatedOptionListConverter.class") => NativeFamily::Comma,
        _ if converter.ends_with("CommaSeparatedOptionSetConverter.class") => NativeFamily::Set,
        _ if converter == "Converters.AssignmentConverter.class" => NativeFamily::Entry,
        _ if converter == "Converters.EnvVarsConverter.class" => NativeFamily::Env,
        _ if converter == "DottedVersionConverter.class" => NativeFamily::Dotted,
        _ if converter == "TestTimeout.TestTimeoutConverter.class" => NativeFamily::Timeout,
        _ if converter == "ShardingStrategyConverter.class" => NativeFamily::Shard,
        _ if converter == "FissionOptionConverter.class" => NativeFamily::Fission,
        _ if converter == "PlatformTypeConverter.class" => NativeFamily::Platform,
        _ if converter == "EmptyListConverter.class" => NativeFamily::Empty,
        _ if option.converter.is_none()
            && matches!(option.field_type, "String" | "List<String>") =>
        {
            NativeFamily::Text
        }
        _ => return None,
    })
}

pub(super) fn convert_occurrence(
    option: &NativeOptionDescriptor,
    input: &str,
) -> Result<NativeOccurrence, ConvertError> {
    let family = classify(option).ok_or(ConvertError::Unsupported)?;
    if family == NativeFamily::Void {
        return if input == "null" {
            Ok(NativeOccurrence::Absent)
        } else {
            Err(ConvertError::Invalid)
        };
    }
    let value = scalar(family, input)?;
    match (option.allow_multiple, value) {
        (true, NativeValue::List(values)) => Ok(NativeOccurrence::List(values)),
        (_, value) => Ok(NativeOccurrence::Scalar(value)),
    }
}

pub(super) fn scalar(family: NativeFamily, input: &str) -> Result<NativeValue, ConvertError> {
    Ok(match family {
        NativeFamily::Bool => NativeValue::Bool(boolean(input)?),
        NativeFamily::Int => NativeValue::Int(integer(input)?),
        NativeFamily::Text => NativeValue::Text(input.into()),
        NativeFamily::Tri => NativeValue::Tri(tri(input)?),
        NativeFamily::Duration => convert_duration(input)?,
        NativeFamily::Comma => NativeValue::List(comma(input)),
        NativeFamily::Set => NativeValue::List(set(input)),
        NativeFamily::Entry => entry(input)?,
        NativeFamily::Env => env(input)?,
        NativeFamily::Dotted => dotted(input)?,
        NativeFamily::Timeout => timeout(input)?,
        NativeFamily::Shard => shard(input)?,
        NativeFamily::Fission => NativeValue::List(fission(input)?),
        NativeFamily::Platform => NativeValue::Text(ascii_lower(input).into()),
        NativeFamily::Empty => NativeValue::List(empty()),
        NativeFamily::Enum(family) => enum_value(family, input)?,
        NativeFamily::Void => return Err(ConvertError::Invalid),
    })
}

fn ascii_lower(input: &str) -> String {
    input
        .chars()
        .map(|character| {
            if character.is_ascii_uppercase() {
                character.to_ascii_lowercase()
            } else {
                character
            }
        })
        .collect()
}

fn boolean(input: &str) -> Result<bool, ConvertError> {
    match ascii_lower(input).as_str() {
        "true" | "1" | "yes" | "t" | "y" => Ok(true),
        "false" | "0" | "no" | "f" | "n" | "null" => Ok(false),
        _ => Err(ConvertError::Invalid),
    }
}

fn tri(input: &str) -> Result<TriState, ConvertError> {
    match ascii_lower(input).as_str() {
        "auto" | "null" => Ok(TriState::Auto),
        "true" | "1" | "yes" | "t" | "y" => Ok(TriState::Yes),
        "false" | "0" | "no" | "f" | "n" => Ok(TriState::No),
        _ => Err(ConvertError::Invalid),
    }
}

fn integer(input: &str) -> Result<i32, ConvertError> {
    let (sign, unsigned) = match input.as_bytes().first() {
        Some(b'+') => (1_i64, &input[1..]),
        Some(b'-') => (-1_i64, &input[1..]),
        _ => (1_i64, input),
    };
    let (radix, digits) = if let Some(digits) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        (16, digits)
    } else if let Some(digits) = unsigned.strip_prefix('#') {
        (16, digits)
    } else if unsigned.len() > 1 && unsigned.starts_with('0') {
        (8, &unsigned[1..])
    } else {
        (10, unsigned)
    };
    if digits.is_empty() {
        return Err(ConvertError::Invalid);
    }
    let magnitude = i64::from_str_radix(digits, radix).map_err(|_| ConvertError::Invalid)?;
    i32::try_from(sign * magnitude).map_err(|_| ConvertError::Invalid)
}

fn comma(input: &str) -> NativeValues {
    if input.is_empty() {
        return empty();
    }
    NativeValues(Arc::from(
        input
            .split(',')
            .map(|item| NativeValue::Text(item.into()))
            .collect::<Vec<_>>(),
    ))
}

fn set(input: &str) -> NativeValues {
    if input.is_empty() {
        return empty();
    }
    let mut values = input.split(',').map(CompactString::new).collect::<Vec<_>>();
    values.sort_unstable_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    values.dedup();
    NativeValues(Arc::from(
        values
            .into_iter()
            .map(NativeValue::Text)
            .collect::<Vec<_>>(),
    ))
}

fn entry(input: &str) -> Result<NativeValue, ConvertError> {
    let (key, value) = input.split_once('=').ok_or(ConvertError::Invalid)?;
    if key.is_empty() {
        Err(ConvertError::Invalid)
    } else {
        Ok(NativeValue::Entry(key.into(), value.into()))
    }
}

fn env(input: &str) -> Result<NativeValue, ConvertError> {
    if input.is_empty() || input == "=" {
        return Err(ConvertError::Invalid);
    }
    if let Some(name) = input.strip_prefix('=') {
        return Ok(NativeValue::Env(EnvValue::Unset(name.into())));
    }
    if let Some((name, value)) = input.split_once('=') {
        Ok(NativeValue::Env(EnvValue::Set(name.into(), value.into())))
    } else {
        Ok(NativeValue::Env(EnvValue::Inherit(input.into())))
    }
}

fn dotted(input: &str) -> Result<NativeValue, ConvertError> {
    if input.is_empty() {
        return Err(ConvertError::Invalid);
    }
    let mut numeric_components = 0;
    for component in input.split('.') {
        if is_descriptive(component) {
            break;
        }
        validate_numeric_component(component)?;
        numeric_components += 1;
    }
    if numeric_components == 0 {
        return Err(ConvertError::Invalid);
    }
    Ok(NativeValue::Dotted(input.into()))
}

fn is_descriptive(component: &str) -> bool {
    component
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && component
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn validate_numeric_component(component: &str) -> Result<(), ConvertError> {
    let leading = component.bytes().take_while(u8::is_ascii_digit).count();
    if leading == 0 {
        return Err(ConvertError::Invalid);
    }
    component[..leading]
        .parse::<i32>()
        .map_err(|_| ConvertError::Invalid)?;
    let remainder = &component[leading..];
    if !remainder.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(ConvertError::Invalid);
    }
    if remainder.bytes().any(|byte| byte.is_ascii_alphabetic()) {
        let trailing = remainder
            .bytes()
            .rev()
            .take_while(u8::is_ascii_digit)
            .count();
        if trailing > 0 {
            remainder[remainder.len() - trailing..]
                .parse::<i32>()
                .map_err(|_| ConvertError::Invalid)?;
        }
    }
    Ok(())
}

fn timeout(input: &str) -> Result<NativeValue, ConvertError> {
    let mut values = Vec::new();
    for token in split_limit(input, ',', 6) {
        if !token.is_empty() || values.len() > 1 {
            values.push(token.parse::<i32>().map_err(|_| ConvertError::Invalid)?);
        }
    }
    let seconds = match values.as_slice() {
        [one] => [*one; 4],
        [short, moderate, long, eternal] => [*short, *moderate, *long, *eternal],
        _ => return Err(ConvertError::Invalid),
    };
    let defaults = [60_i64, 300, 900, 3600];
    let names = ["short", "moderate", "long", "eternal"];
    let pairs = names
        .into_iter()
        .zip(seconds)
        .enumerate()
        .map(|(index, (name, seconds))| {
            let seconds = if seconds <= 0 {
                defaults[index]
            } else {
                i64::from(seconds)
            };
            (
                NativeValue::Text(name.into()),
                NativeValue::Duration(Duration { seconds, nanos: 0 }),
            )
        })
        .collect::<Vec<_>>();
    Ok(NativeValue::OrderedMap(NativePairs(Arc::from(pairs))))
}

fn split_limit(input: &str, delimiter: char, limit: usize) -> Vec<&str> {
    if limit == 0 {
        return Vec::new();
    }
    let mut values = Vec::with_capacity(limit);
    let mut remainder = input;
    while values.len() + 1 < limit {
        let Some(index) = remainder.find(delimiter) else {
            break;
        };
        values.push(&remainder[..index]);
        remainder = &remainder[index + delimiter.len_utf8()..];
    }
    values.push(remainder);
    values
}

fn shard(input: &str) -> Result<NativeValue, ConvertError> {
    let lowered = ascii_lower(input);
    match lowered.as_str() {
        "explicit" => Ok(NativeValue::Shard(ShardValue::Explicit)),
        "disabled" => Ok(NativeValue::Shard(ShardValue::Disabled)),
        other => {
            let count = integer(other.strip_prefix("forced=").ok_or(ConvertError::Invalid)?)?;
            if count < 0 {
                Err(ConvertError::Invalid)
            } else {
                Ok(NativeValue::Shard(ShardValue::Forced(count)))
            }
        }
    }
}

fn fission(input: &str) -> Result<NativeValues, ConvertError> {
    if input == "no" {
        return Ok(empty());
    }
    let inputs = if input == "yes" {
        vec!["fastbuild", "dbg", "opt"]
    } else {
        input.split(',').collect()
    };
    let mut values = Vec::new();
    for input in inputs {
        let value = enum_value(EnumFamily::Compilation, input)?;
        if !values.contains(&value) {
            values.push(value);
        }
    }
    Ok(NativeValues(Arc::from(values)))
}

fn enum_value(family: EnumFamily, input: &str) -> Result<NativeValue, ConvertError> {
    let lowered = ascii_lower(input);
    let (members, lowercase_renderer): (&[&str], bool) = match family {
        EnumFamily::StrictDeps => (&["off", "warn", "error", "strict", "default"], false),
        EnumFamily::Exec => (&["legacy", "off", "full_hash", "diff_to_affected"], false),
        EnumFamily::OutputName => (
            &[
                "legacy",
                "diff_against_baseline",
                "diff_against_dynamic_baseline",
            ],
            false,
        ),
        EnumFamily::OutputPaths => (&["off", "strip"], false),
        EnumFamily::Include => (&["off", "direct", "transitive"], false),
        EnumFamily::Android => (&["main", "android"], false),
        EnumFamily::Apk => (&["v1", "v2", "v1_v2", "v4"], false),
        EnumFamily::Merger => (&["legacy", "android", "force_android"], false),
        EnumFamily::MergerOrder => (
            &[
                "alphabetical",
                "alphabetical_by_configuration",
                "dependency",
            ],
            false,
        ),
        EnumFamily::Apple => (
            &[
                "unknown",
                "applebin_ios",
                "applebin_visionos",
                "applebin_watchos",
                "applebin_tvos",
                "applebin_macos",
                "applebin_catalyst",
                "apple_crosstool",
            ],
            false,
        ),
        EnumFamily::Dynamic => (&["off", "default", "fully"], false),
        EnumFamily::Classpath => (&["off", "javabuilder", "bazel", "bazel_no_fallback"], false),
        EnumFamily::OneVersion => (&["off", "warning", "error"], false),
        EnumFamily::Cancel => (&["never", "on_failed", "on_passed"], false),
        EnumFamily::Compilation => (&["fastbuild", "dbg", "opt"], true),
        EnumFamily::Strip => (&["always", "sometimes", "never"], true),
    };
    let member = match (family, lowered.as_str()) {
        (EnumFamily::Cancel, "true") => "on_passed",
        (EnumFamily::Cancel, "false") => "never",
        (_, member) if members.contains(&member) => member,
        _ => return Err(ConvertError::Invalid),
    };
    Ok(NativeValue::Enum(EnumValue {
        family,
        member: if lowercase_renderer {
            member.into()
        } else {
            member.to_ascii_uppercase().into()
        },
    }))
}

pub(super) fn convert_duration(input: &str) -> Result<NativeValue, ConvertError> {
    if input == "0" {
        return Ok(NativeValue::Duration(Duration {
            seconds: 0,
            nanos: 0,
        }));
    }
    let (digits, unit) = if let Some(digits) = input.strip_suffix("ms") {
        (digits, "ms")
    } else if let Some(digits) = input.strip_suffix("ns") {
        (digits, "ns")
    } else if let Some(digits) = input.strip_suffix('d') {
        (digits, "d")
    } else if let Some(digits) = input.strip_suffix('h') {
        (digits, "h")
    } else if let Some(digits) = input.strip_suffix('m') {
        (digits, "m")
    } else if let Some(digits) = input.strip_suffix('s') {
        (digits, "s")
    } else {
        return Err(ConvertError::Invalid);
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ConvertError::Invalid);
    }
    let value = digits.parse::<i64>().map_err(|_| ConvertError::Invalid)?;
    let (seconds, nanos) = match unit {
        "d" => (value.checked_mul(86_400).ok_or(ConvertError::Invalid)?, 0),
        "h" => (value.checked_mul(3_600).ok_or(ConvertError::Invalid)?, 0),
        "m" => (value.checked_mul(60).ok_or(ConvertError::Invalid)?, 0),
        "s" => (value, 0),
        "ms" => (value / 1_000, ((value % 1_000) * 1_000_000) as u32),
        "ns" => (value / 1_000_000_000, (value % 1_000_000_000) as u32),
        _ => unreachable!(),
    };
    Ok(NativeValue::Duration(Duration { seconds, nanos }))
}

fn empty() -> NativeValues {
    NativeValues(Arc::from([]))
}
