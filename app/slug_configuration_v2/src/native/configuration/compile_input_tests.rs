//! Bazel 9.2 CppOptions/CppConfiguration and builtin_exec_platforms witnesses.
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::*;
use crate::native::CppFragmentProjection;
use crate::native::host::ActionEnvironmentHost;

fn base(os: Option<ActionEnvironmentHostOs>) -> SlugConfiguration {
    // CPU/path flavor deliberately stay identical across all OS discriminators.
    let mut host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    if let Some(os) = os {
        host = host.with_action_environment_host(ActionEnvironmentHost::without_environment(os));
    }
    SlugConfiguration::default_target(&host).unwrap()
}

fn apply(
    base: &SlugConfiguration,
    values: &[(NativeCommandOption, &str)],
) -> Result<SlugConfiguration, SlugConfigurationError> {
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("compile-inputs").unwrap());
    let overlay = CommandConfigurationOverlay::from(
        values
            .iter()
            .map(|(option, value)| {
                CommandConfigurationOccurrence::native(*option, Some(*value), false)
            })
            .collect::<Vec<_>>(),
    );
    base.with_command_configuration_context(
        base.starlark_options().clone(),
        &overlay,
        OptionLabelContext::MainRepository { mapping: &mapping },
    )
}

fn cpp(config: &SlugConfiguration) -> CppFragmentProjection {
    CppFragmentProjection::new(config.clone()).unwrap()
}

fn replace(config: &SlugConfiguration, name: &str, value: OptionValue) -> SlugConfiguration {
    let mut options = config.0.options.to_vec();
    options
        .iter_mut()
        .find(|row| row.class_name == CPP_OPTIONS && row.canonical_name == name)
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
fn compile_projection_defaults_raw_repeats_and_omitfp_prefix_are_exact() {
    let base = base(None);
    let defaults = cpp(&base);
    assert!(defaults.copts().unwrap().next().is_none());
    assert!(defaults.conlyopts().unwrap().next().is_none());
    assert!(defaults.cxxopts().unwrap().next().is_none());
    assert!(defaults.should_generate_dotd_files().unwrap());
    assert!(!defaults.save_temps().unwrap());
    assert!(!defaults.process_headers_in_dependencies().unwrap());
    defaults.per_file_copts_empty().unwrap();
    let mut occurrences = Vec::new();
    for option in [
        NativeCommandOption::Copt,
        NativeCommandOption::Conlyopt,
        NativeCommandOption::Cxxopt,
        NativeCommandOption::HostCopt,
        NativeCommandOption::HostConlyopt,
        NativeCommandOption::HostCxxopt,
    ] {
        for text in ["-Dcomma=a,b", "", "-Dcomma=a,b", "null"] {
            occurrences.push((option, text));
        }
    }
    let changed = apply(&base, &occurrences).unwrap();
    for name in [
        "copt",
        "conlyopt",
        "cxxopt",
        "host_copt",
        "host_conlyopt",
        "host_cxxopt",
    ] {
        assert_eq!(
            changed
                .native_string_list_option(CPP_OPTIONS, name)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            ["-Dcomma=a,b", "", "-Dcomma=a,b", "null"]
        );
    }
    let projection = cpp(&changed);
    assert!(
        projection
            .copts()
            .unwrap()
            .eq(projection.conlyopts().unwrap())
    );
    assert!(
        projection
            .copts()
            .unwrap()
            .eq(projection.cxxopts().unwrap())
    );
    let omitted = apply(
        &changed,
        &[(NativeCommandOption::ExperimentalOmitfp, "true")],
    )
    .unwrap();
    assert_eq!(
        cpp(&omitted).copts().unwrap().collect::<Vec<_>>(),
        [
            "-fomit-frame-pointer",
            "-fasynchronous-unwind-tables",
            "-DNO_FRAME_POINTER",
            "-Dcomma=a,b",
            "",
            "-Dcomma=a,b",
            "null"
        ]
    );
    assert!(
        cpp(&omitted)
            .conlyopts()
            .unwrap()
            .eq(projection.conlyopts().unwrap())
    );
    assert!(
        cpp(&omitted)
            .cxxopts()
            .unwrap()
            .eq(projection.cxxopts().unwrap())
    );
    assert_eq!(
        apply(
            &omitted,
            &[(NativeCommandOption::ExperimentalOmitfp, "false")]
        )
        .unwrap(),
        changed
    );
    assert_ne!(omitted.canonical_bytes(), changed.canonical_bytes());
    assert_eq!(apply(&base, &occurrences).unwrap(), changed);
}

#[test]
fn compile_boolean_converters_preserve_values_and_reject_invalid_input() {
    let base = base(None);
    for option in [
        NativeCommandOption::CcDotdFiles,
        NativeCommandOption::SaveTemps,
        NativeCommandOption::ProcessHeadersInDependencies,
    ] {
        for (text, expected) in [
            ("true", true),
            ("yes", true),
            ("1", true),
            ("false", false),
            ("no", false),
            ("0", false),
        ] {
            let configured = apply(&base, &[(option, text)]).unwrap();
            assert_eq!(
                configured
                    .native_bool(CPP_OPTIONS, option.canonical_name())
                    .unwrap(),
                expected
            );
            let projection = cpp(&configured);
            let actual = match option {
                NativeCommandOption::CcDotdFiles => projection.should_generate_dotd_files(),
                NativeCommandOption::SaveTemps => projection.save_temps(),
                _ => projection.process_headers_in_dependencies(),
            }
            .unwrap();
            assert_eq!(actual, expected);
        }
        for text in ["", "maybe", "true,false"] {
            assert!(
                apply(&base, &[(option, text)]).is_err(),
                "{option:?}={text}"
            );
        }
    }
}

#[test]
fn both_exec_paths_map_host_compile_flags_using_observed_os_and_restore() {
    for os in [
        ActionEnvironmentHostOs::Linux,
        ActionEnvironmentHostOs::Windows,
        ActionEnvironmentHostOs::Macos,
        ActionEnvironmentHostOs::Freebsd,
        ActionEnvironmentHostOs::Openbsd,
        ActionEnvironmentHostOs::Unknown,
    ] {
        let base = base(Some(os));
        let input = [
            (NativeCommandOption::Copt, "target-c"),
            (NativeCommandOption::Cxxopt, "target-cxx"),
            (NativeCommandOption::Conlyopt, "target-conly"),
            (NativeCommandOption::HostCopt, "host-c,raw"),
            (NativeCommandOption::HostCopt, ""),
            (NativeCommandOption::HostCxxopt, "host-cxx"),
            (NativeCommandOption::HostCxxopt, "host-cxx"),
            (NativeCommandOption::HostConlyopt, "host-conly"),
            (NativeCommandOption::CcDotdFiles, "false"),
            (NativeCommandOption::SaveTemps, "true"),
            (NativeCommandOption::ProcessHeadersInDependencies, "true"),
            (NativeCommandOption::ExperimentalOmitfp, "true"),
        ];
        let target = apply(&base, &input).unwrap();
        // Selected execution-platform spelling must not replace the observed
        // process Host OS used by Bazel's builtin exec transition.
        let platform = CanonicalLabel::parse(if os == ActionEnvironmentHostOs::Windows {
            "@@//platforms:linux"
        } else {
            "@@//platforms:windows"
        })
        .unwrap();
        let executions = [
            target.to_exec().unwrap(),
            target.to_exec_for_platform(&platform).unwrap(),
        ];
        for exec in &executions {
            let projection = cpp(exec);
            let suffix = (os != ActionEnvironmentHostOs::Windows).then_some("-g0");
            assert!(
                exec.native_string_list_option(CPP_OPTIONS, "copt")
                    .unwrap()
                    .iter()
                    .eq(["host-c,raw", ""].into_iter().chain(suffix))
            );
            assert!(
                projection
                    .cxxopts()
                    .unwrap()
                    .eq(["host-cxx", "host-cxx"].into_iter().chain(suffix))
            );
            assert_eq!(
                projection.conlyopts().unwrap().collect::<Vec<_>>(),
                ["host-conly"]
            );
            assert!(
                projection.copts().unwrap().eq([
                    "-fomit-frame-pointer",
                    "-fasynchronous-unwind-tables",
                    "-DNO_FRAME_POINTER",
                    "host-c,raw",
                    ""
                ]
                .into_iter()
                .chain(suffix))
            );
            assert!(!projection.should_generate_dotd_files().unwrap());
            assert!(!projection.save_temps().unwrap());
            assert!(!projection.process_headers_in_dependencies().unwrap());
            assert_eq!(exec.execution_host_os().unwrap(), os);
            for name in ["host_copt", "host_cxxopt", "host_conlyopt"] {
                assert_eq!(
                    exec.option_value(CPP_OPTIONS, name).unwrap(),
                    target.option_value(CPP_OPTIONS, name).unwrap()
                );
            }
        }
        assert_eq!(
            cpp(&target).cxxopts().unwrap().collect::<Vec<_>>(),
            ["target-cxx"]
        );
        assert!(cpp(&target).save_temps().unwrap());
        let changed = apply(&target, &[(NativeCommandOption::HostCxxopt, "changed")]).unwrap();
        assert_ne!(changed.to_exec().unwrap(), executions[0]);
        assert_ne!(
            changed.to_exec_for_platform(&platform).unwrap(),
            executions[1]
        );
        let restored = apply(&base, &input).unwrap();
        assert_eq!(restored, target);
        assert_eq!(restored.to_exec().unwrap(), executions[0]);
        assert_eq!(
            restored.to_exec_for_platform(&platform).unwrap(),
            executions[1]
        );
        assert_eq!(
            executions[1].to_exec_for_platform(&platform).unwrap(),
            executions[1]
        );
        let other = CanonicalLabel::parse("@@//platforms:other").unwrap();
        assert_eq!(
            executions[1].to_exec_for_platform(&other).unwrap(),
            target.to_exec_for_platform(&other).unwrap()
        );
    }
    let missing = base(None);
    let platform = CanonicalLabel::parse("@@//platforms:exec").unwrap();
    assert_eq!(
        missing.to_exec(),
        Err(SlugConfigurationError::MissingExecutionHostOs)
    );
    assert_eq!(
        missing.to_exec_for_platform(&platform),
        Err(SlugConfigurationError::MissingExecutionHostOs)
    );
}

#[test]
fn compile_projection_rejects_malformed_lists_booleans_and_nonempty_per_file_state() {
    let base = base(Some(ActionEnvironmentHostOs::Linux));
    let malformed = OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Bool(true)));
    for name in ["copt", "conlyopt", "cxxopt", "per_file_copt"] {
        assert!(
            CppFragmentProjection::new(replace(&base, name, malformed.clone())).is_err(),
            "{name}"
        );
    }
    let mixed = OptionValue::Native(NativeOccurrence::List(NativeValues(Arc::from([
        NativeValue::Int(3),
    ]))));
    for name in ["copt", "conlyopt", "cxxopt", "per_file_copt"] {
        assert!(
            CppFragmentProjection::new(replace(&base, name, mixed.clone())).is_err(),
            "{name}"
        );
    }
    for name in [
        "cc_dotd_files",
        "save_temps",
        "process_headers_in_dependencies",
    ] {
        assert!(
            CppFragmentProjection::new(replace(&base, name, mixed.clone())).is_err(),
            "{name}"
        );
    }
    // Host per-file state must replace target state in both exec pathways;
    // neither unsupported nonempty list can silently become an empty result.
    let target = replace(&base, "per_file_copt", mixed.clone());
    assert!(CppFragmentProjection::new(target.clone()).is_err());
    cpp(&target.to_exec().unwrap())
        .per_file_copts_empty()
        .unwrap();
    let host = replace(&base, "host_per_file_copt", mixed);
    for exec in [
        host.to_exec().unwrap(),
        host.to_exec_for_platform(&CanonicalLabel::parse("@@//platforms:exec").unwrap())
            .unwrap(),
    ] {
        assert!(CppFragmentProjection::new(exec).is_err());
    }
}
