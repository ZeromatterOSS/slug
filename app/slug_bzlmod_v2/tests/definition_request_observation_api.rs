use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequestsObservationError;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequestsObservationKey;
use slug_bzlmod_v2::ObservedHostSelectedExtensionDefinitionLoadRequests;
use slug_workspace_v2::NormalizedAbsolutePath;

#[test]
fn definition_request_observation_surface_is_cross_crate_usable() {
    let key = HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    assert_eq!(
        key.to_string(),
        "observed-host-selected-extension-definition-load-requests:\"/workspace\""
    );

    fn inspect(
        observed: &ObservedHostSelectedExtensionDefinitionLoadRequests,
        _error: &HostSelectedExtensionDefinitionLoadRequestsObservationError,
    ) {
        let _ = observed.result();
        let _ = observed.observations();
    }

    let _ = inspect
        as fn(
            &ObservedHostSelectedExtensionDefinitionLoadRequests,
            &HostSelectedExtensionDefinitionLoadRequestsObservationError,
        );
}
