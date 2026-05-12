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
//!    and registers the result with the materializer
//!
//! ## Pattern Reference
//!
//! This follows the `GitFileOpsDelegateKey` pattern from `slug_external_cells/src/git.rs`.

use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;

use crate::repo_spec::RepoSpec;
use crate::repository_invocations::RepositoryInvocation;

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
        // For now, create a stub result that indicates the repository was "executed"
        // The actual implementation will be filled in during Phase 5d-3 (Blocking I/O Integration)
        //
        // The full implementation will:
        // 1. Get the project root from DICE context
        // 2. Create/clean the working directory
        // 3. Load the repository rule's .bzl file
        // 4. Create a RepositoryContext with the working directory
        // 5. Execute the implementation function
        // 6. Register the result with the materializer

        tracing::info!(
            "Executing repository rule '{}' for repository '{}'",
            self.rule_name,
            self.name
        );

        // Stub: Just create the path that would be used
        let repo_path = PathBuf::from("bazel-external").join(self.name.as_ref());

        Ok(Arc::new(RepositoryRuleResult::success(
            self.name.to_string(),
            repo_path,
        )))
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
}

impl std::hash::Hash for ExtensionRepoExecutionKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the identifying fields; spec_hash represents the repo_spec
        // Note: project_root is intentionally not hashed - it's a runtime configuration
        // that doesn't affect cache identity
        self.canonical_name.hash(state);
        self.extension_id.hash(state);
        self.spec_hash.hash(state);
    }
}

impl PartialEq for ExtensionRepoExecutionKey {
    fn eq(&self, other: &Self) -> bool {
        // Compare by identifying fields; spec_hash represents the repo_spec
        // Note: project_root is intentionally not compared - it's a runtime configuration
        self.canonical_name == other.canonical_name
            && self.extension_id == other.extension_id
            && self.spec_hash == other.spec_hash
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
        Self {
            canonical_name: Arc::from(canonical_name.as_str()),
            extension_id: Arc::from(extension_id.as_str()),
            spec_hash: Arc::from(spec_hash.as_str()),
            repo_spec: Arc::new(repo_spec),
            project_root: Arc::new(project_root),
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
        Self {
            canonical_name,
            extension_id,
            spec_hash: Arc::from(spec_hash.as_str()),
            repo_spec,
            project_root,
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

fn complete_marker(spec_hash: &str) -> String {
    if spec_hash.is_empty() {
        "complete".to_owned()
    } else {
        format!("complete:{spec_hash}")
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
                            Ok(()) => {
                                // Mark as complete and write WORKSPACE if missing
                                if !working_dir.join("WORKSPACE").exists()
                                    && !working_dir.join("WORKSPACE.bazel").exists()
                                {
                                    let _ = std::fs::write(
                                        working_dir.join("WORKSPACE.bazel"),
                                        format!("workspace(name = \"{}\")\n", self.canonical_name),
                                    );
                                }
                                let _ = std::fs::write(
                                    working_dir.join(".slug_repo_complete"),
                                    complete_marker(&self.spec_hash),
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
        // Don't cache errors - retry on next request
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
        assert_eq!(complete_marker(""), "complete");
        assert_eq!(complete_marker("sha256-abc123"), "complete:sha256-abc123");
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
    fn test_extension_repo_key_hash_ignores_project_root() {
        // Same spec with different project roots should have same hash
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

        // Keys should be equal (project_root not in comparison)
        assert_eq!(key1, key2);
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
