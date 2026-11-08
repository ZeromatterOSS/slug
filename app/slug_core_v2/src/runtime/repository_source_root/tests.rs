use std::io::Read;
use std::io::Write;
use std::time::Duration;
use std::time::Instant;

use sha2::Digest;
use sha2::Sha256;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::repository_io::RepositoryMaterializer;

fn bank() -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(512 * 1024 * 1024, 512, 8 * 1024 * 1024 * 1024).unwrap(),
    )
    .unwrap()
}
fn scope(
    workspace: &slug_workspace_v2::NormalizedAbsolutePath,
    resources: &Arc<RepositoryRootResources>,
) -> Arc<SourceServiceScope> {
    let work = Instant::now() + Duration::from_secs(26);
    SourceServiceScope::new_command(
        workspace.clone(),
        work,
        work + Duration::from_secs(1),
        SourceServiceLimits::default(),
        resources.clone(),
    )
    .unwrap()
}
fn request(path: &Path, lease: u64) -> Arc<Request> {
    crate::runtime::dice::workspace_source_request_for_test(path, lease)
}
fn reservation(path: &Path) -> Arc<Reservation> {
    Arc::new(
        Reservation::new(
            request(path, 1),
            slug_workspace_v2::NormalizedAbsolutePath::new("/__slug_repositories__/s1/kernel")
                .unwrap(),
            slug_workspace_v2::PathObservationInstanceId::new(1),
        )
        .unwrap(),
    )
}

#[test]
fn unpolled_source_initializer_arms_input_retirement_before_returning_future() {
    let workspace = tempfile::tempdir().unwrap();
    let normalized = slug_workspace_v2::NormalizedAbsolutePath::new(workspace.path()).unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope(&normalized, &resources);
    let actor = RepositoryMaterializer::new(normalized.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let token = actor.begin_with_command_scope(Some(scope.clone())).unwrap();
    actor.preflight_native(token, []).unwrap();
    let bounds = slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits::new(
        128,
        16 * 1024 * 1024,
        1024 * 1024 * 1024,
    )
    .unwrap();
    let lease = actor
        .begin_general_workspace(token, resources.clone(), bounds, &scope)
        .unwrap();
    let request = request(workspace.path(), lease);
    let ticket = actor
        .reserve_source_snapshot(token, request.clone(), &scope)
        .unwrap();
    let command = RepositoryWorkspaceCommand::new(
        normalized,
        lease,
        bounds,
        resources.clone(),
        scope.clone(),
        &|| true,
    )
    .unwrap();
    let observer = command.retirement_observer();
    assert_eq!(observer.owner_count_for_test(), 1);
    let input = SourceRootInput::capture(
        request,
        &resources,
        &runtime,
        &crate::runtime::ArchiveDistdirPolicy::default(),
        &scope,
        &|| true,
    )
    .unwrap();
    assert!(resources.snapshot().native_fds > 0);
    let active = || true;
    let future = command.initialize_source_snapshot(
        ticket.reservation().clone(),
        ticket.claim().clone(),
        input,
        &scope,
        &active,
    );
    assert_eq!(
        observer.owner_count_for_test(),
        2,
        "input must be armed before the future is polled"
    );
    drop(future);
    assert!(observer.wait_until(Instant::now()));
    assert_eq!(resources.snapshot().native_fds, 0);
    assert_eq!(
        resources.snapshot().peak_backing_bytes,
        0,
        "unpolled initialization must not launch setup"
    );
    drop(observer);
    drop(ticket);
    actor.discard(token).unwrap();
    drop(actor);
    drop(scope);
    assert_eq!(resources.snapshot().retained_bytes, baseline);
}

#[test]
fn source_backing_refuses_general_binding_and_claim_without_refunding_allowance() {
    let workspace = tempfile::tempdir().unwrap();
    let normalized = slug_workspace_v2::NormalizedAbsolutePath::new(workspace.path()).unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope(&normalized, &resources);
    let actor = RepositoryMaterializer::new(normalized.clone());
    let token = actor.begin_with_command_scope(Some(scope.clone())).unwrap();
    actor.preflight_native(token, []).unwrap();
    let bounds = slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits::new(
        128,
        16 * 1024 * 1024,
        1024 * 1024 * 1024,
    )
    .unwrap();
    let lease = actor
        .begin_general_workspace(token, resources.clone(), bounds, &scope)
        .unwrap();
    let source = actor
        .reserve_source_snapshot(token, request(workspace.path(), lease), &scope)
        .unwrap();
    let owner = crate::runtime::dice::physical_root_owner_for_test();
    let repository = slug_identity_v2::CanonicalRepoName::new(format!(
        "{}+backing_guard",
        owner.unique_name().as_str()
    ))
    .unwrap();
    let key = Arc::new(
        slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey::new(
            normalized, owner, repository, 0,
        )
        .unwrap(),
    );
    let spec = Arc::new(slug_bzlmod_v2::RepoSpec {
        rule_id: slug_bzlmod_v2::RepoRuleId {
            bzl_file: slug_identity_v2::CanonicalLabel::parse("@@//:repo.bzl").unwrap(),
            rule_name: "backing_guard".into(),
        },
        attributes: Arc::new(starlark_map::small_map::SmallMap::new()),
    });
    let general = actor.reserve_workspace(token, key, spec, &scope).unwrap();
    let identity = RootIdentity {
        device: 1,
        inode: 1,
        mount: 1,
    };
    for bound_identity in [false, true] {
        let backing =
            RepositoryRootBacking::new(&resources, RepositoryWorkspaceMode::Archive).unwrap();
        if bound_identity {
            backing.bind_source(identity, source.reservation()).unwrap();
        } else {
            backing
                .bind_source_claim(source.reservation(), source.claim().clone())
                .unwrap();
        }
        let before = resources.snapshot();
        assert!(matches!(
            backing.bind(identity, general.reservation()),
            Err(PhysicalRootError::Reservation)
        ));
        assert!(matches!(
            backing.bind_claim(general.reservation(), general.claim().clone()),
            Err(PhysicalRootError::Reservation)
        ));
        assert_eq!(resources.snapshot().retained_bytes, before.retained_bytes);
        assert_eq!(resources.snapshot().backing_bytes, before.backing_bytes);
    }
    drop(general);
    drop(source);
    actor.discard(token).unwrap();
    drop(actor);
    drop(scope);
    assert_eq!(resources.snapshot().retained_bytes, baseline);
    assert_eq!(resources.snapshot().backing_bytes, 0);
    assert_eq!(resources.snapshot().native_fds, 0);
}

#[test]
fn digest_tree_copy_preserves_modes_links_and_rejects_same_bytes_inode_replacement() {
    use std::os::unix::fs::PermissionsExt;
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let reservation = reservation(source.path());
    std::fs::create_dir(source.path().join("dir")).unwrap();
    std::fs::write(source.path().join("dir/body"), b"source body").unwrap();
    std::fs::set_permissions(
        source.path().join("dir"),
        std::fs::Permissions::from_mode(0o555),
    )
    .unwrap();
    std::os::unix::fs::symlink("dir/body", source.path().join("link")).unwrap();
    let resources = bank();
    let scope = scope(reservation.workspace(), &resources);
    let source_fd = File::open(source.path()).unwrap();
    let target_fd = File::open(target.path()).unwrap();
    let source_io = RootIo::new(&source_fd, limits(&reservation).unwrap()).unwrap();
    let target_io = RootIo::new(&target_fd, limits(&reservation).unwrap()).unwrap();
    let first = tree::capture(
        &source_io,
        &reservation,
        Some(&target_io),
        &resources,
        &scope,
        &|| true,
    )
    .unwrap();
    let second =
        tree::capture(&source_io, &reservation, None, &resources, &scope, &|| true).unwrap();
    let output =
        tree::capture(&target_io, &reservation, None, &resources, &scope, &|| true).unwrap();
    assert!(first.same_host_capture(&second));
    assert_eq!(first.entries(), output.entries());
    assert_eq!(
        std::fs::read_link(target.path().join("link")).unwrap(),
        Path::new("dir/body")
    );
    assert!(matches!(
        output
            .entries()
            .iter()
            .find(|e| e.path() == "dir")
            .unwrap()
            .node(),
        Node::Directory
    ));
    std::fs::set_permissions(
        source.path().join("dir"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    std::fs::write(source.path().join("dir/next"), b"source body").unwrap();
    std::fs::rename(
        source.path().join("dir/next"),
        source.path().join("dir/body"),
    )
    .unwrap();
    std::fs::set_permissions(
        source.path().join("dir"),
        std::fs::Permissions::from_mode(0o555),
    )
    .unwrap();
    let replaced =
        tree::capture(&source_io, &reservation, None, &resources, &scope, &|| true).unwrap();
    assert_eq!(first.entries(), replaced.entries());
    assert!(!first.same_host_capture(&replaced));
}

#[test]
fn digest_tree_rewalk_detects_restored_directory_membership_and_refuses_unsafe_sources() {
    let source = tempfile::tempdir().unwrap();
    let reservation = reservation(source.path());
    let resources = bank();
    let scope = scope(reservation.workspace(), &resources);
    std::fs::write(source.path().join("body"), b"bytes").unwrap();
    let fd = File::open(source.path()).unwrap();
    let io = RootIo::new(&fd, limits(&reservation).unwrap()).unwrap();
    let before = tree::capture(&io, &reservation, None, &resources, &scope, &|| true).unwrap();
    std::fs::write(source.path().join("temporary"), b"change").unwrap();
    std::fs::remove_file(source.path().join("temporary")).unwrap();
    fd.set_times(
        std::fs::FileTimes::new()
            .set_modified(std::time::SystemTime::now() + Duration::from_secs(1)),
    )
    .unwrap();
    let after = tree::capture(&io, &reservation, None, &resources, &scope, &|| true).unwrap();
    assert_eq!(before.entries(), after.entries());
    assert!(!before.same_host_capture(&after));
    for target in ["../outside", "/absolute"] {
        std::os::unix::fs::symlink(target, source.path().join("escape")).unwrap();
        assert!(tree::capture(&io, &reservation, None, &resources, &scope, &|| true).is_err());
        std::fs::remove_file(source.path().join("escape")).unwrap();
    }
    std::fs::hard_link(source.path().join("body"), source.path().join("hardlink")).unwrap();
    assert!(tree::capture(&io, &reservation, None, &resources, &scope, &|| true).is_err());
    std::fs::remove_file(source.path().join("hardlink")).unwrap();
    let large = File::create(source.path().join("large")).unwrap();
    large.set_len(64 * 1024 * 1024 + 1).unwrap();
    assert!(tree::capture(&io, &reservation, None, &resources, &scope, &|| true).is_err());
}

fn write_large(path: &Path, byte: u8) {
    let mut file = File::create(path).unwrap();
    let buffer = [byte; 64 * 1024];
    for _ in 0..528 {
        file.write_all(&buffer).unwrap();
    }
}
fn stream_digest(
    root: &Arc<RepositorySourceRoot>,
    scope: &Arc<SourceServiceScope>,
    name: &str,
) -> FileContentDigest {
    let mut input = root.open_member(name, scope, &|| true).unwrap();
    let mut hash = Sha256::new();
    let mut total = 0;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = input.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        total += count as u64;
    }
    FileContentDigest::new(hash.finalize().into(), total).unwrap()
}
fn initialize(
    actor: &RepositoryMaterializer,
    path: &Path,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    runtime: &tokio::runtime::Runtime,
    mutate: bool,
) -> Result<
    (
        crate::runtime::repository_io::RepositorySessionToken,
        RepositoryWorkspaceCommand,
        Arc<RepositorySourceRoot>,
    ),
    PhysicalRootError,
> {
    let token = actor.begin_with_command_scope(Some(scope.clone())).unwrap();
    actor.preflight_native(token, []).unwrap();
    let bounds = slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits::new(
        128,
        16 * 1024 * 1024,
        1024 * 1024 * 1024,
    )
    .unwrap();
    let lease = actor
        .begin_general_workspace(token, resources.clone(), bounds, scope)
        .unwrap();
    let request = request(path, lease);
    let ticket = actor
        .reserve_source_snapshot(token, request.clone(), scope)
        .unwrap();
    let input = SourceRootInput::capture(
        request,
        resources,
        runtime,
        &crate::runtime::ArchiveDistdirPolicy::default(),
        scope,
        &|| true,
    )?;
    let input = if mutate {
        let body = path.join("same");
        input.after_local_capture_for_test(move || {
            std::fs::write(body.with_extension("new"), b"identical").unwrap();
            std::fs::rename(body.with_extension("new"), body).unwrap();
        })
    } else {
        input
    };
    let command = RepositoryWorkspaceCommand::new(
        actor.workspace().clone(),
        lease,
        bounds,
        resources.clone(),
        scope.clone(),
        &|| true,
    )?;
    let result = runtime.block_on(command.initialize_source_snapshot(
        ticket.reservation().clone(),
        ticket.claim().clone(),
        input,
        scope,
        &|| true,
    ));
    if result.is_err() {
        actor.discard(token).unwrap();
    }
    result.map(|(command, receipt)| {
        let root = receipt.root().clone();
        let state = actor
            .adopt_source_snapshot(token, &ticket, receipt, scope)
            .unwrap();
        assert!(Arc::ptr_eq(&state, root.state()));
        (token, command, root)
    })
}

#[test]
#[ignore = "actual compiled CLI digest-only SourceRoot lifetime and >64MiB capability; finite native supervisor required"]
fn native_source_roots_keep_large_digest_only_data_and_original_scope_after_controller_retirement()
{
    let workspace = tempfile::tempdir().unwrap();
    write_large(&workspace.path().join("a"), 0x31);
    write_large(&workspace.path().join("b"), 0xa7);
    let normalized = slug_workspace_v2::NormalizedAbsolutePath::new(workspace.path()).unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let actor = RepositoryMaterializer::new(normalized.clone());
    let a_scope = scope(&normalized, &resources);
    let (a_token, a_command, a) = initialize(
        &actor,
        workspace.path(),
        &resources,
        &a_scope,
        &runtime,
        false,
    )
    .unwrap();
    assert!(a.state().file_bytes() > 64 * 1024 * 1024);
    assert!(a.state().tree_storage_bytes().unwrap() < 1024 * 1024);
    assert!(
        resources.snapshot().peak_retained_bytes - baseline < 64 * 1024 * 1024,
        "source bodies must not be copied into retained-byte storage"
    );
    let a_root = a.clone();
    let a_display = a.reservation().display_root().clone();
    let before = a_scope.snapshot();
    let native_before = resources.snapshot();
    assert!(
        actor
            .observe_native(
                a_token,
                [
                    PathObservationDemand::new(
                        a.anchor().namespace(),
                        slug_workspace_v2::NormalizedAbsolutePath::new(
                            a_display.as_path().join("MODULE.bazel")
                        )
                        .unwrap(),
                        slug_workspace_v2::PathObservationOperation::FileBytes
                    ),
                    PathObservationDemand::new(
                        PathObservationNamespace::Materialization(
                            slug_workspace_v2::PathObservationInstanceId::new(u64::MAX)
                        ),
                        slug_workspace_v2::NormalizedAbsolutePath::new("/unselected").unwrap(),
                        slug_workspace_v2::PathObservationOperation::Lstat
                    ),
                ]
            )
            .is_err()
    );
    assert_eq!(a_scope.snapshot().source_attempts, before.source_attempts);
    assert_eq!(
        resources.snapshot().peak_native_fds,
        native_before.peak_native_fds
    );
    let observer = a_command.retirement_observer();
    let mut reader = a_root.open_member("a", &a_scope, &|| true).unwrap();
    drop(a_command);
    assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
    drop(observer);
    actor.accept(a_token, &[], vec![]).unwrap();
    drop(a);
    let mut small = [0u8; 5];
    reader.read_exact(&mut small).unwrap();
    assert_eq!(small, [0x31; 5]);
    let b_scope = scope(&normalized, &resources);
    let (b_token, mut b_command, b) = initialize(
        &actor,
        workspace.path(),
        &resources,
        &b_scope,
        &runtime,
        false,
    )
    .unwrap();
    assert_eq!(a_display, *b.reservation().display_root());
    assert_ne!(a_root.reservation().instance(), b.reservation().instance());
    assert!(a_root.open_member("a", &b_scope, &|| true).is_err());
    a_scope.cancel();
    assert!(reader.read(&mut small).is_err());
    assert!(actor.session_is_active(b_token));
    b_scope.check().unwrap();
    let expected = match b.state().entry("b").unwrap().node() {
        Node::File(d) => *d,
        _ => panic!("file"),
    };
    assert_eq!(stream_digest(&b, &b_scope, "b"), expected);
    // The >64MiB source is attached readonly beside a genuinely quota-shrunk
    // writable root. The normal owning actor produces the actual Execute record.
    let owner = crate::runtime::dice::physical_root_owner_for_test();
    let key = Arc::new(
        slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey::new(
            normalized.clone(),
            owner,
            slug_identity_v2::CanonicalRepoName::new("source_consumer").unwrap(),
            0,
        )
        .unwrap(),
    );
    let spec = Arc::new(slug_bzlmod_v2::RepoSpec {
        rule_id: slug_bzlmod_v2::RepoRuleId {
            bzl_file: slug_identity_v2::CanonicalLabel::parse("@@//:consumer.bzl").unwrap(),
            rule_name: "declared_consumer".into(),
        },
        attributes: Arc::new(starlark_map::small_map::SmallMap::new()),
    });
    let executor = actor
        .reserve_workspace(b_token, key, spec, &b_scope)
        .unwrap();
    b_command = runtime
        .block_on(b_command.initialize_claimed_root(
            executor.reservation().clone(),
            executor.claim().clone(),
            resources.clone(),
            b_scope.clone(),
            &|| true,
        ))
        .unwrap();
    let views = slug_bzlmod_v2::RepositoryWorkspaceReadViewManifest::new(
        executor.reservation(),
        vec![slug_bzlmod_v2::RepositoryWorkspaceReadView::from_source(b.state().clone()).unwrap()],
    )
    .unwrap();
    let code = r#"import errno,os,pathlib,sys
p=pathlib.Path(sys.argv[1])
assert os.statvfs(p).f_blocks*os.statvfs(p).f_frsize==2147483648
assert os.statvfs('.').f_blocks*os.statvfs('.').f_frsize==67108864
assert (p/'a').stat().st_size+(p/'b').stat().st_size>67108864
for name in ['a','b']:
 with open(p/name,'rb') as f:
  total=0
  while True:
   chunk=f.read(65536)
   if not chunk: break
   total+=len(chunk)
  assert total==34603008
try: os.open(p/'a',os.O_WRONLY|os.O_APPEND)
except OSError as e: assert e.errno==errno.EROFS
else: raise AssertionError('source became writable')
pathlib.Path('owned').write_text('normal process')
print('source-ok')
"#;
    let operation = slug_bzlmod_v2::RepositoryWorkspaceOperation::Execute(
        slug_bzlmod_v2::RepositoryWorkspaceExecute::new(
            [
                "/usr/bin/python3",
                "-c",
                code,
                b.reservation().display_root().as_path().to_str().unwrap(),
            ]
            .iter()
            .map(|v| slug_bzlmod_v2::RepositoryWorkspaceArgument::String((*v).into()))
            .collect::<Vec<_>>()
            .into(),
            slug_bzlmod_v2::RepositoryEnvironmentSnapshot::empty(),
            starlark_map::small_map::SmallMap::new(),
            600,
            true,
            slug_bzlmod_v2::HostRepositoryLabelPathValue::new(
                executor.reservation().display_root().clone(),
                PathObservationNamespace::Materialization(executor.reservation().instance()),
            )
            .unwrap(),
            views,
        )
        .unwrap(),
    );
    let request = b_command
        .request(executor.reservation(), operation, &b_scope, &|| true)
        .unwrap();
    let results = actor.selected_epoch(b_token, &[]).unwrap();
    b_command = runtime
        .block_on(actor.execute_workspace_with_sources(
            b_token,
            b_command,
            request,
            &[],
            &results,
            std::slice::from_ref(b.state()),
            &b_scope,
            &|| true,
        ))
        .unwrap();
    let record = &b_command.epoch().transcripts().unwrap()[0].records()[0];
    assert!(
        matches!(record.result(),slug_bzlmod_v2::RepositoryWorkspaceResult::Execute(result) if result.return_code()==0&&result.stdout()==b"source-ok\n")
    );
    assert_eq!(record.next().mode(), RepositoryWorkspaceMode::Process);
    drop(executor);
    let b_root = b.clone();
    drop(b_command);
    actor.accept(b_token, &[], vec![]).unwrap();
    drop(b);
    drop(actor);
    assert!(!a_display.as_path().exists());
    assert_eq!(resources.snapshot().backing_bytes, 2 * ARCHIVE_BYTES);
    drop(reader);
    drop(a_root);
    assert_eq!(resources.snapshot().backing_bytes, ARCHIVE_BYTES);
    drop(b_root);
    drop(a_scope);
    drop(b_scope);
    assert_eq!(
        (
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes,
            resources.snapshot().retained_bytes
        ),
        (0, 0, baseline)
    );
}

#[test]
#[ignore = "actual compiled CLI Local framed source rewalk rejection; finite native supervisor required"]
fn native_local_snapshot_refuses_identical_byte_replacement_before_source_seal() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("same"), b"identical").unwrap();
    let normalized = slug_workspace_v2::NormalizedAbsolutePath::new(workspace.path()).unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope(&normalized, &resources);
    let actor = RepositoryMaterializer::new(normalized);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert!(matches!(
        initialize(&actor, workspace.path(), &resources, &scope, &runtime, true),
        Err(PhysicalRootError::Snapshot)
    ));
    drop(actor);
    drop(scope);
    assert_eq!(
        (
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes,
            resources.snapshot().retained_bytes
        ),
        (0, 0, baseline)
    );
}
