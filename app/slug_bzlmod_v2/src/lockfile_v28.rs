/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory.
 */

//! Bazel 9.2 lockfile-v28 semantic owner.

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use base64::Engine;
use base64::alphabet;
use base64::engine::DecodePaddingMode;
use base64::engine::general_purpose::GeneralPurpose;
use base64::engine::general_purpose::GeneralPurposeConfig;
use base64::engine::general_purpose::STANDARD as STANDARD_BASE64;
use compact_str::CompactString;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;
use starlark_map::sorted_map::SortedMap;

use crate::module_version::BazelModuleVersion;

pub(crate) const LOCK_FILE_VERSION_28: i32 = 28;
pub(crate) const REGISTRY_FILE_NOT_FOUND_V28: &str = "not found";
pub(crate) const MAX_FACT_NESTING_DEPTH: u8 = 7;
const LENIENT_STANDARD_BASE64: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);

/// The complete immutable semantic value read from a Bazel 9.2 lockfile.
///
/// Structural `PartialEq` is deliberately not derived: Bazel equality is
/// order-independent for maps and sets, ordered for lists, and uses Starlark
/// numeric equality for Facts.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct BazelLockfile {
    pub(crate) lock_file_version: i32,
    pub(crate) registry_file_hashes: SortedMap<CompactString, RegistryFileHash>,
    pub(crate) selected_yanked_versions: SortedMap<LockfileModuleKey, CompactString>,
    pub(crate) module_extensions: SortedMap<
        ModuleExtensionId,
        SortedMap<ModuleExtensionEvalFactors, LockfileModuleExtension>,
    >,
    pub(crate) facts: SortedMap<ModuleExtensionId, Facts>,
    pub(crate) facts_versions: SortedMap<ModuleExtensionId, i32>,
}

#[cfg(test)]
pub(crate) use BazelLockfile as BazelLockfileV28;

impl Default for BazelLockfile {
    fn default() -> Self {
        Self {
            lock_file_version: LOCK_FILE_VERSION_28,
            registry_file_hashes: SortedMap::new(),
            selected_yanked_versions: SortedMap::new(),
            module_extensions: SortedMap::new(),
            facts: SortedMap::new(),
            facts_versions: SortedMap::new(),
        }
    }
}

impl BazelLockfile {
    /// Bazel semantic equality, independent of non-semantic map insertion
    /// order and spelling while retaining ordered list identity.
    pub fn semantically_eq(&self, other: &Self) -> bool {
        self == other
    }

    pub fn lock_file_version(&self) -> i32 {
        self.lock_file_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum RegistryFileHash {
    NotFound,
    Sha256([u8; 32]),
}

#[derive(Debug, Clone, Allocative)]
pub(crate) enum LockfileModuleKey {
    Root,
    Module {
        name: CompactString,
        version: BazelModuleVersion,
    },
}

impl PartialEq for LockfileModuleKey {
    fn eq(&self, other: &Self) -> bool {
        module_key_components(self) == module_key_components(other)
    }
}

impl Eq for LockfileModuleKey {}

impl Hash for LockfileModuleKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        module_key_components(self).hash(state);
    }
}

impl PartialOrd for LockfileModuleKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LockfileModuleKey {
    fn cmp(&self, other: &Self) -> Ordering {
        let (left_name, _) = module_key_components(self);
        let (right_name, _) = module_key_components(other);
        left_name
            .cmp(right_name)
            .then_with(|| compare_module_key_versions(self, other))
    }
}

#[derive(Debug, Clone, Allocative)]
pub(crate) struct LockfileCanonicalLabel {
    /// Canonical adapter spelling, including the root-label shorthand Bazel
    /// emits in extension IDs.
    pub(crate) canonical: CompactString,
}

impl PartialEq for LockfileCanonicalLabel {
    fn eq(&self, other: &Self) -> bool {
        canonical_label_components(&self.canonical) == canonical_label_components(&other.canonical)
    }
}

impl Eq for LockfileCanonicalLabel {}

impl Hash for LockfileCanonicalLabel {
    fn hash<H: Hasher>(&self, state: &mut H) {
        canonical_label_components(&self.canonical).hash(state);
    }
}

impl PartialOrd for LockfileCanonicalLabel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LockfileCanonicalLabel {
    fn cmp(&self, other: &Self) -> Ordering {
        let left = canonical_label_components(&self.canonical);
        let right = canonical_label_components(&other.canonical);
        left.cmp(&right)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub(crate) struct ModuleExtensionId {
    pub(crate) bzl_file: LockfileCanonicalLabel,
    pub(crate) extension_name: CompactString,
    pub(crate) isolation_key: Option<ModuleExtensionIsolationKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub(crate) struct ModuleExtensionIsolationKey {
    pub(crate) module: LockfileModuleKey,
    pub(crate) usage_name: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct ModuleExtensionEvalFactors {
    pub(crate) operating_system: Option<CompactString>,
    pub(crate) architecture: Option<CompactString>,
}

impl PartialOrd for ModuleExtensionEvalFactors {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ModuleExtensionEvalFactors {
    fn cmp(&self, other: &Self) -> Ordering {
        render_factors(self).cmp(&render_factors(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct LockfileModuleExtension {
    pub(crate) bzl_transitive_digest: Arc<[u8]>,
    pub(crate) usages_digest: Arc<[u8]>,
    pub(crate) recorded_inputs: Arc<[RecordedInput]>,
    /// Producer insertion order is retained for canonical rendering.
    pub(crate) generated_repo_specs: SmallMap<CompactString, LockfileRepoSpec>,
    pub(crate) metadata: Option<LockfileModuleExtensionMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct LockfileModuleExtensionMetadata {
    /// `None` is JSON null; `Some(empty)` is an explicitly empty set.
    pub(crate) explicit_root_module_direct_deps: Option<SmallSet<CompactString>>,
    /// `None` is JSON null; `Some(empty)` is an explicitly empty set.
    pub(crate) explicit_root_module_direct_dev_deps: Option<SmallSet<CompactString>>,
    pub(crate) use_all_repos: UseAllRepos,
    pub(crate) reproducible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum UseAllRepos {
    No,
    Regular,
    Dev,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct RecordedInput {
    pub(crate) key: RecordedInputKey,
    /// Adapter values are nullable.
    pub(crate) value: Option<CompactString>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum RecordedInputKey {
    File(RecordedPath),
    DirectoryEntries(RecordedPath),
    DirectoryTree {
        path: RecordedPath,
        excludes: Arc<[CompactString]>,
    },
    Environment(CompactString),
    RepositoryMapping {
        /// Bazel does not validate recorded repository names here.
        source_repository: CompactString,
        apparent_name: CompactString,
    },
    /// All malformed and unknown keys normalize to this one sentinel.
    ParseFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct RecordedPath {
    pub(crate) canonical: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct LockfileRepoSpec {
    pub(crate) repo_rule_id: Option<LockfileRepoRuleId>,
    pub(crate) attributes: Option<AttributeValues>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct LockfileRepoRuleId {
    /// No-percent input retains a null label and the whole input as the name.
    pub(crate) bzl_file: Option<LockfileCanonicalLabel>,
    pub(crate) rule_name: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct AttributeValues {
    /// Adapter dictionary insertion order is retained for rendering.
    pub(crate) values: SmallMap<CompactString, AttributeValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum AttributeValue {
    None,
    Bool(bool),
    /// Gson `getAsInt()` narrowing is completed before retention.
    Int(i32),
    String(CompactString),
    Label(LockfileCanonicalLabel),
    Sequence(Arc<[AttributeValue]>),
    Dict(SmallMap<AttributeKey, AttributeValue>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum AttributeKey {
    String(CompactString),
    Label(LockfileCanonicalLabel),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct Facts {
    /// Facts always have an object root and every dictionary is recursively
    /// normalized into key order.
    pub(crate) values: SortedMap<CompactString, FactValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum FactValue {
    Null,
    Bool(bool),
    Number(FactNumber),
    String(CompactString),
    List(Arc<[FactValue]>),
    Dict(SortedMap<CompactString, FactValue>),
}

#[derive(Debug, Clone, Allocative)]
pub(crate) enum FactNumber {
    /// Canonical arbitrary-size signed decimal spelling.
    Integer(CompactString),
    /// Construction rejects NaN and infinities. Equality must treat negative
    /// zero and integer/float pairs according to Starlark.
    FiniteFloat(f64),
}

impl PartialEq for FactNumber {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::FiniteFloat(a), Self::FiniteFloat(b)) => a == b,
            (Self::Integer(integer), Self::FiniteFloat(float))
            | (Self::FiniteFloat(float), Self::Integer(integer)) => {
                integer_equals_finite_float(integer, *float)
            }
        }
    }
}

impl Eq for FactNumber {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub(crate) enum UnsupportedVersionPolicy {
    ReturnEmpty,
    Error,
}

#[derive(Debug, Clone, Allocative)]
pub(crate) enum LockfileReadOutcome {
    Empty,
    Parsed(BazelLockfile),
}

/// Decode Java-compatible UTF-8 replacement, apply the first textual version
/// marker gate, and parse a recognized v28 object.
pub(crate) fn read_lockfile_v28(
    bytes: &[u8],
    unsupported_version_policy: UnsupportedVersionPolicy,
) -> Result<LockfileReadOutcome, LockfileParseError> {
    let source = java_utf8_decode(bytes);
    let marker = scan_version_marker(&source)?;
    if marker.map(|(value, _)| value) != Some(LOCK_FILE_VERSION_28) {
        return match unsupported_version_policy {
            UnsupportedVersionPolicy::ReturnEmpty => Ok(LockfileReadOutcome::Empty),
            UnsupportedVersionPolicy::Error => Err(parse_error(
                LockfileParseErrorSurface::UnsupportedVersion,
                LockfileParseErrorKind::UnsupportedVersion {
                    found: marker.map(|(_, spelling)| CompactString::from(spelling)),
                },
                None,
                "unsupported or missing lockfile version",
            )),
        };
    }
    let mut tokenizer = GsonTokenizer::new(&source);
    let mut reader = TokenReader::new(&mut tokenizer);
    let value = stream_parse_lockfile(&mut reader)?;
    if reader.next()?.is_some() {
        return Err(syntax_error(reader.position(), "trailing JSON content"));
    }
    Ok(LockfileReadOutcome::Parsed(value))
}

/// Render all six top-level fields in Bazel/Gson order and formatting.
pub(crate) fn render_lockfile_v28(
    lockfile: &BazelLockfile,
) -> Result<CompactString, LockfileRenderError> {
    let mut writer = PrettyWriter::new();
    stream_render_lockfile(&mut writer, lockfile)?;
    Ok(writer.finish())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub struct SourcePosition {
    pub(crate) byte_offset: usize,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

impl SourcePosition {
    pub fn byte_offset(self) -> usize {
        self.byte_offset
    }

    pub fn line(self) -> usize {
        self.line
    }

    pub fn column(self) -> usize {
        self.column
    }
}

/// Non-retained tokens from the Gson-compatible reader.
#[derive(Debug, Clone, PartialEq, Allocative)]
pub(crate) enum GsonToken {
    BeginObject,
    EndObject,
    BeginArray,
    EndArray,
    Colon,
    Comma,
    String(GsonString),
    Number(CompactString),
    Boolean(bool),
    Null,
}

/// A transient decoded Gson string plus the UTF-8 byte offsets at which lone
/// Java UTF-16 surrogates were replaced. Typed adapters consume this metadata
/// before any string reaches the retained lockfile value.
#[derive(Debug, Clone, PartialEq, Allocative)]
pub(crate) struct GsonString {
    value: CompactString,
    lone_surrogate_offsets: Arc<[u32]>,
}

impl GsonString {
    fn normalize(self, domain: AdapterDomain) -> CompactString {
        if domain == AdapterDomain::Facts || self.lone_surrogate_offsets.is_empty() {
            return self.value;
        }
        let value = self.value.as_str();
        let mut normalized = String::with_capacity(
            value.len()
                - self.lone_surrogate_offsets.len() * ('\u{fffd}'.len_utf8() - '?'.len_utf8()),
        );
        let mut start = 0;
        for offset in self.lone_surrogate_offsets.iter().copied() {
            let offset = offset as usize;
            normalized.push_str(&value[start..offset]);
            normalized.push('?');
            start = offset + '\u{fffd}'.len_utf8();
        }
        normalized.push_str(&value[start..]);
        normalized.into()
    }
}

impl PartialEq<&str> for GsonString {
    fn eq(&self, other: &&str) -> bool {
        self.value == *other
    }
}

/// Private lenient token stream. It must preserve duplicate encounter order,
/// nulls, and raw number spelling; typed owners consume and discard tokens.
#[derive(Debug)]
pub(crate) struct GsonTokenizer<'a> {
    source: &'a str,
    offset: usize,
    line: usize,
    column: usize,
}

impl<'a> GsonTokenizer<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            source,
            offset: 0,
            line: 1,
            column: 1,
        }
    }

    pub(crate) fn position(&self) -> SourcePosition {
        SourcePosition {
            byte_offset: self.offset,
            line: self.line,
            column: self.column,
        }
    }

    pub(crate) fn next_token(
        &mut self,
    ) -> Result<Option<(SourcePosition, GsonToken)>, LockfileParseError> {
        self.skip_ignored()?;
        if self.offset == self.source.len() {
            return Ok(None);
        }
        let position = self.position();
        let byte = self.source.as_bytes()[self.offset];
        let token = match byte {
            b'{' => {
                self.advance_ascii(1);
                GsonToken::BeginObject
            }
            b'}' => {
                self.advance_ascii(1);
                GsonToken::EndObject
            }
            b'[' => {
                self.advance_ascii(1);
                GsonToken::BeginArray
            }
            b']' => {
                self.advance_ascii(1);
                GsonToken::EndArray
            }
            b':' => {
                self.advance_ascii(1);
                GsonToken::Colon
            }
            b'=' => {
                self.advance_ascii(1);
                if self.source.as_bytes().get(self.offset) == Some(&b'>') {
                    self.advance_ascii(1);
                }
                GsonToken::Colon
            }
            b',' | b';' => {
                self.advance_ascii(1);
                GsonToken::Comma
            }
            b'"' | b'\'' => GsonToken::String(self.read_quoted(byte)?),
            _ => {
                let literal = self.read_unquoted()?;
                if literal.eq_ignore_ascii_case("null") {
                    GsonToken::Null
                } else if literal.eq_ignore_ascii_case("true") {
                    GsonToken::Boolean(true)
                } else if literal.eq_ignore_ascii_case("false") {
                    GsonToken::Boolean(false)
                } else if looks_like_number(&literal) {
                    GsonToken::Number(literal)
                } else {
                    GsonToken::String(GsonString {
                        value: literal,
                        lone_surrogate_offsets: Arc::from([]),
                    })
                }
            }
        };
        Ok(Some((position, token)))
    }

    #[allow(dead_code)]
    pub(crate) fn skip_value(&mut self) -> Result<(), LockfileParseError> {
        TokenReader::new(self).skip_value()
    }

    fn skip_ignored(&mut self) -> Result<(), LockfileParseError> {
        loop {
            while let Some(character) = self.remaining().chars().next() {
                if !character.is_whitespace() {
                    break;
                }
                self.advance_char(character);
            }
            let remaining = self.remaining();
            if remaining.starts_with("//") || remaining.starts_with('#') {
                while let Some(character) = self.remaining().chars().next() {
                    self.advance_char(character);
                    if character == '\n' {
                        break;
                    }
                }
            } else if remaining.starts_with("/*") {
                self.advance_ascii(2);
                let Some(end) = self.remaining().find("*/") else {
                    return Err(syntax_error(self.position(), "unterminated block comment"));
                };
                let consumed = self.remaining()[..end + 2].to_owned();
                for character in consumed.chars() {
                    self.advance_char(character);
                }
            } else {
                return Ok(());
            }
        }
    }

    fn read_quoted(&mut self, quote: u8) -> Result<GsonString, LockfileParseError> {
        self.advance_ascii(1);
        let mut output = String::new();
        let mut lone_surrogate_offsets = Vec::new();
        loop {
            let Some(character) = self.remaining().chars().next() else {
                return Err(syntax_error(self.position(), "unterminated string"));
            };
            self.advance_char(character);
            if character as u32 == quote as u32 {
                return Ok(GsonString {
                    value: output.into(),
                    lone_surrogate_offsets: lone_surrogate_offsets.into(),
                });
            }
            if character != '\\' {
                output.push(character);
                continue;
            }
            let Some(escape) = self.remaining().chars().next() else {
                return Err(syntax_error(self.position(), "unterminated escape"));
            };
            self.advance_char(escape);
            match escape {
                '"' => output.push('"'),
                '\'' => output.push('\''),
                '\\' => output.push('\\'),
                '/' => output.push('/'),
                'b' => output.push('\u{0008}'),
                'f' => output.push('\u{000c}'),
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                '\n' => {}
                'u' => {
                    let (character, lone_surrogate) = self.read_unicode_escape()?;
                    if lone_surrogate {
                        lone_surrogate_offsets.push(output.len() as u32);
                    }
                    output.push(character);
                }
                other => output.push(other),
            }
        }
    }

    fn read_unicode_escape(&mut self) -> Result<(char, bool), LockfileParseError> {
        let value = self.read_unicode_code_unit()?;
        let scalar = if (0xd800..=0xdbff).contains(&value) {
            if self.remaining().starts_with("\\u")
                && self
                    .peek_unicode_code_unit(2)
                    .is_some_and(|low| (0xdc00..=0xdfff).contains(&low))
            {
                self.advance_ascii(2);
                let low = self.read_unicode_code_unit()?;
                0x10000 + (((value as u32 - 0xd800) << 10) | (low as u32 - 0xdc00))
            } else {
                return Ok(('\u{fffd}', true));
            }
        } else if (0xdc00..=0xdfff).contains(&value) {
            return Ok(('\u{fffd}', true));
        } else {
            value as u32
        };
        char::from_u32(scalar)
            .map(|character| (character, false))
            .ok_or_else(|| syntax_error(self.position(), "invalid unicode scalar"))
    }

    fn read_unicode_code_unit(&mut self) -> Result<u16, LockfileParseError> {
        let bytes = self.remaining().as_bytes();
        if bytes.len() < 4 || !bytes[..4].iter().all(u8::is_ascii_hexdigit) {
            return Err(syntax_error(self.position(), "invalid unicode escape"));
        }
        let digits = std::str::from_utf8(&bytes[..4]).expect("ASCII hex digits");
        let value = u16::from_str_radix(digits, 16)
            .map_err(|_| syntax_error(self.position(), "invalid unicode escape"))?;
        self.advance_ascii(4);
        Ok(value)
    }

    fn peek_unicode_code_unit(&self, offset: usize) -> Option<u16> {
        let bytes = self.remaining().as_bytes().get(offset..offset + 4)?;
        if !bytes.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        let digits = std::str::from_utf8(bytes).expect("ASCII hex digits");
        u16::from_str_radix(digits, 16).ok()
    }

    fn read_unquoted(&mut self) -> Result<CompactString, LockfileParseError> {
        let start = self.offset;
        while let Some(character) = self.remaining().chars().next() {
            if character.is_whitespace()
                || matches!(
                    character,
                    '{' | '}' | '[' | ']' | ':' | ',' | ';' | '=' | '#' | '/' | '\\'
                )
            {
                break;
            }
            self.advance_char(character);
        }
        if start == self.offset {
            return Err(syntax_error(self.position(), "unexpected token"));
        }
        Ok(self.source[start..self.offset].into())
    }

    fn remaining(&self) -> &'a str {
        &self.source[self.offset..]
    }

    fn advance_ascii(&mut self, bytes: usize) {
        self.offset += bytes;
        self.column += bytes;
    }

    fn advance_char(&mut self, character: char) {
        self.offset += character.len_utf8();
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
    }
}

pub(crate) fn java_utf8_decode(bytes: &[u8]) -> String {
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

pub(crate) fn parse_module_key(
    spelling: CompactString,
) -> Result<LockfileModuleKey, LockfileParseError> {
    if spelling == "<root>" {
        return Ok(LockfileModuleKey::Root);
    }
    let mut parts = spelling.split('@');
    let name = parts.next().expect("split always has a first component");
    let Some(version_spelling) = parts.next() else {
        return Err(delimiter_error(AdapterDomain::ModuleKey));
    };
    let version = if version_spelling == "_" {
        BazelModuleVersion::empty()
    } else {
        parse_version(version_spelling)?
    };
    if name.is_empty() && version.is_empty() {
        return Ok(LockfileModuleKey::Root);
    }
    Ok(LockfileModuleKey::Module {
        name: name.into(),
        version,
    })
}

fn module_key_components(key: &LockfileModuleKey) -> (&str, &str) {
    match key {
        LockfileModuleKey::Root => ("", ""),
        LockfileModuleKey::Module { name, version } => (name, version.normalized()),
    }
}

fn parse_version(spelling: &str) -> Result<BazelModuleVersion, LockfileParseError> {
    BazelModuleVersion::parse(spelling)
        .map_err(|error| direct_adapter_error(AdapterDomain::Version, error.lockfile_message()))
}

fn compare_module_key_versions(left: &LockfileModuleKey, right: &LockfileModuleKey) -> Ordering {
    fn version(key: &LockfileModuleKey) -> Option<&BazelModuleVersion> {
        match key {
            LockfileModuleKey::Root => None,
            LockfileModuleKey::Module { version, .. } => Some(version),
        }
    }
    match (version(left), version(right)) {
        (Some(left), Some(right)) => left.cmp(right),
        (None, None) => Ordering::Equal,
        (None, Some(right)) => {
            if right.is_empty() {
                Ordering::Equal
            } else {
                Ordering::Greater
            }
        }
        (Some(left), None) => {
            if left.is_empty() {
                Ordering::Equal
            } else {
                Ordering::Less
            }
        }
    }
}

fn parse_extension_id(spelling: CompactString) -> Result<ModuleExtensionId, LockfileParseError> {
    let mut parts = spelling.split('%');
    let label = parts.next().expect("split always has a first component");
    let Some(extension_name) = parts.next() else {
        return Err(delimiter_error(AdapterDomain::ModuleExtensionId));
    };
    let isolation_key = parts.next().map(parse_isolation_key).transpose()?;
    Ok(ModuleExtensionId {
        bzl_file: parse_label(label, AdapterDomain::ModuleExtensionId)?,
        extension_name: extension_name.into(),
        isolation_key,
    })
}

fn parse_isolation_key(spelling: &str) -> Result<ModuleExtensionIsolationKey, LockfileParseError> {
    let mut parts = spelling.split('+');
    let module = parts.next().expect("split always has a first component");
    let Some(usage_name) = parts.next() else {
        return Err(delimiter_error(AdapterDomain::IsolationKey));
    };
    Ok(ModuleExtensionIsolationKey {
        module: parse_module_key(module.into())?,
        usage_name: usage_name.into(),
    })
}

fn parse_label(
    spelling: &str,
    domain: AdapterDomain,
) -> Result<LockfileCanonicalLabel, LockfileParseError> {
    if let Some(body) = spelling.strip_prefix("//") {
        return canonicalize_label(None, body, domain);
    }
    let repository_form = spelling
        .strip_prefix("@@")
        .or_else(|| spelling.strip_prefix('@'));
    let Some(repository_form) = repository_form else {
        return Err(label_adapter_error(domain, "invalid canonical label"));
    };
    let (repository, body, shorthand_target) =
        if let Some((repository, body)) = repository_form.split_once("//") {
            (repository, body, None)
        } else {
            (repository_form, "", Some(repository_form))
        };
    if !valid_repository_name(repository) {
        return Err(label_adapter_error(domain, "invalid canonical repository"));
    }
    if let Some(target) = shorthand_target {
        canonicalize_label_parts(Some(repository), "", target, domain)
    } else {
        canonicalize_label(Some(repository), body, domain)
    }
}

fn canonicalize_label(
    repository: Option<&str>,
    body: &str,
    domain: AdapterDomain,
) -> Result<LockfileCanonicalLabel, LockfileParseError> {
    let (package, target) = match body.split_once(':') {
        Some((package, target)) if !target.is_empty() && !target.contains(':') => (package, target),
        Some(_) => return Err(label_adapter_error(domain, "invalid label target")),
        None => {
            let Some(target) = body.rsplit('/').next().filter(|target| !target.is_empty()) else {
                return Err(label_adapter_error(domain, "label has no target"));
            };
            (body, target)
        }
    };
    canonicalize_label_parts(repository, package, target, domain)
}

fn canonicalize_label_parts(
    repository: Option<&str>,
    package: &str,
    target: &str,
    domain: AdapterDomain,
) -> Result<LockfileCanonicalLabel, LockfileParseError> {
    if !valid_package_name(package) {
        return Err(label_adapter_error(domain, "invalid label package"));
    }
    let Some(target) = normalize_target_name(target) else {
        return Err(label_adapter_error(domain, "invalid label target"));
    };
    let repository = repository.unwrap_or("");
    let prefix = if domain == AdapterDomain::ModuleExtensionId && repository.is_empty() {
        CompactString::new("//")
    } else {
        format!("@@{repository}//").into()
    };
    Ok(LockfileCanonicalLabel {
        canonical: format!("{prefix}{package}:{target}").into(),
    })
}

fn canonical_label_components(label: &str) -> (&str, &str, &str) {
    let (repository, body) = if let Some(body) = label.strip_prefix("//") {
        ("", body)
    } else {
        label
            .strip_prefix("@@")
            .and_then(|label| label.split_once("//"))
            .expect("validated canonical label")
    };
    let (package, target) = body
        .split_once(':')
        .expect("canonical labels always retain an explicit target");
    (repository, package, target)
}

fn valid_repository_name(repository: &str) -> bool {
    !matches!(repository, "." | "..")
        && repository
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'+'))
}

fn valid_package_name(package: &str) -> bool {
    if package.is_empty() {
        return true;
    }
    if package.starts_with('/') || package.ends_with('/') {
        return false;
    }
    package.split('/').all(|segment| {
        !segment.is_empty()
            && segment.bytes().any(|byte| byte != b'.')
            && segment
                .bytes()
                .all(|byte| (0x20..=0x7e).contains(&byte) && !matches!(byte, b':' | b'\\'))
    })
}

fn normalize_target_name(target: &str) -> Option<&str> {
    if target.is_empty()
        || target.starts_with('/')
        || target == ".."
        || target.starts_with("../")
        || target.starts_with("./")
        || target.ends_with("/..")
        || target.ends_with('/')
        || target
            .as_bytes()
            .iter()
            .any(|byte| *byte <= 0x1f || matches!(*byte, 0x7f | b':' | b'\\'))
        || target.contains("/../")
        || target.contains("/./")
        || target.contains("//")
    {
        return None;
    }
    Some(target)
}

fn parse_factors(spelling: CompactString) -> ModuleExtensionEvalFactors {
    if spelling == "general" {
        return ModuleExtensionEvalFactors {
            operating_system: None,
            architecture: None,
        };
    }
    let mut result = ModuleExtensionEvalFactors {
        operating_system: None,
        architecture: None,
    };
    for part in spelling.split(',') {
        if let Some(value) = part.strip_prefix("os:") {
            result.operating_system = (!value.is_empty()).then(|| value.into());
        } else if let Some(value) = part.strip_prefix("arch:") {
            result.architecture = (!value.is_empty()).then(|| value.into());
        }
    }
    result
}

fn parse_recorded_input_string(
    spelling: CompactString,
) -> Result<RecordedInput, LockfileParseError> {
    let Some(space) = spelling.find(' ').filter(|index| *index > 0) else {
        return Ok(recorded_input_sentinel());
    };
    let Some(input) = recorded_unescape(&spelling[..space]) else {
        return Ok(recorded_input_sentinel());
    };
    let value = recorded_unescape(&spelling[space + 1..]);
    let Some((kind, identity)) = input.split_once(':') else {
        return Ok(recorded_input_sentinel());
    };
    let key = match kind {
        "FILE" => RecordedInputKey::File(parse_recorded_path(identity)?),
        "DIRENTS" => RecordedInputKey::DirectoryEntries(parse_recorded_path(identity)?),
        "DIRTREE" => parse_directory_tree(identity)?,
        "ENV" => RecordedInputKey::Environment(identity.into()),
        "REPO_MAPPING" => {
            let Some((source_repository, apparent_name)) = identity.split_once(',') else {
                return Ok(recorded_input_sentinel());
            };
            RecordedInputKey::RepositoryMapping {
                source_repository: source_repository.into(),
                apparent_name: apparent_name.into(),
            }
        }
        _ => return Ok(recorded_input_sentinel()),
    };
    Ok(RecordedInput { key, value })
}

fn parse_recorded_path(identity: &str) -> Result<RecordedPath, LockfileParseError> {
    if identity.is_empty() {
        return Err(adapter_syntax(
            AdapterDomain::RecordedInput,
            "empty recorded path",
        ));
    }
    Ok(RecordedPath {
        canonical: identity.into(),
    })
}

fn parse_directory_tree(identity: &str) -> Result<RecordedInputKey, LockfileParseError> {
    const DELIMITER: &str = "?/../excludes=";
    let (path, excludes) = match identity.split_once(DELIMITER) {
        Some((path, query)) => {
            let without_trailing_empty = query.trim_end_matches(',');
            let mut excludes = Vec::new();
            if query.is_empty() {
                excludes.push(percent_decode("")?);
            } else if !without_trailing_empty.is_empty() {
                for exclude in without_trailing_empty.split(',') {
                    excludes.push(percent_decode(exclude)?);
                }
            }
            (path, excludes.into())
        }
        None => (identity, Arc::from([])),
    };
    Ok(RecordedInputKey::DirectoryTree {
        path: parse_recorded_path(path)?,
        excludes,
    })
}

fn parse_repo_rule_id(spelling: CompactString) -> Result<LockfileRepoRuleId, LockfileParseError> {
    match spelling.split_once('%') {
        Some((label, name)) => Ok(LockfileRepoRuleId {
            bzl_file: Some(parse_label(label, AdapterDomain::RepoRuleId)?),
            rule_name: name.into(),
        }),
        None => Ok(LockfileRepoRuleId {
            bzl_file: None,
            rule_name: spelling,
        }),
    }
}

fn parse_attribute_string_exact(
    value: CompactString,
) -> Result<AttributeValue, LockfileParseError> {
    if value.starts_with("@@") {
        Ok(AttributeValue::Label(parse_label(
            &value,
            AdapterDomain::AttributeValues,
        )?))
    } else if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        Ok(AttributeValue::String(value[1..value.len() - 1].into()))
    } else {
        Ok(AttributeValue::String(value))
    }
}

fn parse_attribute_key_exact(value: CompactString) -> Result<AttributeKey, LockfileParseError> {
    match parse_attribute_string_exact(value)? {
        AttributeValue::Label(label) => Ok(AttributeKey::Label(label)),
        AttributeValue::String(value) => Ok(AttributeKey::String(value)),
        _ => unreachable!(),
    }
}

fn gson_get_as_i32(spelling: &str) -> Result<i32, LockfileParseError> {
    if let Ok(value) = spelling.parse::<i32>() {
        return Ok(value);
    }
    if let Ok(value) = spelling.parse::<i64>() {
        return Ok(value as i32);
    }
    big_decimal_int_value(spelling)
        .ok_or_else(|| adapter_syntax(AdapterDomain::AttributeValues, "invalid integer"))
}

fn json_reader_next_i32(spelling: &str, domain: AdapterDomain) -> Result<i32, LockfileParseError> {
    if let Ok(value) = spelling.parse::<i32>() {
        return Ok(value);
    }
    let double = java_parse_double(spelling).map_err(|_| match domain {
        AdapterDomain::Version => illegal_argument_adapter_error(
            domain,
            &format!("java.lang.NumberFormatException: For input string: \"{spelling}\""),
        ),
        _ => illegal_argument_adapter_error(domain, "invalid signed 32-bit integer"),
    })?;
    let narrowed = double as i32;
    if f64::from(narrowed) != double {
        return Err(illegal_argument_adapter_error(
            domain,
            "integer does not fit signed 32-bit range exactly",
        ));
    }
    Ok(narrowed)
}

#[cfg(test)]
mod host_lockfile_json_reader_next_i32_tests {
    use super::*;

    #[test]
    fn full_reader_preserves_invalid_version_spelling_in_caught_error() {
        let error = read_lockfile_v28(
            br#"{"decoy":{"lockFileVersion":28},"lockFileVersion":"<<<<<<<"}"#,
            UnsupportedVersionPolicy::ReturnEmpty,
        )
        .unwrap_err();
        assert_eq!(
            error.surface(),
            LockfileParseErrorSurface::CaughtIllegalArgument
        );
        assert!(matches!(
            error.kind(),
            LockfileParseErrorKind::InvalidAdapterValue {
                domain: AdapterDomain::Version,
            }
        ));
        assert_eq!(error.position(), None);
        assert_eq!(
            error.message(),
            "java.lang.NumberFormatException: For input string: \"<<<<<<<\""
        );
        assert_eq!(
            error.to_string(),
            "java.lang.NumberFormatException: For input string: \"<<<<<<<\""
        );
    }

    #[test]
    fn full_reader_preserves_generic_non_version_integer_error() {
        let error = read_lockfile_v28(
            br#"{"lockFileVersion":28,"factsVersions":{"//:ext.bzl%x":"ordinary"}}"#,
            UnsupportedVersionPolicy::ReturnEmpty,
        )
        .unwrap_err();
        assert_eq!(
            error.surface(),
            LockfileParseErrorSurface::CaughtIllegalArgument
        );
        assert!(matches!(
            error.kind(),
            LockfileParseErrorKind::InvalidAdapterValue {
                domain: AdapterDomain::Facts,
            }
        ));
        assert_eq!(error.position(), None);
        assert_eq!(error.message(), "invalid signed 32-bit integer");
        assert_eq!(error.to_string(), "invalid signed 32-bit integer");
    }
}

fn java_parse_double(spelling: &str) -> Result<f64, ()> {
    let spelling = spelling.trim_matches(|character| character <= '\u{0020}');
    let spelling = spelling
        .strip_suffix(['f', 'F', 'd', 'D'])
        .unwrap_or(spelling);
    let unsigned = spelling.strip_prefix(['+', '-']).unwrap_or(spelling);
    if unsigned.starts_with("0x") || unsigned.starts_with("0X") {
        parse_java_hex_float(spelling)
    } else {
        spelling.parse::<f64>().map_err(|_| ())
    }
}

fn parse_java_hex_float(spelling: &str) -> Result<f64, ()> {
    let (negative, spelling) = if let Some(spelling) = spelling.strip_prefix('-') {
        (true, spelling)
    } else {
        (false, spelling.strip_prefix('+').unwrap_or(spelling))
    };
    let spelling = spelling
        .strip_prefix("0x")
        .or_else(|| spelling.strip_prefix("0X"))
        .ok_or(())?;
    let exponent_marker = spelling.find(['p', 'P']).ok_or(())?;
    let mantissa = &spelling[..exponent_marker];
    let exponent = parse_saturated_decimal_i64(&spelling[exponent_marker + 1..]).ok_or(())?;

    let mut digits = Vec::with_capacity(mantissa.len());
    let mut saw_dot = false;
    let mut fractional_digits = 0_i64;
    for byte in mantissa.bytes() {
        if byte == b'.' {
            if saw_dot {
                return Err(());
            }
            saw_dot = true;
            continue;
        }
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return Err(()),
        };
        digits.push(digit);
        if saw_dot {
            fractional_digits = fractional_digits.saturating_add(1);
        }
    }
    if digits.is_empty() || spelling[exponent_marker + 1..].contains(['p', 'P']) {
        return Err(());
    }
    let Some(first_nonzero) = digits.iter().position(|digit| *digit != 0) else {
        return Ok(if negative { -0.0 } else { 0.0 });
    };
    let digits = &digits[first_nonzero..];
    let high_nibble_bits = 8 - digits[0].leading_zeros() as usize;
    let bit_len = (digits.len() - 1)
        .saturating_mul(4)
        .saturating_add(high_nibble_bits);
    let scale = exponent.saturating_sub(fractional_digits.saturating_mul(4));
    let mut value_exponent = (bit_len as i64 - 1).saturating_add(scale);
    let sign_bit = if negative { 1_u64 << 63 } else { 0 };

    if value_exponent > 1023 {
        return Ok(f64::from_bits(sign_bit | (0x7ff_u64 << 52)));
    }
    if value_exponent >= -1022 {
        let mut significand = rounded_hex_shift(digits, bit_len as i64 - 53);
        if significand == 1_u64 << 53 {
            significand >>= 1;
            value_exponent += 1;
            if value_exponent > 1023 {
                return Ok(f64::from_bits(sign_bit | (0x7ff_u64 << 52)));
            }
        }
        let exponent_bits = (value_exponent + 1023) as u64;
        return Ok(f64::from_bits(
            sign_bit | (exponent_bits << 52) | (significand & ((1_u64 << 52) - 1)),
        ));
    }

    let fraction = rounded_hex_shift(digits, scale.saturating_add(1074).saturating_neg());
    if fraction >= 1_u64 << 52 {
        Ok(f64::from_bits(sign_bit | (1_u64 << 52)))
    } else {
        Ok(f64::from_bits(sign_bit | fraction))
    }
}

fn parse_saturated_decimal_i64(spelling: &str) -> Option<i64> {
    let (negative, digits) = if let Some(digits) = spelling.strip_prefix('-') {
        (true, digits)
    } else {
        (false, spelling.strip_prefix('+').unwrap_or(spelling))
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut value = 0_i64;
    for digit in digits.bytes() {
        value = value
            .saturating_mul(10)
            .saturating_add(i64::from(digit - b'0'));
    }
    Some(if negative {
        value.saturating_neg()
    } else {
        value
    })
}

fn rounded_hex_shift(digits: &[u8], right_shift: i64) -> u64 {
    let bit_len = ((digits.len() - 1) * 4 + (8 - digits[0].leading_zeros() as usize)) as i64;
    if right_shift <= 0 {
        let mut value = 0_u64;
        for digit in digits {
            value = (value << 4) | u64::from(*digit);
        }
        return value << (-right_shift as u32);
    }

    let mut value = 0_u64;
    if right_shift < bit_len {
        for position in (right_shift as usize..bit_len as usize).rev() {
            value = (value << 1) | u64::from(hex_bit(digits, position));
        }
    }
    let guard_position = right_shift - 1;
    let guard =
        guard_position >= 0 && guard_position < bit_len && hex_bit(digits, guard_position as usize);
    let sticky_end = guard_position.clamp(0, bit_len) as usize;
    let sticky = (0..sticky_end).any(|position| hex_bit(digits, position));
    if guard && (sticky || value & 1 != 0) {
        value + 1
    } else {
        value
    }
}

fn hex_bit(digits: &[u8], position_from_right: usize) -> bool {
    let nibble = digits[digits.len() - 1 - position_from_right / 4];
    nibble & (1 << (position_from_right % 4)) != 0
}

fn big_decimal_int_value(spelling: &str) -> Option<i32> {
    let (mantissa, exponent) = match spelling.find(['e', 'E']) {
        Some(index) => (
            &spelling[..index],
            spelling[index + 1..].parse::<i32>().ok()?,
        ),
        None => (spelling, 0),
    };
    let (negative, mantissa) = match mantissa.as_bytes().first() {
        Some(b'-') => (true, &mantissa[1..]),
        Some(b'+') => (false, &mantissa[1..]),
        _ => (false, mantissa),
    };
    let mut digits = String::new();
    let mut fraction_digits = 0_i64;
    let mut after_decimal = false;
    for byte in mantissa.bytes() {
        match byte {
            b'.' if !after_decimal => after_decimal = true,
            b'0'..=b'9' => {
                digits.push(char::from(byte));
                if after_decimal {
                    fraction_digits += 1;
                }
            }
            _ => return None,
        }
    }
    if digits.is_empty() {
        return None;
    }
    let scale = fraction_digits.checked_sub(i64::from(exponent))?;
    if !(i64::from(i32::MIN)..=i64::from(i32::MAX)).contains(&scale) {
        return None;
    }
    let shift = -scale;
    let integer_digits = if shift < 0 {
        let keep = digits.len().saturating_sub((-shift) as usize);
        &digits[..keep]
    } else {
        digits.as_str()
    };
    let mut modulo = integer_digits.bytes().fold(0_u32, |value, digit| {
        value.wrapping_mul(10).wrapping_add(u32::from(digit - b'0'))
    });
    if shift > 0 {
        modulo = modulo.wrapping_mul(pow_wrapping_u32(10, shift as u64));
    }
    if negative {
        modulo = modulo.wrapping_neg();
    }
    Some(modulo as i32)
}

fn pow_wrapping_u32(mut base: u32, mut exponent: u64) -> u32 {
    let mut result = 1_u32;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = result.wrapping_mul(base);
        }
        base = base.wrapping_mul(base);
        exponent >>= 1;
    }
    result
}

fn required<T>(value: Option<T>, property: &str) -> Result<T, LockfileParseError> {
    value.ok_or_else(|| {
        parse_error(
            LockfileParseErrorSurface::CaughtJsonSyntax,
            LockfileParseErrorKind::MissingRequiredProperty {
                property: property.into(),
            },
            None,
            "missing required extension property",
        )
    })
}

fn render_module_key(key: &LockfileModuleKey) -> String {
    match key {
        LockfileModuleKey::Root => "<root>".to_owned(),
        LockfileModuleKey::Module { name, version } => format!(
            "{name}@{}",
            if version.is_empty() {
                "_"
            } else {
                version.normalized()
            }
        ),
    }
}

fn render_extension_id(id: &ModuleExtensionId) -> String {
    let mut result = format!("{}%{}", id.bzl_file.canonical, id.extension_name);
    if let Some(isolation) = &id.isolation_key {
        result.push('%');
        result.push_str(&render_module_key(&isolation.module));
        result.push('+');
        result.push_str(&isolation.usage_name);
    }
    result
}

fn render_factors(factors: &ModuleExtensionEvalFactors) -> String {
    match (&factors.operating_system, &factors.architecture) {
        (None, None) => "general".to_owned(),
        (Some(operating_system), Some(architecture)) => {
            format!("os:{operating_system},arch:{architecture}")
        }
        (Some(operating_system), None) => format!("os:{operating_system}"),
        (None, Some(architecture)) => format!("arch:{architecture}"),
    }
}

fn render_recorded_input(input: &RecordedInput) -> Result<CompactString, LockfileRenderError> {
    let identity = match &input.key {
        RecordedInputKey::File(path) => format!("FILE:{}", path.canonical),
        RecordedInputKey::DirectoryEntries(path) => format!("DIRENTS:{}", path.canonical),
        RecordedInputKey::DirectoryTree { path, excludes } => {
            let mut value = format!("DIRTREE:{}", path.canonical);
            if !excludes.is_empty() {
                value.push_str("?/../excludes=");
                for (index, exclude) in excludes.iter().enumerate() {
                    if index != 0 {
                        value.push(',');
                    }
                    value.push_str(&percent_encode(exclude));
                }
            }
            value
        }
        RecordedInputKey::Environment(name) => format!("ENV:{name}"),
        RecordedInputKey::RepositoryMapping {
            source_repository,
            apparent_name,
        } => format!("REPO_MAPPING:{source_repository},{apparent_name}"),
        RecordedInputKey::ParseFailure => {
            return Err(render_error(
                LockfileRenderErrorKind::RecordedInputParseFailureSentinel,
                "recorded-input parse-failure sentinel is not renderable",
            ));
        }
    };
    Ok(format!(
        "{} {}",
        recorded_escape(Some(identity.as_str())),
        recorded_escape(input.value.as_deref())
    )
    .into())
}

fn escape_attribute_string(value: &str) -> CompactString {
    if value.starts_with("@@") || (value.starts_with('\'') && value.ends_with('\'')) {
        format!("'{value}'").into()
    } else {
        value.into()
    }
}

struct TokenReader<'a, 'b> {
    tokenizer: &'a mut GsonTokenizer<'b>,
    peeked: Option<(SourcePosition, GsonToken)>,
    pending_synthetic_null: Option<(SourcePosition, GsonToken)>,
}

impl<'a, 'b> TokenReader<'a, 'b> {
    fn new(tokenizer: &'a mut GsonTokenizer<'b>) -> Self {
        Self {
            tokenizer,
            peeked: None,
            pending_synthetic_null: None,
        }
    }

    fn position(&self) -> SourcePosition {
        self.tokenizer.position()
    }

    fn next(&mut self) -> Result<Option<(SourcePosition, GsonToken)>, LockfileParseError> {
        match self.pending_synthetic_null.take() {
            Some(token) => Ok(Some(token)),
            None => match self.peeked.take() {
                Some(token) => Ok(Some(token)),
                None => self.tokenizer.next_token(),
            },
        }
    }

    fn peek(&mut self) -> Result<Option<&GsonToken>, LockfileParseError> {
        if let Some((_, token)) = &self.pending_synthetic_null {
            return Ok(Some(token));
        }
        if self.peeked.is_none() {
            self.peeked = self.tokenizer.next_token()?;
        }
        Ok(self.peeked.as_ref().map(|(_, token)| token))
    }

    fn push_synthetic_null(&mut self) {
        debug_assert!(self.pending_synthetic_null.is_none());
        self.pending_synthetic_null = Some((self.position(), GsonToken::Null));
    }

    fn take_null(&mut self) -> Result<bool, LockfileParseError> {
        if matches!(self.peek()?, Some(GsonToken::Null)) {
            self.next()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn expect(&mut self, expected: GsonToken) -> Result<(), LockfileParseError> {
        match self.next()? {
            Some((_, actual)) if actual == expected => Ok(()),
            Some((position, _)) => Err(syntax_error(position, "unexpected JSON token")),
            None => Err(syntax_error(self.position(), "unexpected end of JSON")),
        }
    }

    fn begin_object(&mut self) -> Result<ObjectCursor, LockfileParseError> {
        self.expect(GsonToken::BeginObject)?;
        Ok(ObjectCursor { first: true })
    }

    fn begin_array(&mut self) -> Result<ArrayCursor, LockfileParseError> {
        self.expect(GsonToken::BeginArray)?;
        Ok(ArrayCursor { first: true })
    }

    fn string(&mut self, domain: AdapterDomain) -> Result<CompactString, LockfileParseError> {
        match self.next()? {
            Some((_, GsonToken::String(value))) => Ok(value.normalize(domain)),
            Some((_, GsonToken::Number(value))) => Ok(value),
            Some((position, _)) => Err(parse_error(
                LockfileParseErrorSurface::CaughtJsonSyntax,
                LockfileParseErrorKind::InvalidAdapterValue { domain },
                Some(position),
                "expected string",
            )),
            None => Err(adapter_syntax(domain, "unexpected end of JSON")),
        }
    }

    fn boolean(&mut self, domain: AdapterDomain) -> Result<bool, LockfileParseError> {
        match self.next()? {
            Some((_, GsonToken::Boolean(value))) => Ok(value),
            Some((position, _)) => Err(parse_error(
                LockfileParseErrorSurface::CaughtJsonSyntax,
                LockfileParseErrorKind::InvalidAdapterValue { domain },
                Some(position),
                "expected boolean",
            )),
            None => Err(adapter_syntax(domain, "unexpected end of JSON")),
        }
    }

    fn skip_value(&mut self) -> Result<(), LockfileParseError> {
        let Some((position, token)) = self.next()? else {
            return Err(syntax_error(self.position(), "unexpected end of JSON"));
        };
        match token {
            GsonToken::BeginObject => {
                let mut object = ObjectCursor { first: true };
                while object
                    .next_name(self, AdapterDomain::ModuleExtension)?
                    .is_some()
                {
                    self.skip_value()?;
                }
                Ok(())
            }
            GsonToken::BeginArray => {
                let mut array = ArrayCursor { first: true };
                while array.next_value(self)? {
                    self.skip_value()?;
                }
                Ok(())
            }
            GsonToken::String(_)
            | GsonToken::Number(_)
            | GsonToken::Boolean(_)
            | GsonToken::Null => Ok(()),
            _ => Err(syntax_error(position, "expected JSON value")),
        }
    }
}

struct ObjectCursor {
    first: bool,
}

impl ObjectCursor {
    fn next_name(
        &mut self,
        reader: &mut TokenReader<'_, '_>,
        domain: AdapterDomain,
    ) -> Result<Option<CompactString>, LockfileParseError> {
        if self.first {
            self.first = false;
        } else {
            match reader.next()? {
                Some((_, GsonToken::EndObject)) => return Ok(None),
                Some((_, GsonToken::Comma)) => {}
                Some((position, _)) => return Err(syntax_error(position, "expected comma")),
                None => return Err(syntax_error(reader.position(), "unterminated object")),
            }
        }
        match reader.next()? {
            Some((_, GsonToken::EndObject)) => Ok(None),
            Some((_, GsonToken::String(name))) => {
                reader.expect(GsonToken::Colon)?;
                Ok(Some(name.normalize(domain)))
            }
            Some((_, GsonToken::Number(name))) => {
                reader.expect(GsonToken::Colon)?;
                Ok(Some(name))
            }
            Some((position, _)) => Err(syntax_error(position, "expected object name")),
            None => Err(syntax_error(reader.position(), "unterminated object")),
        }
    }
}

struct ArrayCursor {
    first: bool,
}

impl ArrayCursor {
    fn next_value(&mut self, reader: &mut TokenReader<'_, '_>) -> Result<bool, LockfileParseError> {
        let after_separator = if self.first {
            self.first = false;
            false
        } else {
            match reader.next()? {
                Some((_, GsonToken::EndArray)) => return Ok(false),
                Some((_, GsonToken::Comma)) => {}
                Some((position, _)) => return Err(syntax_error(position, "expected comma")),
                None => return Err(syntax_error(reader.position(), "unterminated array")),
            }
            true
        };
        match reader.peek()? {
            Some(GsonToken::EndArray) if !after_separator => {
                reader.next()?;
                Ok(false)
            }
            Some(GsonToken::EndArray | GsonToken::Comma) => {
                reader.push_synthetic_null();
                Ok(true)
            }
            _ => Ok(true),
        }
    }
}

fn stream_parse_lockfile(
    reader: &mut TokenReader<'_, '_>,
) -> Result<BazelLockfile, LockfileParseError> {
    let mut result = BazelLockfile::default();
    let mut object = reader.begin_object()?;
    while let Some(name) = object.next_name(reader, AdapterDomain::ModuleExtension)? {
        if reader.take_null()? {
            continue;
        }
        match name.as_str() {
            "lockFileVersion" => {
                result.lock_file_version = json_reader_next_i32(
                    &reader.string(AdapterDomain::Version)?,
                    AdapterDomain::Version,
                )?
            }
            "registryFileHashes" => {
                result.registry_file_hashes = stream_sorted_map(reader, |reader, key| {
                    let spelling = reader.string(AdapterDomain::Checksum)?;
                    let hash = if spelling == REGISTRY_FILE_NOT_FOUND_V28 {
                        RegistryFileHash::NotFound
                    } else {
                        let decoded = hex::decode(spelling.as_str()).map_err(|_| {
                            direct_adapter_error(AdapterDomain::Checksum, "invalid checksum")
                        })?;
                        RegistryFileHash::Sha256(decoded.try_into().map_err(|_| {
                            direct_adapter_error(AdapterDomain::Checksum, "invalid checksum length")
                        })?)
                    };
                    Ok((key, hash))
                })?;
            }
            "selectedYankedVersions" => {
                result.selected_yanked_versions = stream_sorted_map(reader, |reader, key| {
                    Ok((
                        parse_module_key(key)?,
                        reader.string(AdapterDomain::ModuleKey)?,
                    ))
                })?;
            }
            "moduleExtensions" => {
                result.module_extensions = stream_sorted_map(reader, |reader, key| {
                    Ok((
                        parse_extension_id(key)?,
                        stream_sorted_map(reader, |reader, factor| {
                            Ok((parse_factors(factor), stream_parse_extension(reader)?))
                        })?,
                    ))
                })?;
            }
            "facts" => {
                result.facts = stream_sorted_map(reader, |reader, key| {
                    Ok((parse_extension_id(key)?, stream_parse_facts(reader)?))
                })?;
            }
            "factsVersions" => {
                result.facts_versions = stream_sorted_map(reader, |reader, key| {
                    Ok((
                        parse_extension_id(key)?,
                        json_reader_next_i32(
                            &reader.string(AdapterDomain::Facts)?,
                            AdapterDomain::Facts,
                        )?,
                    ))
                })?;
            }
            _ => reader.skip_value()?,
        }
    }
    Ok(result)
}

fn stream_sorted_map<K, V>(
    reader: &mut TokenReader<'_, '_>,
    mut parse: impl FnMut(&mut TokenReader<'_, '_>, CompactString) -> Result<(K, V), LockfileParseError>,
) -> Result<SortedMap<K, V>, LockfileParseError>
where
    K: Ord + std::hash::Hash,
{
    let mut values = SmallMap::new();
    let mut object = reader.begin_object()?;
    while let Some(key) = object.next_name(reader, AdapterDomain::ModuleExtension)? {
        let (key, value) = parse(reader, key)?;
        if values.insert(key, value).is_some() {
            return Err(duplicate_normalized_map_key_error());
        }
    }
    Ok(values.into())
}

fn stream_small_map<K, V>(
    reader: &mut TokenReader<'_, '_>,
    mut parse: impl FnMut(&mut TokenReader<'_, '_>, CompactString) -> Result<(K, V), LockfileParseError>,
) -> Result<SmallMap<K, V>, LockfileParseError>
where
    K: Eq + std::hash::Hash,
{
    let mut values = SmallMap::new();
    let mut object = reader.begin_object()?;
    while let Some(key) = object.next_name(reader, AdapterDomain::ModuleExtension)? {
        let (key, value) = parse(reader, key)?;
        if values.insert(key, value).is_some() {
            return Err(duplicate_normalized_map_key_error());
        }
    }
    Ok(values)
}

fn stream_small_map_last_wins<K, V>(
    reader: &mut TokenReader<'_, '_>,
    mut parse: impl FnMut(&mut TokenReader<'_, '_>, CompactString) -> Result<(K, V), LockfileParseError>,
) -> Result<SmallMap<K, V>, LockfileParseError>
where
    K: Eq + std::hash::Hash,
{
    let mut values = SmallMap::new();
    let mut object = reader.begin_object()?;
    while let Some(key) = object.next_name(reader, AdapterDomain::ModuleExtension)? {
        let (key, value) = parse(reader, key)?;
        values.insert(key, value);
    }
    Ok(values)
}

fn stream_parse_extension(
    reader: &mut TokenReader<'_, '_>,
) -> Result<LockfileModuleExtension, LockfileParseError> {
    let mut bzl = None;
    let mut usages = None;
    let mut inputs = None;
    let mut specs = None;
    let mut metadata = None;
    let mut object = reader.begin_object()?;
    while let Some(name) = object.next_name(reader, AdapterDomain::ModuleExtension)? {
        if reader.take_null()? {
            continue;
        }
        match name.as_str() {
            "bzlTransitiveDigest" => bzl = Some(stream_digest(reader)?),
            "usagesDigest" => usages = Some(stream_digest(reader)?),
            "recordedInputs" => {
                let mut values = Vec::new();
                let mut array = reader.begin_array()?;
                while array.next_value(reader)? {
                    values.push(parse_recorded_input_string(
                        reader.string(AdapterDomain::RecordedInput)?,
                    )?);
                }
                inputs = Some(values.into());
            }
            "generatedRepoSpecs" => {
                specs = Some(stream_small_map(reader, |reader, key| {
                    Ok((key, stream_parse_repo_spec(reader)?))
                })?)
            }
            "moduleExtensionMetadata" => metadata = Some(stream_parse_metadata(reader)?),
            _ => reader.skip_value()?,
        }
    }
    Ok(LockfileModuleExtension {
        bzl_transitive_digest: required(bzl, "bzlTransitiveDigest")?,
        usages_digest: required(usages, "usagesDigest")?,
        recorded_inputs: required(inputs, "recordedInputs")?,
        generated_repo_specs: required(specs, "generatedRepoSpecs")?,
        metadata,
    })
}

fn stream_digest(reader: &mut TokenReader<'_, '_>) -> Result<Arc<[u8]>, LockfileParseError> {
    LENIENT_STANDARD_BASE64
        .decode(reader.string(AdapterDomain::ModuleExtension)?.as_bytes())
        .map(Arc::from)
        .map_err(|_| {
            parse_error(
                LockfileParseErrorSurface::CaughtIllegalArgument,
                LockfileParseErrorKind::InvalidBase64,
                None,
                "invalid standard Base64 digest",
            )
        })
}

fn stream_parse_metadata(
    reader: &mut TokenReader<'_, '_>,
) -> Result<LockfileModuleExtensionMetadata, LockfileParseError> {
    let mut direct = None;
    let mut dev = None;
    let mut all = None;
    let mut reproducible = false;
    let mut object = reader.begin_object()?;
    while let Some(name) = object.next_name(reader, AdapterDomain::Metadata)? {
        match name.as_str() {
            "explicitRootModuleDirectDeps" => direct = stream_nullable_set(reader)?,
            "explicitRootModuleDirectDevDeps" => dev = stream_nullable_set(reader)?,
            "useAllRepos" => {
                if !reader.take_null()? {
                    all = match reader.string(AdapterDomain::Metadata)?.as_str() {
                        "NO" => Some(UseAllRepos::No),
                        "REGULAR" => Some(UseAllRepos::Regular),
                        "DEV" => Some(UseAllRepos::Dev),
                        _ => None,
                    };
                }
            }
            "reproducible" => {
                if !reader.take_null()? {
                    reproducible = reader.boolean(AdapterDomain::Metadata)?;
                }
            }
            _ => reader.skip_value()?,
        }
    }
    Ok(LockfileModuleExtensionMetadata {
        explicit_root_module_direct_deps: direct,
        explicit_root_module_direct_dev_deps: dev,
        use_all_repos: all.ok_or_else(|| {
            parse_error(
                LockfileParseErrorSurface::CaughtNullPointer,
                LockfileParseErrorKind::MissingMetadataEnum,
                None,
                "missing useAllRepos",
            )
        })?,
        reproducible,
    })
}

fn stream_nullable_set(
    reader: &mut TokenReader<'_, '_>,
) -> Result<Option<SmallSet<CompactString>>, LockfileParseError> {
    if reader.take_null()? {
        return Ok(None);
    }
    let mut result = SmallSet::new();
    let mut array = reader.begin_array()?;
    while array.next_value(reader)? {
        result.insert(reader.string(AdapterDomain::Metadata)?);
    }
    Ok(Some(result))
}

fn stream_parse_repo_spec(
    reader: &mut TokenReader<'_, '_>,
) -> Result<LockfileRepoSpec, LockfileParseError> {
    let mut id = None;
    let mut attributes = None;
    let mut object = reader.begin_object()?;
    while let Some(name) = object.next_name(reader, AdapterDomain::RepoSpec)? {
        match name.as_str() {
            "repoRuleId" => {
                if !reader.take_null()? {
                    id = Some(parse_repo_rule_id(
                        reader.string(AdapterDomain::RepoRuleId)?,
                    )?);
                }
            }
            "attributes" => {
                if !reader.take_null()? {
                    attributes = Some(AttributeValues {
                        values: stream_small_map_last_wins(reader, |reader, key| {
                            Ok((key, stream_parse_attribute(reader)?))
                        })?,
                    });
                }
            }
            _ => reader.skip_value()?,
        }
    }
    Ok(LockfileRepoSpec {
        repo_rule_id: id,
        attributes,
    })
}

fn stream_parse_attribute(
    reader: &mut TokenReader<'_, '_>,
) -> Result<AttributeValue, LockfileParseError> {
    match reader.peek()? {
        Some(GsonToken::Null) => {
            reader.next()?;
            Ok(AttributeValue::None)
        }
        Some(GsonToken::Boolean(_)) => Ok(AttributeValue::Bool(
            reader.boolean(AdapterDomain::AttributeValues)?,
        )),
        Some(GsonToken::Number(_)) => Ok(AttributeValue::Int(gson_get_as_i32(
            &reader.string(AdapterDomain::AttributeValues)?,
        )?)),
        Some(GsonToken::String(_)) => {
            parse_attribute_string_exact(reader.string(AdapterDomain::AttributeValues)?)
        }
        Some(GsonToken::BeginArray) => {
            let mut values = Vec::new();
            let mut array = reader.begin_array()?;
            while array.next_value(reader)? {
                values.push(stream_parse_attribute(reader)?);
            }
            Ok(AttributeValue::Sequence(values.into()))
        }
        Some(GsonToken::BeginObject) => Ok(AttributeValue::Dict(stream_small_map_last_wins(
            reader,
            |reader, key| {
                Ok((
                    parse_attribute_key_exact(key)?,
                    stream_parse_attribute(reader)?,
                ))
            },
        )?)),
        _ => Err(adapter_syntax(
            AdapterDomain::AttributeValues,
            "unsupported attribute value",
        )),
    }
}

fn stream_parse_facts(reader: &mut TokenReader<'_, '_>) -> Result<Facts, LockfileParseError> {
    if !matches!(reader.peek()?, Some(GsonToken::BeginObject)) {
        return Err(facts_error("Facts root must be an object"));
    }
    Ok(Facts {
        values: stream_fact_dict(reader, MAX_FACT_NESTING_DEPTH - 1)?,
    })
}

fn stream_fact_dict(
    reader: &mut TokenReader<'_, '_>,
    remaining: u8,
) -> Result<SortedMap<CompactString, FactValue>, LockfileParseError> {
    let mut values = SmallMap::new();
    let mut object = reader.begin_object()?;
    while let Some(key) = object.next_name(reader, AdapterDomain::Facts)? {
        values.insert(key, stream_fact_value(reader, remaining)?);
    }
    Ok(values.into())
}

fn stream_fact_value(
    reader: &mut TokenReader<'_, '_>,
    remaining: u8,
) -> Result<FactValue, LockfileParseError> {
    match reader.peek()? {
        Some(GsonToken::Null) => {
            reader.next()?;
            Ok(FactValue::Null)
        }
        Some(GsonToken::Boolean(_)) => Ok(FactValue::Bool(reader.boolean(AdapterDomain::Facts)?)),
        Some(GsonToken::String(_)) => Ok(FactValue::String(reader.string(AdapterDomain::Facts)?)),
        Some(GsonToken::Number(_)) => {
            let value = reader.string(AdapterDomain::Facts)?;
            if value.contains('.') || value.contains('e') || value.contains('E') {
                let float = value
                    .parse::<f64>()
                    .map_err(|_| facts_error("invalid float"))?;
                if !float.is_finite() {
                    return Err(facts_error("Facts floats must be finite"));
                }
                Ok(FactValue::Number(FactNumber::FiniteFloat(float)))
            } else {
                Ok(FactValue::Number(FactNumber::Integer(
                    canonical_integer(&value).ok_or_else(|| facts_error("invalid integer"))?,
                )))
            }
        }
        Some(GsonToken::BeginArray) => {
            if remaining == 0 {
                return Err(facts_error("Facts nesting exceeds depth seven"));
            }
            let mut values = Vec::new();
            let mut array = reader.begin_array()?;
            while array.next_value(reader)? {
                values.push(stream_fact_value(reader, remaining - 1)?);
            }
            Ok(FactValue::List(values.into()))
        }
        Some(GsonToken::BeginObject) => {
            if remaining == 0 {
                return Err(facts_error("Facts nesting exceeds depth seven"));
            }
            Ok(FactValue::Dict(stream_fact_dict(reader, remaining - 1)?))
        }
        _ => Err(facts_error("unsupported Facts value")),
    }
}

enum WriteContext {
    Object { first: bool, named: bool },
    Array { first: bool },
}

struct PrettyWriter {
    output: String,
    stack: Vec<WriteContext>,
    root_written: bool,
}

impl PrettyWriter {
    fn new() -> Self {
        Self {
            output: String::new(),
            stack: Vec::new(),
            root_written: false,
        }
    }

    fn finish(mut self) -> CompactString {
        self.output.push('\n');
        self.output.into()
    }

    fn before_value(&mut self) {
        let depth = self.stack.len();
        match self.stack.last_mut() {
            Some(WriteContext::Array { first }) => {
                if !*first {
                    self.output.push(',');
                }
                *first = false;
                self.output.push('\n');
                indent(&mut self.output, depth);
            }
            Some(WriteContext::Object { named, .. }) => {
                assert!(*named, "object value must follow a name");
                *named = false;
            }
            None => {
                assert!(!self.root_written, "only one JSON root");
                self.root_written = true;
            }
        }
    }

    fn name(&mut self, name: &str) {
        let depth = self.stack.len();
        let Some(WriteContext::Object { first, named }) = self.stack.last_mut() else {
            panic!("name outside object");
        };
        assert!(!*named, "previous object name has no value");
        if !*first {
            self.output.push(',');
        }
        *first = false;
        *named = true;
        self.output.push('\n');
        indent(&mut self.output, depth);
        self.output.push_str(&json_quote(name));
        self.output.push_str(": ");
    }

    fn begin_object(&mut self) {
        self.before_value();
        self.output.push('{');
        self.stack.push(WriteContext::Object {
            first: true,
            named: false,
        });
    }

    fn end_object(&mut self) {
        let Some(WriteContext::Object { first, named }) = self.stack.pop() else {
            panic!("object stack mismatch");
        };
        assert!(!named, "object name has no value");
        if !first {
            self.output.push('\n');
            indent(&mut self.output, self.stack.len());
        }
        self.output.push('}');
    }

    fn begin_array(&mut self) {
        self.before_value();
        self.output.push('[');
        self.stack.push(WriteContext::Array { first: true });
    }

    fn end_array(&mut self) {
        let Some(WriteContext::Array { first }) = self.stack.pop() else {
            panic!("array stack mismatch");
        };
        if !first {
            self.output.push('\n');
            indent(&mut self.output, self.stack.len());
        }
        self.output.push(']');
    }

    fn string(&mut self, value: &str) {
        self.before_value();
        self.output.push_str(&json_quote(value));
    }

    fn number(&mut self, value: &str) {
        self.before_value();
        self.output.push_str(value);
    }

    fn boolean(&mut self, value: bool) {
        self.before_value();
        self.output.push_str(if value { "true" } else { "false" });
    }

    fn null(&mut self) {
        self.before_value();
        self.output.push_str("null");
    }
}

fn stream_render_lockfile(
    writer: &mut PrettyWriter,
    lockfile: &BazelLockfile,
) -> Result<(), LockfileRenderError> {
    writer.begin_object();
    writer.name("lockFileVersion");
    writer.number(&lockfile.lock_file_version.to_string());
    writer.name("registryFileHashes");
    writer.begin_object();
    for (url, hash) in &lockfile.registry_file_hashes {
        writer.name(url);
        match hash {
            RegistryFileHash::NotFound => writer.string(REGISTRY_FILE_NOT_FOUND_V28),
            RegistryFileHash::Sha256(bytes) => writer.string(&hex::encode(bytes)),
        }
    }
    writer.end_object();
    writer.name("selectedYankedVersions");
    writer.begin_object();
    for (key, reason) in &lockfile.selected_yanked_versions {
        writer.name(&render_module_key(key));
        writer.string(reason);
    }
    writer.end_object();
    writer.name("moduleExtensions");
    writer.begin_object();
    for (id, factors) in &lockfile.module_extensions {
        writer.name(&render_extension_id(id));
        writer.begin_object();
        for (factor, extension) in factors {
            writer.name(&render_factors(factor));
            stream_render_extension(writer, extension)?;
        }
        writer.end_object();
    }
    writer.end_object();
    writer.name("facts");
    writer.begin_object();
    for (id, facts) in &lockfile.facts {
        writer.name(&render_extension_id(id));
        stream_render_fact_dict(writer, &facts.values);
    }
    writer.end_object();
    writer.name("factsVersions");
    writer.begin_object();
    for (id, version) in &lockfile.facts_versions {
        writer.name(&render_extension_id(id));
        writer.number(&version.to_string());
    }
    writer.end_object();
    writer.end_object();
    Ok(())
}

fn stream_render_extension(
    writer: &mut PrettyWriter,
    extension: &LockfileModuleExtension,
) -> Result<(), LockfileRenderError> {
    writer.begin_object();
    writer.name("bzlTransitiveDigest");
    writer.string(&STANDARD_BASE64.encode(extension.bzl_transitive_digest.as_ref()));
    writer.name("usagesDigest");
    writer.string(&STANDARD_BASE64.encode(extension.usages_digest.as_ref()));
    writer.name("recordedInputs");
    writer.begin_array();
    for input in extension.recorded_inputs.iter() {
        writer.string(&render_recorded_input(input)?);
    }
    writer.end_array();
    writer.name("generatedRepoSpecs");
    writer.begin_object();
    for (name, spec) in &extension.generated_repo_specs {
        writer.name(name);
        stream_render_repo_spec(writer, spec)?;
    }
    writer.end_object();
    if let Some(metadata) = &extension.metadata {
        writer.name("moduleExtensionMetadata");
        writer.begin_object();
        if let Some(values) = &metadata.explicit_root_module_direct_deps {
            writer.name("explicitRootModuleDirectDeps");
            writer.begin_array();
            for value in values {
                writer.string(value);
            }
            writer.end_array();
        }
        if let Some(values) = &metadata.explicit_root_module_direct_dev_deps {
            writer.name("explicitRootModuleDirectDevDeps");
            writer.begin_array();
            for value in values {
                writer.string(value);
            }
            writer.end_array();
        }
        writer.name("useAllRepos");
        writer.string(match metadata.use_all_repos {
            UseAllRepos::No => "NO",
            UseAllRepos::Regular => "REGULAR",
            UseAllRepos::Dev => "DEV",
        });
        writer.name("reproducible");
        writer.boolean(metadata.reproducible);
        writer.end_object();
    }
    writer.end_object();
    Ok(())
}

fn stream_render_repo_spec(
    writer: &mut PrettyWriter,
    spec: &LockfileRepoSpec,
) -> Result<(), LockfileRenderError> {
    writer.begin_object();
    if let Some(id) = &spec.repo_rule_id {
        let Some(label) = &id.bzl_file else {
            return Err(render_error(
                LockfileRenderErrorKind::RepoRuleIdWithoutLabel,
                "RepoRuleId without a label is not renderable",
            ));
        };
        writer.name("repoRuleId");
        writer.string(&format!("{}%{}", label.canonical, id.rule_name));
    }
    if let Some(attributes) = &spec.attributes {
        writer.name("attributes");
        writer.begin_object();
        for (name, value) in &attributes.values {
            writer.name(name);
            stream_render_attribute(writer, value);
        }
        writer.end_object();
    }
    writer.end_object();
    Ok(())
}

fn stream_render_attribute(writer: &mut PrettyWriter, value: &AttributeValue) {
    match value {
        AttributeValue::None => writer.null(),
        AttributeValue::Bool(value) => writer.boolean(*value),
        AttributeValue::Int(value) => writer.number(&value.to_string()),
        AttributeValue::String(value) => writer.string(&escape_attribute_string(value)),
        AttributeValue::Label(label) => writer.string(&label.canonical),
        AttributeValue::Sequence(values) => {
            writer.begin_array();
            for value in values.iter() {
                stream_render_attribute(writer, value);
            }
            writer.end_array();
        }
        AttributeValue::Dict(values) => {
            writer.begin_object();
            for (key, value) in values {
                match key {
                    AttributeKey::String(value) => {
                        let escaped = escape_attribute_string(value);
                        writer.name(&escaped);
                    }
                    AttributeKey::Label(label) => writer.name(&label.canonical),
                }
                stream_render_attribute(writer, value);
            }
            writer.end_object();
        }
    }
}

fn stream_render_fact_dict(
    writer: &mut PrettyWriter,
    values: &SortedMap<CompactString, FactValue>,
) {
    writer.begin_object();
    for (key, value) in values {
        writer.name(key);
        stream_render_fact(writer, value);
    }
    writer.end_object();
}

fn stream_render_fact(writer: &mut PrettyWriter, value: &FactValue) {
    match value {
        FactValue::Null => writer.null(),
        FactValue::Bool(value) => writer.boolean(*value),
        FactValue::Number(FactNumber::Integer(value)) => writer.number(value),
        FactValue::Number(FactNumber::FiniteFloat(value)) => {
            writer.number(&starlark_float_to_string(*value))
        }
        FactValue::String(value) => writer.string(value),
        FactValue::List(values) => {
            writer.begin_array();
            for value in values.iter() {
                stream_render_fact(writer, value);
            }
            writer.end_array();
        }
        FactValue::Dict(values) => stream_render_fact_dict(writer, values),
    }
}

fn starlark_float_to_string(value: f64) -> String {
    debug_assert!(value.is_finite());
    let scientific = format!("{value:.16e}");
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("Rust scientific formatting includes an exponent");
    let exponent = exponent
        .parse::<i32>()
        .expect("Rust scientific exponent is signed decimal");
    if exponent < -4 || exponent >= 17 {
        let mut mantissa = mantissa.to_owned();
        while mantissa.ends_with('0') {
            mantissa.pop();
        }
        if mantissa.ends_with('.') {
            mantissa.pop();
        }
        let sign = if exponent < 0 { '-' } else { '+' };
        return format!("{mantissa}e{sign}{:02}", exponent.unsigned_abs());
    }

    let (negative, mantissa) = match mantissa.strip_prefix('-') {
        Some(mantissa) => (true, mantissa),
        None => (false, mantissa),
    };
    let digits: String = mantissa
        .chars()
        .filter(|character| *character != '.')
        .collect();
    let decimal_position = exponent + 1;
    let mut fixed = String::with_capacity(digits.len() + 8);
    if negative {
        fixed.push('-');
    }
    if decimal_position <= 0 {
        fixed.push_str("0.");
        for _ in 0..-decimal_position {
            fixed.push('0');
        }
        fixed.push_str(&digits);
    } else {
        let decimal_position = decimal_position as usize;
        if decimal_position >= digits.len() {
            fixed.push_str(&digits);
            for _ in digits.len()..decimal_position {
                fixed.push('0');
            }
        } else {
            fixed.push_str(&digits[..decimal_position]);
            fixed.push('.');
            fixed.push_str(&digits[decimal_position..]);
        }
    }
    if let Some(dot) = fixed.find('.') {
        while fixed.len() > dot + 2 && fixed.ends_with('0') {
            fixed.pop();
        }
    } else {
        fixed.push_str(".0");
    }
    fixed
}

fn scan_version_marker(source: &str) -> Result<Option<(i32, &str)>, LockfileParseError> {
    const PREFIX: &str = "\"lockFileVersion\":";
    let mut offset = 0;
    while let Some(relative) = source[offset..].find(PREFIX) {
        let start = offset + relative + PREFIX.len();
        let rest = &source[start..];
        let whitespace = rest
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))
            .count();
        let digits_start = start + whitespace;
        let digits_len = source[digits_start..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        if digits_len > 0 {
            let spelling = &source[digits_start..digits_start + digits_len];
            let value = spelling.parse::<i32>().map_err(|_| {
                parse_error(
                    LockfileParseErrorSurface::CaughtIllegalArgument,
                    LockfileParseErrorKind::VersionMarkerOverflow {
                        spelling: spelling.into(),
                    },
                    None,
                    "lockfile version marker overflows signed 32-bit integer",
                )
            })?;
            return Ok(Some((value, spelling)));
        }
        offset = start;
    }
    Ok(None)
}

fn looks_like_number(value: &str) -> bool {
    if matches!(value, "NaN" | "Infinity" | "-Infinity") {
        return true;
    }
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && (bytes[0].is_ascii_digit() || matches!(bytes[0], b'-' | b'+'))
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E'))
}

fn canonical_integer(value: &str) -> Option<CompactString> {
    let (negative, digits) = match value.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, value.strip_prefix('+').unwrap_or(value)),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some(CompactString::new("0"));
    }
    Some(
        if negative {
            format!("-{digits}")
        } else {
            digits.to_owned()
        }
        .into(),
    )
}

fn integer_equals_finite_float(integer: &str, float: f64) -> bool {
    if !float.is_finite() || float.fract() != 0.0 {
        return false;
    }
    canonical_integer(integer).is_some_and(|integer| integer == exact_integral_float_decimal(float))
}

fn exact_integral_float_decimal(value: f64) -> CompactString {
    if value == 0.0 {
        return CompactString::new("0");
    }
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let mantissa = if exponent == 0 {
        fraction
    } else {
        fraction | (1_u64 << 52)
    };
    let shift = if exponent == 0 {
        -1074
    } else {
        exponent - 1023 - 52
    };
    let mut digits = if shift < 0 {
        (mantissa >> (-shift as u32)).to_string()
    } else {
        mantissa.to_string()
    };
    for _ in 0..shift.max(0) {
        decimal_multiply_two(&mut digits);
    }
    if negative {
        digits.insert(0, '-');
    }
    digits.into()
}

fn decimal_multiply_two(digits: &mut String) {
    let mut bytes = digits.as_bytes().to_vec();
    let mut carry = 0;
    for byte in bytes.iter_mut().rev() {
        let value = (*byte - b'0') * 2 + carry;
        *byte = b'0' + value % 10;
        carry = value / 10;
    }
    *digits = String::from_utf8(bytes).expect("decimal digits remain UTF-8");
    if carry != 0 {
        digits.insert(0, char::from(b'0' + carry));
    }
}

fn recorded_unescape(value: &str) -> Option<CompactString> {
    if value == "\\0" {
        return None;
    }
    let mut output = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            output.push(match character {
                'n' => '\n',
                's' => ' ',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    Some(output.into())
}

fn recorded_escape(value: Option<&str>) -> String {
    match value {
        None => "\\0".to_owned(),
        Some(value) => value
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace(' ', "\\s"),
    }
}

fn recorded_input_sentinel() -> RecordedInput {
    RecordedInput {
        key: RecordedInputKey::ParseFailure,
        value: Some(CompactString::new("")),
    }
}

fn percent_decode(value: &str) -> Result<CompactString, LockfileParseError> {
    let bytes = value.as_bytes();
    let mut output = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(illegal_argument_adapter_error(
                    AdapterDomain::RecordedInput,
                    "invalid percent escape",
                ));
            }
            let byte = u8::from_str_radix(&value[index + 1..index + 3], 16).map_err(|_| {
                illegal_argument_adapter_error(
                    AdapterDomain::RecordedInput,
                    "invalid percent escape",
                )
            })?;
            output.push(byte);
            index += 3;
        } else if bytes[index] == b'+' {
            output.push(b' ');
            index += 1;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    Ok(java_utf8_decode(&output).into())
}

fn percent_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'*') {
            output.push(char::from(byte));
        } else if byte == b' ' {
            output.push('+');
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn json_quote(value: &str) -> String {
    let mut quoted = serde_json::to_string(value).expect("string JSON serialization is infallible");
    if quoted.contains('\u{2028}') || quoted.contains('\u{2029}') {
        quoted = quoted
            .replace('\u{2028}', "\\u2028")
            .replace('\u{2029}', "\\u2029");
    }
    quoted
}

fn indent(output: &mut String, depth: usize) {
    for _ in 0..depth {
        output.push_str("  ");
    }
}

fn parse_error(
    surface: LockfileParseErrorSurface,
    kind: LockfileParseErrorKind,
    position: Option<SourcePosition>,
    message: &str,
) -> LockfileParseError {
    LockfileParseError {
        surface,
        kind,
        position,
        message: message.into(),
    }
}

fn syntax_error(position: SourcePosition, message: &str) -> LockfileParseError {
    parse_error(
        LockfileParseErrorSurface::CaughtJsonSyntax,
        LockfileParseErrorKind::MalformedJson,
        Some(position),
        message,
    )
}

fn adapter_syntax(domain: AdapterDomain, message: &str) -> LockfileParseError {
    parse_error(
        LockfileParseErrorSurface::CaughtJsonSyntax,
        LockfileParseErrorKind::InvalidAdapterValue { domain },
        None,
        message,
    )
}

fn direct_adapter_error(domain: AdapterDomain, message: &str) -> LockfileParseError {
    parse_error(
        LockfileParseErrorSurface::DirectAdapterJsonParse,
        LockfileParseErrorKind::InvalidAdapterValue { domain },
        None,
        message,
    )
}

fn illegal_argument_adapter_error(domain: AdapterDomain, message: &str) -> LockfileParseError {
    parse_error(
        LockfileParseErrorSurface::CaughtIllegalArgument,
        LockfileParseErrorKind::InvalidAdapterValue { domain },
        None,
        message,
    )
}

fn label_adapter_error(domain: AdapterDomain, message: &str) -> LockfileParseError {
    match domain {
        AdapterDomain::RepoRuleId | AdapterDomain::AttributeValues => {
            illegal_argument_adapter_error(domain, message)
        }
        _ => direct_adapter_error(domain, message),
    }
}

fn delimiter_error(domain: AdapterDomain) -> LockfileParseError {
    parse_error(
        LockfileParseErrorSurface::DelimiterIndexOutOfBounds,
        LockfileParseErrorKind::MissingDelimiter { domain },
        None,
        "missing required adapter delimiter",
    )
}

fn facts_error(message: &str) -> LockfileParseError {
    parse_error(
        LockfileParseErrorSurface::CaughtJsonSyntax,
        LockfileParseErrorKind::InvalidFacts,
        None,
        message,
    )
}

fn duplicate_normalized_map_key_error() -> LockfileParseError {
    parse_error(
        LockfileParseErrorSurface::CaughtJsonSyntax,
        LockfileParseErrorKind::DuplicateNormalizedMapKey,
        None,
        "duplicate normalized map key",
    )
}

fn render_error(kind: LockfileRenderErrorKind, message: &str) -> LockfileRenderError {
    LockfileRenderError {
        kind,
        message: message.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum AdapterDomain {
    Checksum,
    ModuleKey,
    Version,
    ModuleExtensionId,
    IsolationKey,
    EvalFactors,
    ModuleExtension,
    Metadata,
    Facts,
    RecordedInput,
    RepoSpec,
    RepoRuleId,
    AttributeValues,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum LockfileParseErrorKind {
    UnsupportedVersion { found: Option<CompactString> },
    VersionMarkerOverflow { spelling: CompactString },
    MalformedJson,
    GsonTokenState,
    MissingRequiredProperty { property: CompactString },
    InvalidBase64,
    InvalidFacts,
    MissingMetadataEnum,
    DuplicateNormalizedMapKey,
    InvalidAdapterValue { domain: AdapterDomain },
    MissingDelimiter { domain: AdapterDomain },
}

/// Bazel 9.2's outer lockfile read catch surface. Direct custom-adapter
/// `JsonParseException` and delimiter `IndexOutOfBoundsException` deliberately
/// remain distinguishable uncaught holes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum LockfileParseErrorSurface {
    UnsupportedVersion,
    CaughtJsonSyntax,
    CaughtNullPointer,
    CaughtIllegalArgument,
    DirectAdapterJsonParse,
    DelimiterIndexOutOfBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct LockfileParseError {
    pub(crate) surface: LockfileParseErrorSurface,
    pub(crate) kind: LockfileParseErrorKind,
    pub(crate) position: Option<SourcePosition>,
    pub(crate) message: CompactString,
}

impl LockfileParseError {
    pub fn surface(&self) -> LockfileParseErrorSurface {
        self.surface
    }

    pub fn kind(&self) -> &LockfileParseErrorKind {
        &self.kind
    }

    pub fn position(&self) -> Option<SourcePosition> {
        self.position
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for LockfileParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(position) = self.position {
            write!(
                f,
                "{} at line {} column {}",
                self.message, position.line, position.column
            )
        } else {
            f.write_str(&self.message)
        }
    }
}

impl Error for LockfileParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum LockfileRenderErrorKind {
    RecordedInputParseFailureSentinel,
    RepoRuleIdWithoutLabel,
    InvalidRetainedValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct LockfileRenderError {
    pub(crate) kind: LockfileRenderErrorKind,
    pub(crate) message: CompactString,
}

impl LockfileRenderError {
    pub fn kind(&self) -> LockfileRenderErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for LockfileRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for LockfileRenderError {}
