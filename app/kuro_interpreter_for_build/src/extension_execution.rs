/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Module extension execution engine.
//!
//! Plan Reference: `thoughts/shared/plans/kuro-bazel-subplans/02-bzlmod.md` Phase 5
//!
//! This module provides the execution engine for Bazel module extensions. When a project
//! uses `use_extension()` in its MODULE.bazel, this engine:
//!
//! 1. Loads the extension's .bzl file
//! 2. Extracts the `module_extension` value
//! 3. Builds a `module_ctx` with aggregated tag data from all modules
//! 4. Invokes the implementation function
//! 5. Captures generated repositories
//!
//! ## Architecture
//!
//! Extension execution bridges two systems:
//! - `kuro_bzlmod`: Parses MODULE.bazel files and aggregates extension usages
//! - `kuro_interpreter_for_build`: Provides Starlark evaluation for .bzl files
//!
//! The flow is:
//! ```text
//! MODULE.bazel parsing (kuro_bzlmod)
//!   ↓
//! ExtensionUsage, ExtensionTag collected
//!   ↓
//! aggregate_extensions() groups tags by extension
//!   ↓
//! ExtensionExecutor loads .bzl file
//!   ↓
//! Build module_ctx from aggregated data
//!   ↓
//! Invoke implementation function
//!   ↓
//! Capture generated repositories
//! ```

use std::collections::HashMap;

use allocative::Allocative;
use derive_more::Display;
use kuro_bzlmod::extensions::AggregatedExtension;
use kuro_bzlmod::extensions::ExtensionResult;
use kuro_bzlmod::extensions::GeneratedRepo;
use kuro_bzlmod::extensions::ModuleInfo;
use kuro_bzlmod::types::ExtensionTag;
use kuro_bzlmod::types::TagValue;
use starlark::values::Heap;
use starlark::values::Value;

use crate::module_ctx::BazelModule;
use crate::module_ctx::ModuleContext;
use crate::module_ctx::SerializedModule;
use crate::module_ctx::SerializedTag;
use crate::module_ctx::SerializedTagValue;

/// Error types for extension execution.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
pub enum ExtensionExecutionError {
    #[error("Extension not found: `{0}` in file `{1}`")]
    ExtensionNotFound(String, String),
    #[error("Value `{0}` is not a module_extension")]
    NotAModuleExtension(String),
    #[error("Failed to load extension file: {0}")]
    LoadFailed(String),
    #[error("Extension execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Tag class `{0}` not defined in extension")]
    UndefinedTagClass(String),
}

/// Convert a TagValue from kuro_bzlmod to SerializedTagValue.
pub fn tag_value_to_serialized(tag_value: &TagValue) -> SerializedTagValue {
    match tag_value {
        TagValue::String(s) => SerializedTagValue::String(s.clone()),
        TagValue::Int(i) => SerializedTagValue::Int(*i),
        TagValue::Bool(b) => SerializedTagValue::Bool(*b),
        TagValue::None => SerializedTagValue::None,
        TagValue::Label(s) => SerializedTagValue::Label(s.clone()),
        TagValue::List(items) => {
            SerializedTagValue::List(items.iter().map(tag_value_to_serialized).collect())
        }
        TagValue::Dict(entries) => SerializedTagValue::Dict(
            entries
                .iter()
                .map(|(k, v)| (k.clone(), tag_value_to_serialized(v)))
                .collect(),
        ),
    }
}

/// Convert a TagValue from kuro_bzlmod to a Starlark Value (for direct use).
pub fn tag_value_to_starlark<'v>(tag_value: &TagValue, heap: Heap<'v>) -> Value<'v> {
    tag_value_to_serialized(tag_value).to_starlark(heap)
}

/// Convert ExtensionTag to SerializedTag.
pub fn extension_tag_to_serialized(tag: &ExtensionTag) -> SerializedTag {
    let kwargs: Vec<(String, SerializedTagValue)> = tag
        .kwargs
        .iter()
        .map(|(k, v)| (k.clone(), tag_value_to_serialized(v)))
        .collect();
    SerializedTag::new(kwargs)
}

/// Convert ExtensionTag to a struct-like Starlark value (for direct use).
pub fn extension_tag_to_struct<'v>(tag: &ExtensionTag, heap: Heap<'v>) -> Value<'v> {
    extension_tag_to_serialized(tag).to_starlark_struct(heap)
}

/// Build a BazelModule from ModuleInfo with full tag data.
pub fn module_info_to_bazel_module(info: &ModuleInfo) -> BazelModule {
    let tags_by_class: HashMap<String, Vec<SerializedTag>> = info
        .tags
        .iter()
        .map(|(class_name, tags)| {
            let serialized_tags: Vec<SerializedTag> =
                tags.iter().map(extension_tag_to_serialized).collect();
            (class_name.clone(), serialized_tags)
        })
        .collect();

    BazelModule::with_tags(
        info.name.clone(),
        info.version.clone(),
        info.is_root,
        tags_by_class,
    )
}

/// Build a SerializedModule from ModuleInfo.
pub fn module_info_to_serialized(info: &ModuleInfo) -> SerializedModule {
    let tags_by_class: HashMap<String, Vec<SerializedTag>> = info
        .tags
        .iter()
        .map(|(class_name, tags)| {
            let serialized_tags: Vec<SerializedTag> =
                tags.iter().map(extension_tag_to_serialized).collect();
            (class_name.clone(), serialized_tags)
        })
        .collect();

    SerializedModule {
        name: info.name.clone(),
        version: info.version.clone(),
        is_root: info.is_root,
        tags_by_class,
    }
}

/// Build ModuleInfo from aggregated extension data.
///
/// This converts the aggregated tags-by-module format into the ModuleInfo
/// format expected by module_ctx.modules.
pub fn build_module_infos(
    aggregated: &AggregatedExtension,
    root_module_name: &str,
) -> Vec<ModuleInfo> {
    let mut infos: Vec<ModuleInfo> = aggregated
        .tags_by_module
        .iter()
        .map(|(module_name, tags)| {
            let is_root = module_name == root_module_name;
            let mut info = ModuleInfo::new(
                module_name.clone(),
                "0.0.0".to_string(), // TODO: Get actual version from resolved graph
                is_root,
            );
            for tag in tags {
                info.add_tag(tag.clone());
            }
            info
        })
        .collect();

    // Sort so root module comes first
    infos.sort_by(|a, b| match (a.is_root, b.is_root) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    infos
}

/// Build a ModuleContext from aggregated extension data.
///
/// This is the main entry point for converting kuro_bzlmod's aggregated
/// extension data into a module_ctx that can be passed to extension implementations.
pub fn build_module_context(
    aggregated: &AggregatedExtension,
    root_module_name: &str,
) -> ModuleContext {
    let infos = build_module_infos(aggregated, root_module_name);

    // Convert to serialized modules with full tag data
    let serialized_modules: Vec<SerializedModule> =
        infos.iter().map(module_info_to_serialized).collect();

    // Check if root module has a non-dev dependency (for now, assume true if root is present)
    let root_has_non_dev = infos.iter().any(|i| i.is_root);

    ModuleContext::from_serialized(serialized_modules, root_has_non_dev)
}

/// Build a ModuleContext with version information from a resolved graph.
///
/// This is the preferred method when you have access to the resolved dependency graph,
/// as it provides accurate version information.
pub fn build_module_context_with_versions(
    aggregated: &AggregatedExtension,
    root_module_name: &str,
    version_map: &HashMap<String, String>,
) -> ModuleContext {
    let mut infos = build_module_infos(aggregated, root_module_name);

    // Update versions from the map
    for info in &mut infos {
        if let Some(version) = version_map.get(&info.name) {
            info.version = version.clone();
        }
    }

    // Convert to serialized modules
    let serialized_modules: Vec<SerializedModule> =
        infos.iter().map(module_info_to_serialized).collect();

    let root_has_non_dev = infos.iter().any(|i| i.is_root);

    ModuleContext::from_serialized(serialized_modules, root_has_non_dev)
}

/// Extension execution context.
///
/// Tracks state during extension execution, including generated repositories.
#[derive(Debug, Default)]
pub struct ExtensionExecutionContext {
    /// Repositories generated during execution.
    pub generated_repos: HashMap<String, GeneratedRepo>,
}

impl ExtensionExecutionContext {
    /// Create a new execution context.
    pub fn new() -> Self {
        Self {
            generated_repos: HashMap::new(),
        }
    }

    /// Register a generated repository.
    pub fn register_repo(
        &mut self,
        name: String,
        rule_class: String,
        attributes: HashMap<String, serde_json::Value>,
    ) {
        self.generated_repos.insert(
            name.clone(),
            GeneratedRepo {
                name,
                rule_class,
                attributes,
                path: None,
            },
        );
    }

    /// Get the execution result.
    pub fn into_result(self, extension_id: String, input_hash: String) -> ExtensionResult {
        ExtensionResult {
            extension_id,
            input_hash,
            generated_repos: self.generated_repos,
        }
    }
}

/// Placeholder for extension executor.
///
/// Full implementation requires DICE integration to load .bzl files.
/// This struct provides the interface that will be implemented.
#[derive(Debug, Display, Allocative)]
#[display("<ExtensionExecutor {}>", extension_id)]
pub struct ExtensionExecutor {
    /// The extension identifier (bzl_file%name).
    extension_id: String,
    /// The .bzl file path.
    bzl_file: String,
    /// The extension name in the .bzl file.
    extension_name: String,
}

impl ExtensionExecutor {
    /// Create a new extension executor.
    pub fn new(bzl_file: String, extension_name: String) -> Self {
        let extension_id = format!("{}%{}", bzl_file, extension_name);
        Self {
            extension_id,
            bzl_file,
            extension_name,
        }
    }

    /// Get the extension identifier.
    pub fn extension_id(&self) -> &str {
        &self.extension_id
    }

    /// Get the .bzl file path.
    pub fn bzl_file(&self) -> &str {
        &self.bzl_file
    }

    /// Get the extension name.
    pub fn extension_name(&self) -> &str {
        &self.extension_name
    }
}

#[cfg(test)]
mod tests {
    use kuro_bzlmod::types::TagValue;

    use super::*;

    #[test]
    fn test_tag_value_serialization() {
        // Test string
        let serialized = tag_value_to_serialized(&TagValue::String("hello".to_string()));
        assert!(matches!(serialized, SerializedTagValue::String(s) if s == "hello"));

        // Test int
        let serialized = tag_value_to_serialized(&TagValue::Int(42));
        assert!(matches!(serialized, SerializedTagValue::Int(42)));

        // Test bool
        let serialized = tag_value_to_serialized(&TagValue::Bool(true));
        assert!(matches!(serialized, SerializedTagValue::Bool(true)));

        // Test none
        let serialized = tag_value_to_serialized(&TagValue::None);
        assert!(matches!(serialized, SerializedTagValue::None));

        // Test list
        let list = TagValue::List(vec![TagValue::String("a".to_string()), TagValue::Int(1)]);
        let serialized = tag_value_to_serialized(&list);
        assert!(matches!(serialized, SerializedTagValue::List(_)));
    }

    #[test]
    fn test_tag_value_to_starlark() {
        use starlark::environment::Module;

        let module = Module::new();
        let heap = module.heap();

        // Test string
        let v = tag_value_to_starlark(&TagValue::String("hello".to_string()), heap);
        assert_eq!(v.unpack_str(), Some("hello"));

        // Test int
        let v = tag_value_to_starlark(&TagValue::Int(42), heap);
        assert_eq!(v.unpack_i32(), Some(42));

        // Test bool
        let v = tag_value_to_starlark(&TagValue::Bool(true), heap);
        assert_eq!(v.unpack_bool(), Some(true));

        // Test none
        let v = tag_value_to_starlark(&TagValue::None, heap);
        assert!(v.is_none());
    }

    #[test]
    fn test_extension_tag_serialization() {
        let mut tag = ExtensionTag::new("install".to_string());
        tag.kwargs
            .push(("name".to_string(), TagValue::String("foo".to_string())));
        tag.kwargs
            .push(("version".to_string(), TagValue::String("1.0".to_string())));

        let serialized = extension_tag_to_serialized(&tag);
        assert_eq!(serialized.kwargs.len(), 2);
        assert!(
            matches!(&serialized.kwargs[0], (k, SerializedTagValue::String(v)) if k == "name" && v == "foo")
        );
    }

    #[test]
    fn test_build_module_infos() {
        let mut aggregated = AggregatedExtension::new("test.bzl", "ext");

        let tag1 = ExtensionTag::new("install".to_string());
        let tag2 = ExtensionTag::new("parse".to_string());

        aggregated.add_module_tags("root", vec![tag1]);
        aggregated.add_module_tags("dep_a", vec![tag2]);

        let infos = build_module_infos(&aggregated, "root");

        // Root should come first
        assert_eq!(infos[0].name, "root");
        assert!(infos[0].is_root);
        assert_eq!(infos[1].name, "dep_a");
        assert!(!infos[1].is_root);
    }

    #[test]
    fn test_build_module_context() {
        let mut aggregated = AggregatedExtension::new("test.bzl", "ext");

        let mut tag = ExtensionTag::new("install".to_string());
        tag.kwargs
            .push(("name".to_string(), TagValue::String("mylib".to_string())));

        aggregated.add_module_tags("root", vec![tag]);

        let ctx = build_module_context(&aggregated, "root");

        // Verify modules were created
        let modules = ctx.get_modules();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name, "root");
        assert!(modules[0].is_root);

        // Verify tag data is present
        let install_tags = modules[0].tags_by_class.get("install").unwrap();
        assert_eq!(install_tags.len(), 1);
        assert_eq!(install_tags[0].kwargs.len(), 1);
    }

    #[test]
    fn test_build_module_context_with_versions() {
        let mut aggregated = AggregatedExtension::new("test.bzl", "ext");
        aggregated.add_module_tags("root", vec![ExtensionTag::new("config".to_string())]);
        aggregated.add_module_tags("dep_a", vec![ExtensionTag::new("install".to_string())]);

        let mut versions = HashMap::new();
        versions.insert("root".to_string(), "1.0.0".to_string());
        versions.insert("dep_a".to_string(), "2.0.0".to_string());

        let ctx = build_module_context_with_versions(&aggregated, "root", &versions);

        let modules = ctx.get_modules();

        // Root comes first
        assert_eq!(modules[0].name, "root");
        assert_eq!(modules[0].version, "1.0.0");

        assert_eq!(modules[1].name, "dep_a");
        assert_eq!(modules[1].version, "2.0.0");
    }

    #[test]
    fn test_extension_executor_new() {
        let exec = ExtensionExecutor::new(
            "@rules_python//extensions:pip.bzl".to_string(),
            "pip".to_string(),
        );

        assert_eq!(exec.extension_id(), "@rules_python//extensions:pip.bzl%pip");
        assert_eq!(exec.bzl_file(), "@rules_python//extensions:pip.bzl");
        assert_eq!(exec.extension_name(), "pip");
    }
}
