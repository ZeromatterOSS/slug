use super::CacheFieldValue;
use super::NATIVE_OPTION_DESCRIPTORS;
use super::NativeOptionDescriptor;
use super::format_cache_field;

/// Deliberately independent, compact pinned source expectation rows. Do not
/// derive this table from the production registry: it guards every field and
/// its cache-key position against an accidental registry edit.
#[derive(Clone, Copy, Debug)]
struct ExpectedDescriptor {
    class_name: &'static str,
    canonical_name: &'static str,
    field_type: &'static str,
    raw_default: &'static str,
    converter: Option<&'static str>,
    allow_multiple: bool,
    old_name: Option<&'static str>,
    expansion: Option<&'static str>,
    implicit_requirements: Option<&'static str>,
    normalizer: &'static str,
}

mod command_configuration_tests {
    use std::mem::size_of;
    use std::sync::Arc;

    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_identity_v2::OptionLabelContext;
    use slug_identity_v2::RepositoryMapping;
    use slug_identity_v2::RepositoryMappingId;

    use super::super::configuration::OptionValue;
    use super::super::configuration::SlugConfiguration;
    use super::super::host::AutoCpuToken;
    use super::super::host::HostConversionInputs;
    use super::super::host::HostPathFlavor;
    use super::super::label_convert::LabelValue;
    use super::super::value::NativeOccurrence;
    use super::super::value::NativeValue;
    use crate::CommandConfigurationOccurrence;
    use crate::CommandConfigurationOverlay;
    use crate::NativeCommandOption;

    const PLATFORM_OPTIONS: &str = "com.google.devtools.build.lib.analysis.PlatformOptions";

    fn configuration() -> SlugConfiguration {
        SlugConfiguration::default_target(
            &HostConversionInputs::new(
                Some(AutoCpuToken::K8),
                Some(HostPathFlavor::Unix),
                None,
                Arc::from([]),
                Arc::from([]),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn mapping() -> RepositoryMapping {
        let mut mapping = RepositoryMapping::new(RepositoryMappingId::new("command-test").unwrap());
        mapping.insert(
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new("mapped+1.0").unwrap(),
        );
        mapping
    }

    fn native(
        option: NativeCommandOption,
        value: impl Into<String>,
    ) -> CommandConfigurationOccurrence {
        CommandConfigurationOccurrence::native(option, Some(value.into()), false)
    }

    fn value<'a>(configuration: &'a SlugConfiguration, name: &str) -> &'a OptionValue {
        &configuration
            .option_records()
            .iter()
            .find(|record| record.canonical_name == name)
            .unwrap()
            .value
    }

    fn native_text<'a>(configuration: &'a SlugConfiguration, name: &str) -> Option<&'a str> {
        match value(configuration, name) {
            OptionValue::Native(NativeOccurrence::Absent) => None,
            OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text(value))) => {
                Some(value.as_str())
            }
            other => panic!("expected native text for {name}, got {other:?}"),
        }
    }

    fn label(configuration: &SlugConfiguration, name: &str) -> Option<String> {
        match value(configuration, name) {
            OptionValue::Label(None) => None,
            OptionValue::Label(Some(LabelValue::Label(value))) => Some(value.to_string()),
            other => panic!("expected label for {name}, got {other:?}"),
        }
    }

    #[test]
    fn overlay_is_arc_shared_and_batch_native_normalization_is_structural() {
        let overlay: CommandConfigurationOverlay = vec![
            CommandConfigurationOccurrence::extra_toolchains("a,b,a"),
            CommandConfigurationOccurrence::extra_execution_platforms("x,,y"),
            CommandConfigurationOccurrence::extra_toolchains("c,b"),
            CommandConfigurationOccurrence::extra_execution_platforms("z,"),
        ]
        .into();
        let copy = overlay.clone();
        assert_eq!(overlay.as_ptr(), copy.as_ptr());

        let base = configuration();
        let native = base.prepare_command_native_options(&overlay).unwrap();
        let prepared = SlugConfiguration::with_prepared_command_configuration(
            native,
            base.starlark_options().clone(),
        );
        assert_eq!(
            prepared
                .native_string_list_option(PLATFORM_OPTIONS, "extra_toolchains")
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            ["a", "c", "b"]
        );
        assert_eq!(
            prepared
                .native_string_list_option(PLATFORM_OPTIONS, "extra_execution_platforms")
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            ["z", ""]
        );
        assert_ne!(prepared.canonical_bytes(), base.canonical_bytes());

        let unchanged = base
            .with_command_configuration(
                base.starlark_options().clone(),
                &CommandConfigurationOverlay::default(),
            )
            .unwrap();
        assert_eq!(
            unchanged.canonical_bytes().as_ptr(),
            base.canonical_bytes().as_ptr()
        );
    }

    #[test]
    fn target_platform_falls_back_to_host_and_exec_projection_installs_selection() {
        let target = configuration();
        assert_eq!(
            target.target_platform_label().unwrap().to_string(),
            "@@bazel_tools//tools:host_platform"
        );
        assert_eq!(
            target.host_platform_label().unwrap().to_string(),
            "@@bazel_tools//tools:host_platform"
        );

        let linux = CanonicalLabel::parse("@@platforms//host:host").unwrap();
        let remote = CanonicalLabel::parse("@@platforms//host:remote").unwrap();
        let overridden = target.with_host_platform_label(&linux);
        assert_eq!(overridden.host_platform_label().unwrap(), linux);
        assert_eq!(overridden.target_platform_label().unwrap(), linux);
        let exec = target.to_exec_for_platform(&linux).unwrap();
        let other = target.to_exec_for_platform(&remote).unwrap();
        assert_eq!(exec.target_platform_label().unwrap(), linux);
        assert_eq!(exec.host_platform_label(), target.host_platform_label());
        assert_ne!(exec.projection(), target.projection());
        assert_ne!(exec.projection(), other.projection());
        assert_eq!(exec, target.to_exec_for_platform(&linux).unwrap());
        assert_eq!(exec.to_exec_for_platform(&remote).unwrap(), other);
    }

    #[test]
    fn typed_fdo_command_closure_uses_descriptor_conversion_and_root_mapping() {
        let mapping = mapping();
        let overlay: CommandConfigurationOverlay = vec![
            native(NativeCommandOption::FdoOptimize, "//profiles:opt"),
            native(NativeCommandOption::XbinaryFdo, "@alias//profiles:xbinary"),
            native(NativeCommandOption::FdoProfile, ""),
            native(NativeCommandOption::CsFdoProfile, "//profiles:cs"),
            native(NativeCommandOption::FdoPrefetchHints, "//profiles:prefetch"),
            native(
                NativeCommandOption::PropellerOptimize,
                "@alias//profiles:propeller",
            ),
            native(NativeCommandOption::MemprofProfile, "//profiles:memprof"),
            native(NativeCommandOption::ProtoProfilePath, "//profiles:proto"),
            native(NativeCommandOption::GrteTop, "//libc"),
            native(NativeCommandOption::FdoInstrument, "instrument"),
            native(NativeCommandOption::CsFdoInstrument, "cs-instrument"),
            CommandConfigurationOccurrence::native(
                NativeCommandOption::CollectCodeCoverage,
                None::<&str>,
                false,
            ),
        ]
        .into();
        let base = configuration();
        let prepared = base
            .with_command_configuration_context(
                base.starlark_options().clone(),
                &overlay,
                OptionLabelContext::MainRepository { mapping: &mapping },
            )
            .unwrap();

        assert_eq!(
            native_text(&prepared, "fdo_optimize"),
            Some("//profiles:opt")
        );
        assert_eq!(native_text(&prepared, "fdo_instrument"), Some("instrument"));
        assert_eq!(
            native_text(&prepared, "cs_fdo_instrument"),
            Some("cs-instrument")
        );
        assert!(matches!(
            value(&prepared, "collect_code_coverage"),
            OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Bool(true)))
        ));
        assert_eq!(
            label(&prepared, "xbinary_fdo").as_deref(),
            Some("@@mapped+1.0//profiles:xbinary")
        );
        assert_eq!(label(&prepared, "fdo_profile"), None);
        assert_eq!(
            label(&prepared, "cs_fdo_profile").as_deref(),
            Some("//profiles:cs")
        );
        assert_eq!(
            label(&prepared, "fdo_prefetch_hints").as_deref(),
            Some("//profiles:prefetch")
        );
        assert_eq!(
            label(&prepared, "propeller_optimize").as_deref(),
            Some("@@mapped+1.0//profiles:propeller")
        );
        assert_eq!(
            label(&prepared, "memprof_profile").as_deref(),
            Some("//profiles:memprof")
        );
        assert_eq!(
            label(&prepared, "proto_profile_path").as_deref(),
            Some("//profiles:proto")
        );
        assert_eq!(
            label(&prepared, "grte_top").as_deref(),
            Some("//libc:everything")
        );

        let default_libc: CommandConfigurationOverlay =
            vec![native(NativeCommandOption::GrteTop, "default")].into();
        let reset = base
            .with_command_configuration_context(
                base.starlark_options().clone(),
                &default_libc,
                OptionLabelContext::MainRepository { mapping: &mapping },
            )
            .unwrap();
        assert_eq!(label(&reset, "grte_top"), None);

        let selector_overlay: CommandConfigurationOverlay = vec![
            native(NativeCommandOption::FdoOptimize, "//profiles:opt"),
            native(NativeCommandOption::FdoProfile, "//profiles:profile"),
            CommandConfigurationOccurrence::native(
                NativeCommandOption::CollectCodeCoverage,
                None::<&str>,
                false,
            ),
            native(NativeCommandOption::FdoInstrument, "instrument"),
        ]
        .into();
        let selector = base
            .with_command_configuration_context(
                base.starlark_options().clone(),
                &selector_overlay,
                OptionLabelContext::MainRepository { mapping: &mapping },
            )
            .unwrap();
        for (name, expected) in [
            ("fdo_optimize", "//profiles:opt"),
            ("fdo_profile", "//profiles:profile"),
            ("collect_code_coverage", "true"),
            ("copt", "-Wno-error"),
        ] {
            assert!(
                selector
                    .matches_config_setting(&[(name.into(), expected.into())], &[])
                    .unwrap(),
                "selector mismatch for {name}"
            );
        }
    }

    #[test]
    fn direct_then_implicit_order_last_wins_and_failed_batches_do_not_publish() {
        // Bazel 9.2 OptionsParserImpl records the direct option before parsing
        // its child-priority implicit requirements. Both instrumentation
        // descriptors source --copt=-Wno-error from the pinned registry.
        let overlay: CommandConfigurationOverlay = vec![
            native(NativeCommandOption::Copt, "before"),
            native(NativeCommandOption::FdoInstrument, "first"),
            native(NativeCommandOption::Copt, "after"),
            native(NativeCommandOption::CsFdoInstrument, "null"),
            native(NativeCommandOption::FdoInstrument, "last"),
            CommandConfigurationOccurrence::native(
                NativeCommandOption::CollectCodeCoverage,
                None::<&str>,
                true,
            ),
        ]
        .into();
        let base = configuration();
        let changed = base
            .with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap();
        assert_eq!(native_text(&changed, "fdo_instrument"), Some("last"));
        assert!(matches!(
            value(&changed, "collect_code_coverage"),
            OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Bool(false)))
        ));
        let OptionValue::Native(NativeOccurrence::List(copts)) = value(&changed, "copt") else {
            panic!("expected repeated copt list")
        };
        assert_eq!(
            copts
                .0
                .iter()
                .map(|value| match value {
                    NativeValue::Text(value) => value.as_str(),
                    other => panic!("expected copt text, got {other:?}"),
                })
                .collect::<Vec<_>>(),
            ["before", "-Wno-error", "after", "-Wno-error", "-Wno-error"]
        );

        let restored = base
            .with_command_configuration(
                base.starlark_options().clone(),
                &CommandConfigurationOverlay::default(),
            )
            .unwrap();
        assert_eq!(restored.canonical_bytes(), base.canonical_bytes());
        assert_eq!(
            restored.canonical_bytes().as_ptr(),
            base.canonical_bytes().as_ptr()
        );
        assert_ne!(changed.canonical_bytes(), base.canonical_bytes());

        let invalid: CommandConfigurationOverlay = vec![
            native(NativeCommandOption::FdoOptimize, "//profiles:valid"),
            CommandConfigurationOccurrence::native(
                NativeCommandOption::CollectCodeCoverage,
                Some("not-a-bool"),
                false,
            ),
        ]
        .into();
        assert!(base.prepare_command_native_options(&invalid).is_err());
        assert_eq!(native_text(&base, "fdo_optimize"), None);
        assert_eq!(size_of::<NativeCommandOption>(), 1);
        assert!(
            size_of::<CommandConfigurationOccurrence>() <= 64,
            "occurrence size: {}",
            size_of::<CommandConfigurationOccurrence>()
        );
        assert!(
            size_of::<CommandConfigurationOverlay>() <= 2 * size_of::<usize>(),
            "overlay size: {}",
            size_of::<CommandConfigurationOverlay>()
        );
    }
}

macro_rules! expected_descriptor {
    ($class:expr; $name:expr; $field_type:expr; $raw_default:expr; $converter:expr; $allow_multiple:expr; $old_name:expr; $expansion:expr; $implicit_requirements:expr; $normalizer:expr) => {
        ExpectedDescriptor {
            class_name: $class,
            canonical_name: $name,
            field_type: $field_type,
            raw_default: $raw_default,
            converter: $converter,
            allow_multiple: $allow_multiple,
            old_name: $old_name,
            expansion: $expansion,
            implicit_requirements: $implicit_requirements,
            normalizer: $normalizer,
        }
    };
}

#[rustfmt::skip]
const EXPECTED: &[ExpectedDescriptor] = &[
    expected_descriptor!("com.google.devtools.build.lib.analysis.PlatformOptions"; "extra_execution_platforms"; "List<String>"; "\"\""; Some("CommaSeparatedOptionListConverter.class"); false; None; None; None; "P"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.PlatformOptions"; "extra_toolchains"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; None; None; None; "P"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.PlatformOptions"; "host_platform"; "Label"; "DEFAULT_HOST_PLATFORM"; Some("HostPlatformConverter.class"); false; Some("\"experimental_host_platform\""); None; None; "P"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.PlatformOptions"; "incompatible_use_toolchain_resolution_for_java_rules"; "boolean"; "\"true\""; None; false; None; None; None; "P"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.PlatformOptions"; "platform_mappings"; "PlatformMappingKey"; "\"\""; Some("PlatformMappingKeyConverter.class"); false; None; None; None; "P"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.PlatformOptions"; "platforms"; "List<Label>"; "\"\""; Some("LabelListConverter.class"); false; Some("\"experimental_platforms\""); None; None; "P"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.PlatformOptions"; "toolchain_resolution_debug"; "RegexFilter"; "\"-.*\""; Some("RegexFilter.RegexFilterConverter.class"); false; None; None; None; "P"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.ShellConfiguration.Options"; "shell_executable"; "PathFragment"; "\"null\""; Some("PathFragmentConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "action_env"; "List<Converters.EnvVar>"; "\"null\""; Some("Converters.EnvVarsConverter.class"); true; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "affected by starlark transition"; "List<String>"; "\"\""; Some("EmptyListConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "allow_analysis_failures"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "allow_unresolved_symlinks"; "boolean"; "\"true\""; None; false; Some("\"experimental_allow_unresolved_symlinks\""); None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "allowed_cpu_values"; "ImmutableList<String>"; "\"\""; Some("CommaSeparatedOptionSetConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "analysis_testing_deps_limit"; "int"; "\"2000\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "archived_tree_artifact_mnemonics_filter"; "RegexFilter"; "\"-.*\""; Some("RegexFilter.RegexFilterConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "build_runfile_links"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "build_runfile_manifests"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "check_licenses"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "check_visibility"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "collect_code_coverage"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "compilation_mode"; "CompilationMode"; "\"fastbuild\""; Some("CompilationMode.Converter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "cpu"; "String"; "\"\""; Some("AutoCpuConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "define"; "List<Map.Entry<String, String>>"; "\"null\""; Some("Converters.AssignmentConverter.class"); true; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "enable_runfiles"; "TriState"; "\"auto\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "enforce_constraints"; "boolean"; "\"true\""; None; false; Some("\"experimental_enforce_constraints\""); None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "evaluating for analysis test"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "exec_aspects"; "List<String>"; "\"null\""; Some("Converters.CommaSeparatedOptionListConverter.class"); true; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_action_listener"; "List<Label>"; "\"null\""; Some("LabelListConverter.class"); true; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_allow_map_directory"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_collect_code_coverage_for_generated_files"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_debug_selects_always_succeed"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_enforce_transitive_visibility"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_exclude_defines_from_exec_config"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_exec_config"; "String"; "\"@_builtins//:common/builtin_exec_platforms.bzl%bazel_exec_transition\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_exec_configuration_distinguisher"; "ExecConfigurationDistinguisherScheme"; "\"off\""; Some("ExecConfigurationDistinguisherSchemeConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_extended_sanity_checks"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_output_directory_naming_scheme"; "OutputDirectoryNamingScheme"; "\"diff_against_dynamic_baseline\""; Some("OutputDirectoryNamingSchemeConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_output_paths"; "OutputPathsMode"; "\"off\""; Some("OutputPathsConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_override_platform_cpu_name"; "List<Map.Entry<Label, String>>"; "\"null\""; Some("LabelToStringEntryConverter.class"); true; Some("\"experimental_override_name_platform_in_output_dir\""); None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_platform_in_output_dir"; "TriState"; "\"Auto\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_propagate_custom_flag"; "List<String>"; "\"null\""; Some("CoreOptionConverters.CustomFlagConverter.class"); true; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_remotable_source_manifests"; "boolean"; "\"false\""; Some("BooleanConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_strict_fileset_output"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_throttle_action_cache_check"; "boolean"; "\"true\""; Some("BooleanConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_use_platforms_in_output_dir_legacy_heuristic"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_writable_outputs"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "features"; "List<String>"; "\"null\""; None; true; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "flag_alias"; "List<Map.Entry<String, Label>>"; "\"null\""; Some("CoreOptionConverters.FlagAliasConverter.class"); true; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "host_action_env"; "List<Converters.EnvVar>"; "\"null\""; Some("Converters.EnvVarsConverter.class"); true; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "host_compilation_mode"; "CompilationMode"; "\"opt\""; Some("CompilationMode.Converter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "host_cpu"; "String"; "\"\""; Some("AutoCpuConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "host_features"; "List<String>"; "\"null\""; None; true; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "include_config_fragments_provider"; "IncludeConfigFragmentsEnum"; "\"off\""; Some("IncludeConfigFragmentsEnumConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_always_include_files_in_data"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_auto_exec_groups"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_bazel_test_exec_run_under"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_bep_cpu_from_platform"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_check_testonly_for_output_files"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_compact_repo_mapping_manifest"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_disable_select_on"; "ImmutableList<String>"; "\"\""; Some("CommaSeparatedOptionSetConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_exclude_starlark_flags_from_exec_config"; "boolean"; "\"false\""; None; false; Some("\"experimental_exclude_starlark_flags_from_exec_config\""); None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_filegroup_runfiles_for_data"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_limit_platforms_in_output_dir_to"; "List<Label>"; "\"\""; Some("LabelListConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_merge_genfiles_directory"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_modify_execution_info_additive"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_target_cpu_from_platform"; "boolean"; "\"true\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "instrument_test_targets"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "instrumentation_filter"; "RegexFilter"; "\"-/javatests[/:],-/test/java[/:]\""; Some("RegexFilter.RegexFilterConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "is exec configuration"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "min_param_file_size"; "int"; "\"32768\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "modify_execution_info"; "List<ExecutionInfoModifier>"; "\"null\""; Some("ExecutionInfoModifier.Converter.class"); true; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "platform_suffix"; "String"; "\"null\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "run_under"; "RunUnder"; "\"null\""; Some("RunUnderConverter.class"); false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "scl_config"; "String"; "\"null\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "stamp"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "strict_filesets"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "target_environment"; "List<Label>"; "\"null\""; Some("LabelListConverter.class"); true; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "use_target_platform_for_tests"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.config.CoreOptions"; "verbose_visibility_errors"; "boolean"; "\"false\""; None; false; None; None; None; "C"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.CoverageConfiguration.CoverageOptions"; "coverage_output_generator"; "Label"; "\"@bazel_tools//tools/test:lcov_merger\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.CoverageConfiguration.CoverageOptions"; "coverage_report_generator"; "Label"; "\"@bazel_tools//tools/test:coverage_report_generator\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "allow_local_tests"; "boolean"; "\"true\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "cache_test_results"; "TriState"; "\"auto\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "coverage_support"; "Label"; "\"@bazel_tools//tools/test:coverage_support\""; Some("LabelConverter.class"); false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "default_test_resources"; "List<Pair<String, Map<TestSize, Double>>>"; "\"null\""; Some("TestResourcesConverter.class"); true; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "experimental_cancel_concurrent_tests"; "CancelConcurrentTests"; "\"never\""; Some("CancelConcurrentTests.Converter.class"); false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "experimental_fetch_all_coverage_outputs"; "boolean"; "\"false\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "experimental_retain_test_configuration_across_testonly"; "boolean"; "\"true\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "experimental_split_coverage_postprocessing"; "boolean"; "\"false\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "incompatible_check_sharding_support"; "boolean"; "\"true\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "incompatible_exclusive_test_sandboxed"; "boolean"; "\"true\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "runs_per_test"; "List<PerLabelOptions>"; "\"1\""; Some("RunsPerTestConverter.class"); true; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "runs_per_test_detects_flakes"; "boolean"; "\"false\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_arg"; "List<String>"; "\"null\""; None; true; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_env"; "List<Converters.EnvVar>"; "\"null\""; Some("Converters.EnvVarsConverter.class"); true; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_filter"; "String"; "\"null\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_result_expiration"; "int"; "\"-1\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_runner_fail_fast"; "boolean"; "\"false\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_sharding_strategy"; "TestShardingStrategy"; "\"explicit\""; Some("ShardingStrategyConverter.class"); false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_timeout"; "Map<TestTimeout, Duration>"; "\"-1\""; Some("TestTimeout.TestTimeoutConverter.class"); false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "trim_test_configuration"; "boolean"; "\"true\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "zip_undeclared_test_outputs"; "boolean"; "\"false\""; None; false; None; None; None; "T"),
    expected_descriptor!("com.google.devtools.build.lib.bazel.rules.BazelRuleClassProvider.StrictActionEnvOptions"; "incompatible_strict_action_env"; "boolean"; "\"true\""; None; false; Some("\"experimental_strict_action_env\""); None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.bazel.rules.python.BazelPythonConfiguration.Options"; "experimental_python_import_all_repositories"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.bazel.rules.python.BazelPythonConfiguration.Options"; "incompatible_remove_ctx_bazel_py_fragment"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.bazel.rules.python.BazelPythonConfiguration.Options"; "python_path"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "Android configuration distinguisher"; "ConfigurationDistinguisher"; "\"MAIN\""; Some("ConfigurationDistinguisherConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_compiler"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_databinding_use_androidx"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_databinding_use_v3_4_args"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_dynamic_mode"; "DynamicMode"; "\"off\""; Some("DynamicModeConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_fixed_resource_neverlinking"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_manifest_merger"; "AndroidManifestMerger"; "\"android\""; Some("AndroidManifestMergerConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_manifest_merger_order"; "ManifestMergerOrder"; "\"alphabetical\""; Some("ManifestMergerOrderConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_migration_tag_check"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_platforms"; "List<Label>"; "\"\""; Some("LabelOrderedSetConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_resource_shrinking"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "apk_signing_method"; "ApkSigningMethod"; "\"v1_v2\""; Some("ApkSigningMethodConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "break_build_on_parallel_dex2oat_failure"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "desugar_for_android"; "boolean"; "\"true\""; None; false; Some("\"experimental_desugar_for_android\""); None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "desugar_java8_libs"; "boolean"; "\"false\""; None; false; Some("\"experimental_desugar_java8_libs\""); None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "dexopts_supported_in_dexmerger"; "List<String>"; "\"--minimal-main-dex,--set-max-idx-number\""; Some("Converters.CommaSeparatedOptionListConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "dexopts_supported_in_dexsharder"; "List<String>"; "\"--minimal-main-dex\""; Some("Converters.CommaSeparatedOptionListConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "dexopts_supported_in_incremental_dexing"; "List<String>"; "\"--no-optimize,--no-locals\""; Some("Converters.CommaSeparatedOptionListConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_allow_android_library_deps_without_srcs"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_always_filter_duplicate_classes_from_android_test"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_assume_minsdkversion"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_compress_java_resources"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_databinding_v2"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_library_exports_manifest_default"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_resource_cycle_shrinking"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_resource_name_obfuscation"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_resource_path_shortening"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_resource_shrinking"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_rewrite_dexes_with_rex"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_use_parallel_dex2oat"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_check_desugar_deps"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_disable_instrumentation_manifest_merge"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_filter_library_jar_with_program_jar"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_filter_r_jars_from_android_test"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_get_android_java_resources_from_optimized_jar"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_incremental_dexing_after_proguard"; "int"; "\"50\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_incremental_dexing_after_proguard_by_default"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_omit_resources_info_provider_from_android_binary"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_one_version_enforcement_use_transitive_jars_for_binary_under_test"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_persistent_aar_extractor"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_remove_r_classes_from_instrumentation_test_jar"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_use_dex_splitter_for_incremental_dexing"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_use_rtxt_from_merged_resources"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "fat_apk_hwasan"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "incompatible_disable_native_android_rules"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "incompatible_remove_ctx_android_fragment"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "incremental_dexing"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "internal_persistent_android_dex_desugar"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "internal_persistent_busybox_tools"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "internal_persistent_multiplex_android_dex_desugar"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "internal_persistent_multiplex_busybox_tools"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "legacy_main_dex_list_generator"; "Label"; "\"null\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "non_incremental_per_target_dexopts"; "List<String>"; "\"--positions\""; Some("Converters.CommaSeparatedOptionListConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "optimizing_dexer"; "Label"; "\"null\""; Some("EmptyToNullLabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "output_library_merged_assets"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "persistent_android_dex_desugar"; "Void"; "\"null\""; None; false; None; Some("{ \"--internal_persistent_android_dex_desugar\", \"--strategy=Desugar=worker\", \"--strategy=DexBuilder=worker\", }"); None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "persistent_android_resource_processor"; "Void"; "\"null\""; None; false; None; Some("{ \"--internal_persistent_busybox_tools\", \"--strategy=AaptPackage=worker\", \"--strategy=AndroidResourceParser=worker\", \"--strategy=AndroidResourceValidator=worker\", \"--strategy=AndroidResourceCompiler=worker\", \"--strategy=RClassGenerator=worker\", \"--strategy=AndroidResourceLink=worker\", \"--strategy=AndroidAapt2=worker\", \"--strategy=AndroidAssetMerger=worker\", \"--strategy=AndroidResourceMerger=worker\", \"--strategy=AndroidCompiledResourceMerger=worker\", \"--strategy=ManifestMerger=worker\", \"--strategy=AndroidManifestMerger=worker\", \"--strategy=Aapt2Optimize=worker\", \"--strategy=AARGenerator=worker\", \"--strategy=ProcessDatabinding=worker\", \"--strategy=GenerateDataBindingBaseClasses=worker\" }"); None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "persistent_multiplex_android_dex_desugar"; "Void"; "\"null\""; None; false; None; Some("{ \"--persistent_android_dex_desugar\", \"--internal_persistent_multiplex_android_dex_desugar\", }"); None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "persistent_multiplex_android_resource_processor"; "Void"; "\"null\""; None; false; None; Some("{ \"--persistent_android_resource_processor\", \"--modify_execution_info=AaptPackage=+supports-multiplex-workers\", \"--modify_execution_info=AndroidResourceParser=+supports-multiplex-workers\", \"--modify_execution_info=AndroidResourceValidator=+supports-multiplex-workers\", \"--modify_execution_info=AndroidResourceCompiler=+supports-multiplex-workers\", \"--modify_execution_info=RClassGenerator=+supports-multiplex-workers\", \"--modify_execution_info=AndroidResourceLink=+supports-multiplex-workers\", \"--modify_execution_info=AndroidAapt2=+supports-multiplex-workers\", \"--modify_execution_info=AndroidAssetMerger=+supports-multiplex-workers\", \"--modify_execution_info=AndroidResourceMerger=+supports-multiplex-workers\", \"--modify_execution_info=AndroidCompiledResourceMerger=+supports-multiplex-workers\", \"--modify_execution_info=ManifestMerger=+supports-multiplex-workers\", \"--modify_execution_info=AndroidManifestMerger=+supports-multiplex-workers\", \"--modify_execution_info=Aapt2Optimize=+supports-multiplex-workers\", \"--modify_execution_info=AARGenerator=+supports-multiplex-workers\", }"); None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "persistent_multiplex_android_tools"; "Void"; "\"null\""; None; false; None; Some("{ \"--internal_persistent_multiplex_busybox_tools\", \"--persistent_multiplex_android_resource_processor\", \"--persistent_multiplex_android_dex_desugar\", }"); None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.android.BazelAndroidConfiguration.Options"; "merge_android_manifest_permissions"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "apple configuration distinguisher"; "ConfigurationDistinguisher"; "\"UNKNOWN\""; Some("ConfigurationDistinguisherConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "apple_platform_type"; "String"; "\"macos\""; Some("PlatformTypeConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "apple_platforms"; "List<Label>"; "\"\""; Some("LabelListConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "apple_split_cpu"; "String"; "\"\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "catalyst_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "experimental_include_xcode_execution_requirements"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "experimental_objc_provider_from_linked"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "experimental_prefer_mutual_xcode"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "host_macos_minimum_os"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "incompatible_enable_apple_toolchain_resolution"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "ios_minimum_os"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "ios_multi_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "ios_sdk_version"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "macos_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "macos_minimum_os"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "macos_sdk_version"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "tvos_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "tvos_minimum_os"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "tvos_sdk_version"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "use_platforms_in_apple_crosstool_transition"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "visionos_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "watchos_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "watchos_minimum_os"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "watchos_sdk_version"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "xcode_version"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "xcode_version_config"; "Label"; "\"@bazel_tools//tools/cpp:host_xcodes\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.config.ConfigFeatureFlagOptions"; "all feature flag values are present (internal)"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.config.ConfigFeatureFlagOptions"; "enforce_transitive_configs_for_config_feature_flag"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "apple_generate_dsym"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "build_test_dwp"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "cc_dotd_files"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "cc_include_scanning"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "cc_output_directory_tag"; "String"; "\"\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "compiler"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "conlyopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "copt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "crosstool_top"; "Label"; "\"@bazel_tools//tools/cpp:toolchain\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "cs_fdo_absolute_path"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "cs_fdo_instrument"; "String"; "\"null\""; None; false; None; None; Some("{\"--copt=-Wno-error\"}"); "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "cs_fdo_profile"; "Label"; "\"null\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "custom_malloc"; "Label"; "\"null\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "cxxopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "dynamic_mode"; "DynamicMode"; "\"default\""; Some("DynamicModeConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "enable_propeller_optimize_absolute_paths"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "enable_remaining_fdo_absolute_paths"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_cc_implementation_deps"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_cpp_compile_resource_estimation"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_cpp_modules"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_generate_llvm_lcov"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_inmemory_dotd_files"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_link_static_libraries_once"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_omitfp"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_save_feature_state"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_unsupported_and_brittle_include_scanning"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_use_cpp_compile_action_args_params_file"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_use_llvm_covmap"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "fdo_instrument"; "String"; "\"null\""; None; false; None; None; Some("{\"--copt=-Wno-error\"}"); "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "fdo_optimize"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "fdo_prefetch_hints"; "Label"; "\"null\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "fdo_profile"; "Label"; "\"null\""; Some("EmptyToNullLabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "fission"; "List<CompilationMode>"; "\"no\""; Some("FissionOptionConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "force_pic"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "grte_top"; "Label"; "\"null\""; Some("LibcTopLabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_compiler"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_conlyopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_copt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_cxxopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_grte_top"; "Label"; "\"null\""; Some("LibcTopLabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_linkopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_per_file_copt"; "List<PerLabelOptions>"; "\"null\""; Some("PerLabelOptions.PerLabelOptionsConverter.class"); true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_disable_legacy_cc_provider"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_disable_nocopts"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_dont_enable_host_nonhost_crosstool_features"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_enable_cc_toolchain_resolution"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_make_thinlto_command_lines_standalone"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_remove_legacy_whole_archive"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_require_ctx_in_configure_features"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_use_cpp_compile_header_mnemonic"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_use_specific_tool_files"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_validate_top_level_header_inclusions"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "interface_shared_objects"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "legacy_whole_archive"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "linkopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "ltobackendopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "ltoindexopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "memprof_profile"; "Label"; "\"null\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "minimum_os_version"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "objc_enable_binary_stripping"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "objc_generate_linkmap"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "objc_use_dotd_pruning"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "objccopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "per_file_copt"; "List<PerLabelOptions>"; "\"null\""; Some("PerLabelOptions.PerLabelOptionsConverter.class"); true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "per_file_ltobackendopt"; "List<PerLabelOptions>"; "\"null\""; Some("PerLabelOptions.PerLabelOptionsConverter.class"); true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "process_headers_in_dependencies"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "propeller_optimize"; "Label"; "\"null\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "propeller_optimize_absolute_cc_profile"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "propeller_optimize_absolute_ld_profile"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "proto_profile"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "proto_profile_path"; "Label"; "\"null\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "save_temps"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "share_native_deps"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "start_end_lib"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "strict_system_includes"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "strip"; "StripMode"; "\"sometimes\""; Some("StripModeConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "stripopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.cpp.CppOptions"; "xbinary_fdo"; "Label"; "\"null\""; Some("EmptyToNullLabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "bytecode_optimization_pass_actions"; "int"; "\"1\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "bytecode_optimizers"; "Map<String, Label>"; "\"Proguard\""; Some("LabelMapConverter.class"); false; Some("\"experimental_bytecode_optimizers\""); None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "enforce_proguard_file_extension"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_add_test_support_to_compile_time_deps"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_enable_jspecify"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_fix_deps_tool"; "String"; "\"add_dep\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_inmemory_jdeps_files"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_java_classpath"; "JavaClasspathMode"; "\"bazel\""; Some("JavaClasspathModeConverter.class"); false; Some("\"java_classpath\""); None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_java_test_auto_create_deploy_jar"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_local_java_optimization_configuration"; "Label"; "\"null\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_local_java_optimizations"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_one_version_enforcement"; "OneVersionEnforcementLevel"; "\"OFF\""; Some("OneVersionEnforcementLevelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_run_android_lint_on_java_rules"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_strict_java_deps"; "StrictDepsMode"; "\"default\""; Some("StrictDepsConverter.class"); false; Some("\"strict_java_deps\""); None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_turbine_annotation_processing"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "explicit_java_test_deps"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "host_java_launcher"; "Label"; "\"null\""; Some("EmptyToNullLabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "host_javacopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "host_jvmopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "incompatible_disallow_java_import_exports"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "incompatible_multi_release_deploy_jars"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "java_debug"; "Void"; "\"null\""; None; false; None; Some("{ \"--test_arg=--wrapper_script_flag=--debug\", \"--test_output=streamed\", \"--test_strategy=exclusive\", \"--test_timeout=9999\", \"--nocache_test_results\" }"); None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "java_deps"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "java_header_compilation"; "boolean"; "\"true\""; None; false; Some("\"experimental_java_header_compilation\""); None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "java_language_version"; "String"; "\"\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "java_launcher"; "Label"; "\"null\""; Some("EmptyToNullLabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "java_runtime_version"; "String"; "\"local_jdk\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "javacopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "jvmopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "one_version_enforcement_on_java_tests"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "plugin"; "List<Label>"; "\"null\""; Some("LabelListConverter.class"); true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "proguard_top"; "Label"; "\"null\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "split_bytecode_optimization_pass"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "tool_java_language_version"; "String"; "\"\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "tool_java_runtime_version"; "String"; "\"remotejdk_11\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.java.JavaOptions"; "use_ijars"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.J2ObjcCommandLineOptions"; "j2objc_dead_code_report"; "Label"; "\"null\""; Some("LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.J2ObjcCommandLineOptions"; "j2objc_translation_flags"; "List<String>"; "\"null\""; Some("Converters.CommaSeparatedOptionListConverter.class"); true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "device_debug_entitlements"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "experimental_objc_fastbuild_options"; "List<String>"; "\"-O0,-DDEBUG=1\""; Some("CommaSeparatedOptionListConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_avoid_hardcoded_objc_compilation_flags"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_builtin_objc_strip_action"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_disable_native_apple_binary_rule"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_disallow_sdk_frameworks_attributes"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_objc_alwayslink_by_default"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_strip_executable_safely"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "ios_memleaks"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "ios_signing_cert_name"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "ios_simulator_device"; "String"; "\"null\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "ios_simulator_version"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "objc_debug_with_GLIBCXX"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "cc_proto_library_header_suffixes"; "List<String>"; "\".pb.h\""; Some("Converters.CommaSeparatedOptionSetConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "cc_proto_library_source_suffixes"; "List<String>"; "\".pb.cc\""; Some("Converters.CommaSeparatedOptionSetConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "experimental_proto_descriptor_sets_include_source_info"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "proto_compiler"; "Label"; "ProtoConstants.DEFAULT_PROTOC_LABEL"; Some("CoreOptionConverters.LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "proto_toolchain_for_cc"; "Label"; "ProtoConstants.DEFAULT_CC_PROTO_LABEL"; Some("CoreOptionConverters.EmptyToNullLabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "proto_toolchain_for_j2objc"; "Label"; "ProtoConstants.DEFAULT_J2OBJC_PROTO_LABEL"; Some("CoreOptionConverters.EmptyToNullLabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "proto_toolchain_for_java"; "Label"; "ProtoConstants.DEFAULT_JAVA_PROTO_LABEL"; Some("CoreOptionConverters.EmptyToNullLabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "proto_toolchain_for_javalite"; "Label"; "ProtoConstants.DEFAULT_JAVA_LITE_PROTO_LABEL"; Some("CoreOptionConverters.LabelConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "protocopt"; "List<String>"; "\"null\""; None; true; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "strict_proto_deps"; "StrictDepsMode"; "\"error\""; Some("CoreOptionConverters.StrictDepsConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "strict_public_imports"; "StrictDepsMode"; "\"off\""; Some("CoreOptionConverters.StrictDepsConverter.class"); false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.python.PythonOptions"; "build_python_zip"; "TriState"; "\"auto\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.python.PythonOptions"; "experimental_py_binaries_include_label"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.python.PythonOptions"; "incompatible_default_to_explicit_init_py"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.python.PythonOptions"; "incompatible_python_disallow_native_rules"; "boolean"; "\"false\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.python.PythonOptions"; "incompatible_remove_ctx_py_fragment"; "boolean"; "\"true\""; None; false; None; None; None; "I"),
    expected_descriptor!("com.google.devtools.build.lib.rules.python.PythonOptions"; "python_native_rules_allowlist"; "Label"; "\"null\""; Some("LabelConverter.class"); false; None; None; None; "I"),
];

const EXPECTED_CLASS_COUNTS: &[(&str, usize)] = &[
    ("com.google.devtools.build.lib.analysis.PlatformOptions", 7),
    (
        "com.google.devtools.build.lib.analysis.ShellConfiguration.Options",
        1,
    ),
    (
        "com.google.devtools.build.lib.analysis.config.CoreOptions",
        71,
    ),
    (
        "com.google.devtools.build.lib.analysis.test.CoverageConfiguration.CoverageOptions",
        2,
    ),
    (
        "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions",
        21,
    ),
    (
        "com.google.devtools.build.lib.bazel.rules.BazelRuleClassProvider.StrictActionEnvOptions",
        1,
    ),
    (
        "com.google.devtools.build.lib.bazel.rules.python.BazelPythonConfiguration.Options",
        3,
    ),
    (
        "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options",
        60,
    ),
    (
        "com.google.devtools.build.lib.rules.android.BazelAndroidConfiguration.Options",
        1,
    ),
    (
        "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions",
        26,
    ),
    (
        "com.google.devtools.build.lib.rules.config.ConfigFeatureFlagOptions",
        2,
    ),
    ("com.google.devtools.build.lib.rules.cpp.CppOptions", 78),
    ("com.google.devtools.build.lib.rules.java.JavaOptions", 36),
    (
        "com.google.devtools.build.lib.rules.objc.J2ObjcCommandLineOptions",
        2,
    ),
    (
        "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions",
        13,
    ),
    (
        "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options",
        11,
    ),
    (
        "com.google.devtools.build.lib.rules.python.PythonOptions",
        6,
    ),
];

#[test]
fn pinned_registry_has_every_metadata_field_in_order() {
    assert_eq!(NATIVE_OPTION_DESCRIPTORS.len(), 341);
    assert_eq!(EXPECTED.len(), 341);
    for (actual, expected) in NATIVE_OPTION_DESCRIPTORS.iter().zip(EXPECTED) {
        assert_eq!(actual.class_name, expected.class_name);
        assert_eq!(actual.canonical_name, expected.canonical_name);
        assert_eq!(actual.field_type, expected.field_type);
        assert_eq!(actual.raw_default, expected.raw_default);
        assert_eq!(actual.converter, expected.converter);
        assert_eq!(actual.allow_multiple, expected.allow_multiple);
        assert_eq!(actual.old_name, expected.old_name);
        assert_eq!(actual.expansion, expected.expansion);
        assert_eq!(actual.implicit_requirements, expected.implicit_requirements);
        assert_eq!(actual.normalizer, expected.normalizer);
    }
}

#[test]
fn pinned_classes_and_options_are_ordered_and_unique() {
    assert_eq!(EXPECTED_CLASS_COUNTS.len(), 17);
    let mut start = 0;
    for &(class_name, count) in EXPECTED_CLASS_COUNTS {
        let end = start + count;
        let class_rows = &NATIVE_OPTION_DESCRIPTORS[start..end];
        assert!(
            class_rows
                .iter()
                .all(|descriptor| descriptor.class_name == class_name)
        );
        assert!(
            class_rows
                .windows(2)
                .all(|pair| pair[0].canonical_name < pair[1].canonical_name)
        );
        start = end;
    }
    assert_eq!(start, NATIVE_OPTION_DESCRIPTORS.len());
    for (index, descriptor) in NATIVE_OPTION_DESCRIPTORS.iter().enumerate() {
        assert!(NATIVE_OPTION_DESCRIPTORS[..index].iter().all(|prior| {
            (prior.class_name, prior.canonical_name)
                != (descriptor.class_name, descriptor.canonical_name)
        }));
    }
}

fn descriptor(class_name: &str, canonical_name: &str) -> &'static NativeOptionDescriptor {
    NATIVE_OPTION_DESCRIPTORS
        .iter()
        .find(|descriptor| {
            descriptor.class_name == class_name && descriptor.canonical_name == canonical_name
        })
        .expect("pinned descriptor")
}

#[test]
fn formerly_missed_and_complex_metadata_rows_are_pinned() {
    let test_filter = descriptor(
        "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions",
        "test_filter",
    );
    assert_eq!(test_filter.field_type, "String");
    assert_eq!(test_filter.raw_default, "\"null\"");
    assert_eq!(test_filter.normalizer, "T");

    let xcode_version = descriptor(
        "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions",
        "xcode_version",
    );
    assert_eq!(xcode_version.field_type, "String");
    assert_eq!(xcode_version.raw_default, "\"null\"");
    assert_eq!(xcode_version.normalizer, "I");

    let start_end_lib = descriptor(
        "com.google.devtools.build.lib.rules.cpp.CppOptions",
        "start_end_lib",
    );
    assert_eq!(start_end_lib.field_type, "boolean");
    assert_eq!(start_end_lib.raw_default, "\"true\"");
    assert_eq!(start_end_lib.normalizer, "I");

    let host_platform = descriptor(
        "com.google.devtools.build.lib.analysis.PlatformOptions",
        "host_platform",
    );
    assert_eq!(
        host_platform.old_name,
        Some("\"experimental_host_platform\"")
    );
    assert_eq!(host_platform.raw_default, "DEFAULT_HOST_PLATFORM");

    let extra_toolchains = descriptor(
        "com.google.devtools.build.lib.analysis.PlatformOptions",
        "extra_toolchains",
    );
    assert!(extra_toolchains.allow_multiple);
    assert_eq!(extra_toolchains.normalizer, "P");

    let action_env = descriptor(
        "com.google.devtools.build.lib.analysis.config.CoreOptions",
        "action_env",
    );
    assert!(action_env.allow_multiple);
    assert_eq!(action_env.normalizer, "C");

    let fdo_instrument = descriptor(
        "com.google.devtools.build.lib.rules.cpp.CppOptions",
        "fdo_instrument",
    );
    assert_eq!(
        fdo_instrument.implicit_requirements,
        Some("{\"--copt=-Wno-error\"}")
    );
    let java_debug = descriptor(
        "com.google.devtools.build.lib.rules.java.JavaOptions",
        "java_debug",
    );
    assert_eq!(
        java_debug.expansion,
        Some(
            "{ \"--test_arg=--wrapper_script_flag=--debug\", \"--test_output=streamed\", \
             \"--test_strategy=exclusive\", \"--test_timeout=9999\", \"--nocache_test_results\" }"
        )
    );

    let proto_compiler = descriptor(
        "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options",
        "proto_compiler",
    );
    assert_eq!(
        proto_compiler.raw_default,
        "ProtoConstants.DEFAULT_PROTOC_LABEL"
    );
}

#[test]
fn cache_field_grammar_preserves_null_empty_and_scalar_bytes() {
    assert_eq!(
        format_cache_field("option", CacheFieldValue::Null),
        "option=NULL, "
    );
    assert_eq!(
        format_cache_field("option", CacheFieldValue::Empty),
        "option=EMPTY, "
    );
    assert_eq!(
        format_cache_field("option", CacheFieldValue::Scalar("plain")),
        "option=\"plain\", "
    );
    assert_eq!(
        format_cache_field("option", CacheFieldValue::Scalar("slash\\quote\"")),
        "option=\"slash\\\\quote\\\"\", "
    );
}

// Retry-7 Phase 1 binds every admitted descriptor to its registry tuple and
// default outcome before any private kernel implementation is authorized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpectedDefaultOutcome {
    SpecialNull,
    RepeatEmpty,
    Converted,
}

#[derive(Clone, Copy, Debug)]
struct Retry7DefaultBinding {
    attachment: &'static str,
    registry_index: usize,
    class_name: &'static str,
    canonical_name: &'static str,
    field_type: &'static str,
    raw_default: &'static str,
    converter: Option<&'static str>,
    allow_multiple: bool,
    family: &'static str,
    route: &'static str,
    outcome: ExpectedDefaultOutcome,
    expected_cache: &'static str,
}

macro_rules! binding {
    ($attachment:expr; $index:expr; $class:expr; $name:expr; $field:expr; $raw:expr; $converter:expr; $multiple:expr; $family:expr; $route:expr; $outcome:ident; $cache:expr) => {
        Retry7DefaultBinding {
            attachment: $attachment,
            registry_index: $index,
            class_name: $class,
            canonical_name: $name,
            field_type: $field,
            raw_default: $raw,
            converter: $converter,
            allow_multiple: $multiple,
            family: $family,
            route: $route,
            outcome: ExpectedDefaultOutcome::$outcome,
            expected_cache: $cache,
        }
    };
}

#[rustfmt::skip]
const RETRY7_DEFAULT_BINDINGS: &[Retry7DefaultBinding] = &[
    binding!("A01.01"; 0; "com.google.devtools.build.lib.analysis.PlatformOptions"; "extra_execution_platforms"; "List<String>"; "\"\""; Some("CommaSeparatedOptionListConverter.class"); false; "F-AllowCommaList"; "S:D/E"; Converted; "extra_execution_platforms=EMPTY, "),
    binding!("A01.02"; 1; "com.google.devtools.build.lib.analysis.PlatformOptions"; "extra_toolchains"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; "F-AllowCommaList"; "R:N/A"; RepeatEmpty; "extra_toolchains=EMPTY, "),
    binding!("A01.04"; 3; "com.google.devtools.build.lib.analysis.PlatformOptions"; "incompatible_use_toolchain_resolution_for_java_rules"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_use_toolchain_resolution_for_java_rules=\"true\", "),
    binding!("A03.01"; 8; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "action_env"; "List<Converters.EnvVar>"; "\"null\""; Some("Converters.EnvVarsConverter.class"); true; "F-Env"; "R:N/A"; RepeatEmpty; "action_env=EMPTY, "),
    binding!("A03.02"; 9; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "affected by starlark transition"; "List<String>"; "\"\""; Some("EmptyListConverter.class"); false; "F-EmptyList"; "S:D/E"; Converted; "affected by starlark transition=EMPTY, "),
    binding!("A03.03"; 10; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "allow_analysis_failures"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "allow_analysis_failures=\"false\", "),
    binding!("A03.04"; 11; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "allow_unresolved_symlinks"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "allow_unresolved_symlinks=\"true\", "),
    binding!("A03.05"; 12; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "allowed_cpu_values"; "ImmutableList<String>"; "\"\""; Some("CommaSeparatedOptionSetConverter.class"); false; "F-StringSet"; "S:D/E"; Converted; "allowed_cpu_values=EMPTY, "),
    binding!("A03.06"; 13; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "analysis_testing_deps_limit"; "int"; "\"2000\""; None; false; "F-Int"; "S:D/E"; Converted; "analysis_testing_deps_limit=\"2000\", "),
    binding!("A03.08"; 15; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "build_runfile_links"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "build_runfile_links=\"true\", "),
    binding!("A03.09"; 16; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "build_runfile_manifests"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "build_runfile_manifests=\"true\", "),
    binding!("A03.10"; 17; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "check_licenses"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "check_licenses=\"false\", "),
    binding!("A03.11"; 18; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "check_visibility"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "check_visibility=\"true\", "),
    binding!("A03.12"; 19; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "collect_code_coverage"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "collect_code_coverage=\"false\", "),
    binding!("A03.13"; 20; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "compilation_mode"; "CompilationMode"; "\"fastbuild\""; Some("CompilationMode.Converter.class"); false; "F-Enum-CompilationMode"; "S:D/E"; Converted; "compilation_mode=\"fastbuild\", "),
    binding!("A03.15"; 22; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "define"; "List<Map.Entry<String, String>>"; "\"null\""; Some("Converters.AssignmentConverter.class"); true; "F-Entry"; "R:N/A"; RepeatEmpty; "define=EMPTY, "),
    binding!("A03.16"; 23; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "enable_runfiles"; "TriState"; "\"auto\""; None; false; "F-Tri"; "S:D/E"; Converted; "enable_runfiles=\"AUTO\", "),
    binding!("A03.17"; 24; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "enforce_constraints"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "enforce_constraints=\"true\", "),
    binding!("A03.18"; 25; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "evaluating for analysis test"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "evaluating for analysis test=\"false\", "),
    binding!("A03.19"; 26; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "exec_aspects"; "List<String>"; "\"null\""; Some("Converters.CommaSeparatedOptionListConverter.class"); true; "F-AllowCommaList"; "R:N/A"; RepeatEmpty; "exec_aspects=EMPTY, "),
    binding!("A03.21"; 28; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_allow_map_directory"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_allow_map_directory=\"true\", "),
    binding!("A03.22"; 29; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_collect_code_coverage_for_generated_files"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_collect_code_coverage_for_generated_files=\"false\", "),
    binding!("A03.23"; 30; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_debug_selects_always_succeed"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_debug_selects_always_succeed=\"false\", "),
    binding!("A03.24"; 31; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_enforce_transitive_visibility"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_enforce_transitive_visibility=\"false\", "),
    binding!("A03.25"; 32; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_exclude_defines_from_exec_config"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_exclude_defines_from_exec_config=\"false\", "),
    binding!("A03.26"; 33; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_exec_config"; "String"; "\"@_builtins//:common/builtin_exec_platforms.bzl%bazel_exec_transition\""; None; false; "F-Text"; "S:D/E"; Converted; "experimental_exec_config=\"@_builtins//:common/builtin_exec_platforms.bzl%bazel_exec_transition\", "),
    binding!("A03.27"; 34; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_exec_configuration_distinguisher"; "ExecConfigurationDistinguisherScheme"; "\"off\""; Some("ExecConfigurationDistinguisherSchemeConverter.class"); false; "F-Enum-ExecConfigurationDistinguisher"; "S:D/E"; Converted; "experimental_exec_configuration_distinguisher=\"OFF\", "),
    binding!("A03.28"; 35; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_extended_sanity_checks"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_extended_sanity_checks=\"false\", "),
    binding!("A03.29"; 36; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_output_directory_naming_scheme"; "OutputDirectoryNamingScheme"; "\"diff_against_dynamic_baseline\""; Some("OutputDirectoryNamingSchemeConverter.class"); false; "F-Enum-OutputDirectoryNaming"; "S:D/E"; Converted; "experimental_output_directory_naming_scheme=\"DIFF_AGAINST_DYNAMIC_BASELINE\", "),
    binding!("A03.30"; 37; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_output_paths"; "OutputPathsMode"; "\"off\""; Some("OutputPathsConverter.class"); false; "F-Enum-OutputPaths"; "S:D/E"; Converted; "experimental_output_paths=\"OFF\", "),
    binding!("A03.32"; 39; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_platform_in_output_dir"; "TriState"; "\"Auto\""; None; false; "F-Tri"; "S:D/E"; Converted; "experimental_platform_in_output_dir=\"AUTO\", "),
    binding!("A03.34"; 41; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_remotable_source_manifests"; "boolean"; "\"false\""; Some("BooleanConverter.class"); false; "F-Bool"; "S:D/E"; Converted; "experimental_remotable_source_manifests=\"false\", "),
    binding!("A03.35"; 42; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_strict_fileset_output"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_strict_fileset_output=\"false\", "),
    binding!("A03.36"; 43; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_throttle_action_cache_check"; "boolean"; "\"true\""; Some("BooleanConverter.class"); false; "F-Bool"; "S:D/E"; Converted; "experimental_throttle_action_cache_check=\"true\", "),
    binding!("A03.37"; 44; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_use_platforms_in_output_dir_legacy_heuristic"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_use_platforms_in_output_dir_legacy_heuristic=\"true\", "),
    binding!("A03.38"; 45; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "experimental_writable_outputs"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_writable_outputs=\"false\", "),
    binding!("A03.39"; 46; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "features"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "features=EMPTY, "),
    binding!("A03.41"; 48; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "host_action_env"; "List<Converters.EnvVar>"; "\"null\""; Some("Converters.EnvVarsConverter.class"); true; "F-Env"; "R:N/A"; RepeatEmpty; "host_action_env=EMPTY, "),
    binding!("A03.42"; 49; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "host_compilation_mode"; "CompilationMode"; "\"opt\""; Some("CompilationMode.Converter.class"); false; "F-Enum-CompilationMode"; "S:D/E"; Converted; "host_compilation_mode=\"opt\", "),
    binding!("A03.44"; 51; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "host_features"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "host_features=EMPTY, "),
    binding!("A03.45"; 52; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "include_config_fragments_provider"; "IncludeConfigFragmentsEnum"; "\"off\""; Some("IncludeConfigFragmentsEnumConverter.class"); false; "F-Enum-IncludeConfigFragments"; "S:D/E"; Converted; "include_config_fragments_provider=\"OFF\", "),
    binding!("A03.46"; 53; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_always_include_files_in_data"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_always_include_files_in_data=\"true\", "),
    binding!("A03.47"; 54; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_auto_exec_groups"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_auto_exec_groups=\"false\", "),
    binding!("A03.48"; 55; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_bazel_test_exec_run_under"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_bazel_test_exec_run_under=\"true\", "),
    binding!("A03.49"; 56; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_bep_cpu_from_platform"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_bep_cpu_from_platform=\"false\", "),
    binding!("A03.50"; 57; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_check_testonly_for_output_files"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_check_testonly_for_output_files=\"false\", "),
    binding!("A03.51"; 58; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_compact_repo_mapping_manifest"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_compact_repo_mapping_manifest=\"true\", "),
    binding!("A03.52"; 59; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_disable_select_on"; "ImmutableList<String>"; "\"\""; Some("CommaSeparatedOptionSetConverter.class"); false; "F-StringSet"; "S:D/E"; Converted; "incompatible_disable_select_on=EMPTY, "),
    binding!("A03.53"; 60; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_exclude_starlark_flags_from_exec_config"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_exclude_starlark_flags_from_exec_config=\"false\", "),
    binding!("A03.54"; 61; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_filegroup_runfiles_for_data"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_filegroup_runfiles_for_data=\"true\", "),
    binding!("A03.56"; 63; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_merge_genfiles_directory"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_merge_genfiles_directory=\"true\", "),
    binding!("A03.57"; 64; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_modify_execution_info_additive"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_modify_execution_info_additive=\"true\", "),
    binding!("A03.58"; 65; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "incompatible_target_cpu_from_platform"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_target_cpu_from_platform=\"true\", "),
    binding!("A03.59"; 66; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "instrument_test_targets"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "instrument_test_targets=\"false\", "),
    binding!("A03.61"; 68; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "is exec configuration"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "is exec configuration=\"false\", "),
    binding!("A03.62"; 69; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "min_param_file_size"; "int"; "\"32768\""; None; false; "F-Int"; "S:D/E"; Converted; "min_param_file_size=\"32768\", "),
    binding!("A03.64"; 71; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "platform_suffix"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "platform_suffix=NULL, "),
    binding!("A03.66"; 73; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "scl_config"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "scl_config=NULL, "),
    binding!("A03.67"; 74; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "stamp"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "stamp=\"false\", "),
    binding!("A03.68"; 75; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "strict_filesets"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "strict_filesets=\"false\", "),
    binding!("A03.70"; 77; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "use_target_platform_for_tests"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "use_target_platform_for_tests=\"false\", "),
    binding!("A03.71"; 78; "com.google.devtools.build.lib.analysis.config.CoreOptions"; "verbose_visibility_errors"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "verbose_visibility_errors=\"false\", "),
    binding!("A05.01"; 81; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "allow_local_tests"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "allow_local_tests=\"true\", "),
    binding!("A05.02"; 82; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "cache_test_results"; "TriState"; "\"auto\""; None; false; "F-Tri"; "S:D/E"; Converted; "cache_test_results=\"AUTO\", "),
    binding!("A05.05"; 85; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "experimental_cancel_concurrent_tests"; "CancelConcurrentTests"; "\"never\""; Some("CancelConcurrentTests.Converter.class"); false; "F-Enum-Cancel"; "S:D/E"; Converted; "experimental_cancel_concurrent_tests=\"NEVER\", "),
    binding!("A05.06"; 86; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "experimental_fetch_all_coverage_outputs"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_fetch_all_coverage_outputs=\"false\", "),
    binding!("A05.07"; 87; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "experimental_retain_test_configuration_across_testonly"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_retain_test_configuration_across_testonly=\"true\", "),
    binding!("A05.08"; 88; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "experimental_split_coverage_postprocessing"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_split_coverage_postprocessing=\"false\", "),
    binding!("A05.09"; 89; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "incompatible_check_sharding_support"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_check_sharding_support=\"true\", "),
    binding!("A05.10"; 90; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "incompatible_exclusive_test_sandboxed"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_exclusive_test_sandboxed=\"true\", "),
    binding!("A05.12"; 92; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "runs_per_test_detects_flakes"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "runs_per_test_detects_flakes=\"false\", "),
    binding!("A05.13"; 93; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_arg"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "test_arg=EMPTY, "),
    binding!("A05.14"; 94; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_env"; "List<Converters.EnvVar>"; "\"null\""; Some("Converters.EnvVarsConverter.class"); true; "F-Env"; "R:N/A"; RepeatEmpty; "test_env=EMPTY, "),
    binding!("A05.15"; 95; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_filter"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "test_filter=NULL, "),
    binding!("A05.16"; 96; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_result_expiration"; "int"; "\"-1\""; None; false; "F-Int"; "S:D/E"; Converted; "test_result_expiration=\"-1\", "),
    binding!("A05.17"; 97; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_runner_fail_fast"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "test_runner_fail_fast=\"false\", "),
    binding!("A05.18"; 98; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_sharding_strategy"; "TestShardingStrategy"; "\"explicit\""; Some("ShardingStrategyConverter.class"); false; "F-Shard"; "S:D/E"; Converted; "test_sharding_strategy=\"EXPLICIT\", "),
    binding!("A05.19"; 99; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "test_timeout"; "Map<TestTimeout, Duration>"; "\"-1\""; Some("TestTimeout.TestTimeoutConverter.class"); false; "F-Timeout"; "S:D/E"; Converted; "test_timeout=\"{short=PT1M, moderate=PT5M, long=PT15M, eternal=PT1H}\", "),
    binding!("A05.20"; 100; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "trim_test_configuration"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "trim_test_configuration=\"true\", "),
    binding!("A05.21"; 101; "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions"; "zip_undeclared_test_outputs"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "zip_undeclared_test_outputs=\"false\", "),
    binding!("A06.01"; 102; "com.google.devtools.build.lib.bazel.rules.BazelRuleClassProvider.StrictActionEnvOptions"; "incompatible_strict_action_env"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_strict_action_env=\"true\", "),
    binding!("A07.01"; 103; "com.google.devtools.build.lib.bazel.rules.python.BazelPythonConfiguration.Options"; "experimental_python_import_all_repositories"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_python_import_all_repositories=\"true\", "),
    binding!("A07.02"; 104; "com.google.devtools.build.lib.bazel.rules.python.BazelPythonConfiguration.Options"; "incompatible_remove_ctx_bazel_py_fragment"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_remove_ctx_bazel_py_fragment=\"true\", "),
    binding!("A07.03"; 105; "com.google.devtools.build.lib.bazel.rules.python.BazelPythonConfiguration.Options"; "python_path"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "python_path=NULL, "),
    binding!("A08.01"; 106; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "Android configuration distinguisher"; "ConfigurationDistinguisher"; "\"MAIN\""; Some("ConfigurationDistinguisherConverter.class"); false; "F-Enum-AndroidConfigurationDistinguisher"; "S:D/E"; Converted; "Android configuration distinguisher=\"MAIN\", "),
    binding!("A08.02"; 107; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_compiler"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "android_compiler=NULL, "),
    binding!("A08.03"; 108; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_databinding_use_androidx"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "android_databinding_use_androidx=\"true\", "),
    binding!("A08.04"; 109; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_databinding_use_v3_4_args"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "android_databinding_use_v3_4_args=\"true\", "),
    binding!("A08.05"; 110; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_dynamic_mode"; "DynamicMode"; "\"off\""; Some("DynamicModeConverter.class"); false; "F-Enum-DynamicMode"; "S:D/E"; Converted; "android_dynamic_mode=\"OFF\", "),
    binding!("A08.06"; 111; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_fixed_resource_neverlinking"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "android_fixed_resource_neverlinking=\"true\", "),
    binding!("A08.07"; 112; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_manifest_merger"; "AndroidManifestMerger"; "\"android\""; Some("AndroidManifestMergerConverter.class"); false; "F-Enum-AndroidManifestMerger"; "S:D/E"; Converted; "android_manifest_merger=\"ANDROID\", "),
    binding!("A08.08"; 113; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_manifest_merger_order"; "ManifestMergerOrder"; "\"alphabetical\""; Some("ManifestMergerOrderConverter.class"); false; "F-Enum-ManifestMergerOrder"; "S:D/E"; Converted; "android_manifest_merger_order=\"ALPHABETICAL\", "),
    binding!("A08.09"; 114; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_migration_tag_check"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "android_migration_tag_check=\"false\", "),
    binding!("A08.11"; 116; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "android_resource_shrinking"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "android_resource_shrinking=\"false\", "),
    binding!("A08.12"; 117; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "apk_signing_method"; "ApkSigningMethod"; "\"v1_v2\""; Some("ApkSigningMethodConverter.class"); false; "F-Enum-ApkSigningMethod"; "S:D/E"; Converted; "apk_signing_method=\"V1_V2\", "),
    binding!("A08.13"; 118; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "break_build_on_parallel_dex2oat_failure"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "break_build_on_parallel_dex2oat_failure=\"false\", "),
    binding!("A08.14"; 119; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "desugar_for_android"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "desugar_for_android=\"true\", "),
    binding!("A08.15"; 120; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "desugar_java8_libs"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "desugar_java8_libs=\"false\", "),
    binding!("A08.16"; 121; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "dexopts_supported_in_dexmerger"; "List<String>"; "\"--minimal-main-dex,--set-max-idx-number\""; Some("Converters.CommaSeparatedOptionListConverter.class"); false; "F-AllowCommaList"; "S:D/E"; Converted; "dexopts_supported_in_dexmerger=\"[--minimal-main-dex, --set-max-idx-number]\", "),
    binding!("A08.17"; 122; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "dexopts_supported_in_dexsharder"; "List<String>"; "\"--minimal-main-dex\""; Some("Converters.CommaSeparatedOptionListConverter.class"); false; "F-AllowCommaList"; "S:D/E"; Converted; "dexopts_supported_in_dexsharder=\"[--minimal-main-dex]\", "),
    binding!("A08.18"; 123; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "dexopts_supported_in_incremental_dexing"; "List<String>"; "\"--no-optimize,--no-locals\""; Some("Converters.CommaSeparatedOptionListConverter.class"); false; "F-AllowCommaList"; "S:D/E"; Converted; "dexopts_supported_in_incremental_dexing=\"[--no-optimize, --no-locals]\", "),
    binding!("A08.19"; 124; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_allow_android_library_deps_without_srcs"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_allow_android_library_deps_without_srcs=\"false\", "),
    binding!("A08.20"; 125; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_always_filter_duplicate_classes_from_android_test"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_always_filter_duplicate_classes_from_android_test=\"false\", "),
    binding!("A08.21"; 126; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_assume_minsdkversion"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_android_assume_minsdkversion=\"false\", "),
    binding!("A08.22"; 127; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_compress_java_resources"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_android_compress_java_resources=\"false\", "),
    binding!("A08.23"; 128; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_databinding_v2"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_android_databinding_v2=\"true\", "),
    binding!("A08.24"; 129; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_library_exports_manifest_default"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_android_library_exports_manifest_default=\"false\", "),
    binding!("A08.25"; 130; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_resource_cycle_shrinking"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_android_resource_cycle_shrinking=\"false\", "),
    binding!("A08.26"; 131; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_resource_name_obfuscation"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_android_resource_name_obfuscation=\"false\", "),
    binding!("A08.27"; 132; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_resource_path_shortening"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_android_resource_path_shortening=\"false\", "),
    binding!("A08.28"; 133; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_resource_shrinking"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_android_resource_shrinking=\"false\", "),
    binding!("A08.29"; 134; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_rewrite_dexes_with_rex"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_android_rewrite_dexes_with_rex=\"false\", "),
    binding!("A08.30"; 135; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_android_use_parallel_dex2oat"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_android_use_parallel_dex2oat=\"false\", "),
    binding!("A08.31"; 136; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_check_desugar_deps"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_check_desugar_deps=\"true\", "),
    binding!("A08.32"; 137; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_disable_instrumentation_manifest_merge"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_disable_instrumentation_manifest_merge=\"false\", "),
    binding!("A08.33"; 138; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_filter_library_jar_with_program_jar"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_filter_library_jar_with_program_jar=\"false\", "),
    binding!("A08.34"; 139; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_filter_r_jars_from_android_test"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_filter_r_jars_from_android_test=\"false\", "),
    binding!("A08.35"; 140; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_get_android_java_resources_from_optimized_jar"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_get_android_java_resources_from_optimized_jar=\"false\", "),
    binding!("A08.36"; 141; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_incremental_dexing_after_proguard"; "int"; "\"50\""; None; false; "F-Int"; "S:D/E"; Converted; "experimental_incremental_dexing_after_proguard=\"50\", "),
    binding!("A08.37"; 142; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_incremental_dexing_after_proguard_by_default"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_incremental_dexing_after_proguard_by_default=\"true\", "),
    binding!("A08.38"; 143; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_omit_resources_info_provider_from_android_binary"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_omit_resources_info_provider_from_android_binary=\"false\", "),
    binding!("A08.39"; 144; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_one_version_enforcement_use_transitive_jars_for_binary_under_test"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_one_version_enforcement_use_transitive_jars_for_binary_under_test=\"false\", "),
    binding!("A08.40"; 145; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_persistent_aar_extractor"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_persistent_aar_extractor=\"false\", "),
    binding!("A08.41"; 146; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_remove_r_classes_from_instrumentation_test_jar"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_remove_r_classes_from_instrumentation_test_jar=\"true\", "),
    binding!("A08.42"; 147; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_use_dex_splitter_for_incremental_dexing"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_use_dex_splitter_for_incremental_dexing=\"true\", "),
    binding!("A08.43"; 148; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "experimental_use_rtxt_from_merged_resources"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_use_rtxt_from_merged_resources=\"false\", "),
    binding!("A08.44"; 149; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "fat_apk_hwasan"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "fat_apk_hwasan=\"false\", "),
    binding!("A08.45"; 150; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "incompatible_disable_native_android_rules"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_disable_native_android_rules=\"false\", "),
    binding!("A08.46"; 151; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "incompatible_remove_ctx_android_fragment"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_remove_ctx_android_fragment=\"false\", "),
    binding!("A08.47"; 152; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "incremental_dexing"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incremental_dexing=\"true\", "),
    binding!("A08.48"; 153; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "internal_persistent_android_dex_desugar"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "internal_persistent_android_dex_desugar=\"false\", "),
    binding!("A08.49"; 154; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "internal_persistent_busybox_tools"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "internal_persistent_busybox_tools=\"false\", "),
    binding!("A08.50"; 155; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "internal_persistent_multiplex_android_dex_desugar"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "internal_persistent_multiplex_android_dex_desugar=\"false\", "),
    binding!("A08.51"; 156; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "internal_persistent_multiplex_busybox_tools"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "internal_persistent_multiplex_busybox_tools=\"false\", "),
    binding!("A08.53"; 158; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "non_incremental_per_target_dexopts"; "List<String>"; "\"--positions\""; Some("Converters.CommaSeparatedOptionListConverter.class"); false; "F-AllowCommaList"; "S:D/E"; Converted; "non_incremental_per_target_dexopts=\"[--positions]\", "),
    binding!("A08.55"; 160; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "output_library_merged_assets"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "output_library_merged_assets=\"true\", "),
    binding!("A08.56"; 161; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "persistent_android_dex_desugar"; "Void"; "\"null\""; None; false; "F-Void"; "S:N/E"; SpecialNull; "persistent_android_dex_desugar=NULL, "),
    binding!("A08.57"; 162; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "persistent_android_resource_processor"; "Void"; "\"null\""; None; false; "F-Void"; "S:N/E"; SpecialNull; "persistent_android_resource_processor=NULL, "),
    binding!("A08.58"; 163; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "persistent_multiplex_android_dex_desugar"; "Void"; "\"null\""; None; false; "F-Void"; "S:N/E"; SpecialNull; "persistent_multiplex_android_dex_desugar=NULL, "),
    binding!("A08.59"; 164; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "persistent_multiplex_android_resource_processor"; "Void"; "\"null\""; None; false; "F-Void"; "S:N/E"; SpecialNull; "persistent_multiplex_android_resource_processor=NULL, "),
    binding!("A08.60"; 165; "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options"; "persistent_multiplex_android_tools"; "Void"; "\"null\""; None; false; "F-Void"; "S:N/E"; SpecialNull; "persistent_multiplex_android_tools=NULL, "),
    binding!("A09.01"; 166; "com.google.devtools.build.lib.rules.android.BazelAndroidConfiguration.Options"; "merge_android_manifest_permissions"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "merge_android_manifest_permissions=\"false\", "),
    binding!("A10.01"; 167; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "apple configuration distinguisher"; "ConfigurationDistinguisher"; "\"UNKNOWN\""; Some("ConfigurationDistinguisherConverter.class"); false; "F-Enum-AppleConfigurationDistinguisher"; "S:D/E"; Converted; "apple configuration distinguisher=\"UNKNOWN\", "),
    binding!("A10.02"; 168; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "apple_platform_type"; "String"; "\"macos\""; Some("PlatformTypeConverter.class"); false; "F-Platform"; "S:D/E"; Converted; "apple_platform_type=\"macos\", "),
    binding!("A10.04"; 170; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "apple_split_cpu"; "String"; "\"\""; None; false; "F-Text"; "S:D/E"; Converted; "apple_split_cpu=\"\", "),
    binding!("A10.05"; 171; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "catalyst_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; "F-AllowCommaList"; "R:N/A"; RepeatEmpty; "catalyst_cpus=EMPTY, "),
    binding!("A10.06"; 172; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "experimental_include_xcode_execution_requirements"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_include_xcode_execution_requirements=\"false\", "),
    binding!("A10.07"; 173; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "experimental_objc_provider_from_linked"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_objc_provider_from_linked=\"false\", "),
    binding!("A10.08"; 174; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "experimental_prefer_mutual_xcode"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_prefer_mutual_xcode=\"true\", "),
    binding!("A10.09"; 175; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "host_macos_minimum_os"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; "F-Dotted"; "S:N/E"; SpecialNull; "host_macos_minimum_os=NULL, "),
    binding!("A10.10"; 176; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "incompatible_enable_apple_toolchain_resolution"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_enable_apple_toolchain_resolution=\"false\", "),
    binding!("A10.11"; 177; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "ios_minimum_os"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; "F-Dotted"; "S:N/E"; SpecialNull; "ios_minimum_os=NULL, "),
    binding!("A10.12"; 178; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "ios_multi_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; "F-AllowCommaList"; "R:N/A"; RepeatEmpty; "ios_multi_cpus=EMPTY, "),
    binding!("A10.13"; 179; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "ios_sdk_version"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; "F-Dotted"; "S:N/E"; SpecialNull; "ios_sdk_version=NULL, "),
    binding!("A10.14"; 180; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "macos_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; "F-AllowCommaList"; "R:N/A"; RepeatEmpty; "macos_cpus=EMPTY, "),
    binding!("A10.15"; 181; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "macos_minimum_os"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; "F-Dotted"; "S:N/E"; SpecialNull; "macos_minimum_os=NULL, "),
    binding!("A10.16"; 182; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "macos_sdk_version"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; "F-Dotted"; "S:N/E"; SpecialNull; "macos_sdk_version=NULL, "),
    binding!("A10.17"; 183; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "tvos_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; "F-AllowCommaList"; "R:N/A"; RepeatEmpty; "tvos_cpus=EMPTY, "),
    binding!("A10.18"; 184; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "tvos_minimum_os"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; "F-Dotted"; "S:N/E"; SpecialNull; "tvos_minimum_os=NULL, "),
    binding!("A10.19"; 185; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "tvos_sdk_version"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; "F-Dotted"; "S:N/E"; SpecialNull; "tvos_sdk_version=NULL, "),
    binding!("A10.20"; 186; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "use_platforms_in_apple_crosstool_transition"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "use_platforms_in_apple_crosstool_transition=\"false\", "),
    binding!("A10.21"; 187; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "visionos_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; "F-AllowCommaList"; "R:N/A"; RepeatEmpty; "visionos_cpus=EMPTY, "),
    binding!("A10.22"; 188; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "watchos_cpus"; "List<String>"; "\"null\""; Some("CommaSeparatedOptionListConverter.class"); true; "F-AllowCommaList"; "R:N/A"; RepeatEmpty; "watchos_cpus=EMPTY, "),
    binding!("A10.23"; 189; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "watchos_minimum_os"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; "F-Dotted"; "S:N/E"; SpecialNull; "watchos_minimum_os=NULL, "),
    binding!("A10.24"; 190; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "watchos_sdk_version"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; "F-Dotted"; "S:N/E"; SpecialNull; "watchos_sdk_version=NULL, "),
    binding!("A10.25"; 191; "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions"; "xcode_version"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "xcode_version=NULL, "),
    binding!("A11.01"; 193; "com.google.devtools.build.lib.rules.config.ConfigFeatureFlagOptions"; "all feature flag values are present (internal)"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "all feature flag values are present (internal)=\"true\", "),
    binding!("A11.02"; 194; "com.google.devtools.build.lib.rules.config.ConfigFeatureFlagOptions"; "enforce_transitive_configs_for_config_feature_flag"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "enforce_transitive_configs_for_config_feature_flag=\"false\", "),
    binding!("A12.01"; 195; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "apple_generate_dsym"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "apple_generate_dsym=\"false\", "),
    binding!("A12.02"; 196; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "build_test_dwp"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "build_test_dwp=\"false\", "),
    binding!("A12.03"; 197; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "cc_dotd_files"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "cc_dotd_files=\"true\", "),
    binding!("A12.04"; 198; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "cc_include_scanning"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "cc_include_scanning=\"false\", "),
    binding!("A12.05"; 199; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "cc_output_directory_tag"; "String"; "\"\""; None; false; "F-Text"; "S:D/E"; Converted; "cc_output_directory_tag=\"\", "),
    binding!("A12.06"; 200; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "compiler"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "compiler=NULL, "),
    binding!("A12.07"; 201; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "conlyopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "conlyopt=EMPTY, "),
    binding!("A12.08"; 202; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "copt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "copt=EMPTY, "),
    binding!("A12.10"; 204; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "cs_fdo_absolute_path"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "cs_fdo_absolute_path=NULL, "),
    binding!("A12.11"; 205; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "cs_fdo_instrument"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "cs_fdo_instrument=NULL, "),
    binding!("A12.14"; 208; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "cxxopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "cxxopt=EMPTY, "),
    binding!("A12.15"; 209; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "dynamic_mode"; "DynamicMode"; "\"default\""; Some("DynamicModeConverter.class"); false; "F-Enum-DynamicMode"; "S:D/E"; Converted; "dynamic_mode=\"DEFAULT\", "),
    binding!("A12.16"; 210; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "enable_propeller_optimize_absolute_paths"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "enable_propeller_optimize_absolute_paths=\"true\", "),
    binding!("A12.17"; 211; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "enable_remaining_fdo_absolute_paths"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "enable_remaining_fdo_absolute_paths=\"true\", "),
    binding!("A12.18"; 212; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_cc_implementation_deps"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_cc_implementation_deps=\"true\", "),
    binding!("A12.19"; 213; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_cpp_compile_resource_estimation"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_cpp_compile_resource_estimation=\"false\", "),
    binding!("A12.20"; 214; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_cpp_modules"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_cpp_modules=\"false\", "),
    binding!("A12.21"; 215; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_generate_llvm_lcov"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_generate_llvm_lcov=\"false\", "),
    binding!("A12.22"; 216; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_inmemory_dotd_files"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_inmemory_dotd_files=\"true\", "),
    binding!("A12.23"; 217; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_link_static_libraries_once"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_link_static_libraries_once=\"true\", "),
    binding!("A12.24"; 218; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_omitfp"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_omitfp=\"false\", "),
    binding!("A12.25"; 219; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_save_feature_state"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_save_feature_state=\"false\", "),
    binding!("A12.26"; 220; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_unsupported_and_brittle_include_scanning"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_unsupported_and_brittle_include_scanning=\"false\", "),
    binding!("A12.27"; 221; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_use_cpp_compile_action_args_params_file"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_use_cpp_compile_action_args_params_file=\"false\", "),
    binding!("A12.28"; 222; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "experimental_use_llvm_covmap"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_use_llvm_covmap=\"false\", "),
    binding!("A12.29"; 223; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "fdo_instrument"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "fdo_instrument=NULL, "),
    binding!("A12.30"; 224; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "fdo_optimize"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "fdo_optimize=NULL, "),
    binding!("A12.33"; 227; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "fission"; "List<CompilationMode>"; "\"no\""; Some("FissionOptionConverter.class"); false; "F-Fission"; "S:D/E"; Converted; "fission=EMPTY, "),
    binding!("A12.34"; 228; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "force_pic"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "force_pic=\"false\", "),
    binding!("A12.36"; 230; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_compiler"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "host_compiler=NULL, "),
    binding!("A12.37"; 231; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_conlyopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "host_conlyopt=EMPTY, "),
    binding!("A12.38"; 232; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_copt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "host_copt=EMPTY, "),
    binding!("A12.39"; 233; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_cxxopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "host_cxxopt=EMPTY, "),
    binding!("A12.41"; 235; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "host_linkopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "host_linkopt=EMPTY, "),
    binding!("A12.43"; 237; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_disable_legacy_cc_provider"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_disable_legacy_cc_provider=\"true\", "),
    binding!("A12.44"; 238; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_disable_nocopts"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_disable_nocopts=\"true\", "),
    binding!("A12.45"; 239; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_dont_enable_host_nonhost_crosstool_features"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_dont_enable_host_nonhost_crosstool_features=\"true\", "),
    binding!("A12.46"; 240; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_enable_cc_toolchain_resolution"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_enable_cc_toolchain_resolution=\"true\", "),
    binding!("A12.47"; 241; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_make_thinlto_command_lines_standalone"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_make_thinlto_command_lines_standalone=\"true\", "),
    binding!("A12.48"; 242; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_remove_legacy_whole_archive"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_remove_legacy_whole_archive=\"true\", "),
    binding!("A12.49"; 243; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_require_ctx_in_configure_features"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_require_ctx_in_configure_features=\"true\", "),
    binding!("A12.50"; 244; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_use_cpp_compile_header_mnemonic"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_use_cpp_compile_header_mnemonic=\"false\", "),
    binding!("A12.51"; 245; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_use_specific_tool_files"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_use_specific_tool_files=\"true\", "),
    binding!("A12.52"; 246; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "incompatible_validate_top_level_header_inclusions"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_validate_top_level_header_inclusions=\"true\", "),
    binding!("A12.53"; 247; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "interface_shared_objects"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "interface_shared_objects=\"true\", "),
    binding!("A12.54"; 248; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "legacy_whole_archive"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "legacy_whole_archive=\"true\", "),
    binding!("A12.55"; 249; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "linkopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "linkopt=EMPTY, "),
    binding!("A12.56"; 250; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "ltobackendopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "ltobackendopt=EMPTY, "),
    binding!("A12.57"; 251; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "ltoindexopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "ltoindexopt=EMPTY, "),
    binding!("A12.59"; 253; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "minimum_os_version"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "minimum_os_version=NULL, "),
    binding!("A12.60"; 254; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "objc_enable_binary_stripping"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "objc_enable_binary_stripping=\"false\", "),
    binding!("A12.61"; 255; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "objc_generate_linkmap"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "objc_generate_linkmap=\"false\", "),
    binding!("A12.62"; 256; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "objc_use_dotd_pruning"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "objc_use_dotd_pruning=\"true\", "),
    binding!("A12.63"; 257; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "objccopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "objccopt=EMPTY, "),
    binding!("A12.66"; 260; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "process_headers_in_dependencies"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "process_headers_in_dependencies=\"false\", "),
    binding!("A12.68"; 262; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "propeller_optimize_absolute_cc_profile"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "propeller_optimize_absolute_cc_profile=NULL, "),
    binding!("A12.69"; 263; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "propeller_optimize_absolute_ld_profile"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "propeller_optimize_absolute_ld_profile=NULL, "),
    binding!("A12.70"; 264; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "proto_profile"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "proto_profile=\"true\", "),
    binding!("A12.72"; 266; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "save_temps"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "save_temps=\"false\", "),
    binding!("A12.73"; 267; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "share_native_deps"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "share_native_deps=\"true\", "),
    binding!("A12.74"; 268; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "start_end_lib"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "start_end_lib=\"true\", "),
    binding!("A12.75"; 269; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "strict_system_includes"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "strict_system_includes=\"false\", "),
    binding!("A12.76"; 270; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "strip"; "StripMode"; "\"sometimes\""; Some("StripModeConverter.class"); false; "F-Enum-StripMode"; "S:D/E"; Converted; "strip=\"sometimes\", "),
    binding!("A12.77"; 271; "com.google.devtools.build.lib.rules.cpp.CppOptions"; "stripopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "stripopt=EMPTY, "),
    binding!("A13.01"; 273; "com.google.devtools.build.lib.rules.java.JavaOptions"; "bytecode_optimization_pass_actions"; "int"; "\"1\""; None; false; "F-Int"; "S:D/E"; Converted; "bytecode_optimization_pass_actions=\"1\", "),
    binding!("A13.03"; 275; "com.google.devtools.build.lib.rules.java.JavaOptions"; "enforce_proguard_file_extension"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "enforce_proguard_file_extension=\"false\", "),
    binding!("A13.04"; 276; "com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_add_test_support_to_compile_time_deps"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_add_test_support_to_compile_time_deps=\"true\", "),
    binding!("A13.05"; 277; "com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_enable_jspecify"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_enable_jspecify=\"true\", "),
    binding!("A13.06"; 278; "com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_fix_deps_tool"; "String"; "\"add_dep\""; None; false; "F-Text"; "S:D/E"; Converted; "experimental_fix_deps_tool=\"add_dep\", "),
    binding!("A13.07"; 279; "com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_inmemory_jdeps_files"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_inmemory_jdeps_files=\"true\", "),
    binding!("A13.08"; 280; "com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_java_classpath"; "JavaClasspathMode"; "\"bazel\""; Some("JavaClasspathModeConverter.class"); false; "F-Enum-JavaClasspathMode"; "S:D/E"; Converted; "experimental_java_classpath=\"BAZEL\", "),
    binding!("A13.09"; 281; "com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_java_test_auto_create_deploy_jar"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_java_test_auto_create_deploy_jar=\"false\", "),
    binding!("A13.11"; 283; "com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_local_java_optimizations"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_local_java_optimizations=\"false\", "),
    binding!("A13.12"; 284; "com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_one_version_enforcement"; "OneVersionEnforcementLevel"; "\"OFF\""; Some("OneVersionEnforcementLevelConverter.class"); false; "F-Enum-JavaOneVersionLevel"; "S:D/E"; Converted; "experimental_one_version_enforcement=\"OFF\", "),
    binding!("A13.13"; 285; "com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_run_android_lint_on_java_rules"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_run_android_lint_on_java_rules=\"false\", "),
    binding!("A13.14"; 286; "com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_strict_java_deps"; "StrictDepsMode"; "\"default\""; Some("StrictDepsConverter.class"); false; "F-Enum-StrictDeps"; "S:D/E"; Converted; "experimental_strict_java_deps=\"DEFAULT\", "),
    binding!("A13.15"; 287; "com.google.devtools.build.lib.rules.java.JavaOptions"; "experimental_turbine_annotation_processing"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_turbine_annotation_processing=\"false\", "),
    binding!("A13.16"; 288; "com.google.devtools.build.lib.rules.java.JavaOptions"; "explicit_java_test_deps"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "explicit_java_test_deps=\"false\", "),
    binding!("A13.18"; 290; "com.google.devtools.build.lib.rules.java.JavaOptions"; "host_javacopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "host_javacopt=EMPTY, "),
    binding!("A13.19"; 291; "com.google.devtools.build.lib.rules.java.JavaOptions"; "host_jvmopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "host_jvmopt=EMPTY, "),
    binding!("A13.20"; 292; "com.google.devtools.build.lib.rules.java.JavaOptions"; "incompatible_disallow_java_import_exports"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_disallow_java_import_exports=\"false\", "),
    binding!("A13.21"; 293; "com.google.devtools.build.lib.rules.java.JavaOptions"; "incompatible_multi_release_deploy_jars"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_multi_release_deploy_jars=\"true\", "),
    binding!("A13.22"; 294; "com.google.devtools.build.lib.rules.java.JavaOptions"; "java_debug"; "Void"; "\"null\""; None; false; "F-Void"; "S:N/E"; SpecialNull; "java_debug=NULL, "),
    binding!("A13.23"; 295; "com.google.devtools.build.lib.rules.java.JavaOptions"; "java_deps"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "java_deps=\"true\", "),
    binding!("A13.24"; 296; "com.google.devtools.build.lib.rules.java.JavaOptions"; "java_header_compilation"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "java_header_compilation=\"true\", "),
    binding!("A13.25"; 297; "com.google.devtools.build.lib.rules.java.JavaOptions"; "java_language_version"; "String"; "\"\""; None; false; "F-Text"; "S:D/E"; Converted; "java_language_version=\"\", "),
    binding!("A13.27"; 299; "com.google.devtools.build.lib.rules.java.JavaOptions"; "java_runtime_version"; "String"; "\"local_jdk\""; None; false; "F-Text"; "S:D/E"; Converted; "java_runtime_version=\"local_jdk\", "),
    binding!("A13.28"; 300; "com.google.devtools.build.lib.rules.java.JavaOptions"; "javacopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "javacopt=EMPTY, "),
    binding!("A13.29"; 301; "com.google.devtools.build.lib.rules.java.JavaOptions"; "jvmopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "jvmopt=EMPTY, "),
    binding!("A13.30"; 302; "com.google.devtools.build.lib.rules.java.JavaOptions"; "one_version_enforcement_on_java_tests"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "one_version_enforcement_on_java_tests=\"true\", "),
    binding!("A13.33"; 305; "com.google.devtools.build.lib.rules.java.JavaOptions"; "split_bytecode_optimization_pass"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "split_bytecode_optimization_pass=\"false\", "),
    binding!("A13.34"; 306; "com.google.devtools.build.lib.rules.java.JavaOptions"; "tool_java_language_version"; "String"; "\"\""; None; false; "F-Text"; "S:D/E"; Converted; "tool_java_language_version=\"\", "),
    binding!("A13.35"; 307; "com.google.devtools.build.lib.rules.java.JavaOptions"; "tool_java_runtime_version"; "String"; "\"remotejdk_11\""; None; false; "F-Text"; "S:D/E"; Converted; "tool_java_runtime_version=\"remotejdk_11\", "),
    binding!("A13.36"; 308; "com.google.devtools.build.lib.rules.java.JavaOptions"; "use_ijars"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "use_ijars=\"true\", "),
    binding!("A14.02"; 310; "com.google.devtools.build.lib.rules.objc.J2ObjcCommandLineOptions"; "j2objc_translation_flags"; "List<String>"; "\"null\""; Some("Converters.CommaSeparatedOptionListConverter.class"); true; "F-AllowCommaList"; "R:N/A"; RepeatEmpty; "j2objc_translation_flags=EMPTY, "),
    binding!("A15.01"; 311; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "device_debug_entitlements"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "device_debug_entitlements=\"true\", "),
    binding!("A15.02"; 312; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "experimental_objc_fastbuild_options"; "List<String>"; "\"-O0,-DDEBUG=1\""; Some("CommaSeparatedOptionListConverter.class"); false; "F-AllowCommaList"; "S:D/E"; Converted; "experimental_objc_fastbuild_options=\"[-O0, -DDEBUG=1]\", "),
    binding!("A15.03"; 313; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_avoid_hardcoded_objc_compilation_flags"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_avoid_hardcoded_objc_compilation_flags=\"true\", "),
    binding!("A15.04"; 314; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_builtin_objc_strip_action"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_builtin_objc_strip_action=\"true\", "),
    binding!("A15.05"; 315; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_disable_native_apple_binary_rule"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_disable_native_apple_binary_rule=\"false\", "),
    binding!("A15.06"; 316; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_disallow_sdk_frameworks_attributes"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_disallow_sdk_frameworks_attributes=\"false\", "),
    binding!("A15.07"; 317; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_objc_alwayslink_by_default"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_objc_alwayslink_by_default=\"false\", "),
    binding!("A15.08"; 318; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "incompatible_strip_executable_safely"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_strip_executable_safely=\"false\", "),
    binding!("A15.09"; 319; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "ios_memleaks"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "ios_memleaks=\"false\", "),
    binding!("A15.10"; 320; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "ios_signing_cert_name"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "ios_signing_cert_name=NULL, "),
    binding!("A15.11"; 321; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "ios_simulator_device"; "String"; "\"null\""; None; false; "F-Text"; "S:N/E"; SpecialNull; "ios_simulator_device=NULL, "),
    binding!("A15.12"; 322; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "ios_simulator_version"; "DottedVersion.Option"; "\"null\""; Some("DottedVersionConverter.class"); false; "F-Dotted"; "S:N/E"; SpecialNull; "ios_simulator_version=NULL, "),
    binding!("A15.13"; 323; "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions"; "objc_debug_with_GLIBCXX"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "objc_debug_with_GLIBCXX=\"false\", "),
    binding!("A16.01"; 324; "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "cc_proto_library_header_suffixes"; "List<String>"; "\".pb.h\""; Some("Converters.CommaSeparatedOptionSetConverter.class"); false; "F-StringSet"; "S:D/E"; Converted; "cc_proto_library_header_suffixes=\"[.pb.h]\", "),
    binding!("A16.02"; 325; "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "cc_proto_library_source_suffixes"; "List<String>"; "\".pb.cc\""; Some("Converters.CommaSeparatedOptionSetConverter.class"); false; "F-StringSet"; "S:D/E"; Converted; "cc_proto_library_source_suffixes=\"[.pb.cc]\", "),
    binding!("A16.03"; 326; "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "experimental_proto_descriptor_sets_include_source_info"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_proto_descriptor_sets_include_source_info=\"false\", "),
    binding!("A16.09"; 332; "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "protocopt"; "List<String>"; "\"null\""; None; true; "F-Text"; "R:N/A"; RepeatEmpty; "protocopt=EMPTY, "),
    binding!("A16.10"; 333; "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "strict_proto_deps"; "StrictDepsMode"; "\"error\""; Some("CoreOptionConverters.StrictDepsConverter.class"); false; "F-Enum-StrictDeps"; "S:D/E"; Converted; "strict_proto_deps=\"ERROR\", "),
    binding!("A16.11"; 334; "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options"; "strict_public_imports"; "StrictDepsMode"; "\"off\""; Some("CoreOptionConverters.StrictDepsConverter.class"); false; "F-Enum-StrictDeps"; "S:D/E"; Converted; "strict_public_imports=\"OFF\", "),
    binding!("A17.01"; 335; "com.google.devtools.build.lib.rules.python.PythonOptions"; "build_python_zip"; "TriState"; "\"auto\""; None; false; "F-Tri"; "S:D/E"; Converted; "build_python_zip=\"AUTO\", "),
    binding!("A17.02"; 336; "com.google.devtools.build.lib.rules.python.PythonOptions"; "experimental_py_binaries_include_label"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "experimental_py_binaries_include_label=\"false\", "),
    binding!("A17.03"; 337; "com.google.devtools.build.lib.rules.python.PythonOptions"; "incompatible_default_to_explicit_init_py"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_default_to_explicit_init_py=\"false\", "),
    binding!("A17.04"; 338; "com.google.devtools.build.lib.rules.python.PythonOptions"; "incompatible_python_disallow_native_rules"; "boolean"; "\"false\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_python_disallow_native_rules=\"false\", "),
    binding!("A17.05"; 339; "com.google.devtools.build.lib.rules.python.PythonOptions"; "incompatible_remove_ctx_py_fragment"; "boolean"; "\"true\""; None; false; "F-Bool"; "S:D/E"; Converted; "incompatible_remove_ctx_py_fragment=\"true\", "),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeferredCohort {
    Regex,
    Host,
    Repository,
}

#[rustfmt::skip]
const RETRY7_EXCLUSIONS: &[(usize, &str, &str, DeferredCohort)] = &[
    (2, "com.google.devtools.build.lib.analysis.PlatformOptions", "host_platform", DeferredCohort::Repository),
    (4, "com.google.devtools.build.lib.analysis.PlatformOptions", "platform_mappings", DeferredCohort::Host),
    (5, "com.google.devtools.build.lib.analysis.PlatformOptions", "platforms", DeferredCohort::Repository),
    (6, "com.google.devtools.build.lib.analysis.PlatformOptions", "toolchain_resolution_debug", DeferredCohort::Regex),
    (7, "com.google.devtools.build.lib.analysis.ShellConfiguration.Options", "shell_executable", DeferredCohort::Host),
    (14, "com.google.devtools.build.lib.analysis.config.CoreOptions", "archived_tree_artifact_mnemonics_filter", DeferredCohort::Regex),
    (21, "com.google.devtools.build.lib.analysis.config.CoreOptions", "cpu", DeferredCohort::Host),
    (27, "com.google.devtools.build.lib.analysis.config.CoreOptions", "experimental_action_listener", DeferredCohort::Repository),
    (38, "com.google.devtools.build.lib.analysis.config.CoreOptions", "experimental_override_platform_cpu_name", DeferredCohort::Repository),
    (40, "com.google.devtools.build.lib.analysis.config.CoreOptions", "experimental_propagate_custom_flag", DeferredCohort::Repository),
    (47, "com.google.devtools.build.lib.analysis.config.CoreOptions", "flag_alias", DeferredCohort::Repository),
    (50, "com.google.devtools.build.lib.analysis.config.CoreOptions", "host_cpu", DeferredCohort::Host),
    (62, "com.google.devtools.build.lib.analysis.config.CoreOptions", "incompatible_limit_platforms_in_output_dir_to", DeferredCohort::Repository),
    (67, "com.google.devtools.build.lib.analysis.config.CoreOptions", "instrumentation_filter", DeferredCohort::Regex),
    (70, "com.google.devtools.build.lib.analysis.config.CoreOptions", "modify_execution_info", DeferredCohort::Regex),
    (72, "com.google.devtools.build.lib.analysis.config.CoreOptions", "run_under", DeferredCohort::Repository),
    (76, "com.google.devtools.build.lib.analysis.config.CoreOptions", "target_environment", DeferredCohort::Repository),
    (79, "com.google.devtools.build.lib.analysis.test.CoverageConfiguration.CoverageOptions", "coverage_output_generator", DeferredCohort::Repository),
    (80, "com.google.devtools.build.lib.analysis.test.CoverageConfiguration.CoverageOptions", "coverage_report_generator", DeferredCohort::Repository),
    (83, "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions", "coverage_support", DeferredCohort::Repository),
    (84, "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions", "default_test_resources", DeferredCohort::Host),
    (91, "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions", "runs_per_test", DeferredCohort::Regex),
    (115, "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options", "android_platforms", DeferredCohort::Repository),
    (157, "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options", "legacy_main_dex_list_generator", DeferredCohort::Repository),
    (159, "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options", "optimizing_dexer", DeferredCohort::Repository),
    (169, "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions", "apple_platforms", DeferredCohort::Repository),
    (192, "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions", "xcode_version_config", DeferredCohort::Repository),
    (203, "com.google.devtools.build.lib.rules.cpp.CppOptions", "crosstool_top", DeferredCohort::Repository),
    (206, "com.google.devtools.build.lib.rules.cpp.CppOptions", "cs_fdo_profile", DeferredCohort::Repository),
    (207, "com.google.devtools.build.lib.rules.cpp.CppOptions", "custom_malloc", DeferredCohort::Repository),
    (225, "com.google.devtools.build.lib.rules.cpp.CppOptions", "fdo_prefetch_hints", DeferredCohort::Repository),
    (226, "com.google.devtools.build.lib.rules.cpp.CppOptions", "fdo_profile", DeferredCohort::Repository),
    (229, "com.google.devtools.build.lib.rules.cpp.CppOptions", "grte_top", DeferredCohort::Repository),
    (234, "com.google.devtools.build.lib.rules.cpp.CppOptions", "host_grte_top", DeferredCohort::Repository),
    (236, "com.google.devtools.build.lib.rules.cpp.CppOptions", "host_per_file_copt", DeferredCohort::Regex),
    (252, "com.google.devtools.build.lib.rules.cpp.CppOptions", "memprof_profile", DeferredCohort::Repository),
    (258, "com.google.devtools.build.lib.rules.cpp.CppOptions", "per_file_copt", DeferredCohort::Regex),
    (259, "com.google.devtools.build.lib.rules.cpp.CppOptions", "per_file_ltobackendopt", DeferredCohort::Regex),
    (261, "com.google.devtools.build.lib.rules.cpp.CppOptions", "propeller_optimize", DeferredCohort::Repository),
    (265, "com.google.devtools.build.lib.rules.cpp.CppOptions", "proto_profile_path", DeferredCohort::Repository),
    (272, "com.google.devtools.build.lib.rules.cpp.CppOptions", "xbinary_fdo", DeferredCohort::Repository),
    (274, "com.google.devtools.build.lib.rules.java.JavaOptions", "bytecode_optimizers", DeferredCohort::Repository),
    (282, "com.google.devtools.build.lib.rules.java.JavaOptions", "experimental_local_java_optimization_configuration", DeferredCohort::Repository),
    (289, "com.google.devtools.build.lib.rules.java.JavaOptions", "host_java_launcher", DeferredCohort::Repository),
    (298, "com.google.devtools.build.lib.rules.java.JavaOptions", "java_launcher", DeferredCohort::Repository),
    (303, "com.google.devtools.build.lib.rules.java.JavaOptions", "plugin", DeferredCohort::Repository),
    (304, "com.google.devtools.build.lib.rules.java.JavaOptions", "proguard_top", DeferredCohort::Repository),
    (309, "com.google.devtools.build.lib.rules.objc.J2ObjcCommandLineOptions", "j2objc_dead_code_report", DeferredCohort::Repository),
    (327, "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options", "proto_compiler", DeferredCohort::Repository),
    (328, "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options", "proto_toolchain_for_cc", DeferredCohort::Repository),
    (329, "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options", "proto_toolchain_for_j2objc", DeferredCohort::Repository),
    (330, "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options", "proto_toolchain_for_java", DeferredCohort::Repository),
    (331, "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options", "proto_toolchain_for_javalite", DeferredCohort::Repository),
    (340, "com.google.devtools.build.lib.rules.python.PythonOptions", "python_native_rules_allowlist", DeferredCohort::Repository),
];

#[test]
fn retry7_descriptor_default_bindings_are_exact_and_disjoint() {
    assert_eq!(RETRY7_DEFAULT_BINDINGS.len(), 287);
    assert_eq!(RETRY7_EXCLUSIONS.len(), 54);
    let mut seen = vec![false; NATIVE_OPTION_DESCRIPTORS.len()];
    let mut ordinals = Vec::with_capacity(NATIVE_OPTION_DESCRIPTORS.len());
    let (mut class_number, mut local_number, mut previous_class) = (0usize, 0usize, "");
    for descriptor in NATIVE_OPTION_DESCRIPTORS {
        if descriptor.class_name != previous_class {
            class_number += 1;
            local_number = 1;
            previous_class = descriptor.class_name;
        } else {
            local_number += 1;
        }
        ordinals.push(format!("A{class_number:02}.{local_number:02}"));
    }
    for case in RETRY7_DEFAULT_BINDINGS {
        let actual = &NATIVE_OPTION_DESCRIPTORS[case.registry_index];
        assert_eq!(
            (actual.class_name, actual.canonical_name),
            (case.class_name, case.canonical_name),
            "{}",
            case.attachment
        );
        assert_eq!(
            (
                actual.field_type,
                actual.raw_default,
                actual.converter,
                actual.allow_multiple
            ),
            (
                case.field_type,
                case.raw_default,
                case.converter,
                case.allow_multiple
            ),
            "{}",
            case.attachment
        );
        assert_eq!(ordinals[case.registry_index], case.attachment);
        assert!(case.family.starts_with("F-"));
        assert!(matches!(case.route, "S:N/E" | "S:D/E" | "R:N/A"));
        match case.outcome {
            ExpectedDefaultOutcome::SpecialNull => {
                assert_eq!(case.route, "S:N/E");
                assert!(case.expected_cache.ends_with("=NULL, "));
            }
            ExpectedDefaultOutcome::RepeatEmpty => {
                assert_eq!(case.route, "R:N/A");
                assert!(case.expected_cache.ends_with("=EMPTY, "));
            }
            ExpectedDefaultOutcome::Converted => assert_eq!(case.route, "S:D/E"),
        }
        assert!(!std::mem::replace(&mut seen[case.registry_index], true));
    }
    let mut counts = [0usize; 3];
    for &(index, class_name, canonical_name, cohort) in RETRY7_EXCLUSIONS {
        let actual = &NATIVE_OPTION_DESCRIPTORS[index];
        assert_eq!(
            (actual.class_name, actual.canonical_name),
            (class_name, canonical_name)
        );
        counts[match cohort {
            DeferredCohort::Regex => 0,
            DeferredCohort::Host => 1,
            DeferredCohort::Repository => 2,
        }] += 1;
        assert!(!std::mem::replace(&mut seen[index], true));
    }
    assert_eq!(counts, [8, 5, 41]);
    assert!(seen.into_iter().all(|entry| entry));
}

// Phase 2 removes only this compile gate after supplying the child-private API.
// These tests are the reviewed implementation contract, not executable stubs.
mod retry7_private_kernel_contract {
    use std::sync::Arc;

    use allocative::Allocative;
    use dupe::Dupe;

    use super::super::cache_grammar::format_native_cache_field;
    use super::super::cache_grammar::native_cache_text;
    use super::super::convert::ConvertError;
    use super::super::convert::NativeFamily;
    use super::super::convert::classify;
    use super::super::convert::convert_duration;
    use super::super::convert::convert_occurrence;
    use super::super::defaults::materialize_default;
    use super::super::matching::NativeConfigSettingMatchError;
    use super::super::matching::matches;
    use super::super::matching::native_occurrence_matches;
    use super::super::value::Duration;
    use super::super::value::NativeOccurrence;
    use super::super::value::NativePairs;
    use super::super::value::NativeValue;
    use super::super::value::NativeValues;
    use super::super::value::RegexFilterDefaultSeed;
    use super::super::value::RegexFilterDefaultSemantic;
    use super::super::value::RunsPerTestSeed;
    use super::super::value::TriState;
    use super::*;

    fn option(class_name: &str, canonical_name: &str) -> &'static NativeOptionDescriptor {
        NATIVE_OPTION_DESCRIPTORS
            .iter()
            .find(|item| item.class_name == class_name && item.canonical_name == canonical_name)
            .unwrap()
    }

    fn occurrence_value(occurrence: NativeOccurrence) -> Option<NativeValue> {
        match occurrence {
            NativeOccurrence::Absent => None,
            NativeOccurrence::Scalar(value) => Some(value),
            NativeOccurrence::List(values) => Some(NativeValue::List(values)),
        }
    }

    fn field(option: &NativeOptionDescriptor, value: Option<&NativeValue>) -> String {
        format_native_cache_field(option.canonical_name, value)
    }

    fn converted_field(
        option: &NativeOptionDescriptor,
        input: &str,
    ) -> Result<String, ConvertError> {
        let value = occurrence_value(convert_occurrence(option, input)?);
        Ok(field(option, value.as_ref()))
    }

    #[test]
    fn native_config_setting_matching_is_typed_borrowed_and_conjunctive() {
        use super::super::configuration::SlugConfiguration;
        use super::super::host::AutoCpuToken;
        use super::super::host::HostConversionInputs;
        use super::super::host::HostPathFlavor;

        let configuration = SlugConfiguration::default_target(
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
        let bytes = configuration.canonical_bytes().as_ptr();
        assert!(
            configuration
                .matches_config_setting(
                    &[
                        ("compilation_mode".into(), "fastbuild".into()),
                        ("stamp".into(), "false".into()),
                    ],
                    &[],
                )
                .unwrap()
        );
        assert!(
            !configuration
                .matches_config_setting(&[("compilation_mode".into(), "opt".into())], &[])
                .unwrap()
        );
        assert!(
            !configuration
                .matches_config_setting(&[], &[("name".into(), "value".into())])
                .unwrap()
        );
        assert_eq!(configuration.canonical_bytes().as_ptr(), bytes);
        assert_eq!(
            configuration
                .matches_config_setting(&[("platform_mappings".into(), "mapping".into())], &[],),
            Err(NativeConfigSettingMatchError::NonConfigurableOption(
                "platform_mappings".into()
            ))
        );
        assert_eq!(
            configuration.matches_config_setting(&[("unknown".into(), "value".into())], &[]),
            Err(NativeConfigSettingMatchError::UnknownOption(
                "unknown".into()
            ))
        );
        for descriptor in NATIVE_OPTION_DESCRIPTORS
            .iter()
            .filter(|descriptor| descriptor.canonical_name.contains(' '))
        {
            assert_eq!(
                configuration.matches_config_setting(
                    &[(descriptor.canonical_name.into(), "anything".into())],
                    &[],
                ),
                Err(NativeConfigSettingMatchError::UnknownOption(
                    descriptor.canonical_name.into()
                )),
                "INTERNAL option became selectable: {}",
                descriptor.canonical_name
            );
        }
        assert_eq!(
            configuration
                .matches_config_setting(&[("compilation_mode".into(), "not-a-mode".into())], &[],),
            Err(NativeConfigSettingMatchError::InvalidValue(
                "compilation_mode".into()
            ))
        );

        let disabled_descriptor = option(
            "com.google.devtools.build.lib.analysis.config.CoreOptions",
            "incompatible_disable_select_on",
        );
        let mut options = configuration.option_records().to_vec();
        let disabled = options
            .iter_mut()
            .find(|record| record.canonical_name == disabled_descriptor.canonical_name)
            .unwrap();
        disabled.value = super::super::configuration::OptionValue::Native(
            convert_occurrence(disabled_descriptor, "stamp").unwrap(),
        );
        assert_eq!(
            matches(&options, &[("stamp".into(), "false".into())], &[]),
            Err(NativeConfigSettingMatchError::NonConfigurableOption(
                "stamp".into()
            ))
        );
    }

    #[test]
    fn repeatable_native_list_and_map_matching_follows_bazel_rules() {
        let actual = NativeOccurrence::List(NativeValues(Arc::from([
            NativeValue::Text("a".into()),
            NativeValue::Text("b".into()),
            NativeValue::Text("c".into()),
        ])));
        let expected = NativeOccurrence::List(NativeValues(Arc::from([
            NativeValue::Text("b".into()),
            NativeValue::Text("c".into()),
        ])));
        assert!(native_occurrence_matches(&actual, &expected, true));
        assert!(!native_occurrence_matches(
            &actual,
            &NativeOccurrence::Scalar(NativeValue::Text("d".into())),
            true,
        ));
        assert!(native_occurrence_matches(
            &NativeOccurrence::Absent,
            &NativeOccurrence::List(NativeValues(Arc::from([]))),
            true,
        ));

        let actual = NativeOccurrence::List(NativeValues(Arc::from([
            NativeValue::Entry("key".into(), "old".into()),
            NativeValue::Entry("other".into(), "kept".into()),
            NativeValue::Entry("key".into(), "new".into()),
        ])));
        assert!(native_occurrence_matches(
            &actual,
            &NativeOccurrence::Scalar(NativeValue::Entry("key".into(), "new".into())),
            true,
        ));
        assert!(!native_occurrence_matches(
            &actual,
            &NativeOccurrence::Scalar(NativeValue::Entry("key".into(), "old".into())),
            true,
        ));
        assert!(!native_occurrence_matches(
            &actual,
            &NativeOccurrence::Scalar(NativeValue::Entry("missing".into(), "new".into())),
            true,
        ));
    }

    #[test]
    fn every_attachment_binds_classification_default_value_and_exact_cache() {
        for case in RETRY7_DEFAULT_BINDINGS {
            let descriptor = option(case.class_name, case.canonical_name);
            assert_eq!(
                classify(descriptor).map(NativeFamily::as_str),
                Some(case.family)
            );
            let default = materialize_default(descriptor).unwrap();
            assert_eq!(field(descriptor, default.as_ref()), case.expected_cache);
            match case.outcome {
                ExpectedDefaultOutcome::SpecialNull => assert_eq!(default, None),
                ExpectedDefaultOutcome::RepeatEmpty => {
                    assert!(
                        matches!(default, Some(NativeValue::List(ref values)) if values.is_empty())
                    );
                }
                ExpectedDefaultOutcome::Converted => {
                    let input = case
                        .raw_default
                        .strip_prefix('"')
                        .and_then(|raw| raw.strip_suffix('"'))
                        .unwrap();
                    assert_eq!(
                        default,
                        occurrence_value(convert_occurrence(descriptor, input).unwrap()),
                        "{}",
                        case.attachment
                    );
                }
            }
        }
    }

    #[test]
    fn deferred_cohorts_and_runs_default_only_boundary_are_exact() {
        for &(_, class_name, canonical_name, _) in RETRY7_EXCLUSIONS {
            let descriptor = option(class_name, canonical_name);
            assert_eq!(classify(descriptor), None);
            assert_eq!(
                convert_occurrence(descriptor, "value"),
                Err(ConvertError::Unsupported)
            );
        }
        let runs = option(
            "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions",
            "runs_per_test",
        );
        let Some(NativeValue::List(values)) = materialize_default(runs).unwrap() else {
            panic!("Runs default must be a retained singleton list")
        };
        let NativeValue::Runs(seed) = &values[0] else {
            panic!("Runs default must retain a typed private seed")
        };
        assert_eq!(seed.positive_runs().get(), 1);
        assert_eq!(RunsPerTestSeed::one(), *seed);
        assert_eq!(
            field(runs, Some(&NativeValue::List(values))),
            "runs_per_test=\"[(?:(?>.*)) Options: [1]]\", "
        );
        assert_eq!(
            convert_occurrence(runs, "+2"),
            Err(ConvertError::Unsupported)
        );
    }

    #[test]
    fn regex_filter_defaults_are_private_exact_seeds() {
        let platform = "com.google.devtools.build.lib.analysis.PlatformOptions";
        let core = "com.google.devtools.build.lib.analysis.config.CoreOptions";
        let cases = [
            (
                option(platform, "toolchain_resolution_debug"),
                "-.*",
                "-(?:(?>.*))",
                RegexFilterDefaultSemantic::ExcludeAll,
            ),
            (
                option(core, "archived_tree_artifact_mnemonics_filter"),
                "-.*",
                "-(?:(?>.*))",
                RegexFilterDefaultSemantic::ExcludeAll,
            ),
            (
                option(core, "instrumentation_filter"),
                "-/javatests[/:],-/test/java[/:]",
                "-(?:(?>/javatests[/:])|(?>/test/java[/:]))",
                RegexFilterDefaultSemantic::InstrumentationDefault,
            ),
        ];
        let mut seeds = Vec::new();
        for (descriptor, original, rendered, semantic) in cases {
            assert_eq!(descriptor.field_type, "RegexFilter");
            assert_eq!(
                descriptor.converter,
                Some("RegexFilter.RegexFilterConverter.class")
            );
            assert_eq!(descriptor.raw_default, format!("\"{original}\""));
            assert!(!descriptor.allow_multiple);
            assert_eq!(classify(descriptor), None);
            let mut repeated = *descriptor;
            repeated.allow_multiple = true;
            assert_eq!(
                materialize_default(&repeated),
                Err(ConvertError::Unsupported)
            );
            let Some(NativeValue::RegexFilterDefault(seed)) =
                materialize_default(descriptor).unwrap()
            else {
                panic!("expected fixed RegexFilter seed")
            };
            assert_eq!(seed.original_input.as_str(), original);
            assert_eq!(seed.canonical_text(), rendered);
            assert_eq!(
                field(
                    descriptor,
                    Some(&NativeValue::RegexFilterDefault(seed.clone()))
                ),
                format!("{}=\"{rendered}\", ", descriptor.canonical_name)
            );
            for explicit in [original, "", ".*", "-other", "+other"] {
                assert_eq!(
                    convert_occurrence(descriptor, explicit),
                    Err(ConvertError::Unsupported)
                );
            }
            assert_eq!(seed.semantic, semantic);
            seeds.push(seed);
        }
        assert_eq!(seeds[0], seeds[1]);
        assert_eq!(seeds[0].cmp(&seeds[1]), std::cmp::Ordering::Equal);
        assert_ne!(seeds[0], seeds[2]);
        assert!(seeds[0] < seeds[2]);
        let changed_original = RegexFilterDefaultSeed::new(
            "different spelling",
            RegexFilterDefaultSemantic::ExcludeAll,
        );
        assert_eq!(seeds[0], changed_original);
        assert_eq!(seeds[0].cmp(&changed_original), std::cmp::Ordering::Equal);
        fn allocative<T: Allocative>() {}
        allocative::<RegexFilterDefaultSeed>();
        let source = include_str!("value.rs");
        let derive = source
            .split("pub(super) struct RegexFilterDefaultSeed")
            .next()
            .unwrap();
        let derive = derive
            .lines()
            .rev()
            .find(|line| line.starts_with("#[derive("))
            .unwrap();
        assert_eq!(derive, "#[derive(Clone, Debug, Allocative)]");
    }

    #[test]
    fn primitive_null_and_occurrence_shapes_are_exact() {
        let core = "com.google.devtools.build.lib.analysis.config.CoreOptions";
        let boolean = option(core, "allow_analysis_failures");
        for (input, expected) in [
            ("true", "true"),
            ("1", "true"),
            ("YES", "true"),
            ("t", "true"),
            ("y", "true"),
            ("false", "false"),
            ("0", "false"),
            ("No", "false"),
            ("f", "false"),
            ("n", "false"),
            ("null", "false"),
        ] {
            assert_eq!(
                converted_field(boolean, input).unwrap(),
                format!("allow_analysis_failures=\"{expected}\", ")
            );
        }
        assert_eq!(
            convert_occurrence(boolean, "maybe"),
            Err(ConvertError::Invalid)
        );

        let integer = option(core, "analysis_testing_deps_limit");
        for (input, expected) in [("-16", -16), ("0x10", 16), ("#10", 16), ("020", 16)] {
            assert_eq!(
                convert_occurrence(integer, input),
                Ok(NativeOccurrence::Scalar(NativeValue::Int(expected)))
            );
        }
        for input in ["0x", "2147483648", "08"] {
            assert_eq!(
                convert_occurrence(integer, input),
                Err(ConvertError::Invalid)
            );
        }

        let tri = option(core, "enable_runfiles");
        for (input, expected) in [
            ("null", "AUTO"),
            ("auto", "AUTO"),
            ("yes", "YES"),
            ("0", "NO"),
        ] {
            assert_eq!(
                converted_field(tri, input).unwrap(),
                format!("enable_runfiles=\"{expected}\", ")
            );
        }
        let bool_null = NativeOptionDescriptor {
            raw_default: "\"null\"",
            ..*boolean
        };
        assert_eq!(
            materialize_default(&bool_null),
            Ok(Some(NativeValue::Bool(false)))
        );
        let tri_null = NativeOptionDescriptor {
            raw_default: "\"null\"",
            ..*tri
        };
        assert_eq!(
            materialize_default(&tri_null),
            Ok(Some(NativeValue::Tri(TriState::Auto)))
        );

        let text = option(core, "platform_suffix");
        assert_eq!(materialize_default(text).unwrap(), None);
        assert_eq!(
            converted_field(text, "null").unwrap(),
            "platform_suffix=\"null\", "
        );
        let void = option(
            "com.google.devtools.build.lib.rules.java.JavaOptions",
            "java_debug",
        );
        assert_eq!(
            convert_occurrence(void, "null"),
            Ok(NativeOccurrence::Absent)
        );
        assert_eq!(convert_occurrence(void, "true"), Err(ConvertError::Invalid));

        let nonrepeat = option(
            "com.google.devtools.build.lib.analysis.PlatformOptions",
            "extra_execution_platforms",
        );
        assert!(
            matches!(convert_occurrence(nonrepeat, "a,b"), Ok(NativeOccurrence::Scalar(NativeValue::List(ref values))) if native_cache_text(&NativeValue::List(values.dupe())).unwrap() == "[a, b]")
        );
        let repeat = option(
            "com.google.devtools.build.lib.analysis.PlatformOptions",
            "extra_toolchains",
        );
        assert!(
            matches!(convert_occurrence(repeat, "a,,b"), Ok(NativeOccurrence::List(ref values)) if native_cache_text(&NativeValue::List(values.dupe())).unwrap() == "[a, , b]")
        );
        for (descriptor, input) in [
            (option(core, "allowed_cpu_values"), "b,a"),
            (
                option(
                    "com.google.devtools.build.lib.rules.cpp.CppOptions",
                    "fission",
                ),
                "dbg,fastbuild",
            ),
            (option(core, "affected by starlark transition"), "anything"),
        ] {
            assert!(matches!(
                convert_occurrence(descriptor, input),
                Ok(NativeOccurrence::Scalar(NativeValue::List(_)))
            ));
        }
    }

    #[test]
    fn lists_sets_entries_env_and_cache_bytes_are_exact() {
        let platform = "com.google.devtools.build.lib.analysis.PlatformOptions";
        let empty_default = option(platform, "extra_execution_platforms");
        assert_eq!(
            field(
                empty_default,
                materialize_default(empty_default).unwrap().as_ref()
            ),
            "extra_execution_platforms=EMPTY, "
        );
        assert_eq!(
            converted_field(empty_default, "a,,b").unwrap(),
            "extra_execution_platforms=\"[a, , b]\", "
        );
        let objc = option(
            "com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions",
            "experimental_objc_fastbuild_options",
        );
        assert_eq!(
            field(objc, materialize_default(objc).unwrap().as_ref()),
            "experimental_objc_fastbuild_options=\"[-O0, -DDEBUG=1]\", "
        );
        let core = "com.google.devtools.build.lib.analysis.config.CoreOptions";
        let set = option(core, "allowed_cpu_values");
        assert_eq!(
            converted_field(set, "\u{e000},\u{10000},\u{e000}").unwrap(),
            "allowed_cpu_values=\"[𐀀, ]\", "
        );
        let entry = option(core, "define");
        assert_eq!(
            converted_field(entry, "a=b=c").unwrap(),
            "define=\"a=b=c\", "
        );
        for input in ["=v", "a"] {
            assert_eq!(convert_occurrence(entry, input), Err(ConvertError::Invalid));
        }
        let env = option(core, "action_env");
        for (input, expected) in [
            ("N=V", "Set[name=N, value=V]"),
            ("N", "Inherit[name=N]"),
            ("=N", "Unset[name=N]"),
        ] {
            assert_eq!(
                converted_field(env, input).unwrap(),
                format!("action_env=\"{expected}\", ")
            );
        }
        for input in ["", "="] {
            assert_eq!(convert_occurrence(env, input), Err(ConvertError::Invalid));
        }
        let escaped = NativeValue::Text("a\\b\"c".into());
        assert_eq!(
            format_native_cache_field("x", Some(&escaped)),
            "x=\"a\\\\b\\\"c\", "
        );
    }

    #[test]
    fn dotted_duration_and_timeout_contracts_are_exact() {
        let dotted = option(
            "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions",
            "ios_minimum_os",
        );
        for input in [
            "1.0",
            "1",
            "1alpha2",
            "2147483647",
            "1a2147483647",
            "1.internal_build",
            "1.2.internal_build.!",
            "1.0.0",
            "1.A",
            "1.A_internal",
            "1.A_internal.!",
        ] {
            assert_eq!(
                converted_field(dotted, input).unwrap(),
                format!("ios_minimum_os=\"{input}\", ")
            );
        }
        assert_ne!(
            convert_occurrence(dotted, "1.0"),
            convert_occurrence(dotted, "1")
        );
        assert_ne!(
            convert_occurrence(dotted, "1.2.internal_build"),
            convert_occurrence(dotted, "1.2.internal_build.!")
        );
        assert_ne!(
            convert_occurrence(dotted, "1.A_internal"),
            convert_occurrence(dotted, "1.A_internal.!")
        );
        for input in [
            "",
            "A",
            "A_internal",
            "2147483648",
            "1a2147483648",
            "1_",
            "1..2",
        ] {
            assert_eq!(
                convert_occurrence(dotted, input),
                Err(ConvertError::Invalid)
            );
        }

        for (input, expected) in [
            ("0", "PT0S"),
            ("1d", "PT24H"),
            ("1h", "PT1H"),
            ("1m", "PT1M"),
            ("1s", "PT1S"),
            ("1ms", "PT0.001S"),
            ("1ns", "PT0.000000001S"),
            ("3661s", "PT1H1M1S"),
            ("18446744074s", "PT5124095H34M34S"),
            ("9223372036854775807ns", "PT2562047H47M16.854775807S"),
        ] {
            assert_eq!(
                native_cache_text(&convert_duration(input).unwrap()).unwrap(),
                expected
            );
        }
        for input in ["-1s", "1us", "1.0s", "9223372036854775808ns"] {
            assert_eq!(convert_duration(input), Err(ConvertError::Invalid));
        }
        let NativeValue::Duration(Duration { seconds, nanos }) =
            convert_duration("18446744074s").unwrap()
        else {
            panic!()
        };
        assert_eq!((seconds, nanos), (18_446_744_074, 0));
        let NativeValue::Duration(Duration { seconds, nanos }) =
            convert_duration("9223372036854775807ns").unwrap()
        else {
            panic!()
        };
        assert_eq!((seconds, nanos), (9_223_372_036, 854_775_807));
        assert!(nanos < 1_000_000_000);
        let max_days = i64::MAX / 86_400;
        assert!(convert_duration(&format!("{max_days}d")).is_ok());
        assert_eq!(
            convert_duration(&format!("{}d", max_days + 1)),
            Err(ConvertError::Invalid)
        );

        let timeout = option(
            "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions",
            "test_timeout",
        );
        for (input, expected) in [
            ("2,", "{short=PT2S, moderate=PT2S, long=PT2S, eternal=PT2S}"),
            (",2", "{short=PT2S, moderate=PT2S, long=PT2S, eternal=PT2S}"),
            ("+2", "{short=PT2S, moderate=PT2S, long=PT2S, eternal=PT2S}"),
            (
                "2,,3,4,5",
                "{short=PT2S, moderate=PT3S, long=PT4S, eternal=PT5S}",
            ),
            (
                "0,-1,3,4",
                "{short=PT1M, moderate=PT5M, long=PT3S, eternal=PT4S}",
            ),
            (
                "3661,61,900,3600",
                "{short=PT1H1M1S, moderate=PT1M1S, long=PT15M, eternal=PT1H}",
            ),
        ] {
            assert_eq!(
                converted_field(timeout, input).unwrap(),
                format!("test_timeout=\"{expected}\", ")
            );
        }
        for input in [
            "",
            "abc",
            "2147483648",
            "1,2,3",
            "1,2,,3,4",
            "1,2,3,4,",
            ",,2,,3,4,5",
        ] {
            assert_eq!(
                convert_occurrence(timeout, input),
                Err(ConvertError::Invalid)
            );
        }
        let NativeOccurrence::Scalar(NativeValue::OrderedMap(pairs)) =
            convert_occurrence(timeout, "3661,61,900,3600").unwrap()
        else {
            panic!()
        };
        let rendered: Vec<_> = pairs
            .iter()
            .map(|(key, value)| {
                (
                    native_cache_text(key).unwrap(),
                    native_cache_text(value).unwrap(),
                )
            })
            .collect();
        assert_eq!(
            rendered,
            [
                ("short".into(), "PT1H1M1S".into()),
                ("moderate".into(), "PT1M1S".into()),
                ("long".into(), "PT15M".into()),
                ("eternal".into(), "PT1H".into())
            ]
        );
        let clone = pairs.dupe();
        assert_eq!(pairs, clone);
        assert_eq!(pairs.as_ptr(), clone.as_ptr());
        let NativePairs(pair_arc) = clone;
        let _: Arc<[(NativeValue, NativeValue)]> = pair_arc;
    }

    #[test]
    fn shard_fission_platform_and_empty_routes_are_exact() {
        let test = "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions";
        let shard = option(test, "test_sharding_strategy");
        for (input, expected) in [
            ("explicit", "EXPLICIT"),
            ("disabled", "DISABLED"),
            ("FORCED=+0x10", "forced=16"),
            ("forced=020", "forced=16"),
        ] {
            assert_eq!(
                converted_field(shard, input).unwrap(),
                format!("test_sharding_strategy=\"{expected}\", ")
            );
        }
        for input in ["forced=-1", "forced=0x", "automatic"] {
            assert_eq!(convert_occurrence(shard, input), Err(ConvertError::Invalid));
        }
        let fission = option(
            "com.google.devtools.build.lib.rules.cpp.CppOptions",
            "fission",
        );
        for (input, expected) in [
            ("no", "fission=EMPTY, "),
            ("yes", "fission=\"[fastbuild, dbg, opt]\", "),
            ("DBG,fastbuild,dbg", "fission=\"[dbg, fastbuild]\", "),
        ] {
            assert_eq!(converted_field(fission, input).unwrap(), expected);
        }
        for input in ["YES", "No", "bad"] {
            assert_eq!(
                convert_occurrence(fission, input),
                Err(ConvertError::Invalid)
            );
        }
        let platform = option(
            "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions",
            "apple_platform_type",
        );
        assert_eq!(
            converted_field(platform, "MÄCOS").unwrap(),
            "apple_platform_type=\"mÄcos\", "
        );
        let empty = option(
            "com.google.devtools.build.lib.analysis.config.CoreOptions",
            "affected by starlark transition",
        );
        for input in ["", "anything"] {
            assert_eq!(
                converted_field(empty, input).unwrap(),
                "affected by starlark transition=EMPTY, "
            );
        }
    }

    #[test]
    fn every_enum_member_alias_and_renderer_is_discriminated() {
        let cases: &[(&str, &str, &[&str], bool)] = &[
            (
                "com.google.devtools.build.lib.analysis.config.CoreOptions",
                "compilation_mode",
                &["fastbuild", "dbg", "opt"],
                true,
            ),
            (
                "com.google.devtools.build.lib.analysis.config.CoreOptions",
                "experimental_exec_configuration_distinguisher",
                &["legacy", "off", "full_hash", "diff_to_affected"],
                false,
            ),
            (
                "com.google.devtools.build.lib.analysis.config.CoreOptions",
                "experimental_output_directory_naming_scheme",
                &[
                    "legacy",
                    "diff_against_baseline",
                    "diff_against_dynamic_baseline",
                ],
                false,
            ),
            (
                "com.google.devtools.build.lib.analysis.config.CoreOptions",
                "experimental_output_paths",
                &["off", "strip"],
                false,
            ),
            (
                "com.google.devtools.build.lib.analysis.config.CoreOptions",
                "include_config_fragments_provider",
                &["off", "direct", "transitive"],
                false,
            ),
            (
                "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions",
                "experimental_cancel_concurrent_tests",
                &["never", "on_failed", "on_passed", "true", "false"],
                false,
            ),
            (
                "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options",
                "Android configuration distinguisher",
                &["main", "android"],
                false,
            ),
            (
                "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options",
                "android_dynamic_mode",
                &["off", "default", "fully"],
                false,
            ),
            (
                "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options",
                "android_manifest_merger",
                &["legacy", "android", "force_android"],
                false,
            ),
            (
                "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options",
                "android_manifest_merger_order",
                &[
                    "alphabetical",
                    "alphabetical_by_configuration",
                    "dependency",
                ],
                false,
            ),
            (
                "com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options",
                "apk_signing_method",
                &["v1", "v2", "v1_v2", "v4"],
                false,
            ),
            (
                "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions",
                "apple configuration distinguisher",
                &[
                    "unknown",
                    "applebin_ios",
                    "applebin_visionos",
                    "applebin_watchos",
                    "applebin_tvos",
                    "applebin_macos",
                    "applebin_catalyst",
                    "apple_crosstool",
                ],
                false,
            ),
            (
                "com.google.devtools.build.lib.rules.cpp.CppOptions",
                "dynamic_mode",
                &["off", "default", "fully"],
                false,
            ),
            (
                "com.google.devtools.build.lib.rules.cpp.CppOptions",
                "strip",
                &["always", "sometimes", "never"],
                true,
            ),
            (
                "com.google.devtools.build.lib.rules.java.JavaOptions",
                "experimental_java_classpath",
                &["off", "javabuilder", "bazel", "bazel_no_fallback"],
                false,
            ),
            (
                "com.google.devtools.build.lib.rules.java.JavaOptions",
                "experimental_one_version_enforcement",
                &["off", "warning", "error"],
                false,
            ),
            (
                "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options",
                "strict_proto_deps",
                &["off", "warn", "error", "strict", "default"],
                false,
            ),
        ];
        for &(class_name, canonical_name, members, lowercase_renderer) in cases {
            let descriptor = option(class_name, canonical_name);
            for &member in members {
                let NativeOccurrence::Scalar(value) =
                    convert_occurrence(descriptor, member).unwrap()
                else {
                    panic!()
                };
                let rendered = native_cache_text(&value).unwrap();
                if canonical_name == "experimental_cancel_concurrent_tests" && member == "true" {
                    assert_eq!(rendered, "ON_PASSED");
                } else if canonical_name == "experimental_cancel_concurrent_tests"
                    && member == "false"
                {
                    assert_eq!(rendered, "NEVER");
                } else if lowercase_renderer {
                    assert_eq!(rendered, member.to_ascii_lowercase());
                } else {
                    assert_eq!(rendered, member.to_ascii_uppercase());
                }
                assert_eq!(
                    convert_occurrence(descriptor, member),
                    convert_occurrence(descriptor, &member.to_ascii_uppercase())
                );
            }
            assert_eq!(
                convert_occurrence(descriptor, "not-a-member"),
                Err(ConvertError::Invalid)
            );
        }
        assert_ne!(
            convert_occurrence(
                option(
                    "com.google.devtools.build.lib.analysis.config.CoreOptions",
                    "experimental_output_paths"
                ),
                "off"
            ),
            convert_occurrence(
                option(
                    "com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options",
                    "strict_proto_deps"
                ),
                "off"
            ),
        );
    }

    #[test]
    fn retained_traits_privacy_and_forbidden_surfaces_are_exact() {
        fn allocative<T: Allocative>() {}
        fn dupe<T: Dupe>() {}
        allocative::<NativeValue>();
        allocative::<NativeValues>();
        allocative::<NativePairs>();
        dupe::<NativeValues>();
        dupe::<NativePairs>();
        let repeat = option(
            "com.google.devtools.build.lib.analysis.PlatformOptions",
            "extra_toolchains",
        );
        let NativeOccurrence::List(values) = convert_occurrence(repeat, "a,b").unwrap() else {
            panic!()
        };
        let cloned_values = values.dupe();
        assert_eq!(values, cloned_values);
        assert_eq!(values.as_ptr(), cloned_values.as_ptr());
        let sources = [
            include_str!("value.rs"),
            include_str!("defaults.rs"),
            include_str!("convert.rs"),
            include_str!("cache_grammar.rs"),
            include_str!("matching.rs"),
        ]
        .join("\n");
        for forbidden in [
            "java_regex",
            "argv",
            "checksum",
            "wire",
            "DICE",
            "RepositoryMapping",
            "CommandLine",
            "HashMap",
            "HashSet",
            "BTreeMap",
            "BTreeSet",
            "DashMap",
            "Mutex",
            "RwLock",
            "RefCell",
            "UnsafeCell",
            "Atomic",
            "LazyLock",
            "OnceLock",
            "thread_local!",
            "static mut",
            "Interner",
            "from_utf8_lossy",
            "from_utf16_lossy",
            "regex::",
            "Pattern::",
            "Matcher::",
            "coverage",
            "to_original",
            "is_included",
            "�",
        ] {
            assert!(
                !sources.contains(forbidden),
                "forbidden deferred surface: {forbidden}"
            );
        }
        for line in sources.lines().map(str::trim) {
            assert!(!line.split_whitespace().any(|token| token == "static"));
        }
        let public_exports: Vec<_> = include_str!("mod.rs")
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("pub use "))
            .collect();
        assert_eq!(
            public_exports,
            [
                "pub use cache_grammar::CacheFieldValue;",
                "pub use cache_grammar::format_cache_field;",
                "pub use configuration::NativeStringListOption;",
                "pub use configuration::PreparedCommandNativeOptions;",
                "pub use configuration::SlugConfiguration;",
                "pub use configuration::SlugConfigurationError;",
                "pub use configuration::SlugConfigurationKind;",
                "pub use configuration::SlugConfigurationProjection;",
                "pub use configuration::StarlarkOption;",
                "pub use configuration::StarlarkOptionScope;",
                "pub use configuration::StarlarkOptionValue;",
                "pub use configuration::StarlarkOptions;",
                "pub use matching::NativeConfigSettingMatchError;",
                "pub use registry::NATIVE_OPTION_DESCRIPTORS;",
                "pub use registry::NativeOptionDescriptor;",
            ]
        );
    }
}

mod label_only_converter_contract {
    use std::sync::Arc;

    use allocative::Allocative;
    use compact_str::CompactString;
    use dupe::Dupe;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalRepoName;
    use slug_identity_v2::OptionLabelContext;
    use slug_identity_v2::PackageIdentifier;
    use slug_identity_v2::RepositoryMapping;
    use slug_identity_v2::RepositoryMappingId;

    use super::super::label_convert::FlagAliasEntry;
    use super::super::label_convert::LabelConvertError;
    use super::super::label_convert::LabelFamily;
    use super::super::label_convert::LabelMapValues;
    use super::super::label_convert::LabelToStringEntry;
    use super::super::label_convert::LabelValue;
    use super::super::label_convert::LabelValues;
    use super::super::label_convert::MixedFamily;
    use super::super::label_convert::MixedValue;
    use super::super::label_convert::RunUnder;
    use super::super::label_convert::RunUnderSuffix;
    use super::super::label_convert::classify as classify_label;
    use super::super::label_convert::classify_mixed;
    use super::super::label_convert::convert_label_occurrence;
    use super::super::label_convert::convert_mixed_occurrence;
    use super::super::label_convert::materialize_label_default;
    use super::super::label_convert::materialize_mixed_default;
    use super::*;

    fn option(name: &str) -> &'static NativeOptionDescriptor {
        NATIVE_OPTION_DESCRIPTORS
            .iter()
            .find(|option| option.canonical_name == name)
            .unwrap()
    }

    fn mapping() -> RepositoryMapping {
        let mut mapping = RepositoryMapping::new(RepositoryMappingId::new("test").unwrap());
        mapping.insert(
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new("mapped").unwrap(),
        );
        mapping
    }

    fn scalar(value: Option<LabelValue>) -> String {
        let Some(LabelValue::Label(value)) = value else {
            panic!("expected one resolved label")
        };
        value.to_string()
    }

    fn labels(value: Option<LabelValue>) -> LabelValues {
        let Some(LabelValue::Labels(value)) = value else {
            panic!("expected an immutable label slice")
        };
        value
    }

    fn label_to_string_entry(value: Option<LabelValue>) -> LabelToStringEntry {
        let Some(LabelValue::LabelToStringEntry(value)) = value else {
            panic!("expected a label-to-string entry")
        };
        value
    }

    fn label_map(value: Option<LabelValue>) -> LabelMapValues {
        let Some(LabelValue::LabelMap(value)) = value else {
            panic!("expected an immutable ordered label map")
        };
        value
    }

    fn flag_alias(value: Option<LabelValue>) -> FlagAliasEntry {
        let Some(LabelValue::FlagAlias(value)) = value else {
            panic!("expected a flag alias entry")
        };
        value
    }

    fn run_under(value: Option<MixedValue>) -> RunUnder {
        let Some(MixedValue::RunUnder(value)) = value else {
            panic!("expected a run-under value")
        };
        value
    }

    fn custom_flag(value: Option<MixedValue>) -> String {
        let Some(MixedValue::CustomFlag(value)) = value else {
            panic!("expected a custom flag")
        };
        value.to_string()
    }

    fn unsupported(name: &str) {
        assert_eq!(classify_label(option(name)), None, "{name}");
        assert_eq!(
            convert_label_occurrence(
                option(name),
                "//p:t",
                OptionLabelContext::FirstRoundCanonical,
            ),
            Err(LabelConvertError::Unsupported),
            "{name}"
        );
        assert_eq!(
            materialize_label_default(option(name), OptionLabelContext::FirstRoundCanonical),
            Err(LabelConvertError::Unsupported),
            "{name} default"
        );
    }

    #[test]
    fn admits_exactly_thirty_nine_routes_and_leaves_only_mixed_and_other_cohorts_deferred() {
        let mut admitted = 0;
        let mut mixed = 0;
        let mut host = 0;
        let mut regex = 0;
        for option in NATIVE_OPTION_DESCRIPTORS {
            admitted += usize::from(classify_label(option).is_some());
            mixed += usize::from(matches!(
                option.converter,
                Some("RunUnderConverter.class" | "CoreOptionConverters.CustomFlagConverter.class")
            ));
            host += usize::from(matches!(
                option.converter,
                Some(
                    "AutoCpuConverter.class"
                        | "PathFragmentConverter.class"
                        | "PlatformMappingKeyConverter.class"
                        | "TestResourcesConverter.class"
                )
            ));
            regex += usize::from(matches!(
                option.converter,
                Some(
                    "RegexFilter.RegexFilterConverter.class"
                        | "ExecutionInfoModifier.Converter.class"
                        | "PerLabelOptions.PerLabelOptionsConverter.class"
                        | "RunsPerTestConverter.class"
                )
            ));
        }
        assert_eq!((admitted, mixed, host, regex), (39, 2, 5, 8));
        assert_eq!(admitted + mixed, 41);
        let mut actual = NATIVE_OPTION_DESCRIPTORS
            .iter()
            .filter(|option| classify_label(option).is_some())
            .map(|option| option.canonical_name)
            .collect::<Vec<_>>();
        let mut expected = vec![
            "coverage_output_generator",
            "coverage_report_generator",
            "coverage_support",
            "legacy_main_dex_list_generator",
            "xcode_version_config",
            "crosstool_top",
            "cs_fdo_profile",
            "custom_malloc",
            "fdo_prefetch_hints",
            "memprof_profile",
            "propeller_optimize",
            "proto_profile_path",
            "experimental_local_java_optimization_configuration",
            "proguard_top",
            "j2objc_dead_code_report",
            "python_native_rules_allowlist",
            "optimizing_dexer",
            "fdo_profile",
            "xbinary_fdo",
            "host_java_launcher",
            "java_launcher",
            "platforms",
            "experimental_action_listener",
            "incompatible_limit_platforms_in_output_dir_to",
            "target_environment",
            "apple_platforms",
            "plugin",
            "android_platforms",
            "grte_top",
            "host_grte_top",
            "host_platform",
            "proto_compiler",
            "proto_toolchain_for_cc",
            "proto_toolchain_for_j2objc",
            "proto_toolchain_for_java",
            "proto_toolchain_for_javalite",
            "experimental_override_platform_cpu_name",
            "bytecode_optimizers",
            "flag_alias",
        ];
        actual.sort_unstable();
        expected.sort_unstable();
        assert_eq!(actual, expected);
        for name in [
            "run_under",
            "experimental_propagate_custom_flag",
            "cpu",
            "host_cpu",
            "shell_executable",
            "platform_mappings",
            "default_test_resources",
            "toolchain_resolution_debug",
            "archived_tree_artifact_mnemonics_filter",
            "instrumentation_filter",
            "modify_execution_info",
            "host_per_file_copt",
            "per_file_copt",
            "per_file_ltobackendopt",
            "runs_per_test",
        ] {
            unsupported(name);
        }
    }

    #[test]
    fn supplied_context_controls_every_label_parse_without_becoming_output_state() {
        let label = option("custom_malloc");
        assert_eq!(
            scalar(
                convert_label_occurrence(
                    label,
                    "@alias//p:t",
                    OptionLabelContext::FirstRoundCanonical,
                )
                .unwrap()
            ),
            "@@alias//p:t"
        );

        let mut mapping = mapping();
        assert_eq!(
            scalar(
                convert_label_occurrence(
                    label,
                    "@alias//p:t",
                    OptionLabelContext::MainRepository { mapping: &mapping },
                )
                .unwrap()
            ),
            "@@mapped//p:t"
        );
        let retained = convert_label_occurrence(
            label,
            "@alias//p:t",
            OptionLabelContext::MainRepository { mapping: &mapping },
        )
        .unwrap();
        mapping.insert(
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new("changed").unwrap(),
        );
        assert_eq!(scalar(retained), "@@mapped//p:t");
        assert_eq!(
            scalar(
                convert_label_occurrence(
                    label,
                    "@missing//p:t",
                    OptionLabelContext::MainRepository { mapping: &mapping },
                )
                .unwrap()
            ),
            "@@[unknown repo 'missing' requested from @@]//p:t"
        );

        let base = PackageIdentifier::parse_bazel_package_identifier("@@owner//base").unwrap();
        assert_eq!(
            scalar(
                convert_label_occurrence(
                    label,
                    ":relative",
                    OptionLabelContext::Package {
                        base_package: &base,
                        mapping: &mapping,
                    },
                )
                .unwrap()
            ),
            "@@owner//base:relative"
        );
        assert_eq!(
            scalar(
                convert_label_occurrence(
                    label,
                    "@alias//p:t",
                    OptionLabelContext::Package {
                        base_package: &base,
                        mapping: &mapping,
                    },
                )
                .unwrap()
            ),
            "@@changed//p:t"
        );
        assert_eq!(
            scalar(
                convert_label_occurrence(
                    label,
                    "@missing//p:t",
                    OptionLabelContext::Package {
                        base_package: &base,
                        mapping: &mapping,
                    },
                )
                .unwrap()
            ),
            "@@[unknown repo 'missing' requested from @@owner]//p:t"
        );
    }

    #[test]
    fn list_and_ordered_set_convert_every_piece_before_retaining_ordered_arcs() {
        let mapping = mapping();
        let context = OptionLabelContext::MainRepository { mapping: &mapping };
        let list = labels(
            convert_label_occurrence(option("platforms"), "//a:x,,@alias//b:y,", context).unwrap(),
        );
        assert_eq!(
            list.0.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["//a:x", "@@mapped//b:y"]
        );

        let set = labels(
            convert_label_occurrence(
                option("android_platforms"),
                "@alias//p:x,@@mapped//p:x,//q:y",
                context,
            )
            .unwrap(),
        );
        assert_eq!(
            set.0.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["@@mapped//p:x", "//q:y"]
        );
        assert_eq!(
            convert_label_occurrence(option("android_platforms"), "//good:x,//bad:", context),
            Err(LabelConvertError::Invalid)
        );
    }

    #[test]
    fn original_thirty_route_defaults_are_literal_null_or_empty_only_when_their_route_allows_it() {
        let mut seen = 0;
        for option in NATIVE_OPTION_DESCRIPTORS.iter().filter(|option| {
            matches!(
                classify_label(option),
                Some(
                    LabelFamily::Label
                        | LabelFamily::EmptyToNull
                        | LabelFamily::List
                        | LabelFamily::OrderedSet
                        | LabelFamily::LibcTop
                )
            )
        }) {
            seen += 1;
            let actual =
                materialize_label_default(option, OptionLabelContext::FirstRoundCanonical).unwrap();
            let raw = option
                .raw_default
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .unwrap();
            match (classify_label(option).unwrap(), raw) {
                (LabelFamily::List, "") | (LabelFamily::OrderedSet, "") => {
                    assert!(labels(actual).0.is_empty(), "{}", option.canonical_name);
                }
                (LabelFamily::List, "null") if option.allow_multiple => {
                    assert!(labels(actual).0.is_empty(), "{}", option.canonical_name);
                }
                (_, "null") => assert!(actual.is_none(), "{}", option.canonical_name),
                _ => assert_eq!(
                    actual,
                    convert_label_occurrence(option, raw, OptionLabelContext::FirstRoundCanonical)
                        .unwrap(),
                    "{}",
                    option.canonical_name
                ),
            }
        }
        assert_eq!(seen, 30);
    }

    #[test]
    fn seven_routes_pin_literal_defaults_contexts_and_empty_behavior() {
        let mut mapping = mapping();
        mapping.insert(
            ApparentRepoName::new("bazel_tools").unwrap(),
            CanonicalRepoName::new("mapped_tools").unwrap(),
        );
        let base = PackageIdentifier::parse_bazel_package_identifier("@@owner//base").unwrap();
        let defaults = [
            (
                "host_platform",
                "DEFAULT_HOST_PLATFORM",
                "@bazel_tools//tools:host_platform",
                LabelFamily::HostPlatform,
            ),
            (
                "proto_compiler",
                "ProtoConstants.DEFAULT_PROTOC_LABEL",
                "@bazel_tools//tools/proto:protoc",
                LabelFamily::CoreLabel,
            ),
            (
                "proto_toolchain_for_cc",
                "ProtoConstants.DEFAULT_CC_PROTO_LABEL",
                "@bazel_tools//tools/proto:cc_toolchain",
                LabelFamily::CoreEmptyToNull,
            ),
            (
                "proto_toolchain_for_j2objc",
                "ProtoConstants.DEFAULT_J2OBJC_PROTO_LABEL",
                "@bazel_tools//tools/j2objc:j2objc_proto_toolchain",
                LabelFamily::CoreEmptyToNull,
            ),
            (
                "proto_toolchain_for_java",
                "ProtoConstants.DEFAULT_JAVA_PROTO_LABEL",
                "@bazel_tools//tools/proto:java_toolchain",
                LabelFamily::CoreEmptyToNull,
            ),
            (
                "proto_toolchain_for_javalite",
                "ProtoConstants.DEFAULT_JAVA_LITE_PROTO_LABEL",
                "@bazel_tools//tools/proto:javalite_toolchain",
                LabelFamily::CoreLabel,
            ),
        ];
        for (name, raw_default, literal, family) in defaults {
            let option = option(name);
            assert_eq!(option.raw_default, raw_default, "{name}");
            assert_eq!(classify_label(option), Some(family), "{name}");
            assert_eq!(
                scalar(
                    materialize_label_default(option, OptionLabelContext::FirstRoundCanonical)
                        .unwrap()
                ),
                format!("@@{}", literal.strip_prefix('@').unwrap()),
                "{name} first round"
            );
            assert_eq!(
                scalar(
                    materialize_label_default(
                        option,
                        OptionLabelContext::MainRepository { mapping: &mapping },
                    )
                    .unwrap()
                ),
                format!(
                    "@@mapped_tools{}",
                    literal.strip_prefix("@bazel_tools").unwrap()
                ),
                "{name} mapped main repository"
            );
            assert_eq!(
                scalar(
                    materialize_label_default(
                        option,
                        OptionLabelContext::Package {
                            base_package: &base,
                            mapping: &mapping,
                        },
                    )
                    .unwrap()
                ),
                format!(
                    "@@mapped_tools{}",
                    literal.strip_prefix("@bazel_tools").unwrap()
                ),
                "{name} mapped package"
            );
        }

        assert_eq!(
            scalar(
                convert_label_occurrence(
                    option("host_platform"),
                    "",
                    OptionLabelContext::FirstRoundCanonical,
                )
                .unwrap()
            ),
            "@@bazel_tools//tools:host_platform"
        );
        assert_eq!(
            scalar(
                convert_label_occurrence(
                    option("host_platform"),
                    "",
                    OptionLabelContext::MainRepository { mapping: &mapping },
                )
                .unwrap()
            ),
            "@@mapped_tools//tools:host_platform"
        );
        for name in [
            "proto_toolchain_for_cc",
            "proto_toolchain_for_j2objc",
            "proto_toolchain_for_java",
        ] {
            assert_eq!(
                convert_label_occurrence(option(name), "", OptionLabelContext::FirstRoundCanonical),
                Ok(None),
                "{name}"
            );
        }
        for name in ["proto_compiler", "proto_toolchain_for_javalite"] {
            assert_eq!(
                convert_label_occurrence(option(name), "", OptionLabelContext::FirstRoundCanonical),
                Err(LabelConvertError::Invalid),
                "{name} parses empty through the ordinary label path"
            );
        }
    }

    #[test]
    fn label_to_string_entry_is_exactly_one_untrimmed_assignment() {
        let option = option("experimental_override_platform_cpu_name");
        assert_eq!(
            classify_label(option),
            Some(LabelFamily::LabelToStringEntry)
        );
        assert!(option.allow_multiple);
        assert_eq!(option.raw_default, "\"null\"");
        assert_eq!(
            materialize_label_default(option, OptionLabelContext::FirstRoundCanonical),
            Ok(None)
        );
        for input in [
            "",
            "plain",
            "=value",
            "label=",
            "label==value",
            "label=value=tail",
        ] {
            assert_eq!(
                convert_label_occurrence(option, input, OptionLabelContext::FirstRoundCanonical),
                Err(LabelConvertError::Invalid),
                "{input:?}"
            );
        }

        let whitespace = label_to_string_entry(
            convert_label_occurrence(
                option,
                "//platforms:one=  exact value\t",
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        assert_eq!(whitespace.label.to_string(), "//platforms:one");
        assert_eq!(whitespace.value.as_str(), "  exact value\t");

        let mapping = mapping();
        let mapped = label_to_string_entry(
            convert_label_occurrence(
                option,
                "@alias//platforms:one=value",
                OptionLabelContext::MainRepository { mapping: &mapping },
            )
            .unwrap(),
        );
        assert_eq!(mapped.label.to_string(), "@@mapped//platforms:one");
        assert_eq!(mapped.value.as_str(), "value");
        let base = PackageIdentifier::parse_bazel_package_identifier("@@owner//base").unwrap();
        let relative = label_to_string_entry(
            convert_label_occurrence(
                option,
                ":relative=value",
                OptionLabelContext::Package {
                    base_package: &base,
                    mapping: &mapping,
                },
            )
            .unwrap(),
        );
        assert_eq!(relative.label.to_string(), "@@owner//base:relative");
        assert_eq!(relative.value.as_str(), "value");
        let nonvisible = label_to_string_entry(
            convert_label_occurrence(
                option,
                "@missing//platforms:one=value",
                OptionLabelContext::MainRepository { mapping: &mapping },
            )
            .unwrap(),
        );
        assert_eq!(
            nonvisible.label.to_string(),
            "@@[unknown repo 'missing' requested from @@]//platforms:one"
        );

        fn allocative<T: Allocative>() {}
        allocative::<LabelToStringEntry>();
    }

    #[test]
    fn label_map_trims_guava_whitespace_before_omitting_and_preserves_ordered_entries() {
        let option = option("bytecode_optimizers");
        assert_eq!(classify_label(option), Some(LabelFamily::LabelMap));
        assert_eq!(option.raw_default, "\"Proguard\"");
        let default = label_map(
            materialize_label_default(option, OptionLabelContext::FirstRoundCanonical).unwrap(),
        );
        assert_eq!(default.0.len(), 1);
        assert_eq!(default.0[0].0.as_str(), "Proguard");
        assert!(default.0[0].1.is_none());

        for whitespace in [
            '\u{0009}', '\u{000a}', '\u{000b}', '\u{000c}', '\u{000d}', '\u{0020}', '\u{0085}',
            '\u{00a0}', '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}',
            '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200a}', '\u{2028}',
            '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
        ] {
            let map = label_map(
                convert_label_occurrence(
                    option,
                    &format!("{whitespace}key=//p:t{whitespace}"),
                    OptionLabelContext::FirstRoundCanonical,
                )
                .unwrap(),
            );
            assert_eq!(map.0[0].0.as_str(), "key");
            assert_eq!(map.0[0].1.as_ref().unwrap().to_string(), "//p:t");

            let omitted = label_map(
                convert_label_occurrence(
                    option,
                    &format!("{whitespace},{whitespace}key{whitespace},{whitespace}"),
                    OptionLabelContext::FirstRoundCanonical,
                )
                .unwrap(),
            );
            assert_eq!(
                omitted
                    .0
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.is_none()))
                    .collect::<Vec<_>>(),
                [("key", true)]
            );
        }
        for non_whitespace in ['\u{180e}', '\u{200b}'] {
            let map = label_map(
                convert_label_occurrence(
                    option,
                    &format!("{non_whitespace}key{non_whitespace}"),
                    OptionLabelContext::FirstRoundCanonical,
                )
                .unwrap(),
            );
            assert_eq!(
                map.0[0].0.as_str(),
                format!("{non_whitespace}key{non_whitespace}")
            );
        }

        let map = label_map(
            convert_label_occurrence(
                option,
                ", bare ,, key= , =//root:target , extra=//p:t=tail ,",
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        assert_eq!(
            map.0
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_ref().map(ToString::to_string)))
                .collect::<Vec<_>>(),
            [
                ("bare", None),
                ("key", None),
                ("", Some("//root:target".to_owned())),
                ("extra", Some("//p:t=tail".to_owned())),
            ]
        );
        let no_key_trim = label_map(
            convert_label_occurrence(
                option,
                "  key =//p:t  ",
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        assert_eq!(no_key_trim.0[0].0.as_str(), "key ");
        assert_eq!(
            convert_label_occurrence(
                option,
                "key= //p:t",
                OptionLabelContext::FirstRoundCanonical,
            ),
            Err(LabelConvertError::Invalid)
        );
        for input in ["same=//p:t,same=//q:t", "same=//p:t,same=//bad:"] {
            assert_eq!(
                convert_label_occurrence(option, input, OptionLabelContext::FirstRoundCanonical),
                Err(LabelConvertError::Invalid),
                "{input}"
            );
        }

        let mapping = mapping();
        let first_round = label_map(
            convert_label_occurrence(
                option,
                "one=@alias//p:t",
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        assert_eq!(
            first_round.0[0].1.as_ref().unwrap().to_string(),
            "@@alias//p:t"
        );
        let mapped = label_map(
            convert_label_occurrence(
                option,
                "one=@alias//p:t",
                OptionLabelContext::MainRepository { mapping: &mapping },
            )
            .unwrap(),
        );
        assert_eq!(mapped.0[0].1.as_ref().unwrap().to_string(), "@@mapped//p:t");
        let base = PackageIdentifier::parse_bazel_package_identifier("@@owner//base").unwrap();
        let relative = label_map(
            convert_label_occurrence(
                option,
                "one=:relative",
                OptionLabelContext::Package {
                    base_package: &base,
                    mapping: &mapping,
                },
            )
            .unwrap(),
        );
        assert_eq!(
            relative.0[0].1.as_ref().unwrap().to_string(),
            "@@owner//base:relative"
        );
        let nonvisible = label_map(
            convert_label_occurrence(
                option,
                "one=@missing//p:t",
                OptionLabelContext::MainRepository { mapping: &mapping },
            )
            .unwrap(),
        );
        assert_eq!(
            nonvisible.0[0].1.as_ref().unwrap().to_string(),
            "@@[unknown repo 'missing' requested from @@]//p:t"
        );

        fn allocative<T: Allocative>() {}
        fn dupe<T: Dupe>() {}
        allocative::<LabelMapValues>();
        dupe::<LabelMapValues>();
        let copied = mapped.dupe();
        assert_eq!(mapped, copied);
        assert_eq!(mapped.0.as_ptr(), copied.0.as_ptr());
    }

    #[test]
    fn flag_alias_uses_ascii_validation_prefix_gate_and_contextual_label_parse() {
        let option = option("flag_alias");
        assert_eq!(classify_label(option), Some(LabelFamily::FlagAlias));
        assert!(option.allow_multiple);
        assert_eq!(option.raw_default, "\"null\"");
        assert_eq!(
            materialize_label_default(option, OptionLabelContext::FirstRoundCanonical),
            Ok(None)
        );
        let ascii = flag_alias(
            convert_label_occurrence(
                option,
                "A_z09=//p:t",
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        assert_eq!(ascii.alias.as_str(), "A_z09");
        assert_eq!(ascii.label.to_string(), "//p:t");
        for input in [
            "no_equals",
            "=//p:t",
            "alias=",
            "alias-=//p:t",
            "fóo=//p:t",
            "fóo=//p:t=tail",
            "alias=//p:t=tail",
            "alias=plain",
            "alias=//bad:",
        ] {
            assert_eq!(
                convert_label_occurrence(option, input, OptionLabelContext::FirstRoundCanonical),
                Err(LabelConvertError::Invalid),
                "{input}"
            );
        }
        for input in ["alias=no//p:t", "alias=no@alias//p:t"] {
            assert_eq!(
                convert_label_occurrence(option, input, OptionLabelContext::FirstRoundCanonical),
                Err(LabelConvertError::Invalid),
                "{input} passes the alias prefix gate but fails label parsing"
            );
        }

        let mapping = mapping();
        let first_round = flag_alias(
            convert_label_occurrence(
                option,
                "alias=@alias//p:t",
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        assert_eq!(first_round.label.to_string(), "@@alias//p:t");
        let mapped = flag_alias(
            convert_label_occurrence(
                option,
                "alias=@alias//p:t",
                OptionLabelContext::MainRepository { mapping: &mapping },
            )
            .unwrap(),
        );
        assert_eq!(mapped.label.to_string(), "@@mapped//p:t");
        let base = PackageIdentifier::parse_bazel_package_identifier("@@owner//base").unwrap();
        let package_context = flag_alias(
            convert_label_occurrence(
                option,
                "alias=@alias//p:t",
                OptionLabelContext::Package {
                    base_package: &base,
                    mapping: &mapping,
                },
            )
            .unwrap(),
        );
        assert_eq!(package_context.label.to_string(), "@@mapped//p:t");
        let nonvisible = flag_alias(
            convert_label_occurrence(
                option,
                "alias=@missing//p:t",
                OptionLabelContext::MainRepository { mapping: &mapping },
            )
            .unwrap(),
        );
        assert_eq!(
            nonvisible.label.to_string(),
            "@@[unknown repo 'missing' requested from @@]//p:t"
        );

        fn allocative<T: Allocative>() {}
        allocative::<FlagAliasEntry>();
    }

    #[test]
    fn mixed_converter_classifier_is_exactly_two_routes_without_changing_label_membership() {
        let routes = NATIVE_OPTION_DESCRIPTORS
            .iter()
            .filter_map(|option| {
                classify_mixed(option).map(|family| (option.canonical_name, family))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            routes,
            [
                (
                    "experimental_propagate_custom_flag",
                    MixedFamily::CustomFlag
                ),
                ("run_under", MixedFamily::RunUnder),
            ]
        );
        assert_eq!(
            NATIVE_OPTION_DESCRIPTORS
                .iter()
                .filter(|option| classify_label(option).is_some())
                .count(),
            39
        );
        for name in ["run_under", "experimental_propagate_custom_flag"] {
            assert_eq!(classify_label(option(name)), None, "{name}");
        }
    }

    #[test]
    fn run_under_tokenizes_before_deciding_label_and_retains_raw_original_and_suffix() {
        let option = option("run_under");
        assert_eq!(classify_mixed(option), Some(MixedFamily::RunUnder));
        assert!(!option.allow_multiple);
        assert_eq!(option.raw_default, "\"null\"");
        assert_eq!(
            materialize_mixed_default(option, OptionLabelContext::FirstRoundCanonical),
            Ok(None)
        );

        let explicit_null = run_under(
            convert_mixed_occurrence(option, "null", OptionLabelContext::FirstRoundCanonical)
                .unwrap(),
        );
        assert!(matches!(
            explicit_null,
            RunUnder::Command {
                ref original,
                ref suffix,
                ref command,
            } if original.as_str() == "null" && suffix.0.is_empty() && command.as_str() == "null"
        ));

        let command = run_under(
            convert_mixed_occurrence(
                option,
                " cmd\tone  two ",
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        let RunUnder::Command {
            original,
            suffix,
            command,
        } = command
        else {
            panic!("expected command run-under")
        };
        assert_eq!(original.as_str(), " cmd\tone  two ");
        assert_eq!(command.as_str(), "cmd");
        assert_eq!(
            suffix
                .0
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["one", "two"]
        );

        let quoted = run_under(
            convert_mixed_occurrence(
                option,
                r#"head' one'" two" '' """#,
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        let RunUnder::Command {
            command, suffix, ..
        } = quoted
        else {
            panic!("expected command run-under")
        };
        assert_eq!(command.as_str(), "head one two");
        assert_eq!(
            suffix
                .0
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["", ""]
        );

        let escapes = run_under(
            convert_mixed_occurrence(
                option,
                r#"cm\ d 'one\ two' "tri\q" "four\\five" "six\"seven" seven\\eight 'op"quote' "op'quote""#,
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        let RunUnder::Command {
            command, suffix, ..
        } = escapes
        else {
            panic!("expected command run-under")
        };
        assert_eq!(command.as_str(), "cm d");
        assert_eq!(
            suffix
                .0
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            [
                "one\\ two",
                "tri\\q",
                "four\\five",
                "six\"seven",
                "seven\\eight",
                "op\"quote",
                "op'quote",
            ]
        );

        for input in ["", " \t", r"cmd\", "\"cmd", "'cmd", r#""cmd\"#] {
            assert_eq!(
                convert_mixed_occurrence(option, input, OptionLabelContext::FirstRoundCanonical),
                Err(LabelConvertError::Invalid),
                "{input:?}"
            );
        }
        let newline = run_under(
            convert_mixed_occurrence(option, "one\ntwo", OptionLabelContext::FirstRoundCanonical)
                .unwrap(),
        );
        let unicode_space = run_under(
            convert_mixed_occurrence(
                option,
                "one\u{00a0}two",
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        assert!(
            matches!(newline, RunUnder::Command { command, suffix, .. } if command.as_str() == "one\ntwo" && suffix.0.is_empty())
        );
        assert!(
            matches!(unicode_space, RunUnder::Command { command, suffix, .. } if command.as_str() == "one\u{00a0}two" && suffix.0.is_empty())
        );

        let decoded_label = run_under(
            convert_mixed_occurrence(
                option,
                r"\//p:t suffix",
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        let RunUnder::Label {
            original,
            suffix,
            label,
        } = decoded_label
        else {
            panic!("expected decoded label run-under")
        };
        assert_eq!(original.as_str(), r"\//p:t suffix");
        assert_eq!(label.to_string(), "//p:t");
        assert_eq!(suffix.0[0].as_str(), "suffix");

        let quoted_label = run_under(
            convert_mixed_occurrence(
                option,
                r#""//pkg:t" suffix"#,
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        assert!(
            matches!(quoted_label, RunUnder::Label { label, suffix, .. } if label.to_string() == "//pkg:t" && suffix.0[0].as_str() == "suffix")
        );
        let suffix_only_label = run_under(
            convert_mixed_occurrence(option, "cmd //...", OptionLabelContext::FirstRoundCanonical)
                .unwrap(),
        );
        assert!(
            matches!(suffix_only_label, RunUnder::Command { command, suffix, .. } if command.as_str() == "cmd" && suffix.0[0].as_str() == "//...")
        );
        for (input, expected) in [(r"cmd\🦀", "cmd🦀"), (r#""cmd\🦀""#, "cmd\\🦀")] {
            assert!(matches!(
                run_under(
                    convert_mixed_occurrence(option, input, OptionLabelContext::FirstRoundCanonical)
                        .unwrap()
                ),
                RunUnder::Command { command, suffix, .. } if command.as_str() == expected && suffix.0.is_empty()
            ));
        }
    }

    #[test]
    fn run_under_label_contexts_and_retained_suffix_traits_are_exact() {
        let option = option("run_under");
        let mapping = mapping();
        let first = run_under(
            convert_mixed_occurrence(
                option,
                "@alias//p:t one",
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        let mapped = run_under(
            convert_mixed_occurrence(
                option,
                "@alias//p:t one",
                OptionLabelContext::MainRepository { mapping: &mapping },
            )
            .unwrap(),
        );
        let base = PackageIdentifier::parse_bazel_package_identifier("@@owner//base").unwrap();
        let package = run_under(
            convert_mixed_occurrence(
                option,
                "@alias//p:t one",
                OptionLabelContext::Package {
                    base_package: &base,
                    mapping: &mapping,
                },
            )
            .unwrap(),
        );
        let package_root = run_under(
            convert_mixed_occurrence(
                option,
                "//pkg:t",
                OptionLabelContext::Package {
                    base_package: &base,
                    mapping: &mapping,
                },
            )
            .unwrap(),
        );
        let nonvisible = run_under(
            convert_mixed_occurrence(
                option,
                "@missing//p:t",
                OptionLabelContext::MainRepository { mapping: &mapping },
            )
            .unwrap(),
        );
        assert!(
            matches!(first, RunUnder::Label { label, .. } if label.to_string() == "@@alias//p:t")
        );
        assert!(
            matches!(mapped, RunUnder::Label { label, .. } if label.to_string() == "@@mapped//p:t")
        );
        assert!(
            matches!(package, RunUnder::Label { label, .. } if label.to_string() == "@@mapped//p:t")
        );
        assert!(
            matches!(package_root, RunUnder::Label { label, .. } if label.to_string() == "@@owner//pkg:t")
        );
        assert!(
            matches!(nonvisible, RunUnder::Label { label, .. } if label.to_string() == "@@[unknown repo 'missing' requested from @@]//p:t")
        );
        assert!(matches!(
            run_under(
                convert_mixed_occurrence(option, ":relative", OptionLabelContext::FirstRoundCanonical)
                    .unwrap()
            ),
            RunUnder::Command { command, .. } if command.as_str() == ":relative"
        ));

        fn allocative<T: Allocative>() {}
        fn dupe<T: Dupe>() {}
        allocative::<RunUnderSuffix>();
        allocative::<RunUnder>();
        dupe::<RunUnderSuffix>();
        let suffix = RunUnderSuffix(Arc::from([CompactString::new("arg")]));
        let copied = suffix.dupe();
        assert_eq!(suffix, copied);
        assert_eq!(suffix.0.as_ptr(), copied.0.as_ptr());
    }

    #[test]
    fn custom_flag_keeps_raw_defines_and_canonicalizes_labels_and_subpackages() {
        let option = option("experimental_propagate_custom_flag");
        assert_eq!(classify_mixed(option), Some(MixedFamily::CustomFlag));
        assert!(option.allow_multiple);
        assert_eq!(option.raw_default, "\"null\"");
        assert_eq!(
            materialize_mixed_default(option, OptionLabelContext::FirstRoundCanonical),
            Ok(None)
        );
        for raw in ["", ":relative", "null", "define=value", "/not/a/label"] {
            assert_eq!(
                custom_flag(
                    convert_mixed_occurrence(option, raw, OptionLabelContext::FirstRoundCanonical)
                        .unwrap()
                ),
                raw,
                "{raw}"
            );
        }
        assert_eq!(
            custom_flag(
                convert_mixed_occurrence(
                    option,
                    "//pkg:target",
                    OptionLabelContext::FirstRoundCanonical,
                )
                .unwrap()
            ),
            "@@//pkg:target"
        );

        let mapping = mapping();
        for input in ["@alias//pkg/...", "@alias//pkg:__subpackages__"] {
            assert_eq!(
                custom_flag(
                    convert_mixed_occurrence(
                        option,
                        input,
                        OptionLabelContext::MainRepository { mapping: &mapping },
                    )
                    .unwrap()
                ),
                "@@mapped//pkg/...",
                "{input}"
            );
        }
        for input in ["//pkg/...", "//pkg:__subpackages__"] {
            assert_eq!(
                custom_flag(
                    convert_mixed_occurrence(
                        option,
                        input,
                        OptionLabelContext::MainRepository { mapping: &mapping },
                    )
                    .unwrap()
                ),
                "@@//pkg/...",
                "{input}"
            );
        }
        let base = PackageIdentifier::parse_bazel_package_identifier("@@owner//base").unwrap();
        for input in ["//pkg/...", "//pkg:__subpackages__"] {
            assert_eq!(
                custom_flag(
                    convert_mixed_occurrence(
                        option,
                        input,
                        OptionLabelContext::Package {
                            base_package: &base,
                            mapping: &mapping,
                        },
                    )
                    .unwrap()
                ),
                "@@owner//pkg/...",
                "{input}"
            );
        }
        assert_eq!(
            custom_flag(
                convert_mixed_occurrence(
                    option,
                    "@missing//pkg/...",
                    OptionLabelContext::MainRepository { mapping: &mapping },
                )
                .unwrap()
            ),
            "@@[unknown repo 'missing' requested from @@]//pkg/..."
        );
        for input in ["//...", "@alias//..."] {
            assert_eq!(
                convert_mixed_occurrence(
                    option,
                    input,
                    OptionLabelContext::MainRepository { mapping: &mapping }
                ),
                Err(LabelConvertError::Invalid),
                "{input}"
            );
        }
        assert_eq!(
            custom_flag(
                convert_mixed_occurrence(
                    option,
                    "@alias",
                    OptionLabelContext::MainRepository { mapping: &mapping },
                )
                .unwrap()
            ),
            "@@mapped//:alias"
        );

        let source = include_str!("label_convert.rs");
        for excluded in [
            "trimForNonTestConfiguration",
            "RunfilesSupport",
            "DICE",
            "checksum",
            "renderer",
            "ShellUtils",
        ] {
            assert!(
                !source.contains(excluded),
                "deferred source surface: {excluded}"
            );
        }
    }

    #[test]
    fn libc_top_rewrites_only_absolute_package_spellings_and_retained_traits_are_exact() {
        let libc = option("grte_top");
        assert_eq!(
            convert_label_occurrence(libc, "default", OptionLabelContext::FirstRoundCanonical),
            Ok(None)
        );
        assert_eq!(
            scalar(
                convert_label_occurrence(
                    libc,
                    "//toolchain",
                    OptionLabelContext::FirstRoundCanonical
                )
                .unwrap()
            ),
            "//toolchain:everything"
        );
        assert_eq!(
            scalar(
                convert_label_occurrence(
                    libc,
                    "//toolchain:old",
                    OptionLabelContext::FirstRoundCanonical
                )
                .unwrap()
            ),
            "//toolchain:everything"
        );
        assert_eq!(
            convert_label_occurrence(libc, "toolchain", OptionLabelContext::FirstRoundCanonical),
            Err(LabelConvertError::Invalid)
        );
        assert_eq!(
            scalar(
                convert_label_occurrence(libc, "//", OptionLabelContext::FirstRoundCanonical)
                    .unwrap()
            ),
            "//:everything"
        );
        assert_eq!(
            convert_label_occurrence(
                option("fdo_profile"),
                "",
                OptionLabelContext::FirstRoundCanonical,
            ),
            Ok(None)
        );
        assert_eq!(
            scalar(
                convert_label_occurrence(
                    option("fdo_profile"),
                    "//profiles:sample",
                    OptionLabelContext::FirstRoundCanonical,
                )
                .unwrap()
            ),
            "//profiles:sample"
        );

        fn allocative<T: Allocative>() {}
        fn dupe<T: Dupe>() {}
        allocative::<LabelValue>();
        allocative::<LabelValues>();
        dupe::<LabelValues>();
        let values = labels(
            convert_label_occurrence(
                option("platforms"),
                "//a:x",
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap(),
        );
        let copied = values.dupe();
        assert_eq!(values, copied);
        assert_eq!(values.0.as_ptr(), copied.0.as_ptr());
        assert!(values <= copied);
    }
}
