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
use std::hash::Hash;
use std::sync::OnceLock;

use allocative::Allocative;
use kuro_artifact::artifact::artifact_type::Artifact;
use kuro_artifact::artifact::artifact_type::OutputArtifact;
use kuro_core::bzl::ImportPath;
use kuro_core::cells::build_file_cell::BuildFileCell;
use kuro_core::cells::cell_path::CellPath;
use kuro_core::cells::name::CellName;
use kuro_core::cells::paths::CellRelativePathBuf;
use kuro_core::configuration::data::ConfigurationData;
use starlark::coerce::Coerce;
use starlark::collections::StarlarkHasher;
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
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueIdentity;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::list::AllocList;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneOr;
use starlark::values::starlark_value;

use crate::artifact_groups::ArtifactGroup;
use crate::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkInputArtifactLike;
use crate::interpreter::rule_defs::artifact_tagging::ArtifactTag;
use crate::interpreter::rule_defs::cmd_args::CommandLineArtifactVisitor;
use crate::interpreter::rule_defs::cmd_args::SimpleCommandLineArtifactVisitor;
use crate::interpreter::rule_defs::cmd_args::value_as::ValueAsCommandLineLike;
use crate::interpreter::rule_defs::nested_set::NestedSetOrder;
use crate::interpreter::rule_defs::nested_set::collect_nested_set;
use crate::interpreter::rule_defs::transitive_set::FrozenTransitiveSetDefinition;
use crate::interpreter::rule_defs::transitive_set::transitive_set_definition::builtin_definition;

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum DepsetError {
    #[error("Invalid order: {order}")]
    InvalidOrder { order: String },
    #[error("at index {index} of transitive, got element of type {item_type}, want depset")]
    TransitiveNotDepset { index: usize, item_type: String },
    #[error("Order '{order}' is incompatible with order '{transitive_order}'")]
    OrderIncompatible {
        order: String,
        transitive_order: String,
    },
    #[error("depset elements must not be mutable values")]
    MutableElement,
    #[error("cannot add an item of type '{item_type}' to a depset of '{depset_type}'")]
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
    #[error("depset depth {depth} exceeds limit ({limit})")]
    DepthExceeded { depth: u32, limit: u32 },
    #[error("expected depset, got {item_type}")]
    ExpectedDepset { item_type: String },
    #[error("at index {index} of depset, got element of type {item_type}, want artifact")]
    ArtifactInputExpected { index: usize, item_type: String },
}

const MAX_DEPSET_DEPTH: u32 = 3500;

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
// DepsetGen - shared live/frozen depset facade
// ============================================================================

/// A Bazel-compatible depset node.
///
/// This single representation is used for both live values and frozen values.
/// It deliberately remains a depset facade: Buck/Kuro `TransitiveSet` keeps its
/// separate streaming representation and use-site traversal semantics.
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
pub struct DepsetGen<V: ValueLifetimeless> {
    /// Direct elements in this depset.
    pub(crate) direct: Vec<V>,
    /// Child depsets whose elements are transitively included.
    pub(crate) transitive: Vec<V>,
    /// Iteration order selected at construction.
    order: NestedSetOrder,
    /// Bazel tracks the top-level Starlark element type without flattening.
    #[freeze(identity)]
    element_type: Option<String>,
    /// O(1) truthiness/emptiness metadata.
    is_empty: bool,
    /// Approximate nested-DAG depth for future depth/error parity work.
    depth: u32,
}

pub type LiveDepsetGen<V> = DepsetGen<V>;

/// Thin allocable wrapper for frozen depsets constructed by native Rust code.
///
/// The nested-set storage is still `DepsetGen<FrozenValue>`; this wrapper keeps
/// existing `heap.alloc(Depset::empty())` call sites working without restoring a
/// second graph representation.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct Depset {
    inner: DepsetGen<FrozenValue>,
}

starlark_simple_value!(Depset);

impl<V: ValueLifetimeless> DepsetGen<V> {
    fn from_validated_parts(
        direct: Vec<V>,
        transitive: Vec<V>,
        order: NestedSetOrder,
        element_type: Option<String>,
        is_empty: bool,
        depth: u32,
    ) -> Self {
        Self {
            direct,
            transitive,
            order,
            element_type,
            is_empty,
            depth,
        }
    }

    pub(crate) fn direct_values(&self) -> &[V] {
        &self.direct
    }

    pub(crate) fn children_values(&self) -> &[V] {
        &self.transitive
    }

    pub(crate) fn order(&self) -> NestedSetOrder {
        self.order
    }

    pub(crate) fn order_str(&self) -> &'static str {
        self.order.as_str()
    }

    pub(crate) fn element_type(&self) -> Option<&str> {
        self.element_type.as_deref()
    }

    pub(crate) fn depth(&self) -> u32 {
        self.depth
    }

    /// Check if the depset is empty.
    pub fn is_empty(&self) -> bool {
        self.is_empty
    }
}

fn write_depset_gen_hash<'v, V: ValueLike<'v>>(
    depset: &DepsetGen<V>,
    hasher: &mut StarlarkHasher,
) -> starlark::Result<()> {
    "depset".hash(hasher);
    depset.order.hash(hasher);
    depset.direct.len().hash(hasher);
    for value in &depset.direct {
        value.to_value().write_hash(hasher)?;
    }
    depset.transitive.len().hash(hasher);
    for child in &depset.transitive {
        child.to_value().write_hash(hasher)?;
    }
    Ok(())
}

impl Depset {
    /// Create an empty depset.
    pub fn empty() -> Self {
        Self {
            inner: DepsetGen::from_validated_parts(
                Vec::new(),
                Vec::new(),
                NestedSetOrder::Default,
                None,
                true,
                0,
            ),
        }
    }

    /// Create a depset with direct elements from frozen values.
    pub(crate) fn from_frozen_values(elements: Vec<FrozenValue>, order: String) -> Self {
        make_frozen_depset_from_values(elements, Vec::new(), &order)
            .expect("valid frozen depset values")
    }

    pub fn direct_values(&self) -> &[FrozenValue] {
        self.inner.direct_values()
    }

    pub fn children_values(&self) -> &[FrozenValue] {
        self.inner.children_values()
    }

    pub fn order(&self) -> NestedSetOrder {
        self.inner.order()
    }

    pub fn order_str(&self) -> &'static str {
        self.inner.order_str()
    }

    pub fn element_type(&self) -> Option<&str> {
        self.inner.element_type()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn depth(&self) -> u32 {
        self.inner.depth()
    }

    /// Collect all elements from this depset and its transitive children.
    pub fn collect_all_frozen(&self) -> Vec<FrozenValue> {
        self.collect_all_frozen_ordered(self.order())
    }

    /// Collect all elements with a specific traversal order.
    pub fn collect_all_frozen_ordered(&self, order: NestedSetOrder) -> Vec<FrozenValue> {
        collect_nested_set(
            self,
            order,
            |depset| depset as *const Depset as usize,
            |depset| depset.direct_values().to_vec(),
            |depset| {
                depset
                    .children_values()
                    .iter()
                    .filter_map(|child| child.downcast_ref::<Depset>())
                    .collect()
            },
        )
    }

    /// Get the number of elements (including transitive).
    pub fn len(&self) -> usize {
        self.collect_all_frozen().len()
    }
}

impl<V: ValueLifetimeless> Display for DepsetGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty {
            write!(f, "depset([])")
        } else {
            write!(f, "depset([...])")
        }
    }
}

impl Display for Depset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "depset([])")
        } else {
            write!(f, "depset([...])")
        }
    }
}

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

fn dedupe_frozen_values_preserving_order(
    elements: Vec<FrozenValue>,
) -> starlark::Result<Vec<FrozenValue>> {
    let mut deduped: Vec<FrozenValue> = Vec::with_capacity(elements.len());
    for value in elements {
        let mut is_dup = false;
        for existing in &deduped {
            if existing.to_value().equals(value.to_value())? {
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
        RES.methods(generic_live_depset_methods)
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

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        write_depset_gen_hash(&self.inner, hasher)
    }
}

starlark::starlark_complex_value!(pub LiveDepset);

// Generic implementation for DepsetGen that works with both unfrozen (Value) and frozen (FrozenValue) variants.
#[starlark::values::starlark_value(type = "depset")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for DepsetGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_methods() -> Option<&'static Methods> {
        // Use the generic method implementation that handles all depset variants
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(generic_live_depset_methods)
    }

    fn to_bool(&self) -> bool {
        !self.is_empty
    }

    fn length(&self) -> starlark::Result<i32> {
        Err(kuro_error::Error::from(DepsetError::LenUnsupported).into())
    }

    fn bit_or(&self, _other: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Err(kuro_error::Error::from(DepsetError::BitOrUnsupported).into())
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        write_depset_gen_hash(self, hasher)
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

    fn direct_values(&self) -> Vec<Value<'v>> {
        match self {
            Self::Live(live) => live.direct.iter().map(|v| v.to_value()).collect(),
            Self::FrozenLive(live) => live.direct.iter().map(|v| v.to_value()).collect(),
            Self::Frozen(depset) => depset
                .direct_values()
                .iter()
                .map(|v| v.to_value())
                .collect(),
        }
    }

    fn child_values(&self) -> Vec<Value<'v>> {
        match self {
            Self::Live(live) => live.transitive.iter().map(|v| v.to_value()).collect(),
            Self::FrozenLive(live) => live.transitive.iter().map(|v| v.to_value()).collect(),
            Self::Frozen(depset) => depset
                .children_values()
                .iter()
                .map(|v| v.to_value())
                .collect(),
        }
    }

    fn order(&self) -> NestedSetOrder {
        match self {
            Self::Live(live) => live.order(),
            Self::FrozenLive(live) => live.order(),
            Self::Frozen(depset) => depset.order(),
        }
    }

    fn element_type(&self) -> Option<&str> {
        match self {
            Self::Live(live) => live.element_type(),
            Self::FrozenLive(live) => live.element_type(),
            Self::Frozen(depset) => depset.element_type(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Live(live) => live.is_empty(),
            Self::FrozenLive(live) => live.is_empty(),
            Self::Frozen(depset) => depset.is_empty(),
        }
    }

    fn depth(&self) -> u32 {
        match self {
            Self::Live(live) => live.depth(),
            Self::FrozenLive(live) => live.depth(),
            Self::Frozen(depset) => depset.depth(),
        }
    }
}

/// Bridge helper for native Rust code that still needs to preserve depset graph
/// shape. Starlark-visible consumers should flatten through `depset_to_list`.
pub fn depset_direct_and_transitive<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<(Vec<Value<'v>>, Vec<Value<'v>>)> {
    let _ = heap;
    if let Some(depset) = DepsetView::from_value(value) {
        return Ok((depset.direct_values(), depset.child_values()));
    }

    Err(kuro_error::Error::from(DepsetError::TransitiveNotDepset {
        index: 0,
        item_type: value.get_type().to_owned(),
    })
    .into())
}

pub fn is_depset_value(value: Value) -> bool {
    DepsetView::from_value(value).is_some()
}

pub fn depset_is_empty(value: Value) -> starlark::Result<bool> {
    let depset = DepsetView::from_value(value).ok_or_else(|| {
        kuro_error::Error::from(DepsetError::ExpectedDepset {
            item_type: value.get_type().to_owned(),
        })
    })?;
    Ok(depset.is_empty())
}

pub fn depset_element_type_name(value: Value) -> starlark::Result<Option<String>> {
    let depset = DepsetView::from_value(value).ok_or_else(|| {
        kuro_error::Error::from(DepsetError::ExpectedDepset {
            item_type: value.get_type().to_owned(),
        })
    })?;
    Ok(depset.element_type().map(str::to_owned))
}

fn depset_to_list_raw<'v>(value: Value<'v>) -> starlark::Result<Vec<Value<'v>>> {
    let depset = DepsetView::from_value(value).ok_or_else(|| {
        kuro_error::Error::from(DepsetError::ExpectedDepset {
            item_type: value.get_type().to_owned(),
        })
    })?;
    let order = depset.order();
    Ok(collect_nested_set(
        value,
        order,
        |depset| depset.identity(),
        |depset| depset_direct_values(depset),
        |depset| depset_child_values(depset),
    ))
}

pub fn depset_to_list<'v>(value: Value<'v>, heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
    let _ = heap;
    dedupe_values_preserving_order(depset_to_list_raw(value)?)
}

pub fn depset_to_list_without_heap<'v>(value: Value<'v>) -> starlark::Result<Vec<Value<'v>>> {
    dedupe_values_preserving_order(depset_to_list_raw(value)?)
}

pub fn depset_to_artifact_inputs<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<Vec<Value<'v>>> {
    let elements = depset_to_list(value, heap)?;
    for (index, element) in elements.iter().enumerate() {
        if <&dyn StarlarkInputArtifactLike<'v>>::unpack_value(*element)?.is_none() {
            return Err(kuro_error::Error::from(DepsetError::ArtifactInputExpected {
                index,
                item_type: element.get_type().to_owned(),
            })
            .into());
        }
    }
    Ok(elements)
}

pub fn depset_to_artifact_group_inputs(value: Value) -> starlark::Result<Vec<ArtifactGroup>> {
    let elements = depset_to_list_without_heap(value)?;
    let mut visitor = SimpleCommandLineArtifactVisitor::new();
    for element in elements {
        ValueAsCommandLineLike::unpack_value_err(element)?
            .0
            .visit_artifacts(&mut visitor)?;
    }
    Ok(visitor.inputs.into_iter().collect())
}

struct DepsetArtifactMetadataVisitor<'a> {
    has_content_based_path: bool,
    is_eligible_for_dedupe: bool,
    target_platform: Option<&'a ConfigurationData>,
}

impl<'a> DepsetArtifactMetadataVisitor<'a> {
    fn content_based() -> Self {
        Self {
            has_content_based_path: false,
            is_eligible_for_dedupe: true,
            target_platform: None,
        }
    }

    fn eligible_for_dedupe(target_platform: Option<&'a ConfigurationData>) -> Self {
        Self {
            has_content_based_path: false,
            is_eligible_for_dedupe: true,
            target_platform,
        }
    }
}

impl<'v> CommandLineArtifactVisitor<'v> for DepsetArtifactMetadataVisitor<'_> {
    fn visit_input(&mut self, input: ArtifactGroup, _tags: Vec<&ArtifactTag>) {
        if input.uses_content_based_path() {
            self.has_content_based_path = true;
        }
        if self.is_eligible_for_dedupe {
            self.is_eligible_for_dedupe = input.is_eligible_for_dedupe(self.target_platform);
        }
    }

    fn visit_declared_output(&mut self, _artifact: OutputArtifact<'v>, _tags: Vec<&ArtifactTag>) {}

    fn visit_frozen_output(&mut self, _artifact: Artifact, _tags: Vec<&ArtifactTag>) {}
}

fn visit_depset_direct_artifacts<'v>(
    value: Value<'v>,
    visitor: &mut dyn CommandLineArtifactVisitor<'v>,
    visited: &mut HashSet<ValueIdentity<'v>>,
) -> starlark::Result<()> {
    if !visited.insert(value.identity()) {
        return Ok(());
    }
    let depset = DepsetView::from_value(value).ok_or_else(|| {
        kuro_error::Error::from(DepsetError::ExpectedDepset {
            item_type: value.get_type().to_owned(),
        })
    })?;
    for direct in depset.direct_values() {
        if let Some(arg) = ValueAsCommandLineLike::unpack_value_opt(direct) {
            arg.0.visit_artifacts(visitor)?;
        }
    }
    for child in depset.child_values() {
        visit_depset_direct_artifacts(child, visitor, visited)?;
    }
    Ok(())
}

pub fn depset_artifact_group_has_content_based_path(value: Value) -> starlark::Result<bool> {
    let mut visitor = DepsetArtifactMetadataVisitor::content_based();
    let mut visited = HashSet::new();
    visit_depset_direct_artifacts(value, &mut visitor, &mut visited)?;
    Ok(visitor.has_content_based_path)
}

pub fn depset_artifact_group_is_eligible_for_dedupe(
    value: Value,
    target_platform: Option<&ConfigurationData>,
) -> starlark::Result<bool> {
    let mut visitor = DepsetArtifactMetadataVisitor::eligible_for_dedupe(target_platform);
    let mut visited = HashSet::new();
    visit_depset_direct_artifacts(value, &mut visitor, &mut visited)?;
    Ok(visitor.is_eligible_for_dedupe)
}

fn depset_child_values<'v>(value: Value<'v>) -> Vec<Value<'v>> {
    DepsetView::from_value(value)
        .map(|depset| depset.child_values())
        .unwrap_or_default()
}

fn depset_direct_values<'v>(value: Value<'v>) -> Vec<Value<'v>> {
    DepsetView::from_value(value)
        .map(|depset| depset.direct_values())
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
        let checkpoint = match DepsetView::from_value(this) {
            Some(DepsetView::Live(_)) => "depset_to_list_live",
            Some(DepsetView::FrozenLive(_) | DepsetView::Frozen(_)) => "depset_to_list_frozen",
            None => "depset_to_list_live",
        };
        let (direct_len, transitive_len) = if kuro_util::memory_checkpoint::enabled() {
            depset_direct_and_transitive(this, heap)
                .map(|(direct, transitive)| (direct.len(), transitive.len()))
                .unwrap_or((0, 0))
        } else {
            (0, 0)
        };
        let elements = depset_to_list_raw(this)?;
        // Bazel depsets deduplicate elements: depset(["a", "a"]).to_list() == ["a"]
        // Dedup by value equality (preserving insertion order) using
        // Value::equals, which short-circuits on ptr_eq. Using the Display
        // string would wrongly collapse distinct values whose representations
        // happen to coincide (e.g. artifacts at different configurations).
        let collected_len = elements.len();
        let deduped = dedupe_values_preserving_order(elements)?;
        depset_to_list_checkpoint(
            checkpoint,
            direct_len,
            transitive_len,
            collected_len,
            deduped.len(),
        );
        Ok(heap.alloc(AllocList(deduped)))
    }
}

fn validate_depset_order<'v>(
    order: &str,
    transitive: &[Value<'v>],
) -> starlark::Result<NestedSetOrder> {
    let Some(parsed_order) = NestedSetOrder::parse(order) else {
        return Err(kuro_error::Error::from(DepsetError::InvalidOrder {
            order: order.to_owned(),
        })
        .into());
    };

    for (index, item) in transitive.iter().enumerate() {
        let Some(item_depset) = DepsetView::from_value(*item) else {
            return Err(kuro_error::Error::from(DepsetError::TransitiveNotDepset {
                index,
                item_type: item.get_type().to_owned(),
            })
            .into());
        };
        let item_order = item_depset.order();
        if !parsed_order.is_compatible_with_child(item_order) {
            return Err(kuro_error::Error::from(DepsetError::OrderIncompatible {
                order: order.to_owned(),
                transitive_order: item_order.as_str().to_owned(),
            })
            .into());
        }
    }

    Ok(parsed_order)
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
    visited: &mut HashSet<ValueIdentity<'v>>,
) -> starlark::Result<Option<String>> {
    if !visited.insert(value.identity()) {
        return Ok(None);
    }

    let Some(depset) = DepsetView::from_value(value) else {
        return Err(kuro_error::Error::from(DepsetError::TransitiveNotDepset {
            index: 0,
            item_type: value.get_type().to_owned(),
        })
        .into());
    };
    if let Some(element_type) = depset.element_type() {
        return Ok(Some(element_type.to_owned()));
    }

    let mut element_type = None;
    for child in depset.child_values() {
        if let Some(child_type) = depset_element_type(child, visited)? {
            merge_depset_element_type(&mut element_type, child_type)?;
        }
    }
    Ok(element_type)
}

fn validate_depset_elements<'v>(
    direct: &[Value<'v>],
    transitive: &[Value<'v>],
) -> starlark::Result<(Option<String>, bool, u32)> {
    let mut element_type = None;
    for item in direct {
        merge_depset_element_type(&mut element_type, validate_direct_depset_element(*item)?)?;
    }
    let mut is_empty = direct.is_empty();
    let mut max_child_depth = 0;
    for (index, child) in transitive.iter().enumerate() {
        let Some(child_depset) = DepsetView::from_value(*child) else {
            return Err(kuro_error::Error::from(DepsetError::TransitiveNotDepset {
                index,
                item_type: child.get_type().to_owned(),
            })
            .into());
        };
        is_empty &= child_depset.is_empty();
        max_child_depth = max_child_depth.max(child_depset.depth());
        let mut visited = HashSet::new();
        if let Some(child_type) = depset_element_type(*child, &mut visited)? {
            merge_depset_element_type(&mut element_type, child_type)?;
        }
    }
    let depth = if is_empty {
        0
    } else if direct.is_empty() {
        max_child_depth
    } else {
        max_child_depth.saturating_add(1)
    };
    if depth > MAX_DEPSET_DEPTH {
        return Err(kuro_error::Error::from(DepsetError::DepthExceeded {
            depth,
            limit: MAX_DEPSET_DEPTH,
        })
        .into());
    }
    Ok((element_type, is_empty, depth))
}

fn make_live_depset_from_values<'v>(
    direct: Vec<Value<'v>>,
    transitive: Vec<Value<'v>>,
    order: &str,
) -> starlark::Result<LiveDepsetGen<Value<'v>>> {
    let direct = dedupe_values_preserving_order(direct)?;
    let effective_order = validate_depset_order(order, &transitive)?;
    let (element_type, is_empty, depth) = validate_depset_elements(&direct, &transitive)?;
    Ok(DepsetGen::from_validated_parts(
        direct,
        transitive,
        effective_order,
        element_type,
        is_empty,
        depth,
    ))
}

fn make_frozen_depset_from_values(
    direct: Vec<FrozenValue>,
    transitive: Vec<FrozenValue>,
    order: &str,
) -> starlark::Result<Depset> {
    let direct = dedupe_frozen_values_preserving_order(direct)?;
    let direct_values: Vec<Value<'static>> = direct.iter().map(|v| v.to_value()).collect();
    let transitive_values: Vec<Value<'static>> = transitive.iter().map(|v| v.to_value()).collect();
    let effective_order = validate_depset_order(order, &transitive_values)?;
    let (element_type, is_empty, depth) =
        validate_depset_elements(&direct_values, &transitive_values)?;
    Ok(Depset {
        inner: DepsetGen::from_validated_parts(
            direct,
            transitive,
            effective_order,
            element_type,
            is_empty,
            depth,
        ),
    })
}

pub fn make_depset_from_lists<'v>(
    heap: Heap<'v>,
    direct: Vec<Value<'v>>,
    transitive: Vec<Value<'v>>,
    order: &str,
) -> starlark::Result<Value<'v>> {
    Ok(heap.alloc(make_live_depset_from_values(direct, transitive, order)?))
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
