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

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;

use crate::dice_graph::BzlmodEventKind;
use crate::dice_graph::RepoMaterializationManifestKey;
use crate::dice_graph::RepoMaterializationManifestValue;
use crate::dice_graph::record_bzlmod_event;
use crate::lockfile::compute_sha256_hex;
use crate::lockfile::validate_recorded_inputs_current;
use crate::repo_spec::RepoSpec;
use crate::repository_invocations::RepositoryInvocation;

pub(crate) const REPO_RECORDED_INPUTS_FILE: &str = ".slug_repo_recorded_inputs";

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
}

impl RepositoryRuleResult {
    /// Create a successful result.
    pub fn success(repo_name: String, repo_path: PathBuf) -> Self {
        Self {
            repo_path,
            content_hash: None,
            repo_name,
            success: true,
        }
    }

    /// Create a result with a content hash.
    pub fn with_content_hash(mut self, hash: String) -> Self {
        self.content_hash = Some(hash);
        self
    }
}

/// DICE key for repository rule execution.
///
/// When this key is computed, it executes the repository rule and materializes
/// the repository content to disk.
#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative, Dupe)]
#[display("RepositoryRuleKey({}, {})", name, rule_name)]
pub struct RepositoryRuleExecutionKey {
    /// Repository name (from the `name` attribute).
    pub name: Arc<str>,

    /// Repository rule name (e.g., "http_archive").
    pub rule_name: Arc<str>,

    /// Hash of attributes for cache invalidation.
    pub attrs_hash: Arc<str>,
}

impl RepositoryRuleExecutionKey {
    /// Create a new execution key from an invocation.
    pub fn from_invocation(invocation: &RepositoryInvocation) -> Self {
        Self {
            name: Arc::from(invocation.name.as_str()),
            rule_name: Arc::from(invocation.rule_name.as_str()),
            attrs_hash: Arc::from(invocation.compute_hash().as_str()),
        }
    }

    /// Create a new execution key directly.
    pub fn new(name: String, rule_name: String, attrs_hash: String) -> Self {
        Self {
            name: Arc::from(name.as_str()),
            rule_name: Arc::from(rule_name.as_str()),
            attrs_hash: Arc::from(attrs_hash.as_str()),
        }
    }
}

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
        x.is_ok()
    }
}

/// Explain why direct `RepositoryRuleExecutionKey` computation is disabled.
pub fn repository_rule_execution_key_unimplemented_error(
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
}

impl std::hash::Hash for ExtensionRepoExecutionKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the identifying fields; spec_hash represents the repo_spec
        self.canonical_name.hash(state);
        self.extension_id.hash(state);
        self.spec_hash.hash(state);
        self.project_root.hash(state);
        self.materialization_manifest_key.hash(state);
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
    }
}

impl Eq for ExtensionRepoExecutionKey {}

impl ExtensionRepoExecutionKey {
    /// Create a new extension repo execution key.
    pub fn new(
        canonical_name: String,
        extension_id: String,
        repo_spec: RepoSpec,
        project_root: PathBuf,
    ) -> Self {
        let spec_hash = repo_spec.compute_hash();
        let repo_spec = Arc::new(repo_spec);
        let materialization_manifest_key = RepoMaterializationManifestKey::for_project_root(
            project_root.clone(),
            canonical_name.as_str(),
            repo_spec.clone(),
        );
        Self {
            canonical_name: Arc::from(canonical_name.as_str()),
            extension_id: Arc::from(extension_id.as_str()),
            spec_hash: Arc::from(spec_hash.as_str()),
            repo_spec,
            project_root: Arc::new(project_root),
            materialization_manifest_key: Arc::new(materialization_manifest_key),
        }
    }

    /// Create from Arc references (avoids cloning for repeated use).
    pub fn from_arcs(
        canonical_name: Arc<str>,
        extension_id: Arc<str>,
        repo_spec: Arc<RepoSpec>,
        project_root: Arc<PathBuf>,
    ) -> Self {
        let spec_hash = repo_spec.compute_hash();
        let materialization_manifest_key = RepoMaterializationManifestKey::for_project_root(
            project_root.as_ref().clone(),
            canonical_name.as_ref(),
            repo_spec.clone(),
        );
        Self {
            canonical_name,
            extension_id,
            spec_hash: Arc::from(spec_hash.as_str()),
            repo_spec,
            project_root,
            materialization_manifest_key: Arc::new(materialization_manifest_key),
        }
    }

    /// Create with default project root (current directory).
    /// Primarily for testing.
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

pub fn repository_recorded_inputs_current(repo_dir: &Path) -> bool {
    repository_recorded_inputs_digest(repo_dir).is_ok()
}

fn repository_recorded_inputs_state(repo_dir: &Path) -> String {
    match repository_recorded_inputs_digest(repo_dir) {
        Ok(None) => "inputs:none".to_owned(),
        Ok(Some(digest)) => format!("inputs:{digest}:valid"),
        Err(reason) => format!("inputs-invalid:{reason}"),
    }
}

fn repository_recorded_inputs_digest(repo_dir: &Path) -> Result<Option<String>, String> {
    let manifest_path = repo_dir.join(REPO_RECORDED_INPUTS_FILE);
    if !manifest_path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(&manifest_path).map_err(|_| "recorded_inputs_unreadable")?;
    let recorded_inputs: Vec<String> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect();
    validate_recorded_inputs_current(&recorded_inputs, None, None, None)?;
    Ok(Some(compute_sha256_hex(content.as_bytes())))
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

fn repo_materialization_manifest_for_key(
    key: &RepoMaterializationManifestKey,
) -> RepoMaterializationManifestValue {
    let canonical_name = key.canonical_repo.as_ref();
    let repo_spec = key.repo_spec.as_ref();
    let spec_hash = key.repo_spec_digest.as_ref();
    let project_root = key.workspace_id.canonical_project_root.as_ref();
    let repo_dir = project_root.join("bazel-external").join(canonical_name);
    let marker_path = repo_dir.join(".slug_repo_complete");
    let marker_state = if repo_spec.local {
        "local-rule".to_owned()
    } else if marker_path.exists() {
        match std::fs::read_to_string(&marker_path) {
            Ok(marker) => {
                let trimmed = marker.trim();
                if complete_marker_matches(trimmed, spec_hash) {
                    format!("marker:{trimmed}")
                } else {
                    format!("marker-mismatch:{trimmed}")
                }
            }
            Err(e) => format!("marker-unreadable:{e}"),
        }
    } else {
        "marker-absent".to_owned()
    };

    let layout_state = match repo_spec_to_invocation(canonical_name, repo_spec) {
        Ok(invocation) => {
            if crate::repository_executor::repo_layout_is_valid_for_invocation(
                &invocation,
                &repo_dir,
            ) {
                "layout-valid".to_owned()
            } else {
                "layout-invalid".to_owned()
            }
        }
        Err(e) => format!("layout-unclassifiable:{e}"),
    };
    let recorded_inputs_state = repository_recorded_inputs_state(&repo_dir);

    RepoMaterializationManifestValue::new(
        key.clone(),
        repo_dir,
        marker_state,
        layout_state,
        recorded_inputs_state,
    )
}

#[async_trait]
impl Key for RepoMaterializationManifestKey {
    type Value = slug_error::Result<Arc<RepoMaterializationManifestValue>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Ok(Arc::new(repo_materialization_manifest_for_key(self)))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.digest == y.digest && x.key == y.key,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        // Recompute every request until marker/layout/recorded-input reads are
        // backed by tracked DICE filesystem dependencies.
        false
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
        let marker_path = working_dir.join(".slug_repo_complete");
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
        } else if manifest.marker_state.starts_with("marker-unreadable:") || marker_path.exists() {
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
                                let output_digest =
                                    crate::repository_executor::repository_output_digest(
                                        &working_dir,
                                    )?;
                                let _ = std::fs::write(
                                    working_dir.join(".slug_repo_complete"),
                                    complete_marker(&self.spec_hash, &output_digest),
                                );
                                return Ok(Arc::new(RepositoryRuleResult::success(
                                    self.canonical_name.to_string(),
                                    working_dir,
                                )));
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
        let result =
            crate::repository_executor::execute_repository_rule(&invocation, &self.project_root)?;
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
        x.is_ok()
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
pub fn repo_spec_to_invocation(
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
pub struct RepositoryRegistry {
    /// Map from repository name to invocation.
    invocations: std::collections::HashMap<String, RepositoryInvocation>,
}

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

    /// Get all invocations.
    pub fn all(&self) -> impl Iterator<Item = &RepositoryInvocation> {
        self.invocations.values()
    }

    /// Check if a repository is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.invocations.contains_key(name)
    }

    /// Get the number of registered repositories.
    pub fn len(&self) -> usize {
        self.invocations.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.invocations.is_empty()
    }

    /// Create DICE keys for all registered repositories.
    pub fn execution_keys(&self) -> Vec<RepositoryRuleExecutionKey> {
        self.invocations
            .values()
            .map(RepositoryRuleExecutionKey::from_invocation)
            .collect()
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
        assert!(!complete_marker_matches(
            "complete:sha256-abc123",
            "sha256-abc123"
        ));
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
    fn test_archive_repo_key_hash_includes_marker_state() {
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

        assert_eq!(valid_manifest.digest, corrupt_manifest.digest);
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
