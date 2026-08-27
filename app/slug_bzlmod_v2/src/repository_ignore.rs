/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the later Host repository-ignore packet.

use std::fmt;
use std::path::Component;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryListing;
use slug_workspace_v2::PathNodeKind;
#[cfg(windows)]
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
#[cfg(windows)]
use slug_workspace_v2::PathObservationEpochError;
#[cfg(windows)]
use slug_workspace_v2::PathObservationKey;
#[cfg(windows)]
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOutcome;

use crate::NonrootModuleKey;
use crate::RootPackagePolicyProjectionError;
use crate::RootRepositoryIgnoreInputsProjectionKey;
use crate::RootRepositoryRoute;
use crate::host_file::HostFileBytes;
use crate::host_file::HostFileBytesKey;
use crate::host_file::HostFileBytesObservationKey;
use crate::host_file::HostFileError;
use crate::repo_file::HostNonregistryRepoFileKey;
use crate::repo_file::HostNonregistryRepoFileObservationKey;
use crate::repo_file::HostRepoFileError;
use crate::repo_file::HostRepoFileKey;
use crate::repo_file::HostRepoFileObservationKey;
use crate::repo_file::HostRepoFileValue;
use crate::repo_file::HostRouteRepoFileError;
use crate::repo_file::HostRouteRepoFileKey;
use crate::repo_file::HostRouteRepoFileObservationKey;
use crate::source_preparation::HostCanonicalRepositorySourceInput;
use crate::source_preparation::HostRepositoryDirectoryListingError;
use crate::source_preparation::HostRepositoryDirectoryListingKey;
use crate::source_preparation::HostRepositoryDirectoryListingObservationKey;
use crate::source_preparation::HostRepositorySourceFileKey;
use crate::source_preparation::HostRepositorySourceFileObservationKey;
use crate::source_preparation::HostRepositorySourceFileValue;
use crate::source_preparation::HostRepositorySourceObservation;
use crate::source_preparation::HostRepositorySourceObservationError;
use crate::source_preparation::HostRepositorySourceRoute;
use crate::source_preparation::RepositorySourceFileError;
use crate::source_preparation::RepositorySourceFileKey;
use crate::source_preparation::RepositorySourceFileObservationKey;
use crate::source_preparation::RepositorySourceFileValue;
use crate::source_preparation::SourcePreparationNeeds;
use crate::source_preparation::SourcePreparationOutcome;
/// A normalized slash-separated `.bazelignore` prefix.
///
/// Unlike `PackagePath`, Bazel's prefix domain includes the empty path and
/// surviving leading `..` components.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Allocative)]
pub(crate) struct RepositoryIgnorePrefix {
    normalized: CompactString,
}

impl RepositoryIgnorePrefix {
    pub(crate) fn new_normalized(normalized: impl Into<CompactString>) -> Self {
        Self {
            normalized: normalized.into(),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        self.normalized.as_str()
    }

    fn can_match_package(&self) -> bool {
        !matches!(self.normalized.split('/').next(), Some(".."))
    }
}

/// Compact repository-relative ignore entries.
///
/// Literal prefixes precede patterns during matching. Pattern spellings are
/// retained in request order while their segment matchers are precompiled.
#[derive(Debug, Clone, Allocative, Dupe)]
pub(crate) struct RepositoryIgnoreMatcher {
    literal_prefixes: Arc<[RepositoryIgnorePrefix]>,
    patterns: Arc<[CompiledPattern]>,
}

impl RepositoryIgnoreMatcher {
    pub(crate) fn new(
        literal_prefixes: impl IntoIterator<Item = RepositoryIgnorePrefix>,
        patterns: impl IntoIterator<Item = CompactString>,
    ) -> Self {
        let mut literal_prefixes = literal_prefixes.into_iter().collect::<Vec<_>>();
        literal_prefixes.sort();
        literal_prefixes.dedup();
        let patterns = patterns
            .into_iter()
            .map(CompiledPattern::new)
            .collect::<Vec<_>>();
        Self {
            literal_prefixes: literal_prefixes.into(),
            patterns: patterns.into(),
        }
    }

    pub(crate) fn matching_entry<'a>(&'a self, directory: &PackagePath) -> Option<&'a str> {
        for prefix in self.literal_prefixes.iter() {
            if prefix.can_match_package()
                && is_component_prefix(prefix.as_str(), directory.as_str())
            {
                return Some(prefix.as_str());
            }
        }

        let path_segments = if directory.as_str().is_empty() {
            Vec::new()
        } else {
            directory.as_str().split('/').collect::<Vec<_>>()
        };
        self.patterns
            .iter()
            .find(|pattern| pattern.matches_prefix(&path_segments))
            .map(|pattern| pattern.original.as_str())
    }
}

impl PartialEq for RepositoryIgnoreMatcher {
    fn eq(&self, other: &Self) -> bool {
        self.literal_prefixes == other.literal_prefixes
            && self.patterns.len() == other.patterns.len()
            && self
                .patterns
                .iter()
                .zip(other.patterns.iter())
                .all(|(left, right)| left.original == right.original)
    }
}

impl Eq for RepositoryIgnoreMatcher {}

fn is_component_prefix(prefix: &str, directory: &str) -> bool {
    if prefix.is_empty() || prefix == directory {
        return true;
    }
    directory
        .strip_prefix(prefix)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostRepositoryIgnoreError {
    RepoFile(HostRepoFileError),
    RouteRepoFile(HostRouteRepoFileError),
    NonregistryRepoFile(HostRouteRepoFileError),
    PolicyProjection(RootPackagePolicyProjectionError),
    RepositoryListing(HostRepositoryDirectoryListingError),
    BuiltinMetadata {
        actual: PathDirectoryEntryKind,
    },
    HostFile(HostFileError),
    RepositorySource(RepositorySourceFileError),
    RepositorySourceObservation(HostRepositorySourceObservationError),
    InvalidAbsolute {
        logical_path: NormalizedAbsolutePath,
        normalized: Arc<[u16]>,
    },
    NativeInvalid {
        logical_path: NormalizedAbsolutePath,
        message: CompactString,
    },
}

impl fmt::Display for HostRepositoryIgnoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepoFile(error) => error.fmt(f),
            Self::RouteRepoFile(error) => error.fmt(f),
            Self::NonregistryRepoFile(error) => error.fmt(f),
            Self::PolicyProjection(error) => error.fmt(f),
            Self::RepositoryListing(error) => error.fmt(f),
            Self::BuiltinMetadata { actual } => write!(
                f,
                "built-in .bazelignore metadata has unsupported entry kind {actual:?}"
            ),
            Self::HostFile(error) => write!(f, "failed to read .bazelignore: {error:?}"),
            Self::RepositorySource(error) => {
                write!(f, "failed to read routed .bazelignore: {error:?}")
            }
            Self::RepositorySourceObservation(error) => {
                write!(f, "failed to read canonical .bazelignore: {error:?}")
            }
            Self::InvalidAbsolute {
                logical_path,
                normalized,
            } => write!(
                f,
                "Invalid path in {}: '{}': cannot be an absolute path",
                logical_path.as_path().display(),
                String::from_utf16_lossy(normalized)
            ),
            Self::NativeInvalid {
                logical_path,
                message,
            } => write!(
                f,
                "Invalid path in {}: {message}",
                logical_path.as_path().display()
            ),
        }
    }
}

impl std::error::Error for HostRepositoryIgnoreError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IgnorePathFlavor {
    Unix,
    Windows,
}

impl IgnorePathFlavor {
    fn native() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedIgnoreLine {
    original: Arc<[u16]>,
    normalized: Arc<[u16]>,
    absolute: bool,
    request_windows_long_path: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedIgnoreFile {
    lines: Vec<PreparedIgnoreLine>,
}

fn java_read_lines(value: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\n' || bytes[index] == b'\r' {
            lines.push(&value[start..index]);
            if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                index += 1;
            }
            index += 1;
            start = index;
        } else {
            index += 1;
        }
    }
    if start < bytes.len() {
        lines.push(&value[start..]);
    }
    lines
}

fn java_utf8_decode(bytes: &[u8]) -> String {
    let mut decoded = String::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let first = bytes[index];
        if first < 0x80 {
            decoded.push(char::from(first));
            index += 1;
            continue;
        }

        let (width, minimum, mask) = match first {
            0xC2..=0xDF => (2, 0x80, 0x1F),
            0xE0..=0xEF => (3, 0x800, 0x0F),
            0xF0..=0xF4 => (4, 0x10000, 0x07),
            _ => {
                decoded.push('\u{FFFD}');
                index += 1;
                continue;
            }
        };
        if let Some(second) = bytes.get(index + 1)
            && ((first == 0xE0 && *second < 0xA0)
                || (first == 0xF0 && *second < 0x90)
                || (first == 0xF4 && *second > 0x8F))
        {
            decoded.push('\u{FFFD}');
            index += 1;
            continue;
        }

        let mut consumed = 1;
        while consumed < width
            && bytes
                .get(index + consumed)
                .is_some_and(|byte| byte & 0xC0 == 0x80)
        {
            consumed += 1;
        }
        if consumed != width {
            decoded.push('\u{FFFD}');
            index += consumed;
            continue;
        }

        let mut scalar = u32::from(first & mask);
        for byte in &bytes[index + 1..index + width] {
            scalar = (scalar << 6) | u32::from(byte & 0x3F);
        }
        if scalar < minimum || (0xD800..=0xDFFF).contains(&scalar) || scalar > 0x10FFFF {
            decoded.push('\u{FFFD}');
        } else {
            decoded.push(char::from_u32(scalar).expect("validated Unicode scalar"));
        }
        index += width;
    }
    decoded
}

fn is_windows_separator(unit: u16) -> bool {
    unit == b'/' as u16 || unit == b'\\' as u16
}

fn is_ascii_drive_letter(unit: u16) -> bool {
    (b'A' as u16..=b'Z' as u16).contains(&unit) || (b'a' as u16..=b'z' as u16).contains(&unit)
}

fn normalize_slash_path(input: &[u16], flavor: IgnorePathFlavor) -> Arc<[u16]> {
    let separator =
        |unit| unit == b'/' as u16 || (flavor == IgnorePathFlavor::Windows && unit == b'\\' as u16);
    let drive_absolute = flavor == IgnorePathFlavor::Windows
        && input.len() >= 3
        && is_ascii_drive_letter(input[0])
        && input[1] == b':' as u16
        && separator(input[2]);
    let root_absolute = input.first().is_some_and(|unit| separator(*unit));
    let absolute = drive_absolute || root_absolute;
    let mut start = if drive_absolute { 3 } else { 0 };
    while start < input.len() && separator(input[start]) {
        start += 1;
    }

    let mut segments: Vec<&[u16]> = Vec::new();
    let mut index = start;
    while index < input.len() {
        while index < input.len() && separator(input[index]) {
            index += 1;
        }
        let begin = index;
        while index < input.len() && !separator(input[index]) {
            index += 1;
        }
        if begin == index {
            continue;
        }
        let segment = &input[begin..index];
        if segment == [b'.' as u16] {
            continue;
        }
        if segment == [b'.' as u16, b'.' as u16] {
            if segments
                .last()
                .is_some_and(|prior| *prior != [b'.' as u16, b'.' as u16])
            {
                segments.pop();
                continue;
            }
            if absolute {
                continue;
            }
        }
        segments.push(segment);
    }

    let mut normalized = Vec::with_capacity(input.len());
    if drive_absolute {
        normalized.push(if (b'a' as u16..=b'z' as u16).contains(&input[0]) {
            input[0] - (b'a' - b'A') as u16
        } else {
            input[0]
        });
        normalized.extend([b':' as u16, b'/' as u16]);
    } else if root_absolute {
        normalized.push(b'/' as u16);
    }
    for (segment_index, segment) in segments.into_iter().enumerate() {
        if segment_index != 0 {
            normalized.push(b'/' as u16);
        }
        normalized.extend_from_slice(segment);
    }
    normalized.into()
}

fn is_normalized_absolute(path: &[u16], flavor: IgnorePathFlavor) -> bool {
    path.first() == Some(&(b'/' as u16))
        || (flavor == IgnorePathFlavor::Windows
            && path.len() >= 3
            && is_ascii_drive_letter(path[0])
            && path[1] == b':' as u16
            && path[2] == b'/' as u16)
}

fn windows_invalid_path(input: &[u16], reason: &str, index: Option<usize>) -> CompactString {
    let input = String::from_utf16_lossy(input);
    CompactString::new(match index {
        Some(index) => format!("{reason} at index {index}: {input}"),
        None => format!("{reason}: {input}"),
    })
}

fn windows_illegal(unit: u16) -> bool {
    unit < 0x20
        || [b'<', b'>', b':', b'"', b'|', b'?', b'*']
            .into_iter()
            .any(|candidate| unit == u16::from(candidate))
}

fn validate_windows_tail(input: &[u16], start: usize) -> Result<(), CompactString> {
    let mut prior = None;
    for (index, unit) in input.iter().copied().enumerate().skip(start) {
        if is_windows_separator(unit) {
            if prior == Some(b' ' as u16) {
                return Err(windows_invalid_path(
                    input,
                    "Trailing char < >",
                    Some(index - 1),
                ));
            }
        } else if windows_illegal(unit) {
            return Err(windows_invalid_path(
                input,
                &format!("Illegal char <{}>", String::from_utf16_lossy(&[unit])),
                Some(index),
            ));
        }
        prior = Some(unit);
    }
    if prior == Some(b' ' as u16) {
        return Err(windows_invalid_path(
            input,
            "Trailing char < >",
            Some(input.len() - 1),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowsNativePathType {
    Relative,
    DirectoryRelative,
    DriveRelative,
    Absolute,
    Unc,
}

fn validate_windows_parsed_path(input: &[u16]) -> Result<WindowsNativePathType, CompactString> {
    if input.len() >= 2 && is_windows_separator(input[0]) {
        if is_windows_separator(input[1]) {
            let mut index = 2;
            while input
                .get(index)
                .is_some_and(|unit| is_windows_separator(*unit))
            {
                index += 1;
            }
            let hostname_start = index;
            while let Some(unit) = input.get(index).copied() {
                if is_windows_separator(unit) {
                    break;
                }
                if windows_illegal(unit) {
                    return Err(windows_invalid_path(
                        input,
                        &format!(
                            "Illegal character [{}] in path",
                            String::from_utf16_lossy(&[unit])
                        ),
                        Some(index),
                    ));
                }
                index += 1;
            }
            if index == hostname_start {
                return Err(windows_invalid_path(
                    input,
                    "UNC path is missing hostname",
                    None,
                ));
            }
            while input
                .get(index)
                .is_some_and(|unit| is_windows_separator(*unit))
            {
                index += 1;
            }
            let share_start = index;
            while let Some(unit) = input.get(index).copied() {
                if is_windows_separator(unit) {
                    break;
                }
                if windows_illegal(unit) {
                    return Err(windows_invalid_path(
                        input,
                        &format!(
                            "Illegal character [{}] in path",
                            String::from_utf16_lossy(&[unit])
                        ),
                        Some(index),
                    ));
                }
                index += 1;
            }
            if index == share_start {
                return Err(windows_invalid_path(
                    input,
                    "UNC path is missing sharename",
                    None,
                ));
            }
            validate_windows_tail(input, index)?;
            return Ok(WindowsNativePathType::Unc);
        }
        validate_windows_tail(input, 1)?;
        return Ok(WindowsNativePathType::DirectoryRelative);
    }

    if input.len() >= 2 && is_ascii_drive_letter(input[0]) && input[1] == b':' as u16 {
        if input.get(2).is_some_and(|unit| is_windows_separator(*unit)) {
            validate_windows_tail(input, 3)?;
            return Ok(WindowsNativePathType::Absolute);
        }
        validate_windows_tail(input, 2)?;
        return Ok(WindowsNativePathType::DriveRelative);
    }

    validate_windows_tail(input, 0)?;
    Ok(WindowsNativePathType::Relative)
}

fn validate_windows_native_path(line: &str) -> Result<(), CompactString> {
    let original = line.encode_utf16().collect::<Vec<_>>();
    let long_prefix = [b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
    if !original.starts_with(&long_prefix) {
        validate_windows_parsed_path(&original)?;
        return Ok(());
    }

    let unc_prefix = [b'U' as u16, b'N' as u16, b'C' as u16, b'\\' as u16];
    if original
        .get(long_prefix.len()..)
        .is_some_and(|tail| tail.starts_with(&unc_prefix))
    {
        let mut transformed = vec![b'\\' as u16, b'\\' as u16];
        transformed.extend_from_slice(&original[long_prefix.len() + unc_prefix.len()..]);
        let path_type = validate_windows_parsed_path(&transformed)?;
        if path_type != WindowsNativePathType::Unc {
            return Err(windows_invalid_path(
                &transformed,
                "Long UNC path prefix can only be used with a UNC path",
                None,
            ));
        }
        return Ok(());
    }

    let transformed = &original[long_prefix.len()..];
    let path_type = validate_windows_parsed_path(transformed)?;
    if path_type != WindowsNativePathType::Absolute {
        return Err(windows_invalid_path(
            transformed,
            "Long path prefix can only be used with an absolute path",
            None,
        ));
    }
    Ok(())
}

fn validate_native_path(line: &str, flavor: IgnorePathFlavor) -> Result<(), CompactString> {
    if flavor == IgnorePathFlavor::Unix && line.contains('\0') {
        return Err(CompactString::new(format!(
            "Nul character not allowed: {line}"
        )));
    }
    if flavor == IgnorePathFlavor::Windows {
        validate_windows_native_path(line)?;
    }
    Ok(())
}

fn java_regex_dot_count(units: &[u16]) -> Option<usize> {
    let mut count = 0;
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        if matches!(unit, 0x000A | 0x000D | 0x0085 | 0x2028 | 0x2029) {
            return None;
        }
        index += if (0xD800..=0xDBFF).contains(&unit)
            && units
                .get(index + 1)
                .is_some_and(|low| (0xDC00..=0xDFFF).contains(low))
        {
            2
        } else {
            1
        };
        count += 1;
    }
    Some(count)
}

fn is_windows_short_path_segment(segment: &[u16]) -> bool {
    if segment.len() > 12 {
        return false;
    }
    for tilde in 1..=6.min(segment.len().saturating_sub(1)) {
        if segment[tilde] != b'~' as u16
            || !matches!(java_regex_dot_count(&segment[..tilde]), Some(1..=6))
        {
            continue;
        }
        let remainder = &segment[tilde + 1..];
        let digits = remainder
            .iter()
            .take_while(|unit| (b'0' as u16..=b'9' as u16).contains(unit))
            .count();
        if digits == 0 || digits > 6 || tilde + digits >= 8 {
            continue;
        }
        let suffix = &remainder[digits..];
        if suffix.is_empty()
            || (suffix[0] == b'.' as u16
                && matches!(java_regex_dot_count(&suffix[1..]), Some(0..=3)))
        {
            return true;
        }
    }
    false
}

fn contains_windows_short_path(path: &[u16]) -> bool {
    path.split(|unit| is_windows_separator(*unit))
        .any(is_windows_short_path_segment)
}

fn has_windows_drive_specifier(path: &[u16]) -> bool {
    path.len() >= 3
        && is_ascii_drive_letter(path[0])
        && path[1] == b':' as u16
        && is_windows_separator(path[2])
}

fn windows_verbatim_drive_view(input: &[u16]) -> &[u16] {
    let prefix = [b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
    input.strip_prefix(&prefix).unwrap_or(input)
}

fn windows_native_eligible(input: &[u16]) -> bool {
    let drive_view = windows_verbatim_drive_view(input);
    if !has_windows_drive_specifier(drive_view) {
        return false;
    }
    let current = [b'.' as u16, b'\\' as u16];
    let parent = [b'.' as u16, b'.' as u16, b'\\' as u16];
    let inner_current = [b'\\' as u16, b'.' as u16, b'\\' as u16];
    let inner_parent = [b'\\' as u16, b'.' as u16, b'.' as u16, b'\\' as u16];
    let trailing_current = [b'\\' as u16, b'.' as u16];
    let trailing_parent = [b'\\' as u16, b'.' as u16, b'.' as u16];
    let normalized = drive_view
        .iter()
        .map(|unit| {
            if *unit == b'/' as u16 {
                b'\\' as u16
            } else {
                *unit
            }
        })
        .collect::<Vec<_>>();
    !normalized.starts_with(&current)
        && !normalized
            .windows(inner_current.len())
            .any(|candidate| candidate == inner_current)
        && !normalized.ends_with(&trailing_current)
        && !normalized.starts_with(&parent)
        && !normalized
            .windows(inner_parent.len())
            .any(|candidate| candidate == inner_parent)
        && !normalized.ends_with(&trailing_parent)
}

fn prepare_ignore_file(
    bytes: &[u8],
    flavor: IgnorePathFlavor,
    logical_path: &NormalizedAbsolutePath,
) -> Result<PreparedIgnoreFile, HostRepositoryIgnoreError> {
    let decoded = java_utf8_decode(bytes);
    let mut lines = Vec::new();
    for line in java_read_lines(&decoded) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        validate_native_path(line, flavor).map_err(|message| {
            HostRepositoryIgnoreError::NativeInvalid {
                logical_path: logical_path.dupe(),
                message,
            }
        })?;
        let original: Arc<[u16]> = line.encode_utf16().collect::<Vec<_>>().into();
        let normalized = normalize_slash_path(&original, flavor);
        lines.push(PreparedIgnoreLine {
            absolute: is_normalized_absolute(&normalized, flavor),
            request_windows_long_path: flavor == IgnorePathFlavor::Windows
                && is_normalized_absolute(&normalized, flavor)
                && contains_windows_short_path(&original)
                && windows_native_eligible(&original),
            original,
            normalized,
        });
    }
    Ok(PreparedIgnoreFile { lines })
}

fn prefix_from_units(units: &[u16]) -> RepositoryIgnorePrefix {
    RepositoryIgnorePrefix::new_normalized(String::from_utf16_lossy(units))
}

fn relative_vendor_prefix(
    root: &NormalizedAbsolutePath,
    vendor: &NormalizedAbsolutePath,
) -> Option<RepositoryIgnorePrefix> {
    let relative = vendor.as_path().strip_prefix(root.as_path()).ok()?;
    let mut components = Vec::new();
    for component in relative.components() {
        if let Component::Normal(component) = component {
            components.push(component.to_string_lossy());
        }
    }
    Some(RepositoryIgnorePrefix::new_normalized(components.join("/")))
}

#[cfg(windows)]
fn windows_long_path_identity(input: &[u16]) -> NormalizedAbsolutePath {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let normalized = normalize_slash_path(
        windows_verbatim_drive_view(input),
        IgnorePathFlavor::Windows,
    );
    NormalizedAbsolutePath::new(OsString::from_wide(&normalized))
        .expect("eligible Windows .bazelignore path is absolute")
}

struct ObservedIgnoreParse {
    result: Result<Vec<RepositoryIgnorePrefix>, HostRepositoryIgnoreError>,
    observations: PathObservationEpoch,
}

fn union_observations(
    left: &PathObservationEpoch,
    right: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        left.observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain(
                right
                    .observations()
                    .iter()
                    .map(|(demand, result)| (demand.dupe(), result.dupe())),
            ),
    )
    .map_err(ObservedPathFrontierError::from)
}

#[cfg(windows)]
fn append_observation(
    observations: &PathObservationEpoch,
    demand: PathObservationDemand,
    result: Arc<PathObservationResult>,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        observations
            .observations()
            .iter()
            .map(|(known, result)| (known.dupe(), result.dupe()))
            .chain(std::iter::once((demand, result))),
    )
    .map_err(ObservedPathFrontierError::from)
}

async fn parse_ignore_file_observed(
    _ctx: &mut DiceComputations<'_>,
    logical_path: &NormalizedAbsolutePath,
    bytes: &[u8],
) -> PathOutcome<Result<ObservedIgnoreParse, ObservedPathFrontierError>> {
    let flavor = IgnorePathFlavor::native();
    let observations = PathObservationEpoch::empty();
    #[cfg(windows)]
    let mut observations = observations;
    let prepared = match prepare_ignore_file(bytes, flavor, logical_path) {
        Ok(prepared) => prepared,
        Err(error) => {
            return PathOutcome::Complete(Ok(ObservedIgnoreParse {
                result: Err(error),
                observations,
            }));
        }
    };
    let mut prefixes = Vec::new();
    let mut first_absolute: Option<Arc<[u16]>> = None;
    for line in prepared.lines {
        let normalized = line.normalized;
        #[cfg(windows)]
        let normalized = if line.request_windows_long_path {
            let demand = PathObservationDemand::windows_long_path(
                windows_long_path_identity(&line.original),
                line.original.dupe(),
            );
            match dice_invariant(_ctx.compute(&PathObservationKey::new(demand.dupe())).await) {
                PathOutcome::Need(need) => return PathOutcome::Need(need),
                PathOutcome::Complete(result) => match result.as_ref() {
                    PathObservationResult::WindowsLongPath(value) => {
                        observations =
                            match append_observation(&observations, demand, result.dupe()) {
                                Ok(observations) => observations,
                                Err(error) => return PathOutcome::Complete(Err(error)),
                            };
                        value.dupe()
                    }
                    PathObservationResult::Lstat(_)
                    | PathObservationResult::ReadLink(_)
                    | PathObservationResult::FileBytes(_)
                    | PathObservationResult::DirectoryEntries(_)
                    | PathObservationResult::WindowsOptionPathLongName(_) => {
                        return PathOutcome::Complete(Err(ObservedPathFrontierError::from(
                            PathObservationEpochError::OperationMismatch {
                                demand,
                                result_operation: result.operation(),
                            },
                        )));
                    }
                },
            }
        } else {
            normalized
        };
        if is_normalized_absolute(&normalized, flavor) {
            if first_absolute.is_none() {
                first_absolute = Some(normalized.dupe());
            }
        } else {
            prefixes.push(prefix_from_units(&normalized));
        }
    }
    if let Some(normalized) = first_absolute {
        return PathOutcome::Complete(Ok(ObservedIgnoreParse {
            result: Err(HostRepositoryIgnoreError::InvalidAbsolute {
                logical_path: logical_path.dupe(),
                normalized,
            }),
            observations,
        }));
    }
    PathOutcome::Complete(Ok(ObservedIgnoreParse {
        result: Ok(prefixes),
        observations,
    }))
}

async fn parse_ignore_file(
    ctx: &mut DiceComputations<'_>,
    logical_path: &NormalizedAbsolutePath,
    bytes: &[u8],
) -> PathOutcome<Result<Vec<RepositoryIgnorePrefix>, HostRepositoryIgnoreError>> {
    match parse_ignore_file_observed(ctx, logical_path, bytes).await {
        PathOutcome::Need(need) => PathOutcome::Need(need),
        PathOutcome::Complete(Ok(parsed)) => PathOutcome::Complete(parsed.result),
        PathOutcome::Complete(Err(error)) => {
            panic!("legacy repository-ignore parser received frontier error: {error}")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostRepositoryIgnoreKey {
    workspace: NormalizedAbsolutePath,
}

impl HostRepositoryIgnoreKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostRepositoryIgnoreKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-repository-ignore:{}", self.workspace)
    }
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host repository-ignore DICE invariant failed: {error:?}"))
}

#[async_trait]
impl Key for HostRepositoryIgnoreKey {
    type Value = PathOutcome<Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let repo = match dice_invariant(
            ctx.compute(&HostRepoFileKey::new(self.workspace.dupe()))
                .await,
        ) {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(value) => match value.as_ref() {
                Ok(value) => value.dupe(),
                Err(error) => {
                    return PathOutcome::Complete(Arc::new(Err(
                        HostRepositoryIgnoreError::RepoFile(error.clone()),
                    )));
                }
            },
        };
        let inputs = match dice_invariant(
            ctx.compute(&RootRepositoryIgnoreInputsProjectionKey::new(
                self.workspace.dupe(),
            ))
            .await,
        ) {
            Ok(inputs) => inputs,
            Err(error) => {
                return PathOutcome::Complete(Arc::new(Err(
                    HostRepositoryIgnoreError::PolicyProjection(error),
                )));
            }
        };

        let mut prefixes = Vec::new();
        for root in inputs.package_roots() {
            if let Some(vendor) = inputs.vendor_directory() {
                if let Some(prefix) = relative_vendor_prefix(root, vendor) {
                    prefixes.push(prefix);
                }
            }
            let logical_path = NormalizedAbsolutePath::new(root.as_path().join(".bazelignore"))
                .expect("joining a normalized package root remains absolute");
            let bytes = match dice_invariant(
                ctx.compute(&HostFileBytesKey::new(logical_path.dupe()))
                    .await,
            ) {
                PathOutcome::Need(need) => return PathOutcome::Need(need),
                PathOutcome::Complete(Ok(HostFileBytes::Missing)) => continue,
                PathOutcome::Complete(Ok(HostFileBytes::Present(bytes))) => bytes,
                PathOutcome::Complete(Err(HostFileError::WrongKind {
                    actual: PathNodeKind::Directory,
                    ..
                })) => continue,
                PathOutcome::Complete(Err(error)) => {
                    return PathOutcome::Complete(Arc::new(Err(
                        HostRepositoryIgnoreError::HostFile(error),
                    )));
                }
            };
            match parse_ignore_file(ctx, &logical_path, &bytes).await {
                PathOutcome::Need(need) => return PathOutcome::Need(need),
                PathOutcome::Complete(Err(error)) => {
                    return PathOutcome::Complete(Arc::new(Err(error)));
                }
                PathOutcome::Complete(Ok(file_prefixes)) => {
                    prefixes.extend(file_prefixes);
                    break;
                }
            }
        }

        PathOutcome::Complete(Arc::new(Ok(RepositoryIgnoreMatcher::new(
            prefixes,
            repo.ignored_directories().iter().cloned(),
        ))))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostRepositoryIgnore {
    result: Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostRepositoryIgnore {
    pub(crate) fn result(&self) -> &Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostRepositoryIgnoreObservationKey {
    workspace: NormalizedAbsolutePath,
}

impl HostRepositoryIgnoreObservationKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostRepositoryIgnoreObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bzlmod-observed-host-repository-ignore:{}",
            self.workspace
        )
    }
}

fn complete_observed_ignore(
    result: Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>,
    observations: PathObservationEpoch,
) -> PathOutcome<Result<ObservedHostRepositoryIgnore, ObservedPathFrontierError>> {
    PathOutcome::Complete(Ok(ObservedHostRepositoryIgnore {
        result: Arc::new(result),
        observations,
    }))
}

#[async_trait]
impl Key for HostRepositoryIgnoreObservationKey {
    type Value = PathOutcome<Result<ObservedHostRepositoryIgnore, ObservedPathFrontierError>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let repo = match dice_invariant(
            ctx.compute(&HostRepoFileObservationKey::new(self.workspace.dupe()))
                .await,
        ) {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(Err(error)) => return PathOutcome::Complete(Err(error)),
            PathOutcome::Complete(Ok(repo)) => repo,
        };
        let mut observations = repo.observations().dupe();
        let repo = match repo.result() {
            Ok(repo) => repo.dupe(),
            Err(error) => {
                return complete_observed_ignore(
                    Err(HostRepositoryIgnoreError::RepoFile(error.clone())),
                    observations,
                );
            }
        };
        let inputs = match dice_invariant(
            ctx.compute(&RootRepositoryIgnoreInputsProjectionKey::new(
                self.workspace.dupe(),
            ))
            .await,
        ) {
            Ok(inputs) => inputs,
            Err(error) => {
                return complete_observed_ignore(
                    Err(HostRepositoryIgnoreError::PolicyProjection(error)),
                    observations,
                );
            }
        };

        let mut prefixes = Vec::new();
        for root in inputs.package_roots() {
            if let Some(vendor) = inputs.vendor_directory()
                && let Some(prefix) = relative_vendor_prefix(root, vendor)
            {
                prefixes.push(prefix);
            }
            let logical_path = NormalizedAbsolutePath::new(root.as_path().join(".bazelignore"))
                .expect("joining a normalized package root remains absolute");
            let file = match dice_invariant(
                ctx.compute(&HostFileBytesObservationKey::new(logical_path.dupe()))
                    .await,
            ) {
                PathOutcome::Need(need) => return PathOutcome::Need(need),
                PathOutcome::Complete(Err(error)) => return PathOutcome::Complete(Err(error)),
                PathOutcome::Complete(Ok(file)) => file,
            };
            observations = match union_observations(&observations, file.observations()) {
                Ok(observations) => observations,
                Err(error) => return PathOutcome::Complete(Err(error)),
            };
            let bytes = match file.result() {
                Ok(HostFileBytes::Missing) => continue,
                Ok(HostFileBytes::Present(bytes)) => bytes.dupe(),
                Err(HostFileError::WrongKind {
                    actual: PathNodeKind::Directory,
                    ..
                }) => continue,
                Err(error) => {
                    return complete_observed_ignore(
                        Err(HostRepositoryIgnoreError::HostFile(error.clone())),
                        observations,
                    );
                }
            };
            let parsed = match parse_ignore_file_observed(ctx, &logical_path, &bytes).await {
                PathOutcome::Need(need) => return PathOutcome::Need(need),
                PathOutcome::Complete(Err(error)) => return PathOutcome::Complete(Err(error)),
                PathOutcome::Complete(Ok(parsed)) => parsed,
            };
            observations = match union_observations(&observations, &parsed.observations) {
                Ok(observations) => observations,
                Err(error) => return PathOutcome::Complete(Err(error)),
            };
            match parsed.result {
                Err(error) => return complete_observed_ignore(Err(error), observations),
                Ok(file_prefixes) => {
                    prefixes.extend(file_prefixes);
                    break;
                }
            }
        }

        complete_observed_ignore(
            Ok(RepositoryIgnoreMatcher::new(
                prefixes,
                repo.ignored_directories().iter().cloned(),
            )),
            observations,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostNonregistryRepositoryIgnoreKey {
    workspace: NormalizedAbsolutePath,
    module: NonrootModuleKey,
}

impl HostNonregistryRepositoryIgnoreKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, module: NonrootModuleKey) -> Self {
        Self { workspace, module }
    }
}

impl fmt::Display for HostNonregistryRepositoryIgnoreKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-nonregistry-repository-ignore:{}@{}",
            self.module.name, self.module.version
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostNonregistryRepositoryIgnoreObservationKey(
    pub(crate) HostNonregistryRepositoryIgnoreKey,
);
impl fmt::Display for HostNonregistryRepositoryIgnoreObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostNonregistryRepositoryIgnore {
    result: Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>>,
    observations: PathObservationEpoch,
}
impl ObservedHostNonregistryRepositoryIgnore {
    pub(crate) fn result(
        &self,
    ) -> &Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}
#[derive(Clone, Copy)]
enum HostNonregistryRepositoryIgnoreMode {
    Legacy,
    Observed,
}
type HostNonregistryRepositoryIgnoreProjection = (
    Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>>,
    PathObservationEpoch,
);
type HostNonregistryRepositoryIgnoreDriverOutcome = SourcePreparationOutcome<
    Result<HostNonregistryRepositoryIgnoreProjection, ObservedPathFrontierError>,
>;
fn nonregistry_ignore_complete(
    result: Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>,
    observations: PathObservationEpoch,
) -> HostNonregistryRepositoryIgnoreDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}
fn finish_observed_nonregistry_ignore_complete<T>(
    result: Result<T, ObservedPathFrontierError>,
) -> Result<T, HostNonregistryRepositoryIgnoreDriverOutcome> {
    result.map_err(|error| SourcePreparationOutcome::Complete(Err(error)))
}
fn project_nonregistry_ignore_legacy(
    outcome: HostNonregistryRepositoryIgnoreDriverOutcome,
) -> <HostNonregistryRepositoryIgnoreKey as Key>::Value {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, _))) => {
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy nonregistry ignore cannot produce an observed outer error")
        }
    }
}

async fn drive_host_nonregistry_repository_ignore(
    ctx: &mut DiceComputations<'_>,
    key: &HostNonregistryRepositoryIgnoreKey,
    mode: HostNonregistryRepositoryIgnoreMode,
) -> HostNonregistryRepositoryIgnoreDriverOutcome {
    let repo_key = HostNonregistryRepoFileKey::new(key.workspace.dupe(), key.module.clone());
    let (repo, mut observations) = match mode {
        HostNonregistryRepositoryIgnoreMode::Legacy => {
            match dice_invariant(ctx.compute(&repo_key).await) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(repo) => (repo, PathObservationEpoch::empty()),
            }
        }
        HostNonregistryRepositoryIgnoreMode::Observed => {
            match dice_invariant(
                ctx.compute(&HostNonregistryRepoFileObservationKey(repo_key))
                    .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(result) => {
                    match finish_observed_nonregistry_ignore_complete(result) {
                        Ok(repo) => (repo.result().dupe(), repo.observations().dupe()),
                        Err(outcome) => return outcome,
                    }
                }
            }
        }
    };
    let repo = match repo.as_ref() {
        Ok(repo) => repo.dupe(),
        Err(error) => {
            return nonregistry_ignore_complete(
                Err(HostRepositoryIgnoreError::NonregistryRepoFile(
                    error.clone(),
                )),
                observations,
            );
        }
    };
    let source_key = RepositorySourceFileKey {
        workspace: key.workspace.as_path().to_owned(),
        module_name: key.module.name.clone(),
        repo_relative_path: ".bazelignore".into(),
    };
    let (source, source_observations) = match mode {
        HostNonregistryRepositoryIgnoreMode::Legacy => {
            match dice_invariant(ctx.compute(&source_key).await) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(source) => {
                    let source = source.as_ref().map(Dupe::dupe).map_err(Dupe::dupe);
                    (source, PathObservationEpoch::empty())
                }
            }
        }
        HostNonregistryRepositoryIgnoreMode::Observed => {
            match dice_invariant(
                ctx.compute(&RepositorySourceFileObservationKey(source_key))
                    .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(result) => {
                    match finish_observed_nonregistry_ignore_complete(result) {
                        Ok(source) => (
                            source.result().as_ref().clone(),
                            source.observations().dupe(),
                        ),
                        Err(outcome) => return outcome,
                    }
                }
            }
        }
    };
    observations = match union_observations(&observations, &source_observations) {
        Ok(observations) => observations,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    let source = match source {
        Ok(RepositorySourceFileValue::Absent) => None,
        Ok(RepositorySourceFileValue::Present(bytes)) => {
            let logical_path = NormalizedAbsolutePath::new(
                key.workspace
                    .as_path()
                    .join(".slug-nonregistry")
                    .join(key.module.name.as_str())
                    .join(".bazelignore"),
            )
            .expect("joining a normalized workspace remains absolute");
            Some((bytes, logical_path))
        }
        Err(RepositorySourceFileError::WrongKind {
            actual: PathNodeKind::Directory,
            ..
        }) => None,
        Err(error) => {
            return nonregistry_ignore_complete(
                Err(HostRepositoryIgnoreError::RepositorySource(error)),
                observations,
            );
        }
    };
    let mut prefixes = Vec::new();
    if let Some((bytes, logical_path)) = source {
        let parsed = match parse_ignore_file_observed(ctx, &logical_path, &bytes).await {
            PathOutcome::Need(need) => {
                return SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need));
            }
            PathOutcome::Complete(result) => {
                match finish_observed_nonregistry_ignore_complete(result) {
                    Ok(parsed) => parsed,
                    Err(outcome) => return outcome,
                }
            }
        };
        observations = match union_observations(&observations, &parsed.observations) {
            Ok(observations) => observations,
            Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
        };
        match parsed.result {
            Ok(parsed) => prefixes = parsed,
            Err(error) => return nonregistry_ignore_complete(Err(error), observations),
        }
    }
    nonregistry_ignore_complete(
        Ok(RepositoryIgnoreMatcher::new(
            prefixes,
            repo.ignored_directories().iter().cloned(),
        )),
        observations,
    )
}

#[async_trait]
impl Key for HostNonregistryRepositoryIgnoreKey {
    type Value =
        SourcePreparationOutcome<Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_nonregistry_ignore_legacy(
            drive_host_nonregistry_repository_ignore(
                ctx,
                self,
                HostNonregistryRepositoryIgnoreMode::Legacy,
            )
            .await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostNonregistryRepositoryIgnoreObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostNonregistryRepositoryIgnore, ObservedPathFrontierError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_host_nonregistry_repository_ignore(
            ctx,
            &self.0,
            HostNonregistryRepositoryIgnoreMode::Observed,
        )
        .await
        .map(|outcome| {
            outcome.map(
                |(result, observations)| ObservedHostNonregistryRepositoryIgnore {
                    result,
                    observations,
                },
            )
        })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostRouteRepositoryIgnoreKey {
    route: HostRepositorySourceRoute,
}

impl HostRouteRepositoryIgnoreKey {
    pub(crate) fn new(route: RootRepositoryRoute) -> Self {
        Self {
            route: HostRepositorySourceRoute::root(route),
        }
    }

    pub(crate) fn new_canonical(input: HostCanonicalRepositorySourceInput) -> Self {
        Self {
            route: HostRepositorySourceRoute::canonical(input),
        }
    }

    pub(crate) fn from_source_route(route: HostRepositorySourceRoute) -> Self {
        Self { route }
    }
}

impl std::hash::Hash for HostRouteRepositoryIgnoreKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.route.hash(state);
    }
}

impl fmt::Display for HostRouteRepositoryIgnoreKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-route-repository-ignore:{}",
            self.route.canonical_repo()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostRouteRepositoryIgnoreObservationKey(pub(crate) HostRouteRepositoryIgnoreKey);

impl fmt::Display for HostRouteRepositoryIgnoreObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostRouteRepositoryIgnore {
    result: Arc<HostRouteRepositoryIgnoreResult>,
    observations: PathObservationEpoch,
}

impl ObservedHostRouteRepositoryIgnore {
    pub(crate) fn result(&self) -> &Arc<HostRouteRepositoryIgnoreResult> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

type HostRouteRepositoryIgnoreResult = Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>;
type HostRouteRepositoryIgnoreProjection =
    (Arc<HostRouteRepositoryIgnoreResult>, PathObservationEpoch);
type HostRouteRepositoryIgnoreDriverOutcome = SourcePreparationOutcome<
    Result<HostRouteRepositoryIgnoreProjection, ObservedPathFrontierError>,
>;
type HostRouteIgnoreSourceProjection = (
    Result<HostRepositorySourceFileValue, HostRepositoryIgnoreError>,
    PathObservationEpoch,
);
type HostRouteIgnoreSourceOutcome =
    SourcePreparationOutcome<Result<HostRouteIgnoreSourceProjection, ObservedPathFrontierError>>;

fn route_ignore_complete(
    result: Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>,
    observations: PathObservationEpoch,
) -> HostRouteRepositoryIgnoreDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

async fn drive_route_ignore_source(
    ctx: &mut DiceComputations<'_>,
    route: &HostRepositorySourceRoute,
    observed_mode: bool,
) -> HostRouteIgnoreSourceOutcome {
    if let Some(root) = route.root_route() {
        return if !observed_mode {
            match dice_invariant(
                ctx.compute(&HostRepositorySourceFileKey::new(
                    root.clone(),
                    ".bazelignore".into(),
                ))
                .await,
            ) {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(source) => {
                    SourcePreparationOutcome::Complete(Ok((
                        source.map_err(HostRepositoryIgnoreError::RepositorySource),
                        PathObservationEpoch::empty(),
                    )))
                }
            }
        } else {
            match dice_invariant(
                ctx.compute(&HostRepositorySourceFileObservationKey::new(
                    root.clone(),
                    ".bazelignore".into(),
                ))
                .await,
            ) {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(Err(error)) => {
                    SourcePreparationOutcome::Complete(Err(error))
                }
                SourcePreparationOutcome::Complete(Ok(observed)) => {
                    SourcePreparationOutcome::Complete(Ok((
                        observed
                            .result()
                            .as_ref()
                            .clone()
                            .map_err(HostRepositoryIgnoreError::RepositorySource),
                        observed.observations().dupe(),
                    )))
                }
            }
        };
    }

    let relative = crate::host_repository_relative_path(".bazelignore".into())
        .expect(".bazelignore is a valid repository-relative path");
    let (result, observations) = if !observed_mode {
        match dice_invariant(ctx.compute(&route.source_observation_key(relative)).await) {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(result) => (result, PathObservationEpoch::empty()),
        }
    } else {
        let observed = match dice_invariant(
            ctx.compute(&route.source_observation_epoch_key(relative))
                .await,
        ) {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok(observed)) => observed,
        };
        (observed.result().dupe(), observed.observations().dupe())
    };
    let source = match result.as_ref() {
        Err(error) => Err(error.request_error().map_or_else(
            || HostRepositoryIgnoreError::RepositorySourceObservation(error.clone()),
            |error| HostRepositoryIgnoreError::RepositorySource(error.clone()),
        )),
        Ok(value) => match value {
            HostRepositorySourceObservation::Request(value) => Ok(value.clone()),
            HostRepositorySourceObservation::Builtin(_) => {
                unreachable!("built-in ignore source is handled by directory listing")
            }
        },
    };
    SourcePreparationOutcome::Complete(Ok((source, observations)))
}

async fn finish_builtin_route_repository_ignore(
    ctx: &mut DiceComputations<'_>,
    key: &HostRouteRepositoryIgnoreKey,
    observed_mode: bool,
    repo: &HostRepoFileValue,
    mut observations: PathObservationEpoch,
) -> HostRouteRepositoryIgnoreDriverOutcome {
    let (listing, listing_observations) = match observed_mode {
        false => match dice_invariant(
            ctx.compute(&HostRepositoryDirectoryListingKey::from_source_route(
                key.route.clone(),
                PackagePath::root(),
            ))
            .await,
        ) {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(listing) => (listing, PathObservationEpoch::empty()),
        },
        true => {
            let observed = match dice_invariant(
                ctx.compute(
                    &HostRepositoryDirectoryListingObservationKey::from_source_route(
                        key.route.clone(),
                        PackagePath::root(),
                    ),
                )
                .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    return SourcePreparationOutcome::Complete(Err(error));
                }
                SourcePreparationOutcome::Complete(Ok(observed)) => observed,
            };
            (
                observed.result().as_ref().clone(),
                observed.observations().dupe(),
            )
        }
    };
    observations = match union_observations(&observations, &listing_observations) {
        Ok(observations) => observations,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    match listing {
        Err(error) => {
            return route_ignore_complete(
                Err(HostRepositoryIgnoreError::RepositoryListing(error)),
                observations,
            );
        }
        Ok(PathDirectoryListing::Present(entries)) => {
            if let Some(entry) = entries
                .entries()
                .iter()
                .find(|entry| entry.name().as_os_str() == ".bazelignore")
                && entry.kind() != PathDirectoryEntryKind::Directory
            {
                return route_ignore_complete(
                    Err(HostRepositoryIgnoreError::BuiltinMetadata {
                        actual: entry.kind(),
                    }),
                    observations,
                );
            }
        }
        Ok(PathDirectoryListing::Missing) => {}
    }
    route_ignore_complete(
        Ok(RepositoryIgnoreMatcher::new(
            Vec::new(),
            repo.ignored_directories().iter().cloned(),
        )),
        observations,
    )
}

async fn drive_host_route_repository_ignore(
    ctx: &mut DiceComputations<'_>,
    key: &HostRouteRepositoryIgnoreKey,
    observed_mode: bool,
) -> HostRouteRepositoryIgnoreDriverOutcome {
    let (repo, mut observations) = match observed_mode {
        false => {
            match dice_invariant(
                ctx.compute(&HostRouteRepoFileKey::from_source_route(key.route.clone()))
                    .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(repo) => {
                    (repo.as_ref().clone(), PathObservationEpoch::empty())
                }
            }
        }
        true => {
            let observed = match dice_invariant(
                ctx.compute(&HostRouteRepoFileObservationKey(
                    HostRouteRepoFileKey::from_source_route(key.route.clone()),
                ))
                .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    return SourcePreparationOutcome::Complete(Err(error));
                }
                SourcePreparationOutcome::Complete(Ok(observed)) => observed,
            };
            (
                observed.result().as_ref().clone(),
                observed.observations().dupe(),
            )
        }
    };
    let repo = match repo {
        Ok(repo) => repo,
        Err(error) => {
            return route_ignore_complete(
                Err(HostRepositoryIgnoreError::RouteRepoFile(error)),
                observations,
            );
        }
    };
    if key.route.is_builtin_bazel_tools() {
        return finish_builtin_route_repository_ignore(
            ctx,
            key,
            observed_mode,
            &repo,
            observations,
        )
        .await;
    }
    let (source, source_observations) =
        match drive_route_ignore_source(ctx, &key.route, observed_mode).await {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok(source)) => source,
        };
    observations = match union_observations(&observations, &source_observations) {
        Ok(observations) => observations,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    let source = match source {
        Ok(HostRepositorySourceFileValue::Absent) => None,
        Ok(HostRepositorySourceFileValue::Present {
            bytes,
            logical_path,
        }) => Some((bytes, logical_path)),
        Err(HostRepositoryIgnoreError::RepositorySource(
            RepositorySourceFileError::WrongKind {
                actual: PathNodeKind::Directory,
                ..
            },
        )) => None,
        Err(error) => return route_ignore_complete(Err(error), observations),
    };
    let mut prefixes = Vec::new();
    if let Some((bytes, logical_path)) = source {
        let parsed = match parse_ignore_file_observed(ctx, &logical_path, &bytes).await {
            PathOutcome::Need(need) => {
                return SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need));
            }
            PathOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            PathOutcome::Complete(Ok(parsed)) => parsed,
        };
        observations = match union_observations(&observations, &parsed.observations) {
            Ok(observations) => observations,
            Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
        };
        match parsed.result {
            Ok(parsed) => prefixes = parsed,
            Err(error) => return route_ignore_complete(Err(error), observations),
        }
    }
    route_ignore_complete(
        Ok(RepositoryIgnoreMatcher::new(
            prefixes,
            repo.ignored_directories().iter().cloned(),
        )),
        observations,
    )
}

#[async_trait]
impl Key for HostRouteRepositoryIgnoreKey {
    type Value =
        SourcePreparationOutcome<Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_host_route_repository_ignore(ctx, self, false).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, _))) => {
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy routed ignore cannot produce an observed outer error")
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostRouteRepositoryIgnoreObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostRouteRepositoryIgnore, ObservedPathFrontierError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_host_route_repository_ignore(ctx, &self.0, true)
            .await
            .map(|outcome| {
                outcome.map(|(result, observations)| ObservedHostRouteRepositoryIgnore {
                    result,
                    observations,
                })
            })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, Allocative)]
struct CompiledPattern {
    original: CompactString,
    segments: Arc<[CompiledPatternSegment]>,
}

impl CompiledPattern {
    fn new(original: CompactString) -> Self {
        let segments = original
            .split('/')
            .map(CompiledPatternSegment::new)
            .collect::<Vec<_>>()
            .into();
        Self { original, segments }
    }

    fn matches_prefix(&self, path: &[&str]) -> bool {
        // With PREFIX semantics, exhausting the pattern succeeds regardless
        // of how many path segments remain.
        let mut suffix = vec![true; path.len() + 1];
        for segment in self.segments.iter().rev() {
            let mut current = vec![false; path.len() + 1];
            match segment {
                CompiledPatternSegment::Recursive => {
                    current[path.len()] = suffix[path.len()];
                    for path_index in (0..path.len()).rev() {
                        current[path_index] = suffix[path_index] || current[path_index + 1];
                    }
                }
                CompiledPatternSegment::Ordinary(segment) => {
                    for path_index in (0..path.len()).rev() {
                        current[path_index] =
                            segment.matches(path[path_index]) && suffix[path_index + 1];
                    }
                }
            }
            suffix = current;
        }
        suffix[0]
    }
}

#[derive(Debug, Clone, Allocative)]
enum CompiledPatternSegment {
    Recursive,
    Ordinary(CompiledOrdinarySegment),
}

impl CompiledPatternSegment {
    fn new(raw: &str) -> Self {
        if raw == "**" {
            return Self::Recursive;
        }
        Self::Ordinary(CompiledOrdinarySegment::new(raw))
    }
}

#[derive(Debug, Clone, Allocative)]
struct CompiledOrdinarySegment {
    explicitly_matches_leading_dot: bool,
    matcher: SegmentMatcher,
}

impl CompiledOrdinarySegment {
    fn new(raw: &str) -> Self {
        let matcher = if raw.is_empty() {
            SegmentMatcher::Never
        } else if raw == "*" {
            SegmentMatcher::Any
        } else if raw.starts_with('*') && raw.rfind('*') == Some(0) {
            SegmentMatcher::SuffixLiteral(CompactString::new(&raw[1..]))
        } else {
            let last_index = raw.len() - 1;
            if raw.ends_with('*') && raw.find('*') == Some(last_index) {
                SegmentMatcher::PrefixLiteral(CompactString::new(&raw[..last_index]))
            } else {
                SegmentMatcher::Generic(
                    raw.chars()
                        .filter_map(|character| match character {
                            '(' | ')' => None,
                            '*' => Some(SegmentAtom::AnyMany),
                            '?' => Some(SegmentAtom::AnyOne),
                            literal => Some(SegmentAtom::Literal(literal.into())),
                        })
                        .collect::<Vec<_>>()
                        .into(),
                )
            }
        };
        Self {
            explicitly_matches_leading_dot: raw.starts_with('.'),
            matcher,
        }
    }

    fn matches(&self, candidate: &str) -> bool {
        if candidate.is_empty() {
            return false;
        }
        match &self.matcher {
            SegmentMatcher::Never => false,
            // Bazel's exact `*` fast path precedes its leading-dot guard.
            SegmentMatcher::Any => true,
            matcher => {
                if candidate.starts_with('.') && !self.explicitly_matches_leading_dot {
                    return false;
                }
                match matcher {
                    SegmentMatcher::SuffixLiteral(suffix) => candidate.ends_with(suffix.as_str()),
                    SegmentMatcher::PrefixLiteral(prefix) => candidate.starts_with(prefix.as_str()),
                    SegmentMatcher::Generic(atoms) => generic_segment_matches(atoms, candidate),
                    SegmentMatcher::Never | SegmentMatcher::Any => unreachable!(),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Allocative)]
enum SegmentMatcher {
    Never,
    Any,
    SuffixLiteral(CompactString),
    PrefixLiteral(CompactString),
    Generic(Arc<[SegmentAtom]>),
}

#[derive(Debug, Clone, Copy, Allocative)]
enum SegmentAtom {
    AnyMany,
    AnyOne,
    Literal(u32),
}

fn generic_segment_matches(atoms: &[SegmentAtom], candidate: &str) -> bool {
    let candidate = candidate.chars().collect::<Vec<_>>();
    let mut matched = vec![false; candidate.len() + 1];
    matched[0] = true;

    for atom in atoms {
        let mut next = vec![false; candidate.len() + 1];
        match atom {
            SegmentAtom::AnyMany => {
                next[0] = matched[0];
                for index in 1..=candidate.len() {
                    next[index] = matched[index]
                        || (next[index - 1] && java_regex_dot_matches(candidate[index - 1]));
                }
            }
            SegmentAtom::AnyOne => {
                for index in 0..candidate.len() {
                    next[index + 1] = matched[index] && java_regex_dot_matches(candidate[index]);
                }
            }
            SegmentAtom::Literal(literal) => {
                for index in 0..candidate.len() {
                    next[index + 1] = matched[index] && u32::from(candidate[index]) == *literal;
                }
            }
        }
        matched = next;
    }
    matched[candidate.len()]
}

fn java_regex_dot_matches(character: char) -> bool {
    !matches!(
        character,
        '\n' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}'
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    #[cfg(unix)]
    use std::sync::Mutex;
    #[cfg(unix)]
    use std::sync::atomic::AtomicUsize;
    #[cfg(unix)]
    use std::sync::atomic::Ordering;

    use compact_str::CompactString;
    #[cfg(unix)]
    use dice::ActivationData;
    #[cfg(unix)]
    use dice::ActivationTracker;
    #[cfg(unix)]
    use dice::DetectCycles;
    #[cfg(unix)]
    use dice::Dice;
    #[cfg(unix)]
    use dice::DynKey;
    #[cfg(unix)]
    use dice::Key;
    #[cfg(unix)]
    use dice::RichActivation;
    #[cfg(unix)]
    use dice::UserComputationData;
    use dupe::Dupe;
    #[cfg(unix)]
    use slug_events_v2::CaptureEvaluationEvents;
    #[cfg(unix)]
    use slug_events_v2::EvaluationEvent;
    #[cfg(unix)]
    use slug_events_v2::EventBatch;
    use slug_identity_v2::PackagePath;
    use slug_workspace_v2::NormalizedAbsolutePath;
    #[cfg(unix)]
    use slug_workspace_v2::ObservedPathFrontierError;
    #[cfg(unix)]
    use slug_workspace_v2::PathLstat;
    #[cfg(unix)]
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpochKey;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationNamespace;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    #[cfg(unix)]
    use slug_workspace_v2::PathOperationResult;
    #[cfg(unix)]
    use slug_workspace_v2::PathOutcome;

    #[cfg(unix)]
    use super::HostRepositoryIgnoreKey;
    #[cfg(unix)]
    use super::HostRepositoryIgnoreObservationKey;
    use super::IgnorePathFlavor;
    use super::RepositoryIgnoreMatcher;
    use super::RepositoryIgnorePrefix;
    use super::contains_windows_short_path;
    use super::is_normalized_absolute;
    use super::java_utf8_decode;
    use super::normalize_slash_path;
    use super::prepare_ignore_file;
    use super::relative_vendor_prefix;
    use super::validate_native_path;
    use super::windows_native_eligible;
    #[cfg(unix)]
    use crate::RootPackagePolicyInputs;
    #[cfg(unix)]
    use crate::SourcePreparationOutcome;
    #[cfg(unix)]
    use crate::host_file::HostFileBytesKey;
    #[cfg(unix)]
    use crate::inject_root_module_request_inputs;
    #[cfg(unix)]
    use crate::inject_root_package_policy_inputs;
    #[cfg(unix)]
    use crate::repo_file::HostRepoFileKey;
    #[cfg(unix)]
    use crate::repo_file::tests::routed_policy_epoch;
    #[cfg(unix)]
    use crate::repo_file::tests::routed_policy_route;
    #[cfg(unix)]
    use crate::repo_file::tests::routed_policy_transaction;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationEpochEntry;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationResult;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationResultEpoch;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationResultEpochKey;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationSuccess;

    fn path(value: &str) -> PackagePath {
        PackagePath::parse(value).unwrap()
    }

    fn matcher(prefixes: &[&str], patterns: &[&str]) -> RepositoryIgnoreMatcher {
        RepositoryIgnoreMatcher::new(
            prefixes
                .iter()
                .map(|prefix| RepositoryIgnorePrefix::new_normalized(*prefix)),
            patterns.iter().map(|pattern| CompactString::new(pattern)),
        )
    }

    fn assert_pattern(pattern: &str, directory: &str, expected: bool) {
        let matcher = matcher(&[], &[pattern]);
        assert_eq!(
            matcher.matching_entry(&path(directory)),
            expected.then_some(pattern),
            "pattern={pattern:?}, directory={directory:?}"
        );
    }

    #[test]
    fn literal_prefixes_are_sorted_deduplicated_and_component_aware() {
        let repository_ignore = matcher(&["foo/bar", "foo", "bar", "foo"], &["pattern/**"]);
        assert_eq!(
            repository_ignore
                .literal_prefixes
                .iter()
                .map(RepositoryIgnorePrefix::as_str)
                .collect::<Vec<_>>(),
            ["bar", "foo", "foo/bar"]
        );
        assert_eq!(repository_ignore.matching_entry(&path("foo")), Some("foo"));
        assert_eq!(
            repository_ignore.matching_entry(&path("foo/bar/baz")),
            Some("foo")
        );
        assert_eq!(repository_ignore.matching_entry(&path("fooz")), None);
        assert_eq!(
            repository_ignore.matching_entry(&path("pattern/child")),
            Some("pattern/**")
        );

        let root = matcher(&["child", ""], &["**"]);
        assert_eq!(root.matching_entry(&path("")), Some(""));
        assert_eq!(root.matching_entry(&path("child/grandchild")), Some(""));
    }

    #[test]
    fn literal_prefix_domain_retains_empty_and_leading_up_level_entries() {
        let repository_ignore = matcher(
            &["../outside", "foo/../bar", "", "../../far", "../outside"],
            &[],
        );
        assert_eq!(
            repository_ignore
                .literal_prefixes
                .iter()
                .map(RepositoryIgnorePrefix::as_str)
                .collect::<Vec<_>>(),
            ["", "../../far", "../outside", "foo/../bar"]
        );
        assert_eq!(repository_ignore.matching_entry(&path("")), Some(""));

        let up_only = matcher(&["../outside", "../../far"], &[]);
        assert_eq!(up_only.matching_entry(&path("outside")), None);
        assert_eq!(up_only.matching_entry(&path("far/child")), None);
    }

    #[test]
    fn matches_bazel_prefix_recursive_and_mixed_wildcard_table() {
        for (pattern, directory, expected) in [
            ("foo/bar", "foo/bar", true),
            ("foo/bar", "foo/bar/child", true),
            ("foo/bar", "foo", false),
            ("foo/bar", "foo/barn", false),
            ("**", "", true),
            ("**", ".hidden/child", true),
            ("**/sub", "sub", true),
            ("**/sub", "a/b/sub/child", true),
            ("**/sub", "a/b", false),
            ("foo/**/bar", "foo/bar", true),
            ("foo/**/bar", "foo/a/b/bar/child", true),
            ("foo/**", "foo", true),
            ("foo/**", "foo/a/b", true),
            ("bar/*/one?ub", "bar/x/onesub", true),
            ("bar/*/one?ub", "bar/x/oneXXub", false),
            ("*/sub", ".hidden/sub", true),
            ("*dden/sub", ".hidden/sub", false),
            (".hi*/*/sub", ".hidden/x/sub", true),
            ("?/sub", ".hidden/sub", false),
        ] {
            assert_pattern(pattern, directory, expected);
        }
    }

    #[test]
    fn matches_bazel_escaping_and_optimization_sensitive_parentheses() {
        for (pattern, directory, expected) in [
            ("*?", "value?", true),
            ("*?", "valuex", false),
            ("?*", "?value", true),
            ("?*", "xvalue", false),
            ("*(bar)", "x(bar)", true),
            ("*(bar)", "xbar", false),
            ("(bar)*", "(bar)x", true),
            ("(bar)*", "barx", false),
            ("foo(bar)", "foobar", true),
            ("foo(bar)", "foo(bar)", false),
            ("*foo(bar)*", "xfoobary", true),
            ("*foo(bar)*", "xfoo(bar)y", false),
            ("foo*(bar)", "fooxbar", true),
            ("foo*(bar)", "foox(bar)", false),
            ("(.hidden)", ".hidden", false),
            (".(hidden)", ".hidden", true),
            (r"^$|+{}[]\.", r"^$|+{}[]\.", true),
            ("file.txt", "fileXtxt", false),
        ] {
            assert_pattern(pattern, directory, expected);
        }
    }

    #[test]
    fn generic_regex_wildcards_reject_java_line_terminators_but_fast_paths_do_not() {
        for separator in ['\n', '\r', '\u{0085}', '\u{2028}', '\u{2029}'] {
            let middle = format!("a{separator}b");
            assert_pattern("a?b", &middle, false);
            assert_pattern("a*b", &middle, false);

            let separator = separator.to_string();
            assert_pattern("*", &separator, true);
            assert_pattern("**", &separator, true);
            assert_pattern("*b", &format!("{separator}b"), true);
            assert_pattern("a*", &format!("a{separator}"), true);
        }
    }

    #[test]
    fn raw_patterns_are_retained_in_order_without_validation() {
        let originals = [
            "",
            "/absolute",
            "trailing/",
            "a//b",
            ".",
            "..",
            "a**b",
            "**foo",
            "a**b",
        ];
        let repository_ignore = matcher(&[], &originals);
        assert_eq!(
            repository_ignore
                .patterns
                .iter()
                .map(|pattern| pattern.original.as_str())
                .collect::<Vec<_>>(),
            originals
        );
        assert_eq!(repository_ignore.matching_entry(&path("")), None);
        assert_eq!(repository_ignore.matching_entry(&path("absolute")), None);
        assert_eq!(repository_ignore.matching_entry(&path("trailing")), None);
        assert_eq!(
            repository_ignore.matching_entry(&path("axxb")),
            Some("a**b")
        );

        let leading_embedded_recursive = matcher(&[], &["**foo"]);
        assert_eq!(
            leading_embedded_recursive.matching_entry(&path("xxfoo")),
            Some("**foo")
        );
    }

    #[test]
    fn matching_entry_prefers_literals_then_original_pattern_order() {
        let pattern_first = matcher(&[], &["**", "foo"]);
        assert_eq!(pattern_first.matching_entry(&path("foo")), Some("**"));

        let exact_first = matcher(&[], &["foo", "**"]);
        assert_eq!(exact_first.matching_entry(&path("foo")), Some("foo"));

        let literal_first = matcher(&["foo"], &["**"]);
        assert_eq!(
            literal_first.matching_entry(&path("foo/child")),
            Some("foo")
        );
    }

    #[test]
    fn semantic_equality_uses_normalized_prefixes_and_ordered_original_patterns() {
        let left = matcher(&["z", "a", "z"], &["**/one", "two"]);
        let same = matcher(&["a", "z"], &["**/one", "two"]);
        let reordered_patterns = matcher(&["a", "z"], &["two", "**/one"]);
        let duplicate_pattern = matcher(&["a", "z"], &["**/one", "two", "two"]);
        let different_prefix = matcher(&["a"], &["**/one", "two"]);

        assert_eq!(left, same);
        assert_ne!(left, reordered_patterns);
        assert_ne!(left, duplicate_pattern);
        assert_ne!(left, different_prefix);
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().collect()
    }

    fn logical_path() -> NormalizedAbsolutePath {
        #[cfg(unix)]
        {
            NormalizedAbsolutePath::new("/workspace/.bazelignore").unwrap()
        }
        #[cfg(windows)]
        {
            NormalizedAbsolutePath::new(r"C:\workspace\.bazelignore").unwrap()
        }
    }

    #[test]
    fn bazelignore_decoding_line_splitting_and_normalization_are_exact() {
        let prepared = prepare_ignore_file(
            b"#comment\r\n a/./b//\r\xef\xbb\xbfkeep\nbad/\xff/../tail\ra/..\n..\n",
            IgnorePathFlavor::Unix,
            &logical_path(),
        )
        .unwrap();
        assert_eq!(
            prepared
                .lines
                .iter()
                .map(|line| String::from_utf16_lossy(&line.normalized))
                .collect::<Vec<_>>(),
            [" a/b", "\u{feff}keep", "bad/tail", "", ".."]
        );
        assert!(prepared.lines.iter().all(|line| !line.absolute));
    }

    #[test]
    fn java_utf8_replacement_consumes_malformed_sequences_like_input_stream_reader() {
        assert_eq!(java_utf8_decode(b"\xed\xa0\x80"), "\u{fffd}");
        assert_eq!(java_utf8_decode(b"\xc0\x80"), "\u{fffd}\u{fffd}");
        assert_eq!(
            java_utf8_decode(b"\xe0\x80\x80"),
            "\u{fffd}\u{fffd}\u{fffd}"
        );
        assert_eq!(java_utf8_decode(b"\xe0\x80"), "\u{fffd}\u{fffd}");
        assert_eq!(java_utf8_decode(b"\xe2(\xa1"), "\u{fffd}(\u{fffd}");
        assert_eq!(java_utf8_decode(b"\xe2\x82"), "\u{fffd}");
        assert_eq!(
            java_utf8_decode(b"\xf0\x80\x80\x80"),
            "\u{fffd}\u{fffd}\u{fffd}\u{fffd}"
        );
        assert_eq!(
            java_utf8_decode(b"\xf4\x90\x80\x80"),
            "\u{fffd}\u{fffd}\u{fffd}\u{fffd}"
        );
        assert_eq!(java_utf8_decode(b"\xff"), "\u{fffd}");
    }

    #[test]
    fn pure_unix_and_windows_path_tables_preserve_bazel_domains() {
        for (flavor, input, normalized, absolute) in [
            (IgnorePathFlavor::Unix, "a//b/./../c/", "a/c", false),
            (IgnorePathFlavor::Unix, "../../a", "../../a", false),
            (IgnorePathFlavor::Unix, r"C:\tmp\A~1", r"C:\tmp\A~1", false),
            (IgnorePathFlavor::Unix, "/../../a", "/a", true),
            (
                IgnorePathFlavor::Windows,
                r"c:\base\\dir\..\PROGRA~1\\",
                "C:/base/PROGRA~1",
                true,
            ),
            (IgnorePathFlavor::Windows, r"..\..\a", "../../a", false),
            (IgnorePathFlavor::Windows, r"\..\a", "/a", true),
            (
                IgnorePathFlavor::Windows,
                r"C:relative",
                "C:relative",
                false,
            ),
        ] {
            let actual = normalize_slash_path(&wide(input), flavor);
            assert_eq!(String::from_utf16_lossy(&actual), normalized, "{input:?}");
            assert_eq!(
                is_normalized_absolute(&actual, flavor),
                absolute,
                "{input:?}"
            );
        }

        assert!(validate_native_path(r"C:\ok", IgnorePathFlavor::Windows).is_ok());
        assert_eq!(
            validate_native_path(r"C:\😀\bad?name", IgnorePathFlavor::Windows)
                .unwrap_err()
                .as_str(),
            r"Illegal char <?> at index 9: C:\😀\bad?name"
        );
        assert!(validate_native_path(r"C:\bad?name", IgnorePathFlavor::Unix).is_ok());
        assert_eq!(
            validate_native_path("bad\0name", IgnorePathFlavor::Unix)
                .unwrap_err()
                .as_str(),
            "Nul character not allowed: bad\0name"
        );
    }

    #[test]
    fn windows_native_path_validation_matches_openjdk_parser_classes() {
        for valid in [
            r"C:\foo",
            "C:/foo",
            "C:foo",
            "C:",
            r"\foo",
            "/foo",
            r"\\server\share",
            r"\\server \share",
            r"\\server\share ",
            r"\\?\C:\foo",
            r"\\?\UNC\server\share",
        ] {
            assert!(
                validate_native_path(valid, IgnorePathFlavor::Windows).is_ok(),
                "{valid:?}"
            );
        }
        for (invalid, expected) in [
            (r"\\", r"UNC path is missing hostname: \\"),
            (r"\\server", r"UNC path is missing sharename: \\server"),
            (
                "\\\\server\\",
                "UNC path is missing sharename: \\\\server\\",
            ),
            (
                r"\\ser?ver\share",
                r"Illegal character [?] in path at index 5: \\ser?ver\share",
            ),
            ("foo \\bar", "Trailing char < > at index 3: foo \\bar"),
            ("C:\\😀?x", "Illegal char <?> at index 5: C:\\😀?x"),
            (
                "\\\\?\\relative",
                "Long path prefix can only be used with an absolute path: relative",
            ),
            (
                "\\\\?\\C:relative",
                "Long path prefix can only be used with an absolute path: C:relative",
            ),
            (
                "\\\\?\\unc\\server\\share",
                "Long path prefix can only be used with an absolute path: unc\\server\\share",
            ),
            (
                "\\\\?\\UNC\\server",
                r"UNC path is missing sharename: \\server",
            ),
        ] {
            assert_eq!(
                validate_native_path(invalid, IgnorePathFlavor::Windows)
                    .unwrap_err()
                    .as_str(),
                expected,
                "{invalid:?}"
            );
        }
        assert_eq!(
            validate_native_path("a\0b", IgnorePathFlavor::Windows)
                .unwrap_err()
                .as_str(),
            "Illegal char <\0> at index 1: a\0b"
        );
    }

    #[test]
    fn windows_long_path_candidate_is_exact_and_pre_normalization_gated() {
        for accepted in [
            r"C:\PROGRA~1\tool",
            r"C:\A~1.TXT",
            "C:\\A~1.😀😀😀",
            r"\\?\C:\PROGRA~1",
        ] {
            assert!(contains_windows_short_path(&wide(accepted)), "{accepted:?}");
            assert!(windows_native_eligible(&wide(accepted)), "{accepted:?}");
        }
        for rejected in [
            r"C:\~1",
            r"C:\ABCDEF~12",
            r"C:\A~1.TOOL",
            r"C:\TOOLONG~1.TXT",
        ] {
            assert!(
                !contains_windows_short_path(&wide(rejected)),
                "{rejected:?}"
            );
        }
        assert!(!windows_native_eligible(&wide(r"C:\base\..\PROGRA~1")));
        assert!(!windows_native_eligible(&wide(r"C:\base\.\PROGRA~1")));
        assert!(!windows_native_eligible(&wide(r"C:PROGRA~1")));
        assert!(!windows_native_eligible(&wide(r"\PROGRA~1")));
        assert!(!windows_native_eligible(&wide(r"\\.\C:\PROGRA~1")));
        assert!(!windows_native_eligible(&wide(r"\??\C:\PROGRA~1")));
    }

    #[test]
    fn vendor_prefixes_are_component_contained_and_inclusive() {
        #[cfg(unix)]
        let (root, nested, outside) = (
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            NormalizedAbsolutePath::new("/workspace/vendor/cache").unwrap(),
            NormalizedAbsolutePath::new("/workspace-other/vendor").unwrap(),
        );
        #[cfg(windows)]
        let (root, nested, outside) = (
            NormalizedAbsolutePath::new(r"C:\workspace").unwrap(),
            NormalizedAbsolutePath::new(r"C:\workspace\vendor\cache").unwrap(),
            NormalizedAbsolutePath::new(r"C:\workspace-other\vendor").unwrap(),
        );
        assert_eq!(relative_vendor_prefix(&root, &root).unwrap().as_str(), "");
        assert_eq!(
            relative_vendor_prefix(&root, &nested).unwrap().as_str(),
            "vendor/cache"
        );
        assert_eq!(relative_vendor_prefix(&root, &outside), None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn host_key_composes_repo_vendor_and_first_file_in_root_order() {
        fn lstat(kind: PathNodeKind, variant: i64) -> PathLstat {
            PathLstat::new(kind, variant, variant, variant, variant, 0o755)
        }
        fn demand(path: &str, operation: PathObservationOperation) -> PathObservationDemand {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        }
        fn present(
            path: &str,
            kind: PathNodeKind,
            variant: i64,
        ) -> (PathObservationDemand, PathObservationResult) {
            (
                demand(path, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(kind, variant))),
            )
        }
        fn missing(path: &str) -> (PathObservationDemand, PathObservationResult) {
            (
                demand(path, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            )
        }
        fn bytes(
            path: &str,
            value: &'static [u8],
        ) -> (PathObservationDemand, PathObservationResult) {
            (
                demand(path, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(value))),
            )
        }

        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let roots: Arc<[NormalizedAbsolutePath]> = Arc::from([
            NormalizedAbsolutePath::new("/root-a").unwrap(),
            NormalizedAbsolutePath::new("/root-b").unwrap(),
            NormalizedAbsolutePath::new("/root-c").unwrap(),
        ]);
        let inputs = RootPackagePolicyInputs::new(
            workspace.dupe(),
            roots.dupe(),
            std::iter::empty::<&str>(),
            Some(NormalizedAbsolutePath::new("/root-b/vendor").unwrap()),
            Some("warning"),
        )
        .unwrap();
        let epoch = PathObservationEpoch::new([
            present("/", PathNodeKind::Directory, 1),
            present("/workspace", PathNodeKind::Directory, 2),
            present("/workspace/REPO.bazel", PathNodeKind::RegularFile, 3),
            bytes(
                "/workspace/REPO.bazel",
                b"ignore_directories(['repo/**'])\n",
            ),
            present("/root-a", PathNodeKind::Directory, 4),
            missing("/root-a/.bazelignore"),
            present("/root-b", PathNodeKind::Directory, 5),
            present("/root-b/.bazelignore", PathNodeKind::SpecialFile, 6),
            bytes("/root-b/.bazelignore", b"literal\n"),
        ])
        .unwrap();
        let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(&mut updater, inputs).unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch.dupe())])
            .unwrap();
        let mut transaction = updater.commit().await;
        let outcome = transaction
            .compute(&HostRepositoryIgnoreKey::new(workspace.dupe()))
            .await
            .unwrap();
        let PathOutcome::Complete(value) = outcome else {
            panic!("complete script returned an observation Need");
        };
        let matcher = value.as_ref().as_ref().unwrap();
        assert_eq!(
            matcher.matching_entry(&PackagePath::parse("vendor/cache").unwrap()),
            Some("vendor")
        );
        assert_eq!(
            matcher.matching_entry(&PackagePath::parse("literal/child").unwrap()),
            Some("literal")
        );
        assert_eq!(
            matcher.matching_entry(&PackagePath::parse("repo/child").unwrap()),
            Some("repo/**")
        );
        assert!(HostRepositoryIgnoreKey::validity(&PathOutcome::Complete(
            value.dupe()
        )));
        assert!(HostRepositoryIgnoreKey::equality(
            &PathOutcome::Complete(value.dupe()),
            &PathOutcome::Complete(value)
        ));

        let later_vendor_inputs = RootPackagePolicyInputs::new(
            workspace.dupe(),
            roots,
            std::iter::empty::<&str>(),
            Some(NormalizedAbsolutePath::new("/root-c/vendor").unwrap()),
            Some("warning"),
        )
        .unwrap();
        let later_vendor_dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = later_vendor_dice.updater();
        inject_root_package_policy_inputs(&mut updater, later_vendor_inputs).unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        let mut transaction = updater.commit().await;
        let outcome = transaction
            .compute(&HostRepositoryIgnoreKey::new(workspace))
            .await
            .unwrap();
        let PathOutcome::Complete(value) = outcome else {
            panic!("later-root vendor must not create a demand after the first file");
        };
        let matcher = value.as_ref().as_ref().unwrap();
        assert_eq!(
            matcher.matching_entry(&PackagePath::parse("vendor/cache").unwrap()),
            None
        );
        assert_eq!(
            matcher.matching_entry(&PackagePath::parse("literal/child").unwrap()),
            Some("literal")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn repo_need_is_transient_and_precedes_every_root_demand() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let root = NormalizedAbsolutePath::new("/root-a").unwrap();
        let inputs = RootPackagePolicyInputs::new(
            workspace.dupe(),
            Arc::from([root.dupe()]),
            std::iter::empty::<&str>(),
            Some(NormalizedAbsolutePath::new("/root-a/vendor").unwrap()),
            Some("warning"),
        )
        .unwrap();
        let lstat = |kind, variant| PathLstat::new(kind, variant, variant, variant, variant, 0o755);
        let demand = |path: &str| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                PathObservationOperation::Lstat,
            )
        };
        let epoch = PathObservationEpoch::new([
            (
                demand("/"),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                    1,
                ))),
            ),
            (
                demand("/workspace"),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                    2,
                ))),
            ),
            (
                demand("/root-a"),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                    3,
                ))),
            ),
            (
                demand("/root-a/.bazelignore"),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ),
        ])
        .unwrap();
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(&mut updater, inputs).unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        let mut transaction = updater.commit().await;

        let repo = transaction
            .compute(&HostRepoFileKey::new(workspace.dupe()))
            .await
            .unwrap();
        let PathOutcome::Need(repo_need) = &repo else {
            panic!("missing REPO observation should be transient");
        };
        assert!(repo_need.demands().iter().all(|demand| {
            demand.path().as_path().starts_with("/workspace")
                && !demand.path().as_path().starts_with("/root-a")
        }));
        assert!(!HostRepoFileKey::validity(&repo));
        assert!(!HostRepoFileKey::equality(&repo, &repo));

        let ignored = transaction
            .compute(&HostRepositoryIgnoreKey::new(workspace))
            .await
            .unwrap();
        let PathOutcome::Need(ignore_need) = &ignored else {
            panic!("ignore owner must propagate the REPO Need");
        };
        assert_eq!(ignore_need.demands(), repo_need.demands());
        assert!(!HostRepositoryIgnoreKey::validity(&ignored));
        assert!(!HostRepositoryIgnoreKey::equality(&ignored, &ignored));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn earlier_root_need_prevents_a_fully_available_later_probe() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let roots: Arc<[NormalizedAbsolutePath]> = Arc::from([
            NormalizedAbsolutePath::new("/root-a").unwrap(),
            NormalizedAbsolutePath::new("/root-b").unwrap(),
        ]);
        let inputs = RootPackagePolicyInputs::new(
            workspace.dupe(),
            roots,
            std::iter::empty::<&str>(),
            Some(NormalizedAbsolutePath::new("/root-b/vendor").unwrap()),
            Some("warning"),
        )
        .unwrap();
        let lstat = |kind, variant| PathLstat::new(kind, variant, variant, variant, variant, 0o755);
        let demand = |path: &str, operation| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        };
        let epoch = PathObservationEpoch::new([
            (
                demand("/", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                    1,
                ))),
            ),
            (
                demand("/workspace", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                    2,
                ))),
            ),
            (
                demand("/workspace/REPO.bazel", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ),
            (
                demand("/root-a", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                    3,
                ))),
            ),
            (
                demand("/root-b", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                    4,
                ))),
            ),
            (
                demand("/root-b/.bazelignore", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::RegularFile,
                    5,
                ))),
            ),
            (
                demand("/root-b/.bazelignore", PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    b"later\n".as_slice(),
                ))),
            ),
        ])
        .unwrap();
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(&mut updater, inputs).unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        let mut transaction = updater.commit().await;
        let outcome = transaction
            .compute(&HostRepositoryIgnoreKey::new(workspace))
            .await
            .unwrap();
        let PathOutcome::Need(need) = outcome else {
            panic!("earlier root probe should remain transient");
        };
        assert!(
            need.demands()
                .iter()
                .all(|demand| demand.path().as_path().starts_with("/root-a"))
        );
        assert!(
            need.demands()
                .iter()
                .all(|demand| !demand.path().as_path().starts_with("/root-b"))
        );
    }
    #[cfg(unix)]
    #[derive(Default)]
    struct ObservedIgnoreTracker {
        legacy_repo: AtomicUsize,
        observed_repo: AtomicUsize,
        observed_file: AtomicUsize,
        parent_event_data: Mutex<Vec<bool>>,
    }

    #[cfg(unix)]
    impl ObservedIgnoreTracker {
        fn assert_clean(&self) {
            assert_eq!(self.legacy_repo.load(Ordering::SeqCst), 0);
            assert!(!self.parent_event_data.lock().unwrap().contains(&true));
        }
    }

    #[cfg(unix)]
    impl ActivationTracker for ObservedIgnoreTracker {
        fn key_activated(
            &self,
            _key: &DynKey,
            _deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            if key
                .downcast_ref::<HostRepositoryIgnoreObservationKey>()
                .is_some()
            {
                self.parent_event_data
                    .lock()
                    .unwrap()
                    .push(activation.evaluation_data().is_some());
            } else if key.downcast_ref::<HostRepositoryIgnoreKey>().is_some()
                || key.downcast_ref::<HostRepoFileKey>().is_some()
                || key.downcast_ref::<HostFileBytesKey>().is_some()
            {
                self.legacy_repo.fetch_add(1, Ordering::SeqCst);
            } else if key
                .downcast_ref::<crate::repo_file::HostRepoFileObservationKey>()
                .is_some()
            {
                self.observed_repo.fetch_add(1, Ordering::SeqCst);
            } else if key
                .downcast_ref::<crate::host_file::HostFileBytesObservationKey>()
                .is_some()
            {
                self.observed_file.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    #[cfg(unix)]
    fn observed_inputs(roots: Arc<[NormalizedAbsolutePath]>) -> RootPackagePolicyInputs {
        RootPackagePolicyInputs::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            roots,
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap()
    }

    #[cfg(unix)]
    async fn compute_observed_ignore(
        dice: &Arc<Dice>,
        tracker: Arc<ObservedIgnoreTracker>,
        inputs: Option<RootPackagePolicyInputs>,
        epoch: PathObservationEpoch,
    ) -> <HostRepositoryIgnoreObservationKey as Key>::Value {
        let mut user_data = UserComputationData {
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(user_data);
        if let Some(inputs) = inputs {
            inject_root_package_policy_inputs(&mut updater, inputs).unwrap();
        }
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        updater
            .commit()
            .await
            .compute(&HostRepositoryIgnoreObservationKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
            ))
            .await
            .unwrap()
    }

    #[cfg(unix)]
    fn observed_epoch(
        repo: &'static [u8],
        ignored: Option<&'static [u8]>,
        file_kind: PathNodeKind,
        variant: i64,
    ) -> PathObservationEpoch {
        let lstat = |kind, value| PathLstat::new(kind, value, value, value, value, 0o755);
        let demand = |path: &str, operation| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        };
        let mut entries = vec![
            (
                demand("/", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                    variant,
                ))),
            ),
            (
                demand("/workspace", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                    variant,
                ))),
            ),
            (
                demand("/workspace/REPO.bazel", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::RegularFile,
                    variant,
                ))),
            ),
            (
                demand("/workspace/REPO.bazel", PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(repo))),
            ),
            (
                demand("/root-a", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                    variant,
                ))),
            ),
            (
                demand("/root-a/.bazelignore", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ),
            (
                demand("/root-b", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                    variant,
                ))),
            ),
        ];
        entries.push((
            demand("/root-b/.bazelignore", PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(lstat(file_kind, variant))),
        ));
        if file_kind != PathNodeKind::Directory {
            let bytes = ignored.map_or(PathOperationResult::Missing, |bytes| {
                PathOperationResult::Present(Arc::from(bytes))
            });
            entries.push((
                demand("/root-b/.bazelignore", PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(bytes),
            ));
        }
        PathObservationEpoch::new(entries).unwrap()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_ignore_retains_every_selected_and_negative_arc_without_legacy_keys() {
        let roots: Arc<[NormalizedAbsolutePath]> = Arc::from([
            NormalizedAbsolutePath::new("/root-a").unwrap(),
            NormalizedAbsolutePath::new("/root-b").unwrap(),
        ]);
        let epoch = observed_epoch(
            b"ignore_directories(['repo/**'])\n",
            Some(b"selected\n"),
            PathNodeKind::RegularFile,
            1,
        );
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ObservedIgnoreTracker::default());
        let value = compute_observed_ignore(
            &dice,
            tracker.dupe(),
            Some(observed_inputs(roots)),
            epoch.dupe(),
        )
        .await;
        assert!(HostRepositoryIgnoreObservationKey::validity(&value));
        assert!(HostRepositoryIgnoreObservationKey::equality(&value, &value));
        let PathOutcome::Complete(Ok(value)) = &value else {
            panic!("observed root ignore must complete");
        };
        let matcher = value.result().as_ref().unwrap();
        assert_eq!(
            matcher.matching_entry(&PackagePath::parse("selected/child").unwrap()),
            Some("selected")
        );
        assert_eq!(
            matcher.matching_entry(&PackagePath::parse("repo/child").unwrap()),
            Some("repo/**")
        );
        for (demand, expected) in epoch.observations() {
            let retained = value
                .observations()
                .get(demand)
                .expect("every selected and negative input is retained");
            assert!(Arc::ptr_eq(expected, retained));
        }
        let result = value.result.dupe();
        assert!(Arc::ptr_eq(&result, &value.result));
        tracker.assert_clean();
        assert_eq!(tracker.observed_repo.load(Ordering::SeqCst), 1);
        assert_eq!(tracker.observed_file.load(Ordering::SeqCst), 3);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_ignore_need_negative_and_parse_error_remain_discriminating() {
        let roots: Arc<[NormalizedAbsolutePath]> = Arc::from([
            NormalizedAbsolutePath::new("/root-a").unwrap(),
            NormalizedAbsolutePath::new("/root-b").unwrap(),
        ]);
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ObservedIgnoreTracker::default());
        let need = compute_observed_ignore(
            &dice,
            tracker.dupe(),
            Some(observed_inputs(roots.dupe())),
            PathObservationEpoch::empty(),
        )
        .await;
        assert!(matches!(need, PathOutcome::Need(_)));
        assert!(!HostRepositoryIgnoreObservationKey::validity(&need));
        assert!(!HostRepositoryIgnoreObservationKey::equality(&need, &need));
        tracker.assert_clean();

        let repo_tracker = Arc::new(ObservedIgnoreTracker::default());
        let PathOutcome::Complete(Ok(repo_error)) = compute_observed_ignore(
            &dice,
            repo_tracker.dupe(),
            Some(observed_inputs(roots.dupe())),
            observed_epoch(b"fail('boom')\n", None, PathNodeKind::Directory, 2),
        )
        .await
        else {
            panic!("REPO failure must retain a completed frontier");
        };
        assert!(matches!(
            repo_error.result(),
            Err(super::HostRepositoryIgnoreError::RepoFile(_))
        ));
        assert_eq!(repo_tracker.observed_file.load(Ordering::SeqCst), 1);
        assert!(
            repo_error
                .observations()
                .observations()
                .keys()
                .all(|demand| !demand.path().as_path().starts_with("/root-a"))
        );
        repo_tracker.assert_clean();

        let file_bytes = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/root-b/.bazelignore").unwrap(),
            PathObservationOperation::FileBytes,
        );
        let host_tracker = Arc::new(ObservedIgnoreTracker::default());
        let PathOutcome::Complete(Ok(host_error)) = compute_observed_ignore(
            &dice,
            host_tracker.dupe(),
            Some(observed_inputs(roots.dupe())),
            observed_epoch(b"", None, PathNodeKind::RegularFile, 3),
        )
        .await
        else {
            panic!("Host-file failure must retain a completed frontier");
        };
        assert!(matches!(
            host_error.result(),
            Err(super::HostRepositoryIgnoreError::HostFile(_))
        ));
        assert!(host_error.observations().get(&file_bytes).is_some());
        host_tracker.assert_clean();

        let directory = compute_observed_ignore(
            &dice,
            Arc::new(ObservedIgnoreTracker::default()),
            Some(observed_inputs(roots.dupe())),
            observed_epoch(b"", None, PathNodeKind::Directory, 2),
        )
        .await;
        let PathOutcome::Complete(Ok(directory)) = directory else {
            panic!("directory negative probe must complete");
        };
        assert!(directory.result().is_ok());

        let parse = compute_observed_ignore(
            &dice,
            Arc::new(ObservedIgnoreTracker::default()),
            Some(observed_inputs(roots)),
            observed_epoch(b"", Some(b"/absolute\n"), PathNodeKind::RegularFile, 3),
        )
        .await;
        let PathOutcome::Complete(Ok(parse)) = parse else {
            panic!("parse failure remains a completed semantic error");
        };
        assert!(matches!(
            parse.result(),
            Err(super::HostRepositoryIgnoreError::InvalidAbsolute { .. })
        ));
        assert!(
            parse
                .observations()
                .observations()
                .keys()
                .any(|demand| demand.path().as_path().ends_with(".bazelignore"))
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_ignore_reuses_a_b_a_without_parent_event_data() {
        let roots: Arc<[NormalizedAbsolutePath]> = Arc::from([
            NormalizedAbsolutePath::new("/root-a").unwrap(),
            NormalizedAbsolutePath::new("/root-b").unwrap(),
        ]);
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ObservedIgnoreTracker::default());
        let a = observed_epoch(b"", Some(b"a\n"), PathNodeKind::RegularFile, 10);
        let b = observed_epoch(b"", Some(b"b\n"), PathNodeKind::RegularFile, 11);
        let first = compute_observed_ignore(
            &dice,
            tracker.dupe(),
            Some(observed_inputs(roots.dupe())),
            a.dupe(),
        )
        .await;
        let warm = compute_observed_ignore(
            &dice,
            tracker.dupe(),
            Some(observed_inputs(roots.dupe())),
            a.dupe(),
        )
        .await;
        assert!(HostRepositoryIgnoreObservationKey::equality(&warm, &first));
        let changed = compute_observed_ignore(
            &dice,
            tracker.dupe(),
            Some(observed_inputs(roots.dupe())),
            b,
        )
        .await;
        assert!(!HostRepositoryIgnoreObservationKey::equality(
            &changed, &first
        ));
        let restored =
            compute_observed_ignore(&dice, tracker.dupe(), Some(observed_inputs(roots)), a).await;
        assert!(HostRepositoryIgnoreObservationKey::equality(
            &restored, &first
        ));
        tracker.assert_clean();
    }

    #[cfg(unix)]
    #[derive(Default)]
    struct RoutedPolicyTracker {
        activations: Mutex<Vec<(String, Option<EventBatch>)>>,
        rows: Mutex<Vec<(String, Vec<String>)>>,
    }

    #[cfg(unix)]
    impl RoutedPolicyTracker {
        fn take(&self) -> Vec<(String, Option<EventBatch>)> {
            std::mem::take(&mut *self.activations.lock().unwrap())
        }

        fn take_rows(&self) -> Vec<(String, Vec<String>)> {
            std::mem::take(&mut *self.rows.lock().unwrap())
        }
        fn clear(&self) {
            self.take();
            self.take_rows();
        }
    }
    #[cfg(unix)]
    const OBS_DEPS: &[&str] = &[
        "observed-host-nonregistry-repo-file:dep@1",
        "observed-repository-source-file:dep:.bazelignore",
    ];
    #[cfg(unix)]
    const LEG_DEPS: &[&str] = &[
        "host-nonregistry-repo-file:dep@1",
        "repository-source-file:dep:.bazelignore",
    ];
    #[cfg(unix)]
    fn assert_row(tracker: &RoutedPolicyTracker, key: impl ToString, deps: &[&str]) {
        assert_eq!(
            tracker.take_rows(),
            [(
                key.to_string(),
                deps.iter().map(|dep| (*dep).to_owned()).collect()
            )]
        );
    }

    #[cfg(unix)]
    impl ActivationTracker for RoutedPolicyTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            let name = key.to_string();
            if name.starts_with("observed-host-nonregistry-repository-ignore:")
                || name.starts_with("host-nonregistry-repository-ignore:")
            {
                self.rows
                    .lock()
                    .unwrap()
                    .push((name, deps.map(ToString::to_string).collect()));
            }
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            self.activations.lock().unwrap().push((
                key.to_string(),
                activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            ));
        }
    }

    #[cfg(unix)]
    async fn compute_observed_routed_policy(
        dice: &Arc<Dice>,
        tracker: &Arc<RoutedPolicyTracker>,
        epoch: PathObservationEpoch,
        inject_policy: bool,
    ) -> SourcePreparationOutcome<
        Result<super::ObservedHostRouteRepositoryIgnore, ObservedPathFrontierError>,
    > {
        let mut transaction = routed_policy_transaction(
            dice,
            tracker.dupe() as Arc<dyn ActivationTracker>,
            epoch,
            inject_policy,
        )
        .await;
        transaction
            .compute(&super::HostRouteRepositoryIgnoreObservationKey(
                super::HostRouteRepositoryIgnoreKey::new(routed_policy_route()),
            ))
            .await
            .unwrap()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_routed_policy_retains_semantic_prefixes_and_outer_polarity() {
        let complete_epoch = routed_policy_epoch(
            Some((b"", PathNodeKind::RegularFile)),
            Some((b"", PathNodeKind::RegularFile)),
            90,
        );
        let policy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let policy_tracker = Arc::new(RoutedPolicyTracker::default());
        let policy = compute_observed_routed_policy(
            &policy_dice,
            &policy_tracker,
            complete_epoch.dupe(),
            false,
        )
        .await;
        let SourcePreparationOutcome::Complete(Ok(policy)) = policy else {
            panic!("missing policy must be a completed semantic terminal");
        };
        assert!(matches!(
            policy.result().as_ref(),
            Err(super::HostRepositoryIgnoreError::RouteRepoFile(
                crate::repo_file::HostRouteRepoFileError::PolicyProjection(_)
            ))
        ));
        assert!(policy.observations().observations().is_empty());
        assert!(
            !policy_tracker
                .take()
                .iter()
                .any(|(key, _)| key.starts_with("observed-host-repository-source-file:"))
        );

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(RoutedPolicyTracker::default());
        let need =
            compute_observed_routed_policy(&dice, &tracker, PathObservationEpoch::empty(), true)
                .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(
            !super::HostRouteRepositoryIgnoreObservationKey::validity(&need)
                && !super::HostRouteRepositoryIgnoreObservationKey::equality(&need, &need)
        );
        let parse_epoch = routed_policy_epoch(
            Some((b"", PathNodeKind::RegularFile)),
            Some((b"/absolute\n", PathNodeKind::RegularFile)),
            92,
        );
        let parsed =
            compute_observed_routed_policy(&dice, &tracker, parse_epoch.dupe(), true).await;
        tracker.take();
        let SourcePreparationOutcome::Complete(Ok(parsed)) = parsed else {
            panic!("ignore parse failure must retain a carrier");
        };
        assert!(matches!(
            parsed.result().as_ref(),
            Err(super::HostRepositoryIgnoreError::InvalidAbsolute { .. })
        ));
        for (demand, result) in parse_epoch.observations() {
            assert!(Arc::ptr_eq(
                parsed.observations().get(demand).unwrap(),
                result
            ));
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_routed_policy_retains_exact_arcs_events_and_family_isolation() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(RoutedPolicyTracker::default());
        let epoch = routed_policy_epoch(
            Some((
                b"print('REPO')\nignore_directories(['repo/**'])\n",
                PathNodeKind::RegularFile,
            )),
            Some((b"ignored\n", PathNodeKind::RegularFile)),
            81,
        );
        let route = routed_policy_route();
        let mut transaction = routed_policy_transaction(
            &dice,
            tracker.dupe() as Arc<dyn ActivationTracker>,
            epoch.dupe(),
            true,
        )
        .await;
        let ignore_key = super::HostRouteRepositoryIgnoreObservationKey(
            super::HostRouteRepositoryIgnoreKey::new(route.clone()),
        );
        let ignore = transaction.compute(&ignore_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(ignore)) = &ignore else {
            panic!("observed routed ignore must complete");
        };
        let matcher = ignore.result().as_ref().as_ref().unwrap();
        assert_eq!(
            matcher.matching_entry(&PackagePath::parse("repo/child").unwrap()),
            Some("repo/**")
        );
        assert_eq!(
            matcher.matching_entry(&PackagePath::parse("ignored/child").unwrap()),
            Some("ignored")
        );
        assert_eq!(
            ignore.observations().observations().len(),
            epoch.observations().len()
        );
        for (demand, result) in epoch.observations() {
            assert!(Arc::ptr_eq(
                ignore.observations().get(demand).unwrap(),
                result
            ));
        }
        let cold = tracker.take();
        let repo_source = cold
            .iter()
            .position(|(key, _)| key.starts_with("observed-host-repository-source-file:"))
            .unwrap();
        let repo_parent = cold
            .iter()
            .position(|(key, _)| key.starts_with("observed-host-route-repo-file:"))
            .unwrap();
        let ignore_source = cold
            .iter()
            .rposition(|(key, _)| key.starts_with("observed-host-repository-source-file:"))
            .unwrap();
        assert!(repo_source < repo_parent && repo_parent < ignore_source);
        assert!(cold.iter().any(|(key, batch)| {
            key.starts_with("observed-host-route-repo-file:")
                && matches!(
                    batch.as_ref().map(EventBatch::events),
                    Some([EvaluationEvent::StarlarkPrint { text, .. }]) if text == "REPO"
                )
        }));
        assert!(cold.iter().all(|(key, batch)| {
            !key.starts_with("observed-host-route-repository-ignore:") || batch.is_none()
        }));
        assert!(!cold.iter().any(|(key, _)| {
            key.starts_with("host-route-")
                || key.starts_with("host-repository-source-file:")
                || key.starts_with("external-repository-package-lookup:")
        }));

        let warm = transaction.compute(&ignore_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(warm)) = &warm else {
            panic!("warm routed ignore must complete");
        };
        assert!(Arc::ptr_eq(ignore.result(), warm.result()));
        assert!(tracker.take().iter().all(|(_, batch)| batch.is_none()));

        let cancelled_tracker = Arc::new(RoutedPolicyTracker::default());
        let mut cancelled = routed_policy_transaction(
            &dice,
            cancelled_tracker.dupe() as Arc<dyn ActivationTracker>,
            PathObservationEpoch::empty(),
            true,
        )
        .await;
        let mut future = Box::pin(cancelled.compute(&ignore_key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        drop(cancelled);
        assert!(
            cancelled_tracker
                .take()
                .iter()
                .all(|(_, batch)| batch.is_none())
        );

        let recovered =
            compute_observed_routed_policy(&dice, &cancelled_tracker, epoch.dupe(), true).await;
        let SourcePreparationOutcome::Complete(Ok(recovered)) = &recovered else {
            panic!("successor transaction must recover");
        };
        assert!(Arc::ptr_eq(ignore.result(), recovered.result()));
        assert!(
            cancelled_tracker
                .take()
                .iter()
                .all(|(_, batch)| batch.is_none())
        );

        let legacy_tracker = Arc::new(RoutedPolicyTracker::default());
        let mut legacy = routed_policy_transaction(
            &dice,
            legacy_tracker.dupe() as Arc<dyn ActivationTracker>,
            epoch,
            true,
        )
        .await;
        let legacy_value = legacy
            .compute(&super::HostRouteRepositoryIgnoreKey::new(route))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(legacy_value) = legacy_value else {
            panic!("legacy routed ignore must complete");
        };
        assert_eq!(legacy_value.as_ref(), ignore.result().as_ref());
        let legacy_activations = legacy_tracker.take();
        assert!(
            !legacy_activations
                .iter()
                .any(|(key, _)| key.starts_with("observed-"))
        );
        let observed_events = cold
            .iter()
            .find(|(key, _)| key.starts_with("observed-host-route-repo-file:"))
            .and_then(|(_, batch)| batch.as_ref())
            .unwrap()
            .events();
        assert_eq!(
            legacy_activations
                .iter()
                .find(|(key, _)| key.starts_with("host-route-repo-file:"))
                .and_then(|(_, batch)| batch.as_ref())
                .unwrap()
                .events(),
            observed_events
        );
    }

    #[cfg(unix)]
    #[test]
    fn observed_ignore_union_retains_the_first_arc_and_rejects_conflicts() {
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            logical_path(),
            PathObservationOperation::Lstat,
        );
        let first = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let equal = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let left = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let right = PathObservationEpoch::from_shared([(demand.dupe(), equal)]).unwrap();
        let union = super::union_observations(&left, &right).unwrap();
        assert!(Arc::ptr_eq(union.get(&demand).unwrap(), &first));
        let stat = PathLstat::new(PathNodeKind::RegularFile, 1, 1, 1, 1, 0o644);
        let changed = PathObservationEpoch::new([(
            demand.dupe(),
            PathObservationResult::Lstat(PathOperationResult::Present(stat)),
        )])
        .unwrap();
        assert!(super::union_observations(&left, &changed).is_err());
        assert!(matches!(
            PathObservationEpoch::from_shared([(
                demand.dupe(),
                Arc::new(PathObservationResult::FileBytes(
                    PathOperationResult::Missing
                )),
            )]),
            Err(slug_workspace_v2::PathObservationEpochError::OperationMismatch { .. })
        ));
        let outer = SourcePreparationOutcome::Complete(Err(ObservedPathFrontierError::from(
            slug_workspace_v2::PathObservationEpochError::ConflictingDemand(demand),
        )));
        assert!(super::HostRouteRepositoryIgnoreObservationKey::validity(
            &outer
        ));
    }

    #[cfg(windows)]
    #[derive(Debug, Clone, PartialEq, Eq, Hash, allocative::Allocative)]
    struct ObservedWindowsParserNeedKey;

    #[cfg(windows)]
    impl std::fmt::Display for ObservedWindowsParserNeedKey {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("observed-windows-parser-need")
        }
    }

    #[cfg(windows)]
    #[async_trait::async_trait]
    impl dice::Key for ObservedWindowsParserNeedKey {
        type Value = slug_workspace_v2::PathOutcome<()>;

        async fn compute(
            &self,
            ctx: &mut dice::DiceComputations,
            _: &dice::CancellationContext,
        ) -> Self::Value {
            super::parse_ignore_file_observed(
                ctx,
                &NormalizedAbsolutePath::new(r"C:\workspace\.bazelignore").unwrap(),
                br"C:\PROGRA~1",
            )
            .await
            .map(|_| ())
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn observed_windows_parser_requests_long_path_before_completion() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                slug_workspace_v2::PathObservationEpochKey,
                PathObservationEpoch::empty(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let outcome = transaction
            .compute(&ObservedWindowsParserNeedKey)
            .await
            .unwrap();
        assert!(matches!(outcome, slug_workspace_v2::PathOutcome::Need(_)));
    }

    #[cfg(windows)]
    #[test]
    fn observed_windows_append_preserves_the_exact_path_observation_arc() {
        let demand = PathObservationDemand::windows_long_path(
            logical_path(),
            Arc::from(r"C:\PROGRA~1".encode_utf16().collect::<Vec<_>>()),
        );
        let result = Arc::new(PathObservationResult::WindowsLongPath(Arc::from(
            r"C:\Program Files".encode_utf16().collect::<Vec<_>>(),
        )));
        let epoch =
            super::append_observation(&PathObservationEpoch::empty(), demand.dupe(), result.dupe())
                .unwrap();
        assert!(Arc::ptr_eq(epoch.get(&demand).unwrap(), &result));
        let mismatch = Arc::new(PathObservationResult::Lstat(
            slug_workspace_v2::PathOperationResult::Missing,
        ));
        assert!(matches!(
            super::append_observation(&epoch, demand, mismatch),
            Err(slug_workspace_v2::ObservedPathFrontierError::Epoch(
                slug_workspace_v2::PathObservationEpochError::OperationMismatch { .. }
            ))
        ));
    }
    #[cfg(unix)]
    fn nonregistry_ignore_epoch(
        root: &str,
        files: [Option<(&[u8], PathNodeKind)>; 2],
        namespace: PathObservationNamespace,
        source_root: &str,
        variant: i64,
    ) -> PathObservationEpoch {
        let lstat = |kind| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, variant, variant, variant, variant, 0o755,
            )))
        };
        let demand = |namespace, path: &str, operation| {
            PathObservationDemand::new(
                namespace,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        };
        let mut entries = starlark_map::small_map::SmallMap::new();
        for path in ["/", "/workspace"] {
            entries.insert(
                demand(
                    PathObservationNamespace::Host,
                    path,
                    PathObservationOperation::Lstat,
                ),
                lstat(PathNodeKind::Directory),
            );
        }
        let module = "/workspace/MODULE.bazel";
        entries.insert(
            demand(
                PathObservationNamespace::Host,
                module,
                PathObservationOperation::Lstat,
            ),
            lstat(PathNodeKind::RegularFile),
        );
        entries.insert(
            demand(
                PathObservationNamespace::Host,
                module,
                PathObservationOperation::FileBytes,
            ),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                root.as_bytes(),
            ))),
        );
        for (name, source) in ["REPO.bazel", ".bazelignore"].into_iter().zip(files) {
            let path = format!("{source_root}/{name}");
            for ancestor in std::path::Path::new(&path).ancestors().skip(1) {
                entries.insert(
                    demand(
                        namespace,
                        ancestor.to_str().unwrap(),
                        PathObservationOperation::Lstat,
                    ),
                    lstat(PathNodeKind::Directory),
                );
            }
            entries.insert(
                demand(namespace, &path, PathObservationOperation::Lstat),
                source.map_or(
                    PathObservationResult::Lstat(PathOperationResult::Missing),
                    |(_, kind)| lstat(kind),
                ),
            );
            if let Some((bytes, PathNodeKind::RegularFile)) = source {
                entries.insert(
                    demand(namespace, &path, PathObservationOperation::FileBytes),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        bytes,
                    ))),
                );
            }
        }
        PathObservationEpoch::new(entries).unwrap()
    }

    #[cfg(unix)]
    fn without_ignore_bytes(epoch: &PathObservationEpoch) -> PathObservationEpoch {
        PathObservationEpoch::from_shared(epoch.observations().iter().filter_map(
            |(demand, result)| {
                (!(demand.path().as_path().ends_with(".bazelignore")
                    && demand.operation() == PathObservationOperation::FileBytes))
                    .then(|| (demand.dupe(), result.dupe()))
            },
        ))
        .unwrap()
    }

    #[cfg(unix)]
    fn ignore_read_error(epoch: &PathObservationEpoch) -> PathObservationEpoch {
        PathObservationEpoch::from_shared(epoch.observations().iter().map(|(demand, result)| {
            let result = if demand.path().as_path().ends_with(".bazelignore")
                && demand.operation() == PathObservationOperation::FileBytes
            {
                Arc::new(PathObservationResult::FileBytes(
                    PathOperationResult::Error(slug_workspace_v2::PathObservationError::Io {
                        kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
                        raw_os_error: None,
                    }),
                ))
            } else {
                result.dupe()
            };
            (demand.dupe(), result)
        }))
        .unwrap()
    }

    #[cfg(unix)]
    fn nonregistry_ignore_key() -> super::HostNonregistryRepositoryIgnoreKey {
        super::HostNonregistryRepositoryIgnoreKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            crate::NonrootModuleKey::new("dep", "1"),
        )
    }

    #[cfg(unix)]
    async fn nonregistry_ignore_transaction(
        dice: &Arc<Dice>,
        tracker: Arc<RoutedPolicyTracker>,
        root: &str,
        epoch: PathObservationEpoch,
        result: Option<RepositoryMaterializationResult>,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let mut data = UserComputationData {
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel"),
                        slug_workspace_v2::WorkspaceFileValue::Present(Arc::new(root.to_owned())),
                    )])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Off,
        )
        .unwrap();
        inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                workspace.dupe(),
                [workspace.dupe()],
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(workspace.dupe(), []).unwrap(),
            )])
            .unwrap();
        let mut tx = updater.commit().await;
        let pending = tx
            .compute(&super::HostNonregistryRepositoryIgnoreObservationKey(
                nonregistry_ignore_key(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Need(need) = pending else {
            panic!("missing materialization result must remain a parent Need");
        };
        let request = need
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .dupe();
        let mut updater = tx.into_updater();
        let Some(result) = result else {
            return updater.commit().await;
        };
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace,
                    [RepositoryMaterializationEpochEntry { request, result }],
                )
                .unwrap(),
            )])
            .unwrap();
        updater.commit().await
    }

    #[cfg(unix)]
    async fn nonregistry_ignore_child_epoch(
        tx: &mut dice::DiceTransaction,
        include_source: bool,
    ) -> PathObservationEpoch {
        let repo_key = crate::repo_file::HostNonregistryRepoFileObservationKey(
            crate::repo_file::HostNonregistryRepoFileKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                crate::NonrootModuleKey::new("dep", "1"),
            ),
        );
        let SourcePreparationOutcome::Complete(Ok(repo)) = tx.compute(&repo_key).await.unwrap()
        else {
            panic!("observed REPO child must complete");
        };
        if !include_source {
            return repo.observations().dupe();
        }
        let source_key =
            super::RepositorySourceFileObservationKey(super::RepositorySourceFileKey {
                workspace: "/workspace".into(),
                module_name: "dep".into(),
                repo_relative_path: ".bazelignore".into(),
            });
        let source_outcome = tx.compute(&source_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(source)) = source_outcome else {
            panic!("observed source child must complete: {source_outcome:?}");
        };
        super::union_observations(repo.observations(), source.observations()).unwrap()
    }

    #[cfg(unix)]
    fn assert_nonregistry_ignore_epoch(
        expected: &PathObservationEpoch,
        actual: &PathObservationEpoch,
    ) {
        assert_eq!(expected.observations().len(), actual.observations().len());
        for ((expected_demand, expected_result), (actual_demand, actual_result)) in
            expected.observations().iter().zip(actual.observations())
        {
            assert_eq!(expected_demand, actual_demand);
            assert!(Arc::ptr_eq(expected_result, actual_result));
        }
    }

    #[cfg(unix)]
    fn observed_nonregistry_ignore(
        value: &<super::HostNonregistryRepositoryIgnoreObservationKey as Key>::Value,
    ) -> &super::ObservedHostNonregistryRepositoryIgnore {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            _ => panic!("observed nonregistry ignore did not complete: {value:?}"),
        }
    }

    #[cfg(unix)]
    async fn ignore_lifecycle(
        root: &str,
        namespace: PathObservationNamespace,
        source_root: &str,
        result: RepositoryMaterializationResult,
        change_repo: bool,
    ) {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(RoutedPolicyTracker::default());
        let key = super::HostNonregistryRepositoryIgnoreObservationKey(nonregistry_ignore_key());
        let ignore_states = [
            Some((b"a\n".as_slice(), PathNodeKind::RegularFile)),
            Some((b"b\n".as_slice(), PathNodeKind::RegularFile)),
            None,
            Some((b"".as_slice(), PathNodeKind::Directory)),
            Some((b"a\n".as_slice(), PathNodeKind::RegularFile)),
        ];
        let repo_states = [
            Some((b"print('A')\n".as_slice(), PathNodeKind::RegularFile)),
            Some((b"print('B')\n".as_slice(), PathNodeKind::RegularFile)),
            None,
            Some((b"".as_slice(), PathNodeKind::Directory)),
            Some((b"print('A')\n".as_slice(), PathNodeKind::RegularFile)),
        ];
        let (mut values, mut held) = (Vec::new(), None);
        for index in 0..5 {
            let epoch = nonregistry_ignore_epoch(
                root,
                if change_repo {
                    [
                        repo_states[index],
                        Some((b"ignored\n".as_slice(), PathNodeKind::RegularFile)),
                    ]
                } else {
                    [
                        Some((
                            b"ignore_directories(['repo/**'])\n".as_slice(),
                            PathNodeKind::RegularFile,
                        )),
                        ignore_states[index],
                    ]
                },
                namespace,
                source_root,
                [70, 71, 72, 73, 70][index],
            );
            let mut tx = nonregistry_ignore_transaction(
                &dice,
                tracker.dupe(),
                root,
                epoch.dupe(),
                Some(result.clone()),
            )
            .await;
            tracker.clear();
            let expected =
                nonregistry_ignore_child_epoch(&mut tx, !(change_repo && index == 3)).await;
            let value = tx.compute(&key).await.unwrap();
            let observed = observed_nonregistry_ignore(&value);
            assert_nonregistry_ignore_epoch(&expected, observed.observations());
            held.get_or_insert_with(|| (observed.result().dupe(), observed.observations().dupe()));
            tracker.clear();
            values.push(value);
        }
        assert!(
            super::HostNonregistryRepositoryIgnoreObservationKey::equality(&values[0], &values[4])
                && !super::HostNonregistryRepositoryIgnoreObservationKey::equality(
                    &values[0], &values[1]
                )
        );
        let first = observed_nonregistry_ignore(&values[0]);
        let restored = observed_nonregistry_ignore(&values[4]);
        assert_eq!(first.result().as_ref(), restored.result().as_ref());
        let (held_result, held_epoch) = held.unwrap();
        assert!(Arc::ptr_eq(first.result(), &held_result));
        assert_eq!(held_result.as_ref(), restored.result().as_ref());
        assert_eq!(first.observations(), &held_epoch);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn nonregistry_ignore_terminal_prefixes_and_outer_are_exact() {
        const LOCAL: &str =
            "module(name='root')\nlocal_path_override(module_name='dep',path='dep')\n";
        let key = super::HostNonregistryRepositoryIgnoreObservationKey(nonregistry_ignore_key());
        let local =
            RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local);
        let valid = nonregistry_ignore_epoch(
            LOCAL,
            [
                Some((
                    b"ignore_directories(['repo/**'])\n".as_slice(),
                    PathNodeKind::RegularFile,
                )),
                Some((b"ignored\n".as_slice(), PathNodeKind::RegularFile)),
            ],
            PathObservationNamespace::Host,
            "/workspace/dep",
            80,
        );
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(RoutedPolicyTracker::default());
        let mut tx = nonregistry_ignore_transaction(
            &dice,
            tracker.dupe(),
            LOCAL,
            without_ignore_bytes(&valid),
            Some(local.clone()),
        )
        .await;
        tracker.clear();
        let need = tx.compute(&key).await.unwrap();
        assert!(
            matches!(need, SourcePreparationOutcome::Need(_))
                && !super::HostNonregistryRepositoryIgnoreObservationKey::validity(&need)
                && !super::HostNonregistryRepositoryIgnoreObservationKey::equality(&need, &need)
        );
        assert_row(&tracker, &key, OBS_DEPS);
        assert!(tracker.take().iter().all(|(name, batch)| {
            !(name.starts_with("observed-repository-source-file:")
                || name.starts_with("observed-host-nonregistry-repository-ignore:"))
                || batch.is_none()
        }));

        let cases = [
            (
                [
                    Some((b"fail('repo')\n".as_slice(), PathNodeKind::RegularFile)),
                    Some((b"ignored\n".as_slice(), PathNodeKind::RegularFile)),
                ],
                false,
            ),
            (
                [
                    Some((
                        b"ignore_directories(['repo/**'])\n".as_slice(),
                        PathNodeKind::RegularFile,
                    )),
                    Some((b"".as_slice(), PathNodeKind::RegularFile)),
                ],
                true,
            ),
            (
                [
                    Some((
                        b"ignore_directories(['repo/**'])\n".as_slice(),
                        PathNodeKind::RegularFile,
                    )),
                    Some((b"/absolute\n".as_slice(), PathNodeKind::RegularFile)),
                ],
                true,
            ),
        ];
        for (index, (files, include_source)) in cases.into_iter().enumerate() {
            let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let tracker = Arc::new(RoutedPolicyTracker::default());
            let epoch = nonregistry_ignore_epoch(
                LOCAL,
                files,
                PathObservationNamespace::Host,
                "/workspace/dep",
                81 + index as i64,
            );
            let epoch = if index == 1 {
                ignore_read_error(&epoch)
            } else {
                epoch
            };
            let mut tx = nonregistry_ignore_transaction(
                &dice,
                tracker.dupe(),
                LOCAL,
                epoch,
                Some(local.clone()),
            )
            .await;
            tracker.clear();
            let expected = nonregistry_ignore_child_epoch(&mut tx, include_source).await;
            tracker.clear();
            let value = tx.compute(&key).await.unwrap();
            let observed = observed_nonregistry_ignore(&value);
            assert_nonregistry_ignore_epoch(&expected, observed.observations());
            let error = observed.result().as_ref().as_ref().unwrap_err();
            assert!(
                [
                    matches!(
                        error,
                        super::HostRepositoryIgnoreError::NonregistryRepoFile(_)
                    ),
                    matches!(
                        error,
                        super::HostRepositoryIgnoreError::RepositorySource(
                            super::RepositorySourceFileError::Observation { .. }
                        )
                    ),
                    matches!(
                        error,
                        super::HostRepositoryIgnoreError::InvalidAbsolute { .. }
                    ),
                ][index]
            );
            let exact = [
                "error evaluating REPO.bazel file at \"/workspace/.slug-nonregistry/dep/REPO.bazel\": Traceback (most recent call last):\n  * /workspace/.slug-nonregistry/dep/REPO.bazel:1, in <module>\n      fail('repo')\nerror: fail: repo\n --> /workspace/.slug-nonregistry/dep/REPO.bazel:1:1\n  |\n1 | fail('repo')\n  | ^^^^^^^^^^^^\n  |\n",
                "failed to read routed .bazelignore: Observation { repo_relative_path: \".bazelignore\", operation: FileBytes, error: Io { kind: PermissionDenied, raw_os_error: None } }",
                "Invalid path in /workspace/.slug-nonregistry/dep/.bazelignore: '/absolute': cannot be an absolute path",
            ][index];
            assert_eq!(error.to_string(), exact);
            let mut expected_deps = vec![
                "observed-host-nonregistry-repo-file:dep@1".to_owned(),
                "observed-repository-source-file:dep:.bazelignore".to_owned(),
            ];
            expected_deps.truncate(1 + usize::from(include_source));
            assert_eq!(tracker.take_rows(), [(key.to_string(), expected_deps)]);
            assert!(tracker.take().iter().all(|(name, batch)| {
                !(name.starts_with("observed-repository-source-file:")
                    || name.starts_with("observed-host-nonregistry-repository-ignore:"))
                    || batch.is_none()
            }));
            let SourcePreparationOutcome::Complete(legacy) =
                tx.compute(&nonregistry_ignore_key()).await.unwrap()
            else {
                panic!("legacy terminal must complete");
            };
            assert_eq!(legacy.as_ref(), observed.result().as_ref());
        }

        let mismatch = ObservedPathFrontierError::from(
            slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                demand: PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    logical_path(),
                    PathObservationOperation::Lstat,
                ),
                result_operation: PathObservationOperation::FileBytes,
            },
        );
        let projected =
            super::finish_observed_nonregistry_ignore_complete::<()>(Err(mismatch.dupe()))
                .unwrap_err();
        assert!(matches!(
            projected,
            SourcePreparationOutcome::Complete(Err(_))
        ));
        let outer = SourcePreparationOutcome::Complete(Err(mismatch));
        assert!(
            super::HostNonregistryRepositoryIgnoreObservationKey::validity(&outer)
                && super::HostNonregistryRepositoryIgnoreObservationKey::equality(&outer, &outer)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn nonregistry_ignore_stops_events_families_and_lifecycles_are_exact() {
        const LOCAL: &str =
            "module(name='root')\nlocal_path_override(module_name='dep',path='dep')\n";
        let epoch = nonregistry_ignore_epoch(
            LOCAL,
            [
                Some((b"print('REPO')\n".as_slice(), PathNodeKind::RegularFile)),
                Some((b"ignored\n".as_slice(), PathNodeKind::RegularFile)),
            ],
            PathObservationNamespace::Host,
            "/workspace/dep",
            60,
        );
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(RoutedPolicyTracker::default());
        let mut need_tx =
            nonregistry_ignore_transaction(&dice, tracker.dupe(), LOCAL, epoch.dupe(), None).await;
        tracker.clear();
        let key = super::HostNonregistryRepositoryIgnoreObservationKey(nonregistry_ignore_key());
        assert_eq!(
            key.to_string(),
            "observed-host-nonregistry-repository-ignore:dep@1"
        );
        let mut identities = std::collections::HashSet::from([key.clone()]);
        assert!(!identities.insert(key.clone()));
        let mut cancelled = Box::pin(need_tx.compute(&key));
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(cancelled.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(cancelled);
        let need = need_tx.compute(&key).await.unwrap();
        assert!(
            !super::HostNonregistryRepositoryIgnoreObservationKey::validity(&need)
                && !super::HostNonregistryRepositoryIgnoreObservationKey::equality(&need, &need)
        );
        assert_row(&tracker, &key, &OBS_DEPS[..1]);
        assert!(tracker.take().iter().all(|(name, batch)| {
            !name.starts_with("observed-host-nonregistry-repository-ignore:") || batch.is_none()
        }));

        let mut tx = nonregistry_ignore_transaction(
            &dice,
            tracker.dupe(),
            LOCAL,
            epoch.dupe(),
            Some(RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Local,
            )),
        )
        .await;
        tracker.clear();
        let expected = nonregistry_ignore_child_epoch(&mut tx, true).await;
        let value = tx.compute(&key).await.unwrap();
        let observed = observed_nonregistry_ignore(&value);
        assert_nonregistry_ignore_epoch(&expected, observed.observations());
        assert_row(&tracker, &key, OBS_DEPS);
        let cold = tracker.take();
        let observed_batches = cold
            .iter()
            .filter(|(name, _)| name.starts_with("observed-host-nonregistry-repo-file:"))
            .filter_map(|(_, batch)| batch.as_ref())
            .collect::<Vec<_>>();
        assert!(matches!(
            observed_batches.as_slice(),
            [batch]
                if matches!(batch.events(), [EvaluationEvent::StarlarkPrint { text, .. }] if text == "REPO")
        ));
        assert!(cold.iter().all(|(name, batch)| {
            !(name.starts_with("observed-repository-source-file:")
                || name.starts_with("observed-host-nonregistry-repository-ignore:"))
                || batch.is_none()
        }));
        assert!(cold.iter().all(|(name, _)| {
            [
                "host-nonregistry-package-preflight:",
                "host-nonregistry-module-closure:",
                "module-source-preparation:",
                "host-discovered-module:",
                "host-selected-module-graph:",
                "registry-file:",
                "host-selected-extension-",
            ]
            .iter()
            .all(|upper| !name.contains(upper))
        }));
        let warm = tx.compute(&key).await.unwrap();
        assert!(Arc::ptr_eq(
            observed_nonregistry_ignore(&warm).result(),
            observed.result()
        ));
        assert!(tracker.take().iter().all(|(_, batch)| batch.is_none()));
        assert!(tracker.take_rows().is_empty());
        let legacy_key = nonregistry_ignore_key();
        let SourcePreparationOutcome::Complete(legacy) = tx.compute(&legacy_key).await.unwrap()
        else {
            panic!("legacy ignore must complete");
        };
        assert_eq!(legacy.as_ref(), observed.result().as_ref());
        assert_row(&tracker, &legacy_key, LEG_DEPS);
        let legacy_events = tracker.take();
        let legacy_batches = legacy_events
            .iter()
            .filter(|(name, _)| name.starts_with("host-nonregistry-repo-file:"))
            .filter_map(|(_, batch)| batch.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(legacy_batches, observed_batches);

        let local =
            RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local);
        for change_repo in [false, true] {
            ignore_lifecycle(
                LOCAL,
                PathObservationNamespace::Host,
                "/workspace/dep",
                local.clone(),
                change_repo,
            )
            .await;
        }
        let instance = slug_workspace_v2::PathObservationInstanceId::new(9);
        let immutable =
            RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
                source_identity: Arc::from("sha256-x"),
                generation_root: "/immutable/9".into(),
                observation_instance: instance,
            });
        for change_repo in [false, true] {
            ignore_lifecycle(
                "module(name='root')\narchive_override(module_name='dep',urls=['https://example.invalid/a.tgz'],integrity='sha256-x')\n",
                PathObservationNamespace::Materialization(instance),
                "/immutable/9",
                immutable.clone(),
                change_repo,
            )
            .await;
        }
    }
}
