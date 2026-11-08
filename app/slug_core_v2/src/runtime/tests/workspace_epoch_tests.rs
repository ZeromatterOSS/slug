use slug_bzlmod_v2::RepositoryWorkspaceEpochKey;
use slug_bzlmod_v2::RepositoryWorkspaceObservationKey;

use super::*;

#[test]
fn command_workspace_epochs_are_empty_globally_fresh_and_revoked_on_cleanup() {
    let workspace = tempfile::tempdir().unwrap();
    let first = test_runtime(workspace.path()).unwrap();
    let second = test_runtime(workspace.path()).unwrap();
    let normalized = NormalizedAbsolutePath::new(workspace.path().to_path_buf()).unwrap();
    let mut leases = starlark_map::small_set::SmallSet::new();
    for runtime in [&first, &second, &first] {
        let mut command = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        command.begin_attempt().unwrap();
        let epoch = runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(command.attempt_user_computation_data().unwrap());
            command.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            runtime.request_revision.bind_cycle_detector(&transaction);
            transaction
                .compute(&RepositoryWorkspaceObservationKey::new(normalized.clone()))
                .await
                .unwrap()
        });
        assert_eq!(epoch.transcripts().unwrap().len(), 0);
        let lease = epoch.lease().unwrap();
        assert_ne!(lease, 0);
        assert!(
            leases.insert(lease),
            "a prior command or materializer reused an empty epoch"
        );
        assert_eq!(Some(lease), command.command().workspace_epoch.lease());
        command.discard().unwrap();
        let revoked = runtime.runtime.block_on(async {
            let mut transaction = runtime.dice.updater().commit().await;
            transaction
                .compute(&RepositoryWorkspaceEpochKey::new(normalized.clone()))
                .await
                .unwrap()
        });
        assert_eq!(revoked.lease(), None);
        assert_eq!(revoked.transcripts(), None);
    }
}
