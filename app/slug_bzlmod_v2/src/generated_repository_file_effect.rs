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
use base64::engine::general_purpose::STANDARD;
use compact_str::CompactString;
use starlark_map::small_set::SmallSet;
use url::Url;

const MAX_ARCHIVES: usize = 6;
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

/// The one archive format admitted for the selected Rust host-tools repository.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum GeneratedRepositoryArchiveFormat {
    TarXz,
}

/// Bazel's omitted `output` argument extracts into the repository root.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum GeneratedRepositoryArchiveOutput {
    RepositoryRoot,
}

/// A selected, checksum-pinned archive intent. No archive bytes or physical
/// capture path are retained in the repository observation.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Hash, Allocative)]
pub struct GeneratedRepositoryArchiveEffect {
    urls: Arc<[CompactString]>,
    sha256: [u8; 32],
    strip_prefix: CompactString,
    format: GeneratedRepositoryArchiveFormat,
    output: GeneratedRepositoryArchiveOutput,
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

/// Result fields for a checksum-pinned selected archive, without requiring
/// Loading to own a separate hex or base64 dependency.
#[doc(hidden)]
pub fn generated_repository_archive_sha256_hex(sha256: [u8; 32]) -> String {
    hex::encode(sha256)
}

#[doc(hidden)]
pub fn generated_repository_archive_sha256_sri(sha256: [u8; 32]) -> String {
    format!("sha256-{}", STANDARD.encode(sha256))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
enum GeneratedRepositoryOrderItem {
    File(usize),
    Archive(GeneratedRepositoryArchiveEffect),
}

/// A borrowed effect in invocation order; file bytes remain owned only by the
/// plan's file slice rather than being cloned into this order projection.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub enum GeneratedRepositoryOrderedEffectRef<'a> {
    File(&'a GeneratedRepositoryFileEffect),
    Archive(&'a GeneratedRepositoryArchiveEffect),
}

/// Ordered, structural output of one selected repository-rule invocation.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct GeneratedRepositoryFileEffectPlan {
    files: Arc<[GeneratedRepositoryFileEffect]>,
    order: Arc<[GeneratedRepositoryOrderItem]>,
}

impl GeneratedRepositoryFileEffectPlan {
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
    TooManyArchives,
    EmptyArchiveUrls,
    TooManyArchiveUrls,
    InvalidArchiveUrl,
    InvalidArchiveStripPrefix,
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
    pub fn push(
        &mut self,
        path: CompactString,
        content: Arc<[u8]>,
        executable: bool,
    ) -> Result<(), GeneratedRepositoryFileEffectPlanError> {
        if !valid_path(&path) {
            return Err(GeneratedRepositoryFileEffectPlanError::InvalidPath(path));
        }
        if !self.seen.insert(path.clone()) {
            return Err(GeneratedRepositoryFileEffectPlanError::RepeatedPath(path));
        }
        self.order
            .push(GeneratedRepositoryOrderItem::File(self.files.len()));
        self.files.push(GeneratedRepositoryFileEffect {
            path,
            content,
            executable,
        });
        Ok(())
    }

    pub fn push_archive(
        &mut self,
        urls: Arc<[CompactString]>,
        sha256: [u8; 32],
        strip_prefix: CompactString,
    ) -> Result<(), GeneratedRepositoryFileEffectPlanError> {
        if self.archive_count == MAX_ARCHIVES {
            return Err(GeneratedRepositoryFileEffectPlanError::TooManyArchives);
        }
        if urls.is_empty() {
            return Err(GeneratedRepositoryFileEffectPlanError::EmptyArchiveUrls);
        }
        if urls.len() > MAX_ARCHIVE_URLS {
            return Err(GeneratedRepositoryFileEffectPlanError::TooManyArchiveUrls);
        }
        if urls.iter().any(|url| !valid_archive_url(url)) {
            return Err(GeneratedRepositoryFileEffectPlanError::InvalidArchiveUrl);
        }
        if strip_prefix.len() > MAX_ARCHIVE_PREFIX_BYTES
            || strip_prefix.split('/').count() > MAX_ARCHIVE_PREFIX_COMPONENTS
            || !valid_path(&strip_prefix)
        {
            return Err(GeneratedRepositoryFileEffectPlanError::InvalidArchiveStripPrefix);
        }
        self.order.push(GeneratedRepositoryOrderItem::Archive(
            GeneratedRepositoryArchiveEffect {
                urls,
                sha256,
                strip_prefix,
                format: GeneratedRepositoryArchiveFormat::TarXz,
                output: GeneratedRepositoryArchiveOutput::RepositoryRoot,
            },
        ));
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
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !parsed.path().ends_with(".tar.xz")
    {
        return false;
    }
    match parsed.scheme() {
        "https" => parsed.host_str().is_some(),
        "file" => parsed.host_str().is_none_or(|host| host.is_empty()),
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

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Arc;

    use compact_str::CompactString;

    use super::GeneratedRepositoryArchiveFormat;
    use super::GeneratedRepositoryArchiveOutput;
    use super::GeneratedRepositoryFileEffectPlan;
    use super::GeneratedRepositoryFileEffectPlanError;
    use super::GeneratedRepositoryOrderedEffectRef;
    use super::generated_repository_archive_sha256_hex;
    use super::generated_repository_archive_sha256_sri;

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
    fn plan_rejects_non_normal_and_repeated_paths_at_first_occurrence() {
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
        assert!(matches!(
            GeneratedRepositoryFileEffectPlan::build([
                (CompactString::new("a"), Arc::from(&b"one"[..]), true),
                (CompactString::new("a"), Arc::from(&b"two"[..]), false),
            ]),
            Err(GeneratedRepositoryFileEffectPlanError::RepeatedPath(path)) if path == "a"
        ));
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
    fn archive_intent_rejects_unselected_shapes_and_enforces_six_call_cap() {
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
            "https://static.rust-lang.org/dist/rustc.tar.xz?token=secret",
            "https://static.rust-lang.org/dist/rustc.zip",
            "file://remote-host/tmp/rustc.tar.xz",
        ] {
            assert_eq!(
                builder.push_archive(urls(&[invalid]), [0; 32], CompactString::new("rustc/rustc")),
                Err(GeneratedRepositoryFileEffectPlanError::InvalidArchiveUrl)
            );
        }
        for invalid in ["", "/rustc", "rustc/../bin", "rustc//bin"] {
            assert_eq!(
                builder.push_archive(
                    urls(&["file:///tmp/rustc.tar.xz"]),
                    [0; 32],
                    CompactString::new(invalid),
                ),
                Err(GeneratedRepositoryFileEffectPlanError::InvalidArchiveStripPrefix)
            );
        }
        for _ in 0..6 {
            builder
                .push_archive(
                    urls(&["file:///tmp/rustc.tar.xz"]),
                    [0; 32],
                    CompactString::new("rustc/rustc"),
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
        assert_eq!(builder.finish().archive_count(), 6);
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
