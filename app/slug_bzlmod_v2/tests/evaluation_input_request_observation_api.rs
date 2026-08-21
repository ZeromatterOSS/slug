use slug_bzlmod_v2::HostSelectedExtensionEvaluationInputRequestsObservationError;
use slug_bzlmod_v2::HostSelectedExtensionEvaluationInputRequestsObservationKey;
use slug_bzlmod_v2::ObservedHostSelectedExtensionEvaluationInputRequests;
use slug_workspace_v2::NormalizedAbsolutePath;

#[test]
fn evaluation_input_request_observation_surface_is_cross_crate_usable() {
    let key = HostSelectedExtensionEvaluationInputRequestsObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    assert_eq!(
        key.to_string(),
        "observed-host-selected-extension-evaluation-inputs:\"/workspace\""
    );

    fn inspect(
        observed: &ObservedHostSelectedExtensionEvaluationInputRequests,
        _error: &HostSelectedExtensionEvaluationInputRequestsObservationError,
    ) {
        let _ = observed.result();
        let _ = observed.observations();
    }

    let _ = inspect
        as fn(
            &ObservedHostSelectedExtensionEvaluationInputRequests,
            &HostSelectedExtensionEvaluationInputRequestsObservationError,
        );
}
