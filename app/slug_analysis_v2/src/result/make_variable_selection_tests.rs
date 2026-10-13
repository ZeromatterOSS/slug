//! Selected provider content participates in existing context publication.
use slug_build_api_v2::ProviderIdentity;
use slug_configuration_v2::HostPathFlavor;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;

use super::*;
use crate::key::ConfigurationKey;

fn configuration() -> ConfigurationKey {
    let inputs = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap()
    .with_action_environment_host(
        slug_configuration_v2::native::host::ActionEnvironmentHost::without_environment(
            slug_configuration_v2::native::host::ActionEnvironmentHostOs::Linux,
        ),
    );
    ConfigurationKey::from_slug(
        SlugConfiguration::default_target(&inputs)
            .unwrap()
            .to_exec()
            .unwrap(),
    )
}

fn context(values: Option<&[(&str, &str)]>) -> ConfiguredActionToolchainContext {
    let configuration = configuration();
    let key = |label: &str| {
        ConfiguredTargetKey::new(CanonicalLabel::parse(label).unwrap(), configuration.clone())
    };
    let platform = key("@@//platform:exec");
    let implementation = key("@@//impl:compiler")
        .with_toolchain_execution_platform(Arc::new(platform.label().clone()));
    let info = values.map(|values| {
        TemplateVariableInfo::new(
            values
                .iter()
                .map(|(key, value)| (CompactString::new(key), CompactString::new(value)))
                .collect(),
        )
    });
    let selection = ConfiguredToolchainSelection::new(
        CanonicalLabel::parse("@@//declaration:compiler").unwrap(),
        implementation.clone(),
        implementation,
        ProviderOccurrence::empty(ProviderIdentity::builtin("ToolchainInfo")),
    )
    .with_template_variable_info(info);
    ConfiguredActionToolchainContext::new(
        platform,
        vec![ConfiguredToolchainContextRow::new(
            key("@@//type:compiler"),
            key("@@//type:compiler"),
            true,
            Some(selection),
        )],
    )
    .unwrap()
}

#[test]
fn equal_fresh_template_occurrences_keep_context_equality() {
    let left = context(Some(&[("Z", "last"), ("λ", "雪")]));
    let right = context(Some(&[("Z", "last"), ("λ", "雪")]));
    let left_info = left.rows()[0]
        .selected()
        .unwrap()
        .template_variable_info()
        .unwrap();
    let right_info = right.rows()[0]
        .selected()
        .unwrap()
        .template_variable_info()
        .unwrap();
    assert_ne!(left_info, right_info);
    assert_eq!(left, right);
}

#[test]
fn provider_only_changes_and_restoration_change_context_equality() {
    let original = context(Some(&[("CC", "first"), ("MODE", "dbg")]));
    let changed = context(Some(&[("CC", "second"), ("MODE", "dbg")]));
    let restored = context(Some(&[("CC", "first"), ("MODE", "dbg")]));
    assert_ne!(original, changed);
    assert_ne!(changed, restored);
    assert_eq!(original, restored);
    assert_ne!(original, context(Some(&[("MODE", "dbg"), ("CC", "first")])));
    assert_ne!(context(None), context(Some(&[])));
}
