// Test metadata comes from the ordinary tracked mapping producer. This helper
// creates no source reservation, native root, scope or completion capability.
pub(in crate::runtime::dice) fn workspace_request(
    workspace: &Path,
    lease: u64,
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
            transaction
                .compute(
                    &slug_bzlmod_v2::HostRootRepositoryMappingObservationKey::new(
                        runtime.repository_materializer.workspace().clone(),
                    ),
                )
                .await
                .unwrap()
                .map(|result| {
                    result
                        .expect("ordinary observed mapping producer")
                        .result()
                        .clone()
                })
        });
        guard.seal_retry().unwrap();
        match value {
            SourcePreparationOutcome::Need(needs) => {
                guard.progress(&needs).unwrap();
            }
            SourcePreparationOutcome::Complete(mapping) => {
                let mapping = mapping.as_ref().as_ref().unwrap();
                let request = Arc::new(
                    slug_bzlmod_v2::RepositorySourceSnapshotRequest::from_workspace(
                        runtime.repository_materializer.workspace().clone(),
                        mapping,
                        lease,
                    )
                    .unwrap(),
                );
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
