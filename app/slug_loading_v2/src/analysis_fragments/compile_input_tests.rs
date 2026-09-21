//! Pinned CppConfiguration, CcStarlarkInternal, and PathFragment input contracts.
use std::path::PathBuf;

use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::NativeCommandOption;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_identity_v2::OptionLabelContext;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::BzlModuleIdentity;
use crate::BzlModuleSourceProvenance;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;
use crate::subrule_invocation::AnalysisArtifactValue;

const FILENAME: &str = "cpp_compile_inputs.bzl";

fn configuration(changed: bool) -> SlugConfiguration {
    let base = SlugConfiguration::default_target(
        &HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap(),
    )
    .unwrap();
    if !changed {
        return base;
    }
    let occurrences = [
        (NativeCommandOption::Copt, "-first,second"),
        (NativeCommandOption::Copt, ""),
        (NativeCommandOption::Copt, "-first,second"),
        (NativeCommandOption::Conlyopt, "-C"),
        (NativeCommandOption::Cxxopt, "-CXX"),
        (NativeCommandOption::ExperimentalOmitfp, "true"),
        (NativeCommandOption::CcDotdFiles, "false"),
        (NativeCommandOption::SaveTemps, "true"),
        (NativeCommandOption::ProcessHeadersInDependencies, "true"),
    ]
    .into_iter()
    .map(|(option, value)| CommandConfigurationOccurrence::native(option, Some(value), false))
    .collect::<Vec<_>>();
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("compile-input-tests").unwrap());
    base.with_command_configuration_context(
        base.starlark_options().clone(),
        &CommandConfigurationOverlay::from(occurrences),
        OptionLabelContext::MainRepository { mapping: &mapping },
    )
    .unwrap()
}

fn internal() -> FrozenModule {
    let module = Module::new();
    let context = BzlEvaluationContext::new("@@rules_cc+//cc/private:compile_inputs.bzl");
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse(
            FILENAME,
            "i = cc_common.internal_DO_NOT_USE()".to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &loading_globals(),
    )
    .unwrap();
    drop(eval);
    module.freeze().unwrap()
}

fn evaluate(source: &str, changed: bool, allowed: bool) -> anyhow::Result<String> {
    let module = Module::new();
    module.import_public_symbols(&internal());
    let frozen_heap = FrozenHeap::new();
    module.set(
        "cpp",
        alloc_frozen_cpp_fragment(
            &frozen_heap,
            RetainedCppFragment::new(CppFragmentProjection::new(configuration(changed))?),
        )
        .to_value(),
    );
    let label = CanonicalLabel::parse("@@//pkg:source.cc").unwrap();
    module.set(
        "source",
        module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(AnalysisArtifact::Source(
                label.clone(),
            ))),
    );
    module.set("label", alloc_starlark_label(module.heap(), label));
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse(if allowed {
            "@@rules_cc+//cc/private:compile_inputs.bzl"
        } else {
            "@@//ordinary:compile_inputs.bzl"
        })
        .unwrap(),
        workspace_path: PathBuf::from(FILENAME),
        repository_mapping: Arc::from([]),
    };
    let context = BzlEvaluationContext::macro_runtime_context(
        identity.clone(),
        Arc::from([(
            CompactString::new(FILENAME),
            BzlModuleSourceProvenance::new(identity, [0; 32]),
        )]),
    );
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse(FILENAME, source.to_owned(), &Dialect::Bazel)
            .map_err(starlark::Error::into_anyhow)?,
        &loading_globals(),
    )
    .map_err(starlark::Error::into_anyhow)?;
    Ok(module.get("RESULT").unwrap().to_repr())
}

#[test]
fn public_compile_option_lists_are_immutable_noncallable_and_preserve_order() {
    assert_eq!(
        evaluate(
            "RESULT = [cpp.copts, cpp.conlyopts, cpp.cxxopts]",
            false,
            false
        )
        .unwrap(),
        "[[], [], []]"
    );
    assert_eq!(
        evaluate(
            "RESULT = [cpp.copts, cpp.conlyopts, cpp.cxxopts]",
            true,
            false
        )
        .unwrap(),
        "[[\"-fomit-frame-pointer\", \"-fasynchronous-unwind-tables\", \"-DNO_FRAME_POINTER\", \"-first,second\", \"\", \"-first,second\"], [\"-C\"], [\"-CXX\"]]"
    );
    for name in ["copts", "conlyopts", "cxxopts"] {
        assert!(
            evaluate(&format!("RESULT = cpp.{name}()"), true, false).is_err(),
            "{name}"
        );
        let error = evaluate(
            &format!("cpp.{name}.append('bad')\nRESULT = None"),
            true,
            false,
        )
        .unwrap_err();
        assert!(
            error.to_string().to_lowercase().contains("immutable"),
            "{error}"
        );
        assert_eq!(
            evaluate(&format!("RESULT = '{name}' in dir(cpp)"), false, false).unwrap(),
            "True"
        );
    }
}

#[test]
fn private_compile_flags_use_current_caller_and_zero_arg_binding() {
    let source = "def query():\n    return [cpp.should_generate_dotd_files(), cpp.save_temps(), cpp.process_headers_in_dependencies()]\nRESULT = query()";
    assert_eq!(
        evaluate(source, false, true).unwrap(),
        "[True, False, False]"
    );
    assert_eq!(evaluate(source, true, true).unwrap(), "[False, True, True]");
    for name in [
        "should_generate_dotd_files",
        "save_temps",
        "process_headers_in_dependencies",
    ] {
        let source = format!("def query():\n    return cpp.{name}()\nRESULT = query()");
        assert!(
            evaluate(&source, true, false)
                .unwrap_err()
                .to_string()
                .contains("private API")
        );
        for arguments in ["True", "unexpected = True"] {
            assert!(
                evaluate(
                    &format!("def query():\n    return cpp.{name}({arguments})\nRESULT = query()"),
                    true,
                    true
                )
                .is_err()
            );
        }
    }
}

#[test]
fn per_file_copts_requires_genuine_positional_inputs_without_private_gate() {
    // The internal module was obtained in allowed code; this invocation is an
    // ordinary current caller, as permitted by CcStarlarkInternal.perFileCopts.
    assert_eq!(
        evaluate(
            "RESULT = i.per_file_copts(cpp, source, label)",
            false,
            false
        )
        .unwrap(),
        "[]"
    );
    assert_eq!(
        evaluate("RESULT = i.per_file_copts(cpp, source, label)", true, false).unwrap(),
        "[]"
    );
    for expression in [
        "i.per_file_copts(struct(), source, label)",
        "i.per_file_copts(cpp, struct(path = 'source.cc'), label)",
        "i.per_file_copts(cpp, source, '//pkg:source.cc')",
        "i.per_file_copts(cpp, source, None)",
        "i.per_file_copts(cpp_configuration = cpp, source_file = source, label = label)",
        "i.per_file_copts(cpp, source)",
        "i.per_file_copts(cpp, source, label, False)",
    ] {
        assert!(
            evaluate(&format!("RESULT = {expression}"), false, false).is_err(),
            "{expression}"
        );
    }
    let error = evaluate(
        "i.per_file_copts(cpp, source, label).append('bad')\nRESULT = None",
        false,
        false,
    )
    .unwrap_err();
    assert!(
        error.to_string().to_lowercase().contains("immutable"),
        "{error}"
    );
}

#[test]
fn file_extension_uses_full_exec_path_and_source_scan_edges() {
    let module = Module::new();
    let heap = module.heap();
    let owner =
        AnalysisConfiguredTargetKey::new(CanonicalLabel::parse("@@//:owner").unwrap(), vec![1]);
    for (path, expected) in [
        ("plain", ""),
        (".h", ""),
        ("pkg/.h", "h"),
        ("archive.tar.gz", "gz"),
        ("name.", ""),
        ("a.b/plain", ""),
        ("ü.扩展", "扩展"),
        ("pkg/.hidden.h", "h"),
    ] {
        for kind in [ActionOutputKind::File, ActionOutputKind::Directory] {
            let value = AnalysisArtifactValue::new(AnalysisArtifact::Derived {
                owner: owner.clone(),
                output: ActionOutput::new(path, kind),
            });
            assert_eq!(
                value.get_attr("extension", heap).unwrap().unpack_str(),
                Some(expected),
                "{path}"
            );
            assert!(value.dir_attr().iter().any(|name| name == "extension"));
        }
        let source = AnalysisArtifactValue::new(AnalysisArtifact::Source(
            CanonicalLabel::parse(&format!("@@//:{path}")).unwrap(),
        ));
        assert_eq!(
            source.get_attr("extension", heap).unwrap().unpack_str(),
            Some(expected),
            "{path}"
        );
    }
    let external = AnalysisArtifactValue::new(AnalysisArtifact::Source(
        CanonicalLabel::parse("@@repo+//:.h").unwrap(),
    ));
    assert_eq!(
        external.get_attr("extension", heap).unwrap().unpack_str(),
        Some("h")
    );
    assert!(evaluate("RESULT = source.extension()", false, false).is_err());
    assert_eq!(
        evaluate("RESULT = source.extension", false, false).unwrap(),
        "\"cc\""
    );
}
