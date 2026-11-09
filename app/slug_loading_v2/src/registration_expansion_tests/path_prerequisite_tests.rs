//! Real observed registration rows, with only selected Lstat facts withheld.
//! These are pure Loading prerequisites, without native roots or completions.
use super::*;

type Outcome = <ModuleRegistrationExpansionObservationKey as Key>::Value;
const BUILD: &str = "filegroup(name='impl')\ntoolchain_type(name='type')\ntoolchain(name='tc',toolchain_type=':type',toolchain=':impl')\n";

fn packages(module: &str, names: &[&str]) -> EpochBuilder {
    let mut builder = EpochBuilder::root_package_builder(module, 61);
    for name in names {
        builder.directory(&format!("/workspace/{name}"), 61);
        builder.file(&format!("/workspace/{name}/BUILD.bazel"), BUILD, 61);
    }
    builder
}
fn lstat(name: &str) -> PathObservationDemand {
    EpochBuilder::demand(
        &format!("/workspace/{name}"),
        PathObservationOperation::Lstat,
    )
}
fn without(
    epoch: &PathObservationEpoch,
    omitted: &[PathObservationDemand],
) -> PathObservationEpoch {
    PathObservationEpoch::from_shared(
        epoch
            .observations()
            .iter()
            .filter(|(d, _)| !omitted.contains(d))
            .map(|(d, r)| (d.dupe(), r.dupe())),
    )
    .unwrap()
}
fn pending(value: &Outcome, demands: &[PathObservationDemand]) {
    let LoadingPreparationOutcome::Need(needs) = value else {
        panic!("expected transient prerequisite")
    };
    let expected = slug_bzlmod_v2::SourcePreparationNeeds::path(
        slug_workspace_v2::NeedPathObservations::try_from_iter(demands.iter().cloned()).unwrap(),
    );
    assert_eq!(
        needs, &expected,
        "only the earlier whole Lstat prerequisite window is returned"
    );
    assert!(!ModuleRegistrationExpansionObservationKey::validity(value));
    assert!(!ModuleRegistrationExpansionObservationKey::equality(
        value, value
    ));
}
fn no_family_batch(tracker: &ExpansionTracker, key: &ModuleRegistrationExpansionObservationKey) {
    assert!(
        tracker
            .take()
            .iter()
            .filter(|row| row.key == key.to_string())
            .all(|row| row.batch.is_none())
    );
}
fn no_c_or_family_batch(
    tracker: &ExpansionTracker,
    key: &ModuleRegistrationExpansionObservationKey,
) -> Vec<TrackedActivation> {
    let rows = tracker.take();
    assert_no_activation_containing(&rows, "host-package-load:\"/workspace\"//c");
    assert!(
        rows.iter()
            .filter(|row| row.key == key.to_string())
            .all(|row| row.batch.is_none())
    );
    rows
}
fn has_b_print(rows: &[TrackedActivation]) -> bool {
    rows.iter()
        .filter_map(|row| row.batch.as_ref())
        .any(|batch| {
            batch.events().iter().any(|event| {
                matches!(event, EvaluationEvent::StarlarkPrint { text, .. }
                if text.as_str() == "DEFERRED_B_PRINT")
            })
        })
}

#[tokio::test]
async fn observed_independent_literal_rows_union_lstat_prerequisites_and_recover_in_order() {
    let module = "module(name='bazel_tools')\nregister_toolchains('//a:tc','//b:tc')\n";
    let complete = packages(module, &["a", "b"]).build();
    let omitted = [lstat("a"), lstat("b")];
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(
        module,
        without(&complete, &omitted),
        true,
        Some(tracker.dupe()),
    )
    .await;
    let key = ModuleRegistrationExpansionObservationKey::toolchains(workspace());
    let value = tx.compute(&key).await.unwrap();
    pending(&value, &omitted);
    no_family_batch(&tracker, &key);

    let mut tx = update_module_epoch(tx, module, complete.dupe()).await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &value else {
        panic!("complete inputs must recover")
    };
    assert_eq!(labels(observed.result()), ["@@//a:tc", "@@//b:tc"]);
    for name in ["a", "b"] {
        assert!(observed.observations().get(&lstat(name)).is_some());
        assert!(
            observed
                .observations()
                .get(&EpochBuilder::demand(
                    &format!("/workspace/{name}/BUILD.bazel"),
                    PathObservationOperation::FileBytes
                ))
                .is_some()
        );
    }
    for (demand, result) in observed.observations().observations() {
        assert!(Arc::ptr_eq(result, complete.get(demand).unwrap()));
    }
    let activations = tracker.take();
    assert!(
        activation_for(&activations, &key)
            .batch
            .as_ref()
            .unwrap()
            .events()
            .is_empty()
    );
    assert!(ModuleRegistrationExpansionObservationKey::validity(&value));
}

#[tokio::test]
async fn observed_non_path_need_defers_behind_pending_lstat_and_stops_before_c() {
    use slug_bzlmod_v2::RepositoryEnvironmentNameFrontier;
    use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
    use slug_bzlmod_v2::RepositoryEnvironmentSnapshotKey;
    use slug_bzlmod_v2::RepositoryExecuteEpoch;
    use slug_bzlmod_v2::RepositoryExecuteEpochKey;
    use slug_bzlmod_v2::RepositoryHostInputTransaction;
    use slug_bzlmod_v2::RepositoryPlatform;
    use slug_bzlmod_v2::RepositoryPlatformKey;
    use slug_bzlmod_v2::SelectedExtensionDownloadTransaction;

    let module = "module(name='consumer')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e,gen='generated')\nregister_toolchains('//a:tc','@gen//:tc','//c:tc')\n";
    let extension = format!(
        "def write(ctx):\n    ctx.file('BUILD.bazel', {BUILD:?}, executable=False)\nrepo=repository_rule(implementation=write)\ndef impl(ctx):\n    repo(name='generated')\next=module_extension(implementation=impl)\n"
    );
    let mut builder = packages(module, &["a", "c"]);
    builder.file("/workspace/BUILD.bazel", "", 61);
    builder.file("/workspace/ext.bzl", extension, 61);
    let complete = builder.build();
    let tracker = Arc::new(ExpansionTracker::default());
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut seed = dice.updater();
    slug_bzlmod_v2::inject_registry_request_inputs(
        &mut seed,
        workspace().as_path(),
        slug_bzlmod_v2::RegistryUrls::new(["https://registry.invalid"]),
        slug_bzlmod_v2::RegistryRequestGeneration(1),
    )
    .unwrap();
    drop(seed.commit().await);
    let tx = transaction_on(
        &dice,
        module,
        without(&complete, &[lstat("a")]),
        true,
        Some(tracker.dupe()),
    )
    .await;
    // Reuse the established literal repository-rule fixture's explicit empty
    // Host inputs/platform. Preserve the existing tracker/cycle detector; no
    // mapping, materialized output, callback or generated source result is injected.
    let data = {
        let old = tx.per_transaction_data();
        let mut data = UserComputationData {
            tracker: old.tracker.clone(),
            spawner: old.spawner.clone(),
            cycle_detector: old.cycle_detector.clone(),
            activation_tracker: old.activation_tracker.clone(),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        data.data.set(RepositoryHostInputTransaction::new(
            RepositoryEnvironmentSnapshot::empty(),
            RepositoryEnvironmentNameFrontier::empty(),
        ));
        data.data
            .set(SelectedExtensionDownloadTransaction::empty(workspace()));
        data.data.set(RepositoryExecuteEpoch::revoked());
        data
    };
    let mut update = tx.into_updater();
    update
        .changed_to([(
            RepositoryPlatformKey::new(workspace()),
            RepositoryPlatform::new("linux", "x86_64"),
        )])
        .unwrap();
    update
        .changed_to([(
            RepositoryEnvironmentSnapshotKey::new(workspace()),
            RepositoryEnvironmentSnapshot::empty(),
        )])
        .unwrap();
    update
        .changed_to([(
            RepositoryExecuteEpochKey::new(workspace()),
            RepositoryExecuteEpoch::revoked(),
        )])
        .unwrap();
    update
        .changed_to([(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace(),
            },
            RepositoryMaterializationResultEpoch::new(workspace(), []).unwrap(),
        )])
        .unwrap();
    let mut tx = update.commit_with_data(data).await;

    // The missing A fact and unmaterialized generated output must not block
    // the mandatory serial registration-selection prerequisite itself.
    let selected = tx
        .compute(&slug_bzlmod_v2::HostSelectedRegistrationPatternsObservationKey::new(workspace()))
        .await
        .unwrap();
    assert!(
        matches!(&selected, LoadingPreparationOutcome::Complete(Ok(value)) if value.result().is_ok()),
        "registration-selection prerequisite must complete: {selected:?}"
    );
    let mapping = tx
        .compute(&slug_bzlmod_v2::HostRootRepositoryMappingObservationKey::new(workspace()))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(Ok(mapping)) = mapping else {
        panic!("the actual generated root mapping must complete")
    };
    let mapping = mapping.result().as_ref().as_ref().unwrap();
    let generated = mapping
        .view()
        .unwrap()
        .mapping()
        .find_map(|(name, repository)| (name.as_str() == "gen").then(|| repository.clone()))
        .unwrap();

    // Independently establish the real later generated output prerequisite,
    // without manufacturing a Need or running the registration family itself.
    let barrier = tx
        .compute(&crate::HostPackageInventoryObservationKey::new(
            workspace(),
            CanonicalLabel::parse(&format!("@@{}//:tc", generated.as_str()))
                .unwrap()
                .package()
                .clone(),
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Need(barrier) = barrier else {
        panic!("unmaterialized original generated output must suspend: {barrier:?}")
    };
    assert_eq!(barrier.repository_materializations().len(), 1);
    assert!(barrier.path_observations().is_none());
    assert!(matches!(
        &barrier
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .kind,
        RepositoryMaterializationKind::GeneratedFileEffects(_)
    ));
    tracker.take();
    let key = ModuleRegistrationExpansionObservationKey::toolchains(workspace());
    pending(&tx.compute(&key).await.unwrap(), &[lstat("a")]);
    no_c_or_family_batch(&tracker, &key);

    // Once A is supplied, the original barrier is returned unchanged; C still
    // cannot activate or contribute a family event batch.
    let mut tx = update_module_epoch(tx, module, complete).await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Need(actual) = &value else {
        panic!("later generated output barrier must remain a Need")
    };
    assert_eq!(actual, &barrier);
    assert!(!ModuleRegistrationExpansionObservationKey::validity(&value));
    no_c_or_family_batch(&tracker, &key);
}

#[tokio::test]
async fn observed_later_error_defers_behind_pending_lstat_and_earlier_error_wins() {
    let module = "module(name='bazel_tools')\nregister_toolchains('//a:tc','//b:tc','//c:tc')\n";
    let mut builder = packages(module, &["a", "b", "c"]);
    builder.file(
        "/workspace/b/BUILD.bazel",
        "print('DEFERRED_B_PRINT')\nfail('later B error')\n",
        61,
    );
    let complete = builder.build();
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(
        module,
        without(&complete, &[lstat("a")]),
        true,
        Some(tracker.dupe()),
    )
    .await;
    let key = ModuleRegistrationExpansionObservationKey::toolchains(workspace());
    pending(&tx.compute(&key).await.unwrap(), &[lstat("a")]);
    let mut captured = no_c_or_family_batch(&tracker, &key);

    // Capture the actual unselected B child even on the serial baseline. A
    // speculative lookahead may already have done so. Keep its real node and
    // batch as retry-owner lineage; capture alone does not publish the print.
    let child = tx
        .compute(&crate::RootPackageLoadObservationKey::new(
            workspace(),
            PackagePath::parse("b").unwrap(),
        ))
        .await
        .unwrap();
    assert!(
        matches!(child, LoadingPreparationOutcome::Complete(Ok(value)) if value.result().is_err())
    );
    captured.extend(tracker.take());
    let marker_nodes = captured
        .iter()
        .filter(|row| has_b_print(std::slice::from_ref(row)))
        .map(|row| row.node)
        .collect::<Vec<_>>();
    assert!(!marker_nodes.is_empty());

    let failed_a =
        PathObservationEpoch::from_shared(complete.observations().iter().map(|(d, r)| {
            (
                d.dupe(),
                if d == &lstat("a") {
                    Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing))
                } else {
                    r.dupe()
                },
            )
        }))
        .unwrap();
    let mut tx = update_module_epoch(tx, module, failed_a.dupe()).await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("observed missing A must terminate")
    };
    let error = observed.result().labels().unwrap_err();
    assert_eq!(error.row(), Some(0));
    assert!(matches!(
        error.kind(),
        ModuleRegistrationExpansionErrorKind::RootPackage(_)
    ));
    assert!(
        observed
            .observations()
            .get(&EpochBuilder::demand(
                "/workspace/b/BUILD.bazel",
                PathObservationOperation::FileBytes
            ))
            .is_none()
    );
    for (demand, result) in observed.observations().observations() {
        assert!(Arc::ptr_eq(result, failed_a.get(demand).unwrap()));
    }
    let rows = tracker.take();
    assert_no_activation_containing(&rows, "host-package-load:\"/workspace\"//c");
    let root = activation_for(&rows, &key).node;
    let closure = tx.activation_closure([root]).await.unwrap();
    assert!(
        marker_nodes
            .iter()
            .all(|marker| { !closure.nodes().iter().any(|node| node.node() == *marker) })
    );

    // A successful retry now reaches B's warm error and the same real captured
    // batch. Exact-version reachability composes with Core's selected-closure
    // event owner; this does not claim fresh-command stdout replay.
    let mut tx = update_module_epoch(tx, module, complete.dupe()).await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("completed A must expose the later B error")
    };
    let error = observed.result().labels().unwrap_err();
    assert_eq!(error.row(), Some(1));
    assert!(matches!(
        error.kind(),
        ModuleRegistrationExpansionErrorKind::RootPackage(_)
    ));
    for (demand, result) in observed.observations().observations() {
        assert!(Arc::ptr_eq(result, complete.get(demand).unwrap()));
    }
    let rows = tracker.take();
    assert_no_activation_containing(&rows, "host-package-load:\"/workspace\"//c");
    let root = activation_for(&rows, &key).node;
    let closure = tx.activation_closure([root]).await.unwrap();
    assert!(
        marker_nodes
            .iter()
            .any(|marker| { closure.nodes().iter().any(|node| node.node() == *marker) })
    );
    assert!(
        rows.iter()
            .any(|row| { marker_nodes.contains(&row.node) && row.kind == ActivationKind::Reused })
    );
    assert!(
        has_b_print(&captured),
        "the actual warm batch remains owned"
    );
}

#[tokio::test]
async fn observed_recursive_partial_row_discards_scratch_before_lstat_window_retry() {
    let module = "module(name='consumer')\n";
    let tree = EpochBuilder::root_tree(module, BUILD, 61);
    let mut extra = packages(module, &["later", "defs"]);
    extra.file("/workspace/defs/marker.bzl", "VALUE=1\n", 61);
    let extra = extra.build();
    let mut complete = tree.try_union(&extra).unwrap();
    complete = PathObservationEpoch::from_shared(complete.observations().iter().map(|(d, r)| {
        (
            d.dupe(),
            if d == &EpochBuilder::demand(
                "/workspace/tree/b/BUILD.bazel",
                PathObservationOperation::FileBytes,
            ) {
                Arc::new(PathObservationResult::FileBytes(
                    PathOperationResult::Present(Arc::from(
                        format!("load('//defs:marker.bzl','VALUE')\n{BUILD}").as_bytes(),
                    )),
                ))
            } else {
                r.dupe()
            },
        )
    }))
    .unwrap();
    let omitted = [
        EpochBuilder::demand(
            "/workspace/defs/marker.bzl",
            PathObservationOperation::Lstat,
        ),
        lstat("later"),
    ];
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut seed = dice.updater();
    slug_bzlmod_v2::inject_registry_request_inputs(
        &mut seed,
        workspace().as_path(),
        slug_bzlmod_v2::RegistryUrls::new(["https://registry.invalid"]),
        slug_bzlmod_v2::RegistryRequestGeneration(1),
    )
    .unwrap();
    drop(seed.commit().await);
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_on(
        &dice,
        module,
        without(&complete, &omitted),
        true,
        Some(tracker.dupe()),
    )
    .await;
    // Command toolchain rows reverse before expansion, so the recursive row
    // precedes later. This isolates the modified loop without expanding the
    // builtin module's independent registered toolchains after our own rows.
    let configuration = command_configuration([CommandConfigurationOccurrence::extra_toolchains(
        "//later:tc,//tree/...:all",
    )]);
    let key = CommandRegistrationExpansionObservationKey::toolchains(workspace(), configuration);
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Need(needs) = &value else {
        panic!("recursive command prerequisites must suspend")
    };
    assert_eq!(
        needs,
        &slug_bzlmod_v2::SourcePreparationNeeds::path(
            slug_workspace_v2::NeedPathObservations::try_from_iter(omitted.iter().cloned())
                .unwrap()
        )
    );
    assert!(!CommandRegistrationExpansionObservationKey::validity(
        &value
    ));
    assert!(!CommandRegistrationExpansionObservationKey::equality(
        &value, &value
    ));
    let rows = tracker.take();
    assert!(
        rows.iter().any(|row| row
            .key
            .contains("host-package-load:\"/workspace\"//tree/b/deep")
            && row.kind == ActivationKind::Evaluated
            && row.batch.is_some()),
        "the earlier recursive package must actually complete before the hole"
    );
    assert!(
        rows.iter()
            .filter(|row| row.key == key.to_string())
            .all(|row| row.batch.is_none())
    );
    let mut tx = update_module_epoch(tx, module, complete.dupe()).await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("complete recursive retry must finish: {value:?}")
    };
    assert_eq!(
        labels(observed.result()),
        [
            "@@//tree/b/deep:tc",
            "@@//tree/b:tc",
            "@@//tree/c:tc",
            "@@//tree:tc",
            "@@//later:tc"
        ]
    );
    for (d, r) in observed.observations().observations() {
        assert!(Arc::ptr_eq(r, complete.get(d).unwrap()));
    }
}

#[tokio::test]
async fn observed_command_pending_rows_preserve_signed_order_and_toolchain_reversal() {
    let module = "module(name='bazel_tools')\n";
    let mut builder = packages(module, &["a", "b"]);
    for name in ["a", "b"] {
        builder.file(
            &format!("/workspace/{name}/BUILD.bazel"),
            format!("{BUILD}platform(name='platform')\n"),
            61,
        );
    }
    let complete = builder.build();
    let omitted = [lstat("a"), lstat("b")];
    for toolchains in [true, false] {
        let configuration = if toolchains {
            command_configuration([CommandConfigurationOccurrence::extra_toolchains(
                "//a:tc,//b:tc,-//a:tc",
            )])
        } else {
            command_configuration([CommandConfigurationOccurrence::extra_execution_platforms(
                "//a:platform,//b:platform,-//a:platform",
            )])
        };
        let key = if toolchains {
            CommandRegistrationExpansionObservationKey::toolchains(workspace(), configuration)
        } else {
            CommandRegistrationExpansionObservationKey::execution_platforms(
                workspace(),
                configuration,
            )
        };
        let tracker = Arc::new(ExpansionTracker::default());
        let mut tx = transaction_with_tracker(
            module,
            without(&complete, &omitted),
            true,
            Some(tracker.dupe()),
        )
        .await;
        let value = tx.compute(&key).await.unwrap();
        let LoadingPreparationOutcome::Need(needs) = &value else {
            panic!("both command rows must suspend")
        };
        assert_eq!(
            needs,
            &slug_bzlmod_v2::SourcePreparationNeeds::path(
                slug_workspace_v2::NeedPathObservations::try_from_iter(omitted.iter().cloned())
                    .unwrap()
            )
        );
        assert!(!CommandRegistrationExpansionObservationKey::validity(
            &value
        ));
        assert!(
            tracker
                .take()
                .iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
        let mut tx = update_module_epoch(tx, module, complete.dupe()).await;
        let value = tx.compute(&key).await.unwrap();
        let LoadingPreparationOutcome::Complete(Ok(observed)) = value else {
            panic!("command retry must finish")
        };
        let expected = if toolchains {
            vec!["@@//b:tc", "@@//a:tc"]
        } else {
            vec!["@@//b:platform"]
        };
        assert_eq!(labels(observed.result()), expected);
        for (d, r) in observed.observations().observations() {
            assert!(Arc::ptr_eq(r, complete.get(d).unwrap()));
        }
    }
}
