//! Bazel 9.2 CppOptions.java:970-979 owns the True default;
//! builtin_exec_platforms.bzl:303 propagates the Boolean to exec configurations.

use super::*;
use crate::CppFragmentProjection;

fn base() -> SlugConfiguration {
    SlugConfiguration::default_target(
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
    .unwrap()
}

fn occurrence(raw: Option<&str>, negated: bool) -> CommandConfigurationOccurrence {
    CommandConfigurationOccurrence::native(
        NativeCommandOption::ExperimentalCcImplementationDeps,
        raw,
        negated,
    )
}

fn apply(
    configuration: &SlugConfiguration,
    occurrences: Vec<CommandConfigurationOccurrence>,
) -> Result<SlugConfiguration, SlugConfigurationError> {
    configuration.with_command_configuration(
        configuration.starlark_options().clone(),
        &CommandConfigurationOverlay::from(occurrences),
    )
}

fn projected(configuration: &SlugConfiguration) -> bool {
    CppFragmentProjection::new(configuration.clone())
        .unwrap()
        .experimental_cc_implementation_deps()
        .unwrap()
}

#[test]
fn implementation_deps_boolean_conversion_restores_structural_configuration_and_exec_values() {
    let original = base();
    assert!(projected(&original));
    let disabled = apply(&original, vec![occurrence(None, true)]).unwrap();
    assert!(!projected(&disabled));
    assert_ne!(disabled, original);
    assert_ne!(disabled.canonical_bytes(), original.canonical_bytes());
    assert_ne!(disabled.projection(), original.projection());
    let restored = apply(&disabled, vec![occurrence(None, false)]).unwrap();
    assert!(projected(&restored));
    assert_eq!(restored, original);
    assert_eq!(restored.canonical_bytes(), original.canonical_bytes());
    assert_eq!(restored.projection(), original.projection());

    for (spellings, expected) in [
        (["true", "1", "yes", "t", "y", "TRUE"].as_slice(), true),
        (["false", "0", "no", "f", "n", "FALSE"].as_slice(), false),
    ] {
        for spelling in spellings {
            let configuration = apply(&original, vec![occurrence(Some(spelling), false)]).unwrap();
            assert_eq!(projected(&configuration), expected, "{spelling}");
            assert_eq!(
                configuration,
                if expected { &original } else { &disabled }.clone()
            );
        }
    }
    assert_eq!(
        apply(
            &original,
            vec![
                occurrence(None, false),
                occurrence(Some("false"), false),
                occurrence(None, true)
            ]
        )
        .unwrap(),
        disabled,
        "the last occurrence wins"
    );
    assert_eq!(
        apply(
            &original,
            vec![occurrence(None, true), occurrence(Some("true"), false)]
        )
        .unwrap(),
        original
    );
    let platform = CanonicalLabel::parse("@@//platforms:exec").unwrap();
    for (configuration, expected) in [(original, true), (disabled, false)] {
        let legacy_exec = configuration.to_exec().unwrap();
        let selected_exec = configuration.to_exec_for_platform(&platform).unwrap();
        assert_eq!(projected(&legacy_exec), expected);
        assert_eq!(projected(&selected_exec), expected);
        assert_eq!(
            selected_exec.to_exec_for_platform(&platform).unwrap(),
            selected_exec,
            "selected-platform exec projection is idempotent"
        );
        assert_eq!(projected(&configuration), expected);
    }
}

#[test]
fn implementation_deps_rejects_invalid_text_and_malformed_retained_state() {
    let original = base();
    for option in [
        NativeCommandOption::ExperimentalCcImplementationDeps,
        NativeCommandOption::ForcePic,
    ] {
        for spelling in ["null", "NULL", "maybe", "", " true", "false "] {
            assert!(
                apply(
                    &original,
                    vec![CommandConfigurationOccurrence::native(
                        option,
                        Some(spelling),
                        false
                    )]
                )
                .is_err(),
                "{option:?} must reject explicit {spelling:?}"
            );
        }
        assert!(
            apply(
                &original,
                vec![CommandConfigurationOccurrence::native(
                    option,
                    Some("true"),
                    true
                )]
            )
            .is_err()
        );
    }
    for value in [
        OptionValue::Native(NativeOccurrence::Absent),
        OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text("false".into()))),
        OptionValue::Label(None),
    ] {
        let mut options = original.0.options.to_vec();
        options
            .iter_mut()
            .find(|record| {
                record.class_name == CPP_OPTIONS
                    && record.canonical_name == "experimental_cc_implementation_deps"
            })
            .unwrap()
            .value = value;
        let malformed = finish_configuration(
            original.0.kind,
            options.into(),
            original.0.starlark_options.clone(),
            original.0.action_environment_host.clone(),
        );
        assert!(CppFragmentProjection::new(malformed).is_err());
    }
}

#[test]
fn implementation_deps_config_setting_matches_the_same_boolean_and_rejects_null_text() {
    let enabled = base();
    let disabled = apply(&enabled, vec![occurrence(None, true)]).unwrap();
    let name = "experimental_cc_implementation_deps";
    for (configuration, expected) in [(&enabled, true), (&disabled, false)] {
        for (text, value) in [("true", true), ("false", false), ("0", false)] {
            assert_eq!(
                configuration
                    .matches_config_setting(&[(name.into(), text.into())], &[])
                    .unwrap(),
                value == expected,
            );
        }
        for option in [name, "force_pic"] {
            for text in ["null", "NULL"] {
                assert_eq!(
                    configuration.matches_config_setting(&[(option.into(), text.into())], &[]),
                    Err(crate::native::NativeConfigSettingMatchError::InvalidValue(
                        option.into()
                    )),
                );
            }
        }
    }
}
