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
//! ~/.cache/kuro/
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

use kuro_error::BuckErrorContext;

/// Errors that can occur during cache operations.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
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
    /// Base cache directory (e.g., ~/.cache/kuro)
    base_dir: PathBuf,
}

impl ModuleCache {
    /// Create a new module cache with the default cache directory.
    pub fn new() -> kuro_error::Result<Self> {
        let base_dir = Self::default_cache_dir()?;
        Self::with_base_dir(base_dir)
    }

    /// Create a new module cache with a custom base directory.
    pub fn with_base_dir(base_dir: PathBuf) -> kuro_error::Result<Self> {
        // Ensure the base directory exists
        std::fs::create_dir_all(&base_dir).map_err(|_| CacheError::CreateDirFailed {
            path: base_dir.display().to_string(),
        })?;

        Ok(Self { base_dir })
    }

    /// Get the default cache directory.
    fn default_cache_dir() -> kuro_error::Result<PathBuf> {
        // Use XDG_CACHE_HOME if set, otherwise ~/.cache
        if let Ok(cache_home) = std::env::var("XDG_CACHE_HOME") {
            Ok(PathBuf::from(cache_home).join("kuro"))
        } else if let Some(home) = dirs::home_dir() {
            Ok(home.join(".cache").join("kuro"))
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

    /// Get the path for a downloaded file by its integrity hash.
    pub fn download_path(&self, integrity: &str) -> PathBuf {
        // Integrity format: "sha256-base64hash"
        // Convert to filename-safe format
        let safe_name = integrity.replace(['/', '+', '='], "_");
        self.base_dir.join("downloads").join(safe_name)
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
    ) -> kuro_error::Result<Option<String>> {
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
    ) -> kuro_error::Result<()> {
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
    ) -> kuro_error::Result<Option<String>> {
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
    ) -> kuro_error::Result<()> {
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
    pub fn read_download(&self, integrity: &str) -> kuro_error::Result<Option<Vec<u8>>> {
        let path = self.download_path(integrity);
        if path.exists() {
            let content =
                std::fs::read(&path).buck_error_context("Failed to read cached download")?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    /// Write download to cache by integrity hash.
    pub fn write_download(&self, integrity: &str, data: &[u8]) -> kuro_error::Result<PathBuf> {
        let path = self.download_path(integrity);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| CacheError::CreateDirFailed {
                path: parent.display().to_string(),
            })?;
        }
        std::fs::write(&path, data).buck_error_context("Failed to write download to cache")?;
        Ok(path)
    }

    /// Create the source directory and return its path.
    pub fn create_source_dir(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
    ) -> kuro_error::Result<PathBuf> {
        let path = self.source_dir(registry_url, name, version);
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
    ) -> kuro_error::Result<()> {
        let marker = self
            .source_dir(registry_url, name, version)
            .join(".complete");
        std::fs::write(&marker, "").buck_error_context("Failed to write completion marker")?;
        Ok(())
    }

    /// Check if source extraction is complete.
    pub fn is_source_complete(&self, registry_url: &str, name: &str, version: &str) -> bool {
        self.source_dir(registry_url, name, version)
            .join(".complete")
            .exists()
    }
}

impl Default for ModuleCache {
    fn default() -> Self {
        Self::new().expect("Failed to create default module cache")
    }
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
    fn test_write_and_read_download() {
        let (_dir, cache) = create_test_cache();
        let data = b"test archive data";

        cache.write_download("sha256-test123", data).unwrap();

        let read_data = cache.read_download("sha256-test123").unwrap();
        assert_eq!(read_data, Some(data.to_vec()));
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
