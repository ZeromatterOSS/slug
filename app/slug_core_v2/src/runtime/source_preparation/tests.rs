//! Genuine imported-owner requests, retained FD reuse and full raw restoration.
use std::cell::Cell;
use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;

use selected_tool_source::tests::Fixture;
use selected_tool_source::tests::scratch_program;
use selected_tool_source::tests::wire;
use slug_bzlmod_v2::GuestNamespaceLimits;
use slug_bzlmod_v2::GuestNamespacePolicy;
use slug_bzlmod_v2::RuntimeSourceDeclarationInput;
use slug_bzlmod_v2::RuntimeSourceDeclarationPolicy;
use slug_bzlmod_v2::RuntimeSourceGuestMappingInput;
use slug_bzlmod_v2::RuntimeSourceLimits;
use slug_bzlmod_v2::RuntimeSourceSelectorInput;
use slug_bzlmod_v2::SelectedToolRole;
use slug_workspace_v2::ImageChargedPath;

use super::*;
use crate::runtime::input_image;

fn guest() -> GuestNamespacePolicy {
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
    .unwrap()
}
fn policy(
    f: &Fixture,
    rows: &[RuntimeSourceDeclarationInput<'_>],
) -> RuntimeSourceDeclarationPolicy {
    let budget = f.runtime.image_resources.descriptor_budget();
    let workspace = ImageChargedPath::pin_existing(
        f.runtime.repository_materializer.workspace().dupe(),
        budget,
    )
    .unwrap();
    RuntimeSourceDeclarationPolicy::new(&workspace, rows, RuntimeSourceLimits::default(), budget)
        .unwrap()
}
fn request<'a>(
    f: &Fixture,
    name: &'a ApparentRepoName,
    inputs: &'a NativeDemandRequestInputBundle,
) -> SelectedSourcePreparationRequest<'a> {
    SelectedSourcePreparationRequest {
        apparent_repository: name,
        inputs,
        scratch_policy: *f.key.policy(),
        tool_policy: selected_tool_source::tests::key(f).policy(),
    }
}
fn read(view: &guest_namespace::PreparedGuestNamespaceViewLease, path: &str) -> Vec<u8> {
    guest_namespace::tests::read(view, path)
}
fn snapshot(runtime: &WorkspaceRuntime) -> AcceptedNativeDemandSnapshot {
    runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone()
}
fn assert_restored(runtime: &WorkspaceRuntime, prior: &AcceptedNativeDemandSnapshot) {
    let actual = snapshot(runtime);
    assert_eq!(actual.inputs, prior.inputs);
    assert_eq!(actual.path_observations, prior.path_observations);
    assert_eq!(actual.repository_results, prior.repository_results);
    assert_eq!(
        actual.repository_environment_frontier,
        prior.repository_environment_frontier
    );
    assert_eq!(actual.selected, prior.selected);
    assert_eq!(actual.events, prior.events);
    runtime.runtime.block_on(runtime.dice.wait_for_idle());
}
fn seed_prior(f: &Fixture, inputs: &NativeDemandRequestInputBundle, name: &ApparentRepoName) {
    let key = SelectedPreparationOwnerKey::new(
        f.runtime.repository_materializer.workspace().dupe(),
        name,
        f.runtime.image_resources.descriptor_budget(),
    )
    .unwrap();
    let mut guard = NativeDemandAbortGuard::new(
        f.runtime
            .begin_native_demand_command_with_inputs(inputs.clone())
            .unwrap()
            .into_command(),
    );
    for _ in 0..128 {
        guard.begin_attempt().unwrap();
        let result = f
            .runtime
            .runtime
            .block_on(attempt::owner(
                &f.runtime,
                &mut guard,
                &key,
                std::time::Instant::now() + std::time::Duration::from_secs(10),
            ))
            .unwrap();
        f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
        match result {
            selected_tool_source::attempt::Attempt::Need(need) => {
                selected_tool_source::progress_baseline(
                    &mut guard,
                    &need,
                    std::time::Instant::now() + std::time::Duration::from_secs(10),
                )
                .unwrap();
            }
            selected_tool_source::attempt::Attempt::Complete(_, selected) => {
                let (_, reachable, terminal) = selected.sidecars.into_parts();
                terminal.reset_to_idle().unwrap();
                guard.accept_selected_for_test(reachable).unwrap();
                return;
            }
        }
    }
    panic!("genuine owner seed did not make progress");
}

#[test]
fn request_selected_empty_runtime_full_caller_and_exact_prior_some_restore() {
    let f = Fixture::new(&scratch_program(1));
    let name = ApparentRepoName::new("proof").unwrap();
    let p = policy(&f, &[]);
    let mut a = NativeDemandRequestInputBundle::normalized_initial();
    a.runtime_sources = Some(p.dupe());
    a.guest_namespace = Some(guest());
    a.command_policy = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        Some("unused@1.0"),
        true,
        f.workspace.path(),
        ["platforms=platforms"],
    )
    .unwrap();
    a.environment_policy =
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("other@2.0")).unwrap();
    a.lockfile_mode = LockfileMode::Off;
    a.registry_urls = RegistryUrls::new(["https://explicit.invalid"]);
    seed_prior(&f, &a, &name);
    let prior = snapshot(&f.runtime);
    assert_eq!(prior.inputs.request, a);
    let mut b = a.clone();
    b.command_policy = BzlmodCommandPolicyKey::from_flags(None, false).unwrap();
    b.environment_policy =
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap();
    b.lockfile_mode = LockfileMode::Update;
    b.registry_urls = RegistryUrls::default_bazel_registry();
    let mut retained = Vec::new();
    for inputs in [&a, &b, &a] {
        let view = f
            .runtime
            .prepare_selected_source_view(request(&f, &name, inputs))
            .unwrap();
        assert_eq!(view.inputs(), inputs);
        assert_eq!(view.selected().owner(), f.key.owner());
        assert_eq!(view.view().facts().profile().images().len(), 0);
        assert_eq!(view.view().sources().test_stage_count(), 0);
        assert_eq!(read(view.view(), "/__slug_module/bin/generator"), wire(1));
        assert_restored(&f.runtime, &prior);
        retained.push(view);
    }
    let mut missing = b.clone();
    missing.runtime_sources = None;
    assert!(
        f.runtime
            .prepare_selected_source_view(request(&f, &name, &missing))
            .is_err()
    );
    assert_restored(&f.runtime, &prior);
    drop(retained);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
}

#[test]
fn request_selected_owner_missing_failure_cancel_restore_full_bundle() {
    let f = Fixture::new(&scratch_program(1));
    let p = policy(&f, &[]);
    let name = ApparentRepoName::new("proof").unwrap();
    let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
    inputs.runtime_sources = Some(p);
    inputs.guest_namespace = Some(guest());
    seed_prior(&f, &inputs, &name);
    let prior = snapshot(&f.runtime);
    let missing = ApparentRepoName::new("absent").unwrap();
    assert!(
        f.runtime
            .prepare_selected_source_view(request(&f, &missing, &inputs))
            .is_err()
    );
    assert_restored(&f.runtime, &prior);
    let called = Cell::new(false);
    let before_restore = Cell::new(false);
    let after_restore = Cell::new(false);
    let held_paths = std::cell::RefCell::new(Vec::new());
    assert!(
        f.runtime
            .prepare_selected_source_view_inner(
                request(&f, &name, &inputs),
                Some(&|phase| {
                    match phase {
                        selected_tool_source::ToolPhase::AfterCapture => {
                            called.set(true);
                            assert!(f.runtime.image_resources.logical_bytes_used() > 0);
                            return Err(refusal("controlled source preparation cancellation"));
                        }
                        selected_tool_source::ToolPhase::BeforeRestore(stages) => {
                            assert!(called.get());
                            before_restore.set(true);
                            stages.visit_stage_paths(&mut |path| {
                                assert!(
                                    path.exists(),
                                    "owned stage must survive until restoration"
                                );
                                held_paths.borrow_mut().push(path.to_owned());
                            });
                            assert!(
                                held_paths.borrow().len() >= 4,
                                "scratch and all three unused tool stages"
                            );
                            assert!(f.runtime.image_resources.logical_bytes_used() > 0);
                        }
                        selected_tool_source::ToolPhase::AfterRestore(stages) => {
                            assert!(before_restore.get());
                            after_restore.set(true);
                            stages.visit_stage_paths(&mut |path| assert!(path.exists()));
                            assert!(held_paths.borrow().iter().all(|p| p.exists()));
                            assert!(f.runtime.image_resources.logical_bytes_used() > 0);
                            assert_restored(&f.runtime, &prior);
                        }
                        _ => {}
                    }
                    Ok(())
                })
            )
            .is_err()
    );
    assert!(called.get() && before_restore.get() && after_restore.get());
    assert!(held_paths.borrow().iter().all(|p| !p.exists()));
    assert_restored(&f.runtime, &prior);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    let repaired = f
        .runtime
        .prepare_selected_source_view(request(&f, &name, &inputs))
        .unwrap();
    assert_eq!(
        read(repaired.view(), "/__slug_module/bin/generator"),
        wire(1)
    );
    drop(repaired);
}

#[test]
fn request_selected_original_covered_library_actual_fd_offset_and_last_drop() {
    let (f, _download, _host, _, key, _) = guest_namespace::tests::composite_fixture();
    let name = ApparentRepoName::new("proof").unwrap();
    let p = policy(
        &f,
        &[RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::SelectedToolOriginLabelParent {
                role: SelectedToolRole::Cargo,
                children: "lib",
            },
            guest_mapping: RuntimeSourceGuestMappingInput::OriginalSource,
        }],
    );
    let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
    inputs.runtime_sources = Some(p.dupe());
    inputs.guest_namespace = Some(guest());
    let prior = snapshot(&f.runtime);
    let req = SelectedSourcePreparationRequest {
        tool_policy: key.profile_key().tool_sources_key().policy(),
        ..request(&f, &name, &inputs)
    };
    let view = f.runtime.prepare_selected_source_view(req).unwrap();
    let sources = view.view().sources();
    let profile = sources.profile();
    assert!(matches!(
        profile.image_binding(0),
        Some(slug_loading_v2::RuntimeSourceImageBinding::CoveredTool { role: 1, .. })
    ));
    let (role, root_entry) = profile.tool_root_member(0).unwrap();
    let path = profile
        .images()
        .next()
        .unwrap()
        .source_root()
        .as_path()
        .join("asset");
    assert_eq!(read(view.view(), path.to_str().unwrap()), b"writer-runtime");
    let tool_fd = sources
        .tools()
        .with_image_root(role, |fd, _| fd.as_raw_fd())
        .unwrap();
    let runtime_fd = sources
        .with_image_source(0, |fd, base, image| {
            assert_eq!(base, Path::new("lib"));
            assert_eq!(image, profile.images().next().unwrap());
            assert_eq!(profile.tool_member(0, 0), Some((role, root_entry)));
            fd.as_raw_fd()
        })
        .unwrap();
    assert_eq!(
        tool_fd, runtime_fd,
        "reuse the actual descriptor, not just equal content"
    );
    assert_eq!(sources.test_captured_count(), 0);
    assert_eq!(sources.test_stage_count(), 1);
    let bytes = profile
        .tool_sources()
        .sources()
        .filter_map(|s| s.label_metadata())
        .next()
        .unwrap()
        .image()
        .entries()
        .iter()
        .map(|e| match e.kind() {
            slug_workspace_v2::InputImageEntryKind::RegularFile(d) => d.size_bytes(),
            _ => 0,
        })
        .sum::<u64>();
    let scratch_bytes = profile
        .tool_sources()
        .prefix()
        .prefix()
        .entries()
        .iter()
        .map(|entry| match entry {
            slug_bzlmod_v2::SelectedScratchEntry::File { blob, .. } => blob.bytes().len() as u64,
            slug_bzlmod_v2::SelectedScratchEntry::Directory { .. } => 0,
        })
        .sum::<u64>();
    assert_eq!(
        f.runtime.image_resources.logical_bytes_used(),
        bytes + scratch_bytes,
        "one Label tool tree plus the independent genuine scratch capture"
    );
    assert_restored(&f.runtime, &prior);
    let clone = view.dupe();
    drop(view);
    assert_eq!(
        read(clone.view(), path.to_str().unwrap()),
        b"writer-runtime"
    );
    drop(clone);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
}

#[test]
fn request_selected_full_environment_and_issued_root_writer_data() {
    let (f, _host, p, _) = selected_label_data::tests::fixture("//:data");
    // Same genuine Root/independent writer, with a normal environment consumer.
    let path = f.workspace.path().join("ext.bzl");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        text.replace(
            "str(root)+'\\n'+str(repo)",
            "str(root)+'\\n'+str(repo)+'\\n'+ctx.getenv('REQUEST_ENV','missing')",
        ),
    )
    .unwrap();
    let name = ApparentRepoName::new("proof").unwrap();
    let prior = snapshot(&f.runtime);
    for value in ["A", "B", "A"] {
        let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
        inputs.runtime_sources = Some(p.dupe());
        inputs.guest_namespace = Some(selected_label_data::tests::enabled(4));
        inputs.repository_environment =
            slug_bzlmod_v2::RepositoryEnvironmentSnapshot::from_canonical(vec![
                slug_bzlmod_v2::RepositoryEnvironmentEntry::new("REQUEST_ENV", value),
            ])
            .unwrap();
        let view = f
            .runtime
            .prepare_selected_source_view(request(&f, &name, &inputs))
            .unwrap();
        assert_eq!(
            view.inputs().repository_environment,
            inputs.repository_environment
        );
        let data = view.view().facts().data_sources().unwrap();
        assert_eq!(data.sources().len(), 2);
        let paths = data
            .requests()
            .requests()
            .map(|r| r.selection().selected_file().as_path().to_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(read(view.view(), paths[0]), b"root-A");
        assert_eq!(read(view.view(), paths[1]), b"generated-data");
        assert_eq!(
            read(view.view(), "/__slug_module/config.json"),
            format!("{}\n{}\n{value}", paths[0], paths[1]).as_bytes()
        );
        assert_restored(&f.runtime, &prior);
    }
}

#[test]
fn request_copy_pressure_before_clone_and_retained_history_credit() {
    let f = Fixture::new(&scratch_program(1));
    let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
    inputs.command_policy =
        BzlmodCommandPolicyKey::from_flags(Some("long_name@1.0,another@2.0"), false).unwrap();
    let budget = f.runtime.image_resources.descriptor_budget();
    let before = budget.used_bytes();
    let held = budget.try_reserve(budget.limit_bytes() - before).unwrap();
    assert!(request::copy_credit(&f.runtime, &inputs).is_err());
    assert_eq!(budget.used_bytes(), budget.limit_bytes());
    drop(held);
    let reservation = request::copy_credit(&f.runtime, &inputs).unwrap();
    let copied = reservation.freeze(inputs.clone()).unwrap();
    assert!(budget.used_bytes() > before);
    let clone = copied.dupe();
    drop(copied);
    assert!(budget.used_bytes() > before);
    drop(clone);
    assert_eq!(budget.used_bytes(), before);
}

#[test]
fn request_materialization_need_refreshes_only_normal_wrapper_and_changed_owner_refuses() {
    for change_owner in [false, true] {
        let (f, _download, _host, p, namespace, _) = guest_namespace::tests::composite_fixture();
        let name = ApparentRepoName::new("proof").unwrap();
        fs::write(
            f.workspace.path().join("second.bzl"),
            scratch_program(8).replace("ext=module_extension", "other=module_extension"),
        )
        .unwrap();
        let original_module = fs::read_to_string(f.workspace.path().join("MODULE.bazel")).unwrap();
        let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
        inputs.runtime_sources = Some(p);
        inputs.guest_namespace = Some(guest());
        let prior = snapshot(&f.runtime);
        let material_need = Cell::new(false);
        let refreshed = Cell::new(false);
        let sibling = std::cell::RefCell::new(None);
        let result = f.runtime.prepare_selected_source_view_inner(
            SelectedSourcePreparationRequest {
                apparent_repository: &name,
                inputs: &inputs,
                scratch_policy: *f.key.policy(),
                tool_policy: namespace.profile_key().tool_sources_key().policy(),
            },
            Some(&|phase| {
                match phase {
                    selected_tool_source::ToolPhase::BeforeProgress(need)
                        if !need.repository_materializations().is_empty()
                            && !material_need.replace(true) =>
                    {
                        // Hold this real /tmp change until the complete operation
                        // has finished. The normal progress owner reobserves it.
                        *sibling.borrow_mut() = Some(tempfile::tempdir().unwrap());
                        if change_owner {
                            let changed = original_module.replace(
                                "use_repo(e,",
                                "f=use_extension('//:second.bzl','other')\nuse_repo(f,",
                            );
                            fs::write(f.workspace.path().join("MODULE.bazel"), changed).unwrap();
                        }
                    }
                    selected_tool_source::ToolPhase::OwnerRefreshed(old, fresh) if old != fresh => {
                        assert_eq!(old.owner(), fresh.owner());
                        assert_eq!(old.requested(), fresh.requested());
                        refreshed.set(true);
                    }
                    _ => {}
                }
                Ok(())
            }),
        );
        assert!(
            material_need.get(),
            "genuine independent writer must request materialization"
        );
        if change_owner {
            assert!(
                matches!(result, Err(NativeDemandSessionError::Computation(ref e)) if e.to_string().contains("selected source ownership changed"))
            );
        } else {
            let view = result.unwrap();
            assert!(
                refreshed.get(),
                "normal complete owner wrapper must carry the new ancestor observation"
            );
            assert_eq!(view.selected().owner(), f.key.owner());
            drop(view);
        }
        assert_restored(&f.runtime, &prior);
        assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    }
}

#[test]
fn request_post_plan_source_change_keeps_all_stages_through_restore_then_repairs() {
    let f = Fixture::new(&scratch_program(1));
    let name = ApparentRepoName::new("proof").unwrap();
    let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
    inputs.runtime_sources = Some(policy(&f, &[]));
    inputs.guest_namespace = Some(guest());
    let prior = snapshot(&f.runtime);
    let changed = Cell::new(false);
    let restored = Cell::new(false);
    let paths = std::cell::RefCell::new(Vec::new());
    let result = f.runtime.prepare_selected_source_view_inner(
        request(&f, &name, &inputs),
        Some(&|phase| {
            match phase {
                selected_tool_source::ToolPhase::AfterCapture => {
                    changed.set(true);
                    f.replace(&(scratch_program(1) + "\n# changed only after complete capture\n"));
                }
                selected_tool_source::ToolPhase::BeforeRestore(stages) => {
                    assert!(changed.get());
                    stages.visit_stage_paths(&mut |path| {
                        assert!(path.exists());
                        paths.borrow_mut().push(path.to_owned());
                    });
                    assert!(f.runtime.image_resources.logical_bytes_used() > 0);
                }
                selected_tool_source::ToolPhase::AfterRestore(stages) => {
                    stages.visit_stage_paths(&mut |path| assert!(path.exists()));
                    assert!(paths.borrow().iter().all(|p| p.exists()));
                    assert_restored(&f.runtime, &prior);
                    restored.set(true);
                }
                _ => {}
            }
            Ok(())
        }),
    );
    assert!(matches!(
        result,
        Err(NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::StaleSource
        ))
    ));
    assert!(changed.get() && restored.get());
    assert!(paths.borrow().iter().all(|p| !p.exists()));
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    let repaired = f
        .runtime
        .prepare_selected_source_view(request(&f, &name, &inputs))
        .unwrap();
    assert_eq!(
        read(repaired.view(), "/__slug_module/bin/generator"),
        wire(1)
    );
    drop(repaired);
}

#[test]
fn request_failed_real_label_capture_retains_partial_and_unused_stages_during_restore() {
    let f = Fixture::new(
        "def impl(ctx):\n    tool=ctx.path(Label('//:tool'))\n    ctx.execute([tool])\next=module_extension(implementation=impl)\n",
    );
    fs::write(f.workspace.path().join("tool"), wire(3)).unwrap();
    fs::set_permissions(
        f.workspace.path().join("tool"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let name = ApparentRepoName::new("proof").unwrap();
    let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
    inputs.runtime_sources = Some(policy(&f, &[]));
    inputs.guest_namespace = Some(guest());
    let prior = snapshot(&f.runtime);
    let copied = Cell::new(false);
    let before = Cell::new(false);
    let after = Cell::new(false);
    let paths = std::cell::RefCell::new(Vec::new());
    let result = f.runtime.prepare_selected_source_view_inner(
        request(&f, &name, &inputs),
        Some(&|phase| {
            match phase {
                selected_tool_source::ToolPhase::CaptureActivity(path)
                    if path.join("tool").is_file() =>
                {
                    copied.set(true);
                    return Err(refusal("cancel genuine captured Label after file copy"));
                }
                selected_tool_source::ToolPhase::BeforeRestore(stages) => {
                    assert!(copied.get());
                    assert_eq!(stages.partial_image_count(), 1);
                    assert!(f.runtime.image_resources.logical_bytes_used() > 0);
                    stages.visit_stage_paths(&mut |path| {
                        assert!(path.exists());
                        paths.borrow_mut().push(path.to_owned());
                    });
                    assert_eq!(
                        paths.borrow().len(),
                        4,
                        "one partial tool, two unused tools and ready scratch"
                    );
                    before.set(true);
                }
                selected_tool_source::ToolPhase::AfterRestore(stages) => {
                    assert!(before.get());
                    assert_eq!(stages.partial_image_count(), 1);
                    assert!(paths.borrow().iter().all(|p| p.exists()));
                    assert!(f.runtime.image_resources.logical_bytes_used() > 0);
                    assert_restored(&f.runtime, &prior);
                    after.set(true);
                }
                _ => {}
            }
            Ok(())
        }),
    );
    assert!(matches!(
        result,
        Err(NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::Cancelled
        ))
    ));
    assert!(copied.get() && before.get() && after.get());
    assert!(paths.borrow().iter().all(|p| !p.exists()));
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    let repaired = f
        .runtime
        .prepare_selected_source_view(request(&f, &name, &inputs))
        .unwrap();
    assert_eq!(
        read(
            repaired.view(),
            f.workspace.path().join("tool").to_str().unwrap()
        ),
        wire(3)
    );
    drop(repaired);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
}

#[test]
fn scoped_cancel_at_restoration_preserves_full_prior_some_and_drops_captured_images() {
    use crate::runtime::dice::source_service::SourceServiceError;
    use crate::runtime::dice::source_service::SourceServiceLimits;
    use crate::runtime::dice::source_service::SourceServiceScope;
    use crate::runtime::dice::source_service::WorkspaceSourceServices;
    let mut f = Fixture::new(&scratch_program(1));
    let work = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let scope = SourceServiceScope::new_for_test(
        f.runtime.repository_materializer.workspace().dupe(),
        work,
        work + std::time::Duration::from_secs(3),
        SourceServiceLimits::default(),
    )
    .unwrap();
    f.runtime = WorkspaceRuntime::new_with_source_services(
        f.workspace.path(),
        crate::runtime::ProcessHostOwner::native(),
        WorkspaceSourceServices::Diagnostic(scope.clone()),
    )
    .unwrap();
    let name = ApparentRepoName::new("proof").unwrap();
    let mut prior_inputs = NativeDemandRequestInputBundle::normalized_initial();
    prior_inputs.runtime_sources = Some(policy(&f, &[]));
    prior_inputs.guest_namespace = Some(guest());
    prior_inputs.lockfile_mode = LockfileMode::Off;
    prior_inputs.command_policy = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        Some("unused@1.0"),
        true,
        f.workspace.path(),
        ["platforms=platforms"],
    )
    .unwrap();
    seed_prior(&f, &prior_inputs, &name);
    let prior = snapshot(&f.runtime);
    let before_restore = Cell::new(false);
    let after_restore = Cell::new(false);
    let mut inputs = prior_inputs.clone();
    inputs.lockfile_mode = LockfileMode::Update;
    let hook = |phase: selected_tool_source::ToolPhase<'_>| {
        match phase {
            selected_tool_source::ToolPhase::BeforeRestore(_) => {
                before_restore.set(true);
                assert!(f.runtime.image_resources.logical_bytes_used() > 0);
                scope.cancel();
            }
            selected_tool_source::ToolPhase::AfterRestore(_) => {
                after_restore.set(true);
                assert!(f.runtime.image_resources.logical_bytes_used() > 0);
            }
            _ => {}
        }
        Ok(())
    };
    let result = f
        .runtime
        .prepare_selected_source_view_inner(request(&f, &name, &inputs), Some(&hook));
    assert!(matches!(
        result,
        Err(NativeDemandSessionError::SourceService(
            SourceServiceError::Cancelled
        ))
    ));
    assert!(before_restore.get() && after_restore.get());
    assert_restored(&f.runtime, &prior);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
}
