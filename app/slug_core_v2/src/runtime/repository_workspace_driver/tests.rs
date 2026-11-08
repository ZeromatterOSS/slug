use std::time::Duration;
use std::time::Instant;

use super::*;
use crate::runtime::repository_physical_root::RepositoryRootResourceLimits;
use crate::runtime::repository_physical_root::RepositoryRootResources;

fn bank() -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(512 * 1024 * 1024, 512, 8 * 1024 * 1024 * 1024).unwrap(),
    )
    .unwrap()
}
fn limits() -> slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits {
    slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits::new(
        128,
        16 * 1024 * 1024,
        1024 * 1024 * 1024,
    )
    .unwrap()
}
fn scoped(workspace: &std::path::Path) -> WorkspaceRuntime {
    scoped_for(workspace, Duration::from_secs(26))
}
fn scoped_for(workspace: &std::path::Path, work: Duration) -> WorkspaceRuntime {
    let workspace = NormalizedAbsolutePath::new(workspace.canonicalize().unwrap()).unwrap();
    let deadline = Instant::now() + work;
    let scope = source_service::SourceServiceScope::new_for_test(
        workspace.clone(),
        deadline,
        deadline + Duration::from_secs(1),
        source_service::SourceServiceLimits::default(),
    )
    .unwrap();
    WorkspaceRuntime::new_with_source_services(
        workspace.as_path(),
        crate::runtime::ProcessHostOwner::native(),
        source_service::WorkspaceSourceServices::Diagnostic(scope),
    )
    .unwrap()
}

fn original_workspace(source: &str, member: &str) -> (tempfile::TempDir, BzlmodCommandPolicyKey) {
    // Use the existing authentic original/archive fixture's declared builtin
    // prerequisites. Nothing replaces or injects its mapping producer.
    use crate::runtime::dice::tests::configured_test_command_policy;
    use crate::runtime::dice::tests::configured_test_module;
    use crate::runtime::dice::tests::general_repository_archive_tests::BUILTIN_MODULE_PREREQUISITES;
    let workspace = tempfile::tempdir().unwrap();
    let policy = configured_test_command_policy(workspace.path());
    let mut module = configured_test_module(
        "module(name='general_process_fixture')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, proof='proof')\n",
    );
    for (name, version) in BUILTIN_MODULE_PREREQUISITES {
        let relative = format!(".general_test_modules/{name}");
        let directory = workspace.path().join(&relative);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("MODULE.bazel"),
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
    std::fs::write(workspace.path().join("MODULE.bazel"), module).unwrap();
    std::fs::write(
        workspace.path().join("BUILD.bazel"),
        format!("filegroup(name='use', srcs=['@proof//:{member}'])\n"),
    )
    .unwrap();
    std::fs::write(workspace.path().join("ext.bzl"), source).unwrap();
    (workspace, policy)
}

#[test]
fn unretired_queued_owner_refuses_cleanup_and_keeps_session_busy() {
    let workspace = tempfile::tempdir().unwrap();
    let mut runtime = scoped_for(workspace.path(), Duration::from_millis(500));
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    let command = guard
        .command
        .as_mut()
        .unwrap()
        .workspace_progress
        .as_mut()
        .unwrap()
        .command
        .take()
        .unwrap();
    let observer = command.retirement_observer();
    let (send, receive) = tokio::sync::oneshot::channel();
    assert!(send.send(command).is_ok());
    let refused = guard.abort::<()>(refusal("primary cancellation"));
    assert!(refused.is_err());
    assert!(!observer.wait_until(Instant::now()));
    assert!(matches!(
        runtime.native_demand_sessions.state.lock().unwrap().phase,
        NativeDemandLeasePhase::Open { .. }
    ));
    assert!(
        !runtime
            .native_demand_sessions
            .take_trace()
            .contains(&NativeDemandTestTrace::LeaseClosed)
    );
    drop(receive);
    assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
    drop(observer);
    drop(guard);
    drop(runtime);
    assert_eq!(resources.snapshot().retained_bytes, baseline);
    assert_eq!(resources.snapshot().native_fds, 0);
}

#[test]
fn generic_policy_is_disabled_by_default_and_scopes_native_commands_without_replacing_acquisition()
{
    let workspace = tempfile::tempdir().unwrap();
    let mut runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    assert_eq!(
        runtime.repository_workspace_policy(),
        slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicy::Disabled
    );
    assert!(runtime.source_scope.is_none());
    let mut disabled = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    assert!(disabled.command().command_source_scope().is_none());
    disabled.discard().unwrap();
    drop(disabled);
    let resources = bank();
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    assert_eq!(
        runtime.repository_workspace_policy(),
        slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1
    );
    let mut leases = Vec::new();
    let mut scopes = Vec::new();
    for _ in 0..2 {
        let mut guard = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        let scope = guard.command().command_source_scope().unwrap().clone();
        assert!(guard.command().owns_command_scope);
        assert!(
            runtime.source_scope.is_none(),
            "permanent Native acquisition owner must stay unscoped"
        );
        assert!(Arc::ptr_eq(
            &scope,
            &runtime
                .repository_materializer
                .command_source_scope(guard.command().repository_session)
                .unwrap()
                .unwrap()
        ));
        assert!(scopes.iter().all(|old| !Arc::ptr_eq(old, &scope)));
        assert_eq!(scope.snapshot().source_attempts, 0);
        scope
            .charge(source_service::SourceCounter::SourceAttempts, 1)
            .unwrap();
        guard.begin_attempt().unwrap();
        let (policy, epoch) = runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(guard.attempt_user_computation_data().unwrap());
            guard.inject_attempt(&mut updater).unwrap();
            let mut transaction = runtime.request_revision.commit(updater).await;
            let workspace = runtime.repository_materializer.workspace().clone();
            (
                transaction
                    .compute(&slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicyKey::new(
                        workspace.clone(),
                    ))
                    .await
                    .unwrap(),
                transaction
                    .compute(&slug_bzlmod_v2::RepositoryWorkspaceObservationKey::new(
                        workspace,
                    ))
                    .await
                    .unwrap(),
            )
        });
        assert_eq!(
            policy,
            slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1
        );
        assert!(epoch.transcripts().unwrap().is_empty());
        assert!(!leases.contains(&epoch.lease().unwrap()));
        leases.push(epoch.lease().unwrap());
        guard.discard().unwrap();
        assert_eq!(
            scope.check(),
            Err(source_service::SourceServiceError::Cancelled)
        );
        scopes.push(scope);
    }
}

#[test]
fn diagnostic_command_scope_keeps_the_original_owner_without_refresh() {
    let workspace = tempfile::tempdir().unwrap();
    let mut runtime = scoped(workspace.path());
    let original = runtime.source_scope.as_ref().unwrap().clone();
    runtime
        .enable_general_repository_workspaces_for_test(bank(), limits())
        .unwrap();
    for _ in 0..2 {
        let mut guard = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        assert!(!guard.command().owns_command_scope);
        assert!(Arc::ptr_eq(
            guard.command().command_source_scope().unwrap(),
            &original
        ));
        guard.discard().unwrap();
        original.check().unwrap();
    }
}

#[test]
fn native_command_cancellation_interrupts_pending_work_and_drains_with_original_cleanup_clock() {
    let workspace = tempfile::tempdir().unwrap();
    let mut runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    let scope = guard.command().command_source_scope().unwrap().clone();
    let cleanup = scope.cleanup_deadline();
    let args_a = super::super::source_execution::RequestArgsContext::new(Some(scope.as_ref()));
    assert!(!slug_build_api_v2::ArgsDirectoryContext::is_cancelled(
        &args_a
    ));
    let result: Result<(), _> = runtime.runtime.block_on(async {
        let cancel = scope.clone();
        let task = tokio::spawn(async move {
            tokio::task::yield_now().await;
            cancel.cancel();
        });
        let result = command_source_work(Some(&scope), std::future::pending()).await;
        task.await.unwrap();
        result
    });
    assert!(matches!(
        result,
        Err(NativeDemandSessionError::SourceService(
            source_service::SourceServiceError::Cancelled
        ))
    ));
    runtime
        .runtime
        .block_on(command_source_cleanup(Some(&scope), async { Ok(()) }))
        .unwrap();
    assert_eq!(scope.cleanup_deadline(), cleanup);
    assert_eq!(
        scope.check(),
        Err(source_service::SourceServiceError::Cancelled)
    );
    assert!(slug_build_api_v2::ArgsDirectoryContext::is_cancelled(
        &args_a
    ));
    guard.discard().unwrap();
    drop(guard);
    let mut next = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    assert!(!Arc::ptr_eq(
        &scope,
        next.command().command_source_scope().unwrap()
    ));
    next.command().check_source_scope().unwrap();
    let args_b = super::super::source_execution::RequestArgsContext::new(
        next.command().command_source_scope().map(Arc::as_ref),
    );
    assert!(!slug_build_api_v2::ArgsDirectoryContext::is_cancelled(
        &args_b
    ));
    assert!(slug_build_api_v2::ArgsDirectoryContext::is_cancelled(
        &args_a
    ));
    drop(args_b);
    next.discard().unwrap();
    drop(next);
    drop(args_a);
    drop(scope);
    drop(runtime);
    assert_eq!(resources.snapshot().retained_bytes, baseline);
    assert_eq!(resources.snapshot().native_fds, 0);
    assert_eq!(resources.snapshot().backing_bytes, 0);
}

#[test]
fn enabled_policy_rejects_legacy_realizations_in_complete_frontier_before_setup() {
    use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
    use slug_bzlmod_v2::RepositoryFilesystemCallId;
    use slug_bzlmod_v2::RepositoryFilesystemOperation;
    use slug_bzlmod_v2::RepositoryFilesystemRecord;
    use slug_bzlmod_v2::RepositoryFilesystemRequest;
    use slug_bzlmod_v2::RepositoryFilesystemResult;
    let workspace = tempfile::tempdir().unwrap();
    let mut runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let resources = bank();
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    let owner = physical_root_owner_for_test();
    let spec = Arc::new(slug_bzlmod_v2::RepoSpec {
        rule_id: slug_bzlmod_v2::RepoRuleId {
            bzl_file: slug_identity_v2::CanonicalLabel::parse("@@//:rules.bzl").unwrap(),
            rule_name: "ordinary".into(),
        },
        attributes: Arc::new(SmallMap::new()),
    });
    let prefix = GeneratedRepositoryFileEffectPlan::builder().finish();
    let operation = RepositoryFilesystemOperation::Delete("old".into());
    let legacy = Arc::new(RepositoryFilesystemRequest::new(
        RepositoryFilesystemCallId::new(owner.clone(), 0, 0),
        "legacy".into(),
        operation.clone(),
        prefix.clone(),
    ));
    let transcript = RepositoryFilesystemTranscript::empty()
        .append(RepositoryFilesystemRecord::new(
            legacy,
            RepositoryFilesystemResult::Delete(false),
        ))
        .unwrap();
    let realization = slug_bzlmod_v2::RepositoryFilesystemRealization::new(
        1,
        owner.clone(),
        0,
        transcript,
        prefix.with_operation(&operation).unwrap(),
    )
    .unwrap();
    for kind in [
        RepositoryMaterializationKind::GeneratedFileEffects(prefix),
        RepositoryMaterializationKind::GeneratedFilesystemEffects(realization),
    ] {
        let mut guard = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        let normalized = runtime.repository_materializer.workspace().clone();
        let key = Arc::new(
            slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey::new(
                normalized.clone(),
                owner.clone(),
                slug_identity_v2::CanonicalRepoName::new("claimed").unwrap(),
                0,
            )
            .unwrap(),
        );
        let reserve =
            slug_bzlmod_v2::RepositoryWorkspaceReservationDemand::new(key, spec.clone()).unwrap();
        let terminal = RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: normalized,
                canonical_repo: slug_identity_v2::CanonicalRepoName::new("legacy").unwrap(),
            },
            repo_spec: (*spec).clone(),
            kind,
        };
        let need = slug_bzlmod_v2::SourcePreparationNeeds::repository_workspace_reservation(
            Arc::new(reserve),
        )
        .try_union(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
            terminal,
        ))
        .unwrap();
        let before = resources.snapshot();
        assert!(guard.progress(&need).is_err());
        assert!(
            guard
                .command()
                .current_workspace_epoch()
                .transcripts()
                .unwrap()
                .is_empty()
        );
        assert_eq!(resources.snapshot().native_fds, 0);
        assert_eq!(resources.snapshot().peak_native_fds, before.peak_native_fds);
        assert_eq!(resources.snapshot().backing_bytes, 0);
        guard.discard().unwrap();
    }
}

#[test]
fn queued_and_refused_result_delivery_keep_retirement_armed_until_command_drop() {
    for queued in [true, false] {
        let workspace = tempfile::tempdir().unwrap();
        let mut runtime = scoped(workspace.path());
        let resources = bank();
        let baseline = resources.snapshot().retained_bytes;
        runtime
            .enable_general_repository_workspaces_for_test(resources.clone(), limits())
            .unwrap();
        let mut guard = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        let command = guard
            .command
            .as_mut()
            .unwrap()
            .workspace_progress
            .as_mut()
            .unwrap()
            .command
            .take()
            .unwrap();
        let observer = command.retirement_observer();
        let (send, receive) = tokio::sync::oneshot::channel();
        if queued {
            assert!(send.send(command).is_ok());
            assert!(!observer.wait_until(Instant::now()));
            drop(receive);
        } else {
            drop(receive);
            let refused = send.send(command).err().unwrap();
            assert!(!observer.wait_until(Instant::now()));
            drop(refused);
        }
        assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
        drop(observer);
        guard.discard().unwrap();
        drop(guard);
        drop(runtime);
        assert_eq!(resources.snapshot().retained_bytes, baseline);
        assert_eq!(resources.snapshot().native_fds, 0);
        assert_eq!(resources.snapshot().backing_bytes, 0);
    }
}

const ORIGINAL: &str = r#"
def _repository(ctx):
    print("before")
    root = str(ctx.path("."))
    if not root.startswith("/__slug_repositories__/v1/"): fail("unreserved path")
    ctx.file("root.txt", root, executable=False)
    ctx.file("seed", "seed", executable=False)
    first = ctx.execute(["/usr/bin/python3", "-c", "import pathlib,sys; pathlib.Path('out').write_text('written'); print('first'); sys.exit(7)"], quiet=False)
    if type(first) != "exec_result" or first.return_code != 7: fail("nonzero result")
    print("middle")
    if not ctx.path("out").exists or ctx.read(ctx.path("out")) != "written": fail("actual process tree")
    ctx.template("rendered", ctx.path("out"), {"written": "rendered"}, executable=False)
    if not ctx.delete(ctx.path("out")): fail("actual delete")
    second = ctx.execute(["/usr/bin/python3", "-c", "import pathlib; pathlib.Path('out').write_text(pathlib.Path('rendered').read_text()+'!'); print('second')"])
    if second.return_code != 0 or ctx.read("out") != "rendered!": fail("second tree")
    ctx.file("BUILD.bazel", "exports_files(['out'])", executable=False)
    print("after")
repository = repository_rule(implementation=_repository)
def _extension(ctx):
    repository(name="proof")
ext = module_extension(implementation=_extension)
"#;

#[test]
#[ignore = "actual compiled CLI + original general Starlark + same-root adoption; finite native supervisor required"]
fn native_original_starlark_uses_general_needs_normal_records_and_same_root_adoption() {
    let (workspace, policy) = original_workspace(ORIGINAL, "out");
    let mut runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let resources = bank();
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    let mut prior_binding = None;
    for ordinal in 0..2 {
        let accepted = runtime
            .cquery_command_with_bzlmod_inputs(
                "//:use",
                true,
                true,
                policy.clone(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                CommandConfigurationOverlay::new(Arc::from([])),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "original command {ordinal} failed at {}: {error}; source={:?}; native={:?}",
                    runtime.native_demand_sessions.workspace_stage(),
                    runtime.source_scope.as_ref().map(|scope| scope.snapshot()),
                    resources.snapshot()
                )
            });
        assert!(
            accepted.terminal_for_test().as_ref().is_ok(),
            "{:#?}",
            accepted.terminal_for_test()
        );
        let state = runtime.native_demand_sessions.state.lock().unwrap();
        assert!(matches!(state.phase, NativeDemandLeasePhase::Idle));
        let requests = state.accepted.selected.repository_requests();
        let request = requests
            .iter()
            .find(|request| {
                matches!(
                    request.kind,
                    RepositoryMaterializationKind::GeneralWorkspace(_)
                )
            })
            .expect("original generated repository must select GeneralWorkspace");
        let RepositoryMaterializationKind::GeneralWorkspace(realization) = &request.kind else {
            unreachable!()
        };
        let binding = (
            realization.reservation().lease(),
            realization.reservation().instance(),
            realization.reservation().display_root().clone(),
        );
        if let Some((lease, instance, display)) = &prior_binding {
            assert_ne!(lease, &binding.0);
            assert_ne!(instance, &binding.1);
            assert_eq!(display, &binding.2);
        }
        let forwarded = accepted
            .batches_for_test()
            .iter()
            .flat_map(|batch| batch.events())
            .filter_map(|event| match event {
                slug_events_v2::EvaluationEvent::RepositoryExecuteOutput {
                    command_lease,
                    stdout,
                    stderr,
                    ..
                } => Some((*command_lease, stdout.as_ref(), stderr.as_ref())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            forwarded,
            [(binding.0, b"first\n".as_slice(), b"".as_slice())],
            "quiet=False completion forwards exactly once despite Need retries, and again for a fresh command"
        );
        let original_events = accepted
            .batches_for_test()
            .iter()
            .flat_map(|batch| batch.events())
            .filter_map(|event| match event {
                slug_events_v2::EvaluationEvent::StarlarkPrint { text, .. }
                    if matches!(text.as_str(), "before" | "middle" | "after") =>
                {
                    Some(text.as_str())
                }
                slug_events_v2::EvaluationEvent::RepositoryExecuteOutput { .. } => {
                    Some("first-output")
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            original_events,
            ["before", "first-output", "middle", "after"],
            "original prints and quiet=False output each publish once, in evaluation order, across Need retries and fresh commands"
        );
        // This original callback's batch contains command-fresh process output.
        // No quiet=True-only or file-only event reconciliation policy is changed.
        prior_binding = Some(binding);
        assert!(!realization.reservation().display_root().as_path().exists());
        let mut executes = 0;
        for record in realization.transcript().records() {
            if let slug_bzlmod_v2::RepositoryWorkspaceResult::Execute(result) = record.result() {
                assert_eq!(result.return_code(), if executes == 0 { 7 } else { 0 });
                assert_eq!(
                    result.stdout(),
                    if executes == 0 {
                        b"first\n".as_slice()
                    } else {
                        b"second\n".as_slice()
                    }
                );
                executes += 1;
            }
        }
        assert_eq!(executes, 2);
        let body = |path: &str| match realization
            .final_state()
            .entries()
            .iter()
            .find(|entry| entry.path() == path)
            .unwrap()
            .node()
        {
            slug_bzlmod_v2::RepositoryWorkspaceNode::File(body) => body.as_ref(),
            _ => panic!("regular final file"),
        };
        assert_eq!(body("out"), b"rendered!");
        assert_eq!(
            body("root.txt"),
            realization
                .reservation()
                .display_root()
                .as_path()
                .to_str()
                .unwrap()
                .as_bytes()
        );
        drop(state);
        drop(accepted);
    }
    drop(runtime);
    assert_eq!(resources.snapshot().native_fds, 0);
    assert_eq!(resources.snapshot().backing_bytes, 0);
}

#[test]
#[ignore = "actual claimed root then malformed full frontier before any new setup; finite native supervisor required"]
fn native_frontier_refuses_foreign_predecessor_and_future_index_before_new_root_setup() {
    use slug_bzlmod_v2::RepositoryWorkspaceOperation;
    use slug_bzlmod_v2::RepositoryWorkspaceOperationDemand;
    use slug_bzlmod_v2::RepositoryWorkspaceReservationDemand;
    use slug_bzlmod_v2::RepositoryWorkspaceResult;
    use slug_bzlmod_v2::RepositoryWorkspaceTranscript;
    let fixture = selected_tool_source::tests::Fixture::new(ORIGINAL);
    let mut runtime = scoped(fixture.workspace.path());
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    let normalized = runtime.repository_materializer.workspace().clone();
    let spec = Arc::new(slug_bzlmod_v2::RepoSpec {
        rule_id: slug_bzlmod_v2::RepoRuleId {
            bzl_file: slug_identity_v2::CanonicalLabel::parse("@@//:ext.bzl").unwrap(),
            rule_name: "repository".into(),
        },
        attributes: Arc::new(SmallMap::new()),
    });
    let demand = |name: &str, ordinal| {
        Arc::new(
            RepositoryWorkspaceReservationDemand::new(
                Arc::new(
                    slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey::new(
                        normalized.clone(),
                        fixture.key.owner().clone(),
                        slug_identity_v2::CanonicalRepoName::new(name).unwrap(),
                        ordinal,
                    )
                    .unwrap(),
                ),
                spec.clone(),
            )
            .unwrap(),
        )
    };
    guard
        .progress(
            &slug_bzlmod_v2::SourcePreparationNeeds::repository_workspace_reservation(demand(
                "ready", 0,
            )),
        )
        .unwrap();
    let initial = guard
        .command()
        .current_workspace_epoch()
        .transcripts()
        .unwrap()[0]
        .clone();
    let foreign = RepositoryWorkspaceTranscript::new(initial.reservation().clone(), 0o700)
        .unwrap()
        .request(RepositoryWorkspaceOperation::Exists("absent".into()))
        .unwrap();
    let first = initial
        .request(RepositoryWorkspaceOperation::Exists("absent".into()))
        .unwrap();
    // Deliberately untrusted pure observations cannot advance the actual native
    // epoch. No native completion capability or result is constructed here.
    let invented = initial
        .complete(
            first,
            RepositoryWorkspaceResult::Exists(false),
            initial.current().mode(),
            initial.current().entries().to_vec(),
        )
        .unwrap();
    let future = invented
        .request(RepositoryWorkspaceOperation::Exists("absent".into()))
        .unwrap();
    for (index, request) in [foreign, future].into_iter().enumerate() {
        let before = resources.snapshot();
        let operation =
            RepositoryWorkspaceOperationDemand::new(Arc::new(request), Arc::from([])).unwrap();
        let need = slug_bzlmod_v2::SourcePreparationNeeds::repository_workspace_reservation(
            demand(&format!("uninitialized{index}"), index + 1),
        )
        .try_union(
            &slug_bzlmod_v2::SourcePreparationNeeds::repository_workspace_operation(Arc::new(
                operation,
            )),
        )
        .unwrap();
        assert!(guard.progress(&need).is_err());
        let epoch = guard.command().current_workspace_epoch();
        assert_eq!(epoch.transcripts().unwrap().len(), 1);
        assert!(epoch.transcripts().unwrap()[0].records().is_empty());
        assert_eq!(resources.snapshot().native_fds, before.native_fds);
        assert_eq!(resources.snapshot().peak_native_fds, before.peak_native_fds);
        assert_eq!(resources.snapshot().backing_bytes, before.backing_bytes);
    }
    drop(initial);
    guard.discard().unwrap();
    drop(guard);
    drop(runtime);
    assert_eq!(resources.snapshot().native_fds, 0);
    assert_eq!(resources.snapshot().backing_bytes, 0);
    assert_eq!(resources.snapshot().retained_bytes, baseline);
}

const CANCELLING_ORIGINAL: &str = r#"
def _repository(ctx):
    ctx.file("seed", "before", executable=False)
    ctx.execute(["/usr/bin/python3", "-c", "import pathlib,time; print('running',flush=True); time.sleep(1000); pathlib.Path('late').write_text('forbidden')"])
    ctx.file("BUILD.bazel", "exports_files(['late'])", executable=False)
repository = repository_rule(implementation=_repository)
def _extension(ctx):
    repository(name="proof")
ext = module_extension(implementation=_extension)
"#;

#[test]
#[ignore = "actual CLI pre-HELLO/payload cancellation and command-retirement barrier; finite native supervisor required"]
fn native_original_cancellation_keeps_session_open_until_bootstrap_or_payload_retires() {
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    for (target, hold) in [(1, true), (3, false)] {
        let (workspace, policy) = original_workspace(CANCELLING_ORIGINAL, "late");
        let mut runtime =
            WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
                .unwrap();
        let resources = bank();
        let baseline = resources.snapshot().retained_bytes;
        runtime
            .enable_general_repository_workspaces_for_test(resources.clone(), limits())
            .unwrap();
        let stage = Arc::new(AtomicUsize::new(0));
        let retirement = runtime.observe_general_processes_for_test(stage.clone(), hold);
        let materializer = runtime.repository_materializer.clone();
        let done = AtomicBool::new(false);
        std::thread::scope(|threads| {
            let sessions = &runtime.native_demand_sessions;
            let watcher = threads.spawn(|| {
                let until = Instant::now() + Duration::from_secs(15);
                while stage.load(Ordering::Acquire) < target {
                    if done.load(Ordering::Acquire) {
                        return Err("command returned before native gate");
                    }
                    if Instant::now() >= until {
                        return Err("native gate deadline");
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                let observer = retirement
                    .lock()
                    .unwrap()
                    .as_ref()
                    .expect("command observer published before launch")
                    .clone();
                assert!(matches!(
                    sessions.state.lock().unwrap().phase,
                    NativeDemandLeasePhase::Open { .. }
                ));
                let token = match sessions.state.lock().unwrap().phase {
                    NativeDemandLeasePhase::Open {
                        repository: Some(token),
                        ..
                    } => token,
                    _ => panic!("native gate must retain its actor token"),
                };
                let scope = materializer.command_source_scope(token).unwrap().unwrap();
                scope.cancel();
                let until = Instant::now() + Duration::from_secs(10);
                while !observer.wait_until(Instant::now() + Duration::from_millis(1)) {
                    if !matches!(
                        sessions.state.lock().unwrap().phase,
                        NativeDemandLeasePhase::Open { .. }
                    ) {
                        // Retirement may race the phase read after the timed
                        // wait; a closed lease must already have reached zero.
                        assert!(
                            observer.wait_until(Instant::now()),
                            "lease closed before actual native retirement"
                        );
                    }
                    if Instant::now() >= until {
                        return Err("actual native retirement deadline");
                    }
                }
                Ok(observer)
            });
            let outcome = runtime.cquery_command_with_bzlmod_inputs(
                "//:use",
                true,
                true,
                policy.clone(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                CommandConfigurationOverlay::new(Arc::from([])),
            );
            done.store(true, Ordering::Release);
            let observed = watcher.join().unwrap();
            assert!(
                observed.is_ok(),
                "stage {target}: {:?}; query {outcome:?}",
                observed.err()
            );
            assert!(
                outcome.is_err(),
                "cancelled original must not publish a terminal"
            );
        });
        assert!(
            runtime
                .native_demand_sessions
                .state
                .lock()
                .unwrap()
                .accepted
                .selected
                .repository_requests()
                .is_empty()
        );
        let trace = runtime.native_demand_sessions.take_trace();
        assert!(!trace.contains(&NativeDemandTestTrace::MaterializerAccepted));
        assert!(!trace.contains(&NativeDemandTestTrace::AcceptedSnapshotReplaced));
        assert_eq!(resources.snapshot().native_fds, 0);
        assert_eq!(resources.snapshot().backing_bytes, 0);
        drop(retirement);
        drop(materializer);
        drop(runtime);
        assert_eq!(resources.snapshot().retained_bytes, baseline);
    }
}
