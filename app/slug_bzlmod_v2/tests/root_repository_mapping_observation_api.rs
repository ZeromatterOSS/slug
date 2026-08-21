use std::sync::Arc;

use slug_bzlmod_v2::HostRootRepositoryMapping;
use slug_bzlmod_v2::HostRootRepositoryMappingError;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationError;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
use slug_bzlmod_v2::ObservedHostRootRepositoryMapping;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;

#[test]
fn root_repository_mapping_observation_surface_is_cross_crate_usable() {
    let key = HostRootRepositoryMappingObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    assert_eq!(
        key.to_string(),
        "observed-host-root-repository-mapping:\"/workspace\""
    );

    fn inspect_value(
        _: &SourcePreparationOutcome<
            Result<ObservedHostRootRepositoryMapping, HostRootRepositoryMappingObservationError>,
        >,
    ) {
    }
    let _ = inspect_value as fn(&<HostRootRepositoryMappingObservationKey as dice::Key>::Value);

    fn inspect_carrier(observed: &ObservedHostRootRepositoryMapping) {
        let _: &Arc<Result<HostRootRepositoryMapping, HostRootRepositoryMappingError>> =
            observed.result();
        let _: &PathObservationEpoch = observed.observations();
    }
    let _ = inspect_carrier as fn(&ObservedHostRootRepositoryMapping);
}
