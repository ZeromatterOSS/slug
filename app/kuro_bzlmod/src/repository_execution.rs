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
//! This follows the `GitFileOpsDelegateKey` pattern from `kuro_external_cells/src/git.rs`.

use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;

use crate::lockfile::RepositoryRuleLockEntry;
use crate::repository_invocations::RepositoryInvocation;

/// Errors that can occur during repository rule execution.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
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

    /// Create a lockfile entry from this result.
    ///
    /// This can be used to cache the repository rule execution in the lockfile.
    pub fn to_lock_entry(&self, rule_name: &str, attrs_hash: &str) -> RepositoryRuleLockEntry {
        let mut entry = RepositoryRuleLockEntry::new(
            rule_name.to_owned(),
            attrs_hash.to_owned(),
        );

        if let Some(hash) = &self.content_hash {
            entry = entry.with_content_hash(hash.clone());
        }

        entry.with_timestamp()
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
    type Value = kuro_error::Result<Arc<RepositoryRuleResult>>;

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
    pub fn require_string(&self, name: &str) -> kuro_error::Result<&str> {
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
        self.get_string(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        inv.attrs
            .insert("url".to_owned(), AttrValue::String("https://example.com".to_owned()));
        inv.attrs.insert(
            "urls".to_owned(),
            AttrValue::StringList(vec![
                "https://example.com/a".to_owned(),
                "https://example.com/b".to_owned(),
            ]),
        );
        inv.attrs.insert("build_file_content".to_owned(), AttrValue::None);

        let attrs = InvocationAttrs::new(&inv);

        assert_eq!(attrs.get_string("url"), Some("https://example.com"));
        assert_eq!(attrs.get_string("sha256"), None);
        assert_eq!(
            attrs.get_string_list("urls"),
            Some(&["https://example.com/a".to_owned(), "https://example.com/b".to_owned()][..])
        );
    }

    #[test]
    fn test_repository_rule_result() {
        let result = RepositoryRuleResult::success("test".to_owned(), PathBuf::from("bazel-external/test"))
            .with_content_hash("sha256-abc123".to_owned());

        assert_eq!(result.repo_name, "test");
        assert_eq!(result.repo_path, PathBuf::from("bazel-external/test"));
        assert_eq!(result.content_hash, Some("sha256-abc123".to_owned()));
        assert!(result.success);
    }
}
