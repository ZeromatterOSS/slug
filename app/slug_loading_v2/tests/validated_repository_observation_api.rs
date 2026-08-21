use std::sync::Arc;

use dice::Key;
use slug_loading_v2::HostValidatedGeneratedRepositorySpecs;
use slug_loading_v2::HostValidatedGeneratedRepositorySpecsError;
use slug_loading_v2::HostValidatedModuleExtensionRepositoriesObservationError;
use slug_loading_v2::HostValidatedModuleExtensionRepositoriesObservationKey;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_loading_v2::ObservedHostValidatedGeneratedRepositorySpecs;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;

#[test]
fn validated_repository_observation_surface_is_cross_crate_usable() {
    let key = HostValidatedModuleExtensionRepositoriesObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    assert_eq!(
        key.to_string(),
        "observed-host-validated-module-extension-repositories:\"/workspace\""
    );

    fn inspect_value(_: &<HostValidatedModuleExtensionRepositoriesObservationKey as Key>::Value) {}
    fn inspect_carrier(observed: &ObservedHostValidatedGeneratedRepositorySpecs) {
        let _: &Arc<
            Result<
                HostValidatedGeneratedRepositorySpecs,
                HostValidatedGeneratedRepositorySpecsError,
            >,
        > = observed.result();
        let _: &PathObservationEpoch = observed.observations();
    }

    let _ = inspect_value
        as fn(
            &LoadingPreparationOutcome<
                Result<
                    ObservedHostValidatedGeneratedRepositorySpecs,
                    HostValidatedModuleExtensionRepositoriesObservationError,
                >,
            >,
        );
    let _ = inspect_carrier as fn(&ObservedHostValidatedGeneratedRepositorySpecs);
}
