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
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderValue;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::CoercedAttributeValue;
use slug_loading_v2::LoadedPackage;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::provider::StarlarkDefaultInfo;
use slug_loading_v2::provider::StarlarkToolchainInfo;
use slug_loading_v2::provider::ToolchainInfoAnalysisContext;
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
use starlark::values::list::ListRef;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

use crate::analysis_value::AnalysisArtifactValue;
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
    retained_owner: AnalysisConfiguredTargetKey,
    target_label: slug_identity_v2::CanonicalLabel,
    package_path: String,
    #[trace(unsafe_ignore)]
    dependencies: Arc<[AnalysisDependency]>,
    resolved_attributes: Arc<[ResolvedRuleAttribute]>,
    build_setting_value: Option<V>,
    toolchain: Option<PreparedToolchain>,
}

unsafe impl<'v> Coerce<AnalysisContextGen<Value<'v>>> for AnalysisContextGen<FrozenValue> {}

starlark_complex_value!(AnalysisContext);

impl<'v> Freeze for AnalysisContext<'v> {
    type Frozen = FrozenAnalysisContext;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(AnalysisContextGen {
            actions: self.actions,
            retained_owner: self.retained_owner,
            target_label: self.target_label,
            package_path: self.package_path,
            dependencies: self.dependencies,
            resolved_attributes: self.resolved_attributes,
            build_setting_value: self
                .build_setting_value
                .map(|value| value.freeze(freezer))
                .transpose()?,
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
        match attribute {
            "label" => Some(slug_loading_v2::provider::alloc_starlark_label(
                heap,
                self.target_label.clone(),
            )),
            "actions" => Some(heap.alloc_simple(AnalysisActions {
                actions: self.actions.clone(),
                package_path: self.package_path.clone(),
                owner: self.retained_owner.clone(),
            })),
            "attr" => Some(heap.alloc_simple(AnalysisAttributes {
                dependencies: self.dependencies.clone(),
                attributes: self.resolved_attributes.clone(),
            })),
            "outputs" => Some(heap.alloc_simple(AnalysisOutputs {
                attributes: self.resolved_attributes.clone(),
                package_path: self.package_path.clone(),
                owner: self.retained_owner.clone(),
            })),
            "toolchains" => self
                .toolchain
                .clone()
                .map(|toolchain| heap.alloc_simple(AnalysisToolchains(toolchain))),
            "build_setting_value" => self.build_setting_value.map(|value| value.to_value()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Allocative)]
pub(crate) struct PreparedDependency {
    pub(crate) key: ConfiguredNodeKey,
    pub(crate) providers: ProviderCollection,
    pub(crate) attribute: CompactString,
}

#[derive(Debug, Clone, Allocative)]
pub(crate) struct PreparedToolchain {
    pub(crate) required_type: CompactString,
    pub(crate) action_context: Arc<ConfiguredActionOwnerContext>,
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisAttributes {
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
struct AnalysisToolchains(PreparedToolchain);

impl fmt::Display for AnalysisToolchains {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<ctx.toolchains>")
    }
}

starlark::starlark_simple_value!(AnalysisToolchains);

#[starlark_value(type = "toolchains")]
impl<'v> StarlarkValue<'v> for AnalysisToolchains {
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        if index.unpack_str() != Some(self.0.required_type.as_str()) {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "ctx.toolchains only contains {}",
                self.0.required_type
            )));
        }
        let marker = heap
            .alloc_str(
                self.0
                    .action_context
                    .toolchain()
                    .expect("prepared toolchain context retains a toolchain")
                    .marker(),
            )
            .to_value();
        Ok(StarlarkToolchainInfo::alloc_value(
            heap,
            [(CompactString::new("marker"), marker)],
        ))
    }
}

#[derive(Debug, Clone, Allocative)]
struct AnalysisDependency {
    attribute: CompactString,
    target: FrozenValue,
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisActions {
    #[allocative(skip)]
    actions: Arc<Mutex<CtxActions>>,
    package_path: String,
    owner: AnalysisConfiguredTargetKey,
}

impl fmt::Display for AnalysisActions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<ctx.actions>")
    }
}

starlark::starlark_simple_value!(AnalysisActions);

#[starlark_module]
fn analysis_actions_methods(builder: &mut MethodsBuilder) {
    fn declare_file(this: Value, path: &str) -> anyhow::Result<AnalysisArtifactValue> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        let path = if actions.package_path.is_empty() {
            path.to_owned()
        } else {
            format!("{}/{}", actions.package_path, path)
        };
        let output = actions
            .actions
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions state lock is poisoned"))?
            .declare_file(path)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
            owner: actions.owner.clone(),
            output,
        }))
    }

    fn write(
        this: Value,
        output: Value,
        content: &str,
        #[starlark(default = false)] is_executable: bool,
    ) -> anyhow::Result<NoneType> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        let output = AnalysisArtifactValue::from_value(output)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.write requires a declared file"))?;
        let output = output
            .output_for_owner(&actions.owner)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.write requires a declared file"))?;
        actions
            .actions
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions state lock is poisoned"))?
            .write(output.clone(), content, is_executable)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(NoneType)
    }

    fn run_shell<'v>(
        this: Value<'v>,
        outputs: Value<'v>,
        command: &str,
        arguments: Value<'v>,
        heap: Heap<'v>,
    ) -> anyhow::Result<NoneType> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        let mut declared = Vec::new();
        for item in outputs
            .iterate(heap)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?
        {
            let file = AnalysisArtifactValue::from_value(item).ok_or_else(|| {
                anyhow::anyhow!("ctx.actions.run_shell outputs must be declared files")
            })?;
            declared.push(
                file.output_for_owner(&actions.owner)
                    .ok_or_else(|| {
                        anyhow::anyhow!("ctx.actions.run_shell outputs must be declared files")
                    })?
                    .clone(),
            );
        }
        let output = declared
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.run_shell requires at least one output"))?;
        let mut args = Vec::new();
        for item in arguments
            .iterate(heap)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?
        {
            args.push(item.to_str());
        }
        actions
            .actions
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions state lock is poisoned"))?
            .run_shell(output, command, args, Vec::new())
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(NoneType)
    }
}

#[starlark_value(type = "analysis_actions")]
impl<'v> StarlarkValue<'v> for AnalysisActions {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(analysis_actions_methods)
    }
}

/// Synchronously evaluate one loaded rule after DICE has prepared all direct
/// dependency providers. No graph lookup or asynchronous work occurs here.
pub(crate) fn evaluate_loaded_rule(
    package: &LoadedPackage,
    target_name: &str,
    key: ConfiguredTargetKey,
    package_path: &str,
    dependencies: Vec<PreparedDependency>,
    resolved_attributes: Vec<ResolvedRuleAttribute>,
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
    let retained_owner = AnalysisConfiguredTargetKey::new(
        key.label().clone(),
        key.configuration().complete_identity_bytes(),
    );
    let dependencies = {
        let mut materializer = AnalysisValueMaterializer::new(module.frozen_heap());
        dependencies
            .into_iter()
            .map(|dependency| {
                let target =
                    materializer.configured_dependency(&dependency.key, dependency.providers)?;
                Ok(AnalysisDependency {
                    attribute: dependency.attribute,
                    target,
                })
            })
            .collect::<Result<Arc<[_]>, String>>()?
    };
    let returned = {
        let toolchain_info_context = ToolchainInfoAnalysisContext;
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&toolchain_info_context);
        if let Some(print_handler) = print_handler {
            evaluator.set_print_handler(print_handler);
        }
        let build_setting_value = build_setting_value
            .as_ref()
            .map(|value| build_setting::alloc_value(value, module.heap(), &mut evaluator))
            .transpose()
            .map_err(|error| error.to_string())?;
        let context = module.heap().alloc(AnalysisContextGen {
            actions: actions.clone(),
            retained_owner: retained_owner.clone(),
            target_label: key.label().clone(),
            package_path: package_path.to_owned(),
            dependencies,
            resolved_attributes: resolved_attributes.into(),
            build_setting_value,
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
                    AnalysisArtifactValue::from_value(executable)
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
