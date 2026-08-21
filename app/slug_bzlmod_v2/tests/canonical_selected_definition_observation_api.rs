use std::sync::Arc;

use dice::Key;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinition;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionError;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionObservationError;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionObservationKey;
use slug_bzlmod_v2::ObservedHostCanonicalSelectedModuleDefinition;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;

#[test]
fn canonical_selected_definition_observation_surface_is_cross_crate_usable() {
    let key = HostCanonicalSelectedModuleDefinitionObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        CanonicalRepoName::new("dep+").unwrap(),
    );
    assert_eq!(
        key.to_string(),
        "observed-host-canonical-selected-module-definition:\"/workspace\":@@dep+"
    );

    fn inspect_value(
        _: &SourcePreparationOutcome<
            Result<
                ObservedHostCanonicalSelectedModuleDefinition,
                HostCanonicalSelectedModuleDefinitionObservationError,
            >,
        >,
    ) {
    }
    let _ =
        inspect_value as fn(&<HostCanonicalSelectedModuleDefinitionObservationKey as Key>::Value);

    fn inspect_carrier(observed: &ObservedHostCanonicalSelectedModuleDefinition) {
        let _: &Arc<
            Result<
                HostCanonicalSelectedModuleDefinition,
                HostCanonicalSelectedModuleDefinitionError,
            >,
        > = observed.result();
        let _: &PathObservationEpoch = observed.observations();
    }
    let _ = inspect_carrier as fn(&ObservedHostCanonicalSelectedModuleDefinition);
}
