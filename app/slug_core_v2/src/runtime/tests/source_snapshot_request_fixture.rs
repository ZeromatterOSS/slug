// Test metadata comes from the ordinary tracked mapping producer. This helper
// creates no source reservation, native root, scope or completion capability.
pub(in crate::runtime::dice) fn workspace_request(
    workspace: &Path,
    lease: u64,
) -> Arc<slug_bzlmod_v2::RepositorySourceSnapshotRequest> {
    source_request(workspace, lease, None)
}

pub(in crate::runtime::dice) fn local_request(
    workspace: &Path,
    lease: u64,
    local_root: &Path,
) -> Arc<slug_bzlmod_v2::RepositorySourceSnapshotRequest> {
    source_request(workspace, lease, Some(local_root))
}

fn source_request(
    workspace: &Path,
    lease: u64,
    local_root: Option<&Path>,
) -> Arc<slug_bzlmod_v2::RepositorySourceSnapshotRequest> {
    use slug_bzlmod_v2::SourcePreparationOutcome;
    let mut module = super::configured_test_module("module(name='source_snapshot_fixture')\n");
    for (name, version) in super::general_repository_archive_tests::BUILTIN_MODULE_PREREQUISITES {
        let relative = format!(".source_test_modules/{name}");
        let root = workspace.join(&relative);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("MODULE.bazel"),
            format!("module(name='{name}',version='{version}')\n"),
        )
        .unwrap();
        module.push_str(&format!(
            "local_path_override(module_name='{name}',path='{relative}')\n"
        ));
        if matches!(
            *name,
            "bazel_features" | "rules_apple" | "rules_swift" | "abseil-cpp"
        ) {
            module.push_str(&format!("bazel_dep(name='{name}',version='{version}')\n"));
        }
    }
    if let Some(local_root) = local_root {
        let relative = local_root
            .strip_prefix(workspace)
            .expect("test Local source must be workspace-relative");
        std::fs::create_dir_all(local_root).unwrap();
        std::fs::write(
            local_root.join("MODULE.bazel"),
            "module(name='source_local',version='1.0')\n",
        )
        .unwrap();
        std::fs::write(
            local_root.join("BUILD.bazel"),
            "exports_files(glob(['*']))\n",
        )
        .unwrap();
        module.push_str("bazel_dep(name='source_local',version='1.0')\n");
        module.push_str(&format!(
            "local_path_override(module_name='source_local',path={:?})\n",
            relative.to_str().expect("test Local root must be UTF-8"),
        ));
    }
    std::fs::write(workspace.join("MODULE.bazel"), module).unwrap();
    let runtime =
        WorkspaceRuntime::new(workspace, crate::runtime::ProcessHostOwner::native()).unwrap();
    let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
    inputs.command_policy = super::configured_test_command_policy(workspace);
    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command_with_inputs(inputs)
            .unwrap()
            .into_command(),
    );
    for _ in 0..256 {
        guard.begin_attempt().unwrap();
        let value = runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(guard.attempt_user_computation_data().unwrap());
            guard.inject_attempt(&mut updater).unwrap();
            let mut transaction = runtime
                .request_revision
                .commit_native_attempt(updater)
                .await
                .unwrap();
            if local_root.is_some() {
                transaction
                    .compute(
                        &slug_loading_v2::HostCanonicalRepositoryLoadRouteObservationKey::new(
                            runtime.repository_materializer.workspace().clone(),
                            slug_identity_v2::CanonicalRepoName::new(
                                slug_bzlmod_v2::bazel_canonical_module_repo_name("source_local"),
                            )
                            .unwrap(),
                        ),
                    )
                    .await
                    .unwrap()
                    .map(|result| {
                        let observed = result.map_err(|error| format!("{error:?}"))?;
                        let route = observed
                            .result()
                            .as_ref()
                            .as_ref()
                            .map_err(|error| format!("{error:?}"))?;
                        slug_bzlmod_v2::RepositorySourceSnapshotRequest::from_canonical(
                            route.input(),
                            lease,
                        )
                        .map(Arc::new)
                        .map_err(|error| format!("{error:?}"))
                    })
            } else {
                transaction
                    .compute(
                        &slug_bzlmod_v2::HostRootRepositoryMappingObservationKey::new(
                            runtime.repository_materializer.workspace().clone(),
                        ),
                    )
                    .await
                    .unwrap()
                    .map(|result| {
                        let observed = result.map_err(|error| format!("{error:?}"))?;
                        let mapping = observed
                            .result()
                            .as_ref()
                            .as_ref()
                            .map_err(|error| format!("{error:?}"))?;
                        slug_bzlmod_v2::RepositorySourceSnapshotRequest::from_workspace(
                            runtime.repository_materializer.workspace().clone(),
                            mapping,
                            lease,
                        )
                        .map(Arc::new)
                        .map_err(|error| format!("{error:?}"))
                    })
            }
        });
        guard.seal_retry().unwrap();
        match value {
            SourcePreparationOutcome::Need(needs) => {
                guard.progress(&needs).unwrap();
            }
            SourcePreparationOutcome::Complete(request) => {
                let request = request.expect("ordinary observed source producer");
                guard.discard().unwrap();
                return request;
            }
        }
    }
    panic!("ordinary mapping fixture did not converge");
}

#[test]
fn workspace_snapshot_request_uses_the_actual_tracked_mapping() {
    let workspace = tempfile::tempdir().unwrap();
    let request = workspace_request(workspace.path(), 17);
    assert_eq!(
        request.kind(),
        slug_bzlmod_v2::RepositorySourceSnapshotKind::Workspace
    );
    assert_eq!(request.lease(), 17);
    assert!(request.repository().is_root());
    assert!(request.repo_spec().is_none());
    assert!(!request.mapping().is_empty());
}

#[test]
fn local_snapshot_request_uses_actual_selected_module_and_preserves_payload() {
    let workspace = tempfile::tempdir().unwrap();
    let local = workspace.path().join("local");
    std::fs::create_dir(&local).unwrap();
    std::fs::write(local.join("payload"), b"actual local source\n").unwrap();
    let request = local_request(workspace.path(), 19, &local);
    assert_eq!(
        request.kind(),
        slug_bzlmod_v2::RepositorySourceSnapshotKind::Local
    );
    assert_eq!(request.lease(), 19);
    assert_eq!(request.repository().as_str(), "source_local+");
    assert_eq!(request.local_root().unwrap().as_path(), local);
    assert!(request.repo_spec().is_some());
    assert!(matches!(
        request.materialization_request().unwrap().kind,
        slug_bzlmod_v2::RepositoryMaterializationKind::Local { .. }
    ));
    assert_eq!(
        std::fs::read(local.join("payload")).unwrap(),
        b"actual local source\n"
    );
}
