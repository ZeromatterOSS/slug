/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! `BazelModule` — represents a single module in the dependency graph,
//! exposed as entries in `module_ctx.modules`.

use std::collections::HashMap;

use allocative::Allocative;
use derive_more::Display;
use starlark::any::ProvidesStaticType;
use starlark::starlark_simple_value;
use starlark::typing::Ty;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

use crate::module_ctx::tags::BazelModuleTags;
use crate::module_ctx::tags::SerializedTag;

// ============================================================================
// BazelModule - Represents a module in the dependency graph (simple value)
// ============================================================================

/// Represents a module in the dependency graph with its tags.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<bazel_module {} {}>", name, version)]
pub struct BazelModule {
    /// Module name (e.g., "rules_python").
    pub(super) name: String,
    /// Module version (e.g., "0.31.0").
    pub(super) version: String,
    /// Whether this is the root module.
    pub(super) is_root: bool,
    /// Tags grouped by tag class name.
    pub(super) tags_by_class: HashMap<String, Vec<SerializedTag>>,
}

starlark_simple_value!(BazelModule);

impl BazelModule {
    /// Create from tag class names only (backward compatibility, empty tags).
    pub fn new(name: String, version: String, is_root: bool, tag_classes: Vec<String>) -> Self {
        let mut tags_by_class = HashMap::new();
        for class in tag_classes {
            tags_by_class.insert(class, Vec::new());
        }
        Self {
            name,
            version,
            is_root,
            tags_by_class,
        }
    }

    /// Create with actual tag data.
    pub fn with_tags(
        name: String,
        version: String,
        is_root: bool,
        tags_by_class: HashMap<String, Vec<SerializedTag>>,
    ) -> Self {
        Self {
            name,
            version,
            is_root,
            tags_by_class,
        }
    }

    /// Get the module name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the module version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Check if this is the root module.
    pub fn is_root(&self) -> bool {
        self.is_root
    }

    /// Get the tags by class.
    pub fn tags_by_class(&self) -> &HashMap<String, Vec<SerializedTag>> {
        &self.tags_by_class
    }
}

#[starlark_value(type = "bazel_module")]
impl<'v> StarlarkValue<'v> for BazelModule {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "name" | "version" | "is_root" | "tags" | "repo_name" | "bazel_module_repo_name"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc(&self.name as &str)),
            "version" => Some(heap.alloc(&self.version as &str)),
            "is_root" => Some(Value::new_bool(self.is_root)),
            "tags" => Some(heap.alloc(BazelModuleTags::with_tags(self.tags_by_class.clone()))),
            // The canonical repo name used for the module's repository.
            // For root module this is usually "" or the module name.
            "repo_name" | "bazel_module_repo_name" => Some(heap.alloc(&self.name as &str)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "name".to_owned(),
            "version".to_owned(),
            "is_root".to_owned(),
            "tags".to_owned(),
            "repo_name".to_owned(),
            "bazel_module_repo_name".to_owned(),
        ]
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}
