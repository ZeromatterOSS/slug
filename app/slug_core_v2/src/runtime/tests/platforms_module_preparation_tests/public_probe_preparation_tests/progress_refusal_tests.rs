use super::*;
use crate::runtime::dice::progress_refusal_diagnostic::ProgressRefusalDiagnostic;

#[derive(Default)]
struct RefusalWitness {
    demand: usize,
    calls: usize,
    captured: usize,
    record: String,
    lease: u64,
}
#[derive(Clone)]
struct RefusalRoot {
    inner: SyntheticCommandRoot,
    root: BuildCommandRootKey,
    scope: Option<Arc<SourceServiceScope>>,
    poison: Option<SourceServiceError>,
    witness: Arc<Mutex<RefusalWitness>>,
}
#[async_trait]
impl NativeCommandRoot for RefusalRoot {
    type Terminal = Result<SyntheticCommandValue, SyntheticCommandError>;
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<SourcePreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        let outcome = self.inner.compute(tx).await?;
        if let SourcePreparationOutcome::Need(needs) = &outcome {
            self.witness.lock().unwrap().demand =
                needs.path_observations().unwrap().demands().as_ptr() as usize;
            if let (Some(scope), Some(error)) = (&self.scope, self.poison) {
                assert_eq!(scope.poison(error), error);
            }
        }
        Ok(outcome)
    }
    fn progress_refused(
        &self,
        needs: &SourcePreparationNeeds,
        command: &NativeDemandCommand<'_>,
        attempt: usize,
        error: &NativeDemandSessionError,
    ) {
        let mut witness = self.witness.lock().unwrap();
        witness.calls += 1;
        assert_eq!(
            needs.path_observations().unwrap().demands().as_ptr() as usize,
            witness.demand
        );
        assert_eq!(attempt, 1);
        assert!(
            matches!(command.runtime.native_demand_sessions.state.lock().unwrap().phase,NativeDemandLeasePhase::Open { lease,.. } if lease == command.lease)
        );
        assert!(command.path_observations.observations().is_empty());
        if let Some(diagnostic) =
            ProgressRefusalDiagnostic::for_refusal(&self.root, command, needs, attempt, error)
        {
            witness.captured += 1;
            witness.lease = command.lease.0;
            witness.record = diagnostic.to_string();
            // Same actual formatter transport failure cannot replace error/abort.
            struct Fail;
            impl fmt::Write for Fail {
                fn write_str(&mut self, _: &str) -> fmt::Result {
                    Err(fmt::Error)
                }
            }
            assert!(fmt::write(&mut Fail, format_args!("{diagnostic}")).is_err());
        }
    }
}

#[test]
fn progress_refusal_reports_original_retry_need_and_restores() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(fixture.path().canonicalize().unwrap()).unwrap();
    // No real source is read: an absent synthetic path reaches the normal DICE
    // path key, then the same original native driver seals and forwards its Need.
    let path = workspace.as_path().join("not-acquired");
    let make = |id, scope: Option<Arc<SourceServiceScope>>, poison| {
        let witness = Arc::new(Mutex::new(RefusalWitness::default()));
        let plan = synthetic_plan(
            id,
            &workspace,
            [],
            [native_host_file_demand(path.clone())],
            Ok(SyntheticCommandValue::Build("complete".into())),
        );
        RefusalRoot {
            inner: SyntheticCommandRoot::Build(SyntheticBuildRootKey { plan }),
            root: BuildCommandRootKey {
                workspace: workspace.dupe(),
                targets: Arc::from([Arc::from("//pkg:probe")]),
                base_configuration: ConfigurationKey::target("proof").unwrap(),
                configuration_overlay: CommandConfigurationOverlay::default(),
            },
            scope,
            poison,
            witness,
        }
    };
    let scoped = |scope: Arc<SourceServiceScope>| {
        WorkspaceRuntime::new_with_source_services(
            workspace.as_path(),
            crate::runtime::ProcessHostOwner::native(),
            source_service::WorkspaceSourceServices::Diagnostic(scope),
        )
        .unwrap()
    };
    let new_scope = || {
        let now = Instant::now();
        SourceServiceScope::new_for_test(
            workspace.dupe(),
            now + Duration::from_secs(5),
            now + Duration::from_secs(8),
            SourceServiceLimits::default(),
        )
        .unwrap()
    };
    for (id, error) in [
        (1100, SourceServiceError::WorkDeadline),
        (1101, SourceServiceError::Cancelled),
    ] {
        let scope = new_scope();
        let scope_weak = Arc::downgrade(&scope);
        let runtime = scoped(scope.clone());
        let before = accepted_native_snapshot(&runtime);
        let root = make(id, Some(scope.clone()), Some(error));
        let witness = root.witness.clone();
        let result = runtime.drive_command(
            NativeDemandRequestInputBundle::normalized_initial(),
            root.clone(),
        );
        assert!(
            matches!(result,Err(NativeDemandSessionError::SourceService(actual)) if actual == error)
        );
        assert_current_native_snapshot(&runtime, &before);
        let counters = scope.snapshot();
        assert_eq!(counters.source_attempts, 0);
        assert_eq!(counters.transferred_bytes, 0);
        assert_eq!(counters.decoded_bytes, 0);
        assert_eq!(counters.written_bytes, 0);
        assert_eq!(counters.live_buffer_bytes, 0);
        let witness_guard = witness.lock().unwrap();
        assert_eq!(witness_guard.calls, 1);
        assert_eq!(
            witness_guard.captured,
            usize::from(error == SourceServiceError::WorkDeadline)
        );
        if error == SourceServiceError::WorkDeadline {
            let json: serde_json::Value = serde_json::from_str(
                witness_guard
                    .record
                    .strip_prefix("WP1100_PROGRESS_REFUSAL ")
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(json["capture"], "captured");
            assert_eq!(json["refusal"], "work_deadline");
            assert_eq!(json["rows"][0][1], "file_bytes");
            assert_eq!(json["rows"][0][2], serde_json::json!(["host", null]));
            assert_eq!(json["rows"][0][3], serde_json::json!(["host", null]));
            assert_eq!(
                json["rows"][0][0][0].as_u64(),
                Some(path.as_os_str().as_bytes().len() as u64)
            );
            let hex = format!("{}", BorrowedHex(Some(path.as_os_str().as_bytes())));
            assert_eq!(json["rows"][0][0][1], hex);
            assert_eq!(json["association"][3].as_u64(), Some(witness_guard.lease));
            assert_eq!(json["association"][4], 1);
            assert_eq!(json["association"][5], "unavailable");
            assert_eq!(json["association"][0], json["association"][1]);
            assert_eq!(
                json["association"][2],
                serde_json::json!([[11, "2f2f706b673a70726f6265"]])
            );
            assert_eq!(json["families"][0], serde_json::json!(["paths", true, 1]));
        } else {
            assert!(witness_guard.record.is_empty());
        }
        drop(witness_guard);
        // Full command preflight must still reject the sticky poisoned scope.
        // That refusal cleans up its fresh lease/materializer; the original
        // lower owners can reopen without reviving source acquisition.
        assert!(matches!(
            runtime.begin_native_demand_command(),
            Err(NativeDemandSessionError::Repository(
                crate::runtime::repository_io::RepositorySessionError::SourceService(actual)
            )) if actual == error
        ));
        assert_current_native_snapshot(&runtime, &before);
        let (lease, _) = runtime.native_demand_sessions.acquire().unwrap();
        let repository = runtime.repository_materializer.begin().unwrap();
        runtime
            .native_demand_sessions
            .attach_repository(lease, repository)
            .unwrap();
        runtime.repository_materializer.discard(repository).unwrap();
        runtime.native_demand_sessions.close(lease).unwrap();
        drop(root);
        drop(runtime);
        drop(scope);
        assert!(scope_weak.upgrade().is_none());
    }
    // Healthy scoped success reaches the same real retry/progress loop with no
    // refusal callback; observation of absence remains ordinary path semantics.
    let scope = new_scope();
    let runtime = scoped(scope.clone());
    let root = make(1102, Some(scope.clone()), None);
    let witness = root.witness.clone();
    let driven = runtime
        .drive_command(NativeDemandRequestInputBundle::normalized_initial(), root)
        .unwrap();
    assert_eq!(driven.attempts, 2);
    assert!(driven.accepted.terminal().as_ref().is_ok());
    assert_eq!(witness.lock().unwrap().calls, 0);
    drop(driven);
    drop(runtime);
    drop(scope);
    // Unarmed command rejects the source-service projection even when offered
    // its real command and a WorkDeadline value; original default root is nop.
    let runtime = test_runtime(workspace.as_path()).unwrap();
    let root = make(1103, None, None);
    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    let needs = SourcePreparationNeeds::path(NeedPathObservations::singleton(
        native_host_file_demand(path),
    ));
    let error = NativeDemandSessionError::SourceService(SourceServiceError::WorkDeadline);
    assert!(
        ProgressRefusalDiagnostic::for_refusal(&root.root, guard.command(), &needs, 1, &error)
            .is_none()
    );
    root.inner
        .progress_refused(&needs, guard.command(), 1, &error);
    assert_eq!(root.witness.lock().unwrap().calls, 0);
    guard.discard().unwrap();
}
