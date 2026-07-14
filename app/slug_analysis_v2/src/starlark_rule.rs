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
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::Depset;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderValue;
use slug_loading_v2::LoadedPackage;
use slug_loading_v2::PackageTargetKind;
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
            _ => None,
        }
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
    #[allocative(skip)]
    output: ActionOutput,
}

impl fmt::Display for DeclaredFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.output.path())
    }
}

starlark::starlark_simple_value!(DeclaredFile);

#[starlark_value(type = "declared_file")]
impl<'v> StarlarkValue<'v> for DeclaredFile {}

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
}

#[starlark_value(type = "analysis_actions")]
impl<'v> StarlarkValue<'v> for AnalysisActions {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(analysis_actions_methods)
    }
}

/// Evaluate one previously loaded custom rule using a prepared analysis
/// context. This is deliberately limited to the first vertical's `ctx.label`,
/// `ctx.actions.declare_file`, and `ctx.actions.write` surface; target
/// configuration and package loading remain DICE-owned inputs at the caller.
pub fn analyze_loaded_rule(
    package: &LoadedPackage,
    target_name: &str,
    key: ConfiguredTargetKey,
    package_path: &str,
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
    let context = module.heap().alloc_simple(AnalysisContext {
        actions: actions.clone(),
        target_name: target.name.clone(),
        package_path: package_path.to_owned(),
    });
    Evaluator::new(&module)
        .eval_function(implementation.frozen_value().to_value(), &[context], &[])
        .map_err(|error| error.to_string())?;

    let actions = actions
        .lock()
        .map_err(|_| "ctx.actions state lock is poisoned".to_owned())?
        .registry()
        .actions()
        .to_vec();
    let declared_outputs = actions
        .iter()
        .flat_map(|action| action.outputs())
        .map(|output| output.path().to_owned())
        .collect::<Vec<_>>();
    let files = Depset::from_direct(DepsetOrder::Default, declared_outputs.clone())
        .map_err(|error| error.to_string())?;
    let providers = ProviderCollection::new(vec![ProviderValue::DefaultInfo(
        DefaultInfo::from_files(files),
    )])
    .map_err(|error| error.to_string())?;

    Ok(AnalysisResult::new(key, providers)
        .with_actions(actions)
        .with_declared_outputs(declared_outputs))
}
