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
    crate::starlark_label::attach_defining_module_context(&module, &identity)?;
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

fn configured_grte_top(spelling: &str) -> SlugConfiguration {
    let base = configured(None, false);
    base.with_command_configuration(
        base.starlark_options().clone(),
        &CommandConfigurationOverlay::from(vec![CommandConfigurationOccurrence::native(
            NativeCommandOption::GrteTop,
            Some(spelling),
            false,
        )]),
    )
    .unwrap()
}

#[test]
fn cpp_grte_top_method_preserves_nullable_label_shape_and_zero_arity() {
    // Bazel 9.2 CppConfigurationApi.java:173-179 declares a nullable Label
    // method; CppConfiguration.java:754-769 checks its private API caller.
    let default = configured(None, false);
    let label = configured_grte_top("//libc:discarded");
    let platform = CanonicalLabel::parse("@@//platforms:exec").unwrap();
    let reset = configured_grte_top("default");
    for configuration in [
        default.clone(),
        label.to_exec().unwrap(),
        label.to_exec_for_platform(&platform).unwrap(),
        reset.clone(),
    ] {
        assert_eq!(
            evaluate(
                configuration,
                true,
                "def call():\n    return [hasattr(cpp, 'grte_top'), cpp.grte_top()]\nresult = call()",
            )
            .unwrap(),
            "[True, None]"
        );
    }
    assert_eq!(default, reset);
    assert_eq!(
        evaluate(
            label.clone(),
            true,
            "def call():\n    value = cpp.grte_top()\n    return [type(value), str(value), value.name, value.package, value.repo_name, value.workspace_root, value == Label('@@//libc:everything')]\nresult = call()",
        )
        .unwrap(),
        "[\"Label\", \"@@//libc:everything\", \"everything\", \"libc\", \"\", \"\", True]"
    );
    for call in ["cpp.grte_top('bad')", "cpp.grte_top(unexpected = True)"] {
        assert!(evaluate(label.clone(), true, &format!("result = {call}")).is_err());
    }
}

#[test]
fn cpp_grte_top_method_checks_current_caller_even_when_transferred() {
    for (configuration, expected) in [
        (configured(None, false), "None"),
        (
            configured_grte_top("//trusted:discarded"),
            "Label(\"@@//trusted:everything\")",
        ),
    ] {
        let call = "def call():\n    return cpp.grte_top()\nresult = call()";
        assert_eq!(
            evaluate(configuration.clone(), true, call).unwrap(),
            expected
        );
        let error = evaluate(configuration.clone(), false, call)
            .unwrap_err()
            .to_string();
        assert!(error.contains("private API"), "{error}");

        let trusted = evaluate_module(
            configuration.clone(),
            true,
            "bound = cpp.grte_top\nsaved_cpp = cpp",
            None,
        )
        .unwrap();
        // Obtaining a bound method does not authorize its subsequent caller.
        evaluate_module(
            configured(None, false),
            false,
            "saved = bound",
            Some(&trusted),
        )
        .unwrap();
        let transferred_call = "def call():\n    return bound()\nresult = call()";
        let allowed = evaluate_module(
            configured_grte_top("//consumer"),
            true,
            transferred_call,
            Some(&trusted),
        )
        .unwrap();
        assert_eq!(allowed.get("result").unwrap().value().to_repr(), expected);
        let error = evaluate_module(
            configured(None, false),
            false,
            transferred_call,
            Some(&trusted),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("private API"), "{error}");
        // Separate facade occurrences remain distinct even over equal options;
        // the transferred facade preserves its own retained occurrence.
        for current in [configuration, configured_grte_top("//consumer")] {
            let compared = evaluate_module(
                current,
                true,
                "alias = saved_cpp\nresult = [alias == saved_cpp, cpp == saved_cpp]",
                Some(&trusted),
            )
            .unwrap();
            assert_eq!(
                compared.get("result").unwrap().value().to_repr(),
                "[True, False]"
            );
        }
    }
}

const PRIVATE_BOOLEAN_METHODS: [(NativeCommandOption, &str, bool); 5] = [
    (
        NativeCommandOption::ExperimentalCcImplementationDeps,
        "experimental_cc_implementation_deps",
        true,
    ),
    (NativeCommandOption::StartEndLib, "start_end_lib", true),
    (
        NativeCommandOption::IncompatibleUseSpecificToolFiles,
        "incompatible_use_specific_tool_files",
        true,
    ),
    (
        NativeCommandOption::IncompatibleRemoveLegacyWholeArchive,
        "incompatible_remove_legacy_whole_archive",
        true,
    ),
    (
        NativeCommandOption::ExperimentalSaveFeatureState,
        "save_feature_state",
        false,
    ),
];

fn configured_private_bool(option: NativeCommandOption, enabled: bool) -> SlugConfiguration {
    let base = configured(None, false);
    base.with_command_configuration(
        base.starlark_options().clone(),
        &CommandConfigurationOverlay::from(vec![CommandConfigurationOccurrence::native(
            option,
            Some(if enabled { "true" } else { "false" }),
            false,
        )]),
    )
    .unwrap()
}

fn configured_implementation_deps(enabled: bool) -> SlugConfiguration {
    configured_private_bool(
        NativeCommandOption::ExperimentalCcImplementationDeps,
        enabled,
    )
}

#[test]
fn cpp_implementation_deps_method_is_a_private_zero_argument_boolean() {
    // Bazel 9.2 CppConfiguration.java:468-491,782-796,866-883,934-937 and
    // CppOptions.java:150-163,790-799,819-837,970-979 own these independent methods.
    let platform = CanonicalLabel::parse("@@//platforms:exec").unwrap();
    for (option, name, default) in PRIVATE_BOOLEAN_METHODS {
        let source = format!("def call():\n    return cpp.{name}()\nresult = call()");
        assert_eq!(
            evaluate(configured(None, false), true, &source).unwrap(),
            if default { "True" } else { "False" }
        );
        for enabled in [false, true] {
            let target = configured_private_bool(option, enabled);
            let expected = if enabled { "True" } else { "False" };
            for configuration in [
                target.clone(),
                target.to_exec().unwrap(),
                target.to_exec_for_platform(&platform).unwrap(),
            ] {
                let source = format!(
                    "def call():\n    value = cpp.{name}()\n    return [hasattr(cpp, '{name}'), type(value), value]\nresult = call()"
                );
                assert_eq!(
                    evaluate(configuration.clone(), true, &source).unwrap(),
                    format!("[True, \"bool\", {expected}]")
                );
                for call in [
                    format!("cpp.{name}(True)"),
                    format!("cpp.{name}(unexpected = True)"),
                ] {
                    assert!(
                        evaluate(configuration.clone(), true, &format!("result = {call}")).is_err()
                    );
                }
            }
        }
    }
}

#[test]
fn cpp_implementation_deps_transferred_method_checks_current_caller() {
    for (option, name, _) in PRIVATE_BOOLEAN_METHODS {
        for enabled in [false, true] {
            let target = configured_private_bool(option, enabled);
            let expected = if enabled { "True" } else { "False" };
            let direct = format!("def call():\n    return cpp.{name}()\nresult = call()");
            assert_eq!(evaluate(target.clone(), true, &direct).unwrap(), expected);
            let error = evaluate(target.clone(), false, &direct)
                .unwrap_err()
                .to_string();
            assert!(error.contains("private API"), "{error}");
            let trusted =
                evaluate_module(target, true, &format!("bound = cpp.{name}"), None).unwrap();
            evaluate_module(
                configured_private_bool(option, !enabled),
                false,
                "saved = bound",
                Some(&trusted),
            )
            .unwrap();
            let invocation = "def call():\n    return bound()\nresult = call()";
            let allowed = evaluate_module(
                configured_private_bool(option, !enabled),
                true,
                invocation,
                Some(&trusted),
            )
            .unwrap();
            assert_eq!(allowed.get("result").unwrap().value().to_repr(), expected);
            let error = evaluate_module(
                configured_private_bool(option, !enabled),
                false,
                invocation,
                Some(&trusted),
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("private API"), "{error}");
        }
    }
}

#[test]
fn cpp_private_boolean_methods_preserve_independent_target_and_exec_values() {
    let platform = CanonicalLabel::parse("@@//platforms:exec").unwrap();
    for (implementation_deps, start_end_lib) in [(false, true), (true, false)] {
        for specific_tools in [false, true] {
            for remove_legacy in [false, true] {
                for save_feature_state in [false, true] {
                    let base = configured_implementation_deps(implementation_deps);
                    let target = base
                        .with_command_configuration(
                            base.starlark_options().clone(),
                            &CommandConfigurationOverlay::from(vec![
                                CommandConfigurationOccurrence::native(
                                    NativeCommandOption::StartEndLib,
                                    Some(if start_end_lib { "true" } else { "false" }),
                                    false,
                                ),
                                CommandConfigurationOccurrence::native(
                                    NativeCommandOption::IncompatibleUseSpecificToolFiles,
                                    Some(if specific_tools { "true" } else { "false" }),
                                    false,
                                ),
                                CommandConfigurationOccurrence::native(
                                    NativeCommandOption::IncompatibleRemoveLegacyWholeArchive,
                                    Some(if remove_legacy { "true" } else { "false" }),
                                    false,
                                ),
                                CommandConfigurationOccurrence::native(
                                    NativeCommandOption::ExperimentalSaveFeatureState,
                                    Some(if save_feature_state { "true" } else { "false" }),
                                    false,
                                ),
                            ]),
                        )
                        .unwrap();
                    let expected = format!(
                        "[{}, {}, {}, {}, {}]",
                        if implementation_deps { "True" } else { "False" },
                        if start_end_lib { "True" } else { "False" },
                        if specific_tools { "True" } else { "False" },
                        if remove_legacy { "True" } else { "False" },
                        if save_feature_state { "True" } else { "False" },
                    );
                    for configuration in [
                        target.clone(),
                        target.to_exec().unwrap(),
                        target.to_exec_for_platform(&platform).unwrap(),
                    ] {
                        assert_eq!(
                        evaluate(
                            configuration,
                            true,
                            "def call():\n    return [cpp.experimental_cc_implementation_deps(), cpp.start_end_lib(), cpp.incompatible_use_specific_tool_files(), cpp.incompatible_remove_legacy_whole_archive(), cpp.save_feature_state()]\nresult = call()",
                        )
                        .unwrap(),
                        expected,
                    );
                    }
                }
            }
        }
    }
}

#[test]
fn pinned_rules_cc_implementation_deps_guard_uses_the_owned_boolean() {
    // Verbatim rules_cc 0.2.17 cc/common/semantics.bzl:113-116 from the
    // existing pinned rustc-map-each fixture; no C++ dependency propagation.
    const GUARD: &str = r#"def _check_can_use_implementation_deps(ctx):
    experimental_cc_implementation_deps = ctx.fragments.cpp.experimental_cc_implementation_deps()
    if (not experimental_cc_implementation_deps and ctx.attr.implementation_deps):
        fail("requires --experimental_cc_implementation_deps", attr = "implementation_deps")
"#;
    for enabled in [false, true] {
        for nonempty in [false, true] {
            let entries = if nonempty { "[1]" } else { "[]" };
            let source = format!(
                "{GUARD}\ndef call():\n    ctx = struct(fragments = struct(cpp = cpp), attr = struct(implementation_deps = {entries}))\n    _check_can_use_implementation_deps(ctx)\n    return True\nresult = call()"
            );
            let result = evaluate(configured_implementation_deps(enabled), true, &source);
            if !enabled && nonempty {
                let error = result.unwrap_err().to_string();
                assert!(
                    error.contains("requires --experimental_cc_implementation_deps"),
                    "{error}"
                );
                assert!(error.contains("implementation_deps"), "{error}");
            } else {
                assert_eq!(result.unwrap(), "True");
            }
        }
    }
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
