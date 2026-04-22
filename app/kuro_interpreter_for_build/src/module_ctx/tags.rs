/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Tag-value serialization and tag/tags-container types for Bazel module
//! extensions. Covers `SerializedTagValue`, `SerializedTag`, `TagInstance`
//! (a single `pip.parse(...)` call), and `BazelModuleTags` (the
//! `tag_class_name -> [tags]` container).

use std::collections::HashMap;

use allocative::Allocative;
use derive_more::Display;
use kuro_build_api::interpreter::rule_defs::bazel_label::BazelLabel;
use starlark::any::ProvidesStaticType;
use starlark::collections::SmallMap;
use starlark::starlark_simple_value;
use starlark::typing::Ty;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::dict::Dict;
use starlark::values::starlark_value;

// ============================================================================
// Tag value serialization for storage in simple values
// ============================================================================

/// Serialized tag value that can be stored in simple Starlark values.
/// This mirrors kuro_bzlmod::types::TagValue but is owned and serializable.
#[derive(Debug, Clone, Allocative)]
pub enum SerializedTagValue {
    String(String),
    Int(i64),
    Bool(bool),
    None,
    Label(String),
    List(Vec<SerializedTagValue>),
    Dict(Vec<(String, SerializedTagValue)>),
}

impl SerializedTagValue {
    /// Convert to a Starlark value.
    pub fn to_starlark<'v>(&self, heap: Heap<'v>) -> Value<'v> {
        match self {
            SerializedTagValue::String(s) => heap.alloc(s.as_str()),
            SerializedTagValue::Int(i) => heap.alloc(*i as i32),
            SerializedTagValue::Bool(b) => Value::new_bool(*b),
            SerializedTagValue::None => Value::new_none(),
            SerializedTagValue::Label(s) => heap.alloc(BazelLabel::parse(s)),
            SerializedTagValue::List(items) => {
                let values: Vec<Value<'v>> = items.iter().map(|v| v.to_starlark(heap)).collect();
                heap.alloc(values)
            }
            SerializedTagValue::Dict(entries) => {
                let mut map = SmallMap::new();
                for (k, v) in entries {
                    map.insert_hashed(
                        heap.alloc(k.as_str())
                            .get_hashed()
                            .expect("string is hashable"),
                        v.to_starlark(heap),
                    );
                }
                heap.alloc(Dict::new(map))
            }
        }
    }
}

/// Convert a CoercedAttr (from tag class attr default) to a SerializedTagValue.
///
/// This handles common attr types. For complex/unsupported types, returns None
/// (the attr will keep its existing "missing = None" behavior).
pub fn coerced_attr_to_serialized_tag_value(
    attr: &kuro_node::attrs::coerced_attr::CoercedAttr,
) -> Option<SerializedTagValue> {
    use kuro_node::attrs::coerced_attr::CoercedAttr;
    match attr {
        CoercedAttr::String(s) => Some(SerializedTagValue::String(s.to_string())),
        CoercedAttr::Int(i) => Some(SerializedTagValue::Int(*i)),
        CoercedAttr::Bool(b) => Some(SerializedTagValue::Bool(b.0)),
        CoercedAttr::None => Some(SerializedTagValue::None),
        CoercedAttr::List(list) => {
            let items: Vec<SerializedTagValue> = list
                .iter()
                .filter_map(coerced_attr_to_serialized_tag_value)
                .collect();
            Some(SerializedTagValue::List(items))
        }
        CoercedAttr::Dict(dict) => {
            let entries: Vec<(String, SerializedTagValue)> = dict
                .iter()
                .filter_map(|(k, v)| {
                    let key = coerced_attr_to_serialized_tag_value(k)?;
                    let val = coerced_attr_to_serialized_tag_value(v)?;
                    // Dict keys should be strings
                    if let SerializedTagValue::String(s) = key {
                        Some((s, val))
                    } else {
                        None
                    }
                })
                .collect();
            Some(SerializedTagValue::Dict(entries))
        }
        _ => None,
    }
}

/// Return a sensible empty default for a given attr type.
///
/// In Bazel, list/dict attrs without an explicit `default` parameter get `[]`/`{}`.
/// This function provides those implicit defaults so tag instances don't return None
/// for unspecified container attributes.
pub fn default_for_attr_type(
    attr_type: &kuro_node::attrs::attr_type::AttrType,
) -> Option<SerializedTagValue> {
    use kuro_node::attrs::attr_type::AttrTypeInner;
    match &attr_type.0.inner {
        AttrTypeInner::List(_) => Some(SerializedTagValue::List(vec![])),
        AttrTypeInner::Dict(_) => Some(SerializedTagValue::Dict(vec![])),
        AttrTypeInner::String(_) => Some(SerializedTagValue::String(String::new())),
        AttrTypeInner::Bool(_) => Some(SerializedTagValue::Bool(false)),
        AttrTypeInner::Int(_) => Some(SerializedTagValue::Int(0)),
        _ => None,
    }
}

/// Serialized extension tag with its attribute values.
#[derive(Debug, Clone, Allocative)]
pub struct SerializedTag {
    /// Keyword arguments passed to the tag.
    pub kwargs: Vec<(String, SerializedTagValue)>,
}

impl SerializedTag {
    /// Create a new serialized tag.
    pub fn new(kwargs: Vec<(String, SerializedTagValue)>) -> Self {
        Self { kwargs }
    }

    /// Convert to a Starlark struct value.
    pub fn to_starlark_struct<'v>(&self, heap: Heap<'v>) -> Value<'v> {
        let fields: HashMap<String, SerializedTagValue> = self
            .kwargs
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        heap.alloc(TagInstance { fields })
    }
}

// ============================================================================
// TagInstance - A tag instance that returns None for missing attributes
// ============================================================================

/// A tag instance from module extension tags. Unlike regular structs, accessing
/// a missing attribute returns None (matching Bazel behavior where tag attrs
/// have default values, typically None for optional attrs).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct TagInstance {
    fields: HashMap<String, SerializedTagValue>,
}

impl std::fmt::Display for TagInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tag_instance({})",
            self.fields.keys().cloned().collect::<Vec<_>>().join(", ")
        )
    }
}

starlark_simple_value!(TagInstance);

#[starlark_value(type = "struct")]
impl<'v> StarlarkValue<'v> for TagInstance {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        true
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match self.fields.get(attribute) {
            Some(v) => Some(v.to_starlark(heap)),
            None => Some(Value::new_none()),
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        self.fields.keys().cloned().collect()
    }

    fn get_type_starlark_repr() -> starlark::typing::Ty {
        starlark::typing::Ty::any()
    }
}

// ============================================================================
// BazelModuleTags - Collection of tags grouped by tag class (simple value)
// ============================================================================

/// Collection of tags from a module, grouped by tag class name.
/// Access like: `mod.tags.install` to get list of install tags.
///
/// Tags are stored as serialized data and converted to Starlark structs on access.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<bazel_module_tags>")]
pub struct BazelModuleTags {
    /// Tags grouped by tag class name.
    /// Key is tag class name (e.g., "install"), value is list of tags.
    tags_by_class: HashMap<String, Vec<SerializedTag>>,
}

starlark_simple_value!(BazelModuleTags);

impl BazelModuleTags {
    /// Create from tag class names only (for backward compatibility).
    pub fn new(tag_classes: Vec<String>) -> Self {
        let mut tags_by_class = HashMap::new();
        for class in tag_classes {
            tags_by_class.insert(class, Vec::new());
        }
        Self { tags_by_class }
    }

    /// Create with actual tag data.
    pub fn with_tags(tags_by_class: HashMap<String, Vec<SerializedTag>>) -> Self {
        Self { tags_by_class }
    }

    /// Create an empty tags collection.
    pub fn empty() -> Self {
        Self {
            tags_by_class: HashMap::new(),
        }
    }

    /// Add a tag to a tag class.
    pub fn add_tag(&mut self, tag_class: String, tag: SerializedTag) {
        self.tags_by_class.entry(tag_class).or_default().push(tag);
    }
}

#[starlark_value(type = "bazel_module_tags")]
impl<'v> StarlarkValue<'v> for BazelModuleTags {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        // All tag class names are valid attributes. Unknown ones return empty lists.
        // This is needed because the tag class names are defined by the extension
        // (in tag_classes={}), and a module may not use all tag classes.
        true
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        let tags = self.tags_by_class.get(attribute);
        let structs: Vec<Value<'v>> = tags
            .map(|t| t.iter().map(|tag| tag.to_starlark_struct(heap)).collect())
            .unwrap_or_default();
        Some(heap.alloc(structs))
    }

    fn dir_attr(&self) -> Vec<String> {
        self.tags_by_class.keys().cloned().collect()
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}
