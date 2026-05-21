/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! DICE-based module extension execution.
//!
//! This module provides DICE keys for evaluating module extensions. Extensions
//! are evaluated to capture `RepoSpec` objects (deferred execution model) - no
//! actual downloads happen during extension evaluation.
//!
//! ## Deferred Execution Model
//!
//! When a module extension is evaluated:
//! 1. A temporary working directory is created for `module_ctx` I/O
//! 2. The extension implementation is called with `module_ctx`
//! 3. Repository rule calls capture `RepoSpec` objects (NOT executed)
//! 4. The temporary directory is cleaned up
//! 5. `ModuleExtensionResult` is returned with all captured specs
//!
//! Actual repository materialization happens later via `ExtensionRepoExecutionKey`
//! when repositories are first accessed during a build.
//!
//! ## Pattern Reference
//!
//! This follows the `RepositoryRuleExecutionKey` pattern from `repository_execution.rs`.

use std::collections::BTreeMap;
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
use fxhash::FxHashMap;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark_syntax::syntax::ast::AstStmt;
use starlark_syntax::syntax::ast::StmtP;

use crate::BzlmodSessionData;
use crate::RepoMappingOverrides;
use crate::RepoMappingSnapshot;
use crate::dice_graph::BzlmodEventKind;
use crate::dice_graph::LockfileContentKey;
use crate::dice_graph::LockfileContentKind;
use crate::dice_graph::WorkspaceId;
use crate::dice_graph::record_bzlmod_event;
use crate::extensions::AggregatedExtension;
use crate::extensions::compute_extension_input_hash;
use crate::lockfile::LockfileMode;
use crate::module_extension_executor::MODULE_EXTENSION_EXECUTOR_IMPL;
use crate::module_extension_executor::ModuleExtensionMetadata;

const MAX_EXTENSION_IDS_IN_WARNING: usize = 25;

fn extension_ids_summary<'a>(extension_ids: impl Iterator<Item = &'a String>) -> String {
    let mut shown = Vec::new();
    let mut total = 0;
    for extension_id in extension_ids {
        total += 1;
        if shown.len() < MAX_EXTENSION_IDS_IN_WARNING {
            shown.push(extension_id.as_str());
        }
    }

    if total <= MAX_EXTENSION_IDS_IN_WARNING {
        return format!("{shown:?} ({total} total)");
    }

    format!(
        "{shown:?} (showing {} of {}; {} omitted)",
        shown.len(),
        total,
        total - shown.len()
    )
}

/// Look up the aggregated extension data and create a `ModuleExtensionExecutionKey`.
///
/// Returns `None` if the extension is not found in the current command's
/// DICE-injected bzlmod session data.
pub fn create_extension_execution_key(
    data: &BzlmodSessionData,
    extension_id: &str,
) -> Option<ModuleExtensionExecutionKey> {
    let aggregated = match data.extension_aggregations.get(extension_id) {
        Some(a) => a,
        None => {
            tracing::warn!(
                "create_extension_execution_key: extension '{}' not found in aggregations. Available: {}",
                extension_id,
                extension_ids_summary(data.extension_aggregations.keys())
            );
            return None;
        }
    };
    Some(ModuleExtensionExecutionKey::new_with_lockfile(
        aggregated.clone(),
        data.root_module_name.clone(),
        data.project_root.clone(),
        data.hidden_lockfile_path.clone(),
        data.lockfile_mode,
        data.repo_env.clone(),
        data.repo_mappings.clone(),
        data.repo_mapping_overrides.clone(),
    ))
}
use crate::repo_spec::RepoSpec;

/// Errors during module extension execution.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
pub enum ModuleExtensionError {
    #[error("Module extension execution failed for '{extension_id}': {reason}")]
    ExecutionFailed {
        extension_id: String,
        reason: String,
    },

    #[error(
        "Failed to create temporary working directory for extension '{extension_id}': {reason}"
    )]
    TempDirFailed {
        extension_id: String,
        reason: String,
    },

    #[error("Extension '{extension_id}' not found")]
    ExtensionNotFound { extension_id: String },

    #[error("Failed to load extension .bzl file: {path}")]
    BzlLoadFailed { path: String },

    #[error(
        "MODULE.bazel.lock is no longer up-to-date because {reason}. \
        Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
    )]
    OutdatedLockfile { reason: String },
}

/// Result of module extension evaluation.
///
/// Contains captured RepoSpecs but NO materialized repositories.
/// Repositories are created lazily when accessed during a build.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ModuleExtensionResult {
    /// Extension identifier: "@@module//path:file.bzl%extension_name"
    pub extension_id: Arc<str>,

    /// Hash of extension inputs (tags from all modules) for cache invalidation.
    pub input_hash: String,

    /// Generated repository specifications (NOT materialized).
    /// Keys are internal names (e.g., "numpy"), values are RepoSpecs.
    ///
    /// `FxHashMap` so iteration is stable across invocations (Plan 21.2).
    pub generated_repo_specs: FxHashMap<String, RepoSpec>,

    /// Canonical name mapping.
    /// Maps internal_name -> canonical_name (e.g., "numpy" -> "_main+pip+numpy")
    pub canonical_names: FxHashMap<String, String>,

    /// Metadata returned by module_ctx.extension_metadata(...).
    ///
    /// Bazel stores facts as part of `SingleExtensionValue` and deliberately
    /// excludes them from normal replay invalidation.
    #[allocative(skip)]
    pub metadata: ModuleExtensionMetadata,
}

impl ModuleExtensionResult {
    /// Create a new extension result.
    ///
    /// `root_module_name` is the name of the root module (from MODULE.bazel
    /// `module(name=…)`). It is required so canonical names use Bazel's
    /// `_main` placeholder for the root module's own extensions; without it
    /// the root module's declared name leaks into canonical names and they
    /// disagree with the cells pre-computed in `pending_repo_cells.rs`.
    pub fn new(
        extension_id: Arc<str>,
        input_hash: String,
        generated_repo_specs: FxHashMap<String, RepoSpec>,
        root_module_name: &str,
    ) -> Self {
        Self::new_with_metadata(
            extension_id,
            input_hash,
            generated_repo_specs,
            root_module_name,
            ModuleExtensionMetadata::default(),
        )
    }

    pub fn new_with_metadata(
        extension_id: Arc<str>,
        input_hash: String,
        generated_repo_specs: FxHashMap<String, RepoSpec>,
        root_module_name: &str,
        metadata: ModuleExtensionMetadata,
    ) -> Self {
        let canonical_names =
            build_canonical_names(&extension_id, &generated_repo_specs, root_module_name);
        Self {
            extension_id,
            input_hash,
            generated_repo_specs,
            canonical_names,
            metadata,
        }
    }

    /// Get the canonical name for a repository by its internal name.
    pub fn canonical_name(&self, internal_name: &str) -> Option<&str> {
        self.canonical_names.get(internal_name).map(|s| s.as_str())
    }

    /// Get a RepoSpec by internal name.
    pub fn get_repo_spec(&self, internal_name: &str) -> Option<&RepoSpec> {
        self.generated_repo_specs.get(internal_name)
    }

    /// Get all internal repository names.
    pub fn repo_names(&self) -> impl Iterator<Item = &str> {
        self.generated_repo_specs.keys().map(|s| s.as_str())
    }

    /// Check if this result contains a repository with the given internal name.
    pub fn contains_repo(&self, internal_name: &str) -> bool {
        self.generated_repo_specs.contains_key(internal_name)
    }

    /// Get the number of generated repositories.
    pub fn repo_count(&self) -> usize {
        self.generated_repo_specs.len()
    }

    /// Look up internal name from canonical name.
    pub fn internal_name_from_canonical(&self, canonical: &str) -> Option<&str> {
        self.canonical_names
            .iter()
            .find(|(_, c)| c.as_str() == canonical)
            .map(|(i, _)| i.as_str())
    }
}

/// DICE key for module extension evaluation.
///
/// When computed, this:
/// 1. Checks lockfile for cached result (if project_root is set)
/// 2. Creates a temporary working directory for module_ctx
/// 3. Loads the extension's .bzl file
/// 4. Builds module_ctx from aggregated tags
/// 5. Executes implementation(module_ctx) with RepoSpec capture
/// 6. Cleans up the temporary directory
/// 7. Returns ModuleExtensionResult with captured specs
///
/// Note: NO downloads or repository materialization happens during this computation.
/// Repositories are materialized lazily via `ExtensionRepoExecutionKey`.
///
/// Note: Hash and Eq are implemented manually because `AggregatedExtension` contains
/// HashMap. The `input_hash` field is used for hashing, ensuring deterministic cache behavior.
/// `project_root` is included because it identifies the workspace whose lockfile,
/// local `.bzl` loads, and generated repo namespace are being evaluated.
#[derive(Clone, Debug, Display, Allocative)]
#[display("ModuleExtensionKey({}, {})", extension_id, input_hash)]
pub struct ModuleExtensionExecutionKey {
    /// Extension identifier: "@@module//path:file.bzl%extension_name"
    pub extension_id: Arc<str>,

    /// Hash of input tags for cache invalidation.
    /// This hash covers all tags from all modules that use this extension.
    pub input_hash: Arc<str>,

    /// Aggregated extension data from all modules.
    /// Contains all the tags needed to build module_ctx.
    pub aggregated: Arc<AggregatedExtension>,

    /// Root module name (needed for build_module_context).
    pub root_module_name: Arc<str>,

    /// Project root for read-only lockfile access (optional).
    /// If set, Bazel-authored lockfile caches may be read. Ordinary builds
    /// must not write `MODULE.bazel.lock`; it is a Bazel-owned compatibility
    /// surface, not a Slug-private extension cache.
    pub project_root: Option<Arc<PathBuf>>,

    /// Hidden lockfile path used as a fallback for replay data and prior facts.
    ///
    /// Bazel reads the workspace lockfile first, then the hidden lockfile.
    /// In ERROR mode, facts are still validated only against workspace facts.
    pub hidden_lockfile_path: Option<Arc<PathBuf>>,

    /// Lockfile policy for extension replay reads.
    ///
    /// This is part of the key identity because Bazel lockfile mode changes
    /// whether replay data can be read at all.
    pub lockfile_mode: LockfileMode,

    /// Effective Bazel repository environment used for ENV recorded-input replay.
    pub repo_env: Arc<BTreeMap<String, String>>,

    /// Current scoped repository mappings used for REPO_MAPPING recorded-input replay.
    pub repo_mappings: Arc<RepoMappingSnapshot>,

    /// Root-module override_repo rows used for extension-generated repo mappings.
    pub repo_mapping_overrides: Arc<RepoMappingOverrides>,
}

impl std::hash::Hash for ModuleExtensionExecutionKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the identifying fields; input_hash represents the aggregated data
        self.extension_id.hash(state);
        self.input_hash.hash(state);
        self.root_module_name.hash(state);
        self.project_root.hash(state);
        self.hidden_lockfile_path.hash(state);
        self.lockfile_mode.hash(state);
        self.repo_env.hash(state);
        self.repo_mappings.hash(state);
        self.repo_mapping_overrides.hash(state);
    }
}

impl PartialEq for ModuleExtensionExecutionKey {
    fn eq(&self, other: &Self) -> bool {
        // Compare by identifying fields; input_hash represents the aggregated data
        self.extension_id == other.extension_id
            && self.input_hash == other.input_hash
            && self.root_module_name == other.root_module_name
            && self.project_root == other.project_root
            && self.hidden_lockfile_path == other.hidden_lockfile_path
            && self.lockfile_mode == other.lockfile_mode
            && self.repo_env == other.repo_env
            && self.repo_mappings == other.repo_mappings
            && self.repo_mapping_overrides == other.repo_mapping_overrides
    }
}

impl Eq for ModuleExtensionExecutionKey {}

// Manual Dupe implementation
impl Dupe for ModuleExtensionExecutionKey {
    fn dupe(&self) -> Self {
        Self {
            extension_id: self.extension_id.dupe(),
            input_hash: self.input_hash.dupe(),
            aggregated: self.aggregated.dupe(),
            root_module_name: self.root_module_name.dupe(),
            project_root: self.project_root.clone(),
            hidden_lockfile_path: self.hidden_lockfile_path.clone(),
            lockfile_mode: self.lockfile_mode,
            repo_env: self.repo_env.clone(),
            repo_mappings: self.repo_mappings.clone(),
            repo_mapping_overrides: self.repo_mapping_overrides.clone(),
        }
    }
}

impl ModuleExtensionExecutionKey {
    /// Create a new extension execution key from aggregated extension data.
    pub fn new(aggregated: AggregatedExtension, root_module_name: String) -> Self {
        let extension_id = Arc::from(aggregated.extension_id.as_str());
        let input_hash = Arc::from(compute_extension_input_hash(&aggregated).as_str());
        Self {
            extension_id,
            input_hash,
            aggregated: Arc::new(aggregated),
            root_module_name: Arc::from(root_module_name.as_str()),
            project_root: None,
            hidden_lockfile_path: None,
            lockfile_mode: LockfileMode::Update,
            repo_env: Arc::new(BTreeMap::new()),
            repo_mappings: Arc::new(RepoMappingSnapshot::new()),
            repo_mapping_overrides: Arc::new(RepoMappingOverrides::new()),
        }
    }

    /// Create a new extension execution key with lockfile support.
    pub fn new_with_lockfile(
        aggregated: AggregatedExtension,
        root_module_name: String,
        project_root: PathBuf,
        hidden_lockfile_path: Option<PathBuf>,
        lockfile_mode: LockfileMode,
        repo_env: BTreeMap<String, String>,
        repo_mappings: RepoMappingSnapshot,
        repo_mapping_overrides: RepoMappingOverrides,
    ) -> Self {
        let extension_id = Arc::from(aggregated.extension_id.as_str());
        let input_hash = Arc::from(compute_extension_input_hash(&aggregated).as_str());
        Self {
            extension_id,
            input_hash,
            aggregated: Arc::new(aggregated),
            root_module_name: Arc::from(root_module_name.as_str()),
            project_root: Some(Arc::new(project_root)),
            hidden_lockfile_path: hidden_lockfile_path.map(Arc::new),
            lockfile_mode,
            repo_env: Arc::new(repo_env),
            repo_mappings: Arc::new(repo_mappings),
            repo_mapping_overrides: Arc::new(repo_mapping_overrides),
        }
    }

    /// Create from Arc references (avoids cloning for repeated use).
    pub fn from_arcs(
        extension_id: Arc<str>,
        input_hash: Arc<str>,
        aggregated: Arc<AggregatedExtension>,
        root_module_name: Arc<str>,
    ) -> Self {
        Self {
            extension_id,
            input_hash,
            aggregated,
            root_module_name,
            project_root: None,
            hidden_lockfile_path: None,
            lockfile_mode: LockfileMode::Update,
            repo_env: Arc::new(BTreeMap::new()),
            repo_mappings: Arc::new(RepoMappingSnapshot::new()),
            repo_mapping_overrides: Arc::new(RepoMappingOverrides::new()),
        }
    }

    /// Create from Arc references with lockfile support.
    pub fn from_arcs_with_lockfile(
        extension_id: Arc<str>,
        input_hash: Arc<str>,
        aggregated: Arc<AggregatedExtension>,
        root_module_name: Arc<str>,
        project_root: Arc<PathBuf>,
        hidden_lockfile_path: Option<Arc<PathBuf>>,
        lockfile_mode: LockfileMode,
        repo_env: Arc<BTreeMap<String, String>>,
        repo_mappings: Arc<RepoMappingSnapshot>,
        repo_mapping_overrides: Arc<RepoMappingOverrides>,
    ) -> Self {
        Self {
            extension_id,
            input_hash,
            aggregated,
            root_module_name,
            project_root: Some(project_root),
            hidden_lockfile_path,
            lockfile_mode,
            repo_env,
            repo_mappings,
            repo_mapping_overrides,
        }
    }

    /// Create a minimal key (for testing or when aggregated data is not available).
    /// This is primarily for backward compatibility with tests.
    pub fn new_minimal(extension_id: String, input_hash: String) -> Self {
        Self {
            extension_id: Arc::from(extension_id.as_str()),
            input_hash: Arc::from(input_hash.as_str()),
            aggregated: Arc::new(AggregatedExtension::default()),
            root_module_name: Arc::from("_main"),
            project_root: None,
            hidden_lockfile_path: None,
            lockfile_mode: LockfileMode::Update,
            repo_env: Arc::new(BTreeMap::new()),
            repo_mappings: Arc::new(RepoMappingSnapshot::new()),
            repo_mapping_overrides: Arc::new(RepoMappingOverrides::new()),
        }
    }

    /// Get the aggregated extension data.
    pub fn aggregated(&self) -> &AggregatedExtension {
        &self.aggregated
    }

    /// Get the root module name.
    pub fn root_module_name(&self) -> &str {
        &self.root_module_name
    }

    /// Get the project root (if set for lockfile support).
    pub fn project_root(&self) -> Option<&PathBuf> {
        self.project_root.as_ref().map(|p| p.as_ref())
    }

    fn workspace_id_for_lockfile(&self) -> Option<WorkspaceId> {
        self.project_root.as_ref().map(|project_root| {
            WorkspaceId::new(
                project_root.as_ref().clone(),
                project_root.join("buck-out/v2"),
            )
        })
    }
}

fn empty_facts() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn facts_for_message(facts: &serde_json::Value) -> String {
    serde_json::to_string(facts).unwrap_or_else(|_| facts.to_string())
}

async fn compute_lockfile_content(
    ctx: &mut DiceComputations<'_>,
    key: &LockfileContentKey,
    label: &str,
) -> slug_error::Result<Arc<crate::dice_graph::LockfileContentValue>> {
    match ctx.compute(key).await {
        Ok(result) => result,
        Err(e) => Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "DICE compute failed for {label} '{}': {}",
            key.path.display(),
            e
        )),
    }
}

fn validate_error_mode_facts(
    extension_id: &str,
    lockfile_mode: LockfileMode,
    new_facts: &serde_json::Value,
    workspace_lockfile_facts: &serde_json::Value,
) -> Result<(), ModuleExtensionError> {
    if lockfile_mode == LockfileMode::Error && new_facts != workspace_lockfile_facts {
        return Err(ModuleExtensionError::OutdatedLockfile {
            reason: format!(
                "the extension '{}' has changed its facts: {} != {}",
                extension_id,
                facts_for_message(new_facts),
                facts_for_message(workspace_lockfile_facts),
            ),
        });
    }

    Ok(())
}

#[async_trait]
impl Key for ModuleExtensionExecutionKey {
    type Value = slug_error::Result<Arc<ModuleExtensionResult>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        tracing::info!(
            "Evaluating module extension '{}' (input_hash: {})",
            self.extension_id,
            self.input_hash
        );

        // Compute digests for lockfile cache validation. For workspace-local
        // extensions, include the extension .bzl file and literal transitive
        // load() dependencies so edits reject stale replay.
        let bzl_transitive_digest = self
            .project_root
            .as_deref()
            .map(|project_root| {
                compute_bzl_transitive_digest_for_project(&self.extension_id, project_root)
            })
            .unwrap_or_else(|| compute_bzl_transitive_digest(&self.extension_id));
        let usages_digest = self.input_hash.to_string();
        let mut prior_facts = empty_facts();
        let mut workspace_lockfile_facts = empty_facts();
        let mut workspace_lockfile_facts_present = false;

        // 1. Read lockfile facts through the Plan 61 lockfile content key if
        //    project_root is set. This is read-only; normal extension replay
        //    must not mutate the visible lockfile.
        if self.lockfile_mode != LockfileMode::Off
            && let (Some(project_root), Some(workspace_id)) =
                (&self.project_root, self.workspace_id_for_lockfile())
        {
            let lockfile_key = LockfileContentKey {
                workspace_id,
                kind: LockfileContentKind::Workspace,
                path: Arc::new(crate::lockfile::lockfile_path(project_root)),
            };
            let lockfile_content =
                compute_lockfile_content(ctx, &lockfile_key, "workspace lockfile").await?;
            if let Some(lockfile) = lockfile_content.lockfile.as_ref() {
                if let Some(facts) = lockfile.get_extension_facts(&self.extension_id) {
                    prior_facts = facts.clone();
                    workspace_lockfile_facts = facts;
                    workspace_lockfile_facts_present = true;
                }
                if let Some(cached_specs) = lockfile.get_extension_cache_for_workspace(
                    &self.extension_id,
                    &bzl_transitive_digest,
                    &usages_digest,
                    Some(project_root),
                    Some(self.repo_env.as_ref()),
                    Some(self.repo_mappings.as_ref()),
                    Some(self.root_module_name()),
                    Some(self.repo_mapping_overrides.as_ref()),
                ) {
                    tracing::info!(
                        "Extension '{}' cache HIT: using {} cached repo specs",
                        self.extension_id,
                        cached_specs.len()
                    );

                    let result = ModuleExtensionResult::new_with_metadata(
                        self.extension_id.clone(),
                        self.input_hash.to_string(),
                        cached_specs,
                        &self.root_module_name,
                        ModuleExtensionMetadata {
                            facts: prior_facts.clone(),
                        },
                    );

                    return Ok(Arc::new(result));
                } else {
                    record_bzlmod_event(
                        BzlmodEventKind::ExtensionReplayMissReason,
                        format!("{}:digest_or_entry_miss", self.extension_id),
                    );
                    tracing::debug!(
                        "Extension '{}' cache MISS: digests don't match",
                        self.extension_id
                    );
                }
            } else {
                record_bzlmod_event(
                    BzlmodEventKind::ExtensionReplayMissReason,
                    format!("{}:lockfile_absent_or_unreadable", self.extension_id),
                );
            }
        }
        if self.lockfile_mode != LockfileMode::Off
            && let (Some(hidden_lockfile_path), Some(workspace_id)) =
                (&self.hidden_lockfile_path, self.workspace_id_for_lockfile())
        {
            let hidden_lockfile_key = LockfileContentKey {
                workspace_id,
                kind: LockfileContentKind::Hidden,
                path: hidden_lockfile_path.clone(),
            };
            let lockfile_content =
                compute_lockfile_content(ctx, &hidden_lockfile_key, "hidden lockfile").await?;
            if let Some(lockfile) = lockfile_content.lockfile.as_ref() {
                if !workspace_lockfile_facts_present {
                    prior_facts = lockfile
                        .get_extension_facts(&self.extension_id)
                        .unwrap_or_else(empty_facts);
                }
                if let Some(cached_specs) = lockfile.get_extension_cache_for_workspace(
                    &self.extension_id,
                    &bzl_transitive_digest,
                    &usages_digest,
                    self.project_root.as_deref().map(|p| p.as_path()),
                    Some(self.repo_env.as_ref()),
                    Some(self.repo_mappings.as_ref()),
                    Some(self.root_module_name()),
                    Some(self.repo_mapping_overrides.as_ref()),
                ) {
                    tracing::info!(
                        "Extension '{}' hidden lockfile cache HIT: using {} cached repo specs",
                        self.extension_id,
                        cached_specs.len()
                    );

                    let result = ModuleExtensionResult::new_with_metadata(
                        self.extension_id.clone(),
                        self.input_hash.to_string(),
                        cached_specs,
                        &self.root_module_name,
                        ModuleExtensionMetadata {
                            facts: prior_facts.clone(),
                        },
                    );

                    return Ok(Arc::new(result));
                }
            }
        }

        // Log the modules that use this extension
        let module_count = self.aggregated.tags_by_module.len();
        let tag_count: usize = self
            .aggregated
            .tags_by_module
            .values()
            .map(|v| v.len())
            .sum();
        tracing::debug!(
            "Extension '{}' used by {} module(s) with {} total tag(s)",
            self.extension_id,
            module_count,
            tag_count
        );

        // 2. Create temporary working directory for module_ctx I/O
        record_bzlmod_event(BzlmodEventKind::ExtensionEval, self.extension_id.as_ref());
        let temp_dir = create_temp_extension_dir(&self.extension_id)?;

        // 3-5. Execute extension via late binding to slug_interpreter_for_build
        //
        // The late binding pattern allows us to call into slug_interpreter_for_build
        // without a direct dependency. The implementation:
        // - Loads the extension's .bzl file via Starlark interpreter
        // - Builds module_ctx from aggregated tags using build_module_context()
        // - Executes extension.implementation(module_ctx) in Starlark
        // - Captures RepoSpecs from repository rule invocations
        let execution_result = match MODULE_EXTENSION_EXECUTOR_IMPL.get() {
            Ok(executor) => {
                executor
                    .execute_extension(
                        ctx,
                        &self.aggregated,
                        &self.root_module_name,
                        &temp_dir,
                        prior_facts,
                    )
                    .await
            }
            Err(e) => Err(ModuleExtensionError::ExecutionFailed {
                extension_id: self.extension_id.to_string(),
                reason: format!("module extension executor is not initialized: {e}"),
            }
            .into()),
        };

        // 6. Clean up temporary working directory
        if temp_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&temp_dir) {
                tracing::warn!(
                    "Failed to clean up temp dir for extension '{}': {}",
                    self.extension_id,
                    e
                );
            }
        }

        // Check for execution errors
        let output = execution_result?;
        validate_error_mode_facts(
            &self.extension_id,
            self.lockfile_mode,
            &output.metadata.facts,
            &workspace_lockfile_facts,
        )?;

        // 7. Build result with canonical names
        let result = ModuleExtensionResult::new_with_metadata(
            self.extension_id.clone(),
            self.input_hash.to_string(),
            output.generated_repo_specs.clone(),
            &self.root_module_name,
            output.metadata.clone(),
        );

        tracing::info!(
            "Extension '{}' generated {} repository specs",
            self.extension_id,
            result.repo_count()
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
        // Don't cache errors - retry on next request
        x.is_ok()
    }
}

/// Create a temporary working directory for extension execution.
///
/// The directory is created under the system temp directory with a name
/// derived from the extension ID. This directory is for `module_ctx` I/O
/// and is deleted after the extension completes.
fn create_temp_extension_dir(extension_id: &str) -> slug_error::Result<PathBuf> {
    // Sanitize extension ID for use in path
    let sanitized = sanitize_extension_id_for_path(extension_id);

    let temp_base = std::env::temp_dir().join("slug-extension");
    std::fs::create_dir_all(&temp_base).map_err(|e| ModuleExtensionError::TempDirFailed {
        extension_id: extension_id.to_owned(),
        reason: format!("failed to create temp base: {}", e),
    })?;

    let temp_dir = temp_base.join(&sanitized);

    // Clean up any previous temp dir for this extension
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    std::fs::create_dir_all(&temp_dir).map_err(|e| ModuleExtensionError::TempDirFailed {
        extension_id: extension_id.to_owned(),
        reason: format!("failed to create temp dir: {}", e),
    })?;

    Ok(temp_dir)
}

/// Sanitize an extension ID for use in a filesystem path.
///
/// Replaces characters that are problematic in paths with underscores.
fn sanitize_extension_id_for_path(extension_id: &str) -> String {
    extension_id
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '@' | '%' | ' ' => '_',
            c if c.is_alphanumeric() || c == '_' || c == '-' || c == '+' || c == '.' => c,
            _ => '_',
        })
        .collect()
}

/// Build canonical names for extension-generated repositories.
///
/// Format: `{owning_module}+{extension_unique_name}+{internal_name}`
/// where `owning_module` is `_main` for extensions defined in the root module
/// (Bazel 9 convention; see `extract_owning_module`) and the canonical module
/// repo name otherwise. The separator is `+`, so non-root module extensions can
/// have Bazel's `rules_rs++crate+repo` shape.
///
/// `root_module_name` is the value of `module(name=…)` in the root MODULE.bazel
/// — required so the root module's declared name (e.g., `llvm-project-overlay`)
/// is canonicalized to `_main`, matching what `pending_repo_cells.rs` registers
/// when it pre-computes the same cells from `use_repo()` declarations.
pub fn build_canonical_names(
    extension_id: &str,
    specs: &FxHashMap<String, RepoSpec>,
    root_module_name: &str,
) -> FxHashMap<String, String> {
    let ext_name = extract_extension_name(extension_id);
    let owning_module = extract_owning_module(extension_id, root_module_name);
    specs
        .keys()
        .map(|internal| {
            let canonical = format!("{}+{}+{}", owning_module, ext_name, internal);
            (internal.clone(), canonical)
        })
        .collect()
}

/// Extract the canonical owning-module prefix from an extension ID.
///
/// Bazel's canonical naming convention prefixes the *root* module's extension
/// repos with the literal string `_main`, regardless of the name the root
/// module declares in MODULE.bazel. Non-root modules use their canonical module
/// repo name, including Bazel 9's trailing `+`.
/// `root_module_name` is the value of `module(name=…)` in the root MODULE.bazel
/// (e.g., `llvm-project-overlay`); pass `""` if the build has no root module.
///
/// Without this substitution, the root module's own extension defined at
/// `@<root_module>//ext.bzl` would be canonicalized as
/// `<root_module>+ext+repo` while `pending_repo_cells.rs` (using the
/// `_main`-rule) registers it as `_main+ext+repo`. The two paths point to
/// different `bazel-external/...` directories and the build fails with
/// "package not found" once the repo rule tries to read its own files.
///
/// Extension ID formats:
/// - `@bazel_features//private:extensions.bzl%version_extension` → `bazel_features+`
/// - `@@rules_cc+//cc:extensions.bzl%cc_configure` → `rules_cc+`
/// - `//path:file.bzl%ext` → `_main` (root module, no repo prefix)
/// - `@<root_module_name>//path:file.bzl%ext` → `_main` (root module via its declared name)
///
/// Falls back to `_main` if the format doesn't match.
pub fn extract_owning_module(extension_id: &str, root_module_name: &str) -> String {
    // Strip the extension name part (after %)
    let bzl_part = extension_id.split('%').next().unwrap_or(extension_id);

    // Look for @module// or @@module// pattern.
    //
    // The module segment may appear in two shapes:
    //   - slug internal:        `@<apparent>//...`        → module = "<apparent>"
    //   - bazel 9 canonical:    `@@<repo>+//...`          → module = "<repo>+"
    // Bazel 9 includes the trailing `+` in canonical module repository names,
    // and extension-generated repositories preserve it before adding their own
    // `+<extension>+<repo>` suffix.
    let stripped = bzl_part
        .strip_prefix("@@")
        .or_else(|| bzl_part.strip_prefix('@'))
        .unwrap_or(bzl_part);
    if let Some(pos) = stripped.find("//") {
        let module = &stripped[..pos];
        if !module.is_empty() {
            // Map the root module's declared name back to Bazel's canonical
            // `_main` placeholder so callers all agree on one canonical
            // prefix per repo, no matter which spelling of the root module
            // they observe in extension IDs / Starlark labels.
            if !root_module_name.is_empty() && module == root_module_name {
                return "_main".to_owned();
            }
            if module.ends_with('+') {
                return module.to_owned();
            }
            return format!("{module}+");
        }
    }

    // No module prefix (e.g., "//path:file.bzl") means root module
    "_main".to_owned()
}

/// Extract the extension name from an extension ID.
///
/// Extension ID format: `@@module//path:file.bzl%extension_name`
/// Returns the `extension_name` part.
///
/// If the format doesn't match, returns the entire ID (sanitized).
pub fn extract_extension_name(extension_id: &str) -> String {
    // Look for %extension_name at the end
    if let Some(pos) = extension_id.rfind('%') {
        extension_id[pos + 1..].to_owned()
    } else if let Some(pos) = extension_id.rfind(':') {
        // Fallback: try to use the bzl file name without extension
        let after_colon = &extension_id[pos + 1..];
        after_colon
            .strip_suffix(".bzl")
            .unwrap_or(after_colon)
            .to_owned()
    } else {
        // Last resort: use the whole thing, sanitized
        extension_id
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect()
    }
}

/// Compute a transitive digest for the extension's .bzl files.
///
/// Ideally, this would hash all .bzl files that the extension transitively depends on.
/// For now, we use a simplified approach that hashes the extension ID. This provides
/// basic cache invalidation when the extension changes but doesn't capture all
/// transitive .bzl file changes.
///
/// TODO: Improve this by integrating with the Starlark module loading system
/// to get the actual transitive digest of all loaded .bzl files.
pub fn compute_bzl_transitive_digest(extension_id: &str) -> String {
    use base64::Engine;
    use sha2::Digest;
    use sha2::Sha256;

    let mut hasher = Sha256::new();
    hasher.update(b"bzl_transitive_v1:");
    hasher.update(extension_id.as_bytes());

    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}

/// Compute a best-effort Bazel-shaped transitive digest for workspace-local
/// extension `.bzl` files.
///
/// Bazel computes this from the loaded module graph. Slug does not yet expose
/// that graph at this layer, so this function hashes files that can be resolved
/// under `project_root` from literal `load()` statements. If the extension file
/// cannot be resolved locally, it falls back to the old extension-id digest so
/// external/registry cases keep their existing behavior until 61.6 owns the
/// full Starlark load graph.
pub fn compute_bzl_transitive_digest_for_project(
    extension_id: &str,
    project_root: &Path,
) -> String {
    let Some(root_bzl) = extension_bzl_path_under_project(extension_id, project_root) else {
        return compute_bzl_transitive_digest(extension_id);
    };
    if !root_bzl.is_file() {
        return compute_bzl_transitive_digest(extension_id);
    }

    let mut seen = std::collections::BTreeSet::new();
    collect_bzl_transitive_files(project_root, &root_bzl, &mut seen);
    if seen.is_empty() {
        return compute_bzl_transitive_digest(extension_id);
    }

    use base64::Engine;
    use sha2::Digest;
    use sha2::Sha256;

    let mut hasher = Sha256::new();
    hasher.update(b"bzl_transitive_v2:");
    hasher.update(extension_id.as_bytes());
    hasher.update([0]);
    for path in seen {
        let rel = path.strip_prefix(project_root).unwrap_or(path.as_path());
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update([0]);
        match std::fs::read(&path) {
            Ok(content) => hasher.update(content),
            Err(e) => {
                hasher.update(b"read_error:");
                hasher.update(e.to_string().as_bytes());
            }
        }
        hasher.update([0]);
    }

    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}

fn extension_bzl_path_under_project(extension_id: &str, project_root: &Path) -> Option<PathBuf> {
    let label = extension_id.split('%').next().unwrap_or(extension_id);
    label_bzl_path_under_project(label, project_root, None)
}

fn label_bzl_path_under_project(
    label: &str,
    project_root: &Path,
    current_dir: Option<&Path>,
) -> Option<PathBuf> {
    let without_repo = if let Some(rest) = label.strip_prefix("@@") {
        rest.split_once("//").map(|(_, target)| target)?
    } else if let Some(rest) = label.strip_prefix('@') {
        rest.split_once("//").map(|(_, target)| target)?
    } else if let Some(rest) = label.strip_prefix("//") {
        rest
    } else if let Some(name) = label.strip_prefix(':') {
        return current_dir.map(|dir| dir.join(name));
    } else if label.contains("//") {
        label.split_once("//").map(|(_, target)| target)?
    } else {
        return current_dir.map(|dir| dir.join(label));
    };

    let (pkg, name) = without_repo.split_once(':')?;
    let mut path = project_root.to_path_buf();
    if !pkg.is_empty() {
        path.push(pkg);
    }
    path.push(name);
    Some(path)
}

fn collect_bzl_transitive_files(
    project_root: &Path,
    path: &Path,
    seen: &mut std::collections::BTreeSet<PathBuf>,
) {
    let path = path.to_path_buf();
    if !seen.insert(path.clone()) {
        return;
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };
    for load in literal_loads(&path, &content) {
        let Some(load_path) = label_bzl_path_under_project(&load, project_root, path.parent())
        else {
            continue;
        };
        if load_path.starts_with(project_root) && load_path.is_file() {
            collect_bzl_transitive_files(project_root, &load_path, seen);
        }
    }
}

fn literal_loads(path: &Path, content: &str) -> Vec<String> {
    let filename = path.to_string_lossy().into_owned();
    let Ok(ast) = AstModule::parse(&filename, content.to_owned(), &Dialect::Standard) else {
        return Vec::new();
    };
    let mut loads = Vec::new();
    collect_literal_loads_from_stmt(ast.statement(), &mut loads);
    loads
}

fn collect_literal_loads_from_stmt(stmt: &AstStmt, loads: &mut Vec<String>) {
    match &stmt.node {
        StmtP::Statements(stmts) => {
            for stmt in stmts {
                collect_literal_loads_from_stmt(stmt, loads);
            }
        }
        StmtP::Load(load) => loads.push(load.module.node.clone()),
        StmtP::If(_, body) => collect_literal_loads_from_stmt(body, loads),
        StmtP::IfElse(_, branches) => {
            collect_literal_loads_from_stmt(&branches.0, loads);
            collect_literal_loads_from_stmt(&branches.1, loads);
        }
        StmtP::For(for_stmt) => collect_literal_loads_from_stmt(&for_stmt.body, loads),
        StmtP::Def(def) => collect_literal_loads_from_stmt(&def.body, loads),
        StmtP::Break
        | StmtP::Continue
        | StmtP::Pass
        | StmtP::Return(_)
        | StmtP::Expression(_)
        | StmtP::Assign(_)
        | StmtP::AssignModify(_, _, _) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository_invocations::AttrValue;

    #[test]
    fn test_module_extension_result_creation() {
        let mut specs = FxHashMap::default();
        specs.insert(
            "numpy".to_owned(),
            RepoSpec::new("@@rules_python//...%pip_install".to_owned())
                .with_attr("version".to_owned(), AttrValue::String("1.24.0".to_owned())),
        );
        specs.insert(
            "requests".to_owned(),
            RepoSpec::new("@@rules_python//...%pip_install".to_owned())
                .with_attr("version".to_owned(), AttrValue::String("2.31.0".to_owned())),
        );

        let result = ModuleExtensionResult::new(
            Arc::from("@@rules_python//python/pip:pip.bzl%pip"),
            "sha256-abc123".to_owned(),
            specs,
            "",
        );

        assert_eq!(
            result.extension_id.as_ref(),
            "@@rules_python//python/pip:pip.bzl%pip"
        );
        assert_eq!(result.input_hash, "sha256-abc123");
        assert_eq!(result.repo_count(), 2);
        assert!(result.contains_repo("numpy"));
        assert!(result.contains_repo("requests"));
        assert!(!result.contains_repo("pandas"));
    }

    #[test]
    fn test_canonical_name_lookup() {
        let mut specs = FxHashMap::default();
        specs.insert("foo".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("bar".to_owned(), RepoSpec::new("rule".to_owned()));

        let result = ModuleExtensionResult::new(
            Arc::from("@@module//path:ext.bzl%my_extension"),
            "hash".to_owned(),
            specs,
            "",
        );

        assert_eq!(
            result.canonical_name("foo"),
            Some("module++my_extension+foo")
        );
        assert_eq!(
            result.canonical_name("bar"),
            Some("module++my_extension+bar")
        );
        assert_eq!(result.canonical_name("baz"), None);
    }

    #[test]
    fn test_internal_name_from_canonical() {
        let mut specs = FxHashMap::default();
        specs.insert("numpy".to_owned(), RepoSpec::new("rule".to_owned()));

        let result = ModuleExtensionResult::new(
            Arc::from("@@rules_python//pip:pip.bzl%pip"),
            "hash".to_owned(),
            specs,
            "",
        );

        assert_eq!(
            result.internal_name_from_canonical("rules_python++pip+numpy"),
            Some("numpy")
        );
        assert_eq!(
            result.internal_name_from_canonical("rules_python++pip+pandas"),
            None
        );
    }

    #[test]
    fn test_extract_extension_name() {
        assert_eq!(
            extract_extension_name("@@rules_python//pip:pip.bzl%pip"),
            "pip"
        );
        assert_eq!(
            extract_extension_name("@@bazel_features//private:extensions.bzl%bazel_features"),
            "bazel_features"
        );
        assert_eq!(
            extract_extension_name("//:my_extension.bzl%my_ext"),
            "my_ext"
        );
        // Fallback cases
        assert_eq!(extract_extension_name("//:extension.bzl"), "extension");
        assert_eq!(extract_extension_name("simple_name"), "simple_name");
    }

    #[test]
    fn test_build_canonical_names() {
        let mut specs = FxHashMap::default();
        specs.insert("numpy".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("pandas".to_owned(), RepoSpec::new("rule".to_owned()));

        let names = build_canonical_names("@@rules_python//pip:pip.bzl%pip", &specs, "");

        assert_eq!(
            names.get("numpy"),
            Some(&"rules_python++pip+numpy".to_owned())
        );
        assert_eq!(
            names.get("pandas"),
            Some(&"rules_python++pip+pandas".to_owned())
        );
    }

    #[test]
    fn test_sanitize_extension_id() {
        assert_eq!(
            sanitize_extension_id_for_path("@@module//path:file.bzl%ext"),
            "__module__path_file.bzl_ext"
        );
        assert_eq!(sanitize_extension_id_for_path("simple_name"), "simple_name");
        assert_eq!(
            sanitize_extension_id_for_path("name with spaces"),
            "name_with_spaces"
        );
    }

    #[test]
    fn test_module_extension_key_creation() {
        use crate::extensions::AggregatedExtension;

        let mut aggregated = AggregatedExtension::new("@@module//ext.bzl", "test");
        aggregated.add_module_tags("root", vec![]);

        let key = ModuleExtensionExecutionKey::new(aggregated, "_main".to_owned());

        assert_eq!(key.extension_id.as_ref(), "@@module//ext.bzl%test");
        assert_eq!(
            base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                key.input_hash.as_ref()
            )
            .unwrap()
            .len(),
            32
        );
        assert_eq!(key.root_module_name.as_ref(), "_main");
    }

    #[test]
    fn test_module_extension_key_minimal() {
        let key = ModuleExtensionExecutionKey::new_minimal(
            "@@module//ext.bzl%test".to_owned(),
            "sha256-abc".to_owned(),
        );

        assert_eq!(key.extension_id.as_ref(), "@@module//ext.bzl%test");
        assert_eq!(key.input_hash.as_ref(), "sha256-abc");
        assert_eq!(key.root_module_name.as_ref(), "_main");
    }

    #[test]
    fn test_module_extension_key_display() {
        let key = ModuleExtensionExecutionKey::new_minimal(
            "@@m//e.bzl%x".to_owned(),
            "hash123".to_owned(),
        );

        let display = format!("{}", key);
        assert_eq!(display, "ModuleExtensionKey(@@m//e.bzl%x, hash123)");
    }

    #[test]
    fn test_module_extension_key_with_tags() {
        use crate::extensions::AggregatedExtension;
        use crate::types::ExtensionTag;
        use crate::types::TagValue;

        let mut aggregated = AggregatedExtension::new("@@rules_python//pip:pip.bzl", "pip");

        let mut parse_tag = ExtensionTag::new("parse".to_owned());
        parse_tag
            .kwargs
            .push(("hub_name".to_owned(), TagValue::String("pip".to_owned())));

        let mut install_tag = ExtensionTag::new("install".to_owned());
        install_tag
            .kwargs
            .push(("name".to_owned(), TagValue::String("numpy".to_owned())));

        aggregated.add_module_tags("root", vec![parse_tag]);
        aggregated.add_module_tags("dep_a", vec![install_tag]);

        let key = ModuleExtensionExecutionKey::new(aggregated, "root".to_owned());

        assert_eq!(key.extension_id.as_ref(), "@@rules_python//pip:pip.bzl%pip");
        assert_eq!(key.root_module_name.as_ref(), "root");
        assert_eq!(key.aggregated().tags_by_module.len(), 2);
    }

    #[test]
    fn test_module_extension_key_hash_eq() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

        use crate::extensions::AggregatedExtension;

        let aggregated1 = AggregatedExtension::new("@@mod//ext.bzl", "ext");
        let aggregated2 = AggregatedExtension::new("@@mod//ext.bzl", "ext");

        let key1 = ModuleExtensionExecutionKey::new(aggregated1, "_main".to_owned());
        let key2 = ModuleExtensionExecutionKey::new(aggregated2, "_main".to_owned());

        // Keys with same aggregated data should be equal
        assert_eq!(key1, key2);

        // Keys with same aggregated data should have same hash
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        key2.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_get_repo_spec() {
        let mut specs = FxHashMap::default();
        specs.insert(
            "test_repo".to_owned(),
            RepoSpec::new("@@bazel_tools//repo:http.bzl%http_archive".to_owned()).with_attr(
                "url".to_owned(),
                AttrValue::String("https://example.com".to_owned()),
            ),
        );

        let result =
            ModuleExtensionResult::new(Arc::from("@@//ext.bzl%test"), "hash".to_owned(), specs, "");

        let spec = result.get_repo_spec("test_repo").unwrap();
        assert_eq!(
            spec.repo_rule_id,
            "@@bazel_tools//repo:http.bzl%http_archive"
        );
        assert!(result.get_repo_spec("nonexistent").is_none());
    }

    #[test]
    fn test_module_extension_result_carries_facts_metadata() {
        let mut specs = FxHashMap::default();
        specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));

        let result = ModuleExtensionResult::new_with_metadata(
            Arc::from("@@//ext.bzl%test"),
            "hash".to_owned(),
            specs,
            "",
            ModuleExtensionMetadata {
                facts: serde_json::json!({"resource": {"checksum": "abc"}}),
            },
        );

        assert_eq!(
            result.metadata.facts,
            serde_json::json!({"resource": {"checksum": "abc"}})
        );
    }

    #[test]
    fn test_error_mode_facts_accepts_matching_workspace_facts() {
        validate_error_mode_facts(
            "@@mod+//:ext.bzl%ext",
            LockfileMode::Error,
            &serde_json::json!({"resource": {"checksum": "abc"}}),
            &serde_json::json!({"resource": {"checksum": "abc"}}),
        )
        .unwrap();
    }

    #[test]
    fn test_error_mode_facts_rejects_changed_workspace_facts() {
        let err = validate_error_mode_facts(
            "@@mod+//:ext.bzl%ext",
            LockfileMode::Error,
            &serde_json::json!({"resource": {"checksum": "new"}}),
            &serde_json::json!({"resource": {"checksum": "old"}}),
        )
        .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("MODULE.bazel.lock is no longer up-to-date"));
        assert!(message.contains("the extension '@@mod+//:ext.bzl%ext' has changed its facts"));
        assert!(message.contains(r#""checksum":"new""#));
        assert!(message.contains(r#""checksum":"old""#));
        assert!(message.contains("bazel mod deps --lockfile_mode=update"));
    }

    #[test]
    fn test_error_mode_facts_rejects_new_facts_when_workspace_facts_absent() {
        let err = validate_error_mode_facts(
            "@@mod+//:ext.bzl%ext",
            LockfileMode::Error,
            &serde_json::json!({"resource": "new"}),
            &empty_facts(),
        )
        .unwrap_err();

        assert!(err.to_string().contains(r#"{"resource":"new"} != {}"#));
    }

    #[test]
    fn test_repo_names_iterator() {
        let mut specs = FxHashMap::default();
        specs.insert("a".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("b".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("c".to_owned(), RepoSpec::new("rule".to_owned()));

        let result =
            ModuleExtensionResult::new(Arc::from("@@//ext.bzl%test"), "hash".to_owned(), specs, "");

        let mut names: Vec<_> = result.repo_names().collect();
        names.sort();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    // =========================================================================
    // Lockfile Integration Tests
    // =========================================================================

    #[test]
    fn test_compute_bzl_transitive_digest() {
        let digest1 = compute_bzl_transitive_digest("@@module//ext.bzl%test");
        let digest2 = compute_bzl_transitive_digest("@@module//ext.bzl%test");
        let digest3 = compute_bzl_transitive_digest("@@other//ext.bzl%test");

        // Same extension ID should produce same digest
        assert_eq!(digest1, digest2);
        // Different extension ID should produce different digest
        assert_ne!(digest1, digest3);
        assert_eq!(
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, digest1)
                .unwrap()
                .len(),
            32
        );
    }

    #[test]
    fn test_lockfile_project_root_reads_without_writing() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("MODULE.bazel.lock");
        let mut lockfile = crate::lockfile::Lockfile::new();
        lockfile.registry_file_hashes.insert(
            "https://bcr.bazel.build/test".to_owned(),
            "sha256-abc".to_owned(),
        );
        lockfile.write(&lock_path).unwrap();
        let before = std::fs::read(&lock_path).unwrap();

        let cached =
            crate::lockfile::read_lockfile_with_mode(temp_dir.path(), LockfileMode::Update)
                .unwrap()
                .unwrap();

        assert_eq!(cached.registry_file_hashes.len(), 1);
        assert_eq!(std::fs::read(&lock_path).unwrap(), before);
    }

    #[test]
    fn test_new_with_lockfile_constructor() {
        use crate::extensions::AggregatedExtension;

        let aggregated = AggregatedExtension::new("@@module//ext.bzl", "test");
        let key = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated,
            "_main".to_owned(),
            PathBuf::from("/tmp/project"),
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );

        assert_eq!(key.extension_id.as_ref(), "@@module//ext.bzl%test");
        assert!(key.project_root.is_some());
        assert_eq!(key.project_root().unwrap(), &PathBuf::from("/tmp/project"));
    }

    #[test]
    fn test_project_root_is_in_hash_and_eq() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

        use crate::extensions::AggregatedExtension;

        let aggregated1 = AggregatedExtension::new("@@mod//ext.bzl", "ext");
        let aggregated2 = AggregatedExtension::new("@@mod//ext.bzl", "ext");

        let key1 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated1,
            "_main".to_owned(),
            PathBuf::from("/project1"),
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );
        let key2 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated2,
            "_main".to_owned(),
            PathBuf::from("/project2"),
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );

        assert_ne!(key1, key2);

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        key2.hash(&mut hasher2);
        assert_ne!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_from_arcs_with_lockfile() {
        use crate::extensions::AggregatedExtension;

        let extension_id = Arc::from("@@mod//ext.bzl%ext");
        let input_hash = Arc::from("sha256-abc");
        let aggregated = Arc::new(AggregatedExtension::new("@@mod//ext.bzl", "ext"));
        let root_module_name = Arc::from("_main");
        let project_root = Arc::new(PathBuf::from("/tmp/test"));

        let key = ModuleExtensionExecutionKey::from_arcs_with_lockfile(
            extension_id,
            input_hash,
            aggregated,
            root_module_name,
            project_root,
            None,
            LockfileMode::Update,
            Arc::new(BTreeMap::new()),
            Arc::new(crate::RepoMappingSnapshot::new()),
            Arc::new(crate::RepoMappingOverrides::new()),
        );

        assert!(key.project_root.is_some());
        assert_eq!(key.project_root().unwrap(), &PathBuf::from("/tmp/test"));
    }

    #[test]
    fn test_lockfile_mode_is_in_hash_and_eq() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

        use crate::extensions::AggregatedExtension;

        let aggregated1 = AggregatedExtension::new("@@mod//ext.bzl", "ext");
        let aggregated2 = AggregatedExtension::new("@@mod//ext.bzl", "ext");

        let key1 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated1,
            "_main".to_owned(),
            PathBuf::from("/project"),
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );
        let key2 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated2,
            "_main".to_owned(),
            PathBuf::from("/project"),
            None,
            LockfileMode::Off,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );

        assert_ne!(key1, key2);

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        key2.hash(&mut hasher2);
        assert_ne!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_key_without_lockfile_has_no_project_root() {
        use crate::extensions::AggregatedExtension;

        let aggregated = AggregatedExtension::new("@@mod//ext.bzl", "ext");
        let key = ModuleExtensionExecutionKey::new(aggregated, "_main".to_owned());

        assert!(key.project_root.is_none());
        assert!(key.project_root().is_none());
    }
}
