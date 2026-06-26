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
//!     "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel": "hex-encoded-sha256"
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

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use base64::Engine;
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::dice_graph::BzlmodEventKind;
use crate::dice_graph::record_bzlmod_event;
use crate::module_extension_executor::ModuleExtensionMetadata;
use crate::repo_spec::RepoSpec;
use crate::repository_invocations::AttrValue;

/// Current lockfile format version.
/// This matches Bazel 9.0's lockfile version (26).
pub const LOCKFILE_VERSION: u32 = 26;

static LOCKFILE_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Errors that can occur during lockfile operations.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
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
        "The version of MODULE.bazel.lock is not supported by this version of Slug. \
        Please run `slug mod update` to update your lockfile."
    )]
    UnsupportedVersion { found: u32, expected: u32 },

    #[error(
        "Lockfile is stale: {reason} \
        Run 'slug mod update' to update the lockfile."
    )]
    StaleLockfile { reason: String },

    #[error(
        "Lockfile would change but --lockfile_mode=error was specified: {reason} \
        Run 'slug mod update' to update the lockfile."
    )]
    LockfileModeError { reason: String },
}

/// Explicit capability for writing Bazel-visible lockfiles.
///
/// Both `ExplicitModUpdate` and `ResolutionUpdate` produce identical on-disk
/// content — the distinction is a capability gate for future `slug mod update`
/// plumbing.  `ResolutionUpdate` is used by the production post-build path;
/// `ExplicitModUpdate` is reserved for the (not-yet-existent) `slug mod`
/// command; tests use the cfg-gated `Test` variant.
///
/// Bazel 9 reference: `BazelLockFileModule.afterCommand()` writes the
/// lockfile unconditionally (no purpose distinction). The purpose enum
/// exists so that a future `slug mod update` can enforce a stricter
/// write policy without refactoring the call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockfileWritePurpose {
    ExplicitModUpdate,
    ResolutionUpdate,
    #[cfg(test)]
    Test,
}

/// The MODULE.bazel.lock file content.
///
/// Compatible with Bazel 9.0's lockfile format (lockFileVersion 26).
/// Deprecated fields from older formats are preserved for backwards-compatible
/// deserialization but are no longer written.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

    /// DEPRECATED: Repository rule execution results (Slug-specific, not in Bazel).
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
    /// - `REPO_MAPPING:<source_repo>,<apparent_name> <canonical_name>`
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

    /// Create extension data from freshly evaluated repository specs.
    ///
    /// The lockfile is serialized deterministically, so repo specs are sorted
    /// by internal name rather than preserving `FxHashMap` iteration order.
    pub fn from_repo_specs_with_recorded_inputs(
        bzl_transitive_digest: String,
        usages_digest: String,
        generated_repo_specs: &fxhash::FxHashMap<String, RepoSpec>,
        recorded_inputs: Vec<String>,
    ) -> Self {
        Self::from_repo_specs_with_recorded_inputs_and_metadata(
            bzl_transitive_digest,
            usages_digest,
            generated_repo_specs,
            recorded_inputs,
            None,
        )
    }

    pub fn from_repo_specs_with_recorded_inputs_and_metadata(
        bzl_transitive_digest: String,
        usages_digest: String,
        generated_repo_specs: &fxhash::FxHashMap<String, RepoSpec>,
        recorded_inputs: Vec<String>,
        metadata: Option<&ModuleExtensionMetadata>,
    ) -> Self {
        let mut entries: Vec<_> = generated_repo_specs.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        let lockfile_specs: IndexMap<String, LockfileRepoSpec> = entries
            .into_iter()
            .map(|(name, spec)| (name.clone(), LockfileRepoSpec::from_repo_spec(spec)))
            .collect();

        let mut ext_data = Self::new(bzl_transitive_digest, usages_digest, lockfile_specs);
        if let Some(general) = ext_data.general.as_mut() {
            general.recorded_inputs = recorded_inputs;
            general.module_extension_metadata =
                metadata.and_then(ModuleExtensionMetadata::lockfile_module_extension_metadata);
        }
        ext_data
    }

    pub fn is_reproducible(&self) -> bool {
        self.general.as_ref().is_some_and(|general| {
            lockfile_module_extension_metadata_is_reproducible(
                general.module_extension_metadata.as_ref(),
            )
        })
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

    /// Check that recorded extension inputs still match current filesystem
    /// state. Unsupported recorded input kinds are conservative replay misses.
    #[cfg(test)]
    pub fn recorded_inputs_current(
        &self,
        workspace_root: Option<&Path>,
        repo_env: Option<&BTreeMap<String, String>>,
        repo_mappings: Option<&crate::RepoMappingSnapshot>,
    ) -> Result<(), String> {
        let Some(general) = &self.general else {
            return Err("missing_general".to_owned());
        };
        validate_recorded_inputs_for_replay(
            &general.recorded_inputs,
            workspace_root,
            repo_env,
            repo_mappings,
        )
    }

    /// Get the generated repo specs if valid.
    pub fn get_repo_specs(&self) -> Option<&IndexMap<String, LockfileRepoSpec>> {
        self.general.as_ref().map(|g| &g.generated_repo_specs)
    }
}

/// Build a Bazel-style recorded FILE input marker for current filesystem state.
pub fn recorded_file_input(path: &Path) -> std::io::Result<String> {
    recorded_file_input_with_recorded_path(path, path)
}

/// Build a Bazel-style recorded FILE input marker using a Bazel repo-friendly
/// recorded path while hashing the actual on-disk path.
pub fn recorded_file_input_with_recorded_path(
    recorded_path: &Path,
    actual_path: &Path,
) -> std::io::Result<String> {
    Ok(format_recorded_input(
        "FILE",
        recorded_path,
        &recorded_file_marker_value(actual_path)?,
    ))
}

/// Build a Bazel-style recorded DIRENTS input marker for current filesystem state.
pub fn recorded_dirents_input(path: &Path) -> std::io::Result<String> {
    recorded_dirents_input_with_recorded_path(path, path)
}

/// Build a Bazel-style recorded DIRENTS input marker using a Bazel repo-friendly
/// recorded path while reading the actual on-disk directory entries.
pub fn recorded_dirents_input_with_recorded_path(
    recorded_path: &Path,
    actual_path: &Path,
) -> std::io::Result<String> {
    Ok(format_recorded_input(
        "DIRENTS",
        recorded_path,
        &recorded_dirents_marker_value(actual_path)?,
    ))
}

/// Build a Bazel-style recorded DIRTREE input marker for current filesystem state.
pub fn recorded_dirtree_input(path: &Path) -> std::io::Result<String> {
    Ok(format_recorded_input(
        "DIRTREE",
        path,
        &recorded_dirtree_marker_value(path)?,
    ))
}

/// Build a Bazel-style recorded ENV input marker for current repo-env state.
pub fn recorded_env_input(name: &str, value: Option<&str>) -> String {
    let input = format!("ENV:{name}");
    let value = value
        .map(escape_recorded_input_part)
        .unwrap_or_else(|| "\\0".to_owned());
    format!("{} {}", escape_recorded_input_part(&input), value)
}

/// Build a Bazel-style recorded repository-mapping marker.
pub fn recorded_repo_mapping_input(
    source_repo: &str,
    apparent: &str,
    value: Option<&str>,
) -> String {
    let input = format!("REPO_MAPPING:{source_repo},{apparent}");
    let value = value
        .map(escape_recorded_input_part)
        .unwrap_or_else(|| "\\0".to_owned());
    format!("{} {}", escape_recorded_input_part(&input), value)
}

/// Validate recorded inputs against current filesystem/env/repo-mapping state.
#[cfg(test)]
pub fn validate_recorded_inputs_current(
    recorded_inputs: &[String],
    workspace_root: Option<&Path>,
    repo_env: Option<&BTreeMap<String, String>>,
    repo_mappings: Option<&crate::RepoMappingSnapshot>,
) -> Result<(), String> {
    validate_recorded_inputs_for_replay(recorded_inputs, workspace_root, repo_env, repo_mappings)
}

/// Lockfile extension replay data selected after extension-id and digest checks.
///
/// Recorded-input validation is intentionally left to the caller so DICE replay
/// paths can route it through a named child key.
#[derive(Debug, Clone)]
pub struct SelectedExtensionCache {
    pub(crate) selected_key: String,
    pub(crate) repo_specs: fxhash::FxHashMap<String, RepoSpec>,
    pub(crate) recorded_inputs: Vec<String>,
    pub(crate) module_extension_metadata: Option<serde_json::Value>,
    pub(crate) workspace_root: Option<PathBuf>,
    pub(crate) repo_env: Option<BTreeMap<String, String>>,
    pub(crate) repo_mappings: Option<crate::RepoMappingSnapshot>,
}

impl SelectedExtensionCache {
    #[cfg(test)]
    pub(crate) fn recorded_inputs_current(&self) -> Result<(), String> {
        validate_recorded_inputs_current(
            &self.recorded_inputs,
            self.workspace_root.as_deref(),
            self.repo_env.as_ref(),
            self.repo_mappings.as_ref(),
        )
    }

    pub fn record_hit(&self, extension_id: &str) {
        record_bzlmod_event(
            BzlmodEventKind::ExtensionReplayHit,
            format!(
                "{extension_id}:{}:{} repo specs",
                self.selected_key,
                self.repo_specs.len()
            ),
        );
    }

    pub fn repo_specs(&self) -> &fxhash::FxHashMap<String, RepoSpec> {
        &self.repo_specs
    }

    pub fn recorded_inputs(&self) -> &[String] {
        &self.recorded_inputs
    }

    pub fn module_extension_metadata(&self) -> Option<&serde_json::Value> {
        self.module_extension_metadata.as_ref()
    }

    pub fn is_reproducible(&self) -> bool {
        lockfile_module_extension_metadata_is_reproducible(self.module_extension_metadata.as_ref())
    }

    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace_root.as_deref()
    }

    pub fn repo_env(&self) -> Option<&BTreeMap<String, String>> {
        self.repo_env.as_ref()
    }

    pub fn repo_mappings(&self) -> Option<&crate::RepoMappingSnapshot> {
        self.repo_mappings.as_ref()
    }
}

fn lockfile_module_extension_metadata_is_reproducible(
    metadata: Option<&serde_json::Value>,
) -> bool {
    metadata
        .and_then(|metadata| metadata.get("reproducible"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
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
            local: false,
            attributes: self
                .attributes
                .iter()
                .map(|(k, v)| (k.clone(), json_to_attr_value(v)))
                .collect(),
        }
    }
}

fn validate_base64_sha256_lockfile_digest(
    path: &Path,
    extension_id: &str,
    field: &str,
    value: &str,
) -> slug_error::Result<()> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|e| {
            LockfileError::ParseError(format!(
                "{}: invalid {field} for {extension_id}: {e}",
                path.display()
            ))
        })?;
    if decoded.len() != 32 {
        return Err(LockfileError::ParseError(format!(
            "{}: invalid {field} for {extension_id}: decoded {} bytes, expected 32",
            path.display(),
            decoded.len()
        ))
        .into());
    }
    Ok(())
}

pub(crate) enum RecordedInput {
    File(PathBuf),
    Dirents(PathBuf),
    DirTree(PathBuf),
    Env(String),
    RepoMapping {
        source_repo: String,
        apparent: String,
    },
    Unsupported,
}

/// Per-entry state used to reproduce Bazel's `RepoRecordedInput.DirTree`
/// marker format from DICE-backed file reads.
pub enum RecordedDirtreeEntryState {
    FileSha256(Vec<u8>),
    DirectoryDigest(String),
    Other,
}

#[cfg(test)]
fn validate_recorded_inputs_for_replay(
    recorded_inputs: &[String],
    workspace_root: Option<&Path>,
    repo_env: Option<&BTreeMap<String, String>>,
    repo_mappings: Option<&crate::RepoMappingSnapshot>,
) -> Result<(), String> {
    for raw in recorded_inputs {
        let (input, old_value) = parse_recorded_input_with_value(raw)
            .ok_or_else(|| "recorded_input_malformed".to_owned())?;
        match input {
            RecordedInput::File(path) => {
                let path = resolve_recorded_path(&path, workspace_root)?;
                let Some(old_value) = old_value else {
                    return Err("recorded_input_malformed".to_owned());
                };
                let current = recorded_file_marker_value(&path)
                    .map_err(|_| "recorded_input_stat_failed".to_owned())?;
                if current != old_value {
                    return Err(recorded_input_changed_reason_with_values(
                        raw, &old_value, &current,
                    ));
                }
            }
            RecordedInput::Dirents(path) => {
                let path = resolve_recorded_path(&path, workspace_root)?;
                let Some(old_value) = old_value else {
                    return Err("recorded_input_malformed".to_owned());
                };
                let current = recorded_dirents_marker_value(&path)
                    .map_err(|_| "recorded_input_stat_failed".to_owned())?;
                if current != old_value {
                    return Err(recorded_input_changed_reason(raw));
                }
            }
            RecordedInput::DirTree(path) => {
                let path = resolve_recorded_path(&path, workspace_root)?;
                let Some(old_value) = old_value else {
                    return Err("recorded_input_malformed".to_owned());
                };
                let current = recorded_dirtree_marker_value(&path)
                    .map_err(|_| "recorded_input_stat_failed".to_owned())?;
                if current != old_value {
                    return Err(recorded_input_changed_reason(raw));
                }
            }
            RecordedInput::Env(name) => {
                let Some(repo_env) = repo_env else {
                    return Err("recorded_input_unsupported".to_owned());
                };
                let current = repo_env.get(&name).cloned();
                if current != old_value {
                    return Err(recorded_input_changed_reason(raw));
                }
            }
            RecordedInput::RepoMapping {
                source_repo,
                apparent,
            } => {
                let Some(repo_mappings) = repo_mappings else {
                    return Err("recorded_input_unsupported".to_owned());
                };
                let Some(source_mapping) = repo_mappings.get(&source_repo) else {
                    // Source repo has no mappings at all. This is valid only
                    // if the recorded value was also absent (None).
                    if old_value.is_none() {
                        continue;
                    }
                    return Err(recorded_input_changed_reason(raw));
                };
                let current = source_mapping
                    .get(&apparent)
                    .cloned()
                    .or_else(|| Some(apparent.clone()));
                if current != old_value {
                    return Err(recorded_input_changed_reason(raw));
                }
            }
            RecordedInput::Unsupported => {
                return Err("recorded_input_unsupported".to_owned());
            }
        }
    }
    Ok(())
}

pub(crate) fn recorded_input_changed_reason(raw: &str) -> String {
    let identity = raw
        .split_once(' ')
        .map(|(identity, _)| identity)
        .unwrap_or(raw);
    format!("recorded_input_changed:{identity}")
}

pub(crate) fn recorded_input_changed_reason_with_values(
    raw: &str,
    old: &str,
    current: &str,
) -> String {
    format!(
        "{}:old={}:current={}",
        recorded_input_changed_reason(raw),
        old,
        current
    )
}

#[cfg(test)]
fn resolve_recorded_path(path: &Path, workspace_root: Option<&Path>) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let raw = path.to_string_lossy();
    for prefix in ["@@//", "@//", "//"] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            let Some(workspace_root) = workspace_root else {
                return Err("recorded_input_unsupported".to_owned());
            };
            return Ok(workspace_root.join(rest));
        }
    }
    if let Some(rest) = raw.strip_prefix("@@")
        && let Some(separator) = rest.rfind("+//")
    {
        let Some(workspace_root) = workspace_root else {
            return Err("recorded_input_unsupported".to_owned());
        };
        let legacy_repo = &rest[..separator];
        let repo_relative = &rest[separator + 3..];
        if legacy_repo.is_empty() {
            return Err("recorded_input_unsupported".to_owned());
        }
        let exact_repo = rest
            .find("//")
            .map(|exact_separator| &rest[..exact_separator])
            .filter(|repo| !repo.is_empty());
        let mut fallback = None;
        for repo in exact_repo.into_iter().chain(std::iter::once(legacy_repo)) {
            let external_path = workspace_root
                .join("bazel-external")
                .join(repo)
                .join(repo_relative);
            if external_path.exists() {
                return Ok(external_path);
            }
            fallback.get_or_insert_with(|| external_path.clone());
            let bzlmod_cell_path = workspace_root
                .join("buck-out/v2/external_cells/bzlmod")
                .join(repo)
                .join(repo_relative);
            if bzlmod_cell_path.exists() {
                return Ok(bzlmod_cell_path);
            }
        }
        return Ok(fallback.expect("non-empty repo fallback"));
    }
    Err("recorded_input_unsupported".to_owned())
}

pub(crate) fn parse_recorded_input_with_value(
    raw: &str,
) -> Option<(RecordedInput, Option<String>)> {
    let space = raw.find(' ')?;
    if space == 0 {
        return None;
    }
    let input = unescape_recorded_input_part(&raw[..space])?;
    let value = unescape_recorded_input_part(&raw[space + 1..]);
    Some((parse_recorded_input(&input), value))
}

fn parse_recorded_input(input: &str) -> RecordedInput {
    let Some((kind, payload)) = input.split_once(':') else {
        return RecordedInput::Unsupported;
    };
    match kind {
        "FILE" => RecordedInput::File(PathBuf::from(payload)),
        "DIRENTS" => RecordedInput::Dirents(PathBuf::from(payload)),
        "DIRTREE" => RecordedInput::DirTree(PathBuf::from(payload)),
        "ENV" => RecordedInput::Env(payload.to_owned()),
        "REPO_MAPPING" => {
            if let Some((source_repo, apparent)) = payload.split_once(',') {
                RecordedInput::RepoMapping {
                    source_repo: source_repo.to_owned(),
                    apparent: apparent.to_owned(),
                }
            } else {
                RecordedInput::Unsupported
            }
        }
        _ => RecordedInput::Unsupported,
    }
}

fn unescape_recorded_input_part(input: &str) -> Option<String> {
    if input == "\\0" {
        return None;
    }
    let mut result = String::with_capacity(input.len());
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            match ch {
                'n' => result.push('\n'),
                's' => result.push(' '),
                other => result.push(other),
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            result.push(ch);
        }
    }
    if escaped {
        result.push('\\');
    }
    Some(result)
}

fn format_recorded_input(kind: &str, path: &Path, value: &str) -> String {
    let input = format!("{kind}:{}", path.to_string_lossy());
    format!(
        "{} {}",
        escape_recorded_input_part(&input),
        escape_recorded_input_part(value)
    )
}

fn escape_recorded_input_part(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            ' ' => result.push_str("\\s"),
            other => result.push(other),
        }
    }
    result
}

fn recorded_file_marker_value(path: &Path) -> std::io::Result<String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok("DIR".to_owned()),
        Ok(_) => {
            let bytes = fs::read(path)?;
            Ok(hex::encode(Sha256::digest(&bytes)))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("ENOENT".to_owned()),
        Err(e) => Err(e),
    }
}

fn recorded_dirents_marker_value(path: &Path) -> std::io::Result<String> {
    let mut entries = sorted_directory_entry_names(path)?;
    entries.sort();
    Ok(recorded_dirents_marker_value_from_entries(&entries))
}

fn recorded_dirtree_marker_value(path: &Path) -> std::io::Result<String> {
    let entries = sorted_directory_entry_names(path)?;
    let mut entry_states = Vec::new();
    for entry in &entries {
        let entry_path = path.join(entry);
        let metadata = fs::metadata(&entry_path)?;
        if metadata.is_dir() {
            entry_states.push(RecordedDirtreeEntryState::DirectoryDigest(
                recorded_dirtree_marker_value(&entry_path)?,
            ));
        } else if metadata.is_file() {
            entry_states.push(RecordedDirtreeEntryState::FileSha256(
                Sha256::digest(fs::read(&entry_path)?).to_vec(),
            ));
        } else {
            entry_states.push(RecordedDirtreeEntryState::Other);
        }
    }

    recorded_dirtree_marker_value_from_entry_states(&entries, &entry_states)
        .map_err(std::io::Error::other)
}

pub fn recorded_dirents_marker_value_from_entries(entries: &[String]) -> String {
    let mut entries = entries.to_vec();
    entries.sort();
    bazel_fingerprint_add_strings_hex(&entries)
}

pub fn recorded_dirtree_marker_value_from_entry_states(
    entries: &[String],
    entry_states: &[RecordedDirtreeEntryState],
) -> Result<String, String> {
    if entries.len() != entry_states.len() {
        return Err(format!(
            "dirtree entry state mismatch: {} entries, {} states",
            entries.len(),
            entry_states.len()
        ));
    }

    let mut subdir_digests = Vec::new();
    let mut file_values = Vec::new();
    for state in entry_states {
        match state {
            RecordedDirtreeEntryState::FileSha256(digest) => {
                file_values.push((0, Some(digest.as_slice())));
            }
            RecordedDirtreeEntryState::DirectoryDigest(digest) => {
                subdir_digests.push(digest.clone());
                file_values.push((2, None));
            }
            RecordedDirtreeEntryState::Other => {
                file_values.push((1, None));
            }
        }
    }

    let mut bytes = Vec::new();
    bazel_fingerprint_add_strings(&entries, &mut bytes);
    bazel_fingerprint_add_strings(&subdir_digests, &mut bytes);
    for (file_state_type_ordinal, digest) in file_values {
        encode_varint(file_state_type_ordinal, &mut bytes);
        if let Some(digest) = digest {
            bytes.extend_from_slice(digest);
        }
    }
    Ok(hex::encode(Sha256::digest(&bytes)))
}

fn sorted_directory_entry_names(path: &Path) -> std::io::Result<Vec<String>> {
    let mut entries = fs::read_dir(path)?
        .map(|entry| {
            entry.and_then(|entry| {
                entry.file_name().into_string().map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "directory entry is not valid UTF-8",
                    )
                })
            })
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    Ok(entries)
}

fn bazel_fingerprint_add_strings_hex(inputs: &[String]) -> String {
    let mut bytes = Vec::new();
    bazel_fingerprint_add_strings(inputs, &mut bytes);
    hex::encode(Sha256::digest(&bytes))
}

fn bazel_fingerprint_add_strings(inputs: &[String], bytes: &mut Vec<u8>) {
    encode_varint(inputs.len() as u64, bytes);
    for input in inputs {
        let input = input.as_bytes();
        encode_varint(input.len() as u64, bytes);
        bytes.extend_from_slice(input);
    }
}

fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push(((value as u8) & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// Convert an AttrValue to a serde_json::Value for lockfile storage.
///
/// Parity: Bazel 9 `AttributeValuesAdapter.serializeObject`. Labels are
/// serialized as their unambiguous canonical form (`@@repo+//pkg:target`),
/// which always starts with `@@`. Plain strings starting with `@@` are
/// escaped by wrapping in single quotes to avoid label ambiguity.
pub fn attr_value_to_json(value: &AttrValue) -> serde_json::Value {
    match value {
        AttrValue::String(s) => {
            if s.starts_with("@@") || (s.starts_with('\'') && s.ends_with('\'')) {
                serde_json::Value::String(format!("'{}'", s))
            } else {
                serde_json::Value::String(s.clone())
            }
        }
        AttrValue::Int(i) => serde_json::Value::Number((*i).into()),
        AttrValue::Bool(b) => serde_json::Value::Bool(*b),
        AttrValue::None => serde_json::Value::Null,
        AttrValue::StringList(list) => serde_json::Value::Array(
            list.iter()
                .map(|s| {
                    if s.starts_with("@@") || (s.starts_with('\'') && s.ends_with('\'')) {
                        serde_json::Value::String(format!("'{}'", s))
                    } else {
                        serde_json::Value::String(s.clone())
                    }
                })
                .collect(),
        ),
        AttrValue::Label(s) => {
            let canonical = if s.starts_with("@@") {
                s.clone()
            } else if let Some(rest) = s.strip_prefix('@') {
                if let Some(slash_pos) = rest.find("//") {
                    let name = &rest[..slash_pos];
                    let after = &rest[slash_pos..];
                    format!("@@{name}+{after}")
                } else {
                    format!("@@{s}")
                }
            } else if s.starts_with("//") {
                format!("@@_main+{s}")
            } else {
                format!("@@_main+//{s}")
            };
            serde_json::Value::String(canonical)
        }
        AttrValue::Dict(dict) => {
            let obj: serde_json::Map<String, serde_json::Value> = dict
                .iter()
                .map(|(k, v)| (attr_key_to_json_string(k), attr_value_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
    }
}

fn attr_key_to_json_string(key: &str) -> String {
    if key.starts_with("@@") || (key.starts_with('\'') && key.ends_with('\'')) {
        format!("'{}'", key)
    } else {
        key.to_owned()
    }
}

/// Convert a serde_json::Value back to an AttrValue.
///
/// Parity: Bazel 9 `AttributeValuesAdapter.deserializeObject`. Strings
/// starting with `@@` are labels. Strings wrapped in single quotes have
/// one layer of quotes stripped.
pub fn json_to_attr_value(value: &serde_json::Value) -> AttrValue {
    match value {
        serde_json::Value::String(s) => {
            if s.starts_with("@@") {
                let label = if let Some(rest) = s.strip_prefix("@@") {
                    if let Some(slash_pos) = rest.find("//") {
                        let name = &rest[..slash_pos];
                        let after = &rest[slash_pos..];
                        if name.ends_with('+') {
                            format!("@{}{}", &name[..name.len() - 1], after)
                        } else {
                            format!("@{}{}", name, after)
                        }
                    } else {
                        s.clone()
                    }
                } else {
                    s.clone()
                };
                AttrValue::Label(label)
            } else if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
                AttrValue::String(s[1..s.len() - 1].to_owned())
            } else {
                AttrValue::String(s.clone())
            }
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                AttrValue::Int(i)
            } else {
                AttrValue::String(n.to_string())
            }
        }
        serde_json::Value::Bool(b) => AttrValue::Bool(*b),
        serde_json::Value::Null => AttrValue::None,
        serde_json::Value::Array(arr) => {
            let strings: Vec<String> = arr
                .iter()
                .filter_map(|v| {
                    v.as_str().map(|s| {
                        if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
                            s[1..s.len() - 1].to_owned()
                        } else {
                            s.to_owned()
                        }
                    })
                })
                .collect();
            AttrValue::StringList(strings)
        }
        serde_json::Value::Object(obj) => {
            let dict: IndexMap<String, AttrValue> = obj
                .iter()
                .map(|(k, v)| {
                    let key = if k.starts_with('\'') && k.ends_with('\'') && k.len() >= 2 {
                        k[1..k.len() - 1].to_owned()
                    } else {
                        k.clone()
                    };
                    (key, json_to_attr_value(v))
                })
                .collect();
            AttrValue::Dict(dict)
        }
    }
}

fn first_invalid_empty_target_label(
    specs: &fxhash::FxHashMap<String, RepoSpec>,
) -> Option<(&str, &str, &str)> {
    specs.iter().find_map(|(repo_name, spec)| {
        spec.attributes.iter().find_map(|(attr_name, value)| {
            first_invalid_empty_target_label_attr(value)
                .map(|label| (repo_name.as_str(), attr_name.as_str(), label))
        })
    })
}

fn first_invalid_empty_target_label_attr(value: &AttrValue) -> Option<&str> {
    match value {
        AttrValue::Label(label) => invalid_empty_target_label(label).then_some(label.as_str()),
        AttrValue::String(s) => invalid_empty_target_label(s).then_some(s.as_str()),
        AttrValue::StringList(items) => items
            .iter()
            .find(|item| invalid_empty_target_label(item))
            .map(String::as_str),
        AttrValue::Dict(entries) => entries
            .values()
            .find_map(first_invalid_empty_target_label_attr),
        AttrValue::Int(_) | AttrValue::Bool(_) | AttrValue::None => None,
    }
}

fn invalid_empty_target_label(value: &str) -> bool {
    if !(value.starts_with('@') || value.starts_with("//") || value.starts_with(':')) {
        return false;
    }
    crate::repo_mapping::canonicalize_label_with_package_context(value, "", "", None).is_none()
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

    /// Read a lockfile from disk (default Update mode for version checks).
    pub fn read(path: &Path) -> slug_error::Result<Self> {
        Self::read_with_mode(path, LockfileMode::Update)
    }

    /// Read a lockfile from disk with explicit mode for version-mismatch policy.
    pub fn read_with_mode(path: &Path, mode: LockfileMode) -> slug_error::Result<Self> {
        record_bzlmod_event(BzlmodEventKind::LockfileRead, path.display().to_string());

        if !path.exists() {
            return Err(LockfileError::NotFound(path.display().to_string()).into());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| LockfileError::ReadError(format!("{}: {}", path.display(), e)))?;

        parse_lockfile_content_with_mode(path, &content, mode)
    }

    fn validate_extension_digests(&self, path: &Path) -> slug_error::Result<()> {
        for (extension_id, data) in &self.module_extensions {
            let Some(general) = &data.general else {
                continue;
            };
            validate_base64_sha256_lockfile_digest(
                path,
                extension_id,
                "bzlTransitiveDigest",
                &general.bzl_transitive_digest,
            )?;
            validate_base64_sha256_lockfile_digest(
                path,
                extension_id,
                "usagesDigest",
                &general.usages_digest,
            )?;
        }
        Ok(())
    }

    /// Write the lockfile to disk.
    ///
    /// The `purpose` parameter is currently advisory — both
    /// `ResolutionUpdate` and `ExplicitModUpdate` produce the same output.
    /// It exists to allow a future `slug mod update` command to inject a
    /// different write policy (e.g., always write even if unchanged) without
    /// refactoring call sites.
    ///
    /// Bazel 9 reference: `BazelLockFileModule.updateLockfile()` writes
    /// unconditionally — there is no purpose distinction.
    pub fn write_for_purpose(
        &self,
        path: &Path,
        _purpose: LockfileWritePurpose,
    ) -> slug_error::Result<()> {
        self.write_impl(path)
    }

    /// Build a new lockfile from a resolved graph, incorporating registry
    /// file hashes and selected yanked versions from the resolution result.
    ///
    /// This produces a lockfile with *only* registry-level data — no extension
    /// entries. Use [`Self::from_resolved_graph_with_extensions`] for the
    /// production path that merges extension data from the old lockfile and
    /// newly evaluated extension results.
    pub fn from_resolved_graph(graph: &crate::resolution::ResolvedGraph) -> Self {
        let mut lockfile = Self::new();
        for (url, hash) in &graph.registry_file_hashes {
            lockfile
                .registry_file_hashes
                .insert(url.clone(), hash.clone());
        }
        for (module_version, reason) in &graph.selected_yanked_versions {
            lockfile
                .selected_yanked_versions
                .insert(module_version.clone(), reason.clone());
        }
        lockfile
    }

    /// Build a lockfile from a resolved graph, merging extension data and
    /// facts from the old lockfile and newly evaluated extensions.
    ///
    /// This follows Bazel 9's `BazelLockFileModule.afterCommand()` algorithm:
    ///
    /// 1. Registry file hashes come from the resolved graph. Local `file:`
    ///    registry URLs are excluded (Bazel: "they are not needed for
    ///    reproducibility" and "would contribute absolute paths").
    /// 2. Selected yanked versions come from the resolved graph.
    /// 3. Extension data is a union of old lockfile entries (for extensions
    ///    that still have usages) and new extension results.
    /// 4. Facts are merged: old facts for extensions still present in the
    ///    dep graph, plus new facts from freshly evaluated extensions.
    pub fn from_resolved_graph_with_extensions(
        graph: &crate::resolution::ResolvedGraph,
        old_lockfile: Option<&Lockfile>,
        new_extension_results: &[(String, LockfileExtensionData)],
        active_extension_ids: &[String],
        new_facts: &[(String, serde_json::Value)],
    ) -> Self {
        let mut lockfile = Self::new();

        for (url, hash) in &graph.registry_file_hashes {
            if url.starts_with("file:") {
                continue;
            }
            lockfile
                .registry_file_hashes
                .insert(url.clone(), hash.clone());
        }
        for (module_version, reason) in &graph.selected_yanked_versions {
            lockfile
                .selected_yanked_versions
                .insert(module_version.clone(), reason.clone());
        }

        let active_set: std::collections::HashSet<&str> =
            active_extension_ids.iter().map(|s| s.as_str()).collect();

        if let Some(old) = old_lockfile {
            for (ext_id, ext_data) in &old.module_extensions {
                if !active_set.contains(ext_id.as_str()) {
                    continue;
                }
                lockfile
                    .module_extensions
                    .insert(ext_id.clone(), ext_data.clone());
            }

            for (ext_id, facts) in &old.facts {
                if active_set.contains(ext_id.as_str())
                    && (!facts.is_object() || facts.as_object().is_some_and(|obj| !obj.is_empty()))
                {
                    lockfile.facts.insert(ext_id.clone(), facts.clone());
                }
            }
        }

        for (ext_id, ext_data) in new_extension_results {
            lockfile
                .module_extensions
                .insert(ext_id.clone(), ext_data.clone());
        }

        for (ext_id, facts) in new_facts {
            if facts.is_object() && facts.as_object().is_some_and(|obj| obj.is_empty()) {
                continue;
            }
            lockfile.facts.insert(ext_id.clone(), facts.clone());
        }

        let mut sorted_extensions: Vec<_> = lockfile.module_extensions.into_iter().collect();
        sorted_extensions.sort_by(|a, b| a.0.cmp(&b.0));
        lockfile.module_extensions = sorted_extensions.into_iter().collect();

        let mut sorted_facts: Vec<_> = lockfile.facts.into_iter().collect();
        sorted_facts.sort_by(|a, b| a.0.cmp(&b.0));
        lockfile.facts = sorted_facts.into_iter().collect();

        lockfile
    }

    /// Compute the set of extension IDs to preserve and refresh when persisting
    /// the lockfile after a build.
    ///
    /// `aggregated` are the extensions (re-)aggregated during this resolution.
    /// `locked` are the extension IDs already recorded in the injected lockfile
    /// inputs. The result is the deduplicated, sorted union of the two.
    ///
    /// The union matters because on a cache-replayed build (nothing in the
    /// module graph changed) `aggregated` is empty. Feeding only the aggregated
    /// IDs into [`Self::from_resolved_graph_with_extensions`] as the active set
    /// would prune every extension out of the lockfile (`moduleExtensions: {}`),
    /// which makes all extension-generated repos (e.g. crate repos pulled in by
    /// rules_rust's internal extension) unresolvable and breaks every build.
    /// Keeping already-locked IDs active preserves them across replays.
    pub fn active_extension_ids_for_persist(
        aggregated: impl IntoIterator<Item = String>,
        locked: impl IntoIterator<Item = String>,
    ) -> Vec<String> {
        let mut ids: Vec<String> = aggregated.into_iter().chain(locked).collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Enforce `--lockfile_mode=error` by comparing the resolved graph against
    /// the on-disk lockfile.
    ///
    /// Returns `Ok(())` if the lockfile is consistent with the resolved graph.
    /// Returns `Err` with `StaleLockfile` or `LockfileModeError` if drift is
    /// detected.
    pub fn enforce_error_mode(
        resolved_graph: &crate::resolution::ResolvedGraph,
        existing_lockfile: &Lockfile,
    ) -> slug_error::Result<()> {
        // 1. Registry file hash drift.
        for (url, current_hash) in &resolved_graph.registry_file_hashes {
            match existing_lockfile.registry_file_hashes.get(url) {
                Some(locked_hash) if locked_hash == current_hash => {}
                Some(locked_hash) => {
                    return Err(LockfileError::StaleLockfile {
                        reason: format!(
                            "registry file hash drift for '{}': lockfile has '{}', \
                             resolution produced '{}'.",
                            url, locked_hash, current_hash
                        ),
                    }
                    .into());
                }
                None => {
                    // New registry file not in lockfile.
                    return Err(LockfileError::LockfileModeError {
                        reason: format!("new registry file '{}' not in lockfile.", url),
                    }
                    .into());
                }
            }
        }

        // 2. Yanked version drift: a yanked version that wasn't previously
        //    allowed is now selected.
        for (module_version, reason) in &resolved_graph.selected_yanked_versions {
            match existing_lockfile
                .selected_yanked_versions
                .get(module_version)
            {
                Some(locked_reason) if locked_reason == reason => {}
                _ => {
                    return Err(LockfileError::LockfileModeError {
                        reason: format!(
                            "yanked version '{}' selected but not in lockfile.",
                            module_version
                        ),
                    }
                    .into());
                }
            }
        }

        // 3. Resolved-graph drift: compare the lockfile's recorded
        //    registry hashes with the resolved graph to detect removed
        //    entries (a module that was in the lockfile but is no longer
        //    resolved).
        for url in existing_lockfile.registry_file_hashes.keys() {
            if !resolved_graph.registry_file_hashes.contains_key(url) {
                return Err(LockfileError::StaleLockfile {
                    reason: format!(
                        "registry file '{}' is in the lockfile but no longer \
                         in the resolved graph.",
                        url
                    ),
                }
                .into());
            }
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn write(&self, path: &Path) -> slug_error::Result<()> {
        self.write_for_purpose(path, LockfileWritePurpose::Test)
    }

    fn write_impl(&self, path: &Path) -> slug_error::Result<()> {
        record_bzlmod_event(
            BzlmodEventKind::LockfileWriteAttempt,
            path.display().to_string(),
        );

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| LockfileError::WriteError(format!("JSON serialization failed: {}", e)))?;

        // Write atomically by writing to a temp file first. Use a unique
        // filename because multiple extension computations may update the
        // lockfile concurrently.
        let temp_path = path.with_extension(format!(
            "lock.tmp.{}.{}",
            std::process::id(),
            LOCKFILE_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));

        let mut file = fs::File::create(&temp_path)
            .map_err(|e| LockfileError::WriteError(format!("{}: {}", temp_path.display(), e)))?;

        file.write_all(content.as_bytes())
            .map_err(|e| LockfileError::WriteError(format!("{}: {}", temp_path.display(), e)))?;

        file.sync_all()
            .map_err(|e| LockfileError::WriteError(format!("sync failed: {}", e)))?;

        // Rename temp file to final path
        fs::rename(&temp_path, path).map_err(|e| {
            LockfileError::WriteError(format!(
                "rename {} -> {} failed: {}",
                temp_path.display(),
                path.display(),
                e
            ))
        })?;

        Ok(())
    }

    /// Add a registry file hash to the lockfile.
    pub fn add_registry_hash(&mut self, url: &str, content: &str) {
        let hash = compute_sha256_hex(content.as_bytes());
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
    #[cfg(test)]
    pub fn get_extension_cache(
        &self,
        extension_id: &str,
        bzl_transitive_digest: &str,
        usages_digest: &str,
    ) -> Option<fxhash::FxHashMap<String, RepoSpec>> {
        self.get_extension_cache_for_workspace(
            extension_id,
            bzl_transitive_digest,
            usages_digest,
            None,
            None,
            None,
            None,
            None,
        )
    }

    #[cfg(test)]
    pub fn get_extension_cache_for_workspace(
        &self,
        extension_id: &str,
        bzl_transitive_digest: &str,
        usages_digest: &str,
        workspace_root: Option<&Path>,
        repo_env: Option<&BTreeMap<String, String>>,
        repo_mappings: Option<&crate::RepoMappingSnapshot>,
        root_module_name: Option<&str>,
        repo_mapping_overrides: Option<&crate::RepoMappingOverrides>,
    ) -> Option<fxhash::FxHashMap<String, RepoSpec>> {
        let selected = self.select_extension_cache_for_workspace(
            extension_id,
            bzl_transitive_digest,
            usages_digest,
            workspace_root,
            repo_env,
            repo_mappings,
            root_module_name,
            repo_mapping_overrides,
        )?;
        if let Err(reason) = selected.recorded_inputs_current() {
            record_bzlmod_event(
                BzlmodEventKind::ExtensionReplayMissReason,
                format!("{extension_id}:{reason}"),
            );
            tracing::debug!(
                "Extension cache miss for '{}': recorded input validation failed ({})",
                extension_id,
                reason
            );
            return None;
        }
        tracing::debug!(
            "Extension cache hit for '{}' via '{}': {} repo specs",
            extension_id,
            selected.selected_key,
            selected.repo_specs.len()
        );
        selected.record_hit(extension_id);
        Some(selected.repo_specs)
    }

    pub fn select_extension_cache_for_workspace(
        &self,
        extension_id: &str,
        bzl_transitive_digest: &str,
        usages_digest: &str,
        workspace_root: Option<&Path>,
        repo_env: Option<&BTreeMap<String, String>>,
        repo_mappings: Option<&crate::RepoMappingSnapshot>,
        root_module_name: Option<&str>,
        repo_mapping_overrides: Option<&crate::RepoMappingOverrides>,
    ) -> Option<SelectedExtensionCache> {
        let mut saw_candidate = false;
        let mut selected = None;
        let candidate_keys = Self::extension_candidate_keys(extension_id);
        for candidate_key in &candidate_keys {
            let Some(ext_data) = self.module_extensions.get(candidate_key) else {
                continue;
            };
            saw_candidate = true;

            // Validate that the cached data matches our current inputs.
            // Mismatched digests mean this particular spelling is stale, but a
            // lockfile can contain both legacy and canonical spellings. Keep
            // searching so a stale duplicate does not mask a valid entry.
            if !ext_data.is_valid(bzl_transitive_digest, usages_digest) {
                tracing::debug!(
                    "Extension cache candidate '{}' for '{}' has digest mismatch",
                    candidate_key,
                    extension_id
                );
                continue;
            }

            selected = Some((candidate_key.as_str(), ext_data));
            break;
        }

        let Some((selected_key, ext_data)) = selected else {
            if saw_candidate {
                record_bzlmod_event(
                    BzlmodEventKind::ExtensionReplayMissReason,
                    format!("{extension_id}:digest_mismatch"),
                );
                tracing::debug!(
                    "Extension cache miss for '{}': all candidate digests mismatched",
                    extension_id
                );
            }
            return None;
        };

        let general = ext_data.general.as_ref()?;
        let recorded_inputs = general.recorded_inputs.clone();
        let canonical_extension_id = lockfile_canonical_extension_id(extension_id);
        let augmented_repo_mappings;
        let repo_mappings_for_validation = if let (Some(base_mappings), Some(root_module_name)) =
            (repo_mappings, root_module_name)
        {
            let mut snapshot = base_mappings.clone();
            if let Some(repo_specs) = ext_data.get_repo_specs() {
                let owner = crate::extension_execution_dice::extract_owning_module(
                    extension_id,
                    root_module_name,
                );
                let ext_name =
                    crate::extension_execution_dice::extract_extension_name(extension_id);
                let generated_repos = repo_specs.keys().map(|repo_name| {
                    (repo_name.clone(), format!("{owner}+{ext_name}+{repo_name}"))
                });
                let overrides = repo_mapping_overrides.and_then(|all_overrides| {
                    all_overrides
                        .get(extension_id)
                        .or_else(|| all_overrides.get(selected_key))
                        .or_else(|| all_overrides.get(&canonical_extension_id))
                });
                crate::repo_mapping::add_extension_generated_repo_mappings(
                    &mut snapshot,
                    extension_id,
                    root_module_name,
                    generated_repos,
                    overrides,
                );
            }
            augmented_repo_mappings = Some(snapshot);
            augmented_repo_mappings.as_ref()
        } else {
            repo_mappings
        };

        // Convert lockfile specs back to RepoSpecs
        let repo_specs = ext_data.get_repo_specs()?;

        // Don't treat empty generatedRepoSpecs as a valid cache hit.
        // Empty specs usually indicate a previous failed/stub execution that
        // was incorrectly cached (e.g., from a Bazel lockfile or a slug run
        // before extension execution was implemented). Re-executing the
        // extension may produce real repos now.
        if repo_specs.is_empty() {
            record_bzlmod_event(
                BzlmodEventKind::ExtensionReplayMissReason,
                format!("{extension_id}:empty_generated_repo_specs"),
            );
            tracing::debug!(
                "Extension cache miss for '{}': empty generatedRepoSpecs (forcing re-execution)",
                extension_id
            );
            return None;
        }

        let result: fxhash::FxHashMap<String, RepoSpec> = repo_specs
            .iter()
            .map(|(name, spec)| (name.clone(), spec.to_repo_spec()))
            .collect();

        if let Some((repo_name, attr_name, label)) = first_invalid_empty_target_label(&result) {
            record_bzlmod_event(
                BzlmodEventKind::ExtensionReplayMissReason,
                format!(
                    "{extension_id}:invalid_empty_target_label:{repo_name}:{attr_name}:{label}"
                ),
            );
            tracing::debug!(
                "Extension cache miss for '{}': cached RepoSpec '{}' attr '{}' contains invalid empty-target label '{}'",
                extension_id,
                repo_name,
                attr_name,
                label
            );
            return None;
        }

        Some(SelectedExtensionCache {
            selected_key: selected_key.to_owned(),
            repo_specs: result,
            recorded_inputs,
            module_extension_metadata: general.module_extension_metadata.clone(),
            workspace_root: workspace_root.map(Path::to_path_buf),
            repo_env: repo_env.cloned(),
            repo_mappings: repo_mappings_for_validation.cloned(),
        })
    }

    fn extension_candidate_keys(extension_id: &str) -> Vec<String> {
        let mut candidate_keys = vec![
            extension_id.to_owned(),
            lockfile_canonical_extension_id(extension_id),
        ];
        if extension_id.starts_with(':') {
            candidate_keys.push(format!("//{}", extension_id));
        }
        if let Some(stripped) = extension_id.strip_prefix("//") {
            candidate_keys.push(stripped.to_owned());
        }
        candidate_keys.sort();
        candidate_keys.dedup();
        candidate_keys
    }

    /// Return top-level facts for an extension, accepting the same extension-id
    /// spellings as module extension cache lookup.
    pub fn get_extension_facts(&self, extension_id: &str) -> Option<serde_json::Value> {
        for candidate_key in Self::extension_candidate_keys(extension_id) {
            if let Some(facts) = self.facts.get(&candidate_key) {
                return Some(facts.clone());
            }
        }
        None
    }

    /// Store top-level facts for an extension.
    pub fn set_extension_facts(&mut self, extension_id: String, facts: serde_json::Value) {
        match &facts {
            serde_json::Value::Object(map) if map.is_empty() => {
                self.facts.shift_remove(&extension_id);
            }
            _ => {
                self.facts.insert(extension_id, facts);
            }
        }
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
        self.set_extension_cache_with_recorded_inputs(
            extension_id,
            bzl_transitive_digest,
            usages_digest,
            generated_repo_specs,
            Vec::new(),
        );
    }

    pub fn set_extension_cache_with_recorded_inputs(
        &mut self,
        extension_id: String,
        bzl_transitive_digest: String,
        usages_digest: String,
        generated_repo_specs: &fxhash::FxHashMap<String, RepoSpec>,
        recorded_inputs: Vec<String>,
    ) {
        let ext_data = LockfileExtensionData::from_repo_specs_with_recorded_inputs(
            bzl_transitive_digest,
            usages_digest,
            generated_repo_specs,
            recorded_inputs,
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
        self.module_extensions.shift_remove(extension_id)
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

    pub fn has_extension_cache_candidate(&self, extension_id: &str) -> bool {
        Self::extension_candidate_keys(extension_id)
            .iter()
            .any(|candidate| self.module_extensions.contains_key(candidate))
    }
}

pub fn parse_lockfile_content(path: &Path, content: &str) -> slug_error::Result<Lockfile> {
    parse_lockfile_content_with_mode(path, content, LockfileMode::Update)
}

/// Parse lockfile content with explicit mode for version-mismatch policy.
///
/// Matches Bazel 9's `BazelLockFileFunction.getLockfileValue`:
/// - **Error mode**: unsupported version → hard error ("not supported by this
///   version of Slug, please run `slug mod update`").
/// - **Update/Refresh mode**: unsupported version → data is unusable, return an
///   empty lockfile so resolution proceeds from scratch.
/// - **Off mode**: the caller should not call this function; returns empty.
pub fn parse_lockfile_content_with_mode(
    path: &Path,
    content: &str,
    mode: LockfileMode,
) -> slug_error::Result<Lockfile> {
    if mode == LockfileMode::Off {
        return Ok(Lockfile::new());
    }

    let lockfile: Lockfile = serde_json::from_str(content)
        .map_err(|e| LockfileError::ParseError(format!("{}: {}", path.display(), e)))?;

    if lockfile.lock_file_version != LOCKFILE_VERSION {
        // Bazel 9: "The version of MODULE.bazel.lock is not supported by this
        // version of Bazel.  Please run `bazel mod deps --lockfile_mode=update`"
        if mode == LockfileMode::Error {
            return Err(LockfileError::UnsupportedVersion {
                found: lockfile.lock_file_version,
                expected: LOCKFILE_VERSION,
            }
            .into());
        }
        // Update / Refresh: treat old data as unusable (like Bazel's
        // EMPTY_LOCKFILE) so resolution proceeds from scratch.
        return Ok(Lockfile::new());
    }

    lockfile.validate_extension_digests(path)?;
    slug_util::memory_checkpoint::checkpoint(
        "bzlmod_lockfile_read",
        [
            ("bytes", content.len()),
            ("extensions", lockfile.module_extensions.len()),
            ("registry_hashes", lockfile.registry_file_hashes.len()),
        ],
    );

    Ok(lockfile)
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
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

/// Compute SHA256 hash of bytes and return Bazel lockfile registry hash format.
pub fn compute_sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Lockfile mode for controlling resolution behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Allocative)]
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
    /// Stable lower-case tag for persisted and DICE identity digests.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Update => "update",
            Self::Refresh => "refresh",
            Self::Error => "error",
            Self::Off => "off",
        }
    }

    /// Parse from string (CLI argument).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            value if value == Self::Update.as_str() => Some(Self::Update),
            value if value == Self::Refresh.as_str() => Some(Self::Refresh),
            value if value == Self::Error.as_str() => Some(Self::Error),
            value if value == Self::Off.as_str() => Some(Self::Off),
            _ => None,
        }
    }
}

/// Get the lockfile path for a workspace.
pub fn lockfile_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("MODULE.bazel.lock")
}

/// Translate slug's internal `@<apparent>//pkg:file.bzl%name` extension id to
/// bazel 9's canonical `@@<repo>+//pkg:file.bzl%name` lockfile-key form.
///
/// Used for both writing (so slug emits the same key shape as bazel) and
/// reading (so slug accepts bazel-written lockfiles unchanged).
pub fn lockfile_canonical_extension_id(internal_id: &str) -> String {
    if internal_id.starts_with("@@") {
        return internal_id.to_owned();
    }
    if let Some(rest) = internal_id.strip_prefix('@') {
        if let Some(slash_pos) = rest.find("//") {
            let name = &rest[..slash_pos];
            let after = &rest[slash_pos..];
            return format!("@@{name}+{after}");
        }
    }
    if internal_id.starts_with("//") {
        return format!("@@_main+{internal_id}");
    }
    internal_id.to_owned()
}

/// Read a lockfile under explicit Bazel lockfile policy.
///
/// Returns `None` if the lockfile is absent. Existing lockfiles that cannot be
/// read or parsed are hard errors in every mode that reads lockfiles, matching
/// Bazel 9's default lockfile behavior. There is deliberately no process-wide
/// parse cache here: callers that need replayable lockfile identity must go
/// through their DICE-owned lockfile content key.
#[cfg(test)]
pub fn read_lockfile_at_path(
    path: PathBuf,
    mode: LockfileMode,
) -> slug_error::Result<Option<std::sync::Arc<Lockfile>>> {
    if mode == LockfileMode::Off {
        return Ok(None);
    }

    if !path.exists() {
        return Ok(None);
    }

    let parsed = std::sync::Arc::new(Lockfile::read_with_mode(&path, mode)?);
    slug_util::memory_checkpoint::checkpoint(
        "bzlmod_lockfile_read",
        [("extensions", parsed.module_extensions.len())],
    );
    Ok(Some(parsed))
}

/// Read a hidden lockfile using Bazel's hidden-lockfile policy.
///
/// Bazel parses the hidden output-base lockfile as `update` regardless of the
/// command's visible lockfile mode, and treats read/parse failures as an empty
/// hidden lockfile. Visible workspace lockfile failures remain hard errors.
#[cfg(test)]
pub fn read_hidden_lockfile_path(
    path: &Path,
) -> slug_error::Result<Option<std::sync::Arc<Lockfile>>> {
    match read_lockfile_at_path(path.to_path_buf(), LockfileMode::Update) {
        Ok(lockfile) => Ok(lockfile),
        Err(e) => {
            tracing::warn!(
                "Ignoring unreadable hidden lockfile '{}': {}",
                path.display(),
                e
            );
            Ok(None)
        }
    }
}

/// Read `MODULE.bazel.lock` from `workspace_root` with explicit Bazel
/// lockfile policy.
#[cfg(test)]
pub fn read_lockfile_with_mode(
    workspace_root: &Path,
    mode: LockfileMode,
) -> slug_error::Result<Option<std::sync::Arc<Lockfile>>> {
    read_lockfile_at_path(lockfile_path(workspace_root), mode)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use fxhash::FxHashMap;
    use tempfile::TempDir;

    use super::*;

    fn raw_sha256_digest(byte: u8) -> String {
        base64::engine::general_purpose::STANDARD.encode([byte; 32])
    }

    #[test]
    fn test_lockfile_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");

        let mut lockfile = Lockfile::new();
        lockfile.set_extension_facts(
            "@@rules_rs+//rs:extensions.bzl%crate".to_owned(),
            serde_json::json!({
                "serde_1.0.0": {
                    "checksum": "abc123",
                    "downloads": [1, 2],
                    "usable": true,
                    "missing": null
                }
            }),
        );
        lockfile.write(&path).unwrap();

        let loaded = Lockfile::read(&path).unwrap();
        assert_eq!(loaded.lock_file_version, LOCKFILE_VERSION);
        // Deprecated fields should not be serialized
        assert!(loaded.module_file_hash.is_empty());
        assert!(loaded.module_dep_graph.is_empty());
        assert!(loaded.repository_rules.is_empty());
        // New fields should be present and facts should round-trip.
        assert_eq!(
            loaded.get_extension_facts("@@rules_rs+//rs:extensions.bzl%crate"),
            Some(serde_json::json!({
                "serde_1.0.0": {
                    "checksum": "abc123",
                    "downloads": [1, 2],
                    "usable": true,
                    "missing": null
                }
            }))
        );
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
        // Bazel 9 behavior: old lockfile versions are unsupported.
        // - Error mode: hard error with descriptive message
        // - Update mode: treat data as unusable (return empty lockfile)
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

        // Error mode: should fail with UnsupportedVersion
        let result = Lockfile::read_with_mode(&path, LockfileMode::Error);
        let err = result.unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("not supported"),
            "Error mode should report unsupported version, got: {msg}"
        );

        // Update mode: should succeed with empty lockfile (data unusable)
        let result = Lockfile::read_with_mode(&path, LockfileMode::Update);
        assert!(result.is_ok(), "Update mode should succeed for old version");
        let lockfile = result.unwrap();
        assert_eq!(
            lockfile.lock_file_version, LOCKFILE_VERSION,
            "Update mode should return a fresh lockfile with current version"
        );
        assert!(
            lockfile.module_extensions.is_empty(),
            "Old data should be discarded"
        );
    }

    #[test]
    fn test_lockfile_future_version_error_mode() {
        // Future versions are also unsupported.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");

        let future_json = r#"{
            "lockFileVersion": 99,
            "registryFileHashes": {},
            "selectedYankedVersions": {},
            "moduleExtensions": {},
            "facts": {}
        }"#;

        fs::write(&path, future_json).unwrap();

        // Error mode
        let result = Lockfile::read_with_mode(&path, LockfileMode::Error);
        assert!(
            result.is_err(),
            "Future version should be rejected in error mode"
        );

        // Update mode: treat as unusable
        let result = Lockfile::read_with_mode(&path, LockfileMode::Update);
        assert!(
            result.is_ok(),
            "Update mode should succeed for future version"
        );
        let lockfile = result.unwrap();
        assert_eq!(lockfile.lock_file_version, LOCKFILE_VERSION);
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
    fn test_compute_sha256_hex_matches_bazel_registry_hash_format() {
        let data = b"hello world";
        assert_eq!(
            compute_sha256_hex(data),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
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

    #[test]
    fn malformed_lockfile_errors_in_default_update_mode() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");
        fs::write(&path, "{ this is not json }\n").unwrap();

        let err = read_lockfile_with_mode(dir.path(), LockfileMode::Update).unwrap_err();
        assert!(
            format!("{err:#}").contains("Failed to parse lockfile"),
            "{err:#}"
        );
    }

    #[test]
    fn malformed_lockfile_errors_in_refresh_mode() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");
        fs::write(&path, "{ this is not json }\n").unwrap();

        let err = read_lockfile_with_mode(dir.path(), LockfileMode::Refresh).unwrap_err();
        assert!(
            format!("{err:#}").contains("Failed to parse lockfile"),
            "{err:#}"
        );
    }

    #[test]
    fn malformed_lockfile_is_not_read_in_off_mode() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");
        fs::write(&path, "{ this is not json }\n").unwrap();

        let result = read_lockfile_with_mode(dir.path(), LockfileMode::Off).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn malformed_hidden_lockfile_is_treated_as_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");
        fs::write(&path, "{ this is not json }\n").unwrap();

        let result = read_hidden_lockfile_path(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn valid_hidden_lockfile_is_read_with_update_policy() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");
        let mut lockfile = Lockfile::new();
        lockfile.set_extension_facts(
            "@@ext+//:ext.bzl%ext".to_owned(),
            serde_json::json!({"source": "hidden"}),
        );
        lockfile.write(&path).unwrap();

        let result = read_hidden_lockfile_path(&path).unwrap().unwrap();
        assert_eq!(
            result.get_extension_facts("@@ext+//:ext.bzl%ext"),
            Some(serde_json::json!({"source": "hidden"}))
        );
    }

    #[test]
    fn lockfile_reader_rereads_changed_existing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");

        let mut first = Lockfile::new();
        first.set_extension_facts(
            "@@ext+//:ext.bzl%ext".to_owned(),
            serde_json::json!({"version": 1}),
        );
        first.write(&path).unwrap();

        let loaded_first = read_lockfile_with_mode(dir.path(), LockfileMode::Update)
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded_first.get_extension_facts("@@ext+//:ext.bzl%ext"),
            Some(serde_json::json!({"version": 1}))
        );

        let mut second = Lockfile::new();
        second.set_extension_facts(
            "@@ext+//:ext.bzl%ext".to_owned(),
            serde_json::json!({"version": 2}),
        );
        second.write(&path).unwrap();

        let loaded_second = read_lockfile_with_mode(dir.path(), LockfileMode::Update)
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded_second.get_extension_facts("@@ext+//:ext.bzl%ext"),
            Some(serde_json::json!({"version": 2}))
        );
    }

    #[test]
    fn lockfile_reader_reads_file_created_after_missing_lookup() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");

        assert!(
            read_lockfile_with_mode(dir.path(), LockfileMode::Update)
                .unwrap()
                .is_none()
        );

        let mut lockfile = Lockfile::new();
        lockfile.set_extension_facts(
            "@@ext+//:ext.bzl%ext".to_owned(),
            serde_json::json!({"created": true}),
        );
        lockfile.write(&path).unwrap();

        let loaded = read_lockfile_with_mode(dir.path(), LockfileMode::Update)
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.get_extension_facts("@@ext+//:ext.bzl%ext"),
            Some(serde_json::json!({"created": true}))
        );
    }

    #[test]
    fn extension_digest_with_sri_prefix_is_rejected() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");
        let valid_digest = raw_sha256_digest(0);
        fs::write(
            &path,
            format!(
                r#"{{
                    "lockFileVersion": 26,
                    "registryFileHashes": {{}},
                    "selectedYankedVersions": {{}},
                    "moduleExtensions": {{
                        "@@ext+//:ext.bzl%ext": {{
                            "general": {{
                                "bzlTransitiveDigest": "sha256-{valid_digest}",
                                "usagesDigest": "{valid_digest}",
                                "recordedInputs": [],
                                "generatedRepoSpecs": {{}},
                                "moduleExtensionMetadata": null
                            }}
                        }}
                    }},
                    "facts": {{}}
                }}"#
            ),
        )
        .unwrap();

        let err = read_lockfile_with_mode(dir.path(), LockfileMode::Update).unwrap_err();
        assert!(
            format!("{err:#}").contains("invalid bzlTransitiveDigest"),
            "{err:#}"
        );
    }

    #[test]
    fn extension_digest_must_decode_to_sha256_bytes() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("MODULE.bazel.lock");
        let short_digest = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
        let valid_digest = raw_sha256_digest(0);
        fs::write(
            &path,
            format!(
                r#"{{
                    "lockFileVersion": 26,
                    "registryFileHashes": {{}},
                    "selectedYankedVersions": {{}},
                    "moduleExtensions": {{
                        "@@ext+//:ext.bzl%ext": {{
                            "general": {{
                                "bzlTransitiveDigest": "{valid_digest}",
                                "usagesDigest": "{short_digest}",
                                "recordedInputs": [],
                                "generatedRepoSpecs": {{}},
                                "moduleExtensionMetadata": null
                            }}
                        }}
                    }},
                    "facts": {{}}
                }}"#
            ),
        )
        .unwrap();

        let err = read_lockfile_with_mode(dir.path(), LockfileMode::Update).unwrap_err();
        assert!(
            format!("{err:#}").contains("invalid usagesDigest"),
            "{err:#}"
        );
    }

    // =========================================================================
    // Module Extension Cache Tests
    // =========================================================================

    #[test]
    fn test_lockfile_extension_data_creation() {
        let mut specs: IndexMap<String, LockfileRepoSpec> = IndexMap::new();
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
        let specs: IndexMap<String, LockfileRepoSpec> = IndexMap::new();
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

        // Test label — Bazel 9 serializes as @@canonical string
        let val = AttrValue::Label("//foo:bar".to_string());
        let json = attr_value_to_json(&val);
        assert_eq!(json, serde_json::json!("@@_main+//foo:bar"));
        assert_eq!(
            json_to_attr_value(&json),
            AttrValue::Label("@_main//foo:bar".to_string())
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
        let mut repo_specs = FxHashMap::default();
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
    fn test_set_extension_cache_with_recorded_inputs() {
        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("numpy".to_owned(), RepoSpec::new("rule".to_owned()));

        lockfile.set_extension_cache_with_recorded_inputs(
            "@@pip//pip:pip.bzl%pip".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
            vec!["ENV:PLAN61_REPO_ENV first".to_owned()],
        );

        let general = lockfile
            .module_extensions
            .get("@@pip//pip:pip.bzl%pip")
            .unwrap()
            .general
            .as_ref()
            .unwrap();
        assert_eq!(
            general.recorded_inputs,
            vec!["ENV:PLAN61_REPO_ENV first".to_owned()]
        );
    }

    #[test]
    fn lockfile_extension_data_from_repo_specs_sorts_and_records_inputs() {
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("z_repo".to_owned(), RepoSpec::new("rule_z".to_owned()));
        repo_specs.insert("a_repo".to_owned(), RepoSpec::new("rule_a".to_owned()));

        let ext_data = LockfileExtensionData::from_repo_specs_with_recorded_inputs(
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
            vec!["FILE:@@//watched.txt abc123".to_owned()],
        );

        let general = ext_data.general.as_ref().unwrap();
        assert_eq!(
            general
                .generated_repo_specs
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["a_repo".to_owned(), "z_repo".to_owned()]
        );
        assert_eq!(
            general.recorded_inputs,
            vec!["FILE:@@//watched.txt abc123".to_owned()]
        );
    }

    #[test]
    fn lockfile_extension_data_preserves_reproducible_metadata() {
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        let facts_only_metadata = ModuleExtensionMetadata {
            facts: serde_json::json!({"resource": "ok"}),
            ..Default::default()
        };
        let facts_only_ext_data =
            LockfileExtensionData::from_repo_specs_with_recorded_inputs_and_metadata(
                "bzl-digest".to_owned(),
                "usages-digest".to_owned(),
                &repo_specs,
                Vec::new(),
                Some(&facts_only_metadata),
            );
        assert_eq!(
            facts_only_ext_data
                .general
                .as_ref()
                .and_then(|general| general.module_extension_metadata.as_ref()),
            None
        );

        let metadata = ModuleExtensionMetadata {
            reproducible: true,
            ..Default::default()
        };

        let ext_data = LockfileExtensionData::from_repo_specs_with_recorded_inputs_and_metadata(
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
            Vec::new(),
            Some(&metadata),
        );

        assert!(ext_data.is_reproducible());
        assert_eq!(
            ext_data
                .general
                .as_ref()
                .and_then(|general| general.module_extension_metadata.as_ref()),
            Some(&serde_json::json!({"reproducible": true}))
        );
    }

    #[test]
    fn recorded_file_input_matches_current_sha() {
        let dir = TempDir::new().unwrap();
        let watched = dir.path().join("watched.txt");
        fs::write(&watched, "first\n").unwrap();
        let digest = hex::encode(Sha256::digest(fs::read(&watched).unwrap()));

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push(format!("FILE:{} {digest}", watched.display()));

        let cached =
            lockfile.get_extension_cache("@@ext//ext.bzl%ext", "bzl-digest", "usages-digest");
        assert!(cached.is_some());
    }

    #[test]
    fn recorded_file_input_changed_rejects_replay() {
        let dir = TempDir::new().unwrap();
        let watched = dir.path().join("watched.txt");
        fs::write(&watched, "first\n").unwrap();
        let digest = hex::encode(Sha256::digest(fs::read(&watched).unwrap()));
        fs::write(&watched, "second\n").unwrap();

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push(format!("FILE:{} {digest}", watched.display()));

        let cached =
            lockfile.get_extension_cache("@@ext//ext.bzl%ext", "bzl-digest", "usages-digest");
        assert!(cached.is_none());
    }

    #[test]
    fn recorded_main_workspace_file_input_matches_current_sha() {
        let dir = TempDir::new().unwrap();
        let watched = dir.path().join("watched.txt");
        fs::write(&watched, "first\n").unwrap();
        let digest = hex::encode(Sha256::digest(fs::read(&watched).unwrap()));

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push(format!("FILE:@@//watched.txt {digest}"));

        let cached = lockfile.get_extension_cache_for_workspace(
            "@@ext//ext.bzl%ext",
            "bzl-digest",
            "usages-digest",
            Some(dir.path()),
            None,
            None,
            None,
            None,
        );
        assert!(cached.is_some());
    }

    #[test]
    fn recorded_external_repo_file_input_matches_current_sha() {
        let dir = TempDir::new().unwrap();
        let watched = dir
            .path()
            .join("bazel-external")
            .join("rules_zig")
            .join("zig/private/versions.json");
        fs::create_dir_all(watched.parent().unwrap()).unwrap();
        fs::write(&watched, "first\n").unwrap();
        let digest = hex::encode(Sha256::digest(fs::read(&watched).unwrap()));

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push(format!(
                "FILE:@@rules_zig+//zig/private/versions.json {digest}"
            ));

        let cached = lockfile.get_extension_cache_for_workspace(
            "@@ext//ext.bzl%ext",
            "bzl-digest",
            "usages-digest",
            Some(dir.path()),
            None,
            None,
            None,
            None,
        );
        assert!(cached.is_some());
    }

    #[test]
    fn recorded_module_repo_file_input_uses_bzlmod_cell_path_before_symlink_exists() {
        let dir = TempDir::new().unwrap();
        let watched = dir
            .path()
            .join("buck-out/v2/external_cells/bzlmod")
            .join("rules_rs+")
            .join("tools/rust_analyzer/Cargo.lock");
        fs::create_dir_all(watched.parent().unwrap()).unwrap();
        fs::write(&watched, "first\n").unwrap();
        let digest = hex::encode(Sha256::digest(fs::read(&watched).unwrap()));

        let recorded_inputs = vec![format!(
            "FILE:@@rules_rs+//tools/rust_analyzer/Cargo.lock {digest}"
        )];

        assert!(
            validate_recorded_inputs_current(&recorded_inputs, Some(dir.path()), None, None)
                .is_ok()
        );
    }

    #[test]
    fn recorded_main_workspace_dirents_input_matches_current_listing() {
        let dir = TempDir::new().unwrap();
        let watched = dir.path().join("watched_dir");
        fs::create_dir(&watched).unwrap();
        fs::write(watched.join("b.txt"), "b\n").unwrap();
        fs::write(watched.join("a.txt"), "a\n").unwrap();
        let digest = recorded_dirents_marker_value(&watched).unwrap();

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push(format!("DIRENTS:@@//watched_dir {digest}"));

        let cached = lockfile.get_extension_cache_for_workspace(
            "@@ext//ext.bzl%ext",
            "bzl-digest",
            "usages-digest",
            Some(dir.path()),
            None,
            None,
            None,
            None,
        );
        assert!(cached.is_some());
    }

    #[test]
    fn recorded_dirents_input_changed_rejects_replay() {
        let dir = TempDir::new().unwrap();
        let watched = dir.path().join("watched_dir");
        fs::create_dir(&watched).unwrap();
        fs::write(watched.join("a.txt"), "a\n").unwrap();
        let digest = recorded_dirents_marker_value(&watched).unwrap();
        fs::write(watched.join("b.txt"), "b\n").unwrap();

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push(format!("DIRENTS:{} {digest}", watched.display()));

        let cached =
            lockfile.get_extension_cache("@@ext//ext.bzl%ext", "bzl-digest", "usages-digest");
        assert!(cached.is_none());
    }

    #[test]
    fn recorded_main_workspace_dirtree_input_matches_current_tree() {
        let dir = TempDir::new().unwrap();
        let watched = dir.path().join("watched_tree");
        fs::create_dir(&watched).unwrap();
        fs::write(watched.join("a.txt"), "a\n").unwrap();
        fs::create_dir(watched.join("sub")).unwrap();
        fs::write(watched.join("sub").join("b.txt"), "b\n").unwrap();
        let digest = recorded_dirtree_marker_value(&watched).unwrap();

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push(format!("DIRTREE:@@//watched_tree {digest}"));

        let cached = lockfile.get_extension_cache_for_workspace(
            "@@ext//ext.bzl%ext",
            "bzl-digest",
            "usages-digest",
            Some(dir.path()),
            None,
            None,
            None,
            None,
        );
        assert!(cached.is_some());
    }

    #[test]
    fn recorded_dirtree_input_changed_rejects_replay() {
        let dir = TempDir::new().unwrap();
        let watched = dir.path().join("watched_tree");
        fs::create_dir(&watched).unwrap();
        fs::write(watched.join("a.txt"), "a\n").unwrap();
        fs::create_dir(watched.join("sub")).unwrap();
        fs::write(watched.join("sub").join("b.txt"), "b\n").unwrap();
        let digest = recorded_dirtree_marker_value(&watched).unwrap();
        fs::write(watched.join("sub").join("b.txt"), "changed\n").unwrap();

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push(format!("DIRTREE:{} {digest}", watched.display()));

        let cached =
            lockfile.get_extension_cache("@@ext//ext.bzl%ext", "bzl-digest", "usages-digest");
        assert!(cached.is_none());
    }

    #[test]
    fn recorded_env_input_matches_repo_env_value() {
        let mut repo_env = BTreeMap::new();
        repo_env.insert("PLAN61_REPO_ENV".to_owned(), "first".to_owned());

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push("ENV:PLAN61_REPO_ENV first".to_owned());

        let cached = lockfile.get_extension_cache_for_workspace(
            "@@ext//ext.bzl%ext",
            "bzl-digest",
            "usages-digest",
            None,
            Some(&repo_env),
            None,
            None,
            None,
        );
        assert!(cached.is_some());
    }

    #[test]
    fn recorded_env_input_changed_rejects_replay() {
        let mut repo_env = BTreeMap::new();
        repo_env.insert("PLAN61_REPO_ENV".to_owned(), "second".to_owned());

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push("ENV:PLAN61_REPO_ENV first".to_owned());

        let cached = lockfile.get_extension_cache_for_workspace(
            "@@ext//ext.bzl%ext",
            "bzl-digest",
            "usages-digest",
            None,
            Some(&repo_env),
            None,
            None,
            None,
        );
        assert!(cached.is_none());
    }

    #[test]
    fn recorded_env_input_unset_matches_absent_repo_env() {
        let repo_env = BTreeMap::new();

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push("ENV:PLAN61_REPO_ENV \\0".to_owned());

        let cached = lockfile.get_extension_cache_for_workspace(
            "@@ext//ext.bzl%ext",
            "bzl-digest",
            "usages-digest",
            None,
            Some(&repo_env),
            None,
            None,
            None,
        );
        assert!(cached.is_some());
    }

    #[test]
    fn recorded_repo_mapping_input_matches_current_mapping() {
        let mut root_mapping = BTreeMap::new();
        root_mapping.insert("dep".to_owned(), "dep_canonical".to_owned());
        let mut repo_mappings = crate::RepoMappingSnapshot::new();
        repo_mappings.insert(String::new(), root_mapping);

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push("REPO_MAPPING:,dep dep_canonical".to_owned());

        let cached = lockfile.get_extension_cache_for_workspace(
            "@@ext//ext.bzl%ext",
            "bzl-digest",
            "usages-digest",
            None,
            None,
            Some(&repo_mappings),
            None,
            None,
        );
        assert!(cached.is_some());
    }

    #[test]
    fn recorded_repo_mapping_input_changed_rejects_replay() {
        let mut root_mapping = BTreeMap::new();
        root_mapping.insert("dep".to_owned(), "other_canonical".to_owned());
        let mut repo_mappings = crate::RepoMappingSnapshot::new();
        repo_mappings.insert(String::new(), root_mapping);

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push("REPO_MAPPING:,dep dep_canonical".to_owned());

        let cached = lockfile.get_extension_cache_for_workspace(
            "@@ext//ext.bzl%ext",
            "bzl-digest",
            "usages-digest",
            None,
            None,
            Some(&repo_mappings),
            None,
            None,
        );
        assert!(cached.is_none());
    }

    #[test]
    fn recorded_repo_mapping_input_nonvisible_null_rejects_replay() {
        let mut repo_mappings = crate::RepoMappingSnapshot::new();
        repo_mappings.insert(String::new(), BTreeMap::new());

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push("REPO_MAPPING:,missing \\0".to_owned());

        let cached = lockfile.get_extension_cache_for_workspace(
            "@@ext//ext.bzl%ext",
            "bzl-digest",
            "usages-digest",
            None,
            None,
            Some(&repo_mappings),
            None,
            None,
        );
        assert!(cached.is_none());
    }

    #[test]
    fn recorded_repo_mapping_input_missing_source_repo_rejects_replay() {
        let repo_mappings = crate::RepoMappingSnapshot::new();

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push("REPO_MAPPING:missing_source,dep dep".to_owned());

        let cached = lockfile.get_extension_cache_for_workspace(
            "@@ext//ext.bzl%ext",
            "bzl-digest",
            "usages-digest",
            None,
            None,
            Some(&repo_mappings),
            None,
            None,
        );
        assert!(cached.is_none());
    }

    #[test]
    fn recorded_repo_mapping_input_for_extension_repo_uses_candidate_generated_specs() {
        let extension_id = "@owner//:ext.bzl%ext";
        let mut repo_mappings = crate::RepoMappingSnapshot::new();
        repo_mappings.insert("owner".to_owned(), BTreeMap::new());

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        repo_specs.insert("tool".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            extension_id.to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut(extension_id)
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push("REPO_MAPPING:owner++ext+tool,repo owner++ext+repo".to_owned());

        let cached = lockfile.get_extension_cache_for_workspace(
            extension_id,
            "bzl-digest",
            "usages-digest",
            None,
            None,
            Some(&repo_mappings),
            Some("root"),
            None,
        );
        assert!(cached.is_some());
    }

    #[test]
    fn recorded_repo_mapping_input_for_extension_repo_applies_root_override() {
        let extension_id = "@owner//:ext.bzl%ext";
        let mut repo_mappings = crate::RepoMappingSnapshot::new();
        repo_mappings.insert("owner".to_owned(), BTreeMap::new());
        let mut overrides = crate::RepoMappingOverrides::new();
        let mut extension_overrides = BTreeMap::new();
        extension_overrides.insert("repo".to_owned(), "actual_dep".to_owned());
        overrides.insert(extension_id.to_owned(), extension_overrides);

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        repo_specs.insert("tool".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            extension_id.to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut(extension_id)
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push("REPO_MAPPING:owner++ext+tool,repo actual_dep".to_owned());

        let cached = lockfile.get_extension_cache_for_workspace(
            extension_id,
            "bzl-digest",
            "usages-digest",
            None,
            None,
            Some(&repo_mappings),
            Some("root"),
            Some(&overrides),
        );
        assert!(cached.is_some());
    }

    #[test]
    fn malformed_recorded_input_rejects_replay() {
        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        lockfile.set_extension_cache(
            "@@ext//ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut("@@ext//ext.bzl%ext")
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push("FILE:/tmp/no-value".to_owned());

        let cached =
            lockfile.get_extension_cache("@@ext//ext.bzl%ext", "bzl-digest", "usages-digest");
        assert!(cached.is_none());
    }

    #[test]
    fn test_extension_facts_canonical_lookup() {
        let mut lockfile = Lockfile::new();
        lockfile.set_extension_facts(
            "@@rules_rs+//rs:extensions.bzl%crate".to_owned(),
            serde_json::json!({"serde_1.0.0": "cached"}),
        );

        assert_eq!(
            lockfile.get_extension_facts("@rules_rs//rs:extensions.bzl%crate"),
            Some(serde_json::json!({"serde_1.0.0": "cached"}))
        );
        assert_eq!(
            lockfile.get_extension_facts("@@rules_rs+//rs:extensions.bzl%crate"),
            Some(serde_json::json!({"serde_1.0.0": "cached"}))
        );
        assert_eq!(lockfile.get_extension_facts("@@other+//:ext.bzl%ext"), None);
    }

    #[test]
    fn extension_cache_misses_on_invalid_empty_target_label() {
        use crate::repository_invocations::AttrValue;

        let mut lockfile = Lockfile::new();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert(
            "zstd-sys".to_owned(),
            RepoSpec::new("@@rules_rs//rs:crate.bzl%crate_repository".to_owned()).with_attr(
                "deps".to_owned(),
                AttrValue::StringList(vec!["@@zstd//:".to_owned()]),
            ),
        );

        lockfile.set_extension_cache(
            "@@rules_rs//rs:extensions.bzl%crate".to_owned(),
            "bzl-digest".to_owned(),
            "usages-digest".to_owned(),
            &repo_specs,
        );

        assert!(
            lockfile
                .get_extension_cache(
                    "@@rules_rs//rs:extensions.bzl%crate",
                    "bzl-digest",
                    "usages-digest",
                )
                .is_none()
        );
    }

    #[test]
    fn test_extension_cache_miss_wrong_bzl_digest() {
        use crate::repository_invocations::AttrValue;

        let mut lockfile = Lockfile::new();

        let mut repo_specs = FxHashMap::default();
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

        let mut repo_specs = FxHashMap::default();
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

        let mut repo_specs = FxHashMap::default();
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
        let mut repo_specs = FxHashMap::default();
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
            raw_sha256_digest(1),
            raw_sha256_digest(2),
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
            &raw_sha256_digest(1),
            &raw_sha256_digest(2),
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
        let mut specs1 = FxHashMap::default();
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
        let mut specs2 = FxHashMap::default();
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

        let specs = FxHashMap::default();
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
            &FxHashMap::default(),
        );
        lockfile.set_extension_cache(
            "@@b//b.bzl%b".to_string(),
            "d2".to_string(),
            "u2".to_string(),
            &FxHashMap::default(),
        );

        let mut ids: Vec<_> = lockfile.extension_ids().collect();
        ids.sort();
        assert_eq!(ids, vec!["@@a//a.bzl%a", "@@b//b.bzl%b"]);
    }

    // --- Recorded-input replay tests ---

    #[test]
    fn test_recorded_input_file_create_and_delete() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("data.txt");

        // File does not exist initially
        let marker_when_absent = recorded_file_input(&file_path).unwrap();
        assert!(marker_when_absent.contains("ENOENT"));

        // Create file
        fs::write(&file_path, b"hello").unwrap();
        let marker_when_present = recorded_file_input(&file_path).unwrap();
        assert!(!marker_when_present.contains("ENOENT"));

        // Validate: absent marker should fail against present file
        let inputs_absent = vec![marker_when_absent.clone()];
        let result = validate_recorded_inputs_current(&inputs_absent, None, None, None);
        assert!(
            result.is_err(),
            "absent FILE marker should fail when file exists"
        );

        // Validate: present marker should pass
        let inputs_present = vec![marker_when_present.clone()];
        let result = validate_recorded_inputs_current(&inputs_present, None, None, None);
        assert!(
            result.is_ok(),
            "present FILE marker should pass when file exists"
        );

        // Delete file
        fs::remove_file(&file_path).unwrap();
        let result = validate_recorded_inputs_current(&inputs_present, None, None, None);
        assert!(
            result.is_err(),
            "present FILE marker should fail when file is deleted"
        );
    }

    #[test]
    fn test_recorded_input_dirents_create_and_delete() {
        let dir = TempDir::new().unwrap();
        let sub_dir = dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let marker_initial = recorded_dirents_input(&sub_dir).unwrap();

        // Add an entry
        fs::write(sub_dir.join("child.txt"), b"data").unwrap();
        let marker_with_child = recorded_dirents_input(&sub_dir).unwrap();
        assert_ne!(
            marker_initial, marker_with_child,
            "DIRENTS marker should change when entry added"
        );

        // Validate: initial marker should fail after adding entry
        let inputs_initial = vec![marker_initial.clone()];
        let result = validate_recorded_inputs_current(&inputs_initial, None, None, None);
        assert!(
            result.is_err(),
            "initial DIRENTS marker should fail after entry added"
        );

        // Validate: marker_with_child should pass
        let inputs_with_child = vec![marker_with_child.clone()];
        let result = validate_recorded_inputs_current(&inputs_with_child, None, None, None);
        assert!(
            result.is_ok(),
            "DIRENTS marker should pass when entries match"
        );

        // Delete the child
        fs::remove_file(sub_dir.join("child.txt")).unwrap();
        let result = validate_recorded_inputs_current(&inputs_with_child, None, None, None);
        assert!(
            result.is_err(),
            "DIRENTS marker should fail after entry deleted"
        );
    }

    #[test]
    fn test_recorded_input_dirtree_create_and_delete() {
        let dir = TempDir::new().unwrap();
        let sub_dir = dir.path().join("tree_dir");
        fs::create_dir(&sub_dir).unwrap();

        let marker_initial = recorded_dirtree_input(&sub_dir).unwrap();

        // Add a file in the tree
        fs::write(sub_dir.join("leaf.txt"), b"leaf-data").unwrap();
        let marker_with_leaf = recorded_dirtree_input(&sub_dir).unwrap();
        assert_ne!(
            marker_initial, marker_with_leaf,
            "DIRTREE marker should change when file added"
        );

        // Validate: initial marker should fail after adding leaf
        let inputs_initial = vec![marker_initial.clone()];
        let result = validate_recorded_inputs_current(&inputs_initial, None, None, None);
        assert!(
            result.is_err(),
            "initial DIRTREE marker should fail after leaf added"
        );

        // Validate: marker_with_leaf should pass
        let inputs_with_leaf = vec![marker_with_leaf.clone()];
        let result = validate_recorded_inputs_current(&inputs_with_leaf, None, None, None);
        assert!(
            result.is_ok(),
            "DIRTREE marker should pass when tree matches"
        );

        // Delete the leaf file
        fs::remove_file(sub_dir.join("leaf.txt")).unwrap();
        let result = validate_recorded_inputs_current(&inputs_with_leaf, None, None, None);
        assert!(
            result.is_err(),
            "DIRTREE marker should fail after leaf deleted"
        );
    }

    #[test]
    fn test_recorded_input_env_create_and_delete() {
        let env: BTreeMap<String, String> = BTreeMap::new();

        // Record an env var that is absent
        let marker_absent = recorded_env_input("MY_VAR", None);
        let inputs_absent = vec![marker_absent.clone()];

        // Validate with empty env: should pass
        let result = validate_recorded_inputs_current(&inputs_absent, None, Some(&env), None);
        assert!(
            result.is_ok(),
            "absent ENV marker should pass when var is absent"
        );

        // Env now has the variable
        let env_present: BTreeMap<String, String> = {
            let mut m = BTreeMap::new();
            m.insert("MY_VAR".to_owned(), "value".to_owned());
            m
        };
        let result =
            validate_recorded_inputs_current(&inputs_absent, None, Some(&env_present), None);
        assert!(
            result.is_err(),
            "absent ENV marker should fail when var is present"
        );

        // Record a present env var
        let marker_present = recorded_env_input("MY_VAR", Some("value"));
        let inputs_present = vec![marker_present.clone()];

        let result =
            validate_recorded_inputs_current(&inputs_present, None, Some(&env_present), None);
        assert!(
            result.is_ok(),
            "present ENV marker should pass when var matches"
        );

        // Env var removed again
        let result = validate_recorded_inputs_current(&inputs_present, None, Some(&env), None);
        assert!(
            result.is_err(),
            "present ENV marker should fail when var is removed"
        );
    }

    #[test]
    fn test_recorded_input_repo_mapping_create_and_delete() {
        let mut snapshot = crate::RepoMappingSnapshot::new();

        // No mapping for source repo initially
        let marker_absent = recorded_repo_mapping_input("source_repo", "apparent_name", None);
        let inputs_absent = vec![marker_absent.clone()];

        let result = validate_recorded_inputs_current(&inputs_absent, None, None, Some(&snapshot));
        assert!(
            result.is_ok(),
            "absent REPO_MAPPING marker should pass when mapping absent"
        );

        // Add a mapping
        let mut mapping = BTreeMap::new();
        mapping.insert("apparent_name".to_owned(), "canonical_name".to_owned());
        snapshot.insert("source_repo".to_owned(), mapping);

        let result = validate_recorded_inputs_current(&inputs_absent, None, None, Some(&snapshot));
        assert!(
            result.is_err(),
            "absent REPO_MAPPING marker should fail when mapping present"
        );

        // Record present mapping
        let marker_present =
            recorded_repo_mapping_input("source_repo", "apparent_name", Some("canonical_name"));
        let inputs_present = vec![marker_present.clone()];

        let result = validate_recorded_inputs_current(&inputs_present, None, None, Some(&snapshot));
        assert!(
            result.is_ok(),
            "present REPO_MAPPING marker should pass when mapping matches"
        );

        // Remove the mapping
        let empty_snapshot = crate::RepoMappingSnapshot::new();
        let result =
            validate_recorded_inputs_current(&inputs_present, None, None, Some(&empty_snapshot));
        assert!(
            result.is_err(),
            "present REPO_MAPPING marker should fail when mapping removed"
        );
    }

    #[test]
    fn lockfile_lifecycle_creates_lockfile_on_first_build() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let workspace_root = temp_dir.path();

        let mut graph = crate::resolution::ResolvedGraph::default();
        graph.registry_file_hashes.insert(
            "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel".to_owned(),
            "abcdef1234567890".to_owned(),
        );

        let written = crate::persist_lockfile_after_resolution(
            workspace_root,
            LockfileMode::Update,
            &graph,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(written, "first build should write the lockfile");

        let lockfile_path = workspace_root.join("MODULE.bazel.lock");
        assert!(lockfile_path.exists(), "lockfile should exist on disk");

        let parsed: Lockfile =
            serde_json::from_str(&std::fs::read_to_string(&lockfile_path).unwrap()).unwrap();
        assert_eq!(parsed.lock_file_version, LOCKFILE_VERSION);
        assert_eq!(parsed.registry_file_hashes.len(), 1);
        assert!(
            parsed
                .registry_file_hashes
                .contains_key("https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel")
        );
    }

    #[test]
    fn lockfile_lifecycle_skips_rewrite_when_unchanged() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let workspace_root = temp_dir.path();

        let mut graph = crate::resolution::ResolvedGraph::default();
        graph.registry_file_hashes.insert(
            "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel".to_owned(),
            "abcdef1234567890".to_owned(),
        );

        let written1 = crate::persist_lockfile_after_resolution(
            workspace_root,
            LockfileMode::Update,
            &graph,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(written1, "first build should write");

        let written2 = crate::persist_lockfile_after_resolution(
            workspace_root,
            LockfileMode::Update,
            &graph,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(!written2, "second build with same data should not rewrite");
    }

    #[test]
    fn lockfile_lifecycle_refresh_mode_creates_lockfile() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let workspace_root = temp_dir.path();

        let mut graph = crate::resolution::ResolvedGraph::default();
        graph.registry_file_hashes.insert(
            "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel".to_owned(),
            "abcdef1234567890".to_owned(),
        );

        let written = crate::persist_lockfile_after_resolution(
            workspace_root,
            LockfileMode::Refresh,
            &graph,
            &[],
            &[],
            &[],
        )
        .unwrap();

        assert!(written, "refresh mode should write the lockfile");
        assert!(
            workspace_root.join("MODULE.bazel.lock").exists(),
            "refresh mode should create the lockfile when missing"
        );
    }

    #[test]
    fn lockfile_lifecycle_error_mode_rejects_registry_hash_drift() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let workspace_root = temp_dir.path();

        let mut graph = crate::resolution::ResolvedGraph::default();
        graph.registry_file_hashes.insert(
            "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel".to_owned(),
            "old_hash".to_owned(),
        );

        crate::persist_lockfile_after_resolution(
            workspace_root,
            LockfileMode::Update,
            &graph,
            &[],
            &[],
            &[],
        )
        .unwrap();

        let mut drifted_graph = crate::resolution::ResolvedGraph::default();
        drifted_graph.registry_file_hashes.insert(
            "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel".to_owned(),
            "new_hash".to_owned(),
        );

        let existing: Lockfile = serde_json::from_str(
            &std::fs::read_to_string(workspace_root.join("MODULE.bazel.lock")).unwrap(),
        )
        .unwrap();

        let err = Lockfile::enforce_error_mode(&drifted_graph, &existing).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("registry file hash drift"),
            "error mode should detect hash drift: {msg}"
        );
    }

    #[test]
    fn lockfile_lifecycle_error_mode_rejects_new_registry_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let workspace_root = temp_dir.path();

        let mut graph = crate::resolution::ResolvedGraph::default();
        graph.registry_file_hashes.insert(
            "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel".to_owned(),
            "hash1".to_owned(),
        );

        crate::persist_lockfile_after_resolution(
            workspace_root,
            LockfileMode::Update,
            &graph,
            &[],
            &[],
            &[],
        )
        .unwrap();

        let mut new_graph = crate::resolution::ResolvedGraph::default();
        new_graph.registry_file_hashes.insert(
            "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel".to_owned(),
            "hash1".to_owned(),
        );
        new_graph.registry_file_hashes.insert(
            "https://bcr.bazel.build/modules/rules_java/7.1/MODULE.bazel".to_owned(),
            "hash2".to_owned(),
        );

        let existing: Lockfile = serde_json::from_str(
            &std::fs::read_to_string(workspace_root.join("MODULE.bazel.lock")).unwrap(),
        )
        .unwrap();

        let err = Lockfile::enforce_error_mode(&new_graph, &existing).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("new registry file") && msg.contains("not in lockfile"),
            "error mode should detect new registry file: {msg}"
        );
    }

    #[test]
    fn lockfile_lifecycle_off_mode_neither_reads_nor_writes() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let workspace_root = temp_dir.path();

        let mut graph = crate::resolution::ResolvedGraph::default();
        graph.registry_file_hashes.insert(
            "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel".to_owned(),
            "hash1".to_owned(),
        );

        let written = crate::persist_lockfile_after_resolution(
            workspace_root,
            LockfileMode::Off,
            &graph,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(!written, "off mode should not write");
        assert!(
            !workspace_root.join("MODULE.bazel.lock").exists(),
            "off mode should not create lockfile"
        );
    }

    #[test]
    fn lockfile_lifecycle_error_mode_skips_write() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let workspace_root = temp_dir.path();

        let graph = crate::resolution::ResolvedGraph::default();

        let written = crate::persist_lockfile_after_resolution(
            workspace_root,
            LockfileMode::Error,
            &graph,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(!written, "error mode should not write");
        assert!(
            !workspace_root.join("MODULE.bazel.lock").exists(),
            "error mode should not create lockfile"
        );
    }

    #[test]
    fn from_resolved_graph_with_extensions_merges_extension_data() {
        let mut graph = crate::resolution::ResolvedGraph::default();
        graph.registry_file_hashes.insert(
            "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel".to_owned(),
            "hash1".to_owned(),
        );

        let mut old_lockfile = Lockfile::new();
        let old_ext_data = LockfileExtensionData::new(
            "old_bzl_digest".to_owned(),
            "old_usages_digest".to_owned(),
            IndexMap::new(),
        );
        old_lockfile.module_extensions.insert(
            "@@rules_python+//python/extensions:pip.bzl%pip".to_owned(),
            old_ext_data,
        );
        old_lockfile.facts.insert(
            "@@rules_python+//python/extensions:pip.bzl%pip".to_owned(),
            serde_json::json!({"version": "1.0"}),
        );

        let new_ext_data = LockfileExtensionData::new(
            "new_bzl_digest".to_owned(),
            "new_usages_digest".to_owned(),
            IndexMap::new(),
        );

        let active_ids = vec![
            "@@rules_python+//python/extensions:pip.bzl%pip".to_owned(),
            "@@rules_java+//java/extensions:java.bzl%java".to_owned(),
        ];

        let new_facts = vec![(
            "@@rules_python+//python/extensions:pip.bzl%pip".to_owned(),
            serde_json::json!({"version": "2.0"}),
        )];

        let result = Lockfile::from_resolved_graph_with_extensions(
            &graph,
            Some(&old_lockfile),
            &[(
                "@@rules_java+//java/extensions:java.bzl%java".to_owned(),
                new_ext_data,
            )],
            &active_ids,
            &new_facts,
        );

        assert_eq!(result.registry_file_hashes.len(), 1);
        assert_eq!(result.module_extensions.len(), 2);
        assert!(
            result
                .module_extensions
                .contains_key("@@rules_python+//python/extensions:pip.bzl%pip")
        );
        assert!(
            result
                .module_extensions
                .contains_key("@@rules_java+//java/extensions:java.bzl%java")
        );

        let pip_facts = result
            .facts
            .get("@@rules_python+//python/extensions:pip.bzl%pip")
            .unwrap();
        assert_eq!(pip_facts["version"], "2.0");
    }

    #[test]
    fn from_resolved_graph_with_extensions_excludes_file_registry_urls() {
        let mut graph = crate::resolution::ResolvedGraph::default();
        graph.registry_file_hashes.insert(
            "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel".to_owned(),
            "hash1".to_owned(),
        );
        graph.registry_file_hashes.insert(
            "file:///home/user/local-registry/modules/foo/MODULE.bazel".to_owned(),
            "local_hash".to_owned(),
        );

        let result = Lockfile::from_resolved_graph_with_extensions(&graph, None, &[], &[], &[]);

        assert_eq!(result.registry_file_hashes.len(), 1);
        assert!(
            !result
                .registry_file_hashes
                .contains_key("file:///home/user/local-registry/modules/foo/MODULE.bazel")
        );
        assert!(
            result
                .registry_file_hashes
                .contains_key("https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel")
        );
    }

    #[test]
    fn from_resolved_graph_with_extensions_prunes_inactive_extensions() {
        let graph = crate::resolution::ResolvedGraph::default();
        let mut old_lockfile = Lockfile::new();
        old_lockfile.module_extensions.insert(
            "@@old_extension//ext.bzl%old".to_owned(),
            LockfileExtensionData::new("d1".to_owned(), "d2".to_owned(), IndexMap::new()),
        );
        old_lockfile.facts.insert(
            "@@old_extension//ext.bzl%old".to_owned(),
            serde_json::json!({"orphaned_key": "should_be_pruned"}),
        );

        let result = Lockfile::from_resolved_graph_with_extensions(
            &graph,
            Some(&old_lockfile),
            &[],
            &["@@new_extension//ext.bzl%new".to_owned()],
            &[],
        );

        assert!(
            !result
                .module_extensions
                .contains_key("@@old_extension//ext.bzl%old"),
            "inactive extensions should be pruned"
        );
        assert!(
            !result.facts.contains_key("@@old_extension//ext.bzl%old"),
            "facts for inactive extensions should be pruned"
        );
    }

    #[test]
    fn from_resolved_graph_with_extensions_sorts_output_deterministically() {
        let graph = crate::resolution::ResolvedGraph::default();
        let ext_z = LockfileExtensionData::new("z".to_owned(), "z".to_owned(), IndexMap::new());
        let ext_a = LockfileExtensionData::new("a".to_owned(), "a".to_owned(), IndexMap::new());

        let result = Lockfile::from_resolved_graph_with_extensions(
            &graph,
            None,
            &[
                ("@@z_ext//ext.bzl%z".to_owned(), ext_z),
                ("@@a_ext//ext.bzl%a".to_owned(), ext_a),
            ],
            &[
                "@@z_ext//ext.bzl%z".to_owned(),
                "@@a_ext//ext.bzl%a".to_owned(),
            ],
            &[],
        );

        let keys: Vec<&str> = result
            .module_extensions
            .keys()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            keys,
            &["@@a_ext//ext.bzl%a", "@@z_ext//ext.bzl%z"],
            "extensions should be sorted lexicographically"
        );
    }

    #[test]
    fn active_extension_ids_for_persist_preserves_locked_on_replay() {
        // Cache-replayed build: nothing re-aggregated this resolution, but the
        // injected lockfile already records an extension in use. It must stay
        // active so the post-build persist does not prune it to {}.
        let result = Lockfile::active_extension_ids_for_persist(
            std::iter::empty(),
            ["@@rules_rust+//rust/private/internal_extensions.bzl%i".to_owned()],
        );
        assert_eq!(
            result,
            vec!["@@rules_rust+//rust/private/internal_extensions.bzl%i".to_owned()],
            "locked extensions must be preserved when nothing was re-aggregated"
        );
    }

    #[test]
    fn active_extension_ids_for_persist_unions_and_dedups_sorted() {
        let result = Lockfile::active_extension_ids_for_persist(
            ["@@b//e.bzl%x".to_owned(), "@@a//e.bzl%y".to_owned()],
            ["@@a//e.bzl%y".to_owned(), "@@c//e.bzl%z".to_owned()],
        );
        assert_eq!(
            result,
            vec![
                "@@a//e.bzl%y".to_owned(),
                "@@b//e.bzl%x".to_owned(),
                "@@c//e.bzl%z".to_owned(),
            ],
            "result must be the deduplicated, sorted union of aggregated and locked"
        );
    }
}
