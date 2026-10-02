//! Genuine normal configured sources and one restored retained composite lease.
use std::cell::Cell;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::time::Duration;
use std::time::Instant;

use selected_tool_source::tests::Fixture;
use selected_tool_source::tests::discover_owner;
use selected_tool_source::tests::literal;
use selected_tool_source::tests::scratch_program;
use selected_tool_source::tests::wire;
use slug_bzlmod_v2::RuntimeSourceDeclarationInput;
use slug_bzlmod_v2::RuntimeSourceDeclarationPolicyKey;
use slug_bzlmod_v2::RuntimeSourceLimits;
use slug_bzlmod_v2::RuntimeSourceSelectorInput;
use slug_bzlmod_v2::SelectedToolOperand;
use slug_bzlmod_v2::SelectedToolRole;
use slug_bzlmod_v2::SelectedToolSourcesPolicy;
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::ImageChargedPath;

use super::*;

fn key(f: &Fixture) -> RuntimeSourceProfileKey {
    RuntimeSourceProfileKey::new(selected_tool_source::tests::key(f).requests_key().clone())
}
fn policy(
    f: &Fixture,
    rows: &[RuntimeSourceDeclarationInput<'_>],
    limits: RuntimeSourceLimits,
) -> RuntimeSourceDeclarationPolicy {
    let budget = f.runtime.image_resources.descriptor_budget();
    let workspace = ImageChargedPath::pin_existing(
        f.runtime.repository_materializer.workspace().dupe(),
        budget,
    )
    .unwrap();
    RuntimeSourceDeclarationPolicy::new(&workspace, rows, limits, budget).unwrap()
}
fn host_policy(f: &Fixture, host: &Path, guest: &str) -> RuntimeSourceDeclarationPolicy {
    policy(
        f,
        &[RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                anchor: host.to_str().unwrap(),
            },
            guest_prefix: guest,
        }],
        RuntimeSourceLimits::default(),
    )
}
fn run(f: &Fixture, p: &RuntimeSourceDeclarationPolicy) -> CertifiedRuntimeSourcesLease {
    f.runtime
        .prepare_selected_runtime_sources(key(f), Some(p.dupe()))
        .unwrap()
}
fn retired(
    payload: &slug_bzlmod_v2::SelectedScratchPayloadBudget,
    descriptor: &slug_workspace_v2::ImageDescriptorBudget,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while (payload.used_bytes() != 0 || descriptor.used_bytes() != 0) && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(payload.used_bytes(), 0);
    assert_eq!(descriptor.used_bytes(), 0);
}

#[test]
fn runtime_sources_outside_host_workspace_policy_a_b_a_and_last_drop() {
    let f = Fixture::new(&scratch_program(1));
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("asset"), "external source").unwrap();
    fs::create_dir(f.workspace.path().join("runtime")).unwrap();
    fs::write(
        f.workspace.path().join("runtime/config"),
        "workspace source",
    )
    .unwrap();
    fs::set_permissions(host.path().join("asset"), fs::Permissions::from_mode(0o640)).unwrap();
    let mut retained = Vec::new();
    for guest in ["/runtime-a", "/runtime-b", "/runtime-a"] {
        let p = policy(
            &f,
            &[
                RuntimeSourceDeclarationInput {
                    source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                        anchor: host.path().to_str().unwrap(),
                    },
                    guest_prefix: guest,
                },
                RuntimeSourceDeclarationInput {
                    source: RuntimeSourceSelectorInput::WorkspaceRelativeDirectory {
                        relative: "runtime",
                    },
                    guest_prefix: "/workspace-runtime",
                },
            ],
            RuntimeSourceLimits::default(),
        );
        let lease = run(&f, &p);
        assert_eq!(lease.profile().images().len(), 2);
        assert_eq!(lease.profile().tool_sources().sources().len(), 1);
        assert_eq!(
            lease
                .profile()
                .requests()
                .requests()
                .next()
                .unwrap()
                .guest_prefix()
                .as_str(),
            guest
        );
        assert_eq!(
            lease
                .profile()
                .images()
                .next()
                .unwrap()
                .anchor_root()
                .as_path(),
            host.path()
        );
        assert_eq!(
            fs::read_to_string(
                lease.0.get().images[0]
                    .as_ref()
                    .unwrap()
                    .test_path()
                    .join("asset")
            )
            .unwrap(),
            "external source"
        );
        assert_eq!(
            fs::metadata(
                lease.0.get().images[0]
                    .as_ref()
                    .unwrap()
                    .test_path()
                    .join("asset")
            )
            .unwrap()
            .permissions()
            .mode()
                & 0o7777,
            0o640
        );
        assert_eq!(
            fs::read_to_string(
                lease.0.get().images[1]
                    .as_ref()
                    .unwrap()
                    .test_path()
                    .join("config")
            )
            .unwrap(),
            "workspace source"
        );
        assert_eq!(lease.0.get().unused.iter().flatten().count(), 62);
        f.restored();
        retained.push(lease);
    }
    assert_ne!(
        retained[0].profile().requests().policy(),
        retained[1].profile().requests().policy()
    );
    assert_eq!(
        retained[0].profile().requests().policy(),
        retained[2].profile().requests().policy()
    );
    assert_eq!(
        retained[0].profile().images().next().unwrap().content(),
        retained[2].profile().images().next().unwrap().content()
    );
    let lease = retained.pop().unwrap();
    let clone = lease.dupe();
    let stage = lease.0.get().images[0]
        .as_ref()
        .unwrap()
        .test_path()
        .to_owned();
    let empty: Vec<_> = lease
        .0
        .get()
        .unused
        .iter()
        .flatten()
        .map(|s| s.test_path().to_owned())
        .collect();
    drop(lease);
    assert!(stage.exists());
    assert!(empty.iter().all(|p| p.exists()));
    drop(clone);
    assert!(!stage.exists());
    assert!(empty.iter().all(|p| !p.exists()));
    drop(retained);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
}

#[test]
fn runtime_sources_genuine_immutable_label_origin_and_tools_share_one_lease() {
    let source = format!(
        r#"
def _tools(ctx):
    ctx.file('REPO.bazel','')
    ctx.file('BUILD.bazel',"exports_files(['marker'])\n")
    ctx.file('marker','producer')
    ctx.file('bin/tool',{},executable=True)
    ctx.file('lib/asset','writer-runtime',executable=False)
tools_repo=repository_rule(implementation=_tools)
def _producer(ctx):
    tools_repo(name='tools')
producer=module_extension(implementation=_producer)
path_tag=tag_class(attrs={{'file':attr.label()}})
def _consumer(ctx):
    for module in ctx.modules:
        for tag in module.tags.path:
            tool=ctx.path(tag.file).dirname.get_child('bin/tool')
            generator=ctx.path('generator')
            ctx.file(generator,{},executable=True)
            ctx.execute([generator,tool])
    fail('no root tag')
ext=module_extension(implementation=_consumer,tag_classes={{'path':path_tag}})
"#,
        literal(&wire(5)),
        literal(&wire(4))
    );
    let mut f = Fixture::new(&source);
    let module = fs::read_to_string(f.workspace.path().join("MODULE.bazel")).unwrap();
    fs::write(f.workspace.path().join("MODULE.bazel"),module.replace("use_repo(e, proof='proof')\n",
        "use_repo(e, proof='proof')\nb=use_extension('//:ext.bzl','producer')\nuse_repo(b,tools='tools')\ne.path(file='@tools//:marker')\n")).unwrap();
    f.key = slug_loading_v2::HostSelectedExtensionScratchPrefixKey::new(
        f.runtime.repository_materializer.workspace().dupe(),
        discover_owner(&f.runtime),
        slug_bzlmod_v2::SelectedScratchPolicy::plain(),
    );
    let tool_policy = SelectedToolSourcesPolicy::new(
        &[
            SelectedToolOperand {
                role: SelectedToolRole::Generator,
                argument_index: 0,
            },
            SelectedToolOperand {
                role: SelectedToolRole::Cargo,
                argument_index: 1,
            },
        ],
        ElfMetadataPolicy::default(),
    )
    .unwrap();
    let k = RuntimeSourceProfileKey::new(
        slug_loading_v2::SelectedExtensionToolSourceRequestsKey::new(f.key.clone(), tool_policy),
    );
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("host-lib"), "host").unwrap();
    fs::create_dir(f.workspace.path().join("runtime")).unwrap();
    fs::write(
        f.workspace.path().join("runtime/workspace-lib"),
        "workspace",
    )
    .unwrap();
    let p = policy(
        &f,
        &[
            RuntimeSourceDeclarationInput {
                source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                    anchor: host.path().to_str().unwrap(),
                },
                guest_prefix: "/host-runtime",
            },
            RuntimeSourceDeclarationInput {
                source: RuntimeSourceSelectorInput::WorkspaceRelativeDirectory {
                    relative: "runtime",
                },
                guest_prefix: "/workspace-runtime",
            },
            RuntimeSourceDeclarationInput {
                source: RuntimeSourceSelectorInput::SelectedToolOriginLabelParent {
                    role: SelectedToolRole::Cargo,
                    children: "lib",
                },
                guest_prefix: "/writer-runtime",
            },
        ],
        RuntimeSourceLimits::default(),
    );
    let lease = f
        .runtime
        .prepare_selected_runtime_sources(k.clone(), Some(p))
        .unwrap();
    assert_eq!(lease.profile().images().len(), 3);
    assert_eq!(lease.profile().tool_sources().sources().len(), 2);
    let selected = lease
        .profile()
        .requests()
        .requests()
        .nth(2)
        .unwrap()
        .selected_label_source()
        .unwrap();
    let (
        _,
        slug_bzlmod_v2::RepositoryMaterialization::Immutable {
            generation_root, ..
        },
    ) = selected.repository().unwrap()
    else {
        panic!("genuine writer")
    };
    let root = generation_root.clone();
    assert_eq!(
        lease
            .profile()
            .images()
            .nth(2)
            .unwrap()
            .source_root()
            .as_path(),
        root.join("lib")
    );
    assert_eq!(
        fs::read_to_string(
            lease.0.get().images[2]
                .as_ref()
                .unwrap()
                .test_path()
                .join("asset")
        )
        .unwrap(),
        "writer-runtime"
    );
    assert!(
        !lease.0.get().images[2]
            .as_ref()
            .unwrap()
            .test_path()
            .join("marker")
            .exists()
    );
    f.restored();
    // Obtain each spelling from the current normal plan in the same command
    // that checks shadowing: generated roots have command-session identity.
    for ancestor in [false, true] {
        let (mut guard, plan, _) = current_plan(&f, lease.profile().requests().policy(), &k);
        let spelling = plan
            .tool_requests()
            .requests()
            .nth(1)
            .unwrap()
            .selection()
            .spelling();
        assert!(!Path::new(spelling).starts_with("/__slug_module"));
        let guest = if ancestor {
            Path::new(spelling).parent().unwrap().to_str().unwrap()
        } else {
            spelling
        };
        let shadow = policy(
            &f,
            &[RuntimeSourceDeclarationInput {
                source: RuntimeSourceSelectorInput::SelectedToolOriginLabelParent {
                    role: SelectedToolRole::Cargo,
                    children: "lib",
                },
                guest_prefix: guest,
            }],
            RuntimeSourceLimits::default(),
        );
        let mut inputs = guard.command().inputs.clone();
        inputs.request.runtime_sources = Some(shadow);
        guard.command.as_mut().unwrap().inputs = inputs;
        factory::reset_opens();
        guard.begin_attempt().unwrap();
        let error = match f.runtime.runtime.block_on(attempt::plan(
            &f.runtime,
            &mut guard,
            &k,
            Instant::now() + Duration::from_secs(10),
        )) {
            Err(error) => error,
            Ok(_) => panic!("current normal Label tool shadow was admitted"),
        };
        f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
        assert!(
            matches!(error, NativeDemandSessionError::Computation(ref error)
            if matches!(error.downcast_ref::<slug_loading_v2::RuntimeSourcesError>(),
                Some(slug_loading_v2::RuntimeSourcesError::InvalidScope)))
        );
        assert_eq!(factory::opens(), 0);
        guard.discard().unwrap();
        f.restored();
    }
    drop(k);
    let payload = f.runtime.scratch_resources.payload_budget().dupe();
    let descriptor = f.runtime.image_resources.descriptor_budget().dupe();
    let stage = lease.0.get().images[2]
        .as_ref()
        .unwrap()
        .test_path()
        .to_owned();
    drop(f);
    drop(host);
    assert!(stage.exists());
    assert!(root.exists());
    drop(lease);
    assert!(!stage.exists());
    assert!(!root.exists());
    retired(&payload, &descriptor);
}

#[test]
fn runtime_sources_none_and_ordinary_none_pressure_restore_warm_refusal() {
    let f = Fixture::new(&scratch_program(1));
    factory::reset_opens();
    assert!(
        f.runtime
            .prepare_selected_runtime_sources(key(&f), None)
            .unwrap_err()
            .to_string()
            .contains("missing runtime source")
    );
    assert_eq!(factory::opens(), 0);
    let budget = f.runtime.image_resources.descriptor_budget();
    let held = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    let mut guard = NativeDemandAbortGuard::new(
        f.runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    guard.begin_attempt().unwrap();
    f.runtime.runtime.block_on(async {
        let data = guard.attempt_user_computation_data().unwrap();
        assert!(
            data.data
                .get::<RuntimeSourceDeclarationTransaction>()
                .is_err()
        );
        let mut updater = f.runtime.dice.updater_with_data(data);
        guard.inject_attempt(&mut updater).unwrap();
        let transaction = f.runtime.request_revision.commit(updater).await;
        drop(transaction);
    });
    f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
    guard.seal_retry().unwrap();
    guard.discard().unwrap();
    drop(held);
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("asset"), "value").unwrap();
    let p = host_policy(&f, host.path(), "/runtime");
    let lease = run(&f, &p);
    let raw = RuntimeSourceDeclarationPolicyKey::new(
        f.runtime.repository_materializer.workspace().dupe(),
    );
    f.runtime.runtime.block_on(async {
        let mut updater = f
            .runtime
            .dice
            .updater_with_data(f.runtime.user_computation_data(None).unwrap());
        let mut transaction = f.runtime.request_revision.commit(updater).await;
        assert_eq!(transaction.compute(&raw).await.unwrap(), None);
        drop(transaction);
        updater = f
            .runtime
            .dice
            .updater_with_data(f.runtime.user_computation_data(None).unwrap());
        updater.changed_to([(raw.clone(), Some(p.dupe()))]).unwrap();
        let mut transaction = f.runtime.request_revision.commit(updater).await;
        match slug_loading_v2::compute_runtime_source_profile(&mut transaction, &key(&f)).await {
            Err(error) | Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(error))) => {
                assert!(error.is_transient());
                assert!(matches!(
                    error,
                    slug_loading_v2::RuntimeSourcesError::Operational
                ));
            }
            Ok(_) => panic!("missing declaration transaction admitted normal source progress"),
        }
        drop(transaction);
        let mut updater = f
            .runtime
            .dice
            .updater_with_data(f.runtime.user_computation_data(None).unwrap());
        updater.changed_to([(raw, None)]).unwrap();
        drop(f.runtime.request_revision.commit(updater).await);
    });
    f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
    drop(lease);
    f.restored();

    // Publish a real lower normal selection with raw policy A as the prior
    // accepted snapshot. The existing acceptance owner installs its matching
    // declaration transaction and complete snapshot in the same updater.
    let (mut seed, _, reachable) = current_plan(&f, &p, &key(&f));
    seed.accept_selected_for_test(reachable).unwrap();
    drop(seed);
    let prior = f
        .runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    assert_eq!(prior.inputs.request.runtime_sources, Some(p.dupe()));
    let b = host_policy(&f, host.path(), "/runtime-b");
    assert_ne!(p, b);
    let prepared_b = run(&f, &b);
    assert_eq!(prepared_b.profile().requests().policy(), &b);
    assert_eq!(
        f.runtime
            .native_demand_sessions
            .state
            .lock()
            .unwrap()
            .accepted
            .inputs,
        prior.inputs
    );
    let raw = RuntimeSourceDeclarationPolicyKey::new(
        f.runtime.repository_materializer.workspace().dupe(),
    );
    f.runtime.runtime.block_on(async {
        let updater = f
            .runtime
            .dice
            .updater_with_data(f.runtime.user_computation_data(None).unwrap());
        let mut transaction = updater.existing_state().await;
        f.runtime.request_revision.bind_cycle_detector(&transaction);
        assert_eq!(transaction.compute(&raw).await.unwrap(), Some(p.dupe()));
        drop(transaction);
    });
    f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
    drop(prepared_b);
}

#[test]
fn runtime_sources_full_frontier_mutations_anchor_redirect_and_recovery() {
    let f = Fixture::new(&scratch_program(1));
    let host = tempfile::tempdir().unwrap();
    let root = host.path().join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("asset"), "source").unwrap();
    let p = host_policy(&f, &root, "/runtime");
    for mutation in 0..4 {
        let changed = Cell::new(false);
        let error = f
            .runtime
            .prepare_selected_runtime_sources_inner(
                key(&f),
                Some(p.dupe()),
                Some(&|phase| {
                    if matches!(phase, selected_tool_source::ToolPhase::BeforeCapture)
                        && !changed.replace(true)
                    {
                        match mutation {
                            0 => fs::write(root.join("asset"), "changed").unwrap(),
                            1 => fs::set_permissions(
                                root.join("asset"),
                                fs::Permissions::from_mode(0o700),
                            )
                            .unwrap(),
                            2 => fs::write(root.join("new-member"), "new").unwrap(),
                            _ => {
                                fs::rename(&root, host.path().join("old-root")).unwrap();
                                fs::create_dir(&root).unwrap();
                                fs::write(root.join("asset"), "replacement").unwrap();
                            }
                        }
                    }
                    Ok(())
                }),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            NativeDemandSessionError::InputImage(input_image::InputImageCaptureError::StaleSource)
        ));
        assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
        f.restored();
        drop(run(&f, &p));
        f.restored();
    }
    fs::remove_dir_all(&root).unwrap();
    symlink(host.path().join("old-root"), &root).unwrap();
    assert!(
        f.runtime
            .prepare_selected_runtime_sources(key(&f), Some(p.dupe()))
            .is_err()
    );
    f.restored();
    fs::remove_file(&root).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("asset"), "repaired").unwrap();
    drop(run(&f, &p));
    let parent = host.path().join("parent");
    fs::create_dir(&parent).unwrap();
    fs::create_dir(parent.join("tree")).unwrap();
    let parent_policy = host_policy(&f, &parent.join("tree"), "/runtime-parent");
    fs::rename(&parent, host.path().join("renamed-parent")).unwrap();
    symlink(host.path().join("renamed-parent"), &parent).unwrap();
    assert!(
        f.runtime
            .prepare_selected_runtime_sources(key(&f), Some(parent_policy))
            .is_err()
    );
    f.restored();
}

#[test]
fn runtime_sources_alias_missing_type_cancel_pressure_and_repair() {
    let mut f = Fixture::new(&scratch_program(1));
    let host = tempfile::tempdir().unwrap();
    let root = host.path().join("tree");
    let p = host_policy(&f, &root, "/runtime");
    assert!(
        f.runtime
            .prepare_selected_runtime_sources(key(&f), Some(p.dupe()))
            .is_err()
    );
    f.restored();
    fs::write(&root, "regular root").unwrap();
    assert!(
        f.runtime
            .prepare_selected_runtime_sources(key(&f), Some(p.dupe()))
            .is_err()
    );
    f.restored();
    fs::remove_file(&root).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("asset"), "regular").unwrap();
    symlink("asset", root.join("alias")).unwrap();
    assert!(
        f.runtime
            .prepare_selected_runtime_sources(key(&f), Some(p.dupe()))
            .is_err()
    );
    f.restored();
    fs::remove_file(root.join("alias")).unwrap();
    let error = f
        .runtime
        .prepare_selected_runtime_sources_inner(
            key(&f),
            Some(p.dupe()),
            Some(&|phase| {
                if matches!(phase, selected_tool_source::ToolPhase::AfterCapture) {
                    return Err(refusal("cancel after real captures"));
                }
                Ok(())
            }),
        )
        .unwrap_err();
    assert!(error.to_string().contains("cancel after real captures"));
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    f.restored();
    let budget = f.runtime.image_resources.descriptor_budget().dupe();
    let held = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    assert!(
        f.runtime
            .prepare_selected_runtime_sources(key(&f), Some(p.dupe()))
            .is_err()
    );
    drop(held);
    f.restored();
    f.runtime.image_resources = input_image::WorkspaceImageResources::new(budget.dupe(), 32);
    assert!(
        f.runtime
            .prepare_selected_runtime_sources(key(&f), Some(p.dupe()))
            .is_err()
    );
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    f.restored();
    f.runtime.image_resources = input_image::WorkspaceImageResources::new(budget, 2 << 30);
    drop(run(&f, &p));
    f.restored();
}

pub(in crate::runtime::dice) fn current_plan<'a>(
    f: &'a Fixture,
    p: &RuntimeSourceDeclarationPolicy,
    k: &RuntimeSourceProfileKey,
) -> (
    NativeDemandAbortGuard<'a>,
    ObservedRuntimeSourceRequests,
    SelectedWorkspaceDemands,
) {
    let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
    inputs.runtime_sources = Some(p.dupe());
    let mut guard = NativeDemandAbortGuard::new(
        f.runtime
            .begin_native_demand_command_with_inputs(inputs)
            .unwrap()
            .into_command(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    for _ in 0..128 {
        guard.begin_attempt().unwrap();
        let result = f
            .runtime
            .runtime
            .block_on(attempt::plan(&f.runtime, &mut guard, k, deadline))
            .unwrap();
        f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
        match result {
            selected_tool_source::attempt::Attempt::Need(need) => {
                selected_tool_source::progress_baseline(&mut guard, &need, deadline).unwrap()
            }
            selected_tool_source::attempt::Attempt::Complete(plan, selected) => {
                validate_plan(
                    guard.command(),
                    k,
                    &plan,
                    selected.sidecars().demands(),
                    deadline,
                )
                .unwrap();
                let (_, reachable, terminal) = selected.sidecars.into_parts();
                terminal.reset_to_idle().unwrap();
                return (guard, plan, reachable);
            }
        }
    }
    panic!("normal runtime request plan made no bounded progress")
}

#[test]
fn runtime_sources_invalid_second_before_first_proof_or_factory_and_live_control() {
    use slug_bzlmod_v2::SelectedScratchElfMetadataNeeds;
    use slug_bzlmod_v2::SelectedScratchElfMetadataTransaction;
    use slug_workspace_v2::ImageObservationOperation;
    use slug_workspace_v2::ImageObservationRequest;
    use slug_workspace_v2::NeedPathObservations;

    use crate::runtime::input_image::elf_metadata;
    struct Restore {
        original: PathBuf,
        hidden: PathBuf,
        armed: bool,
    }
    impl Restore {
        fn new(path: &Path) -> Self {
            let hidden = path.with_extension("runtime-hidden");
            fs::rename(path, &hidden).unwrap();
            Self {
                original: path.to_owned(),
                hidden,
                armed: true,
            }
        }
        fn restore(mut self) {
            fs::rename(&self.hidden, &self.original).unwrap();
            self.armed = false;
        }
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            if self.armed {
                let _ = fs::rename(&self.hidden, &self.original);
            }
        }
    }
    let f = Fixture::new(&scratch_program(9));
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("asset"), "source").unwrap();
    fs::create_dir(f.workspace.path().join("runtime")).unwrap();
    fs::write(f.workspace.path().join("runtime/asset"), "workspace source").unwrap();
    let p = policy(
        &f,
        &[
            RuntimeSourceDeclarationInput {
                source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                    anchor: host.path().to_str().unwrap(),
                },
                guest_prefix: "/runtime",
            },
            RuntimeSourceDeclarationInput {
                source: RuntimeSourceSelectorInput::WorkspaceRelativeDirectory {
                    relative: "runtime",
                },
                guest_prefix: "/workspace-runtime",
            },
        ],
        RuntimeSourceLimits::default(),
    );
    let k = key(&f);
    let deadline = Instant::now() + Duration::from_secs(15);
    let _storage = prepare::Storage::new(&f.runtime).unwrap();
    let (mut guard, plan, reachable) = current_plan(&f, &p, &k);
    let resources = &f.runtime.scratch_resources;
    guard.command.as_mut().unwrap().scratch_metadata = Some(
        selected_tool_source::ScratchMetadataState::new(plan.tool_requests(), resources).unwrap(),
    );
    let scratch = plan
        .tool_requests()
        .requests()
        .next()
        .unwrap()
        .scratch_request()
        .unwrap();
    let first = SourcePreparationNeeds::scratch_elf_metadata(
        SelectedScratchElfMetadataNeeds::singleton(scratch, resources).unwrap(),
    );
    let anchor = ImageChargedPath::pin_existing(
        f.runtime.repository_materializer.workspace().dupe(),
        resources.descriptor_budget(),
    )
    .unwrap();
    let foreign = PathObservationDemand::input_image(
        PathObservationNamespace::Materialization(
            slug_workspace_v2::PathObservationInstanceId::new(999),
        ),
        anchor.dupe(),
        ImageObservationRequest::new(anchor, ImageObservationOperation::Lstat),
    )
    .unwrap();
    let mixed = first
        .try_union(&SourcePreparationNeeds::path(
            NeedPathObservations::singleton(foreign),
        ))
        .unwrap();
    let tool = selected_tool_source::declarations::Catalog::new(
        &f.runtime,
        plan.tool_requests(),
        deadline,
    )
    .unwrap();
    let runtime = catalogue::Catalogue::new(&f.runtime, &k, &plan, &reachable, deadline).unwrap();
    // A valid scratch proof and selected-tree Lstat do not admit directory or
    // digest reads of a traversal ancestor outside the selected subtree.
    let row = plan.requests().nth(1).unwrap();
    let image_key = row.image_key();
    let budget = resources.descriptor_budget();
    let anchor = ImageChargedPath::pin_existing(image_key.anchor_root().dupe(), budget).unwrap();
    let subtree = ImageChargedPath::pin_existing(image_key.source_root().dupe(), budget).unwrap();
    let valid_row = PathObservationDemand::input_image(
        image_key.namespace(),
        subtree,
        ImageObservationRequest::new(anchor.dupe(), ImageObservationOperation::Lstat),
    )
    .unwrap();
    let ancestor_lstat = PathObservationDemand::input_image(
        image_key.namespace(),
        anchor.dupe(),
        ImageObservationRequest::new(anchor.dupe(), ImageObservationOperation::Lstat),
    )
    .unwrap();
    runtime
        .preflight(guard.command(), &[ancestor_lstat])
        .unwrap();
    let limits = image_key.policy().limits();
    for operation in [
        ImageObservationOperation::BoundedDirectory(
            slug_workspace_v2::DirectoryObservationLimits {
                max_entries: limits.max_entries,
                max_name_bytes_total: limits.max_descriptor_bytes,
                max_component_bytes: limits.max_path_bytes,
            },
        ),
        ImageObservationOperation::BoundedDigest {
            max_bytes: limits.max_file_bytes,
        },
    ] {
        let bad = PathObservationDemand::input_image(
            image_key.namespace(),
            anchor.dupe(),
            ImageObservationRequest::new(anchor.dupe(), operation),
        )
        .unwrap();
        let rows = SourcePreparationNeeds::path(NeedPathObservations::singleton(valid_row.clone()))
            .try_union(&SourcePreparationNeeds::path(
                NeedPathObservations::singleton(bad),
            ))
            .unwrap();
        let batch = first.try_union(&rows).unwrap();
        elf_metadata::test_reset_borrowed_read_count();
        factory::reset_opens();
        let error = prepare::progress_sources(
            &mut guard, &batch, &k, &plan, &reachable, &tool, &runtime, deadline,
        )
        .unwrap_err();
        assert!(error.to_string().contains("undeclared image observation"));
        assert_eq!(elf_metadata::test_borrowed_read_count(), 0);
        assert_eq!(factory::opens(), 0);
        assert!(
            guard
                .command()
                .scratch_metadata
                .as_ref()
                .unwrap()
                .transaction()
                .unwrap()
                .get(scratch)
                .is_none()
        );
    }
    elf_metadata::test_reset_borrowed_read_count();
    factory::reset_opens();
    let moved = Restore::new(host.path());
    let error = prepare::progress_sources(
        &mut guard, &mixed, &k, &plan, &reachable, &tool, &runtime, deadline,
    )
    .unwrap_err();
    moved.restore();
    assert!(error.to_string().contains("undeclared image observation"));
    assert_eq!(elf_metadata::test_borrowed_read_count(), 0);
    assert_eq!(factory::opens(), 0);
    assert!(
        guard
            .command()
            .scratch_metadata
            .as_ref()
            .unwrap()
            .transaction()
            .unwrap()
            .get(scratch)
            .is_none()
    );
    guard.discard().unwrap();
    f.restored();
    // Recompute normal facts after rename/restore; do not repair a certificate.
    let _next_storage = prepare::Storage::new(&f.runtime).unwrap();
    let (mut guard, plan, reachable) = current_plan(&f, &p, &k);
    guard.command.as_mut().unwrap().scratch_metadata = Some(
        selected_tool_source::ScratchMetadataState::new(plan.tool_requests(), resources).unwrap(),
    );
    let request = plan
        .tool_requests()
        .requests()
        .next()
        .unwrap()
        .scratch_request()
        .unwrap();
    let need = SourcePreparationNeeds::scratch_elf_metadata(
        SelectedScratchElfMetadataNeeds::singleton(request, resources).unwrap(),
    );
    let mut tool = selected_tool_source::declarations::Catalog::new(
        &f.runtime,
        plan.tool_requests(),
        deadline,
    )
    .unwrap();
    let runtime = catalogue::Catalogue::new(&f.runtime, &k, &plan, &reachable, deadline).unwrap();
    prepare::progress_sources(
        &mut guard, &need, &k, &plan, &reachable, &tool, &runtime, deadline,
    )
    .unwrap();
    assert!(elf_metadata::test_borrowed_read_count() > 0);
    let tx: SelectedScratchElfMetadataTransaction = guard
        .command()
        .scratch_metadata
        .as_ref()
        .unwrap()
        .transaction()
        .unwrap();
    assert!(tx.get(request).is_some());
    guard.begin_attempt().unwrap();
    let need = match f
        .runtime
        .runtime
        .block_on(attempt::sources(
            &f.runtime, &mut guard, &k, &plan, &mut tool, deadline,
        ))
        .unwrap()
    {
        selected_tool_source::attempt::Attempt::Need(need) => need,
        _ => panic!("normal runtime image needs actual source observations"),
    };
    f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
    prepare::progress_sources(
        &mut guard, &need, &k, &plan, &reachable, &tool, &runtime, deadline,
    )
    .unwrap();
    assert!(factory::opens() > 0);
    let row = plan.requests().next().unwrap();
    let other = host_policy(&f, host.path(), "/runtime-other");
    guard
        .command
        .as_mut()
        .unwrap()
        .inputs
        .request
        .runtime_sources = Some(other);
    factory::reset_opens();
    assert!(factory::source(guard.command(), &plan, row, &|| true).is_err());
    assert_eq!(factory::opens(), 0);
    guard
        .command
        .as_mut()
        .unwrap()
        .inputs
        .request
        .runtime_sources = Some(p);
    factory::reset_opens();
    assert!(factory::source(guard.command(), &plan, row, &|| false).is_err());
    assert_eq!(factory::opens(), 0);
    guard.discard().unwrap();
    f.restored();
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
}

#[test]
fn runtime_sources_aggregate_refuses_before_next_tree_and_recovers() {
    let f = Fixture::new(&scratch_program(1));
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let third = tempfile::tempdir().unwrap();
    let missing = third.path().join("not-opened");
    fs::write(first.path().join("asset"), "one").unwrap();
    fs::write(second.path().join("asset"), "two").unwrap();
    let rows = [
        RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                anchor: first.path().to_str().unwrap(),
            },
            guest_prefix: "/runtime-one",
        },
        RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                anchor: second.path().to_str().unwrap(),
            },
            guest_prefix: "/runtime-two",
        },
        RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                anchor: missing.to_str().unwrap(),
            },
            guest_prefix: "/runtime-three",
        },
    ];
    let mut limits = RuntimeSourceLimits::default();
    limits.max_entries = 3;
    let p = policy(&f, &rows, limits);
    factory::reset_opens();
    let error = f
        .runtime
        .prepare_selected_runtime_sources(key(&f), Some(p))
        .unwrap_err();
    assert!(
        matches!(error,NativeDemandSessionError::Computation(ref error)
        if matches!(error.downcast_ref::<slug_loading_v2::RuntimeSourcesError>(),Some(slug_loading_v2::RuntimeSourcesError::AggregateLimit)))
    );
    assert_eq!(factory::opened_declarations() & 3, 3);
    assert_eq!(factory::opened_declarations() & 4, 0);
    f.restored();
    let p = policy(&f, &rows[..2], RuntimeSourceLimits::default());
    let lease = run(&f, &p);
    assert_eq!(lease.profile().totals().entries, 4);
    drop(lease);
    f.restored();
}
