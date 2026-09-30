//! Bazel 9.2 CppConfiguration.java:754-769 and builtin_exec_platforms.bzl's
//! CppOptions mapping: grte_top is projected from retained host_grte_top.

use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

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

fn command(configuration: &SlugConfiguration, spelling: &str) -> SlugConfiguration {
    configuration
        .with_command_configuration(
            configuration.starlark_options().clone(),
            &CommandConfigurationOverlay::from(vec![CommandConfigurationOccurrence::native(
                NativeCommandOption::GrteTop,
                Some(spelling),
                false,
            )]),
        )
        .unwrap()
}

fn retained(
    configuration: &SlugConfiguration,
    option: &str,
    value: OptionValue,
) -> SlugConfiguration {
    let mut options = configuration.0.options.to_vec();
    options
        .iter_mut()
        .find(|record| record.class_name == CPP_OPTIONS && record.canonical_name == option)
        .unwrap()
        .value = value;
    finish_configuration(
        configuration.0.kind,
        options.into(),
        configuration.0.starlark_options.clone(),
        configuration.0.action_environment_host.clone(),
    )
}

fn projected(configuration: &SlugConfiguration) -> Option<CanonicalLabel> {
    let label = CppFragmentProjection::new(configuration.clone())
        .unwrap()
        .grte_top()
        .unwrap();
    let field = ConfigurationFieldIdentity::new(
        ConfigurationField::cpp(CppConfigurationField::LibcTop),
        CanonicalRepoName::new("bazel_tools").unwrap(),
    );
    assert_eq!(
        configuration.configuration_field_label(&field).unwrap(),
        label
    );
    label
}

#[test]
fn grte_top_projects_converted_labels_defaults_and_both_exec_configurations() {
    let original = base();
    assert_eq!(projected(&original), None);
    // LibcTopLabelConverter replaces the target of an absolute package label.
    let target = command(&original, "//libc:discarded");
    assert_eq!(
        projected(&target),
        Some(CanonicalLabel::parse("@@//libc:everything").unwrap())
    );
    let restored = command(&target, "default");
    assert_eq!(projected(&restored), None);
    assert_eq!(restored, original);
    assert_eq!(restored.canonical_bytes(), original.canonical_bytes());
    assert_ne!(target, original);
    assert_ne!(
        CppFragmentProjection::new(target.clone()).unwrap(),
        CppFragmentProjection::new(original.clone()).unwrap()
    );
    for spelling in ["libc", ":libc", "@libc//libc", "@@libc+1.0//libc"] {
        assert!(
            original
                .with_command_configuration(
                    original.starlark_options().clone(),
                    &CommandConfigurationOverlay::from(vec![
                        CommandConfigurationOccurrence::native(
                            NativeCommandOption::GrteTop,
                            Some(spelling),
                            false,
                        )
                    ]),
                )
                .is_err(),
            "LibcTopLabelConverter must reject {spelling}"
        );
    }

    let platform = CanonicalLabel::parse("@@//platforms:exec").unwrap();
    for exec in [
        target.to_exec().unwrap(),
        target.to_exec_for_platform(&platform).unwrap(),
    ] {
        assert_eq!(
            projected(&exec),
            None,
            "the default host libc replaces the target libc"
        );
    }

    // External command spellings remain unsupported by LibcTopLabelConverter.
    // A visible canonical label already retained by the owner is preserved.
    let mut mapping = RepositoryMapping::new(RepositoryMappingId::new("libc-projection").unwrap());
    mapping.insert(
        ApparentRepoName::new("libc").unwrap(),
        CanonicalRepoName::new("libc+1.0").unwrap(),
    );
    let host_label = OptionLabelContext::MainRepository { mapping: &mapping }
        .parse("@libc//host:everything")
        .unwrap();
    let mapped_target = retained(
        &target,
        "grte_top",
        OptionValue::Label(Some(LabelValue::Label(host_label.clone()))),
    );
    assert_eq!(projected(&mapped_target), host_label.canonical());
    let host_override = retained(
        &target,
        "host_grte_top",
        OptionValue::Label(Some(LabelValue::Label(host_label.clone()))),
    );
    assert_eq!(projected(&host_override), projected(&target));
    assert_ne!(host_override, target);
    for exec in [
        host_override.to_exec().unwrap(),
        host_override.to_exec_for_platform(&platform).unwrap(),
    ] {
        assert_eq!(
            projected(&exec),
            host_label.canonical(),
            "both exec paths use the typed host libc label"
        );
    }
    assert_eq!(
        projected(&target),
        Some(CanonicalLabel::parse("@@//libc:everything").unwrap())
    );
}

#[test]
fn grte_top_rejects_malformed_and_nonvisible_retained_state() {
    let base = base();
    let platform = CanonicalLabel::parse("@@//platforms:exec").unwrap();
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("missing-libc").unwrap());
    let invisible = OptionLabelContext::MainRepository { mapping: &mapping }
        .parse("@missing//libc:everything")
        .unwrap();
    assert!(invisible.canonical().is_none());
    for value in [
        OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text(
            "//libc:everything".into(),
        ))),
        OptionValue::Label(Some(LabelValue::Labels(LabelValues(Arc::from([]))))),
        OptionValue::Label(Some(LabelValue::Label(invisible))),
    ] {
        let malformed = retained(&base, "grte_top", value.clone());
        assert!(CppFragmentProjection::new(malformed.clone()).is_err());
        let field = ConfigurationFieldIdentity::new(
            ConfigurationField::cpp(CppConfigurationField::LibcTop),
            CanonicalRepoName::new("bazel_tools").unwrap(),
        );
        assert!(malformed.configuration_field_label(&field).is_err());
        let malformed_host = retained(&base, "host_grte_top", value);
        for exec in [
            malformed_host.to_exec().unwrap(),
            malformed_host.to_exec_for_platform(&platform).unwrap(),
        ] {
            assert!(
                CppFragmentProjection::new(exec.clone()).is_err(),
                "a malformed host libc must not become a nullable default"
            );
            assert!(exec.configuration_field_label(&field).is_err());
        }
    }
}
