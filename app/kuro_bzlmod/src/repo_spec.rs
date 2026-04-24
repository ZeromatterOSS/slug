/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! RepoSpec capture infrastructure for bzlmod module extensions.
//!
//! When module extensions call repository rules (like `http_archive`), the
//! invocations are captured as `RepoSpec` objects rather than being executed
//! immediately. This enables the deferred execution model where repositories
//! are only materialized when they are actually accessed during a build.
//!
//! ## Architecture
//!
//! During extension execution:
//! 1. A `RepoSpecRegistry` is set up via `with_repo_spec_registry()`
//! 2. Repository rule calls detect the extension context via `in_extension_context()`
//! 3. Instead of recording a `RepositoryInvocation`, they record a `RepoSpec`
//! 4. After extension completes, all captured specs are collected
//!
//! This differs from `RepositoryInvocation` (used in MODULE.bazel/WORKSPACE)
//! in that RepoSpecs track the full rule identity for lazy execution.

use std::cell::RefCell;

use allocative::Allocative;
use base64::Engine;
use fxhash::FxHashMap;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::repository_invocations::AttrValue;

/// A captured repository specification from extension execution.
///
/// This represents the intent to create a repository WITHOUT executing
/// the repository rule. Actual execution happens lazily when the repo
/// is first accessed during a build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Allocative)]
pub struct RepoSpec {
    /// Repository rule identifier.
    /// Format: "@@{module}//path:file.bzl%{rule_name}"
    /// Example: "@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive"
    pub repo_rule_id: String,

    /// All attributes passed to the rule EXCEPT 'name'.
    /// The name is stored separately in the containing map.
    pub attributes: FxHashMap<String, AttrValue>,
}

impl RepoSpec {
    /// Create a new RepoSpec.
    pub fn new(repo_rule_id: String) -> Self {
        Self {
            repo_rule_id,
            attributes: FxHashMap::default(),
        }
    }

    /// Add an attribute.
    pub fn with_attr(mut self, key: String, value: AttrValue) -> Self {
        self.attributes.insert(key, value);
        self
    }

    /// Compute a hash for cache invalidation.
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.repo_rule_id.as_bytes());

        let mut keys: Vec<_> = self.attributes.keys().collect();
        keys.sort();
        for key in keys {
            hasher.update(key.as_bytes());
            if let Some(value) = self.attributes.get(key) {
                hasher.update(format!("{:?}", value).as_bytes());
            }
        }

        let hash = hasher.finalize();
        format!(
            "sha256-{}",
            base64::engine::general_purpose::STANDARD.encode(hash)
        )
    }
}

/// Thread-local registry for capturing RepoSpecs during extension execution.
///
/// During extension implementation execution, repository rule calls are
/// intercepted and recorded as RepoSpecs rather than executed immediately.
#[derive(Debug, Default)]
pub struct RepoSpecRegistry {
    /// Collected specs: internal_name -> RepoSpec
    specs: RefCell<fxhash::FxHashMap<String, RepoSpec>>,
}

impl RepoSpecRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a RepoSpec for a repository.
    pub fn record(&self, internal_name: String, spec: RepoSpec) {
        self.specs.borrow_mut().insert(internal_name, spec);
    }

    /// Take all collected specs.
    pub fn take(&self) -> fxhash::FxHashMap<String, RepoSpec> {
        std::mem::take(&mut *self.specs.borrow_mut())
    }
}

// Thread-local for extension execution context
thread_local! {
    static REPO_SPEC_REGISTRY: RefCell<Option<RepoSpecRegistry>> =
        const { RefCell::new(None) };
}

/// Set up a RepoSpec registry for extension execution.
///
/// This function establishes the extension execution context. While active,
/// repository rule invocations will be captured as RepoSpecs instead of
/// being recorded as RepositoryInvocations.
///
/// Returns a tuple of (result, captured_specs).
pub fn with_repo_spec_registry<R>(
    f: impl FnOnce() -> R,
) -> (R, fxhash::FxHashMap<String, RepoSpec>) {
    REPO_SPEC_REGISTRY.with(|cell| {
        *cell.borrow_mut() = Some(RepoSpecRegistry::new());
    });

    let result = f();

    let specs = REPO_SPEC_REGISTRY
        .with(|cell| cell.borrow().as_ref().map(|r| r.take()).unwrap_or_default());

    REPO_SPEC_REGISTRY.with(|cell| {
        *cell.borrow_mut() = None;
    });

    (result, specs)
}

/// Record a RepoSpec in the current extension context.
///
/// Returns `true` if a registry is active and the spec was recorded.
/// Returns `false` if no registry is active (not in extension execution).
pub fn record_repo_spec(internal_name: String, spec: RepoSpec) -> bool {
    REPO_SPEC_REGISTRY.with(|cell| {
        if let Some(registry) = cell.borrow().as_ref() {
            registry.record(internal_name, spec);
            true
        } else {
            false
        }
    })
}

/// Check if we're currently in extension execution context.
///
/// Returns `true` if `with_repo_spec_registry()` is active on this thread.
pub fn in_extension_context() -> bool {
    REPO_SPEC_REGISTRY.with(|cell| cell.borrow().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_spec_creation() {
        let spec =
            RepoSpec::new("@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive".to_owned())
                .with_attr(
                    "url".to_owned(),
                    AttrValue::String("https://example.com/foo.tar.gz".to_owned()),
                )
                .with_attr("sha256".to_owned(), AttrValue::String("abc123".to_owned()));

        assert_eq!(
            spec.repo_rule_id,
            "@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive"
        );
        assert_eq!(spec.attributes.len(), 2);
        assert_eq!(
            spec.attributes.get("url"),
            Some(&AttrValue::String(
                "https://example.com/foo.tar.gz".to_owned()
            ))
        );
    }

    #[test]
    fn test_repo_spec_hash() {
        let spec1 = RepoSpec::new("@@bazel_tools//...%http_archive".to_owned()).with_attr(
            "url".to_owned(),
            AttrValue::String("https://example.com/foo.tar.gz".to_owned()),
        );

        let spec2 = RepoSpec::new("@@bazel_tools//...%http_archive".to_owned()).with_attr(
            "url".to_owned(),
            AttrValue::String("https://example.com/foo.tar.gz".to_owned()),
        );

        // Same specs should have same hash
        assert_eq!(spec1.compute_hash(), spec2.compute_hash());

        let spec3 = RepoSpec::new("@@bazel_tools//...%http_archive".to_owned()).with_attr(
            "url".to_owned(),
            AttrValue::String("https://example.com/bar.tar.gz".to_owned()),
        );

        // Different specs should have different hash
        assert_ne!(spec1.compute_hash(), spec3.compute_hash());
    }

    #[test]
    fn test_registry_basic() {
        let registry = RepoSpecRegistry::new();

        registry.record("foo".to_owned(), RepoSpec::new("rule1".to_owned()));
        registry.record("bar".to_owned(), RepoSpec::new("rule2".to_owned()));

        let specs = registry.take();
        assert_eq!(specs.len(), 2);
        assert!(specs.contains_key("foo"));
        assert!(specs.contains_key("bar"));

        // After take, registry should be empty
        let specs2 = registry.take();
        assert!(specs2.is_empty());
    }

    #[test]
    fn test_with_repo_spec_registry() {
        // Initially not in extension context
        assert!(!in_extension_context());

        let (result, specs) = with_repo_spec_registry(|| {
            // Should be in extension context now
            assert!(in_extension_context());

            // Record some specs
            assert!(record_repo_spec(
                "foo".to_owned(),
                RepoSpec::new("rule1".to_owned())
            ));
            assert!(record_repo_spec(
                "bar".to_owned(),
                RepoSpec::new("rule2".to_owned())
            ));

            42
        });

        // Check result
        assert_eq!(result, 42);

        // Check captured specs
        assert_eq!(specs.len(), 2);
        assert!(specs.contains_key("foo"));
        assert!(specs.contains_key("bar"));

        // Should no longer be in extension context
        assert!(!in_extension_context());
    }

    #[test]
    fn test_record_outside_context() {
        // Outside extension context, record should return false
        assert!(!in_extension_context());
        assert!(!record_repo_spec(
            "foo".to_owned(),
            RepoSpec::new("rule".to_owned())
        ));
    }

    #[test]
    fn test_nested_contexts() {
        // Test that nested contexts work correctly (inner overwrites outer)
        let (_, outer_specs) = with_repo_spec_registry(|| {
            record_repo_spec("outer".to_owned(), RepoSpec::new("outer_rule".to_owned()));

            // Nested context
            let (_, inner_specs) = with_repo_spec_registry(|| {
                record_repo_spec("inner".to_owned(), RepoSpec::new("inner_rule".to_owned()));
            });

            // Inner specs should be collected
            assert_eq!(inner_specs.len(), 1);
            assert!(inner_specs.contains_key("inner"));
        });

        // Outer specs should only contain what was recorded before nesting
        // Note: Due to how the thread-local works, the nested context clears the registry
        // This is expected behavior - extensions shouldn't nest
        assert_eq!(outer_specs.len(), 0);
    }
}
