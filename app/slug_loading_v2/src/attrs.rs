/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Loading-owned, unconfigured attribute metadata.
//!
//! These values deliberately retain configurable structure.  In particular,
//! they are not an alternate spelling of a rule's aggregate dependency list:
//! Stage 8 will project reachable labels from this representation.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use starlark::values::FrozenValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum AttributeKind {
    Label,
    LabelList,
    StringKeyedLabelDict,
    LabelKeyedStringDict,
    LabelListDict,
    Output,
    OutputList,
    String,
    StringList,
    StringListDict,
    Boolean,
    Integer,
    StringDict,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum AllowSingleFile {
    False,
    True,
    Extensions(Arc<[CompactString]>),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum AllowedAttributeValues {
    None,
    Integer(Arc<[i32]>),
    String(Arc<[CompactString]>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Allocative)]
pub(crate) struct AttributeFlags(u32);

impl AttributeFlags {
    const DIRECT_COMPILE_TIME_INPUT: u32 = 1 << 6;

    pub(crate) fn insert_direct_compile_time_input(&mut self) {
        self.0 |= Self::DIRECT_COMPILE_TIME_INPUT;
    }

    pub(crate) fn direct_compile_time_input(self) -> bool {
        self.0 & Self::DIRECT_COMPILE_TIME_INPUT != 0
    }

    pub(crate) fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum AttributeDependencyConfiguration {
    Target,
    Exec,
    Starlark(TransitionDefinition),
}

impl AttributeKind {
    pub(crate) fn reaches_labels(self) -> bool {
        !matches!(
            self,
            Self::String
                | Self::StringList
                | Self::StringListDict
                | Self::Boolean
                | Self::Integer
                | Self::StringDict
        )
    }

    pub(crate) fn contributes_ordinary_dependencies(self) -> bool {
        matches!(
            self,
            Self::Label
                | Self::LabelList
                | Self::StringKeyedLabelDict
                | Self::LabelKeyedStringDict
                | Self::LabelListDict
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct AttributeSchema {
    declaration_name: CompactString,
    query_name: CompactString,
    kind: AttributeKind,
    mandatory: bool,
    configurable: bool,
    label_reachable: bool,
    order_independent: bool,
    ordinary_dependency: bool,
    builtin: bool,
    allow_files: bool,
    allow_single_file: Option<AllowSingleFile>,
    flags: AttributeFlags,
    allowed_values: AllowedAttributeValues,
    default: Option<Arc<CoercedAttributeValue>>,
    dependency_configuration: AttributeDependencyConfiguration,
    executable: bool,
}

impl AttributeSchema {
    pub(crate) fn new(
        declaration_name: impl Into<CompactString>,
        kind: AttributeKind,
        mandatory: bool,
        configurable: bool,
        default: Option<CoercedAttributeValue>,
    ) -> Self {
        let declaration_name = declaration_name.into();
        let query_name = declaration_name
            .strip_prefix('_')
            .map(|name| CompactString::from(format!("${name}")))
            .unwrap_or_else(|| declaration_name.clone());
        Self {
            declaration_name,
            query_name,
            kind,
            mandatory,
            configurable,
            label_reachable: kind.reaches_labels(),
            order_independent: false,
            ordinary_dependency: kind.contributes_ordinary_dependencies(),
            builtin: false,
            allow_files: false,
            allow_single_file: None,
            flags: AttributeFlags::default(),
            allowed_values: AllowedAttributeValues::None,
            default: default.map(Arc::new),
            dependency_configuration: AttributeDependencyConfiguration::Target,
            executable: false,
        }
    }

    /// Constructs one of Bazel's fixed RuleClass attributes.  User-declared
    /// Starlark attributes retain the ordinary `new` defaults above; built-ins
    /// carry their separately observable ordering and topology policy.
    pub(crate) fn builtin(
        declaration_name: impl Into<CompactString>,
        kind: AttributeKind,
        mandatory: bool,
        configurable: bool,
        default: Option<CoercedAttributeValue>,
        order_independent: bool,
        ordinary_dependency: bool,
    ) -> Self {
        let mut schema = Self::new(declaration_name, kind, mandatory, configurable, default);
        schema.order_independent = order_independent;
        schema.ordinary_dependency = ordinary_dependency;
        schema.builtin = true;
        schema
    }

    pub fn declaration_name(&self) -> &str {
        &self.declaration_name
    }
    pub fn query_name(&self) -> &str {
        &self.query_name
    }
    pub fn kind(&self) -> AttributeKind {
        self.kind
    }
    pub fn mandatory(&self) -> bool {
        self.mandatory
    }
    pub fn configurable(&self) -> bool {
        self.configurable
    }
    pub fn dependency_reachable(&self) -> bool {
        self.label_reachable
    }
    pub fn ordinary_dependency(&self) -> bool {
        self.ordinary_dependency
    }
    pub fn is_builtin(&self) -> bool {
        self.builtin
    }
    pub fn allow_files(&self) -> bool {
        self.allow_files
    }
    pub(crate) fn order_independent(&self) -> bool {
        self.order_independent
    }
    pub fn allow_single_file(&self) -> Option<&AllowSingleFile> {
        self.allow_single_file.as_ref()
    }
    pub fn direct_compile_time_input(&self) -> bool {
        self.flags.direct_compile_time_input()
    }
    pub(crate) fn allowed_values(&self) -> &AllowedAttributeValues {
        &self.allowed_values
    }
    pub(crate) fn with_allow_single_file(
        mut self,
        allow_single_file: Option<AllowSingleFile>,
    ) -> Self {
        self.allow_single_file = allow_single_file;
        self
    }
    pub(crate) fn with_allow_files(mut self, allow_files: bool) -> Self {
        self.allow_files = allow_files;
        self
    }
    pub(crate) fn with_flags(mut self, flags: AttributeFlags) -> Self {
        self.flags = flags;
        self
    }
    pub(crate) fn with_allowed_values(mut self, values: AllowedAttributeValues) -> Self {
        self.allowed_values = values;
        self
    }
    pub fn default(&self) -> Option<&CoercedAttributeValue> {
        self.default.as_deref()
    }
    pub fn dependency_configuration(&self) -> &AttributeDependencyConfiguration {
        &self.dependency_configuration
    }
    pub fn executable(&self) -> bool {
        self.executable
    }
    pub fn transition(&self) -> Option<&TransitionDefinition> {
        match &self.dependency_configuration {
            AttributeDependencyConfiguration::Starlark(transition) => Some(transition),
            AttributeDependencyConfiguration::Target | AttributeDependencyConfiguration::Exec => {
                None
            }
        }
    }
    pub(crate) fn with_dependency_configuration(
        mut self,
        dependency_configuration: AttributeDependencyConfiguration,
        executable: bool,
    ) -> Self {
        self.dependency_configuration = dependency_configuration;
        self.executable = executable;
        self
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct TransitionDefinition {
    #[allocative(skip)]
    implementation: FrozenValue,
    output: CompactString,
}
impl TransitionDefinition {
    pub fn new(implementation: FrozenValue, output: impl Into<CompactString>) -> Self {
        Self {
            implementation,
            output: output.into(),
        }
    }
    pub fn implementation(&self) -> FrozenValue {
        self.implementation
    }
    pub fn output(&self) -> &str {
        &self.output
    }
}
impl PartialEq for TransitionDefinition {
    fn eq(&self, other: &Self) -> bool {
        self.output == other.output
    }
}
impl Eq for TransitionDefinition {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum AttributeProvenance {
    Explicit,
    Default,
    Implicit,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct AttributeValue {
    pub declaration_name: CompactString,
    pub provenance: AttributeProvenance,
    pub value: Arc<CoercedAttributeValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAttributeOrder {
    Ordered,
    OrderIndependent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAttributePolicy {
    Callable,
    Implicit,
    Forced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeAttributeSchema {
    query_name: &'static str,
    kind: AttributeKind,
    order: NativeAttributeOrder,
    policy: NativeAttributePolicy,
}

impl NativeAttributeSchema {
    pub const fn query_name(self) -> &'static str {
        self.query_name
    }

    pub const fn kind(self) -> AttributeKind {
        self.kind
    }

    pub const fn order(self) -> NativeAttributeOrder {
        self.order
    }

    pub const fn policy(self) -> NativeAttributePolicy {
        self.policy
    }
}

macro_rules! native_schema {
    ($name:ident = [$($query_name:literal : $kind:ident, $order:ident, $policy:ident;)+]) => {
        static $name: &[NativeAttributeSchema] = &[
            $(NativeAttributeSchema {
                query_name: $query_name,
                kind: AttributeKind::$kind,
                order: NativeAttributeOrder::$order,
                policy: NativeAttributePolicy::$policy,
            },)+
        ];
    };
}

native_schema!(FILEGROUP_SCHEMA = [
    "name": String, Ordered, Callable;
    "visibility": LabelList, OrderIndependent, Callable;
    "transitive_configs": LabelList, OrderIndependent, Callable;
    "deprecation": String, Ordered, Callable;
    "tags": StringList, OrderIndependent, Callable;
    "generator_name": String, Ordered, Callable;
    "generator_function": String, Ordered, Callable;
    "generator_location": String, Ordered, Callable;
    "testonly": Boolean, Ordered, Callable;
    "features": StringList, OrderIndependent, Callable;
    ":action_listener": LabelList, Ordered, Implicit;
    "compatible_with": LabelList, Ordered, Callable;
    "restricted_to": LabelList, Ordered, Callable;
    "$config_dependencies": LabelList, Ordered, Implicit;
    "package_metadata": LabelList, Ordered, Callable;
    "aspect_hints": LabelList, Ordered, Callable;
    "licenses": StringList, Ordered, Callable;
    "distribs": StringList, Ordered, Callable;
    "target_compatible_with": LabelList, Ordered, Callable;
    "srcs": LabelList, Ordered, Callable;
    "output_group": String, Ordered, Callable;
    "data": LabelList, Ordered, Callable;
    "output_licenses": StringList, Ordered, Callable;
]);

native_schema!(ALIAS_SCHEMA = [
    "name": String, Ordered, Callable;
    "visibility": LabelList, OrderIndependent, Callable;
    "transitive_configs": LabelList, OrderIndependent, Callable;
    "deprecation": String, Ordered, Callable;
    "tags": StringList, OrderIndependent, Callable;
    "generator_name": String, Ordered, Callable;
    "generator_function": String, Ordered, Callable;
    "generator_location": String, Ordered, Callable;
    "testonly": Boolean, Ordered, Callable;
    "features": StringList, OrderIndependent, Callable;
    "compatible_with": LabelList, Ordered, Callable;
    "restricted_to": LabelList, Ordered, Callable;
    "$config_dependencies": LabelList, Ordered, Implicit;
    "package_metadata": LabelList, Ordered, Callable;
    "aspect_hints": LabelList, Ordered, Callable;
    "target_compatible_with": LabelList, Ordered, Callable;
    "actual": Label, Ordered, Callable;
]);

native_schema!(CONFIG_SETTING_SCHEMA = [
    "name": String, Ordered, Callable;
    "visibility": LabelList, OrderIndependent, Callable;
    "transitive_configs": LabelList, OrderIndependent, Callable;
    "deprecation": String, Ordered, Callable;
    "tags": StringList, OrderIndependent, Callable;
    "generator_name": String, Ordered, Callable;
    "generator_function": String, Ordered, Callable;
    "generator_location": String, Ordered, Callable;
    "testonly": Boolean, Ordered, Callable;
    "features": StringList, OrderIndependent, Callable;
    ":action_listener": LabelList, Ordered, Implicit;
    "$config_dependencies": LabelList, Ordered, Implicit;
    "package_metadata": LabelList, Ordered, Callable;
    "aspect_hints": LabelList, Ordered, Callable;
    "licenses": StringList, Ordered, Forced;
    "distribs": StringList, Ordered, Callable;
    "values": StringDict, Ordered, Callable;
    "define_values": StringDict, Ordered, Callable;
    "flag_values": LabelKeyedStringDict, Ordered, Callable;
    "constraint_values": LabelList, Ordered, Callable;
    ":flag_alias_settings": LabelList, Ordered, Implicit;
]);

native_schema!(TEST_SUITE_SCHEMA = [
    "name": String, Ordered, Callable;
    "visibility": LabelList, OrderIndependent, Callable;
    "transitive_configs": LabelList, OrderIndependent, Callable;
    "deprecation": String, Ordered, Callable;
    "tags": StringList, OrderIndependent, Callable;
    "generator_name": String, Ordered, Callable;
    "generator_function": String, Ordered, Callable;
    "generator_location": String, Ordered, Callable;
    "testonly": Boolean, Ordered, Callable;
    "features": StringList, OrderIndependent, Callable;
    ":action_listener": LabelList, Ordered, Implicit;
    "compatible_with": LabelList, Ordered, Callable;
    "restricted_to": LabelList, Ordered, Callable;
    "$config_dependencies": LabelList, Ordered, Implicit;
    "package_metadata": LabelList, Ordered, Callable;
    "aspect_hints": LabelList, Ordered, Callable;
    "licenses": StringList, Ordered, Callable;
    "distribs": StringList, Ordered, Callable;
    "target_compatible_with": LabelList, Ordered, Callable;
    "tests": LabelList, OrderIndependent, Callable;
    "$implicit_tests": LabelList, OrderIndependent, Implicit;
]);

native_schema!(CONSTRAINT_SETTING_SCHEMA = [
    "name": String, Ordered, Callable;
    "visibility": LabelList, OrderIndependent, Callable;
    "transitive_configs": LabelList, OrderIndependent, Callable;
    "deprecation": String, Ordered, Callable;
    "tags": StringList, OrderIndependent, Callable;
    "generator_name": String, Ordered, Callable;
    "generator_function": String, Ordered, Callable;
    "generator_location": String, Ordered, Callable;
    "testonly": Boolean, Ordered, Callable;
    "features": StringList, OrderIndependent, Callable;
    "$config_dependencies": LabelList, Ordered, Implicit;
    "aspect_hints": LabelList, Ordered, Callable;
    "licenses": StringList, Ordered, Callable;
    "distribs": StringList, Ordered, Callable;
    "default_constraint_value": Label, Ordered, Callable;
    "refines_constraint_value": Label, Ordered, Callable;
]);

native_schema!(CONSTRAINT_VALUE_SCHEMA = [
    "name": String, Ordered, Callable;
    "visibility": LabelList, OrderIndependent, Callable;
    "transitive_configs": LabelList, OrderIndependent, Callable;
    "deprecation": String, Ordered, Callable;
    "tags": StringList, OrderIndependent, Callable;
    "generator_name": String, Ordered, Callable;
    "generator_function": String, Ordered, Callable;
    "generator_location": String, Ordered, Callable;
    "testonly": Boolean, Ordered, Callable;
    "features": StringList, OrderIndependent, Callable;
    "$config_dependencies": LabelList, Ordered, Implicit;
    "aspect_hints": LabelList, Ordered, Callable;
    "licenses": StringList, Ordered, Callable;
    "distribs": StringList, Ordered, Callable;
    "constraint_setting": Label, Ordered, Callable;
]);

native_schema!(PLATFORM_SCHEMA = [
    "name": String, Ordered, Callable;
    "visibility": LabelList, OrderIndependent, Callable;
    "transitive_configs": LabelList, OrderIndependent, Callable;
    "deprecation": String, Ordered, Callable;
    "tags": StringList, OrderIndependent, Callable;
    "generator_name": String, Ordered, Callable;
    "generator_function": String, Ordered, Callable;
    "generator_location": String, Ordered, Callable;
    "testonly": Boolean, Ordered, Callable;
    "features": StringList, OrderIndependent, Callable;
    "$config_dependencies": LabelList, Ordered, Implicit;
    "aspect_hints": LabelList, Ordered, Callable;
    "licenses": StringList, Ordered, Callable;
    "distribs": StringList, Ordered, Callable;
    "constraint_values": LabelList, Ordered, Callable;
    "parents": LabelList, Ordered, Callable;
    "remote_execution_properties": String, Ordered, Callable;
    "exec_properties": StringDict, Ordered, Callable;
    "flags": StringList, Ordered, Callable;
    "missing_toolchain_error": String, Ordered, Callable;
    "required_settings": LabelList, Ordered, Callable;
    "check_toolchain_types": Boolean, Ordered, Callable;
    "allowed_toolchain_types": LabelList, Ordered, Callable;
]);

native_schema!(TOOLCHAIN_TYPE_SCHEMA = [
    "name": String, Ordered, Callable;
    "visibility": LabelList, OrderIndependent, Callable;
    "transitive_configs": LabelList, OrderIndependent, Callable;
    "deprecation": String, Ordered, Callable;
    "tags": StringList, OrderIndependent, Callable;
    "generator_name": String, Ordered, Callable;
    "generator_function": String, Ordered, Callable;
    "generator_location": String, Ordered, Callable;
    "testonly": Boolean, Ordered, Callable;
    "features": StringList, OrderIndependent, Callable;
    "compatible_with": LabelList, Ordered, Callable;
    "restricted_to": LabelList, Ordered, Callable;
    "$config_dependencies": LabelList, Ordered, Implicit;
    "package_metadata": LabelList, Ordered, Callable;
    "aspect_hints": LabelList, Ordered, Callable;
    "target_compatible_with": LabelList, Ordered, Callable;
    "no_match_error": String, Ordered, Callable;
]);

native_schema!(TOOLCHAIN_SCHEMA = [
    "name": String, Ordered, Callable;
    "visibility": LabelList, OrderIndependent, Callable;
    "transitive_configs": LabelList, OrderIndependent, Callable;
    "deprecation": String, Ordered, Callable;
    "tags": StringList, OrderIndependent, Callable;
    "generator_name": String, Ordered, Callable;
    "generator_function": String, Ordered, Callable;
    "generator_location": String, Ordered, Callable;
    "testonly": Boolean, Ordered, Callable;
    "features": StringList, OrderIndependent, Callable;
    "$config_dependencies": LabelList, Ordered, Implicit;
    "package_metadata": LabelList, Ordered, Callable;
    "aspect_hints": LabelList, Ordered, Callable;
    "licenses": StringList, Ordered, Callable;
    "distribs": StringList, Ordered, Callable;
    "target_compatible_with": LabelList, Ordered, Callable;
    "toolchain_type": Label, Ordered, Callable;
    "toolchain": Label, Ordered, Callable;
    "exec_compatible_with": LabelList, Ordered, Callable;
    "use_target_platform_constraints": Boolean, Ordered, Callable;
    "target_settings": LabelList, Ordered, Callable;
]);

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum NativeRuleClass {
    Filegroup,
    Alias,
    ConfigSetting,
    TestSuite,
    ConstraintSetting,
    ConstraintValue,
    Platform,
    ToolchainType,
    Toolchain,
}

impl NativeRuleClass {
    pub const fn schema(self) -> &'static [NativeAttributeSchema] {
        match self {
            Self::Filegroup => FILEGROUP_SCHEMA,
            Self::Alias => ALIAS_SCHEMA,
            Self::ConfigSetting => CONFIG_SETTING_SCHEMA,
            Self::TestSuite => TEST_SUITE_SCHEMA,
            Self::ConstraintSetting => CONSTRAINT_SETTING_SCHEMA,
            Self::ConstraintValue => CONSTRAINT_VALUE_SCHEMA,
            Self::Platform => PLATFORM_SCHEMA,
            Self::ToolchainType => TOOLCHAIN_TYPE_SCHEMA,
            Self::Toolchain => TOOLCHAIN_SCHEMA,
        }
    }

    pub fn slot(self, name: &str) -> Option<(usize, NativeAttributeSchema)> {
        self.schema()
            .iter()
            .copied()
            .enumerate()
            .find(|(_, schema)| schema.query_name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NativeAttributeValue {
    pub provenance: AttributeProvenance,
    pub value: CoercedAttributeValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NativeRuleAttributes {
    pub class: NativeRuleClass,
    values: Arc<[NativeAttributeValue]>,
}

impl NativeRuleAttributes {
    pub fn new(class: NativeRuleClass, values: Vec<NativeAttributeValue>) -> Self {
        assert_eq!(class.schema().len(), values.len());
        Self {
            class,
            values: values.into(),
        }
    }

    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (NativeAttributeSchema, &NativeAttributeValue)> {
        self.class.schema().iter().copied().zip(self.values.iter())
    }

    pub fn get(&self, name: &str) -> Option<(NativeAttributeSchema, &NativeAttributeValue)> {
        self.class
            .slot(name)
            .map(|(slot, schema)| (schema, &self.values[slot]))
    }

    pub(crate) fn values_mut(&mut self) -> &mut [NativeAttributeValue] {
        Arc::make_mut(&mut self.values)
    }
}

/// Immutable attribute data that an unconfigured query may inspect.
///
/// This stays owned by loading: query projections retain the already-coerced
/// value rather than creating a string rendering or a second attribute model.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct AttributeQueryValue {
    pub kind: AttributeKind,
    pub provenance: AttributeProvenance,
    pub value: CoercedAttributeValue,
}

impl AttributeValue {
    pub fn query_value(&self, schema: &AttributeSchema) -> AttributeQueryValue {
        debug_assert_eq!(self.declaration_name, schema.declaration_name());
        AttributeQueryValue {
            kind: schema.kind(),
            provenance: self.provenance,
            value: self.value.as_ref().clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum CoercedAttributeValue {
    /// Bazel's optional scalar-label default is null; it is not a label.
    None,
    Label(CanonicalLabel),
    LabelList(Arc<[CanonicalLabel]>),
    String(CompactString),
    StringList(Arc<[CompactString]>),
    StringListDict(Arc<[(CompactString, Arc<[CompactString]>)]>),
    Boolean(bool),
    Integer(i32),
    StringDict(Arc<[(CompactString, CompactString)]>),
    StringKeyedLabelDict(Arc<[(CompactString, CanonicalLabel)]>),
    LabelKeyedStringDict(Arc<[(CanonicalLabel, CompactString)]>),
    LabelListDict(Arc<[(CompactString, Arc<[CanonicalLabel]>)]>),
    Output(CanonicalLabel),
    OutputList(Arc<[CanonicalLabel]>),
    Selector {
        /// Condition labels deliberately remain separate from branch values;
        /// `getReachableLabels(..., false)` excludes these keys.
        branches: Arc<[(CanonicalLabel, Arc<CoercedAttributeValue>)]>,
        default: Option<Arc<CoercedAttributeValue>>,
    },
    Concatenation(Arc<CoercedAttributeValue>, Arc<CoercedAttributeValue>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrCandidateError {
    left: &'static str,
    right: &'static str,
}

impl fmt::Display for AttrCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cannot concatenate attribute candidate types {} and {}",
            self.left, self.right
        )
    }
}

impl std::error::Error for AttrCandidateError {}

impl CoercedAttributeValue {
    /// Returns the selector condition labels reachable from this retained
    /// expression, in first-seen order. Selector keys are configuration
    /// dependencies, not ordinary labels from branch values.
    pub fn selector_key_labels(&self) -> Vec<CanonicalLabel> {
        fn collect(value: &CoercedAttributeValue, labels: &mut Vec<CanonicalLabel>) {
            match value {
                CoercedAttributeValue::Selector { branches, default } => {
                    for (condition, branch) in branches.iter() {
                        if !labels.contains(condition) {
                            labels.push(condition.clone());
                        }
                        collect(branch, labels);
                    }
                    if let Some(default) = default {
                        collect(default, labels);
                    }
                }
                CoercedAttributeValue::Concatenation(left, right) => {
                    collect(left, labels);
                    collect(right, labels);
                }
                _ => {}
            }
        }

        let mut labels = Vec::new();
        collect(self, &mut labels);
        labels
    }

    /// Concatenates two already-resolved values using the retained attribute
    /// type as the single owner of Bazel's typed `+` behavior.
    pub fn concatenate_resolved(&self, right: &Self) -> Result<Self, AttrCandidateError> {
        fn merged<K: Clone + PartialEq, V: Clone>(
            left: &[(K, V)],
            right: &[(K, V)],
        ) -> Arc<[(K, V)]> {
            let mut result = left.to_vec();
            for (key, value) in right {
                if let Some((_, existing)) = result.iter_mut().find(|(existing, _)| existing == key)
                {
                    *existing = value.clone();
                } else {
                    result.push((key.clone(), value.clone()));
                }
            }
            result.into()
        }

        let shape = |value: &Self| match value {
            Self::String(_) => "string",
            Self::LabelList(_) | Self::StringList(_) | Self::OutputList(_) => "list",
            Self::StringListDict(_)
            | Self::StringDict(_)
            | Self::StringKeyedLabelDict(_)
            | Self::LabelKeyedStringDict(_)
            | Self::LabelListDict(_) => "dictionary",
            Self::None => "None",
            Self::Label(_) | Self::Output(_) => "label",
            Self::Boolean(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::Selector { .. } => "selector",
            Self::Concatenation(_, _) => "concatenation",
        };
        let mismatch = || AttrCandidateError {
            left: shape(self),
            right: shape(right),
        };
        Ok(match (self, right) {
            (Self::String(left), Self::String(right)) => {
                let mut value = left.clone();
                value.push_str(right);
                Self::String(value)
            }
            (Self::LabelList(left), Self::LabelList(right)) => Self::LabelList(
                left.iter()
                    .chain(right.iter())
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ),
            (Self::StringList(left), Self::StringList(right)) => Self::StringList(
                left.iter()
                    .chain(right.iter())
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ),
            (Self::OutputList(left), Self::OutputList(right)) => Self::OutputList(
                left.iter()
                    .chain(right.iter())
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ),
            (Self::StringListDict(left), Self::StringListDict(right)) => {
                Self::StringListDict(merged(left, right))
            }
            (Self::StringDict(left), Self::StringDict(right)) => {
                Self::StringDict(merged(left, right))
            }
            (Self::StringKeyedLabelDict(left), Self::StringKeyedLabelDict(right)) => {
                Self::StringKeyedLabelDict(merged(left, right))
            }
            (Self::LabelKeyedStringDict(left), Self::LabelKeyedStringDict(right)) => {
                Self::LabelKeyedStringDict(merged(left, right))
            }
            (Self::LabelListDict(left), Self::LabelListDict(right)) => {
                Self::LabelListDict(merged(left, right))
            }
            _ => return Err(mismatch()),
        })
    }

    /// Rebind labels parsed in a repository package's provisional root context.
    pub fn rebind_provisional_root_labels(
        &self,
        destination: &CanonicalRepoName,
    ) -> Result<Self, String> {
        let label = |value: &CanonicalLabel| {
            if value.package().repo().is_root() {
                value.rebind_provisional_root_repository(destination)
            } else {
                Ok(value.clone())
            }
        };
        Ok(match self {
            Self::None => Self::None,
            Self::Label(value) => Self::Label(label(value)?),
            Self::Output(value) => Self::Output(label(value)?),
            Self::LabelList(values) => Self::LabelList(
                values
                    .iter()
                    .map(label)
                    .collect::<Result<Vec<_>, _>>()?
                    .into(),
            ),
            Self::OutputList(values) => Self::OutputList(
                values
                    .iter()
                    .map(label)
                    .collect::<Result<Vec<_>, _>>()?
                    .into(),
            ),
            Self::StringKeyedLabelDict(values) => Self::StringKeyedLabelDict(
                values
                    .iter()
                    .map(|(key, value)| Ok((key.clone(), label(value)?)))
                    .collect::<Result<Vec<_>, String>>()?
                    .into(),
            ),
            Self::LabelKeyedStringDict(values) => Self::LabelKeyedStringDict(
                values
                    .iter()
                    .map(|(key, value)| Ok((label(key)?, value.clone())))
                    .collect::<Result<Vec<_>, String>>()?
                    .into(),
            ),
            Self::LabelListDict(values) => Self::LabelListDict(
                values
                    .iter()
                    .map(|(key, labels)| {
                        Ok((
                            key.clone(),
                            labels
                                .iter()
                                .map(label)
                                .collect::<Result<Vec<_>, _>>()?
                                .into(),
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?
                    .into(),
            ),
            Self::Selector { branches, default } => Self::Selector {
                branches: branches
                    .iter()
                    .map(|(condition, value)| {
                        Ok((
                            label(condition)?,
                            Arc::new(value.rebind_provisional_root_labels(destination)?),
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?
                    .into(),
                default: default
                    .as_ref()
                    .map(|value| {
                        value
                            .rebind_provisional_root_labels(destination)
                            .map(Arc::new)
                    })
                    .transpose()?,
            },
            Self::Concatenation(left, right) => Self::Concatenation(
                Arc::new(left.rebind_provisional_root_labels(destination)?),
                Arc::new(right.rebind_provisional_root_labels(destination)?),
            ),
            Self::String(value) => Self::String(value.clone()),
            Self::StringList(values) => Self::StringList(values.clone()),
            Self::StringListDict(values) => Self::StringListDict(values.clone()),
            Self::Boolean(value) => Self::Boolean(*value),
            Self::Integer(value) => Self::Integer(*value),
            Self::StringDict(values) => Self::StringDict(values.clone()),
        })
    }

    /// Returns every Bazel-visible string candidate for an unconfigured `attr()` query.
    ///
    /// The strings are rendered only for this request. Loading continues to retain the
    /// typed value, including selector structure, rather than a second cached string
    /// representation. `None` is an unset optional scalar and therefore contributes no
    /// candidate.
    pub fn attr_visible_candidates(
        &self,
        render_label: impl Fn(&CanonicalLabel) -> CompactString,
    ) -> Result<Vec<CompactString>, AttrCandidateError> {
        Ok(expand_attr_candidates(self)?
            .into_iter()
            .map(|candidate| candidate.value.render(&render_label))
            .collect())
    }

    pub fn labels(&self, labels: &mut Vec<CanonicalLabel>) {
        match self {
            Self::Label(label) | Self::Output(label) => labels.push(label.clone()),
            Self::LabelList(values) | Self::OutputList(values) => {
                labels.extend(values.iter().cloned())
            }
            Self::StringKeyedLabelDict(values) => {
                labels.extend(values.iter().map(|(_, value)| value.clone()))
            }
            Self::LabelKeyedStringDict(values) => {
                labels.extend(values.iter().map(|(key, _)| key.clone()))
            }
            Self::LabelListDict(values) => {
                labels.extend(values.iter().flat_map(|(_, values)| values.iter().cloned()))
            }
            Self::Selector { branches, default } => {
                for (_, value) in branches.iter() {
                    value.labels(labels);
                }
                if let Some(default) = default {
                    default.labels(labels);
                }
            }
            Self::Concatenation(left, right) => {
                left.labels(labels);
                right.labels(labels);
            }
            Self::String(_)
            | Self::StringList(_)
            | Self::StringListDict(_)
            | Self::Boolean(_)
            | Self::Integer(_)
            | Self::StringDict(_)
            | Self::None => {}
        }
    }
}

impl AttributeQueryValue {
    /// Request-time candidates for a later ordinary-query `attr()` matcher.
    pub fn attr_visible_candidates(
        &self,
        render_label: impl Fn(&CanonicalLabel) -> CompactString,
    ) -> Result<Vec<CompactString>, AttrCandidateError> {
        self.value.attr_visible_candidates(render_label)
    }
}

/// A temporary typed value is necessary because configurable list and dictionary
/// concatenations are rendered only after their branches have been combined.
/// It deliberately does not escape `attr_visible_candidates`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AttrCandidateAtom<'a> {
    String(CompactString),
    Label(&'a CanonicalLabel),
}

impl AttrCandidateAtom<'_> {
    fn render(&self, render_label: &impl Fn(&CanonicalLabel) -> CompactString) -> CompactString {
        match self {
            Self::String(value) => value.clone(),
            Self::Label(label) => render_label(label),
        }
    }
}

#[derive(Debug, Clone)]
enum AttrCandidateValue<'a> {
    Scalar(AttrCandidateAtom<'a>),
    List(Vec<AttrCandidateAtom<'a>>),
    Dict(Vec<(AttrCandidateAtom<'a>, AttrCandidateValue<'a>)>),
}

impl AttrCandidateValue<'_> {
    fn render(self, render_label: &impl Fn(&CanonicalLabel) -> CompactString) -> CompactString {
        match self {
            Self::Scalar(value) => value.render(render_label),
            Self::List(values) => CompactString::new(format!(
                "[{}]",
                values
                    .iter()
                    .map(|value| value.render(render_label))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Self::Dict(entries) => CompactString::new(format!(
                "{{{}}}",
                entries
                    .into_iter()
                    .map(|(key, value)| {
                        format!(
                            "{}={}",
                            key.render(render_label),
                            value.render(render_label)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    fn shape(&self) -> &'static str {
        match self {
            Self::Scalar(AttrCandidateAtom::String(_)) => "string",
            Self::Scalar(AttrCandidateAtom::Label(_)) => "label",
            Self::List(_) => "list",
            Self::Dict(_) => "dictionary",
        }
    }

    fn concatenate(self, right: Self) -> Result<Self, AttrCandidateError> {
        let left_shape = self.shape();
        let right_shape = right.shape();
        match (self, right) {
            (Self::Scalar(left), Self::Scalar(right)) => match (left, right) {
                (AttrCandidateAtom::String(mut left), AttrCandidateAtom::String(right)) => {
                    left.push_str(&right);
                    Ok(Self::Scalar(AttrCandidateAtom::String(left)))
                }
                _ => Err(AttrCandidateError {
                    left: left_shape,
                    right: right_shape,
                }),
            },
            (Self::List(mut left), Self::List(right)) => {
                left.extend(right);
                Ok(Self::List(left))
            }
            (Self::Dict(mut left), Self::Dict(right)) => {
                // Bazel's dictionary type keeps the last value for a repeated key.
                // Replacing in place retains the original map's observable key order.
                for (key, value) in right {
                    if let Some((_, existing)) =
                        left.iter_mut().find(|(existing, _)| *existing == key)
                    {
                        *existing = value;
                    } else {
                        left.push((key, value));
                    }
                }
                Ok(Self::Dict(left))
            }
            _ => Err(AttrCandidateError {
                left: left_shape,
                right: right_shape,
            }),
        }
    }
}

#[derive(Clone)]
struct AttrCandidate<'a> {
    value: AttrCandidateValue<'a>,
    /// One entry for each selector key set encountered on the path. This is
    /// request-local bookkeeping: equal key sets must select the same condition,
    /// while distinct key sets form a cross product.
    bindings: Vec<SelectorBinding<'a>>,
}

#[derive(Clone)]
struct SelectorBinding<'a> {
    selector: SelectorKeySet<'a>,
    selected: Option<&'a CanonicalLabel>,
}

#[derive(Clone, Copy)]
struct SelectorKeySet<'a> {
    branches: &'a [(CanonicalLabel, Arc<CoercedAttributeValue>)],
}

impl PartialEq for SelectorKeySet<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.branches.len() == other.branches.len()
            && self
                .branches
                .iter()
                .all(|(left, _)| other.branches.iter().any(|(right, _)| left == right))
    }
}

impl Eq for SelectorKeySet<'_> {}

fn expand_attr_candidates(
    value: &CoercedAttributeValue,
) -> Result<Vec<AttrCandidate<'_>>, AttrCandidateError> {
    Ok(match value {
        CoercedAttributeValue::None => Vec::new(),
        CoercedAttributeValue::Label(label) | CoercedAttributeValue::Output(label) => {
            scalar_label_candidate(label)
        }
        CoercedAttributeValue::String(value) => scalar_string_candidate(value.clone()),
        CoercedAttributeValue::Boolean(value) => {
            scalar_string_candidate(CompactString::new(if *value { "1" } else { "0" }))
        }
        CoercedAttributeValue::Integer(value) => {
            scalar_string_candidate(CompactString::new(value.to_string()))
        }
        CoercedAttributeValue::LabelList(values) | CoercedAttributeValue::OutputList(values) => {
            list_candidate(values.iter().map(AttrCandidateAtom::Label))
        }
        CoercedAttributeValue::StringList(values) => {
            list_candidate(values.iter().cloned().map(AttrCandidateAtom::String))
        }
        CoercedAttributeValue::StringListDict(values) => {
            dict_candidate(values.iter().map(|(key, values)| {
                (
                    AttrCandidateAtom::String(key.clone()),
                    AttrCandidateValue::List(
                        values
                            .iter()
                            .cloned()
                            .map(AttrCandidateAtom::String)
                            .collect(),
                    ),
                )
            }))
        }
        CoercedAttributeValue::StringDict(values) => {
            dict_candidate(values.iter().map(|(key, value)| {
                (
                    AttrCandidateAtom::String(key.clone()),
                    AttrCandidateValue::Scalar(AttrCandidateAtom::String(value.clone())),
                )
            }))
        }
        CoercedAttributeValue::StringKeyedLabelDict(values) => {
            dict_candidate(values.iter().map(|(key, value)| {
                (
                    AttrCandidateAtom::String(key.clone()),
                    AttrCandidateValue::Scalar(AttrCandidateAtom::Label(value)),
                )
            }))
        }
        CoercedAttributeValue::LabelKeyedStringDict(values) => {
            dict_candidate(values.iter().map(|(key, value)| {
                (
                    AttrCandidateAtom::Label(key),
                    AttrCandidateValue::Scalar(AttrCandidateAtom::String(value.clone())),
                )
            }))
        }
        CoercedAttributeValue::LabelListDict(values) => {
            dict_candidate(values.iter().map(|(key, values)| {
                (
                    AttrCandidateAtom::String(key.clone()),
                    AttrCandidateValue::List(values.iter().map(AttrCandidateAtom::Label).collect()),
                )
            }))
        }
        CoercedAttributeValue::Selector { branches, default } => {
            let selector = SelectorKeySet { branches };
            let mut candidates = Vec::new();
            for (condition, branch) in branches.iter() {
                candidates.extend(
                    expand_attr_candidates(branch)?
                        .into_iter()
                        .filter_map(|candidate| {
                            bind_selector(candidate, selector, Some(condition))
                        }),
                );
            }
            if let Some(default) = default {
                candidates.extend(
                    expand_attr_candidates(default)?
                        .into_iter()
                        .filter_map(|candidate| bind_selector(candidate, selector, None)),
                );
            }
            candidates
        }
        CoercedAttributeValue::Concatenation(left, right) => combine_attr_candidates(
            expand_attr_candidates(left)?,
            expand_attr_candidates(right)?,
        )?,
    })
}

fn bind_selector<'a>(
    mut candidate: AttrCandidate<'a>,
    selector: SelectorKeySet<'a>,
    selected: Option<&'a CanonicalLabel>,
) -> Option<AttrCandidate<'a>> {
    if let Some(existing) = candidate
        .bindings
        .iter()
        .find(|binding| binding.selector == selector)
    {
        return (existing.selected == selected).then_some(candidate);
    }
    candidate
        .bindings
        .push(SelectorBinding { selector, selected });
    Some(candidate)
}

fn scalar_string_candidate(value: CompactString) -> Vec<AttrCandidate<'static>> {
    vec![AttrCandidate {
        value: AttrCandidateValue::Scalar(AttrCandidateAtom::String(value)),
        bindings: Vec::new(),
    }]
}

fn scalar_label_candidate(label: &CanonicalLabel) -> Vec<AttrCandidate<'_>> {
    vec![AttrCandidate {
        value: AttrCandidateValue::Scalar(AttrCandidateAtom::Label(label)),
        bindings: Vec::new(),
    }]
}

fn list_candidate<'a>(
    values: impl IntoIterator<Item = AttrCandidateAtom<'a>>,
) -> Vec<AttrCandidate<'a>> {
    vec![AttrCandidate {
        value: AttrCandidateValue::List(values.into_iter().collect()),
        bindings: Vec::new(),
    }]
}

fn dict_candidate<'a>(
    entries: impl IntoIterator<Item = (AttrCandidateAtom<'a>, AttrCandidateValue<'a>)>,
) -> Vec<AttrCandidate<'a>> {
    vec![AttrCandidate {
        value: AttrCandidateValue::Dict(entries.into_iter().collect()),
        bindings: Vec::new(),
    }]
}

fn combine_attr_candidates<'a>(
    left: Vec<AttrCandidate<'a>>,
    right: Vec<AttrCandidate<'a>>,
) -> Result<Vec<AttrCandidate<'a>>, AttrCandidateError> {
    let mut combined = Vec::with_capacity(left.len().saturating_mul(right.len()));
    for left_candidate in left {
        for right_candidate in &right {
            let mut bindings = left_candidate.bindings.clone();
            let mut compatible = true;
            for right_binding in &right_candidate.bindings {
                if let Some(left_binding) = bindings
                    .iter()
                    .find(|left_binding| left_binding.selector == right_binding.selector)
                {
                    if left_binding.selected != right_binding.selected {
                        compatible = false;
                        break;
                    }
                } else {
                    bindings.push(right_binding.clone());
                }
            }
            if compatible {
                combined.push(AttrCandidate {
                    value: left_candidate
                        .value
                        .clone()
                        .concatenate(right_candidate.value.clone())?,
                    bindings,
                });
            }
        }
    }
    Ok(combined)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use compact_str::CompactString;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;

    use super::AttributeFlags;
    use super::AttributeKind;
    use super::AttributeProvenance;
    use super::AttributeQueryValue;
    use super::AttributeSchema;
    use super::AttributeValue;
    use super::CoercedAttributeValue;
    use super::NativeAttributePolicy;
    use super::NativeRuleClass;

    #[test]
    fn attribute_flags_are_one_word_and_project_through_schema() {
        assert_eq!(std::mem::size_of::<AttributeFlags>(), 4);
        let mut flags = AttributeFlags::default();
        assert!(flags.is_empty());
        flags.insert_direct_compile_time_input();
        flags.insert_direct_compile_time_input();
        assert!(flags.direct_compile_time_input());
        assert!(
            AttributeSchema::new("deps", AttributeKind::LabelList, false, true, None)
                .with_flags(flags)
                .direct_compile_time_input()
        );
    }

    #[test]
    fn native_rule_schemas_have_exact_unique_bazel_slots() {
        let classes = [
            (NativeRuleClass::Filegroup, 23),
            (NativeRuleClass::Alias, 17),
            (NativeRuleClass::ConfigSetting, 21),
            (NativeRuleClass::TestSuite, 21),
            (NativeRuleClass::ConstraintSetting, 16),
            (NativeRuleClass::ConstraintValue, 15),
            (NativeRuleClass::Platform, 23),
            (NativeRuleClass::ToolchainType, 17),
            (NativeRuleClass::Toolchain, 21),
        ];
        for (class, expected) in classes {
            let schema = class.schema();
            assert_eq!(schema.len(), expected, "{class:?}");
            for (index, attribute) in schema.iter().enumerate() {
                assert!(
                    schema[..index]
                        .iter()
                        .all(|prior| prior.query_name() != attribute.query_name()),
                    "duplicate {} in {class:?}",
                    attribute.query_name()
                );
            }
            for generator in ["generator_name", "generator_function", "generator_location"] {
                assert_eq!(
                    class.slot(generator).unwrap().1.policy(),
                    NativeAttributePolicy::Callable,
                    "{class:?}.{generator}"
                );
            }
        }
        assert_eq!(
            NativeRuleClass::ConfigSetting
                .slot("licenses")
                .unwrap()
                .1
                .policy(),
            NativeAttributePolicy::Forced
        );
    }

    #[test]
    fn external_rebinding_reaches_selector_keys_values_and_foreign_labels() {
        let foreign = label("@@bazel_tools//tools:test");
        let value = CoercedAttributeValue::Selector {
            branches: Arc::from([(
                label("@@//pkg:condition"),
                Arc::new(CoercedAttributeValue::LabelKeyedStringDict(Arc::from([
                    (label("@@//pkg:key"), "local".into()),
                    (foreign.clone(), "foreign".into()),
                ]))),
            )]),
            default: Some(Arc::new(CoercedAttributeValue::LabelListDict(Arc::from([
                (
                    "values".into(),
                    Arc::from([label("@@//pkg:value"), foreign.clone()]),
                ),
            ])))),
        };
        let repo = CanonicalRepoName::new("dep+").unwrap();
        let rebound = value.rebind_provisional_root_labels(&repo).unwrap();
        let CoercedAttributeValue::Selector { branches, default } = rebound else {
            panic!("selector shape changed")
        };
        assert_eq!(branches[0].0.to_string(), "@@dep+//pkg:condition");
        let CoercedAttributeValue::LabelKeyedStringDict(entries) = branches[0].1.as_ref() else {
            panic!("branch dictionary shape changed")
        };
        assert_eq!(entries[0].0.to_string(), "@@dep+//pkg:key");
        assert_eq!(entries[1].0, foreign);
        let Some(CoercedAttributeValue::LabelListDict(entries)) = default.as_deref() else {
            panic!("default dictionary shape changed")
        };
        assert_eq!(entries[0].1[0].to_string(), "@@dep+//pkg:value");
        assert_eq!(entries[0].1[1].to_string(), "@@bazel_tools//tools:test");
    }

    fn label(value: &str) -> CanonicalLabel {
        CanonicalLabel::parse(value).unwrap()
    }

    fn render_bazel_label(label: &CanonicalLabel) -> CompactString {
        if label.package().repo().is_root() {
            CompactString::new(format!(
                "//{}:{}",
                label.package().package(),
                label.target()
            ))
        } else {
            CompactString::new(label.to_string())
        }
    }

    fn string_selector(branches: &[(&str, &str)]) -> CoercedAttributeValue {
        CoercedAttributeValue::Selector {
            branches: branches
                .iter()
                .map(|(condition, value)| {
                    (
                        label(condition),
                        Arc::new(CoercedAttributeValue::String(CompactString::new(value))),
                    )
                })
                .collect::<Vec<_>>()
                .into(),
            default: None,
        }
    }

    fn string_value(value: &str) -> Arc<CoercedAttributeValue> {
        Arc::new(CoercedAttributeValue::String(CompactString::new(value)))
    }

    #[test]
    fn query_value_keeps_the_loading_value_structure_order_and_provenance() {
        let schema = AttributeSchema::new("chosen", AttributeKind::LabelList, false, true, None);
        let before = CanonicalLabel::parse("@@//pkg:before").unwrap();
        let selected = CanonicalLabel::parse("@@//pkg:selected").unwrap();
        let fallback = CanonicalLabel::parse("@@//pkg:fallback").unwrap();
        let retained = Arc::new(CoercedAttributeValue::Concatenation(
            Arc::new(CoercedAttributeValue::LabelList(Arc::from(
                [before.clone()],
            ))),
            Arc::new(CoercedAttributeValue::Selector {
                branches: Arc::from([(
                    CanonicalLabel::parse("@@//conditions:enabled").unwrap(),
                    Arc::new(CoercedAttributeValue::LabelList(Arc::from([
                        selected.clone()
                    ]))),
                )]),
                default: Some(Arc::new(CoercedAttributeValue::LabelList(Arc::from([
                    fallback.clone(),
                ])))),
            }),
        ));
        let value = AttributeValue {
            declaration_name: "chosen".into(),
            provenance: AttributeProvenance::Explicit,
            value: retained.clone(),
        };

        let query_value = value.query_value(&schema);

        assert_eq!(query_value.kind, AttributeKind::LabelList);
        assert_eq!(query_value.provenance, AttributeProvenance::Explicit);
        assert_eq!(&query_value.value, retained.as_ref());
        let mut labels = Vec::new();
        query_value.value.labels(&mut labels);
        assert_eq!(labels, [before, selected, fallback]);
    }

    #[test]
    fn attr_candidates_preserve_equal_selector_key_correlation() {
        let left = string_selector(&[
            ("@@//conditions:enabled", "left-enabled"),
            ("@@//conditions:disabled", "left-disabled"),
        ]);
        let right = string_selector(&[
            ("@@//conditions:disabled", "-right-disabled"),
            ("@@//conditions:enabled", "-right-enabled"),
        ]);

        let candidates = CoercedAttributeValue::Concatenation(Arc::new(left), Arc::new(right))
            .attr_visible_candidates(render_bazel_label)
            .unwrap();

        assert_eq!(
            candidates,
            ["left-enabled-right-enabled", "left-disabled-right-disabled"]
        );
    }

    #[test]
    fn attr_candidates_cross_product_distinct_selector_key_sets() {
        let left = string_selector(&[
            ("@@//conditions:enabled", "left-enabled"),
            ("@@//conditions:disabled", "left-disabled"),
        ]);
        let right = string_selector(&[
            ("@@//conditions:linux", "-right-linux"),
            ("@@//conditions:mac", "-right-mac"),
        ]);

        let candidates = CoercedAttributeValue::Concatenation(Arc::new(left), Arc::new(right))
            .attr_visible_candidates(render_bazel_label)
            .unwrap();

        assert_eq!(
            candidates,
            [
                "left-enabled-right-linux",
                "left-enabled-right-mac",
                "left-disabled-right-linux",
                "left-disabled-right-mac",
            ]
        );
    }

    #[test]
    fn attr_candidates_correlate_explicit_keys_even_when_only_one_selector_has_default() {
        let condition = label("@@//conditions:a");
        let left = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition.clone(), string_value("x"))]),
            default: Some(string_value("y")),
        };
        let right = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition, string_value("z"))]),
            default: None,
        };

        let candidates = CoercedAttributeValue::Concatenation(Arc::new(left), Arc::new(right))
            .attr_visible_candidates(render_bazel_label)
            .unwrap();

        assert_eq!(candidates, ["xz"]);
    }

    #[test]
    fn nested_selectors_reject_conflicting_equal_key_and_default_bindings() {
        let condition = label("@@//conditions:a");
        let inner_for_explicit = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition.clone(), string_value("explicit-explicit"))]),
            default: Some(string_value("explicit-default-conflict")),
        };
        let explicit_outer = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition.clone(), Arc::new(inner_for_explicit))]),
            default: None,
        };

        assert_eq!(
            explicit_outer
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["explicit-explicit"]
        );

        let inner_for_default = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition.clone(), string_value("default-explicit-conflict"))]),
            default: Some(string_value("default-default")),
        };
        let default_outer = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition, Arc::new(CoercedAttributeValue::None))]),
            default: Some(Arc::new(inner_for_default)),
        };

        assert_eq!(
            default_outer
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["default-default"]
        );
    }

    #[test]
    fn invalid_concatenation_returns_a_typed_error() {
        let invalid = CoercedAttributeValue::Concatenation(
            Arc::new(CoercedAttributeValue::Label(label("@@//pkg:left"))),
            Arc::new(CoercedAttributeValue::Label(label("@@//pkg:right"))),
        );

        let error = invalid
            .attr_visible_candidates(render_bazel_label)
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            "cannot concatenate attribute candidate types label and label"
        );
    }

    #[test]
    fn attr_candidates_render_ordered_lists_and_dictionaries_with_canonical_labels() {
        let scalar_label = CoercedAttributeValue::Label(label("@@//pkg:scalar"));
        let scalar_string = CoercedAttributeValue::String(CompactString::new("literal"));
        let labels = CoercedAttributeValue::LabelList(Arc::from([
            label("@@//pkg:first"),
            label("@@//pkg:second"),
            label("@@//pkg:second"),
        ]));
        let keyed_labels = CoercedAttributeValue::StringKeyedLabelDict(Arc::from([
            (CompactString::new("z"), label("@@//pkg:last")),
            (CompactString::new("a"), label("@@//pkg:first")),
        ]));
        let label_keyed = CoercedAttributeValue::LabelKeyedStringDict(Arc::from([
            (label("@@//pkg:z"), CompactString::new("last")),
            (label("@@//pkg:a"), CompactString::new("first")),
        ]));
        let label_lists = CoercedAttributeValue::LabelListDict(Arc::from([(
            CompactString::new("ordered"),
            Arc::from([label("@@//pkg:one"), label("@@//pkg:one")]),
        )]));
        let string_lists = CoercedAttributeValue::StringListDict(Arc::from([(
            CompactString::new("ordered"),
            Arc::from([CompactString::new("one"), CompactString::new("one")]),
        )]));

        assert_eq!(
            scalar_label
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["//pkg:scalar"]
        );
        assert_eq!(
            scalar_string
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["literal"]
        );
        assert_eq!(
            labels.attr_visible_candidates(render_bazel_label).unwrap(),
            ["[//pkg:first, //pkg:second, //pkg:second]"]
        );
        assert_eq!(
            keyed_labels
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["{z=//pkg:last, a=//pkg:first}"]
        );
        assert_eq!(
            label_keyed
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["{//pkg:z=last, //pkg:a=first}"]
        );
        assert_eq!(
            label_lists
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["{ordered=[//pkg:one, //pkg:one]}"]
        );
        assert_eq!(
            string_lists
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["{ordered=[one, one]}"]
        );
    }

    #[test]
    fn attr_candidates_skip_null_optional_values_and_are_available_from_query_values() {
        let value = AttributeQueryValue {
            kind: AttributeKind::Label,
            provenance: AttributeProvenance::Default,
            value: CoercedAttributeValue::None,
        };

        assert!(
            value
                .attr_visible_candidates(render_bazel_label)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn configured_helpers_collect_nested_keys_and_concatenate_typed_values() {
        let first = label("@@//conditions:first");
        let second = label("@@//conditions:second");
        let expression = CoercedAttributeValue::Concatenation(
            Arc::new(CoercedAttributeValue::Selector {
                branches: Arc::from([(
                    first.clone(),
                    Arc::new(CoercedAttributeValue::StringList(Arc::from([
                        CompactString::new("left"),
                    ]))),
                )]),
                default: Some(Arc::new(CoercedAttributeValue::Selector {
                    branches: Arc::from([(
                        second.clone(),
                        Arc::new(CoercedAttributeValue::StringList(Arc::from([]))),
                    )]),
                    default: None,
                })),
            }),
            Arc::new(CoercedAttributeValue::StringList(Arc::from([
                CompactString::new("right"),
            ]))),
        );
        assert_eq!(expression.selector_key_labels(), [first, second]);
        assert_eq!(
            CoercedAttributeValue::StringList(Arc::from([CompactString::new("left")]))
                .concatenate_resolved(&CoercedAttributeValue::StringList(Arc::from([
                    CompactString::new("right"),
                ])))
                .unwrap(),
            CoercedAttributeValue::StringList(Arc::from([
                CompactString::new("left"),
                CompactString::new("right"),
            ]))
        );

        let merged = CoercedAttributeValue::StringDict(Arc::from([
            (CompactString::new("first"), CompactString::new("old")),
            (CompactString::new("kept"), CompactString::new("value")),
        ]))
        .concatenate_resolved(&CoercedAttributeValue::StringDict(Arc::from([
            (CompactString::new("first"), CompactString::new("new")),
            (CompactString::new("last"), CompactString::new("value")),
        ])))
        .unwrap();
        assert_eq!(
            merged,
            CoercedAttributeValue::StringDict(Arc::from([
                (CompactString::new("first"), CompactString::new("new")),
                (CompactString::new("kept"), CompactString::new("value")),
                (CompactString::new("last"), CompactString::new("value")),
            ]))
        );
    }
}
