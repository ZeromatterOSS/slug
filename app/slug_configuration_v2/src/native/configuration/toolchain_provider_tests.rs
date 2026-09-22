//! Pinned CppOptions141–148/CoreOptions349–356 and builtin_exec_platforms.
use super::*;
use crate::native::CppFragmentProjection;

fn host() -> HostConversionInputs {
    HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap()
    .with_action_environment_host(ActionEnvironmentHost::without_environment(
        ActionEnvironmentHostOs::Linux,
    ))
}
fn base() -> SlugConfiguration {
    SlugConfiguration::default_target(&host()).unwrap()
}
fn apply(
    base: &SlugConfiguration,
    values: &[(NativeCommandOption, Option<&str>, bool)],
) -> Result<SlugConfiguration, SlugConfigurationError> {
    let overlay: CommandConfigurationOverlay = values
        .iter()
        .map(|(option, value, negated)| {
            CommandConfigurationOccurrence::native(*option, *value, *negated)
        })
        .collect::<Vec<_>>()
        .into();
    base.with_command_configuration(base.starlark_options().clone(), &overlay)
}
fn cpp(config: &SlugConfiguration) -> CppFragmentProjection {
    CppFragmentProjection::new(config.clone()).unwrap()
}
fn replace(
    config: &SlugConfiguration,
    class: &str,
    name: &str,
    value: OptionValue,
) -> SlugConfiguration {
    let mut options = config.0.options.to_vec();
    options
        .iter_mut()
        .find(|row| row.class_name == class && row.canonical_name == name)
        .unwrap()
        .value = value;
    finish_configuration(
        config.kind(),
        options.into(),
        config.0.starlark_options.clone(),
        config.0.action_environment_host.clone(),
    )
}

#[test]
fn toolchain_provider_defaults_are_nullable_text_and_false_stamp() {
    for config in [base(), SlugConfiguration::default_exec(&host()).unwrap()] {
        assert_eq!(cpp(&config).minimum_os_version().unwrap(), None);
        assert!(!config.stamp_binaries().unwrap());
        assert_eq!(
            config
                .option_value(CPP_OPTIONS, "minimum_os_version")
                .unwrap(),
            &OptionValue::Native(NativeOccurrence::Absent)
        );
        assert_eq!(
            config.option_value(CORE_OPTIONS, "stamp").unwrap(),
            &OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Bool(false)))
        );
    }
    assert_eq!(base().kind(), SlugConfigurationKind::Target);
    assert_eq!(
        SlugConfiguration::default_exec(&host()).unwrap().kind(),
        SlugConfigurationKind::Exec
    );
    for (name, option, boolean) in [
        (
            "minimum_os_version",
            NativeCommandOption::MinimumOsVersion,
            false,
        ),
        ("stamp", NativeCommandOption::Stamp, true),
    ] {
        assert_eq!(NativeCommandOption::from_name(name), Some(option));
        assert_eq!(option.canonical_name(), name);
        assert_eq!(option.is_boolean(), boolean);
        assert!(!option.requires_repository_mapping());
    }
}

#[test]
fn minimum_os_text_preserves_empty_null_arbitrary_and_restores_full_identity() {
    let base = base();
    for text in ["", "null", "14.2", "not a version", "非ASCII", "13,14"] {
        let changed = apply(
            &base,
            &[(NativeCommandOption::MinimumOsVersion, Some(text), false)],
        )
        .unwrap();
        assert_eq!(cpp(&changed).minimum_os_version().unwrap(), Some(text));
        assert_ne!(base, changed);
        assert_ne!(base.canonical_bytes(), changed.canonical_bytes());
        assert_eq!(
            changed
                .option_value(CPP_OPTIONS, "minimum_os_version")
                .unwrap(),
            &OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text(text.into())))
        );
    }
    let a = apply(
        &base,
        &[(NativeCommandOption::MinimumOsVersion, Some("A"), false)],
    )
    .unwrap();
    let b = apply(
        &a,
        &[(NativeCommandOption::MinimumOsVersion, Some("B"), false)],
    )
    .unwrap();
    let restored = apply(
        &b,
        &[(NativeCommandOption::MinimumOsVersion, Some("A"), false)],
    )
    .unwrap();
    assert_ne!(a, b);
    assert_eq!(a, restored);
    assert_eq!(a.canonical_bytes(), restored.canonical_bytes());
    assert_eq!(a.projection(), restored.projection());
    assert_eq!(
        apply(
            &base,
            &[
                (NativeCommandOption::MinimumOsVersion, Some("B"), false),
                (NativeCommandOption::MinimumOsVersion, Some("A"), false)
            ]
        )
        .unwrap(),
        a
    );
    for (raw, negated) in [(None, false), (None, true), (Some("A"), true)] {
        assert!(
            apply(
                &base,
                &[(NativeCommandOption::MinimumOsVersion, raw, negated)]
            )
            .is_err()
        );
    }
}

#[test]
fn stamp_core_boolean_aliases_no_forms_and_restoration_are_exact() {
    let base = base();
    for text in ["true", "1", "yes", "t", "y", "TrUe"] {
        let changed = apply(&base, &[(NativeCommandOption::Stamp, Some(text), false)]).unwrap();
        assert!(changed.stamp_binaries().unwrap());
        assert_ne!(changed, base);
        assert_eq!(
            changed.option_value(CORE_OPTIONS, "stamp").unwrap(),
            &OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Bool(true)))
        );
    }
    let enabled = apply(&base, &[(NativeCommandOption::Stamp, None, false)]).unwrap();
    for text in ["false", "0", "no", "f", "n", "FaLsE"] {
        assert_eq!(
            apply(&enabled, &[(NativeCommandOption::Stamp, Some(text), false)]).unwrap(),
            base
        );
    }
    assert_eq!(
        apply(&enabled, &[(NativeCommandOption::Stamp, None, true)]).unwrap(),
        base
    );
    assert_eq!(
        apply(
            &enabled,
            &[
                (NativeCommandOption::Stamp, None, true),
                (NativeCommandOption::Stamp, None, false)
            ]
        )
        .unwrap(),
        enabled
    );
    for text in ["", "maybe", " true "] {
        assert!(apply(&base, &[(NativeCommandOption::Stamp, Some(text), false)]).is_err());
    }
    assert!(apply(&base, &[(NativeCommandOption::Stamp, Some("false"), true)]).is_err());
}

#[test]
fn both_exec_paths_reset_target_minimum_os_and_stamp_without_mutating_target() {
    let base = base();
    let platform = CanonicalLabel::parse("@@//platforms:exec").unwrap();
    for minimum in ["", "null", "14.2"] {
        let target = apply(
            &base,
            &[
                (NativeCommandOption::MinimumOsVersion, Some(minimum), false),
                (NativeCommandOption::Stamp, None, false),
            ],
        )
        .unwrap();
        for exec in [
            target.to_exec().unwrap(),
            target.to_exec_for_platform(&platform).unwrap(),
        ] {
            assert_eq!(exec.kind(), SlugConfigurationKind::Exec);
            assert_eq!(cpp(&exec).minimum_os_version().unwrap(), None);
            assert!(!exec.stamp_binaries().unwrap());
            assert_ne!(target, exec);
            assert_eq!(
                exec.to_exec_for_platform(&platform).unwrap().kind(),
                SlugConfigurationKind::Exec
            );
        }
        assert_eq!(target.to_exec().unwrap(), base.to_exec().unwrap());
        assert_eq!(
            target.to_exec_for_platform(&platform).unwrap(),
            base.to_exec_for_platform(&platform).unwrap()
        );
        assert_eq!(cpp(&target).minimum_os_version().unwrap(), Some(minimum));
        assert!(target.stamp_binaries().unwrap());
    }
}

#[test]
fn toolchain_provider_getters_reject_malformed_retained_shapes() {
    let base = base();
    for value in [
        OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Bool(false))),
        OptionValue::Native(NativeOccurrence::List(NativeValues(Arc::from([])))),
        OptionValue::Label(None),
    ] {
        let bad = replace(&base, CPP_OPTIONS, "minimum_os_version", value);
        assert!(CppFragmentProjection::new(bad).is_err());
    }
    for value in [
        OptionValue::Native(NativeOccurrence::Absent),
        OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text("true".into()))),
        OptionValue::Native(NativeOccurrence::List(NativeValues(Arc::from([])))),
        OptionValue::Label(None),
    ] {
        let bad = replace(&base, CORE_OPTIONS, "stamp", value);
        assert!(bad.stamp_binaries().is_err());
    }
    assert_eq!(cpp(&base).minimum_os_version().unwrap(), None);
    assert!(!base.stamp_binaries().unwrap());
}

#[test]
fn unused_configuration_remains_structural_under_toolchain_projections() {
    let a = apply(
        &base(),
        &[
            (NativeCommandOption::MinimumOsVersion, Some("14.2"), false),
            (NativeCommandOption::Stamp, None, false),
        ],
    )
    .unwrap();
    let b = apply(
        &a,
        &[(
            NativeCommandOption::StrictSystemIncludes,
            Some("true"),
            false,
        )],
    )
    .unwrap();
    assert_eq!(
        cpp(&a).minimum_os_version().unwrap(),
        cpp(&b).minimum_os_version().unwrap()
    );
    assert_eq!(a.stamp_binaries().unwrap(), b.stamp_binaries().unwrap());
    assert_ne!(a, b);
    assert_ne!(cpp(&a), cpp(&b));
    assert_ne!(a.canonical_bytes(), b.canonical_bytes());
    let restored = apply(
        &b,
        &[(
            NativeCommandOption::StrictSystemIncludes,
            Some("false"),
            false,
        )],
    )
    .unwrap();
    assert_eq!(restored, a);
    assert_eq!(cpp(&restored), cpp(&a));
}
