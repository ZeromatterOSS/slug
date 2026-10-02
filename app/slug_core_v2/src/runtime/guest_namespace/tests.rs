//! Genuine sources -> normal facts -> same restored private descriptor view.
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
use sha2::Digest;
use slug_bzlmod_v2::GuestNamespaceLimits;
use slug_bzlmod_v2::RawGuestNamespacePolicyKey;
use slug_bzlmod_v2::RuntimeSourceDeclarationInput;
use slug_bzlmod_v2::RuntimeSourceDeclarationPolicy;
use slug_bzlmod_v2::RuntimeSourceLimits;
use slug_bzlmod_v2::RuntimeSourceSelectorInput;
use slug_bzlmod_v2::SelectedToolOperand;
use slug_bzlmod_v2::SelectedToolRole;
use slug_bzlmod_v2::SelectedToolSourcesPolicy;
use slug_loading_v2::GuestLookupError;
use slug_loading_v2::GuestNamespaceOperation;
use slug_loading_v2::RuntimeSourceProfileKey;
use slug_loading_v2::SelectedExtensionToolSourceRequestsKey;
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::ImageChargedPath;

use super::reader::ViewError;
use super::*;

fn guest(epoch: u32) -> GuestNamespacePolicy {
    GuestNamespacePolicy::new(
        1001,
        1002,
        1001,
        1002,
        9,
        7,
        epoch,
        0o755,
        GuestNamespaceLimits::default(),
    )
    .unwrap()
}
fn host_policy(f: &Fixture, host: &Path) -> RuntimeSourceDeclarationPolicy {
    let budget = f.runtime.image_resources.descriptor_budget();
    let workspace = ImageChargedPath::pin_existing(
        f.runtime.repository_materializer.workspace().dupe(),
        budget,
    )
    .unwrap();
    RuntimeSourceDeclarationPolicy::new(
        &workspace,
        &[RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                anchor: host.to_str().unwrap(),
            },
            guest_prefix: "/external",
        }],
        RuntimeSourceLimits::default(),
        budget,
    )
    .unwrap()
}
fn key(f: &Fixture) -> SelectedGuestNamespaceFactsKey {
    SelectedGuestNamespaceFactsKey::new(RuntimeSourceProfileKey::new(
        selected_tool_source::tests::key(f).requests_key().clone(),
    ))
}
fn prepare(
    f: &Fixture,
    p: &RuntimeSourceDeclarationPolicy,
    g: GuestNamespacePolicy,
) -> PreparedGuestNamespaceViewLease {
    f.runtime
        .prepare_selected_guest_namespace_view(key(f), Some(p.dupe()), Some(g))
        .unwrap()
}
fn read(view: &PreparedGuestNamespaceViewLease, path: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    view.read_regular(
        path,
        &mut |chunk| {
            bytes.extend_from_slice(chunk);
            Ok(())
        },
        &|| true,
    )
    .unwrap();
    bytes
}
fn known(
    view: &PreparedGuestNamespaceViewLease,
    path: &str,
    op: GuestNamespaceOperation,
    error: GuestLookupError,
) {
    assert!(matches!(view.lookup(path,op),Err(ViewError::Lookup(actual)) if actual==error));
    assert!(view.is_valid());
}
fn retire(
    payload: &slug_bzlmod_v2::SelectedScratchPayloadBudget,
    budget: &slug_workspace_v2::ImageDescriptorBudget,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while (payload.used_bytes() != 0 || budget.used_bytes() != 0) && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(payload.used_bytes(), 0);
    assert_eq!(budget.used_bytes(), 0);
}

fn source_occurrences(profile: &slug_loading_v2::ObservedRuntimeSourceProfile) -> (usize, usize) {
    // Count complete normal source occurrences, not the final deduplicated table.
    let ancestors = |root: &Path| root.ancestors().skip(1).count();
    let scratch = profile.tool_sources().prefix().prefix();
    let mut total = scratch.entries().len() + ancestors(Path::new(scratch.policy().guest_root()));
    let mut repeated_role = 0;
    for (role, source) in profile.tool_sources().sources().enumerate() {
        if let Some(metadata) = source.label_metadata() {
            let image = metadata.image();
            let rows = image.entries().len() + ancestors(image.source_root().as_path());
            total += rows;
            if role == 2 {
                repeated_role = rows;
            }
        }
    }
    for (request, image) in profile.requests().requests().zip(profile.images()) {
        total += image.entries().len() + ancestors(request.guest_prefix().as_path());
    }
    (total, repeated_role)
}

fn entry_policy(max_entries: usize) -> GuestNamespacePolicy {
    GuestNamespacePolicy::new(
        1001,
        1002,
        1001,
        1002,
        9,
        7,
        8,
        0o755,
        GuestNamespaceLimits {
            max_entries,
            ..GuestNamespaceLimits::default()
        },
    )
    .unwrap()
}

fn composite_fixture() -> (
    Fixture,
    tempfile::TempDir,
    tempfile::TempDir,
    RuntimeSourceDeclarationPolicy,
    SelectedGuestNamespaceFactsKey,
    Vec<u8>,
) {
    let download = tempfile::tempdir().unwrap();
    let generator = wire(4);
    fs::write(download.path().join("generator"), &generator).unwrap();
    let hash = format!("{:x}", sha2::Sha256::digest(&generator));
    let source = format!(
        r#"
def _tools(ctx):
    ctx.file('REPO.bazel','')
    ctx.file('BUILD.bazel',"exports_files(['marker'])\n")
    ctx.file('marker','producer')
    ctx.file('bin/tool',{},executable=True)
    ctx.file('lib/asset','writer-runtime')
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
            ctx.file('config','scratch-config')
            ctx.execute([generator,tool,tool])
    fail('no root tag')
ext=module_extension(implementation=_consumer,tag_classes={{'path':path_tag}})
"#,
        literal(&wire(5)),
        download.path().join("generator").display(),
        hash
    );
    let mut f = Fixture::new(&source);
    let module = fs::read_to_string(f.workspace.path().join("MODULE.bazel")).unwrap();
    fs::write(f.workspace.path().join("MODULE.bazel"),module.replace("use_repo(e, proof='proof')\n","use_repo(e, proof='proof')\nb=use_extension('//:ext.bzl','producer')\nuse_repo(b,tools='tools')\ne.path(file='@tools//:marker')\n")).unwrap();
    f.key = slug_loading_v2::HostSelectedExtensionScratchPrefixKey::new(
        f.runtime.repository_materializer.workspace().dupe(),
        discover_owner(&f.runtime),
        slug_bzlmod_v2::SelectedScratchPolicy::plain(),
    );
    let roles = SelectedToolSourcesPolicy::new(
        &[
            SelectedToolOperand {
                role: SelectedToolRole::Generator,
                argument_index: 0,
            },
            SelectedToolOperand {
                role: SelectedToolRole::Cargo,
                argument_index: 1,
            },
            SelectedToolOperand {
                role: SelectedToolRole::Rustc,
                argument_index: 2,
            },
        ],
        ElfMetadataPolicy::default(),
    )
    .unwrap();
    let k = SelectedGuestNamespaceFactsKey::new(RuntimeSourceProfileKey::new(
        SelectedExtensionToolSourceRequestsKey::new(f.key.clone(), roles),
    ));
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("asset"), "host").unwrap();
    fs::create_dir(f.workspace.path().join("runtime")).unwrap();
    fs::write(f.workspace.path().join("runtime/asset"), "workspace").unwrap();
    let budget = f.runtime.image_resources.descriptor_budget();
    let workspace = ImageChargedPath::pin_existing(
        f.runtime.repository_materializer.workspace().dupe(),
        budget,
    )
    .unwrap();
    let p = RuntimeSourceDeclarationPolicy::new(
        &workspace,
        &[
            RuntimeSourceDeclarationInput {
                source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                    anchor: host.path().to_str().unwrap(),
                },
                guest_prefix: "/external",
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
        budget,
    )
    .unwrap();
    (f, download, host, p, k, generator)
}

#[test]
fn guest_view_genuine_download_writer_workspace_host_original_reads_and_metadata() {
    let (f, download, host, p, k, generator) = composite_fixture();
    let budget = f.runtime.image_resources.descriptor_budget();
    let view = f
        .runtime
        .prepare_selected_guest_namespace_view(k.clone(), Some(p.dupe()), Some(guest(8)))
        .unwrap();
    assert_eq!(view.facts().profile().tool_sources().sources().len(), 3);
    assert_eq!(view.facts().profile().images().len(), 3);
    assert_eq!(read(&view, "/__slug_module/generator"), generator);
    assert_eq!(read(&view, "/__slug_module/config"), b"scratch-config");
    assert_eq!(read(&view, "/external/asset"), b"host");
    assert_eq!(read(&view, "/workspace-runtime/asset"), b"workspace");
    assert_eq!(read(&view, "/writer-runtime/asset"), b"writer-runtime");
    let tool = view
        .facts()
        .profile()
        .tool_sources()
        .sources()
        .nth(1)
        .unwrap()
        .selection()
        .spelling();
    assert_eq!(read(&view, tool), wire(5));
    let duplicate = view
        .facts()
        .profile()
        .tool_sources()
        .sources()
        .nth(2)
        .unwrap();
    assert_eq!(duplicate.selection().spelling(), tool);
    assert_eq!(read(&view, duplicate.selection().spelling()), wire(5));
    let tools = view.0.get().sources.tools();
    let first = tools
        .with_image_root(1, |root, _| nix::sys::stat::fstat(root).unwrap())
        .unwrap();
    let second = tools
        .with_image_root(2, |root, _| nix::sys::stat::fstat(root).unwrap())
        .unwrap();
    assert_eq!((first.st_dev, first.st_ino), (second.st_dev, second.st_ino));
    let parent = Path::new(tool).parent().unwrap().to_str().unwrap();
    assert_eq!(view.list_directory(parent).unwrap().len(), 1);
    assert_eq!(
        view.facts()
            .profile()
            .tool_sources()
            .sources()
            .nth(1)
            .unwrap()
            .label_metadata()
            .unwrap()
            .image(),
        duplicate.label_metadata().unwrap().image()
    );
    let (total, repeated_role) = source_occurrences(view.facts().profile());
    assert!(repeated_role > 0);
    let one_tree_cap = total - repeated_role;
    assert!(view.facts().nodes().len() <= one_tree_cap);
    let exact = f
        .runtime
        .prepare_selected_guest_namespace_view(k.clone(), Some(p.dupe()), Some(entry_policy(total)))
        .unwrap();
    assert_eq!(
        source_occurrences(exact.facts().profile()),
        (total, repeated_role)
    );
    assert_eq!(read(&exact, "/writer-runtime/asset"), b"writer-runtime");
    drop(exact);
    f.restored();
    let error = f
        .runtime
        .prepare_selected_guest_namespace_view(
            k.clone(),
            Some(p.dupe()),
            Some(entry_policy(one_tree_cap)),
        )
        .unwrap_err();
    let NativeDemandSessionError::Computation(error) = error else {
        panic!("occurrence cap must remain a normal namespace refusal")
    };
    assert!(matches!(
        error.downcast_ref::<slug_loading_v2::GuestNamespaceFactsError>(),
        Some(slug_loading_v2::GuestNamespaceFactsError::Limit)
    ));
    f.restored();

    assert_eq!(view.list_directory("/external").unwrap().len(), 1);
    assert_eq!(view.list_directory("/__slug_module").unwrap().len(), 2);
    for (index, node) in view.facts().nodes().iter().enumerate() {
        let m = node.metadata();
        assert_eq!(m.inode, index as u64 + 1);
        assert_eq!(m.device, 9);
        assert_eq!(m.uid, 1001);
        assert_eq!(m.gid, 1002);
        assert_eq!((m.epoch_seconds, m.epoch_nanoseconds), (7, 8));
        assert_eq!(m.block_size, 4096);
        assert_eq!(m.blocks, (m.size + 511) / 512);
        assert_eq!(m.mode & 0o777, node.mode());
        if matches!(node.kind(), GuestNamespaceNodeKind::Directory) {
            assert_eq!(m.size, 4096);
            assert_eq!(
                m.nlink,
                2 + view
                    .facts()
                    .children(index)
                    .iter()
                    .filter(|i| matches!(
                        view.facts().node(**i).unwrap().kind(),
                        GuestNamespaceNodeKind::Directory
                    ))
                    .count() as u64
            );
        } else {
            assert_eq!(m.nlink, 1);
        }
    }
    assert_eq!(
        view.lookup("/__slug_module", GuestNamespaceOperation::Lstat)
            .unwrap()
            .mode(),
        0o755
    );
    known(
        &view,
        "/__slug_module/generator",
        GuestNamespaceOperation::Access(2),
        GuestLookupError::ReadOnly,
    );
    view.lookup("/__slug_module/config", GuestNamespaceOperation::Access(2))
        .unwrap();
    known(
        &view,
        "/writer-runtime/missing",
        GuestNamespaceOperation::Lstat,
        GuestLookupError::Missing,
    );
    f.restored();
    let payload = f.runtime.scratch_resources.payload_budget().dupe();
    let budget = budget.dupe();
    let stage = view.0.get().sources.test_image_path(0).to_owned();
    let clone = view.dupe();
    drop(view);
    assert!(stage.exists());
    drop(p);
    drop(k);
    drop(f);
    drop(host);
    drop(download);
    assert_eq!(read(&clone, "/external/asset"), b"host");
    drop(clone);
    assert!(!stage.exists());
    retire(&payload, &budget);
}

#[test]
fn guest_view_certified_errno_sequential_path_and_shared_sticky_unknown() {
    let f = Fixture::new(&scratch_program(1));
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("asset"), "host").unwrap();
    fs::set_permissions(host.path().join("asset"), fs::Permissions::from_mode(0o666)).unwrap();
    fs::create_dir(host.path().join("denied")).unwrap();
    // The daemon must be able to enumerate the actual source. Guest denial
    // uses explicit nonowner credentials, rather than denying the capturer.
    fs::set_permissions(
        host.path().join("denied"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let p = host_policy(&f, host.path());
    let caller = GuestNamespacePolicy::new(
        1001,
        1002,
        1003,
        1004,
        9,
        7,
        8,
        0o755,
        GuestNamespaceLimits::default(),
    )
    .unwrap();
    let view = prepare(&f, &p, caller);
    for path in [
        "/external/asset/..",
        "/external/asset/.",
        "/external/asset/",
    ] {
        known(
            &view,
            path,
            GuestNamespaceOperation::Stat,
            GuestLookupError::NotDirectory,
        );
    }
    known(
        &view,
        "/external/nope",
        GuestNamespaceOperation::Stat,
        GuestLookupError::Missing,
    );
    known(
        &view,
        "/external/denied/nope",
        GuestNamespaceOperation::Stat,
        GuestLookupError::PermissionDenied,
    );
    known(
        &view,
        "/external/asset",
        GuestNamespaceOperation::ReadLink,
        GuestLookupError::NotSymlink,
    );
    known(
        &view,
        "/external",
        GuestNamespaceOperation::ReadRegular,
        GuestLookupError::IsDirectory,
    );
    known(
        &view,
        "/external/asset",
        GuestNamespaceOperation::Access(2),
        GuestLookupError::ReadOnly,
    );
    assert_eq!(read(&view, "//external/./asset"), b"host");
    assert_eq!(read(&view, "/external/../external/asset"), b"host");
    let clone = view.dupe();
    assert!(matches!(
        view.lookup("/unowned", GuestNamespaceOperation::Lstat),
        Err(ViewError::Lookup(GuestLookupError::Unowned))
    ));
    assert!(!clone.is_valid());
    assert!(matches!(
        clone.lookup("/external/asset", GuestNamespaceOperation::Lstat),
        Err(ViewError::Invalid)
    ));
    f.restored();
}

#[test]
fn guest_view_capture_tamper_bytes_mode_member_link_poison_without_semantic_rewrite() {
    let f = Fixture::new(&scratch_program(1));
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("asset"), "host").unwrap();
    let p = host_policy(&f, host.path());
    for kind in 0..4 {
        let view = prepare(&f, &p, guest(8));
        let stage = view.0.get().sources.test_image_path(0);
        let asset = stage.join("asset");
        match kind {
            0 => fs::write(&asset, "evil").unwrap(),
            1 => fs::set_permissions(&asset, fs::Permissions::from_mode(0o600)).unwrap(),
            2 => fs::write(stage.join("new"), "new").unwrap(),
            _ => {
                fs::remove_file(&asset).unwrap();
                symlink(host.path().join("asset"), &asset).unwrap();
            }
        }
        let expected = view
            .facts()
            .lookup("/external/asset", GuestNamespaceOperation::Lstat)
            .unwrap()
            .kind();
        assert!(matches!(expected, GuestNamespaceNodeKind::RegularFile(_)));
        let result = if kind == 2 {
            view.list_directory("/external").map(|_| ())
        } else {
            view.read_regular("/external/asset", &mut |_| Ok(()), &|| true)
        };
        assert!(result.is_err());
        assert!(!view.is_valid());
        assert!(matches!(
            view.lookup("/external/asset", GuestNamespaceOperation::Stat),
            Err(ViewError::Invalid)
        ));
        drop(view);
        assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
        f.restored();
    }
}

#[test]
fn guest_view_cancel_after_actual_chunk_is_provisional_and_releases_reader_credit() {
    let f = Fixture::new(&scratch_program(1));
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("asset"), vec![7; 128 * 1024]).unwrap();
    let p = host_policy(&f, host.path());
    let view = prepare(&f, &p, guest(8));
    let baseline = view.budget().used_bytes();
    let active = Cell::new(true);
    let chunks = Cell::new(0usize);
    let result = view.read_regular(
        "/external/asset",
        &mut |chunk| {
            assert_eq!(chunk.len(), 64 * 1024);
            chunks.set(chunks.get() + 1);
            active.set(false);
            Ok(())
        },
        &|| active.get(),
    );
    assert!(matches!(result, Err(ViewError::Cancelled)));
    assert_eq!(chunks.get(), 1);
    assert!(!view.is_valid());
    assert_eq!(view.budget().used_bytes(), baseline);
    f.restored();
}

#[test]
fn guest_view_source_mutation_refuses_export_and_recovery() {
    let f = Fixture::new(&scratch_program(1));
    let host = tempfile::tempdir().unwrap();
    let asset = host.path().join("asset");
    fs::write(&asset, "host").unwrap();
    let p = host_policy(&f, host.path());
    let changed = Cell::new(false);
    let result = f.runtime.prepare_selected_guest_namespace_view_inner(
        key(&f),
        Some(p.dupe()),
        Some(guest(8)),
        Some(&|phase| {
            if matches!(phase, selected_tool_source::ToolPhase::BeforeCapture)
                && !changed.replace(true)
            {
                fs::write(&asset, "changed").unwrap();
            }
            Ok(())
        }),
    );
    assert!(result.is_err());
    assert!(changed.get());
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    f.restored();
    fs::write(&asset, "host").unwrap();
    let view = prepare(&f, &p, guest(8));
    assert_eq!(read(&view, "/external/asset"), b"host");
    assert!(view.is_valid());
    f.restored();
}

#[test]
fn guest_view_raw_some_restore_policy_a_b_a_none_pressure_and_repair() {
    let f = Fixture::new(&scratch_program(1));
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("asset"), "host").unwrap();
    let p = host_policy(&f, host.path());
    let budget = f.runtime.image_resources.descriptor_budget();
    let held = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    assert!(
        f.runtime
            .prepare_selected_guest_namespace_view(key(&f), Some(p.dupe()), None)
            .unwrap_err()
            .to_string()
            .contains("missing guest policy")
    );
    let mut guard = NativeDemandAbortGuard::new(
        f.runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    guard.begin_attempt().unwrap();
    f.runtime.runtime.block_on(async {
        let data = guard.attempt_user_computation_data().unwrap();
        assert!(data.data.get::<GuestNamespaceTransaction>().is_err());
        let mut updater = f.runtime.dice.updater_with_data(data);
        guard.inject_attempt(&mut updater).unwrap();
        drop(f.runtime.request_revision.commit(updater).await);
    });
    f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
    guard.seal_retry().unwrap();
    guard.discard().unwrap();
    drop(held);
    let a = prepare(&f, &p, guest(8));
    let b = prepare(&f, &p, guest(9));
    let again = prepare(&f, &p, guest(8));
    assert_ne!(a.facts().policy(), b.facts().policy());
    assert_eq!(a.facts().policy(), again.facts().policy());
    assert_eq!(read(&again, "/external/asset"), b"host");
    drop(a);
    drop(b);
    drop(again);
    f.restored();
    let runtime_key = key(&f);
    let (mut seed, _, reachable) =
        runtime_sources::tests::current_plan(&f, &p, runtime_key.profile_key());
    let mut inputs = seed.command().inputs.clone();
    inputs.request.guest_namespace = Some(guest(8));
    seed.command.as_mut().unwrap().inputs = inputs;
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
    let b = prepare(&f, &p, guest(9));
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
    f.runtime.runtime.block_on(async {
        let updater = f
            .runtime
            .dice
            .updater_with_data(f.runtime.user_computation_data(None).unwrap());
        let mut tx = updater.existing_state().await;
        f.runtime.request_revision.bind_cycle_detector(&tx);
        let raw =
            RawGuestNamespacePolicyKey::new(f.runtime.repository_materializer.workspace().dupe());
        assert_eq!(tx.compute(&raw).await.unwrap(), Some(guest(8)));
        drop(tx);
    });
    f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
    let held = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    assert_eq!(
        b.facts()
            .lookup("/external/asset", GuestNamespaceOperation::Unsupported)
            .unwrap_err(),
        GuestLookupError::Unsupported
    );
    assert_eq!(
        b.facts()
            .lookup("/external/asset", GuestNamespaceOperation::Lstat)
            .unwrap_err(),
        GuestLookupError::ObservationRejected
    );
    assert!(b.is_valid());
    let chunks = Cell::new(0usize);
    assert!(matches!(
        b.read_regular(
            "/external/asset",
            &mut |_| {
                chunks.set(chunks.get() + 1);
                Ok(())
            },
            &|| true
        ),
        Err(ViewError::Lookup(GuestLookupError::ObservationRejected))
    ));
    assert_eq!(chunks.get(), 0);
    assert!(!b.is_valid());
    drop(held);
    drop(b);
    let repaired = prepare(&f, &p, guest(8));
    assert_eq!(read(&repaired, "/external/asset"), b"host");
    assert!(repaired.is_valid());
}

#[test]
fn guest_view_denied_final_root_and_nondirectory_tamper_are_sticky_stale() {
    let f = Fixture::new(&scratch_program(1));
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("asset"), "host").unwrap();
    let p = host_policy(&f, host.path());
    let other = GuestNamespacePolicy::new(
        1001,
        1002,
        1003,
        1004,
        9,
        7,
        8,
        0o755,
        GuestNamespaceLimits::default(),
    )
    .unwrap();
    // The runtime root's parent is a policy scaffold. There is no physical
    // parent Membership check which could detect these changes first.
    for (mode, operation, errno, changed_mode) in [
        (
            0o700,
            GuestNamespaceOperation::ListDirectory,
            GuestLookupError::PermissionDenied,
            0o755,
        ),
        (
            0o755,
            GuestNamespaceOperation::ReadRegular,
            GuestLookupError::IsDirectory,
            0o700,
        ),
        (
            0o755,
            GuestNamespaceOperation::ReadLink,
            GuestLookupError::NotSymlink,
            0o700,
        ),
        (
            0o777,
            GuestNamespaceOperation::Access(2),
            GuestLookupError::ReadOnly,
            0o755,
        ),
    ] {
        fs::set_permissions(host.path(), fs::Permissions::from_mode(mode)).unwrap();
        let view = prepare(&f, &p, other);
        known(&view, "/external", operation, errno);
        let clone = view.dupe();
        fs::set_permissions(
            view.0.get().sources.test_image_path(0),
            fs::Permissions::from_mode(changed_mode),
        )
        .unwrap();
        assert!(matches!(
            view.lookup("/external", operation),
            Err(ViewError::Stale)
        ));
        assert!(!clone.is_valid());
        assert!(matches!(
            clone.lookup("/external", GuestNamespaceOperation::Lstat),
            Err(ViewError::Invalid)
        ));
        drop(clone);
        drop(view);
        assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
        f.restored();
    }
    fs::set_permissions(host.path(), fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(host.path().join("asset"), fs::Permissions::from_mode(0o600)).unwrap();
    for (operation, path, errno, mutation) in [
        (
            GuestNamespaceOperation::Access(4),
            "/external/asset",
            GuestLookupError::PermissionDenied,
            0,
        ),
        (
            GuestNamespaceOperation::ReadLink,
            "/external/asset",
            GuestLookupError::NotSymlink,
            1,
        ),
        (
            GuestNamespaceOperation::Stat,
            "/external/asset/..",
            GuestLookupError::NotDirectory,
            2,
        ),
    ] {
        let view = prepare(&f, &p, other);
        known(&view, path, operation, errno);
        let asset = view.0.get().sources.test_image_path(0).join("asset");
        match mutation {
            0 => fs::set_permissions(&asset, fs::Permissions::from_mode(0o644)).unwrap(),
            1 => fs::write(&asset, "changed-length").unwrap(),
            _ => {
                fs::remove_file(&asset).unwrap();
                fs::create_dir(&asset).unwrap();
            }
        }
        assert!(matches!(
            view.lookup(path, operation),
            Err(ViewError::Stale)
        ));
        assert!(!view.is_valid());
        drop(view);
        f.restored();
    }
}

#[test]
fn guest_view_normal_alias_deny_is_typed_and_repairs() {
    let f = Fixture::new(&scratch_program(1));
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("asset"), "host").unwrap();
    symlink("asset", host.path().join("alias")).unwrap();
    let p = host_policy(&f, host.path());
    let error = f
        .runtime
        .prepare_selected_guest_namespace_view(key(&f), Some(p.dupe()), Some(guest(8)))
        .unwrap_err();
    let NativeDemandSessionError::Computation(error) = error else {
        panic!("normal alias error must remain typed")
    };
    assert!(matches!(
        error.downcast_ref::<slug_loading_v2::GuestNamespaceFactsError>(),
        Some(slug_loading_v2::GuestNamespaceFactsError::Runtime(
            slug_loading_v2::RuntimeSourcesError::Image(
                slug_workspace_v2::InputImageError::InvalidAlias
            )
        ))
    ));
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    f.restored();
    fs::remove_file(host.path().join("alias")).unwrap();
    let repaired = prepare(&f, &p, guest(8));
    assert_eq!(read(&repaired, "/external/asset"), b"host");
    assert!(repaired.is_valid());
    f.restored();
}

#[test]
fn guest_view_genuine_current_label_tree_runtime_overlap_refuses_composition() {
    let (f, _download, _host, p, k, _) = composite_fixture();
    // Precreate storage before normal Label ancestor facts, just like the
    // production driver. No capture or completed profile is injected here.
    let storage = runtime_sources::prepare::Storage::new(&f.runtime).unwrap();
    let (mut guard, initial, _) = runtime_sources::tests::current_plan(&f, &p, k.profile_key());
    let original_root = initial
        .tool_requests()
        .requests()
        .nth(1)
        .unwrap()
        .label_selection()
        .unwrap()
        .image_key()
        .source_root()
        .as_path();
    let collision = original_root.join("lib");
    let workspace = ImageChargedPath::pin_existing(
        f.runtime.repository_materializer.workspace().dupe(),
        f.runtime.image_resources.descriptor_budget(),
    )
    .unwrap();
    let overlap = RuntimeSourceDeclarationPolicy::new(
        &workspace,
        &[RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::SelectedToolOriginLabelParent {
                role: SelectedToolRole::Cargo,
                children: "lib",
            },
            guest_prefix: collision.to_str().unwrap(),
        }],
        RuntimeSourceLimits::default(),
        f.runtime.image_resources.descriptor_budget(),
    )
    .unwrap();
    let mut inputs = guard.command().inputs.clone();
    inputs.request.runtime_sources = Some(overlap.dupe());
    inputs.request.guest_namespace = Some(guest(8));
    guard.command.as_mut().unwrap().inputs = inputs;
    let deadline = Instant::now() + Duration::from_secs(10);
    let (plan, reachable) = (0..128)
        .find_map(|_| {
            guard.begin_attempt().unwrap();
            let attempt = f
                .runtime
                .runtime
                .block_on(runtime_sources::attempt::plan(
                    &f.runtime,
                    &mut guard,
                    k.profile_key(),
                    deadline,
                ))
                .unwrap();
            f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
            match attempt {
                selected_tool_source::attempt::Attempt::Need(need) => {
                    selected_tool_source::progress_baseline(&mut guard, &need, deadline).unwrap();
                    None
                }
                selected_tool_source::attempt::Attempt::Complete(plan, selected) => {
                    let (_, reachable, terminal) = selected.sidecars.into_parts();
                    terminal.reset_to_idle().unwrap();
                    Some((plan, reachable))
                }
            }
        })
        .expect("same-command normal runtime plan must finish");
    assert_eq!(
        plan.requests().next().unwrap().guest_prefix().as_path(),
        collision
    );
    assert_eq!(
        plan.tool_requests()
            .requests()
            .nth(1)
            .unwrap()
            .label_selection()
            .unwrap()
            .image_key()
            .source_root()
            .as_path(),
        original_root
    );
    guard.command.as_mut().unwrap().scratch_metadata = Some(
        selected_tool_source::ScratchMetadataState::new(
            plan.tool_requests(),
            &f.runtime.scratch_resources,
        )
        .unwrap(),
    );
    let mut tools = selected_tool_source::declarations::Catalog::new(
        &f.runtime,
        plan.tool_requests(),
        deadline,
    )
    .unwrap();
    let runtime = runtime_sources::catalogue::Catalogue::new(
        &f.runtime,
        k.profile_key(),
        &plan,
        &reachable,
        deadline,
    )
    .unwrap();
    let error = (0..128)
        .find_map(|_| {
            guard.begin_attempt().unwrap();
            let attempt = f.runtime.runtime.block_on(attempt::sources(
                &f.runtime, &mut guard, &k, &plan, &mut tools, deadline,
            ));
            f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
            match attempt {
                Err(error) => Some(error),
                Ok(selected_tool_source::attempt::Attempt::Need(need)) => {
                    runtime_sources::prepare::progress_sources(
                        &mut guard,
                        &need,
                        k.profile_key(),
                        &plan,
                        &reachable,
                        &tools,
                        &runtime,
                        deadline,
                    )
                    .unwrap();
                    None
                }
                Ok(selected_tool_source::attempt::Attempt::Complete(_, _)) => {
                    panic!("overlapping genuine source trees must not export facts")
                }
            }
        })
        .expect("normal namespace must finish with its typed collision");
    let NativeDemandSessionError::Computation(error) = error else {
        panic!("composition failure must remain typed")
    };
    assert!(matches!(
        error.downcast_ref::<slug_loading_v2::GuestNamespaceFactsError>(),
        Some(slug_loading_v2::GuestNamespaceFactsError::Collision)
    ));
    guard.discard().unwrap();
    drop(storage);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    f.restored();
}
