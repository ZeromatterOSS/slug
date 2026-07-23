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
use slug_loading_v2::provider::StarlarkUserProvider;
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

use crate::key::ConfiguredTargetKey;
use crate::result::AnalysisResult;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisContext {
    #[allocative(skip)]
    actions: Arc<Mutex<CtxActions>>,
    target_name: String,
    package_path: String,
    dependencies: Arc<[PreparedDependency]>,
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
            })),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Allocative)]
pub(crate) struct PreparedDependency {
    pub(crate) key: ConfiguredTargetKey,
    pub(crate) providers: ProviderCollection,
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisAttributes {
    dependencies: Arc<[PreparedDependency]>,
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
        (attribute == "deps").then(|| {
            heap.alloc(
                self.dependencies
                    .iter()
                    .cloned()
                    .map(AnalysisDependency)
                    .collect::<Vec<_>>(),
            )
        })
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
) -> Result<AnalysisResult, String> {
    let target = package
        .targets
        .iter()
        .find(|target| target.name == target_name)
        .ok_or_else(|| format!("target `{target_name}` was not found in loaded package"))?;
    let PackageTargetKind::StarlarkRule(implementation) = &target.kind else {
        return Err(format!("target `{target_name}` is not a Starlark rule"));
    };

    let actions = Arc::new(Mutex::new(CtxActions::new()));
    let module = Module::new();
    let direct_dependencies = dependencies
        .iter()
        .map(|dependency| dependency.key.clone())
        .collect();
    let context = module.heap().alloc_simple(AnalysisContext {
        actions: actions.clone(),
        target_name: target.name.clone(),
        package_path: package_path.to_owned(),
        dependencies: dependencies.into(),
    });
    let returned = Evaluator::new(&module)
        .eval_function(implementation.frozen_value().to_value(), &[context], &[])
        .map_err(|error| error.to_string())?;

    let returned = ListRef::from_value(returned)
        .ok_or_else(|| "rule implementation must return a list of providers".to_owned())?;
    let mut provider_values = Vec::with_capacity(returned.len());
    for value in returned.iter() {
        if let Some(files) = StarlarkDefaultInfo::files_from_value(value) {
            let files = StarlarkDepset::direct_from_value(files).ok_or_else(|| {
                "DefaultInfo.files must be the result of depset([...])".to_owned()
            })?;
            let declared_outputs = files
                .iter()
                .map(|value| {
                    DeclaredFile::from_value(*value)
                        .map(|file| file.output.path().to_owned())
                        .ok_or_else(|| {
                            "DefaultInfo.files depset must contain declared files".to_owned()
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let files = slug_build_api_v2::Depset::from_direct(
                slug_build_api_v2::DepsetOrder::Default,
                declared_outputs,
            )
            .map_err(|error| error.to_string())?;
            provider_values.push(ProviderValue::DefaultInfo(
                slug_build_api_v2::DefaultInfo::from_files(files),
            ));
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
        } else {
            return Err(format!(
                "rule implementation returned non-provider value `{}`",
                value.to_repr()
            ));
        }
    }
    let providers = ProviderCollection::new(provider_values).map_err(|error| error.to_string())?;
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
    Ok(AnalysisResult::new(key, providers)
        .with_direct_dependencies(direct_dependencies)
        .with_actions(actions)
        .with_declared_outputs(declared_outputs))
}
