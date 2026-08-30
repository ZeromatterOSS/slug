/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::OnceCell;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::ProviderIdentity;
use slug_configuration_v2::ConfigurationField;
use slug_configuration_v2::ConfigurationFieldIdentity;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use starlark::any::ProvidesStaticType;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::values::Freeze;
use starlark::values::FreezeError;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenValue;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::list::ListRef;
use starlark::values::starlark_value;
use starlark::values::tuple::TupleRef;
use starlark_map::StarlarkHasher;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::attrs::AllowSingleFile;
use crate::attrs::AllowedAttributeValues;
use crate::attrs::AttributeDependencyConfiguration;
use crate::attrs::AttributeKind;
use crate::attrs::AttributeSchema;
use crate::attrs::CoercedAttributeValue;
use crate::bzl_module::BzlModuleIdentity;
use crate::package::ToolchainTypeRequirement;
use crate::package::subrule_attribute_from_value;
use crate::package::subrule_toolchain_requirements;
use crate::provider::BzlEvaluationContext;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct ConfigurationFieldValue(ConfigurationFieldIdentity);

impl ConfigurationFieldValue {
    pub(crate) fn identity(&self) -> &ConfigurationFieldIdentity {
        &self.0
    }
}

impl fmt::Display for ConfigurationFieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<late-bound default>")
    }
}

starlark::starlark_simple_value!(ConfigurationFieldValue);

#[starlark_value(type = "late_bound_default")]
impl<'v> StarlarkValue<'v> for ConfigurationFieldValue {
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.0.hash(hasher);
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.0 == other.0))
    }
}

fn tools_repository(source: &BzlModuleIdentity) -> CanonicalRepoName {
    let apparent = ApparentRepoName::new("bazel_tools")
        .expect("the pinned tools apparent repository is valid");
    source
        .repository_mapping
        .iter()
        .find_map(|(candidate, canonical)| (candidate == &apparent).then(|| canonical.clone()))
        .unwrap_or_else(|| {
            CanonicalRepoName::new("bazel_tools")
                .expect("the pinned tools canonical repository is valid")
        })
}

pub(crate) fn configuration_field_global<'v>(
    fragment: &str,
    name: &str,
    eval: &Evaluator<'v, '_, '_>,
) -> anyhow::Result<ConfigurationFieldValue> {
    if fragment != "cpp" {
        anyhow::bail!("invalid configuration fragment name '{fragment}'");
    }
    let Some(field) = ConfigurationField::from_starlark_names(fragment, name) else {
        anyhow::bail!(
            "invalid configuration field name '{name}' on fragment '{}'",
            fragment
        );
    };
    let source = BzlEvaluationContext::from_evaluator(eval)?.source_identity_for_call(eval)?;
    Ok(ConfigurationFieldValue(ConfigurationFieldIdentity::new(
        field,
        tools_repository(source),
    )))
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum SubruleAttributeDefault {
    Literal(CoercedAttributeValue),
    ConfigurationField(ConfigurationFieldIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct SubruleAttribute {
    pub(crate) user_name: CompactString,
    pub(crate) kind: AttributeKind,
    pub(crate) configurable: bool,
    pub(crate) default: SubruleAttributeDefault,
    pub(crate) allow_files: bool,
    pub(crate) allow_single_file: Option<AllowSingleFile>,
    pub(crate) allowed_values: AllowedAttributeValues,
    pub(crate) executable: bool,
    pub(crate) exec_configuration: bool,
    pub(crate) required_providers: Arc<[Arc<[ProviderIdentity]>]>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub(crate) struct SubruleIdentity {
    pub(crate) defining_label: CanonicalLabel,
    pub(crate) exported_name: CompactString,
}

impl SubruleIdentity {
    fn hidden_label(&self) -> CompactString {
        if self.defining_label.package().repo().is_root() {
            CompactString::new(format!(
                "//{}:{}",
                self.defining_label.package().package(),
                self.defining_label.target()
            ))
        } else {
            CompactString::new(self.defining_label.to_string())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct LiftedSubruleAttribute {
    pub(crate) owner: Arc<SubruleIdentity>,
    pub(crate) user_name: CompactString,
    pub(crate) rule_name: CompactString,
    pub(crate) kind: AttributeKind,
    pub(crate) configurable: bool,
    pub(crate) default: SubruleAttributeDefault,
    pub(crate) allow_files: bool,
    pub(crate) allow_single_file: Option<AllowSingleFile>,
    pub(crate) allowed_values: AllowedAttributeValues,
    pub(crate) executable: bool,
    pub(crate) exec_configuration: bool,
    pub(crate) required_providers: Arc<[Arc<[ProviderIdentity]>]>,
}

impl LiftedSubruleAttribute {
    fn new(owner: &Arc<SubruleIdentity>, attribute: &SubruleAttribute) -> Self {
        let prefix = match attribute.default {
            SubruleAttributeDefault::Literal(_) => '$',
            SubruleAttributeDefault::ConfigurationField(_) => ':',
        };
        Self {
            owner: owner.clone(),
            user_name: attribute.user_name.clone(),
            rule_name: CompactString::new(format!(
                "{prefix}{}%{}%{}",
                owner.hidden_label(),
                owner.exported_name,
                attribute.user_name
            )),
            kind: attribute.kind,
            configurable: attribute.configurable,
            default: attribute.default.clone(),
            allow_files: attribute.allow_files,
            allow_single_file: attribute.allow_single_file.clone(),
            allowed_values: attribute.allowed_values.clone(),
            executable: attribute.executable,
            exec_configuration: attribute.exec_configuration,
            required_providers: attribute.required_providers.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct SubruleAttributeSpan {
    pub(crate) owner: Arc<SubruleIdentity>,
    pub(crate) start: u32,
    pub(crate) len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct SubruleSemanticDefinition {
    pub(crate) identity: Arc<SubruleIdentity>,
    pub(crate) direct_subrules: Arc<[Arc<SubruleIdentity>]>,
    pub(crate) fragments: Arc<SmallSet<CompactString>>,
    pub(crate) toolchains: Arc<[ToolchainTypeRequirement]>,
}

/// Compact package/rule-owned projection. Definitions are sorted for
/// set-semantic equality; lifted rows/spans preserve Bazel first-encounter and
/// descriptor order for query presentation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Allocative)]
pub(crate) struct AttachedSubrules {
    pub(crate) direct: Arc<[Arc<SubruleIdentity>]>,
    pub(crate) definitions: Arc<[SubruleSemanticDefinition]>,
    pub(crate) lifted_attributes: Arc<[LiftedSubruleAttribute]>,
    pub(crate) spans: Arc<[SubruleAttributeSpan]>,
}

/// Sparse rule-owned row for an ordinary attribute whose default is produced
/// by `configuration_field`. The ordinary schema stays compact; this shared
/// slice is the sole future configured-resolution input.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct LateBoundRuleAttribute {
    pub(crate) schema_index: u32,
    pub(crate) identity: ConfigurationFieldIdentity,
    pub(crate) required_providers: Arc<[Arc<[ProviderIdentity]>]>,
}

#[derive(Clone, Copy, Debug)]
pub enum ConfiguredDependencyDefault<'a> {
    Literal(&'a CoercedAttributeValue),
    ConfigurationField(&'a ConfigurationFieldIdentity),
}

/// Borrowed view over an ordinary late-bound or lifted hidden dependency.
#[derive(Clone, Copy, Debug)]
pub struct ConfiguredDependencyAttribute<'a> {
    name: &'a str,
    user_name: Option<&'a str>,
    kind: AttributeKind,
    default: ConfiguredDependencyDefault<'a>,
    allow_files: bool,
    allow_single_file: Option<&'a AllowSingleFile>,
    executable: bool,
    exec_configuration: bool,
    required_providers: &'a [Arc<[ProviderIdentity]>],
}

impl<'a> ConfiguredDependencyAttribute<'a> {
    pub(crate) fn from_ordinary(
        attribute: &'a LateBoundRuleAttribute,
        schema: &'a AttributeSchema,
    ) -> Self {
        Self {
            name: schema.declaration_name(),
            user_name: None,
            kind: schema.kind(),
            default: ConfiguredDependencyDefault::ConfigurationField(&attribute.identity),
            allow_files: schema.allow_files(),
            allow_single_file: schema.allow_single_file(),
            executable: schema.executable(),
            exec_configuration: matches!(
                schema.dependency_configuration(),
                AttributeDependencyConfiguration::Exec
            ),
            required_providers: &attribute.required_providers,
        }
    }

    pub(crate) fn from_hidden(attribute: &'a LiftedSubruleAttribute) -> Self {
        let default = match &attribute.default {
            SubruleAttributeDefault::Literal(value) => ConfiguredDependencyDefault::Literal(value),
            SubruleAttributeDefault::ConfigurationField(identity) => {
                ConfiguredDependencyDefault::ConfigurationField(identity)
            }
        };
        Self {
            name: attribute.rule_name.as_str(),
            user_name: Some(attribute.user_name.as_str()),
            kind: attribute.kind,
            default,
            allow_files: attribute.allow_files,
            allow_single_file: attribute.allow_single_file.as_ref(),
            executable: attribute.executable,
            exec_configuration: attribute.exec_configuration,
            required_providers: &attribute.required_providers,
        }
    }

    pub fn name(self) -> &'a str {
        self.name
    }

    pub fn user_name(self) -> Option<&'a str> {
        self.user_name
    }

    pub fn kind(self) -> AttributeKind {
        self.kind
    }

    pub fn default(self) -> ConfiguredDependencyDefault<'a> {
        self.default
    }

    pub fn allow_files(self) -> bool {
        self.allow_files
    }

    pub fn allow_single_file(self) -> Option<&'a AllowSingleFile> {
        self.allow_single_file
    }

    pub fn executable(self) -> bool {
        self.executable
    }

    pub fn exec_configuration(self) -> bool {
        self.exec_configuration
    }

    pub fn required_providers(self) -> &'a [Arc<[ProviderIdentity]>] {
        self.required_providers
    }

    pub fn is_hidden(self) -> bool {
        self.user_name.is_some()
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct DeferredSubruleRuleImplementation {
    #[allocative(skip)]
    implementation: FrozenValue,
    first_subrule: Option<CompactString>,
    first_late_bound_attribute: Option<CompactString>,
}

impl fmt::Display for DeferredSubruleRuleImplementation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<function with deferred subrules>")
    }
}

starlark::starlark_simple_value!(DeferredSubruleRuleImplementation);

#[starlark_value(type = "function")]
impl<'v> StarlarkValue<'v> for DeferredSubruleRuleImplementation {
    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _lifetime_only = self.implementation;
        let message = match &self.first_subrule {
            Some(subrule) => format!(
                "configured analysis of subrule '{subrule}' reached the deferred invocation boundary"
            ),
            None => format!(
                "configured analysis of rule attribute '{}' reached the deferred late-bound value materialization boundary",
                self.first_late_bound_attribute
                    .as_deref()
                    .expect("the wrapper retains one deferred semantic")
            ),
        };
        Err(starlark::Error::new_other(anyhow::anyhow!(message)))
    }
}

pub(crate) fn fail_closed_rule_implementation(
    freezer: &Freezer,
    implementation: FrozenValue,
    attached: &AttachedSubrules,
    first_late_bound_attribute: Option<CompactString>,
) -> FrozenValue {
    match (attached.definitions.first(), first_late_bound_attribute) {
        (None, None) => implementation,
        (definition, attribute) => freezer.alloc(DeferredSubruleRuleImplementation {
            implementation,
            first_subrule: definition.map(|definition| definition.identity.exported_name.clone()),
            first_late_bound_attribute: attribute,
        }),
    }
}

impl AttachedSubrules {
    pub(crate) fn definition_count(&self) -> usize {
        self.definitions.len()
    }

    pub(crate) fn hidden_attribute_names(&self) -> impl Iterator<Item = &str> {
        self.lifted_attributes
            .iter()
            .map(|attribute| attribute.rule_name.as_str())
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub(crate) struct SubruleDefinitionGen<V> {
    implementation: V,
    #[trace(unsafe_ignore)]
    transient_identity: Arc<()>,
    #[trace(unsafe_ignore)]
    definition_source: Arc<BzlModuleIdentity>,
    #[trace(unsafe_ignore)]
    attributes: Arc<[SubruleAttribute]>,
    #[trace(unsafe_ignore)]
    toolchains: Arc<[ToolchainTypeRequirement]>,
    #[trace(unsafe_ignore)]
    fragments: Arc<SmallSet<CompactString>>,
    direct_subrules: Vec<V>,
    #[allocative(skip)]
    #[trace(unsafe_ignore)]
    exported_identity: OnceCell<Arc<SubruleIdentity>>,
}

pub(crate) type SubruleDefinition<'v> = SubruleDefinitionGen<Value<'v>>;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct FrozenSubruleDefinition {
    #[allocative(skip)]
    implementation: FrozenValue,
    identity: Arc<SubruleIdentity>,
    attributes: Arc<[SubruleAttribute]>,
    toolchains: Arc<[ToolchainTypeRequirement]>,
    fragments: Arc<SmallSet<CompactString>>,
    direct_subrules: Arc<[FrozenValue]>,
}

starlark::starlark_complex_values!(SubruleDefinition);

impl<V> fmt::Display for SubruleDefinitionGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.exported_identity.get() {
            Some(identity) => write!(f, "<subrule {}>", identity.exported_name),
            None => f.write_str("<subrule unexported subrule>"),
        }
    }
}

impl fmt::Display for FrozenSubruleDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<subrule {}>", self.identity.exported_name)
    }
}

impl<'v> Freeze for SubruleDefinition<'v> {
    type Frozen = FrozenSubruleDefinition;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let Some(identity) = self.exported_identity.into_inner() else {
            return Err(FreezeError::new(
                "the result of subrule() must be assigned to a top-level variable".to_owned(),
            ));
        };
        Ok(FrozenSubruleDefinition {
            implementation: self.implementation.freeze(freezer)?,
            identity,
            attributes: self.attributes,
            toolchains: self.toolchains,
            fragments: self.fragments,
            direct_subrules: self
                .direct_subrules
                .iter()
                .map(|value| value.freeze(freezer))
                .collect::<FreezeResult<Vec<_>>>()?
                .into(),
        })
    }
}

fn identity_from_value(value: Value<'_>) -> Option<Arc<SubruleIdentity>> {
    match SubruleDefinition::from_value(value)? {
        starlark::__macro_refs::Either::Left(subrule) => subrule.exported_identity.get().cloned(),
        starlark::__macro_refs::Either::Right(subrule) => Some(subrule.identity.clone()),
    }
}

fn subrule_sequence<'v>(value: Option<Value<'v>>) -> anyhow::Result<Vec<Value<'v>>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if let Some(values) = ListRef::from_value(value) {
        return Ok(values.iter().collect());
    }
    if let Some(values) = TupleRef::from_value(value) {
        return Ok(values.iter().collect());
    }
    anyhow::bail!("subrules must be a sequence of subrule values")
}

fn direct_identities(values: &[Value<'_>]) -> anyhow::Result<Arc<[Arc<SubruleIdentity>]>> {
    let mut identities = values
        .iter()
        .map(|value| {
            identity_from_value(*value)
                .ok_or_else(|| anyhow::anyhow!("subrules entries must be exported subrules"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    identities.sort();
    identities.dedup();
    Ok(identities.into())
}

fn visit_attached<'v>(
    value: Value<'v>,
    seen: &mut SmallSet<Arc<SubruleIdentity>>,
    callables: &mut Vec<(Arc<SubruleIdentity>, Value<'v>)>,
    definitions: &mut Vec<SubruleSemanticDefinition>,
    lifted: &mut Vec<LiftedSubruleAttribute>,
    spans: &mut Vec<SubruleAttributeSpan>,
) -> anyhow::Result<()> {
    let identity = identity_from_value(value)
        .ok_or_else(|| anyhow::anyhow!("subrules entries must be exported subrules"))?;
    if !seen.insert(identity.clone()) {
        return Ok(());
    }
    callables.push((identity.clone(), value));
    match SubruleDefinition::from_value(value).expect("identity proved the value type") {
        starlark::__macro_refs::Either::Left(subrule) => {
            let start = u32::try_from(lifted.len())?;
            lifted.extend(
                subrule
                    .attributes
                    .iter()
                    .map(|attribute| LiftedSubruleAttribute::new(&identity, attribute)),
            );
            spans.push(SubruleAttributeSpan {
                owner: identity.clone(),
                start,
                len: u32::try_from(subrule.attributes.len())?,
            });
            let direct = direct_identities(&subrule.direct_subrules)?;
            definitions.push(SubruleSemanticDefinition {
                identity,
                direct_subrules: direct,
                fragments: subrule.fragments.clone(),
                toolchains: subrule.toolchains.clone(),
            });
            for child in subrule.direct_subrules.iter() {
                visit_attached(*child, seen, callables, definitions, lifted, spans)?;
            }
        }
        starlark::__macro_refs::Either::Right(subrule) => {
            let start = u32::try_from(lifted.len())?;
            lifted.extend(
                subrule
                    .attributes
                    .iter()
                    .map(|attribute| LiftedSubruleAttribute::new(&identity, attribute)),
            );
            spans.push(SubruleAttributeSpan {
                owner: identity.clone(),
                start,
                len: u32::try_from(subrule.attributes.len())?,
            });
            let children = subrule
                .direct_subrules
                .iter()
                .map(|value| value.to_value())
                .collect::<Vec<_>>();
            let direct = direct_identities(&children)?;
            definitions.push(SubruleSemanticDefinition {
                identity,
                direct_subrules: direct,
                fragments: subrule.fragments.clone(),
                toolchains: subrule.toolchains.clone(),
            });
            for child in children {
                visit_attached(child, seen, callables, definitions, lifted, spans)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn attached_subrules(
    value: Option<Value<'_>>,
) -> anyhow::Result<(AttachedSubrules, Vec<Value<'_>>)> {
    let direct = subrule_sequence(value)?;
    let direct_identities = direct_identities(&direct)?;
    let mut seen = SmallSet::new();
    let mut callables = Vec::new();
    let mut definitions = Vec::new();
    let mut lifted = Vec::new();
    let mut spans = Vec::new();
    for subrule in direct {
        visit_attached(
            subrule,
            &mut seen,
            &mut callables,
            &mut definitions,
            &mut lifted,
            &mut spans,
        )?;
    }
    definitions.sort_by(|left, right| left.identity.cmp(&right.identity));
    callables.sort_by(|(left, _), (right, _)| left.cmp(right));
    debug_assert!(
        definitions
            .iter()
            .zip(&callables)
            .all(|(definition, (identity, _))| definition.identity == *identity)
    );
    Ok((
        AttachedSubrules {
            direct: direct_identities,
            definitions: definitions.into(),
            lifted_attributes: lifted.into(),
            spans: spans.into(),
        },
        callables.into_iter().map(|(_, value)| value).collect(),
    ))
}

#[starlark_value(type = "subrule")]
impl<'v> StarlarkValue<'v> for SubruleDefinition<'v> {
    type Canonical = FrozenSubruleDefinition;

    fn export_as(
        &self,
        variable_name: &str,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<()> {
        if self.exported_identity.get().is_none() {
            let _ = self.exported_identity.set(Arc::new(SubruleIdentity {
                defining_label: self.definition_source.label.clone(),
                exported_name: variable_name.into(),
            }));
        }
        Ok(())
    }

    fn write_hash(&self, _hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        match self.exported_identity.get() {
            Some(identity) => {
                Ok(identity_from_value(other).is_some_and(|other| other == *identity))
            }
            None => Ok(
                SubruleDefinition::from_value(other).is_some_and(|other| match other {
                    starlark::__macro_refs::Either::Left(other) => {
                        Arc::ptr_eq(&self.transient_identity, &other.transient_identity)
                    }
                    starlark::__macro_refs::Either::Right(_) => false,
                }),
            ),
        }
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        if self.exported_identity.get().is_none() {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "Invalid subrule hasn't been exported by a bzl file"
            )));
        }
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "subrules may only be called from a rule implementation"
        )))
    }
}

#[starlark_value(type = "subrule")]
impl<'v> StarlarkValue<'v> for FrozenSubruleDefinition {
    type Canonical = Self;

    fn write_hash(&self, _hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(identity_from_value(other).is_some_and(|other| other == self.identity))
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _ = self.implementation;
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "configured analysis of subrule '{}' does not yet support hidden late-bound dependency resolution",
            self.identity.exported_name
        )))
    }
}

pub(crate) fn subrule_global<'v>(
    implementation: Value<'v>,
    attrs: Option<SmallMap<String, Value<'v>>>,
    toolchains: Option<Value<'v>>,
    fragments: Option<Value<'v>>,
    subrules: Option<Value<'v>>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<SubruleDefinition<'v>> {
    if implementation.parameters_spec().is_none() {
        anyhow::bail!("subrule implementation must be a Starlark function");
    }
    let mut attributes = Vec::new();
    for (name, value) in attrs.unwrap_or_default() {
        attributes.push(subrule_attribute_from_value(name, value)?);
    }
    let toolchains = subrule_toolchain_requirements(toolchains, eval)?;
    if toolchains.len() > 1 {
        anyhow::bail!("subrules may require at most 1 toolchain, got: {toolchains:?}");
    }
    let fragments = subrule_sequence(fragments)?
        .into_iter()
        .map(|value| {
            value
                .unpack_str()
                .map(CompactString::new)
                .ok_or_else(|| anyhow::anyhow!("fragments entries must be strings"))
        })
        .collect::<anyhow::Result<SmallSet<_>>>()?;
    let direct_subrules = subrule_sequence(subrules)?;
    for value in &direct_subrules {
        if identity_from_value(*value).is_none() {
            anyhow::bail!("subrules entries must be exported subrules");
        }
    }
    let context = BzlEvaluationContext::from_evaluator(eval)?;
    Ok(SubruleDefinitionGen {
        implementation,
        transient_identity: Arc::new(()),
        definition_source: Arc::new(context.source_identity_for_call(eval)?.clone()),
        attributes: attributes.into(),
        toolchains,
        fragments: Arc::new(fragments),
        direct_subrules,
        exported_identity: OnceCell::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;
    use std::path::PathBuf;

    use super::*;

    fn owner(label: &str, tools_repository: &str) -> BzlModuleIdentity {
        BzlModuleIdentity {
            label: CanonicalLabel::parse(label).unwrap(),
            workspace_path: PathBuf::from("/workspace/defs.bzl"),
            repository_mapping: Arc::from([(
                ApparentRepoName::new("bazel_tools").unwrap(),
                CanonicalRepoName::new(tools_repository).unwrap(),
            )]),
        }
    }

    fn field(source: &BzlModuleIdentity, name: &str) -> ConfigurationFieldIdentity {
        ConfigurationFieldIdentity::new(
            ConfigurationField::from_starlark_names("cpp", name).unwrap(),
            tools_repository(source),
        )
    }

    #[test]
    fn typed_field_identity_ignores_module_and_discriminates_field_and_tools_repository() {
        let first = owner("@@one+//pkg:first.bzl", "tools+1.0");
        let second = owner("@@two+//other:second.bzl", "tools+1.0");
        let remapped = owner("@@one+//pkg:first.bzl", "tools+2.0");

        assert_eq!(
            field(&first, "fdo_optimize"),
            field(&second, "fdo_optimize")
        );
        assert_ne!(field(&first, "fdo_optimize"), field(&first, "fdo_profile"));
        assert_ne!(
            field(&first, "fdo_optimize"),
            field(&remapped, "fdo_optimize")
        );
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn retained_rule_projection_stays_sparse_and_pointer_owned() {
        assert_eq!(size_of::<AttachedSubrules>(), 64);
        assert!(
            size_of::<ConfigurationFieldIdentity>() <= 64,
            "configuration field: {}",
            size_of::<ConfigurationFieldIdentity>()
        );
        assert!(
            size_of::<LateBoundRuleAttribute>() <= 96,
            "late-bound rule row: {}",
            size_of::<LateBoundRuleAttribute>()
        );
        assert!(
            size_of::<SubruleSemanticDefinition>() <= 128,
            "semantic definition: {}",
            size_of::<SubruleSemanticDefinition>()
        );
        assert!(
            size_of::<SubruleAttributeSpan>() <= 96,
            "attribute span: {}",
            size_of::<SubruleAttributeSpan>()
        );
    }
}
