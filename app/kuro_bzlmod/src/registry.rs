/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel Central Registry (BCR) client for bzlmod.
//!
//! This module implements fetching module information from the Bazel Central Registry
//! and other compatible registries.
//!
//! # BCR URL Structure
//!
//! ```text
//! https://bcr.bazel.build/modules/{name}/metadata.json
//! https://bcr.bazel.build/modules/{name}/{version}/MODULE.bazel
//! https://bcr.bazel.build/modules/{name}/{version}/source.json
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use kuro_error::BuckErrorContext;
use kuro_http::HttpClient;
use kuro_http::HttpClientBuilder;
use kuro_http::to_bytes;
use serde::Deserialize;
use serde::Serialize;

use crate::cache::ModuleCache;
use crate::version::Version;

/// Default BCR URL.
pub const DEFAULT_REGISTRY_URL: &str = "https://bcr.bazel.build";

/// Errors that can occur during registry operations.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
pub enum RegistryError {
    #[error("Module not found in registry: {name}@{version}")]
    ModuleNotFound { name: String, version: String },

    #[error("Module metadata not found: {name}")]
    MetadataNotFound { name: String },

    #[error("Invalid registry response for {url}: {reason}")]
    InvalidResponse { url: String, reason: String },

    #[error("Failed to fetch from registry: {url}")]
    FetchFailed { url: String },

    #[error("Registry URL is invalid: {url}")]
    InvalidUrl { url: String },

    #[error("No version of module '{name}' satisfies requirement {requirement}")]
    NoMatchingVersion { name: String, requirement: String },
}

/// Module metadata from the registry.
///
/// This corresponds to the `metadata.json` file in the BCR.
#[derive(Debug, Clone, Serialize, Deserialize, Allocative)]
pub struct ModuleMetadata {
    /// Homepage URL for the module.
    pub homepage: Option<String>,

    /// List of maintainers.
    pub maintainers: Option<Vec<Maintainer>>,

    /// Repository URL.
    pub repository: Option<Vec<String>>,

    /// Available versions of the module.
    pub versions: Vec<String>,

    /// Yanked versions (should not be used).
    #[serde(default)]
    pub yanked_versions: HashMap<String, String>,
}

/// Maintainer information.
#[derive(Debug, Clone, Serialize, Deserialize, Allocative)]
pub struct Maintainer {
    /// Maintainer name.
    pub name: Option<String>,

    /// Maintainer email.
    pub email: Option<String>,

    /// GitHub username.
    pub github: Option<String>,
}

/// Source information from the registry.
///
/// This corresponds to the `source.json` file in the BCR.
#[derive(Debug, Clone, Serialize, Deserialize, Allocative)]
pub struct SourceInfo {
    /// Type of source (usually "archive" or "git_repository").
    #[serde(rename = "type")]
    pub source_type: Option<String>,

    /// URL(s) to download the source archive.
    pub url: Option<String>,

    /// Additional URLs (for archive type).
    pub urls: Option<Vec<String>>,

    /// Subresource Integrity hash (e.g., "sha256-base64hash").
    pub integrity: Option<String>,

    /// Directory prefix to strip from archive.
    pub strip_prefix: Option<String>,

    /// Patches to apply after extraction.
    #[serde(default)]
    pub patches: HashMap<String, String>,

    /// Number of leading path components to strip from patch paths.
    #[serde(default)]
    pub patch_strip: u32,

    /// For git_repository type: the remote URL.
    pub remote: Option<String>,

    /// For git_repository type: the commit hash.
    pub commit: Option<String>,

    /// For git_repository type: shallow_since date for faster clones.
    pub shallow_since: Option<String>,
}

impl SourceInfo {
    /// Get all URLs for downloading the source archive.
    pub fn get_urls(&self) -> Vec<String> {
        let mut result = Vec::new();
        if let Some(url) = &self.url {
            result.push(url.clone());
        }
        if let Some(urls) = &self.urls {
            result.extend(urls.clone());
        }
        result
    }

    /// Check if this is a git repository source.
    pub fn is_git(&self) -> bool {
        self.source_type.as_deref() == Some("git_repository") || self.remote.is_some()
    }

    /// Check if this is an archive source.
    pub fn is_archive(&self) -> bool {
        !self.is_git()
    }
}

/// Registry client for fetching module information.
#[derive(Clone, Dupe, Allocative)]
pub struct RegistryClient {
    /// Base URL of the registry.
    #[allocative(skip)]
    base_url: Arc<str>,

    /// HTTP client for making requests.
    #[allocative(skip)]
    http_client: Arc<HttpClient>,

    /// Module cache.
    #[allocative(skip)]
    cache: Arc<ModuleCache>,
}

impl RegistryClient {
    /// Create a new registry client with the given base URL.
    pub async fn new(base_url: &str, cache: ModuleCache) -> kuro_error::Result<Self> {
        let http_client = HttpClientBuilder::https_with_system_roots()
            .await?
            .with_max_redirects(10)
            .build();

        Ok(Self {
            base_url: Arc::from(base_url.trim_end_matches('/')),
            http_client: Arc::new(http_client),
            cache: Arc::new(cache),
        })
    }

    /// Create a new registry client for the default BCR.
    pub async fn bcr(cache: ModuleCache) -> kuro_error::Result<Self> {
        Self::new(DEFAULT_REGISTRY_URL, cache).await
    }

    /// Get the base URL of this registry.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Fetch module metadata (available versions, maintainers, etc.).
    pub async fn fetch_metadata(&self, name: &str) -> kuro_error::Result<ModuleMetadata> {
        let url = format!("{}/modules/{}/metadata.json", self.base_url, name);
        tracing::debug!("Fetching module metadata from {}", url);

        let response = self
            .http_client
            .get(&url)
            .await
            .buck_error_context("Failed to fetch module metadata")?;

        let body = to_bytes(response.into_body()).await?;
        let metadata: ModuleMetadata =
            serde_json::from_slice(&body).map_err(|e| RegistryError::InvalidResponse {
                url: url.clone(),
                reason: e.to_string(),
            })?;

        Ok(metadata)
    }

    /// Fetch MODULE.bazel content for a specific version.
    pub async fn fetch_module_bazel(
        &self,
        name: &str,
        version: &str,
    ) -> kuro_error::Result<String> {
        // Check cache first
        if let Some(cached) = self
            .cache
            .read_module_bazel(&self.base_url, name, version)?
        {
            tracing::debug!("Using cached MODULE.bazel for {}@{}", name, version);
            return Ok(cached);
        }

        let url = format!(
            "{}/modules/{}/{}/MODULE.bazel",
            self.base_url, name, version
        );
        tracing::debug!("Fetching MODULE.bazel from {}", url);

        let response = self
            .http_client
            .get(&url)
            .await
            .buck_error_context("Failed to fetch MODULE.bazel")?;

        let body = to_bytes(response.into_body()).await?;
        let content =
            String::from_utf8(body.to_vec()).map_err(|_| RegistryError::InvalidResponse {
                url: url.clone(),
                reason: "MODULE.bazel is not valid UTF-8".to_string(),
            })?;

        // Cache the result
        self.cache
            .write_module_bazel(&self.base_url, name, version, &content)?;

        Ok(content)
    }

    /// Fetch source.json for a specific version.
    pub async fn fetch_source_info(
        &self,
        name: &str,
        version: &str,
    ) -> kuro_error::Result<SourceInfo> {
        // Check cache first
        if let Some(cached) = self.cache.read_source_json(&self.base_url, name, version)? {
            tracing::debug!("Using cached source.json for {}@{}", name, version);
            let source_info: SourceInfo =
                serde_json::from_str(&cached).buck_error_context("Failed to parse source.json")?;
            return Ok(source_info);
        }

        let url = format!("{}/modules/{}/{}/source.json", self.base_url, name, version);
        tracing::debug!("Fetching source.json from {}", url);

        let response = self
            .http_client
            .get(&url)
            .await
            .buck_error_context("Failed to fetch source.json")?;

        let body = to_bytes(response.into_body()).await?;
        let content =
            String::from_utf8(body.to_vec()).map_err(|_| RegistryError::InvalidResponse {
                url: url.clone(),
                reason: "source.json is not valid UTF-8".to_string(),
            })?;

        let source_info: SourceInfo =
            serde_json::from_str(&content).map_err(|e| RegistryError::InvalidResponse {
                url: url.clone(),
                reason: e.to_string(),
            })?;

        // Cache the result
        self.cache
            .write_source_json(&self.base_url, name, version, &content)?;

        Ok(source_info)
    }

    /// Check if a module version exists in the registry.
    pub async fn has_version(&self, name: &str, version: &str) -> kuro_error::Result<bool> {
        // Check cache first
        if self.cache.has_module(&self.base_url, name, version) {
            return Ok(true);
        }

        // Otherwise, fetch metadata and check versions list
        match self.fetch_metadata(name).await {
            Ok(metadata) => Ok(metadata.versions.contains(&version.to_string())),
            Err(_) => Ok(false),
        }
    }

    /// Find the best matching version for a version requirement.
    ///
    /// For now, this returns the exact version if it exists.
    /// TODO: Implement proper version matching (semver ranges, etc.)
    pub async fn find_best_version(
        &self,
        name: &str,
        requirement: &str,
    ) -> kuro_error::Result<String> {
        let metadata = self.fetch_metadata(name).await?;

        // Check if exact version exists
        if metadata.versions.contains(&requirement.to_string()) {
            // Check if yanked
            if metadata.yanked_versions.contains_key(requirement) {
                tracing::warn!(
                    "Module {}@{} is yanked: {}",
                    name,
                    requirement,
                    metadata.yanked_versions.get(requirement).unwrap()
                );
            }
            return Ok(requirement.to_string());
        }

        // If exact version not found, return error
        // TODO: Implement semver range matching
        Err(RegistryError::NoMatchingVersion {
            name: name.to_string(),
            requirement: requirement.to_string(),
        }
        .into())
    }

    /// Get the path to the cached source directory for a module.
    pub fn source_path(&self, name: &str, version: &str) -> PathBuf {
        self.cache.source_dir(&self.base_url, name, version)
    }

    /// Check if the source is already fetched and extracted.
    pub fn has_source(&self, name: &str, version: &str) -> bool {
        self.cache.is_source_complete(&self.base_url, name, version)
    }

    /// Get the cache for direct access.
    pub fn cache(&self) -> &ModuleCache {
        &self.cache
    }
}

/// A resolved module from the registry.
#[derive(Debug, Clone, Allocative)]
pub struct ResolvedRegistryModule {
    /// The module name.
    pub name: String,

    /// The resolved version.
    pub version: Version,

    /// The registry URL this was fetched from.
    pub registry_url: String,

    /// Path to the extracted source directory.
    pub source_path: PathBuf,

    /// The source info for fetching.
    #[allocative(skip)]
    pub source_info: SourceInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_info_get_urls() {
        let source = SourceInfo {
            source_type: None,
            url: Some("https://example.com/a.tar.gz".to_string()),
            urls: Some(vec!["https://mirror.com/a.tar.gz".to_string()]),
            integrity: None,
            strip_prefix: None,
            patches: HashMap::new(),
            patch_strip: 0,
            remote: None,
            commit: None,
            shallow_since: None,
        };

        let urls = source.get_urls();
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://example.com/a.tar.gz");
        assert_eq!(urls[1], "https://mirror.com/a.tar.gz");
    }

    #[test]
    fn test_source_info_is_git() {
        let archive_source = SourceInfo {
            source_type: None,
            url: Some("https://example.com/a.tar.gz".to_string()),
            urls: None,
            integrity: Some("sha256-abc".to_string()),
            strip_prefix: None,
            patches: HashMap::new(),
            patch_strip: 0,
            remote: None,
            commit: None,
            shallow_since: None,
        };
        assert!(!archive_source.is_git());
        assert!(archive_source.is_archive());

        let git_source = SourceInfo {
            source_type: Some("git_repository".to_string()),
            url: None,
            urls: None,
            integrity: None,
            strip_prefix: None,
            patches: HashMap::new(),
            patch_strip: 0,
            remote: Some("https://github.com/example/repo.git".to_string()),
            commit: Some("abc123".to_string()),
            shallow_since: None,
        };
        assert!(git_source.is_git());
        assert!(!git_source.is_archive());
    }

    #[test]
    fn test_parse_source_info() {
        let json = r#"{
            "url": "https://github.com/bazelbuild/rules_cc/releases/download/0.0.9/rules_cc-0.0.9.tar.gz",
            "integrity": "sha256-wLoLQVeHb/8a/so988MhVoaxM6HOYQ3MDYE7Z9pd1TI=",
            "strip_prefix": "rules_cc-0.0.9"
        }"#;

        let source: SourceInfo = serde_json::from_str(json).unwrap();
        assert!(source.url.is_some());
        assert!(source.integrity.is_some());
        assert_eq!(source.strip_prefix, Some("rules_cc-0.0.9".to_string()));
        assert!(source.is_archive());
    }

    #[test]
    fn test_parse_metadata() {
        let json = r#"{
            "homepage": "https://github.com/bazelbuild/rules_cc",
            "versions": ["0.0.1", "0.0.2", "0.0.9"],
            "yanked_versions": {
                "0.0.1": "Known issues with this version"
            }
        }"#;

        let metadata: ModuleMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.versions.len(), 3);
        assert!(metadata.yanked_versions.contains_key("0.0.1"));
    }
}
