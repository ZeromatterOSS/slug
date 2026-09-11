use super::*;
async fn registration_error_fixture(
    dice: &Arc<dice::Dice>,
    pattern: &str,
) -> Arc<slug_loading_v2::ModuleRegistrationExpansionError> {
    use slug_bzlmod_v2::*;
    use slug_workspace_v2::*;
    use starlark_map::sorted_map::SortedMap;
    let workspace = NormalizedAbsolutePath::new("/registration-errors").unwrap();
    let module = format!("module(name='bazel_tools')\nregister_toolchains({pattern:?})\n");
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::empty(),
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.as_path().to_owned(),
            },
            Arc::new(WorkspaceSnapshot {
                files: Arc::new(SortedMap::from_iter([(
                    workspace.as_path().join("MODULE.bazel"),
                    WorkspaceFileValue::Present(Arc::new(module)),
                )])),
            }),
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceRawSnapshotKey {
                workspace: workspace.as_path().to_owned(),
            },
            Arc::new(WorkspaceRawSnapshot {
                files: Arc::new(SortedMap::from_iter([(
                    workspace.as_path().join("MODULE.bazel.lock"),
                    WorkspaceRawFileValue::Absent,
                )])),
            }),
        )])
        .unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            workspace.clone(),
            [workspace.clone()],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        workspace.as_path(),
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let mut tx = updater.commit().await;
    let outcome = tx
        .compute(&slug_loading_v2::ModuleRegistrationExpansionKey::toolchains(workspace))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("fixture returned Need")
    };
    let error = value.labels_with_shared_error().unwrap_err();
    assert!(matches!(
        error.kind(),
        slug_loading_v2::ModuleRegistrationExpansionErrorKind::Parse(_)
    ));
    Arc::clone(error)
}

#[tokio::test]
async fn registration_error_completed_core_identity_and_output_envelope() {
    let dice = Arc::new(dice::Dice::builder().build(dice::DetectCycles::Enabled));
    let pattern = format!("@{}A", "\\\"".repeat(4096));
    let first = registration_error_fixture(&dice, &pattern).await;
    let weak = Arc::downgrade(&first);
    let wrap =
        |error| BuildCommandError::new(BuildCommandErrorKind::Analysis(AnalysisError::from(error)));
    let a = wrap(Arc::clone(&first));
    let b = wrap(registration_error_fixture(&dice, &format!("@{}B", "\\\"".repeat(4096))).await);
    let restored = wrap(registration_error_fixture(&dice, &pattern).await);
    let completed = |error| {
        slug_bzlmod_v2::SourcePreparationOutcome::Complete(
            SingletonRootSingleBuildCommandTerminal {
                result: Arc::new(Err(error)),
                observations: None,
            },
        )
    };
    let display = a.to_string();
    let stderr = format!("{a:?}\n");
    assert!(display.len() <= 3072 && stderr.len() < 8192);
    assert!(stderr.contains("diagnostic incomplete: output limit"));
    eprintln!(
        "registration_error Core string bytes={} capacity={} stderr bytes={} capacity={}",
        display.len(),
        display.capacity(),
        stderr.len(),
        stderr.capacity()
    );
    let first_terminal = completed(a);
    let changed_terminal = completed(b);
    let restored_terminal = completed(restored);
    assert!(!first_terminal.complete_eq(&changed_terminal));
    assert!(first_terminal.complete_eq(&restored_terminal));
    drop(first);
    dice.wait_for_idle().await;
    drop(dice);
    assert!(weak.upgrade().is_some());
    drop(first_terminal);
    drop(changed_terminal);
    drop(restored_terminal);
    assert!(weak.upgrade().is_none());
}
