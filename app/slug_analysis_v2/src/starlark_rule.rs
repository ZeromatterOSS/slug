/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_configuration_v2::CppFragmentProjection;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::BzlModuleIdentity;
use slug_loading_v2::CoercedAttributeValue;
use slug_loading_v2::LoadedPackage;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::SubruleIdentity;
use slug_loading_v2::analysis_fragments::CppFragmentValue;
use slug_loading_v2::analysis_fragments::RuleFragmentCollection;
use slug_loading_v2::package::resolve_rule_definition_label;
use slug_loading_v2::provider::StarlarkDefaultInfo;
use slug_loading_v2::provider::starlark_label;
use slug_loading_v2::subrule_invocation::AnalysisActions;
use slug_loading_v2::subrule_invocation::AnalysisArtifactValue;
use slug_loading_v2::subrule_invocation::AnalysisCallToken;
use slug_loading_v2::subrule_invocation::AnalysisEvaluationContext;
use slug_loading_v2::subrule_invocation::PreparedSubruleInvocation;
use starlark::PrintHandler;
use starlark::any::ProvidesStaticType;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::starlark_complex_value;
use starlark::values::Coerce;
use starlark::values::Freeze;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::AllocDict;
use starlark::values::list::ListRef;
use starlark::values::starlark_value;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::analysis_value::AnalysisValueLowerer;
use crate::analysis_value::AnalysisValueMaterializer;
use crate::build_setting;
use crate::configured_attribute::ResolvedRuleAttribute;
use crate::key::ConfiguredNodeKey;
use crate::key::ConfiguredTargetKey;
use crate::result::ConfiguredActionOwnerContext;
use crate::result::ConfiguredNodeResult;

/// Errors produced while synchronously evaluating a loaded rule after DICE has
/// prepared its inputs. The executable-rule case remains distinct so command
/// owners can classify its established terminal without inspecting text.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum LoadedRuleError {
    Message(String),
    ExecutableRuleMissingExecutable { rule_class: CompactString },
}

impl From<String> for LoadedRuleError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl fmt::Display for LoadedRuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => f.write_str(message),
            Self::ExecutableRuleMissingExecutable { rule_class } => write!(
                f,
                "The rule '{rule_class}' is executable. It needs to create an executable File and pass it as the 'executable' parameter to the DefaultInfo it returns."
            ),
        }
    }
}

#[derive(Debug, Clone, Trace, ProvidesStaticType, NoSerialize, Allocative)]
#[repr(C)]
struct AnalysisContextGen<V> {
    #[allocative(skip)]
    actions: Arc<Mutex<CtxActions>>,
    #[allocative(skip)]
    #[trace(unsafe_ignore)]
    token: AnalysisCallToken,
    retained_owner: AnalysisConfiguredTargetKey,
    target_label: slug_identity_v2::CanonicalLabel,
    package_path: String,
    #[trace(unsafe_ignore)]
    dependencies: Arc<[AnalysisDependency]>,
    resolved_attributes: Arc<[ResolvedRuleAttribute]>,
    build_setting_value: Option<V>,
    fragments: V,
    #[trace(unsafe_ignore)]
    toolchain: Option<PreparedAnalysisToolchains>,
}

unsafe impl<'v> Coerce<AnalysisContextGen<Value<'v>>> for AnalysisContextGen<FrozenValue> {}

starlark_complex_value!(AnalysisContext);

impl<'v> Freeze for AnalysisContext<'v> {
    type Frozen = FrozenAnalysisContext;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(AnalysisContextGen {
            actions: self.actions,
            token: self.token,
            retained_owner: self.retained_owner,
            target_label: self.target_label,
            package_path: self.package_path,
            dependencies: self.dependencies,
            resolved_attributes: self.resolved_attributes,
            build_setting_value: self
                .build_setting_value
                .map(|value| value.freeze(freezer))
                .transpose()?,
            fragments: self.fragments.freeze(freezer)?,
            toolchain: self.toolchain,
        })
    }
}

impl<V> fmt::Display for AnalysisContextGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<analysis ctx>")
    }
}

#[starlark_value(type = "analysis_ctx")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for AnalysisContextGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        self.token.require_active(attribute, "rule context").ok()?;
        match attribute {
            "label" => Some(slug_loading_v2::provider::alloc_starlark_label(
                heap,
                self.target_label.clone(),
            )),
            "actions" => Some(heap.alloc_simple(AnalysisActions::new(
                self.actions.clone(),
                self.package_path.clone(),
                self.retained_owner.clone(),
                self.token.clone(),
                "rule context",
            ))),
            "attr" => Some(heap.alloc_simple(AnalysisAttributes {
                token: self.token.clone(),
                dependencies: self.dependencies.clone(),
                attributes: self.resolved_attributes.clone(),
            })),
            "outputs" => Some(heap.alloc_simple(AnalysisOutputs {
                token: self.token.clone(),
                attributes: self.resolved_attributes.clone(),
                package_path: self.package_path.clone(),
                owner: self.retained_owner.clone(),
            })),
            "toolchains" => self.toolchain.clone().map(|toolchain| {
                heap.alloc_simple(AnalysisToolchains {
                    token: self.token.clone(),
                    toolchains: toolchain,
                })
            }),
            "build_setting_value" => self.build_setting_value.map(|value| value.to_value()),
            "fragments" => Some(self.fragments.to_value()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Allocative)]
pub(crate) struct PreparedDependency {
    pub(crate) key: ConfiguredNodeKey,
    pub(crate) providers: ProviderCollection,
    pub(crate) attribute: CompactString,
    pub(crate) target_shape: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedConfiguredAttribute {
    pub(crate) owner: Option<Arc<SubruleIdentity>>,
    pub(crate) user_name: Option<CompactString>,
    pub(crate) value: AnalysisValue,
}

#[derive(Debug, Clone, Allocative)]
pub(crate) struct PreparedToolchain {
    pub(crate) action_context: Arc<ConfiguredActionOwnerContext>,
}

#[derive(Debug, Clone, Allocative)]
struct PreparedAnalysisToolchainRow {
    requested: slug_identity_v2::CanonicalLabel,
    actual: slug_identity_v2::CanonicalLabel,
    #[allocative(skip)]
    info: Option<FrozenValue>,
}

#[derive(Debug, Clone, Allocative)]
struct PreparedAnalysisToolchains {
    definition_source: Arc<BzlModuleIdentity>,
    rows: Arc<[PreparedAnalysisToolchainRow]>,
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisAttributes {
    #[allocative(skip)]
    token: AnalysisCallToken,
    dependencies: Arc<[AnalysisDependency]>,
    attributes: Arc<[ResolvedRuleAttribute]>,
}

impl fmt::Display for AnalysisAttributes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<ctx.attr>")
    }
}

starlark::starlark_simple_value!(AnalysisAttributes);

#[starlark_value(type = "analysis_attrs")]
impl<'v> StarlarkValue<'v> for AnalysisAttributes {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        self.token
            .require_active(attribute, "rule context attributes")
            .ok()?;
        let value = self
            .attributes
            .iter()
            .find(|candidate| candidate.declaration_name == attribute)?;
        (!matches!(
            value.kind,
            AttributeKind::Output | AttributeKind::OutputList
        ))
        .then(|| {
            allocate_analysis_attribute(value, &self.dependencies, heap)
                .expect("resolved attribute shape matches its loading declaration")
        })
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisOutputs {
    #[allocative(skip)]
    token: AnalysisCallToken,
    attributes: Arc<[ResolvedRuleAttribute]>,
    package_path: String,
    owner: AnalysisConfiguredTargetKey,
}

impl fmt::Display for AnalysisOutputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<ctx.outputs>")
    }
}

starlark::starlark_simple_value!(AnalysisOutputs);

#[starlark_value(type = "analysis_outputs")]
impl<'v> StarlarkValue<'v> for AnalysisOutputs {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        self.token
            .require_active(attribute, "rule context outputs")
            .ok()?;
        let attribute = self
            .attributes
            .iter()
            .find(|candidate| candidate.declaration_name == attribute)?;
        match &attribute.value {
            CoercedAttributeValue::None if attribute.kind == AttributeKind::Output => {
                Some(Value::new_none())
            }
            CoercedAttributeValue::Output(label) if attribute.kind == AttributeKind::Output => {
                Some(heap.alloc_simple(predeclared_file(label, &self.package_path, &self.owner)))
            }
            CoercedAttributeValue::OutputList(labels)
                if attribute.kind == AttributeKind::OutputList =>
            {
                Some(
                    heap.alloc(
                        labels
                            .iter()
                            .map(|label| predeclared_file(label, &self.package_path, &self.owner))
                            .collect::<Vec<_>>(),
                    ),
                )
            }
            _ => None,
        }
    }
}

fn predeclared_file(
    label: &slug_identity_v2::CanonicalLabel,
    package_path: &str,
    owner: &AnalysisConfiguredTargetKey,
) -> AnalysisArtifactValue {
    let target = label.target().as_str();
    let path = if package_path.is_empty() {
        target.to_owned()
    } else {
        format!("{package_path}/{target}")
    };
    AnalysisArtifactValue::new(AnalysisArtifact::Derived {
        owner: owner.clone(),
        output: ActionOutput::new(path, ActionOutputKind::File),
    })
}

fn allocate_analysis_attribute<'v>(
    attribute: &ResolvedRuleAttribute,
    dependencies: &[AnalysisDependency],
    heap: Heap<'v>,
) -> Result<Value<'v>, String> {
    let mut dependencies = dependencies
        .iter()
        .filter(|dependency| dependency.attribute == attribute.declaration_name);
    let mut dependency = || {
        dependencies
            .next()
            .map(|dependency| dependency.target.to_value())
            .ok_or_else(|| {
                format!(
                    "resolved attribute `{}` is missing a prepared dependency",
                    attribute.declaration_name
                )
            })
    };
    Ok(match &attribute.value {
        CoercedAttributeValue::None => Value::new_none(),
        CoercedAttributeValue::Label(_) if attribute.sequence => heap.alloc(vec![dependency()?]),
        CoercedAttributeValue::Label(_) => dependency()?,
        CoercedAttributeValue::LabelList(labels) => heap.alloc(
            labels
                .iter()
                .map(|_| dependency())
                .collect::<Result<Vec<_>, _>>()?,
        ),
        CoercedAttributeValue::String(value) => heap.alloc_str(value).to_value(),
        CoercedAttributeValue::StringList(values) => heap.alloc(
            values
                .iter()
                .map(|value| heap.alloc_str(value).to_value())
                .collect::<Vec<_>>(),
        ),
        CoercedAttributeValue::StringListDict(values) => heap.alloc(AllocDict(
            values
                .iter()
                .map(|(key, values)| {
                    (
                        heap.alloc_str(key).to_value(),
                        heap.alloc(
                            values
                                .iter()
                                .map(|value| heap.alloc_str(value).to_value())
                                .collect::<Vec<_>>(),
                        ),
                    )
                })
                .collect::<Vec<_>>(),
        )),
        CoercedAttributeValue::Boolean(value) => Value::new_bool(*value),
        CoercedAttributeValue::Integer(value) => heap.alloc(*value),
        CoercedAttributeValue::StringDict(values) => heap.alloc(AllocDict(
            values
                .iter()
                .map(|(key, value)| {
                    (
                        heap.alloc_str(key).to_value(),
                        heap.alloc_str(value).to_value(),
                    )
                })
                .collect::<Vec<_>>(),
        )),
        CoercedAttributeValue::StringKeyedLabelDict(values) => heap.alloc(AllocDict(
            values
                .iter()
                .map(|(key, _)| Ok((heap.alloc_str(key).to_value(), dependency()?)))
                .collect::<Result<Vec<_>, String>>()?,
        )),
        CoercedAttributeValue::LabelKeyedStringDict(values) => heap.alloc(AllocDict(
            values
                .iter()
                .map(|(_, value)| Ok((dependency()?, heap.alloc_str(value).to_value())))
                .collect::<Result<Vec<_>, String>>()?,
        )),
        CoercedAttributeValue::LabelListDict(values) => heap.alloc(AllocDict(
            values
                .iter()
                .map(|(key, labels)| {
                    Ok((
                        heap.alloc_str(key).to_value(),
                        heap.alloc(
                            labels
                                .iter()
                                .map(|_| dependency())
                                .collect::<Result<Vec<_>, String>>()?,
                        ),
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?,
        )),
        CoercedAttributeValue::Output(_)
        | CoercedAttributeValue::OutputList(_)
        | CoercedAttributeValue::Selector { .. }
        | CoercedAttributeValue::Concatenation(_, _) => {
            return Err(format!(
                "attribute `{}` reached ctx.attr with an unresolved or output-only value",
                attribute.declaration_name
            ));
        }
    })
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisToolchains {
    #[allocative(skip)]
    token: AnalysisCallToken,
    toolchains: PreparedAnalysisToolchains,
}

impl AnalysisToolchains {
    fn transform(&self, index: Value<'_>) -> starlark::Result<slug_identity_v2::CanonicalLabel> {
        if let Some(label) = starlark_label(index) {
            return Ok(label);
        }
        let Some(raw) = index.unpack_str() else {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "ctx.toolchains indices must be Labels or Strings"
            )));
        };
        resolve_rule_definition_label(raw, &self.toolchains.definition_source)
            .map_err(starlark::Error::new_other)
    }

    fn row(
        &self,
        label: &slug_identity_v2::CanonicalLabel,
    ) -> Option<&PreparedAnalysisToolchainRow> {
        self.toolchains
            .rows
            .iter()
            .find(|row| &row.requested == label || &row.actual == label)
    }
}

impl fmt::Display for AnalysisToolchains {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<ctx.toolchains>")
    }
}

starlark::starlark_simple_value!(AnalysisToolchains);

#[starlark_value(type = "toolchains")]
impl<'v> StarlarkValue<'v> for AnalysisToolchains {
    fn at(&self, index: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        self.token.require_active("[]", "rule context toolchains")?;
        let label = self.transform(index)?;
        let row = self.row(&label).ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "ctx.toolchains does not contain requested type {label}"
            ))
        })?;
        Ok(row.info.map_or_else(Value::new_none, FrozenValue::to_value))
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        self.token.require_active("in", "rule context toolchains")?;
        Ok(self.row(&self.transform(other)?).is_some())
    }
}

#[derive(Debug, Clone, Allocative)]
struct AnalysisDependency {
    attribute: CompactString,
    target: FrozenValue,
}

/// Synchronously evaluate one loaded rule after DICE has prepared all direct
/// dependency providers. No graph lookup or asynchronous work occurs here.
fn materialize_toolchain_info(
    actual: &slug_identity_v2::CanonicalLabel,
    info: &ProviderOccurrence,
    materialized: &mut SmallMap<slug_identity_v2::CanonicalLabel, FrozenValue>,
    materializer: &mut AnalysisValueMaterializer<'_>,
) -> Result<FrozenValue, String> {
    if let Some(value) = materialized.get(actual) {
        return Ok(*value);
    }
    let value = materializer.value(&AnalysisValue::provider(info.clone()))?;
    materialized.insert(actual.clone(), value);
    Ok(value)
}

fn materialize_analysis_toolchains(
    toolchain: PreparedToolchain,
    definition_source: Arc<BzlModuleIdentity>,
    materializer: &mut AnalysisValueMaterializer<'_>,
) -> Result<PreparedAnalysisToolchains, String> {
    let context = toolchain
        .action_context
        .toolchain()
        .expect("prepared toolchain retains its configured context");
    let mut materialized = SmallMap::with_capacity(context.rows().len());
    let rows = context
        .rows()
        .iter()
        .map(|row| {
            let info = row
                .selected()
                .map(|selected| {
                    materialize_toolchain_info(
                        row.actual().label(),
                        selected.info(),
                        &mut materialized,
                        materializer,
                    )
                })
                .transpose()?;
            Ok(PreparedAnalysisToolchainRow {
                requested: row.requested().label().clone(),
                actual: row.actual().label().clone(),
                info,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(PreparedAnalysisToolchains {
        definition_source,
        rows: rows.into(),
    })
}

#[cfg(test)]
mod toolchain_materialization_tests {
    use slug_build_api_v2::ProviderIdentity;
    use starlark::values::FrozenHeap;

    use super::*;

    #[test]
    fn distinct_requested_aliases_share_one_actual_toolchain_value() {
        let heap = FrozenHeap::new();
        let mut materializer = AnalysisValueMaterializer::new(&heap);
        let mut materialized = SmallMap::new();
        let actual = slug_identity_v2::CanonicalLabel::parse("@@//:actual").unwrap();
        let other = slug_identity_v2::CanonicalLabel::parse("@@//:other").unwrap();
        let alias_a = ProviderOccurrence::new(
            ProviderIdentity::builtin("ToolchainInfo"),
            [("marker", AnalysisValue::string("shared"))],
        );
        let alias_b = alias_a.clone();

        let first =
            materialize_toolchain_info(&actual, &alias_a, &mut materialized, &mut materializer)
                .unwrap();
        let second =
            materialize_toolchain_info(&actual, &alias_b, &mut materialized, &mut materializer)
                .unwrap();
        let distinct =
            materialize_toolchain_info(&other, &alias_b, &mut materialized, &mut materializer)
                .unwrap();

        assert!(first.to_value().ptr_eq(second.to_value()));
        assert!(!first.to_value().ptr_eq(distinct.to_value()));
        assert_eq!(materialized.len(), 2);
    }
}

pub(crate) fn evaluate_loaded_rule(
    package: &LoadedPackage,
    target_name: &str,
    key: ConfiguredTargetKey,
    package_path: &str,
    dependencies: Vec<PreparedDependency>,
    resolved_attributes: Vec<ResolvedRuleAttribute>,
    configured_attributes: Vec<PreparedConfiguredAttribute>,
    action_context: Arc<ConfiguredActionOwnerContext>,
    toolchain: Option<PreparedToolchain>,
    print_handler: Option<&dyn PrintHandler>,
) -> Result<ConfiguredNodeResult, LoadedRuleError> {
    let target = package
        .targets
        .iter()
        .find(|target| target.name == target_name)
        .ok_or_else(|| format!("target `{target_name}` was not found in loaded package"))?;
    let rule_capability = target.rule_capability().cloned();
    let PackageTargetKind::StarlarkRule(implementation) = &target.kind else {
        return Err(format!("target `{target_name}` is not a Starlark rule").into());
    };
    let build_setting_value = match implementation
        .build_setting_declaration()
        .map_err(|error| error.to_string())?
    {
        Some(declaration) => Some(
            build_setting::effective_value(
                key.label(),
                &declaration,
                key.configuration().starlark_option(key.label()),
            )
            .map_err(LoadedRuleError::from)?,
        ),
        None => None,
    };

    let action_contexts = vec![action_context];
    let actions = Arc::new(Mutex::new(CtxActions::new()));
    let module = Module::new();
    let needs_cpp_fragment = implementation
        .required_fragments()
        .iter()
        .any(|fragment| fragment == "cpp")
        || implementation
            .subrule_invocations()
            .any(|(_, _, _, fragments)| fragments.contains("cpp"));
    let cpp_fragment = if needs_cpp_fragment {
        let structural_configuration =
            key.configuration().slug_configuration().ok_or_else(|| {
                "configured fragment projection requires structural configuration".to_owned()
            })?;
        module.frozen_heap().alloc(CppFragmentValue::new(
            CppFragmentProjection::new(structural_configuration.clone())
                .map_err(|error| error.to_string())?,
            implementation.source_identities_by_filename().clone(),
        ))
    } else {
        FrozenValue::new_none()
    };
    let retained_owner = AnalysisConfiguredTargetKey::new(
        key.label().clone(),
        key.configuration().complete_identity_bytes(),
    );
    let (dependencies, toolchain) = {
        let mut materializer = AnalysisValueMaterializer::new(module.frozen_heap());
        let dependencies = dependencies
            .into_iter()
            .map(|dependency| {
                let target = if dependency.target_shape {
                    materializer
                        .configured_dependency_target(&dependency.key, dependency.providers)?
                } else {
                    materializer.configured_dependency(&dependency.key, dependency.providers)?
                };
                Ok(AnalysisDependency {
                    attribute: dependency.attribute,
                    target,
                })
            })
            .collect::<Result<Arc<[_]>, String>>()?;
        let toolchain = toolchain
            .map(|toolchain| {
                materialize_analysis_toolchains(
                    toolchain,
                    implementation.definition_source().clone(),
                    &mut materializer,
                )
            })
            .transpose()?;
        (dependencies, toolchain)
    };
    let prepared_subrules = {
        let mut materializer = AnalysisValueMaterializer::new(module.frozen_heap());
        implementation
            .subrule_invocations()
            .map(|(identity, _, _, fragments)| {
                let hidden = configured_attributes
                    .iter()
                    .filter(|attribute| attribute.owner.as_ref() == Some(&identity))
                    .map(|attribute| {
                        Ok((
                            attribute
                                .user_name
                                .clone()
                                .expect("subrule attributes retain their user-facing name"),
                            materializer.value(&attribute.value)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(PreparedSubruleInvocation::new(identity, hidden, fragments))
            })
            .collect::<Result<Vec<_>, String>>()?
    };
    let returned = {
        let analysis_context = AnalysisEvaluationContext::new(
            implementation.direct_subrule_identities(),
            prepared_subrules,
            key.label().clone(),
            package_path.to_owned(),
            retained_owner.clone(),
            actions.clone(),
            cpp_fragment,
        );
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&analysis_context);
        if let Some(print_handler) = print_handler {
            evaluator.set_print_handler(print_handler);
        }
        let build_setting_value = build_setting_value
            .as_ref()
            .map(|value| build_setting::alloc_value(value, module.heap(), &mut evaluator))
            .transpose()
            .map_err(|error| error.to_string())?;
        let root_fragment_declarations = Arc::new(
            implementation
                .required_fragments()
                .iter()
                .cloned()
                .collect::<SmallSet<_>>(),
        );
        let fragments = module.heap().alloc_simple(RuleFragmentCollection::new(
            analysis_context.root_token(),
            root_fragment_declarations,
            cpp_fragment,
        ));
        let context = module.heap().alloc(AnalysisContextGen {
            actions: actions.clone(),
            token: analysis_context.root_token(),
            retained_owner: retained_owner.clone(),
            target_label: key.label().clone(),
            package_path: package_path.to_owned(),
            dependencies,
            resolved_attributes: resolved_attributes.into(),
            build_setting_value,
            fragments,
            toolchain,
        });
        let result =
            evaluator.eval_function(implementation.frozen_value().to_value(), &[context], &[]);
        drop(evaluator);
        result
    }
    .map_err(|error| error.to_string())?;

    let returned = ListRef::from_value(returned)
        .ok_or_else(|| "rule implementation must return a list of providers".to_owned())?;
    let mut lowerer = AnalysisValueLowerer::default();
    let mut provider_values = Vec::with_capacity(returned.len());
    for (index, value) in returned.iter().enumerate() {
        if let Some((files, executable)) = StarlarkDefaultInfo::fields_from_value(value) {
            let files = files
                .map(|files| {
                    let files = lowerer.lower(files, &format!("$[{index}].files"))?;
                    let AnalysisValueKind::Depset(files) = files.kind() else {
                        return Err(
                            "DefaultInfo.files must be the result of depset([...])".to_owned()
                        );
                    };
                    let declared_outputs =
                        files
                            .to_list()
                            .into_iter()
                            .map(|value| match value.kind() {
                                AnalysisValueKind::Artifact(AnalysisArtifact::Derived {
                                    output,
                                    ..
                                }) => Ok(output.path().to_owned()),
                                _ => Err("DefaultInfo.files depset must contain declared files"
                                    .to_owned()),
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                    slug_build_api_v2::Depset::from_direct(files.order(), declared_outputs)
                        .map_err(|error| error.to_string())
                })
                .transpose()?;
            let executable = executable
                .map(|executable| {
                    AnalysisArtifactValue::from_starlark(executable)
                        .and_then(|value| value.output_for_owner(&retained_owner))
                        .map(|output| output.path().to_owned())
                        .ok_or_else(|| "DefaultInfo.executable must be a declared file".to_owned())
                })
                .transpose()?;
            let default_info = match executable {
                Some(executable) => {
                    slug_build_api_v2::DefaultInfo::from_executable(executable, files)
                }
                None => slug_build_api_v2::DefaultInfo::from_files(
                    files.unwrap_or_else(slug_build_api_v2::Depset::empty),
                ),
            };
            provider_values.push(ProviderValue::DefaultInfo(default_info));
        } else {
            let lowered = lowerer.lower(value, &format!("$[{index}]"))?;
            let AnalysisValueKind::Provider(provider) = lowered.kind() else {
                return Err(format!(
                    "rule implementation returned non-provider value `{}`",
                    value.to_repr()
                )
                .into());
            };
            provider_values.push(ProviderValue::Occurrence(provider.clone()));
        }
    }
    if !provider_values
        .iter()
        .any(|provider| matches!(provider, ProviderValue::DefaultInfo(_)))
    {
        provider_values.push(ProviderValue::DefaultInfo(
            slug_build_api_v2::DefaultInfo::empty(),
        ));
    }
    let providers = ProviderCollection::new(provider_values).map_err(|error| error.to_string())?;
    if rule_capability
        .as_ref()
        .is_some_and(|capability| capability.executable)
        && providers
            .default_info()
            .is_some_and(|default_info| default_info.executable.is_none())
    {
        return Err(LoadedRuleError::ExecutableRuleMissingExecutable {
            rule_class: rule_capability
                .as_ref()
                .expect("executable rule has a capability")
                .rule_class
                .clone(),
        });
    }
    let declared_outputs = providers
        .default_info()
        .expect("ProviderCollection validated DefaultInfo")
        .files
        .to_list();
    let actions = actions
        .lock()
        .map_err(|_| "ctx.actions state lock is poisoned".to_owned())?
        .registry()
        .actions()
        .to_vec();
    let result = ConfiguredNodeResult::new_rule(key, providers, rule_capability)
        .with_action_specs(actions, action_contexts)
        .map_err(LoadedRuleError::from)?;
    Ok(result.with_declared_outputs(declared_outputs))
}
