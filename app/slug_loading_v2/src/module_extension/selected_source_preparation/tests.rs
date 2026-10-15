use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey as OwnerTestMappingKey;
use slug_bzlmod_v2::HostSelectedExtensionDemandObservationKey as OwnerTestDemandKey;

use super::selected_source_preparation as owner_test_api;

#[test]
fn selected_preparation_owner_refusal_is_transient_with_unchanged_formatting() {
    use owner_test_api::SelectedPreparationOwnerError as Error;
    use owner_test_api::SelectedPreparationOwnerKey as OwnerKey;
    use owner_test_api::SelectedPreparationOwnerRefusalSite as Site;

    for site in [
        Site::MappingObservation,
        Site::MappingSemantic,
        Site::MappingView,
        Site::DemandObservation,
        Site::DemandSemantic,
    ] {
        let error = Error::SourceSelection {
            site,
            classification: slug_bzlmod_v2::SelectedSourceRefusal::default(),
        };
        assert!(error.is_transient());
        assert_eq!(
            error.source_selection(),
            Some((site, slug_bzlmod_v2::SelectedSourceRefusal::default()))
        );
        assert_eq!(format!("{error:?}"), "SourceSelection");
        assert_eq!(
            error.to_string(),
            "selected source owner refused: SourceSelection"
        );
        let value = SourcePreparationOutcome::Complete(Err(error));
        assert!(!OwnerKey::validity(&value));
        assert!(!OwnerKey::equality(&value, &value));
    }
    for (error, name) in [
        (Error::InvalidSelector, "InvalidSelector"),
        (Error::MissingImport, "MissingImport"),
        (Error::NotGenerated, "NotGenerated"),
        (Error::WrongOwnerFamily, "WrongOwnerFamily"),
        (Error::Operational, "Operational"),
    ] {
        assert_eq!(format!("{error:?}"), name);
        assert_eq!(error.source_selection(), None);
    }
}

#[tokio::test]
async fn selected_preparation_owner_normal_refusal_repair_need_and_success_equality() {
    use owner_test_api::SelectedPreparationOwnerError as Error;
    use owner_test_api::SelectedPreparationOwnerKey as OwnerKey;
    use owner_test_api::SelectedPreparationOwnerRefusalSite as Site;

    let source = tool_test_source(true, true, "");
    let (dice, mut tx, _, resources, tracker) = tool_test_fixture(&source).await;
    let key = owner_test_key("proof", &resources);
    let original = crate::compute_selected_preparation_owner(&mut tx, &key)
        .await
        .unwrap();
    assert!(matches!(
        original,
        SourcePreparationOutcome::Complete(Ok(_))
    ));
    assert!(OwnerKey::validity(&original) && OwnerKey::equality(&original, &original));
    let epoch = tx.compute(&PathObservationEpochKey).await.unwrap();
    let mut known = epoch.observations().iter().map(|(d, _)| d.dupe()).collect();
    let missing = PathObservationEpoch::from_shared(
        epoch
            .observations()
            .iter()
            .filter(|(d, _)| {
                !(d.namespace() == PathObservationNamespace::Host
                    && d.path().as_path()
                        == std::path::Path::new(REPO_RULE_WORKSPACE).join("MODULE.bazel")
                    && d.operation() == PathObservationOperation::FileBytes)
            })
            .map(|(d, v)| (d.dupe(), v.dupe())),
    )
    .unwrap();
    tx = data_test_observations(tx, &missing, &mut known).await;
    let need = crate::compute_selected_preparation_owner(&mut tx, &key)
        .await
        .unwrap();
    assert!(
        matches!(need, SourcePreparationOutcome::Need(_)),
        "normal empty frontier must Need"
    );
    assert!(!OwnerKey::validity(&need) && !OwnerKey::equality(&need, &need));

    // Malformed actual root input travels through the normal mapping producer,
    // not an injected completed owner/error. The diagnostic payload is opaque
    // precisely where the existing evaluator has already retained only text.
    tx = data_test_observations(tx, &selected_factor_epoch("module(", &source), &mut known).await;
    let refused = crate::compute_selected_preparation_owner(&mut tx, &key)
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Err(Error::SourceSelection {
        site,
        classification,
    })) = &refused
    else {
        panic!("normal malformed root must return the typed source refusal");
    };
    assert_eq!(*site, Site::MappingSemantic);
    assert!(!classification.codes().is_empty());
    assert!(classification.variant_path_complete());
    assert_eq!(
        classification.termination(),
        slug_bzlmod_v2::SelectedSourceRefusalTermination::OpaqueLeaf
    );
    assert!(classification.native_cause().is_none() && !classification.native_detail_observed());
    assert!(!OwnerKey::validity(&refused) && !OwnerKey::equality(&refused, &refused));
    tx = data_test_observations(tx, &epoch, &mut known).await;
    // A fresh normal transaction retains the original service/activation owner.
    tx = scratch_test_inputs(
        tx,
        tracker,
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        None,
    )
    .await;
    let repaired = crate::compute_selected_preparation_owner(&mut tx, &key)
        .await
        .unwrap();
    assert!(OwnerKey::validity(&repaired));
    assert!(OwnerKey::equality(&original, &repaired));
    assert!(OwnerKey::equality(
        &repaired,
        &crate::compute_selected_preparation_owner(&mut tx, &key)
            .await
            .unwrap()
    ));
    drop(tx);
    dice.wait_for_idle().await;
}

fn owner_test_key(
    name: &str,
    r: &runtime_test_bzl::SelectedScratchResources,
) -> owner_test_api::SelectedPreparationOwnerKey {
    owner_test_api::SelectedPreparationOwnerKey::new(
        NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
        &ApparentRepoName::new(name).unwrap(),
        r.descriptor_budget(),
    )
    .unwrap()
}
fn owner_test_value(
    value: <owner_test_api::SelectedPreparationOwnerKey as Key>::Value,
) -> owner_test_api::ObservedSelectedPreparationOwner {
    match value {
        SourcePreparationOutcome::Complete(Ok(value)) => value,
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn selected_preparation_owner_normal_import_full_frontier_and_warm_services() {
    let (dice, mut tx, tool, r, tracker) =
        tool_test_fixture(&tool_test_source(true, true, "")).await;
    let key = owner_test_key("proof", &r);
    let a = owner_test_value(
        crate::compute_selected_preparation_owner(&mut tx, &key)
            .await
            .unwrap(),
    );
    assert_eq!(a.owner(), tool.checkpoint().owner());
    assert!(a.matches_key(&key) && a.same_resources(&r));
    assert!(!a.observations().observations().is_empty());
    let mapping = tx
        .compute(&OwnerTestMappingKey::new(key.workspace().dupe()))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(mapping)) = mapping else {
        panic!("mapping")
    };
    let demand = tx
        .compute(&OwnerTestDemandKey::new(
            key.workspace().dupe(),
            a.requested().clone(),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(demand)) = demand else {
        panic!("demand")
    };
    for (path, value) in mapping
        .observations()
        .observations()
        .iter()
        .chain(demand.observations().observations())
    {
        assert!(
            a.observations()
                .observations()
                .iter()
                .any(|(p, v)| p == path && v == value)
        );
    }
    assert_eq!(
        a,
        owner_test_value(
            crate::compute_selected_preparation_owner(&mut tx, &key)
                .await
                .unwrap()
        )
    );
    let foreign = scratch_test_resources();
    let foreign_key = owner_test_key("proof", &foreign);
    assert_eq!(
        key, foreign_key,
        "service identity is outside semantic equality"
    );
    assert!(
        crate::compute_selected_preparation_owner(&mut tx, &foreign_key)
            .await
            .unwrap_err()
            .is_transient()
    );
    tx = scratch_test_inputs(
        tx,
        tracker.clone(),
        &foreign,
        RepositoryEnvironmentSnapshot::empty(),
        None,
    )
    .await;
    assert!(
        crate::compute_selected_preparation_owner(&mut tx, &key)
            .await
            .unwrap_err()
            .is_transient()
    );
    tx = scratch_test_inputs(
        tx,
        tracker,
        &r,
        RepositoryEnvironmentSnapshot::empty(),
        Some(std::time::Instant::now()),
    )
    .await;
    assert!(
        crate::compute_selected_preparation_owner(&mut tx, &key)
            .await
            .unwrap_err()
            .is_transient()
    );
    drop(tx);
    dice.wait_for_idle().await;
}

#[tokio::test]
async fn selected_preparation_owner_mapping_a_b_a_and_typed_absence() {
    let (dice, tx, _, r, tracker) = tool_test_fixture(&tool_test_source(true, true, "")).await;
    drop(tx);
    dice.wait_for_idle().await;
    let key = owner_test_key("proof", &r);
    let source = tool_test_source(true, true, "");
    let mut versions = Vec::new();
    let b = SCRATCH_TEST_MODULE.replace("proof='proof'", "proof='alternate'");
    for module in [SCRATCH_TEST_MODULE, b.as_str(), SCRATCH_TEST_MODULE] {
        let (base, _) =
            selected_factor_transaction(&dice, module, &source, tracker.clone(), true).await;
        let mut tx = scratch_test_inputs(
            base,
            tracker.clone(),
            &r,
            RepositoryEnvironmentSnapshot::empty(),
            None,
        )
        .await;
        versions.push(owner_test_value(
            crate::compute_selected_preparation_owner(&mut tx, &key)
                .await
                .unwrap(),
        ));
        let missing =
            crate::compute_selected_preparation_owner(&mut tx, &owner_test_key("unimported", &r))
                .await
                .unwrap();
        assert!(missing.complete_eq(&SourcePreparationOutcome::Complete(Err(
            owner_test_api::SelectedPreparationOwnerError::MissingImport
        ))));
        assert!(owner_test_api::SelectedPreparationOwnerKey::validity(
            &missing
        ));
        let direct =
            crate::compute_selected_preparation_owner(&mut tx, &owner_test_key("dep_alias", &r))
                .await
                .unwrap();
        assert!(direct.complete_eq(&SourcePreparationOutcome::Complete(Err(
            owner_test_api::SelectedPreparationOwnerError::NotGenerated
        ))));
        drop(tx);
        dice.wait_for_idle().await;
    }
    assert_ne!(versions[0].requested(), versions[1].requested());
    assert_ne!(versions[0], versions[1]);
    assert_eq!(versions[0], versions[2]);
}

#[tokio::test]
async fn selected_preparation_owner_need_is_invalid_and_credit_failure_repairs() {
    let (dice, mut tx, _, r, tracker) = tool_test_fixture(&tool_test_source(true, true, "")).await;
    let key = owner_test_key("proof", &r);
    let epoch = tx.compute(&PathObservationEpochKey).await.unwrap();
    let missing = PathObservationEpoch::from_shared(
        epoch
            .observations()
            .iter()
            .filter(|(d, _)| {
                !(d.namespace() == PathObservationNamespace::Host
                    && d.path().as_path()
                        == std::path::Path::new(REPO_RULE_WORKSPACE).join("MODULE.bazel")
                    && d.operation() == PathObservationOperation::FileBytes)
            })
            .map(|(d, v)| (d.dupe(), v.dupe())),
    )
    .unwrap();
    let mut known = epoch.observations().iter().map(|(d, _)| d.dupe()).collect();
    tx = data_test_observations(tx, &missing, &mut known).await;
    let need = crate::compute_selected_preparation_owner(&mut tx, &key)
        .await
        .unwrap();
    assert!(
        matches!(need, SourcePreparationOutcome::Need(_)),
        "{need:?}"
    );
    assert!(!owner_test_api::SelectedPreparationOwnerKey::validity(
        &need
    ));
    tx = data_test_observations(tx, &epoch, &mut known).await;
    let held = r
        .descriptor_budget()
        .try_reserve(r.descriptor_budget().limit_bytes() - r.descriptor_budget().used_bytes())
        .unwrap();
    let failed = crate::compute_selected_preparation_owner(&mut tx, &key)
        .await
        .unwrap();
    assert!(matches!(
        failed,
        SourcePreparationOutcome::Complete(Err(
            owner_test_api::SelectedPreparationOwnerError::Operational
        ))
    ));
    assert!(!owner_test_api::SelectedPreparationOwnerKey::validity(
        &failed
    ));
    drop(held);
    // Invalid resource results remain memoized in the live transaction. Reopen
    // the same semantic version after draining, as the native driver does.
    let live = tx.live_context_id();
    let updater = tx.into_updater();
    dice.wait_for_idle().await;
    let mut tx = updater.commit().await;
    assert_ne!(live, tx.live_context_id());
    let value = owner_test_value(
        crate::compute_selected_preparation_owner(&mut tx, &key)
            .await
            .unwrap(),
    );
    assert!(value.same_resources(&r));
    scratch_provenance_no_publication(
        &tracker,
        &super::selected_scratch::HostSelectedExtensionScratchPrefixKey::new(
            key.workspace().dupe(),
            value.owner().clone(),
            runtime_test_bzl::SelectedScratchPolicy::plain(),
        ),
    );
    drop(tx);
    dice.wait_for_idle().await;
}

#[tokio::test]
async fn selected_preparation_owner_innate_repository_rule_is_wrong_family() {
    let (dice, tx, _, r, tracker) = tool_test_fixture(&tool_test_source(true, true, "")).await;
    drop(tx);
    dice.wait_for_idle().await;
    let module = "module(name='consumer')\nrepo=use_repo_rule('//:repo.bzl','simple_repo')\nrepo(name='proof')\n";
    let tx =
        repository_rule_sources_transaction(&dice, module, &[], 0, Some(tracker.clone())).await;
    let tx = selected_watch_observations(tx, selected_factor_epoch(module, "")).await;
    let mut tx = scratch_test_inputs(
        tx,
        tracker,
        &r,
        RepositoryEnvironmentSnapshot::empty(),
        None,
    )
    .await;
    let value = crate::compute_selected_preparation_owner(&mut tx, &owner_test_key("proof", &r))
        .await
        .unwrap();
    assert!(
        value.complete_eq(&SourcePreparationOutcome::Complete(Err(
            owner_test_api::SelectedPreparationOwnerError::WrongOwnerFamily
        ))),
        "{value:?}"
    );
    assert!(owner_test_api::SelectedPreparationOwnerKey::validity(
        &value
    ));
}
