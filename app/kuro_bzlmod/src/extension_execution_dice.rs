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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use fxhash::FxHashMap;

use crate::extensions::AggregatedExtension;
use crate::extensions::compute_extension_input_hash;
use crate::lockfile::Lockfile;
use crate::lockfile::lockfile_path;
use crate::module_extension_executor::MODULE_EXTENSION_EXECUTOR_IMPL;

const MAX_EXTENSION_IDS_IN_WARNING: usize = 25;

/// Global storage for extension aggregation data, populated during cell resolution.
///
/// This data is needed when extension repos are lazily executed inside DICE.
/// It contains the aggregated tags from all modules for each extension,
/// plus the root module name and project root needed to create execution keys.
struct ExtensionAggregationData {
    aggregations: HashMap<String, AggregatedExtension>,
    root_module_name: String,
    project_root: PathBuf,
}

static EXTENSION_AGGREGATIONS: Mutex<Option<ExtensionAggregationData>> = Mutex::new(None);

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

/// Store aggregated extension data for later DICE-based execution.
///
/// Called during cell resolution after aggregating all extension usages
/// from MODULE.bazel files across the dependency graph.
pub fn set_extension_aggregations(
    aggregations: HashMap<String, AggregatedExtension>,
    root_module_name: String,
    project_root: PathBuf,
) {
    let mut guard = EXTENSION_AGGREGATIONS.lock().unwrap();
    *guard = Some(ExtensionAggregationData {
        aggregations,
        root_module_name,
        project_root,
    });
}

/// Look up the aggregated extension data and create a `ModuleExtensionExecutionKey`.
///
/// Returns `None` if the extension is not found or aggregation data hasn't been set.
pub fn create_extension_execution_key(extension_id: &str) -> Option<ModuleExtensionExecutionKey> {
    let guard = EXTENSION_AGGREGATIONS.lock().unwrap();
    let data = match guard.as_ref() {
        Some(d) => d,
        None => {
            tracing::warn!(
                "create_extension_execution_key: EXTENSION_AGGREGATIONS not set (extension_id='{}')",
                extension_id
            );
            return None;
        }
    };
    let aggregated = match data.aggregations.get(extension_id) {
        Some(a) => a,
        None => {
            tracing::warn!(
                "create_extension_execution_key: extension '{}' not found in aggregations. Available: {}",
                extension_id,
                extension_ids_summary(data.aggregations.keys())
            );
            return None;
        }
    };
    Some(ModuleExtensionExecutionKey::new_with_lockfile(
        aggregated.clone(),
        data.root_module_name.clone(),
        data.project_root.clone(),
    ))
}
use crate::repo_spec::RepoSpec;

/// Errors during module extension execution.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
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
        let canonical_names =
            build_canonical_names(&extension_id, &generated_repo_specs, root_module_name);
        Self {
            extension_id,
            input_hash,
            generated_repo_specs,
            canonical_names,
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
/// 7. Updates lockfile with result (if project_root is set)
/// 8. Returns ModuleExtensionResult with captured specs
///
/// Note: NO downloads or repository materialization happens during this computation.
/// Repositories are materialized lazily via `ExtensionRepoExecutionKey`.
///
/// Note: Hash and Eq are implemented manually because `AggregatedExtension` contains
/// HashMap. The `input_hash` field is used for hashing, ensuring deterministic cache behavior.
/// The `project_root` field is intentionally excluded from Hash/Eq as it's runtime configuration.
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

    /// Project root for lockfile access (optional).
    /// If set, lockfile caching will be used.
    /// Excluded from Hash/Eq as it's runtime configuration.
    pub project_root: Option<Arc<PathBuf>>,
}

impl std::hash::Hash for ModuleExtensionExecutionKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the identifying fields; input_hash represents the aggregated data
        // Note: project_root is intentionally not hashed - it's runtime configuration
        self.extension_id.hash(state);
        self.input_hash.hash(state);
    }
}

impl PartialEq for ModuleExtensionExecutionKey {
    fn eq(&self, other: &Self) -> bool {
        // Compare by identifying fields; input_hash represents the aggregated data
        // Note: project_root is intentionally not compared - it's runtime configuration
        self.extension_id == other.extension_id && self.input_hash == other.input_hash
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
        }
    }

    /// Create a new extension execution key with lockfile support.
    pub fn new_with_lockfile(
        aggregated: AggregatedExtension,
        root_module_name: String,
        project_root: PathBuf,
    ) -> Self {
        let extension_id = Arc::from(aggregated.extension_id.as_str());
        let input_hash = Arc::from(compute_extension_input_hash(&aggregated).as_str());
        Self {
            extension_id,
            input_hash,
            aggregated: Arc::new(aggregated),
            root_module_name: Arc::from(root_module_name.as_str()),
            project_root: Some(Arc::new(project_root)),
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
        }
    }

    /// Create from Arc references with lockfile support.
    pub fn from_arcs_with_lockfile(
        extension_id: Arc<str>,
        input_hash: Arc<str>,
        aggregated: Arc<AggregatedExtension>,
        root_module_name: Arc<str>,
        project_root: Arc<PathBuf>,
    ) -> Self {
        Self {
            extension_id,
            input_hash,
            aggregated,
            root_module_name,
            project_root: Some(project_root),
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
}

#[async_trait]
impl Key for ModuleExtensionExecutionKey {
    type Value = kuro_error::Result<Arc<ModuleExtensionResult>>;

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

        // Compute digests for lockfile cache validation
        // Note: bzl_transitive_digest ideally hashes all .bzl files the extension depends on.
        // For now, we use a simpler approach based on extension_id. This can be improved
        // later when we have better access to the Starlark module dependency graph.
        let bzl_transitive_digest = compute_bzl_transitive_digest(&self.extension_id);
        let usages_digest = self.input_hash.to_string();

        // 1. Check lockfile cache (if project_root is set). Lockfile parse
        //    is shared with `cached_lockfile` callers (e.g. startup spoke
        //    seeding) so zeromatter-sized lockfiles only get parsed once.
        if let Some(project_root) = &self.project_root {
            if let Some(lockfile) = crate::lockfile::cached_lockfile(project_root) {
                if let Some(cached_specs) = lockfile.get_extension_cache(
                    &self.extension_id,
                    &bzl_transitive_digest,
                    &usages_digest,
                ) {
                    tracing::info!(
                        "Extension '{}' cache HIT: using {} cached repo specs",
                        self.extension_id,
                        cached_specs.len()
                    );

                    let result = ModuleExtensionResult::new(
                        self.extension_id.clone(),
                        self.input_hash.to_string(),
                        cached_specs,
                        &self.root_module_name,
                    );

                    return Ok(Arc::new(result));
                } else {
                    tracing::debug!(
                        "Extension '{}' cache MISS: digests don't match",
                        self.extension_id
                    );
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
        let temp_dir = create_temp_extension_dir(&self.extension_id)?;

        // 3-5. Execute extension via late binding to kuro_interpreter_for_build
        //
        // The late binding pattern allows us to call into kuro_interpreter_for_build
        // without a direct dependency. The implementation:
        // - Loads the extension's .bzl file via Starlark interpreter
        // - Builds module_ctx from aggregated tags using build_module_context()
        // - Executes extension.implementation(module_ctx) in Starlark
        // - Captures RepoSpecs from repository rule invocations
        let execution_result = match MODULE_EXTENSION_EXECUTOR_IMPL.get() {
            Ok(executor) => {
                executor
                    .execute_extension(ctx, &self.aggregated, &self.root_module_name, &temp_dir)
                    .await
            }
            Err(e) => {
                // Late binding not initialized - fall back to logging only (testing mode)
                tracing::warn!(
                    "MODULE_EXTENSION_EXECUTOR_IMPL not initialized: {}. \
                     Extension execution will be a no-op.",
                    e
                );
                tracing::debug!(
                    "Extension '{}' execution context (stub mode):",
                    self.extension_id
                );
                tracing::debug!("  - BZL file: {}", self.aggregated.extension_bzl_file);
                tracing::debug!("  - Extension name: {}", self.aggregated.extension_name);
                tracing::debug!("  - Root module: {}", self.root_module_name);
                tracing::debug!("  - Temp working dir: {:?}", temp_dir);
                tracing::debug!("  - Imported repos: {:?}", self.aggregated.imported_repos);

                // Log tags by module in stub mode
                for (module_name, tags) in &self.aggregated.tags_by_module {
                    tracing::debug!("  - Module '{}' tags:", module_name);
                    for tag in tags {
                        tracing::debug!("    - {}: {} kwarg(s)", tag.tag_name, tag.kwargs.len());
                    }
                }

                // Return empty result in stub mode
                Ok(crate::module_extension_executor::ExtensionExecutionOutput {
                    generated_repo_specs: FxHashMap::default(),
                })
            }
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

        // 7. Build result with canonical names
        let result = ModuleExtensionResult::new(
            self.extension_id.clone(),
            self.input_hash.to_string(),
            output.generated_repo_specs.clone(),
            &self.root_module_name,
        );

        tracing::info!(
            "Extension '{}' generated {} repository specs",
            self.extension_id,
            result.repo_count()
        );

        // 8. Update lockfile cache (if project_root is set and we have real specs)
        // Don't cache empty results — they likely indicate a failed extension execution
        // (graceful fallback), and caching them would poison future builds.
        if !output.generated_repo_specs.is_empty() {
            if let Some(project_root) = &self.project_root {
                let lock_path = lockfile_path(project_root);
                match update_lockfile_extension_cache(
                    &lock_path,
                    &self.extension_id,
                    &bzl_transitive_digest,
                    &usages_digest,
                    &output.generated_repo_specs,
                ) {
                    Ok(()) => {
                        tracing::debug!(
                            "Updated lockfile cache for extension '{}'",
                            self.extension_id
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to update lockfile cache for extension '{}': {}",
                            self.extension_id,
                            e
                        );
                    }
                }
            }
        }

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
fn create_temp_extension_dir(extension_id: &str) -> kuro_error::Result<PathBuf> {
    // Sanitize extension ID for use in path
    let sanitized = sanitize_extension_id_for_path(extension_id);

    let temp_base = std::env::temp_dir().join("kuro-extension");
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
/// (Bazel 9 convention; see `extract_owning_module`) and the declared module
/// name otherwise. The separator is `+`.
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
/// module declares in MODULE.bazel. Non-root modules use their declared name.
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
/// - `@bazel_features//private:extensions.bzl%version_extension` → `bazel_features`
/// - `@@rules_cc//cc:extensions.bzl%cc_configure` → `rules_cc`
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
    //   - kuro internal:        `@<apparent>//...`        → module = "<apparent>"
    //   - bazel 9 canonical:    `@@<repo>+//...`          → module = "<repo>+"
    // Strip the bazel-canonical trailing `+` so both shapes converge on the
    // same owning-module name (otherwise format!"{}+{}+{}" produces
    // `<repo>++<ext>+<repo>` for the canonical form).
    let stripped = bzl_part
        .strip_prefix("@@")
        .or_else(|| bzl_part.strip_prefix('@'))
        .unwrap_or(bzl_part);
    if let Some(pos) = stripped.find("//") {
        let module = &stripped[..pos];
        let module = module.strip_suffix('+').unwrap_or(module);
        if !module.is_empty() {
            // Map the root module's declared name back to Bazel's canonical
            // `_main` placeholder so callers all agree on one canonical
            // prefix per repo, no matter which spelling of the root module
            // they observe in extension IDs / Starlark labels.
            if !root_module_name.is_empty() && module == root_module_name {
                return "_main".to_owned();
            }
            return module.to_owned();
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
    format!(
        "sha256-{}",
        base64::engine::general_purpose::STANDARD.encode(hash)
    )
}

/// Update the lockfile with extension cache data.
///
/// This reads the existing lockfile (or creates a new one), adds/updates the
/// extension cache entry, and writes it back atomically.
fn update_lockfile_extension_cache(
    lock_path: &std::path::Path,
    extension_id: &str,
    bzl_transitive_digest: &str,
    usages_digest: &str,
    generated_repo_specs: &FxHashMap<String, RepoSpec>,
) -> kuro_error::Result<()> {
    // Read existing lockfile or create new one
    let mut lockfile = if lock_path.exists() {
        match Lockfile::read(lock_path) {
            Ok(lf) => lf,
            Err(_) => {
                // If we can't read it, start fresh
                tracing::debug!("Creating new lockfile for extension cache");
                Lockfile::new()
            }
        }
    } else {
        Lockfile::new()
    };

    // Drop legacy bare `//pkg:file%name` keys that pre-date kuro emitting
    // canonical `@@<repo>+//...` form. They're stale (no cell info) and
    // create duplicate entries on every write.
    lockfile
        .module_extensions
        .retain(|k, _| !k.starts_with("//") && !k.starts_with(':'));

    // Update the extension cache. Translate the internal extension id to
    // bazel's canonical `@@<repo>+//...` lockfile key form so the file
    // round-trips cleanly with `bazel mod`.
    let lockfile_key = crate::lockfile::lockfile_canonical_extension_id(extension_id);
    lockfile.set_extension_cache(
        lockfile_key,
        bzl_transitive_digest.to_owned(),
        usages_digest.to_owned(),
        generated_repo_specs,
    );

    // Write back
    lockfile.write(lock_path)?;

    // Drop the cached parse so subsequent `cached_lockfile` calls see the new
    // contents (e.g. the spec we just persisted).
    if let Some(parent) = lock_path.parent() {
        crate::lockfile::invalidate_cached_lockfile(parent);
    }

    Ok(())
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
            Some("module+my_extension+foo")
        );
        assert_eq!(
            result.canonical_name("bar"),
            Some("module+my_extension+bar")
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
            result.internal_name_from_canonical("rules_python+pip+numpy"),
            Some("numpy")
        );
        assert_eq!(
            result.internal_name_from_canonical("rules_python+pip+pandas"),
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
            Some(&"rules_python+pip+numpy".to_owned())
        );
        assert_eq!(
            names.get("pandas"),
            Some(&"rules_python+pip+pandas".to_owned())
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
        assert!(key.input_hash.starts_with("sha256-"));
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
        // Should be in SRI format
        assert!(digest1.starts_with("sha256-"));
    }

    #[test]
    fn test_update_lockfile_extension_cache() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("MODULE.bazel.lock");

        // Create test repo specs
        let mut specs = FxHashMap::default();
        specs.insert(
            "numpy".to_owned(),
            RepoSpec::new("@@rules_python//pip:pip.bzl%pip_install".to_owned())
                .with_attr("version".to_owned(), AttrValue::String("1.24.0".to_owned())),
        );

        // Update lockfile
        update_lockfile_extension_cache(
            &lock_path,
            "@@rules_python//pip:pip.bzl%pip",
            "bzl-digest-123",
            "usages-digest-456",
            &specs,
        )
        .unwrap();

        // Verify lockfile was created and contains the extension cache
        let lockfile = Lockfile::read(&lock_path).unwrap();
        assert!(lockfile.has_extension_cache());

        let cached = lockfile.get_extension_cache(
            "@@rules_python//pip:pip.bzl%pip",
            "bzl-digest-123",
            "usages-digest-456",
        );
        assert!(cached.is_some());
        let cached_specs = cached.unwrap();
        assert_eq!(cached_specs.len(), 1);
        assert!(cached_specs.contains_key("numpy"));
    }

    #[test]
    fn test_update_lockfile_preserves_existing_data() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("MODULE.bazel.lock");

        // Create initial lockfile with a registry hash
        let mut lockfile = Lockfile::new();
        lockfile.registry_file_hashes.insert(
            "https://bcr.bazel.build/test".to_owned(),
            "sha256-abc".to_owned(),
        );
        lockfile.write(&lock_path).unwrap();

        // Update with extension cache
        let specs = FxHashMap::default();
        update_lockfile_extension_cache(&lock_path, "@@ext//ext.bzl%ext", "bzl", "usages", &specs)
            .unwrap();

        // Verify existing data is preserved
        let lockfile = Lockfile::read(&lock_path).unwrap();
        assert!(
            lockfile
                .registry_file_hashes
                .contains_key("https://bcr.bazel.build/test")
        );
        assert!(lockfile.has_extension_cache());
    }

    #[test]
    fn test_new_with_lockfile_constructor() {
        use crate::extensions::AggregatedExtension;

        let aggregated = AggregatedExtension::new("@@module//ext.bzl", "test");
        let key = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated,
            "_main".to_owned(),
            PathBuf::from("/tmp/project"),
        );

        assert_eq!(key.extension_id.as_ref(), "@@module//ext.bzl%test");
        assert!(key.project_root.is_some());
        assert_eq!(key.project_root().unwrap(), &PathBuf::from("/tmp/project"));
    }

    #[test]
    fn test_project_root_not_in_hash_or_eq() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

        use crate::extensions::AggregatedExtension;

        let aggregated1 = AggregatedExtension::new("@@mod//ext.bzl", "ext");
        let aggregated2 = AggregatedExtension::new("@@mod//ext.bzl", "ext");

        // Create keys with different project_roots
        let key1 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated1,
            "_main".to_owned(),
            PathBuf::from("/project1"),
        );
        let key2 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated2,
            "_main".to_owned(),
            PathBuf::from("/project2"),
        );

        // Keys should be equal (project_root not in comparison)
        assert_eq!(key1, key2);

        // Hashes should be equal (project_root not in hash)
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        key2.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());
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
        );

        assert!(key.project_root.is_some());
        assert_eq!(key.project_root().unwrap(), &PathBuf::from("/tmp/test"));
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
