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
use slug_workspace_v2::PathNodeKind;
#[cfg(windows)]
use slug_workspace_v2::PathObservationDemand;
#[cfg(windows)]
use slug_workspace_v2::PathObservationKey;
#[cfg(windows)]
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOutcome;

use crate::RootPackagePolicyProjectionError;
use crate::RootRepositoryIgnoreInputsProjectionKey;
use crate::host_file::HostFileBytes;
use crate::host_file::HostFileBytesKey;
use crate::host_file::HostFileError;
use crate::repo_file::HostRepoFileError;
use crate::repo_file::HostRepoFileKey;
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
    PolicyProjection(RootPackagePolicyProjectionError),
    HostFile(HostFileError),
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
            Self::PolicyProjection(error) => error.fmt(f),
            Self::HostFile(error) => write!(f, "failed to read .bazelignore: {error:?}"),
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

async fn parse_ignore_file(
    _ctx: &mut DiceComputations<'_>,
    logical_path: &NormalizedAbsolutePath,
    bytes: &[u8],
) -> PathOutcome<Result<Vec<RepositoryIgnorePrefix>, HostRepositoryIgnoreError>> {
    let flavor = IgnorePathFlavor::native();
    let prepared = match prepare_ignore_file(bytes, flavor, logical_path) {
        Ok(prepared) => prepared,
        Err(error) => return PathOutcome::Complete(Err(error)),
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
            match dice_invariant(_ctx.compute(&PathObservationKey::new(demand)).await) {
                PathOutcome::Need(need) => return PathOutcome::Need(need),
                PathOutcome::Complete(result) => match result.as_ref() {
                    PathObservationResult::WindowsLongPath(value) => value.dupe(),
                    PathObservationResult::Lstat(_)
                    | PathObservationResult::ReadLink(_)
                    | PathObservationResult::FileBytes(_)
                    | PathObservationResult::DirectoryEntries(_) => {
                        unreachable!(
                            "WindowsLongPath demand must return a WindowsLongPath observation"
                        )
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
        return PathOutcome::Complete(Err(HostRepositoryIgnoreError::InvalidAbsolute {
            logical_path: logical_path.dupe(),
            normalized,
        }));
    }
    PathOutcome::Complete(Ok(prefixes))
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
    #[cfg(unix)]
    use std::sync::Arc;

    use compact_str::CompactString;
    #[cfg(unix)]
    use dice::DetectCycles;
    #[cfg(unix)]
    use dice::Dice;
    #[cfg(unix)]
    use dice::Key;
    #[cfg(unix)]
    use dupe::Dupe;
    use slug_identity_v2::PackagePath;
    use slug_workspace_v2::NormalizedAbsolutePath;
    #[cfg(unix)]
    use slug_workspace_v2::PathLstat;
    #[cfg(unix)]
    use slug_workspace_v2::PathNodeKind;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationDemand;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpoch;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpochKey;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationNamespace;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationOperation;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationResult;
    #[cfg(unix)]
    use slug_workspace_v2::PathOperationResult;
    #[cfg(unix)]
    use slug_workspace_v2::PathOutcome;

    #[cfg(unix)]
    use super::HostRepositoryIgnoreKey;
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
    use crate::inject_root_package_policy_inputs;
    #[cfg(unix)]
    use crate::repo_file::HostRepoFileKey;

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
        let dice = Dice::builder().build(DetectCycles::Enabled);
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
}
