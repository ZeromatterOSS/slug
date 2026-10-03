include!("test_fixture.rs");

#[tokio::test]
async fn runtime_sources_explicit_empty_profile_keeps_tools_and_missing_is_distinct() {
    let (_dice, tx, tool, r, tracker) = tool_test_fixture(&tool_test_source(true, true, "")).await;
    let key = runtime_test_api::RuntimeSourceProfileKey::new(tool.requests_key().clone());
    let empty = runtime_test_policy(&r, &[], Default::default());
    let mut tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(empty.dupe()),
        Some(runtime_test_authorization(&empty, &r)),
        None,
        None,
    )
    .await;
    let plan = runtime_test_requests(
        crate::compute_runtime_source_requests(&mut tx, key.requests_key())
            .await
            .unwrap(),
    );
    assert_eq!(plan.requests().len(), 0);
    assert_eq!(plan.tool_requests().requests().len(), 1);
    tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(empty.dupe()),
        Some(runtime_test_authorization(&empty, &r)),
        Some(runtime_test_ready_scratch(&plan, &r)),
        None,
    )
    .await;
    let profile = runtime_test_profile(
        crate::compute_runtime_source_profile(&mut tx, &key)
            .await
            .unwrap(),
    );
    assert_eq!(profile.images().len(), 0);
    assert_eq!(profile.tool_sources().sources().len(), 1);
    assert_eq!(
        profile.totals(),
        runtime_test_api::RuntimeSourceTotals {
            entries: 0,
            file_bytes: 0,
            descriptor_bytes: 0
        }
    );
    assert_eq!(profile.image_binding(0), None);
    tx = runtime_test_install(tx, tracker, &r, None, None, None, None).await;
    assert!(
        crate::compute_runtime_source_requests(&mut tx, key.requests_key())
            .await
            .unwrap()
            .complete_eq(&SourcePreparationOutcome::Complete(Err(
                runtime_test_api::RuntimeSourcesError::MissingDeclaration
            )))
    );
}

#[tokio::test]
async fn runtime_sources_original_mapping_resolves_normal_source_and_full_overlap_batch() {
    use runtime_test_bzl::RuntimeSourceGuestMappingInput as M;
    use runtime_test_bzl::RuntimeSourceSelectorInput as S;
    let (_dice, tx, tool, r, tracker) = tool_test_fixture(&tool_test_source(true, true, "")).await;
    let key = runtime_test_api::RuntimeSourceRequestsKey::new(tool.requests_key().clone());
    let original = runtime_test_policy(
        &r,
        &[runtime_test_bzl::RuntimeSourceDeclarationInput {
            source: S::WorkspaceRelativeDirectory {
                relative: "runtime",
            },
            guest_mapping: M::OriginalSource,
        }],
        Default::default(),
    );
    let mut tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(original.dupe()),
        Some(runtime_test_authorization(&original, &r)),
        None,
        None,
    )
    .await;
    let a = runtime_test_requests(
        crate::compute_runtime_source_requests(&mut tx, &key)
            .await
            .unwrap(),
    );
    let row = a.requests().next().unwrap();
    assert_eq!(
        row.guest_prefix().as_path(),
        row.image_key().source_root().as_path()
    );
    assert_eq!(
        row.guest_prefix().as_str(),
        format!("{REPO_RULE_WORKSPACE}/runtime")
    );
    let explicit = runtime_test_policy(
        &r,
        &[runtime_test_relative(
            "runtime",
            row.guest_prefix().as_str(),
        )],
        Default::default(),
    );
    tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(explicit.dupe()),
        Some(runtime_test_authorization(&explicit, &r)),
        None,
        None,
    )
    .await;
    let b = runtime_test_requests(
        crate::compute_runtime_source_requests(&mut tx, &key)
            .await
            .unwrap(),
    );
    assert_ne!(
        a, b,
        "raw mapping instruction remains semantic even for equal resolved bytes"
    );
    assert_eq!(
        a.requests().next().unwrap().image_key(),
        b.requests().next().unwrap().image_key()
    );
    assert_eq!(
        a.requests().next().unwrap().guest_prefix(),
        b.requests().next().unwrap().guest_prefix()
    );
    // Unknown original spelling conflicts with a later explicit spelling only
    // after normal resolution. No image observations have been installed.
    let overlap = runtime_test_policy(
        &r,
        &[
            runtime_test_bzl::RuntimeSourceDeclarationInput {
                source: S::WorkspaceRelativeDirectory {
                    relative: "runtime",
                },
                guest_mapping: M::OriginalSource,
            },
            runtime_test_relative("other", row.guest_prefix().as_str()),
        ],
        Default::default(),
    );
    tx = runtime_test_install(
        tx,
        tracker.clone(),
        &r,
        Some(overlap.dupe()),
        Some(runtime_test_authorization(&overlap, &r)),
        None,
        None,
    )
    .await;
    let refused = crate::compute_runtime_source_requests(&mut tx, &key)
        .await
        .unwrap();
    assert!(refused.complete_eq(&SourcePreparationOutcome::Complete(Err(
        runtime_test_api::RuntimeSourcesError::InvalidScope
    ))));
    tx = runtime_test_install(
        tx,
        tracker,
        &r,
        Some(original.dupe()),
        Some(runtime_test_authorization(&original, &r)),
        None,
        None,
    )
    .await;
    assert_eq!(
        a,
        runtime_test_requests(
            crate::compute_runtime_source_requests(&mut tx, &key)
                .await
                .unwrap()
        )
    );
}

// Independent raw rows feed each normal image key. Never inject complete images
// or a tool/profile certificate, including the missing/extra/digest negatives.
fn runtime_covered_epoch(
    key: &runtime_test_ws::ObservedInputImageKey,
    r: &runtime_test_bzl::SelectedScratchResources,
    digest: runtime_test_ws::FileContentDigest,
    member: bool,
) -> PathObservationEpoch {
    use runtime_test_ws::ImageObservationData as D;
    let epoch = runtime_test_image_epoch(key, r, 1, member.then_some((1, 0o755)));
    PathObservationEpoch::from_shared(epoch.observations().iter().map(|(d, v)| {
        let PathObservationResult::InputImage(v) = v.as_ref() else {
            panic!("image row")
        };
        let data = match v.data() {
            D::Digest(_) => D::Digest(PathOperationResult::Present(digest)),
            D::Lstat(PathOperationResult::Present(s)) if s.kind() == PathNodeKind::RegularFile => {
                D::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::RegularFile,
                    digest.size_bytes() as i64,
                    1,
                    1,
                    1,
                    0o755,
                )))
            }
            other => other.clone(),
        };
        (
            d.dupe(),
            Arc::new(PathObservationResult::InputImage(
                runtime_test_ws::ImageObservationResult::new(
                    v.issued_path().dupe(),
                    v.request().dupe(),
                    data,
                    r.descriptor_budget().try_reserve(512).unwrap(),
                )
                .unwrap(),
            )),
        )
    }))
    .unwrap()
}

#[tokio::test]
async fn runtime_sources_original_label_tree_certifies_complete_interval_and_refuses_missing_digest()
 {
    let mut f = DataTestFixture::label_case("//pkg:data", "", "runtime", "/rt", true).await;
    let selected = f
        .requests
        .tool_requests()
        .requests()
        .next()
        .unwrap()
        .label_selection()
        .unwrap()
        .dupe();
    let key = f.key.profile_key().clone();
    let policy = runtime_test_policy(
        &f.r,
        &[runtime_test_bzl::RuntimeSourceDeclarationInput {
            source: runtime_test_bzl::RuntimeSourceSelectorInput::SelectedToolOriginLabelParent {
                role: SelectedToolRole::Generator,
                children: "",
            },
            guest_mapping: runtime_test_bzl::RuntimeSourceGuestMappingInput::OriginalSource,
        }],
        runtime_test_bzl::RuntimeSourceLimits {
            // Distinct bounded-directory requests let independent normal images
            // carry the missing-member negative without conflicting raw cells.
            max_entries: selected.image_key().policy().limits().max_entries - 1,
            max_file_bytes: 1024,
            ..Default::default()
        },
    );
    let mut tx = runtime_test_install(
        f.tx.take().unwrap(),
        f.tracker.clone(),
        &f.r,
        Some(policy.dupe()),
        Some(runtime_test_authorization(&policy, &f.r)),
        Some(data_test_ready_tools(&f.requests, &f.r)),
        None,
    )
    .await;
    let plan = runtime_test_requests(
        crate::compute_runtime_source_requests(&mut tx, key.requests_key())
            .await
            .unwrap(),
    );
    let row = plan.requests().next().unwrap();
    assert_eq!(
        row.guest_prefix().as_path(),
        selected.image_key().source_root().as_path()
    );
    let tool_epoch = data_test_tool_epoch(&selected, &f.r);
    let digest = tool_epoch
        .observations()
        .iter()
        .find_map(|(d, v)| {
            if d.path() != selected.selected_file().normalized_ref() {
                return None;
            }
            match v.as_ref() {
                PathObservationResult::InputImage(v) => match v.data() {
                    runtime_test_ws::ImageObservationData::Digest(
                        PathOperationResult::Present(d),
                    ) => Some(*d),
                    _ => None,
                },
                _ => None,
            }
        })
        .unwrap();
    let baseline = runtime_test_union(&f.epoch, &tool_epoch);
    let mut good = None;
    for (present, binding) in [
        (true, digest),
        (false, digest),
        (
            true,
            runtime_test_ws::FileContentDigest::new([7; 32], 64).unwrap(),
        ),
        (true, digest),
    ] {
        let epoch = runtime_test_union(
            &baseline,
            &runtime_covered_epoch(row.image_key(), &f.r, binding, present),
        );
        tx = data_test_observations(tx, &epoch, &mut f.known_paths).await;
        tx = runtime_test_install(
            tx,
            f.tracker.clone(),
            &f.r,
            Some(policy.dupe()),
            Some(runtime_test_authorization(&policy, &f.r)),
            Some(data_test_ready_tools(&plan, &f.r)),
            None,
        )
        .await;
        let outcome = crate::compute_runtime_source_profile(&mut tx, &key)
            .await
            .unwrap();
        if present && binding == digest {
            let profile = runtime_test_profile(outcome);
            assert_eq!(
                profile.image_binding(0),
                Some(runtime_test_api::RuntimeSourceImageBinding::CoveredTool {
                    role: 0,
                    root_entry: 0,
                    entry_count: 2
                })
            );
            assert_eq!(profile.tool_root_member(0), Some((0, 0)));
            assert_eq!(profile.tool_member(0, 0), Some((0, 0)));
            assert_eq!(profile.tool_member(0, 1), Some((0, 1)));
            assert_eq!(profile.tool_member(0, 2), None);
            assert_eq!(profile.totals().entries, 2);
            assert_eq!(
                profile.totals().file_bytes,
                64,
                "semantic occurrences survive physical reuse"
            );
            if let Some(a) = &good {
                assert_eq!(a, &profile)
            } else {
                good = Some(profile)
            }
        } else {
            assert!(
                outcome.complete_eq(&SourcePreparationOutcome::Complete(Err(
                    runtime_test_api::RuntimeSourcesError::InvalidScope
                ))),
                "{outcome:?}"
            );
        }
    }
    let profile = good.unwrap();
    // Retain the genuine normal tool certificate A while independently computing
    // normal image B. Shared Lstat inputs cannot describe inconsistent simultaneous
    // modes, so this exercises the actual interval comparator across real epochs.
    for extra in [false, true] {
        use runtime_test_ws::ImageObservationData as D;
        let own = runtime_covered_epoch(row.image_key(), &f.r, digest, true);
        let mut rows = own
            .observations()
            .iter()
            .map(|(d, v)| {
                let PathObservationResult::InputImage(v) = v.as_ref() else {
                    panic!("image row")
                };
                let data = match v.data() {
                    D::Lstat(PathOperationResult::Present(s))
                        if !extra && s.kind() == PathNodeKind::RegularFile =>
                    {
                        D::Lstat(PathOperationResult::Present(PathLstat::new(
                            PathNodeKind::RegularFile,
                            64,
                            1,
                            1,
                            1,
                            0o700,
                        )))
                    }
                    D::Directory(_) if extra => D::Directory(PathOperationResult::Present(
                        runtime_test_ws::PathDirectoryEntries::new(
                            ["data", "extra"].into_iter().map(|n| {
                                runtime_test_ws::PathDirectoryEntry::new(
                                    runtime_test_ws::PathDirectoryName::new(n).unwrap(),
                                    runtime_test_ws::PathDirectoryEntryKind::File,
                                )
                            }),
                        ),
                    )),
                    d => d.clone(),
                };
                (
                    d.dupe(),
                    Arc::new(PathObservationResult::InputImage(
                        runtime_test_ws::ImageObservationResult::new(
                            v.issued_path().dupe(),
                            v.request().dupe(),
                            data,
                            f.r.descriptor_budget().try_reserve(512).unwrap(),
                        )
                        .unwrap(),
                    )),
                )
            })
            .collect::<Vec<_>>();
        if extra {
            let anchor = runtime_test_ws::ImageChargedPath::pin_existing(
                row.image_key().anchor_root().dupe(),
                f.r.descriptor_budget(),
            )
            .unwrap();
            let path = runtime_test_ws::ImageChargedPath::pin_existing(
                row.image_key().source_root().dupe(),
                f.r.descriptor_budget(),
            )
            .unwrap()
            .try_join_component(&runtime_test_ws::PathDirectoryName::new("extra").unwrap())
            .unwrap();
            for (operation, data) in [
                (
                    runtime_test_ws::ImageObservationOperation::Lstat,
                    D::Lstat(PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::RegularFile,
                        1,
                        1,
                        1,
                        1,
                        0o644,
                    ))),
                ),
                (
                    runtime_test_ws::ImageObservationOperation::BoundedDigest {
                        max_bytes: row.image_key().policy().limits().max_file_bytes,
                    },
                    D::Digest(PathOperationResult::Present(
                        runtime_test_ws::FileContentDigest::new([9; 32], 1).unwrap(),
                    )),
                ),
            ] {
                let request =
                    runtime_test_ws::ImageObservationRequest::new(anchor.dupe(), operation);
                let demand = PathObservationDemand::input_image(
                    row.image_key().namespace(),
                    path.dupe(),
                    request.dupe(),
                )
                .unwrap();
                let result = runtime_test_ws::ImageObservationResult::new(
                    path.dupe(),
                    request,
                    data,
                    f.r.descriptor_budget().try_reserve(512).unwrap(),
                )
                .unwrap();
                rows.push((demand, Arc::new(PathObservationResult::InputImage(result))));
            }
        }
        // Replace rows rather than silently unioning conflicting observations.
        let mut changed_rows = baseline
            .observations()
            .iter()
            .filter(|(d, _)| !rows.iter().any(|(new, _)| new == *d))
            .map(|(d, v)| (d.dupe(), v.dupe()))
            .collect::<Vec<_>>();
        changed_rows.extend(rows);
        let changed = PathObservationEpoch::from_shared(changed_rows).unwrap();
        tx = data_test_observations(tx, &changed, &mut f.known_paths).await;
        tx = runtime_test_install(
            tx,
            f.tracker.clone(),
            &f.r,
            Some(policy.dupe()),
            Some(runtime_test_authorization(&policy, &f.r)),
            Some(data_test_ready_tools(&plan, &f.r)),
            None,
        )
        .await;
        let image = match tx.compute(row.image_key()).await.unwrap() {
            PathOutcome::Complete(Ok(image)) => image,
            other => panic!("normal image B: {other:?}"),
        };
        assert_eq!(
            runtime_test_api::certify_runtime_test(row, &image, profile.tool_sources())
                .unwrap_err(),
            runtime_test_api::RuntimeSourcesError::InvalidScope
        );
    }
    let epoch = runtime_test_union(
        &baseline,
        &runtime_covered_epoch(row.image_key(), &f.r, digest, true),
    );
    tx = data_test_observations(tx, &epoch, &mut f.known_paths).await;
    tx = runtime_test_install(
        tx,
        f.tracker.clone(),
        &f.r,
        Some(policy.dupe()),
        Some(runtime_test_authorization(&policy, &f.r)),
        Some(data_test_ready_tools(&plan, &f.r)),
        None,
    )
    .await;
    let tool_image = profile
        .tool_sources()
        .sources()
        .next()
        .unwrap()
        .label_metadata()
        .unwrap()
        .image();
    let parents = |root: &std::path::Path| root.ancestors().skip(1).count();
    let occurrence = profile.tool_sources().prefix().prefix().entries().len()
        + parents(std::path::Path::new("/__slug_module"))
        + tool_image.entries().len()
        + parents(tool_image.source_root().as_path())
        + profile.images().next().unwrap().entries().len()
        + parents(row.guest_prefix().as_path());
    let mut guest_limits = GuestTestLimits::default();
    guest_limits.max_entries = occurrence;
    let guest =
        GuestTestPolicy::new(1000, 100, 1000, 100, 9, 123, 456, 0o755, guest_limits).unwrap();
    let guest_key = guest_test_api::SelectedGuestNamespaceFactsKey::new(key.clone());
    tx = runtime_test_install_with_guest(
        tx,
        f.tracker.clone(),
        &f.r,
        Some(policy.dupe()),
        Some(runtime_test_authorization(&policy, &f.r)),
        Some(data_test_ready_tools(&plan, &f.r)),
        None,
        Some((Some(guest), Some(guest_test_tx(guest, &f.r)))),
    )
    .await;
    let facts = guest_test_facts(
        crate::compute_selected_guest_namespace_facts(&mut tx, &guest_key)
            .await
            .unwrap(),
    );
    assert!(matches!(
        facts
            .lookup(
                row.guest_prefix().as_str(),
                guest_test_api::GuestNamespaceOperation::ListDirectory
            )
            .unwrap()
            .binding(),
        guest_test_api::GuestNamespaceBinding::ToolImage { role: 0, entry: 0 }
    ));
    guest_limits.max_entries = occurrence - 2;
    let limited =
        GuestTestPolicy::new(1000, 100, 1000, 100, 9, 123, 456, 0o755, guest_limits).unwrap();
    tx = runtime_test_install_with_guest(
        tx,
        f.tracker.clone(),
        &f.r,
        Some(policy.dupe()),
        Some(runtime_test_authorization(&policy, &f.r)),
        Some(data_test_ready_tools(&plan, &f.r)),
        None,
        Some((Some(limited), Some(guest_test_tx(limited, &f.r)))),
    )
    .await;
    let refusal = crate::compute_selected_guest_namespace_facts(&mut tx, &guest_key)
        .await
        .unwrap();
    assert!(
        refusal.complete_eq(&SourcePreparationOutcome::Complete(Err(
            guest_test_api::GuestNamespaceFactsError::Limit
        ))),
        "covered rows must count before dedup: {refusal:?}"
    );
    drop(tx);
    f.dice.wait_for_idle().await;
}

#[tokio::test]
async fn runtime_sources_original_marker_parent_does_not_widen_direct_binary_origin() {
    for (label, derive, suffix) in [
        (
            "//pkg:data",
            "    data=data.dirname.get_child('bin').get_child('cargo')\n",
            "/pkg/lib",
        ),
        ("//pkg:bin/cargo", "", "/pkg/bin/lib"),
    ] {
        let mut f = DataTestFixture::label_case(label, derive, "runtime", "/rt", true).await;
        let policy = runtime_test_policy(
            &f.r,
            &[runtime_test_bzl::RuntimeSourceDeclarationInput {
                source:
                    runtime_test_bzl::RuntimeSourceSelectorInput::SelectedToolOriginLabelParent {
                        role: SelectedToolRole::Generator,
                        children: "lib",
                    },
                guest_mapping: runtime_test_bzl::RuntimeSourceGuestMappingInput::OriginalSource,
            }],
            Default::default(),
        );
        let mut tx = runtime_test_install(
            f.tx.take().unwrap(),
            f.tracker.clone(),
            &f.r,
            Some(policy.dupe()),
            Some(runtime_test_authorization(&policy, &f.r)),
            Some(data_test_ready_tools(&f.requests, &f.r)),
            None,
        )
        .await;
        let plan = runtime_test_requests(
            crate::compute_runtime_source_requests(&mut tx, f.key.profile_key().requests_key())
                .await
                .unwrap(),
        );
        let row = plan.requests().next().unwrap();
        assert_eq!(
            row.guest_prefix().as_str(),
            format!("{REPO_RULE_WORKSPACE}{suffix}")
        );
        assert_eq!(
            row.guest_prefix().as_path(),
            row.image_key().source_root().as_path()
        );
        assert_eq!(
            row.selected_label_source()
                .unwrap()
                .recipe()
                .label()
                .target()
                .as_str(),
            label.split(':').nth(1).unwrap()
        );
        drop(tx);
        f.dice.wait_for_idle().await;
    }
}

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
                guest_mapping: runtime_test_bzl::RuntimeSourceGuestMappingInput::ExplicitPrefix(
                    "/rt",
                ),
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
                guest_mapping: runtime_test_bzl::RuntimeSourceGuestMappingInput::ExplicitPrefix(
                    "/rt",
                ),
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
