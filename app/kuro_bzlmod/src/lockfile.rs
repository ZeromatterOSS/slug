/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! MODULE.bazel.lock file handling.
//!
//! The lockfile caches dependency resolution results to avoid re-resolving
//! on every build. It also provides reproducibility guarantees by recording
//! the exact versions and integrity hashes of all dependencies.
//!
//! # Lockfile Format
//!
//! The lockfile is a JSON file compatible with Bazel's MODULE.bazel.lock format:
//!
//! ```json
//! {
//!   "lockFileVersion": 24,
//!   "registryFileHashes": {
//!     "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel": "sha256-..."
//!   },
//!   "selectedYankedVersions": {},
//!   "moduleDepGraph": {
//!     "rules_cc@0.0.9": {
//!       "name": "rules_cc",
//!       "version": "0.0.9",
//!       "compatibilityLevel": 0,
//!       "dependencies": {}
//!     }
//!   }
//! }
//! ```

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use kuro_error::BuckErrorContext;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::repo_spec::RepoSpec;
use crate::repository_invocations::AttrValue;
use crate::resolution::ModuleSource;
use crate::resolution::ResolvedGraph;
use crate::resolution::ResolvedModuleInfo;
use crate::types::Module;

/// Current lockfile format version.
/// This matches Bazel 9.0's lockfile version.
pub const LOCKFILE_VERSION: u32 = 24;

/// Errors that can occur during lockfile operations.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
pub enum LockfileError {
    #[error("Lockfile not found at {0}")]
    NotFound(String),

    #[error("Failed to read lockfile: {0}")]
    ReadError(String),

    #[error("Failed to write lockfile: {0}")]
    WriteError(String),

    #[error("Failed to parse lockfile: {0}")]
    ParseError(String),

    #[error("Lockfile version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },

    #[error(
        "Lockfile is stale: MODULE.bazel has changed. \
        Run 'kuro mod update' to update the lockfile."
    )]
    StaleLockfile,

    #[error(
        "Lockfile would change but --lockfile_mode=error was specified. \
        Run 'kuro mod update' to update the lockfile."
    )]
    LockfileModeError,
}

/// The MODE.bazel.lock file content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lockfile {
    /// Lockfile format version.
    pub lock_file_version: u32,

    /// Hash of the root MODULE.bazel file.
    #[serde(default)]
    pub module_file_hash: String,

    /// Map from registry file URL to its integrity hash.
    /// Keys are URLs like "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel"
    /// Values are SRI hashes like "sha256-base64encodedHash"
    #[serde(default)]
    pub registry_file_hashes: HashMap<String, String>,

    /// Map of yanked versions that were explicitly allowed.
    /// Keys are "module@version", values are the yanked reason.
    #[serde(default)]
    pub selected_yanked_versions: HashMap<String, String>,

    /// The resolved module dependency graph.
    /// Keys are "module@version" or just "module" for single-version modules.
    #[serde(default)]
    pub module_dep_graph: HashMap<String, LockfileModuleNode>,

    /// Module extension results.
    /// Keys are extension identifiers.
    #[serde(default)]
    pub module_extensions: HashMap<String, LockfileExtensionData>,

    /// Repository rule execution results.
    /// Keys are repository names (e.g., "rules_cc").
    #[serde(default)]
    pub repository_rules: HashMap<String, RepositoryRuleLockEntry>,
}

/// A module node in the lockfile dependency graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockfileModuleNode {
    /// Module name.
    pub name: String,

    /// Module version.
    pub version: String,

    /// Compatibility level.
    #[serde(default)]
    pub compatibility_level: u32,

    /// Direct dependencies (module name -> version).
    #[serde(default)]
    pub dependencies: HashMap<String, String>,

    /// Registry URL this module was fetched from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,

    /// For non-registry modules, the source type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,

    /// For local path overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,

    /// For git overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_remote: Option<String>,

    /// Git commit hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,

    /// For archive overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_urls: Option<Vec<String>>,
}

/// Module extension data in the lockfile (Bazel-compatible format).
///
/// This structure matches Bazel's MODULE.bazel.lock format for extensions,
/// allowing for potential OS-specific extension data in the future.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LockfileExtensionData {
    /// General extension data (not OS-specific).
    /// This is the primary extension data for most use cases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub general: Option<LockfileExtensionGeneral>,
}

/// General (non-OS-specific) extension lock data.
///
/// Contains the information needed to validate cached extension results
/// and the actual generated repository specifications.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LockfileExtensionGeneral {
    /// Transitive digest of all .bzl files the extension depends on.
    /// Used for cache invalidation when extension code changes.
    pub bzl_transitive_digest: String,

    /// Digest of all module usages (tags passed to the extension).
    /// Used for cache invalidation when extension inputs change.
    pub usages_digest: String,

    /// Generated repository specifications.
    /// Keys are internal names (e.g., "numpy"), values are full RepoSpec data.
    #[serde(default)]
    pub generated_repo_specs: HashMap<String, LockfileRepoSpec>,
}

/// A repository specification in the lockfile (Bazel-compatible format).
///
/// This represents a repository that will be created by a module extension,
/// storing the full rule identity and attributes for lazy execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LockfileRepoSpec {
    /// Repository rule identifier.
    /// Format: "@@module//path:file.bzl%rule_name"
    /// Example: "@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive"
    pub repo_rule_id: String,

    /// All attributes (serialized as JSON values).
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}

impl LockfileExtensionData {
    /// Create a new extension data with general info.
    pub fn new(
        bzl_transitive_digest: String,
        usages_digest: String,
        generated_repo_specs: HashMap<String, LockfileRepoSpec>,
    ) -> Self {
        Self {
            general: Some(LockfileExtensionGeneral {
                bzl_transitive_digest,
                usages_digest,
                generated_repo_specs,
            }),
        }
    }

    /// Check if the cached data is valid for the given digests.
    ///
    /// Returns true if both the bzl_transitive_digest and usages_digest match.
    pub fn is_valid(&self, bzl_transitive_digest: &str, usages_digest: &str) -> bool {
        match &self.general {
            Some(general) => {
                general.bzl_transitive_digest == bzl_transitive_digest
                    && general.usages_digest == usages_digest
            }
            None => false,
        }
    }

    /// Get the generated repo specs if valid.
    pub fn get_repo_specs(&self) -> Option<&HashMap<String, LockfileRepoSpec>> {
        self.general.as_ref().map(|g| &g.generated_repo_specs)
    }
}

impl LockfileRepoSpec {
    /// Create a new lockfile repo spec.
    pub fn new(repo_rule_id: String) -> Self {
        Self {
            repo_rule_id,
            attributes: HashMap::new(),
        }
    }

    /// Add an attribute.
    pub fn with_attr(mut self, key: String, value: serde_json::Value) -> Self {
        self.attributes.insert(key, value);
        self
    }

    /// Create from a RepoSpec.
    pub fn from_repo_spec(spec: &RepoSpec) -> Self {
        Self {
            repo_rule_id: spec.repo_rule_id.clone(),
            attributes: spec
                .attributes
                .iter()
                .map(|(k, v)| (k.clone(), attr_value_to_json(v)))
                .collect(),
        }
    }

    /// Convert to a RepoSpec.
    pub fn to_repo_spec(&self) -> RepoSpec {
        RepoSpec {
            repo_rule_id: self.repo_rule_id.clone(),
            attributes: self
                .attributes
                .iter()
                .map(|(k, v)| (k.clone(), json_to_attr_value(v)))
                .collect(),
        }
    }
}

/// Convert an AttrValue to a serde_json::Value for lockfile storage.
pub fn attr_value_to_json(value: &AttrValue) -> serde_json::Value {
    match value {
        AttrValue::String(s) => serde_json::Value::String(s.clone()),
        AttrValue::Int(i) => serde_json::Value::Number((*i).into()),
        AttrValue::Bool(b) => serde_json::Value::Bool(*b),
        AttrValue::None => serde_json::Value::Null,
        AttrValue::StringList(list) => {
            serde_json::Value::Array(list.iter().map(|s| serde_json::Value::String(s.clone())).collect())
        }
        AttrValue::Label(s) => {
            // Labels are stored as objects with a special marker
            serde_json::json!({ "__label__": s })
        }
        AttrValue::Dict(dict) => {
            let obj: serde_json::Map<String, serde_json::Value> = dict
                .iter()
                .map(|(k, v)| (k.clone(), attr_value_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
    }
}

/// Convert a serde_json::Value back to an AttrValue.
pub fn json_to_attr_value(value: &serde_json::Value) -> AttrValue {
    match value {
        serde_json::Value::String(s) => AttrValue::String(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                AttrValue::Int(i)
            } else {
                // Fallback for floats: convert to string
                AttrValue::String(n.to_string())
            }
        }
        serde_json::Value::Bool(b) => AttrValue::Bool(*b),
        serde_json::Value::Null => AttrValue::None,
        serde_json::Value::Array(arr) => {
            // Assume it's a string list (most common case)
            let strings: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            AttrValue::StringList(strings)
        }
        serde_json::Value::Object(obj) => {
            // Check for label marker
            if let Some(serde_json::Value::String(label)) = obj.get("__label__") {
                return AttrValue::Label(label.clone());
            }
            // Otherwise, treat as dict
            let dict: HashMap<String, AttrValue> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_attr_value(v)))
                .collect();
            AttrValue::Dict(dict)
        }
    }
}

/// Lock entry for a repository rule execution result.
///
/// This caches the result of executing a repository rule (like `http_archive`
/// or `git_repository`) so that subsequent builds don't need to re-download
/// or re-execute the rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRuleLockEntry {
    /// The repository rule that created this repository (e.g., "http_archive").
    pub rule_name: String,

    /// Hash of input attributes for cache invalidation.
    pub attrs_hash: String,

    /// Hash of the downloaded/generated content (for integrity verification).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,

    /// Files that were downloaded during execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub downloaded_files: Vec<DownloadedFileLockEntry>,

    /// Timestamp when this entry was created (for debugging).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// A file downloaded during repository rule execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedFileLockEntry {
    /// The URL the file was downloaded from.
    pub url: String,

    /// Integrity hash in SRI format (e.g., "sha256-base64hash").
    pub integrity: String,

    /// Output path relative to repository root.
    pub output_path: String,
}

impl RepositoryRuleLockEntry {
    /// Create a new lock entry for a repository rule.
    pub fn new(rule_name: String, attrs_hash: String) -> Self {
        Self {
            rule_name,
            attrs_hash,
            content_hash: None,
            downloaded_files: Vec::new(),
            created_at: None,
        }
    }

    /// Set the content hash.
    pub fn with_content_hash(mut self, hash: String) -> Self {
        self.content_hash = Some(hash);
        self
    }

    /// Add a downloaded file entry.
    pub fn with_downloaded_file(mut self, url: String, integrity: String, output_path: String) -> Self {
        self.downloaded_files.push(DownloadedFileLockEntry {
            url,
            integrity,
            output_path,
        });
        self
    }

    /// Set the creation timestamp.
    pub fn with_timestamp(mut self) -> Self {
        use std::time::SystemTime;
        if let Ok(duration) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            self.created_at = Some(format!("{}", duration.as_secs()));
        }
        self
    }
}

impl Lockfile {
    /// Create a new empty lockfile.
    pub fn new() -> Self {
        Self {
            lock_file_version: LOCKFILE_VERSION,
            module_file_hash: String::new(),
            registry_file_hashes: HashMap::new(),
            selected_yanked_versions: HashMap::new(),
            module_dep_graph: HashMap::new(),
            module_extensions: HashMap::new(),
            repository_rules: HashMap::new(),
        }
    }

    /// Read a lockfile from disk.
    pub fn read(path: &Path) -> kuro_error::Result<Self> {
        if !path.exists() {
            return Err(LockfileError::NotFound(path.display().to_string()).into());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| LockfileError::ReadError(format!("{}: {}", path.display(), e)))?;

        let lockfile: Lockfile = serde_json::from_str(&content)
            .map_err(|e| LockfileError::ParseError(format!("{}: {}", path.display(), e)))?;

        // Check version compatibility
        if lockfile.lock_file_version > LOCKFILE_VERSION {
            return Err(LockfileError::VersionMismatch {
                expected: LOCKFILE_VERSION,
                found: lockfile.lock_file_version,
            }
            .into());
        }

        Ok(lockfile)
    }

    /// Write the lockfile to disk.
    pub fn write(&self, path: &Path) -> kuro_error::Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| LockfileError::WriteError(format!("JSON serialization failed: {}", e)))?;

        // Write atomically by writing to a temp file first
        let temp_path = path.with_extension("lock.tmp");

        let mut file = fs::File::create(&temp_path)
            .map_err(|e| LockfileError::WriteError(format!("{}: {}", temp_path.display(), e)))?;

        file.write_all(content.as_bytes())
            .map_err(|e| LockfileError::WriteError(format!("{}: {}", temp_path.display(), e)))?;

        file.sync_all()
            .map_err(|e| LockfileError::WriteError(format!("sync failed: {}", e)))?;

        // Rename temp file to final path
        fs::rename(&temp_path, path)
            .map_err(|e| LockfileError::WriteError(format!("rename failed: {}", e)))?;

        Ok(())
    }

    /// Check if the lockfile is valid for the given root module.
    ///
    /// Returns true if the lockfile can be used (MODULE.bazel hasn't changed).
    pub fn is_valid_for(&self, root_module: &Module, module_bazel_path: &Path) -> bool {
        // Check version
        if self.lock_file_version > LOCKFILE_VERSION {
            return false;
        }

        // Check MODULE.bazel hash
        let current_hash = match compute_file_hash(module_bazel_path) {
            Ok(h) => h,
            Err(_) => return false,
        };

        if self.module_file_hash != current_hash {
            return false;
        }

        // Basic sanity check - module name should match
        if let Some(root_node) = self.module_dep_graph.get(&root_module.name) {
            if root_node.name != root_module.name {
                return false;
            }
        }

        true
    }

    /// Convert from a ResolvedGraph to populate the lockfile.
    pub fn from_resolved_graph(
        graph: &ResolvedGraph,
        module_bazel_path: &Path,
    ) -> kuro_error::Result<Self> {
        let mut lockfile = Lockfile::new();

        // Compute hash of root MODULE.bazel
        lockfile.module_file_hash = compute_file_hash(module_bazel_path)?;

        // Convert resolved modules to lockfile nodes
        for (name, info) in &graph.modules {
            let key = format!("{}@{}", name, info.version);
            let node = LockfileModuleNode::from_resolved_info(info);
            lockfile.module_dep_graph.insert(key, node);
        }

        Ok(lockfile)
    }

    /// Convert the lockfile back to a ResolvedGraph.
    pub fn to_resolved_graph(&self) -> ResolvedGraph {
        let mut graph = ResolvedGraph::default();

        for (key, node) in &self.module_dep_graph {
            // Extract name from key (could be "name" or "name@version")
            let name = if key.contains('@') {
                key.split('@').next().unwrap().to_string()
            } else {
                key.clone()
            };

            graph
                .selected_versions
                .insert(name.clone(), node.version.clone());

            let source = node.to_module_source();

            let info = ResolvedModuleInfo {
                name: name.clone(),
                version: node.version.clone(),
                compatibility_level: node.compatibility_level,
                dependencies: node.dependencies.clone(),
                source,
                source_path: node.local_path.as_ref().map(PathBuf::from),
            };

            graph.modules.insert(name.clone(), info);
            graph.resolution_order.push(name);
        }

        graph
    }

    /// Add a registry file hash to the lockfile.
    pub fn add_registry_hash(&mut self, url: &str, content: &str) {
        let hash = compute_sri_hash(content.as_bytes());
        self.registry_file_hashes.insert(url.to_string(), hash);
    }

    /// Check if a repository rule has a valid cache entry.
    ///
    /// Returns `Some(&entry)` if the repository exists in the lockfile and
    /// the attrs_hash matches (indicating the inputs haven't changed).
    pub fn get_repository_rule_cache(
        &self,
        repo_name: &str,
        attrs_hash: &str,
    ) -> Option<&RepositoryRuleLockEntry> {
        self.repository_rules.get(repo_name).filter(|entry| {
            entry.attrs_hash == attrs_hash
        })
    }

    /// Add or update a repository rule cache entry.
    pub fn set_repository_rule_cache(
        &mut self,
        repo_name: String,
        entry: RepositoryRuleLockEntry,
    ) {
        self.repository_rules.insert(repo_name, entry);
    }

    /// Remove a repository rule cache entry.
    pub fn remove_repository_rule_cache(&mut self, repo_name: &str) -> Option<RepositoryRuleLockEntry> {
        self.repository_rules.remove(repo_name)
    }

    /// Check if any repository rules are cached.
    pub fn has_repository_rules(&self) -> bool {
        !self.repository_rules.is_empty()
    }

    /// Get all cached repository rule names.
    pub fn repository_rule_names(&self) -> impl Iterator<Item = &str> {
        self.repository_rules.keys().map(|s| s.as_str())
    }

    // =========================================================================
    // Module Extension Cache Operations
    // =========================================================================

    /// Check if a module extension has a valid cached result.
    ///
    /// Returns `Some(HashMap<internal_name, RepoSpec>)` if the extension exists
    /// in the lockfile and both digests match, indicating the cache is valid.
    ///
    /// # Arguments
    ///
    /// * `extension_id` - The extension identifier (e.g., "@@module//path:file.bzl%name")
    /// * `bzl_transitive_digest` - Hash of all .bzl files the extension depends on
    /// * `usages_digest` - Hash of all tags from modules using this extension
    ///
    /// # Returns
    ///
    /// The cached generated repo specs if valid, or None if cache miss.
    pub fn get_extension_cache(
        &self,
        extension_id: &str,
        bzl_transitive_digest: &str,
        usages_digest: &str,
    ) -> Option<HashMap<String, RepoSpec>> {
        let ext_data = self.module_extensions.get(extension_id)?;

        // Validate that the cached data matches our current inputs
        if !ext_data.is_valid(bzl_transitive_digest, usages_digest) {
            tracing::debug!(
                "Extension cache miss for '{}': digest mismatch",
                extension_id
            );
            return None;
        }

        // Convert lockfile specs back to RepoSpecs
        let repo_specs = ext_data.get_repo_specs()?;
        let result = repo_specs
            .iter()
            .map(|(name, spec)| (name.clone(), spec.to_repo_spec()))
            .collect();

        tracing::debug!(
            "Extension cache hit for '{}': {} repo specs",
            extension_id,
            repo_specs.len()
        );

        Some(result)
    }

    /// Store a module extension result in the lockfile cache.
    ///
    /// This caches the generated repo specs along with the digests needed
    /// for cache validation on subsequent builds.
    ///
    /// # Arguments
    ///
    /// * `extension_id` - The extension identifier
    /// * `bzl_transitive_digest` - Hash of all .bzl files the extension depends on
    /// * `usages_digest` - Hash of all tags from modules using this extension
    /// * `generated_repo_specs` - The repository specifications generated by the extension
    pub fn set_extension_cache(
        &mut self,
        extension_id: String,
        bzl_transitive_digest: String,
        usages_digest: String,
        generated_repo_specs: &HashMap<String, RepoSpec>,
    ) {
        // Convert RepoSpecs to lockfile format
        let lockfile_specs: HashMap<String, LockfileRepoSpec> = generated_repo_specs
            .iter()
            .map(|(name, spec)| (name.clone(), LockfileRepoSpec::from_repo_spec(spec)))
            .collect();

        let ext_data = LockfileExtensionData::new(
            bzl_transitive_digest,
            usages_digest,
            lockfile_specs,
        );

        tracing::debug!(
            "Caching extension '{}' with {} repo specs",
            extension_id,
            generated_repo_specs.len()
        );

        self.module_extensions.insert(extension_id, ext_data);
    }

    /// Remove a module extension from the cache.
    pub fn remove_extension_cache(&mut self, extension_id: &str) -> Option<LockfileExtensionData> {
        self.module_extensions.remove(extension_id)
    }

    /// Check if any module extensions are cached.
    pub fn has_extension_cache(&self) -> bool {
        !self.module_extensions.is_empty()
    }

    /// Get all cached extension identifiers.
    pub fn extension_ids(&self) -> impl Iterator<Item = &str> {
        self.module_extensions.keys().map(|s| s.as_str())
    }

    /// Get extension data by ID (for inspection/debugging).
    pub fn get_extension_data(&self, extension_id: &str) -> Option<&LockfileExtensionData> {
        self.module_extensions.get(extension_id)
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}

impl LockfileModuleNode {
    /// Create from a ResolvedModuleInfo.
    pub fn from_resolved_info(info: &ResolvedModuleInfo) -> Self {
        let mut node = Self {
            name: info.name.clone(),
            version: info.version.clone(),
            compatibility_level: info.compatibility_level,
            dependencies: info.dependencies.clone(),
            registry: None,
            source_type: None,
            local_path: None,
            git_remote: None,
            git_commit: None,
            archive_urls: None,
        };

        match &info.source {
            ModuleSource::Registry { url } => {
                node.registry = Some(url.clone());
            }
            ModuleSource::LocalPath { path } => {
                node.source_type = Some("local_path".to_string());
                node.local_path = Some(path.clone());
            }
            ModuleSource::Git { remote, commit } => {
                node.source_type = Some("git".to_string());
                node.git_remote = Some(remote.clone());
                node.git_commit = Some(commit.clone());
            }
            ModuleSource::Archive { urls } => {
                node.source_type = Some("archive".to_string());
                node.archive_urls = Some(urls.clone());
            }
        }

        node
    }

    /// Convert back to ModuleSource.
    pub fn to_module_source(&self) -> ModuleSource {
        if let Some(url) = &self.registry {
            return ModuleSource::Registry { url: url.clone() };
        }

        match self.source_type.as_deref() {
            Some("local_path") => ModuleSource::LocalPath {
                path: self.local_path.clone().unwrap_or_default(),
            },
            Some("git") => ModuleSource::Git {
                remote: self.git_remote.clone().unwrap_or_default(),
                commit: self.git_commit.clone().unwrap_or_default(),
            },
            Some("archive") => ModuleSource::Archive {
                urls: self.archive_urls.clone().unwrap_or_default(),
            },
            _ => {
                // Default to registry with empty URL
                ModuleSource::Registry {
                    url: String::new(),
                }
            }
        }
    }
}

/// Compute SHA256 hash of a file and return as SRI format.
pub fn compute_file_hash(path: &Path) -> kuro_error::Result<String> {
    let content = fs::read(path).buck_error_context(format!(
        "Failed to read file for hashing: {}",
        path.display()
    ))?;

    Ok(compute_sri_hash(&content))
}

/// Compute SHA256 hash of bytes and return as SRI format.
pub fn compute_sri_hash(data: &[u8]) -> String {
    use base64::Engine;
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    format!(
        "sha256-{}",
        base64::engine::general_purpose::STANDARD.encode(hash)
    )
}

/// Lockfile mode for controlling resolution behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LockfileMode {
    /// Update lockfile if needed (default).
    #[default]
    Update,
    /// Refresh lockfile (always re-resolve).
    Refresh,
    /// Error if lockfile would change.
    Error,
    /// Don't use lockfile.
    Off,
}

impl LockfileMode {
    /// Parse from string (CLI argument).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "update" => Some(Self::Update),
            "refresh" => Some(Self::Refresh),
            "error" => Some(Self::Error),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

/// Get the lockfile path for a workspace.
pub fn lockfile_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("MODULE.bazel.lock")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_lockfile_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");

        let mut lockfile = Lockfile::new();
        lockfile.module_file_hash = "sha256-abc123".to_string();
        lockfile.module_dep_graph.insert(
            "rules_cc@0.0.9".to_string(),
            LockfileModuleNode {
                name: "rules_cc".to_string(),
                version: "0.0.9".to_string(),
                compatibility_level: 0,
                dependencies: HashMap::new(),
                registry: Some("https://bcr.bazel.build".to_string()),
                source_type: None,
                local_path: None,
                git_remote: None,
                git_commit: None,
                archive_urls: None,
            },
        );

        lockfile.write(&path).unwrap();

        let loaded = Lockfile::read(&path).unwrap();
        assert_eq!(loaded.lock_file_version, LOCKFILE_VERSION);
        assert_eq!(loaded.module_file_hash, "sha256-abc123");
        assert!(loaded.module_dep_graph.contains_key("rules_cc@0.0.9"));
    }

    #[test]
    fn test_lockfile_not_found() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.lock");

        let result = Lockfile::read(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_sri_hash() {
        let data = b"hello world";
        let hash = compute_sri_hash(data);
        assert!(hash.starts_with("sha256-"));
        // SHA256 of "hello world" in base64
        assert!(hash.len() > 7); // "sha256-" + base64
    }

    #[test]
    fn test_lockfile_validity() {
        let dir = TempDir::new().unwrap();
        let module_path = dir.path().join("MODULE.bazel");
        fs::write(&module_path, "module(name = \"test\", version = \"1.0.0\")").unwrap();

        let mut lockfile = Lockfile::new();
        lockfile.module_file_hash = compute_file_hash(&module_path).unwrap();

        let module = Module::new("test".to_string(), crate::version::Version::parse("1.0.0").unwrap());

        assert!(lockfile.is_valid_for(&module, &module_path));

        // Modify MODULE.bazel
        fs::write(&module_path, "module(name = \"test\", version = \"2.0.0\")").unwrap();
        assert!(!lockfile.is_valid_for(&module, &module_path));
    }

    #[test]
    fn test_module_node_source_conversion() {
        // Test registry source
        let info = ResolvedModuleInfo {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            compatibility_level: 0,
            dependencies: HashMap::new(),
            source: ModuleSource::Registry {
                url: "https://bcr.bazel.build".to_string(),
            },
            source_path: None,
        };
        let node = LockfileModuleNode::from_resolved_info(&info);
        assert_eq!(node.registry, Some("https://bcr.bazel.build".to_string()));

        // Test local path source
        let info = ResolvedModuleInfo {
            name: "local".to_string(),
            version: "0.0.0".to_string(),
            compatibility_level: 0,
            dependencies: HashMap::new(),
            source: ModuleSource::LocalPath {
                path: "../local_module".to_string(),
            },
            source_path: Some(PathBuf::from("../local_module")),
        };
        let node = LockfileModuleNode::from_resolved_info(&info);
        assert_eq!(node.source_type, Some("local_path".to_string()));
        assert_eq!(node.local_path, Some("../local_module".to_string()));
    }

    #[test]
    fn test_lockfile_mode_parsing() {
        assert_eq!(LockfileMode::from_str("update"), Some(LockfileMode::Update));
        assert_eq!(LockfileMode::from_str("refresh"), Some(LockfileMode::Refresh));
        assert_eq!(LockfileMode::from_str("error"), Some(LockfileMode::Error));
        assert_eq!(LockfileMode::from_str("off"), Some(LockfileMode::Off));
        assert_eq!(LockfileMode::from_str("invalid"), None);
    }

    #[test]
    fn test_repository_rule_lock_entry() {
        let entry = RepositoryRuleLockEntry::new(
            "http_archive".to_string(),
            "sha256-abc123".to_string(),
        )
        .with_content_hash("sha256-def456".to_string())
        .with_downloaded_file(
            "https://example.com/archive.tar.gz".to_string(),
            "sha256-xyz789".to_string(),
            "archive.tar.gz".to_string(),
        );

        assert_eq!(entry.rule_name, "http_archive");
        assert_eq!(entry.attrs_hash, "sha256-abc123");
        assert_eq!(entry.content_hash, Some("sha256-def456".to_string()));
        assert_eq!(entry.downloaded_files.len(), 1);
        assert_eq!(entry.downloaded_files[0].url, "https://example.com/archive.tar.gz");
    }

    #[test]
    fn test_lockfile_repository_rule_cache() {
        let mut lockfile = Lockfile::new();

        // Initially empty
        assert!(!lockfile.has_repository_rules());
        assert!(lockfile.get_repository_rule_cache("foo", "hash1").is_none());

        // Add an entry
        let entry = RepositoryRuleLockEntry::new(
            "http_archive".to_string(),
            "hash1".to_string(),
        );
        lockfile.set_repository_rule_cache("foo".to_string(), entry);

        // Now it should exist
        assert!(lockfile.has_repository_rules());
        assert!(lockfile.get_repository_rule_cache("foo", "hash1").is_some());

        // Wrong hash should not match
        assert!(lockfile.get_repository_rule_cache("foo", "hash2").is_none());

        // Wrong name should not match
        assert!(lockfile.get_repository_rule_cache("bar", "hash1").is_none());

        // Remove it
        let removed = lockfile.remove_repository_rule_cache("foo");
        assert!(removed.is_some());
        assert!(!lockfile.has_repository_rules());
    }

    #[test]
    fn test_lockfile_repository_rules_serialization() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");

        let mut lockfile = Lockfile::new();
        lockfile.set_repository_rule_cache(
            "rules_cc".to_string(),
            RepositoryRuleLockEntry::new(
                "http_archive".to_string(),
                "sha256-abc".to_string(),
            )
            .with_content_hash("sha256-def".to_string())
            .with_downloaded_file(
                "https://github.com/rules_cc/archive.tar.gz".to_string(),
                "sha256-ghi".to_string(),
                "rules_cc.tar.gz".to_string(),
            ),
        );

        lockfile.write(&path).unwrap();

        let loaded = Lockfile::read(&path).unwrap();
        assert!(loaded.has_repository_rules());

        let entry = loaded.get_repository_rule_cache("rules_cc", "sha256-abc").unwrap();
        assert_eq!(entry.rule_name, "http_archive");
        assert_eq!(entry.content_hash, Some("sha256-def".to_string()));
        assert_eq!(entry.downloaded_files.len(), 1);
    }

    // =========================================================================
    // Module Extension Cache Tests
    // =========================================================================

    #[test]
    fn test_lockfile_extension_data_creation() {
        let mut specs = HashMap::new();
        specs.insert(
            "numpy".to_string(),
            LockfileRepoSpec::new("@@rules_python//pip:pip.bzl%pip_install".to_string())
                .with_attr("version".to_string(), serde_json::json!("1.24.0")),
        );
        specs.insert(
            "requests".to_string(),
            LockfileRepoSpec::new("@@rules_python//pip:pip.bzl%pip_install".to_string())
                .with_attr("version".to_string(), serde_json::json!("2.31.0")),
        );

        let ext_data = LockfileExtensionData::new(
            "sha256-bzl-digest".to_string(),
            "sha256-usages-digest".to_string(),
            specs,
        );

        assert!(ext_data.general.is_some());
        let general = ext_data.general.as_ref().unwrap();
        assert_eq!(general.bzl_transitive_digest, "sha256-bzl-digest");
        assert_eq!(general.usages_digest, "sha256-usages-digest");
        assert_eq!(general.generated_repo_specs.len(), 2);
        assert!(general.generated_repo_specs.contains_key("numpy"));
        assert!(general.generated_repo_specs.contains_key("requests"));
    }

    #[test]
    fn test_lockfile_extension_data_validation() {
        let specs = HashMap::new();
        let ext_data = LockfileExtensionData::new(
            "digest1".to_string(),
            "digest2".to_string(),
            specs,
        );

        // Both digests must match
        assert!(ext_data.is_valid("digest1", "digest2"));
        assert!(!ext_data.is_valid("digest1", "other"));
        assert!(!ext_data.is_valid("other", "digest2"));
        assert!(!ext_data.is_valid("other1", "other2"));
    }

    #[test]
    fn test_lockfile_repo_spec_roundtrip() {
        use crate::repository_invocations::AttrValue;

        // Create a RepoSpec
        let repo_spec = RepoSpec::new("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_string())
            .with_attr("url".to_string(), AttrValue::String("https://example.com/archive.tar.gz".to_string()))
            .with_attr("sha256".to_string(), AttrValue::String("abc123def456".to_string()))
            .with_attr("strip_prefix".to_string(), AttrValue::String("mylib-1.0".to_string()));

        // Convert to lockfile format
        let lockfile_spec = LockfileRepoSpec::from_repo_spec(&repo_spec);
        assert_eq!(lockfile_spec.repo_rule_id, "@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive");
        assert_eq!(lockfile_spec.attributes.len(), 3);

        // Convert back to RepoSpec
        let roundtrip_spec = lockfile_spec.to_repo_spec();
        assert_eq!(roundtrip_spec.repo_rule_id, repo_spec.repo_rule_id);
        assert_eq!(roundtrip_spec.attributes.len(), repo_spec.attributes.len());

        // Check values roundtrip correctly
        assert_eq!(
            roundtrip_spec.attributes.get("url"),
            Some(&AttrValue::String("https://example.com/archive.tar.gz".to_string()))
        );
    }

    #[test]
    fn test_attr_value_json_conversion() {
        use crate::repository_invocations::AttrValue;

        // Test string
        let val = AttrValue::String("hello".to_string());
        let json = attr_value_to_json(&val);
        assert_eq!(json, serde_json::json!("hello"));
        assert_eq!(json_to_attr_value(&json), AttrValue::String("hello".to_string()));

        // Test int
        let val = AttrValue::Int(42);
        let json = attr_value_to_json(&val);
        assert_eq!(json, serde_json::json!(42));
        assert_eq!(json_to_attr_value(&json), AttrValue::Int(42));

        // Test bool
        let val = AttrValue::Bool(true);
        let json = attr_value_to_json(&val);
        assert_eq!(json, serde_json::json!(true));
        assert_eq!(json_to_attr_value(&json), AttrValue::Bool(true));

        // Test None
        let val = AttrValue::None;
        let json = attr_value_to_json(&val);
        assert_eq!(json, serde_json::Value::Null);
        assert_eq!(json_to_attr_value(&json), AttrValue::None);

        // Test string list
        let val = AttrValue::StringList(vec!["a".to_string(), "b".to_string()]);
        let json = attr_value_to_json(&val);
        assert_eq!(json, serde_json::json!(["a", "b"]));
        assert_eq!(json_to_attr_value(&json), AttrValue::StringList(vec!["a".to_string(), "b".to_string()]));

        // Test label (special format)
        let val = AttrValue::Label("//foo:bar".to_string());
        let json = attr_value_to_json(&val);
        assert_eq!(json, serde_json::json!({"__label__": "//foo:bar"}));
        assert_eq!(json_to_attr_value(&json), AttrValue::Label("//foo:bar".to_string()));
    }

    #[test]
    fn test_extension_cache_hit() {
        use crate::repository_invocations::AttrValue;

        let mut lockfile = Lockfile::new();

        // Initially empty
        assert!(!lockfile.has_extension_cache());
        assert!(lockfile.get_extension_cache("@@pip//pip:pip.bzl%pip", "bzl-digest", "usages-digest").is_none());

        // Create and cache an extension result
        let mut repo_specs = HashMap::new();
        repo_specs.insert(
            "numpy".to_string(),
            RepoSpec::new("@@rules_python//pip:pip.bzl%pip_install".to_string())
                .with_attr("version".to_string(), AttrValue::String("1.24.0".to_string())),
        );

        lockfile.set_extension_cache(
            "@@pip//pip:pip.bzl%pip".to_string(),
            "bzl-digest".to_string(),
            "usages-digest".to_string(),
            &repo_specs,
        );

        // Now it should exist
        assert!(lockfile.has_extension_cache());

        // Cache hit with matching digests
        let cached = lockfile.get_extension_cache("@@pip//pip:pip.bzl%pip", "bzl-digest", "usages-digest");
        assert!(cached.is_some());
        let cached_specs = cached.unwrap();
        assert_eq!(cached_specs.len(), 1);
        assert!(cached_specs.contains_key("numpy"));

        // Verify the spec data
        let numpy_spec = cached_specs.get("numpy").unwrap();
        assert_eq!(numpy_spec.repo_rule_id, "@@rules_python//pip:pip.bzl%pip_install");
    }

    #[test]
    fn test_extension_cache_miss_wrong_bzl_digest() {
        use crate::repository_invocations::AttrValue;

        let mut lockfile = Lockfile::new();

        let mut repo_specs = HashMap::new();
        repo_specs.insert(
            "foo".to_string(),
            RepoSpec::new("rule".to_string())
                .with_attr("key".to_string(), AttrValue::String("value".to_string())),
        );

        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_string(),
            "original-bzl-digest".to_string(),
            "usages-digest".to_string(),
            &repo_specs,
        );

        // Cache miss when bzl_transitive_digest differs
        assert!(lockfile.get_extension_cache("@@ext//ext.bzl%ext", "different-bzl-digest", "usages-digest").is_none());
    }

    #[test]
    fn test_extension_cache_miss_wrong_usages_digest() {
        use crate::repository_invocations::AttrValue;

        let mut lockfile = Lockfile::new();

        let mut repo_specs = HashMap::new();
        repo_specs.insert(
            "foo".to_string(),
            RepoSpec::new("rule".to_string())
                .with_attr("key".to_string(), AttrValue::String("value".to_string())),
        );

        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_string(),
            "bzl-digest".to_string(),
            "original-usages-digest".to_string(),
            &repo_specs,
        );

        // Cache miss when usages_digest differs
        assert!(lockfile.get_extension_cache("@@ext//ext.bzl%ext", "bzl-digest", "different-usages-digest").is_none());
    }

    #[test]
    fn test_extension_cache_miss_wrong_extension_id() {
        use crate::repository_invocations::AttrValue;

        let mut lockfile = Lockfile::new();

        let mut repo_specs = HashMap::new();
        repo_specs.insert(
            "foo".to_string(),
            RepoSpec::new("rule".to_string())
                .with_attr("key".to_string(), AttrValue::String("value".to_string())),
        );

        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_string(),
            "bzl-digest".to_string(),
            "usages-digest".to_string(),
            &repo_specs,
        );

        // Cache miss when extension ID differs
        assert!(lockfile.get_extension_cache("@@other//other.bzl%other", "bzl-digest", "usages-digest").is_none());
    }

    #[test]
    fn test_extension_cache_serialization_roundtrip() {
        use crate::repository_invocations::AttrValue;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");

        let mut lockfile = Lockfile::new();

        // Set up extension cache with complex attrs
        let mut repo_specs = HashMap::new();
        repo_specs.insert(
            "numpy".to_string(),
            RepoSpec::new("@@rules_python//pip:pip.bzl%pip_install".to_string())
                .with_attr("version".to_string(), AttrValue::String("1.24.0".to_string()))
                .with_attr("extras".to_string(), AttrValue::StringList(vec!["all".to_string()]))
                .with_attr("timeout".to_string(), AttrValue::Int(300)),
        );

        lockfile.set_extension_cache(
            "@@rules_python//pip:pip.bzl%pip".to_string(),
            "sha256-bzl-digest".to_string(),
            "sha256-usages-digest".to_string(),
            &repo_specs,
        );

        // Write to disk
        lockfile.write(&path).unwrap();

        // Read back
        let loaded = Lockfile::read(&path).unwrap();
        assert!(loaded.has_extension_cache());

        // Verify cache hit
        let cached = loaded.get_extension_cache(
            "@@rules_python//pip:pip.bzl%pip",
            "sha256-bzl-digest",
            "sha256-usages-digest",
        );
        assert!(cached.is_some());

        let cached_specs = cached.unwrap();
        assert_eq!(cached_specs.len(), 1);

        let numpy = cached_specs.get("numpy").unwrap();
        assert_eq!(numpy.repo_rule_id, "@@rules_python//pip:pip.bzl%pip_install");
        assert_eq!(numpy.attributes.get("version"), Some(&AttrValue::String("1.24.0".to_string())));
        assert_eq!(numpy.attributes.get("extras"), Some(&AttrValue::StringList(vec!["all".to_string()])));
        assert_eq!(numpy.attributes.get("timeout"), Some(&AttrValue::Int(300)));
    }

    #[test]
    fn test_extension_cache_update() {
        let mut lockfile = Lockfile::new();
        let ext_id = "@@ext//ext.bzl%ext".to_string();

        // Initial cache
        let mut specs1 = HashMap::new();
        specs1.insert("v1_repo".to_string(), RepoSpec::new("rule".to_string()));

        lockfile.set_extension_cache(
            ext_id.clone(),
            "digest1".to_string(),
            "usages1".to_string(),
            &specs1,
        );

        // Verify initial state
        let cached1 = lockfile.get_extension_cache(&ext_id, "digest1", "usages1").unwrap();
        assert!(cached1.contains_key("v1_repo"));
        assert!(!cached1.contains_key("v2_repo"));

        // Update with new data
        let mut specs2 = HashMap::new();
        specs2.insert("v2_repo".to_string(), RepoSpec::new("rule2".to_string()));

        lockfile.set_extension_cache(
            ext_id.clone(),
            "digest2".to_string(),
            "usages2".to_string(),
            &specs2,
        );

        // Old cache should be invalidated
        assert!(lockfile.get_extension_cache(&ext_id, "digest1", "usages1").is_none());

        // New cache should work
        let cached2 = lockfile.get_extension_cache(&ext_id, "digest2", "usages2").unwrap();
        assert!(!cached2.contains_key("v1_repo"));
        assert!(cached2.contains_key("v2_repo"));
    }

    #[test]
    fn test_extension_cache_remove() {
        let mut lockfile = Lockfile::new();
        let ext_id = "@@ext//ext.bzl%ext";

        let specs = HashMap::new();
        lockfile.set_extension_cache(
            ext_id.to_string(),
            "digest".to_string(),
            "usages".to_string(),
            &specs,
        );

        assert!(lockfile.has_extension_cache());
        assert!(lockfile.extension_ids().any(|id| id == ext_id));

        // Remove the extension cache
        let removed = lockfile.remove_extension_cache(ext_id);
        assert!(removed.is_some());
        assert!(!lockfile.has_extension_cache());
        assert!(lockfile.get_extension_cache(ext_id, "digest", "usages").is_none());
    }

    #[test]
    fn test_extension_ids_iterator() {
        let mut lockfile = Lockfile::new();

        lockfile.set_extension_cache(
            "@@a//a.bzl%a".to_string(),
            "d1".to_string(),
            "u1".to_string(),
            &HashMap::new(),
        );
        lockfile.set_extension_cache(
            "@@b//b.bzl%b".to_string(),
            "d2".to_string(),
            "u2".to_string(),
            &HashMap::new(),
        );

        let mut ids: Vec<_> = lockfile.extension_ids().collect();
        ids.sort();
        assert_eq!(ids, vec!["@@a//a.bzl%a", "@@b//b.bzl%b"]);
    }
}
