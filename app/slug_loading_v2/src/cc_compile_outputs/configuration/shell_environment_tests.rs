//! Pinned static environment attribute: mutable copies of fixed entries only.
use std::sync::Arc;

use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::NativeCommandOption;
use slug_configuration_v2::native::host::ActionEnvironmentHost;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use starlark::environment::FrozenModule;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

fn native(option: NativeCommandOption, value: &str) -> CommandConfigurationOccurrence {
    CommandConfigurationOccurrence::native(option, Some(value), false)
}

fn configuration(with_host: bool, rows: Vec<CommandConfigurationOccurrence>) -> SlugConfiguration {
    let mut host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    if with_host {
        host = host.with_action_environment_host(ActionEnvironmentHost::without_environment(
            ActionEnvironmentHostOs::Linux,
        ));
    }
    SlugConfiguration::default_target(&host)
        .unwrap()
        .with_command_configuration(Default::default(), &CommandConfigurationOverlay::from(rows))
        .unwrap()
}

fn evaluate(configuration: SlugConfiguration, source: &str) -> anyhow::Result<FrozenModule> {
    let module = Module::new();
    module.set("CONFIG", alloc_configuration(module.heap(), configuration)?);
    let mut evaluator = Evaluator::new(&module);
    evaluator
        .eval_module(
            AstModule::parse(
                "unrelated_shell_consumer.bzl",
                source.to_owned(),
                &Dialect::Bazel,
            )
            .map_err(starlark::Error::into_anyhow)?,
            &Globals::standard(),
        )
        .map_err(starlark::Error::into_anyhow)?;
    drop(evaluator);
    // Publish only ordinary values. No configuration facade is a producer root.
    module.set("CONFIG", Value::new_none());
    Ok(module.freeze()?)
}

fn result(configuration: SlugConfiguration, source: &str) -> String {
    evaluate(configuration, source)
        .unwrap()
        .get("result")
        .unwrap()
        .value()
        .to_repr()
}

#[test]
fn default_shell_env_returns_fresh_mutable_plain_dict() {
    for rows in [
        vec![native(
            NativeCommandOption::IncompatibleStrictActionEnv,
            "false",
        )],
        vec![native(NativeCommandOption::ActionEnv, "FIXED=original")],
    ] {
        let configuration = configuration(true, rows);
        assert_eq!(
            result(
                configuration.clone(),
                "first = CONFIG.default_shell_env\nsecond = CONFIG.default_shell_env\nfirst['LOCAL'] = 'scratch'\nsecond['SECOND'] = 'isolated'\nresult = (type(first), 'LOCAL' in second, 'SECOND' in first, 'LOCAL' in CONFIG.default_shell_env, 'default_shell_env' in dir(CONFIG))\n"
            ),
            "(\"dict\", False, False, False, True)"
        );
        assert!(evaluate(configuration, "result = CONFIG.default_shell_env()\n").is_err());
    }
}

#[test]
fn fixed_entries_omit_inherited_and_unset_and_keep_last_row() {
    let configuration = configuration(
        true,
        vec![
            native(NativeCommandOption::IncompatibleStrictActionEnv, "false"),
            native(NativeCommandOption::ActionEnv, "SET=first"),
            native(NativeCommandOption::ActionEnv, "SET=last"),
            native(NativeCommandOption::ActionEnv, "EMPTY="),
            native(NativeCommandOption::ActionEnv, "INHERITED=removed"),
            native(NativeCommandOption::ActionEnv, "INHERITED"),
            native(NativeCommandOption::ActionEnv, "UNSET=removed"),
            native(NativeCommandOption::ActionEnv, "=UNSET"),
        ],
    );
    assert!(
        configuration
            .configured_action_environment()
            .unwrap()
            .inherited()
            .contains("INHERITED")
    );
    assert_eq!(
        result(configuration, "result = CONFIG.default_shell_env\n"),
        "{\"EMPTY\": \"\", \"SET\": \"last\"}"
    );
}

#[test]
fn dictionary_order_is_utf16_without_changing_native_canonical_order() {
    let configuration = configuration(
        true,
        vec![
            native(NativeCommandOption::IncompatibleStrictActionEnv, "false"),
            native(NativeCommandOption::ActionEnv, "\u{e000}=bmp"),
            native(NativeCommandOption::ActionEnv, "\u{10000}=supplementary"),
            native(NativeCommandOption::ActionEnv, "a=ascii"),
        ],
    );
    let environment = configuration.configured_action_environment().unwrap();
    assert_eq!(
        environment
            .fixed()
            .iter()
            .map(|(key, _)| key)
            .collect::<Vec<_>>(),
        ["a", "\u{e000}", "\u{10000}"]
    );
    assert_eq!(
        result(
            configuration,
            "result = CONFIG.default_shell_env.keys() == ['a', '𐀀', '']\n"
        ),
        "True"
    );
}

#[test]
fn missing_host_is_lazy_and_preserves_unrelated_configuration_fields() {
    let configuration = configuration(false, vec![]);
    assert_eq!(
        result(
            configuration.clone(),
            "result = (CONFIG.coverage_enabled, CONFIG.is_tool_configuration())\n"
        ),
        "(False, False)"
    );
    let error = evaluate(configuration, "result = CONFIG.default_shell_env\n")
        .unwrap_err()
        .to_string();
    assert!(error.contains("missing retained Host facts"), "{error}");
}

#[test]
fn returned_dictionary_freezes_after_configuration_producer_drop() {
    let frozen = evaluate(
        configuration(
            true,
            vec![native(NativeCommandOption::ActionEnv, "OWNED=retained")],
        ),
        "result = CONFIG.default_shell_env\nresult['MUTATED'] = 'before_freeze'\n",
    )
    .unwrap();
    let consumer = Module::new();
    consumer.import_public_symbols(&frozen);
    let mut evaluator = Evaluator::new(&consumer);
    evaluator
        .eval_module(
            AstModule::parse(
                "other_consumer.bzl",
                "checked = result['OWNED'] + ':' + result['MUTATED']\n".to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::standard(),
        )
        .unwrap();
    assert_eq!(
        consumer.get("checked").unwrap().to_repr(),
        "\"retained:before_freeze\""
    );
    assert!(
        evaluator
            .eval_module(
                AstModule::parse(
                    "mutation.bzl",
                    "result['OWNED'] = 'after_freeze'\n".to_owned(),
                    &Dialect::Bazel
                )
                .unwrap(),
                &Globals::standard()
            )
            .is_err()
    );
}
