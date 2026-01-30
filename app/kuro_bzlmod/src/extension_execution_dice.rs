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

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;

use crate::extensions::AggregatedExtension;
use crate::extensions::compute_extension_input_hash;
use crate::module_extension_executor::MODULE_EXTENSION_EXECUTOR_IMPL;
use crate::repo_spec::RepoSpec;

/// Errors during module extension execution.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
pub enum ModuleExtensionError {
    #[error("Module extension execution failed for '{extension_id}': {reason}")]
    ExecutionFailed { extension_id: String, reason: String },

    #[error("Failed to create temporary working directory for extension '{extension_id}': {reason}")]
    TempDirFailed { extension_id: String, reason: String },

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
    pub generated_repo_specs: HashMap<String, RepoSpec>,

    /// Canonical name mapping.
    /// Maps internal_name -> canonical_name (e.g., "numpy" -> "_main~pip~numpy")
    pub canonical_names: HashMap<String, String>,
}

impl ModuleExtensionResult {
    /// Create a new extension result.
    pub fn new(
        extension_id: Arc<str>,
        input_hash: String,
        generated_repo_specs: HashMap<String, RepoSpec>,
    ) -> Self {
        let canonical_names = build_canonical_names(&extension_id, &generated_repo_specs);
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
/// 1. Creates a temporary working directory for module_ctx
/// 2. Loads the extension's .bzl file
/// 3. Builds module_ctx from aggregated tags
/// 4. Executes implementation(module_ctx) with RepoSpec capture
/// 5. Cleans up the temporary directory
/// 6. Returns ModuleExtensionResult with captured specs
///
/// Note: NO downloads or repository materialization happens during this computation.
/// Repositories are materialized lazily via `ExtensionRepoExecutionKey`.
///
/// Note: Hash and Eq are implemented manually because `AggregatedExtension` contains
/// HashMap. The `input_hash` field is used for hashing, ensuring deterministic cache behavior.
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
}

impl std::hash::Hash for ModuleExtensionExecutionKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the identifying fields; input_hash represents the aggregated data
        self.extension_id.hash(state);
        self.input_hash.hash(state);
    }
}

impl PartialEq for ModuleExtensionExecutionKey {
    fn eq(&self, other: &Self) -> bool {
        // Compare by identifying fields; input_hash represents the aggregated data
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

        // Log the modules that use this extension
        let module_count = self.aggregated.tags_by_module.len();
        let tag_count: usize = self.aggregated.tags_by_module.values().map(|v| v.len()).sum();
        tracing::debug!(
            "Extension '{}' used by {} module(s) with {} total tag(s)",
            self.extension_id,
            module_count,
            tag_count
        );

        // 1. Create temporary working directory for module_ctx I/O
        let temp_dir = create_temp_extension_dir(&self.extension_id)?;

        // 2-4. Execute extension via late binding to kuro_interpreter_for_build
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
                    .execute_extension(
                        ctx,
                        &self.aggregated,
                        &self.root_module_name,
                        &temp_dir,
                    )
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
                    generated_repo_specs: HashMap::new(),
                })
            }
        };

        // 5. Clean up temporary working directory
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

        // 6. Build result with canonical names
        let result = ModuleExtensionResult::new(
            self.extension_id.clone(),
            self.input_hash.to_string(),
            output.generated_repo_specs,
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
fn create_temp_extension_dir(extension_id: &str) -> kuro_error::Result<PathBuf> {
    // Sanitize extension ID for use in path
    let sanitized = sanitize_extension_id_for_path(extension_id);

    let temp_base = std::env::temp_dir().join("kuro-extension");
    std::fs::create_dir_all(&temp_base).map_err(|e| {
        ModuleExtensionError::TempDirFailed {
            extension_id: extension_id.to_owned(),
            reason: format!("failed to create temp base: {}", e),
        }
    })?;

    let temp_dir = temp_base.join(&sanitized);

    // Clean up any previous temp dir for this extension
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        ModuleExtensionError::TempDirFailed {
            extension_id: extension_id.to_owned(),
            reason: format!("failed to create temp dir: {}", e),
        }
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
            c if c.is_alphanumeric() || c == '_' || c == '-' || c == '~' || c == '.' => c,
            _ => '_',
        })
        .collect()
}

/// Build canonical names for extension-generated repositories.
///
/// Format: `_main~{extension_unique_name}~{internal_name}`
///
/// The extension unique name is derived from the extension ID by extracting
/// the extension name (after the `%` in the bzl label).
pub fn build_canonical_names(
    extension_id: &str,
    specs: &HashMap<String, RepoSpec>,
) -> HashMap<String, String> {
    let ext_name = extract_extension_name(extension_id);
    specs
        .keys()
        .map(|internal| {
            let canonical = format!("_main~{}~{}", ext_name, internal);
            (internal.clone(), canonical)
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository_invocations::AttrValue;

    #[test]
    fn test_module_extension_result_creation() {
        let mut specs = HashMap::new();
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
        );

        assert_eq!(result.extension_id.as_ref(), "@@rules_python//python/pip:pip.bzl%pip");
        assert_eq!(result.input_hash, "sha256-abc123");
        assert_eq!(result.repo_count(), 2);
        assert!(result.contains_repo("numpy"));
        assert!(result.contains_repo("requests"));
        assert!(!result.contains_repo("pandas"));
    }

    #[test]
    fn test_canonical_name_lookup() {
        let mut specs = HashMap::new();
        specs.insert("foo".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("bar".to_owned(), RepoSpec::new("rule".to_owned()));

        let result = ModuleExtensionResult::new(
            Arc::from("@@module//path:ext.bzl%my_extension"),
            "hash".to_owned(),
            specs,
        );

        assert_eq!(result.canonical_name("foo"), Some("_main~my_extension~foo"));
        assert_eq!(result.canonical_name("bar"), Some("_main~my_extension~bar"));
        assert_eq!(result.canonical_name("baz"), None);
    }

    #[test]
    fn test_internal_name_from_canonical() {
        let mut specs = HashMap::new();
        specs.insert("numpy".to_owned(), RepoSpec::new("rule".to_owned()));

        let result = ModuleExtensionResult::new(
            Arc::from("@@rules_python//pip:pip.bzl%pip"),
            "hash".to_owned(),
            specs,
        );

        assert_eq!(
            result.internal_name_from_canonical("_main~pip~numpy"),
            Some("numpy")
        );
        assert_eq!(result.internal_name_from_canonical("_main~pip~pandas"), None);
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
        assert_eq!(
            extract_extension_name("//:extension.bzl"),
            "extension"
        );
        assert_eq!(
            extract_extension_name("simple_name"),
            "simple_name"
        );
    }

    #[test]
    fn test_build_canonical_names() {
        let mut specs = HashMap::new();
        specs.insert("numpy".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("pandas".to_owned(), RepoSpec::new("rule".to_owned()));

        let names = build_canonical_names("@@rules_python//pip:pip.bzl%pip", &specs);

        assert_eq!(names.get("numpy"), Some(&"_main~pip~numpy".to_owned()));
        assert_eq!(names.get("pandas"), Some(&"_main~pip~pandas".to_owned()));
    }

    #[test]
    fn test_sanitize_extension_id() {
        assert_eq!(
            sanitize_extension_id_for_path("@@module//path:file.bzl%ext"),
            "__module__path_file.bzl_ext"
        );
        assert_eq!(
            sanitize_extension_id_for_path("simple_name"),
            "simple_name"
        );
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

        let key = ModuleExtensionExecutionKey::new(
            aggregated,
            "_main".to_owned(),
        );

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
        parse_tag.kwargs.push(("hub_name".to_owned(), TagValue::String("pip".to_owned())));

        let mut install_tag = ExtensionTag::new("install".to_owned());
        install_tag.kwargs.push(("name".to_owned(), TagValue::String("numpy".to_owned())));

        aggregated.add_module_tags("root", vec![parse_tag]);
        aggregated.add_module_tags("dep_a", vec![install_tag]);

        let key = ModuleExtensionExecutionKey::new(
            aggregated,
            "root".to_owned(),
        );

        assert_eq!(key.extension_id.as_ref(), "@@rules_python//pip:pip.bzl%pip");
        assert_eq!(key.root_module_name.as_ref(), "root");
        assert_eq!(key.aggregated().tags_by_module.len(), 2);
    }

    #[test]
    fn test_module_extension_key_hash_eq() {
        use crate::extensions::AggregatedExtension;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

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
        let mut specs = HashMap::new();
        specs.insert(
            "test_repo".to_owned(),
            RepoSpec::new("@@bazel_tools//repo:http.bzl%http_archive".to_owned())
                .with_attr("url".to_owned(), AttrValue::String("https://example.com".to_owned())),
        );

        let result = ModuleExtensionResult::new(
            Arc::from("@@//ext.bzl%test"),
            "hash".to_owned(),
            specs,
        );

        let spec = result.get_repo_spec("test_repo").unwrap();
        assert_eq!(spec.repo_rule_id, "@@bazel_tools//repo:http.bzl%http_archive");
        assert!(result.get_repo_spec("nonexistent").is_none());
    }

    #[test]
    fn test_repo_names_iterator() {
        let mut specs = HashMap::new();
        specs.insert("a".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("b".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("c".to_owned(), RepoSpec::new("rule".to_owned()));

        let result = ModuleExtensionResult::new(
            Arc::from("@@//ext.bzl%test"),
            "hash".to_owned(),
            specs,
        );

        let mut names: Vec<_> = result.repo_names().collect();
        names.sort();
        assert_eq!(names, vec!["a", "b", "c"]);
    }
}
