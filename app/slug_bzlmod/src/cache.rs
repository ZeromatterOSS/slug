/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Module cache for bzlmod.
//!
//! This module provides caching for fetched modules from registries.
//! The cache is organized as:
//!
//! ```text
//! ~/.cache/slug/
//! ├── registry/
//! │   └── bcr.bazel.build/
//! │       └── modules/
//! │           └── rules_cc/
//! │               └── 0.0.9/
//! │                   ├── MODULE.bazel
//! │                   ├── source.json
//! │                   └── source/  (extracted source)
//! └── downloads/
//!     └── sha256-abc123...  (downloaded archives by hash)
//! ```

use std::path::Path;
use std::path::PathBuf;

use sha2::Digest;
use sha2::Sha256;
use slug_error::BuckErrorContext;

/// Errors that can occur during cache operations.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
pub enum CacheError {
    #[error("Failed to determine cache directory")]
    NoCacheDir,

    #[error("Failed to create cache directory: {path}")]
    CreateDirFailed { path: String },

    #[error("Cache read error: {0}")]
    ReadError(String),

    #[error("Cache write error: {0}")]
    WriteError(String),
}

/// Cache for bzlmod modules.
#[derive(Debug, Clone)]
pub struct ModuleCache {
    /// Base cache directory (e.g., ~/.cache/slug)
    base_dir: PathBuf,
}

impl ModuleCache {
    /// Create a new module cache with the default cache directory.
    pub fn new() -> slug_error::Result<Self> {
        let base_dir = Self::default_cache_dir()?;
        Self::with_base_dir(base_dir)
    }

    /// Create a new module cache with a custom base directory.
    pub fn with_base_dir(base_dir: PathBuf) -> slug_error::Result<Self> {
        // Ensure the base directory exists
        std::fs::create_dir_all(&base_dir).map_err(|_| CacheError::CreateDirFailed {
            path: base_dir.display().to_string(),
        })?;

        Ok(Self { base_dir })
    }

    /// Get the default cache directory.
    fn default_cache_dir() -> slug_error::Result<PathBuf> {
        // Use XDG_CACHE_HOME if set, otherwise ~/.cache
        if let Ok(cache_home) = std::env::var("XDG_CACHE_HOME") {
            Ok(PathBuf::from(cache_home).join("slug"))
        } else if let Some(home) = dirs::home_dir() {
            Ok(home.join(".cache").join("slug"))
        } else {
            Err(CacheError::NoCacheDir.into())
        }
    }

    /// Get the base cache directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Get the registry cache directory for a specific registry URL.
    pub fn registry_dir(&self, registry_url: &str) -> PathBuf {
        // Convert URL to directory name (e.g., "https://bcr.bazel.build" -> "bcr.bazel.build")
        let registry_name = registry_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/');
        self.base_dir.join("registry").join(registry_name)
    }

    /// Get the module directory for a specific module and version.
    pub fn module_dir(&self, registry_url: &str, name: &str, version: &str) -> PathBuf {
        self.registry_dir(registry_url)
            .join("modules")
            .join(name)
            .join(version)
    }

    /// Get the path for the cached MODULE.bazel file.
    pub fn module_bazel_path(&self, registry_url: &str, name: &str, version: &str) -> PathBuf {
        self.module_dir(registry_url, name, version)
            .join("MODULE.bazel")
    }

    /// Get the path for the cached source.json file.
    pub fn source_json_path(&self, registry_url: &str, name: &str, version: &str) -> PathBuf {
        self.module_dir(registry_url, name, version)
            .join("source.json")
    }

    /// Get the path for the extracted source directory.
    pub fn source_dir(&self, registry_url: &str, name: &str, version: &str) -> PathBuf {
        self.module_dir(registry_url, name, version).join("source")
    }

    /// Get the path for an extracted source directory with extra repository
    /// materialization identity, such as root override patches.
    pub fn source_dir_with_identity(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
        source_identity: Option<&str>,
    ) -> PathBuf {
        match source_identity {
            Some(identity) => self
                .module_dir(registry_url, name, version)
                .join(format!("source-{}", download_safe_name(identity))),
            None => self.source_dir(registry_url, name, version),
        }
    }

    /// Get the path for a downloaded file by its integrity hash.
    pub fn download_path(&self, integrity: &str) -> PathBuf {
        self.base_dir
            .join("downloads")
            .join(download_safe_name(integrity))
    }

    fn download_canonical_id_path(&self, integrity: &str, canonical_id: &str) -> PathBuf {
        self.base_dir.join("downloads").join(format!(
            "{}.canonical-{}",
            download_safe_name(integrity),
            canonical_id_digest(canonical_id)
        ))
    }

    /// Get the cache directory for a git override.
    pub fn git_override_dir(&self, git: &crate::types::GitOverride) -> PathBuf {
        self.git_override_dir_with_patch_digest(git, None)
    }

    /// Get the cache directory for a git override, including local patch bytes
    /// when override patches are present.
    pub fn git_override_dir_with_patch_digest(
        &self,
        git: &crate::types::GitOverride,
        patch_digest: Option<&str>,
    ) -> PathBuf {
        let source_identity = git_override_source_identity(git, patch_digest);
        self.base_dir
            .join("overrides")
            .join(&git.module_name)
            .join(format!("git-{}-{}", git.commit, source_identity))
    }

    /// Get the cache directory for an archive override.
    pub fn archive_override_dir(&self, archive: &crate::types::ArchiveOverride) -> PathBuf {
        self.archive_override_dir_with_patch_digest(archive, None)
    }

    /// Get the cache directory for an archive override, including local patch
    /// bytes when override patches are present.
    pub fn archive_override_dir_with_patch_digest(
        &self,
        archive: &crate::types::ArchiveOverride,
        patch_digest: Option<&str>,
    ) -> PathBuf {
        let source_identity = archive_override_source_identity(archive, patch_digest);
        self.base_dir
            .join("overrides")
            .join(&archive.module_name)
            .join(format!("archive-{}", source_identity))
    }

    /// Check if a module is cached.
    pub fn has_module(&self, registry_url: &str, name: &str, version: &str) -> bool {
        self.module_bazel_path(registry_url, name, version).exists()
    }

    /// Check if the extracted source is cached.
    pub fn has_source(&self, registry_url: &str, name: &str, version: &str) -> bool {
        self.source_dir(registry_url, name, version).exists()
    }

    /// Check if a download is cached by integrity hash.
    pub fn has_download(&self, integrity: &str) -> bool {
        self.download_path(integrity).exists()
    }

    /// Read cached MODULE.bazel content.
    pub fn read_module_bazel(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
    ) -> slug_error::Result<Option<String>> {
        let path = self.module_bazel_path(registry_url, name, version);
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .buck_error_context("Failed to read cached MODULE.bazel")?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    /// Write MODULE.bazel content to cache.
    pub fn write_module_bazel(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
        content: &str,
    ) -> slug_error::Result<()> {
        let path = self.module_bazel_path(registry_url, name, version);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| CacheError::CreateDirFailed {
                path: parent.display().to_string(),
            })?;
        }
        std::fs::write(&path, content)
            .buck_error_context("Failed to write MODULE.bazel to cache")?;
        Ok(())
    }

    /// Read cached source.json content.
    pub fn read_source_json(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
    ) -> slug_error::Result<Option<String>> {
        let path = self.source_json_path(registry_url, name, version);
        if path.exists() {
            let content =
                std::fs::read_to_string(&path).buck_error_context("Failed to read source.json")?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    /// Write source.json content to cache.
    pub fn write_source_json(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
        content: &str,
    ) -> slug_error::Result<()> {
        let path = self.source_json_path(registry_url, name, version);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| CacheError::CreateDirFailed {
                path: parent.display().to_string(),
            })?;
        }
        std::fs::write(&path, content)
            .buck_error_context("Failed to write source.json to cache")?;
        Ok(())
    }

    /// Read cached download by integrity hash.
    pub fn read_download(&self, integrity: &str) -> slug_error::Result<Option<Vec<u8>>> {
        let path = self.download_path(integrity);
        if path.exists() {
            let content =
                std::fs::read(&path).buck_error_context("Failed to read cached download")?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    /// Read cached download by integrity hash, restricted by canonical id when set.
    pub fn read_download_with_canonical_id(
        &self,
        integrity: &str,
        canonical_id: &str,
    ) -> slug_error::Result<Option<Vec<u8>>> {
        if !canonical_id.is_empty()
            && !self
                .download_canonical_id_path(integrity, canonical_id)
                .exists()
        {
            return Ok(None);
        }
        self.read_download(integrity)
    }

    /// Write download to cache by integrity hash.
    pub fn write_download(&self, integrity: &str, data: &[u8]) -> slug_error::Result<PathBuf> {
        let path = self.download_path(integrity);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| CacheError::CreateDirFailed {
                path: parent.display().to_string(),
            })?;
        }
        std::fs::write(&path, data).buck_error_context("Failed to write download to cache")?;
        Ok(path)
    }

    /// Write download to cache by integrity hash and associate a canonical id when set.
    pub fn write_download_with_canonical_id(
        &self,
        integrity: &str,
        canonical_id: &str,
        data: &[u8],
    ) -> slug_error::Result<PathBuf> {
        let path = self.write_download(integrity, data)?;
        if !canonical_id.is_empty() {
            let marker = self.download_canonical_id_path(integrity, canonical_id);
            std::fs::write(&marker, b"")
                .buck_error_context("Failed to write download canonical id marker")?;
        }
        Ok(path)
    }

    /// Create the source directory and return its path.
    pub fn create_source_dir(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
    ) -> slug_error::Result<PathBuf> {
        self.create_source_dir_with_identity(registry_url, name, version, None)
    }

    /// Create the source directory for an optional source identity and return its path.
    pub fn create_source_dir_with_identity(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
        source_identity: Option<&str>,
    ) -> slug_error::Result<PathBuf> {
        let path = self.source_dir_with_identity(registry_url, name, version, source_identity);
        std::fs::create_dir_all(&path).map_err(|_| CacheError::CreateDirFailed {
            path: path.display().to_string(),
        })?;
        Ok(path)
    }

    /// Mark a source extraction as complete by writing a marker file.
    pub fn mark_source_complete(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
    ) -> slug_error::Result<()> {
        self.mark_source_complete_with_identity(registry_url, name, version, None)
    }

    /// Mark a source extraction with optional extra identity as complete.
    pub fn mark_source_complete_with_identity(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
        source_identity: Option<&str>,
    ) -> slug_error::Result<()> {
        let marker = self
            .source_dir_with_identity(registry_url, name, version, source_identity)
            .join(".complete");
        std::fs::write(&marker, "").buck_error_context("Failed to write completion marker")?;
        Ok(())
    }

    /// Check if source extraction is complete.
    pub fn is_source_complete(&self, registry_url: &str, name: &str, version: &str) -> bool {
        self.is_source_complete_with_identity(registry_url, name, version, None)
    }

    /// Check if source extraction with optional extra identity is complete.
    pub fn is_source_complete_with_identity(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
        source_identity: Option<&str>,
    ) -> bool {
        self.source_dir_with_identity(registry_url, name, version, source_identity)
            .join(".complete")
            .exists()
    }
}

fn git_override_source_identity(
    git: &crate::types::GitOverride,
    patch_digest: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"slug-git-override-cache-v1");
    update_digest_str(&mut hasher, &git.remote);
    update_digest_str(&mut hasher, &git.commit);
    update_digest_optional_str(&mut hasher, git.shallow_since.as_deref());
    update_digest_str_list(&mut hasher, &git.patches);
    update_digest_u32(&mut hasher, git.patch_strip);
    update_digest_optional_str(&mut hasher, patch_digest);
    hex::encode(hasher.finalize())[..16].to_owned()
}

fn archive_override_source_identity(
    archive: &crate::types::ArchiveOverride,
    patch_digest: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"slug-archive-override-cache-v1");
    for url in &archive.urls {
        update_digest_str(&mut hasher, url);
    }
    update_digest_optional_str(&mut hasher, archive.integrity.as_deref());
    update_digest_optional_str(&mut hasher, archive.strip_prefix.as_deref());
    update_digest_str_list(&mut hasher, &archive.patches);
    update_digest_u32(&mut hasher, archive.patch_strip);
    update_digest_optional_str(&mut hasher, patch_digest);
    hex::encode(hasher.finalize())[..16].to_owned()
}

fn update_digest_str(hasher: &mut Sha256, value: &str) {
    hasher.update([0]);
    hasher.update(value.as_bytes());
}

fn update_digest_optional_str(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.as_bytes());
        }
        None => hasher.update([0]),
    }
}

fn update_digest_str_list(hasher: &mut Sha256, values: &[String]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        update_digest_str(hasher, value);
    }
}

fn update_digest_u32(hasher: &mut Sha256, value: u32) {
    hasher.update(value.to_le_bytes());
}

impl Default for ModuleCache {
    fn default() -> Self {
        Self::new().expect("Failed to create default module cache")
    }
}

fn download_safe_name(integrity: &str) -> String {
    // Integrity format often includes base64 punctuation. Keep the current
    // filename-compatible cache layout while preserving the logical cache key.
    integrity.replace(['/', '+', '='], "_")
}

fn canonical_id_digest(canonical_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_id.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn create_test_cache() -> (TempDir, ModuleCache) {
        let dir = TempDir::new().unwrap();
        let cache = ModuleCache::with_base_dir(dir.path().to_path_buf()).unwrap();
        (dir, cache)
    }

    #[test]
    fn test_registry_dir() {
        let (_dir, cache) = create_test_cache();
        let registry_dir = cache.registry_dir("https://bcr.bazel.build");
        assert!(registry_dir.ends_with("registry/bcr.bazel.build"));
    }

    #[test]
    fn test_module_dir() {
        let (_dir, cache) = create_test_cache();
        let module_dir = cache.module_dir("https://bcr.bazel.build", "rules_cc", "0.0.9");
        assert!(module_dir.ends_with("registry/bcr.bazel.build/modules/rules_cc/0.0.9"));
    }

    #[test]
    fn test_write_and_read_module_bazel() {
        let (_dir, cache) = create_test_cache();
        let content = "module(name = \"test\", version = \"1.0.0\")";

        cache
            .write_module_bazel("https://bcr.bazel.build", "test", "1.0.0", content)
            .unwrap();

        let read_content = cache
            .read_module_bazel("https://bcr.bazel.build", "test", "1.0.0")
            .unwrap();
        assert_eq!(read_content, Some(content.to_string()));
    }

    #[test]
    fn test_has_module() {
        let (_dir, cache) = create_test_cache();

        assert!(!cache.has_module("https://bcr.bazel.build", "test", "1.0.0"));

        cache
            .write_module_bazel("https://bcr.bazel.build", "test", "1.0.0", "content")
            .unwrap();

        assert!(cache.has_module("https://bcr.bazel.build", "test", "1.0.0"));
    }

    #[test]
    fn test_download_path() {
        let (_dir, cache) = create_test_cache();
        let path = cache.download_path("sha256-abc123+def/ghi=");
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert!(filename.contains("sha256-abc123_def_ghi_"));
    }

    #[test]
    fn git_override_dir_includes_source_identity() {
        let (_dir, cache) = create_test_cache();
        let base = crate::types::GitOverride {
            module_name: "dep".to_owned(),
            remote: "https://example.invalid/one.git".to_owned(),
            commit: "abcdef".to_owned(),
            shallow_since: None,
            patches: Vec::new(),
            patch_strip: 0,
        };
        let same = crate::types::GitOverride {
            remote: base.remote.clone(),
            ..base.clone()
        };
        let different_remote = crate::types::GitOverride {
            remote: "https://example.invalid/two.git".to_owned(),
            ..base.clone()
        };
        let different_patches = crate::types::GitOverride {
            patches: vec!["//:fix.patch".to_owned()],
            ..base.clone()
        };
        let different_patch_strip = crate::types::GitOverride {
            patch_strip: 1,
            ..base.clone()
        };

        assert_eq!(cache.git_override_dir(&base), cache.git_override_dir(&same));
        assert_ne!(
            cache.git_override_dir(&base),
            cache.git_override_dir(&different_remote)
        );
        assert_ne!(
            cache.git_override_dir(&base),
            cache.git_override_dir(&different_patches)
        );
        assert_ne!(
            cache.git_override_dir(&base),
            cache.git_override_dir(&different_patch_strip)
        );
        assert_ne!(
            cache.git_override_dir_with_patch_digest(&different_patches, Some("digest-one")),
            cache.git_override_dir_with_patch_digest(&different_patches, Some("digest-two"))
        );
    }

    #[test]
    fn archive_override_dir_includes_extraction_identity() {
        let (_dir, cache) = create_test_cache();
        let base = crate::types::ArchiveOverride {
            module_name: "dep".to_owned(),
            urls: vec!["https://example.invalid/archive.tar.gz".to_owned()],
            integrity: Some("sha256-example".to_owned()),
            strip_prefix: Some("one".to_owned()),
            patches: Vec::new(),
            patch_strip: 0,
        };
        let same = crate::types::ArchiveOverride {
            strip_prefix: base.strip_prefix.clone(),
            ..base.clone()
        };
        let different_strip_prefix = crate::types::ArchiveOverride {
            strip_prefix: Some("two".to_owned()),
            ..base.clone()
        };
        let different_patches = crate::types::ArchiveOverride {
            patches: vec!["//:fix.patch".to_owned()],
            ..base.clone()
        };
        let different_patch_strip = crate::types::ArchiveOverride {
            patch_strip: 1,
            ..base.clone()
        };

        assert_eq!(
            cache.archive_override_dir(&base),
            cache.archive_override_dir(&same)
        );
        assert_ne!(
            cache.archive_override_dir(&base),
            cache.archive_override_dir(&different_strip_prefix)
        );
        assert_ne!(
            cache.archive_override_dir(&base),
            cache.archive_override_dir(&different_patches)
        );
        assert_ne!(
            cache.archive_override_dir(&base),
            cache.archive_override_dir(&different_patch_strip)
        );
        assert_ne!(
            cache.archive_override_dir_with_patch_digest(&different_patches, Some("digest-one")),
            cache.archive_override_dir_with_patch_digest(&different_patches, Some("digest-two"))
        );
    }

    #[test]
    fn test_write_and_read_download() {
        let (_dir, cache) = create_test_cache();
        let data = b"test archive data";

        cache.write_download("sha256-test123", data).unwrap();

        let read_data = cache.read_download("sha256-test123").unwrap();
        assert_eq!(read_data, Some(data.to_vec()));
    }

    #[test]
    fn test_download_canonical_id_restricts_cache_hits() {
        let (_dir, cache) = create_test_cache();
        let data = b"test archive data";

        cache
            .write_download_with_canonical_id("sha256-test123", "repo-a", data)
            .unwrap();

        assert_eq!(
            cache
                .read_download_with_canonical_id("sha256-test123", "repo-a")
                .unwrap(),
            Some(data.to_vec())
        );
        assert_eq!(
            cache
                .read_download_with_canonical_id("sha256-test123", "repo-b")
                .unwrap(),
            None
        );
        assert_eq!(
            cache
                .read_download_with_canonical_id("sha256-test123", "")
                .unwrap(),
            Some(data.to_vec())
        );

        cache.write_download("sha256-plain", data).unwrap();
        assert_eq!(
            cache
                .read_download_with_canonical_id("sha256-plain", "repo-a")
                .unwrap(),
            None
        );
    }

    #[test]
    fn test_source_complete_marker() {
        let (_dir, cache) = create_test_cache();

        assert!(!cache.is_source_complete("https://bcr.bazel.build", "test", "1.0.0"));

        cache
            .create_source_dir("https://bcr.bazel.build", "test", "1.0.0")
            .unwrap();
        cache
            .mark_source_complete("https://bcr.bazel.build", "test", "1.0.0")
            .unwrap();

        assert!(cache.is_source_complete("https://bcr.bazel.build", "test", "1.0.0"));
    }
}
