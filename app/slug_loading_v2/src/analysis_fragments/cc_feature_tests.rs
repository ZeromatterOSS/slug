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

const FILENAME: &str = "cpp_features.bzl";
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
        BzlModuleSourceProvenance::new(identity, [0; 32]),
    )]);
    module.set(
        "cpp",
        module.heap().alloc_simple(CppFragmentValue::new(
            CppFragmentProjection::new(configuration(changed))?,
            callers,
        )),
    );
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(FILENAME, source.to_owned(), &Dialect::Bazel)
                .map_err(starlark::Error::into_anyhow)?,
            &Globals::standard(),
        )
        .map_err(starlark::Error::into_anyhow)?;
    Ok(module.get("RESULT").unwrap().to_repr())
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
