use super::*;

fn request_module(rows: &[(&str, &str)]) -> String {
    format!(
        "module(name='consumer')\n{}",
        rows.iter()
            .enumerate()
            .map(|(index, (file, export))| format!(
                "e{index}=use_extension('//:{file}','{export}')\n"
            ))
            .collect::<String>()
    )
}

async fn missing_bytes(
    transaction: DiceTransaction,
    full: &PathObservationEpoch,
    missing: &[&str],
) -> DiceTransaction {
    let filtered = PathObservationEpoch::from_shared(
        full.observations()
            .iter()
            .filter(|(demand, _)| {
                !(demand.operation() == PathObservationOperation::FileBytes
                    && missing
                        .iter()
                        .any(|name| demand.path().as_path().ends_with(name)))
            })
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .unwrap();
    let mut updater = transaction.into_updater();
    updater
        .changed_to([(PathObservationEpochKey, filtered)])
        .unwrap();
    updater.commit().await
}

fn assert_need_paths<T: std::fmt::Debug>(outcome: &SourcePreparationOutcome<T>, names: &[&str]) {
    let SourcePreparationOutcome::Need(need) = outcome else {
        panic!("expected Need: {outcome:?}")
    };
    let expected = NeedPathObservations::try_from_iter(names.iter().map(|name| {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}")).unwrap(),
            PathObservationOperation::FileBytes,
        )
    }))
    .unwrap();
    assert_eq!(
        need.path_observations().unwrap().demands(),
        expected.demands()
    );
    assert!(need.repository_materializations().is_empty());
}

fn assert_no_upper(rows: &[BzlActivationRow]) {
    assert!(
        rows.iter().all(|row| ![
            "host-prepared-module-extension-inputs:",
            "observed-host-prepared-module-extension-inputs:",
            "host-pure-module-extension-invocations:",
            "host-instantiated-module-extension-repositories:",
            "host-validated-module-extension-repositories:",
            "host-generated-repository-definition:",
        ]
        .iter()
        .any(|prefix| row.key.starts_with(prefix))),
        "{rows:?}"
    );
}

#[tokio::test]
async fn observed_extension_definition_frontier_batches_repairs_and_preserves_legacy() {
    assert_loaded_definition_production_slice_has_no_upper_keys();
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let key = HostLoadedModuleExtensionDefinitionsObservationKey::new_with_scope(
        workspace.dupe(),
        slug_bzlmod_v2::HostModuleExtensionUsageScope::RootUsages,
    );
    let legacy_key = HostLoadedModuleExtensionDefinitionsKey::new_with_scope(
        workspace,
        slug_bzlmod_v2::HostModuleExtensionUsageScope::RootUsages,
    );
    for order in [["ext.bzl", "other.bzl"], ["other.bzl", "ext.bzl"]] {
        let dice = builtin_environment_dice();
        let tracker = Arc::new(BzlEventTracker::default());
        let module = request_module(&[(order[0], "ext"), (order[1], "ext")]);
        let mut transaction = case_transaction_with_other(
            &dice,
            &module,
            &loaded_test_source("A"),
            "",
            &loaded_test_source("B"),
            true,
            Some(tracker.clone()),
        )
        .await;
        let full = transaction.compute(&PathObservationEpochKey).await.unwrap();
        transaction = missing_bytes(transaction, &full, &order).await;
        let legacy_need = transaction.compute(&legacy_key).await.unwrap();
        assert_need_paths(&legacy_need, &[order[0]]);
        let (legacy_rows, _) = tracker.take_rows();
        assert!(
            !legacy_rows
                .iter()
                .any(|row| row.key.starts_with("bzlmod-host-bzl-module:")
                    && row.key.ends_with(&format!("//:{}", order[1])))
        );
        let need = transaction.compute(&key).await.unwrap();
        assert_need_paths(&need, &["ext.bzl", "other.bzl"]);
        assert!(!HostLoadedModuleExtensionDefinitionsObservationKey::validity(&need));
        let (rows, _) = tracker.take_rows();
        assert_no_upper(&rows);
        assert!(rows.iter().any(|row| row.key == key.to_string()));
        assert!(
            rows.iter()
                .all(|row| row.key != key.to_string() || row.batch.is_none())
        );
        for name in order {
            assert!(rows.iter().any(
                |row| row.key.starts_with("observed-bzlmod-host-bzl-module:")
                    && row.key.ends_with(&format!("//:{name}"))
            ));
        }
        transaction = missing_bytes(transaction, &full, &[order[1]]).await;
        let partial = transaction.compute(&key).await.unwrap();
        assert_need_paths(&partial, &[order[1]]);
        let (rows, _) = tracker.take_rows();
        assert_no_upper(&rows);
        assert!(
            rows.iter()
                .all(|row| row.key != key.to_string() || row.batch.is_none())
        );
        transaction = missing_bytes(transaction, &full, &[]).await;
        let complete = transaction.compute(&key).await.unwrap();
        let value = observed_loaded(&complete);
        let definitions = value.result().as_ref().as_ref().unwrap();
        assert_eq!(
            definitions
                .definitions
                .iter()
                .map(|definition| definition.request.parts().0.target().as_str())
                .collect::<Vec<_>>(),
            order
        );
        for (demand, result) in value.observations().observations() {
            assert!(Arc::ptr_eq(result, full.get(demand).unwrap()));
        }
        let weak = Arc::downgrade(value.result());
        let legacy = transaction.compute(&legacy_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy_value) = &legacy else {
            panic!("legacy repair")
        };
        assert_eq!(legacy_value.as_ref(), value.result().as_ref());
        tracker.take();
        tracker.take_rows();
        let warm = transaction.compute(&key).await.unwrap();
        assert!(Arc::ptr_eq(value.result(), observed_loaded(&warm).result()));
        let (rows, _) = tracker.take_rows();
        assert_no_upper(&rows);
        assert!(
            rows.iter()
                .all(|row| row.kind == ActivationKind::Reused && row.batch.is_none())
        );
        assert!(tracker.take().is_empty());
        drop(warm);
        drop(complete);
        drop(legacy);
        drop(transaction);
        drop(dice);
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while weak.strong_count() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(weak.upgrade().is_none());
    }
    let dice = builtin_environment_dice();
    let source = format!(
        "{}alternate=module_extension(implementation=implementation)\n",
        loaded_test_source("")
    );
    let module = request_module(&[("ext.bzl", "ext"), ("ext.bzl", "alternate")]);
    let mut transaction =
        case_transaction_with_other(&dice, &module, &source, "", "", true, None).await;
    let full = transaction.compute(&PathObservationEpochKey).await.unwrap();
    transaction = missing_bytes(transaction, &full, &["ext.bzl"]).await;
    assert_need_paths(&transaction.compute(&key).await.unwrap(), &["ext.bzl"]);
    transaction = missing_bytes(transaction, &full, &[]).await;
    let repaired = transaction.compute(&key).await.unwrap();
    assert_eq!(
        observed_loaded(&repaired)
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .definitions
            .len(),
        2
    );
}

#[tokio::test]
async fn observed_extension_definition_frontier_preserves_semantic_barriers() {
    let key = HostLoadedModuleExtensionDefinitionsObservationKey::new_with_scope(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        slug_bzlmod_v2::HostModuleExtensionUsageScope::RootUsages,
    );
    for (source, export, kind) in [
        ("def broken(\n".to_owned(), "ext", "Bzl"),
        ("ext=7\n".to_owned(), "ext", "WrongKind"),
        (loaded_test_source(""), "missing", "Export"),
    ] {
        for error_first in [true, false] {
            let dice = builtin_environment_dice();
            let tracker = Arc::new(BzlEventTracker::default());
            let rows = if error_first {
                [
                    ("other.bzl", export),
                    ("ext.bzl", "ext"),
                    ("child.bzl", "ext"),
                ]
            } else {
                [
                    ("ext.bzl", "ext"),
                    ("other.bzl", export),
                    ("child.bzl", "ext"),
                ]
            };
            let module = request_module(&rows);
            let mut transaction = case_transaction_with_other(
                &dice,
                &module,
                &loaded_test_source(""),
                &loaded_test_source(""),
                &source,
                true,
                Some(tracker.clone()),
            )
            .await;
            let full = transaction.compute(&PathObservationEpochKey).await.unwrap();
            transaction = missing_bytes(transaction, &full, &["ext.bzl", "child.bzl"]).await;
            let first = transaction.compute(&key).await.unwrap();
            if error_first {
                assert!(observed_loaded(&first).result().is_err());
            } else {
                assert_need_paths(&first, &["ext.bzl"]);
            }
            let (activations, _) = tracker.take_rows();
            assert_no_upper(&activations);
            assert!(!activations.iter().any(|row| {
                row.key.starts_with("observed-bzlmod-host-bzl-module:")
                    && row.key.ends_with("//:child.bzl")
            }));
            if error_first {
                assert!(!activations.iter().any(|row| {
                    row.key.starts_with("observed-bzlmod-host-bzl-module:")
                        && row.key.ends_with("//:ext.bzl")
                }));
            } else {
                assert!(
                    activations
                        .iter()
                        .all(|row| row.key != key.to_string() || row.batch.is_none())
                );
            }
            transaction = missing_bytes(transaction, &full, &[]).await;
            let repaired = transaction.compute(&key).await.unwrap();
            let Err(HostLoadedModuleExtensionDefinitionsError::Request { request, error, .. }) =
                observed_loaded(&repaired).result().as_ref()
            else {
                panic!("expected original ordered error")
            };
            assert_eq!(request.parts().0.target().as_str(), "other.bzl");
            assert!(matches!(
                (kind, error),
                ("Bzl", HostLoadedModuleExtensionDefinitionError::Bzl { .. })
                    | (
                        "WrongKind",
                        HostLoadedModuleExtensionDefinitionError::WrongKind { .. }
                    )
                    | (
                        "Export",
                        HostLoadedModuleExtensionDefinitionError::Export { .. }
                    )
            ));
            let (activations, _) = tracker.take_rows();
            assert_no_upper(&activations);
            assert!(!activations.iter().any(|row| {
                row.key.starts_with("observed-bzlmod-host-bzl-module:")
                    && row.key.ends_with("//:child.bzl")
            }));
        }
    }
}

#[test]
fn extension_definition_frontier_conflict_preserves_earlier_need() {
    let original = bv::RepositoryMaterializationRequest {
        id: bv::RepositoryMaterializationRequestId {
            workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
        },
        repo_spec: bv::RepoSpec {
            rule_id: bv::RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
                    .unwrap(),
                rule_name: "local_repository".into(),
            },
            attributes: Arc::new(SmallMap::new()),
        },
        kind: bv::RepositoryMaterializationKind::Local {
            logical_root: NormalizedAbsolutePath::new("/earlier").unwrap(),
        },
    };
    let mut conflicting = original.clone();
    conflicting.kind = bv::RepositoryMaterializationKind::Local {
        logical_root: NormalizedAbsolutePath::new("/later").unwrap(),
    };
    let earlier = SourcePreparationNeeds::repository(original.clone());
    let retained = earlier
        .repository_materializations()
        .values()
        .next()
        .unwrap()
        .clone();
    let weak = Arc::downgrade(&retained);
    let mut needs = Some(earlier);
    assert!(
        try_accumulate_external_bzl_need(
            &mut needs,
            SourcePreparationNeeds::repository(conflicting)
        )
        .is_err()
    );
    assert!(Arc::ptr_eq(
        needs
            .as_ref()
            .unwrap()
            .repository_materializations()
            .values()
            .next()
            .unwrap(),
        &retained
    ));
    try_accumulate_external_bzl_need(
        &mut needs,
        SourcePreparationNeeds::repository(original.clone()),
    )
    .unwrap();
    assert!(Arc::ptr_eq(
        needs
            .as_ref()
            .unwrap()
            .repository_materializations()
            .values()
            .next()
            .unwrap(),
        &retained
    ));
    let mut compatible = original;
    compatible.id.canonical_repo = CanonicalRepoName::new("second+").unwrap();
    try_accumulate_external_bzl_need(&mut needs, SourcePreparationNeeds::repository(compatible))
        .unwrap();
    assert_eq!(
        needs.as_ref().unwrap().repository_materializations().len(),
        2
    );
    drop(needs);
    drop(retained);
    assert!(weak.upgrade().is_none());
}
