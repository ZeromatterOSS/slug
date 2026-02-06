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
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::list::AllocList;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;

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

        // Store direct elements as a list
        let direct_list = heap.alloc(AllocList(direct.items));

        // Store transitive depsets as a list
        let transitive_list = heap.alloc(AllocList(transitive.items));

        // Create the live depset
        Ok(heap.alloc(LiveDepsetGen {
            direct: direct_list,
            transitive: transitive_list,
            order: order.to_owned(),
        }))
    }
}
