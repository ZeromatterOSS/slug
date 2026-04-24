/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Repository rule invocation registry for bzlmod.
//!
//! When repository rules (like `http_archive`) are invoked during MODULE.bazel
//! parsing or extension execution, the invocations are recorded here. The actual
//! repository fetching happens later via DICE.
//!
//! ## Architecture
//!
//! Repository rules can be invoked in two contexts:
//! 1. Direct calls in MODULE.bazel or WORKSPACE
//! 2. From module extension implementations
//!
//! Both paths record invocations to this registry, which is then processed
//! by the DICE-based repository execution system.

use std::cell::RefCell;
use std::sync::Mutex;

use allocative::Allocative;
use base64::Engine;
use fxhash::FxHashMap;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

/// A recorded repository rule invocation.
///
/// When a repository rule like `http_archive(name = "foo", ...)` is called,
/// we record the invocation here rather than executing it immediately.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Serialize, Deserialize)]
pub struct RepositoryInvocation {
    /// The repository name (from `name` attribute).
    pub name: String,

    /// The repository rule name (e.g., "http_archive", "new_local_repository").
    pub rule_name: String,

    /// The .bzl file path where the rule is defined, if known.
    pub rule_source: Option<String>,

    /// Attribute values passed to the invocation.
    pub attrs: FxHashMap<String, AttrValue>,
}

impl RepositoryInvocation {
    /// Create a new repository invocation.
    pub fn new(name: String, rule_name: String) -> Self {
        Self {
            name,
            rule_name,
            rule_source: None,
            attrs: FxHashMap::default(),
        }
    }

    /// Add a rule source path.
    pub fn with_rule_source(mut self, source: String) -> Self {
        self.rule_source = Some(source);
        self
    }

    /// Add an attribute.
    pub fn with_attr(mut self, key: String, value: AttrValue) -> Self {
        self.attrs.insert(key, value);
        self
    }

    /// Compute a hash of the invocation for caching purposes.
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.name.as_bytes());
        hasher.update(self.rule_name.as_bytes());
        if let Some(source) = &self.rule_source {
            hasher.update(source.as_bytes());
        }
        // Sort keys for deterministic hashing
        let mut keys: Vec<_> = self.attrs.keys().collect();
        keys.sort();
        for key in keys {
            hasher.update(key.as_bytes());
            if let Some(value) = self.attrs.get(key) {
                hasher.update(value.hash_bytes().as_slice());
            }
        }
        let hash = hasher.finalize();
        format!(
            "sha256-{}",
            base64::engine::general_purpose::STANDARD.encode(hash)
        )
    }
}

/// A simplified attribute value that can be serialized.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Serialize, Deserialize)]
pub enum AttrValue {
    String(String),
    Int(i64),
    Bool(bool),
    None,
    StringList(Vec<String>),
    Label(String),
    Dict(FxHashMap<String, AttrValue>),
}

impl AttrValue {
    /// Convert to a string if this is a string value.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            AttrValue::String(s) | AttrValue::Label(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to a string list if this is a string list value.
    pub fn as_string_list(&self) -> Option<&[String]> {
        match self {
            AttrValue::StringList(list) => Some(list),
            _ => None,
        }
    }

    /// Convert to bool if this is a bool value.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            AttrValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Convert to int if this is an int value.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            AttrValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Compute bytes for hashing.
    fn hash_bytes(&self) -> Vec<u8> {
        match self {
            AttrValue::String(s) => s.as_bytes().to_vec(),
            AttrValue::Int(i) => i.to_le_bytes().to_vec(),
            AttrValue::Bool(b) => vec![if *b { 1 } else { 0 }],
            AttrValue::None => vec![],
            AttrValue::StringList(list) => {
                let mut bytes = Vec::new();
                for s in list {
                    bytes.extend(s.as_bytes());
                    bytes.push(0);
                }
                bytes
            }
            AttrValue::Label(s) => s.as_bytes().to_vec(),
            AttrValue::Dict(map) => {
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort();
                let mut bytes = Vec::new();
                for key in keys {
                    bytes.extend(key.as_bytes());
                    if let Some(value) = map.get(key) {
                        bytes.extend(value.hash_bytes());
                    }
                }
                bytes
            }
        }
    }
}

/// Thread-safe registry for collecting repository invocations during parsing.
///
/// This uses thread-local storage so that multiple parsings can happen
/// concurrently without interference.
#[derive(Debug, Default)]
pub struct RepositoryInvocationRegistry {
    /// Invocations collected during parsing.
    invocations: Mutex<Vec<RepositoryInvocation>>,
}

impl RepositoryInvocationRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            invocations: Mutex::new(Vec::new()),
        }
    }

    /// Record a repository invocation.
    pub fn record(&self, invocation: RepositoryInvocation) {
        let mut invocations = self.invocations.lock().unwrap();
        invocations.push(invocation);
    }

    /// Get all recorded invocations.
    pub fn invocations(&self) -> Vec<RepositoryInvocation> {
        self.invocations.lock().unwrap().clone()
    }

    /// Clear all recorded invocations.
    pub fn clear(&self) {
        self.invocations.lock().unwrap().clear();
    }

    /// Take all invocations, leaving the registry empty.
    pub fn take(&self) -> Vec<RepositoryInvocation> {
        std::mem::take(&mut *self.invocations.lock().unwrap())
    }
}

// Thread-local registry for current parsing context
thread_local! {
    static CURRENT_REGISTRY: RefCell<Option<RepositoryInvocationRegistry>> = const { RefCell::new(None) };
}

/// Set up a registry for the current thread's parsing context.
///
/// Returns a guard that will clear the registry when dropped.
pub fn with_registry<R>(f: impl FnOnce(&RepositoryInvocationRegistry) -> R) -> R {
    CURRENT_REGISTRY.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if borrow.is_none() {
            *borrow = Some(RepositoryInvocationRegistry::new());
        }
        f(borrow.as_ref().unwrap())
    })
}

/// Record a repository invocation in the current thread's registry.
pub fn record_invocation(invocation: RepositoryInvocation) {
    CURRENT_REGISTRY.with(|cell| {
        if let Some(registry) = cell.borrow().as_ref() {
            registry.record(invocation);
        }
    });
}

/// Check if there's an active registry for the current thread.
pub fn has_active_registry() -> bool {
    CURRENT_REGISTRY.with(|cell| cell.borrow().is_some())
}

/// Take all invocations from the current thread's registry.
pub fn take_invocations() -> Vec<RepositoryInvocation> {
    CURRENT_REGISTRY.with(|cell| cell.borrow().as_ref().map(|r| r.take()).unwrap_or_default())
}

/// Clear the current thread's registry.
pub fn clear_registry() {
    CURRENT_REGISTRY.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// A guard that manages the lifecycle of the repository invocation registry.
pub struct RegistryGuard;

impl RegistryGuard {
    /// Create a new registry guard, setting up the thread-local registry.
    pub fn new() -> Self {
        CURRENT_REGISTRY.with(|cell| {
            *cell.borrow_mut() = Some(RepositoryInvocationRegistry::new());
        });
        RegistryGuard
    }

    /// Take all invocations from the registry.
    pub fn take(&self) -> Vec<RepositoryInvocation> {
        take_invocations()
    }
}

impl Drop for RegistryGuard {
    fn drop(&mut self) {
        clear_registry();
    }
}

impl Default for RegistryGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invocation_creation() {
        let inv = RepositoryInvocation::new("foo".to_owned(), "http_archive".to_owned())
            .with_attr(
                "url".to_owned(),
                AttrValue::String("https://example.com/foo.tar.gz".to_owned()),
            )
            .with_attr("sha256".to_owned(), AttrValue::String("abc123".to_owned()));

        assert_eq!(inv.name, "foo");
        assert_eq!(inv.rule_name, "http_archive");
        assert_eq!(inv.attrs.len(), 2);
    }

    #[test]
    fn test_invocation_hash() {
        let inv1 = RepositoryInvocation::new("foo".to_owned(), "http_archive".to_owned())
            .with_attr(
                "url".to_owned(),
                AttrValue::String("https://example.com/foo.tar.gz".to_owned()),
            );

        let inv2 = RepositoryInvocation::new("foo".to_owned(), "http_archive".to_owned())
            .with_attr(
                "url".to_owned(),
                AttrValue::String("https://example.com/foo.tar.gz".to_owned()),
            );

        // Same invocations should have same hash
        assert_eq!(inv1.compute_hash(), inv2.compute_hash());

        let inv3 = RepositoryInvocation::new("bar".to_owned(), "http_archive".to_owned())
            .with_attr(
                "url".to_owned(),
                AttrValue::String("https://example.com/bar.tar.gz".to_owned()),
            );

        // Different invocations should have different hash
        assert_ne!(inv1.compute_hash(), inv3.compute_hash());
    }

    #[test]
    fn test_registry() {
        let registry = RepositoryInvocationRegistry::new();

        registry.record(RepositoryInvocation::new(
            "foo".to_owned(),
            "http_archive".to_owned(),
        ));
        registry.record(RepositoryInvocation::new(
            "bar".to_owned(),
            "git_repository".to_owned(),
        ));

        let invocations = registry.invocations();
        assert_eq!(invocations.len(), 2);
        assert_eq!(invocations[0].name, "foo");
        assert_eq!(invocations[1].name, "bar");
    }

    #[test]
    fn test_registry_guard() {
        {
            let guard = RegistryGuard::new();

            record_invocation(RepositoryInvocation::new(
                "test".to_owned(),
                "local_repository".to_owned(),
            ));

            let invocations = guard.take();
            assert_eq!(invocations.len(), 1);
            assert_eq!(invocations[0].name, "test");
        }

        // After guard is dropped, registry should be cleared
        assert!(!has_active_registry());
    }

    #[test]
    fn test_attr_value_types() {
        assert_eq!(
            AttrValue::String("hello".to_owned()).as_string(),
            Some("hello")
        );
        assert_eq!(
            AttrValue::Label("//foo:bar".to_owned()).as_string(),
            Some("//foo:bar")
        );
        assert_eq!(AttrValue::Int(42).as_int(), Some(42));
        assert_eq!(AttrValue::Bool(true).as_bool(), Some(true));
        assert_eq!(
            AttrValue::StringList(vec!["a".to_owned(), "b".to_owned()]).as_string_list(),
            Some(&["a".to_owned(), "b".to_owned()][..])
        );
    }
}
