//! Pinned CoreOptions/CppOptions conversions and the admitted exec projection.
use std::sync::Arc;

use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::OptionLabelContext;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::CppFragmentProjection;
use super::SlugConfiguration;
use super::SlugConfigurationError;
use super::host::ActionEnvironmentHost;
use super::host::ActionEnvironmentHostOs;
use super::host::AutoCpuToken;
use super::host::HostConversionInputs;
use super::host::HostPathFlavor;
use crate::CommandConfigurationOccurrence;
use crate::CommandConfigurationOverlay;
use crate::NativeCommandOption;

const CORE: &str = "com.google.devtools.build.lib.analysis.config.CoreOptions";
const CPP: &str = "com.google.devtools.build.lib.rules.cpp.CppOptions";

fn base(os: Option<ActionEnvironmentHostOs>) -> SlugConfiguration {
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
fn native(option: NativeCommandOption, text: &str) -> CommandConfigurationOccurrence {
    CommandConfigurationOccurrence::native(option, Some(text), false)
}
fn apply(
    base: &SlugConfiguration,
    inputs: Vec<CommandConfigurationOccurrence>,
) -> SlugConfiguration {
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("cc-feature-tests").unwrap());
    base.with_command_configuration_context(
        base.starlark_options().clone(),
        &CommandConfigurationOverlay::from(inputs),
        OptionLabelContext::MainRepository { mapping: &mapping },
    )
    .unwrap()
}
fn strings<'a>(config: &'a SlugConfiguration, name: &'static str) -> Vec<&'a str> {
    config
        .native_string_list_option(CORE, name)
        .unwrap()
        .iter()
        .collect()
}

#[test]
fn link_projection_strip_and_fission_follow_compilation_mode_matrix() {
    let base = base(None);
    for mode in ["fastbuild", "dbg", "opt"] {
        for strip in ["always", "sometimes", "never"] {
            for (fission, enabled_modes) in [
                ("no", vec![]),
                ("yes", vec!["fastbuild", "dbg", "opt"]),
                ("DBG,fastbuild,dbg", vec!["dbg", "fastbuild"]),
                ("opt", vec!["opt"]),
            ] {
                let config = apply(
                    &base,
                    vec![
                        native(NativeCommandOption::CompilationMode, mode),
                        native(NativeCommandOption::Strip, strip),
                        native(NativeCommandOption::Fission, fission),
                    ],
                );
                let projection = CppFragmentProjection::new(config).unwrap();
                assert_eq!(
                    projection.should_strip_binaries().unwrap(),
                    strip == "always" || (strip == "sometimes" && mode == "fastbuild"),
                    "{mode}/{strip}"
                );
                assert_eq!(
                    projection
                        .fission_active_for_current_compilation_mode()
                        .unwrap(),
                    enabled_modes.contains(&mode),
                    "{mode}/{fission}"
                );
            }
        }
    }
    let default = CppFragmentProjection::new(base.clone()).unwrap();
    assert!(default.should_strip_binaries().unwrap());
    assert!(
        !default
            .fission_active_for_current_compilation_mode()
            .unwrap()
    );
    let last = apply(
        &base,
        vec![
            native(NativeCommandOption::Fission, "yes"),
            native(NativeCommandOption::Fission, "opt"),
            native(NativeCommandOption::Strip, "always"),
            native(NativeCommandOption::Strip, "NEVER"),
        ],
    );
    let last = CppFragmentProjection::new(last).unwrap();
    assert!(!last.fission_active_for_current_compilation_mode().unwrap());
    assert!(!last.should_strip_binaries().unwrap());
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("link-validation").unwrap());
    for (option, value) in [
        (NativeCommandOption::Fission, "YES"),
        (NativeCommandOption::Fission, "No"),
        (NativeCommandOption::Fission, "dbg,"),
        (NativeCommandOption::Fission, ""),
        (NativeCommandOption::Strip, "sometimes,never"),
        (NativeCommandOption::Strip, ""),
    ] {
        assert!(
            base.with_command_configuration_context(
                base.starlark_options().clone(),
                &CommandConfigurationOverlay::from(vec![native(option, value)]),
                OptionLabelContext::MainRepository { mapping: &mapping }
            )
            .is_err(),
            "{option:?}={value}"
        );
    }
}

#[test]
fn link_projection_preserves_raw_flags_and_omitfp_derivation_structurally() {
    let base = base(None);
    let inputs = vec![
        native(NativeCommandOption::Linkopt, "-Wl,a,b"),
        native(NativeCommandOption::Linkopt, ""),
        native(NativeCommandOption::Linkopt, "-Wl,a,b"),
        native(NativeCommandOption::Linkopt, "-Wl,--eh-frame-hdr"),
    ];
    let configured = apply(&base, inputs.clone());
    let flags = |config| {
        CppFragmentProjection::new(config)
            .unwrap()
            .linkopts()
            .unwrap()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert!(flags(base.clone()).is_empty());
    assert_eq!(
        flags(configured.clone()),
        ["-Wl,a,b", "", "-Wl,a,b", "-Wl,--eh-frame-hdr"]
    );
    let omitfp = apply(
        &configured,
        vec![native(NativeCommandOption::ExperimentalOmitfp, "true")],
    );
    assert_eq!(
        flags(omitfp.clone()),
        [
            "-Wl,a,b",
            "",
            "-Wl,a,b",
            "-Wl,--eh-frame-hdr",
            "-Wl,--eh-frame-hdr"
        ]
    );
    assert_eq!(
        flags(apply(
            &omitfp,
            vec![native(NativeCommandOption::ExperimentalOmitfp, "false")]
        )),
        flags(configured.clone())
    );
    assert_ne!(configured, omitfp);
    assert_ne!(configured.canonical_bytes(), omitfp.canonical_bytes());
    assert_eq!(configured, apply(&base, inputs));
    for (option, value) in [
        (NativeCommandOption::Fission, "dbg"),
        (NativeCommandOption::Strip, "never"),
        (NativeCommandOption::Linkopt, "one"),
        (NativeCommandOption::HostLinkopt, "host"),
        (NativeCommandOption::ExperimentalOmitfp, "true"),
    ] {
        let changed = apply(&base, vec![native(option, value)]);
        assert_ne!(base, changed, "{option:?}");
        assert_ne!(
            base.canonical_bytes(),
            changed.canonical_bytes(),
            "{option:?}"
        );
    }
}

#[test]
fn both_exec_paths_reset_link_modes_remap_host_flags_and_preserve_omitfp() {
    let base = base(Some(ActionEnvironmentHostOs::Linux));
    let target = apply(
        &base,
        vec![
            native(NativeCommandOption::Fission, "yes"),
            native(NativeCommandOption::Strip, "never"),
            native(NativeCommandOption::Linkopt, "target"),
            native(NativeCommandOption::HostLinkopt, "host"),
            native(NativeCommandOption::HostLinkopt, ""),
            native(NativeCommandOption::HostLinkopt, "host"),
            native(NativeCommandOption::ExperimentalOmitfp, "true"),
        ],
    );
    let platform = CanonicalLabel::parse("@@//platform:exec").unwrap();
    let selected = target.to_exec_for_platform(&platform).unwrap();
    for config in [target.to_exec().unwrap(), selected.clone()] {
        let projection = CppFragmentProjection::new(config.clone()).unwrap();
        assert!(
            !projection
                .fission_active_for_current_compilation_mode()
                .unwrap()
        );
        assert!(projection.should_strip_binaries().unwrap());
        assert_eq!(
            projection.linkopts().unwrap().collect::<Vec<_>>(),
            ["host", "", "host", "-Wl,--eh-frame-hdr"]
        );
        assert_eq!(
            config
                .native_string_list_option(CPP, "host_linkopt")
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            ["host", "", "host"]
        );
    }
    assert_eq!(selected, selected.to_exec_for_platform(&platform).unwrap());
    let target = CppFragmentProjection::new(target).unwrap();
    assert!(
        target
            .fission_active_for_current_compilation_mode()
            .unwrap()
    );
    assert!(!target.should_strip_binaries().unwrap());
    assert_eq!(
        target.linkopts().unwrap().collect::<Vec<_>>(),
        ["target", "-Wl,--eh-frame-hdr"]
    );
}

#[test]
fn feature_occurrences_are_raw_repeated_strings_and_structural() {
    let base = base(None);
    assert!(strings(&base, "features").is_empty());
    let changed = apply(
        &base,
        vec![
            native(NativeCommandOption::Features, "a,b"),
            native(NativeCommandOption::Features, "-c"),
            native(NativeCommandOption::Features, "a,b"),
            native(NativeCommandOption::Features, ""),
            native(NativeCommandOption::HostFeatures, "host,a"),
            native(NativeCommandOption::HostFeatures, "null"),
        ],
    );
    assert_eq!(strings(&changed, "features"), ["a,b", "-c", "a,b", ""]);
    assert_eq!(strings(&changed, "host_features"), ["host,a", "null"]);
    let split = apply(
        &base,
        vec![
            native(NativeCommandOption::Features, "a"),
            native(NativeCommandOption::Features, "b"),
        ],
    );
    assert_ne!(changed, split);
    let repeated = apply(
        &changed,
        vec![native(NativeCommandOption::Features, "last")],
    );
    assert_eq!(
        strings(&repeated, "features"),
        ["a,b", "-c", "a,b", "", "last"]
    );
    assert_eq!(apply(&base, vec![]), base);
}

#[test]
fn cpp_feature_defaults_overlays_labels_and_instrumentation_are_typed() {
    let base = base(Some(ActionEnvironmentHostOs::Linux));
    let projection = CppFragmentProjection::new(base.clone()).unwrap();
    assert!(!projection.force_pic().unwrap());
    assert!(!projection.apple_generate_dsym().unwrap());
    assert!(projection.dont_enable_host_nonhost().unwrap());
    assert!(!projection.use_llvm_coverage_map_format().unwrap());
    assert_eq!(projection.fdo_instrument().unwrap(), None);
    assert_eq!(projection.cs_fdo_instrument().unwrap(), None);
    assert_eq!(projection.fdo_prefetch_hints_label().unwrap(), None);
    assert!(!base.coverage_enabled().unwrap());
    let changed = apply(
        &base,
        vec![
            native(NativeCommandOption::ForcePic, "true"),
            native(NativeCommandOption::AppleGenerateDsym, "yes"),
            native(NativeCommandOption::ExperimentalUseLlvmCovmap, "1"),
            native(
                NativeCommandOption::IncompatibleDontEnableHostNonhostCrosstoolFeatures,
                "false",
            ),
            native(NativeCommandOption::FdoInstrument, "/raw/instrument/path"),
            native(NativeCommandOption::CsFdoInstrument, "null"),
            native(NativeCommandOption::FdoPrefetchHints, "//profiles:hints"),
            native(NativeCommandOption::CollectCodeCoverage, "true"),
        ],
    );
    let projection = CppFragmentProjection::new(changed.clone()).unwrap();
    assert!(projection.force_pic().unwrap());
    assert!(projection.apple_generate_dsym().unwrap());
    assert!(!projection.dont_enable_host_nonhost().unwrap());
    assert!(projection.use_llvm_coverage_map_format().unwrap());
    assert_eq!(
        projection.fdo_instrument().unwrap(),
        Some("/raw/instrument/path")
    );
    assert_eq!(projection.cs_fdo_instrument().unwrap(), Some("null"));
    assert_eq!(
        projection.fdo_prefetch_hints_label().unwrap(),
        Some(CanonicalLabel::parse("@@//profiles:hints").unwrap())
    );
    assert!(changed.coverage_enabled().unwrap());
    assert_eq!(
        changed
            .native_string_list_option(CPP, "copt")
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        ["-Wno-error", "-Wno-error"]
    );
    assert_ne!(base.canonical_bytes(), changed.canonical_bytes());
    assert_eq!(
        CppFragmentProjection::new(base.clone())
            .unwrap()
            .force_pic()
            .unwrap(),
        false
    );
    assert_eq!(apply(&base, vec![]), base);
}

#[test]
fn both_exec_paths_share_feature_mapping_resets_and_propagation() {
    let original = base(Some(ActionEnvironmentHostOs::Freebsd));
    let target = apply(
        &original,
        vec![
            native(NativeCommandOption::Features, "target"),
            native(NativeCommandOption::HostFeatures, "host,raw"),
            native(NativeCommandOption::HostFeatures, "-disabled"),
            native(NativeCommandOption::ForcePic, "true"),
            native(NativeCommandOption::AppleGenerateDsym, "true"),
            native(NativeCommandOption::ExperimentalUseLlvmCovmap, "true"),
            native(
                NativeCommandOption::IncompatibleDontEnableHostNonhostCrosstoolFeatures,
                "false",
            ),
            native(NativeCommandOption::FdoInstrument, "instrument"),
            native(NativeCommandOption::CsFdoInstrument, "cs-instrument"),
            native(NativeCommandOption::FdoPrefetchHints, "//profiles:hints"),
            native(NativeCommandOption::CollectCodeCoverage, "true"),
            native(NativeCommandOption::CustomMalloc, "//malloc:custom"),
        ],
    );
    let platform = CanonicalLabel::parse("@@//platform:exec").unwrap();
    let exec = target.to_exec().unwrap();
    let platform_exec = target.to_exec_for_platform(&platform).unwrap();
    for value in [&exec, &platform_exec] {
        let cpp = CppFragmentProjection::new(value.clone()).unwrap();
        assert_eq!(strings(value, "features"), ["host,raw", "-disabled"]);
        assert_eq!(strings(value, "host_features"), ["host,raw", "-disabled"]);
        assert!(!cpp.force_pic().unwrap());
        assert!(!cpp.apple_generate_dsym().unwrap());
        assert!(!value.coverage_enabled().unwrap());
        assert_eq!(cpp.fdo_instrument().unwrap(), None);
        assert_eq!(cpp.cs_fdo_instrument().unwrap(), None);
        assert_eq!(cpp.fdo_prefetch_hints_label().unwrap(), None);
        assert!(cpp.use_llvm_coverage_map_format().unwrap());
        assert!(!cpp.dont_enable_host_nonhost().unwrap());
        assert_eq!(
            value.execution_host_os().unwrap(),
            ActionEnvironmentHostOs::Freebsd
        );
        // Explicitly preserve unrelated previously admitted behavior.
        assert_eq!(
            cpp.custom_malloc().unwrap(),
            Some(CanonicalLabel::parse("@@//malloc:custom").unwrap())
        );
        assert_eq!(
            value
                .native_string_list_option(CPP, "copt")
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            // Compile options now follow their own exec remapping: target
            // FDO-injected options do not replace the host compile flags.
            ["-g0"]
        );
    }
    assert!(exec.to_exec().is_err());
    assert_eq!(
        platform_exec.to_exec_for_platform(&platform).unwrap(),
        platform_exec
    );
    let other = CanonicalLabel::parse("@@//platform:other").unwrap();
    assert_eq!(
        platform_exec.to_exec_for_platform(&other).unwrap(),
        target.to_exec_for_platform(&other).unwrap()
    );
    assert_eq!(strings(&target, "features"), ["target"]);
    assert!(target.coverage_enabled().unwrap());
}

#[test]
fn structural_host_os_is_required_and_not_inferred_from_cpu_or_path_flavor() {
    assert_eq!(
        base(None).execution_host_os(),
        Err(SlugConfigurationError::MissingExecutionHostOs)
    );
    let linux = base(Some(ActionEnvironmentHostOs::Linux));
    for os in [
        ActionEnvironmentHostOs::Windows,
        ActionEnvironmentHostOs::Macos,
        ActionEnvironmentHostOs::Freebsd,
        ActionEnvironmentHostOs::Openbsd,
        ActionEnvironmentHostOs::Unknown,
    ] {
        let config = base(Some(os));
        assert_eq!(config.execution_host_os().unwrap(), os);
        assert_ne!(config, linux);
        assert_eq!(config.to_exec().unwrap().execution_host_os().unwrap(), os);
    }
}
