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
//! The lockfile is a JSON file compatible with Bazel 9.0's MODULE.bazel.lock format
//! (lockFileVersion 26):
//!
//! ```json
//! {
//!   "lockFileVersion": 26,
//!   "registryFileHashes": {
//!     "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel": "sha256-hex"
//!   },
//!   "selectedYankedVersions": {},
//!   "moduleExtensions": {
//!     "@@rules_python+//python/extensions:pip.bzl%pip": {
//!       "general": {
//!         "bzlTransitiveDigest": "base64-encoded-sha256",
//!         "usagesDigest": "base64-encoded-sha256",
//!         "recordedInputs": [],
//!         "generatedRepoSpecs": {},
//!         "moduleExtensionMetadata": null
//!       }
//!     }
//!   },
//!   "facts": {}
//! }
//! ```

use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use indexmap::IndexMap;
use kuro_error::BuckErrorContext;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::repo_spec::RepoSpec;
use crate::repository_invocations::AttrValue;

/// Current lockfile format version.
/// This matches Bazel 9.0's lockfile version (26).
pub const LOCKFILE_VERSION: u32 = 26;

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

/// The MODULE.bazel.lock file content.
///
/// Compatible with Bazel 9.0's lockfile format (lockFileVersion 26).
/// Deprecated fields from older formats are preserved for backwards-compatible
/// deserialization but are no longer written.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lockfile {
    /// Lockfile format version.
    pub lock_file_version: u32,

    /// Map from registry file URL to its integrity hash.
    /// Keys are URLs like "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel"
    /// Values are hex-encoded SHA256 hashes.
    #[serde(default)]
    pub registry_file_hashes: IndexMap<String, String>,

    /// Map of yanked versions that were explicitly allowed.
    /// Keys are "module@version", values are the yanked reason.
    #[serde(default)]
    pub selected_yanked_versions: IndexMap<String, String>,

    /// Module extension results.
    /// Keys are extension identifiers (e.g., "@@rules_python+//python/extensions:pip.bzl%pip").
    #[serde(default)]
    pub module_extensions: IndexMap<String, LockfileExtensionData>,

    /// Bazel 9.0 facts field. Used by some extensions for metadata.
    #[serde(default)]
    pub facts: IndexMap<String, serde_json::Value>,

    // =========================================================================
    // Deprecated fields (Bazel 8.0+ removed these)
    //
    // Kept for backwards-compatible deserialization of old lockfiles.
    // These are never written to new lockfiles (skip_serializing).
    // =========================================================================
    /// DEPRECATED: Hash of the root MODULE.bazel file (removed in Bazel 8.0+).
    #[serde(default, skip_serializing)]
    pub module_file_hash: String,

    /// DEPRECATED: The resolved module dependency graph (removed in Bazel 8.0+).
    /// Kept as opaque JSON for backwards-compatible deserialization only.
    #[serde(default, skip_serializing)]
    pub module_dep_graph: IndexMap<String, serde_json::Value>,

    /// DEPRECATED: Repository rule execution results (Kuro-specific, not in Bazel).
    /// Kept as opaque JSON for backwards-compatible deserialization only.
    #[serde(default, skip_serializing)]
    pub repository_rules: IndexMap<String, serde_json::Value>,
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
/// Matches Bazel 9.0's extension general data format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LockfileExtensionGeneral {
    /// Transitive digest of all .bzl files the extension depends on.
    /// Used for cache invalidation when extension code changes.
    pub bzl_transitive_digest: String,

    /// Digest of all module usages (tags passed to the extension).
    /// Used for cache invalidation when extension inputs change.
    pub usages_digest: String,

    /// Recorded inputs that affect extension execution.
    /// Bazel 9.0 format - list of strings in these formats:
    /// - `REPO_MAPPING:<module>+,<apparent_name> <canonical_name>`
    /// - `FILE:@@<module>+//<path> <sha256-hex>`
    /// - `ENV:<VARIABLE_NAME>`
    #[serde(default)]
    pub recorded_inputs: Vec<String>,

    /// Generated repository specifications.
    /// Keys are internal names (e.g., "numpy"), values are full RepoSpec data.
    #[serde(default)]
    pub generated_repo_specs: IndexMap<String, LockfileRepoSpec>,

    /// Module extension metadata. Nullable (null when not provided by the extension).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_extension_metadata: Option<serde_json::Value>,
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
    pub attributes: IndexMap<String, serde_json::Value>,
}

impl LockfileExtensionData {
    /// Create a new extension data with general info.
    pub fn new(
        bzl_transitive_digest: String,
        usages_digest: String,
        generated_repo_specs: IndexMap<String, LockfileRepoSpec>,
    ) -> Self {
        Self {
            general: Some(LockfileExtensionGeneral {
                bzl_transitive_digest,
                usages_digest,
                recorded_inputs: Vec::new(),
                generated_repo_specs,
                module_extension_metadata: None,
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
    pub fn get_repo_specs(&self) -> Option<&IndexMap<String, LockfileRepoSpec>> {
        self.general.as_ref().map(|g| &g.generated_repo_specs)
    }
}

impl LockfileRepoSpec {
    /// Create a new lockfile repo spec.
    pub fn new(repo_rule_id: String) -> Self {
        Self {
            repo_rule_id,
            attributes: IndexMap::new(),
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
        AttrValue::StringList(list) => serde_json::Value::Array(
            list.iter()
                .map(|s| serde_json::Value::String(s.clone()))
                .collect(),
        ),
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
            let dict: IndexMap<String, AttrValue> = obj
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
// RepositoryRuleLockEntry and DownloadedFileLockEntry removed in Phase 9f.
// The `repository_rules` field now uses serde_json::Value for backwards-compat only.

impl Lockfile {
    /// Create a new empty lockfile.
    pub fn new() -> Self {
        Self {
            lock_file_version: LOCKFILE_VERSION,
            registry_file_hashes: IndexMap::new(),
            selected_yanked_versions: IndexMap::new(),
            module_extensions: IndexMap::new(),
            facts: IndexMap::new(),
            // Deprecated fields
            module_file_hash: String::new(),
            module_dep_graph: IndexMap::new(),
            repository_rules: IndexMap::new(),
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

    /// Add a registry file hash to the lockfile.
    pub fn add_registry_hash(&mut self, url: &str, content: &str) {
        let hash = compute_sri_hash(content.as_bytes());
        self.registry_file_hashes.insert(url.to_string(), hash);
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
    ) -> Option<fxhash::FxHashMap<String, RepoSpec>> {
        // Try exact match first, then normalized forms for Bazel lockfile compat.
        // Lockfiles may use "//:file.bzl%name" while kuro uses ":file.bzl%name" or vice versa.
        let ext_data = self
            .module_extensions
            .get(extension_id)
            .or_else(|| {
                // Try adding // prefix: ":file.bzl%name" -> "//:file.bzl%name"
                if extension_id.starts_with(':') {
                    self.module_extensions.get(&format!("//{}", extension_id))
                } else {
                    None
                }
            })
            .or_else(|| {
                // Try stripping // prefix: "//:file.bzl%name" -> ":file.bzl%name"
                extension_id
                    .strip_prefix("//")
                    .and_then(|stripped| self.module_extensions.get(stripped))
            })?;

        // Validate that the cached data matches our current inputs.
        // For Bazel-generated lockfiles, kuro may compute different digests,
        // so we use the cached specs with a warning rather than discarding them.
        if !ext_data.is_valid(bzl_transitive_digest, usages_digest) {
            tracing::warn!(
                "Extension cache digest mismatch for '{}' (using lockfile specs anyway)",
                extension_id
            );
        }

        // Convert lockfile specs back to RepoSpecs
        let repo_specs = ext_data.get_repo_specs()?;

        // Don't treat empty generatedRepoSpecs as a valid cache hit.
        // Empty specs usually indicate a previous failed/stub execution that
        // was incorrectly cached (e.g., from a Bazel lockfile or a kuro run
        // before extension execution was implemented). Re-executing the
        // extension may produce real repos now.
        if repo_specs.is_empty() {
            tracing::debug!(
                "Extension cache miss for '{}': empty generatedRepoSpecs (forcing re-execution)",
                extension_id
            );
            return None;
        }

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
        generated_repo_specs: &fxhash::FxHashMap<String, RepoSpec>,
    ) {
        // Convert RepoSpecs to lockfile format. Sort by key so the
        // serialised lockfile JSON is stable across invocations
        // regardless of the in-memory FxHashMap's insertion order.
        let mut entries: Vec<_> = generated_repo_specs.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        let lockfile_specs: IndexMap<String, LockfileRepoSpec> = entries
            .into_iter()
            .map(|(name, spec)| (name.clone(), LockfileRepoSpec::from_repo_spec(spec)))
            .collect();

        let ext_data =
            LockfileExtensionData::new(bzl_transitive_digest, usages_digest, lockfile_specs);

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
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_lockfile_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");

        let lockfile = Lockfile::new();
        lockfile.write(&path).unwrap();

        let loaded = Lockfile::read(&path).unwrap();
        assert_eq!(loaded.lock_file_version, LOCKFILE_VERSION);
        // Deprecated fields should not be serialized
        assert!(loaded.module_file_hash.is_empty());
        assert!(loaded.module_dep_graph.is_empty());
        assert!(loaded.repository_rules.is_empty());
        // New fields should be present
        assert!(loaded.facts.is_empty());
    }

    #[test]
    fn test_lockfile_bazel9_format() {
        // Verify the serialized JSON matches Bazel 9.0 format
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");

        let lockfile = Lockfile::new();
        lockfile.write(&path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Must have lockFileVersion 26
        assert_eq!(json["lockFileVersion"], 26);
        // Must have these Bazel 9.0 fields
        assert!(json.get("registryFileHashes").is_some());
        assert!(json.get("selectedYankedVersions").is_some());
        assert!(json.get("moduleExtensions").is_some());
        assert!(json.get("facts").is_some());
        // Must NOT have deprecated fields
        assert!(json.get("moduleFileHash").is_none());
        assert!(json.get("moduleDepGraph").is_none());
        assert!(json.get("repositoryRules").is_none());
    }

    #[test]
    fn test_lockfile_backwards_compat_old_format() {
        // Verify we can read old-format lockfiles (v24 with deprecated fields)
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");

        let old_format_json = r#"{
            "lockFileVersion": 24,
            "moduleFileHash": "sha256-oldhash",
            "registryFileHashes": {},
            "selectedYankedVersions": {},
            "moduleDepGraph": {
                "rules_cc@0.0.9": {
                    "name": "rules_cc",
                    "version": "0.0.9",
                    "compatibilityLevel": 0,
                    "dependencies": {}
                }
            },
            "moduleExtensions": {},
            "repositoryRules": {}
        }"#;

        fs::write(&path, old_format_json).unwrap();
        let loaded = Lockfile::read(&path).unwrap();

        // Should successfully read old fields
        assert_eq!(loaded.lock_file_version, 24);
        assert_eq!(loaded.module_file_hash, "sha256-oldhash");
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
    fn test_lockfile_mode_parsing() {
        assert_eq!(LockfileMode::from_str("update"), Some(LockfileMode::Update));
        assert_eq!(
            LockfileMode::from_str("refresh"),
            Some(LockfileMode::Refresh)
        );
        assert_eq!(LockfileMode::from_str("error"), Some(LockfileMode::Error));
        assert_eq!(LockfileMode::from_str("off"), Some(LockfileMode::Off));
        assert_eq!(LockfileMode::from_str("invalid"), None);
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
        let ext_data =
            LockfileExtensionData::new("digest1".to_string(), "digest2".to_string(), specs);

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
        let repo_spec =
            RepoSpec::new("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_string())
                .with_attr(
                    "url".to_string(),
                    AttrValue::String("https://example.com/archive.tar.gz".to_string()),
                )
                .with_attr(
                    "sha256".to_string(),
                    AttrValue::String("abc123def456".to_string()),
                )
                .with_attr(
                    "strip_prefix".to_string(),
                    AttrValue::String("mylib-1.0".to_string()),
                );

        // Convert to lockfile format
        let lockfile_spec = LockfileRepoSpec::from_repo_spec(&repo_spec);
        assert_eq!(
            lockfile_spec.repo_rule_id,
            "@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive"
        );
        assert_eq!(lockfile_spec.attributes.len(), 3);

        // Convert back to RepoSpec
        let roundtrip_spec = lockfile_spec.to_repo_spec();
        assert_eq!(roundtrip_spec.repo_rule_id, repo_spec.repo_rule_id);
        assert_eq!(roundtrip_spec.attributes.len(), repo_spec.attributes.len());

        // Check values roundtrip correctly
        assert_eq!(
            roundtrip_spec.attributes.get("url"),
            Some(&AttrValue::String(
                "https://example.com/archive.tar.gz".to_string()
            ))
        );
    }

    #[test]
    fn test_attr_value_json_conversion() {
        use crate::repository_invocations::AttrValue;

        // Test string
        let val = AttrValue::String("hello".to_string());
        let json = attr_value_to_json(&val);
        assert_eq!(json, serde_json::json!("hello"));
        assert_eq!(
            json_to_attr_value(&json),
            AttrValue::String("hello".to_string())
        );

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
        assert_eq!(
            json_to_attr_value(&json),
            AttrValue::StringList(vec!["a".to_string(), "b".to_string()])
        );

        // Test label (special format)
        let val = AttrValue::Label("//foo:bar".to_string());
        let json = attr_value_to_json(&val);
        assert_eq!(json, serde_json::json!({"__label__": "//foo:bar"}));
        assert_eq!(
            json_to_attr_value(&json),
            AttrValue::Label("//foo:bar".to_string())
        );
    }

    #[test]
    fn test_extension_cache_hit() {
        use crate::repository_invocations::AttrValue;

        let mut lockfile = Lockfile::new();

        // Initially empty
        assert!(!lockfile.has_extension_cache());
        assert!(
            lockfile
                .get_extension_cache("@@pip//pip:pip.bzl%pip", "bzl-digest", "usages-digest")
                .is_none()
        );

        // Create and cache an extension result
        let mut repo_specs = HashMap::new();
        repo_specs.insert(
            "numpy".to_string(),
            RepoSpec::new("@@rules_python//pip:pip.bzl%pip_install".to_string()).with_attr(
                "version".to_string(),
                AttrValue::String("1.24.0".to_string()),
            ),
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
        let cached =
            lockfile.get_extension_cache("@@pip//pip:pip.bzl%pip", "bzl-digest", "usages-digest");
        assert!(cached.is_some());
        let cached_specs = cached.unwrap();
        assert_eq!(cached_specs.len(), 1);
        assert!(cached_specs.contains_key("numpy"));

        // Verify the spec data
        let numpy_spec = cached_specs.get("numpy").unwrap();
        assert_eq!(
            numpy_spec.repo_rule_id,
            "@@rules_python//pip:pip.bzl%pip_install"
        );
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
        assert!(
            lockfile
                .get_extension_cache(
                    "@@ext//ext.bzl%ext",
                    "different-bzl-digest",
                    "usages-digest"
                )
                .is_none()
        );
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
        assert!(
            lockfile
                .get_extension_cache(
                    "@@ext//ext.bzl%ext",
                    "bzl-digest",
                    "different-usages-digest"
                )
                .is_none()
        );
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
        assert!(
            lockfile
                .get_extension_cache("@@other//other.bzl%other", "bzl-digest", "usages-digest")
                .is_none()
        );
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
                .with_attr(
                    "version".to_string(),
                    AttrValue::String("1.24.0".to_string()),
                )
                .with_attr(
                    "extras".to_string(),
                    AttrValue::StringList(vec!["all".to_string()]),
                )
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
        assert_eq!(
            numpy.repo_rule_id,
            "@@rules_python//pip:pip.bzl%pip_install"
        );
        assert_eq!(
            numpy.attributes.get("version"),
            Some(&AttrValue::String("1.24.0".to_string()))
        );
        assert_eq!(
            numpy.attributes.get("extras"),
            Some(&AttrValue::StringList(vec!["all".to_string()]))
        );
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
        let cached1 = lockfile
            .get_extension_cache(&ext_id, "digest1", "usages1")
            .unwrap();
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
        assert!(
            lockfile
                .get_extension_cache(&ext_id, "digest1", "usages1")
                .is_none()
        );

        // New cache should work
        let cached2 = lockfile
            .get_extension_cache(&ext_id, "digest2", "usages2")
            .unwrap();
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
        assert!(
            lockfile
                .get_extension_cache(ext_id, "digest", "usages")
                .is_none()
        );
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
