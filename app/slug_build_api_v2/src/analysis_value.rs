/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the root directory of this source tree. You
 * may select, at your option, one of the above-listed licenses.
 */

use std::error::Error;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::LazyLock;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use fxhash::FxHashMap;
use slug_identity_v2::CanonicalLabel;
use starlark_map::StarlarkHasher;
use starlark_map::small_map::SmallMap;

use crate::ActionOutput;
use crate::depset::Depset;
use crate::depset::DepsetError;
use crate::depset::DepsetOrder;
use crate::depset::DepsetSuccessor;
use crate::depset::MAX_DEPTH;
use crate::providers::ProviderCollection;
use crate::providers::ProviderId;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum AnalysisValueType {
    Empty,
    None,
    Boolean,
    Integer,
    Float,
    String,
    Label,
    ConfiguredTarget,
    Artifact,
    List,
    Tuple,
    Dictionary,
    Struct,
    ToolchainInfo,
    Depset,
}

impl AnalysisValueType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::None => "NoneType",
            Self::Boolean => "bool",
            Self::Integer => "int",
            Self::Float => "float",
            Self::String => "string",
            Self::Label => "Label",
            Self::ConfiguredTarget => "Target",
            Self::Artifact => "File",
            Self::List => "list",
            Self::Tuple => "tuple",
            Self::Dictionary => "dict",
            Self::Struct => "struct",
            Self::ToolchainInfo => "ToolchainInfo",
            Self::Depset => "depset",
        }
    }
}

impl fmt::Display for AnalysisValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct AnalysisInteger {
    negative: bool,
    magnitude: Arc<[u8]>,
}

impl AnalysisInteger {
    pub fn from_i64(value: i64) -> Self {
        Self::from_magnitude(value.is_negative(), value.unsigned_abs().to_be_bytes())
    }

    pub fn from_magnitude(negative: bool, magnitude: impl AsRef<[u8]>) -> Self {
        let magnitude = magnitude.as_ref();
        let first = magnitude
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(magnitude.len());
        Self {
            negative: negative && first != magnitude.len(),
            magnitude: Arc::from(&magnitude[first..]),
        }
    }

    pub fn is_negative(&self) -> bool {
        self.negative
    }

    pub fn magnitude(&self) -> &[u8] {
        &self.magnitude
    }

    fn from_integral_float(value: f64) -> Option<Self> {
        if !value.is_finite() || value != value.trunc() {
            return None;
        }
        if value == 0.0 {
            return Some(Self::from_i64(0));
        }
        let bits = value.abs().to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023;
        if exponent < 0 {
            return None;
        }
        let mantissa = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
        let mut magnitude = if exponent < 52 {
            let shift = 52 - exponent;
            if mantissa & ((1u64 << shift) - 1) != 0 {
                return None;
            }
            (mantissa >> shift).to_be_bytes().to_vec()
        } else {
            let mut bytes = mantissa.to_be_bytes().to_vec();
            for _ in 0..(exponent - 52) {
                let mut carry = 0;
                for byte in bytes.iter_mut().rev() {
                    let next = *byte >> 7;
                    *byte = (*byte << 1) | carry;
                    carry = next;
                }
                if carry != 0 {
                    bytes.insert(0, carry);
                }
            }
            bytes
        };
        while magnitude.first() == Some(&0) {
            magnitude.remove(0);
        }
        Some(Self::from_magnitude(value.is_sign_negative(), magnitude))
    }
}

#[derive(Debug, Clone, Allocative)]
pub enum AnalysisNumber {
    Integer(AnalysisInteger),
    Float(u64),
}

impl AnalysisNumber {
    pub fn integer(value: i64) -> Self {
        Self::Integer(AnalysisInteger::from_i64(value))
    }

    pub fn integer_from_magnitude(negative: bool, magnitude: impl AsRef<[u8]>) -> Self {
        Self::Integer(AnalysisInteger::from_magnitude(negative, magnitude))
    }

    pub fn float(value: f64) -> Self {
        Self::Float(value.to_bits())
    }

    pub fn as_integer(&self) -> Option<&AnalysisInteger> {
        match self {
            Self::Integer(value) => Some(value),
            Self::Float(_) => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Integer(_) => None,
            Self::Float(bits) => Some(f64::from_bits(*bits)),
        }
    }

    fn publication_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left == right,
            _ => false,
        }
    }
}

impl PartialEq for AnalysisNumber {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (Self::Integer(left), Self::Float(bits)) | (Self::Float(bits), Self::Integer(left)) => {
                AnalysisInteger::from_integral_float(f64::from_bits(*bits))
                    .is_some_and(|right| *left == right)
            }
            (Self::Float(left), Self::Float(right)) => {
                let left = f64::from_bits(*left);
                let right = f64::from_bits(*right);
                left == right || (left.is_nan() && right.is_nan())
            }
        }
    }
}

impl Eq for AnalysisNumber {}

impl Hash for AnalysisNumber {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Integer(value) => {
                0u8.hash(state);
                value.hash(state);
            }
            Self::Float(bits) => {
                let value = f64::from_bits(*bits);
                if let Some(value) = AnalysisInteger::from_integral_float(value) {
                    0u8.hash(state);
                    value.hash(state);
                } else {
                    1u8.hash(state);
                    if value.is_nan() {
                        f64::NAN.to_bits().hash(state);
                    } else {
                        bits.hash(state);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct AnalysisConfiguredTargetKey(Arc<AnalysisConfiguredTargetKeyData>);

#[derive(Debug, Eq, PartialEq, Hash, Allocative)]
struct AnalysisConfiguredTargetKeyData {
    label: CanonicalLabel,
    configuration: Arc<[u8]>,
}

impl AnalysisConfiguredTargetKey {
    pub fn new(label: CanonicalLabel, configuration: impl Into<Arc<[u8]>>) -> Self {
        Self(Arc::new(AnalysisConfiguredTargetKeyData {
            label,
            configuration: configuration.into(),
        }))
    }

    pub fn label(&self) -> &CanonicalLabel {
        &self.0.label
    }

    pub fn configuration(&self) -> &[u8] {
        &self.0.configuration
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub enum AnalysisArtifact {
    Source(CanonicalLabel),
    Derived {
        owner: AnalysisConfiguredTargetKey,
        output: ActionOutput,
    },
}

#[derive(Debug, Clone, Allocative)]
pub struct ConfiguredTargetValue {
    identity: AnalysisConfiguredTargetKey,
    providers: ProviderCollection,
}

impl ConfiguredTargetValue {
    pub fn new(identity: AnalysisConfiguredTargetKey, providers: ProviderCollection) -> Self {
        Self {
            identity,
            providers,
        }
    }

    pub fn identity(&self) -> &AnalysisConfiguredTargetKey {
        &self.identity
    }

    pub fn providers(&self) -> &ProviderCollection {
        &self.providers
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub enum ProviderIdentity {
    Builtin(CompactString),
    User(ProviderId),
}

impl ProviderIdentity {
    pub fn builtin(name: impl Into<CompactString>) -> Self {
        Self::Builtin(name.into())
    }

    pub fn user(id: ProviderId) -> Self {
        Self::User(id)
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Builtin(name) => name,
            Self::User(id) => id.exported_name(),
        }
    }

    pub fn user_id(&self) -> Option<&ProviderId> {
        match self {
            Self::User(id) => Some(id),
            Self::Builtin(_) => None,
        }
    }

    pub fn is_builtin(&self, name: &str) -> bool {
        matches!(self, Self::Builtin(candidate) if candidate == name)
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct ProviderOccurrence {
    identity: ProviderIdentity,
    fields: SmallMap<CompactString, AnalysisValue>,
}

impl ProviderOccurrence {
    pub fn new(
        identity: ProviderIdentity,
        fields: impl IntoIterator<Item = (impl Into<CompactString>, AnalysisValue)>,
    ) -> Self {
        let mut fields = fields
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect::<SmallMap<_, _>>();
        fields.sort_keys();
        Self { identity, fields }
    }

    pub fn empty(identity: ProviderIdentity) -> Self {
        Self::new(
            identity,
            std::iter::empty::<(CompactString, AnalysisValue)>(),
        )
    }

    pub fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }

    pub fn fields(&self) -> &SmallMap<CompactString, AnalysisValue> {
        &self.fields
    }

    pub fn field(&self, name: &str) -> Option<&AnalysisValue> {
        self.fields.get(name)
    }

    pub fn value_type(&self) -> AnalysisValueType {
        if self.identity.is_builtin("ToolchainInfo") {
            AnalysisValueType::ToolchainInfo
        } else {
            AnalysisValueType::Struct
        }
    }

    pub fn is_starlark_immutable(&self) -> bool {
        !self.identity.is_builtin("ToolchainInfo")
            && self
                .fields
                .values()
                .all(AnalysisValue::is_starlark_immutable)
    }

    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.identity == other.identity
            && self.fields.len() == other.fields.len()
            && self.fields.iter().all(|(name, value)| {
                other
                    .fields
                    .get(name)
                    .is_some_and(|other| value.publication_eq_with(other, state))
            })
    }
}

impl PartialEq for ProviderOccurrence {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity && self.fields == other.fields
    }
}

impl Eq for ProviderOccurrence {}

impl Hash for ProviderOccurrence {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
        self.fields.len().hash(state);
        for field in &self.fields {
            field.hash(state);
        }
    }
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct AnalysisDepset(Depset<AnalysisValue, AnalysisDepsetMetadata>);

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct AnalysisDepsetOccurrence(Arc<()>);

impl AnalysisDepsetOccurrence {
    pub fn new() -> Self {
        Self(Arc::new(()))
    }

    fn pointer(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }

    pub fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for AnalysisDepsetOccurrence {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for AnalysisDepsetOccurrence {
    fn eq(&self, other: &Self) -> bool {
        self.shares_with(other)
    }
}

impl Eq for AnalysisDepsetOccurrence {}

impl Hash for AnalysisDepsetOccurrence {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pointer().hash(state);
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
struct AnalysisDepsetMetadata {
    occurrence: AnalysisDepsetOccurrence,
    element_type: AnalysisValueType,
}

#[derive(Debug, Clone)]
pub enum AnalysisDepsetSuccessor<'a> {
    Direct(&'a AnalysisValue),
    Transitive(AnalysisDepset),
}

#[derive(Debug, Clone)]
pub enum AnalysisDepsetInput {
    Direct(AnalysisValue),
    Transitive(AnalysisDepset),
}

impl AnalysisDepset {
    pub fn empty(order: DepsetOrder) -> Self {
        static DEFAULT: LazyLock<AnalysisDepset> =
            LazyLock::new(|| AnalysisDepset::empty_owned(DepsetOrder::Default));
        static POSTORDER: LazyLock<AnalysisDepset> =
            LazyLock::new(|| AnalysisDepset::empty_owned(DepsetOrder::Postorder));
        static PREORDER: LazyLock<AnalysisDepset> =
            LazyLock::new(|| AnalysisDepset::empty_owned(DepsetOrder::Preorder));
        static TOPOLOGICAL: LazyLock<AnalysisDepset> =
            LazyLock::new(|| AnalysisDepset::empty_owned(DepsetOrder::Topological));
        match order {
            DepsetOrder::Default => DEFAULT.dupe(),
            DepsetOrder::Postorder => POSTORDER.dupe(),
            DepsetOrder::Preorder => PREORDER.dupe(),
            DepsetOrder::Topological => TOPOLOGICAL.dupe(),
        }
    }

    fn empty_owned(order: DepsetOrder) -> Self {
        Self(Depset::from_canonical_successors(
            order,
            Vec::new(),
            0,
            AnalysisDepsetMetadata {
                occurrence: AnalysisDepsetOccurrence::new(),
                element_type: AnalysisValueType::Empty,
            },
        ))
    }

    pub fn new(
        order: DepsetOrder,
        direct: Vec<AnalysisValue>,
        transitive: Vec<AnalysisDepset>,
    ) -> Result<Self, AnalysisValueError> {
        Self::new_with_occurrence(AnalysisDepsetOccurrence::new(), order, direct, transitive)
    }

    pub fn new_with_occurrence(
        occurrence: AnalysisDepsetOccurrence,
        order: DepsetOrder,
        direct: Vec<AnalysisValue>,
        transitive: Vec<AnalysisDepset>,
    ) -> Result<Self, AnalysisValueError> {
        let mut element_type = AnalysisValueType::Empty;
        for value in &direct {
            if !value.is_valid_depset_leaf() {
                return Err(AnalysisValueError::InvalidDepsetLeaf {
                    value_type: value.value_type(),
                });
            }
            merge_depset_type(&mut element_type, value.value_type())?;
        }

        let transitive = transitive
            .into_iter()
            .filter(|child| !child.is_empty())
            .collect::<Vec<_>>();
        for child in &transitive {
            merge_depset_type(&mut element_type, child.element_type())?;
        }
        if direct.is_empty() && transitive.is_empty() {
            return Ok(Self::empty(order));
        }

        let graph = Depset::new_with_metadata(
            order,
            direct,
            transitive.iter().map(|child| child.0.clone()).collect(),
            AnalysisDepsetMetadata {
                occurrence,
                element_type,
            },
        )?;
        if let Some(child) = transitive
            .iter()
            .find(|child| graph.shares_node_with(&child.0))
        {
            return Ok(child.dupe());
        }
        Ok(Self(graph))
    }

    pub fn from_canonical_successors(
        occurrence: AnalysisDepsetOccurrence,
        order: DepsetOrder,
        depth: usize,
        successors: Vec<AnalysisDepsetInput>,
    ) -> Result<Self, AnalysisValueError> {
        if depth > MAX_DEPTH {
            return Err(DepsetError::DepthLimitExceeded {
                depth,
                max: MAX_DEPTH,
            }
            .into());
        }
        let mut element_type = AnalysisValueType::Empty;
        let mut graph_successors = Vec::with_capacity(successors.len());
        for successor in successors {
            match successor {
                AnalysisDepsetInput::Direct(value) => {
                    if !value.is_valid_depset_leaf() {
                        return Err(AnalysisValueError::InvalidDepsetLeaf {
                            value_type: value.value_type(),
                        });
                    }
                    merge_depset_type(&mut element_type, value.value_type())?;
                    graph_successors.push(DepsetSuccessor::Direct(value));
                }
                AnalysisDepsetInput::Transitive(child) => {
                    if !order.compatible_with(child.order()) {
                        return Err(DepsetError::IncompatibleOrder {
                            parent: order,
                            child: child.order(),
                        }
                        .into());
                    }
                    merge_depset_type(&mut element_type, child.element_type())?;
                    graph_successors.push(DepsetSuccessor::Transitive(child.0));
                }
            }
        }
        if graph_successors.is_empty() {
            return Ok(Self::empty(order));
        }
        Ok(Self(Depset::from_canonical_successors(
            order,
            graph_successors,
            depth,
            AnalysisDepsetMetadata {
                occurrence,
                element_type,
            },
        )))
    }

    pub fn from_dereferenced_child(
        occurrence: AnalysisDepsetOccurrence,
        order: DepsetOrder,
        child: &Self,
    ) -> Result<Self, AnalysisValueError> {
        if !order.compatible_with(child.order()) {
            return Err(DepsetError::IncompatibleOrder {
                parent: order,
                child: child.order(),
            }
            .into());
        }
        Ok(Self(Depset::rewrap(
            order,
            &child.0,
            AnalysisDepsetMetadata {
                occurrence,
                element_type: child.element_type(),
            },
        )))
    }

    pub fn order(&self) -> DepsetOrder {
        self.0.order()
    }

    pub fn element_type(&self) -> AnalysisValueType {
        self.0.metadata().element_type
    }

    pub fn successors(&self) -> impl Iterator<Item = AnalysisDepsetSuccessor<'_>> {
        self.0.successors().iter().map(|successor| match successor {
            DepsetSuccessor::Direct(value) => AnalysisDepsetSuccessor::Direct(value),
            DepsetSuccessor::Transitive(child) => {
                AnalysisDepsetSuccessor::Transitive(AnalysisDepset(child.clone()))
            }
        })
    }

    pub fn depth(&self) -> usize {
        self.0.depth()
    }

    pub fn is_empty(&self) -> bool {
        self.element_type() == AnalysisValueType::Empty
    }

    pub fn to_list(&self) -> Vec<AnalysisValue> {
        self.0.to_list()
    }

    pub fn singleton_value(&self) -> Option<&AnalysisValue> {
        match self.0.successors() {
            [DepsetSuccessor::Direct(value)] => Some(value),
            _ => None,
        }
    }

    pub fn shares_occurrence_with(&self, other: &Self) -> bool {
        self.0
            .metadata()
            .occurrence
            .shares_with(&other.0.metadata().occurrence)
    }

    pub fn shares_successors_with(&self, other: &Self) -> bool {
        self.0.shares_successors_with(&other.0)
    }

    pub fn occurrence(&self) -> AnalysisDepsetOccurrence {
        self.0.metadata().occurrence.dupe()
    }

    fn pointer(&self) -> usize {
        self.0.node_key()
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        let left = self.pointer();
        let right = other.pointer();
        if let Some(previous) = state.left_depsets.get(&left) {
            return *previous == right;
        }
        if let Some(previous) = state.right_depsets.get(&right) {
            return *previous == left;
        }
        state.left_depsets.insert(left, right);
        state.right_depsets.insert(right, left);

        if self.order() != other.order() || self.element_type() != other.element_type() {
            return false;
        }
        let mut left = self.successors();
        let mut right = other.successors();
        loop {
            match (left.next(), right.next()) {
                (None, None) => return true,
                (
                    Some(AnalysisDepsetSuccessor::Direct(left)),
                    Some(AnalysisDepsetSuccessor::Direct(right)),
                ) if left.publication_eq_with(right, state) => {}
                (
                    Some(AnalysisDepsetSuccessor::Transitive(left)),
                    Some(AnalysisDepsetSuccessor::Transitive(right)),
                ) if left.publication_eq_with(&right, state) => {}
                _ => return false,
            }
        }
    }
}

impl PartialEq for AnalysisDepset {
    fn eq(&self, other: &Self) -> bool {
        self.shares_occurrence_with(other)
    }
}

impl Eq for AnalysisDepset {}

impl Hash for AnalysisDepset {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.is_empty() {
            0u8.hash(state);
            self.order().hash(state);
        } else {
            1u8.hash(state);
            self.0.metadata().occurrence.pointer().hash(state);
        }
    }
}

fn merge_depset_type(
    current: &mut AnalysisValueType,
    candidate: AnalysisValueType,
) -> Result<(), AnalysisValueError> {
    if *current == AnalysisValueType::Empty {
        *current = candidate;
    } else if *current != candidate {
        return Err(AnalysisValueError::HeterogeneousDepset {
            existing: *current,
            candidate,
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct AnalysisValue(Arc<AnalysisValueData>);

#[derive(Debug, Allocative)]
enum AnalysisValueData {
    None,
    Boolean(bool),
    Number(AnalysisNumber),
    String(CompactString),
    Label(CanonicalLabel),
    ConfiguredTarget(ConfiguredTargetValue),
    Artifact(AnalysisArtifact),
    List(Arc<[AnalysisValue]>),
    Tuple(Arc<[AnalysisValue]>),
    Dictionary(Arc<[(AnalysisValue, AnalysisValue)]>),
    Struct(SmallMap<CompactString, AnalysisValue>),
    Provider(ProviderOccurrence),
    Depset(AnalysisDepset),
}

#[derive(Debug, Clone, Copy)]
pub enum AnalysisValueKind<'a> {
    None,
    Boolean(bool),
    Number(&'a AnalysisNumber),
    String(&'a str),
    Label(&'a CanonicalLabel),
    ConfiguredTarget(&'a ConfiguredTargetValue),
    Artifact(&'a AnalysisArtifact),
    List(&'a [AnalysisValue]),
    Tuple(&'a [AnalysisValue]),
    Dictionary(&'a [(AnalysisValue, AnalysisValue)]),
    Struct(&'a SmallMap<CompactString, AnalysisValue>),
    Provider(&'a ProviderOccurrence),
    Depset(&'a AnalysisDepset),
}

impl AnalysisValue {
    fn new(value: AnalysisValueData) -> Self {
        Self(Arc::new(value))
    }

    pub fn none() -> Self {
        Self::new(AnalysisValueData::None)
    }

    pub fn boolean(value: bool) -> Self {
        Self::new(AnalysisValueData::Boolean(value))
    }

    pub fn integer(value: i64) -> Self {
        Self::new(AnalysisValueData::Number(AnalysisNumber::integer(value)))
    }

    pub fn integer_from_magnitude(negative: bool, magnitude: impl AsRef<[u8]>) -> Self {
        Self::new(AnalysisValueData::Number(
            AnalysisNumber::integer_from_magnitude(negative, magnitude),
        ))
    }

    pub fn float(value: f64) -> Self {
        Self::new(AnalysisValueData::Number(AnalysisNumber::float(value)))
    }

    pub fn string(value: impl Into<CompactString>) -> Self {
        Self::new(AnalysisValueData::String(value.into()))
    }

    pub fn label(value: CanonicalLabel) -> Self {
        Self::new(AnalysisValueData::Label(value))
    }

    pub fn configured_target(value: ConfiguredTargetValue) -> Self {
        Self::new(AnalysisValueData::ConfiguredTarget(value))
    }

    pub fn artifact(value: AnalysisArtifact) -> Self {
        Self::new(AnalysisValueData::Artifact(value))
    }

    pub fn list(values: impl Into<Arc<[AnalysisValue]>>) -> Self {
        Self::new(AnalysisValueData::List(values.into()))
    }

    pub fn tuple(values: impl Into<Arc<[AnalysisValue]>>) -> Self {
        Self::new(AnalysisValueData::Tuple(values.into()))
    }

    pub fn dictionary(
        values: impl IntoIterator<Item = (AnalysisValue, AnalysisValue)>,
    ) -> Result<Self, AnalysisValueError> {
        let mut entries = SmallMap::new();
        for (key, value) in values {
            if !key.is_starlark_hashable() {
                return Err(AnalysisValueError::UnhashableDictionaryKey {
                    value_type: key.value_type(),
                });
            }
            if entries.insert(key, value).is_some() {
                return Err(AnalysisValueError::DuplicateDictionaryKey);
            }
        }
        Ok(Self::new(AnalysisValueData::Dictionary(
            entries.into_iter().collect::<Vec<_>>().into(),
        )))
    }

    pub fn strukt(
        fields: impl IntoIterator<Item = (impl Into<CompactString>, AnalysisValue)>,
    ) -> Self {
        let mut fields = fields
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect::<SmallMap<_, _>>();
        fields.sort_keys();
        Self::new(AnalysisValueData::Struct(fields))
    }

    pub fn provider(value: ProviderOccurrence) -> Self {
        Self::new(AnalysisValueData::Provider(value))
    }

    pub fn depset(value: AnalysisDepset) -> Self {
        Self::new(AnalysisValueData::Depset(value))
    }

    pub fn kind(&self) -> AnalysisValueKind<'_> {
        match self.0.as_ref() {
            AnalysisValueData::None => AnalysisValueKind::None,
            AnalysisValueData::Boolean(value) => AnalysisValueKind::Boolean(*value),
            AnalysisValueData::Number(value) => AnalysisValueKind::Number(value),
            AnalysisValueData::String(value) => AnalysisValueKind::String(value),
            AnalysisValueData::Label(value) => AnalysisValueKind::Label(value),
            AnalysisValueData::ConfiguredTarget(value) => {
                AnalysisValueKind::ConfiguredTarget(value)
            }
            AnalysisValueData::Artifact(value) => AnalysisValueKind::Artifact(value),
            AnalysisValueData::List(value) => AnalysisValueKind::List(value),
            AnalysisValueData::Tuple(value) => AnalysisValueKind::Tuple(value),
            AnalysisValueData::Dictionary(value) => AnalysisValueKind::Dictionary(value),
            AnalysisValueData::Struct(value) => AnalysisValueKind::Struct(value),
            AnalysisValueData::Provider(value) => AnalysisValueKind::Provider(value),
            AnalysisValueData::Depset(value) => AnalysisValueKind::Depset(value),
        }
    }

    pub fn value_type(&self) -> AnalysisValueType {
        match self.0.as_ref() {
            AnalysisValueData::None => AnalysisValueType::None,
            AnalysisValueData::Boolean(_) => AnalysisValueType::Boolean,
            AnalysisValueData::Number(AnalysisNumber::Integer(_)) => AnalysisValueType::Integer,
            AnalysisValueData::Number(AnalysisNumber::Float(_)) => AnalysisValueType::Float,
            AnalysisValueData::String(_) => AnalysisValueType::String,
            AnalysisValueData::Label(_) => AnalysisValueType::Label,
            AnalysisValueData::ConfiguredTarget(_) => AnalysisValueType::ConfiguredTarget,
            AnalysisValueData::Artifact(_) => AnalysisValueType::Artifact,
            AnalysisValueData::List(_) => AnalysisValueType::List,
            AnalysisValueData::Tuple(_) => AnalysisValueType::Tuple,
            AnalysisValueData::Dictionary(_) => AnalysisValueType::Dictionary,
            AnalysisValueData::Struct(_) => AnalysisValueType::Struct,
            AnalysisValueData::Provider(value) => value.value_type(),
            AnalysisValueData::Depset(_) => AnalysisValueType::Depset,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self.0.as_ref() {
            AnalysisValueData::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn is_starlark_immutable(&self) -> bool {
        match self.0.as_ref() {
            AnalysisValueData::Tuple(values) => values.iter().all(Self::is_starlark_immutable),
            AnalysisValueData::Struct(fields) => fields.values().all(Self::is_starlark_immutable),
            AnalysisValueData::Provider(value) => value.is_starlark_immutable(),
            _ => true,
        }
    }

    pub fn is_starlark_hashable(&self) -> bool {
        match self.0.as_ref() {
            AnalysisValueData::List(_) | AnalysisValueData::Dictionary(_) => false,
            AnalysisValueData::Tuple(values) => values.iter().all(Self::is_starlark_hashable),
            AnalysisValueData::Struct(fields) => fields.values().all(Self::is_starlark_immutable),
            AnalysisValueData::Provider(value) => value.is_starlark_immutable(),
            _ => true,
        }
    }

    pub fn starlark_hash(&self) -> Result<u64, AnalysisValueError> {
        if !self.is_starlark_hashable() {
            return Err(AnalysisValueError::UnhashableDictionaryKey {
                value_type: self.value_type(),
            });
        }
        let mut hasher = StarlarkHasher::default();
        self.hash(&mut hasher);
        Ok(hasher.finish())
    }

    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn is_valid_depset_leaf(&self) -> bool {
        self.is_starlark_immutable()
            && !matches!(
                self.0.as_ref(),
                AnalysisValueData::List(_) | AnalysisValueData::Dictionary(_)
            )
    }

    pub fn publication_eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }

    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        match (self.0.as_ref(), other.0.as_ref()) {
            (AnalysisValueData::None, AnalysisValueData::None) => true,
            (AnalysisValueData::Boolean(left), AnalysisValueData::Boolean(right)) => left == right,
            (AnalysisValueData::Number(left), AnalysisValueData::Number(right)) => {
                left.publication_eq(right)
            }
            (AnalysisValueData::String(left), AnalysisValueData::String(right)) => left == right,
            (AnalysisValueData::Label(left), AnalysisValueData::Label(right)) => left == right,
            (
                AnalysisValueData::ConfiguredTarget(left),
                AnalysisValueData::ConfiguredTarget(right),
            ) => {
                left.identity == right.identity
                    && left.providers.publication_eq_with(&right.providers, state)
            }
            (AnalysisValueData::Artifact(left), AnalysisValueData::Artifact(right)) => {
                left == right
            }
            (AnalysisValueData::List(left), AnalysisValueData::List(right))
            | (AnalysisValueData::Tuple(left), AnalysisValueData::Tuple(right)) => {
                publication_slice_eq(left, right, state)
            }
            (AnalysisValueData::Dictionary(left), AnalysisValueData::Dictionary(right)) => {
                left.len() == right.len()
                    && left.iter().zip(right.iter()).all(
                        |((left_key, left_value), (right_key, right_value))| {
                            left_key.publication_eq_with(right_key, state)
                                && left_value.publication_eq_with(right_value, state)
                        },
                    )
            }
            (AnalysisValueData::Struct(left), AnalysisValueData::Struct(right)) => {
                publication_fields_eq(left, right, state)
            }
            (AnalysisValueData::Provider(left), AnalysisValueData::Provider(right)) => {
                left.publication_eq_with(right, state)
            }
            (AnalysisValueData::Depset(left), AnalysisValueData::Depset(right)) => {
                left.publication_eq_with(right, state)
            }
            _ => false,
        }
    }
}

fn publication_slice_eq(
    left: &[AnalysisValue],
    right: &[AnalysisValue],
    state: &mut PublicationEqState,
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.publication_eq_with(right, state))
}

fn publication_fields_eq(
    left: &SmallMap<CompactString, AnalysisValue>,
    right: &SmallMap<CompactString, AnalysisValue>,
    state: &mut PublicationEqState,
) -> bool {
    left.len() == right.len()
        && left.iter().all(|(name, value)| {
            right
                .get(name)
                .is_some_and(|right| value.publication_eq_with(right, state))
        })
}

impl PartialEq for AnalysisValue {
    fn eq(&self, other: &Self) -> bool {
        match (self.0.as_ref(), other.0.as_ref()) {
            (AnalysisValueData::None, AnalysisValueData::None) => true,
            (AnalysisValueData::Boolean(left), AnalysisValueData::Boolean(right)) => left == right,
            (AnalysisValueData::Number(left), AnalysisValueData::Number(right)) => left == right,
            (AnalysisValueData::String(left), AnalysisValueData::String(right)) => left == right,
            (AnalysisValueData::Label(left), AnalysisValueData::Label(right)) => left == right,
            (
                AnalysisValueData::ConfiguredTarget(left),
                AnalysisValueData::ConfiguredTarget(right),
            ) => left.identity == right.identity,
            (AnalysisValueData::Artifact(left), AnalysisValueData::Artifact(right)) => {
                left == right
            }
            (AnalysisValueData::List(left), AnalysisValueData::List(right))
            | (AnalysisValueData::Tuple(left), AnalysisValueData::Tuple(right)) => left == right,
            (AnalysisValueData::Dictionary(left), AnalysisValueData::Dictionary(right)) => {
                left.len() == right.len()
                    && left.iter().all(|(key, value)| {
                        right
                            .iter()
                            .find(|(candidate, _)| candidate == key)
                            .is_some_and(|(_, candidate)| candidate == value)
                    })
            }
            (AnalysisValueData::Struct(left), AnalysisValueData::Struct(right)) => left == right,
            (AnalysisValueData::Provider(left), AnalysisValueData::Provider(right)) => {
                left == right
            }
            (AnalysisValueData::Depset(left), AnalysisValueData::Depset(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for AnalysisValue {}

impl Hash for AnalysisValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.0.as_ref() {
            AnalysisValueData::None => 0u8.hash(state),
            AnalysisValueData::Boolean(value) => {
                1u8.hash(state);
                value.hash(state);
            }
            AnalysisValueData::Number(value) => {
                2u8.hash(state);
                value.hash(state);
            }
            AnalysisValueData::String(value) => {
                3u8.hash(state);
                value.hash(state);
            }
            AnalysisValueData::Label(value) => {
                4u8.hash(state);
                value.hash(state);
            }
            AnalysisValueData::ConfiguredTarget(value) => {
                5u8.hash(state);
                value.identity.hash(state);
            }
            AnalysisValueData::Artifact(value) => {
                6u8.hash(state);
                value.hash(state);
            }
            AnalysisValueData::List(value) => {
                7u8.hash(state);
                value.hash(state);
            }
            AnalysisValueData::Tuple(value) => {
                8u8.hash(state);
                value.hash(state);
            }
            AnalysisValueData::Dictionary(value) => {
                9u8.hash(state);
                hash_unordered_entries(value, state);
            }
            AnalysisValueData::Struct(value) => {
                10u8.hash(state);
                value.len().hash(state);
                for field in value {
                    field.hash(state);
                }
            }
            AnalysisValueData::Provider(value) => {
                11u8.hash(state);
                value.hash(state);
            }
            AnalysisValueData::Depset(value) => {
                12u8.hash(state);
                value.hash(state);
            }
        }
    }
}

fn hash_unordered_entries<H: Hasher>(entries: &[(AnalysisValue, AnalysisValue)], state: &mut H) {
    let mut xor = 0u64;
    let mut sum = 0u64;
    for entry in entries {
        let mut hasher = StarlarkHasher::default();
        entry.hash(&mut hasher);
        let hash = hasher.finish();
        xor ^= hash.rotate_left(17);
        sum = sum.wrapping_add(hash);
    }
    entries.len().hash(state);
    xor.hash(state);
    sum.hash(state);
}

#[derive(Default)]
pub(crate) struct PublicationEqState {
    left_depsets: FxHashMap<usize, usize>,
    right_depsets: FxHashMap<usize, usize>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AnalysisValueError {
    UnhashableDictionaryKey {
        value_type: AnalysisValueType,
    },
    DuplicateDictionaryKey,
    InvalidDepsetLeaf {
        value_type: AnalysisValueType,
    },
    HeterogeneousDepset {
        existing: AnalysisValueType,
        candidate: AnalysisValueType,
    },
    Depset(DepsetError),
}

impl fmt::Display for AnalysisValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnhashableDictionaryKey { value_type } => {
                write!(f, "unhashable type: '{value_type}'")
            }
            Self::DuplicateDictionaryKey => f.write_str("dictionary key specified twice"),
            Self::InvalidDepsetLeaf { value_type } => {
                write!(f, "depsets cannot contain items of type '{value_type}'")
            }
            Self::HeterogeneousDepset {
                existing,
                candidate,
            } => write!(
                f,
                "cannot add an item of type '{candidate}' to a depset of '{existing}'"
            ),
            Self::Depset(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl Error for AnalysisValueError {}

impl From<DepsetError> for AnalysisValueError {
    fn from(value: DepsetError) -> Self {
        Self::Depset(value)
    }
}
