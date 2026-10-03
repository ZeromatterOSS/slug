use std::cell::Cell;
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
    let (diagnostic, inputs) = diagnostic_fixture(&f);
    (f, diagnostic, inputs)
}

fn diagnostic_fixture(
    f: &selected_tool_source::tests::Fixture,
) -> (
    SourcePreparationDiagnosticRuntime,
    NativeDemandRequestInputBundle,
) {
    diagnostic_fixture_with_limits(f, SourceServiceLimits::default())
}

fn diagnostic_fixture_with_limits(
    f: &selected_tool_source::tests::Fixture,
    limits: SourceServiceLimits,
) -> (
    SourcePreparationDiagnosticRuntime,
    NativeDemandRequestInputBundle,
) {
    let workspace = f.runtime.repository_materializer.workspace().clone();
    let work = Instant::now() + Duration::from_secs(10);
    let diagnostic = SourcePreparationDiagnosticRuntime::new(
        workspace,
        crate::runtime::ProcessHostOwner::native(),
        work,
        work + Duration::from_secs(3),
        limits,
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
    (diagnostic, inputs)
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

struct RestoredDiagnosticObserver {
    prior: AcceptedNativeDemandSnapshot,
    calls: Cell<usize>,
    reject: bool,
}
impl DiagnosticObserver for RestoredDiagnosticObserver {
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

#[test]
fn observed_single_driver_awaits_raw_restoration_and_preserves_primary_error() {
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
        let descriptors = images.descriptor_budget().clone();
        let before_peak = descriptors.peak_reserved_bytes();
        let observer = RestoredDiagnosticObserver {
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
        // The real normal driver and raw observer have settled. This is the
        // same bank as the caller's policy and actual prepared source owners.
        let settled_peak = descriptors.peak_reserved_bytes();
        assert!(settled_peak > 0 && settled_peak >= before_peak);
        assert!(settled_peak >= descriptors.used_bytes());
        drop(inputs);
        drop(observer);
        await_descriptor_retirement(&descriptors);
        assert_eq!(descriptors.peak_reserved_bytes(), settled_peak);
    }
}

#[test]
fn actual_local_registry_refusal_retains_cause_and_fresh_policy_repair_releases_owners() {
    use std::os::unix::fs::symlink;

    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalSite;
    use slug_bzlmod_v2::SelectedSourceRefusalTermination;
    use slug_loading_v2::SelectedPreparationOwnerError;

    let f =
        selected_tool_source::tests::Fixture::new(&selected_tool_source::tests::scratch_program(1));
    // Keep the actual platforms overrides. An additional ordinary dependency
    // selects the registry owner without injecting a completed semantic result.
    let module = f.workspace.path().join("MODULE.bazel");
    let mut text = std::fs::read_to_string(&module).unwrap();
    text.push_str("\nbazel_dep(name='refusal_probe', version='1.0')\n");
    std::fs::write(&module, text).unwrap();
    let local = f.workspace.path().join("local_probe");
    std::fs::create_dir(&local).unwrap();
    std::fs::write(
        local.join("MODULE.bazel"),
        "module(name='refusal_probe', version='1.0')\n",
    )
    .unwrap();
    let registry = f.workspace.path().join("refusal_registry");
    let leaf = registry.join("modules/refusal_probe/1.0");
    std::fs::create_dir_all(&leaf).unwrap();
    std::fs::write(registry.join("bazel_registry.json"), "{}").unwrap();
    // O_PATH follows this loop and fails with ELOOP independently of UID/DAC.
    symlink("MODULE.bazel", leaf.join("MODULE.bazel")).unwrap();
    let url = url::Url::from_directory_path(&registry)
        .unwrap()
        .to_string();
    let name = ApparentRepoName::new("proof").unwrap();

    for repaired in [false, true] {
        // Repair is an honest command-policy change on a fresh consumed engine;
        // it is not a same-engine or same-caller warm recovery claim.
        let (diagnostic, mut inputs) = diagnostic_fixture(&f);
        inputs.registry_urls = RegistryUrls::new([url.as_str()]);
        if repaired {
            inputs.command_policy = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
                Some("unused@1.0"),
                true,
                f.workspace.path(),
                ["platforms=platforms", "refusal_probe=local_probe"],
            )
            .unwrap();
        }
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
        let descriptors = images.descriptor_budget().clone();
        let scope = diagnostic.scope.clone();
        let scope_weak = Arc::downgrade(&scope);
        let observer = RestoredDiagnosticObserver {
            prior,
            calls: Cell::new(0),
            reject: !repaired,
        };
        let hook = |_: selected_tool_source::ToolPhase<'_>| Ok(());
        let result = diagnostic.prepare_once_observed(
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
        assert_eq!(
            scope.check(),
            Err(if repaired {
                SourceServiceError::AcquisitionClosed
            } else {
                SourceServiceError::Cancelled
            })
        );
        if repaired {
            let view = result.unwrap();
            assert_eq!(view.inputs(), &inputs);
            assert!(view.view().is_valid());
            let clone = view.dupe();
            drop(view);
            scope.cancel();
            assert_eq!(
                guest_namespace::tests::read(clone.view(), "/__slug_module/bin/generator"),
                selected_tool_source::tests::wire(1)
            );
            assert!(images.logical_bytes_used() > 0);
            drop(scope);
            assert!(scope_weak.upgrade().is_some());
            drop(clone);
        } else {
            let Err(NativeDemandSessionError::Computation(error)) = result else {
                panic!("actual registry refusal must preserve the primary owner error");
            };
            let owner = error
                .downcast_ref::<SelectedPreparationOwnerError>()
                .unwrap();
            let SelectedPreparationOwnerError::SourceSelection { classification, .. } = owner
            else {
                panic!("actual registry refusal must follow source selection");
            };
            assert_eq!(
                classification.termination(),
                SelectedSourceRefusalTermination::Classified
            );
            assert!(classification.variant_path_complete());
            assert!(classification.native_detail_observed());
            let cause = classification.native_cause().unwrap();
            assert_eq!(cause.site, RegistryRefusalSite::LocalPin);
            assert!(
                matches!(cause.detail, RegistryRefusalDetail::Io { errno: Some(code), .. }
                if code == libc::ELOOP)
            );
            drop(error);
            drop(scope);
        }
        assert!(scope_weak.upgrade().is_none());
        assert_eq!(images.logical_bytes_used(), 0);
        let peak = descriptors.peak_reserved_bytes();
        assert!(peak > 0 && peak >= descriptors.used_bytes());
        drop(inputs);
        drop(observer);
        await_descriptor_retirement(&descriptors);
        assert_eq!(descriptors.peak_reserved_bytes(), peak);
    }
}

#[test]
fn normal_selected_owner_registry_limit_pairs_first_origin_and_retires_recorder() {
    use slug_bzlmod_v2::RegistryRefusalCause;
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalScopeCounter;
    use slug_bzlmod_v2::RegistryRefusalScopeFailure;
    use slug_bzlmod_v2::RegistryRefusalSite;
    use slug_bzlmod_v2::SelectedSourceRefusalTermination;
    use slug_loading_v2::SelectedPreparationOwnerError;

    use crate::runtime::dice::source_service::RegistryLimitRelation;

    struct FullRawObserver(RestoredDiagnosticObserver, Cell<bool>);
    impl DiagnosticObserver for FullRawObserver {
        fn after_restore<'a>(
            &'a self,
            runtime: &'a WorkspaceRuntime,
            prepared: &'a Result<PreparedSelectedSourceView, NativeDemandSessionError>,
        ) -> Pin<Box<dyn Future<Output = Result<(), NativeDemandSessionError>> + 'a>> {
            Box::pin(async move {
                // The existing helper checks every accepted field and Runtime/Guest raw cells.
                let checked = self.0.after_restore(runtime, prepared).await;
                let prior = &self.0.prior.inputs.request;
                let workspace = runtime.workspace.clone();
                let mut tx = runtime.dice.updater().commit().await;
                macro_rules! raw {
                    ($key:expr, $expected:expr) => {
                        assert_eq!(tx.compute(&$key).await.unwrap(), $expected);
                    };
                }
                raw!(
                    RootModuleCommandPolicyKey {
                        workspace: workspace.clone()
                    },
                    slug_bzlmod_v2::RootModuleCommandPolicy::from(prior.command_policy.clone())
                );
                raw!(
                    RootModuleEnvironmentPolicyKey {
                        workspace: workspace.clone()
                    },
                    slug_bzlmod_v2::RootModuleEnvironmentPolicy::from(
                        prior.environment_policy.clone()
                    )
                );
                raw!(
                    RootModuleLockfileModeKey {
                        workspace: workspace.clone()
                    },
                    slug_bzlmod_v2::RootModuleLockfileMode::from(prior.lockfile_mode.clone())
                );
                raw!(
                    RootModuleRegistryUrlsKey { workspace },
                    slug_bzlmod_v2::RootModuleRegistryUrls::from(prior.registry_urls.clone())
                );
                raw!(
                    slug_bzlmod_v2::RepositoryEnvironmentSnapshotKey::new(
                        runtime.repository_materializer.workspace().clone()
                    ),
                    prior.repository_environment.clone()
                );
                drop(tx);
                runtime.dice.wait_for_idle().await;
                self.1.set(true);
                checked
            })
        }
    }

    let f =
        selected_tool_source::tests::Fixture::new(&selected_tool_source::tests::scratch_program(1));
    let module = f.workspace.path().join("MODULE.bazel");
    let mut root = std::fs::read_to_string(&module).unwrap();
    root.push_str("\nbazel_dep(name='association_probe',version='1.0')\n");
    std::fs::write(&module, root).unwrap();
    let registry = f.workspace.path().join("association_registry");
    let parent = registry.join("modules/association_probe/1.0");
    let child = registry.join("modules/association_child/1.0");
    std::fs::create_dir_all(&parent).unwrap();
    std::fs::create_dir_all(&child).unwrap();
    let source = "module(name='association_probe',version='1.0')\nbazel_dep(name='association_child',version='1.0')\n";
    std::fs::write(parent.join("MODULE.bazel"), source).unwrap();
    std::fs::write(
        child.join("MODULE.bazel"),
        "module(name='association_child',version='1.0')\n",
    )
    .unwrap();
    let (diagnostic, mut inputs) = diagnostic_fixture_with_limits(
        &f,
        SourceServiceLimits {
            registry_attempts: 1,
            ..SourceServiceLimits::default()
        },
    );
    inputs.registry_urls =
        RegistryUrls::new([url::Url::from_directory_path(&registry).unwrap().as_str()]);
    let (engine, images, prior, recorder) = diagnostic.inspect_for_test(|runtime, scope| {
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
            scope.arm_registry_association_for_test().unwrap(),
        )
    });
    let scope = diagnostic.scope.clone();
    let scope_weak = Arc::downgrade(&scope);
    assert!(recorder.same_scope(&scope));
    let observer = FullRawObserver(
        RestoredDiagnosticObserver {
            prior,
            calls: Cell::new(0),
            reject: true,
        },
        Cell::new(false),
    );
    let result = diagnostic.prepare_once_observed(
        SelectedSourcePreparationRequest {
            apparent_repository: &ApparentRepoName::new("proof").unwrap(),
            inputs: &inputs,
            scratch_policy: *f.key.policy(),
            tool_policy: selected_tool_source::tests::key(&f).policy(),
        },
        &|_| Ok(()),
        &observer,
    );
    assert_eq!(observer.0.calls.get(), 1);
    assert!(observer.1.get());
    assert!(engine.upgrade().is_none());
    let Err(NativeDemandSessionError::Computation(error)) = result else {
        panic!("normal selected owner must retain the registry limit as primary");
    };
    let owner = error
        .downcast_ref::<SelectedPreparationOwnerError>()
        .unwrap();
    let SelectedPreparationOwnerError::SourceSelection { classification, .. } = owner else {
        panic!("registry limit must follow the chosen source predecessor");
    };
    assert_eq!(
        classification.termination(),
        SelectedSourceRefusalTermination::Classified
    );
    assert!(classification.variant_path_complete() && classification.native_detail_observed());
    assert_eq!(
        classification.native_cause(),
        Some(RegistryRefusalCause {
            site: RegistryRefusalSite::Scope,
            detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::Limit(
                RegistryRefusalScopeCounter::RegistryAttempts
            )),
        })
    );
    assert_eq!(
        scope.first_error_for_test(),
        Some(SourceServiceError::Limit(SourceCounter::RegistryAttempts))
    );
    let counters = scope.snapshot();
    assert_eq!(counters.registry_attempts, 1);
    assert_eq!(counters.registry_admitted_bytes, source.len() as u64);
    assert_eq!(counters.source_attempts, 0);
    let observed = recorder.snapshot();
    assert!(observed.complete);
    assert_eq!(observed.admitted_entries, 1);
    assert_eq!(observed.scope_admitted_entries, counters.registry_attempts);
    assert_eq!(observed.admitted_distinct, 1);
    assert_eq!(observed.admitted_repeated, 0);
    assert_eq!(observed.retained_rows, 1);
    assert_eq!(observed.first_limit_call_ordinal, Some(2));
    assert_eq!(
        observed.first_limit_relation,
        RegistryLimitRelation::NewDistinctUnretained { unique_ordinal: 2 }
    );
    assert_eq!(observed.attempted_entries, 2);
    assert_eq!(observed.refused_entries, 1);
    assert_eq!(observed.sticky_refused_entries, 0);
    assert!(recorder.same_scope(&scope));
    assert_eq!(
        observed.attempted_entries,
        observed.admitted_entries + observed.refused_entries
    );
    assert_eq!(counters.live_buffer_bytes, observed.charged_bytes);
    drop(error);
    drop(scope);
    assert!(
        scope_weak.upgrade().is_some(),
        "recorder owns its charged scope pin"
    );
    drop(recorder);
    assert!(
        scope_weak.upgrade().is_none(),
        "no Scope/recorder strong cycle"
    );
    assert_eq!(images.logical_bytes_used(), 0);
    let descriptors = images.descriptor_budget().clone();
    drop(inputs);
    drop(observer);
    await_descriptor_retirement(&descriptors);
}

fn await_descriptor_retirement(budget: &slug_workspace_v2::ImageDescriptorBudget) {
    // Engine Weak retirement precedes asynchronous history/task destruction.
    // Keep the strict zero assertion, bounded by the existing 2s lifecycle gate.
    let deadline = Instant::now() + Duration::from_secs(2);
    while budget.used_bytes() != 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(
        budget.used_bytes(),
        0,
        "actual descriptor history must retire"
    );
}
