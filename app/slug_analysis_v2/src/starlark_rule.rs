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
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::UserProvider;
use slug_loading_v2::LoadedPackage;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::provider::FrozenUserProviderCallable;
use slug_loading_v2::provider::StarlarkDefaultInfo;
use slug_loading_v2::provider::StarlarkDepset;
use slug_loading_v2::provider::StarlarkToolchainInfo;
use slug_loading_v2::provider::StarlarkUserProvider;
use slug_loading_v2::provider::ToolchainInfoAnalysisContext;
use starlark::PrintHandler;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::list::ListRef;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

use crate::key::ConfiguredNodeKey;
use crate::key::ConfiguredTargetKey;
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

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisContext {
    #[allocative(skip)]
    actions: Arc<Mutex<CtxActions>>,
    target_name: String,
    package_path: String,
    dependencies: Arc<[PreparedDependency]>,
    build_setting_value: Option<CompactString>,
    marker: Option<CompactString>,
    toolchain: Option<PreparedToolchain>,
}

impl fmt::Display for AnalysisContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<analysis ctx>")
    }
}

starlark::starlark_simple_value!(AnalysisContext);

#[starlark_value(type = "analysis_ctx")]
impl<'v> StarlarkValue<'v> for AnalysisContext {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "label" => Some(heap.alloc_simple(AnalysisLabel {
                name: self.target_name.clone(),
            })),
            "actions" => Some(heap.alloc_simple(AnalysisActions {
                actions: self.actions.clone(),
                package_path: self.package_path.clone(),
            })),
            "attr" => Some(heap.alloc_simple(AnalysisAttributes {
                dependencies: self.dependencies.clone(),
                marker: self.marker.clone(),
            })),
            "toolchains" => self
                .toolchain
                .clone()
                .map(|toolchain| heap.alloc_simple(AnalysisToolchains(toolchain))),
            "build_setting_value" => self
                .build_setting_value
                .as_ref()
                .map(|value| heap.alloc_str(value).to_value()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Allocative)]
pub(crate) struct PreparedDependency {
    pub(crate) key: ConfiguredNodeKey,
    pub(crate) providers: ProviderCollection,
    pub(crate) attribute: CompactString,
    pub(crate) sequence: bool,
}

#[derive(Debug, Clone, Allocative)]
pub(crate) struct PreparedToolchain {
    pub(crate) required_type: CompactString,
    pub(crate) marker: CompactString,
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisAttributes {
    dependencies: Arc<[PreparedDependency]>,
    marker: Option<CompactString>,
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
        if attribute == "marker" {
            return self
                .marker
                .as_ref()
                .map(|marker| heap.alloc_str(marker).to_value());
        }
        let dependencies = self
            .dependencies
            .iter()
            .filter(|dependency| attribute == "deps" || dependency.attribute == attribute)
            .cloned()
            .map(AnalysisDependency)
            .collect::<Vec<_>>();
        (!dependencies.is_empty()).then(|| {
            if attribute != "deps" && !dependencies[0].0.sequence {
                heap.alloc_simple(dependencies.into_iter().next().expect("nonempty"))
            } else {
                heap.alloc(dependencies)
            }
        })
    }
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
        Ok(heap.alloc_simple(AnalysisToolchainInfo(self.0.clone())))
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisToolchainInfo(PreparedToolchain);

impl fmt::Display for AnalysisToolchainInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ToolchainInfo(...)")
    }
}

starlark::starlark_simple_value!(AnalysisToolchainInfo);

#[starlark_value(type = "ToolchainInfo")]
impl<'v> StarlarkValue<'v> for AnalysisToolchainInfo {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (attribute == "marker").then(|| heap.alloc_str(&self.0.marker).to_value())
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisDependency(PreparedDependency);

impl fmt::Display for AnalysisDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.key, f)
    }
}

starlark::starlark_simple_value!(AnalysisDependency);

#[starlark_value(type = "configured_target")]
impl<'v> StarlarkValue<'v> for AnalysisDependency {
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let callable = FrozenUserProviderCallable::from_value(index).ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "dependency provider lookup requires an exported provider constructor"
            ))
        })?;
        let provider = self.0.providers.user(callable.id()).ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "dependency {} does not provide {}",
                self.0.key,
                callable.id()
            ))
        })?;
        Ok(heap.alloc_simple(StarlarkUserProvider::new(
            provider.id.dupe(),
            provider.fields.clone(),
        )))
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (attribute == "label").then(|| heap.alloc_str(&self.0.key.label().to_string()).to_value())
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisLabel {
    name: String,
}

impl fmt::Display for AnalysisLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

starlark::starlark_simple_value!(AnalysisLabel);

#[starlark_value(type = "label")]
impl<'v> StarlarkValue<'v> for AnalysisLabel {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (attribute == "name").then(|| heap.alloc_str(&self.name).to_value())
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisActions {
    #[allocative(skip)]
    actions: Arc<Mutex<CtxActions>>,
    package_path: String,
}

impl fmt::Display for AnalysisActions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<ctx.actions>")
    }
}

starlark::starlark_simple_value!(AnalysisActions);

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct DeclaredFile {
    output: ActionOutput,
}

impl fmt::Display for DeclaredFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.output.path())
    }
}

starlark::starlark_simple_value!(DeclaredFile);

#[starlark_value(type = "declared_file")]
impl<'v> StarlarkValue<'v> for DeclaredFile {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        if attribute == "path" {
            Some(heap.alloc_str(self.output.path()).to_value())
        } else {
            None
        }
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        attribute == "path"
    }
}

#[starlark_module]
fn analysis_actions_methods(builder: &mut MethodsBuilder) {
    fn declare_file(this: Value, path: &str) -> anyhow::Result<DeclaredFile> {
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
        Ok(DeclaredFile { output })
    }

    fn write(this: Value, output: Value, content: &str) -> anyhow::Result<NoneType> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        let output = DeclaredFile::from_value(output)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.write requires a declared file"))?;
        actions
            .actions
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions state lock is poisoned"))?
            .write(output.output.clone(), content, false)
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
            let file = DeclaredFile::from_value(item).ok_or_else(|| {
                anyhow::anyhow!("ctx.actions.run_shell outputs must be declared files")
            })?;
            declared.push(file.output.clone());
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
    marker: Option<CompactString>,
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

    let actions = Arc::new(Mutex::new(CtxActions::new()));
    let module = Module::new();
    let context = module.heap().alloc_simple(AnalysisContext {
        actions: actions.clone(),
        target_name: target.name.clone(),
        package_path: package_path.to_owned(),
        dependencies: dependencies.into(),
        build_setting_value: implementation.is_root_string_build_setting().then(|| {
            key.configuration()
                .root_string_setting()
                .expect("root setting key carries value")
                .as_str()
                .into()
        }),
        marker,
        toolchain,
    });
    let returned = {
        let mut evaluator = Evaluator::new(&module);
        let toolchain_info_context = ToolchainInfoAnalysisContext;
        evaluator.extra = Some(&toolchain_info_context);
        if let Some(print_handler) = print_handler {
            evaluator.set_print_handler(print_handler);
        }
        let result =
            evaluator.eval_function(implementation.frozen_value().to_value(), &[context], &[]);
        drop(evaluator);
        result
    }
    .map_err(|error| error.to_string())?;

    let returned = ListRef::from_value(returned)
        .ok_or_else(|| "rule implementation must return a list of providers".to_owned())?;
    let mut provider_values = Vec::with_capacity(returned.len());
    for value in returned.iter() {
        if let Some((files, executable)) = StarlarkDefaultInfo::fields_from_value(value) {
            let files = files
                .map(|files| {
                    let files = StarlarkDepset::direct_from_value(files).ok_or_else(|| {
                        "DefaultInfo.files must be the result of depset([...])".to_owned()
                    })?;
                    let declared_outputs = files
                        .iter()
                        .map(|value| {
                            DeclaredFile::from_value(*value)
                                .map(|file| file.output.path().to_owned())
                                .ok_or_else(|| {
                                    "DefaultInfo.files depset must contain declared files"
                                        .to_owned()
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    slug_build_api_v2::Depset::from_direct(
                        slug_build_api_v2::DepsetOrder::Default,
                        declared_outputs,
                    )
                    .map_err(|error| error.to_string())
                })
                .transpose()?;
            let executable = executable
                .map(|executable| {
                    DeclaredFile::from_value(executable)
                        .map(|file| file.output.path().to_owned())
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
        } else if let Some(provider) = StarlarkUserProvider::from_value(value) {
            provider_values.push(ProviderValue::User(
                UserProvider::with_id(
                    provider.id().dupe(),
                    provider
                        .fields()
                        .iter()
                        .map(|(name, value)| (name.clone(), value.clone())),
                )
                .map_err(|error| error.to_string())?,
            ));
        } else if let Some(info) = StarlarkToolchainInfo::from_value(value) {
            provider_values.push(ProviderValue::ToolchainInfo(
                slug_build_api_v2::providers::ToolchainInfo::new(info.marker()),
            ));
        } else {
            return Err(format!(
                "rule implementation returned non-provider value `{}`",
                value.to_repr()
            )
            .into());
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
    Ok(
        ConfiguredNodeResult::new_rule(key, providers, rule_capability)
            .with_actions(actions)
            .with_declared_outputs(declared_outputs),
    )
}
