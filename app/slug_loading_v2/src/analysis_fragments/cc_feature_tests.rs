//! Real typed Cpp projection and private-call boundaries, not authored defaults.
use std::path::PathBuf;

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
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::BzlModuleIdentity;
use crate::BzlModuleSourceProvenance;
use crate::provider::BzlEvaluationContext;

const FILENAME: &str = "cpp_features.bzl";

#[test]
fn retained_cpp_fragment_preserves_identity_and_uses_current_caller() {
    let retained =
        RetainedCppFragment::new(CppFragmentProjection::new(configuration(true)).unwrap());
    let frozen_heap = FrozenHeap::new();
    let cpp = alloc_frozen_cpp_fragment(&frozen_heap, retained.clone());
    let alias = alloc_frozen_cpp_fragment(&frozen_heap, retained.clone());
    let distinct = alloc_frozen_cpp_fragment(
        &frozen_heap,
        RetainedCppFragment::new(retained.projection().clone()),
    );
    assert!(cpp.to_value().equals(alias.to_value()).unwrap());
    assert!(!cpp.to_value().equals(distinct.to_value()).unwrap());
    assert_eq!(
        cpp.to_value().get_hashed().unwrap().hash(),
        alias.to_value().get_hashed().unwrap().hash()
    );
    for allowed in [true, false, true] {
        let module = Module::new();
        module.set("cpp", cpp.to_value());
        let identity = BzlModuleIdentity {
            label: CanonicalLabel::parse(if allowed {
                "@@rules_cc+//cc/private:consumer.bzl"
            } else {
                "@@//ordinary:consumer.bzl"
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
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        let result = evaluator.eval_module(
            AstModule::parse(
                FILENAME,
                "def query():\n    return cpp.force_pic()\nRESULT = query()".to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::standard(),
        );
        if allowed {
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(module.get("RESULT").unwrap().unpack_bool(), Some(true));
        } else {
            assert!(result.unwrap_err().to_string().contains("private API"));
        }
    }
}
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
    let inputs = [
        (NativeCommandOption::ForcePic, "true"),
        (NativeCommandOption::AppleGenerateDsym, "true"),
        (NativeCommandOption::ExperimentalUseLlvmCovmap, "true"),
        (
            NativeCommandOption::IncompatibleDontEnableHostNonhostCrosstoolFeatures,
            "false",
        ),
        (NativeCommandOption::FdoInstrument, "/instrument"),
        (NativeCommandOption::CsFdoInstrument, "null"),
        (NativeCommandOption::FdoPrefetchHints, "//profiles:hints"),
    ]
    .into_iter()
    .map(|(option, value)| CommandConfigurationOccurrence::native(option, Some(value), false))
    .collect::<Vec<_>>();
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("cpp-features").unwrap());
    base.with_command_configuration_context(
        base.starlark_options().clone(),
        &CommandConfigurationOverlay::from(inputs),
        OptionLabelContext::MainRepository { mapping: &mapping },
    )
    .unwrap()
}
fn evaluate(source: &str, changed: bool, allowed: bool) -> anyhow::Result<String> {
    evaluate_configuration(source, configuration(changed), allowed)
}
fn evaluate_configuration(
    source: &str,
    configuration: SlugConfiguration,
    allowed: bool,
) -> anyhow::Result<String> {
    let module = Module::new();
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse(if allowed {
            "@@rules_cc+//cc/private:cpp_features.bzl"
        } else {
            "@@//ordinary:cpp_features.bzl"
        })
        .unwrap(),
        workspace_path: PathBuf::from(FILENAME),
        repository_mapping: Arc::from([]),
    };
    let callers = Arc::from([(
        CompactString::new(FILENAME),
        BzlModuleSourceProvenance::new(identity.clone(), [0; 32]),
    )]);
    module.set(
        "cpp",
        module
            .heap()
            .alloc_simple(CppFragmentValue::new(CppFragmentProjection::new(
                configuration,
            )?)),
    );
    let context = BzlEvaluationContext::macro_runtime_context(identity, callers);
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&context);
    evaluator
        .eval_module(
            AstModule::parse(FILENAME, source.to_owned(), &Dialect::Bazel)
                .map_err(starlark::Error::into_anyhow)?,
            &Globals::standard(),
        )
        .map_err(starlark::Error::into_anyhow)?;
    Ok(module.get("RESULT").unwrap().to_repr())
}

fn link_configuration(changed: bool) -> SlugConfiguration {
    let base = configuration(false);
    if !changed {
        return base;
    }
    let inputs = [
        (NativeCommandOption::CompilationMode, "dbg"),
        (NativeCommandOption::Fission, "DBG"),
        (NativeCommandOption::Strip, "sometimes"),
        (NativeCommandOption::Linkopt, "-Wl,one,two"),
        (NativeCommandOption::Linkopt, ""),
        (NativeCommandOption::Linkopt, "-Wl,one,two"),
        (NativeCommandOption::ExperimentalOmitfp, "true"),
    ]
    .into_iter()
    .map(|(option, value)| CommandConfigurationOccurrence::native(option, Some(value), false))
    .collect::<Vec<_>>();
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("cpp-link-tests").unwrap());
    base.with_command_configuration_context(
        base.starlark_options().clone(),
        &CommandConfigurationOverlay::from(inputs),
        OptionLabelContext::MainRepository { mapping: &mapping },
    )
    .unwrap()
}

#[test]
fn cpp_link_projection_methods_use_owned_configuration_and_private_binding() {
    const READ: &str = "def read(): return (cpp.fission_active_for_current_compilation_mode(), cpp.should_strip_binaries())\nRESULT=read()";
    assert_eq!(
        evaluate_configuration(READ, link_configuration(false), true).unwrap(),
        "(False, True)"
    );
    assert_eq!(
        evaluate_configuration(READ, link_configuration(true), true).unwrap(),
        "(True, False)"
    );
    assert_eq!(
        evaluate_configuration(READ, link_configuration(false), true).unwrap(),
        "(False, True)"
    );
    for method in [
        "fission_active_for_current_compilation_mode",
        "should_strip_binaries",
    ] {
        let read = format!("def read(): return cpp.{method}()\nRESULT=read()");
        let error = evaluate_configuration(&read, link_configuration(true), false)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("cannot use private API"),
            "{method}: {error}"
        );
        for call in [
            format!("cpp.{method}(1)"),
            format!("cpp.{method}(unexpected=True)"),
        ] {
            let source = format!("def read(): return {call}\nRESULT=read()");
            assert!(
                evaluate_configuration(&source, link_configuration(true), true).is_err(),
                "{call}"
            );
        }
        // Taking a bound method is allowed; calling it still checks its caller.
        let copied = format!("bound=cpp.{method}\ndef read(): return bound()\nRESULT=read()");
        assert!(
            evaluate_configuration(&copied, link_configuration(true), false)
                .unwrap_err()
                .to_string()
                .contains("cannot use private API")
        );
        assert!(evaluate_configuration(&copied, link_configuration(true), true).is_ok());
    }
}

#[test]
fn cpp_linkopts_is_public_immutable_noncallable_and_keeps_raw_order() {
    const READ: &str = "RESULT=(type(cpp.linkopts), cpp.linkopts)";
    assert_eq!(
        evaluate_configuration(READ, link_configuration(false), false).unwrap(),
        "(\"list\", [])"
    );
    assert_eq!(
        evaluate_configuration(READ, link_configuration(true), false).unwrap(),
        "(\"list\", [\"-Wl,one,two\", \"\", \"-Wl,one,two\", \"-Wl,--eh-frame-hdr\"])"
    );
    for source in [
        "RESULT=cpp.linkopts()",
        "v=cpp.linkopts\nv.append('bad')\nRESULT=v",
        "v=cpp.linkopts\nv[0]='bad'\nRESULT=v",
    ] {
        assert!(
            evaluate_configuration(source, link_configuration(true), false).is_err(),
            "{source}"
        );
    }
    assert_eq!(
        evaluate_configuration(
            "RESULT='linkopts' in dir(cpp)",
            link_configuration(true),
            false
        )
        .unwrap(),
        "True"
    );
    assert_eq!(
        evaluate_configuration(READ, link_configuration(false), false).unwrap(),
        "(\"list\", [])"
    );
}

#[test]
fn cpp_feature_facade_projects_real_default_and_nondefault_options() {
    let source = "def read():\n    label = cpp._fdo_prefetch_hints_label\n    return (cpp.force_pic(), cpp.apple_generate_dsym, cpp._dont_enable_host_nonhost, cpp.use_llvm_coverage_map_format(), cpp.fdo_instrument(), cpp.cs_fdo_instrument(), str(label) if label != None else None)\nRESULT=read()";
    assert_eq!(
        evaluate(source, false, true).unwrap(),
        "(False, False, True, False, None, None, None)"
    );
    assert_eq!(
        evaluate(source, true, true).unwrap(),
        "(True, True, False, True, \"/instrument\", \"null\", \"@@//profiles:hints\")"
    );
    assert_eq!(
        evaluate(source, false, true).unwrap(),
        "(False, False, True, False, None, None, None)"
    );
}

#[test]
fn cpp_feature_facade_preserves_public_attributes_and_restricted_methods() {
    let public = "def read(): return (cpp.apple_generate_dsym, cpp._dont_enable_host_nonhost, cpp._fdo_prefetch_hints_label)\nRESULT=read()";
    assert_eq!(
        evaluate(public, false, false).unwrap(),
        "(False, True, None)"
    );
    for method in [
        "force_pic",
        "fdo_instrument",
        "cs_fdo_instrument",
        "use_llvm_coverage_map_format",
    ] {
        let source = format!("def read(): return cpp.{method}()\nRESULT=read()");
        let error = evaluate(&source, true, false).unwrap_err().to_string();
        assert!(
            error.contains("cannot use private API"),
            "{method}: {error}"
        );
        if matches!(method, "force_pic" | "fdo_instrument") {
            assert!(
                error.contains(&format!("feature '{method}' in CppConfiguration")),
                "{error}"
            );
        }
        let extra = format!("def read(): return cpp.{method}(1)\nRESULT=read()");
        assert!(evaluate(&extra, true, true).is_err());
    }
    for attribute in [
        "apple_generate_dsym",
        "_dont_enable_host_nonhost",
        "_fdo_prefetch_hints_label",
    ] {
        let source = format!("def read(): return cpp.{attribute}()\nRESULT=read()");
        assert!(evaluate(&source, true, true).is_err());
    }
    for deferred in ["objc_generate_linkmap", "objc_should_strip_binary"] {
        let source = format!("def read(): return cpp.{deferred}\nRESULT=read()");
        assert!(evaluate(&source, false, true).is_err());
    }
}
