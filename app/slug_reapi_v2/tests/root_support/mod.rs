#![allow(dead_code)]
use std::sync::Arc;

use slug_build_api_v2::DerivedArtifactRoot;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::SlugConfigurationKind;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;

pub fn root() -> DerivedArtifactRoot {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    DerivedArtifactRoot::bin(
        SlugConfiguration::new_default(SlugConfigurationKind::Target, &host).unwrap(),
    )
    .unwrap()
}

pub fn owner_root(owner: &slug_analysis_v2::ConfiguredTargetKey) -> DerivedArtifactRoot {
    DerivedArtifactRoot::bin(
        owner
            .configuration()
            .slug_configuration()
            .expect("structural fixture configuration")
            .clone(),
    )
    .unwrap()
}

pub fn path(relative: &str) -> String {
    format!("{}/{relative}", root().path())
}
pub fn owner_path(owner: &slug_analysis_v2::ConfiguredTargetKey, relative: &str) -> String {
    format!("{}/{relative}", owner_root(owner).path())
}
