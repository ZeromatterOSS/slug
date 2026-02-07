/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible depset implementation.
//!
//! In Bazel, a depset is an immutable collection that supports efficient
//! union operations. This is similar to Kuro's transitive_set but with
//! a different API.

use std::fmt;
use std::fmt::Display;
use std::sync::OnceLock;

use allocative::Allocative;
use starlark::coerce::Coerce;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Freeze;
use starlark::values::FrozenValue;
use starlark::values::FrozenHeap;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::OwnedFrozenValueTyped;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::list::AllocList;
use starlark::values::list::ListRef;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::starlark_value;
use starlark::values::FrozenValueTyped;

use kuro_core::bzl::ImportPath;
use kuro_core::cells::build_file_cell::BuildFileCell;
use kuro_core::cells::cell_path::CellPath;
use kuro_core::cells::name::CellName;
use kuro_core::cells::paths::CellRelativePathBuf;

use crate::interpreter::rule_defs::transitive_set::FrozenTransitiveSetDefinition;
use crate::interpreter::rule_defs::transitive_set::transitive_set_definition::builtin_definition;

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum DepsetError {
    #[error(
        "depset order must be one of `default`, `preorder`, `postorder`, `topological`, got `{order}`"
    )]
    InvalidOrder { order: String },
    #[error("depset transitive elements must be depsets")]
    TransitiveNotDepset,
    #[error("depset transitive elements must all have the same order, got `{first}` and `{second}`")]
    TransitiveOrderMismatch { first: String, second: String },
    #[error("depset order `{order}` is incompatible with transitive depset order `{transitive_order}`")]
    OrderIncompatible {
        order: String,
        transitive_order: String,
    },
}

pub fn bazel_depset_tset_definition(
) -> kuro_error::Result<&'static OwnedFrozenValueTyped<FrozenTransitiveSetDefinition>> {
    static DEF: OnceLock<OwnedFrozenValueTyped<FrozenTransitiveSetDefinition>> = OnceLock::new();
    DEF.get_or_try_init(|| {
        let cell = CellName::unchecked_new("kuro_builtin")?;
        let cell_path = CellPath::new(
            cell,
            CellRelativePathBuf::unchecked_new("builtin/depset.bzl".to_owned()),
        );
        let import_path =
            ImportPath::new_with_build_file_cells(cell_path, BuildFileCell::new(cell))?;
        let definition = builtin_definition("BazelDepsetTset", import_path)?;
        let heap = FrozenHeap::new();
        let value = FrozenValueTyped::new(heap.alloc_simple(definition))
            .expect("frozen depset tset definition");
        Ok(unsafe { OwnedFrozenValueTyped::new(heap.into_ref(), value) })
    })
}

// ============================================================================
// FrozenDepset - Immutable depset for frozen modules
// ============================================================================

/// A Bazel-compatible depset using FrozenValues (for already-frozen modules).
///
/// This is used for depsets that have been frozen (e.g., from loaded modules).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct Depset {
    /// Direct elements in this depset (stored as frozen values).
    direct: Vec<FrozenValue>,
    /// Child depsets whose elements are transitively included.
    children: Vec<FrozenValue>,
    /// Iteration order: "default", "preorder", "postorder", "topological".
    #[allow(dead_code)]
    order: String,
}

impl Depset {
    /// Create an empty depset.
    pub fn empty() -> Self {
        Self {
            direct: Vec::new(),
            children: Vec::new(),
            order: "default".to_owned(),
        }
    }

    /// Create a depset with direct elements from frozen values.
    pub fn from_frozen_values(elements: Vec<FrozenValue>, order: String) -> Self {
        Self {
            direct: elements,
            children: Vec::new(),
            order,
        }
    }

    /// Create a depset with direct elements and transitive children.
    pub fn new(direct: Vec<FrozenValue>, children: Vec<FrozenValue>, order: String) -> Self {
        Self {
            direct,
            children,
            order,
        }
    }

    pub fn direct_values(&self) -> &[FrozenValue] {
        &self.direct
    }

    pub fn children_values(&self) -> &[FrozenValue] {
        &self.children
    }

    pub fn order_str(&self) -> &str {
        &self.order
    }

    /// Collect all elements from this depset and its transitive children.
    /// Uses preorder traversal (direct elements first, then transitive).
    pub fn collect_all_frozen(&self) -> Vec<FrozenValue> {
        let mut result = Vec::new();
        self.collect_frozen_recursive(&mut result);
        result
    }

    fn collect_frozen_recursive(&self, result: &mut Vec<FrozenValue>) {
        // Add direct elements first
        for elem in &self.direct {
            result.push(*elem);
        }
        // Then recurse into transitive children
        for child in &self.children {
            if let Some(child_depset) = child.downcast_ref::<Depset>() {
                child_depset.collect_frozen_recursive(result);
            }
        }
    }

    /// Check if the depset is empty.
    pub fn is_empty(&self) -> bool {
        self.direct.is_empty()
            && self
                .children
                .iter()
                .all(|c| c.downcast_ref::<Depset>().is_some_and(|d| d.is_empty()))
    }

    /// Get the number of elements (including transitive).
    pub fn len(&self) -> usize {
        self.collect_all_frozen().len()
    }
}

impl Display for Depset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let elements = self.collect_all_frozen();
        if elements.is_empty() {
            write!(f, "depset([])")
        } else {
            write!(f, "depset([")?;
            for (i, elem) in elements.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", elem)?;
            }
            write!(f, "])")
        }
    }
}

starlark_simple_value!(Depset);

#[starlark_value(type = "depset")]
impl<'v> StarlarkValue<'v> for Depset {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(frozen_depset_methods)
    }

    fn to_bool(&self) -> bool {
        !self.is_empty()
    }

    fn length(&self) -> starlark::Result<i32> {
        Ok(self.len() as i32)
    }
}

/// Methods available on frozen depset objects.
#[starlark_module]
fn frozen_depset_methods(builder: &mut MethodsBuilder) {
    /// Return a list of all elements in the depset.
    fn to_list<'v>(#[starlark(this)] this: &Depset, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let elements: Vec<Value<'v>> = this
            .collect_all_frozen()
            .into_iter()
            .map(|v| v.to_value())
            .collect();
        Ok(heap.alloc(AllocList(elements)))
    }
}

// ============================================================================
// LiveDepset - Mutable depset for values during evaluation
// ============================================================================

/// A depset that stores live (non-frozen) values during evaluation.
///
/// This handles the case where values haven't been frozen yet.
/// When the module is frozen, this converts to a regular Depset.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct LiveDepsetGen<V: ValueLifetimeless> {
    /// Direct elements (stored as Values that freeze to FrozenValues)
    pub(crate) direct: V, // Actually a list value
    /// Transitive children (depsets)
    pub(crate) transitive: V, // Actually a list value
    /// Iteration order
    #[freeze(identity)]
    order: String,
}

impl<V: ValueLifetimeless> LiveDepsetGen<V> {
    pub(crate) fn new(direct: V, transitive: V, order: String) -> Self {
        Self {
            direct,
            transitive,
            order,
        }
    }
}

starlark::starlark_complex_value!(pub LiveDepset);

impl<V: ValueLifetimeless> Display for LiveDepsetGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "depset([...])")
    }
}

// Generic implementation for LiveDepsetGen that works with both unfrozen (Value) and frozen (FrozenValue) variants
#[starlark::values::starlark_value(type = "depset")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for LiveDepsetGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_methods() -> Option<&'static Methods> {
        // Use the generic method implementation that handles all depset variants
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(generic_live_depset_methods)
    }

    fn to_bool(&self) -> bool {
        // A depset is truthy iff it is non-empty.
        // Check direct elements first.
        let direct_val = self.direct.to_value();
        if direct_val.length().unwrap_or(0) > 0 {
            return true;
        }
        // Check transitive children - if any transitive child is non-empty, we're non-empty.
        let transitive_val = self.transitive.to_value();
        let trans_len = transitive_val.length().unwrap_or(0);
        if trans_len == 0 {
            return false;
        }
        // We have transitive children. Check if any are non-empty.
        // Use ListRef to iterate without needing a Heap.
        if let Some(list) = starlark::values::list::ListRef::from_value(transitive_val) {
            for child in list.iter() {
                if child.to_bool() {
                    return true;
                }
            }
        }
        false
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "direct" | "transitive")
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "direct" => Some(self.direct.to_value()),
            "transitive" => Some(self.transitive.to_value()),
            _ => None,
        }
    }
}

/// Helper function to recursively collect elements from any depset type.
pub(crate) fn collect_depset_elements<'v>(
    value: Value<'v>,
    elements: &mut Vec<Value<'v>>,
    heap: Heap<'v>,
) {
    // Try unfrozen live depset first
    if let Some(live) = value.downcast_ref::<LiveDepsetGen<Value>>() {
        // Collect direct elements
        if let Ok(direct_iter) = live.direct.iterate(heap) {
            for elem in direct_iter {
                elements.push(elem);
            }
        }
        // Recursively collect transitive
        if let Ok(trans_iter) = live.transitive.iterate(heap) {
            for child in trans_iter {
                collect_depset_elements(child, elements, heap);
            }
        }
    }
    // Try regular frozen depset
    else if let Some(frozen_depset) = value.downcast_ref::<Depset>() {
        for elem in frozen_depset.collect_all_frozen() {
            elements.push(elem.to_value());
        }
    }
    // If type is "depset", try to access via attributes (handles frozen LiveDepset)
    else if value.get_type() == "depset" {
        // Use get_attr to access direct and transitive fields
        if let Some(direct_attr) = value.get_attr("direct", heap).ok().flatten() {
            if let Ok(direct_iter) = direct_attr.iterate(heap) {
                for elem in direct_iter {
                    elements.push(elem);
                }
            }
        }
        if let Some(trans_attr) = value.get_attr("transitive", heap).ok().flatten() {
            if let Ok(trans_iter) = trans_attr.iterate(heap) {
                for child in trans_iter {
                    collect_depset_elements(child, elements, heap);
                }
            }
        }
    }
    // Else: not a depset, ignore
}

pub fn depset_direct_and_transitive<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<(Vec<Value<'v>>, Vec<Value<'v>>)> {
    if let Some(live) = value.downcast_ref::<LiveDepsetGen<Value<'v>>>() {
        let mut direct = Vec::new();
        if let Ok(iter) = live.direct.iterate(heap) {
            for item in iter {
                direct.push(item);
            }
        }
        let mut transitive = Vec::new();
        if let Ok(iter) = live.transitive.iterate(heap) {
            for item in iter {
                transitive.push(item);
            }
        }
        return Ok((direct, transitive));
    }

    if let Some(live) = value.downcast_ref::<LiveDepsetGen<FrozenValue>>() {
        let mut direct = Vec::new();
        if let Some(list) = ListRef::from_value(live.direct.to_value()) {
            for item in list.iter() {
                direct.push(item);
            }
        }
        let mut transitive = Vec::new();
        if let Some(list) = ListRef::from_value(live.transitive.to_value()) {
            for item in list.iter() {
                transitive.push(item);
            }
        }
        return Ok((direct, transitive));
    }

    if let Some(depset) = value.downcast_ref::<Depset>() {
        let direct = depset
            .direct_values()
            .iter()
            .map(|v| v.to_value())
            .collect::<Vec<_>>();
        let transitive = depset
            .children_values()
            .iter()
            .map(|v| v.to_value())
            .collect::<Vec<_>>();
        return Ok((direct, transitive));
    }

    if value.get_type() == "depset" {
        let mut direct = Vec::new();
        if let Some(direct_attr) = value.get_attr("direct", heap).ok().flatten() {
            if let Ok(iter) = direct_attr.iterate(heap) {
                for item in iter {
                    direct.push(item);
                }
            }
        }
        let mut transitive = Vec::new();
        if let Some(trans_attr) = value.get_attr("transitive", heap).ok().flatten() {
            if let Ok(iter) = trans_attr.iterate(heap) {
                for item in iter {
                    transitive.push(item);
                }
            }
        }
        return Ok((direct, transitive));
    }

    Err(kuro_error::Error::from(DepsetError::TransitiveNotDepset).into())
}

// Removed live_depset_methods - using generic_live_depset_methods for all cases

/// Generic methods for depsets that use Value and handle any depset variant.
/// This handles both LiveDepset and FrozenLiveDepset via the Value interface.
#[starlark_module]
fn generic_live_depset_methods(builder: &mut MethodsBuilder) {
    /// Return a list of all elements in the depset.
    fn to_list<'v>(
        #[starlark(this)] this: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let mut elements: Vec<Value<'v>> = Vec::new();
        // Use the generic collector which handles all depset variants
        collect_depset_elements(this, &mut elements, heap);
        Ok(heap.alloc(AllocList(elements)))
    }
}

fn depset_order_from_value<'v>(value: Value<'v>) -> Option<&'v str> {
    if let Some(live) = value.downcast_ref::<LiveDepsetGen<Value<'v>>>() {
        return Some(live.order.as_str());
    }
    if let Some(live) = value.downcast_ref::<LiveDepsetGen<FrozenValue>>() {
        return Some(live.order.as_str());
    }
    if let Some(depset) = value.downcast_ref::<Depset>() {
        return Some(depset.order_str());
    }
    None
}

fn validate_depset_order<'v>(
    order: &str,
    transitive: &[Value<'v>],
) -> starlark::Result<String> {
    let mut effective_order = order.to_owned();
    match effective_order.as_str() {
        "default" | "preorder" | "postorder" | "topological" => {}
        _ => {
            return Err(
                kuro_error::Error::from(DepsetError::InvalidOrder {
                    order: order.to_owned(),
                })
                .into(),
            );
        }
    }

    let mut transitive_order: Option<String> = None;
    for item in transitive {
        let Some(item_order) = depset_order_from_value(*item) else {
            return Err(kuro_error::Error::from(DepsetError::TransitiveNotDepset).into());
        };
        if item_order == "default" {
            continue;
        }
        match &transitive_order {
            None => transitive_order = Some(item_order.to_owned()),
            Some(existing) => {
                if existing != item_order {
                    return Err(
                        kuro_error::Error::from(DepsetError::TransitiveOrderMismatch {
                            first: existing.clone(),
                            second: item_order.to_owned(),
                        })
                        .into(),
                    );
                }
            }
        }
    }

    if effective_order == "default" {
        if let Some(non_default) = transitive_order {
            effective_order = non_default;
        }
    } else if let Some(non_default) = transitive_order {
        if non_default != effective_order {
            return Err(
                kuro_error::Error::from(DepsetError::OrderIncompatible {
                    order: order.to_owned(),
                    transitive_order: non_default,
                })
                .into(),
            );
        }
    }

    Ok(effective_order)
}

pub fn make_depset_from_lists<'v>(
    heap: Heap<'v>,
    direct: Vec<Value<'v>>,
    transitive: Vec<Value<'v>>,
    order: &str,
) -> starlark::Result<Value<'v>> {
    let effective_order = validate_depset_order(order, &transitive)?;
    let direct_list = heap.alloc(AllocList(direct));
    let transitive_list = heap.alloc(AllocList(transitive));
    Ok(heap.alloc(LiveDepsetGen {
        direct: direct_list,
        transitive: transitive_list,
        order: effective_order,
    }))
}

// ============================================================================
// Registration
// ============================================================================

/// Register the depset global function.
#[starlark_module]
pub fn register_depset(globals: &mut GlobalsBuilder) {
    /// Create a depset (immutable set with efficient union).
    ///
    /// Args:
    ///     direct: Elements to include directly in this depset.
    ///     transitive: Other depsets whose elements should be transitively included.
    ///     order: Iteration order ("default", "preorder", "postorder", "topological").
    ///
    /// Returns:
    ///     A new depset containing the specified elements.
    fn depset<'v>(
        #[starlark(default = UnpackListOrTuple::default())] direct: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        transitive: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = "default")] order: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        make_depset_from_lists(heap, direct.items, transitive.items, order)
    }
}
