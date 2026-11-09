//! Actual original evaluation must retain an unchanged complete certificate.
//! This is pure DICE metadata ownership, without native roots or IO receipts.
use super::*;

#[tokio::test]
async fn observed_literal_file_original_reuses_owner_certificate_map() {
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut transaction = transaction_untracked(&dice, MODULE, EXTENSION, true).await;
    let owner = owner(&mut transaction).await;
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    assert_eq!(
        transaction
            .compute(&RepositoryWorkspaceExecutionPolicyKey::new(
                workspace.dupe()
            ))
            .await
            .unwrap(),
        RepositoryWorkspaceExecutionPolicy::Disabled
    );
    let certificate = transaction
        .compute(&HostSelectedExtensionOwnerCertificateObservationKey::new(
            workspace.dupe(),
            owner.clone(),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(certificate)) = certificate else {
        panic!("actual original owner certificate must complete")
    };
    let selected = certificate.result().as_ref().as_ref().unwrap();
    assert!(selected.repository(0).is_some());

    // Disabled uses the actual original evaluator to produce its literal File
    // plan; no native effects or synthetic completion records are supplied.
    let observed = transaction
        .compute(&HostSelectedRepositoryFileEffectObservationKey::new(
            workspace, owner, 0,
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(observed)) = observed else {
        panic!("literal File original evaluation must complete")
    };
    let effect = observed.result().as_ref().as_ref().unwrap();
    assert_eq!(effect.plan().effects().len(), 2);
    assert!(effect.workspace_realization().is_none());
    assert_eq!(effect.certificate.as_ref(), selected);
    assert!(!certificate.observations().observations().is_empty());

    // The rule definition is in the same ext.bzl already authenticated by the
    // owner. Loading it again adds no fact, and must retain every original
    // result Arc before the stronger complete-map ownership assertion below.
    assert_eq!(observed.observations(), certificate.observations());
    for (demand, result) in certificate.observations().observations() {
        assert!(Arc::ptr_eq(
            result,
            observed.observations().get(demand).unwrap()
        ));
    }
    assert!(
        std::ptr::eq(
            observed.observations().observations(),
            certificate.observations().observations(),
        ),
        "a contained definition certificate must reuse the original complete map owner"
    );
}
