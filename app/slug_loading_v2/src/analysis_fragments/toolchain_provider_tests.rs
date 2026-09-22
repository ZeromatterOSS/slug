//! Public configuration methods and current-caller stamp authorization.
use std::path::PathBuf;

use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::NativeCommandOption;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::ActionEnvironmentHost;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::BzlModuleIdentity;
use crate::BzlModuleSourceProvenance;
use crate::cc_compile_outputs::alloc_configuration;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;

fn configured(minimum: Option<&str>, stamp: bool) -> SlugConfiguration {
    let base = SlugConfiguration::default_target(
        &HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap()
        .with_action_environment_host(ActionEnvironmentHost::without_environment(
            ActionEnvironmentHostOs::Linux,
        )),
    )
    .unwrap();
    let mut options = vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::Stamp,
        Some(if stamp { "true" } else { "false" }),
        false,
    )];
    if let Some(minimum) = minimum {
        options.push(CommandConfigurationOccurrence::native(
            NativeCommandOption::MinimumOsVersion,
            Some(minimum),
            false,
        ));
    }
    base.with_command_configuration(
        base.starlark_options().clone(),
        &CommandConfigurationOverlay::from(options),
    )
    .unwrap()
}

fn evaluate_module(
    configuration: SlugConfiguration,
    allowed: bool,
    source: &str,
    imported: Option<&FrozenModule>,
) -> anyhow::Result<FrozenModule> {
    const FILE: &str = "toolchain_config_methods.bzl";
    let module = Module::new();
    if let Some(imported) = imported {
        module.import_public_symbols(imported);
    }
    module.set(
        "configuration",
        alloc_configuration(module.heap(), configuration.clone())?,
    );
    module.set(
        "cpp",
        module
            .heap()
            .alloc_simple(CppFragmentValue::new(CppFragmentProjection::new(
                configuration,
            )?)),
    );
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse(if allowed {
            "@@rules_cc+//cc/private/rules_impl:toolchain_config_methods.bzl"
        } else {
            "@@//ordinary:toolchain_config_methods.bzl"
        })
        .unwrap(),
        workspace_path: PathBuf::from(FILE),
        repository_mapping: Arc::from([]),
    };
    let context = BzlEvaluationContext::macro_runtime_context(
        identity.clone(),
        Arc::from([(
            CompactString::new(FILE),
            BzlModuleSourceProvenance::new(identity, [0; 32]),
        )]),
    );
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse(FILE, source.to_owned(), &Dialect::Bazel)
            .map_err(starlark::Error::into_anyhow)?,
        &loading_globals(),
    )
    .map_err(starlark::Error::into_anyhow)?;
    drop(eval);
    module.freeze().map_err(anyhow::Error::from)
}

fn evaluate(
    configuration: SlugConfiguration,
    allowed: bool,
    source: &str,
) -> anyhow::Result<String> {
    let module = evaluate_module(configuration, allowed, source, None)?;
    Ok(module.get("result")?.value().to_repr())
}

#[test]
fn toolchain_configuration_public_methods_preserve_nullable_plain_text() {
    for (minimum, expected) in [
        (None, "None"),
        (Some(""), "\"\""),
        (Some("null"), "\"null\""),
        (Some("-wololoo"), "\"-wololoo\""),
    ] {
        let configuration = configured(minimum, false);
        assert_eq!(
            evaluate(
                configuration.clone(),
                false,
                "result = [cpp.minimum_os_version(), configuration.is_tool_configuration()]"
            )
            .unwrap(),
            format!("[{expected}, False]")
        );
        for call in [
            "cpp.minimum_os_version('bad')",
            "configuration.is_tool_configuration(True)",
        ] {
            assert!(evaluate(configuration.clone(), false, &format!("result = {call}")).is_err());
        }
        assert_eq!(evaluate(configuration, false,
            "result = [hasattr(cpp, 'minimum_os_version'), hasattr(configuration, 'is_tool_configuration')]").unwrap(), "[True, True]");
    }
}

#[test]
fn stamp_configuration_method_checks_current_caller_even_for_bound_method() {
    for stamp in [false, true] {
        let configuration = configured(Some("13.0"), stamp);
        let source =
            "f = configuration.stamp_binaries\ndef call():\n    return f()\nresult = call()";
        assert_eq!(
            evaluate(configuration.clone(), true, source).unwrap(),
            if stamp { "True" } else { "False" }
        );
        let error = evaluate(configuration, false, source)
            .unwrap_err()
            .to_string();
        assert!(error.contains("private API"), "{error}");
    }
    let trusted = evaluate_module(
        configured(None, true),
        true,
        "bound = configuration.stamp_binaries",
        None,
    )
    .unwrap();
    assert_eq!(
        evaluate_module(
            configured(None, false),
            true,
            "def call():\n    return bound()\nresult = call()",
            Some(&trusted)
        )
        .unwrap()
        .get("result")
        .unwrap()
        .value()
        .to_repr(),
        "True"
    );
    let error = evaluate_module(
        configured(None, false),
        false,
        "def call():\n    return bound()\nresult = call()",
        Some(&trusted),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("private API"), "{error}");
}

#[test]
fn configuration_methods_observe_both_exec_resets_and_restore_target_values() {
    let host_like = SlugConfiguration::default_host_like(
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
    assert_eq!(
        evaluate(
            host_like,
            false,
            "result = configuration.is_tool_configuration()"
        )
        .unwrap(),
        "False"
    );
    let target = configured(Some("-wololoo"), true);
    let platform = CanonicalLabel::parse("@@//platforms:exec").unwrap();
    for exec in [
        target.to_exec().unwrap(),
        target.to_exec_for_platform(&platform).unwrap(),
    ] {
        assert_eq!(
            evaluate(
                exec.clone(),
                false,
                "result = [cpp.minimum_os_version(), configuration.is_tool_configuration()]"
            )
            .unwrap(),
            "[None, True]"
        );
        assert_eq!(
            evaluate(
                exec,
                true,
                "def call():\n    return configuration.stamp_binaries()\nresult = call()"
            )
            .unwrap(),
            "False"
        );
    }
    assert_eq!(evaluate(target, true,
        "def call():\n    return [cpp.minimum_os_version(), configuration.is_tool_configuration(), configuration.stamp_binaries()]\nresult = call()").unwrap(), "[\"-wololoo\", False, True]");
}
