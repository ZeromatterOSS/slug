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
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::ArgsWriteSpec;
use slug_build_api_v2::ArtifactInputSource;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::RetainedArgCall;
use slug_build_api_v2::RetainedArgsDepset;
use slug_build_api_v2::RetainedArgsRecipe;
use slug_build_api_v2::RetainedArtifactInputs;
use slug_build_api_v2::RetainedCommandLine;
use slug_build_api_v2::RetainedCommandLineSegment;
use slug_build_api_v2::RetainedRunfiles;
use slug_build_api_v2::RetainedSpawnArgsSnapshot;
use slug_build_api_v2::RetainedSpawnInvocation;
use slug_build_api_v2::RetainedSpawnParamFilePolicy;
use slug_build_api_v2::RetainedVectorArg;
use slug_build_api_v2::RetainedVectorSource;
use slug_build_api_v2::RunfilesConflictPolicy;
use slug_build_api_v2::RunfilesPackageDepset;
use slug_build_api_v2::RunfilesSymlink;
use slug_build_api_v2::RunfilesSymlinkDepset;
use slug_build_api_v2::SpawnExecutable;
use slug_build_api_v2::SpawnSpec;
use slug_build_api_v2::SymlinkSpec;
use slug_build_api_v2::SymlinkTarget;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::CppFragmentProjection;
use slug_configuration_v2::HostPathFlavor;
use slug_configuration_v2::NormalizedAbsoluteBazelPath;
use slug_configuration_v2::NormalizedBazelPath;
use slug_configuration_v2::RetainedActionEnvironment;
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
use slug_loading_v2::provider::StarlarkDefaultInfoFields;
use slug_loading_v2::provider::starlark_label;
use slug_loading_v2::subrule_invocation::AnalysisActionCallScope;
use slug_loading_v2::subrule_invocation::AnalysisActionSink;
use slug_loading_v2::subrule_invocation::AnalysisActions;
use slug_loading_v2::subrule_invocation::AnalysisArtifactValue;
use slug_loading_v2::subrule_invocation::AnalysisCallToken;
use slug_loading_v2::subrule_invocation::AnalysisEvaluationContext;
use slug_loading_v2::subrule_invocation::AnalysisRunRequest;
use slug_loading_v2::subrule_invocation::AnalysisSpawnInvocation;
use slug_loading_v2::subrule_invocation::AnalysisSpawnRequest;
use slug_loading_v2::subrule_invocation::EvaluatorArgCallGen;
use slug_loading_v2::subrule_invocation::EvaluatorArgsSnapshot;
use slug_loading_v2::subrule_invocation::EvaluatorVectorArgGen;
use slug_loading_v2::subrule_invocation::EvaluatorVectorSourceGen;
use slug_loading_v2::subrule_invocation::PreparedSubruleInvocation;
use slug_loading_v2::subrule_invocation::StarlarkArgs;
use starlark::PrintHandler;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::starlark_complex_value;
use starlark::starlark_module;
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
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::starlark_value;
use starlark::values::tuple::TupleRef;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::analysis_value::AnalysisValueLowerer;
use crate::analysis_value::AnalysisValueMaterializer;
use crate::analysis_value::lower_runfiles_symlink_depset;
use crate::analysis_value::materialize_runfiles;
use crate::analysis_value::retained_runfiles;
use crate::build_setting;
use crate::configured_attribute::ResolvedRuleAttribute;
use crate::key::ConfiguredNodeKey;
use crate::key::ConfiguredTargetKey;
use crate::result::ConfiguredActionOwnerContext;
use crate::result::ConfiguredNodeResult;
use crate::runfiles_support::complete_runfiles_support;

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
    action_sink: Arc<dyn AnalysisActionSink>,
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
            action_sink: self.action_sink,
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
                self.action_sink.clone(),
                self.token.clone(),
                "rule context",
                AnalysisActionCallScope::Root,
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

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(analysis_context_methods)
    }
}

fn normalized_runfiles_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && path
            .split('/')
            .all(|component| !matches!(component, "" | "." | ".."))
}

fn runfiles_symlinks(
    value: Option<Value<'_>>,
    name: &str,
) -> Result<(Vec<RunfilesSymlink>, Vec<RunfilesSymlinkDepset>), String> {
    let Some(value) = value else {
        return Ok((Vec::new(), Vec::new()));
    };
    if let Some(values) = DictRef::from_value(value) {
        let direct = values
            .iter()
            .map(|(path, artifact)| {
                let path = path
                    .unpack_str()
                    .ok_or_else(|| format!("ctx.runfiles {name} keys must be strings"))?;
                if !normalized_runfiles_path(path) {
                    return Err(format!(
                        "ctx.runfiles {name} path `{path}` must be normalized and relative"
                    ));
                }
                let artifact = AnalysisArtifactValue::from_starlark(artifact)
                    .ok_or_else(|| format!("ctx.runfiles {name} values must be Files"))?;
                Ok(RunfilesSymlink::new(path, artifact.artifact().clone()))
            })
            .collect::<Result<Vec<_>, String>>()?;
        return Ok((direct, Vec::new()));
    }
    let depset = lower_runfiles_symlink_depset(value, &format!("ctx.runfiles.{name}"))?;
    Ok((Vec::new(), vec![depset]))
}

#[starlark_module]
fn analysis_context_methods(builder: &mut MethodsBuilder) {
    fn runfiles<'v>(
        this: &AnalysisContext<'v>,
        #[starlark(default = UnpackListOrTuple::default())] files: UnpackListOrTuple<Value<'v>>,
        transitive_files: Option<Value<'v>>,
        #[starlark(default = false)] collect_data: bool,
        #[starlark(default = false)] collect_default: bool,
        symlinks: Option<Value<'v>>,
        root_symlinks: Option<Value<'v>>,
        #[starlark(require = named, default = false)] skip_conflict_checking: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        this.token.require_active("runfiles", "rule context")?;
        if collect_data || collect_default {
            anyhow::bail!("ctx.runfiles collect_data and collect_default are unsupported/deferred");
        }
        if skip_conflict_checking {
            anyhow::bail!("ctx.runfiles skip_conflict_checking is unsupported/deferred");
        }
        let direct_files = files
            .items
            .into_iter()
            .map(|value| {
                AnalysisArtifactValue::from_starlark(value)
                    .map(|value| value.artifact().clone())
                    .ok_or_else(|| anyhow::anyhow!("ctx.runfiles files must contain Files"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let transitive_files = transitive_files
            .filter(|value| !value.is_none())
            .map(|value| {
                let mut lowerer = AnalysisValueLowerer::default();
                let lowered = lowerer.lower(value, "ctx.runfiles.transitive_files")?;
                let AnalysisValueKind::Depset(files) = lowered.kind() else {
                    return Err(
                        "ctx.runfiles transitive_files must be a depset of Files".to_owned()
                    );
                };
                if !matches!(files.order(), DepsetOrder::Default | DepsetOrder::Postorder) {
                    return Err(format!(
                        "order '{}' is invalid for transitive_files",
                        files.order()
                    ));
                }
                Ok(files.clone())
            })
            .transpose()
            .map_err(anyhow::Error::msg)?
            .into_iter()
            .collect();
        let (direct_symlinks, transitive_symlinks) =
            runfiles_symlinks(symlinks, "symlinks").map_err(anyhow::Error::msg)?;
        let (direct_root_symlinks, transitive_root_symlinks) =
            runfiles_symlinks(root_symlinks, "root_symlinks").map_err(anyhow::Error::msg)?;
        let conflict_policy = if direct_symlinks.is_empty()
            && transitive_symlinks.is_empty()
            && direct_root_symlinks.is_empty()
            && transitive_root_symlinks.is_empty()
        {
            RunfilesConflictPolicy::Warn
        } else {
            RunfilesConflictPolicy::Error
        };
        let retained = RetainedRunfiles::from_parts(
            direct_files,
            transitive_files,
            direct_symlinks,
            transitive_symlinks,
            direct_root_symlinks,
            transitive_root_symlinks,
            conflict_policy,
        )?;
        Ok(materialize_runfiles(&retained, eval.frozen_heap())
            .map_err(anyhow::Error::msg)?
            .to_value())
    }
}

#[derive(Debug, Clone, Allocative)]
pub(crate) struct PreparedDependency {
    pub(crate) key: ConfiguredNodeKey,
    pub(crate) providers: ProviderCollection,
    pub(crate) attribute: CompactString,
    pub(crate) target_shape: bool,
    pub(crate) executable: Option<slug_build_api_v2::FilesToRunProvider>,
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
    pub(crate) runfiles_packages: Arc<[crate::result::RunfilesPackageClosureRow]>,
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

#[derive(Debug)]
struct SynchronousAnalysisActionSink {
    actions: Arc<Mutex<CtxActions>>,
    package_path: String,
    owner: AnalysisConfiguredTargetKey,
    typed_configuration: Result<Option<(HostPathFlavor, RetainedActionEnvironment)>, String>,
    executable_provenance: Arc<ExecutableArtifactProvenance>,
    execution_tags: CanonicalStringMap,
}

#[derive(Debug, Default)]
struct ExecutableArtifactProvenance {
    root: SmallMap<AnalysisArtifact, slug_build_api_v2::FilesToRunProvider>,
    subrules: SmallMap<
        Arc<SubruleIdentity>,
        SmallMap<AnalysisArtifact, slug_build_api_v2::FilesToRunProvider>,
    >,
}

impl ExecutableArtifactProvenance {
    fn contains(&self, scope: &AnalysisActionCallScope, artifact: &AnalysisArtifact) -> bool {
        match scope {
            AnalysisActionCallScope::Root => self.root.contains_key(artifact),
            AnalysisActionCallScope::Subrule(identity) => self
                .subrules
                .get(identity)
                .is_some_and(|artifacts| artifacts.contains_key(artifact)),
        }
    }
}

fn files_to_run_provider(value: &AnalysisValue) -> Option<slug_build_api_v2::FilesToRunProvider> {
    let AnalysisValueKind::Provider(provider) = value.kind() else {
        return None;
    };
    slug_build_api_v2::FilesToRunProvider::from_occurrence(provider)
}

fn executable_artifact_provenance(
    dependencies: &[PreparedDependency],
    configured_attributes: &[PreparedConfiguredAttribute],
) -> ExecutableArtifactProvenance {
    let mut provenance = ExecutableArtifactProvenance::default();
    for provider in dependencies
        .iter()
        .filter_map(|dependency| dependency.executable.clone())
    {
        if let Some(executable) = provider.executable.clone() {
            provenance.root.insert(executable, provider);
        }
    }
    for attribute in configured_attributes {
        let (Some(owner), Some(provider)) =
            (&attribute.owner, files_to_run_provider(&attribute.value))
        else {
            continue;
        };
        let Some(executable) = provider.executable.clone() else {
            continue;
        };
        if let Some(artifacts) = provenance.subrules.get_mut(owner) {
            artifacts.insert(executable, provider);
        } else {
            provenance
                .subrules
                .insert(owner.clone(), SmallMap::from_iter([(executable, provider)]));
        }
    }
    provenance
}

impl SynchronousAnalysisActionSink {
    fn owned_output(&self, value: Value<'_>, operation: &str) -> anyhow::Result<ActionOutput> {
        AnalysisArtifactValue::from_starlark(value)
            .and_then(|file| file.output_for_owner(&self.owner))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.{operation} requires a declared file"))
    }

    fn outputs(&self, value: Value<'_>, operation: &str) -> anyhow::Result<Vec<ActionOutput>> {
        let values = sequence_values(value).ok_or_else(|| {
            anyhow::anyhow!("ctx.actions.{operation} outputs must be a sequence of declared Files")
        })?;
        let outputs = values
            .into_iter()
            .map(|value| self.owned_output(value, operation))
            .collect::<anyhow::Result<Vec<_>>>()?;
        if outputs.is_empty() {
            anyhow::bail!("ctx.actions.{operation} requires at least one output");
        }
        Ok(outputs)
    }

    fn typed_configuration(&self) -> anyhow::Result<(HostPathFlavor, &RetainedActionEnvironment)> {
        let configured = self
            .typed_configuration
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let (flavor, environment) = configured.as_ref().ok_or_else(|| {
            anyhow::anyhow!("typed actions require structural action configuration")
        })?;
        Ok((*flavor, environment))
    }

    fn register(
        &self,
        register: impl FnOnce(&mut CtxActions) -> Result<usize, slug_build_api_v2::ActionError>,
    ) -> anyhow::Result<()> {
        let mut actions = self
            .actions
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions state lock is poisoned"))?;
        register(&mut actions)
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    fn retained_invocation(
        &self,
        invocation: AnalysisSpawnInvocation<'_>,
        scope: &AnalysisActionCallScope,
        path_flavor: HostPathFlavor,
        pad_dollar_zero: bool,
    ) -> anyhow::Result<RetainedSpawnInvocation> {
        match invocation {
            AnalysisSpawnInvocation::Executable(value) => {
                let executable = if let Some(file) = AnalysisArtifactValue::from_starlark(value) {
                    reject_directory_file(file, "ctx.actions.run executable")?;
                    reject_associated_executable(
                        &self.executable_provenance,
                        scope,
                        file.artifact(),
                        "ctx.actions.run executable",
                    )?;
                    SpawnExecutable::Artifact(file.artifact().clone())
                } else if let Some(path) = value.unpack_str() {
                    SpawnExecutable::Path(
                        NormalizedBazelPath::new(path_flavor, path)
                            .map_err(|error| anyhow::anyhow!(error.to_string()))?,
                    )
                } else {
                    anyhow::bail!(
                        "ctx.actions.run executable must be an unassociated File or string path"
                    )
                };
                Ok(RetainedSpawnInvocation::Executable(executable))
            }
            AnalysisSpawnInvocation::Shell(value) => value
                .unpack_str()
                .map(|command| RetainedSpawnInvocation::Shell {
                    command: CompactString::new(command),
                    pad_dollar_zero,
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "ctx.actions.run_shell command must be a string under Bazel 9 defaults"
                    )
                }),
        }
    }
}

impl AnalysisActionSink for SynchronousAnalysisActionSink {
    fn declare_file(&self, path: &str) -> anyhow::Result<AnalysisArtifactValue> {
        let path = if self.package_path.is_empty() {
            path.to_owned()
        } else {
            format!("{}/{path}", self.package_path)
        };
        let output = self
            .actions
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions state lock is poisoned"))?
            .declare_file(path)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
            owner: self.owner.clone(),
            output,
        }))
    }

    fn write(
        &self,
        output: Value<'_>,
        content: Value<'_>,
        is_executable: bool,
    ) -> anyhow::Result<()> {
        let output = self.owned_output(output, "write")?;
        if output.kind() != ActionOutputKind::File {
            anyhow::bail!("ctx.actions.write output must be a regular declared File");
        }
        if let Some(content) = content.unpack_str() {
            return self.register(|actions| actions.write(output, content, is_executable));
        }
        let snapshot = StarlarkArgs::snapshot(content)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.write content must be a string or Args"))?;
        let mut lowerer = AnalysisValueLowerer::default();
        let (recipe, _) = lower_args_snapshot(snapshot, &mut lowerer)?;
        self.register(|actions| {
            actions.register_args_write(ArgsWriteSpec::new(output, recipe, is_executable))
        })
    }

    fn run_shell(
        &self,
        outputs: Value<'_>,
        command: &str,
        arguments: Value<'_>,
    ) -> anyhow::Result<()> {
        let output = self
            .outputs(outputs, "run_shell")?
            .into_iter()
            .next()
            .expect("outputs checked nonempty");
        let arguments = sequence_values(arguments)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.run_shell arguments must be a sequence"))?
            .into_iter()
            .map(|value| value.to_str())
            .collect();
        self.register(|actions| actions.run_shell(output, command, arguments, Vec::new()))
    }

    fn run(&self, request: AnalysisRunRequest<'_>) -> anyhow::Result<()> {
        let outputs = self.outputs(request.outputs, "run")?;
        let (path_flavor, configured_environment) = self.typed_configuration()?;
        let executable =
            if let Some(file) = AnalysisArtifactValue::from_starlark(request.executable) {
                reject_directory_file(file, "ctx.actions.run executable")?;
                SpawnExecutable::Artifact(file.artifact().clone())
            } else if let Some(path) = request.executable.unpack_str() {
                SpawnExecutable::Path(
                    NormalizedBazelPath::new(path_flavor, path)
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?,
                )
            } else {
                anyhow::bail!("ctx.actions.run executable must be a File or string path")
            };
        let mut lowerer = AnalysisValueLowerer::default();
        let (command_line, _) = retained_command_line(request.arguments, &mut lowerer)?;
        let inputs = retained_artifact_inputs(request.inputs, false, "inputs", &mut lowerer)?;
        let tools = retained_artifact_inputs(request.tools, true, "tools", &mut lowerer)?;
        let action_environment = configured_environment.for_action(
            request.use_default_shell_env,
            string_dict(request.env, "ctx.actions.run env")?,
        );
        let mnemonic = request.mnemonic.unwrap_or("Action");
        if mnemonic.is_empty() || !mnemonic.chars().all(char::is_alphanumeric) {
            anyhow::bail!("ctx.actions.run mnemonic must be nonempty and alphanumeric");
        }
        let spec = SpawnSpec::new(
            RetainedSpawnInvocation::Executable(executable),
            command_line,
            inputs,
            tools,
            outputs,
            None,
            action_environment,
            CanonicalStringMap::default(),
            mnemonic,
            request.progress_message,
        );
        self.register(|actions| actions.register_spawn(spec))
    }

    fn spawn(&self, request: AnalysisSpawnRequest<'_>) -> anyhow::Result<()> {
        let operation = match request.invocation {
            AnalysisSpawnInvocation::Executable(_) => "run",
            AnalysisSpawnInvocation::Shell(_) => "run_shell",
        };
        let mut lowerer = AnalysisValueLowerer::default();
        let (command_line, has_arguments) = retained_command_line(request.arguments, &mut lowerer)?;
        let (path_flavor, configured_environment) = self.typed_configuration()?;
        let invocation = self.retained_invocation(
            request.invocation,
            &request.scope,
            path_flavor,
            has_arguments,
        )?;
        let inputs = retained_artifact_inputs(request.inputs, false, "inputs", &mut lowerer)?;
        let outputs = self.outputs(request.outputs, operation)?;
        let unused_inputs_list = optional_regular_file(
            request.unused_inputs_list,
            "ctx.actions.run unused_inputs_list",
        )?;
        let tools = retained_tools(
            request.tools,
            &request.scope,
            &self.executable_provenance,
            &mut lowerer,
        )?;
        let mnemonic = validated_mnemonic(request.mnemonic, operation)?;
        let action_environment = configured_environment.for_action(
            request.use_default_shell_env,
            string_dict(request.env, &format!("ctx.actions.{operation} env"))?,
        );
        let execution_requirements = retained_execution_requirements(
            request.execution_requirements,
            &self.execution_tags,
            operation,
        )?;
        validate_default_spawn_context(&request, operation)?;
        let spec = SpawnSpec::new(
            invocation,
            command_line,
            inputs,
            tools,
            outputs,
            unused_inputs_list,
            action_environment,
            execution_requirements,
            mnemonic,
            request.progress_message,
        );
        self.register(|actions| actions.register_spawn(spec))
    }

    fn is_files_to_run_provider(&self, value: Value<'_>) -> bool {
        crate::analysis_value::is_files_to_run_provider(value)
    }

    fn artifact_symlink(
        &self,
        output: Value<'_>,
        target_file: Value<'_>,
        is_executable: bool,
        progress_message: Option<&str>,
    ) -> anyhow::Result<()> {
        let output = self.owned_output(output, "symlink")?;
        if output.kind() != ActionOutputKind::File {
            anyhow::bail!("ctx.actions.symlink output must be a regular declared File");
        }
        let input = AnalysisArtifactValue::from_starlark(target_file)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.symlink target_file must be a File"))?;
        reject_directory_file(input, "ctx.actions.symlink target_file")?;
        let spec = SymlinkSpec::new(
            output,
            SymlinkTarget::Artifact {
                input: input.artifact().clone(),
                require_executable: is_executable,
                use_exec_root_for_source: false,
            },
            progress_message,
        );
        self.register(|actions| actions.register_symlink(spec))
    }

    fn absolute_symlink(
        &self,
        output: Value<'_>,
        target_path: &str,
        progress_message: Option<&str>,
    ) -> anyhow::Result<()> {
        let output = self.owned_output(output, "absolute_symlink")?;
        if output.kind() != ActionOutputKind::File {
            anyhow::bail!("absolute_symlink output must be a regular declared File");
        }
        let (path_flavor, _) = self.typed_configuration()?;
        let target = NormalizedAbsoluteBazelPath::new(path_flavor, target_path)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let spec = SymlinkSpec::new(
            output,
            SymlinkTarget::AbsolutePath { target },
            progress_message,
        );
        self.register(|actions| actions.register_symlink(spec))
    }
}

fn sequence_values(value: Value<'_>) -> Option<Vec<Value<'_>>> {
    if let Some(values) = ListRef::from_value(value) {
        Some(values.iter().collect())
    } else {
        TupleRef::from_value(value).map(|values| values.iter().collect())
    }
}

fn reject_directory_file(file: &AnalysisArtifactValue, name: &str) -> anyhow::Result<()> {
    if matches!(
        file.artifact(),
        AnalysisArtifact::Derived { output, .. } if output.kind() == ActionOutputKind::Directory
    ) {
        anyhow::bail!("{name} must be a regular File")
    }
    Ok(())
}

fn retained_command_line<'v>(
    arguments: Option<Value<'v>>,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<(RetainedCommandLine, bool)> {
    let Some(arguments) = arguments else {
        return Ok((RetainedCommandLine::new(Vec::new()), false));
    };
    if arguments.is_none() {
        anyhow::bail!("ctx.actions.run arguments must be a sequence");
    }
    let arguments = sequence_values(arguments)
        .ok_or_else(|| anyhow::anyhow!("ctx.actions.run arguments must be a sequence"))?;
    let has_arguments = !arguments.is_empty();
    let mut segments = Vec::new();
    let mut literals = Vec::new();
    for argument in arguments {
        if let Some(args) = StarlarkArgs::snapshot(argument) {
            if !literals.is_empty() {
                segments.push(RetainedCommandLineSegment::LiteralRun(
                    std::mem::take(&mut literals).into(),
                ));
            }
            let (recipe, policy) = lower_args_snapshot(args, lowerer)?;
            segments.push(RetainedCommandLineSegment::ArgsSnapshot(
                RetainedSpawnArgsSnapshot::new(recipe, policy),
            ));
        } else if let Some(literal) = argument.unpack_str() {
            literals.push(CompactString::new(literal));
        } else {
            anyhow::bail!("ctx.actions.run arguments entries must be strings or Args")
        }
    }
    if !literals.is_empty() {
        segments.push(RetainedCommandLineSegment::LiteralRun(literals.into()));
    }
    Ok((RetainedCommandLine::new(segments), has_arguments))
}

fn lower_args_snapshot<'v>(
    snapshot: EvaluatorArgsSnapshot<'v>,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<(RetainedArgsRecipe, Option<RetainedSpawnParamFilePolicy>)> {
    let calls = snapshot
        .calls
        .into_iter()
        .map(|call| match call {
            EvaluatorArgCallGen::Scalar(value) => Ok(RetainedArgCall::Scalar(value)),
            EvaluatorArgCallGen::AddAll(value) => {
                lower_vector_arg(value, lowerer).map(RetainedArgCall::AddAll)
            }
            EvaluatorArgCallGen::AddJoined(value) => {
                lower_vector_arg(value, lowerer).map(RetainedArgCall::AddJoined)
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let policy = snapshot.param_file.map(|(flag_format, use_always)| {
        RetainedSpawnParamFilePolicy::new(flag_format, use_always)
    });
    Ok((RetainedArgsRecipe::new(calls, snapshot.format), policy))
}

fn lower_vector_arg<'v>(
    value: EvaluatorVectorArgGen<Value<'v>>,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<RetainedVectorArg> {
    let source = match value.source {
        EvaluatorVectorSourceGen::Sequence(values) => RetainedVectorSource::Sequence(
            values
                .into_iter()
                .map(vector_scalar_value)
                .collect::<anyhow::Result<Vec<_>>>()?
                .into(),
        ),
        EvaluatorVectorSourceGen::Depset(value) => {
            let lowered = lowerer
                .lower(value, "Args vector depset")
                .map_err(anyhow::Error::msg)?;
            let AnalysisValueKind::Depset(depset) = lowered.kind() else {
                anyhow::bail!("Args vector values must be a sequence or depset")
            };
            RetainedVectorSource::Depset(
                RetainedArgsDepset::new(depset.clone())
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?,
            )
        }
    };
    Ok(RetainedVectorArg::new(source, value.options))
}

fn vector_scalar_value(value: Value<'_>) -> anyhow::Result<slug_build_api_v2::RetainedScalarValue> {
    if let Some(value) = value.unpack_str() {
        return Ok(slug_build_api_v2::RetainedScalarValue::String(value.into()));
    }
    if value.get_type() == "int" {
        return Ok(slug_build_api_v2::RetainedScalarValue::Integer(
            value.to_str().into(),
        ));
    }
    if let Some(file) = AnalysisArtifactValue::from_starlark(value) {
        reject_directory_file(file, "Args vector value")?;
        return Ok(slug_build_api_v2::RetainedScalarValue::Artifact(
            file.artifact().clone(),
        ));
    }
    anyhow::bail!(
        "Args vector supports only strings, integers, and regular Files, got {}",
        value.get_type()
    )
}

fn retained_artifact_inputs<'v>(
    value: Option<Value<'v>>,
    allow_nested_depsets: bool,
    name: &str,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<ArtifactInputs> {
    let Some(value) = value else {
        return Ok(ArtifactInputs::new(Vec::new()));
    };
    if value.is_none() {
        anyhow::bail!("ctx.actions.run {name} must be a sequence or depset of Files");
    }
    let lowered = lowerer
        .lower(value, &format!("ctx.actions.run {name}"))
        .map_err(anyhow::Error::msg)?;
    let values = match lowered.kind() {
        AnalysisValueKind::Depset(depset) => {
            let retained = RetainedArtifactInputs::new(depset.clone())
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            validate_regular_inputs(&retained, name)?;
            return Ok(ArtifactInputs::new(vec![ArtifactInputSource::Depset(
                retained,
            )]));
        }
        AnalysisValueKind::List(values) | AnalysisValueKind::Tuple(values) => values,
        _ => anyhow::bail!("ctx.actions.run {name} must be a sequence or depset of Files"),
    };
    let sources = values
        .iter()
        .map(|value| match value.kind() {
            AnalysisValueKind::Artifact(artifact) => {
                reject_directory_artifact(artifact, &format!("ctx.actions.run {name}"))?;
                Ok(ArtifactInputSource::Direct(artifact.clone()))
            }
            AnalysisValueKind::Depset(depset) if allow_nested_depsets => {
                let retained = RetainedArtifactInputs::new(depset.clone())
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                validate_regular_inputs(&retained, name)?;
                Ok(ArtifactInputSource::Depset(retained))
            }
            _ => Err(anyhow::anyhow!(
                "ctx.actions.run {name} entries must be Files{}",
                if allow_nested_depsets {
                    " or depsets of Files"
                } else {
                    ""
                }
            )),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(ArtifactInputs::new(sources))
}

fn reject_directory_artifact(artifact: &AnalysisArtifact, name: &str) -> anyhow::Result<()> {
    if matches!(
        artifact,
        AnalysisArtifact::Derived { output, .. } if output.kind() == ActionOutputKind::Directory
    ) {
        anyhow::bail!("{name} must contain only regular Files")
    }
    Ok(())
}

fn validate_regular_inputs(inputs: &RetainedArtifactInputs, name: &str) -> anyhow::Result<()> {
    let mut error = None;
    inputs
        .visit(|artifact| {
            if error.is_none() {
                error =
                    reject_directory_artifact(artifact, &format!("ctx.actions.run {name}")).err();
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    error.map_or(Ok(()), Err)
}

fn reject_associated_executable(
    provenance: &ExecutableArtifactProvenance,
    scope: &AnalysisActionCallScope,
    artifact: &AnalysisArtifact,
    name: &str,
) -> anyhow::Result<()> {
    if provenance.contains(scope, artifact) {
        anyhow::bail!(
            "{name} is associated with FilesToRunProvider; FilesToRun/runfiles expansion is not supported"
        )
    }
    Ok(())
}

fn retained_tools<'v>(
    value: Option<Value<'v>>,
    scope: &AnalysisActionCallScope,
    provenance: &ExecutableArtifactProvenance,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<ArtifactInputs> {
    let Some(value) = value else {
        return Ok(ArtifactInputs::new(Vec::new()));
    };
    if value.is_none() {
        anyhow::bail!("ctx.actions.run tools must be a sequence or depset");
    }
    let lowered = lowerer
        .lower(value, "ctx.actions.run tools")
        .map_err(anyhow::Error::msg)?;
    // Bazel 9.2 StarlarkActionFactory.registerAction expands a top-level
    // tools depset before its Artifact-to-FilesToRun lookup.
    if let AnalysisValueKind::Depset(depset) = lowered.kind() {
        let retained = RetainedArtifactInputs::new(depset.clone())
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        validate_tool_depset(&retained, scope, provenance, true)?;
        return Ok(ArtifactInputs::new(vec![ArtifactInputSource::Depset(
            retained,
        )]));
    }
    let (AnalysisValueKind::List(values) | AnalysisValueKind::Tuple(values)) = lowered.kind()
    else {
        anyhow::bail!("ctx.actions.run tools must contain Files or depsets; FilesToRun is deferred")
    };
    let sources = values
        .iter()
        .map(|value| match value.kind() {
            AnalysisValueKind::Artifact(artifact) => {
                reject_directory_artifact(artifact, "ctx.actions.run tools")?;
                reject_associated_executable(
                    provenance,
                    scope,
                    artifact,
                    "ctx.actions.run direct tool",
                )?;
                Ok(ArtifactInputSource::Direct(artifact.clone()))
            }
            AnalysisValueKind::Depset(depset) => {
                let retained = RetainedArtifactInputs::new(depset.clone())
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                // The Sequence branch adds nested depsets transitively and
                // deliberately performs no per-leaf FilesToRun lookup.
                validate_tool_depset(&retained, scope, provenance, false)?;
                Ok(ArtifactInputSource::Depset(retained))
            }
            _ => Err(anyhow::anyhow!(
                "ctx.actions.run tools entries must be Files or depsets; FilesToRun is deferred"
            )),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(ArtifactInputs::new(sources))
}

fn validate_tool_depset(
    tools: &RetainedArtifactInputs,
    scope: &AnalysisActionCallScope,
    provenance: &ExecutableArtifactProvenance,
    check_association: bool,
) -> anyhow::Result<()> {
    let mut error = None;
    tools
        .visit(|artifact| {
            if error.is_some() {
                return;
            }
            error = reject_directory_artifact(artifact, "ctx.actions.run tools").err();
            if error.is_none() && check_association {
                error = reject_associated_executable(
                    provenance,
                    scope,
                    artifact,
                    "ctx.actions.run top-level depset tool",
                )
                .err();
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    error.map_or(Ok(()), Err)
}

fn optional_regular_file(
    value: Option<Value<'_>>,
    name: &str,
) -> anyhow::Result<Option<AnalysisArtifact>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    let file = AnalysisArtifactValue::from_starlark(value)
        .ok_or_else(|| anyhow::anyhow!("{name} must be a File or None"))?;
    reject_directory_file(file, name)?;
    Ok(Some(file.artifact().clone()))
}

fn validated_mnemonic<'a>(mnemonic: Option<&'a str>, operation: &str) -> anyhow::Result<&'a str> {
    let mnemonic = mnemonic.unwrap_or("Action");
    if mnemonic.is_empty() || !mnemonic.chars().all(char::is_alphanumeric) {
        anyhow::bail!("ctx.actions.{operation} mnemonic must be nonempty and alphanumeric");
    }
    Ok(mnemonic)
}

fn is_legal_execution_info_key(key: &str) -> bool {
    // TargetUtils.getExecutionInfo is the Bazel 9.2 owner of this allowlist.
    ["block-", "requires-", "no-", "supports-", "disable-"]
        .iter()
        .any(|prefix| key.starts_with(prefix))
        || key.starts_with("cpu:")
        || key.starts_with("resources:")
        || matches!(key, "local" | "worker-key-mnemonic")
}

fn retained_execution_requirements(
    value: Option<Value<'_>>,
    tags: &CanonicalStringMap,
    operation: &str,
) -> anyhow::Result<CanonicalStringMap> {
    let mut pairs = tags
        .iter()
        .map(|(key, value)| (CompactString::new(key), CompactString::new(value)))
        .collect::<Vec<_>>();
    pairs.extend(
        string_dict(
            value,
            &format!("ctx.actions.{operation} execution_requirements"),
        )?
        .into_iter()
        .filter(|(key, _)| is_legal_execution_info_key(key)),
    );
    Ok(CanonicalStringMap::from_pairs(pairs))
}

fn validate_default_spawn_context(
    request: &AnalysisSpawnRequest<'_>,
    operation: &str,
) -> anyhow::Result<()> {
    if request.exec_group.is_some() {
        anyhow::bail!("ctx.actions.{operation} named exec_group is not supported");
    }
    if request.toolchain.is_some_and(|value| !value.is_none()) {
        anyhow::bail!("ctx.actions.{operation} nondefault toolchain selection is not supported");
    }
    if request
        .shadowed_action
        .is_some_and(|value| !value.is_none())
    {
        anyhow::bail!("ctx.actions.{operation} shadowed_action is not supported");
    }
    if request.has_resource_set {
        anyhow::bail!("ctx.actions.{operation} callable resource_set is not supported");
    }
    Ok(())
}

fn target_execution_tags(attributes: &[ResolvedRuleAttribute]) -> CanonicalStringMap {
    let Some(CoercedAttributeValue::StringList(tags)) = attributes
        .iter()
        .find(|attribute| attribute.declaration_name == "tags")
        .map(|attribute| &attribute.value)
    else {
        return CanonicalStringMap::default();
    };
    CanonicalStringMap::from_pairs(
        tags.iter()
            .filter(|tag| is_legal_execution_info_key(tag))
            .map(|tag| (tag.clone(), CompactString::default())),
    )
}

fn string_dict(
    value: Option<Value<'_>>,
    name: &str,
) -> anyhow::Result<Vec<(CompactString, CompactString)>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(Vec::new());
    };
    let values = DictRef::from_value(value)
        .ok_or_else(|| anyhow::anyhow!("{name} must be a dictionary of strings"))?;
    values
        .iter()
        .map(|(key, value)| {
            let key = key
                .unpack_str()
                .ok_or_else(|| anyhow::anyhow!("{name} keys must be strings"))?;
            let value = value
                .unpack_str()
                .ok_or_else(|| anyhow::anyhow!("{name} values must be strings"))?;
            Ok((CompactString::new(key), CompactString::new(value)))
        })
        .collect()
}

fn default_info_files<'v>(
    value: Value<'v>,
    path: &str,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> Result<AnalysisDepset, String> {
    let value = lowerer.lower(value, path)?;
    let AnalysisValueKind::Depset(files) = value.kind() else {
        return Err("DefaultInfo.files must be the result of depset([...])".to_owned());
    };
    for value in files.to_list() {
        match value.kind() {
            AnalysisValueKind::Artifact(AnalysisArtifact::Source(_)) => {}
            AnalysisValueKind::Artifact(AnalysisArtifact::Derived { output, .. })
                if output.kind() == ActionOutputKind::File => {}
            AnalysisValueKind::Artifact(AnalysisArtifact::Derived { .. }) => {
                return Err("DefaultInfo.files depset must contain regular files".to_owned());
            }
            _ => return Err("DefaultInfo.files must be a depset of Files".to_owned()),
        }
    }
    Ok(files.clone())
}

fn default_info_executable(
    value: Value<'_>,
    owner: &AnalysisConfiguredTargetKey,
) -> Result<AnalysisArtifact, String> {
    AnalysisArtifactValue::from_starlark(value)
        .filter(|value| {
            value
                .output_for_owner(owner)
                .is_some_and(|output| output.kind() == ActionOutputKind::File)
        })
        .map(|value| value.artifact().clone())
        .ok_or_else(|| "DefaultInfo.executable must be a declared file".to_owned())
}

fn predeclared_output_artifacts(
    attributes: &[ResolvedRuleAttribute],
    package_path: &str,
    owner: &AnalysisConfiguredTargetKey,
) -> Vec<AnalysisArtifact> {
    attributes
        .iter()
        .flat_map(|attribute| match (&attribute.kind, &attribute.value) {
            (AttributeKind::Output, CoercedAttributeValue::Output(label)) => vec![label],
            (AttributeKind::OutputList, CoercedAttributeValue::OutputList(labels)) => {
                labels.iter().collect()
            }
            _ => Vec::new(),
        })
        .map(|label| {
            predeclared_file(label, package_path, owner)
                .artifact()
                .clone()
        })
        .collect()
}

fn lower_default_info<'v>(
    fields: StarlarkDefaultInfoFields<'v>,
    index: usize,
    attributes: &[ResolvedRuleAttribute],
    package_path: &str,
    owner: &AnalysisConfiguredTargetKey,
    test_rule: bool,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> Result<DefaultInfo, String> {
    if fields.runfiles.is_some()
        && (fields.default_runfiles.is_some() || fields.data_runfiles.is_some())
    {
        return Err(
            "DefaultInfo.runfiles cannot be combined with default_runfiles or data_runfiles"
                .to_owned(),
        );
    }
    let executable = fields
        .executable
        .map(|value| default_info_executable(value, owner))
        .transpose()?;
    let files = fields
        .files
        .map(|value| default_info_files(value, &format!("$[{index}].files"), lowerer))
        .transpose()?
        .map(Ok)
        .unwrap_or_else(|| {
            let mut artifacts = predeclared_output_artifacts(attributes, package_path, owner);
            artifacts.extend(executable.iter().cloned());
            AnalysisDepset::new(
                DepsetOrder::Default,
                artifacts.into_iter().map(AnalysisValue::artifact).collect(),
                Vec::new(),
            )
        })
        .map_err(|error| error.to_string())?;

    let as_runfiles = |value: Value<'_>, name: &str| {
        retained_runfiles(value)
            .cloned()
            .ok_or_else(|| format!("DefaultInfo.{name} must be a runfiles value"))
    };
    let legacy_runfiles = fields.runfiles.is_some()
        || (fields.default_runfiles.is_none() && fields.data_runfiles.is_none());
    let (default_runfiles, data_runfiles) = if legacy_runfiles {
        let mut runfiles = fields
            .runfiles
            .map(|value| as_runfiles(value, "runfiles"))
            .transpose()?
            .unwrap_or_else(RetainedRunfiles::empty);
        if let Some(executable) = &executable {
            runfiles = runfiles
                .with_artifact(executable.clone())
                .map_err(|error| error.to_string())?;
        }
        (runfiles.clone(), runfiles)
    } else {
        let mut default_runfiles = fields
            .default_runfiles
            .map(|value| as_runfiles(value, "default_runfiles"))
            .transpose()?
            .unwrap_or_else(RetainedRunfiles::empty);
        let data_runfiles = fields
            .data_runfiles
            .map(|value| as_runfiles(value, "data_runfiles"))
            .transpose()?
            .unwrap_or_else(RetainedRunfiles::empty);
        if (executable.is_some() || test_rule)
            && let Some(executable) = &executable
        {
            default_runfiles = default_runfiles
                .with_artifact(executable.clone())
                .map_err(|error| error.to_string())?;
        }
        (default_runfiles, data_runfiles)
    };
    DefaultInfo::from_effective(files, default_runfiles, data_runfiles, executable)
        .map_err(|error| error.to_string())
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
    runfiles_packages: RunfilesPackageDepset,
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
    let typed_configuration = key
        .configuration()
        .slug_configuration()
        .map(|configuration| -> Result<_, String> {
            Ok((
                configuration
                    .configured_action_path_flavor()
                    .map_err(|error| error.to_string())?,
                configuration
                    .configured_action_environment()
                    .map_err(|error| error.to_string())?,
            ))
        })
        .transpose();
    let support_configuration =
        typed_configuration
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|value| {
                value.as_ref().cloned().ok_or_else(|| {
                    "runfiles support requires structural action configuration".to_owned()
                })
            });
    let executable_provenance = Arc::new(executable_artifact_provenance(
        &dependencies,
        &configured_attributes,
    ));
    let execution_tags = target_execution_tags(&resolved_attributes);
    let resolved_attributes: Arc<[ResolvedRuleAttribute]> = resolved_attributes.into();
    let action_sink: Arc<dyn AnalysisActionSink> = Arc::new(SynchronousAnalysisActionSink {
        actions: actions.clone(),
        package_path: package_path.to_owned(),
        owner: retained_owner.clone(),
        typed_configuration,
        executable_provenance,
        execution_tags,
    });
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
            action_sink.clone(),
            cpp_fragment,
            implementation.source_identities_by_filename().clone(),
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
            action_sink,
            token: analysis_context.root_token(),
            retained_owner: retained_owner.clone(),
            target_label: key.label().clone(),
            package_path: package_path.to_owned(),
            dependencies,
            resolved_attributes: resolved_attributes.clone(),
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
        if let Some(fields) = StarlarkDefaultInfo::fields_from_value(value) {
            let test_rule = rule_capability.as_ref().is_some_and(|capability| {
                capability.test_kind == Some(slug_loading_v2::package::TestRuleKind::Test)
            });
            provider_values.push(ProviderValue::DefaultInfo(lower_default_info(
                fields,
                index,
                &resolved_attributes,
                package_path,
                &retained_owner,
                test_rule,
                &mut lowerer,
            )?));
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
    let mut actions = actions
        .lock()
        .map_err(|_| "ctx.actions state lock is poisoned".to_owned())?;
    let providers = complete_runfiles_support(
        providers,
        &mut actions,
        &retained_owner,
        &runfiles_packages,
        support_configuration,
    )?;
    let declared_outputs = providers
        .default_info()
        .expect("ProviderCollection validated DefaultInfo")
        .file_artifacts()
        .into_iter()
        .map(|artifact| artifact.path().into_owned())
        .collect();
    let actions = actions.registry().actions().to_vec();
    let result = ConfiguredNodeResult::new_rule(key, providers, rule_capability, runfiles_packages)
        .with_action_specs(actions, action_contexts)
        .map_err(LoadedRuleError::from)?;
    Ok(result.with_declared_outputs(declared_outputs))
}
