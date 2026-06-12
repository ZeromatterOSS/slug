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
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::repository_invocations::AttrValue;

pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

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
    ///
    /// `IndexMap` preserves insertion order so the serialised JSON
    /// (`repo_spec_json`) reflects the order the attributes were passed at
    /// the Starlark call site. This matches Bazel's lockfile behaviour and
    /// gives stable JSON across invocations without sorting.
    pub attributes: IndexMap<String, AttrValue>,

    /// Whether the repository rule was declared `local = True`.
    ///
    /// Bazel does not reuse cached repository contents for local repository
    /// rules across server instances; this bit lets Slug make the same
    /// materialization decision when executing extension-generated repos.
    #[serde(default, skip_serializing_if = "is_false")]
    pub local: bool,
}

impl RepoSpec {
    /// Create a new RepoSpec.
    pub fn new(repo_rule_id: String) -> Self {
        Self {
            repo_rule_id,
            attributes: IndexMap::new(),
            local: false,
        }
    }

    /// Add an attribute.
    pub fn with_attr(mut self, key: String, value: AttrValue) -> Self {
        self.attributes.insert(key, value);
        self
    }

    /// Compute a hash for cache invalidation.
    ///
    /// Uses `AttrValue::stable_hash_bytes` which produces deterministic output
    /// with type discriminators, independent of Rust `Debug` formatting or
    /// `serde_json` output format.
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.repo_rule_id.as_bytes());
        hasher.update([self.local as u8]);

        let mut keys: Vec<_> = self.attributes.keys().collect();
        keys.sort();
        for key in keys {
            hasher.update(key.as_bytes());
            hasher.update([0u8]);
            if let Some(value) = self.attributes.get(key) {
                let mut buf = Vec::new();
                value.stable_hash_bytes(&mut buf);
                hasher.update(&buf);
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
    specs: RefCell<FxHashMap<String, RepoSpec>>,
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
    pub fn take(&self) -> FxHashMap<String, RepoSpec> {
        std::mem::take(&mut *self.specs.borrow_mut())
    }
}

// Thread-local for extension execution context
thread_local! {
    static REPO_SPEC_REGISTRY: RefCell<Option<RepoSpecRegistry>> =
        const { RefCell::new(None) };
}

struct RepoSpecRegistryScope {
    previous: Option<RepoSpecRegistry>,
}

impl Drop for RepoSpecRegistryScope {
    fn drop(&mut self) {
        let previous = self.previous.take();
        REPO_SPEC_REGISTRY.with(|cell| {
            *cell.borrow_mut() = previous;
        });
    }
}

/// Set up a RepoSpec registry for extension execution.
///
/// This function establishes the extension execution context. While active,
/// repository rule invocations will be captured as RepoSpecs instead of
/// being recorded as RepositoryInvocations.
///
/// Returns a tuple of (result, captured_specs).
pub fn with_repo_spec_registry<R>(f: impl FnOnce() -> R) -> (R, FxHashMap<String, RepoSpec>) {
    let previous = REPO_SPEC_REGISTRY.with(|cell| cell.borrow_mut().take());
    let _scope = RepoSpecRegistryScope { previous };
    REPO_SPEC_REGISTRY.with(|cell| *cell.borrow_mut() = Some(RepoSpecRegistry::new()));

    let result = f();

    let specs = REPO_SPEC_REGISTRY
        .with(|cell| cell.borrow().as_ref().map(|r| r.take()).unwrap_or_default());

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

    /// Phase 64.6: verify compute_hash uses stable JSON serialization,
    /// not Rust Debug formatting. Distinct AttrValue variants that might
    /// coincidentally have the same Debug representation must produce
    /// distinct hashes.
    #[test]
    fn test_compute_hash_stable_json_not_debug() {
        // String "42" vs Int 42 — these have different Debug formats
        // ("String(\"42\")" vs "Int(42)"), but the key test is that the
        // hash is deterministic and not dependent on Rust compiler version.
        let spec_str = RepoSpec::new("test_rule".to_owned())
            .with_attr("val".to_owned(), AttrValue::String("42".to_owned()));
        let spec_int =
            RepoSpec::new("test_rule".to_owned()).with_attr("val".to_owned(), AttrValue::Int(42));

        // Different AttrValue variants must produce different hashes
        assert_ne!(
            spec_str.compute_hash(),
            spec_int.compute_hash(),
            "String(\"42\") and Int(42) must hash differently"
        );

        // Repeated calls produce the same result (deterministic)
        let hash1 = spec_str.compute_hash();
        let hash2 = spec_str.compute_hash();
        assert_eq!(hash1, hash2, "compute_hash must be deterministic");

        // Bool and Int must also differ
        let spec_bool = RepoSpec::new("test_rule".to_owned())
            .with_attr("val".to_owned(), AttrValue::Bool(true));
        assert_ne!(
            spec_int.compute_hash(),
            spec_bool.compute_hash(),
            "Int(42) and Bool(true) must hash differently"
        );
    }

    /// Phase 64.6: verify that attribute key ordering does not affect the hash.
    /// This is a property of the sorted-key iteration, not JSON serialization.
    #[test]
    fn test_compute_hash_attribute_ordering_stable() {
        let spec_a = RepoSpec::new("test_rule".to_owned())
            .with_attr("alpha".to_owned(), AttrValue::String("a".to_owned()))
            .with_attr("beta".to_owned(), AttrValue::String("b".to_owned()));

        let spec_b = RepoSpec::new("test_rule".to_owned())
            .with_attr("beta".to_owned(), AttrValue::String("b".to_owned()))
            .with_attr("alpha".to_owned(), AttrValue::String("a".to_owned()));

        assert_eq!(
            spec_a.compute_hash(),
            spec_b.compute_hash(),
            "Attribute insertion order must not affect hash"
        );
    }

    /// Phase 64.6: String and Label with the same text must produce
    /// distinct hashes. The old hash_bytes() had no type discriminators,
    /// so String("foo") and Label("foo") would collide.
    #[test]
    fn test_compute_hash_string_vs_label_distinct() {
        let spec_str = RepoSpec::new("test_rule".to_owned())
            .with_attr("val".to_owned(), AttrValue::String("foo".to_owned()));
        let spec_label = RepoSpec::new("test_rule".to_owned())
            .with_attr("val".to_owned(), AttrValue::Label("foo".to_owned()));
        assert_ne!(
            spec_str.compute_hash(),
            spec_label.compute_hash(),
            "String(\"foo\") and Label(\"foo\") must hash differently"
        );
    }

    /// Phase 64.6: verify that stable_hash_bytes produces different
    /// bytes for every AttrValue variant, even when the payload is
    /// identical text.
    #[test]
    fn test_attr_value_stable_hash_bytes_discriminates_variants() {
        let mut bytes_str = Vec::new();
        AttrValue::String("x".to_owned()).stable_hash_bytes(&mut bytes_str);

        let mut bytes_label = Vec::new();
        AttrValue::Label("x".to_owned()).stable_hash_bytes(&mut bytes_label);

        let mut bytes_int = Vec::new();
        AttrValue::Int(1).stable_hash_bytes(&mut bytes_int);

        let mut bytes_bool = Vec::new();
        AttrValue::Bool(true).stable_hash_bytes(&mut bytes_bool);

        let mut bytes_none = Vec::new();
        AttrValue::None.stable_hash_bytes(&mut bytes_none);

        assert_ne!(bytes_str, bytes_label, "String vs Label must differ");
        assert_ne!(bytes_str, bytes_int, "String vs Int must differ");
        assert_ne!(bytes_str, bytes_bool, "String vs Bool must differ");
        assert_ne!(bytes_str, bytes_none, "String vs None must differ");
        assert_ne!(bytes_int, bytes_bool, "Int vs Bool must differ");
        assert_ne!(bytes_int, bytes_none, "Int vs None must differ");
        assert!(!bytes_str.is_empty(), "String hash bytes must not be empty");
        assert!(
            bytes_str.starts_with(b"str:"),
            "String must have str: discriminator"
        );
        assert!(
            bytes_label.starts_with(b"label:"),
            "Label must have label: discriminator"
        );
        assert!(
            bytes_int.starts_with(b"int:"),
            "Int must have int: discriminator"
        );
        assert!(
            bytes_bool.starts_with(b"bool:"),
            "Bool must have bool: discriminator"
        );
        assert!(
            bytes_none.starts_with(b"none"),
            "None must have none discriminator"
        );
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
        let (_, outer_specs) = with_repo_spec_registry(|| {
            record_repo_spec("outer".to_owned(), RepoSpec::new("outer_rule".to_owned()));

            let (_, inner_specs) = with_repo_spec_registry(|| {
                record_repo_spec("inner".to_owned(), RepoSpec::new("inner_rule".to_owned()));
            });

            assert_eq!(inner_specs.len(), 1);
            assert!(inner_specs.contains_key("inner"));

            assert!(in_extension_context());
            record_repo_spec(
                "outer_after_inner".to_owned(),
                RepoSpec::new("outer_after_inner_rule".to_owned()),
            );
        });

        assert_eq!(outer_specs.len(), 2);
        assert!(outer_specs.contains_key("outer"));
        assert!(outer_specs.contains_key("outer_after_inner"));
        assert!(!in_extension_context());
    }

    #[test]
    fn registry_scope_restores_context_after_panic() {
        let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_repo_spec_registry(|| {
                assert!(in_extension_context());
                record_repo_spec("panic".to_owned(), RepoSpec::new("panic_rule".to_owned()));
                panic!("forced repo spec registry panic");
            });
        }));
        assert!(panic_result.is_err());
        assert!(!in_extension_context());
    }
}
