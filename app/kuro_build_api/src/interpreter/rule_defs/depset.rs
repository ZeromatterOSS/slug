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

use std::collections::HashSet;
use std::fmt;
use std::fmt::Display;
use std::sync::OnceLock;

use allocative::Allocative;
use kuro_core::bzl::ImportPath;
use kuro_core::cells::build_file_cell::BuildFileCell;
use kuro_core::cells::cell_path::CellPath;
use kuro_core::cells::name::CellName;
use kuro_core::cells::paths::CellRelativePathBuf;
use starlark::coerce::Coerce;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Freeze;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::FrozenValueTyped;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::OwnedFrozenValueTyped;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueIdentity;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::list::AllocList;
use starlark::values::list::ListRef;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneOr;
use starlark::values::starlark_value;

use crate::interpreter::rule_defs::nested_set::NestedSetOrder;
use crate::interpreter::rule_defs::nested_set::collect_nested_set;
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
    #[error(
        "depset order `{order}` is incompatible with transitive depset order `{transitive_order}`"
    )]
    OrderIncompatible {
        order: String,
        transitive_order: String,
    },
    #[error("depset elements must not be mutable values")]
    MutableElement,
    #[error("cannot add an item of type `{item_type}` to a depset of `{depset_type}`")]
    ElementTypeMismatch {
        item_type: String,
        depset_type: String,
    },
    #[error(
        "in call to len(), parameter 'x' got value of type 'depset', want 'iterable or string'"
    )]
    LenUnsupported,
    #[error("unsupported binary operation: depset | depset")]
    BitOrUnsupported,
}

pub fn bazel_depset_tset_definition()
-> kuro_error::Result<&'static OwnedFrozenValueTyped<FrozenTransitiveSetDefinition>> {
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
        self.collect_all_frozen_ordered(&self.order)
    }

    /// Collect all elements with a specific traversal order.
    pub fn collect_all_frozen_ordered(&self, order: &str) -> Vec<FrozenValue> {
        let order = NestedSetOrder::parse(order).unwrap_or(NestedSetOrder::Default);
        collect_nested_set(
            self,
            order,
            |depset| depset as *const Depset as usize,
            |depset| depset.direct.clone(),
            |depset| {
                depset
                    .children
                    .iter()
                    .filter_map(|child| child.downcast_ref::<Depset>())
                    .collect()
            },
        )
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

fn dedupe_values_preserving_order<'v>(
    elements: Vec<Value<'v>>,
) -> starlark::Result<Vec<Value<'v>>> {
    let mut deduped: Vec<Value<'v>> = Vec::with_capacity(elements.len());
    for value in elements {
        let mut is_dup = false;
        for existing in &deduped {
            if existing.equals(value)? {
                is_dup = true;
                break;
            }
        }
        if !is_dup {
            deduped.push(value);
        }
    }
    Ok(deduped)
}

fn depset_to_list_checkpoint(
    checkpoint: &'static str,
    direct_len: usize,
    transitive_len: usize,
    collected_len: usize,
    deduped_len: usize,
) {
    kuro_util::memory_checkpoint::checkpoint(
        checkpoint,
        [
            ("direct_len", direct_len),
            ("transitive_len", transitive_len),
            ("collected_len", collected_len),
            ("deduped_len", deduped_len),
            ("duplicate_len", collected_len.saturating_sub(deduped_len)),
        ],
    );
}

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
        Err(kuro_error::Error::from(DepsetError::LenUnsupported).into())
    }

    fn bit_or(&self, _other: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Err(kuro_error::Error::from(DepsetError::BitOrUnsupported).into())
    }
}

/// Methods available on frozen depset objects.
#[starlark_module]
fn frozen_depset_methods(builder: &mut MethodsBuilder) {
    /// Return a list of all elements in the depset.
    fn to_list<'v>(#[starlark(this)] this: &Depset, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let collected: Vec<Value<'v>> = this
            .collect_all_frozen()
            .into_iter()
            .map(|v| v.to_value())
            .collect();
        // Bazel depsets deduplicate elements by value equality while preserving
        // insertion order. Use Value::equals (which short-circuits on ptr_eq) to
        // merge values that are semantically equal, since arbitrary Starlark
        // values are not Hash-stable and Display-string collisions would
        // incorrectly merge distinct values.
        let collected_len = collected.len();
        let deduped = dedupe_values_preserving_order(collected)?;
        depset_to_list_checkpoint(
            "depset_to_list_frozen",
            this.direct.len(),
            this.children.len(),
            collected_len,
            deduped.len(),
        );
        Ok(heap.alloc(AllocList(deduped)))
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

    fn length(&self) -> starlark::Result<i32> {
        Err(kuro_error::Error::from(DepsetError::LenUnsupported).into())
    }

    fn bit_or(&self, _other: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Err(kuro_error::Error::from(DepsetError::BitOrUnsupported).into())
    }
}

enum DepsetView<'v> {
    Live(&'v LiveDepsetGen<Value<'v>>),
    FrozenLive(&'v LiveDepsetGen<FrozenValue>),
    Frozen(&'v Depset),
}

impl<'v> DepsetView<'v> {
    fn from_value(value: Value<'v>) -> Option<Self> {
        if let Some(live) = value.downcast_ref::<LiveDepsetGen<Value<'v>>>() {
            return Some(Self::Live(live));
        }
        if let Some(live) = value.downcast_ref::<LiveDepsetGen<FrozenValue>>() {
            return Some(Self::FrozenLive(live));
        }
        value.downcast_ref::<Depset>().map(Self::Frozen)
    }

    fn direct_values(&self, heap: Heap<'v>) -> Vec<Value<'v>> {
        match self {
            Self::Live(live) => live
                .direct
                .iterate(heap)
                .map(|iter| iter.collect())
                .unwrap_or_default(),
            Self::FrozenLive(live) => ListRef::from_value(live.direct.to_value())
                .map(|list| list.iter().collect())
                .unwrap_or_default(),
            Self::Frozen(depset) => depset
                .direct_values()
                .iter()
                .map(|v| v.to_value())
                .collect(),
        }
    }

    fn child_values(&self, heap: Heap<'v>) -> Vec<Value<'v>> {
        match self {
            Self::Live(live) => live
                .transitive
                .iterate(heap)
                .map(|iter| iter.collect())
                .unwrap_or_default(),
            Self::FrozenLive(live) => ListRef::from_value(live.transitive.to_value())
                .map(|list| list.iter().collect())
                .unwrap_or_default(),
            Self::Frozen(depset) => depset
                .children_values()
                .iter()
                .map(|v| v.to_value())
                .collect(),
        }
    }

    fn order_str(&self) -> &'v str {
        match self {
            Self::Live(live) => live.order.as_str(),
            Self::FrozenLive(live) => live.order.as_str(),
            Self::Frozen(depset) => depset.order_str(),
        }
    }
}

/// Helper function to recursively collect elements from any depset type.
pub fn collect_depset_elements<'v>(
    value: Value<'v>,
    elements: &mut Vec<Value<'v>>,
    heap: Heap<'v>,
) {
    if let Some(order) = depset_order_from_value(value) {
        collect_depset_elements_ordered(value, elements, heap, order);
    }
}

pub fn depset_direct_and_transitive<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<(Vec<Value<'v>>, Vec<Value<'v>>)> {
    if let Some(depset) = DepsetView::from_value(value) {
        return Ok((depset.direct_values(heap), depset.child_values(heap)));
    }

    Err(kuro_error::Error::from(DepsetError::TransitiveNotDepset).into())
}

/// Recursively collect elements from any depset type, respecting traversal order.
fn collect_depset_elements_ordered<'v>(
    value: Value<'v>,
    elements: &mut Vec<Value<'v>>,
    heap: Heap<'v>,
    order: &str,
) {
    let order = NestedSetOrder::parse(order).unwrap_or(NestedSetOrder::Default);
    elements.extend(collect_nested_set(
        value,
        order,
        |depset| depset.identity(),
        |depset| depset_direct_values(depset, heap),
        |depset| depset_child_values(depset, heap),
    ));
}

fn depset_child_values<'v>(value: Value<'v>, heap: Heap<'v>) -> Vec<Value<'v>> {
    DepsetView::from_value(value)
        .map(|depset| depset.child_values(heap))
        .unwrap_or_default()
}

fn depset_direct_values<'v>(value: Value<'v>, heap: Heap<'v>) -> Vec<Value<'v>> {
    DepsetView::from_value(value)
        .map(|depset| depset.direct_values(heap))
        .unwrap_or_default()
}

/// Generic methods for depsets that use Value and handle any depset variant.
/// This handles both LiveDepset and FrozenLiveDepset via the Value interface.
#[starlark_module]
fn generic_live_depset_methods(builder: &mut MethodsBuilder) {
    /// Return a list of all elements in the depset.
    fn to_list<'v>(
        #[starlark(this)] this: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let order = depset_order_from_value(this).unwrap_or("default");
        let (direct_len, transitive_len) = if kuro_util::memory_checkpoint::enabled() {
            depset_direct_and_transitive(this, heap)
                .map(|(direct, transitive)| (direct.len(), transitive.len()))
                .unwrap_or((0, 0))
        } else {
            (0, 0)
        };
        let mut elements: Vec<Value<'v>> = Vec::new();
        collect_depset_elements_ordered(this, &mut elements, heap, order);
        // Bazel depsets deduplicate elements: depset(["a", "a"]).to_list() == ["a"]
        // Dedup by value equality (preserving insertion order) using
        // Value::equals, which short-circuits on ptr_eq. Using the Display
        // string would wrongly collapse distinct values whose representations
        // happen to coincide (e.g. artifacts at different configurations).
        let collected_len = elements.len();
        let deduped = dedupe_values_preserving_order(elements)?;
        depset_to_list_checkpoint(
            "depset_to_list_live",
            direct_len,
            transitive_len,
            collected_len,
            deduped.len(),
        );
        Ok(heap.alloc(AllocList(deduped)))
    }
}

fn depset_order_from_value<'v>(value: Value<'v>) -> Option<&'v str> {
    DepsetView::from_value(value).map(|depset| depset.order_str())
}

fn validate_depset_order<'v>(order: &str, transitive: &[Value<'v>]) -> starlark::Result<String> {
    let Some(parsed_order) = NestedSetOrder::parse(order) else {
        return Err(kuro_error::Error::from(DepsetError::InvalidOrder {
            order: order.to_owned(),
        })
        .into());
    };

    for item in transitive {
        let Some(item_order) = depset_order_from_value(*item) else {
            return Err(kuro_error::Error::from(DepsetError::TransitiveNotDepset).into());
        };
        let Some(item_order) = NestedSetOrder::parse(item_order) else {
            return Err(kuro_error::Error::from(DepsetError::OrderIncompatible {
                order: order.to_owned(),
                transitive_order: item_order.to_owned(),
            })
            .into());
        };
        if !parsed_order.is_compatible_with_child(item_order) {
            return Err(kuro_error::Error::from(DepsetError::OrderIncompatible {
                order: order.to_owned(),
                transitive_order: item_order.as_str().to_owned(),
            })
            .into());
        }
    }

    Ok(parsed_order.as_str().to_owned())
}

fn validate_direct_depset_element(value: Value) -> starlark::Result<String> {
    value
        .get_hashed()
        .map_err(|_| kuro_error::Error::from(DepsetError::MutableElement))?;
    Ok(value.get_type().to_owned())
}

fn merge_depset_element_type(
    expected: &mut Option<String>,
    item_type: String,
) -> starlark::Result<()> {
    match expected {
        None => {
            *expected = Some(item_type);
            Ok(())
        }
        Some(depset_type) if depset_type == &item_type => Ok(()),
        Some(depset_type) => Err(kuro_error::Error::from(DepsetError::ElementTypeMismatch {
            item_type,
            depset_type: depset_type.clone(),
        })
        .into()),
    }
}

fn depset_element_type<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
    visited: &mut HashSet<ValueIdentity<'v>>,
) -> starlark::Result<Option<String>> {
    if !visited.insert(value.identity()) {
        return Ok(None);
    }

    for direct in depset_direct_values(value, heap) {
        return Ok(Some(validate_direct_depset_element(direct)?));
    }

    let mut element_type = None;
    for child in depset_child_values(value, heap) {
        if let Some(child_type) = depset_element_type(child, heap, visited)? {
            merge_depset_element_type(&mut element_type, child_type)?;
        }
    }
    Ok(element_type)
}

fn validate_depset_elements<'v>(
    direct: &[Value<'v>],
    transitive: &[Value<'v>],
    heap: Heap<'v>,
) -> starlark::Result<()> {
    let mut element_type = None;
    for item in direct {
        merge_depset_element_type(&mut element_type, validate_direct_depset_element(*item)?)?;
    }
    for child in transitive {
        let Some(_) = depset_order_from_value(*child) else {
            return Err(kuro_error::Error::from(DepsetError::TransitiveNotDepset).into());
        };
        let mut visited = HashSet::new();
        if let Some(child_type) = depset_element_type(*child, heap, &mut visited)? {
            merge_depset_element_type(&mut element_type, child_type)?;
        }
    }
    Ok(())
}

pub fn make_depset_from_lists<'v>(
    heap: Heap<'v>,
    direct: Vec<Value<'v>>,
    transitive: Vec<Value<'v>>,
    order: &str,
) -> starlark::Result<Value<'v>> {
    validate_depset_elements(&direct, &transitive, heap)?;
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
        #[starlark(default = NoneOr::None)] direct: NoneOr<UnpackListOrTuple<Value<'v>>>,
        // Accept `None` for `transitive` as equivalent to an empty list —
        // bazel's `depset(transitive = None)` works, kuro's used to reject
        // it. rules_rust's `make_libstd_and_allocator_ccinfo` passes
        // through `transitive = allocator_inputs` which can be `None`.
        #[starlark(require = named, default = NoneOr::None)] transitive: NoneOr<
            UnpackListOrTuple<Value<'v>>,
        >,
        #[starlark(require = named, default = "default")] order: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let direct_items = match direct {
            NoneOr::None => Vec::new(),
            NoneOr::Other(list) => list.items,
        };
        let transitive_items = match transitive {
            NoneOr::None => Vec::new(),
            NoneOr::Other(list) => list.items,
        };
        make_depset_from_lists(heap, direct_items, transitive_items, order)
    }
}
