use slug_configuration_v2::native::host::ActionEnvironmentHost;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::result::ConfiguredActionAspectProvenance;
use crate::result::ConfiguredActionPlatformConstraint;
use crate::result::PlatformSemanticFact;

fn configuration(host: Option<ActionEnvironmentHostOs>) -> SlugConfiguration {
    let mut inputs = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    if let Some(host) = host {
        inputs =
            inputs.with_action_environment_host(ActionEnvironmentHost::without_environment(host));
    }
    SlugConfiguration::default_target(&inputs).unwrap()
}

fn projection(
    host: Option<ActionEnvironmentHostOs>,
    constraints: &[(&str, &str)],
) -> CcContextProjection {
    let configuration = configuration(host);
    let owner = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:owner").unwrap(),
        crate::ConfigurationKey::from_slug(configuration.clone()),
    );
    // The selected platform's fixture needs a constructible exec configuration,
    // independently of the intentionally missing Host on the tested owner.
    let exec = crate::ConfigurationKey::from_slug(
        self::configuration(Some(ActionEnvironmentHostOs::Linux))
            .to_exec()
            .unwrap(),
    );
    let constraints = constraints
        .iter()
        .map(|(setting, value)| {
            ConfiguredActionPlatformConstraint::new(
                ConfiguredTargetKey::new(CanonicalLabel::parse(value).unwrap(), exec.clone()),
                ConfiguredTargetKey::new(CanonicalLabel::parse(setting).unwrap(), exec.clone()),
            )
        })
        .collect();
    CcContextProjection {
        configuration: Some(configuration),
        features: None,
        default_execution: Arc::new(
            ConfiguredActionOwnerContext::new(
                owner,
                ConfiguredExecGroup::Default,
                ConfiguredTargetKey::new(CanonicalLabel::parse("@@//:remote").unwrap(), exec),
                PlatformSemanticFact {
                    exec_properties: Arc::from([]),
                    missing_toolchain_error: None,
                },
                &Default::default(),
                &Default::default(),
                constraints,
                None,
                ConfiguredActionAspectProvenance::Absent,
            )
            .unwrap(),
        ),
    }
}

#[test]
fn execution_os_matches_canonical_constraints_and_distinguishes_absence() {
    for (value, expected) in [
        ("linux", "LINUX"),
        ("osx", "DARWIN"),
        ("macos", "DARWIN"),
        ("windows", "WINDOWS"),
        ("freebsd", "FREEBSD"),
        ("openbsd", "OPENBSD"),
        ("none", "UNKNOWN"),
        ("mystery", "UNKNOWN"),
    ] {
        let label = format!("@@platforms//os:{value}");
        let ctx = projection(None, &[("@@platforms//os:os", &label)]);
        assert_eq!(ctx.execution_os().unwrap(), expected);
    }
    // A lookalike value attached to the real setting is UNKNOWN, never fallback.
    assert_eq!(
        projection(
            Some(ActionEnvironmentHostOs::Linux),
            &[("@@platforms//os:os", "@@other//os:linux")]
        )
        .execution_os()
        .unwrap(),
        "UNKNOWN"
    );
    // A lookalike setting leaves the standard setting absent, hence Host fallback.
    assert_eq!(
        projection(
            Some(ActionEnvironmentHostOs::Openbsd),
            &[("@@platforms+//os:os", "@@platforms+//os:linux")]
        )
        .execution_os()
        .unwrap(),
        "OPENBSD"
    );
    for (host, expected) in [
        (ActionEnvironmentHostOs::Linux, "LINUX"),
        (ActionEnvironmentHostOs::Windows, "WINDOWS"),
        (ActionEnvironmentHostOs::Macos, "DARWIN"),
        (ActionEnvironmentHostOs::Freebsd, "FREEBSD"),
        (ActionEnvironmentHostOs::Openbsd, "OPENBSD"),
        (ActionEnvironmentHostOs::Unknown, "UNKNOWN"),
    ] {
        assert_eq!(
            projection(Some(host), &[]).execution_os().unwrap(),
            expected
        );
    }
}

#[test]
fn execution_os_fails_closed_without_required_structural_owners() {
    assert!(
        projection(None, &[])
            .execution_os()
            .unwrap_err()
            .to_string()
            .contains("structural Host OS")
    );
    let mut ctx = projection(Some(ActionEnvironmentHostOs::Linux), &[]);
    ctx.configuration = None;
    assert!(
        ctx.execution_os()
            .unwrap_err()
            .to_string()
            .contains("structural configuration")
    );
    ctx.default_execution = Arc::new(
        ConfiguredActionOwnerContext::unresolved_default(ctx.default_execution.owner().clone())
            .unwrap(),
    );
    assert!(
        ctx.execution_os()
            .unwrap_err()
            .to_string()
            .contains("resolved default execution platform")
    );
}
