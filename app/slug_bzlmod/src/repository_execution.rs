/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! DICE-based repository rule execution.
//!
//! This module provides DICE keys and computation for executing repository rules
//! like `http_archive` and `git_repository`. Repository rules are executed
//! lazily via DICE, allowing:
//!
//! - Caching of repository rule results
//! - Parallel execution of independent repository rules
//! - Incremental re-execution when inputs change
//!
//! ## Architecture
//!
//! 1. Repository rule invocations are recorded during MODULE.bazel/extension parsing
//! 2. When a repository is needed, `RepositoryRuleExecutionKey::compute()` is called
//! 3. The computation creates a working directory, invokes the rule implementation,
//!    and registers the result with the materializer.
//!
//! Until this key is fully wired to the native/Starlark repository executor, it
//! must fail directly rather than returning a placeholder successful repository.
//!
//! ## Pattern Reference
//!
//! This follows the `GitFileOpsDelegateKey` pattern from `slug_external_cells/src/git.rs`.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use base64::Engine;
use derive_more::Display;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_util::late_binding::LateBinding;

use crate::dice_graph::BzlmodEventKind;
use crate::dice_graph::RepoMaterializationManifestKey;
use crate::dice_graph::RepoMaterializationManifestValue;
use crate::dice_graph::record_bzlmod_event;
use crate::dice_graph::repo_env_policy_digest;
use crate::lockfile::compute_sha256_hex;
use crate::lockfile::validate_recorded_inputs_current;
use crate::repo_spec::RepoSpec;
use crate::repository_executor::RepositoryLabelResolution;
use crate::repository_invocations::AttrValue;
use crate::repository_invocations::RepositoryInvocation;

pub(crate) const REPO_RECORDED_INPUTS_FILE: &str = ".slug_repo_recorded_inputs";
const REPO_COMPLETE_MARKER_FILE: &str = ".slug_repo_complete";
const REPO_RULE_LOCAL_FILE: &str = ".slug_repo_rule_local";
const MARKER_CONTENT_PREFIX: &str = "marker-content:";

/// Late-bound reader for repository materialization state files.
///
/// `slug_bzlmod` owns the materialization manifest keys, but the DICE file
/// readers live in `slug_common`. This trait lets production installs read
/// materialization state through those DICE keys without adding a crate cycle.
#[async_trait]
pub trait RepositoryMaterializationStateReader: Send + Sync + 'static {
    async fn read_repo_state_file_if_exists(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: crate::WorkspaceId,
        repo_dir: Arc<PathBuf>,
        file_name: &'static str,
    ) -> Result<Option<Arc<str>>, Arc<str>>;

    async fn repo_state_file_exists(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: crate::WorkspaceId,
        repo_dir: Arc<PathBuf>,
        file_name: &'static str,
    ) -> Result<bool, Arc<str>>;

    async fn repo_has_foreign_top_level_symlink(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: crate::WorkspaceId,
        repo_dir: Arc<PathBuf>,
    ) -> Result<bool, Arc<str>>;

    async fn repo_dir_entry_names(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: crate::WorkspaceId,
        dir: Arc<PathBuf>,
    ) -> Result<Arc<Vec<String>>, Arc<str>>;

    async fn repo_symlink_points_to(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: crate::WorkspaceId,
        symlink_path: Arc<PathBuf>,
        expected_target: Arc<PathBuf>,
    ) -> Result<bool, Arc<str>>;

    async fn repo_output_digest(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: crate::WorkspaceId,
        repo_dir: Arc<PathBuf>,
    ) -> Result<Arc<str>, Arc<str>>;
}

/// Initialized by `slug_external_cells::init_late_bindings()`.
pub static REPOSITORY_MATERIALIZATION_STATE_READER_IMPL: LateBinding<
    &'static dyn RepositoryMaterializationStateReader,
> = LateBinding::new("REPOSITORY_MATERIALIZATION_STATE_READER_IMPL");

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "RepositoryLabelResolutionKey({}, {})",
    workspace_id.stable_hash(),
    project_root.display()
)]
struct RepositoryLabelResolutionKey {
    workspace_id: crate::WorkspaceId,
    project_root: Arc<PathBuf>,
}

impl RepositoryLabelResolutionKey {
    fn new(workspace_id: crate::WorkspaceId, project_root: Arc<PathBuf>) -> Self {
        Self {
            workspace_id,
            project_root,
        }
    }
}

#[async_trait]
impl Key for RepositoryLabelResolutionKey {
    type Value = slug_error::Result<Arc<RepositoryLabelResolution>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let cell_graph =
            crate::bzlmod_cell_graph_for_workspace_id(ctx, self.workspace_id.clone()).await?;
        Ok(Arc::new(RepositoryLabelResolution::from_cell_graph(
            &self.project_root,
            &cell_graph,
        )))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
}

/// Errors that can occur during repository rule execution.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
pub enum RepositoryExecutionError {
    #[error("Repository rule execution failed for '{name}': {reason}")]
    ExecutionFailed { name: String, reason: String },

    #[error("Repository '{name}' not found in invocation registry")]
    RepositoryNotFound { name: String },

    #[error("Required attribute '{attr}' not found for repository '{name}'")]
    MissingAttribute { name: String, attr: String },

    #[error("Working directory creation failed: {reason}")]
    WorkingDirFailed { reason: String },

    #[error("Repository rule '{name}' has no implementation")]
    NoImplementation { name: String },

    #[error("Failed to convert RepoSpec to invocation for '{canonical_name}': {reason}")]
    RepoSpecConversionFailed {
        canonical_name: String,
        reason: String,
    },

    #[error(
        "Invalid repo_rule_id format: '{repo_rule_id}' (expected format: @@module//path:file.bzl%rule_name)"
    )]
    InvalidRepoRuleId { repo_rule_id: String },
}

/// Result of executing a repository rule.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryRuleResult {
    /// Path to the materialized repository (project-relative).
    pub repo_path: PathBuf,

    /// Hash of repo contents for cache invalidation.
    pub content_hash: Option<String>,

    /// The repository name.
    pub repo_name: String,

    /// Whether execution succeeded.
    pub success: bool,

    /// Whether DICE may reuse this result across transactions.
    pub cacheable: bool,
}

impl RepositoryRuleResult {
    /// Create a successful result.
    pub fn success(repo_name: String, repo_path: PathBuf) -> Self {
        Self {
            repo_path,
            content_hash: None,
            repo_name,
            success: true,
            cacheable: true,
        }
    }

    /// Create a result with a content hash.
    pub fn with_content_hash(mut self, hash: String) -> Self {
        self.content_hash = Some(hash);
        self
    }

    /// Mark this result as unsuitable for DICE reuse across transactions.
    pub fn non_cacheable(mut self) -> Self {
        self.cacheable = false;
        self
    }
}

/// DICE key for repository rule execution.
///
/// When this key is computed, it executes the repository rule and materializes
/// the repository content to disk.
#[cfg(test)]
#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("RepositoryRuleKey({}, {})", name, rule_name)]
pub struct RepositoryRuleExecutionKey {
    /// Repository name (from the `name` attribute).
    pub name: Arc<str>,

    /// Repository rule name (e.g., "http_archive").
    pub rule_name: Arc<str>,

    /// Hash of attributes for cache invalidation.
    pub attrs_hash: Arc<str>,
}

#[cfg(test)]
impl RepositoryRuleExecutionKey {
    /// Create a new execution key from an invocation.
    pub fn from_invocation(invocation: &RepositoryInvocation) -> Self {
        Self {
            name: Arc::from(invocation.name.as_str()),
            rule_name: Arc::from(invocation.rule_name.as_str()),
            attrs_hash: Arc::from(invocation.compute_hash().as_str()),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl Key for RepositoryRuleExecutionKey {
    type Value = slug_error::Result<Arc<RepositoryRuleResult>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        tracing::warn!(
            "RepositoryRuleExecutionKey for '{}' cannot execute repository rule '{}' yet",
            self.name,
            self.rule_name
        );
        Err(repository_rule_execution_key_unimplemented_error(&self.name, &self.rule_name).into())
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        // Don't cache errors - retry on next request
        x.as_ref().is_ok_and(|result| result.cacheable)
    }
}

/// Explain why direct `RepositoryRuleExecutionKey` computation is disabled.
#[cfg(test)]
fn repository_rule_execution_key_unimplemented_error(
    repository_name: &str,
    rule_name: &str,
) -> RepositoryExecutionError {
    tracing::debug!(
        "RepositoryRuleExecutionKey for '{}' has no direct executor for rule '{}'",
        repository_name,
        rule_name
    );
    RepositoryExecutionError::NoImplementation {
        name: rule_name.to_owned(),
    }
}

/// DICE key for lazy execution of extension-generated repositories.
///
/// This key is computed when a repository generated by a module extension is first
/// accessed during a build. It takes a `RepoSpec` (captured during extension evaluation)
/// and materializes the repository to disk.
///
/// Unlike `RepositoryRuleExecutionKey` which works with `RepositoryInvocation`,
/// this key works with `RepoSpec` which includes the full rule identifier needed
/// to locate the repository rule implementation.
///
/// Note: Hash and Eq are implemented manually because `RepoSpec` contains a HashMap.
/// The `spec_hash` field is used for hashing, ensuring deterministic cache behavior.
#[derive(Clone, Debug, Display, Allocative, Dupe)]
#[display("ExtensionRepoKey({}, {})", canonical_name, spec_hash)]
pub struct ExtensionRepoExecutionKey {
    /// Canonical repo name (e.g., "_main+pip+numpy").
    pub canonical_name: Arc<str>,

    /// Extension that generated this repo (e.g., "@@rules_python//pip:pip.bzl%pip").
    pub extension_id: Arc<str>,

    /// Hash of RepoSpec for cache invalidation.
    pub spec_hash: Arc<str>,

    /// The RepoSpec to execute.
    pub repo_spec: Arc<RepoSpec>,

    /// Project root for repository materialization.
    /// Repositories are created under {project_root}/bazel-external/{canonical_name}/
    pub project_root: Arc<PathBuf>,

    /// DICE-owned pre-materialization manifest key for marker/layout reuse.
    ///
    /// This mirrors Bazel's repository marker-file pruning model: key the
    /// fetch on marker contents and cheap layout checks, not a full tree walk
    /// of every external repo on access.
    pub materialization_manifest_key: Arc<RepoMaterializationManifestKey>,

    /// Effective repository environment for Starlark repository rule execution.
    pub repo_env: Arc<BTreeMap<String, String>>,

    /// Current scoped repository mappings used for REPO_MAPPING recorded-input replay.
    pub repo_mappings: Arc<crate::RepoMappingSnapshot>,
}

impl std::hash::Hash for ExtensionRepoExecutionKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the identifying fields; spec_hash represents the repo_spec
        self.canonical_name.hash(state);
        self.extension_id.hash(state);
        self.spec_hash.hash(state);
        self.project_root.hash(state);
        self.materialization_manifest_key.hash(state);
        self.repo_env.hash(state);
        self.repo_mappings.hash(state);
    }
}

impl PartialEq for ExtensionRepoExecutionKey {
    fn eq(&self, other: &Self) -> bool {
        // Compare by identifying fields; spec_hash represents the repo_spec
        self.canonical_name == other.canonical_name
            && self.extension_id == other.extension_id
            && self.spec_hash == other.spec_hash
            && self.project_root == other.project_root
            && self.materialization_manifest_key == other.materialization_manifest_key
            && self.repo_env == other.repo_env
            && self.repo_mappings == other.repo_mappings
    }
}

impl Eq for ExtensionRepoExecutionKey {}

pub fn repo_execution_spec_hash(
    repo_spec: &RepoSpec,
    repo_env: &BTreeMap<String, String>,
) -> String {
    let repo_spec_hash = repo_spec.compute_hash();
    if repo_env.is_empty() {
        return repo_spec_hash;
    }

    let repo_env_digest = repo_env_policy_digest(repo_env);
    let mut hasher = Sha256::new();
    hasher.update(b"repo-execution-spec-v1");
    hasher.update([0]);
    hasher.update(repo_spec_hash.as_bytes());
    hasher.update([0]);
    hasher.update(repo_env_digest.as_bytes());
    format!(
        "sha256-{}",
        base64::engine::general_purpose::STANDARD.encode(hasher.finalize())
    )
}

impl ExtensionRepoExecutionKey {
    /// Create a new extension repo execution key.
    #[cfg(test)]
    pub fn new(
        canonical_name: String,
        extension_id: String,
        repo_spec: RepoSpec,
        project_root: PathBuf,
    ) -> Self {
        Self::new_with_repo_env(
            canonical_name,
            extension_id,
            repo_spec,
            project_root,
            Arc::new(BTreeMap::new()),
        )
    }

    /// Create a new extension repo execution key with command repo-env.
    #[cfg(test)]
    pub fn new_with_repo_env(
        canonical_name: String,
        extension_id: String,
        repo_spec: RepoSpec,
        project_root: PathBuf,
        repo_env: Arc<BTreeMap<String, String>>,
    ) -> Self {
        Self::new_with_workspace_id_and_repo_env(
            canonical_name,
            extension_id,
            repo_spec,
            crate::WorkspaceId::for_project_root(project_root),
            repo_env,
        )
    }

    /// Create a new extension repo execution key with explicit workspace identity.
    #[cfg(test)]
    pub fn new_with_workspace_id(
        canonical_name: String,
        extension_id: String,
        repo_spec: RepoSpec,
        workspace_id: crate::WorkspaceId,
    ) -> Self {
        Self::new_with_workspace_id_and_repo_env(
            canonical_name,
            extension_id,
            repo_spec,
            workspace_id,
            Arc::new(BTreeMap::new()),
        )
    }

    /// Create a new extension repo execution key with explicit workspace
    /// identity and command repo-env.
    pub fn new_with_workspace_id_and_repo_env(
        canonical_name: String,
        extension_id: String,
        repo_spec: RepoSpec,
        workspace_id: crate::WorkspaceId,
        repo_env: Arc<BTreeMap<String, String>>,
    ) -> Self {
        Self::new_with_workspace_id_repo_env_and_repo_mappings(
            canonical_name,
            extension_id,
            repo_spec,
            workspace_id,
            repo_env,
            Arc::new(crate::RepoMappingSnapshot::new()),
        )
    }

    /// Create a new extension repo execution key with explicit workspace
    /// identity, command repo-env, and recorded-input repo mappings.
    pub fn new_with_workspace_id_repo_env_and_repo_mappings(
        canonical_name: String,
        extension_id: String,
        repo_spec: RepoSpec,
        workspace_id: crate::WorkspaceId,
        repo_env: Arc<BTreeMap<String, String>>,
        repo_mappings: Arc<crate::RepoMappingSnapshot>,
    ) -> Self {
        let project_root = workspace_id.canonical_project_root.as_ref().clone();
        let spec_hash = repo_execution_spec_hash(&repo_spec, &repo_env);
        let repo_spec = Arc::new(repo_spec);
        let materialization_manifest_key =
            RepoMaterializationManifestKey::for_workspace_id_with_repo_spec_digest_repo_env_and_repo_mappings(
                workspace_id,
                canonical_name.as_str(),
                repo_spec.clone(),
                spec_hash.clone(),
                repo_env.clone(),
                repo_mappings.clone(),
            );
        Self {
            canonical_name: Arc::from(canonical_name.as_str()),
            extension_id: Arc::from(extension_id.as_str()),
            spec_hash: Arc::from(spec_hash.as_str()),
            repo_spec,
            project_root: Arc::new(project_root),
            materialization_manifest_key: Arc::new(materialization_manifest_key),
            repo_env,
            repo_mappings,
        }
    }

    /// Create from Arc references (avoids cloning for repeated use).
    #[cfg(test)]
    pub fn from_arcs(
        canonical_name: Arc<str>,
        extension_id: Arc<str>,
        repo_spec: Arc<RepoSpec>,
        project_root: Arc<PathBuf>,
    ) -> Self {
        Self::from_arcs_with_repo_env(
            canonical_name,
            extension_id,
            repo_spec,
            project_root,
            Arc::new(BTreeMap::new()),
        )
    }

    /// Create from Arc references with command repo-env.
    #[cfg(test)]
    pub fn from_arcs_with_repo_env(
        canonical_name: Arc<str>,
        extension_id: Arc<str>,
        repo_spec: Arc<RepoSpec>,
        project_root: Arc<PathBuf>,
        repo_env: Arc<BTreeMap<String, String>>,
    ) -> Self {
        Self::from_arcs_with_workspace_id_and_repo_env(
            canonical_name,
            extension_id,
            repo_spec,
            crate::WorkspaceId::for_project_root(project_root.as_ref().clone()),
            repo_env,
        )
    }

    /// Create from Arc references with explicit workspace identity and command repo-env.
    pub fn from_arcs_with_workspace_id_and_repo_env(
        canonical_name: Arc<str>,
        extension_id: Arc<str>,
        repo_spec: Arc<RepoSpec>,
        workspace_id: crate::WorkspaceId,
        repo_env: Arc<BTreeMap<String, String>>,
    ) -> Self {
        Self::from_arcs_with_workspace_id_repo_env_and_repo_mappings(
            canonical_name,
            extension_id,
            repo_spec,
            workspace_id,
            repo_env,
            Arc::new(crate::RepoMappingSnapshot::new()),
        )
    }

    /// Create from Arc references with explicit workspace identity, command
    /// repo-env, and recorded-input repo mappings.
    pub fn from_arcs_with_workspace_id_repo_env_and_repo_mappings(
        canonical_name: Arc<str>,
        extension_id: Arc<str>,
        repo_spec: Arc<RepoSpec>,
        workspace_id: crate::WorkspaceId,
        repo_env: Arc<BTreeMap<String, String>>,
        repo_mappings: Arc<crate::RepoMappingSnapshot>,
    ) -> Self {
        let project_root = workspace_id.canonical_project_root.clone();
        let spec_hash = repo_execution_spec_hash(&repo_spec, &repo_env);
        let materialization_manifest_key =
            RepoMaterializationManifestKey::for_workspace_id_with_repo_spec_digest_repo_env_and_repo_mappings(
                workspace_id,
                canonical_name.as_ref(),
                repo_spec.clone(),
                spec_hash.clone(),
                repo_env.clone(),
                repo_mappings.clone(),
            );
        Self {
            canonical_name,
            extension_id,
            spec_hash: Arc::from(spec_hash.as_str()),
            repo_spec,
            project_root,
            materialization_manifest_key: Arc::new(materialization_manifest_key),
            repo_env,
            repo_mappings,
        }
    }

    /// Create with default project root (current directory).
    /// Primarily for testing.
    #[cfg(test)]
    pub fn new_with_cwd(canonical_name: String, extension_id: String, repo_spec: RepoSpec) -> Self {
        Self::new(
            canonical_name,
            extension_id,
            repo_spec,
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        )
    }
}

fn complete_marker(spec_hash: &str, output_digest: &str) -> String {
    if spec_hash.is_empty() && output_digest.is_empty() {
        "complete".to_owned()
    } else if output_digest.is_empty() {
        format!("complete:{spec_hash}")
    } else if spec_hash.is_empty() {
        format!("complete:output:{output_digest}")
    } else {
        format!("complete:{spec_hash}:output:{output_digest}")
    }
}

fn complete_marker_matches(marker: &str, spec_hash: &str) -> bool {
    let marker = marker.trim();
    if spec_hash.is_empty() {
        return marker == "complete" || marker.starts_with("complete:output:");
    }
    marker
        .strip_prefix(&format!("complete:{spec_hash}:output:"))
        .is_some_and(|output_digest| !output_digest.is_empty())
}

fn complete_marker_expected_output<'a>(marker: &'a str, spec_hash: &str) -> Option<&'a str> {
    let marker = marker.trim();
    if spec_hash.is_empty() {
        return marker.strip_prefix("complete:output:");
    }
    marker.strip_prefix(&format!("complete:{spec_hash}:output:"))
}

#[cfg(test)]
fn complete_marker_state(marker: &str, spec_hash: &str, repo_dir: &Path) -> String {
    complete_marker_state_with_output_digest(marker, spec_hash, || {
        crate::repository_executor::repository_output_digest(repo_dir).map_err(|e| e.to_string())
    })
}

#[cfg(test)]
fn complete_marker_state_with_output_digest(
    marker: &str,
    spec_hash: &str,
    current_output_digest: impl FnOnce() -> Result<String, String>,
) -> String {
    let marker = marker.trim();
    if !complete_marker_matches(marker, spec_hash) {
        return format!("marker-mismatch:{marker}");
    }
    let Some(expected_output_digest) = complete_marker_expected_output(marker, spec_hash) else {
        return format!("marker:{marker}");
    };
    match current_output_digest() {
        Ok(current_output_digest) if current_output_digest == expected_output_digest => {
            format!("marker:{marker}")
        }
        Ok(current_output_digest) => {
            format!("marker-output-mismatch:{marker}:current:{current_output_digest}")
        }
        Err(e) => format!("marker-output-unreadable:{marker}:{e}"),
    }
}

#[cfg(test)]
fn repository_recorded_inputs_digest(
    repo_dir: &Path,
    repo_env: Option<&BTreeMap<String, String>>,
    repo_mappings: Option<&crate::RepoMappingSnapshot>,
) -> Result<Option<String>, String> {
    let manifest_path = repo_dir.join(REPO_RECORDED_INPUTS_FILE);
    if !manifest_path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(&manifest_path).map_err(|_| "recorded_inputs_unreadable")?;
    let recorded_inputs = parse_repository_recorded_inputs(&content);
    validate_recorded_inputs_current(&recorded_inputs, None, repo_env, repo_mappings)?;
    Ok(Some(compute_sha256_hex(content.as_bytes())))
}

fn parse_repository_recorded_inputs(content: &str) -> Vec<String> {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn write_repository_recorded_inputs(repo_dir: &Path, inputs: &[String]) -> slug_error::Result<()> {
    let manifest_path = repo_dir.join(REPO_RECORDED_INPUTS_FILE);
    if inputs.is_empty() {
        if let Err(e) = std::fs::remove_file(&manifest_path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(RepositoryExecutionError::WorkingDirFailed {
                reason: format!(
                    "Failed to remove recorded input manifest '{}': {}",
                    manifest_path.display(),
                    e
                ),
            }
            .into());
        }
        return Ok(());
    }

    let mut stable_inputs = inputs.to_vec();
    stable_inputs.sort();
    stable_inputs.dedup();
    let mut content = stable_inputs.join("\n");
    content.push('\n');
    std::fs::write(&manifest_path, content).map_err(|e| {
        RepositoryExecutionError::WorkingDirFailed {
            reason: format!(
                "Failed to write recorded input manifest '{}': {}",
                manifest_path.display(),
                e
            ),
        }
    })?;
    Ok(())
}

fn write_repository_rule_local_state(repo_dir: &Path, local: bool) -> slug_error::Result<()> {
    let marker_path = repo_dir.join(REPO_RULE_LOCAL_FILE);
    if local {
        std::fs::write(&marker_path, "local\n").map_err(|e| {
            RepositoryExecutionError::WorkingDirFailed {
                reason: format!(
                    "Failed to write repository local marker '{}': {}",
                    marker_path.display(),
                    e
                ),
            }
        })?;
    } else if let Err(e) = std::fs::remove_file(&marker_path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        return Err(RepositoryExecutionError::WorkingDirFailed {
            reason: format!(
                "Failed to remove repository local marker '{}': {}",
                marker_path.display(),
                e
            ),
        }
        .into());
    }
    Ok(())
}

#[cfg(test)]
pub fn repo_materialization_manifest(
    canonical_name: &str,
    repo_spec: &RepoSpec,
    project_root: &PathBuf,
) -> RepoMaterializationManifestValue {
    let key = RepoMaterializationManifestKey::for_project_root(
        project_root.clone(),
        canonical_name,
        Arc::new(repo_spec.clone()),
    );
    repo_materialization_manifest_for_key(&key)
}

#[cfg(test)]
fn repo_materialization_manifest_for_key(
    key: &RepoMaterializationManifestKey,
) -> RepoMaterializationManifestValue {
    let marker_state = repo_materialization_marker_state_for_key(key);
    let layout_state = repo_materialization_layout_state_for_key(key);
    let recorded_inputs_state = repo_materialization_recorded_inputs_state_for_key(key);
    repo_materialization_manifest_from_states(
        key,
        Arc::from(marker_state.as_str()),
        Arc::from(layout_state.as_str()),
        Arc::from(recorded_inputs_state.as_str()),
    )
}

#[cfg(test)]
fn repo_materialization_recorded_inputs_state_for_key(
    key: &RepoMaterializationManifestKey,
) -> String {
    let repo_dir = repo_dir_for_materialization_manifest_key(key);
    match repository_recorded_inputs_digest(
        &repo_dir,
        Some(key.repo_env.as_ref()),
        Some(key.repo_mappings.as_ref()),
    ) {
        Ok(Some(digest)) => format!("inputs:{digest}:valid"),
        Ok(None) => "inputs:none".to_owned(),
        Err(reason) => format!("inputs-invalid:{reason}"),
    }
}

fn repo_dir_for_materialization_manifest_key(key: &RepoMaterializationManifestKey) -> PathBuf {
    let canonical_name = key.canonical_repo.as_ref();
    key.workspace_id
        .canonical_project_root
        .join("bazel-external")
        .join(canonical_name)
}

#[cfg(test)]
fn repo_materialization_marker_state_for_key(key: &RepoMaterializationManifestKey) -> String {
    let content_state = repo_materialization_marker_content_state_for_key(key);
    repo_materialization_marker_state_from_content_state(
        key.repo_spec_digest.as_ref(),
        &repo_dir_for_materialization_manifest_key(key),
        &content_state,
    )
}

#[cfg(test)]
fn repo_materialization_marker_content_state_for_key(
    key: &RepoMaterializationManifestKey,
) -> String {
    let repo_spec = key.repo_spec.as_ref();
    let repo_dir = repo_dir_for_materialization_manifest_key(key);
    repo_materialization_marker_content_state(
        repo_spec.local,
        repo_materialization_rule_local_state(&repo_dir),
        &repo_dir,
    )
}

#[cfg(test)]
fn repo_materialization_marker_content_state(
    repo_spec_local: bool,
    repo_rule_local: bool,
    repo_dir: &Path,
) -> String {
    let marker_path = repo_dir.join(".slug_repo_complete");
    if repo_spec_local || repo_rule_local {
        "local-rule".to_owned()
    } else if marker_path.exists() {
        match std::fs::read_to_string(&marker_path) {
            Ok(marker) => {
                let trimmed = marker.trim();
                format!("{MARKER_CONTENT_PREFIX}{trimmed}")
            }
            Err(e) => format!("marker-unreadable:{e}"),
        }
    } else {
        "marker-absent".to_owned()
    }
}

#[cfg(test)]
fn repo_materialization_rule_local_state(repo_dir: &Path) -> bool {
    repo_dir.join(REPO_RULE_LOCAL_FILE).exists()
}

#[cfg(test)]
fn repo_materialization_marker_state_from_content_state(
    spec_hash: &str,
    repo_dir: &Path,
    content_state: &str,
) -> String {
    let Some(marker) = content_state.strip_prefix(MARKER_CONTENT_PREFIX) else {
        return content_state.to_owned();
    };
    complete_marker_state(marker, spec_hash, repo_dir)
}

#[cfg(test)]
fn repo_materialization_layout_state_for_key(key: &RepoMaterializationManifestKey) -> String {
    let canonical_name = key.canonical_repo.as_ref();
    let repo_spec = key.repo_spec.as_ref();
    let repo_dir = repo_dir_for_materialization_manifest_key(key);
    if repo_spec_requires_build_file(repo_spec)
        && !repo_materialization_build_file_present(&repo_dir)
    {
        return "layout-missing-build-file".to_owned();
    }
    if repo_has_invalid_empty_target_label(&repo_dir) {
        return "layout-invalid-empty-target-label".to_owned();
    }
    if repo_has_foreign_top_level_symlink(
        &repo_dir,
        key.workspace_id.canonical_project_root.as_ref(),
    ) {
        return "layout-foreign-top-level-symlink".to_owned();
    }
    repo_materialization_invocation_layout_state(canonical_name, repo_spec, &repo_dir)
}

#[cfg(test)]
fn repo_materialization_build_file_present(repo_dir: &Path) -> bool {
    repo_dir.join("BUILD.bazel").exists() || repo_dir.join("BUILD").exists()
}

#[cfg(test)]
fn repo_materialization_invocation_layout_state(
    canonical_name: &str,
    repo_spec: &RepoSpec,
    repo_dir: &Path,
) -> String {
    match repo_spec_to_invocation(canonical_name, repo_spec) {
        Ok(invocation) => match crate::repository_executor::repo_layout_is_valid_for_invocation(
            &invocation,
            repo_dir,
        ) {
            true => "layout-valid".to_owned(),
            false => "layout-invalid".to_owned(),
        },
        Err(e) => format!("layout-unclassifiable:{e}"),
    }
}

fn repo_materialization_invocation_layout_state_value(valid: bool) -> Arc<str> {
    match valid {
        true => Arc::from("layout-valid"),
        false => Arc::from("layout-invalid"),
    }
}

#[cfg(test)]
fn repo_materialization_invocation_layout_state_fallback(
    canonical_name: &str,
    repo_spec: &RepoSpec,
    repo_dir: &Path,
) -> Arc<str> {
    Arc::from(repo_materialization_invocation_layout_state(
        canonical_name,
        repo_spec,
        repo_dir,
    ))
}

#[cfg(test)]
fn repo_has_invalid_empty_target_label(repo_path: &Path) -> bool {
    ["BUILD.bazel", "BUILD"].into_iter().any(|name| {
        std::fs::read_to_string(repo_path.join(name))
            .ok()
            .is_some_and(|content| build_file_has_invalid_empty_target_label(&content))
    })
}

fn build_file_has_invalid_empty_target_label(content: &str) -> bool {
    content.contains("//:\"") || content.contains("//:'")
}

#[cfg(test)]
fn repo_has_foreign_top_level_symlink(repo_path: &Path, project_root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(repo_path) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            return false;
        };
        if !metadata.file_type().is_symlink() {
            return false;
        }
        let Ok(target) = std::fs::read_link(path) else {
            return false;
        };
        target.is_absolute() && !target.starts_with(project_root)
    })
}

fn repo_spec_requires_build_file(repo_spec: &RepoSpec) -> bool {
    repo_spec
        .attributes
        .get("build_file")
        .is_some_and(attr_value_is_present)
        || repo_spec
            .attributes
            .get("build_file_content")
            .is_some_and(attr_value_is_present)
}

fn attr_value_is_present(value: &AttrValue) -> bool {
    match value {
        AttrValue::None => false,
        AttrValue::String(s) | AttrValue::Label(s) => !s.is_empty(),
        AttrValue::StringList(items) => !items.is_empty(),
        AttrValue::Dict(entries) => !entries.is_empty(),
        AttrValue::Int(_) | AttrValue::Bool(_) => true,
    }
}

fn repo_materialization_manifest_from_states(
    key: &RepoMaterializationManifestKey,
    marker_state: Arc<str>,
    layout_state: Arc<str>,
    recorded_inputs_state: Arc<str>,
) -> RepoMaterializationManifestValue {
    RepoMaterializationManifestValue::new(
        key.clone(),
        repo_dir_for_materialization_manifest_key(key),
        marker_state.as_ref().to_owned(),
        layout_state.as_ref().to_owned(),
        recorded_inputs_state.as_ref().to_owned(),
    )
}

#[derive(Clone, Debug, Display, Eq, Allocative)]
#[display("RepoMaterializationMarkerStateKey({})", _0)]
struct RepoMaterializationMarkerStateKey(RepoMaterializationManifestKey);

impl PartialEq for RepoMaterializationMarkerStateKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl std::hash::Hash for RepoMaterializationMarkerStateKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[async_trait]
impl Key for RepoMaterializationMarkerStateKey {
    type Value = Arc<str>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let repo_dir = repo_dir_for_materialization_manifest_key(&self.0);
        let repo_rule_local = ctx
            .compute(&RepoMaterializationRuleLocalStateKey {
                workspace_id: self.0.workspace_id.clone(),
                repo_dir: Arc::new(repo_dir.clone()),
            })
            .await
            .unwrap_or(false);
        let content_key = RepoMaterializationMarkerContentKey {
            workspace_id: self.0.workspace_id.clone(),
            repo_spec_local: self.0.repo_spec.local,
            repo_rule_local,
            repo_dir: Arc::new(repo_dir.clone()),
        };
        let content_state = match ctx.compute(&content_key).await {
            Ok(content_state) => content_state,
            Err(e) => return Arc::from(format!("marker-unreadable:{e}").as_str()),
        };
        let Some(marker) = content_state.strip_prefix(MARKER_CONTENT_PREFIX) else {
            return content_state;
        };
        let spec_hash = self.0.repo_spec_digest.as_ref();
        if !complete_marker_matches(marker, spec_hash) {
            return Arc::from(format!("marker-mismatch:{marker}").as_str());
        }
        let Some(expected_output_digest) = complete_marker_expected_output(marker, spec_hash)
        else {
            return Arc::from(format!("marker:{marker}").as_str());
        };
        let output_digest_key = RepoMaterializationOutputDigestKey {
            workspace_id: self.0.workspace_id.clone(),
            repo_dir: Arc::new(repo_dir),
        };
        match ctx.compute(&output_digest_key).await {
            Ok(Ok(current_output_digest))
                if current_output_digest.as_ref() == expected_output_digest =>
            {
                Arc::from(format!("marker:{marker}").as_str())
            }
            Ok(Ok(current_output_digest)) => Arc::from(
                format!("marker-output-mismatch:{marker}:current:{current_output_digest}").as_str(),
            ),
            Ok(Err(e)) => Arc::from(format!("marker-output-unreadable:{marker}:{e}").as_str()),
            Err(e) => Arc::from(format!("marker-output-unreadable:{marker}:{e}").as_str()),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // This repo-state read still polls disk because slug_bzlmod cannot use
        // slug_common's project file watcher without creating a crate cycle.
        // Keeping it as a child key lets unchanged state cut off the parent
        // manifest while changed state invalidates repository execution.
        false
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "RepoMaterializationRuleLocalStateKey({}, {})",
    workspace_id.stable_hash(),
    repo_dir.display()
)]
struct RepoMaterializationRuleLocalStateKey {
    workspace_id: crate::WorkspaceId,
    repo_dir: Arc<PathBuf>,
}

#[async_trait]
impl Key for RepoMaterializationRuleLocalStateKey {
    type Value = bool;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if let Ok(reader) = REPOSITORY_MATERIALIZATION_STATE_READER_IMPL.get() {
            return reader
                .repo_state_file_exists(
                    ctx,
                    self.workspace_id.clone(),
                    self.repo_dir.clone(),
                    REPO_RULE_LOCAL_FILE,
                )
                .await
                .unwrap_or(true);
        }

        #[cfg(test)]
        {
            repo_materialization_rule_local_state(&self.repo_dir)
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // See RepoMaterializationMarkerStateKey::validity.
        false
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "RepoMaterializationMarkerContentKey({}, {}, {}, {})",
    workspace_id.stable_hash(),
    repo_spec_local,
    repo_rule_local,
    repo_dir.display()
)]
struct RepoMaterializationMarkerContentKey {
    workspace_id: crate::WorkspaceId,
    repo_spec_local: bool,
    repo_rule_local: bool,
    repo_dir: Arc<PathBuf>,
}

#[async_trait]
impl Key for RepoMaterializationMarkerContentKey {
    type Value = Arc<str>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if self.repo_spec_local || self.repo_rule_local {
            return Arc::from("local-rule");
        }

        if let Ok(reader) = REPOSITORY_MATERIALIZATION_STATE_READER_IMPL.get() {
            return match reader
                .read_repo_state_file_if_exists(
                    ctx,
                    self.workspace_id.clone(),
                    self.repo_dir.clone(),
                    REPO_COMPLETE_MARKER_FILE,
                )
                .await
            {
                Ok(Some(marker)) => {
                    Arc::from(format!("{MARKER_CONTENT_PREFIX}{}", marker.trim()).as_str())
                }
                Ok(None) => Arc::from("marker-absent"),
                Err(reason) => Arc::from(format!("marker-unreadable:{reason}").as_str()),
            };
        }

        #[cfg(test)]
        {
            Arc::from(
                repo_materialization_marker_content_state(false, false, &self.repo_dir).as_str(),
            )
        }
        #[cfg(not(test))]
        {
            Arc::from("marker-unreadable:repository materialization state reader unavailable")
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // See RepoMaterializationMarkerStateKey::validity.
        false
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "RepoMaterializationOutputDigestKey({}, {})",
    workspace_id.stable_hash(),
    repo_dir.display()
)]
struct RepoMaterializationOutputDigestKey {
    workspace_id: crate::WorkspaceId,
    repo_dir: Arc<PathBuf>,
}

#[async_trait]
impl Key for RepoMaterializationOutputDigestKey {
    type Value = Result<Arc<str>, Arc<str>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if let Ok(reader) = REPOSITORY_MATERIALIZATION_STATE_READER_IMPL.get() {
            return reader
                .repo_output_digest(ctx, self.workspace_id.clone(), self.repo_dir.clone())
                .await;
        }

        #[cfg(test)]
        {
            crate::repository_executor::repository_output_digest(&self.repo_dir)
                .map(|digest| Arc::from(digest.as_str()))
                .map_err(|e| {
                    let reason = e.to_string();
                    Arc::from(reason.as_str())
                })
        }
        #[cfg(not(test))]
        {
            Err(Arc::from(
                "repository materialization state reader unavailable",
            ))
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // See RepoMaterializationMarkerStateKey::validity.
        false
    }
}

#[derive(Clone, Debug, Display, Eq, Allocative)]
#[display("RepoMaterializationLayoutStateKey({})", _0)]
struct RepoMaterializationLayoutStateKey(RepoMaterializationManifestKey);

impl PartialEq for RepoMaterializationLayoutStateKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl std::hash::Hash for RepoMaterializationLayoutStateKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[async_trait]
impl Key for RepoMaterializationLayoutStateKey {
    type Value = Arc<str>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let repo_spec = self.0.repo_spec.as_ref();
        let repo_dir = repo_dir_for_materialization_manifest_key(&self.0);
        if repo_spec_requires_build_file(repo_spec) {
            let build_file_present = ctx
                .compute(&RepoMaterializationBuildFilePresenceKey {
                    workspace_id: self.0.workspace_id.clone(),
                    repo_dir: Arc::new(repo_dir.clone()),
                })
                .await
                .unwrap_or(false);
            if !build_file_present {
                return Arc::from("layout-missing-build-file");
            }
        }

        let invalid_empty_target_label = ctx
            .compute(&RepoMaterializationInvalidEmptyTargetLabelKey {
                workspace_id: self.0.workspace_id.clone(),
                repo_dir: Arc::new(repo_dir.clone()),
            })
            .await
            .unwrap_or(false);
        if invalid_empty_target_label {
            return Arc::from("layout-invalid-empty-target-label");
        }

        let foreign_top_level_symlink = ctx
            .compute(&RepoMaterializationForeignTopLevelSymlinkKey {
                workspace_id: self.0.workspace_id.clone(),
                repo_dir: Arc::new(repo_dir.clone()),
                project_root: self.0.workspace_id.canonical_project_root.clone(),
            })
            .await
            .unwrap_or(false);
        if foreign_top_level_symlink {
            return Arc::from("layout-foreign-top-level-symlink");
        }

        ctx.compute(&RepoMaterializationInvocationLayoutStateKey(self.0.clone()))
            .await
            .unwrap_or_else(|e| Arc::from(format!("layout-unclassifiable:{e}").as_str()))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // See RepoMaterializationMarkerStateKey::validity.
        false
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "RepoMaterializationBuildFilePresenceKey({}, {})",
    workspace_id.stable_hash(),
    repo_dir.display()
)]
struct RepoMaterializationBuildFilePresenceKey {
    workspace_id: crate::WorkspaceId,
    repo_dir: Arc<PathBuf>,
}

#[async_trait]
impl Key for RepoMaterializationBuildFilePresenceKey {
    type Value = bool;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if let Ok(reader) = REPOSITORY_MATERIALIZATION_STATE_READER_IMPL.get() {
            if reader
                .repo_state_file_exists(
                    ctx,
                    self.workspace_id.clone(),
                    self.repo_dir.clone(),
                    "BUILD.bazel",
                )
                .await
                .unwrap_or(false)
            {
                return true;
            }
            return reader
                .repo_state_file_exists(
                    ctx,
                    self.workspace_id.clone(),
                    self.repo_dir.clone(),
                    "BUILD",
                )
                .await
                .unwrap_or(false);
        }

        #[cfg(test)]
        {
            repo_materialization_build_file_present(&self.repo_dir)
        }
        #[cfg(not(test))]
        {
            false
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // See RepoMaterializationMarkerStateKey::validity.
        false
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "RepoMaterializationInvalidEmptyTargetLabelKey({}, {})",
    workspace_id.stable_hash(),
    repo_dir.display()
)]
struct RepoMaterializationInvalidEmptyTargetLabelKey {
    workspace_id: crate::WorkspaceId,
    repo_dir: Arc<PathBuf>,
}

#[async_trait]
impl Key for RepoMaterializationInvalidEmptyTargetLabelKey {
    type Value = bool;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if let Ok(reader) = REPOSITORY_MATERIALIZATION_STATE_READER_IMPL.get() {
            for name in ["BUILD.bazel", "BUILD"] {
                let content = reader
                    .read_repo_state_file_if_exists(
                        ctx,
                        self.workspace_id.clone(),
                        self.repo_dir.clone(),
                        name,
                    )
                    .await
                    .ok()
                    .flatten();
                if content
                    .as_ref()
                    .is_some_and(|content| build_file_has_invalid_empty_target_label(content))
                {
                    return true;
                }
            }
            return false;
        }

        #[cfg(test)]
        {
            repo_has_invalid_empty_target_label(&self.repo_dir)
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // See RepoMaterializationMarkerStateKey::validity.
        false
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "RepoMaterializationForeignTopLevelSymlinkKey({}, {}, {})",
    workspace_id.stable_hash(),
    repo_dir.display(),
    project_root.display()
)]
struct RepoMaterializationForeignTopLevelSymlinkKey {
    workspace_id: crate::WorkspaceId,
    repo_dir: Arc<PathBuf>,
    project_root: Arc<PathBuf>,
}

#[async_trait]
impl Key for RepoMaterializationForeignTopLevelSymlinkKey {
    type Value = bool;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if let Ok(reader) = REPOSITORY_MATERIALIZATION_STATE_READER_IMPL.get() {
            return reader
                .repo_has_foreign_top_level_symlink(
                    ctx,
                    self.workspace_id.clone(),
                    self.repo_dir.clone(),
                )
                .await
                .unwrap_or(false);
        }

        #[cfg(test)]
        {
            repo_has_foreign_top_level_symlink(&self.repo_dir, &self.project_root)
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // See RepoMaterializationMarkerStateKey::validity.
        false
    }
}

async fn repo_materialization_source_entries_match_links(
    ctx: &mut DiceComputations<'_>,
    workspace_id: crate::WorkspaceId,
    source_dir: &Path,
    repo_dir: &Path,
    reader: &dyn RepositoryMaterializationStateReader,
) -> Result<bool, Arc<str>> {
    let entries = reader
        .repo_dir_entry_names(
            ctx,
            workspace_id.clone(),
            Arc::new(source_dir.to_path_buf()),
        )
        .await?;

    let mut checked_any = false;
    for name in entries.iter() {
        if crate::repository_executor::should_skip_local_repository_entry(name) {
            continue;
        }
        checked_any = true;
        if !reader
            .repo_symlink_points_to(
                ctx,
                workspace_id.clone(),
                Arc::new(repo_dir.join(name)),
                Arc::new(source_dir.join(name)),
            )
            .await?
        {
            return Ok(false);
        }
    }

    Ok(checked_any)
}

async fn repo_materialization_new_local_build_file_is_valid(
    ctx: &mut DiceComputations<'_>,
    workspace_id: crate::WorkspaceId,
    repo_dir: &Path,
    invocation: &RepositoryInvocation,
    reader: &dyn RepositoryMaterializationStateReader,
) -> Result<bool, Arc<str>> {
    let Some(expected) = invocation
        .attrs
        .get("build_file_content")
        .and_then(|attr| attr.as_string())
    else {
        return Ok(true);
    };

    for name in ["BUILD.bazel", "BUILD"] {
        let actual = reader
            .read_repo_state_file_if_exists(
                ctx,
                workspace_id.clone(),
                Arc::new(repo_dir.to_path_buf()),
                name,
            )
            .await?;
        if actual
            .as_ref()
            .is_some_and(|actual| actual.as_ref() == expected)
        {
            return Ok(true);
        }
    }

    Ok(false)
}

async fn repo_materialization_local_repository_layout_is_valid(
    ctx: &mut DiceComputations<'_>,
    workspace_id: crate::WorkspaceId,
    invocation: &RepositoryInvocation,
    repo_dir: &Path,
    reader: &dyn RepositoryMaterializationStateReader,
) -> Result<bool, Arc<str>> {
    let Some(source_dir) =
        crate::repository_executor::local_repository_source_path(invocation, repo_dir)
    else {
        return Ok(true);
    };
    reader
        .repo_symlink_points_to(
            ctx,
            workspace_id,
            Arc::new(repo_dir.to_path_buf()),
            Arc::new(source_dir),
        )
        .await
}

async fn repo_materialization_new_local_repository_layout_is_valid(
    ctx: &mut DiceComputations<'_>,
    workspace_id: crate::WorkspaceId,
    invocation: &RepositoryInvocation,
    repo_dir: &Path,
    reader: &dyn RepositoryMaterializationStateReader,
) -> Result<bool, Arc<str>> {
    if !repo_materialization_new_local_build_file_is_valid(
        ctx,
        workspace_id.clone(),
        repo_dir,
        invocation,
        reader,
    )
    .await?
    {
        return Ok(false);
    }

    let Some(source_dir) =
        crate::repository_executor::local_repository_source_path(invocation, repo_dir)
    else {
        return Ok(true);
    };

    if repo_materialization_source_entries_match_links(
        ctx,
        workspace_id.clone(),
        &source_dir,
        repo_dir,
        reader,
    )
    .await?
    {
        return Ok(true);
    }

    if reader
        .repo_state_file_exists(
            ctx,
            workspace_id.clone(),
            Arc::new(repo_dir.to_path_buf()),
            "BUILD.bazel",
        )
        .await?
    {
        return Ok(true);
    }
    reader
        .repo_state_file_exists(ctx, workspace_id, Arc::new(repo_dir.to_path_buf()), "BUILD")
        .await
}

fn llvm_subproject_source_path(
    invocation: &RepositoryInvocation,
    repo_dir: &Path,
) -> Option<PathBuf> {
    let dir = invocation
        .attrs
        .get("dir")
        .and_then(|attr| attr.as_string())?;
    if dir.is_empty() {
        return None;
    }
    let project_root = repo_dir.parent().and_then(|external| external.parent())?;
    let prefix = invocation.name.rsplit_once('+').map(|(prefix, _)| prefix)?;
    Some(
        project_root
            .join("bazel-external")
            .join(format!("{prefix}+llvm-raw"))
            .join(dir),
    )
}

async fn repo_materialization_llvm_subproject_layout_is_valid(
    ctx: &mut DiceComputations<'_>,
    workspace_id: crate::WorkspaceId,
    invocation: &RepositoryInvocation,
    repo_dir: &Path,
    reader: &dyn RepositoryMaterializationStateReader,
) -> Result<bool, Arc<str>> {
    let Some(source_dir) = llvm_subproject_source_path(invocation, repo_dir) else {
        return Ok(true);
    };
    if !repo_materialization_source_entries_match_links(
        ctx,
        workspace_id.clone(),
        &source_dir,
        repo_dir,
        reader,
    )
    .await?
    {
        return Ok(false);
    }
    reader
        .repo_state_file_exists(
            ctx,
            workspace_id,
            Arc::new(repo_dir.to_path_buf()),
            "BUILD.bazel",
        )
        .await
}

#[derive(Clone, Debug, Display, Eq, Allocative)]
#[display("RepoMaterializationInvocationLayoutStateKey({})", _0)]
struct RepoMaterializationInvocationLayoutStateKey(RepoMaterializationManifestKey);

impl PartialEq for RepoMaterializationInvocationLayoutStateKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl std::hash::Hash for RepoMaterializationInvocationLayoutStateKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[async_trait]
impl Key for RepoMaterializationInvocationLayoutStateKey {
    type Value = Arc<str>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let canonical_name = self.0.canonical_repo.as_ref();
        let repo_spec = self.0.repo_spec.as_ref();
        let repo_dir = repo_dir_for_materialization_manifest_key(&self.0);
        match repo_spec_to_invocation(canonical_name, repo_spec) {
            Ok(invocation) => {
                let Ok(reader) = REPOSITORY_MATERIALIZATION_STATE_READER_IMPL.get() else {
                    #[cfg(test)]
                    {
                        return repo_materialization_invocation_layout_state_fallback(
                            canonical_name,
                            repo_spec,
                            &repo_dir,
                        );
                    }
                    #[cfg(not(test))]
                    {
                        return Arc::from(
                            "layout-invalid:repository-materialization-state-reader-unavailable",
                        );
                    }
                };
                let workspace_id = self.0.workspace_id.clone();
                let layout_valid = match invocation.rule_name.as_str() {
                    "git_repository" | "new_git_repository" => {
                        reader
                            .repo_state_file_exists(
                                ctx,
                                workspace_id,
                                Arc::new(repo_dir.clone()),
                                ".git",
                            )
                            .await
                    }
                    "local_repository" => {
                        repo_materialization_local_repository_layout_is_valid(
                            ctx,
                            workspace_id,
                            &invocation,
                            &repo_dir,
                            *reader,
                        )
                        .await
                    }
                    "new_local_repository" => {
                        repo_materialization_new_local_repository_layout_is_valid(
                            ctx,
                            workspace_id,
                            &invocation,
                            &repo_dir,
                            *reader,
                        )
                        .await
                    }
                    "_llvm_subproject_repository" => {
                        repo_materialization_llvm_subproject_layout_is_valid(
                            ctx,
                            workspace_id,
                            &invocation,
                            &repo_dir,
                            *reader,
                        )
                        .await
                    }
                    _ => Ok(
                        crate::repository_executor::repo_layout_is_valid_for_invocation(
                            &invocation,
                            &repo_dir,
                        ),
                    ),
                }
                .unwrap_or_else(|_| {
                    #[cfg(test)]
                    {
                        crate::repository_executor::repo_layout_is_valid_for_invocation(
                            &invocation,
                            &repo_dir,
                        )
                    }
                    #[cfg(not(test))]
                    {
                        false
                    }
                });
                repo_materialization_invocation_layout_state_value(layout_valid)
            }
            Err(e) => Arc::from(format!("layout-unclassifiable:{e}").as_str()),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // See RepoMaterializationMarkerStateKey::validity.
        false
    }
}

#[derive(Clone, Debug, Display, Eq, Allocative)]
#[display("RepoMaterializationRecordedInputsStateKey({})", _0)]
struct RepoMaterializationRecordedInputsStateKey(RepoMaterializationManifestKey);

impl PartialEq for RepoMaterializationRecordedInputsStateKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl std::hash::Hash for RepoMaterializationRecordedInputsStateKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[async_trait]
impl Key for RepoMaterializationRecordedInputsStateKey {
    type Value = Arc<str>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let repo_dir = repo_dir_for_materialization_manifest_key(&self.0);
        let content_key = RepoMaterializationRecordedInputsManifestContentKey {
            workspace_id: self.0.workspace_id.clone(),
            repo_dir: Arc::new(repo_dir),
        };
        let manifest_content = match ctx.compute(&content_key).await {
            Ok(Ok(Some(content))) => content,
            Ok(Ok(None)) => return Arc::from("inputs:none"),
            Ok(Err(reason)) => return Arc::from(format!("inputs-invalid:{reason}").as_str()),
            Err(e) => return Arc::from(format!("inputs-invalid:{e}").as_str()),
        };
        let recorded_inputs = Arc::new(parse_repository_recorded_inputs(&manifest_content));
        let validation_key = RepoMaterializationRecordedInputsValidationKey {
            recorded_inputs,
            repo_env: self.0.repo_env.clone(),
            repo_mappings: self.0.repo_mappings.clone(),
        };
        match ctx.compute(&validation_key).await {
            Ok(Ok(())) => Arc::from(
                format!(
                    "inputs:{}:valid",
                    compute_sha256_hex(manifest_content.as_bytes())
                )
                .as_str(),
            ),
            Ok(Err(reason)) => Arc::from(format!("inputs-invalid:{reason}").as_str()),
            Err(e) => Arc::from(format!("inputs-invalid:{e}").as_str()),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // See RepoMaterializationMarkerStateKey::validity.
        false
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "RepoMaterializationRecordedInputsManifestContentKey({}, {})",
    workspace_id.stable_hash(),
    repo_dir.display()
)]
struct RepoMaterializationRecordedInputsManifestContentKey {
    workspace_id: crate::WorkspaceId,
    repo_dir: Arc<PathBuf>,
}

#[async_trait]
impl Key for RepoMaterializationRecordedInputsManifestContentKey {
    type Value = Result<Option<Arc<str>>, Arc<str>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if let Ok(reader) = REPOSITORY_MATERIALIZATION_STATE_READER_IMPL.get() {
            return reader
                .read_repo_state_file_if_exists(
                    ctx,
                    self.workspace_id.clone(),
                    self.repo_dir.clone(),
                    REPO_RECORDED_INPUTS_FILE,
                )
                .await
                .map_err(|_| Arc::from("recorded_inputs_unreadable"));
        }

        #[cfg(test)]
        {
            let manifest_path = self.repo_dir.join(REPO_RECORDED_INPUTS_FILE);
            if !manifest_path.exists() {
                return Ok(None);
            }
            std::fs::read_to_string(&manifest_path)
                .map(|content| Some(Arc::from(content.as_str())))
                .map_err(|_| Arc::from("recorded_inputs_unreadable"))
        }
        #[cfg(not(test))]
        {
            Err(Arc::from("recorded_inputs_reader_unavailable"))
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // See RepoMaterializationMarkerStateKey::validity.
        false
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative, Dupe)]
#[display(
    "RepoMaterializationRecordedInputsValidationKey({})",
    recorded_inputs.len()
)]
struct RepoMaterializationRecordedInputsValidationKey {
    recorded_inputs: Arc<Vec<String>>,
    repo_env: Arc<BTreeMap<String, String>>,
    repo_mappings: Arc<crate::RepoMappingSnapshot>,
}

#[async_trait]
impl Key for RepoMaterializationRecordedInputsValidationKey {
    type Value = Result<(), Arc<str>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        validate_recorded_inputs_current(
            self.recorded_inputs.as_slice(),
            None,
            Some(self.repo_env.as_ref()),
            Some(self.repo_mappings.as_ref()),
        )
        .map_err(Arc::from)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        // See RepoMaterializationMarkerStateKey::validity.
        false
    }
}

#[async_trait]
impl Key for RepoMaterializationManifestKey {
    type Value = slug_error::Result<Arc<RepoMaterializationManifestValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let marker_state = ctx
            .compute(&RepoMaterializationMarkerStateKey(self.clone()))
            .await?;
        let layout_state = ctx
            .compute(&RepoMaterializationLayoutStateKey(self.clone()))
            .await?;
        let recorded_inputs_state = ctx
            .compute(&RepoMaterializationRecordedInputsStateKey(self.clone()))
            .await?;
        Ok(Arc::new(repo_materialization_manifest_from_states(
            self,
            marker_state,
            layout_state,
            recorded_inputs_state,
        )))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.digest == y.digest && x.key == y.key,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
}

#[async_trait]
impl Key for ExtensionRepoExecutionKey {
    type Value = slug_error::Result<Arc<RepositoryRuleResult>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        tracing::info!(
            "Executing repository '{}' from extension '{}' (rule: '{}')",
            self.canonical_name,
            self.extension_id,
            self.repo_spec.repo_rule_id
        );

        // Convert RepoSpec to RepositoryInvocation
        let invocation = repo_spec_to_invocation(&self.canonical_name, &self.repo_spec)?;

        let working_dir = self
            .project_root
            .join("bazel-external")
            .join(self.canonical_name.as_ref());

        let manifest = ctx
            .compute(self.materialization_manifest_key.as_ref())
            .await??;
        let marker_matches =
            !self.repo_spec.local && manifest.marker_state.starts_with("marker:complete:");
        let layout_valid = manifest.layout_state.as_ref() == "layout-valid";
        let recorded_inputs_current = !manifest
            .recorded_inputs_state
            .starts_with("inputs-invalid:");
        if marker_matches && layout_valid && recorded_inputs_current {
            record_bzlmod_event(
                BzlmodEventKind::RepoMaterializationHit,
                self.canonical_name.as_ref(),
            );
            return Ok(Arc::new(RepositoryRuleResult::success(
                self.canonical_name.to_string(),
                working_dir,
            )));
        }

        let miss_reason = if self.repo_spec.local {
            "local_rule"
        } else if marker_matches && layout_valid && !recorded_inputs_current {
            "recorded_inputs_changed"
        } else if marker_matches && !layout_valid {
            "marker_layout_invalid"
        } else if manifest.marker_state.starts_with("marker-mismatch:") {
            "marker_digest_mismatch"
        } else if manifest.marker_state.starts_with("marker-output-mismatch:") {
            "marker_output_mismatch"
        } else if manifest
            .marker_state
            .starts_with("marker-output-unreadable:")
        {
            "marker_output_unreadable"
        } else if manifest.marker_state.starts_with("marker-unreadable:") {
            "marker_unreadable"
        } else {
            "marker_absent"
        };
        record_bzlmod_event(
            BzlmodEventKind::RepoMaterializationMissReason,
            format!("{}:{miss_reason}", self.canonical_name),
        );

        if working_dir.exists() {
            tracing::debug!(
                "Removing incomplete repository rule working dir for '{}': {:?}",
                self.canonical_name,
                working_dir
            );
            std::fs::remove_dir_all(&working_dir).map_err(|e| {
                RepositoryExecutionError::WorkingDirFailed {
                    reason: format!(
                        "Failed to remove incomplete repository directory {:?}: {}",
                        working_dir, e
                    ),
                }
            })?;
        }

        // For non-builtin rules with a known Starlark source, try Starlark execution
        let is_builtin =
            crate::starlark_repo_rule_executor::is_builtin_repo_rule(&invocation.rule_name);
        if !is_builtin {
            if let Some(rule_source) = &invocation.rule_source {
                // Extract bzl_path and rule_name from rule_source
                // Format: "@@module//path:file.bzl%rule_name"
                if let Some(percent_pos) = rule_source.rfind('%') {
                    let rule_bzl_path = &rule_source[..percent_pos];
                    let rule_fn_name = &rule_source[percent_pos + 1..];

                    if let Ok(executor) =
                        crate::starlark_repo_rule_executor::STARLARK_REPO_RULE_EXECUTOR_IMPL.get()
                    {
                        tracing::info!(
                            "Executing Starlark repo rule '{}' from '{}' for '{}'",
                            rule_fn_name,
                            rule_bzl_path,
                            self.canonical_name
                        );

                        // Prepare working directory
                        if !working_dir.exists() {
                            std::fs::create_dir_all(&working_dir).map_err(|e| {
                                RepositoryExecutionError::WorkingDirFailed {
                                    reason: format!("Failed to create directory: {}", e),
                                }
                            })?;
                        }

                        match executor
                            .execute_rule(
                                ctx,
                                &invocation,
                                rule_bzl_path,
                                rule_fn_name,
                                &working_dir,
                                self.repo_env.clone(),
                                self.repo_mappings.clone(),
                                self.materialization_manifest_key.workspace_id.clone(),
                            )
                            .await
                        {
                            Ok(execution) => {
                                // Mark as complete and write WORKSPACE if missing
                                if !working_dir.join("WORKSPACE").exists()
                                    && !working_dir.join("WORKSPACE.bazel").exists()
                                {
                                    let _ = std::fs::write(
                                        working_dir.join("WORKSPACE.bazel"),
                                        format!("workspace(name = \"{}\")\n", self.canonical_name),
                                    );
                                }
                                write_repository_recorded_inputs(
                                    &working_dir,
                                    &execution.recorded_inputs,
                                )?;
                                let effective_local = self.repo_spec.local || execution.local;
                                write_repository_rule_local_state(&working_dir, effective_local)?;
                                let output_digest =
                                    crate::repository_executor::repository_output_digest(
                                        &working_dir,
                                    )?;
                                let _ = std::fs::write(
                                    working_dir.join(".slug_repo_complete"),
                                    complete_marker(&self.spec_hash, &output_digest),
                                );
                                let mut result = RepositoryRuleResult::success(
                                    self.canonical_name.to_string(),
                                    working_dir,
                                );
                                if effective_local {
                                    result = result.non_cacheable();
                                }
                                return Ok(Arc::new(result));
                            }
                            Err(e) => {
                                return Err(RepositoryExecutionError::ExecutionFailed {
                                    name: self.canonical_name.to_string(),
                                    reason: format!(
                                        "Starlark repository rule '{}' from '{}' failed: {}",
                                        rule_fn_name, rule_bzl_path, e
                                    ),
                                }
                                .into());
                            }
                        }
                    }
                }
            }
        }

        // Execute the repository rule using the native repository executor
        // This handles http_archive, git_repository, local_repository, etc.
        let label_resolution = ctx
            .compute(&RepositoryLabelResolutionKey::new(
                self.materialization_manifest_key.workspace_id.clone(),
                self.project_root.clone(),
            ))
            .await??;
        let mut result =
            crate::repository_executor::execute_repository_rule_fresh_with_label_resolution(
                &invocation,
                &self.project_root,
                label_resolution.as_ref(),
            )?;
        if self.repo_spec.local {
            result = result.non_cacheable();
        }
        write_repository_rule_local_state(&result.repo_path, self.repo_spec.local)?;
        let output_digest =
            crate::repository_executor::repository_output_digest(&result.repo_path)?;

        std::fs::write(
            result.repo_path.join(".slug_repo_complete"),
            complete_marker(&self.spec_hash, &output_digest),
        )
        .map_err(|e| RepositoryExecutionError::WorkingDirFailed {
            reason: format!(
                "Failed to write spec-hashed completion marker for '{}': {}",
                self.canonical_name, e
            ),
        })?;

        tracing::info!(
            "Successfully materialized repository '{}' at {:?}",
            self.canonical_name,
            result.repo_path
        );

        Ok(Arc::new(result))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        // Don't cache errors - retry on next request. Successful values are
        // guarded by `RepoMaterializationManifestKey`, so marker/layout
        // corruption dirties the DICE dependency while same-transaction
        // materialization stays deduped.
        x.as_ref().is_ok_and(|result| result.cacheable)
    }
}

/// Convert a RepoSpec to a RepositoryInvocation.
///
/// This extracts the rule name from the `repo_rule_id` (format: `@@module//path:file.bzl%rule_name`)
/// and copies attributes from the RepoSpec to create a RepositoryInvocation suitable
/// for execution.
///
/// # Arguments
/// * `canonical_name` - The canonical name for this repository (e.g., "_main+pip+numpy")
/// * `repo_spec` - The captured RepoSpec from extension execution
///
/// # Returns
/// A RepositoryInvocation that can be passed to the repository executor.
pub(crate) fn repo_spec_to_invocation(
    canonical_name: &str,
    repo_spec: &RepoSpec,
) -> slug_error::Result<RepositoryInvocation> {
    // Extract rule name from repo_rule_id
    // Format: "@@module//path:file.bzl%rule_name"
    let rule_name = extract_rule_name_from_id(&repo_spec.repo_rule_id).ok_or_else(|| {
        RepositoryExecutionError::InvalidRepoRuleId {
            repo_rule_id: repo_spec.repo_rule_id.clone(),
        }
    })?;

    let mut invocation = RepositoryInvocation::new(canonical_name.to_owned(), rule_name.to_owned())
        .with_rule_source(repo_spec.repo_rule_id.clone());

    // Copy all attributes from RepoSpec
    for (key, value) in &repo_spec.attributes {
        invocation.attrs.insert(key.clone(), value.clone());
    }

    Ok(invocation)
}

/// Extract the rule name from a repo_rule_id.
///
/// Handles formats:
/// - `@@module//path:file.bzl%rule_name` → `rule_name`
/// - `rule_name` (plain name without bzl path) → `rule_name`
fn extract_rule_name_from_id(repo_rule_id: &str) -> Option<String> {
    if let Some(pos) = repo_rule_id.rfind('%') {
        Some(repo_rule_id[pos + 1..].to_owned())
    } else if !repo_rule_id.is_empty() {
        // Plain rule name (e.g., from DICE-based extension execution
        // where bzl_context wasn't set)
        Some(repo_rule_id.to_owned())
    } else {
        None
    }
}

/// Registry of repository rule invocations for DICE lookup.
///
/// This holds all recorded repository invocations so they can be looked up
/// when a DICE computation needs to execute them.
#[derive(Debug, Default, Clone, Allocative)]
#[cfg(test)]
pub struct RepositoryRegistry {
    /// Map from repository name to invocation.
    invocations: std::collections::HashMap<String, RepositoryInvocation>,
}

#[cfg(test)]
impl RepositoryRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add invocations to the registry.
    pub fn add_invocations(&mut self, invocations: impl IntoIterator<Item = RepositoryInvocation>) {
        for inv in invocations {
            self.invocations.insert(inv.name.clone(), inv);
        }
    }

    /// Get an invocation by repository name.
    pub fn get(&self, name: &str) -> Option<&RepositoryInvocation> {
        self.invocations.get(name)
    }

    /// Check if a repository is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.invocations.contains_key(name)
    }

    /// Get the number of registered repositories.
    pub fn len(&self) -> usize {
        self.invocations.len()
    }
}

/// Helper to get common attributes from a repository invocation.
pub struct InvocationAttrs<'a> {
    invocation: &'a RepositoryInvocation,
}

impl<'a> InvocationAttrs<'a> {
    pub fn new(invocation: &'a RepositoryInvocation) -> Self {
        Self { invocation }
    }

    /// Get a string attribute.
    pub fn get_string(&self, name: &str) -> Option<&str> {
        self.invocation.attrs.get(name).and_then(|v| v.as_string())
    }

    /// Get a required string attribute.
    pub fn require_string(&self, name: &str) -> slug_error::Result<&str> {
        self.get_string(name).ok_or_else(|| {
            RepositoryExecutionError::MissingAttribute {
                name: self.invocation.name.clone(),
                attr: name.to_owned(),
            }
            .into()
        })
    }

    /// Get a string list attribute.
    pub fn get_string_list(&self, name: &str) -> Option<&[String]> {
        self.invocation
            .attrs
            .get(name)
            .and_then(|v| v.as_string_list())
    }

    /// Get a boolean attribute with a default.
    pub fn get_bool(&self, name: &str, default: bool) -> bool {
        self.invocation
            .attrs
            .get(name)
            .and_then(|v| v.as_bool())
            .unwrap_or(default)
    }

    /// Get an optional string attribute.
    pub fn get_optional_string(&self, name: &str) -> Option<&str> {
        self.get_string(name).filter(|s| !s.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SetBzlmodDiceInputs;
    use crate::repo_spec::RepoSpec;
    use crate::repository_invocations::AttrValue;

    #[test]
    fn test_execution_key_from_invocation() {
        let inv = RepositoryInvocation::new("test_repo".to_owned(), "http_archive".to_owned());
        let key = RepositoryRuleExecutionKey::from_invocation(&inv);

        assert_eq!(key.name.as_ref(), "test_repo");
        assert_eq!(key.rule_name.as_ref(), "http_archive");
        assert!(key.attrs_hash.starts_with("sha256-"));
    }

    #[test]
    fn direct_repository_rule_execution_key_fails_instead_of_stubbing() {
        let err = repository_rule_execution_key_unimplemented_error("test_repo", "http_archive");
        assert!(err.to_string().contains("has no implementation"));
        assert!(err.to_string().contains("http_archive"));
    }

    #[test]
    fn test_repository_registry() {
        let mut registry = RepositoryRegistry::new();

        registry.add_invocations([
            RepositoryInvocation::new("foo".to_owned(), "http_archive".to_owned()),
            RepositoryInvocation::new("bar".to_owned(), "git_repository".to_owned()),
        ]);

        assert_eq!(registry.len(), 2);
        assert!(registry.contains("foo"));
        assert!(registry.contains("bar"));
        assert!(!registry.contains("baz"));

        let foo = registry.get("foo").unwrap();
        assert_eq!(foo.rule_name, "http_archive");
    }

    #[tokio::test]
    async fn repository_label_resolution_key_projects_cell_graph_paths() -> slug_error::Result<()> {
        let project_root = PathBuf::from("/tmp/slug-plan61-repo-label-resolution");
        let workspace_id = crate::WorkspaceId::new(
            project_root.clone(),
            project_root.join("buck-out/repo-label-resolution"),
        );
        let cell_graph = crate::BzlmodCellGraphValue {
            workspace_id: workspace_id.clone(),
            root_module_name: "root".to_owned(),
            cells: Arc::new(vec![crate::BzlmodCellGraphCell {
                name: "rules_cc+".to_owned(),
                path: "bazel-external/rules_cc+".to_owned(),
                module_setup: None,
                bundled: false,
            }]),
            extension_cells: Arc::new(vec![crate::BzlmodCellGraphExtensionCell {
                canonical_name: "root+ext+tool".to_owned(),
                internal_name: "tool".to_owned(),
                path: "bazel-external/root+ext+tool".to_owned(),
                extension_id: "@@root//:ext.bzl%ext".to_owned(),
                spec_hash: "tool-hash".to_owned(),
                repo_spec_json: "{}".to_owned(),
                repo_env_json: "{}".to_owned(),
                extension_usages_digest: String::new(),
                extension_replay_inputs_identity_digest: String::new(),
                extension_repo_mappings_digest: String::new(),
                extension_repo_mapping_overrides_digest: String::new(),
                extension_bzl_transitive_digest: String::new(),
                extension_recorded_inputs_json: String::new(),
                materialized: true,
                lazy: false,
            }]),
            root_aliases: Arc::new(vec![crate::BzlmodCellGraphAlias {
                apparent_name: "rules_cc".to_owned(),
                target_name: "rules_cc+".to_owned(),
            }]),
            module_symlinks: Arc::new(Vec::new()),
            scoped_aliases: Arc::new(Vec::new()),
            dynamic_aliases: Arc::new(vec![crate::BzlmodCellGraphDynamicAlias {
                apparent_name: "tool".to_owned(),
                canonical_name: "root+ext+tool".to_owned(),
            }]),
        };
        let expected = RepositoryLabelResolution::from_cell_graph(&project_root, &cell_graph);

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.set_bzlmod_cell_graph_data_with_inputs(
            cell_graph,
            crate::BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                workspace_id.clone(),
                "root".to_owned(),
                Arc::new(Default::default()),
            ),
            crate::BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(crate::BzlmodLockfileInputsValue::default()),
            ),
            crate::BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            ),
            crate::RegisteredToolchainsDataValue::for_workspace(workspace_id.clone(), Vec::new()),
            crate::RegisteredExecutionPlatformsDataValue::for_workspace(
                workspace_id.clone(),
                Vec::new(),
            ),
            crate::BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                workspace_id.clone(),
                "root".to_owned(),
                Arc::new(Default::default()),
            ),
            crate::BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            ),
            crate::BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::from([(
                    String::new(),
                    BTreeMap::from([
                        ("rules_cc".to_owned(), "rules_cc+".to_owned()),
                        ("tool".to_owned(), "root+ext+tool".to_owned()),
                    ]),
                )])),
                Arc::new(Default::default()),
            ),
        )?;
        let mut dice = updater.commit().await;

        let actual = dice
            .compute(&RepositoryLabelResolutionKey::new(
                workspace_id,
                Arc::new(project_root),
            ))
            .await??;

        assert_eq!(actual.as_ref(), &expected);
        Ok(())
    }

    #[test]
    fn test_invocation_attrs() {
        let mut inv = RepositoryInvocation::new("test".to_owned(), "http_archive".to_owned());
        inv.attrs.insert(
            "url".to_owned(),
            AttrValue::String("https://example.com".to_owned()),
        );
        inv.attrs.insert(
            "urls".to_owned(),
            AttrValue::StringList(vec![
                "https://example.com/a".to_owned(),
                "https://example.com/b".to_owned(),
            ]),
        );
        inv.attrs
            .insert("build_file_content".to_owned(), AttrValue::None);

        let attrs = InvocationAttrs::new(&inv);

        assert_eq!(attrs.get_string("url"), Some("https://example.com"));
        assert_eq!(attrs.get_string("sha256"), None);
        assert_eq!(
            attrs.get_string_list("urls"),
            Some(
                &[
                    "https://example.com/a".to_owned(),
                    "https://example.com/b".to_owned()
                ][..]
            )
        );
    }

    #[test]
    fn test_repository_rule_result() {
        let result =
            RepositoryRuleResult::success("test".to_owned(), PathBuf::from("bazel-external/test"))
                .with_content_hash("sha256-abc123".to_owned());

        assert_eq!(result.repo_name, "test");
        assert_eq!(result.repo_path, PathBuf::from("bazel-external/test"));
        assert_eq!(result.content_hash, Some("sha256-abc123".to_owned()));
        assert!(result.success);
        assert!(result.cacheable);
    }

    // Tests for ExtensionRepoExecutionKey

    #[test]
    fn test_extension_repo_key_creation() {
        let repo_spec =
            RepoSpec::new("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_owned())
                .with_attr(
                    "url".to_owned(),
                    AttrValue::String("https://example.com/foo.tar.gz".to_owned()),
                )
                .with_attr("sha256".to_owned(), AttrValue::String("abc123".to_owned()));

        let key = ExtensionRepoExecutionKey::new(
            "_main+pip+numpy".to_owned(),
            "@@rules_python//pip:pip.bzl%pip".to_owned(),
            repo_spec,
            PathBuf::from("/tmp/project"),
        );

        assert_eq!(key.canonical_name.as_ref(), "_main+pip+numpy");
        assert_eq!(key.extension_id.as_ref(), "@@rules_python//pip:pip.bzl%pip");
        assert!(key.spec_hash.starts_with("sha256-"));
        assert_eq!(
            key.repo_spec.repo_rule_id,
            "@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive"
        );
        assert_eq!(key.project_root.as_ref(), &PathBuf::from("/tmp/project"));
    }

    #[test]
    fn test_extension_repo_key_preserves_workspace_output_base() {
        let project_root = PathBuf::from("/tmp/slug-plan61-repo-key-workspace");
        let workspace_id = crate::WorkspaceId::new(
            project_root.clone(),
            PathBuf::from("/tmp/slug-plan61-repo-key-output-base"),
        );
        let repo_spec =
            RepoSpec::new("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_owned())
                .with_attr(
                    "url".to_owned(),
                    AttrValue::String("https://example.com/foo.tar.gz".to_owned()),
                )
                .with_attr("sha256".to_owned(), AttrValue::String("abc123".to_owned()));

        let key = ExtensionRepoExecutionKey::new_with_workspace_id(
            "_main+pip+numpy".to_owned(),
            "@@rules_python//pip:pip.bzl%pip".to_owned(),
            repo_spec,
            workspace_id.clone(),
        );

        assert_eq!(key.project_root.as_ref(), &project_root);
        assert_eq!(key.materialization_manifest_key.workspace_id, workspace_id);
        assert_eq!(
            key.materialization_manifest_key.output_base,
            workspace_id.output_base
        );
        assert_ne!(
            key.materialization_manifest_key.workspace_id,
            crate::WorkspaceId::for_project_root(project_root)
        );
    }

    #[test]
    fn test_extension_repo_key_from_arcs() {
        let repo_spec = Arc::new(
            RepoSpec::new("@@bazel_tools//repo:git.bzl%git_repository".to_owned()).with_attr(
                "remote".to_owned(),
                AttrValue::String("https://github.com/foo/bar".to_owned()),
            ),
        );

        let key = ExtensionRepoExecutionKey::from_arcs(
            Arc::from("_main+go_deps+gazelle"),
            Arc::from("@@rules_go//deps:go_deps.bzl%go_deps"),
            repo_spec.clone(),
            Arc::new(PathBuf::from("/project")),
        );

        assert_eq!(key.canonical_name.as_ref(), "_main+go_deps+gazelle");
        assert_eq!(
            key.extension_id.as_ref(),
            "@@rules_go//deps:go_deps.bzl%go_deps"
        );
        // Verify the spec is shared (Arc)
        assert!(Arc::ptr_eq(&key.repo_spec, &repo_spec));
    }

    #[test]
    fn test_extension_repo_key_display() {
        let repo_spec = RepoSpec::new("@@tools//repo:http.bzl%http_archive".to_owned());
        let key = ExtensionRepoExecutionKey::new_with_cwd(
            "_main+ext+repo".to_owned(),
            "@@module//ext.bzl%ext".to_owned(),
            repo_spec,
        );

        let display = format!("{}", key);
        assert!(display.starts_with("ExtensionRepoKey(_main+ext+repo, sha256-"));
        assert!(display.ends_with(")"));
    }

    #[test]
    fn test_extension_repo_complete_marker_includes_spec_hash() {
        assert_eq!(complete_marker("", ""), "complete");
        assert_eq!(
            complete_marker("sha256-abc123", ""),
            "complete:sha256-abc123"
        );
        assert_eq!(
            complete_marker("", "sha256-out"),
            "complete:output:sha256-out"
        );
        assert_eq!(
            complete_marker("sha256-abc123", "sha256-out"),
            "complete:sha256-abc123:output:sha256-out"
        );
        assert!(complete_marker_matches(
            "complete:sha256-abc123:output:sha256-out",
            "sha256-abc123"
        ));
        assert_eq!(
            complete_marker_expected_output(
                "complete:sha256-abc123:output:sha256-out",
                "sha256-abc123"
            ),
            Some("sha256-out")
        );
        assert!(!complete_marker_matches(
            "complete:sha256-abc123",
            "sha256-abc123"
        ));
    }

    #[test]
    fn materialization_manifest_treats_recorded_local_rule_as_non_cacheable() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let repo_dir = project_root.join("bazel-external").join("_main+ext+repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(repo_dir.join(".slug_repo_complete"), "complete").unwrap();
        write_repository_rule_local_state(&repo_dir, true).unwrap();

        let spec = RepoSpec::new("//:repo.bzl%custom_repository".to_owned());
        let manifest = repo_materialization_manifest("_main+ext+repo", &spec, &project_root);

        assert_eq!(manifest.marker_state.as_ref(), "local-rule");
    }

    #[test]
    fn test_extension_repo_key_hash_stability() {
        // Same inputs should produce same hash
        let spec1 = RepoSpec::new("@@tools//repo:http.bzl%http_archive".to_owned()).with_attr(
            "url".to_owned(),
            AttrValue::String("https://example.com".to_owned()),
        );
        let spec2 = RepoSpec::new("@@tools//repo:http.bzl%http_archive".to_owned()).with_attr(
            "url".to_owned(),
            AttrValue::String("https://example.com".to_owned()),
        );

        let key1 = ExtensionRepoExecutionKey::new_with_cwd(
            "_main+ext+repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            spec1,
        );
        let key2 = ExtensionRepoExecutionKey::new_with_cwd(
            "_main+ext+repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            spec2,
        );

        assert_eq!(key1.spec_hash, key2.spec_hash);
    }

    #[test]
    fn test_extension_repo_key_hash_includes_repo_env() {
        let spec = RepoSpec::new("@@tools//repo:local.bzl%repository_rule".to_owned());
        let mut first_env = BTreeMap::new();
        first_env.insert("PLAN61_REPO_ENV".to_owned(), "first".to_owned());
        let mut second_env = BTreeMap::new();
        second_env.insert("PLAN61_REPO_ENV".to_owned(), "second".to_owned());

        let key1 = ExtensionRepoExecutionKey::new_with_repo_env(
            "_main+ext+repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            spec.clone(),
            PathBuf::from("/project"),
            Arc::new(first_env),
        );
        let key2 = ExtensionRepoExecutionKey::new_with_repo_env(
            "_main+ext+repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            spec,
            PathBuf::from("/project"),
            Arc::new(second_env),
        );

        assert_ne!(key1.spec_hash, key2.spec_hash);
        assert_ne!(
            key1.materialization_manifest_key.repo_spec_digest,
            key2.materialization_manifest_key.repo_spec_digest
        );
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_extension_repo_key_hash_includes_project_root() {
        // Same spec with different project roots belongs to different workspace state.
        let spec = RepoSpec::new("@@tools//repo:http.bzl%http_archive".to_owned());

        let key1 = ExtensionRepoExecutionKey::new(
            "_main+ext+repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            spec.clone(),
            PathBuf::from("/project1"),
        );
        let key2 = ExtensionRepoExecutionKey::new(
            "_main+ext+repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            spec,
            PathBuf::from("/project2"),
        );

        assert_ne!(key1, key2);
    }

    #[test]
    fn extension_repo_execution_validity_rejects_non_cacheable_results() {
        let cacheable = Ok(Arc::new(RepositoryRuleResult::success(
            "_main+ext+repo".to_owned(),
            PathBuf::from("/project/bazel-external/_main+ext+repo"),
        )));
        assert!(<ExtensionRepoExecutionKey as Key>::validity(&cacheable));

        let non_cacheable = Ok(Arc::new(
            RepositoryRuleResult::success(
                "_main+ext+repo".to_owned(),
                PathBuf::from("/project/bazel-external/_main+ext+repo"),
            )
            .non_cacheable(),
        ));
        assert!(!<ExtensionRepoExecutionKey as Key>::validity(
            &non_cacheable
        ));
    }

    #[test]
    fn test_materialization_manifest_value_tracks_layout_state() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let source_dir = project_root.join("repo_src");
        let repo_dir = project_root
            .join("bazel-external")
            .join("_main+ext+local_repo");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(source_dir.join("data.txt"), "fresh").unwrap();

        let repo_spec = RepoSpec::new(
            "@@bazel_tools//tools/build_defs/repo:local.bzl%new_local_repository".to_owned(),
        )
        .with_attr(
            "path".to_owned(),
            AttrValue::String(source_dir.to_string_lossy().to_string()),
        )
        .with_attr(
            "build_file_content".to_owned(),
            AttrValue::String("exports_files([\"data.txt\"])\n".to_owned()),
        );
        let spec_hash = repo_spec.compute_hash();
        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(source_dir.join("data.txt"), repo_dir.join("data.txt")).unwrap();
        #[cfg(not(unix))]
        std::fs::write(repo_dir.join("data.txt"), "fresh").unwrap();
        let digest = crate::repository_executor::repository_output_digest(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join(".slug_repo_complete"),
            complete_marker(&spec_hash, &digest),
        )
        .unwrap();

        let valid = ExtensionRepoExecutionKey::new(
            "_main+ext+local_repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            repo_spec.clone(),
            project_root.clone(),
        );
        let valid_manifest =
            repo_materialization_manifest_for_key(&valid.materialization_manifest_key);

        #[cfg(unix)]
        {
            std::fs::remove_file(repo_dir.join("data.txt")).unwrap();
            std::fs::write(repo_dir.join("data.txt"), "corrupt").unwrap();
        }
        #[cfg(not(unix))]
        {
            std::fs::remove_file(repo_dir.join("data.txt")).unwrap();
        }

        let corrupt = ExtensionRepoExecutionKey::new(
            "_main+ext+local_repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            repo_spec,
            project_root,
        );
        let corrupt_manifest =
            repo_materialization_manifest_for_key(&corrupt.materialization_manifest_key);

        assert_ne!(valid_manifest.digest, corrupt_manifest.digest);
        assert_ne!(
            valid_manifest.state_summary(),
            corrupt_manifest.state_summary()
        );
        assert_eq!(valid, corrupt);
    }

    #[test]
    fn test_archive_repo_manifest_tracks_output_digest_marker_state() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let repo_dir = project_root
            .join("bazel-external")
            .join("_main+ext+archive_repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        let repo_spec =
            RepoSpec::new("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_owned())
                .with_attr(
                    "url".to_owned(),
                    AttrValue::String("https://example.invalid/archive.tar.gz".to_owned()),
                )
                .with_attr("sha256".to_owned(), AttrValue::String("abc123".to_owned()));
        let spec_hash = repo_spec.compute_hash();
        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();
        std::fs::write(repo_dir.join("data.txt"), "fresh").unwrap();
        let digest = crate::repository_executor::repository_output_digest(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join(".slug_repo_complete"),
            complete_marker(&spec_hash, &digest),
        )
        .unwrap();

        let valid = ExtensionRepoExecutionKey::new(
            "_main+ext+archive_repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            repo_spec.clone(),
            project_root.clone(),
        );
        let valid_manifest =
            repo_materialization_manifest_for_key(&valid.materialization_manifest_key);

        std::fs::write(repo_dir.join("data.txt"), "corrupt").unwrap();

        let corrupt = ExtensionRepoExecutionKey::new(
            "_main+ext+archive_repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            repo_spec,
            project_root,
        );
        let corrupt_manifest =
            repo_materialization_manifest_for_key(&corrupt.materialization_manifest_key);

        assert_ne!(valid_manifest.digest, corrupt_manifest.digest);
        assert!(
            corrupt_manifest
                .marker_state
                .starts_with("marker-output-mismatch:")
        );
        assert_eq!(valid, corrupt);
    }

    #[test]
    fn test_recorded_input_manifest_changes_materialization_manifest() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let watched = project_root.join("watched.txt");
        let repo_dir = project_root
            .join("bazel-external")
            .join("_main+ext+watched_repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(&watched, "first").unwrap();
        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();
        std::fs::write(repo_dir.join("data.txt"), "stable").unwrap();

        let repo_spec = RepoSpec::new("@@//:watched_repo.bzl%watched_repository".to_owned())
            .with_attr(
                "name".to_owned(),
                AttrValue::String("watched_repo".to_owned()),
            );
        let spec_hash = repo_spec.compute_hash();
        let output_digest =
            crate::repository_executor::repository_output_digest(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join(".slug_repo_complete"),
            complete_marker(&spec_hash, &output_digest),
        )
        .unwrap();
        std::fs::write(
            repo_dir.join(REPO_RECORDED_INPUTS_FILE),
            format!(
                "{}\n",
                crate::lockfile::recorded_file_input(&watched).unwrap()
            ),
        )
        .unwrap();

        let valid = ExtensionRepoExecutionKey::new(
            "_main+ext+watched_repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            repo_spec.clone(),
            project_root.clone(),
        );
        let valid_manifest =
            repo_materialization_manifest_for_key(&valid.materialization_manifest_key);

        std::fs::write(&watched, "second").unwrap();

        let stale = ExtensionRepoExecutionKey::new(
            "_main+ext+watched_repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            repo_spec,
            project_root,
        );
        let stale_manifest =
            repo_materialization_manifest_for_key(&stale.materialization_manifest_key);

        assert_ne!(valid_manifest.digest, stale_manifest.digest);
        assert_eq!(valid, stale);
        assert!(
            stale_manifest
                .recorded_inputs_state
                .contains("inputs-invalid:recorded_input_changed")
        );
    }

    #[test]
    fn test_recorded_env_input_manifest_uses_repo_env() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let repo_dir = project_root
            .join("bazel-external")
            .join("_main+ext+env_repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();
        std::fs::write(repo_dir.join("data.txt"), "stable").unwrap();

        let repo_spec = RepoSpec::new("@@//:env_repo.bzl%env_repository".to_owned())
            .with_attr("name".to_owned(), AttrValue::String("env_repo".to_owned()));
        let mut first_env = BTreeMap::new();
        first_env.insert("PLAN61_REPO_ENV".to_owned(), "first".to_owned());
        let first = ExtensionRepoExecutionKey::new_with_repo_env(
            "_main+ext+env_repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            repo_spec.clone(),
            project_root.clone(),
            Arc::new(first_env),
        );
        let output_digest =
            crate::repository_executor::repository_output_digest(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join(".slug_repo_complete"),
            complete_marker(&first.spec_hash, &output_digest),
        )
        .unwrap();
        std::fs::write(
            repo_dir.join(REPO_RECORDED_INPUTS_FILE),
            format!(
                "{}\n",
                crate::lockfile::recorded_env_input("PLAN61_REPO_ENV", Some("first"))
            ),
        )
        .unwrap();

        let valid_manifest =
            repo_materialization_manifest_for_key(&first.materialization_manifest_key);
        assert!(valid_manifest.recorded_inputs_state.ends_with(":valid"));

        let mut second_env = BTreeMap::new();
        second_env.insert("PLAN61_REPO_ENV".to_owned(), "second".to_owned());
        let stale = ExtensionRepoExecutionKey::new_with_repo_env(
            "_main+ext+env_repo".to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            repo_spec,
            project_root,
            Arc::new(second_env),
        );
        std::fs::write(
            repo_dir.join(".slug_repo_complete"),
            complete_marker(&stale.spec_hash, &output_digest),
        )
        .unwrap();

        let stale_manifest =
            repo_materialization_manifest_for_key(&stale.materialization_manifest_key);
        assert_ne!(valid_manifest.digest, stale_manifest.digest);
        assert!(
            stale_manifest
                .recorded_inputs_state
                .contains("inputs-invalid:recorded_input_changed")
        );
    }

    #[test]
    fn test_recorded_repo_mapping_input_manifest_uses_repo_mappings() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let canonical_name = "_main+ext+mapping_repo";
        let repo_dir = project_root.join("bazel-external").join(canonical_name);
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();
        std::fs::write(repo_dir.join("data.txt"), "stable").unwrap();

        let repo_spec = Arc::new(
            RepoSpec::new("@@//:mapping_repo.bzl%mapping_repository".to_owned()).with_attr(
                "name".to_owned(),
                AttrValue::String("mapping_repo".to_owned()),
            ),
        );
        let spec_hash = repo_spec.compute_hash();
        let output_digest =
            crate::repository_executor::repository_output_digest(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join(".slug_repo_complete"),
            complete_marker(&spec_hash, &output_digest),
        )
        .unwrap();
        std::fs::write(
            repo_dir.join(REPO_RECORDED_INPUTS_FILE),
            "REPO_MAPPING:,mapped_dep dep+\n",
        )
        .unwrap();

        let mut first_root_mapping = BTreeMap::new();
        first_root_mapping.insert("mapped_dep".to_owned(), "dep+".to_owned());
        let mut first_mappings = crate::RepoMappingSnapshot::new();
        first_mappings.insert(String::new(), first_root_mapping);
        let first_key =
            RepoMaterializationManifestKey::for_workspace_id_with_repo_spec_digest_repo_env_and_repo_mappings(
                crate::WorkspaceId::for_project_root(project_root.clone()),
                canonical_name,
                repo_spec.clone(),
                spec_hash.clone(),
                Arc::new(BTreeMap::new()),
                Arc::new(first_mappings),
            );
        let valid_manifest = repo_materialization_manifest_for_key(&first_key);
        assert!(valid_manifest.recorded_inputs_state.ends_with(":valid"));

        let mut second_root_mapping = BTreeMap::new();
        second_root_mapping.insert("mapped_dep".to_owned(), "other+".to_owned());
        let mut second_mappings = crate::RepoMappingSnapshot::new();
        second_mappings.insert(String::new(), second_root_mapping);
        let stale_key =
            RepoMaterializationManifestKey::for_workspace_id_with_repo_spec_digest_repo_env_and_repo_mappings(
                crate::WorkspaceId::for_project_root(project_root),
                canonical_name,
                repo_spec,
                spec_hash,
                Arc::new(BTreeMap::new()),
                Arc::new(second_mappings),
            );
        let stale_manifest = repo_materialization_manifest_for_key(&stale_key);

        assert_ne!(first_key, stale_key);
        assert_ne!(valid_manifest.digest, stale_manifest.digest);
        assert!(
            stale_manifest
                .recorded_inputs_state
                .contains("inputs-invalid:recorded_input_changed")
        );
    }

    #[test]
    fn materialization_manifest_layout_rejects_missing_declared_build_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let canonical_name = "_main+ext+missing_build_repo";
        let repo_dir = project_root.join("bazel-external").join(canonical_name);
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(repo_dir.join("data.txt"), "payload\n").unwrap();

        let repo_spec = RepoSpec::new("@@example//:repo.bzl%custom_repository".to_owned())
            .with_attr(
                "build_file_content".to_owned(),
                AttrValue::String("exports_files([\"data.txt\"])\n".to_owned()),
            );
        let key = RepoMaterializationManifestKey::for_project_root(
            project_root.clone(),
            canonical_name,
            Arc::new(repo_spec.clone()),
        );

        let missing = repo_materialization_manifest_for_key(&key);
        assert_eq!(missing.layout_state.as_ref(), "layout-missing-build-file");

        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();
        let present = repo_materialization_manifest_for_key(&key);
        assert_eq!(present.layout_state.as_ref(), "layout-valid");
        assert_ne!(missing.digest, present.digest);
    }

    #[test]
    fn materialization_manifest_layout_rejects_invalid_empty_target_label() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let canonical_name = "_main+ext+invalid_empty_target_repo";
        let repo_dir = project_root.join("bazel-external").join(canonical_name);
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "rust_crate(name = \"x\", deps = [\"@@zstd//:\"])\n",
        )
        .unwrap();

        let repo_spec = RepoSpec::new("@@rules_rs//rs:crate.bzl%crate_repository".to_owned());
        let key = RepoMaterializationManifestKey::for_project_root(
            project_root.clone(),
            canonical_name,
            Arc::new(repo_spec.clone()),
        );
        let invalid = repo_materialization_manifest_for_key(&key);
        assert_eq!(
            invalid.layout_state.as_ref(),
            "layout-invalid-empty-target-label"
        );

        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "rust_crate(name = \"x\", deps = [\"@@zstd//:zstd\"])\n",
        )
        .unwrap();
        let valid = repo_materialization_manifest_for_key(&key);
        assert_eq!(valid.layout_state.as_ref(), "layout-valid");
        assert_ne!(invalid.digest, valid.digest);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn materialization_manifest_layout_rejects_foreign_top_level_symlink() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().join("project");
        let canonical_name = "_main+ext+foreign_link_repo";
        let repo_dir = project_root.join("bazel-external").join(canonical_name);
        let foreign_dir = temp.path().join("foreign/src");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::create_dir_all(&foreign_dir).unwrap();
        std::fs::write(repo_dir.join("BUILD.bazel"), "exports_files([\"src\"])\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&foreign_dir, repo_dir.join("src")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&foreign_dir, repo_dir.join("src")).unwrap();

        let repo_spec = RepoSpec::new("@@example//:repo.bzl%custom_repository".to_owned());
        let key = RepoMaterializationManifestKey::for_project_root(
            project_root.clone(),
            canonical_name,
            Arc::new(repo_spec.clone()),
        );
        let foreign = repo_materialization_manifest_for_key(&key);
        assert_eq!(
            foreign.layout_state.as_ref(),
            "layout-foreign-top-level-symlink"
        );

        std::fs::remove_file(repo_dir.join("src")).unwrap();
        std::fs::write(repo_dir.join("src"), "payload").unwrap();
        let valid = repo_materialization_manifest_for_key(&key);
        assert_eq!(valid.layout_state.as_ref(), "layout-valid");
        assert_ne!(foreign.digest, valid.digest);
    }

    #[tokio::test]
    async fn materialization_manifest_key_observes_layout_state_dependency() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let canonical_name = "_main+ext+missing_build_repo";
        let repo_dir = project_root.join("bazel-external").join(canonical_name);
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(repo_dir.join("data.txt"), "payload\n").unwrap();

        let repo_spec = RepoSpec::new("@@example//:repo.bzl%custom_repository".to_owned())
            .with_attr(
                "build_file_content".to_owned(),
                AttrValue::String("exports_files([\"data.txt\"])\n".to_owned()),
            );
        let key = RepoMaterializationManifestKey::for_project_root(
            project_root.clone(),
            canonical_name,
            Arc::new(repo_spec),
        );
        let mut dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await.unwrap().unwrap();
        assert_eq!(first.layout_state.as_ref(), "layout-missing-build-file");

        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();

        let mut dice = dice.into_updater().commit().await;
        let second = dice.compute(&key).await.unwrap().unwrap();
        assert_ne!(first.digest, second.digest);
        assert_eq!(second.layout_state.as_ref(), "layout-valid");
    }

    #[tokio::test]
    async fn materialization_manifest_key_observes_marker_state_dependency() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let canonical_name = "_main+ext+archive_repo";
        let repo_dir = project_root.join("bazel-external").join(canonical_name);
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();
        std::fs::write(repo_dir.join("data.txt"), "fresh").unwrap();

        let repo_spec =
            RepoSpec::new("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_owned())
                .with_attr(
                    "url".to_owned(),
                    AttrValue::String("https://example.invalid/archive.tar.gz".to_owned()),
                )
                .with_attr("sha256".to_owned(), AttrValue::String("abc123".to_owned()));
        let spec_hash = repo_spec.compute_hash();
        let key = RepoMaterializationManifestKey::for_project_root(
            project_root.clone(),
            canonical_name,
            Arc::new(repo_spec),
        );
        let mut dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await.unwrap().unwrap();
        assert_eq!(first.marker_state.as_ref(), "marker-absent");
        assert!(<RepoMaterializationManifestKey as Key>::validity(&Ok(
            first.dupe()
        )));

        let output_digest =
            crate::repository_executor::repository_output_digest(&repo_dir).unwrap();
        let marker = complete_marker(&spec_hash, &output_digest);
        std::fs::write(repo_dir.join(".slug_repo_complete"), format!("{marker}\n")).unwrap();

        let mut dice = dice.into_updater().commit().await;
        let second = dice.compute(&key).await.unwrap().unwrap();
        assert_ne!(first.digest, second.digest);
        assert_eq!(second.marker_state.as_ref(), format!("marker:{marker}"));
    }

    #[tokio::test]
    async fn materialization_manifest_key_observes_rule_local_state_dependency() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let canonical_name = "_main+ext+local_state_repo";
        let repo_dir = project_root.join("bazel-external").join(canonical_name);
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();
        std::fs::write(repo_dir.join("data.txt"), "fresh").unwrap();

        let repo_spec = RepoSpec::new("//:repo.bzl%custom_repository".to_owned());
        let spec_hash = repo_spec.compute_hash();
        let output_digest =
            crate::repository_executor::repository_output_digest(&repo_dir).unwrap();
        let marker = complete_marker(&spec_hash, &output_digest);
        std::fs::write(repo_dir.join(".slug_repo_complete"), format!("{marker}\n")).unwrap();
        let key = RepoMaterializationManifestKey::for_project_root(
            project_root.clone(),
            canonical_name,
            Arc::new(repo_spec),
        );
        let mut dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await.unwrap().unwrap();
        assert_eq!(first.marker_state.as_ref(), format!("marker:{marker}"));

        write_repository_rule_local_state(&repo_dir, true).unwrap();

        let mut dice = dice.into_updater().commit().await;
        let second = dice.compute(&key).await.unwrap().unwrap();
        assert_ne!(first.digest, second.digest);
        assert_eq!(second.marker_state.as_ref(), "local-rule");
    }

    #[tokio::test]
    async fn materialization_manifest_key_observes_marker_output_digest_dependency() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let canonical_name = "_main+ext+archive_repo";
        let repo_dir = project_root.join("bazel-external").join(canonical_name);
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();
        std::fs::write(repo_dir.join("data.txt"), "fresh").unwrap();

        let repo_spec =
            RepoSpec::new("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_owned())
                .with_attr(
                    "url".to_owned(),
                    AttrValue::String("https://example.invalid/archive.tar.gz".to_owned()),
                )
                .with_attr("sha256".to_owned(), AttrValue::String("abc123".to_owned()));
        let spec_hash = repo_spec.compute_hash();
        let output_digest =
            crate::repository_executor::repository_output_digest(&repo_dir).unwrap();
        let marker = complete_marker(&spec_hash, &output_digest);
        std::fs::write(repo_dir.join(".slug_repo_complete"), format!("{marker}\n")).unwrap();
        let key = RepoMaterializationManifestKey::for_project_root(
            project_root.clone(),
            canonical_name,
            Arc::new(repo_spec),
        );
        let mut dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await.unwrap().unwrap();
        assert_eq!(first.marker_state.as_ref(), format!("marker:{marker}"));

        std::fs::write(repo_dir.join("data.txt"), "corrupt").unwrap();

        let mut dice = dice.into_updater().commit().await;
        let second = dice.compute(&key).await.unwrap().unwrap();
        assert_ne!(first.digest, second.digest);
        assert!(second.marker_state.starts_with("marker-output-mismatch:"));
    }

    #[tokio::test]
    async fn materialization_manifest_key_observes_recorded_input_state_dependency() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let canonical_name = "_main+ext+watched_repo";
        let repo_dir = project_root.join("bazel-external").join(canonical_name);
        let watched = project_root.join("watched.txt");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(&watched, "first\n").unwrap();
        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();
        std::fs::write(repo_dir.join("data.txt"), "stable\n").unwrap();

        let repo_spec = RepoSpec::new("@@//:watched_repo.bzl%watched_repository".to_owned());
        let spec_hash = repo_spec.compute_hash();
        let output_digest =
            crate::repository_executor::repository_output_digest(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join(".slug_repo_complete"),
            complete_marker(&spec_hash, &output_digest),
        )
        .unwrap();
        let key = RepoMaterializationManifestKey::for_project_root(
            project_root.clone(),
            canonical_name,
            Arc::new(repo_spec),
        );
        let mut dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await.unwrap().unwrap();
        assert_eq!(first.recorded_inputs_state.as_ref(), "inputs:none");

        std::fs::write(
            repo_dir.join(REPO_RECORDED_INPUTS_FILE),
            format!(
                "{}\n",
                crate::lockfile::recorded_file_input(&watched).unwrap()
            ),
        )
        .unwrap();

        let mut dice = dice.into_updater().commit().await;
        let current = dice.compute(&key).await.unwrap().unwrap();
        assert_ne!(first.digest, current.digest);
        assert!(current.recorded_inputs_state.ends_with(":valid"));

        std::fs::write(&watched, "second\n").unwrap();
        let mut dice = dice.into_updater().commit().await;
        let stale = dice.compute(&key).await.unwrap().unwrap();
        assert_ne!(current.digest, stale.digest);
        assert!(
            stale
                .recorded_inputs_state
                .contains("inputs-invalid:recorded_input_changed")
        );
    }

    #[tokio::test]
    async fn extension_repo_execution_consumes_materialization_manifest_key() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path().to_path_buf();
        let canonical_name = "_main+ext+archive_repo";
        let exec_repo_spec =
            RepoSpec::new("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_owned())
                .with_attr(
                    "url".to_owned(),
                    AttrValue::String("https://example.invalid/archive.tar.gz".to_owned()),
                )
                .with_attr("sha256".to_owned(), AttrValue::String("abc123".to_owned()));
        let manifest_repo_spec =
            RepoSpec::new("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_owned())
                .with_attr(
                    "url".to_owned(),
                    AttrValue::String("https://example.invalid/archive.tar.gz".to_owned()),
                )
                .with_attr("sha256".to_owned(), AttrValue::String("def456".to_owned()));
        let manifest_spec_hash = manifest_repo_spec.compute_hash();
        let repo_dir = project_root.join("bazel-external").join(canonical_name);
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join("BUILD.bazel"),
            "exports_files([\"data.txt\"])\n",
        )
        .unwrap();
        std::fs::write(repo_dir.join("data.txt"), "fresh").unwrap();
        let output_digest =
            crate::repository_executor::repository_output_digest(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join(".slug_repo_complete"),
            complete_marker(&manifest_spec_hash, &output_digest),
        )
        .unwrap();

        let mut key = ExtensionRepoExecutionKey::new(
            canonical_name.to_owned(),
            "@@m//e.bzl%ext".to_owned(),
            exec_repo_spec,
            project_root.clone(),
        );
        key.materialization_manifest_key =
            Arc::new(RepoMaterializationManifestKey::for_project_root(
                project_root.clone(),
                canonical_name,
                Arc::new(manifest_repo_spec),
            ));

        let mut dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let result = dice.compute(&key).await.unwrap().unwrap();

        assert_eq!(result.repo_name, canonical_name);
        assert_eq!(
            result.repo_path,
            project_root.join("bazel-external").join(canonical_name)
        );
        assert!(result.repo_path.exists());
    }

    // Tests for repo_spec_to_invocation

    #[test]
    fn test_repo_spec_to_invocation_basic() {
        let repo_spec =
            RepoSpec::new("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_owned())
                .with_attr(
                    "url".to_owned(),
                    AttrValue::String("https://example.com/foo.tar.gz".to_owned()),
                )
                .with_attr("sha256".to_owned(), AttrValue::String("abc123".to_owned()));

        let invocation = repo_spec_to_invocation("_main+pip+numpy", &repo_spec).unwrap();

        assert_eq!(invocation.name, "_main+pip+numpy");
        assert_eq!(invocation.rule_name, "http_archive");
        assert_eq!(
            invocation.rule_source,
            Some("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_owned())
        );
        assert_eq!(invocation.attrs.len(), 2);
        assert_eq!(
            invocation.attrs.get("url"),
            Some(&AttrValue::String(
                "https://example.com/foo.tar.gz".to_owned()
            ))
        );
        assert_eq!(
            invocation.attrs.get("sha256"),
            Some(&AttrValue::String("abc123".to_owned()))
        );
    }

    #[test]
    fn test_repo_spec_to_invocation_with_complex_attrs() {
        let repo_spec = RepoSpec::new("@@rules_go//go:deps.bzl%go_repository".to_owned())
            .with_attr(
                "importpath".to_owned(),
                AttrValue::String("github.com/foo/bar".to_owned()),
            )
            .with_attr("sum".to_owned(), AttrValue::String("h1:abc=".to_owned()))
            .with_attr("version".to_owned(), AttrValue::String("v1.2.3".to_owned()))
            .with_attr(
                "build_file_generation".to_owned(),
                AttrValue::String("auto".to_owned()),
            );

        let invocation =
            repo_spec_to_invocation("_main+go_deps+com_github_foo_bar", &repo_spec).unwrap();

        assert_eq!(invocation.name, "_main+go_deps+com_github_foo_bar");
        assert_eq!(invocation.rule_name, "go_repository");
        assert_eq!(invocation.attrs.len(), 4);
    }

    #[test]
    fn test_repo_spec_to_invocation_no_attrs() {
        let repo_spec = RepoSpec::new("@@//local:repo.bzl%local_repository".to_owned());

        let invocation = repo_spec_to_invocation("_main+local+myrepo", &repo_spec).unwrap();

        assert_eq!(invocation.name, "_main+local+myrepo");
        assert_eq!(invocation.rule_name, "local_repository");
        assert!(invocation.attrs.is_empty());
    }

    #[test]
    fn test_repo_spec_to_invocation_plain_rule_name() {
        // Plain rule name (no % separator) - common in DICE-based extension execution
        let repo_spec = RepoSpec::new("http_archive".to_owned());

        let invocation = repo_spec_to_invocation("_main+ext+repo", &repo_spec).unwrap();
        assert_eq!(invocation.rule_name, "http_archive");
    }

    #[test]
    fn test_repo_spec_to_invocation_empty_rule_id() {
        let repo_spec = RepoSpec::new(String::new());

        let result = repo_spec_to_invocation("_main+ext+repo", &repo_spec);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_rule_name_from_id() {
        assert_eq!(
            extract_rule_name_from_id("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive"),
            Some("http_archive".to_owned())
        );
        assert_eq!(
            extract_rule_name_from_id("@@rules_python//pip:pip.bzl%pip_install"),
            Some("pip_install".to_owned())
        );
        assert_eq!(
            extract_rule_name_from_id("//:local.bzl%my_rule"),
            Some("my_rule".to_owned())
        );
        // Edge case: multiple % chars (use last one)
        assert_eq!(
            extract_rule_name_from_id("@@module//path%weird:file.bzl%actual_rule"),
            Some("actual_rule".to_owned())
        );
        // Plain rule name (no bzl path)
        assert_eq!(
            extract_rule_name_from_id("http_archive"),
            Some("http_archive".to_owned())
        );
        // Empty string
        assert_eq!(extract_rule_name_from_id(""), None);
    }
}
