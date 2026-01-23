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

/// Module extension data in the lockfile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockfileExtensionData {
    /// The extension's bzl file label.
    pub bzl_file: String,

    /// The extension name.
    pub extension_name: String,

    /// Hash of extension inputs (tags from all using modules).
    pub input_hash: String,

    /// Generated repositories.
    #[serde(default)]
    pub generated_repos: HashMap<String, LockfileGeneratedRepo>,
}

/// A repository generated by a module extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockfileGeneratedRepo {
    /// Repository rule name.
    pub rule_class: String,

    /// Repository rule attributes.
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
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
}
