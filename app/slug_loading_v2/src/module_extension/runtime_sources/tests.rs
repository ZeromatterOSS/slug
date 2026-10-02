include!("test_fixture.rs");

#[tokio::test]
async fn runtime_sources_missing_policy_precedes_tool_dependencies_and_budget_pressure() {
    let (dice, mut tx, tool, r, tracker) =
        tool_test_fixture(&tool_test_source(true, true, "")).await;
    let key = runtime_test_api::RuntimeSourceRequestsKey::new(tool.requests_key().clone());
    tx = runtime_test_install(tx, tracker.clone(), &r, None, None, None, None).await;
    tracker.take_dependencies();
    let held = r
        .descriptor_budget()
        .try_reserve(r.descriptor_budget().limit_bytes() - r.descriptor_budget().used_bytes())
        .unwrap();
    let outcome = crate::compute_runtime_source_requests(&mut tx, &key)
        .await
        .unwrap();
    assert!(outcome.complete_eq(&SourcePreparationOutcome::Complete(Err(
        runtime_test_api::RuntimeSourcesError::MissingDeclaration
    ))));
    assert!(<runtime_test_api::RuntimeSourceRequestsKey as Key>::validity(&outcome));
    drop(held);
    drop(tx);
    dice.wait_for_idle().await;
    assert!(
        tracker
            .take_dependencies()
            .iter()
            .all(|(name, _)| name != &tool.requests_key().to_string()),
        "absence must not activate the normal tool plan"
    );
}

#[tokio::test]
async fn runtime_sources_normal_policy_a_b_a_and_warm_current_transaction() {
    let (_dice, tx, tool, r, tracker) = tool_test_fixture(&tool_test_source(true, true, "")).await;
    let key = runtime_test_api::RuntimeSourceRequestsKey::new(tool.requests_key().clone());
    let a = runtime_test_policy(
        &r,
        &[runtime_test_relative("runtime-a", "/rt")],
        Default::default(),
    );
    let b = runtime_test_policy(
        &r,
        &[runtime_test_relative("runtime-b", "/rt")],
        Default::default(),
    );
    let mut tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(a.dupe()),
        Some(runtime_test_authorization(&a, &r)),
        None,
        None,
    )
    .await;
    let va = runtime_test_requests(
        crate::compute_runtime_source_requests(&mut tx, &key)
            .await
            .unwrap(),
    );
    assert_eq!(va.requests().next().unwrap().guest_prefix().as_str(), "/rt");
    tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(b.dupe()),
        Some(runtime_test_authorization(&b, &r)),
        None,
        None,
    )
    .await;
    let vb = runtime_test_requests(
        crate::compute_runtime_source_requests(&mut tx, &key)
            .await
            .unwrap(),
    );
    assert_ne!(va, vb);
    assert_ne!(
        va.requests().next().unwrap().image_key(),
        vb.requests().next().unwrap().image_key()
    );
    tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(a.dupe()),
        Some(runtime_test_authorization(&a, &r)),
        None,
        None,
    )
    .await;
    assert_eq!(
        va,
        runtime_test_requests(
            crate::compute_runtime_source_requests(&mut tx, &key)
                .await
                .unwrap()
        )
    );
    tx = runtime_test_install(tx, tracker.clone(), &r, Some(a.dupe()), None, None, None).await;
    assert!(
        crate::compute_runtime_source_requests(&mut tx, &key)
            .await
            .unwrap_err()
            .is_transient()
    );
    let foreign = scratch_test_resources();
    let foreign_a = runtime_test_policy(
        &foreign,
        &[runtime_test_relative("runtime-a", "/rt")],
        Default::default(),
    );
    assert_eq!(a, foreign_a);
    tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(a.dupe()),
        Some(runtime_test_authorization(&foreign_a, &foreign)),
        None,
        None,
    )
    .await;
    assert!(
        crate::compute_runtime_source_requests(&mut tx, &key)
            .await
            .unwrap_err()
            .is_transient()
    );
    tx = runtime_test_install(tx, tracker, &r, None, None, None, None).await;
    assert!(
        crate::compute_runtime_source_requests(&mut tx, &key)
            .await
            .unwrap()
            .complete_eq(&SourcePreparationOutcome::Complete(Err(
                runtime_test_api::RuntimeSourcesError::MissingDeclaration
            )))
    );
}

#[tokio::test]
async fn runtime_sources_normal_resolution_rejects_tool_shadow_overlap_and_scratch_label_role() {
    let (_dice, mut tx, tool, r, tracker) =
        tool_test_fixture(&tool_test_source(true, true, "")).await;
    let key = runtime_test_api::RuntimeSourceRequestsKey::new(tool.requests_key().clone());
    let workspace = REPO_RULE_WORKSPACE;
    let containing_host = std::path::Path::new(workspace)
        .parent()
        .unwrap()
        .to_str()
        .unwrap();
    for (declarations, pre_admission_error, expected) in [
        (
            vec![runtime_test_relative("runtime", "/__slug_module_extra")],
            None,
            None,
        ),
        (
            vec![runtime_test_relative("runtime", "/__slug_module/generator")],
            Some(runtime_test_bzl::RuntimeSourceDeclarationError::Overlap),
            None,
        ),
        (
            vec![
                runtime_test_relative("", "/rt-a"),
                runtime_test_relative("child", "/rt-b"),
            ],
            Some(runtime_test_bzl::RuntimeSourceDeclarationError::Overlap),
            None,
        ),
        (
            vec![runtime_test_bzl::RuntimeSourceDeclarationInput {
                source: runtime_test_bzl::RuntimeSourceSelectorInput::ExplicitHostDirectory {
                    anchor: containing_host,
                },
                guest_prefix: "/rt",
            }],
            Some(runtime_test_bzl::RuntimeSourceDeclarationError::InvalidDeclaration),
            None,
        ),
        (
            vec![runtime_test_bzl::RuntimeSourceDeclarationInput {
                source:
                    runtime_test_bzl::RuntimeSourceSelectorInput::SelectedToolOriginLabelParent {
                        role: SelectedToolRole::Generator,
                        children: "lib",
                    },
                guest_prefix: "/rt",
            }],
            None,
            Some(runtime_test_api::RuntimeSourcesError::WrongLabelRole),
        ),
    ] {
        // Syntactically known overlaps fail at whole borrowed policy admission;
        // the remaining cases enter the genuine normal resolution producer.
        let charged = runtime_test_ws::ImageChargedPath::pin_existing(
            NormalizedAbsolutePath::new(workspace).unwrap(),
            r.descriptor_budget(),
        )
        .unwrap();
        let policy = runtime_test_bzl::RuntimeSourceDeclarationPolicy::new(
            &charged,
            &declarations,
            Default::default(),
            r.descriptor_budget(),
        );
        if let Some(error) = pre_admission_error {
            assert_eq!(policy.unwrap_err(), error);
            continue;
        }
        let policy = policy.unwrap();
        tx = runtime_test_install(
            tx,
            tracker.clone(),
            &r,
            Some(policy.dupe()),
            Some(runtime_test_authorization(&policy, &r)),
            None,
            None,
        )
        .await;
        let value = crate::compute_runtime_source_requests(&mut tx, &key)
            .await
            .unwrap();
        match expected {
            Some(error) => {
                assert!(value.complete_eq(&SourcePreparationOutcome::Complete(Err(error))));
                assert!(<runtime_test_api::RuntimeSourceRequestsKey as Key>::validity(&value));
            }
            None => {
                runtime_test_requests(value);
            }
        }
    }
}

#[tokio::test]
async fn runtime_sources_profile_need_normal_images_content_mode_frontier_and_warm_authority() {
    let (_dice, tx, tool, r, tracker) = tool_test_fixture(&tool_test_source(true, true, "")).await;
    let key = runtime_test_api::RuntimeSourceProfileKey::new(tool.requests_key().clone());
    let policy = runtime_test_policy(
        &r,
        &[runtime_test_relative("runtime", "/rt")],
        Default::default(),
    );
    let mut tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(policy.dupe()),
        Some(runtime_test_authorization(&policy, &r)),
        None,
        None,
    )
    .await;
    let requests = runtime_test_requests(
        crate::compute_runtime_source_requests(&mut tx, key.requests_key())
            .await
            .unwrap(),
    );
    let scratch = runtime_test_ready_scratch(&requests, &r);
    let empty = SelectedScratchElfMetadataTransaction::new(
        requests.tool_requests().selector(),
        &r,
        Some(
            &SelectedScratchElfMetadataNeeds::singleton(
                requests
                    .tool_requests()
                    .requests()
                    .next()
                    .unwrap()
                    .scratch_request()
                    .unwrap(),
                &r,
            )
            .unwrap(),
        ),
        &[],
    )
    .unwrap();
    tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(policy.dupe()),
        Some(runtime_test_authorization(&policy, &r)),
        Some((
            empty,
            Some((
                scratch.1.as_ref().unwrap().0.clone(),
                SelectedScratchElfMetadataEpoch::AuthorizedEmpty,
            )),
        )),
        None,
    )
    .await;
    assert!(
        matches!(crate::compute_runtime_source_profile(&mut tx,&key).await.unwrap(),SourcePreparationOutcome::Need(ref n) if n.scratch_elf_metadata_requests().is_some())
    );
    tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(policy.dupe()),
        Some(runtime_test_authorization(&policy, &r)),
        Some(scratch.clone()),
        None,
    )
    .await;
    assert!(matches!(
        crate::compute_runtime_source_profile(&mut tx, &key)
            .await
            .unwrap(),
        SourcePreparationOutcome::Need(_)
    ));
    let baseline = tx.compute(&PathObservationEpochKey).await.unwrap();
    let image = requests.requests().next().unwrap().image_key();
    let mut versions = Vec::new();
    for (digest, mode, freshness) in [
        (1, 0o644, 1),
        (2, 0o644, 1),
        (2, 0o600, 1),
        (2, 0o600, 2),
        (1, 0o644, 1),
    ] {
        let epoch = runtime_test_union(
            &baseline,
            &runtime_test_image_epoch(image, &r, freshness, Some((digest, mode))),
        );
        tx = runtime_test_install(
            tx,
            tracker.clone(),
            &r,
            Some(policy.dupe()),
            Some(runtime_test_authorization(&policy, &r)),
            Some(scratch.clone()),
            Some(epoch),
        )
        .await;
        versions.push(runtime_test_profile(
            crate::compute_runtime_source_profile(&mut tx, &key)
                .await
                .unwrap(),
        ));
    }
    assert_ne!(versions[0], versions[1]);
    assert_ne!(versions[1], versions[2]);
    assert_ne!(versions[2], versions[3]);
    assert_eq!(versions[0], versions[4]);
    assert_eq!(
        versions[2].images().next().unwrap().content(),
        versions[3].images().next().unwrap().content()
    );
    assert_eq!(versions[0].totals().entries, 2);
    assert_eq!(versions[0].totals().file_bytes, 1);
    let semantic = versions[0]
        .images()
        .next()
        .unwrap()
        .entries()
        .iter()
        .map(|e| e.path().as_path().as_os_str().len())
        .sum::<usize>();
    assert_eq!(versions[0].totals().descriptor_bytes, semantic);
    assert_eq!(
        versions[0].tool_sources().requests(),
        requests.tool_requests()
    );
    tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(policy.dupe()),
        None,
        Some(scratch.clone()),
        None,
    )
    .await;
    assert!(
        crate::compute_runtime_source_profile(&mut tx, &key)
            .await
            .unwrap_err()
            .is_transient()
    );
    let denied = SelectedScratchElfMetadataTransaction::new(
        requests.tool_requests().selector(),
        &r,
        None,
        &[],
    )
    .unwrap();
    tx = runtime_test_install(
        tx,
        tracker,
        &r,
        Some(policy.dupe()),
        Some(runtime_test_authorization(&policy, &r)),
        Some((denied, None)),
        None,
    )
    .await;
    assert!(
        crate::compute_runtime_source_profile(&mut tx, &key)
            .await
            .unwrap_err()
            .is_transient()
    );
}

#[tokio::test]
async fn runtime_sources_aggregate_overflow_stops_before_next_normal_image() {
    runtime_test_aggregate(
        runtime_test_bzl::RuntimeSourceLimits {
            max_entries: 1,
            ..Default::default()
        },
        None,
    )
    .await;
}

#[tokio::test]
async fn runtime_sources_aggregate_file_byte_boundary_stops_before_next_normal_image() {
    runtime_test_aggregate(
        runtime_test_bzl::RuntimeSourceLimits {
            max_total_file_bytes: 1,
            ..Default::default()
        },
        Some((1, 0o644)),
    )
    .await;
}

#[tokio::test]
async fn runtime_sources_aggregate_descriptor_root_boundary_stops_before_next_normal_image() {
    runtime_test_aggregate(
        runtime_test_bzl::RuntimeSourceLimits {
            // Every empty image contributes exactly its root path, including the
            // root entry counted by ImageBuilder before walking descendants.
            max_image_descriptor_bytes: REPO_RULE_WORKSPACE.len() + 2,
            ..Default::default()
        },
        None,
    )
    .await;
}

async fn runtime_test_aggregate(
    limits: runtime_test_bzl::RuntimeSourceLimits,
    file: Option<(u8, u32)>,
) {
    let (dice, tx, tool, r, tracker) = tool_test_fixture(&tool_test_source(true, true, "")).await;
    let key = runtime_test_api::RuntimeSourceProfileKey::new(tool.requests_key().clone());
    let policy = runtime_test_policy(
        &r,
        &[
            runtime_test_relative("a", "/a"),
            runtime_test_relative("b", "/b"),
            runtime_test_relative("c", "/c"),
        ],
        limits,
    );
    let mut tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(policy.dupe()),
        Some(runtime_test_authorization(&policy, &r)),
        None,
        None,
    )
    .await;
    let requests = runtime_test_requests(
        crate::compute_runtime_source_requests(&mut tx, key.requests_key())
            .await
            .unwrap(),
    );
    let mut epoch = tx.compute(&PathObservationEpochKey).await.unwrap();
    for row in requests.requests() {
        epoch = runtime_test_union(
            &epoch,
            &runtime_test_image_epoch(row.image_key(), &r, 1, file),
        );
    }
    tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(policy.dupe()),
        Some(runtime_test_authorization(&policy, &r)),
        Some(runtime_test_ready_scratch(&requests, &r)),
        Some(epoch),
    )
    .await;
    tracker.take_dependencies();
    let value = crate::compute_runtime_source_profile(&mut tx, &key)
        .await
        .unwrap();
    assert!(value.complete_eq(&SourcePreparationOutcome::Complete(Err(
        runtime_test_api::RuntimeSourcesError::AggregateLimit
    ))));
    assert!(<runtime_test_api::RuntimeSourceProfileKey as Key>::validity(&value));
    drop(tx);
    dice.wait_for_idle().await;
    let dependencies = tracker.take_dependencies();
    for (index, row) in requests.requests().enumerate() {
        assert_eq!(
            dependencies
                .iter()
                .any(|(name, _)| name == &row.image_key().to_string()),
            index < 2,
            "image {index}: {dependencies:?}"
        );
    }
}

#[tokio::test]
async fn runtime_sources_platform_refusal_and_normal_allocation_pressure_repair() {
    let (dice, tx, tool, r, tracker) = tool_test_fixture(&tool_test_source(true, true, "")).await;
    let key = runtime_test_api::RuntimeSourceRequestsKey::new(tool.requests_key().clone());
    let policy = runtime_test_policy(
        &r,
        &[runtime_test_relative("runtime", "/rt")],
        Default::default(),
    );
    let mut tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(policy.dupe()),
        Some(runtime_test_authorization(&policy, &r)),
        None,
        None,
    )
    .await;
    let plan = crate::compute_selected_tool_source_requests(&mut tx, tool.requests_key())
        .await
        .unwrap();
    assert!(matches!(plan, SourcePreparationOutcome::Complete(Ok(_))));
    let held = r
        .descriptor_budget()
        .try_reserve(r.descriptor_budget().limit_bytes() - r.descriptor_budget().used_bytes())
        .unwrap();
    let failed = crate::compute_runtime_source_requests(&mut tx, &key)
        .await
        .unwrap();
    assert!(
        matches!(failed, SourcePreparationOutcome::Complete(Err(ref error)) if error.is_transient())
    );
    assert!(!<runtime_test_api::RuntimeSourceRequestsKey as Key>::validity(&failed));
    drop(held);
    // Invalid operational results remain in the current live cache until its
    // last transaction closes. Reopen the same semantic version after drain,
    // matching the native command's retry lifecycle.
    let updater = tx.into_updater();
    dice.wait_for_idle().await;
    tx = updater.commit().await;
    runtime_test_requests(
        crate::compute_runtime_source_requests(&mut tx, &key)
            .await
            .unwrap(),
    );
    let mut updater = tx.into_updater();
    updater
        .changed_to([(
            RepositoryPlatformKey::new(NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap()),
            RepositoryPlatform::new("linux", "aarch64"),
        )])
        .unwrap();
    tx = updater.commit().await;
    let refused = crate::compute_runtime_source_requests(&mut tx, &key)
        .await
        .unwrap();
    assert!(refused.complete_eq(&SourcePreparationOutcome::Complete(Err(
        runtime_test_api::RuntimeSourcesError::UnsupportedPlatform
    ))));
    assert!(<runtime_test_api::RuntimeSourceRequestsKey as Key>::validity(&refused));
    let mut updater = tx.into_updater();
    updater
        .changed_to([(
            RepositoryPlatformKey::new(NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap()),
            RepositoryPlatform::new("linux", "x86_64"),
        )])
        .unwrap();
    tx = updater.commit().await;
    runtime_test_requests(
        crate::compute_runtime_source_requests(&mut tx, &key)
            .await
            .unwrap(),
    );
}
