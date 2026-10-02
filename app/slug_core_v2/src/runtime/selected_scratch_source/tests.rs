//! Every staged prefix originates in the actual definition/usage/Need producer.
use std::cell::Cell;
use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use slug_bzlmod_v2::SelectedScratchPayloadBudget;
use slug_bzlmod_v2::SelectedScratchPolicy;
use slug_bzlmod_v2::SelectedScratchResources;

use super::*;
use crate::runtime::ProcessHostOwner;

struct Fixture {
    workspace: TempDir,
    runtime: WorkspaceRuntime,
    key: HostSelectedExtensionScratchPrefixKey,
}
impl Fixture {
    fn new(source: &str) -> Self {
        let workspace = tempfile::tempdir().unwrap();
        let platforms = workspace.path().join("platforms");
        fs::create_dir_all(platforms.join("host")).unwrap();
        fs::write(
            platforms.join("MODULE.bazel"),
            "module(name='platforms', version='1.0.0')\n",
        )
        .unwrap();
        fs::write(
            platforms.join("host/BUILD.bazel"),
            "exports_files(['constraints.bzl'])\nplatform(name='host')\n",
        )
        .unwrap();
        fs::write(
            platforms.join("host/constraints.bzl"),
            "HOST_CONSTRAINTS = []\n",
        )
        .unwrap();
        fs::write(workspace.path().join("MODULE.bazel"),
            "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, proof='proof')\nbazel_dep(name='platforms', version='1.0.0')\nlocal_path_override(module_name='platforms', path='platforms')\n"
        ).unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            "exports_files(['marker'])\n",
        )
        .unwrap();
        fs::write(workspace.path().join("marker"), "marker").unwrap();
        fs::write(workspace.path().join("tool"), "actual-source-tool").unwrap();
        fs::write(workspace.path().join("ext.bzl"), source).unwrap();
        let runtime = WorkspaceRuntime::new(workspace.path(), ProcessHostOwner::native()).unwrap();
        let owner = discover_owner(&runtime);
        let key = HostSelectedExtensionScratchPrefixKey::new(
            runtime.repository_materializer.workspace().clone(),
            owner,
            SelectedScratchPolicy::plain(),
        );
        Self {
            workspace,
            runtime,
            key,
        }
    }
    fn replace(&self, source: &str) {
        fs::write(self.workspace.path().join("ext.bzl"), source).unwrap();
    }
    fn restored(&self) {
        let preflight = self.runtime.begin_native_demand_command().unwrap();
        assert!(preflight.path_observations().observations().is_empty());
        assert!(preflight.command.download_results.is_empty());
        NativeDemandAbortGuard::new(preflight.into_command())
            .discard()
            .unwrap();
    }
}

fn discover_owner(runtime: &WorkspaceRuntime) -> Arc<HostSelectedExtensionOwner> {
    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    let workspace = runtime.repository_materializer.workspace().clone();
    let owner = (0..128)
        .find_map(|_| {
            guard.begin_attempt().unwrap();
            let outcome = runtime.runtime.block_on(async {
                let mut updater = runtime
                    .dice
                    .updater_with_data(guard.attempt_user_computation_data().unwrap());
                guard.inject_attempt(&mut updater).unwrap();
                let mut transaction = runtime.request_revision.commit(updater).await;
                let mapping = transaction
                    .compute(
                        &slug_bzlmod_v2::HostRootRepositoryMappingObservationKey::new(
                            workspace.clone(),
                        ),
                    )
                    .await
                    .unwrap();
                let mapping = match mapping {
                    SourcePreparationOutcome::Need(need) => {
                        guard.seal_retry().unwrap();
                        return Err(need);
                    }
                    SourcePreparationOutcome::Complete(value) => value.unwrap(),
                };
                let requested = mapping
                    .result()
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .view()
                    .unwrap()
                    .mapping()
                    .find(|(name, _)| name.as_str() == "proof")
                    .unwrap()
                    .1
                    .clone();
                let demand = transaction
                    .compute(
                        &slug_bzlmod_v2::HostSelectedExtensionDemandObservationKey::new(
                            workspace.clone(),
                            requested,
                        ),
                    )
                    .await
                    .unwrap();
                guard.seal_retry().unwrap();
                match demand {
                    SourcePreparationOutcome::Need(need) => Err(need),
                    SourcePreparationOutcome::Complete(value) => Ok(value
                        .unwrap()
                        .result()
                        .as_ref()
                        .as_ref()
                        .unwrap()
                        .owner()
                        .clone()),
                }
            });
            runtime.runtime.block_on(runtime.dice.wait_for_idle());
            match outcome {
                Ok(owner) => Some(owner),
                Err(need) => {
                    guard.progress(&need).unwrap();
                    None
                }
            }
        })
        .expect("genuine root definition/usage resolves owner");
    guard.discard().unwrap();
    owner
}

fn program(value: &str) -> String {
    format!(
        r#"
def impl(ctx):
    path = ctx.path('out').get_child('config.json')
    ctx.file(path, 'overwritten', executable=False)
    ctx.file(path, '{value}', executable=True)
    ctx.execute(['tool', path, '--flag'], environment={{'EMPTY':'', 'REMOVE':None}}, working_directory='out', quiet=False)
    fail('post-boundary must never execute')
ext = module_extension(implementation=impl)
"#
    )
}

#[test]
fn native_scratch_real_program_overwrite_modes_a_b_a_and_last_drop() {
    let fixture = Fixture::new(&program("producer-a"));
    let first = fixture
        .runtime
        .stage_selected_scratch_source(fixture.key.clone())
        .unwrap();
    let stage = first.test_path().to_owned();
    assert_eq!(
        fs::read(stage.join("out/config.json")).unwrap(),
        b"producer-a"
    );
    assert_eq!(
        fs::metadata(stage.join("out/config.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );
    assert_eq!(
        fs::metadata(stage.join("out"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );
    let process = first.observed().prefix().process();
    assert_eq!(
        process.arguments()[1].spelling(),
        "/__slug_module/out/config.json"
    );
    assert!(process.arguments()[1].scratch_path().is_some());
    assert_eq!(process.cwd(), "/__slug_module/out");
    assert!(!process.quiet());
    assert_eq!(process.environment()[0].value(), Some(""));
    assert_eq!(process.environment()[1].value(), None);
    let warm = fixture
        .runtime
        .stage_selected_scratch_source(fixture.key.clone())
        .unwrap();
    assert_eq!(first.observed().prefix(), warm.observed().prefix());
    fixture.replace(&program("producer-b-longer"));
    let changed = fixture
        .runtime
        .stage_selected_scratch_source(fixture.key.clone())
        .unwrap();
    assert_eq!(
        fs::read(changed.test_path().join("out/config.json")).unwrap(),
        b"producer-b-longer"
    );
    assert_ne!(first.observed().prefix(), changed.observed().prefix());
    fixture.replace(&program("producer-a"));
    let restored = fixture
        .runtime
        .stage_selected_scratch_source(fixture.key.clone())
        .unwrap();
    assert_eq!(first.observed().prefix(), restored.observed().prefix());
    let held = first.dupe();
    drop(first);
    assert!(stage.exists());
    drop(held);
    assert!(!stage.exists());
    drop((warm, changed, restored));
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
    fixture.restored();
}

fn label_program() -> &'static str {
    r#"
def impl(ctx):
    tool = ctx.path(Label('//:marker')).dirname.get_child('tool')
    missing = ctx.getenv('EXPLICIT_ABSENT')
    config = ctx.path('config.json')
    ctx.file(config, str(tool), executable=False)
    ctx.execute([tool, config], environment={'MISSING':missing})
ext = module_extension(implementation=impl)
"#
}

#[test]
fn native_scratch_label_getenv_download_need_full_origin_and_restore() {
    let fixture = Fixture::new(label_program());
    let artifact = fixture.workspace.path().join("artifact.bin");
    fs::write(&artifact, b"genuine-checksummed-generator").unwrap();
    let checksum = hex::encode(sha2::Sha256::digest(b"genuine-checksummed-generator"));
    fixture.replace(&format!(
        r#"
def impl(ctx):
    tool = ctx.path(Label('//:marker')).dirname.get_child('tool')
    ctx.getenv('EXPLICIT_ABSENT')
    generator = ctx.path('generator')
    ctx.download(url='file://{}', output=generator, sha256='{checksum}', executable=True)
    config = ctx.path('config.json')
    ctx.file(config, str(tool), executable=False)
    ctx.execute([generator, tool, config], environment={{'REMOVE':None, 'EMPTY':''}})
ext = module_extension(implementation=impl)
"#,
        artifact.display()
    ));
    let ancestor = fixture.workspace.path().parent().unwrap();
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(ancestor.to_owned()).unwrap(),
        PathObservationOperation::Lstat,
    );
    let mutations = RefCell::new(Vec::new());
    let refreshed = RefCell::new(None);
    let result = fixture
        .runtime
        .stage_selected_scratch_source_inner(
            fixture.key.clone(),
            Some(&|phase| {
                match phase {
                    StagePhase::BeforeRefresh => {
                        // Keep a new sibling alive so the already observed ancestor
                        // changes deterministically, independent of timestamp granularity.
                        mutations
                            .borrow_mut()
                            .push(tempfile::tempdir_in(ancestor).unwrap());
                    }
                    StagePhase::Refreshed { previous, fresh } => {
                        assert_eq!(previous.observations().len(), fresh.observations().len());
                        assert!(
                            previous
                                .observations()
                                .keys()
                                .zip(fresh.observations().keys())
                                .all(|(left, right)| left == right)
                        );
                        if let Some(old) = previous.get(&demand) {
                            let current = fresh.get(&demand).unwrap();
                            assert_ne!(
                                old, current,
                                "complete raw refresh includes changed ancestor"
                            );
                            *refreshed.borrow_mut() = Some(current.dupe());
                        }
                    }
                    _ => {}
                }
                Ok(())
            }),
        )
        .unwrap();
    assert_eq!(
        result.observed().observations().get(&demand),
        refreshed.borrow().as_ref()
    );
    assert!(
        refreshed.borrow().is_some(),
        "normal certificate derives from refreshed raw facts"
    );
    assert_eq!(
        fs::read(result.test_path().join("generator")).unwrap(),
        b"genuine-checksummed-generator"
    );
    assert_eq!(
        fs::metadata(result.test_path().join("generator"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );
    assert_eq!(
        fs::read_to_string(result.test_path().join("config.json")).unwrap(),
        fixture.workspace.path().join("tool").to_str().unwrap()
    );
    assert_eq!(result.observed().download_requests().len(), 1);
    assert_eq!(result.observed().label_sources().len(), 3);
    assert!(
        result.observed().prefix().process().arguments()[1]
            .external_path()
            .is_some()
    );
    assert_eq!(result.observed().inputs().owner(), fixture.key.owner());
    let payload = fixture.runtime.scratch_resources.payload_budget().dupe();
    let descriptor = fixture.runtime.scratch_resources.descriptor_budget().dupe();
    let path = result.test_path().to_owned();
    fixture.restored();
    drop(fixture);
    // Neither runtime nor transaction lifetime may retire the live source lease.
    assert!(path.exists());
    assert!(payload.used_bytes() >= b"genuine-checksummed-generator".len());
    assert!(descriptor.used_bytes() >= 4352);
    drop(result);
    assert!(!path.exists());
    // The physical lease is already gone. Engine actor shutdown may still own
    // normal cached prefix values; observe their actual last-Drop reservations
    // retiring, without clearing history or changing either strict zero gate.
    let deadline = Instant::now() + std::time::Duration::from_secs(2);
    while (payload.used_bytes() != 0 || descriptor.used_bytes() != 0) && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(payload.used_bytes(), 0);
    assert_eq!(descriptor.used_bytes(), 0);
}

#[test]
fn native_scratch_final_full_build_frontier_stale_and_recovery() {
    let fixture = Fixture::new(label_program());
    let stage = RefCell::new(None);
    let error = fixture.runtime.stage_selected_scratch_source_inner(fixture.key.clone(), Some(&|phase| {
        match phase {
            StagePhase::Prepared(path) => *stage.borrow_mut() = Some(path.to_owned()),
            StagePhase::AfterCopy => fs::write(fixture.workspace.path().join("BUILD.bazel"),
                "exports_files(['marker'])\n# current package frontier changed outside staged tree\n").unwrap(),
            _ => {}
        }
        Ok(())
    })).unwrap_err();
    assert!(
        matches!(error, NativeDemandSessionError::Computation(ref error)
        if matches!(error.downcast_ref::<ScratchSourceFailure>(), Some(ScratchSourceFailure::StaleSource)))
    );
    assert!(!stage.borrow().as_ref().unwrap().exists());
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
    fixture.restored();
    fs::write(
        fixture.workspace.path().join("BUILD.bazel"),
        "exports_files(['marker'])\n",
    )
    .unwrap();
    let recovered = fixture
        .runtime
        .stage_selected_scratch_source(fixture.key.clone())
        .unwrap();
    assert_eq!(
        fs::read_to_string(recovered.test_path().join("config.json")).unwrap(),
        fixture.workspace.path().join("tool").to_str().unwrap()
    );
}

#[test]
fn native_scratch_mid_copy_cancel_error_cleanup_and_restoration() {
    let fixture = Fixture::new(&program(&"x".repeat(128 * 1024)));
    let stage = RefCell::new(None);
    let chunks = Cell::new(0);
    let error = fixture
        .runtime
        .stage_selected_scratch_source_inner(
            fixture.key.clone(),
            Some(&|phase| {
                match phase {
                    StagePhase::Prepared(path) => *stage.borrow_mut() = Some(path.to_owned()),
                    StagePhase::Chunk => {
                        chunks.set(chunks.get() + 1);
                        if chunks.get() == 2 {
                            return Err(failure(ScratchSourceFailure::Cancelled));
                        }
                    }
                    _ => {}
                }
                Ok(())
            }),
        )
        .unwrap_err();
    assert_eq!(chunks.get(), 2);
    assert!(
        matches!(error, NativeDemandSessionError::Computation(ref error)
        if matches!(error.downcast_ref::<ScratchSourceFailure>(), Some(ScratchSourceFailure::Cancelled)))
    );
    assert!(!stage.borrow().as_ref().unwrap().exists());
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
    fixture.restored();
    let recovered = fixture
        .runtime
        .stage_selected_scratch_source(fixture.key.clone())
        .unwrap();
    assert_eq!(
        fs::read(recovered.test_path().join("out/config.json"))
            .unwrap()
            .len(),
        128 * 1024
    );
    // Exercise the actual writer's active callback, after a real partial large
    // write, independently of the driver error-injection hook above.
    let baseline = fixture
        .runtime
        .scratch_resources
        .descriptor_budget()
        .used_bytes();
    let owned = PreparedStage::new(&fixture.runtime).unwrap();
    let copy_active = Cell::new(true);
    let chunks = Cell::new(0);
    let error = write_prefix(
        &owned.owner.root,
        recovered.observed().prefix(),
        &fixture.runtime,
        &|| {
            if copy_active.get() {
                Ok(())
            } else {
                Err(failure(ScratchSourceFailure::Cancelled))
            }
        },
        Some(&|phase| {
            if matches!(phase, StagePhase::Chunk) {
                chunks.set(chunks.get() + 1);
                if chunks.get() == 2 {
                    copy_active.set(false);
                }
            }
            Ok(())
        }),
    )
    .unwrap_err();
    assert_eq!(chunks.get(), 2);
    assert!(
        matches!(error, NativeDemandSessionError::Computation(ref error)
        if matches!(error.downcast_ref::<ScratchSourceFailure>(), Some(ScratchSourceFailure::Cancelled)))
    );
    let owned_path = owned.owner.stage.as_ref().unwrap().path().to_owned();
    drop(owned);
    assert!(!owned_path.exists());
    assert_eq!(
        fixture
            .runtime
            .scratch_resources
            .descriptor_budget()
            .used_bytes(),
        baseline
    );
    drop(recovered);
    let stage = RefCell::new(None);
    let error = fixture
        .runtime
        .stage_selected_scratch_source_inner(
            fixture.key.clone(),
            Some(&|phase| {
                match phase {
                    StagePhase::Prepared(path) => *stage.borrow_mut() = Some(path.to_owned()),
                    StagePhase::BeforeCopy => {
                        let root = stage.borrow();
                        fs::write(
                            root.as_ref().unwrap().join("out"),
                            b"non-directory collision",
                        )
                        .unwrap();
                    }
                    _ => {}
                }
                Ok(())
            }),
        )
        .unwrap_err();
    assert!(
        matches!(error, NativeDemandSessionError::Computation(ref error)
        if matches!(error.downcast_ref::<ScratchSourceFailure>(), Some(ScratchSourceFailure::Io(_))))
    );
    assert!(!stage.borrow().as_ref().unwrap().exists());
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
    fixture.restored();
}

#[test]
fn native_scratch_foreign_workspace_and_physical_budget_repair() {
    let mut fixture = Fixture::new(&program("physical-budget-payload"));
    let foreign = Fixture::new(&program("foreign"));
    let phases = Cell::new(0);
    assert!(
        fixture
            .runtime
            .stage_selected_scratch_source_inner(
                foreign.key.clone(),
                Some(&|_| {
                    phases.set(phases.get() + 1);
                    Ok(())
                })
            )
            .is_err()
    );
    assert_eq!(
        phases.get(),
        0,
        "foreign workspace must precede empty stage/source effects"
    );
    let resources = fixture.runtime.scratch_resources.dupe();
    fixture.runtime.scratch_resources = SelectedScratchResources::new(
        SelectedScratchPayloadBudget::new(1),
        resources.descriptor_budget().dupe(),
    );
    assert!(
        fixture
            .runtime
            .stage_selected_scratch_source(fixture.key.clone())
            .is_err()
    );
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
    fixture.restored();
    fixture.runtime.scratch_resources = resources;
    let restored = fixture
        .runtime
        .stage_selected_scratch_source(fixture.key.clone())
        .unwrap();
    assert_eq!(
        fs::read(restored.test_path().join("out/config.json")).unwrap(),
        b"physical-budget-payload"
    );
    fixture.restored();
}

#[test]
fn native_scratch_whole_frontier_current_batch_refuses_before_source_open() {
    let fixture = Fixture::new(label_program());
    let runtime = &fixture.runtime;
    // Empty credited stage precedes all ordinary ancestor observations, including
    // this test's independently driven real checkpoint attempt.
    let stage = PreparedStage::new(runtime).unwrap();
    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    let deadline = Instant::now() + std::time::Duration::from_secs(2);
    let (observed, selected) = (0..128)
        .find_map(|_| {
            guard.begin_attempt().unwrap();
            let attempt = runtime
                .runtime
                .block_on(prefix_attempt(runtime, &mut guard, &fixture.key, deadline))
                .unwrap();
            runtime.runtime.block_on(runtime.dice.wait_for_idle());
            match attempt {
                PrefixAttempt::Need(need) => {
                    guard.progress(&need).unwrap();
                    None
                }
                PrefixAttempt::Complete(observed, selected) => Some((observed, selected)),
            }
        })
        .expect("actual normal checkpoint completed");
    let reachable = selected.sidecars().demands();
    validate(
        guard.command(),
        &fixture.key,
        &observed,
        reachable,
        deadline,
    )
    .unwrap();
    let missing = observed
        .observations()
        .observations()
        .keys()
        .last()
        .unwrap()
        .clone();
    assert!(observed.observations().observations().len() > 1);
    let original = guard.command().path_observations.dupe();
    guard.command.as_mut().unwrap().path_observations = PathObservationEpoch::from_shared(
        original
            .observations()
            .iter()
            .filter(|(demand, _)| *demand != &missing)
            .map(|(demand, value)| (demand.clone(), value.dupe())),
    )
    .unwrap();
    // The named source authority is unavailable; a factory/open would fail.
    // The entire current-row batch must instead refuse before reaching that IO.
    let hidden = fixture.workspace.path().with_extension("source-hidden");
    fs::rename(fixture.workspace.path(), &hidden).unwrap();
    let error = validate(
        guard.command(),
        &fixture.key,
        &observed,
        reachable,
        deadline,
    )
    .unwrap_err();
    fs::rename(&hidden, fixture.workspace.path()).unwrap();
    assert!(
        matches!(error, NativeDemandSessionError::Computation(ref error)
        if matches!(error.downcast_ref::<ScratchSourceFailure>(), Some(ScratchSourceFailure::Unauthorized)))
    );
    guard.command.as_mut().unwrap().path_observations = original;
    drop(selected);
    guard.discard().unwrap();
    let path = stage.owner.stage.as_ref().unwrap().path().to_owned();
    drop(stage);
    assert!(!path.exists());
    fixture.restored();
}

#[test]
fn native_scratch_immutable_label_writer_pin_survives_discard_and_last_drop() {
    let mut fixture = Fixture::new(
        r#"
def _tools(ctx):
    ctx.file('REPO.bazel', '')
    ctx.file('BUILD.bazel', "exports_files(['marker'])\n")
    ctx.file('marker', 'producer-marker')
    ctx.file('bin/tool', 'producer-tool', executable=True)
tools_repo = repository_rule(implementation=_tools)
def _producer(ctx):
    tools_repo(name='tools')
producer = module_extension(implementation=_producer)
path_tag = tag_class(attrs={'file':attr.label()})
def _consumer(ctx):
    for module in ctx.modules:
        for tag in module.tags.path:
            tool = ctx.path(tag.file).dirname.get_child('bin/tool')
            config = ctx.path('config.json')
            ctx.file(config, str(tool), executable=False)
            ctx.execute([tool, config])
    fail('missing authentic root tag')
ext = module_extension(implementation=_consumer, tag_classes={'path':path_tag})
"#,
    );
    let module = fs::read_to_string(fixture.workspace.path().join("MODULE.bazel")).unwrap();
    fs::write(
        fixture.workspace.path().join("MODULE.bazel"),
        module.replace(
            "use_repo(e, proof='proof')\n",
            "use_repo(e, proof='proof')\nb=use_extension('//:ext.bzl','producer')\nuse_repo(b, tools='tools')\ne.path(file='@tools//:marker')\n",
        ),
    )
    .unwrap();
    fixture.key = HostSelectedExtensionScratchPrefixKey::new(
        fixture.runtime.repository_materializer.workspace().clone(),
        discover_owner(&fixture.runtime),
        SelectedScratchPolicy::plain(),
    );
    let lease = fixture
        .runtime
        .stage_selected_scratch_source(fixture.key.clone())
        .unwrap();
    assert_eq!(lease.observed().label_sources().len(), 3);
    let selected = lease.observed().label_sources().last().unwrap().selection();
    let (request, result) = selected
        .repository()
        .expect("normally selected generated owner");
    let slug_bzlmod_v2::RepositoryMaterialization::Immutable {
        generation_root,
        observation_instance,
        ..
    } = result
    else {
        panic!("actual writer must produce immutable source")
    };
    let root = generation_root.clone();
    assert_eq!(selected.image_key().anchor_root().as_path(), &root);
    assert_eq!(
        selected.image_key().namespace(),
        PathObservationNamespace::Materialization(*observation_instance)
    );
    let slug_bzlmod_v2::RepositoryMaterializationKind::GeneratedFileEffects(plan) = &request.kind
    else {
        panic!("normal repository-rule writer plan")
    };
    for effect in plan.effects() {
        assert_eq!(
            fs::read(root.join(effect.path())).unwrap(),
            effect.content()
        );
    }
    let tool = root.join("bin/tool");
    assert_eq!(selected.selected_file().as_path(), tool);
    assert_eq!(
        fs::read_to_string(lease.test_path().join("config.json")).unwrap(),
        tool.to_str().unwrap()
    );
    assert_eq!(
        lease.observed().prefix().process().arguments()[0].spelling(),
        tool.to_str().unwrap()
    );
    assert!(
        lease.observed().prefix().process().arguments()[0]
            .external_path()
            .is_some()
    );
    fixture.restored();
    let stage = lease.test_path().to_owned();
    let held = lease.dupe();
    drop(lease);
    drop(fixture);
    // The command/session/runtime are gone. Only the actual lease owns the
    // factory-minted source pin and private staged bytes now.
    assert_eq!(fs::read(&tool).unwrap(), b"producer-tool");
    assert!(stage.exists());
    drop(held);
    assert!(!stage.exists());
    assert!(!root.exists());
}

#[test]
fn native_scratch_descriptor_and_logical_exhaustion_repair() {
    let mut fixture = Fixture::new(&program("quota"));
    let descriptor = fixture.runtime.scratch_resources.descriptor_budget().dupe();
    let baseline = descriptor.used_bytes();
    let credit = descriptor
        .try_reserve(descriptor.limit_bytes() - baseline)
        .unwrap();
    let phases = Cell::new(0);
    let error = fixture
        .runtime
        .stage_selected_scratch_source_inner(
            fixture.key.clone(),
            Some(&|_| {
                phases.set(phases.get() + 1);
                Ok(())
            }),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::DescriptorBudget(_)
        )
    ));
    assert_eq!(
        phases.get(),
        0,
        "descriptor refusal precedes stage creation and command/source IO"
    );
    drop(credit);
    assert_eq!(descriptor.used_bytes(), baseline);
    fixture.restored();

    let occurrence = (b"overwritten".len() + b"quota".len()) as u64;
    fixture.runtime.image_resources =
        input_image::WorkspaceImageResources::new(descriptor, occurrence);
    let first = fixture
        .runtime
        .stage_selected_scratch_source(fixture.key.clone())
        .unwrap();
    assert_eq!(
        first.observed().prefix().occurrence_bytes() as u64,
        occurrence
    );
    assert_eq!(
        fixture.runtime.image_resources.logical_bytes_used(),
        occurrence
    );
    let refused_stage = RefCell::new(None);
    let error = fixture
        .runtime
        .stage_selected_scratch_source_inner(
            fixture.key.clone(),
            Some(&|phase| {
                if let StagePhase::Prepared(path) = phase {
                    *refused_stage.borrow_mut() = Some(path.to_owned());
                }
                Ok(())
            }),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::LogicalByteBudget
        )
    ));
    assert!(!refused_stage.borrow().as_ref().unwrap().exists());
    assert_eq!(
        fixture.runtime.image_resources.logical_bytes_used(),
        occurrence
    );
    fixture.restored();
    let held = first.dupe();
    drop(first);
    assert_eq!(
        fixture.runtime.image_resources.logical_bytes_used(),
        occurrence
    );
    drop(held);
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
    let repaired = fixture
        .runtime
        .stage_selected_scratch_source(fixture.key.clone())
        .unwrap();
    assert_eq!(
        fs::read(repaired.test_path().join("out/config.json")).unwrap(),
        b"quota"
    );
    drop(repaired);
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
    fixture.restored();
}
