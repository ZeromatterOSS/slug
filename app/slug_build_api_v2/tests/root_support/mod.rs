//! Explicit structural configurations for low-level output fixtures.
#![allow(dead_code)]
use std::sync::Arc;

use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::StarlarkOption;
use slug_configuration_v2::StarlarkOptionScope;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_identity_v2::CanonicalLabel;

use super::DerivedArtifactRoot;

pub fn root() -> DerivedArtifactRoot {
    root_for(b"fixture")
}

pub fn root_for(identity: &[u8]) -> DerivedArtifactRoot {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    let configuration = SlugConfiguration::default_target(&host)
        .unwrap()
        .with_starlark_option(StarlarkOption::string(
            CanonicalLabel::parse("@@//fixture:root_identity").unwrap(),
            format!("{identity:?}"),
            StarlarkOptionScope::Universal,
        ));
    DerivedArtifactRoot::bin(configuration).unwrap()
}

pub fn exec_path(relative: &str) -> String {
    format!("{}/{relative}", root().path())
}
