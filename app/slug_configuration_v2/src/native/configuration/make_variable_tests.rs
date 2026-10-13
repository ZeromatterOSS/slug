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

fn apply(
    base: &SlugConfiguration,
    rows: &[(NativeCommandOption, &str)],
) -> Result<SlugConfiguration, SlugConfigurationError> {
    let overlay: CommandConfigurationOverlay = rows
        .iter()
        .map(|(option, value)| CommandConfigurationOccurrence::native(*option, Some(*value), false))
        .collect::<Vec<_>>()
        .into();
    base.with_command_configuration(base.starlark_options().clone(), &overlay)
}

#[test]
fn make_variable_defines_normalize_last_wins_utf16_order_and_borrow_retained_values() {
    assert_eq!(
        NativeCommandOption::from_name("define"),
        Some(NativeCommandOption::Define)
    );
    assert_eq!(NativeCommandOption::Define.canonical_name(), "define");
    assert!(!NativeCommandOption::Define.is_boolean());
    assert!(!NativeCommandOption::Define.requires_repository_mapping());
    let base = base();
    let defined = apply(
        &base,
        &[
            (NativeCommandOption::Define, "z=first"),
            (NativeCommandOption::Define, "a="),
            (NativeCommandOption::Define, "z=last=with=equals"),
            (NativeCommandOption::Define, "COMPILATION_MODE=custom"),
            (NativeCommandOption::Define, "TARGET_CPU=custom-cpu"),
            (NativeCommandOption::Define, "BINDIR=custom-bin"),
            (NativeCommandOption::Define, "GENDIR=custom-gen"),
            (NativeCommandOption::Define, "\u{e000}=bmp"),
            (NativeCommandOption::Define, "\u{10000}=supplementary"),
        ],
    )
    .unwrap();
    let inputs = defined.make_variable_inputs().unwrap();
    assert_eq!(inputs.compilation_mode(), "fastbuild");
    let rows = inputs.defines().collect::<Vec<_>>();
    assert_eq!(
        rows,
        [
            ("BINDIR", "custom-bin"),
            ("COMPILATION_MODE", "custom"),
            ("GENDIR", "custom-gen"),
            ("TARGET_CPU", "custom-cpu"),
            ("a", ""),
            ("z", "last=with=equals"),
            ("\u{10000}", "supplementary"),
            ("\u{e000}", "bmp"),
        ]
    );
    assert_eq!(inputs.defines().len(), 8);
    let OptionValue::Native(NativeOccurrence::List(values)) =
        defined.option_value(CORE_OPTIONS, "define").unwrap()
    else {
        panic!("defines must retain normalized native entries")
    };
    for ((key, value), stored) in inputs.defines().zip(values.iter()) {
        let NativeValue::Entry(stored_key, stored_value) = stored else {
            unreachable!()
        };
        assert_eq!(key.as_ptr(), stored_key.as_ptr());
        assert_eq!(value.as_ptr(), stored_value.as_ptr());
    }
    let equivalent = apply(
        &base,
        &[
            (NativeCommandOption::Define, "\u{e000}=bmp"),
            (NativeCommandOption::Define, "\u{10000}=supplementary"),
            (NativeCommandOption::Define, "z=last=with=equals"),
            (NativeCommandOption::Define, "a="),
            (NativeCommandOption::Define, "TARGET_CPU=custom-cpu"),
            (NativeCommandOption::Define, "GENDIR=custom-gen"),
            (NativeCommandOption::Define, "BINDIR=custom-bin"),
            (NativeCommandOption::Define, "COMPILATION_MODE=custom"),
        ],
    )
    .unwrap();
    assert_eq!(defined, equivalent);
    assert_eq!(defined.canonical_bytes(), equivalent.canonical_bytes());
    let changed = apply(&defined, &[(NativeCommandOption::Define, "z=B")]).unwrap();
    assert_ne!(defined, changed);
    let restored = apply(
        &changed,
        &[(NativeCommandOption::Define, "z=last=with=equals")],
    )
    .unwrap();
    assert_eq!(defined, restored);
}

#[test]
fn make_variable_define_conversion_preserves_errors_from_overwritten_occurrences() {
    let base = base();
    for invalid in ["", "bare", "=empty-key"] {
        assert_eq!(
            apply(
                &base,
                &[
                    (NativeCommandOption::Define, invalid),
                    (NativeCommandOption::Define, "valid=later")
                ]
            ),
            Err(SlugConfigurationError::InvalidCommandNativeOption { option: "define" })
        );
    }
    for (raw_value, negated) in [(None, false), (Some("valid=value"), true)] {
        let overlay: CommandConfigurationOverlay = vec![CommandConfigurationOccurrence::native(
            NativeCommandOption::Define,
            raw_value,
            negated,
        )]
        .into();
        assert_eq!(
            base.with_command_configuration(base.starlark_options().clone(), &overlay),
            Err(SlugConfigurationError::InvalidCommandNativeOption { option: "define" })
        );
    }
    let good = apply(
        &base,
        &[
            (NativeCommandOption::Define, "valid=one=two"),
            (NativeCommandOption::Define, "empty="),
        ],
    )
    .unwrap();
    assert_eq!(
        good.make_variable_inputs()
            .unwrap()
            .defines()
            .collect::<Vec<_>>(),
        [("empty", ""), ("valid", "one=two")]
    );
    assert_eq!(base.make_variable_inputs().unwrap().defines().len(), 0);
}

#[test]
fn make_variable_exec_modes_use_shared_core_projection_and_preserve_defines() {
    let base = base();
    let platform = CanonicalLabel::parse("@@platforms+//:execution").unwrap();
    for mode in ["fastbuild", "dbg", "opt"] {
        let target = apply(
            &base,
            &[
                (NativeCommandOption::CompilationMode, "dbg"),
                (NativeCommandOption::HostCompilationMode, mode),
                (NativeCommandOption::Define, "A=target"),
            ],
        )
        .unwrap();
        assert_eq!(
            target.make_variable_inputs().unwrap().compilation_mode(),
            "dbg"
        );
        for exec in [
            target.to_exec().unwrap(),
            target.to_exec_for_platform(&platform).unwrap(),
        ] {
            assert_eq!(exec.kind(), SlugConfigurationKind::Exec);
            assert_eq!(
                exec.make_variable_inputs().unwrap().compilation_mode(),
                mode
            );
            assert_eq!(
                CppFragmentProjection::new(exec.clone())
                    .unwrap()
                    .compilation_mode()
                    .unwrap(),
                mode
            );
            assert_eq!(
                exec.make_variable_inputs()
                    .unwrap()
                    .defines()
                    .collect::<Vec<_>>(),
                [("A", "target")]
            );
            let rebound = exec.to_exec_for_platform(&platform).unwrap();
            assert_eq!(rebound, rebound.to_exec_for_platform(&platform).unwrap());
        }
    }
}

fn with_option(base: &SlugConfiguration, name: &'static str, raw: &str) -> SlugConfiguration {
    let mut options = base.option_records().to_vec();
    let record = options
        .iter_mut()
        .find(|row| row.class_name == CORE_OPTIONS && row.canonical_name == name)
        .unwrap();
    let descriptor = NATIVE_OPTION_DESCRIPTORS
        .iter()
        .find(|row| row.class_name == CORE_OPTIONS && row.canonical_name == name)
        .unwrap();
    record.value = if label_convert::classify(descriptor).is_some() {
        OptionValue::Label(
            label_convert::convert_label_occurrence(
                descriptor,
                raw,
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        )
    } else if label_convert::classify_mixed(descriptor).is_some() {
        OptionValue::Mixed(
            label_convert::convert_mixed_occurrence(
                descriptor,
                raw,
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        )
    } else {
        OptionValue::Native(super::super::convert::convert_occurrence(descriptor, raw).unwrap())
    };
    finish_configuration(
        base.kind(),
        options.into(),
        base.starlark_options().clone(),
        base.action_environment_host().cloned(),
    )
}

#[test]
fn make_variable_controls_refuse_only_when_projection_is_requested() {
    let base = base();
    for (name, raw) in [
        ("incompatible_target_cpu_from_platform", "false"),
        (
            "experimental_override_platform_cpu_name",
            "@@//platforms:custom=custom-cpu",
        ),
        ("experimental_exclude_defines_from_exec_config", "true"),
        ("experimental_propagate_custom_flag", "CUSTOM"),
        ("incompatible_merge_genfiles_directory", "false"),
    ] {
        let unsupported = with_option(&base, name, raw);
        assert_ne!(base, unsupported);
        let configured = apply(&unsupported, &[(NativeCommandOption::Define, "A=value")]).unwrap();
        assert!(
            matches!(configured.make_variable_inputs(), Err(SlugConfigurationError::UnsupportedMakeVariableOption { option }) if option == name)
        );
        assert_eq!(
            base.make_variable_inputs().unwrap().compilation_mode(),
            "fastbuild"
        );
    }
}
