//! Genuine definition/usage checkpoint -> ordinary Needs -> one retained lease.
use std::cell::Cell;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use slug_bzlmod_v2::SelectedScratchPolicy;
use slug_bzlmod_v2::SelectedToolOperand;
use slug_bzlmod_v2::SelectedToolRole;
use slug_bzlmod_v2::SelectedToolSourcesPolicy;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_loading_v2::HostSelectedExtensionScratchPrefixKey;
use slug_workspace_v2::ElfMetadataPolicy;
use tempfile::TempDir;

use super::*;
use crate::runtime::ProcessHostOwner;

pub(in crate::runtime::dice) struct Fixture {
    pub(in crate::runtime::dice) workspace: TempDir,
    pub(in crate::runtime::dice) runtime: WorkspaceRuntime,
    pub(in crate::runtime::dice) key: HostSelectedExtensionScratchPrefixKey,
}
impl Fixture {
    pub(in crate::runtime::dice) fn new(source: &str) -> Self {
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
    pub(in crate::runtime::dice) fn replace(&self, source: &str) {
        fs::write(self.workspace.path().join("ext.bzl"), source).unwrap();
    }
    pub(in crate::runtime::dice) fn restored(&self) {
        let preflight = self.runtime.begin_native_demand_command().unwrap();
        assert!(preflight.path_observations().observations().is_empty());
        assert!(preflight.command.download_results.is_empty());
        NativeDemandAbortGuard::new(preflight.into_command())
            .discard()
            .unwrap();
    }
}

pub(in crate::runtime::dice) fn discover_owner(
    runtime: &WorkspaceRuntime,
) -> Arc<HostSelectedExtensionOwner> {
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

pub(in crate::runtime::dice) fn wire(entry: u8) -> Vec<u8> {
    let mut bytes = vec![0; 64];
    bytes[..9].copy_from_slice(b"\x7fELF\x02\x01\x01\x00\x00");
    bytes[16] = 2;
    bytes[18] = 62;
    bytes[20] = 1;
    bytes[24] = entry;
    bytes[52] = 64;
    bytes[54] = 56;
    bytes
}
pub(in crate::runtime::dice) fn literal(bytes: &[u8]) -> String {
    assert!(
        bytes.iter().all(|byte| *byte < 128),
        "wire fixture is ASCII"
    );
    format!(
        "'{}'",
        bytes
            .iter()
            .map(|byte| format!("\\{byte:03o}"))
            .collect::<String>()
    )
}
pub(in crate::runtime::dice) fn scratch_program(entry: u8) -> String {
    format!(
        "def impl(ctx):\n    tool=ctx.path('bin/generator')\n    ctx.file(tool,{},executable=True)\n    ctx.file('config.json','config',executable=False)\n    ctx.execute([tool,'query',ctx.path('config.json')])\next=module_extension(implementation=impl)\n",
        literal(&wire(entry))
    )
}
pub(in crate::runtime::dice) fn key(fixture: &Fixture) -> SelectedExtensionToolSourcesKey {
    SelectedExtensionToolSourcesKey::new(
        fixture.key.clone(),
        SelectedToolSourcesPolicy::new(
            &[SelectedToolOperand {
                role: SelectedToolRole::Generator,
                argument_index: 0,
            }],
            ElfMetadataPolicy::default(),
        )
        .unwrap(),
    )
}
fn entry(lease: &CertifiedSelectedToolInputsLease) -> u64 {
    let source = lease.certificate().sources().next().unwrap();
    let slug_workspace_v2::ElfMetadataOutcome::Parsed(facts) = source.metadata().outcome() else {
        panic!("certified executable must parse");
    };
    facts.header().entry
}
#[test]
fn native_tools_genuine_scratch_proof_stage_a_b_a_and_last_drop() {
    let fixture = Fixture::new(&scratch_program(1));
    let mut retained = Vec::new();
    for expected in [1, 2, 1] {
        fixture.replace(&scratch_program(expected));
        let lease = fixture
            .runtime
            .prepare_selected_tool_inputs(key(&fixture))
            .unwrap();
        assert_eq!(entry(&lease), expected as u64);
        assert_eq!(lease.certificate().sources().len(), 1);
        assert_eq!(
            fs::read(lease.0.get().scratch.test_path().join("bin/generator")).unwrap(),
            wire(expected)
        );
        assert_eq!(
            fs::metadata(lease.0.get().scratch.test_path().join("bin/generator"))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o755
        );
        assert_eq!(
            lease.certificate().prefix().prefix().process().arguments()[0]
                .scratch_path()
                .unwrap()
                .relative(),
            "bin/generator"
        );
        fixture.restored();
        retained.push(lease);
    }
    assert_eq!(
        retained[0]
            .certificate()
            .sources()
            .next()
            .unwrap()
            .metadata(),
        retained[2]
            .certificate()
            .sources()
            .next()
            .unwrap()
            .metadata()
    );
    assert_ne!(
        retained[0]
            .certificate()
            .sources()
            .next()
            .unwrap()
            .metadata(),
        retained[1]
            .certificate()
            .sources()
            .next()
            .unwrap()
            .metadata()
    );
    // Full freshness Eq is conservative: source file metadata may differ on A/B/A.
    let lease = retained.pop().unwrap();
    let clone = lease.dupe();
    let root = lease.0.get().scratch.test_path().to_owned();
    let empty: Vec<_> = lease
        .0
        .get()
        .unused_stages
        .iter()
        .flatten()
        .map(|stage| stage.test_path().to_owned())
        .collect();
    assert_eq!(empty.len(), 3);
    drop(lease);
    assert!(root.exists());
    assert!(empty.iter().all(|path| path.exists()));
    drop(clone);
    assert!(!root.exists());
    assert!(empty.iter().all(|path| !path.exists()));
    drop(retained);
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
}
#[test]
fn native_tools_root_label_normal_image_elf_and_same_scope_members() {
    let fixture = Fixture::new(
        "def impl(ctx):\n    generator=ctx.path(Label('//:tool'))\n    cargo=ctx.path(Label('//:cargo'))\n    ctx.execute([generator,cargo])\next=module_extension(implementation=impl)\n",
    );
    fs::write(fixture.workspace.path().join("tool"), wire(7)).unwrap();
    fs::write(fixture.workspace.path().join("cargo"), wire(8)).unwrap();
    for name in ["tool", "cargo"] {
        fs::set_permissions(
            fixture.workspace.path().join(name),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    let policy = SelectedToolSourcesPolicy::new(
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
    let lease = fixture
        .runtime
        .prepare_selected_tool_inputs(SelectedExtensionToolSourcesKey::new(
            fixture.key.clone(),
            policy,
        ))
        .unwrap();
    assert_eq!(lease.certificate().sources().len(), 2);
    assert!(
        lease
            .certificate()
            .sources()
            .all(|source| source.label_metadata().is_some())
    );
    assert_eq!(lease.0.get().images.iter().flatten().count(), 1);
    assert_eq!(lease.0.get().image_for_role[..2], [Some(0), Some(0)]);
    let image = lease.0.get().images[0].as_ref().unwrap();
    assert_eq!(fs::read(image.test_path().join("tool")).unwrap(), wire(7));
    assert_eq!(fs::read(image.test_path().join("cargo")).unwrap(), wire(8));
    fixture.restored();
    drop(lease);
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
}
#[test]
fn native_tools_literal_nonexec_malformed_missing_refuse_and_restore() {
    let fixture = Fixture::new(&scratch_program(1));
    for program in [
        "def impl(ctx):\n    ctx.execute(['bin/generator'])\next=module_extension(implementation=impl)\n".to_owned(),
        scratch_program(1).replace("executable=True","executable=False"),
        scratch_program(1).replace(&literal(&wire(1)),"'malformed'"),
        "def impl(ctx):\n    ctx.execute([ctx.path('absent')])\next=module_extension(implementation=impl)\n".to_owned(),
    ] {
        fixture.replace(&program);
        assert!(fixture.runtime.prepare_selected_tool_inputs(key(&fixture)).is_err());
        fixture.restored();assert_eq!(fixture.runtime.image_resources.logical_bytes_used(),0);
    }
    fixture.replace(&scratch_program(1));
    assert_eq!(
        entry(
            &fixture
                .runtime
                .prepare_selected_tool_inputs(key(&fixture))
                .unwrap()
        ),
        1
    );
}
#[test]
fn native_tools_stale_build_and_cancel_cleanup_then_repair() {
    let fixture = Fixture::new(&scratch_program(1));
    let modified = Cell::new(false);
    let error = fixture
        .runtime
        .prepare_selected_tool_inputs_inner(
            key(&fixture),
            Some(&|phase| {
                if matches!(phase, ToolPhase::BeforeCapture) && !modified.replace(true) {
                    fs::write(
                        fixture.workspace.path().join("BUILD.bazel"),
                        "# changed outside tool subtree\n",
                    )
                    .unwrap();
                }
                Ok(())
            }),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        NativeDemandSessionError::InputImage(input_image::InputImageCaptureError::StaleSource)
    ));
    fixture.restored();
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
    let error = fixture
        .runtime
        .prepare_selected_tool_inputs_inner(
            key(&fixture),
            Some(&|phase| {
                if matches!(phase, ToolPhase::AfterCapture) {
                    return Err(refusal("deterministic cancellation"));
                }
                Ok(())
            }),
        )
        .unwrap_err();
    assert!(error.to_string().contains("deterministic cancellation"));
    fixture.restored();
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
    assert_eq!(
        entry(
            &fixture
                .runtime
                .prepare_selected_tool_inputs(key(&fixture))
                .unwrap()
        ),
        1
    );
}

#[test]
fn native_tools_generated_label_writer_pin_and_download_proof() {
    use sha2::Digest;
    // Generator download bytes are genuine file:// transport; the independent
    // writer supplies the Cargo operand, selected through an authentic root tag.
    let artifact = tempfile::NamedTempFile::new().unwrap();
    fs::write(artifact.path(), wire(4)).unwrap();
    let digest = sha2::Sha256::digest(wire(4));
    let digest = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let producer = format!(
        r#"
def _tools(ctx):
    ctx.file('REPO.bazel', '')
    ctx.file('BUILD.bazel', "exports_files(['marker'])\n")
    ctx.file('marker', 'producer-marker')
    ctx.file('bin/tool', {}, executable=True)
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
            ctx.download(url='file://{}',output=generator,sha256='{}',executable=True)
            config=ctx.path('config.json')
            manifest=ctx.path('splicing_manifest.json')
            lockfile=ctx.path(Label('//:marker'))
            ctx.file(config,str(tool),executable=False)
            ctx.file(manifest,'{{}}',executable=False)
            ctx.execute([generator,'query','--lockfile',lockfile,'--config',config,'--splicing-manifest',manifest,'--cargo',tool,'--rustc',generator])
    fail('missing actual root tag')
ext=module_extension(implementation=_consumer,tag_classes={{'path':path_tag}})
"#,
        literal(&wire(5)),
        artifact.path().to_str().unwrap(),
        digest
    );
    let mut fixture = Fixture::new(&producer);
    let module = fs::read_to_string(fixture.workspace.path().join("MODULE.bazel")).unwrap();
    fs::write(fixture.workspace.path().join("MODULE.bazel"),module.replace(
        "use_repo(e, proof='proof')\n",
        "use_repo(e, proof='proof')\nb=use_extension('//:ext.bzl','producer')\nuse_repo(b, tools='tools')\ne.path(file='@tools//:marker')\n")).unwrap();
    fixture.key = HostSelectedExtensionScratchPrefixKey::new(
        fixture.runtime.repository_materializer.workspace().clone(),
        discover_owner(&fixture.runtime),
        SelectedScratchPolicy::plain(),
    );
    let policy = SelectedToolSourcesPolicy::new(
        &[
            SelectedToolOperand {
                role: SelectedToolRole::Generator,
                argument_index: 0,
            },
            SelectedToolOperand {
                role: SelectedToolRole::Cargo,
                argument_index: 9,
            },
            SelectedToolOperand {
                role: SelectedToolRole::Rustc,
                argument_index: 11,
            },
        ],
        ElfMetadataPolicy::default(),
    )
    .unwrap();
    let lease = fixture
        .runtime
        .prepare_selected_tool_inputs(SelectedExtensionToolSourcesKey::new(
            fixture.key.clone(),
            policy,
        ))
        .unwrap();
    assert_eq!(entry(&lease), 4);
    assert_eq!(lease.certificate().sources().len(), 3);
    let argv = lease.certificate().prefix().prefix().process().arguments();
    assert_eq!(argv.len(), 12);
    for (index, spelling) in [
        (1, "query"),
        (2, "--lockfile"),
        (4, "--config"),
        (6, "--splicing-manifest"),
        (8, "--cargo"),
        (10, "--rustc"),
    ] {
        assert_eq!(argv[index].spelling(), spelling);
    }
    assert!(argv[0].scratch_path().is_some());
    assert!(argv[3].external_path().is_some());
    assert!(argv[5].scratch_path().is_some());
    assert!(argv[7].scratch_path().is_some());
    assert!(argv[9].external_path().is_some());
    assert!(argv[11].scratch_path().is_some());
    let generator = lease
        .certificate()
        .sources()
        .next()
        .unwrap()
        .scratch_result()
        .unwrap();
    let rustc = lease
        .certificate()
        .sources()
        .nth(2)
        .unwrap()
        .scratch_result()
        .unwrap();
    assert_eq!(
        generator.request().operand(),
        SelectedToolOperand {
            role: SelectedToolRole::Generator,
            argument_index: 0
        }
    );
    assert_eq!(
        rustc.request().operand(),
        SelectedToolOperand {
            role: SelectedToolRole::Rustc,
            argument_index: 11
        }
    );
    assert_ne!(generator.request(), rustc.request());
    assert_ne!(
        SelectedScratchElfMetadataEpochKey::new(generator.request()),
        SelectedScratchElfMetadataEpochKey::new(rustc.request())
    );
    assert_eq!(
        generator.request().entry_index(),
        rustc.request().entry_index()
    );
    assert_eq!(generator.request().member(), rustc.request().member());
    assert!(std::ptr::eq(
        generator.request().blob().bytes(),
        rustc.request().blob().bytes()
    ));
    assert_eq!(generator.verified(), rustc.verified());
    // Native final validation requires the selected raw cell count to equal
    // these two exact source results; a three-role success proves both cells.
    assert_eq!(
        lease
            .certificate()
            .sources()
            .filter(|source| source.scratch_result().is_some())
            .count(),
        2
    );
    assert_eq!(lease.certificate().prefix().download_requests().len(), 1);
    assert!(
        lease
            .certificate()
            .sources()
            .next()
            .unwrap()
            .scratch_result()
            .is_some()
    );
    let metadata = lease.certificate().sources().nth(1).unwrap();
    let selected = metadata.label_selection().unwrap();
    let (
        _,
        slug_bzlmod_v2::RepositoryMaterialization::Immutable {
            generation_root, ..
        },
    ) = selected.repository().unwrap()
    else {
        panic!("actual writer produces immutable bytes");
    };
    let root = generation_root.clone();
    assert_eq!(fs::read(root.join("bin/tool")).unwrap(), wire(5));
    let image = lease.0.get().images[1].as_ref().unwrap();
    assert_eq!(
        fs::read(image.test_path().join("bin/tool")).unwrap(),
        wire(5)
    );
    assert_eq!(
        fs::read(lease.0.get().scratch.test_path().join("generator")).unwrap(),
        wire(4)
    );
    assert_eq!(
        fs::read_to_string(lease.0.get().scratch.test_path().join("config.json")).unwrap(),
        root.join("bin/tool").to_str().unwrap()
    );
    fixture.restored();
    let payload = fixture.runtime.scratch_resources.payload_budget().dupe();
    let descriptor = fixture.runtime.scratch_resources.descriptor_budget().dupe();
    let stage = lease.0.get().scratch.test_path().to_owned();
    drop(fixture);
    assert!(root.exists());
    assert!(stage.exists());
    assert!(payload.used_bytes() >= 64);
    drop(lease);
    assert!(!stage.exists());
    assert!(!root.exists());
    let deadline = Instant::now() + Duration::from_secs(2);
    while (payload.used_bytes() != 0 || descriptor.used_bytes() != 0) && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(payload.used_bytes(), 0);
    assert_eq!(descriptor.used_bytes(), 0);
}

#[test]
fn native_tools_service_exhaustion_foreign_workspace_and_repair() {
    let mut fixture = Fixture::new(&scratch_program(1));
    let budget = fixture.runtime.scratch_resources.descriptor_budget().dupe();
    let held = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    assert!(matches!(
        fixture.runtime.prepare_selected_tool_inputs(key(&fixture)),
        Err(NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::DescriptorBudget(_)
        ))
    ));
    drop(held);
    fixture.restored();
    fixture.runtime.image_resources = input_image::WorkspaceImageResources::new(budget.dupe(), 32);
    assert!(matches!(
        fixture.runtime.prepare_selected_tool_inputs(key(&fixture)),
        Err(NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::LogicalByteBudget
        ))
    ));
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
    fixture.restored();
    fixture.runtime.image_resources =
        input_image::WorkspaceImageResources::new(budget.dupe(), 2 * 1024 * 1024 * 1024);
    let lease = fixture
        .runtime
        .prepare_selected_tool_inputs(key(&fixture))
        .unwrap();
    assert_eq!(entry(&lease), 1);
    drop(lease);
    let foreign = Fixture::new(&scratch_program(1));
    assert!(
        fixture
            .runtime
            .prepare_selected_tool_inputs(key(&foreign))
            .is_err()
    );
    fixture.restored();
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
}

#[test]
fn native_tools_ready_cells_revoke_in_restoration_and_normal_consumer_refuses() {
    use slug_bzlmod_v2::SelectedScratchElfMetadataEpoch;
    let fixture = Fixture::new(&scratch_program(3));
    let key = key(&fixture);
    let lease = fixture
        .runtime
        .prepare_selected_tool_inputs(key.clone())
        .unwrap();
    let cell = SelectedScratchElfMetadataEpochKey::new(
        lease
            .certificate()
            .sources()
            .next()
            .unwrap()
            .scratch_result()
            .unwrap()
            .request(),
    );
    let mut guard = NativeDemandAbortGuard::new(
        fixture
            .runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    guard.begin_attempt().unwrap();
    fixture.runtime.runtime.block_on(async {
        let mut updater = fixture
            .runtime
            .dice
            .updater_with_data(guard.attempt_user_computation_data().unwrap());
        guard.inject_attempt(&mut updater).unwrap();
        let mut transaction = fixture.runtime.request_revision.commit(updater).await;
        assert!(matches!(
            transaction.compute(&cell).await.unwrap(),
            SelectedScratchElfMetadataEpoch::Unauthorized
        ));
        match slug_loading_v2::compute_selected_tool_sources(&mut transaction, &key).await {
            Ok(SourcePreparationOutcome::Complete(Ok(_))) => {
                panic!("revoked raw cells/userdata cannot produce a normal certificate")
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) | Err(error) => {
                assert!(error.is_transient())
            }
            Ok(SourcePreparationOutcome::Need(_)) => {}
        }
        guard.seal_retry().unwrap();
    });
    fixture
        .runtime
        .runtime
        .block_on(fixture.runtime.dice.wait_for_idle());
    guard.discard().unwrap();
    assert_eq!(entry(&lease), 3);
    drop(lease);
    fixture.restored();
}

// This helper only drives genuine normal requests/Needs; it constructs no
// completed prefix or byte certificate. The caller precreates its owned stage.
fn current_plan<'a>(
    fixture: &'a Fixture,
    key: &SelectedExtensionToolSourcesKey,
    deadline: Instant,
) -> (
    NativeDemandAbortGuard<'a>,
    ObservedSelectedExtensionToolSourceRequests,
    SelectedWorkspaceDemands,
) {
    let mut guard = NativeDemandAbortGuard::new(
        fixture
            .runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    for _ in 0..128 {
        guard.begin_attempt().unwrap();
        let outcome = fixture
            .runtime
            .runtime
            .block_on(attempt::plan(&fixture.runtime, &mut guard, key, deadline))
            .unwrap();
        fixture
            .runtime
            .runtime
            .block_on(fixture.runtime.dice.wait_for_idle());
        match outcome {
            attempt::Attempt::Need(need) => progress_baseline(&mut guard, &need, deadline).unwrap(),
            attempt::Attempt::Complete(plan, selected) => {
                validate_plan(
                    guard.command(),
                    key,
                    &plan,
                    selected.sidecars().demands(),
                    deadline,
                )
                .unwrap();
                let (_, demands, terminal) = selected.sidecars.into_parts();
                terminal.reset_to_idle().unwrap();
                return (guard, plan, demands);
            }
        }
    }
    panic!("ordinary bounded plan did not progress");
}
#[test]
fn native_tools_mixed_need_invalid_second_refuses_before_proof_and_source_io() {
    use slug_bzlmod_v2::SelectedScratchElfMetadataNeeds;
    use slug_workspace_v2::ImageChargedPath;
    use slug_workspace_v2::ImageObservationOperation;
    use slug_workspace_v2::ImageObservationRequest;
    use slug_workspace_v2::NeedPathObservations;

    use crate::runtime::input_image::elf_metadata;
    struct RestoreWorkspace {
        original: PathBuf,
        hidden: PathBuf,
        armed: bool,
    }
    impl RestoreWorkspace {
        fn new(original: &Path) -> Self {
            let hidden = original.with_extension("tool-source-hidden");
            fs::rename(original, &hidden).unwrap();
            Self {
                original: original.to_owned(),
                hidden,
                armed: true,
            }
        }
        fn restore(mut self) {
            fs::rename(&self.hidden, &self.original).unwrap();
            self.armed = false;
        }
    }
    impl Drop for RestoreWorkspace {
        fn drop(&mut self) {
            if self.armed {
                let _ = fs::rename(&self.hidden, &self.original);
            }
        }
    }
    let fixture = Fixture::new(&scratch_program(9));
    let key = key(&fixture);
    let deadline = Instant::now() + Duration::from_secs(10);
    let _stage = selected_scratch_source::PreparedStage::new(&fixture.runtime).unwrap();
    let (mut guard, plan, reachable) = current_plan(&fixture, &key, deadline);
    let resources = &fixture.runtime.scratch_resources;
    let state = ScratchMetadataState::new(&plan, resources).unwrap();
    let request = plan.requests().next().unwrap().scratch_request().unwrap();
    let scratch_need = SourcePreparationNeeds::scratch_elf_metadata(
        SelectedScratchElfMetadataNeeds::singleton(request, resources).unwrap(),
    );
    let anchor = ImageChargedPath::pin_existing(
        fixture.runtime.repository_materializer.workspace().clone(),
        resources.descriptor_budget(),
    )
    .unwrap();
    let unknown = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        anchor.dupe(),
        ImageObservationRequest::new(anchor, ImageObservationOperation::Lstat),
    )
    .unwrap();
    let mixed = scratch_need
        .try_union(&SourcePreparationNeeds::path(
            NeedPathObservations::singleton(unknown),
        ))
        .unwrap();
    let catalog = declarations::Catalog::new(&fixture.runtime, &plan, deadline).unwrap();
    guard.command.as_mut().unwrap().scratch_metadata = Some(state);
    elf_metadata::test_reset_borrowed_read_count();
    let hidden = RestoreWorkspace::new(fixture.workspace.path());
    // If prefix/source IO preceded incoming-batch authorization it would instead
    // observe the absent workspace and yield a stale-source/I/O failure.
    let error = progress_sources(
        &mut guard, &mixed, &key, &plan, &reachable, &catalog, deadline,
    )
    .unwrap_err();
    hidden.restore();
    assert!(error.to_string().contains("undeclared image observation"));
    assert_eq!(elf_metadata::test_borrowed_read_count(), 0);
    assert!(
        guard
            .command()
            .scratch_metadata
            .as_ref()
            .unwrap()
            .transaction()
            .unwrap()
            .get(request)
            .is_none()
    );
    guard.discard().unwrap();
    fixture.restored();
    assert_eq!(fixture.runtime.image_resources.logical_bytes_used(), 0);
    // Recompute the genuine plan after rename/restore, preserving every ancestor
    // fact. Actual successful progress demonstrates the read counter is live.
    let _next_stage = selected_scratch_source::PreparedStage::new(&fixture.runtime).unwrap();
    let (mut guard, plan, reachable) = current_plan(&fixture, &key, deadline);
    let state = ScratchMetadataState::new(&plan, resources).unwrap();
    let request = plan.requests().next().unwrap().scratch_request().unwrap();
    let need = SourcePreparationNeeds::scratch_elf_metadata(
        SelectedScratchElfMetadataNeeds::singleton(request, resources).unwrap(),
    );
    let catalog = declarations::Catalog::new(&fixture.runtime, &plan, deadline).unwrap();
    guard.command.as_mut().unwrap().scratch_metadata = Some(state);
    progress_sources(
        &mut guard, &need, &key, &plan, &reachable, &catalog, deadline,
    )
    .unwrap();
    assert!(elf_metadata::test_borrowed_read_count() > 0);
    assert!(
        guard
            .command()
            .scratch_metadata
            .as_ref()
            .unwrap()
            .transaction()
            .unwrap()
            .get(request)
            .is_some()
    );
    guard.discard().unwrap();
    fixture.restored();
}
