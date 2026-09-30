//! Bazel 9.2 CppOptions:150-163,828-837 defaults and
//! builtin_exec_platforms:305,313 exec propagation.
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

const OPTIONS: [(NativeCommandOption, &str); 2] = [
    (NativeCommandOption::StartEndLib, "start_end_lib"),
    (
        NativeCommandOption::IncompatibleUseSpecificToolFiles,
        "incompatible_use_specific_tool_files",
    ),
];

fn set(
    configuration: &SlugConfiguration,
    option: NativeCommandOption,
    value: bool,
) -> SlugConfiguration {
    configuration
        .with_command_configuration(
            configuration.starlark_options().clone(),
            &CommandConfigurationOverlay::from(vec![CommandConfigurationOccurrence::native(
                option,
                None::<&str>,
                !value,
            )]),
        )
        .unwrap()
}

fn projected(projection: &CppFragmentProjection, option: NativeCommandOption) -> bool {
    match option {
        NativeCommandOption::StartEndLib => projection.start_end_lib().unwrap(),
        NativeCommandOption::IncompatibleUseSpecificToolFiles => {
            projection.incompatible_use_specific_tool_files().unwrap()
        }
        _ => unreachable!("only the two requested Boolean methods are tested"),
    }
}

#[test]
fn start_end_lib_preserves_structural_identity_restoration_exec_and_config_setting() {
    for (option, name) in OPTIONS {
        let original = base();
        let disabled = set(&original, option, false);
        assert_ne!(original, disabled);
        assert_ne!(original.canonical_bytes(), disabled.canonical_bytes());
        assert_ne!(original.projection(), disabled.projection());
        let restored = set(&disabled, option, true);
        assert_eq!(restored, original);
        assert_eq!(restored.canonical_bytes(), original.canonical_bytes());
        assert_eq!(restored.projection(), original.projection());
        assert_eq!(option.canonical_name(), name);
        assert_eq!(NativeCommandOption::from_name(name), Some(option));
        assert!(option.is_boolean());
        let platform = CanonicalLabel::parse("@@//platforms:exec").unwrap();
        for (configuration, expected) in [(original, true), (disabled, false)] {
            for view in [
                configuration.clone(),
                configuration.to_exec().unwrap(),
                configuration.to_exec_for_platform(&platform).unwrap(),
            ] {
                let projection = CppFragmentProjection::new(view.clone()).unwrap();
                assert_eq!(projected(&projection, option), expected);
                assert!(projection.experimental_cc_implementation_deps().unwrap());
                for (other, _) in OPTIONS.into_iter().filter(|(other, _)| *other != option) {
                    assert!(projected(&projection, other));
                }
                for (text, value) in [("true", true), ("0", false)] {
                    assert_eq!(
                        view.matches_config_setting(&[(name.into(), text.into())], &[])
                            .unwrap(),
                        expected == value
                    );
                }
            }
            let exec = configuration.to_exec_for_platform(&platform).unwrap();
            assert_eq!(exec.to_exec_for_platform(&platform).unwrap(), exec);
        }
    }
}

#[test]
fn start_end_lib_rejects_malformed_retained_state_at_projection_construction() {
    let original = base();
    for (_, name) in OPTIONS {
        for value in [
            OptionValue::Native(NativeOccurrence::Absent),
            OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text("true".into()))),
            OptionValue::Label(None),
        ] {
            let mut options = original.0.options.to_vec();
            options
                .iter_mut()
                .find(|record| record.class_name == CPP_OPTIONS && record.canonical_name == name)
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
}
