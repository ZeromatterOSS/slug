use slug_workspace_v2::WorkspaceDirectorySnapshotKey;
use slug_workspace_v2::WorkspaceSnapshotKey;

use super::*;

#[tokio::test]
async fn automatic_sources_keep_active_and_retired_real_outputs_out_of_host_views() {
    let workspace = crate::scratch();
    crate::ensure_host_platform_fixture(&workspace);
    std::fs::create_dir_all(workspace.join(".slug_generated_registration_authored")).unwrap();
    std::fs::write(
        workspace.join(".slug_generated_registration_authored/owned.txt"),
        "authored",
    )
    .unwrap();
    // Shared ancestry plus root BUILD/lock and caller Missing overrides exercise
    // full old-vs-new Epoch equality, not only record counts or path keys.
    std::fs::create_dir_all(workspace.join("BUILD/shared/empty")).unwrap();
    std::fs::write(workspace.join("BUILD/shared/a.txt"), "a").unwrap();
    std::fs::write(workspace.join("BUILD/shared/b.txt"), "b").unwrap();
    std::fs::write(workspace.join("BUILD/shared/BUILD.bazel"), "# authored").unwrap();
    std::fs::write(workspace.join("MODULE.bazel.lock"), "authored lock").unwrap();
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let (tx, first) = again(
        &dice,
        &workspace,
        &need(&root, b"A", false),
        host(&workspace).0,
    )
    .await;
    drop(tx);
    let first_root = immutable(&first).1.to_path_buf();
    let mut updater = dice.updater();
    let mut saved = capture(&updater, &workspace).await.unwrap();
    let mut entries = saved
        .inject_initial(
            &mut updater,
            &workspace,
            Vec::new(),
            FixtureObservations::Automatic,
        )
        .unwrap();
    let mut tx = updater.commit().await;
    let before = tx
        .compute(&RepositoryMaterializationResultEpochKey {
            workspace: root.clone(),
        })
        .await
        .unwrap();
    let source_a = tx
        .compute(&WorkspaceSnapshotKey {
            workspace: workspace.clone(),
        })
        .await
        .unwrap();
    assert!(!source_a.files.keys().any(|p| p.starts_with(&first_root)));
    assert!(
        source_a
            .files
            .contains_key(&workspace.join(".slug_generated_registration_authored/owned.txt"))
    );
    let directory_a = tx
        .compute(&WorkspaceDirectorySnapshotKey {
            workspace: workspace.clone(),
        })
        .await
        .unwrap();
    for missing in [
        Vec::new(),
        vec![workspace.join("BUILD/shared"), workspace.join("absent")],
    ] {
        assert_eq!(
            crate::observation_epoch_from_snapshot(&workspace, &source_a, missing.clone()),
            crate::observation_epoch_from_snapshot_inner(&workspace, &source_a, missing, false),
            "complete original Epoch including overwrite order is unchanged",
        );
    }
    let mut exhausted = super::super::super::Budget {
        records: 65_536,
        bytes: 0,
    };
    let source_guards = Arc::strong_count(&source_a);
    assert!(
        super::super::initial_inputs::host_epoch(
            &workspace,
            &source_a,
            &directory_a,
            &mut exhausted,
        )
        .is_err()
    );
    assert_eq!(Arc::strong_count(&source_a), source_guards);
    assert!(!directory_a.directories.contains_key(&first_root));
    assert!(
        !directory_a
            .directories
            .get(&workspace)
            .unwrap()
            .entries()
            .unwrap()
            .iter()
            .any(|e| e.name.as_str() == first_root.file_name().unwrap().to_str().unwrap())
    );
    let SavedInputs {
        mut prior,
        mut budget,
        floor,
        explicit,
    } = saved;
    tx = super::super::super::materialize(
        tx,
        &workspace,
        &root,
        &need(&root, b"B", true),
        &mut entries,
        &mut budget,
        &mut prior,
        floor,
        &explicit,
    )
    .await
    .unwrap();
    let active_b = tx
        .compute(&RepositoryMaterializationResultEpochKey {
            workspace: root.clone(),
        })
        .await
        .unwrap();
    assert_ne!(before, active_b);
    assert_eq!(
        active_b
            .borrowed_retained_generated(&root, 2)
            .unwrap()
            .len(),
        2
    );
    let b = entries
        .iter()
        .find(|e| e.request.id.canonical_repo == first.request.id.canonical_repo)
        .unwrap();
    let second_root = immutable(b).1.to_path_buf();
    assert_ne!(first_root, second_root);
    assert!(
        matches!(
            slug_bzlmod_v2::source_preparation::compute_repository_materialization_result(
                &mut tx,
                first.request.clone()
            )
            .await
            .unwrap(),
            SourcePreparationOutcome::Need(_)
        ),
        "retired A is not an active successful request"
    );
    drop(tx);
    drop(prior);
    let mut updater = dice.updater();
    let mut saved = capture(&updater, &workspace).await.unwrap();
    let mut entries = saved
        .inject_initial(
            &mut updater,
            &workspace,
            Vec::new(),
            FixtureObservations::Automatic,
        )
        .unwrap();
    let mut tx = updater.commit().await;
    let source_b = tx
        .compute(&WorkspaceSnapshotKey {
            workspace: workspace.clone(),
        })
        .await
        .unwrap();
    assert_eq!(
        source_a, source_b,
        "active and retired resource trees never become authored Host source"
    );
    assert!(
        !source_b
            .files
            .keys()
            .any(|p| p.starts_with(&first_root) || p.starts_with(&second_root))
    );
    let actual = tx
        .compute(&RepositoryMaterializationResultEpochKey {
            workspace: root.clone(),
        })
        .await
        .unwrap();
    assert_eq!(
        active_b, actual,
        "initial commit keeps complete active+retired input declaration"
    );
    assert!(
        RepositoryMaterializationResultEpoch::new(root.clone(), entries.clone())
            .unwrap()
            .retaining_generated_outputs(Some(&before), 1)
            .is_err()
    );
    assert!(
        RepositoryMaterializationResultEpoch::new(root.clone(), entries.clone())
            .unwrap()
            .retaining_generated_outputs(Some(&before), 2)
            .is_ok()
    );
    // Restore A through a new genuine writer incarnation. The retired first A
    // remains only a physical root proof; it cannot silently serve the request.
    let SavedInputs {
        mut prior,
        mut budget,
        floor,
        explicit,
    } = saved;
    tx = super::super::super::materialize(
        tx,
        &workspace,
        &root,
        &need(&root, b"A", false),
        &mut entries,
        &mut budget,
        &mut prior,
        floor,
        &explicit,
    )
    .await
    .unwrap();
    let restored = tx
        .compute(&RepositoryMaterializationResultEpochKey {
            workspace: root.clone(),
        })
        .await
        .unwrap();
    assert_eq!(
        restored
            .borrowed_retained_generated(&root, 3)
            .unwrap()
            .len(),
        3
    );
    drop(tx);
    drop(prior);
    let mut updater = dice.updater();
    let mut saved = capture(&updater, &workspace).await.unwrap();
    let entries = saved
        .inject_initial(
            &mut updater,
            &workspace,
            Vec::new(),
            FixtureObservations::Automatic,
        )
        .unwrap();
    let mut tx = updater.commit().await;
    let source_restored = tx
        .compute(&WorkspaceSnapshotKey {
            workspace: workspace.clone(),
        })
        .await
        .unwrap();
    assert_eq!(source_restored, source_a);
    // A caller may explicitly activate an older genuine incarnation while a
    // newer retired root remains owned. Host-only observations cannot release
    // that higher ID for a future writer to issue again.
    let highest = restored
        .borrowed_retained_generated(&root, 3)
        .unwrap()
        .iter()
        .map(|entry| immutable(entry).2.value())
        .max()
        .unwrap();
    assert!(highest > immutable(&first).2.value());
    drop(tx);
    drop(saved);
    let older_active = RepositoryMaterializationResultEpoch::new(root.clone(), [first.clone()])
        .unwrap()
        .retaining_generated_outputs(Some(&restored), 3)
        .unwrap();
    let mut updater = dice.updater();
    updater
        .changed_to([(
            RepositoryMaterializationResultEpochKey {
                workspace: root.clone(),
            },
            older_active,
        )])
        .unwrap();
    updater
        .changed_to([(PathObservationEpochKey, host(&workspace).0)])
        .unwrap();
    let tx = updater.commit().await;
    drop(tx);
    let saved = capture(&dice.updater(), &workspace).await.unwrap();
    assert_eq!(saved.floor, highest);
    std::fs::write(
        workspace.join(".slug_generated_registration_authored/owned.txt"),
        "authored changed",
    )
    .unwrap();
    drop(saved);
    let mut updater = dice.updater();
    let mut saved = capture(&updater, &workspace).await.unwrap();
    saved
        .inject_initial(
            &mut updater,
            &workspace,
            Vec::new(),
            FixtureObservations::Automatic,
        )
        .unwrap();
    let mut tx = updater.commit().await;
    assert_ne!(
        tx.compute(&WorkspaceSnapshotKey {
            workspace: workspace.clone()
        })
        .await
        .unwrap(),
        source_a,
        "ordinary authored changes remain source inputs"
    );
    let guards = Arc::strong_count(&entries[0].request);
    assert!(restored.borrowed_retained_generated(&root, 2).is_err());
    assert_eq!(Arc::strong_count(&entries[0].request), guards);
    drop(tx);
    drop(saved);
    drop(dice);
    std::fs::remove_dir_all(workspace).unwrap();
}

#[tokio::test]
async fn explicit_epoch_missing_and_canonical_declarations_override_saved_inputs() {
    let workspace = crate::scratch();
    crate::ensure_host_platform_fixture(&workspace);
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let (tx, first) = again(
        &dice,
        &workspace,
        &need(&root, b"A", false),
        host(&workspace).0,
    )
    .await;
    drop(tx);
    let (_, generation, instance) = immutable(&first);
    let demand = slug_workspace_v2::PathObservationDemand::new(
        PathObservationNamespace::Materialization(instance),
        NormalizedAbsolutePath::new(generation.join("marker")).unwrap(),
        slug_workspace_v2::PathObservationOperation::FileBytes,
    );
    let missing = Arc::new(slug_workspace_v2::PathObservationResult::FileBytes(
        slug_workspace_v2::PathOperationResult::Missing,
    ));
    let explicit = PathObservationEpoch::from_shared([(demand.clone(), missing.clone())]).unwrap();
    let mut updater = dice.updater();
    let mut saved = capture(&updater, &workspace).await.unwrap();
    let mut declared = first.clone();
    declared.result = RepositoryMaterializationResult::SpecError("explicit terminal".into());
    let entries = saved
        .inject_initial(
            &mut updater,
            &workspace,
            vec![declared.clone()],
            FixtureObservations::Explicit(explicit),
        )
        .unwrap();
    assert_eq!(entries, vec![declared]);
    let mut tx = updater.commit().await;
    let epoch = tx.compute(&PathObservationEpochKey).await.unwrap();
    assert_eq!(epoch.observations().len(), 1);
    assert!(Arc::ptr_eq(epoch.get(&demand).unwrap(), &missing));
    assert!(saved.explicit.contains(&first.request.id.canonical_repo));
    let full_source = tx
        .compute(&WorkspaceSnapshotKey {
            workspace: workspace.clone(),
        })
        .await
        .unwrap();
    assert!(
        full_source.files.contains_key(&generation.join("marker")),
        "explicit source view is unfiltered"
    );
    drop(tx);
    drop(saved);
    drop(dice);
    std::fs::remove_dir_all(workspace).unwrap();
}

#[tokio::test]
async fn initial_inventory_preflight_refuses_before_guard_clones_or_updater_commit() {
    let workspace = crate::scratch();
    crate::ensure_host_platform_fixture(&workspace);
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let (tx, first) = again(
        &dice,
        &workspace,
        &need(&root, b"A", false),
        host(&workspace).0,
    )
    .await;
    drop(tx);
    for full_bytes in [false, true] {
        let mut updater = dice.updater();
        let mut saved = capture(&updater, &workspace).await.unwrap();
        let guards = Arc::strong_count(&first.request);
        if full_bytes {
            saved.budget.bytes = 64 * 1024 * 1024;
        } else {
            saved.budget.records = 65_536;
        }
        assert!(
            saved
                .inject_initial(
                    &mut updater,
                    &workspace,
                    Vec::new(),
                    FixtureObservations::Automatic
                )
                .is_err()
        );
        assert_eq!(Arc::strong_count(&first.request), guards);
    }
    drop(dice);
    std::fs::remove_dir_all(workspace).unwrap();
}

#[tokio::test]
async fn inventory_full_foreign_equality_and_exact_limit_preserve_original_guards() {
    let workspace = crate::scratch();
    crate::ensure_host_platform_fixture(&workspace);
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let (tx, first) = again(
        &dice,
        &workspace,
        &need(&root, b"A", false),
        host(&workspace).0,
    )
    .await;
    drop(tx);
    let empty =
        RepositoryMaterializationResultEpoch::new(root.clone(), vec![first.clone()]).unwrap();
    assert!(
        empty
            .borrowed_retained_generated(&root, 0)
            .unwrap()
            .is_empty(),
        "existing constructor stays empty"
    );
    let one = empty.retaining_generated_outputs(None, 1).unwrap();
    let current_dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let mut updater = current_dice.updater();
    let mut saved = capture(&updater, &workspace).await.unwrap();
    assert!(saved.prior.is_none());
    saved
        .inject_initial(
            &mut updater,
            &workspace,
            vec![first.clone()],
            FixtureObservations::Automatic,
        )
        .unwrap();
    let mut current_tx = updater.commit().await;
    assert!(
        !current_tx
            .compute(&WorkspaceSnapshotKey {
                workspace: workspace.clone()
            })
            .await
            .unwrap()
            .files
            .contains_key(&immutable(&first).1.join("marker")),
        "current genuine declaration is classified before source capture"
    );
    drop(current_tx);
    drop(saved);
    drop(current_dice);

    let mut foreign = first.clone();
    foreign.request = need(&root, b"A", false)
        .repository_materializations()
        .values()
        .next()
        .unwrap()
        .clone();
    assert!(!Arc::ptr_eq(&foreign.request, &first.request));
    let equal = RepositoryMaterializationResultEpoch::new(root.clone(), vec![foreign.clone()])
        .unwrap()
        .retaining_generated_outputs(Some(&one), 1)
        .unwrap();
    assert_eq!(one, equal);
    assert!(Arc::ptr_eq(
        &one.borrowed_retained_generated(&root, 1).unwrap()[0],
        &equal.borrowed_retained_generated(&root, 1).unwrap()[0]
    ));
    let mut changed = foreign.request.as_ref().clone();
    changed.repo_spec.rule_id.rule_name.push_str("_changed");
    foreign.request = Arc::new(changed);
    assert!(
        RepositoryMaterializationResultEpoch::new(root.clone(), vec![foreign])
            .unwrap()
            .retaining_generated_outputs(Some(&one), 1)
            .is_err(),
        "same physical incarnation with changed complete request is a conflict"
    );
    assert!(
        RepositoryMaterializationResultEpoch::new(root.clone(), vec![first])
            .unwrap()
            .retaining_generated_outputs(None, 0)
            .is_err()
    );
    drop(dice);
    std::fs::remove_dir_all(workspace).unwrap();
}
