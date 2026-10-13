//! CPU comes from the tracked actual target platform and native setting.
use super::*;

fn platform(constraints: &[(&str, &str)]) -> ConfiguredPlatform {
    let key = |label: &str| {
        ConfiguredTargetKey::new(
            CanonicalLabel::parse(label).unwrap(),
            ConfigurationKey::target("cpu-projection").unwrap(),
        )
    };
    ConfiguredPlatform::new(
        key("@@//platform:requested_alias"),
        key("@@//platform:actual_target"),
        PlatformSemanticFact {
            exec_properties: Arc::from([]),
            missing_toolchain_error: None,
        },
        constraints
            .iter()
            .map(|(setting, value)| {
                ConfiguredActionPlatformConstraint::new(key(value), key(setting))
            })
            .collect(),
    )
}

#[test]
fn actual_native_cpu_value_is_projected_without_value_repository_recipe() {
    assert_eq!(
        target_platform_cpu(&platform(&[
            ("@@platforms//os:os", "@@platforms//os:linux"),
            (
                "@@platforms//cpu:cpu",
                "@@independent//cpu:unrelated_processor"
            ),
        ])),
        "unrelated_processor"
    );
    assert_eq!(
        target_platform_cpu(&platform(&[(
            "@@platforms//cpu:cpu",
            "@@platforms//cpu:aarch64"
        ),])),
        "aarch64"
    );
}

#[test]
fn absent_native_cpu_does_not_remap_or_guess_host_cpu() {
    for setting in [
        "@@platforms+//cpu:cpu",
        "@@other//cpu:cpu",
        "@@platforms//other:cpu",
        "@@platforms//cpu:other",
    ] {
        assert_eq!(
            target_platform_cpu(&platform(&[(setting, "@@platforms//cpu:x86_64")])),
            ""
        );
    }
    assert_eq!(target_platform_cpu(&platform(&[])), "");
}
