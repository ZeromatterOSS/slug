use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::NativeCommandOption;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;

use super::*;
use crate::AnalysisValue;

fn projection(options: &[(NativeCommandOption, &str)]) -> CppFragmentProjection {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    let base = SlugConfiguration::default_target(&host).unwrap();
    let overlay = CommandConfigurationOverlay::from(
        options
            .iter()
            .map(|(option, value)| {
                CommandConfigurationOccurrence::native(*option, Some(*value), false)
            })
            .collect::<Vec<_>>(),
    );
    let configuration = base
        .with_command_configuration(base.starlark_options().clone(), &overlay)
        .unwrap();
    CppFragmentProjection::new(configuration).unwrap()
}

fn value(projection: CppFragmentProjection) -> AnalysisValue {
    AnalysisValue::cpp_fragment(RetainedCppFragment::new(projection))
}

#[test]
fn cpp_publication_checks_full_configuration_not_only_consumed_projections() {
    let baseline = projection(&[]);
    let first = value(baseline.clone());
    assert!(first.publication_eq(&value(baseline.clone())));
    for (option, text) in [
        (NativeCommandOption::ForcePic, "true"),
        (NativeCommandOption::Linkopt, "--target"),
        (NativeCommandOption::HostLinkopt, "--unused-host"),
        (NativeCommandOption::CompilationMode, "opt"),
        (NativeCommandOption::Features, "unconsumed-feature"),
    ] {
        let changed = projection(&[(option, text)]);
        if matches!(
            option,
            NativeCommandOption::HostLinkopt | NativeCommandOption::Features
        ) {
            assert!(baseline.linkopts().unwrap().eq(changed.linkopts().unwrap()));
            assert_eq!(baseline.force_pic().unwrap(), changed.force_pic().unwrap());
        }
        assert!(!first.publication_eq(&value(changed)), "{option:?}");
    }
}

#[test]
fn cpp_occurrence_aliases_survive_cloning_but_fresh_views_remain_distinct() {
    let projected = projection(&[]);
    let left = RetainedCppFragment::new(projected.clone());
    let right = RetainedCppFragment::new(projected.clone());
    assert_eq!(left, left.clone());
    assert_ne!(left, right);
    assert_eq!(left.projection(), &projected);
    let a = AnalysisValue::cpp_fragment(left);
    let b = AnalysisValue::cpp_fragment(right);
    assert!(a.is_starlark_hashable());
    assert_eq!(
        a.starlark_hash().unwrap(),
        a.clone().starlark_hash().unwrap()
    );
    assert!(a.publication_eq(&b));
    let repeated = AnalysisValue::tuple(vec![a.clone(), a]);
    let same_topology = AnalysisValue::tuple(vec![b.clone(), b.clone()]);
    let split = AnalysisValue::tuple(vec![b, value(projected)]);
    assert!(repeated.publication_eq(&same_topology));
    assert!(!repeated.publication_eq(&split));
    assert!(!split.publication_eq(&repeated));
}
