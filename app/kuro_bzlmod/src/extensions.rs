/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Module extension execution.
//!
//! This module handles the execution of module extensions defined in .bzl files.
//! Extensions allow custom dependency resolution logic, such as:
//! - `pip.parse()` for Python dependencies
//! - `maven.install()` for JVM dependencies
//! - `crate.from_cargo()` for Rust crate dependencies
//!
//! # Extension Lifecycle
//!
//! 1. **Collection**: During MODULE.bazel parsing, `use_extension()` calls are
//!    recorded along with their tags (e.g., `pip.parse(...)`).
//!
//! 2. **Aggregation**: Tags from all modules using the same extension are
//!    collected and grouped by the extension.
//!
//! 3. **Execution**: The extension's implementation function is called with a
//!    `module_ctx` object containing all the collected tags.
//!
//! 4. **Repository Generation**: The extension creates repositories that are
//!    then made available via `use_repo()`.
//!
//! # Example
//!
//! ```starlark
//! # In MODULE.bazel
//! pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
//! pip.parse(
//!     hub_name = "pip",
//!     python_version = "3.11",
//!     requirements_lock = "//:requirements_lock.txt",
//! )
//! use_repo(pip, "pip")
//!
//! # In the extension's .bzl file
//! def _pip_impl(module_ctx):
//!     for mod in module_ctx.modules:
//!         for tag in mod.tags.parse:
//!             # Process the parse tag and create repos
//!             pass
//!
//! pip = module_extension(
//!     implementation = _pip_impl,
//!     tag_classes = {
//!         "parse": tag_class(attrs = {...}),
//!     },
//! )
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

use allocative::Allocative;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;

use crate::types::ExtensionTag;
use crate::types::ExtensionUsage;
use crate::types::TagValue;

/// Aggregated extension data from all modules.
///
/// This represents all the tags applied to a single extension across
/// all modules in the dependency graph.
#[derive(Debug, Clone, Default, Allocative)]
pub struct AggregatedExtension {
    /// The extension identifier (bzl_file%name).
    pub extension_id: String,

    /// The .bzl file containing the extension.
    pub extension_bzl_file: String,

    /// The extension name.
    pub extension_name: String,

    /// Tags grouped by module that applied them.
    /// Key is module name, value is the tags from that module.
    pub tags_by_module: HashMap<String, Vec<ExtensionTag>>,

    /// All repositories imported via use_repo() for this extension.
    pub imported_repos: Vec<String>,
}

impl AggregatedExtension {
    /// Create a new aggregated extension.
    pub fn new(bzl_file: &str, name: &str) -> Self {
        Self {
            extension_id: format!("{}%{}", bzl_file, name),
            extension_bzl_file: bzl_file.to_string(),
            extension_name: name.to_string(),
            tags_by_module: HashMap::new(),
            imported_repos: Vec::new(),
        }
    }

    /// Add tags from a module.
    pub fn add_module_tags(&mut self, module_name: &str, tags: Vec<ExtensionTag>) {
        self.tags_by_module
            .entry(module_name.to_string())
            .or_default()
            .extend(tags);
    }

    /// Add imported repositories.
    pub fn add_imported_repos(&mut self, repos: impl IntoIterator<Item = String>) {
        self.imported_repos.extend(repos);
    }

    /// Get all tags flattened into a single list.
    pub fn all_tags(&self) -> Vec<&ExtensionTag> {
        self.tags_by_module.values().flatten().collect()
    }
}

/// Aggregate extension usages from all modules.
///
/// This collects all extension usages from the dependency graph and
/// groups them by extension. Dev-dependency usages from non-root modules
/// are skipped (Bazel 9.0 behavior).
pub fn aggregate_extensions(
    module_extensions: &HashMap<String, Vec<ExtensionUsage>>,
) -> HashMap<String, AggregatedExtension> {
    aggregate_extensions_with_root(module_extensions, None)
}

/// Aggregate extension usages, filtering dev_dependency from non-root modules.
pub fn aggregate_extensions_with_root(
    module_extensions: &HashMap<String, Vec<ExtensionUsage>>,
    root_module_name: Option<&str>,
) -> HashMap<String, AggregatedExtension> {
    let mut aggregated: HashMap<String, AggregatedExtension> = HashMap::new();

    for (module_name, usages) in module_extensions {
        for usage in usages {
            // Skip dev_dependency usages from non-root modules
            if usage.dev_dependency {
                let is_root = root_module_name
                    .map_or(true, |root| module_name == root || module_name == "_main");
                if !is_root {
                    tracing::debug!(
                        "Skipping dev_dependency extension '{}' from non-root module '{}'",
                        usage.extension_id(),
                        module_name
                    );
                    continue;
                }
            }

            // Same module extension can be referenced in two shapes: the
            // owning module writes `use_extension("//:ext.bzl", "name")`
            // (relative), consumers write `use_extension("@owner//:ext.bzl",
            // "name")` (explicit). Both must collapse into a single
            // AggregatedExtension; otherwise the consumer's tags + root-ness
            // live in one entry and the owner's in another, and whichever
            // the executor happens to look up is missing half the data.
            let ext_id = canonical_extension_id(
                &usage.extension_bzl_file,
                &usage.extension_name,
                module_name,
            );

            let agg = aggregated.entry(ext_id.clone()).or_insert_with(|| {
                let resolved_bzl_file =
                    if usage.extension_bzl_file.starts_with("//") && !module_name.is_empty() {
                        format!("@{}{}", module_name, usage.extension_bzl_file)
                    } else {
                        usage.extension_bzl_file.clone()
                    };
                let mut ext = AggregatedExtension::new(&resolved_bzl_file, &usage.extension_name);
                ext.extension_id = ext_id.clone();
                ext
            });

            // Add tags from this module
            agg.add_module_tags(module_name, usage.tags.clone());

            // Add imported repos
            for import in &usage.imports {
                agg.add_imported_repos(import.repos.iter().cloned());
                agg.add_imported_repos(
                    import.repo_mapping.iter().map(|(_, actual)| actual.clone()),
                );
            }
        }
    }

    aggregated
}

/// Build the canonical extension id from a `use_extension()` call.
///
/// The two in-workspace shapes
///
/// - `use_extension("//:ext.bzl", "name")` inside module `X`
/// - `use_extension("@X//:ext.bzl", "name")` from anywhere else
///
/// must produce the same id, or the executor loads a partial aggregation
/// keyed under one shape and misses the other. Returns
/// `@X//:ext.bzl%name` in both cases.
pub fn canonical_extension_id(
    extension_bzl_file: &str,
    extension_name: &str,
    declaring_module_name: &str,
) -> String {
    let resolved = if extension_bzl_file.starts_with("//") && !declaring_module_name.is_empty() {
        format!("@{}{}", declaring_module_name, extension_bzl_file)
    } else if let Some(rest) = extension_bzl_file.strip_prefix(':') {
        if declaring_module_name.is_empty() {
            format!("//:{rest}")
        } else {
            format!("@{}//:{}", declaring_module_name, rest)
        }
    } else {
        extension_bzl_file.to_owned()
    };
    format!("{resolved}%{extension_name}")
}

/// Result of executing a module extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionResult {
    /// The extension identifier.
    pub extension_id: String,

    /// Hash of the extension inputs (for caching).
    pub input_hash: String,

    /// Generated repositories.
    pub generated_repos: HashMap<String, GeneratedRepo>,
}

/// A repository generated by a module extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedRepo {
    /// Repository name.
    pub name: String,

    /// Repository rule class (e.g., "http_archive", "new_local_repository").
    pub rule_class: String,

    /// Repository rule attributes.
    pub attributes: HashMap<String, serde_json::Value>,

    /// Path to the repository content (after fetching).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

/// Module information available in module_ctx.modules.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// The module name.
    pub name: String,

    /// The module version.
    pub version: String,

    /// Whether this is the root module.
    pub is_root: bool,

    /// Tags applied by this module, grouped by tag class.
    pub tags: HashMap<String, Vec<ExtensionTag>>,
}

impl ModuleInfo {
    /// Create a new module info.
    pub fn new(name: String, version: String, is_root: bool) -> Self {
        Self {
            name,
            version,
            is_root,
            tags: HashMap::new(),
        }
    }

    /// Add a tag to this module.
    pub fn add_tag(&mut self, tag: ExtensionTag) {
        self.tags.entry(tag.tag_name.clone()).or_default().push(tag);
    }
}

/// Compute a hash of extension inputs for caching.
pub fn compute_extension_input_hash(extension: &AggregatedExtension) -> String {
    use sha2::Digest;
    use sha2::Sha256;

    let mut hasher = Sha256::new();

    // Hash the extension ID
    hasher.update(extension.extension_id.as_bytes());

    // Hash all tags (sorted for determinism)
    let mut module_names: Vec<_> = extension.tags_by_module.keys().collect();
    module_names.sort();

    for module_name in module_names {
        hasher.update(module_name.as_bytes());
        if let Some(tags) = extension.tags_by_module.get(module_name) {
            let mut tags: Vec<_> = tags.iter().collect();
            tags.sort_by_key(|tag| canonical_tag_hash_input(tag));
            for tag in tags {
                hash_extension_tag(tag, &mut hasher);
            }
        }
    }

    let hash = hasher.finalize();
    format!(
        "sha256-{}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, hash)
    )
}

fn canonical_tag_hash_input(tag: &ExtensionTag) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_extension_tag_hash_input(tag, &mut bytes);
    bytes
}

fn hash_extension_tag(tag: &ExtensionTag, hasher: &mut sha2::Sha256) {
    hasher.update(canonical_tag_hash_input(tag));
}

fn append_extension_tag_hash_input(tag: &ExtensionTag, bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(b"tag:");
    bytes.extend_from_slice(tag.tag_name.as_bytes());
    bytes.extend_from_slice(b"\0");

    let mut kwargs: Vec<_> = tag.kwargs.iter().collect();
    kwargs.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (key, value) in kwargs {
        bytes.extend_from_slice(b"kw:");
        bytes.extend_from_slice(key.as_bytes());
        bytes.extend_from_slice(b"=");
        append_tag_value_hash_input(value, bytes);
        bytes.extend_from_slice(b"\0");
    }
}

fn append_tag_value_hash_input(value: &TagValue, bytes: &mut Vec<u8>) {
    match value {
        TagValue::String(s) => {
            bytes.extend_from_slice(b"string:");
            bytes.extend_from_slice(s.as_bytes());
        }
        TagValue::Label(s) => {
            bytes.extend_from_slice(b"label:");
            bytes.extend_from_slice(s.as_bytes());
        }
        TagValue::Int(i) => {
            bytes.extend_from_slice(b"int:");
            bytes.extend_from_slice(&i.to_le_bytes());
        }
        TagValue::Bool(b) => {
            bytes.extend_from_slice(b"bool:");
            bytes.push(u8::from(*b));
        }
        TagValue::List(items) => {
            bytes.extend_from_slice(b"list:");
            bytes.extend_from_slice(&items.len().to_le_bytes());
            for item in items {
                append_tag_value_hash_input(item, bytes);
                bytes.extend_from_slice(b"\0");
            }
        }
        TagValue::Dict(entries) => {
            bytes.extend_from_slice(b"dict:");
            bytes.extend_from_slice(&entries.len().to_le_bytes());
            let mut entries: Vec<_> = entries.iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            for (key, value) in entries {
                bytes.extend_from_slice(key.as_bytes());
                bytes.extend_from_slice(b"=");
                append_tag_value_hash_input(value, bytes);
                bytes.extend_from_slice(b"\0");
            }
        }
        TagValue::None => {
            bytes.extend_from_slice(b"none");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UseRepo;

    #[test]
    fn test_aggregated_extension_new() {
        let agg = AggregatedExtension::new("@rules_python//python/extensions:pip.bzl", "pip");
        assert_eq!(agg.extension_name, "pip");
        assert_eq!(
            agg.extension_id,
            "@rules_python//python/extensions:pip.bzl%pip"
        );
    }

    #[test]
    fn test_aggregated_extension_add_tags() {
        let mut agg = AggregatedExtension::new("test.bzl", "ext");

        let tag1 = ExtensionTag::new("install".to_string());
        let tag2 = ExtensionTag::new("parse".to_string());

        agg.add_module_tags("module_a", vec![tag1]);
        agg.add_module_tags("module_b", vec![tag2]);

        assert_eq!(agg.tags_by_module.len(), 2);
        assert_eq!(agg.all_tags().len(), 2);
    }

    #[test]
    fn test_aggregate_extensions() {
        let mut ext1 = ExtensionUsage::new("@rules_python//pip.bzl".to_string(), "pip".to_string());
        ext1.tags.push(ExtensionTag::new("parse".to_string()));
        ext1.imports
            .push(UseRepo::new().add_repo("pip".to_string()));

        let mut ext2 = ExtensionUsage::new("@rules_python//pip.bzl".to_string(), "pip".to_string());
        ext2.tags.push(ExtensionTag::new("install".to_string()));

        let mut module_extensions = HashMap::new();
        module_extensions.insert("root".to_string(), vec![ext1]);
        module_extensions.insert("dep_a".to_string(), vec![ext2]);

        let aggregated = aggregate_extensions(&module_extensions);

        assert_eq!(aggregated.len(), 1);
        let pip_ext = aggregated.get("@rules_python//pip.bzl%pip").unwrap();
        assert_eq!(pip_ext.tags_by_module.len(), 2);
        assert_eq!(pip_ext.all_tags().len(), 2);
        assert_eq!(pip_ext.imported_repos, vec!["pip"]);
    }

    #[test]
    fn test_module_info() {
        let mut info = ModuleInfo::new("test".to_string(), "1.0.0".to_string(), true);

        let tag1 = ExtensionTag::new("parse".to_string());
        let tag2 = ExtensionTag::new("parse".to_string());
        let tag3 = ExtensionTag::new("install".to_string());

        info.add_tag(tag1);
        info.add_tag(tag2);
        info.add_tag(tag3);

        assert_eq!(info.tags.get("parse").unwrap().len(), 2);
        assert_eq!(info.tags.get("install").unwrap().len(), 1);
    }

    #[test]
    fn test_compute_extension_input_hash() {
        let mut agg = AggregatedExtension::new("test.bzl", "ext");
        agg.add_module_tags("mod", vec![ExtensionTag::new("tag".to_string())]);

        let hash = compute_extension_input_hash(&agg);
        assert!(hash.starts_with("sha256-"));
    }

    #[test]
    fn test_compute_extension_input_hash_includes_kwarg_values() {
        let mut first = AggregatedExtension::new("@rules_rs//:toolchains.bzl", "toolchains");
        first.add_module_tags(
            "root",
            vec![
                ExtensionTag::new("toolchain".to_owned())
                    .with_kwarg("version".to_owned(), TagValue::String("1.94.1".to_owned()))
                    .with_kwarg("edition".to_owned(), TagValue::String("2021".to_owned())),
            ],
        );

        let mut second = AggregatedExtension::new("@rules_rs//:toolchains.bzl", "toolchains");
        second.add_module_tags(
            "root",
            vec![
                ExtensionTag::new("toolchain".to_owned())
                    .with_kwarg("version".to_owned(), TagValue::String("1.95.0".to_owned()))
                    .with_kwarg("edition".to_owned(), TagValue::String("2024".to_owned())),
            ],
        );

        assert_ne!(
            compute_extension_input_hash(&first),
            compute_extension_input_hash(&second)
        );
    }

    #[test]
    fn test_compute_extension_input_hash_sorts_tags_by_content() {
        let tag_a = ExtensionTag::new("crate".to_owned())
            .with_kwarg("name".to_owned(), TagValue::String("a".to_owned()));
        let tag_b = ExtensionTag::new("crate".to_owned())
            .with_kwarg("name".to_owned(), TagValue::String("b".to_owned()));

        let mut first = AggregatedExtension::new("@rules_rs//:crates.bzl", "crates");
        first.add_module_tags("root", vec![tag_a.clone(), tag_b.clone()]);

        let mut second = AggregatedExtension::new("@rules_rs//:crates.bzl", "crates");
        second.add_module_tags("root", vec![tag_b, tag_a]);

        assert_eq!(
            compute_extension_input_hash(&first),
            compute_extension_input_hash(&second)
        );
    }
}
