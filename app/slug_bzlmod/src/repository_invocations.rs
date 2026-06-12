/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Repository rule invocation data for bzlmod.
//!
//! `RepositoryInvocation` is the serializable representation of repository rule
//! calls that MODULE.bazel parsing can carry forward. The old thread-local
//! capture registry is test-only; production module parsing records directives
//! through `ModuleFileContext`, and extension execution captures `RepoSpec`
//! values instead.
//!
//! ## Architecture
//!
//! Repository rules can be invoked in two contexts:
//! 1. Direct calls in MODULE.bazel or WORKSPACE
//! 2. From module extension implementations
//!
//! DICE repository execution consumes explicit invocation/spec keys rather than
//! exposing the thread-local registry as a caller-managed API.

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::sync::Mutex;

use allocative::Allocative;
use base64::Engine;
use indexmap::IndexMap;
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
    ///
    /// `IndexMap` preserves insertion order (the Starlark call-site kwarg
    /// order) so serialising this as JSON produces stable output matching
    /// Bazel conventions.
    pub attrs: IndexMap<String, AttrValue>,
}

impl RepositoryInvocation {
    /// Create a new repository invocation.
    pub fn new(name: String, rule_name: String) -> Self {
        Self {
            name,
            rule_name,
            rule_source: None,
            attrs: IndexMap::new(),
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
        if let Some(src) = &self.rule_source {
            hasher.update(src.as_bytes());
        }
        let mut keys: Vec<_> = self.attrs.keys().collect();
        keys.sort();
        for key in keys {
            hasher.update(key.as_bytes());
            if let Some(val) = self.attrs.get(key) {
                let mut buf = Vec::new();
                val.stable_hash_bytes(&mut buf);
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

/// A simplified attribute value that can be serialized.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Serialize, Deserialize)]
pub enum AttrValue {
    String(String),
    Int(i64),
    Bool(bool),
    None,
    StringList(Vec<String>),
    Label(String),
    Dict(IndexMap<String, AttrValue>),
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

    pub fn stable_hash_bytes(&self, out: &mut Vec<u8>) {
        match self {
            AttrValue::String(s) => {
                out.extend_from_slice(b"str:");
                out.extend_from_slice(s.as_bytes());
            }
            AttrValue::Int(i) => {
                out.extend_from_slice(b"int:");
                out.extend_from_slice(&i.to_le_bytes());
            }
            AttrValue::Bool(b) => {
                out.extend_from_slice(b"bool:");
                out.push(if *b { 1 } else { 0 });
            }
            AttrValue::None => {
                out.extend_from_slice(b"none");
            }
            AttrValue::StringList(list) => {
                out.extend_from_slice(b"list:");
                out.extend_from_slice(&list.len().to_le_bytes());
                for s in list {
                    out.extend_from_slice(s.as_bytes());
                    out.push(0);
                }
            }
            AttrValue::Label(s) => {
                out.extend_from_slice(b"label:");
                out.extend_from_slice(s.as_bytes());
            }
            AttrValue::Dict(map) => {
                out.extend_from_slice(b"dict:");
                out.extend_from_slice(&map.len().to_le_bytes());
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort();
                for key in keys {
                    out.extend_from_slice(key.as_bytes());
                    out.push(0);
                    if let Some(value) = map.get(key) {
                        value.stable_hash_bytes(out);
                    }
                }
            }
        }
    }
}

/// Thread-safe registry for collecting repository invocations during parsing.
///
/// This uses thread-local storage so that multiple parsings can happen
/// concurrently without interference.
#[derive(Debug, Default)]
#[cfg(test)]
struct RepositoryInvocationRegistry {
    /// Invocations collected during parsing.
    invocations: Mutex<Vec<RepositoryInvocation>>,
}

#[cfg(test)]
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

    /// Take all invocations, leaving the registry empty.
    pub fn take(&self) -> Vec<RepositoryInvocation> {
        std::mem::take(&mut *self.invocations.lock().unwrap())
    }
}

// Thread-local registry for current parsing context
#[cfg(test)]
thread_local! {
    static CURRENT_REGISTRY: RefCell<Option<RepositoryInvocationRegistry>> = const { RefCell::new(None) };
}

/// Record a repository invocation in the current thread's test registry.
#[cfg(test)]
pub fn record_invocation(invocation: RepositoryInvocation) {
    CURRENT_REGISTRY.with(|cell| {
        if let Some(registry) = cell.borrow().as_ref() {
            registry.record(invocation);
        }
    });
}

/// Check if there's an active registry for the current thread.
#[cfg(test)]
fn has_active_registry() -> bool {
    CURRENT_REGISTRY.with(|cell| cell.borrow().is_some())
}

/// Take all invocations from the current thread's registry.
#[cfg(test)]
fn take_invocations() -> Vec<RepositoryInvocation> {
    CURRENT_REGISTRY.with(|cell| cell.borrow().as_ref().map(|r| r.take()).unwrap_or_default())
}

/// A guard that manages the lifecycle of the repository invocation registry.
#[cfg(test)]
struct RegistryGuard {
    previous: Option<RepositoryInvocationRegistry>,
}

#[cfg(test)]
impl RegistryGuard {
    /// Create a new registry guard, setting up the thread-local registry.
    pub fn new() -> Self {
        let previous = CURRENT_REGISTRY.with(|cell| cell.borrow_mut().take());
        CURRENT_REGISTRY.with(|cell| {
            *cell.borrow_mut() = Some(RepositoryInvocationRegistry::new());
        });
        RegistryGuard { previous }
    }

    /// Take all invocations from the registry.
    pub fn take(&self) -> Vec<RepositoryInvocation> {
        take_invocations()
    }
}

#[cfg(test)]
impl Drop for RegistryGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        CURRENT_REGISTRY.with(|cell| {
            *cell.borrow_mut() = previous;
        });
    }
}

#[cfg(test)]
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
    fn registry_guard_restores_outer_registry() {
        let outer = RegistryGuard::new();
        record_invocation(RepositoryInvocation::new(
            "outer".to_owned(),
            "local_repository".to_owned(),
        ));

        {
            let inner = RegistryGuard::new();
            record_invocation(RepositoryInvocation::new(
                "inner".to_owned(),
                "http_archive".to_owned(),
            ));
            let inner_invocations = inner.take();
            assert_eq!(inner_invocations.len(), 1);
            assert_eq!(inner_invocations[0].name, "inner");
        }

        assert!(has_active_registry());
        record_invocation(RepositoryInvocation::new(
            "outer_after_inner".to_owned(),
            "git_repository".to_owned(),
        ));
        let outer_invocations = outer.take();
        assert_eq!(outer_invocations.len(), 2);
        assert_eq!(outer_invocations[0].name, "outer");
        assert_eq!(outer_invocations[1].name, "outer_after_inner");
    }

    #[test]
    fn test_attr_value_stable_hash_bytes_none() {
        let mut bytes = Vec::new();
        AttrValue::None.stable_hash_bytes(&mut bytes);
        assert_eq!(bytes.as_slice(), b"none");
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
