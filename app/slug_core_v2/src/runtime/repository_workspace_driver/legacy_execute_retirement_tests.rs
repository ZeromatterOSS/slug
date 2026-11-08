//! Retired Execute descriptors cannot authorize any progress, including replay.
use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryExecuteArg;
use slug_bzlmod_v2::RepositoryExecuteCallId;
use slug_bzlmod_v2::RepositoryExecuteRecord;
use slug_bzlmod_v2::RepositoryExecuteRequest;
use slug_bzlmod_v2::RepositoryExecuteResult;
use slug_bzlmod_v2::RepositoryFilesystemCallId;
use slug_bzlmod_v2::RepositoryFilesystemOperation;
use slug_bzlmod_v2::RepositoryFilesystemRequest;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;

use super::*;

#[test]
fn legacy_execute_refuses_complete_mixed_frontier_before_io_under_both_policies() {
    for general in [false, true] {
        let workspace = tempfile::tempdir().unwrap();
        let mut runtime = scoped(workspace.path());
        let resources = bank();
        if general {
            runtime
                .enable_general_repository_workspaces_for_test(resources.clone(), limits())
                .unwrap();
        }
        let mut guard = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        let owner = physical_root_owner_for_test();
        let prefix = GeneratedRepositoryFileEffectPlan::builder().finish();
        let execute = RepositoryExecuteRequest::new(
            RepositoryExecuteCallId::new(owner.clone(), 0, 0),
            "ordinary".into(),
            Arc::from([RepositoryExecuteArg::String("/bin/false".into())]),
            RepositoryEnvironmentSnapshot::empty(),
            prefix.clone(),
        );
        // Even a structurally exact completed legacy record grants no replay.
        guard.command.as_mut().unwrap().execute_transcript = RepositoryExecuteTranscript::empty()
            .append(RepositoryExecuteRecord::new(
                Arc::new(execute.clone()),
                RepositoryExecuteResult::new(0, Arc::from(&b"old"[..]), Arc::from(&b""[..])),
            ))
            .unwrap();
        let normalized = runtime.repository_materializer.workspace().clone();
        let untouched = workspace.path().join("untouched");
        std::fs::write(&untouched, "original").unwrap();
        let path = SourcePreparationNeeds::path(NeedPathObservations::singleton(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(untouched.clone()).unwrap(),
                PathObservationOperation::FileBytes,
            ),
        ));
        let filesystem = SourcePreparationNeeds::repository_filesystem_operation(
            RepositoryFilesystemRequest::new(
                RepositoryFilesystemCallId::new(owner.clone(), 0, 0),
                "ordinary".into(),
                RepositoryFilesystemOperation::Delete("untouched".into()),
                prefix,
            ),
        );
        let key = Arc::new(
            slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey::new(
                normalized,
                owner,
                slug_identity_v2::CanonicalRepoName::new("claimed").unwrap(),
                0,
            )
            .unwrap(),
        );
        let spec = Arc::new(slug_bzlmod_v2::RepoSpec {
            rule_id: slug_bzlmod_v2::RepoRuleId {
                bzl_file: slug_identity_v2::CanonicalLabel::parse("@@//:ordinary.bzl").unwrap(),
                rule_name: "ordinary".into(),
            },
            attributes: Arc::new(SmallMap::new()),
        });
        let reserve = SourcePreparationNeeds::repository_workspace_reservation(Arc::new(
            slug_bzlmod_v2::RepositoryWorkspaceReservationDemand::new(key, spec).unwrap(),
        ));
        let legacy = SourcePreparationNeeds::repository_execute(execute);
        // Each category independently discriminates preflight order: neither
        // source/path observation, filesystem staging nor native root setup runs.
        for other in [path, filesystem, reserve] {
            let needs = legacy.try_union(&other).unwrap();
            let before = resources.snapshot();
            let source_before = guard.command().command_source_scope().unwrap().snapshot();
            let stage = runtime.native_demand_sessions.workspace_stage();
            assert!(matches!(
                guard.progress(&needs),
                Err(NativeDemandSessionError::LegacyRepositoryExecuteUnsupported)
            ));
            assert_eq!(runtime.native_demand_sessions.workspace_stage(), stage);
            let after = guard.command().command_source_scope().unwrap().snapshot();
            assert_eq!(after.registry_attempts, source_before.registry_attempts);
            assert_eq!(
                after.registry_admitted_bytes,
                source_before.registry_admitted_bytes
            );
            assert_eq!(after.source_attempts, source_before.source_attempts);
            assert_eq!(after.transferred_bytes, source_before.transferred_bytes);
            assert_eq!(after.decoded_bytes, source_before.decoded_bytes);
            assert_eq!(after.written_bytes, source_before.written_bytes);
            assert_eq!(after.live_buffer_bytes, source_before.live_buffer_bytes);
            assert_eq!(after.peak_buffer_bytes, source_before.peak_buffer_bytes);
            assert_eq!(after.live_decoder_bytes, source_before.live_decoder_bytes);
            assert_eq!(after.peak_decoder_bytes, source_before.peak_decoder_bytes);
            assert!(guard.command().path_observations.observations().is_empty());
            assert!(guard.command().filesystem_transcript.records().is_empty());
            assert!(guard.command().issued_requests.is_empty());
            assert!(guard.command().issued_downloads.is_empty());
            assert_eq!(guard.command().execute_transcript.records().len(), 1);
            assert_eq!(std::fs::read(&untouched).unwrap(), b"original");
            assert_eq!(resources.snapshot().native_fds, before.native_fds);
            assert_eq!(resources.snapshot().peak_native_fds, before.peak_native_fds);
            assert_eq!(resources.snapshot().backing_bytes, before.backing_bytes);
            assert_eq!(resources.snapshot().retained_bytes, before.retained_bytes);
            if general {
                assert!(
                    guard
                        .command()
                        .current_workspace_epoch()
                        .transcripts()
                        .unwrap()
                        .is_empty()
                );
            }
        }
        guard.discard().unwrap();
        let mut next = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        next.discard().unwrap();
    }
}
