/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use dupe::Dupe;
use slug_build_api_v2::ProviderIdentity;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::FrozenModule;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::FileLoader;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::FrozenValue;
use starlark::values::list::FrozenListRef;

use crate::bzl_module::BzlModuleIdentity;
use crate::package::FrozenAspectDefinition;
use crate::package::FrozenRuleDefinition;
use crate::package::build_file_loading_globals;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;
use crate::provider::starlark_provider_identity;

const TOP_LEVELS: &[&str] = &[
    "testing",
    "coverage_common",
    "InstrumentedFilesInfo",
    "AnalysisFailureInfo",
    "AnalysisTestResultInfo",
];

fn owner(label: &str) -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse(label).unwrap(),
        workspace_path: label.into(),
        repository_mapping: Arc::from([]),
    }
}

fn freeze_with_loader(
    source: &str,
    identity: BzlModuleIdentity,
    loader: Option<&dyn FileLoader>,
) -> Result<FrozenModule, String> {
    let ast = AstModule::parse(
        identity.workspace_path.to_string_lossy().as_ref(),
        source.to_owned(),
        &Dialect::Bazel,
    )
    .map_err(|error| error.to_string())?;
    let module = Module::new();
    let context = BzlEvaluationContext::from_identity(identity);
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&context);
    if let Some(loader) = loader {
        evaluator.set_loader(loader);
    }
    evaluator
        .eval_module(ast, &loading_globals())
        .map_err(|error| error.to_string())?;
    drop(evaluator);
    module.freeze().map_err(|error| format!("{error:?}"))
}

fn freeze(source: &str) -> Result<FrozenModule, String> {
    freeze_with_loader(source, owner("@@//:testing_bootstrap_test.bzl"), None)
}

fn global(globals: &Globals, name: &str) -> FrozenValue {
    globals
        .iter()
        .find_map(|(candidate, value)| (candidate == name).then_some(value))
        .unwrap_or_else(|| panic!("missing global {name}"))
}

struct OneModuleLoader {
    path: &'static str,
    module: FrozenModule,
}

impl FileLoader for OneModuleLoader {
    fn load(&self, path: &str) -> starlark::Result<FrozenModule> {
        (path == self.path)
            .then(|| self.module.dupe())
            .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("unexpected load {path}")))
    }
}

#[test]
fn complete_bootstrap_inventory_is_bzl_only_and_process_stable() {
    let first = loading_globals();
    let second = loading_globals();
    let build = build_file_loading_globals();
    for name in TOP_LEVELS {
        assert!(first.names().any(|candidate| candidate.as_str() == *name));
        assert!(!build.names().any(|candidate| candidate.as_str() == *name));
        assert!(
            global(&first, name)
                .to_value()
                .ptr_eq(global(&second, name).to_value()),
            "{name}"
        );
    }

    let values = freeze(
        "TESTING_DIR = dir(testing)\nCOVERAGE_DIR = dir(coverage_common)\n\
         TESTING_KIND = type(testing)\nCOVERAGE_KIND = type(coverage_common)\n\
         I_KIND = type(InstrumentedFilesInfo)\nF_KIND = type(AnalysisFailureInfo)\n\
         R_KIND = type(AnalysisTestResultInfo)\nE_KIND = type(testing.ExecutionInfo)\n\
         I_REPR = repr(InstrumentedFilesInfo)\nF_REPR = repr(AnalysisFailureInfo)\n\
         R_REPR = repr(AnalysisTestResultInfo)\nE_REPR = repr(testing.ExecutionInfo)\n",
    )
    .unwrap();
    let strings = |name| {
        FrozenListRef::from_value(values.get(name).unwrap().value())
            .unwrap()
            .iter()
            .map(|value| value.to_value().unpack_str().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        strings("TESTING_DIR"),
        ["ExecutionInfo", "TestEnvironment", "analysis_test"]
    );
    assert_eq!(strings("COVERAGE_DIR"), ["instrumented_files_info"]);
    assert_eq!(
        values.get("TESTING_KIND").unwrap().unpack_str(),
        Some("testing")
    );
    assert_eq!(
        values.get("COVERAGE_KIND").unwrap().unpack_str(),
        Some("coverage_common")
    );
    for name in ["I_KIND", "F_KIND", "R_KIND", "E_KIND"] {
        assert_eq!(values.get(name).unwrap().unpack_str(), Some("Provider"));
    }
    for (binding, name) in [
        ("I_REPR", "InstrumentedFilesInfo"),
        ("F_REPR", "AnalysisFailureInfo"),
        ("R_REPR", "AnalysisTestResultInfo"),
        ("E_REPR", "ExecutionInfo"),
    ] {
        assert_eq!(
            values.get(binding).unwrap().unpack_str().unwrap(),
            format!("<function {name}>")
        );
    }
}

#[test]
fn top_level_identities_survive_freezing_and_imports() {
    let child =
        freeze("TESTING = testing\nCOVERAGE = coverage_common\nRESULT = AnalysisTestResultInfo\n")
            .unwrap();
    for (binding, global_name) in [
        ("TESTING", "testing"),
        ("COVERAGE", "coverage_common"),
        ("RESULT", "AnalysisTestResultInfo"),
    ] {
        assert!(
            child
                .get(binding)
                .unwrap()
                .value()
                .ptr_eq(global(&loading_globals(), global_name).to_value())
        );
    }
    let loader = OneModuleLoader {
        path: "//:child.bzl",
        module: child.dupe(),
    };
    let parent = freeze_with_loader(
        "load('//:child.bzl', 'TESTING', 'COVERAGE', 'RESULT')\n\
         IMPORTED_TESTING = TESTING\nIMPORTED_COVERAGE = COVERAGE\nIMPORTED_RESULT = RESULT\n",
        owner("@@//:parent.bzl"),
        Some(&loader),
    )
    .unwrap();
    for (parent_name, child_name) in [
        ("IMPORTED_TESTING", "TESTING"),
        ("IMPORTED_COVERAGE", "COVERAGE"),
        ("IMPORTED_RESULT", "RESULT"),
    ] {
        assert!(
            parent
                .get(parent_name)
                .unwrap()
                .value()
                .ptr_eq(child.get(child_name).unwrap().value())
        );
    }
}

#[test]
fn callable_matrix_fails_closed_and_lazy_coverage_is_side_effect_free() {
    for (source, diagnostic) in [
        (
            "X = testing.ExecutionInfo()",
            "ExecutionInfo construction is unsupported",
        ),
        (
            "X = AnalysisTestResultInfo()",
            "AnalysisTestResultInfo construction is unsupported",
        ),
        (
            "X = testing.TestEnvironment()",
            "testing.TestEnvironment is unsupported",
        ),
        (
            "X = testing.analysis_test()",
            "testing.analysis_test is unsupported",
        ),
        (
            "X = coverage_common.instrumented_files_info()",
            "coverage_common.instrumented_files_info is unsupported",
        ),
    ] {
        let error = freeze(source).unwrap_err();
        assert!(error.contains(diagnostic), "{error}");
    }
    for source in ["X = InstrumentedFilesInfo()", "X = AnalysisFailureInfo()"] {
        let error = freeze(source).unwrap_err();
        assert!(
            error.contains("Operation `call()` not supported on type `Provider`"),
            "{error}"
        );
        assert!(!error.contains("configured analysis semantics"), "{error}");
    }

    let lazy = freeze(
        r#"events = []
def _sh_executable_impl(ctx):
    info = coverage_common.instrumented_files_info(
        ctx,
        source_attributes = ["srcs"],
        dependency_attributes = ["deps", "_runfiles_dep", "data"],
    )
    events.append(info)
    return [info]
SH_EXECUTABLE_IMPL = _sh_executable_impl
"#,
    )
    .unwrap();
    assert!(
        FrozenListRef::from_value(lazy.get("events").unwrap().value())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        lazy.get("SH_EXECUTABLE_IMPL").unwrap().value().get_type(),
        "function"
    );
}

#[test]
fn builtin_provider_identity_flows_through_rule_and_aspect_provides() {
    let module = freeze(
        "PROVIDERS = [testing.ExecutionInfo, InstrumentedFilesInfo, AnalysisFailureInfo, AnalysisTestResultInfo]\n\
         def _rule_impl(ctx): return []\n\
         def _aspect_impl(target, ctx): return []\n\
         EXECUTION = testing.ExecutionInfo\n\
         sample_rule = rule(implementation = _rule_impl, provides = PROVIDERS)\n\
         sample_aspect = aspect(implementation = _aspect_impl, provides = PROVIDERS)\n",
    )
    .unwrap();
    let expected = [
        ProviderIdentity::builtin("ExecutionInfo"),
        ProviderIdentity::builtin("InstrumentedFilesInfo"),
        ProviderIdentity::builtin("AnalysisFailureInfo"),
        ProviderIdentity::builtin("AnalysisTestResultInfo"),
    ];
    let rule = module
        .get("sample_rule")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    let aspect = module
        .get("sample_aspect")
        .unwrap()
        .downcast::<FrozenAspectDefinition>()
        .unwrap();
    assert_eq!(rule.advertised_providers(), expected);
    assert_eq!(aspect.advertised_providers.as_ref(), expected);

    for (binding, expected) in [
        ("InstrumentedFilesInfo", "InstrumentedFilesInfo"),
        ("AnalysisFailureInfo", "AnalysisFailureInfo"),
        ("AnalysisTestResultInfo", "AnalysisTestResultInfo"),
    ] {
        let value = global(&loading_globals(), binding);
        assert_eq!(
            starlark_provider_identity(value.to_value()),
            Some(ProviderIdentity::builtin(expected))
        );
    }
    assert_eq!(
        starlark_provider_identity(module.get("EXECUTION").unwrap().value()),
        Some(ProviderIdentity::builtin("ExecutionInfo"))
    );
}
