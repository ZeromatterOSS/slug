//! Native component proof reuses the real Source producer/bootstrap fixture.
//! Original Loading, actor current-prefix admission and CAS are separate gates.
use slug_bzlmod_v2::RepositoryWorkspaceOperation as Op;
use slug_bzlmod_v2::RepositoryWorkspaceReadView as View;
use slug_bzlmod_v2::RepositoryWorkspaceResult as ResultValue;
use slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::runtime::repository_physical_root::SourceLinkSet;
use crate::runtime::repository_physical_root::routed;

pub(super) fn general(
    actor: &RepositoryMaterializer,
    token: crate::runtime::repository_io::RepositorySessionToken,
    command: RepositoryWorkspaceCommand,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    runtime: &tokio::runtime::Runtime,
) -> (
    RepositoryWorkspaceCommand,
    Arc<RepositoryWorkspaceReservation>,
) {
    general_named(
        actor,
        token,
        command,
        resources,
        scope,
        runtime,
        "source_link_consumer",
    )
}
fn general_named(
    actor: &RepositoryMaterializer,
    token: crate::runtime::repository_io::RepositorySessionToken,
    command: RepositoryWorkspaceCommand,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    runtime: &tokio::runtime::Runtime,
    name: &str,
) -> (
    RepositoryWorkspaceCommand,
    Arc<RepositoryWorkspaceReservation>,
) {
    let (command, ticket) = general_ticket(actor, token, command, resources, scope, runtime, name);
    (command, ticket.reservation().clone())
}
pub(super) fn general_ticket(
    actor: &RepositoryMaterializer,
    token: crate::runtime::repository_io::RepositorySessionToken,
    command: RepositoryWorkspaceCommand,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    runtime: &tokio::runtime::Runtime,
    name: &str,
) -> (
    RepositoryWorkspaceCommand,
    crate::runtime::repository_io::reservations::ReservedRepositoryWorkspace,
) {
    let owner = crate::runtime::dice::physical_root_owner_for_test();
    let key = Arc::new(
        RepositoryWorkspaceStableKey::new(
            actor.workspace().clone(),
            owner,
            CanonicalRepoName::new(name).unwrap(),
            0,
        )
        .unwrap(),
    );
    let spec = Arc::new(slug_bzlmod_v2::RepoSpec {
        rule_id: slug_bzlmod_v2::RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@//:repo.bzl").unwrap(),
            rule_name: "generic".into(),
        },
        attributes: Arc::new(SmallMap::new()),
    });
    let ticket = actor.reserve_workspace(token, key, spec, scope).unwrap();
    let reservation = ticket.reservation().clone();
    let command = runtime
        .block_on(command.initialize_claimed_root(
            reservation.clone(),
            ticket.claim().clone(),
            resources.clone(),
            scope.clone(),
            &|| true,
        ))
        .unwrap();
    (command, ticket)
}
fn apply(
    command: RepositoryWorkspaceCommand,
    reservation: &Arc<RepositoryWorkspaceReservation>,
    sources: &Arc<SourceLinkSet>,
    op: Op,
    runtime: &tokio::runtime::Runtime,
    scope: &Arc<SourceServiceScope>,
) -> RepositoryWorkspaceCommand {
    let request = command.request(reservation, op, scope, &|| true).unwrap();
    command
        .apply_with_sources(
            request,
            sources.clone(),
            runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            scope,
            &|| true,
        )
        .unwrap()
}
fn link(path: &str, source: &Arc<RepositorySourceRoot>, relative: &str) -> Op {
    Op::Symlink {
        path: path.into(),
        target: NormalizedAbsolutePath::new(
            source.reservation().display_root().as_path().join(relative),
        )
        .unwrap(),
        source: Some(View::from_source(source.state().clone()).unwrap()),
    }
}

fn own_aliases(
    actor: &RepositoryMaterializer,
    token: crate::runtime::repository_io::RepositorySessionToken,
    command: RepositoryWorkspaceCommand,
    resources: &Arc<RepositoryRootResources>,
    sources: &Arc<SourceLinkSet>,
    scope: &Arc<SourceServiceScope>,
    runtime: &tokio::runtime::Runtime,
) -> RepositoryWorkspaceCommand {
    let (mut command, reservation) = general_named(
        actor,
        token,
        command,
        resources,
        scope,
        runtime,
        "own_alias_consumer",
    );
    for (path, body) in [
        ("body0", b"old".as_slice()),
        ("body1", b"new".as_slice()),
        ("dir0/child", b"old directory".as_slice()),
        ("dir1/child", b"new directory".as_slice()),
    ] {
        command = apply(
            command,
            &reservation,
            sources,
            Op::File {
                path: path.into(),
                body: slug_bzlmod_v2::RepositoryWorkspaceBody::from_arc(Arc::from(body)),
                mode: 0o644,
            },
            runtime,
            scope,
        );
    }
    for (path, target) in [("file_alias", "body0"), ("dir_alias", "dir0")] {
        command = apply(
            command,
            &reservation,
            sources,
            Op::Symlink {
                path: path.into(),
                target: NormalizedAbsolutePath::new(
                    reservation.display_root().as_path().join(target),
                )
                .unwrap(),
                source: None,
            },
            runtime,
            scope,
        );
    }
    let count = command
        .epoch()
        .transcripts()
        .unwrap()
        .iter()
        .find(|t| t.reservation() == &reservation)
        .unwrap()
        .records()
        .len();
    let (command, root) = command
        .seal_root(&reservation, count, scope, &|| true)
        .unwrap();
    assert!(root.state().source_bindings().is_empty());
    let before = scope.snapshot();
    let mut readers = Vec::new();
    for (path, expected) in [
        ("file_alias", b"old".as_slice()),
        ("dir_alias/child", b"old directory".as_slice()),
    ] {
        let resolved = root.state().resolve_path(path).unwrap();
        let digest =
            FileContentDigest::new(Sha256::digest(expected).into(), expected.len() as u64).unwrap();
        let mut reader = root
            .open_routed_member(
                path,
                resolved.content_origin(),
                resolved.real_path(),
                digest,
                scope,
            )
            .unwrap();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, expected);
        std::io::Seek::seek(&mut reader, std::io::SeekFrom::Start(0)).unwrap();
        readers.push(reader);
    }
    let io = RootIo::new(&root.file, io_limits(root.mode, &reservation)).unwrap();
    for (path, target) in [("file_alias", "body1"), ("dir_alias", "dir1")] {
        io.unlink(Path::new(path)).unwrap();
        let target =
            NormalizedAbsolutePath::new(reservation.display_root().as_path().join(target)).unwrap();
        let proof = sources
            .validate_absolute(&reservation, path, &target)
            .unwrap();
        io.symlink_validated_absolute(Path::new(path), &proof)
            .unwrap();
    }
    for mut reader in readers {
        assert!(std::io::Seek::seek(&mut reader, std::io::SeekFrom::Start(0)).is_err());
        assert!(reader.read(&mut [0]).is_err());
    }
    assert_eq!(scope.snapshot().source_attempts, before.source_attempts);
    assert_eq!(scope.snapshot().decoded_bytes, before.decoded_bytes);
    drop(io);
    drop(root);
    command
}

#[test]
fn validated_absolute_link_creation_keeps_relative_policy_and_never_follows_target() {
    // Descriptor kernel only: this Host fixture is not a physical-root factory.
    let directory = tempfile::tempdir().unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let normalized = NormalizedAbsolutePath::new(directory.path()).unwrap();
    let scope = scope(&normalized, &resources);
    let reservation = crate::runtime::repository_physical_root::native_tests::reservation(
        Path::new("/__slug_repositories__/v1/kernel-links"),
        3223,
    );
    let sources = SourceLinkSet::empty(&resources, &scope, &|| true).unwrap();
    let file_credit = resources.reserve(0, 1, 0).unwrap();
    let file = File::open(directory.path()).unwrap();
    let scratch = resources.reserve(IO_SCRATCH, 3, 0).unwrap();
    let io = RootIo::new(
        &file,
        io_limits(RepositoryWorkspaceMode::Archive, &reservation),
    )
    .unwrap();
    let target =
        NormalizedAbsolutePath::new(reservation.display_root().as_path().join("absent")).unwrap();
    assert!(
        io.symlink_relative(Path::new("legacy"), target.as_path())
            .is_err()
    );
    let proof = sources
        .validate_absolute(&reservation, "link", &target)
        .unwrap();
    io.symlink_validated_absolute(Path::new("link"), &proof)
        .unwrap();
    let mut literal = [0u8; 4097];
    assert_eq!(
        io.read_link(Path::new("link"), &mut literal).unwrap(),
        target.as_path().as_os_str().as_encoded_bytes()
    );
    assert!(io.open_read_regular(Path::new("link")).is_err());
    for target in [
        "/host/not-declared",
        "/__slug_repositories__/s1/not-declared/payload",
        "/__slug_repositories__/v1/kernel-links-sibling/body",
        "../../escape",
    ] {
        assert!(sources.classify(&reservation, "link", target).is_err());
    }
    drop(proof);
    drop(io);
    drop(scratch);
    drop(file);
    drop(file_credit);
    drop(sources);
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

#[test]
#[ignore = "actual CLI Source-backed symlinks, routed reads and last-owner data lifetime; finite native supervisor required"]
fn native_source_links_preserve_literal_targets_route_fd_reads_and_keep_only_data_after_retirement()
{
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir(workspace.path().join("dir")).unwrap();
    std::fs::write(workspace.path().join("dir/body"), b"selected source bytes").unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let normalized = NormalizedAbsolutePath::new(workspace.path()).unwrap();
    let scope = scope(&normalized, &resources);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let actor = RepositoryMaterializer::new(normalized);
    let (token, command, source) = initialize(
        &actor,
        workspace.path(),
        &resources,
        &scope,
        &runtime,
        false,
    )
    .unwrap();
    let views = [View::from_source(source.state().clone()).unwrap()];
    let sources = command.source_link_set_for_test(&views, &scope).unwrap();
    let display = source
        .reservation()
        .display_root()
        .as_path()
        .to_str()
        .unwrap();
    let classification_reservation =
        crate::runtime::repository_physical_root::native_tests::reservation(
            Path::new("/__slug_repositories__/v1/classifier"),
            3321,
        );
    for invalid in [
        format!("{display}//dir/body"),
        format!("{display}/./dir/body"),
        format!("{display}/dir/../dir/body"),
    ] {
        assert!(
            sources
                .classify(&classification_reservation, "alias", &invalid)
                .is_err()
        );
    }
    let (mut command, reservation) = general(&actor, token, command, &resources, &scope, &runtime);
    for (path, target) in [
        ("file", "dir/body"),
        ("directory", "dir"),
        ("missing", "absent"),
    ] {
        command = apply(
            command,
            &reservation,
            &sources,
            link(path, &source, target),
            &runtime,
            &scope,
        );
    }
    let root_text = reservation.display_root().as_path().to_str().unwrap();
    let own_target = NormalizedAbsolutePath::new(format!("{root_text}/file")).unwrap();
    command = apply(
        command,
        &reservation,
        &sources,
        Op::Symlink {
            path: "own".into(),
            target: own_target,
            source: None,
        },
        &runtime,
        &scope,
    );
    command = apply(
        command,
        &reservation,
        &sources,
        Op::Read("directory/body".into()),
        &runtime,
        &scope,
    );
    assert!(
        matches!(command.epoch().transcripts().unwrap()[0].records().last().unwrap().result(),ResultValue::Read(body) if body.equals_slice(b"selected source bytes"))
    );
    command = apply(
        command,
        &reservation,
        &sources,
        Op::Exists("missing".into()),
        &runtime,
        &scope,
    );
    assert!(matches!(
        command.epoch().transcripts().unwrap()[0]
            .records()
            .last()
            .unwrap()
            .result(),
        ResultValue::Exists(false)
    ));
    let count = command.epoch().transcripts().unwrap()[0].records().len();
    let (command, root) = command
        .seal_root(&reservation, count, &scope, &|| true)
        .unwrap();
    assert_eq!(root.state().source_bindings().len(), 1);
    assert_eq!(
        root.state().entry("file").unwrap().node(),
        &slug_bzlmod_v2::RepositoryWorkspaceNode::Link(
            source
                .reservation()
                .display_root()
                .as_path()
                .join("dir/body")
                .to_str()
                .unwrap()
                .into()
        )
    );
    let resolved = root.state().resolve_path("own").unwrap();
    let digest = match source.state().entry("dir/body").unwrap().node() {
        Node::File(digest) => *digest,
        _ => panic!(),
    };
    root.verify_routed_source(
        "own",
        resolved.content_origin(),
        resolved.real_path(),
        digest,
        &scope,
    )
    .unwrap();
    assert!(
        root.verify_routed_source(
            "missing",
            resolved.content_origin(),
            resolved.real_path(),
            digest,
            &scope
        )
        .is_err()
    );
    let before = scope.snapshot();
    let fds = resources.snapshot().native_fds;
    let wrong_origin = slug_workspace_v2::ResolvedPathContentOrigin::new(
        root.anchor().namespace(),
        Some(root.anchor().clone()),
    )
    .unwrap();
    assert!(
        root.verify_routed_source_metadata("own", &wrong_origin, resolved.real_path(), digest)
            .is_err()
    );
    let wrong_path = NormalizedAbsolutePath::new(
        source
            .reservation()
            .display_root()
            .as_path()
            .join("dir/absent"),
    )
    .unwrap();
    assert!(
        root.verify_routed_source_metadata("own", resolved.content_origin(), &wrong_path, digest)
            .is_err()
    );
    let wrong_digest = FileContentDigest::new([0x55; 32], digest.size_bytes()).unwrap();
    assert!(
        root.verify_routed_source_metadata(
            "own",
            resolved.content_origin(),
            resolved.real_path(),
            wrong_digest
        )
        .is_err()
    );
    assert_eq!(scope.snapshot().source_attempts, before.source_attempts);
    assert_eq!(resources.snapshot().native_fds, fds);
    let mut reader = root
        .open_routed_member(
            "own",
            resolved.content_origin(),
            resolved.real_path(),
            digest,
            &scope,
        )
        .unwrap();
    let observer = command.retirement_observer();
    drop(command);
    assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
    drop(observer);
    actor.discard(token).unwrap();
    drop(actor);
    drop(source);
    drop(sources);
    drop(views);
    drop(root);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"selected source bytes");
    scope.cancel();
    assert!(reader.read(&mut [0]).is_err());
    drop(reader);
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

#[test]
#[ignore = "actual CLI Source capture with exact content-attempt bound and changed-link refusal; finite native supervisor required"]
fn native_source_link_reads_charge_content_only_and_refuse_changed_own_route_before_source_io() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir(workspace.path().join("dir")).unwrap();
    std::fs::write(workspace.path().join("payload"), b"body").unwrap();
    File::create(workspace.path().join("large"))
        .unwrap()
        .set_len(2 * 1024 * 1024 + 1)
        .unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let normalized = NormalizedAbsolutePath::new(workspace.path()).unwrap();
    let scope = scope(&normalized, &resources);
    let actor = RepositoryMaterializer::new(normalized);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (token, command, source) = initialize(
        &actor,
        workspace.path(),
        &resources,
        &scope,
        &runtime,
        false,
    )
    .unwrap();
    let views = [View::from_source(source.state().clone()).unwrap()];
    let sources = command.source_link_set_for_test(&views, &scope).unwrap();
    let command = own_aliases(
        &actor, token, command, &resources, &sources, &scope, &runtime,
    );
    let (mut command, reservation) = general(&actor, token, command, &resources, &scope, &runtime);
    command = apply(
        command,
        &reservation,
        &sources,
        Op::File {
            path: "own".into(),
            body: slug_bzlmod_v2::RepositoryWorkspaceBody::from_arc(Arc::from(&b"own"[..])),
            mode: 0o644,
        },
        &runtime,
        &scope,
    );
    for (path, target) in [
        ("file", "payload"),
        ("missing", "absent"),
        ("directory", "dir"),
        ("oversize", "large"),
    ] {
        command = apply(
            command,
            &reservation,
            &sources,
            link(path, &source, target),
            &runtime,
            &scope,
        );
    }
    let count = command
        .epoch()
        .transcripts()
        .unwrap()
        .iter()
        .find(|t| t.reservation() == &reservation)
        .unwrap()
        .records()
        .len();
    let (command, root) = command
        .seal_root(&reservation, count, &scope, &|| true)
        .unwrap();
    let io = RootIo::new(&root.file, io_limits(root.mode, &reservation)).unwrap();
    let active = || true;
    let route = routed::RouteRoots::new(
        &root.file,
        root.state(),
        &sources,
        &resources,
        &scope,
        &active,
    );
    let before = scope.snapshot();
    drop(route.read("own").unwrap());
    assert_eq!(scope.snapshot().source_attempts, before.source_attempts);
    assert_eq!(scope.snapshot().decoded_bytes, before.decoded_bytes);
    io.unlink(Path::new("own")).unwrap();
    let target = NormalizedAbsolutePath::new(
        source
            .reservation()
            .display_root()
            .as_path()
            .join("payload"),
    )
    .unwrap();
    let proof = sources
        .validate_absolute(&reservation, "own", &target)
        .unwrap();
    io.symlink_validated_absolute(Path::new("own"), &proof)
        .unwrap();
    drop(proof);
    let foreign = std::cell::Cell::new(0);
    let observe = |namespace| {
        if namespace == source.anchor().namespace() {
            foreign.set(foreign.get() + 1);
        }
    };
    assert!(
        routed::RouteRoots::new(
            &root.file,
            root.state(),
            &sources,
            &resources,
            &scope,
            &|| true
        )
        .observe_for_test(&observe)
        .read("own")
        .is_err()
    );
    assert_eq!(foreign.get(), 0);
    assert_eq!(scope.snapshot().source_attempts, before.source_attempts);
    for path in ["missing", "directory", "oversize"] {
        assert!(route.read(path).is_err());
    }
    assert_eq!(scope.snapshot().source_attempts, before.source_attempts + 3);
    assert_eq!(before.source_attempts, 0);
    for _ in 3..64 {
        drop(route.read("file").unwrap());
    }
    assert_eq!(scope.snapshot().source_attempts, 64);
    let peak = resources.snapshot().peak_native_fds;
    let facts = std::cell::Cell::new(0);
    let observe_refusal = |_| facts.set(facts.get() + 1);
    assert!(
        routed::RouteRoots::new(
            &root.file,
            root.state(),
            &sources,
            &resources,
            &scope,
            &active
        )
        .observe_for_test(&observe_refusal)
        .read("file")
        .is_err()
    );
    assert_eq!(facts.get(), 0);
    assert_eq!(resources.snapshot().peak_native_fds, peak);
    drop(route);
    drop(io);
    let observer = command.retirement_observer();
    drop(command);
    assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
    drop(observer);
    actor.discard(token).unwrap();
    drop(actor);
    drop(root);
    drop(source);
    drop(sources);
    drop(views);
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
