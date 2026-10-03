use std::time::Duration;

use slug_bzlmod_v2::GuestNamespaceLimits;
use slug_bzlmod_v2::GuestNamespacePolicy;
use slug_bzlmod_v2::RuntimeSourceDeclarationPolicy;
use slug_bzlmod_v2::RuntimeSourceLimits;
use slug_identity_v2::ApparentRepoName;
use slug_workspace_v2::ImageChargedPath;

use super::*;

fn fixture() -> (
    selected_tool_source::tests::Fixture,
    SourcePreparationDiagnosticRuntime,
    NativeDemandRequestInputBundle,
) {
    let f =
        selected_tool_source::tests::Fixture::new(&selected_tool_source::tests::scratch_program(1));
    let workspace = f.runtime.repository_materializer.workspace().clone();
    let work = Instant::now() + Duration::from_secs(10);
    let diagnostic = SourcePreparationDiagnosticRuntime::new(
        workspace,
        crate::runtime::ProcessHostOwner::native(),
        work,
        work + Duration::from_secs(3),
        SourceServiceLimits::default(),
    )
    .unwrap();
    let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
    let pin = ImageChargedPath::pin_existing(
        diagnostic.workspace().clone(),
        diagnostic.descriptor_budget(),
    )
    .unwrap();
    inputs.runtime_sources = Some(
        RuntimeSourceDeclarationPolicy::new(
            &pin,
            &[],
            RuntimeSourceLimits::default(),
            diagnostic.descriptor_budget(),
        )
        .unwrap(),
    );
    inputs.guest_namespace = Some(
        GuestNamespacePolicy::new(
            1001,
            1002,
            1001,
            1002,
            9,
            7,
            8,
            0o755,
            GuestNamespaceLimits::default(),
        )
        .unwrap(),
    );
    inputs.command_policy = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        Some("unused@1.0"),
        true,
        f.workspace.path(),
        ["platforms=platforms"],
    )
    .unwrap();
    inputs.environment_policy =
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("other@2.0")).unwrap();
    inputs.registry_urls = RegistryUrls::new(["https://explicit.invalid"]);
    inputs.lockfile_mode = LockfileMode::Off;
    (f, diagnostic, inputs)
}

#[test]
fn consumed_scope_full_caller_normal_need_engine_drop_and_reader_last_drop() {
    let (f, diagnostic, inputs) = fixture();
    let engine = Arc::downgrade(&diagnostic.runtime.as_ref().unwrap().dice);
    let scope = diagnostic.scope.clone();
    let scope_weak = Arc::downgrade(&scope);
    let images = diagnostic.runtime.as_ref().unwrap().image_resources.clone();
    let name = ApparentRepoName::new("proof").unwrap();
    let view = diagnostic
        .prepare_once(SelectedSourcePreparationRequest {
            apparent_repository: &name,
            inputs: &inputs,
            scratch_policy: *f.key.policy(),
            tool_policy: selected_tool_source::tests::key(&f).policy(),
        })
        .unwrap();
    assert!(
        engine.upgrade().is_none(),
        "consumed engine is destroyed before export"
    );
    assert_eq!(view.inputs(), &inputs);
    assert_eq!(view.selected().owner(), f.key.owner());
    assert!(matches!(
        scope.check(),
        Err(SourceServiceError::AcquisitionClosed)
    ));
    // No reader can acquire more sources; cancellation of acquisition does
    // not revoke the independent captured FD proof and operation-local reader.
    scope.cancel();
    let clone = view.dupe();
    drop(view);
    assert_eq!(
        guest_namespace::tests::read(clone.view(), "/__slug_module/bin/generator"),
        selected_tool_source::tests::wire(1)
    );
    assert!(images.logical_bytes_used() > 0);
    drop(scope);
    assert!(scope_weak.upgrade().is_some());
    drop(clone);
    assert_eq!(images.logical_bytes_used(), 0);
    assert!(
        scope_weak.upgrade().is_none(),
        "actual last captured owner releases scope pin"
    );
}

#[test]
fn cancelled_scope_no_export_engine_and_image_release() {
    let (f, diagnostic, inputs) = fixture();
    let engine = Arc::downgrade(&diagnostic.runtime.as_ref().unwrap().dice);
    let images = diagnostic.runtime.as_ref().unwrap().image_resources.clone();
    let scope = diagnostic.scope.clone();
    scope.cancel();
    let name = ApparentRepoName::new("proof").unwrap();
    let result = diagnostic.prepare_once(SelectedSourcePreparationRequest {
        apparent_repository: &name,
        inputs: &inputs,
        scratch_policy: *f.key.policy(),
        tool_policy: selected_tool_source::tests::key(&f).policy(),
    });
    assert!(matches!(
        result,
        Err(NativeDemandSessionError::SourceService(
            SourceServiceError::Cancelled
        ))
    ));
    assert!(engine.upgrade().is_none());
    assert_eq!(images.logical_bytes_used(), 0);
    assert_eq!(scope.snapshot().source_attempts, 0);
}

#[test]
fn differently_spelled_workspace_refuses_before_dice_or_sources() {
    use std::os::unix::fs::symlink;
    let outer = tempfile::tempdir().unwrap();
    let actual = outer.path().join("real");
    std::fs::create_dir(&actual).unwrap();
    let alias = outer.path().join("alias");
    symlink(&actual, &alias).unwrap();
    let work = Instant::now() + Duration::from_secs(10);
    let result = SourcePreparationDiagnosticRuntime::new(
        NormalizedAbsolutePath::new(alias).unwrap(),
        crate::runtime::ProcessHostOwner::native(),
        work,
        work + Duration::from_secs(3),
        SourceServiceLimits::default(),
    );
    assert!(result.is_err());
    assert!(
        result
            .err()
            .unwrap()
            .downcast_ref::<SourceServiceError>()
            .is_some_and(|e| *e == SourceServiceError::ForeignScope)
    );
}

#[test]
fn absolute_cleanup_expiry_never_polls_ready_work_and_keeps_primary_error() {
    use std::cell::Cell;
    let f =
        selected_tool_source::tests::Fixture::new(&selected_tool_source::tests::scratch_program(1));
    let work = Instant::now() + Duration::from_millis(100);
    let cleanup = work + Duration::from_millis(50);
    let diagnostic = SourcePreparationDiagnosticRuntime::new(
        f.runtime.repository_materializer.workspace().clone(),
        crate::runtime::ProcessHostOwner::native(),
        work,
        cleanup,
        SourceServiceLimits::default(),
    )
    .unwrap();
    let runtime = diagnostic.runtime.as_ref().unwrap();
    let called = Cell::new(false);
    diagnostic.scope.cancel();
    runtime
        .runtime
        .block_on(runtime.source_cleanup(async {
            called.set(true);
            Ok(())
        }))
        .unwrap();
    assert!(
        called.replace(false),
        "poison cannot suppress timely restoration"
    );
    runtime.runtime.block_on(async {
        tokio::time::sleep_until(tokio::time::Instant::from_std(cleanup)).await;
    });
    let refused = runtime.runtime.block_on(runtime.source_cleanup(async {
        called.set(true);
        Ok(())
    }));
    assert!(refused.is_err());
    assert!(!called.get(), "expired cleanup cannot poll even ready work");

    let fresh_work = Instant::now() + Duration::from_millis(100);
    let fresh_cleanup = fresh_work + Duration::from_millis(50);
    let fresh = SourcePreparationDiagnosticRuntime::new(
        f.runtime.repository_materializer.workspace().clone(),
        crate::runtime::ProcessHostOwner::native(),
        fresh_work,
        fresh_cleanup,
        SourceServiceLimits::default(),
    )
    .unwrap();
    let rt = fresh.runtime.as_ref().unwrap();
    let result: Result<(), NativeDemandSessionError> =
        rt.runtime.block_on(rt.source_cleanup(async {
            std::thread::sleep(
                fresh_cleanup.saturating_duration_since(Instant::now()) + Duration::from_millis(1),
            );
            Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
                "primary restoration refusal"
            )))
        }));
    assert!(
        matches!(result, Err(NativeDemandSessionError::Computation(ref e)) if e.to_string() == "primary restoration refusal")
    );
}

#[test]
fn observed_single_driver_awaits_raw_restoration_and_preserves_primary_error() {
    use std::cell::Cell;

    struct Observer {
        prior: AcceptedNativeDemandSnapshot,
        calls: Cell<usize>,
        reject: bool,
    }
    impl DiagnosticObserver for Observer {
        fn after_restore<'a>(
            &'a self,
            runtime: &'a WorkspaceRuntime,
            prepared: &'a Result<PreparedSelectedSourceView, NativeDemandSessionError>,
        ) -> Pin<Box<dyn Future<Output = Result<(), NativeDemandSessionError>> + 'a>> {
            Box::pin(async move {
                self.calls.set(self.calls.get() + 1);
                {
                    let state = runtime.native_demand_sessions.state.lock().unwrap();
                    assert!(matches!(state.phase, NativeDemandLeasePhase::Idle));
                    let actual = &state.accepted;
                    assert_eq!(actual.inputs, self.prior.inputs);
                    assert_eq!(actual.repository_results, self.prior.repository_results);
                    assert_eq!(actual.path_observations, self.prior.path_observations);
                    assert_eq!(actual.download_results, self.prior.download_results);
                    assert_eq!(actual.selected, self.prior.selected);
                    assert_eq!(actual.events, self.prior.events);
                    assert_eq!(
                        actual.repository_environment_frontier,
                        self.prior.repository_environment_frontier
                    );
                }
                // Direct awaits inside the one enclosing cleanup executor;
                // no observer calls block_on or selects another source owner.
                let workspace = runtime.repository_materializer.workspace();
                let mut transaction = runtime.dice.updater().commit().await;
                let raw_runtime = transaction
                    .compute(&slug_bzlmod_v2::RuntimeSourceDeclarationPolicyKey::new(
                        workspace.dupe(),
                    ))
                    .await
                    .unwrap();
                let raw_guest = transaction
                    .compute(&slug_bzlmod_v2::RawGuestNamespacePolicyKey::new(
                        workspace.dupe(),
                    ))
                    .await
                    .unwrap();
                assert_eq!(raw_runtime, self.prior.inputs.request.runtime_sources);
                assert_eq!(raw_guest, self.prior.inputs.request.guest_namespace);
                drop(transaction);
                runtime.dice.wait_for_idle().await;
                assert_eq!(prepared.is_err(), self.reject);
                if self.reject {
                    Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
                        "observer rejection"
                    )))
                } else {
                    Ok(())
                }
            })
        }
    }

    for reject in [false, true] {
        let (f, diagnostic, inputs) = fixture();
        let (engine, images, prior) = diagnostic.inspect_for_test(|runtime, _| {
            (
                Arc::downgrade(&runtime.dice),
                runtime.image_resources.clone(),
                runtime
                    .native_demand_sessions
                    .state
                    .lock()
                    .unwrap()
                    .accepted
                    .clone(),
            )
        });
        let observer = Observer {
            prior,
            calls: Cell::new(0),
            reject,
        };
        let hook = |phase: selected_tool_source::ToolPhase<'_>| {
            if reject && matches!(phase, selected_tool_source::ToolPhase::BeforeCapture) {
                Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
                    "primary source refusal"
                )))
            } else {
                Ok(())
            }
        };
        let name = ApparentRepoName::new("proof").unwrap();
        let outcome = diagnostic.prepare_once_observed(
            SelectedSourcePreparationRequest {
                apparent_repository: &name,
                inputs: &inputs,
                scratch_policy: *f.key.policy(),
                tool_policy: selected_tool_source::tests::key(&f).policy(),
            },
            &hook,
            &observer,
        );
        assert_eq!(observer.calls.get(), 1);
        assert!(engine.upgrade().is_none());
        if reject {
            assert!(
                matches!(outcome, Err(NativeDemandSessionError::Computation(ref e)) if e.to_string() == "primary source refusal")
            );
        } else {
            let view = outcome.unwrap();
            assert_eq!(view.inputs(), &inputs);
            assert!(view.view().is_valid());
            drop(view);
        }
        assert_eq!(images.logical_bytes_used(), 0);
    }
}
