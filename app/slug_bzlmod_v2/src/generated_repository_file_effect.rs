/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select either.
 */

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use base64::Engine;
use base64::engine::DecodePaddingMode;
use base64::engine::general_purpose::GeneralPurpose;
use base64::engine::general_purpose::GeneralPurposeConfig;
use base64::engine::general_purpose::STANDARD;
use compact_str::CompactString;
use starlark_map::small_set::SmallSet;
use url::Url;

use crate::repository_label_path::HostRepositoryLabelPathValue;

const MAX_ARCHIVES: usize = 128;
const MAX_ARCHIVE_URLS: usize = 8;
const MAX_ARCHIVE_URL_BYTES: usize = 2048;
const MAX_ARCHIVE_PREFIX_BYTES: usize = 256;
const MAX_ARCHIVE_PREFIX_COMPONENTS: usize = 32;

/// One normalized, repository-relative file authored by the admitted
/// `repository_ctx.file` subset.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct GeneratedRepositoryFileEffect {
    path: CompactString,
    content: Arc<[u8]>,
    executable: bool,
}

impl GeneratedRepositoryFileEffect {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn content(&self) -> &[u8] {
        &self.content
    }

    pub fn executable(&self) -> bool {
        self.executable
    }
}

/// One selected external Label source linked at a normalized repository-relative
/// destination. Observed source bytes participate in structural plan identity.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct GeneratedRepositorySymlinkEffect {
    path: CompactString,
    source: HostRepositoryLabelPathValue,
    content: Arc<[u8]>,
}

impl GeneratedRepositorySymlinkEffect {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn source(&self) -> &HostRepositoryLabelPathValue {
        &self.source
    }

    pub fn content(&self) -> &[u8] {
        &self.content
    }
}

/// Compression formats admitted by the shared bounded tar extractor.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum GeneratedRepositoryArchiveFormat {
    TarXz,
    TarGz,
}

/// Bazel's omitted `output` argument extracts into the repository root.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum GeneratedRepositoryArchiveOutput {
    RepositoryRoot,
}

/// A general checksum-pinned archive intent. No archive bytes or physical
/// capture path are retained in the repository observation.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Hash, Allocative)]
pub struct GeneratedRepositoryArchiveEffect {
    urls: Arc<[CompactString]>,
    sha256: [u8; 32],
    strip_prefix: CompactString,
    format: GeneratedRepositoryArchiveFormat,
    output: GeneratedRepositoryArchiveOutput,
    canonical_id: CompactString,
    declared_integrity: CompactString,
}

impl fmt::Debug for GeneratedRepositoryArchiveEffect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedRepositoryArchiveEffect")
            .field("url_count", &self.urls.len())
            .field("sha256", &self.sha256_hex())
            .field("format", &self.format)
            .field("output", &self.output)
            .finish_non_exhaustive()
    }
}

impl GeneratedRepositoryArchiveEffect {
    pub fn new(
        urls: Arc<[CompactString]>,
        sha256: [u8; 32],
        strip_prefix: CompactString,
        format: GeneratedRepositoryArchiveFormat,
        canonical_id: CompactString,
        declared_integrity: CompactString,
    ) -> Result<Self, GeneratedRepositoryFileEffectPlanError> {
        if urls.is_empty() {
            return Err(GeneratedRepositoryFileEffectPlanError::EmptyArchiveUrls);
        }
        if urls.len() > MAX_ARCHIVE_URLS {
            return Err(GeneratedRepositoryFileEffectPlanError::TooManyArchiveUrls);
        }
        if urls.iter().any(|url| !valid_archive_url(url)) {
            return Err(GeneratedRepositoryFileEffectPlanError::InvalidArchiveUrl);
        }
        if !strip_prefix.is_empty()
            && (strip_prefix.len() > MAX_ARCHIVE_PREFIX_BYTES
                || strip_prefix.split('/').count() > MAX_ARCHIVE_PREFIX_COMPONENTS
                || !valid_path(&strip_prefix))
        {
            return Err(GeneratedRepositoryFileEffectPlanError::InvalidArchiveStripPrefix);
        }
        if !declared_integrity.is_empty()
            && generated_repository_archive_parse_sha256("", &declared_integrity)? != sha256
        {
            return Err(GeneratedRepositoryFileEffectPlanError::InvalidArchiveChecksum);
        }
        Ok(Self {
            urls,
            sha256,
            strip_prefix,
            format,
            output: GeneratedRepositoryArchiveOutput::RepositoryRoot,
            canonical_id,
            declared_integrity,
        })
    }

    pub fn canonical_id(&self) -> &str {
        &self.canonical_id
    }

    pub fn declared_integrity(&self) -> &str {
        &self.declared_integrity
    }

    pub fn urls(&self) -> &[CompactString] {
        &self.urls
    }

    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    pub fn sha256_hex(&self) -> String {
        generated_repository_archive_sha256_hex(self.sha256)
    }

    pub fn sha256_sri(&self) -> String {
        generated_repository_archive_sha256_sri(self.sha256)
    }

    pub fn strip_prefix(&self) -> &str {
        &self.strip_prefix
    }

    pub fn format(&self) -> GeneratedRepositoryArchiveFormat {
        self.format
    }

    pub fn output(&self) -> GeneratedRepositoryArchiveOutput {
        self.output
    }
}

/// Result fields for a checksum-pinned archive, without requiring
/// Loading to own a separate hex or base64 dependency.
#[doc(hidden)]
pub fn generated_repository_archive_sha256_hex(sha256: [u8; 32]) -> String {
    hex::encode(sha256)
}

#[doc(hidden)]
pub fn generated_repository_archive_sha256_sri(sha256: [u8; 32]) -> String {
    format!("sha256-{}", STANDARD.encode(sha256))
}

/// Parse the admitted Bazel SHA256 checksum forms. Other algorithms and
/// omitted checksums are explicit capability boundaries.
#[doc(hidden)]
pub fn generated_repository_archive_parse_sha256(
    sha256: &str,
    integrity: &str,
) -> Result<[u8; 32], GeneratedRepositoryFileEffectPlanError> {
    use GeneratedRepositoryFileEffectPlanError as Error;
    if !sha256.is_empty() && !integrity.is_empty() {
        return Err(Error::ConflictingArchiveChecksum);
    }
    if !sha256.is_empty() {
        let mut digest = [0; 32];
        hex::decode_to_slice(sha256, &mut digest).map_err(|_| Error::InvalidArchiveChecksum)?;
        return Ok(digest);
    }
    if integrity.is_empty() {
        return Err(Error::MissingArchiveChecksum);
    }
    let encoded = integrity
        .strip_prefix("sha256-")
        .ok_or(Error::UnsupportedArchiveChecksum)?;
    // Java's basic Base64 decoder accepts optional padding and unused pad bits.
    let decoder = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    if !matches!(encoded.len(), 43 | 44) {
        return Err(Error::InvalidArchiveChecksum);
    }
    // The decoder's conservative output estimate is 33 bytes for padded SHA256.
    let mut decoded = [0; 33];
    let length = decoder
        .decode_slice(encoded, &mut decoded)
        .map_err(|_| Error::InvalidArchiveChecksum)?;
    if length != 32 {
        return Err(Error::InvalidArchiveChecksum);
    }
    let mut digest = [0; 32];
    digest.copy_from_slice(&decoded[..32]);
    Ok(digest)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
enum GeneratedRepositoryOrderItem {
    File(usize),
    Archive(GeneratedRepositoryArchiveEffect),
    Symlink(GeneratedRepositorySymlinkEffect),
}

/// A borrowed effect in invocation order; file bytes remain owned only by the
/// plan's file slice rather than being cloned into this order projection.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedRepositoryOrderedEffectRef<'a> {
    File(&'a GeneratedRepositoryFileEffect),
    Archive(&'a GeneratedRepositoryArchiveEffect),
    Symlink(&'a GeneratedRepositorySymlinkEffect),
}

/// Ordered, structural output of one selected repository-rule invocation.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct GeneratedRepositoryFileEffectPlan {
    files: Arc<[GeneratedRepositoryFileEffect]>,
    order: Arc<[GeneratedRepositoryOrderItem]>,
}

impl GeneratedRepositoryFileEffectPlan {
    pub fn effect_count(&self) -> usize {
        self.order.len()
    }

    pub fn is_prefix_of(&self, other: &Self) -> bool {
        self.effect_count() <= other.effect_count()
            && self
                .ordered_effects()
                .zip(other.ordered_effects())
                .all(|(left, right)| left == right)
    }

    pub fn with_archive(
        &self,
        archive: GeneratedRepositoryArchiveEffect,
    ) -> Result<Self, GeneratedRepositoryFileEffectPlanError> {
        let mut builder = GeneratedRepositoryFileEffectPlanBuilder {
            seen: SmallSet::new(),
            files: self.files.to_vec(),
            order: self.order.to_vec(),
            archive_count: self.archive_count(),
        };
        builder.push_archive_effect(archive)?;
        Ok(builder.finish())
    }

    pub fn effects(&self) -> &[GeneratedRepositoryFileEffect] {
        &self.files
    }

    pub fn ordered_effects(&self) -> impl Iterator<Item = GeneratedRepositoryOrderedEffectRef<'_>> {
        self.order.iter().map(|item| match item {
            GeneratedRepositoryOrderItem::File(index) => {
                GeneratedRepositoryOrderedEffectRef::File(&self.files[*index])
            }
            GeneratedRepositoryOrderItem::Archive(archive) => {
                GeneratedRepositoryOrderedEffectRef::Archive(archive)
            }
            GeneratedRepositoryOrderItem::Symlink(symlink) => {
                GeneratedRepositoryOrderedEffectRef::Symlink(symlink)
            }
        })
    }

    pub fn archive_count(&self) -> usize {
        self.order
            .iter()
            .filter(|item| matches!(item, GeneratedRepositoryOrderItem::Archive(_)))
            .count()
    }

    pub fn has_archives(&self) -> bool {
        self.order
            .iter()
            .any(|item| matches!(item, GeneratedRepositoryOrderItem::Archive(_)))
    }

    pub fn has_symlinks(&self) -> bool {
        self.order
            .iter()
            .any(|item| matches!(item, GeneratedRepositoryOrderItem::Symlink(_)))
    }

    pub fn builder() -> GeneratedRepositoryFileEffectPlanBuilder {
        GeneratedRepositoryFileEffectPlanBuilder {
            seen: SmallSet::new(),
            files: Vec::new(),
            order: Vec::new(),
            archive_count: 0,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum GeneratedRepositoryFileEffectPlanError {
    InvalidPath(CompactString),
    RepeatedPath(CompactString),
    ConflictingPath(CompactString),
    SymlinkArchiveMix,
    TooManyArchives,
    EmptyArchiveUrls,
    TooManyArchiveUrls,
    InvalidArchiveUrl,
    InvalidArchiveStripPrefix,
    MissingArchiveChecksum,
    ConflictingArchiveChecksum,
    InvalidArchiveChecksum,
    UnsupportedArchiveChecksum,
}

impl GeneratedRepositoryFileEffectPlan {
    pub fn build(
        effects: impl IntoIterator<Item = (CompactString, Arc<[u8]>, bool)>,
    ) -> Result<Self, GeneratedRepositoryFileEffectPlanError> {
        let mut builder = Self::builder();
        for (path, content, executable) in effects {
            builder.push(path, content, executable)?;
        }
        Ok(builder.finish())
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct GeneratedRepositoryFileEffectPlanBuilder {
    seen: SmallSet<CompactString>,
    files: Vec<GeneratedRepositoryFileEffect>,
    order: Vec<GeneratedRepositoryOrderItem>,
    archive_count: usize,
}

impl GeneratedRepositoryFileEffectPlanBuilder {
    /// Freeze the effects preceding an execute call without consuming the
    /// builder. The effect payloads remain shared through their Arc owners.
    pub fn snapshot(&self) -> GeneratedRepositoryFileEffectPlan {
        GeneratedRepositoryFileEffectPlan {
            files: self.files.clone().into(),
            order: self.order.clone().into(),
        }
    }

    pub fn push(
        &mut self,
        path: CompactString,
        content: Arc<[u8]>,
        executable: bool,
    ) -> Result<(), GeneratedRepositoryFileEffectPlanError> {
        if !valid_path(&path) {
            return Err(GeneratedRepositoryFileEffectPlanError::InvalidPath(path));
        }
        if self.order.iter().any(|item| {
            matches!(item, GeneratedRepositoryOrderItem::Symlink(symlink) if path == symlink.path())
        }) {
            return Err(GeneratedRepositoryFileEffectPlanError::RepeatedPath(path));
        }
        if self.order.iter().any(|item| {
            matches!(item, GeneratedRepositoryOrderItem::Symlink(symlink) if paths_conflict(&path, symlink.path()))
        }) {
            return Err(GeneratedRepositoryFileEffectPlanError::ConflictingPath(path));
        }
        self.seen.insert(path.clone());
        self.order
            .push(GeneratedRepositoryOrderItem::File(self.files.len()));
        self.files.push(GeneratedRepositoryFileEffect {
            path,
            content,
            executable,
        });
        Ok(())
    }

    pub fn push_symlink(
        &mut self,
        path: CompactString,
        source: HostRepositoryLabelPathValue,
        content: Arc<[u8]>,
    ) -> Result<(), GeneratedRepositoryFileEffectPlanError> {
        if !valid_path(&path) {
            return Err(GeneratedRepositoryFileEffectPlanError::InvalidPath(path));
        }
        if self.seen.contains(&path) {
            return Err(GeneratedRepositoryFileEffectPlanError::RepeatedPath(path));
        }
        if self.archive_count != 0 {
            return Err(GeneratedRepositoryFileEffectPlanError::SymlinkArchiveMix);
        }
        if self.files.iter().any(|file| paths_conflict(&path, file.path()))
            || self.order.iter().any(|item| {
                matches!(item, GeneratedRepositoryOrderItem::Symlink(symlink) if paths_conflict(&path, symlink.path()))
            })
        {
            return Err(GeneratedRepositoryFileEffectPlanError::ConflictingPath(path));
        }
        self.seen.insert(path.clone());
        self.order.push(GeneratedRepositoryOrderItem::Symlink(
            GeneratedRepositorySymlinkEffect {
                path,
                source,
                content,
            },
        ));
        Ok(())
    }

    pub fn push_archive(
        &mut self,
        urls: Arc<[CompactString]>,
        sha256: [u8; 32],
        strip_prefix: CompactString,
    ) -> Result<(), GeneratedRepositoryFileEffectPlanError> {
        self.push_archive_effect(GeneratedRepositoryArchiveEffect::new(
            urls,
            sha256,
            strip_prefix,
            GeneratedRepositoryArchiveFormat::TarXz,
            CompactString::new(""),
            CompactString::new(""),
        )?)
    }

    pub fn push_archive_effect(
        &mut self,
        archive: GeneratedRepositoryArchiveEffect,
    ) -> Result<(), GeneratedRepositoryFileEffectPlanError> {
        if self
            .order
            .iter()
            .any(|item| matches!(item, GeneratedRepositoryOrderItem::Symlink(_)))
        {
            return Err(GeneratedRepositoryFileEffectPlanError::SymlinkArchiveMix);
        }
        if self.archive_count >= MAX_ARCHIVES {
            return Err(GeneratedRepositoryFileEffectPlanError::TooManyArchives);
        }
        self.order
            .push(GeneratedRepositoryOrderItem::Archive(archive));
        self.archive_count += 1;
        Ok(())
    }

    pub fn finish(self) -> GeneratedRepositoryFileEffectPlan {
        GeneratedRepositoryFileEffectPlan {
            files: self.files.into(),
            order: self.order.into(),
        }
    }
}

fn valid_archive_url(url: &str) -> bool {
    if url.is_empty() || url.len() > MAX_ARCHIVE_URL_BYTES {
        return false;
    }
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.fragment().is_some() {
        return false;
    }
    match parsed.scheme() {
        "https" => parsed.host_str().is_some(),
        "file" => parsed.host_str().is_none_or(|host| host.is_empty()) && parsed.query().is_none(),
        _ => false,
    }
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && !path.ends_with('/')
        && !path.contains('\\')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn paths_conflict(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|remainder| remainder.starts_with('/'))
        || right
            .strip_prefix(left)
            .is_some_and(|remainder| remainder.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Arc;

    use compact_str::CompactString;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathObservationInstanceId;
    use slug_workspace_v2::PathObservationNamespace;

    use super::GeneratedRepositoryArchiveFormat;
    use super::GeneratedRepositoryArchiveOutput;
    use super::GeneratedRepositoryFileEffectPlan;
    use super::GeneratedRepositoryFileEffectPlanError;
    use super::GeneratedRepositoryOrderedEffectRef;
    use super::generated_repository_archive_sha256_hex;
    use super::generated_repository_archive_sha256_sri;
    use crate::repository_label_path::HostRepositoryLabelPathValue;

    fn source(path: &str, instance: u64) -> HostRepositoryLabelPathValue {
        HostRepositoryLabelPathValue::new(
            NormalizedAbsolutePath::new(path).unwrap(),
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance)),
        )
        .unwrap()
    }

    fn symlink_plan(
        destination: &str,
        source_path: &str,
        instance: u64,
        content: &'static [u8],
    ) -> GeneratedRepositoryFileEffectPlan {
        let mut builder = GeneratedRepositoryFileEffectPlan::builder();
        builder
            .push_symlink(
                destination.into(),
                source(source_path, instance),
                Arc::from(content),
            )
            .unwrap();
        builder.finish()
    }

    fn urls(values: &[&str]) -> Arc<[CompactString]> {
        values
            .iter()
            .map(|value| CompactString::new(*value))
            .collect()
    }

    fn archive_plan(
        archive_urls: Arc<[CompactString]>,
        sha256: [u8; 32],
        strip_prefix: &str,
        file_content: &'static [u8],
        executable: bool,
    ) -> GeneratedRepositoryFileEffectPlan {
        let mut builder = GeneratedRepositoryFileEffectPlan::builder();
        builder
            .push_archive(archive_urls, sha256, CompactString::new(strip_prefix))
            .unwrap();
        builder
            .push(
                CompactString::new("BUILD.bazel"),
                Arc::from(file_content),
                executable,
            )
            .unwrap();
        builder.finish()
    }

    #[test]
    fn builder_snapshot_freezes_exact_ordered_prefix_without_copying_bytes() {
        let source_bytes: Arc<[u8]> = Arc::from(&b"int main() {}"[..]);
        let mut builder = GeneratedRepositoryFileEffectPlan::builder();
        builder
            .push("tools/cpp/empty.cc".into(), source_bytes.clone(), false)
            .unwrap();
        let prefix = builder.snapshot();
        assert_eq!(prefix.ordered_effects().count(), 1);
        assert!(std::ptr::eq(
            prefix.effects()[0].content().as_ptr(),
            source_bytes.as_ptr()
        ));
        builder
            .push(
                "BUILD.bazel".into(),
                Arc::from(&b"exports_files([])"[..]),
                false,
            )
            .unwrap();
        let after = builder.snapshot();
        assert_eq!(prefix.ordered_effects().count(), 1);
        assert_eq!(after.ordered_effects().count(), 2);
        assert_ne!(prefix, after);
        assert_eq!(after, builder.finish());
    }

    #[test]
    fn plan_preserves_order_bytes_executable_and_structural_identity() {
        let plan = GeneratedRepositoryFileEffectPlan::build([
            (
                CompactString::new("BUILD.bazel"),
                Arc::from(&b"exports_files([])\n"[..]),
                true,
            ),
            (
                CompactString::new("generated.txt"),
                Arc::from(&b"hello\n"[..]),
                false,
            ),
        ])
        .unwrap();
        assert_eq!(plan.effects().len(), 2);
        assert_eq!(plan.effects()[0].path(), "BUILD.bazel");
        assert_eq!(plan.effects()[0].content(), b"exports_files([])\n");
        assert!(plan.effects()[0].executable());
        assert_eq!(plan.effects()[1].path(), "generated.txt");
        assert!(!plan.effects()[1].executable());
        let mut left = DefaultHasher::new();
        plan.hash(&mut left);
        let swapped = GeneratedRepositoryFileEffectPlan::build([
            (
                CompactString::new("generated.txt"),
                Arc::from(&b"hello\n"[..]),
                false,
            ),
            (
                CompactString::new("BUILD.bazel"),
                Arc::from(&b"exports_files([])\n"[..]),
                true,
            ),
        ])
        .unwrap();
        assert_ne!(plan, swapped);
        let mut right = DefaultHasher::new();
        swapped.hash(&mut right);
        assert_ne!(left.finish(), right.finish());
    }

    #[test]
    fn plan_rejects_non_normal_paths_and_preserves_ordered_file_overwrites() {
        for path in ["", "/absolute", "a/../b", "a//b", "a\\b", "a/"] {
            assert!(matches!(
                GeneratedRepositoryFileEffectPlan::build([(
                    CompactString::new(path),
                    Arc::from(&b"x"[..]),
                    true,
                )]),
                Err(GeneratedRepositoryFileEffectPlanError::InvalidPath(_))
            ));
        }
        let plan = GeneratedRepositoryFileEffectPlan::build([
            (CompactString::new("a"), Arc::from(&b"one"[..]), true),
            (CompactString::new("a"), Arc::from(&b"two"[..]), false),
        ])
        .unwrap();
        assert_eq!(plan.effect_count(), 2);
        assert_eq!(plan.effects()[0].content(), b"one");
        assert!(plan.effects()[0].executable());
        assert_eq!(plan.effects()[1].content(), b"two");
        assert!(!plan.effects()[1].executable());
    }

    #[test]
    fn symlink_effect_preserves_order_source_and_bytes_in_structural_identity() {
        let mut builder = GeneratedRepositoryFileEffectPlan::builder();
        builder
            .push(
                "BUILD.bazel".into(),
                Arc::from(&b"exports_files([])"[..]),
                false,
            )
            .unwrap();
        builder
            .push_symlink(
                "nested/source.bzl".into(),
                source("/source-a/config.bzl", 7),
                Arc::from(&b"source A"[..]),
            )
            .unwrap();
        builder
            .push("last.txt".into(), Arc::from(&b"last"[..]), false)
            .unwrap();
        let plan = builder.finish();
        assert!(plan.has_symlinks());
        assert!(!plan.has_archives());
        assert_eq!(plan.effects().len(), 2);
        let mut ordered = plan.ordered_effects();
        assert!(
            matches!(ordered.next(), Some(GeneratedRepositoryOrderedEffectRef::File(file)) if file.path() == "BUILD.bazel")
        );
        let Some(GeneratedRepositoryOrderedEffectRef::Symlink(link)) = ordered.next() else {
            panic!("second effect should be a symlink");
        };
        assert_eq!(link.path(), "nested/source.bzl");
        assert_eq!(link.source(), &source("/source-a/config.bzl", 7));
        assert_eq!(link.content(), b"source A");
        assert!(
            matches!(ordered.next(), Some(GeneratedRepositoryOrderedEffectRef::File(file)) if file.path() == "last.txt")
        );
        assert!(ordered.next().is_none());

        let base = symlink_plan("link.bzl", "/source-a/config.bzl", 7, b"source A");
        assert_eq!(
            base,
            symlink_plan("link.bzl", "/source-a/config.bzl", 7, b"source A")
        );
        for changed in [
            symlink_plan("other.bzl", "/source-a/config.bzl", 7, b"source A"),
            symlink_plan("link.bzl", "/source-b/config.bzl", 7, b"source A"),
            symlink_plan("link.bzl", "/source-a/config.bzl", 8, b"source A"),
            symlink_plan("link.bzl", "/source-a/config.bzl", 7, b"source B"),
        ] {
            assert_ne!(base, changed);
            let mut original_hash = DefaultHasher::new();
            base.hash(&mut original_hash);
            let mut changed_hash = DefaultHasher::new();
            changed.hash(&mut changed_hash);
            assert_ne!(original_hash.finish(), changed_hash.finish());
        }
        let mut file_then_link = GeneratedRepositoryFileEffectPlan::builder();
        file_then_link
            .push("BUILD.bazel".into(), Arc::from(&b"file"[..]), false)
            .unwrap();
        file_then_link
            .push_symlink(
                "link.bzl".into(),
                source("/source-a/config.bzl", 7),
                Arc::from(&b"source A"[..]),
            )
            .unwrap();
        let mut link_then_file = GeneratedRepositoryFileEffectPlan::builder();
        link_then_file
            .push_symlink(
                "link.bzl".into(),
                source("/source-a/config.bzl", 7),
                Arc::from(&b"source A"[..]),
            )
            .unwrap();
        link_then_file
            .push("BUILD.bazel".into(), Arc::from(&b"file"[..]), false)
            .unwrap();
        assert_ne!(file_then_link.finish(), link_then_file.finish());
    }

    #[test]
    fn symlink_builder_rejects_invalid_duplicate_ancestor_and_archive_mixtures() {
        for invalid in ["", "/absolute", "a/../b", "a//b", "a\\b", "a/"] {
            let mut builder = GeneratedRepositoryFileEffectPlan::builder();
            assert!(matches!(
                builder.push_symlink(
                    invalid.into(),
                    source("/source/config.bzl", 1),
                    Arc::from(&b"x"[..]),
                ),
                Err(GeneratedRepositoryFileEffectPlanError::InvalidPath(_))
            ));
            assert!(!builder.finish().has_symlinks());
        }
        for (first_is_symlink, first, second) in [
            (false, "a", "a"),
            (true, "a", "a"),
            (false, "a", "a/b"),
            (true, "a", "a/b"),
            (false, "a/b", "a"),
            (true, "a/b", "a"),
        ] {
            let mut builder = GeneratedRepositoryFileEffectPlan::builder();
            if first_is_symlink {
                builder
                    .push_symlink(
                        first.into(),
                        source("/source/config.bzl", 1),
                        Arc::from(&b"x"[..]),
                    )
                    .unwrap();
            } else {
                builder
                    .push(first.into(), Arc::from(&b"x"[..]), false)
                    .unwrap();
            }
            let result = if first_is_symlink {
                builder.push(second.into(), Arc::from(&b"y"[..]), false)
            } else {
                builder.push_symlink(
                    second.into(),
                    source("/source/config.bzl", 1),
                    Arc::from(&b"y"[..]),
                )
            };
            assert_eq!(
                result,
                Err(if first == second {
                    GeneratedRepositoryFileEffectPlanError::RepeatedPath(second.into())
                } else {
                    GeneratedRepositoryFileEffectPlanError::ConflictingPath(second.into())
                })
            );
            let plan = builder.finish();
            assert_eq!(plan.ordered_effects().count(), 1);
        }
        for (first, second) in [("a", "a"), ("a", "a/b"), ("a/b", "a")] {
            let mut builder = GeneratedRepositoryFileEffectPlan::builder();
            builder
                .push_symlink(
                    first.into(),
                    source("/source/config.bzl", 1),
                    Arc::from(&b"x"[..]),
                )
                .unwrap();
            assert_eq!(
                builder.push_symlink(
                    second.into(),
                    source("/source/other.bzl", 1),
                    Arc::from(&b"y"[..]),
                ),
                Err(if first == second {
                    GeneratedRepositoryFileEffectPlanError::RepeatedPath(second.into())
                } else {
                    GeneratedRepositoryFileEffectPlanError::ConflictingPath(second.into())
                })
            );
            assert_eq!(builder.finish().ordered_effects().count(), 1);
        }
        let mut sibling = GeneratedRepositoryFileEffectPlan::builder();
        sibling
            .push("a/b".into(), Arc::from(&b"x"[..]), false)
            .unwrap();
        sibling
            .push_symlink(
                "a/bc".into(),
                source("/source/config.bzl", 1),
                Arc::from(&b"y"[..]),
            )
            .unwrap();
        assert_eq!(sibling.finish().ordered_effects().count(), 2);

        let mut archive_first = GeneratedRepositoryFileEffectPlan::builder();
        archive_first
            .push_archive(urls(&["file:///tmp/rustc.tar.xz"]), [1; 32], "rustc".into())
            .unwrap();
        assert_eq!(
            archive_first.push_symlink(
                "link.bzl".into(),
                source("/source/config.bzl", 1),
                Arc::from(&b"x"[..]),
            ),
            Err(GeneratedRepositoryFileEffectPlanError::SymlinkArchiveMix)
        );
        assert!(!archive_first.finish().has_symlinks());

        let mut symlink_first = GeneratedRepositoryFileEffectPlan::builder();
        symlink_first
            .push_symlink(
                "link.bzl".into(),
                source("/source/config.bzl", 1),
                Arc::from(&b"x"[..]),
            )
            .unwrap();
        assert_eq!(
            symlink_first.push_archive(
                urls(&["file:///tmp/rustc.tar.xz"]),
                [1; 32],
                "rustc".into(),
            ),
            Err(GeneratedRepositoryFileEffectPlanError::SymlinkArchiveMix)
        );
        assert_eq!(symlink_first.finish().archive_count(), 0);
    }

    #[test]
    fn archive_and_files_keep_invocation_order_without_copying_file_bytes() {
        let mut builder = GeneratedRepositoryFileEffectPlan::builder();
        builder
            .push(
                CompactString::new("first.txt"),
                Arc::from(&b"first"[..]),
                false,
            )
            .unwrap();
        builder
            .push_archive(
                urls(&["https://static.rust-lang.org/dist/rustc.tar.xz"]),
                [7; 32],
                CompactString::new("rustc/rustc"),
            )
            .unwrap();
        builder
            .push(
                CompactString::new("manifest.in"),
                Arc::from(&b"last writer"[..]),
                true,
            )
            .unwrap();
        let plan = builder.finish();
        assert_eq!(plan.effects().len(), 2);
        assert_eq!(plan.archive_count(), 1);
        assert!(plan.has_archives());
        let mut ordered = plan.ordered_effects();
        let Some(GeneratedRepositoryOrderedEffectRef::File(first)) = ordered.next() else {
            panic!("first effect should be a file");
        };
        assert_eq!(first.path(), "first.txt");
        assert!(std::ptr::eq(first, &plan.effects()[0]));
        let Some(GeneratedRepositoryOrderedEffectRef::Archive(archive)) = ordered.next() else {
            panic!("second effect should be an archive");
        };
        assert_eq!(archive.sha256(), [7; 32]);
        assert_eq!(archive.strip_prefix(), "rustc/rustc");
        assert_eq!(archive.format(), GeneratedRepositoryArchiveFormat::TarXz);
        assert_eq!(
            archive.output(),
            GeneratedRepositoryArchiveOutput::RepositoryRoot
        );
        assert_eq!(
            archive.urls(),
            urls(&["https://static.rust-lang.org/dist/rustc.tar.xz"]).as_ref()
        );
        assert_eq!(
            archive.sha256_hex(),
            generated_repository_archive_sha256_hex([7; 32])
        );
        assert_eq!(
            archive.sha256_sri(),
            generated_repository_archive_sha256_sri([7; 32])
        );
        let Some(GeneratedRepositoryOrderedEffectRef::File(last)) = ordered.next() else {
            panic!("third effect should be a file");
        };
        assert_eq!(last.path(), "manifest.in");
        assert_eq!(last.content(), b"last writer");
        assert!(std::ptr::eq(last, &plan.effects()[1]));
        assert!(ordered.next().is_none());
    }

    #[test]
    fn archive_plan_identity_includes_urls_sha_prefix_order_and_file_data() {
        let first_url = "https://static.rust-lang.org/dist/rustc.tar.xz";
        let second_url = "https://mirror.example/dist/rustc.tar.xz";
        let base = archive_plan(
            urls(&[first_url, second_url]),
            [1; 32],
            "rustc/rustc",
            b"exports_files([])",
            false,
        );
        for changed in [
            archive_plan(
                urls(&[second_url, first_url]),
                [1; 32],
                "rustc/rustc",
                b"exports_files([])",
                false,
            ),
            archive_plan(
                urls(&[first_url, second_url]),
                [2; 32],
                "rustc/rustc",
                b"exports_files([])",
                false,
            ),
            archive_plan(
                urls(&[first_url, second_url]),
                [1; 32],
                "rustc/other",
                b"exports_files([])",
                false,
            ),
            archive_plan(
                urls(&[first_url, second_url]),
                [1; 32],
                "rustc/rustc",
                b"exports_files([\"changed\"])",
                false,
            ),
            archive_plan(
                urls(&[first_url, second_url]),
                [1; 32],
                "rustc/rustc",
                b"exports_files([])",
                true,
            ),
        ] {
            assert_ne!(base, changed);
            let mut original_hash = DefaultHasher::new();
            base.hash(&mut original_hash);
            let mut changed_hash = DefaultHasher::new();
            changed.hash(&mut changed_hash);
            assert_ne!(original_hash.finish(), changed_hash.finish());
        }
        let mut reordered = GeneratedRepositoryFileEffectPlan::builder();
        reordered
            .push(
                CompactString::new("BUILD.bazel"),
                Arc::from(&b"exports_files([])"[..]),
                false,
            )
            .unwrap();
        reordered
            .push_archive(
                urls(&[first_url, second_url]),
                [1; 32],
                CompactString::new("rustc/rustc"),
            )
            .unwrap();
        assert_ne!(base, reordered.finish());
    }

    #[test]
    fn general_archive_identity_and_prefix_helpers_preserve_all_fields() {
        let make = |format, canonical_id: &str, integrity: &str| {
            super::GeneratedRepositoryArchiveEffect::new(
                urls(&["https://example.org/source.crate?download=1"]),
                [7; 32],
                "".into(),
                format,
                canonical_id.into(),
                integrity.into(),
            )
            .unwrap()
        };
        let effect = make(GeneratedRepositoryArchiveFormat::TarGz, "scope", "");
        assert_eq!(effect.strip_prefix(), "");
        assert_eq!(effect.canonical_id(), "scope");
        let mut builder = GeneratedRepositoryFileEffectPlan::builder();
        builder
            .push("same".into(), Arc::from(&b"before"[..]), false)
            .unwrap();
        let prefix = builder.snapshot();
        let through_archive = prefix.with_archive(effect.clone()).unwrap();
        assert!(prefix.is_prefix_of(&through_archive));
        assert!(!through_archive.is_prefix_of(&prefix));
        assert_eq!(through_archive.effect_count(), 2);
        builder.push_archive_effect(effect).unwrap();
        assert_eq!(builder.snapshot(), through_archive);
        builder
            .push("same".into(), Arc::from(&b"after"[..]), true)
            .unwrap();
        let final_plan = builder.finish();
        assert!(through_archive.is_prefix_of(&final_plan));
        assert_eq!(final_plan.effects()[1].content(), b"after");
        for changed in [
            make(GeneratedRepositoryArchiveFormat::TarXz, "scope", ""),
            make(GeneratedRepositoryArchiveFormat::TarGz, "other", ""),
            make(
                GeneratedRepositoryArchiveFormat::TarGz,
                "scope",
                &generated_repository_archive_sha256_sri([7; 32]),
            ),
        ] {
            let changed = prefix.with_archive(changed).unwrap();
            assert_ne!(through_archive, changed);
            assert!(!through_archive.is_prefix_of(&changed));
        }
        let link = symlink_plan("link", "/source/file", 1, b"x");
        assert_eq!(
            link.with_archive(make(GeneratedRepositoryArchiveFormat::TarGz, "", "")),
            Err(GeneratedRepositoryFileEffectPlanError::SymlinkArchiveMix)
        );
    }

    #[test]
    fn explicit_sha_and_sri_are_exclusive_normalized_and_length_checked() {
        use GeneratedRepositoryFileEffectPlanError as Error;

        use super::generated_repository_archive_parse_sha256 as parse;
        let hex = generated_repository_archive_sha256_hex([0xab; 32]);
        let sri = generated_repository_archive_sha256_sri([0xab; 32]);
        assert_eq!(parse(&hex.to_uppercase(), ""), Ok([0xab; 32]));
        assert_eq!(parse("", &sri), Ok([0xab; 32]));
        assert_eq!(parse("", sri.trim_end_matches('=')), Ok([0xab; 32]));
        assert_eq!(parse(&hex, &sri), Err(Error::ConflictingArchiveChecksum));
        assert_eq!(parse("", ""), Err(Error::MissingArchiveChecksum));
        assert_eq!(
            parse("", "sha512-AA=="),
            Err(Error::UnsupportedArchiveChecksum)
        );
        for invalid in ["short", "sha256-AA==", "sha256-invalid"] {
            assert!(parse("", invalid).is_err());
        }
        assert_eq!(parse("abc", ""), Err(Error::InvalidArchiveChecksum));
        assert!(
            super::GeneratedRepositoryArchiveEffect::new(
                urls(&["file:///tmp/source.tar.gz"]),
                [1; 32],
                "".into(),
                GeneratedRepositoryArchiveFormat::TarGz,
                "".into(),
                sri.into(),
            )
            .is_err()
        );
    }

    #[test]
    fn archive_intent_rejects_unsafe_shapes_and_enforces_shared_call_cap() {
        let mut builder = GeneratedRepositoryFileEffectPlan::builder();
        assert_eq!(
            builder.push_archive(Arc::from([]), [0; 32], CompactString::new("rustc/rustc")),
            Err(GeneratedRepositoryFileEffectPlanError::EmptyArchiveUrls)
        );
        assert_eq!(
            builder.push_archive(
                urls(&["https://static.rust-lang.org/dist/rustc.tar.xz"; 9]),
                [0; 32],
                CompactString::new("rustc/rustc"),
            ),
            Err(GeneratedRepositoryFileEffectPlanError::TooManyArchiveUrls)
        );
        for invalid in [
            "http://static.rust-lang.org/dist/rustc.tar.xz",
            "https://user:secret@static.rust-lang.org/dist/rustc.tar.xz",
            "file://remote-host/tmp/rustc.tar.xz",
        ] {
            assert_eq!(
                builder.push_archive(urls(&[invalid]), [0; 32], CompactString::new("rustc/rustc")),
                Err(GeneratedRepositoryFileEffectPlanError::InvalidArchiveUrl)
            );
        }
        for invalid in ["/rustc", "rustc/../bin", "rustc//bin"] {
            assert_eq!(
                builder.push_archive(
                    urls(&["file:///tmp/rustc.tar.xz"]),
                    [0; 32],
                    CompactString::new(invalid),
                ),
                Err(GeneratedRepositoryFileEffectPlanError::InvalidArchiveStripPrefix)
            );
        }
        for _ in 0..128 {
            builder
                .push_archive(
                    urls(&["file:///tmp/unrelated.payload"]),
                    [0; 32],
                    CompactString::new(""),
                )
                .unwrap();
        }
        assert_eq!(
            builder.push_archive(
                urls(&["file:///tmp/rustc.tar.xz"]),
                [0; 32],
                CompactString::new("rustc/rustc"),
            ),
            Err(GeneratedRepositoryFileEffectPlanError::TooManyArchives)
        );
        assert_eq!(builder.finish().archive_count(), 128);
    }

    #[test]
    fn archive_result_sha_encodings_use_bazel_field_forms() {
        assert_eq!(
            generated_repository_archive_sha256_hex([0; 32]),
            "00".repeat(32)
        );
        assert_eq!(
            generated_repository_archive_sha256_sri([0; 32]),
            "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
        );
    }

    #[test]
    fn archive_plan_debug_never_exposes_url_text() {
        let plan = archive_plan(
            urls(&["https://private.example/internal/dist/rustc.tar.xz"]),
            [3; 32],
            "rustc/rustc",
            b"file",
            false,
        );
        let debug = format!("{plan:?}");
        assert!(debug.contains("url_count: 1"));
        assert!(!debug.contains("private.example"));
        assert!(!debug.contains("internal/dist"));
    }
}
