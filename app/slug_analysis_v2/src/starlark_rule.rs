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
use dupe::Dupe;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::ArtifactInputSource;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::DerivedArtifactRoot;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::RetainedArtifactInputs;
use slug_build_api_v2::RetainedCommandLine;
use slug_build_api_v2::RetainedRunfiles;
use slug_build_api_v2::RetainedSpawnInvocation;
use slug_build_api_v2::RunfilesConflictPolicy;
use slug_build_api_v2::RunfilesPackageDepset;
use slug_build_api_v2::RunfilesPackageMetadata;
use slug_build_api_v2::RunfilesSymlink;
use slug_build_api_v2::RunfilesSymlinkDepset;
use slug_build_api_v2::SpawnExecutable;
use slug_build_api_v2::SpawnSpec;
use slug_build_api_v2::SymlinkSpec;
use slug_build_api_v2::SymlinkTarget;
use slug_configuration_v2::AppleFragmentProjection;
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
use slug_loading_v2::analysis_fragments::AppleFragmentValue;
use slug_loading_v2::analysis_fragments::CoverageFragmentValue;
use slug_loading_v2::analysis_fragments::CppFragmentValue;
use slug_loading_v2::analysis_fragments::RuleFragmentCollection;
use slug_loading_v2::package::resolve_rule_definition_label;
use slug_loading_v2::provider::StarlarkDefaultInfo;
use slug_loading_v2::provider::StarlarkDefaultInfoFields;
use slug_loading_v2::provider::starlark_label;
use slug_loading_v2::rule_outputs::PredeclaredOutput;
use slug_loading_v2::subrule_invocation::AnalysisActionCallScope;
use slug_loading_v2::subrule_invocation::AnalysisActionSink;
use slug_loading_v2::subrule_invocation::AnalysisActions;
use slug_loading_v2::subrule_invocation::AnalysisArtifactValue;
use slug_loading_v2::subrule_invocation::AnalysisCallToken;
use slug_loading_v2::subrule_invocation::AnalysisEvaluationContext;
use slug_loading_v2::subrule_invocation::AnalysisSpawnInvocation;
use slug_loading_v2::subrule_invocation::AnalysisSpawnRequest;
use slug_loading_v2::subrule_invocation::AnalysisToolchainRequest;
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
use starlark::values::Demand;
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

use crate::analysis_value::AnalysisConfiguredTargetValue;
use crate::analysis_value::AnalysisValueLowerer;
use crate::analysis_value::AnalysisValueMaterializer;
use crate::analysis_value::lower_runfiles_symlink_depset;
use crate::analysis_value::materialize_runfiles;
use crate::analysis_value::retained_runfiles;
use crate::build_setting;
use crate::configured_attribute::ResolvedRuleAttribute;
use crate::exec_group::ConfiguredExecGroup;
use crate::execution_groups::ConfiguredExecGroupCollection;
use crate::execution_groups::parse_target_context_label;
use crate::files_to_run_spawn::ExecutableArtifactProvenance;
use crate::files_to_run_spawn::executable_artifact_provenance;
use crate::files_to_run_spawn::retained_invocation;
use crate::files_to_run_spawn::retained_tools;
use crate::key::ConfiguredNodeKey;
use crate::key::ConfiguredTargetKey;
use crate::result::ConfiguredActionOwnerContext;
use crate::result::ConfiguredNodeResult;
use crate::runfiles_support::complete_runfiles_support;

mod attribute_files;
mod cc_compile_action;
mod cc_context;
mod make_variables;
mod run_environment_info;
use cc_context::CcContextProjection;
pub(crate) use cc_context::selected_execution_os;

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
    #[trace(unsafe_ignore)]
    output_root: DerivedArtifactRoot,
    #[trace(unsafe_ignore)]
    workspace_status: Arc<slug_build_api_v2::WorkspaceStatusArtifacts>,
    #[trace(unsafe_ignore)]
    cc_context: CcContextProjection,
    target_label: slug_identity_v2::CanonicalLabel,
    package_path: String,
    #[trace(unsafe_ignore)]
    dependencies: Arc<[AnalysisDependency]>,
    executables: Arc<[AnalysisExecutable]>,
    attribute_file: FrozenValue,
    attribute_files: FrozenValue,
    make_variables: Result<FrozenValue, CompactString>,
    resolved_attributes: Arc<[ResolvedRuleAttribute]>,
    #[trace(unsafe_ignore)]
    predeclared_outputs: Arc<[PredeclaredOutput]>,
    build_setting_value: Option<V>,
    fragments: V,
    #[trace(unsafe_ignore)]
    toolchain: Option<PreparedAnalysisToolchains>,
    exec_groups: Option<PreparedAnalysisExecGroups>,
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
            output_root: self.output_root,
            workspace_status: self.workspace_status,
            cc_context: self.cc_context,
            target_label: self.target_label,
            package_path: self.package_path,
            dependencies: self.dependencies,
            executables: self.executables,
            attribute_file: self.attribute_file,
            attribute_files: self.attribute_files,
            make_variables: self.make_variables,
            resolved_attributes: self.resolved_attributes,
            predeclared_outputs: self.predeclared_outputs,
            build_setting_value: self
                .build_setting_value
                .map(|value| value.freeze(freezer))
                .transpose()?,
            fragments: self.fragments.freeze(freezer)?,
            toolchain: self.toolchain,
            exec_groups: self.exec_groups,
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
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn slug_loading_v2::cc_context::CcExecutionContext>(self);
        demand.provide_value::<&dyn slug_loading_v2::cc_context::CcCompileOutputContext>(self);
        demand.provide_value::<&dyn slug_loading_v2::cc_context::CcCompileActionContext>(self);
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        self.token.require_active(attribute, "rule context").ok()?;
        match attribute {
            "info_file" => Some(heap.alloc_simple(AnalysisArtifactValue::new(
                AnalysisArtifact::WorkspaceStatus(self.workspace_status.stable().clone()),
            ))),
            "version_file" => Some(heap.alloc_simple(AnalysisArtifactValue::new(
                AnalysisArtifact::WorkspaceStatus(self.workspace_status.volatile().clone()),
            ))),
            "bin_dir" => Some(slug_loading_v2::alloc_artifact_root(
                heap,
                slug_build_api_v2::AnalysisArtifactRoot::Derived(self.output_root.clone()),
            )),
            "genfiles_dir" => self
                .cc_context
                .merged_genfiles()
                .ok()
                .filter(|merged| *merged)
                .map(|_| {
                    slug_loading_v2::alloc_artifact_root(
                        heap,
                        slug_build_api_v2::AnalysisArtifactRoot::Derived(self.output_root.clone()),
                    )
                }),
            "features" => self.cc_context.features(false, heap).ok(),
            "disabled_features" => self.cc_context.features(true, heap).ok(),
            "configuration" => self.cc_context.configuration(heap).ok(),
            "workspace_name" => Some(heap.alloc_str("_main").to_value()),
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
            "executable" => Some(heap.alloc_simple(AnalysisExecutables {
                token: self.token.clone(),
                executables: self.executables.clone(),
            })),
            "file" => Some(self.attribute_file.to_value()),
            "files" => Some(self.attribute_files.to_value()),
            "outputs" => Some(heap.alloc_simple(AnalysisOutputs {
                token: self.token.clone(),
                attributes: self.resolved_attributes.clone(),
                predeclared_outputs: self.predeclared_outputs.clone(),
                package_path: self.package_path.clone(),
                owner: self.retained_owner.clone(),
                root: self.output_root.clone(),
            })),
            "toolchains" => self.toolchain.clone().map(|toolchain| {
                heap.alloc_simple(AnalysisToolchains {
                    token: self.token.clone(),
                    toolchains: toolchain,
                })
            }),
            "exec_groups" => self.exec_groups.clone().map(|groups| {
                heap.alloc_simple(AnalysisExecGroups {
                    token: self.token.clone(),
                    groups,
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
    #[starlark(attribute)]
    fn var<'v>(this: &AnalysisContext<'v>) -> anyhow::Result<Value<'v>> {
        this.token.require_active("var", "rule context")?;
        this.make_variables
            .as_ref()
            .map(|value| value.to_value())
            .map_err(|error| anyhow::anyhow!("{error}"))
    }

    fn coverage_instrumented<'v>(
        this: &AnalysisContext<'v>,
        target: Option<Value<'v>>,
    ) -> anyhow::Result<bool> {
        this.token
            .require_active("coverage_instrumented", "rule context")?;
        if let Some(target) = target {
            anyhow::ensure!(
                target.is_none() || AnalysisConfiguredTargetValue::from_value(target).is_some(),
                "ctx.coverage_instrumented target must be a Target or None"
            );
        }
        this.cc_context.coverage_instrumented()
    }

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
    pub(crate) filtered: bool,
    pub(crate) executable: Option<slug_build_api_v2::FilesToRunProvider>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedConfiguredAttribute {
    pub(crate) owner: Option<Arc<SubruleIdentity>>,
    pub(crate) user_name: Option<CompactString>,
    pub(crate) executable: bool,
    pub(crate) value: AnalysisValue,
}

#[derive(Debug, Clone, Allocative)]
pub(crate) struct PreparedToolchain {
    pub(crate) platform_cpu: CompactString,
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

#[derive(Debug, Clone, Allocative)]
struct PreparedAnalysisExecGroupRow {
    name: CompactString,
    toolchains: PreparedAnalysisToolchains,
}

#[derive(Debug, Clone, Allocative)]
struct PreparedAnalysisExecGroups {
    rows: Arc<[PreparedAnalysisExecGroupRow]>,
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisAttributes {
    #[allocative(skip)]
    token: AnalysisCallToken,
    dependencies: Arc<[AnalysisDependency]>,
    attributes: Arc<[ResolvedRuleAttribute]>,
}

#[derive(Debug, Clone, Allocative)]
struct AnalysisExecutable {
    attribute: CompactString,
    executable: Option<FrozenValue>,
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisExecutables {
    #[allocative(skip)]
    token: AnalysisCallToken,
    executables: Arc<[AnalysisExecutable]>,
}

impl fmt::Display for AnalysisExecutables {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<ctx.executable>")
    }
}

starlark::starlark_simple_value!(AnalysisExecutables);

#[starlark_value(type = "analysis_executables")]
impl<'v> StarlarkValue<'v> for AnalysisExecutables {
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        self.token
            .require_active(attribute, "rule context executables")
            .ok()?;
        let executable = self
            .executables
            .iter()
            .find(|candidate| candidate.attribute == attribute)?;
        Some(
            executable
                .executable
                .map_or_else(Value::new_none, FrozenValue::to_value),
        )
    }
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
    predeclared_outputs: Arc<[PredeclaredOutput]>,
    package_path: String,
    owner: AnalysisConfiguredTargetKey,
    root: DerivedArtifactRoot,
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
        let explicit = self.attributes.iter().find(|candidate| {
            candidate.declaration_name == attribute
                && matches!(
                    candidate.kind,
                    AttributeKind::Output | AttributeKind::OutputList
                )
        });
        if let Some(attribute) = explicit {
            return match (&attribute.kind, &attribute.value) {
                (AttributeKind::Output, CoercedAttributeValue::None) => Some(Value::new_none()),
                (AttributeKind::Output, CoercedAttributeValue::Output(label)) => {
                    Some(heap.alloc_simple(predeclared_file(
                        label,
                        &self.package_path,
                        &self.owner,
                        &self.root,
                    )))
                }
                (AttributeKind::OutputList, CoercedAttributeValue::OutputList(labels)) => Some(
                    heap.alloc(
                        labels
                            .iter()
                            .map(|label| {
                                predeclared_file(label, &self.package_path, &self.owner, &self.root)
                            })
                            .collect::<Vec<_>>(),
                    ),
                ),
                _ => None,
            };
        }
        self.predeclared_outputs
            .iter()
            .find(|output| output.key == attribute)
            .map(|output| {
                heap.alloc_simple(predeclared_file(
                    &output.label,
                    &self.package_path,
                    &self.owner,
                    &self.root,
                ))
            })
    }
}

fn predeclared_file(
    label: &slug_identity_v2::CanonicalLabel,
    package_path: &str,
    owner: &AnalysisConfiguredTargetKey,
    root: &DerivedArtifactRoot,
) -> AnalysisArtifactValue {
    let target = label.target().as_str();
    let path = if package_path.is_empty() {
        target.to_owned()
    } else {
        format!("{package_path}/{target}")
    };
    AnalysisArtifactValue::new(AnalysisArtifact::Derived {
        owner: owner.clone(),
        output: ActionOutput::new(
            root.clone(),
            slug_build_api_v2::repository_relative_output_path(
                owner.label().package().repo(),
                &path,
            ),
            ActionOutputKind::File,
        ),
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
            .map(|dependency| dependency.target.map(FrozenValue::to_value))
            .ok_or_else(|| {
                format!(
                    "resolved attribute `{}` is missing a prepared dependency",
                    attribute.declaration_name
                )
            })
    };
    Ok(match &attribute.value {
        CoercedAttributeValue::None => Value::new_none(),
        CoercedAttributeValue::Label(_) if attribute.sequence => {
            heap.alloc(dependency()?.into_iter().collect::<Vec<_>>())
        }
        CoercedAttributeValue::Label(_) => dependency()?.unwrap_or_else(Value::new_none),
        CoercedAttributeValue::LabelList(labels) => heap.alloc(
            labels
                .iter()
                .map(|_| dependency())
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
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
        CoercedAttributeValue::IntegerList(values) => {
            heap.alloc(values.iter().copied().collect::<Vec<_>>())
        }
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
                .map(|(key, _)| {
                    Ok(dependency()?.map(|target| (heap.alloc_str(key).to_value(), target)))
                })
                .collect::<Result<Vec<_>, String>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
        )),
        CoercedAttributeValue::LabelKeyedStringDict(values) => heap.alloc(AllocDict(
            values
                .iter()
                .map(|(_, value)| {
                    Ok(dependency()?.map(|target| (target, heap.alloc_str(value).to_value())))
                })
                .collect::<Result<Vec<_>, String>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
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
                                .collect::<Result<Vec<_>, String>>()?
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>(),
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

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisExecGroup {
    #[allocative(skip)]
    token: AnalysisCallToken,
    toolchains: PreparedAnalysisToolchains,
}

impl fmt::Display for AnalysisExecGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<execution group>")
    }
}

starlark::starlark_simple_value!(AnalysisExecGroup);

#[starlark_value(type = "exec_group")]
impl<'v> StarlarkValue<'v> for AnalysisExecGroup {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        self.token
            .require_active(attribute, "rule context execution group")
            .ok()?;
        match attribute {
            "toolchains" => Some(heap.alloc_simple(AnalysisToolchains {
                token: self.token.clone(),
                toolchains: self.toolchains.clone(),
            })),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisExecGroups {
    #[allocative(skip)]
    token: AnalysisCallToken,
    groups: PreparedAnalysisExecGroups,
}

impl fmt::Display for AnalysisExecGroups {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<ctx.exec_groups>")
    }
}

starlark::starlark_simple_value!(AnalysisExecGroups);

#[starlark_value(type = "exec_groups")]
impl<'v> StarlarkValue<'v> for AnalysisExecGroups {
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        self.token
            .require_active("[]", "rule context execution groups")?;
        let name = index.unpack_str().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!("ctx.exec_groups indices must be strings"))
        })?;
        let row = self
            .groups
            .rows
            .iter()
            .find(|row| row.name == name)
            .ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "ctx.exec_groups does not contain execution group '{name}'"
                ))
            })?;
        Ok(heap.alloc_simple(AnalysisExecGroup {
            token: self.token.clone(),
            toolchains: row.toolchains.clone(),
        }))
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        self.token
            .require_active("in", "rule context execution groups")?;
        let Some(name) = other.unpack_str() else {
            return Ok(false);
        };
        Ok(self.groups.rows.iter().any(|row| row.name == name))
    }
}

#[derive(Debug, Clone, Allocative)]
struct AnalysisDependency {
    attribute: CompactString,
    target: Option<FrozenValue>,
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
    action_context: &ConfiguredActionOwnerContext,
    definition_source: Arc<BzlModuleIdentity>,
    materialized: &mut SmallMap<slug_identity_v2::CanonicalLabel, FrozenValue>,
    materializer: &mut AnalysisValueMaterializer<'_>,
) -> Result<PreparedAnalysisToolchains, String> {
    let rows = action_context
        .toolchain()
        .into_iter()
        .flat_map(|context| context.rows())
        .map(|row| {
            let info = row
                .selected()
                .map(|selected| {
                    materialize_toolchain_info(
                        row.actual().label(),
                        selected.info(),
                        materialized,
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

fn exec_group_name(identity: &ConfiguredExecGroup) -> Option<CompactString> {
    match identity {
        ConfiguredExecGroup::Default => None,
        _ => Some(identity.runtime_name()),
    }
}

fn materialize_execution_group_views(
    groups: &ConfiguredExecGroupCollection,
    definition_source: Arc<BzlModuleIdentity>,
    materializer: &mut AnalysisValueMaterializer<'_>,
) -> Result<
    (
        Option<PreparedAnalysisToolchains>,
        Option<PreparedAnalysisExecGroups>,
    ),
    String,
> {
    let mut materialized = SmallMap::new();
    let root = if groups.use_auto_exec_groups() {
        let mut rows = Vec::new();
        for requirement in groups.default_declared_requirements() {
            let identity = ConfiguredExecGroup::automatic(requirement.label().clone());
            let row = groups.row(&identity).ok_or_else(|| {
                format!(
                    "automatic execution group for toolchain {} was not resolved",
                    requirement.label()
                )
            })?;
            rows.extend(
                materialize_analysis_toolchains(
                    row.action_context(),
                    definition_source.clone(),
                    &mut materialized,
                    materializer,
                )?
                .rows
                .iter()
                .cloned(),
            );
        }
        (!rows.is_empty()).then(|| PreparedAnalysisToolchains {
            definition_source: definition_source.clone(),
            rows: rows.into(),
        })
    } else {
        let default = groups
            .row(&ConfiguredExecGroup::Default)
            .expect("configured execution groups retain Default");
        default
            .action_context()
            .toolchain()
            .map(|_| {
                materialize_analysis_toolchains(
                    default.action_context(),
                    definition_source.clone(),
                    &mut materialized,
                    materializer,
                )
            })
            .transpose()?
    };
    let rows = groups
        .rows()
        .iter()
        .filter_map(|row| exec_group_name(row.identity()).map(|name| (name, row)))
        .map(|(name, row)| {
            Ok(PreparedAnalysisExecGroupRow {
                name,
                toolchains: materialize_analysis_toolchains(
                    row.action_context(),
                    definition_source.clone(),
                    &mut materialized,
                    materializer,
                )?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let exec_groups = (!rows.is_empty()).then(|| PreparedAnalysisExecGroups { rows: rows.into() });
    Ok((root, exec_groups))
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
    workspace_status: Arc<slug_build_api_v2::WorkspaceStatusArtifacts>,
    root: DerivedArtifactRoot,
    actions: Arc<Mutex<CtxActions<PendingRegisteredAction>>>,
    // Evaluation-scoped declarations, including predeclared outputs. Subrule
    // calls share this sink; only the resulting Derived artifacts survive it.
    declared_outputs: Mutex<SmallMap<String, ActionOutput>>,
    package_path: String,
    owner: AnalysisConfiguredTargetKey,
    typed_configuration: Result<Option<(HostPathFlavor, RetainedActionEnvironment)>, String>,
    executable_provenance: Arc<ExecutableArtifactProvenance>,
    execution_tags: CanonicalStringMap,
    target_package: Arc<RunfilesPackageMetadata>,
    exec_groups: ConfiguredExecGroupCollection,
}

impl SynchronousAnalysisActionSink {
    fn declare_output(
        &self,
        path: &str,
        kind: ActionOutputKind,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        let path = if self.package_path.is_empty() {
            path.to_owned()
        } else {
            format!("{}/{path}", self.package_path)
        };
        self.declare_bin_relative_output(
            slug_build_api_v2::repository_relative_output_path(
                self.owner.label().package().repo(),
                &path,
            ),
            kind,
        )
    }

    fn declare_bin_relative_output(
        &self,
        path: String,
        kind: ActionOutputKind,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        let output = ActionOutput::new(self.root.clone(), path.clone(), kind);
        slug_build_api_v2::actions::registry::validate_output(&output)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let mut declarations = self
            .declared_outputs
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions declaration lock is poisoned"))?;
        if let Some(previous) = declarations.get(&path) {
            if previous != &output {
                anyhow::bail!(
                    "output '{path}' has already been declared with a different type or metadata"
                );
            }
            return Ok(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
                owner: self.owner.clone(),
                output: previous.clone(),
            }));
        } else {
            declarations.insert(path, output.clone());
        }
        Ok(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
            owner: self.owner.clone(),
            output,
        }))
    }

    fn action_toolchain_label(
        &self,
        value: Value<'_>,
        operation: &str,
    ) -> anyhow::Result<slug_identity_v2::CanonicalLabel> {
        if let Some(label) = starlark_label(value) {
            return Ok(label);
        }
        let raw = value
            .unpack_str()
            .expect("toolchain binder admitted only labels and strings");
        parse_target_context_label(raw, &self.target_package)
            .map_err(|error| anyhow::anyhow!("ctx.actions.{operation} invalid toolchain: {error}"))
    }

    fn is_identifier(name: &str) -> bool {
        let mut chars = name.chars();
        chars
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
    }

    fn contains_unassociated_artifact(
        &self,
        value: &AnalysisValue,
        scope: &AnalysisActionCallScope,
        sequence_member: bool,
    ) -> bool {
        match value.kind() {
            AnalysisValueKind::Artifact(artifact) => {
                !self.executable_provenance.is_associated(scope, artifact)
            }
            AnalysisValueKind::List(values) | AnalysisValueKind::Tuple(values) => values
                .iter()
                .any(|value| self.contains_unassociated_artifact(value, scope, true)),
            AnalysisValueKind::Depset(values) => {
                sequence_member
                    || values
                        .to_list()
                        .iter()
                        .any(|value| self.contains_unassociated_artifact(value, scope, false))
            }
            AnalysisValueKind::Provider(provider) => {
                slug_build_api_v2::FilesToRunProvider::from_occurrence(provider).is_some_and(
                    |provider| !self.executable_provenance.is_associated_provider(&provider),
                )
            }
            _ => false,
        }
    }

    fn omitted_toolchain_is_ambiguous(
        &self,
        request: &AnalysisSpawnRequest<'_>,
    ) -> anyhow::Result<bool> {
        let executable = match request.invocation {
            AnalysisSpawnInvocation::Executable(value) => {
                AnalysisArtifactValue::from_starlark(value).is_some_and(|file| {
                    !self
                        .executable_provenance
                        .is_associated(&request.scope, file.artifact())
                }) || crate::analysis_value::files_to_run_provider(value).is_some_and(|provider| {
                    !self.executable_provenance.is_associated_provider(provider)
                })
            }
            AnalysisSpawnInvocation::Shell(_) => false,
        };
        if executable {
            return Ok(true);
        }
        let Some(tools) = request.tools.filter(|value| !value.is_none()) else {
            return Ok(false);
        };
        let mut lowerer = AnalysisValueLowerer::default();
        let tools = lowerer
            .lower(tools, "ctx.actions.run tools")
            .map_err(anyhow::Error::msg)?;
        Ok(self.contains_unassociated_artifact(&tools, &request.scope, false))
    }

    fn validate_subrule_action_overrides(
        &self,
        request: &AnalysisSpawnRequest<'_>,
        operation: &str,
    ) -> anyhow::Result<()> {
        if !matches!(request.scope, AnalysisActionCallScope::Subrule(_)) {
            return Ok(());
        }
        if request.exec_group.is_some() {
            anyhow::bail!("ctx.actions.{operation} exec_group must be None in a subrule");
        }
        if !matches!(request.toolchain, AnalysisToolchainRequest::Omitted) {
            anyhow::bail!("ctx.actions.{operation} toolchain may not be supplied in a subrule");
        }
        Ok(())
    }

    fn selected_action_group(
        &self,
        request: &AnalysisSpawnRequest<'_>,
        operation: &str,
    ) -> anyhow::Result<ConfiguredExecGroup> {
        if matches!(request.scope, AnalysisActionCallScope::Subrule(_)) {
            return Ok(ConfiguredExecGroup::Default);
        }
        let toolchain = match request.toolchain {
            AnalysisToolchainRequest::Value(value) => {
                Some(self.action_toolchain_label(value, operation)?)
            }
            _ => None,
        };
        let explicit = request
            .exec_group
            .map(|name| {
                let row = self.exec_groups.runtime_named(name).ok_or_else(|| {
                    anyhow::anyhow!("ctx.actions.{operation} unknown execution group '{name}'")
                })?;
                if !Self::is_identifier(name)
                    || !matches!(row.identity(), ConfiguredExecGroup::Named(_))
                {
                    anyhow::bail!("ctx.actions.{operation} invalid exec_group '{name}'");
                }
                Ok(row.identity().clone())
            })
            .transpose()?;
        if !self.exec_groups.use_auto_exec_groups() {
            return Ok(explicit.unwrap_or(ConfiguredExecGroup::Default));
        }
        match (explicit, request.toolchain, toolchain) {
            (Some(group), AnalysisToolchainRequest::Value(_), Some(label)) => {
                let row = self
                    .exec_groups
                    .row(&group)
                    .expect("validated named execution group remains present");
                if !row
                    .requirements()
                    .iter()
                    .any(|requirement| requirement.label() == &label)
                {
                    anyhow::bail!(
                        "ctx.actions.{operation} toolchain {label} is not a member of execution group '{}'",
                        request.exec_group.expect("explicit group is present")
                    );
                }
                Ok(group)
            }
            (Some(group), _, _) => Ok(group),
            (None, AnalysisToolchainRequest::Value(_), Some(label)) => {
                let group = ConfiguredExecGroup::automatic(label.clone());
                self.exec_groups.row(&group).map(|_| group).ok_or_else(|| {
                    anyhow::anyhow!("ctx.actions.{operation} unknown automatic toolchain {label}")
                })
            }
            (None, AnalysisToolchainRequest::None, _) => Ok(ConfiguredExecGroup::Default),
            (None, AnalysisToolchainRequest::Omitted, _) => {
                if self.exec_groups.rows().len() > 1
                    && self.omitted_toolchain_is_ambiguous(request)?
                {
                    anyhow::bail!(
                        "ctx.actions.{operation} requires an explicit toolchain or None for an unassociated executable or tool under automatic execution groups"
                    );
                }
                Ok(ConfiguredExecGroup::Default)
            }
            (_, AnalysisToolchainRequest::Value(_), None) => {
                unreachable!("value toolchain was parsed")
            }
        }
    }

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
        register: impl FnOnce(
            &mut CtxActions<PendingRegisteredAction>,
        ) -> Result<usize, slug_build_api_v2::ActionError>,
    ) -> anyhow::Result<()> {
        let mut actions = self
            .actions
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions state lock is poisoned"))?;
        register(&mut actions)
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }
}

impl AnalysisActionSink for SynchronousAnalysisActionSink {
    fn instrumented_files_info_empty<'v>(
        &self,
        ctx: Value<'v>,
        heap: &starlark::values::FrozenHeap,
    ) -> anyhow::Result<FrozenValue> {
        self.materialize_empty_instrumented_files(ctx, heap)
    }

    fn transform_build_info(
        &self,
        request: slug_loading_v2::build_info_transform::BuildInfoTransformRequest<'_>,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        self.register_build_info_transform(request)
    }
    fn prepare_cc_compile<'v>(
        &self,
        request: &slug_loading_v2::cc_compile_action::CcCompileRequest<'v>,
    ) -> anyhow::Result<slug_loading_v2::cc_compile_action::PreparedCcCompile<'v>> {
        self.prepare_native_cc_compile(request)
    }
    fn dynamic_library_symlink(
        &self,
        library: Value<'_>,
        solib_directory: &str,
        preserve_name: bool,
        prefix_consumer: bool,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        let input = AnalysisArtifactValue::from_starlark(library)
            .ok_or_else(|| anyhow::anyhow!("dynamic_library_symlink library must be a File"))?;
        let spec = slug_build_api_v2::SolibSymlinkSpec::new(
            input.artifact().clone(),
            self.root.clone(),
            solib_directory,
            self.owner.label(),
            preserve_name,
            prefix_consumer,
        )?;
        let output = self.declare_bin_relative_output(
            spec.output().root_relative_path().to_owned(),
            ActionOutputKind::File,
        )?;
        self.register(|actions| actions.register_solib_symlink(spec))?;
        Ok(output)
    }

    fn declare_file(
        &self,
        filename: &str,
        sibling: Option<&AnalysisArtifact>,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        let flavor = self
            .root
            .configuration()
            .configured_action_path_flavor()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        if flavor == HostPathFlavor::Windows {
            if sibling.is_some() {
                anyhow::bail!("ctx.actions.declare_file sibling on Windows is deferred");
            }
            return self.declare_output(filename, ActionOutputKind::File);
        }
        let package = slug_build_api_v2::repository_relative_output_path(
            self.owner.label().package().repo(),
            &self.package_path,
        );
        let package = package.trim_end_matches('/');
        let sibling_path = sibling.map(|artifact| match artifact {
            AnalysisArtifact::Derived { output, .. } => {
                std::borrow::Cow::Borrowed(output.root_relative_path())
            }
            AnalysisArtifact::WorkspaceStatus(status) => {
                std::borrow::Cow::Borrowed(status.short_path())
            }
            AnalysisArtifact::Source(_) => artifact.path(),
        });
        let directory = sibling_path.as_ref().map_or(package, |path| {
            path.rsplit_once('/').map_or("", |(parent, _)| parent)
        });
        let path = if filename.starts_with('/') || directory.is_empty() {
            filename.to_owned()
        } else {
            format!("{directory}/{filename}")
        };
        let path = NormalizedBazelPath::new(flavor, &path)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let path = path.as_str();
        if path.starts_with('/')
            || (!package.is_empty()
                && path != package
                && !path
                    .strip_prefix(package)
                    .is_some_and(|suffix| suffix.starts_with('/')))
        {
            anyhow::bail!(
                "the output artifact '{path}' is not under package directory '{package}' for target '{}'",
                self.owner.label()
            );
        }
        self.declare_bin_relative_output(path.to_owned(), ActionOutputKind::File)
    }

    fn declare_compile_output(&self, path: String) -> anyhow::Result<AnalysisArtifactValue> {
        self.declare_bin_relative_output(path, ActionOutputKind::File)
    }

    fn declare_directory(&self, path: &str) -> anyhow::Result<AnalysisArtifactValue> {
        self.declare_output(path, ActionOutputKind::Directory)
    }

    fn write<'v>(
        &self,
        module: &'v Module,
        output: Value<'v>,
        content: Value<'v>,
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
        let registration =
            slug_loading_v2::subrule_invocation::register_args_snapshot(module, snapshot)?;
        self.register(|actions| {
            actions.register_pending(PendingRegisteredAction::args_write(
                output,
                registration,
                is_executable,
            ))
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

    fn spawn<'v>(
        &self,
        module: &'v Module,
        request: AnalysisSpawnRequest<'v>,
    ) -> anyhow::Result<()> {
        let operation = match request.invocation {
            AnalysisSpawnInvocation::Executable(_) => "run",
            AnalysisSpawnInvocation::Shell(_) => "run_shell",
        };
        self.validate_subrule_action_overrides(&request, operation)?;
        let mut lowerer = AnalysisValueLowerer::default();
        let (command_line, has_arguments) = PendingCommandLine::capture(module, request.arguments)?;
        let (path_flavor, configured_environment) = self.typed_configuration()?;
        let invocation = retained_invocation(
            request.invocation,
            &request.scope,
            &self.executable_provenance,
            path_flavor,
            has_arguments,
        )?;
        let inputs = retained_artifact_inputs(request.inputs, false, true, "inputs", &mut lowerer)?;
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
        let selected_group = self.selected_action_group(&request, operation)?;
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
        validate_supported_spawn_options(&request, operation)?;
        let mut spec = SpawnSpec::new(
            invocation,
            RetainedCommandLine::new(Vec::new()),
            inputs,
            tools,
            outputs,
            unused_inputs_list,
            action_environment,
            execution_requirements,
            mnemonic,
            request.progress_message,
        );
        if let slug_loading_v2::subrule_invocation::AnalysisResourceSet::Recipe(recipe) =
            request.resource_set
        {
            spec = spec.with_resource_set(recipe);
        }
        let mut action = slug_build_api_v2::ActionSpec::spawn(spec);
        if let Some(name) = exec_group_name(&selected_group) {
            action = action.with_exec_group(name);
        }
        let pending = PendingRegisteredAction::spawn(action, command_line)?.with_inputs(
            module,
            request.inputs,
            request.tools,
            request.scope.clone(),
        )?;
        self.register(|actions| actions.register_pending(pending))
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
            progress_message.or(Some("Creating symlink %{output}")),
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

fn vector_scalar_value(
    value: Value<'_>,
    expand_directories: bool,
) -> anyhow::Result<slug_build_api_v2::RetainedScalarValue> {
    if let Some(value) = value.unpack_str() {
        return Ok(slug_build_api_v2::RetainedScalarValue::String(value.into()));
    }
    if value.get_type() == "int" {
        return Ok(slug_build_api_v2::RetainedScalarValue::Integer(
            value.to_str().into(),
        ));
    }
    if let Some(file) = AnalysisArtifactValue::from_starlark(value) {
        if expand_directories {
            reject_directory_file(file, "Args vector value")?;
        }
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
    allow_directories: bool,
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
            validate_input_artifacts(&retained, name, allow_directories)?;
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
                validate_input_artifact(
                    artifact,
                    &format!("ctx.actions.run {name}"),
                    allow_directories,
                )?;
                Ok(ArtifactInputSource::Direct(artifact.clone()))
            }
            AnalysisValueKind::Depset(depset) if allow_nested_depsets => {
                let retained = RetainedArtifactInputs::new(depset.clone())
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                validate_input_artifacts(&retained, name, allow_directories)?;
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

fn validate_input_artifact(
    artifact: &AnalysisArtifact,
    name: &str,
    allow_directories: bool,
) -> anyhow::Result<()> {
    if !allow_directories
        && matches!(
            artifact,
            AnalysisArtifact::Derived { output, .. } if output.kind() == ActionOutputKind::Directory
        )
    {
        anyhow::bail!("{name} must contain only regular Files")
    }
    Ok(())
}

fn validate_input_artifacts(
    inputs: &RetainedArtifactInputs,
    name: &str,
    allow_directories: bool,
) -> anyhow::Result<()> {
    let mut error = None;
    inputs
        .visit(|artifact| {
            if error.is_none() {
                error = validate_input_artifact(
                    artifact,
                    &format!("ctx.actions.run {name}"),
                    allow_directories,
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

fn validate_supported_spawn_options(
    request: &AnalysisSpawnRequest<'_>,
    operation: &str,
) -> anyhow::Result<()> {
    if request
        .shadowed_action
        .is_some_and(|value| !value.is_none())
    {
        anyhow::bail!("ctx.actions.{operation} shadowed_action is not supported");
    }
    if matches!(
        request.resource_set,
        slug_loading_v2::subrule_invocation::AnalysisResourceSet::Unsupported
    ) {
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
            AnalysisValueKind::Artifact(
                AnalysisArtifact::Source(_) | AnalysisArtifact::WorkspaceStatus(_),
            ) => {}
            AnalysisValueKind::Artifact(AnalysisArtifact::Derived { output, .. })
                if matches!(
                    output.kind(),
                    ActionOutputKind::File | ActionOutputKind::Directory
                ) => {}
            AnalysisValueKind::Artifact(AnalysisArtifact::Derived { .. }) => {
                return Err(
                    "DefaultInfo.files depset must contain files or declared directories"
                        .to_owned(),
                );
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
    predeclared_outputs: &[PredeclaredOutput],
    attributes: &[ResolvedRuleAttribute],
    package_path: &str,
    owner: &AnalysisConfiguredTargetKey,
    root: &DerivedArtifactRoot,
) -> Vec<AnalysisArtifact> {
    let mut outputs = predeclared_outputs
        .iter()
        .map(|output| &output.label)
        .collect::<Vec<_>>();
    for attribute in attributes {
        match (&attribute.kind, &attribute.value) {
            (AttributeKind::Output, CoercedAttributeValue::Output(label)) => outputs.push(label),
            (AttributeKind::OutputList, CoercedAttributeValue::OutputList(labels)) => {
                outputs.extend(labels.iter())
            }
            _ => {}
        }
    }
    outputs
        .into_iter()
        .map(|label| {
            predeclared_file(label, package_path, owner, root)
                .artifact()
                .clone()
        })
        .collect()
}

fn lower_default_info<'v>(
    fields: StarlarkDefaultInfoFields<'v>,
    index: usize,
    attributes: &[ResolvedRuleAttribute],
    predeclared_outputs: &[PredeclaredOutput],
    package_path: &str,
    owner: &AnalysisConfiguredTargetKey,
    root: &DerivedArtifactRoot,
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
            let mut artifacts = predeclared_output_artifacts(
                predeclared_outputs,
                attributes,
                package_path,
                owner,
                root,
            );
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
    workspace_status: Arc<slug_build_api_v2::WorkspaceStatusArtifacts>,
    target_name: &str,
    key: ConfiguredTargetKey,
    package_path: &str,
    dependencies: Vec<PreparedDependency>,
    resolved_attributes: Vec<ResolvedRuleAttribute>,
    configured_attributes: Vec<PreparedConfiguredAttribute>,
    rust_helpers: Arc<[slug_loading_v2::PreparedRustToolchainHelperBinding]>,
    exec_groups: ConfiguredExecGroupCollection,
    platform_cpu: CompactString,
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
    let validation_groups = crate::output_groups::collect_validation_groups(
        implementation.schema(),
        &dependencies,
        implementation
            .late_bound_rule_attributes()
            .map(|(name, _)| name),
    )?;
    let reserves_executable = rule_capability.as_ref().is_some_and(|cap| cap.executable);
    if let Some(output) = implementation.predeclared_outputs.iter().find(|output| {
        (reserves_executable && output.key == "executable")
            || resolved_attributes.iter().any(|attribute| {
                attribute.declaration_name == output.key
                    && matches!(
                        attribute.kind,
                        AttributeKind::Output | AttributeKind::OutputList
                    )
            })
    }) {
        return Err(format!("multiple outputs with the same key: {}", output.key).into());
    }
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

    let action_contexts = exec_groups
        .rows()
        .iter()
        .map(|row| row.action_context().clone())
        .collect::<Vec<_>>();
    let cc_context = CcContextProjection::new(
        package,
        key.configuration().slug_configuration(),
        &resolved_attributes,
        exec_groups
            .row(&ConfiguredExecGroup::Default)
            .expect("configured execution groups require a default row")
            .action_context()
            .clone(),
    )?;
    let output_root = DerivedArtifactRoot::bin(
        key.configuration()
            .slug_configuration()
            .ok_or_else(|| "derived outputs require structural configuration".to_owned())?
            .clone(),
    )?;
    let actions = Arc::new(Mutex::new(
        CtxActions::<PendingRegisteredAction>::new_typed(output_root.clone()),
    ));
    let module = Module::new();
    package.retain_bzl_heaps(module.frozen_heap());
    let make_variables = make_variables::prepare(
        module.frozen_heap(),
        key.configuration().slug_configuration(),
        &platform_cpu,
        &output_root,
        implementation,
        &dependencies,
        &exec_groups,
    );
    let needs_cpp_fragment = implementation
        .required_fragments()
        .iter()
        .any(|fragment| fragment == "cpp")
        || implementation
            .subrule_invocations()
            .any(|(_, _, _, fragments)| fragments.contains("cpp"));
    let needs_coverage_fragment = implementation
        .required_fragments()
        .iter()
        .any(|fragment| fragment == "coverage")
        || implementation
            .subrule_invocations()
            .any(|(_, _, _, fragments)| fragments.contains("coverage"));
    let needs_apple_fragment = implementation
        .required_fragments()
        .iter()
        .any(|fragment| fragment == "apple");
    let cpp_fragment = if needs_cpp_fragment {
        let structural_configuration =
            key.configuration().slug_configuration().ok_or_else(|| {
                "configured fragment projection requires structural configuration".to_owned()
            })?;
        module.frozen_heap().alloc(CppFragmentValue::new(
            CppFragmentProjection::new(structural_configuration.clone())
                .map_err(|error| error.to_string())?,
        ))
    } else {
        FrozenValue::new_none()
    };
    let coverage_fragment = if needs_coverage_fragment {
        let structural_configuration =
            key.configuration().slug_configuration().ok_or_else(|| {
                "configured fragment projection requires structural configuration".to_owned()
            })?;
        let output_generator = structural_configuration
            .coverage_output_generator_label()
            .map_err(|error| error.to_string())?;
        module
            .frozen_heap()
            .alloc(CoverageFragmentValue::new(output_generator))
    } else {
        FrozenValue::new_none()
    };
    let apple_fragment = if needs_apple_fragment {
        let structural_configuration =
            key.configuration().slug_configuration().ok_or_else(|| {
                "configured fragment projection requires structural configuration".to_owned()
            })?;
        module.frozen_heap().alloc(AppleFragmentValue::new(
            AppleFragmentProjection::new(structural_configuration.clone())
                .map_err(|error| error.to_string())?,
        ))
    } else {
        FrozenValue::new_none()
    };
    let retained_owner = crate::analysis_value::analysis_configured_key(&key);
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
    let predeclared_outputs = implementation.predeclared_outputs.clone();
    let action_sink: Arc<dyn AnalysisActionSink> = Arc::new(SynchronousAnalysisActionSink {
        workspace_status: workspace_status.clone(),
        root: output_root.clone(),
        actions: actions.clone(),
        declared_outputs: Mutex::new(
            predeclared_output_artifacts(
                &predeclared_outputs,
                &resolved_attributes,
                package_path,
                &retained_owner,
                &output_root,
            )
            .into_iter()
            .map(|artifact| match artifact {
                AnalysisArtifact::Derived { output, .. } => {
                    (output.root_relative_path().to_owned(), output)
                }
                AnalysisArtifact::Source(_) | AnalysisArtifact::WorkspaceStatus(_) => {
                    unreachable!("predeclared outputs are derived")
                }
            })
            .collect(),
        ),
        package_path: package_path.to_owned(),
        owner: retained_owner.clone(),
        typed_configuration,
        executable_provenance: executable_provenance.clone(),
        execution_tags,
        target_package: package.runfiles_package().clone(),
        exec_groups: exec_groups.clone(),
    });
    let (
        dependencies,
        executables,
        attribute_file,
        attribute_files,
        toolchain,
        prepared_exec_groups,
    ) = {
        let mut materializer =
            AnalysisValueMaterializer::new(module.frozen_heap()).with_rust_helpers(&rust_helpers);
        let (attribute_file, attribute_files) = attribute_files::materialize(
            implementation.schema(),
            &dependencies,
            &mut materializer,
        )?;
        let executables = implementation
            .schema()
            .iter()
            .filter(|schema| schema.executable())
            .map(|schema| {
                let executable = dependencies
                    .iter()
                    .find(|dependency| dependency.attribute == schema.declaration_name())
                    .and_then(|dependency| dependency.executable.as_ref())
                    .and_then(|provider| provider.executable.clone())
                    .map(|artifact| {
                        module
                            .frozen_heap()
                            .alloc(AnalysisArtifactValue::new(artifact))
                    });
                AnalysisExecutable {
                    attribute: CompactString::new(schema.declaration_name()),
                    executable,
                }
            })
            .collect::<Arc<[_]>>();
        let dependencies = dependencies
            .into_iter()
            .map(|dependency| {
                let target = if dependency.filtered {
                    None
                } else {
                    Some(
                        materializer
                            .configured_dependency_target(&dependency.key, dependency.providers)?,
                    )
                };
                Ok(AnalysisDependency {
                    attribute: dependency.attribute,
                    target,
                })
            })
            .collect::<Result<Arc<[_]>, String>>()?;
        let (toolchain, prepared_exec_groups) = materialize_execution_group_views(
            &exec_groups,
            implementation.definition_source().clone(),
            &mut materializer,
        )?;
        (
            dependencies,
            executables,
            attribute_file,
            attribute_files,
            toolchain,
            prepared_exec_groups,
        )
    };
    let prepared_subrules = {
        let mut materializer =
            AnalysisValueMaterializer::new(module.frozen_heap()).with_rust_helpers(&rust_helpers);
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
        let analysis_context = AnalysisEvaluationContext::new_with_coverage(
            implementation.direct_subrule_identities(),
            prepared_subrules,
            key.label().clone(),
            action_sink.clone(),
            cpp_fragment,
            coverage_fragment,
            crate::analysis_value::rust_toolchain::merge_helper_sources(
                implementation.source_identities_by_filename(),
                &rust_helpers,
            )?,
        )
        .with_rust_toolchain_helpers(rust_helpers.clone())
        .with_host_os(
            key.configuration()
                .slug_configuration()
                .and_then(|configuration| configuration.execution_host_os().ok()),
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
            apple_fragment,
            coverage_fragment,
        ));
        let context = module.heap().alloc(AnalysisContextGen {
            action_sink,
            workspace_status,
            cc_context,
            token: analysis_context.root_token(),
            retained_owner: retained_owner.clone(),
            output_root: output_root.clone(),
            target_label: key.label().clone(),
            package_path: package_path.to_owned(),
            dependencies,
            executables,
            attribute_file,
            attribute_files,
            make_variables,
            resolved_attributes: resolved_attributes.clone(),
            predeclared_outputs: predeclared_outputs.clone(),
            build_setting_value,
            fragments,
            toolchain,
            exec_groups: prepared_exec_groups,
        });
        let root_context = analysis_context
            .enter_root_context(&module, context)
            .map_err(|error| error.to_string())?;
        let result =
            evaluator.eval_function(implementation.frozen_value().to_value(), &[context], &[]);
        drop(root_context);
        drop(evaluator);
        result
    }
    .map_err(|error| error.to_string())?;

    let returned = build_info_transform::returned_providers(returned);
    let mut lowerer = AnalysisValueLowerer::from_module(&module);
    let mut provider_values = Vec::with_capacity(returned.len());
    for (index, value) in returned.into_iter().enumerate() {
        if slug_loading_v2::provider::is_default_xcode_version_info(value) {
            // The pinned default Xcode provider has callable fields. Retain its
            // validated semantic variant instead of asking the generic value
            // lowerer to serialize evaluator-local Starlark functions.
            provider_values.push(ProviderValue::XcodeVersionInfo(
                slug_build_api_v2::XcodeVersionInfo::default_empty_version(),
            ));
        } else if let Some(info) = slug_loading_v2::provider::run_environment_info(value) {
            run_environment_info::validate_declaration(rule_capability.as_ref())?;
            provider_values.push(ProviderValue::RunEnvironmentInfo(info));
        } else if let Some(info) = slug_loading_v2::provider::feature_flag_info(value) {
            provider_values.push(ProviderValue::FeatureFlagInfo(info));
        } else if let Some(fields) = StarlarkDefaultInfo::fields_from_value(value) {
            let test_rule = rule_capability.as_ref().is_some_and(|capability| {
                capability.test_kind == Some(slug_loading_v2::package::TestRuleKind::Test)
            });
            provider_values.push(ProviderValue::DefaultInfo(lower_default_info(
                fields,
                index,
                &resolved_attributes,
                &predeclared_outputs,
                package_path,
                &retained_owner,
                &output_root,
                test_rule,
                &mut lowerer,
            )?));
        } else {
            let lowered = lowerer.lower(value, &format!("$[{index}]"))?;
            if let AnalysisValueKind::PackageSpecificationInfo(info) = lowered.kind() {
                provider_values.push(ProviderValue::PackageSpecificationInfo(info.clone()));
                continue;
            }
            if let AnalysisValueKind::TemplateVariableInfo(info) = lowered.kind() {
                provider_values.push(ProviderValue::TemplateVariableInfo(info.dupe()));
                continue;
            }
            let AnalysisValueKind::Provider(provider) = lowered.kind() else {
                return Err(format!(
                    "rule implementation returned non-provider value `{}`",
                    value.to_repr()
                )
                .into());
            };
            if provider.identity().is_builtin("OutputGroupInfo") {
                let groups = slug_build_api_v2::OutputGroupInfo::from_occurrence(provider)
                    .map_err(|error| error.to_string())?;
                if groups.groups().contains_key("_validation_transitive") {
                    return Err("configured _validation_transitive override is unsupported"
                        .to_owned()
                        .into());
                }
                provider_values.push(ProviderValue::OutputGroupInfo(groups));
            } else {
                provider_values.push(ProviderValue::Occurrence(provider.clone()));
            }
        }
    }
    if !provider_values
        .iter()
        .any(|provider| matches!(provider, ProviderValue::DefaultInfo(_)))
    {
        let artifacts = predeclared_output_artifacts(
            &predeclared_outputs,
            &resolved_attributes,
            package_path,
            &retained_owner,
            &output_root,
        );
        let files = AnalysisDepset::new(
            DepsetOrder::Default,
            artifacts.into_iter().map(AnalysisValue::artifact).collect(),
            Vec::new(),
        )
        .map_err(|error| error.to_string())?;
        provider_values.push(ProviderValue::DefaultInfo(
            DefaultInfo::from_effective(
                files,
                RetainedRunfiles::empty(),
                RetainedRunfiles::empty(),
                None,
            )
            .map_err(|error| error.to_string())?,
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
    // Move phase-private registrations out of the sink before publication.
    // No callback inspection, evaluation or DICE work runs under this lock.
    let mut pending_actions = {
        let mut guard = actions
            .lock()
            .map_err(|_| "ctx.actions state lock is poisoned".to_owned())?;
        std::mem::replace(
            &mut *guard,
            CtxActions::<PendingRegisteredAction>::new_typed(output_root.clone()),
        )
    };
    cc_compile_action::finalize(&module, &mut lowerer, &mut pending_actions)
        .map_err(|error| error.to_string())?;
    let mut publisher = args_callbacks::Publisher::new(&module);
    let prepared = pending_actions
        .registry()
        .actions()
        .iter()
        .map(|action| {
            let inputs = action.lower_inputs(&module, &mut lowerer, &executable_provenance)?;
            action.prepare_with_inputs(&module, inputs, &mut |snapshot| {
                publisher.prepare(snapshot, &mut lowerer)
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    let graph = publisher
        .seal(&mut lowerer)
        .map_err(|error| error.to_string())?;
    drop(lowerer);
    let frozen_module = module
        .freeze()
        .map_err(|error| format!("Args publication freeze failed: {error:?}"))?;
    let frozen_roots = frozen_module
        .owned_extra_value()
        .ok_or_else(|| "missing frozen Args publication root".to_owned())?;
    let publisher = args_callbacks::FrozenPublisher::new(graph, frozen_roots);
    let mut prepared = prepared.into_iter();
    let mut actions = pending_actions
        .try_map::<slug_build_api_v2::ActionSpec, anyhow::Error>(|_| {
            prepared
                .next()
                .ok_or_else(|| anyhow::anyhow!("missing prepared action"))?
                .complete(&mut |draft| publisher.complete(draft))
        })
        .map_err(|error| error.to_string())?;
    let providers = complete_runfiles_support(
        providers,
        &mut actions,
        &retained_owner,
        &runfiles_packages,
        support_configuration,
    )?;
    let providers =
        crate::output_groups::complete_rule_output_groups(providers, validation_groups)?;
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

#[cfg(test)]
mod integer_list_projection_tests {
    use super::*;

    #[test]
    fn analysis_attribute_projects_signed_integer_list_in_order() {
        let module = Module::new();
        let attribute = ResolvedRuleAttribute {
            declaration_name: "numbers".into(),
            kind: AttributeKind::IntegerList,
            sequence: false,
            value: CoercedAttributeValue::IntegerList(Arc::from([1, -2, 3])),
        };
        let value = allocate_analysis_attribute(&attribute, &[], module.heap()).unwrap();
        assert_eq!(value.to_repr(), "[1, -2, 3]");
    }
}

mod args_callbacks;
mod build_info_transform;
use args_callbacks::pending::PendingCommandLine;
use args_callbacks::pending::PendingRegisteredAction;
mod coverage_common;

#[cfg(test)]
mod solib_tests;

#[cfg(test)]
mod context_lifecycle_tests;
