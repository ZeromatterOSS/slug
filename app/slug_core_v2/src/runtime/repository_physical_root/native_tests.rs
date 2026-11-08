//! Explicit native gates. Synthetic immutable protocol records describe the
//! fixed trusted fixture; these tests claim no original/archive execution proof.
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::GeneratedRepositoryArchiveEffect;
use slug_bzlmod_v2::GeneratedRepositoryArchiveFormat;
use slug_bzlmod_v2::HostRepositoryLabelPathValue;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryExecuteResult;
use slug_bzlmod_v2::RepositoryWorkspaceArgument;
use slug_bzlmod_v2::RepositoryWorkspaceEntry;
use slug_bzlmod_v2::RepositoryWorkspaceExecute;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits;
use slug_bzlmod_v2::RepositoryWorkspaceTranscript;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use starlark_map::small_map::SmallMap;

use super::native_fixture::BODY;
use super::tests::resources;
use super::tests::scope;
use super::*;

pub(super) fn reservation(display: &Path, instance: u64) -> Arc<RepositoryWorkspaceReservation> {
    let owner = crate::runtime::dice::physical_root_owner_for_test();
    let repository =
        CanonicalRepoName::new(format!("{}+repo", owner.unique_name().as_str())).unwrap();
    Arc::new(
        RepositoryWorkspaceReservation::new(
            NormalizedAbsolutePath::new("/physical-root-test").unwrap(),
            owner,
            repository,
            0,
            Arc::new(slug_bzlmod_v2::RepoSpec {
                rule_id: slug_bzlmod_v2::RepoRuleId {
                    bzl_file: slug_identity_v2::CanonicalLabel::parse("@@//:repo.bzl").unwrap(),
                    rule_name: "generic".into(),
                },
                attributes: Arc::new(SmallMap::new()),
            }),
            NormalizedAbsolutePath::new(display).unwrap(),
            PathObservationInstanceId::new(instance),
            711,
            RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, ARCHIVE_BYTES).unwrap(),
        )
        .unwrap(),
    )
}

pub(super) fn realization(
    reservation: Arc<RepositoryWorkspaceReservation>,
    mode: RepositoryWorkspaceMode,
    body: &[u8],
) -> RepositoryWorkspaceRealization {
    let initial = RepositoryWorkspaceTranscript::new(reservation.clone(), 0o755).unwrap();
    let (operation, result) = match mode {
        RepositoryWorkspaceMode::Archive => (
            RepositoryWorkspaceOperation::Archive(
                GeneratedRepositoryArchiveEffect::new(
                    Arc::from(["https://example.invalid/fixture.tar.gz".into()]),
                    [0; 32],
                    "".into(),
                    GeneratedRepositoryArchiveFormat::TarGz,
                    "physical-root-fixture".into(),
                    "".into(),
                )
                .unwrap(),
            ),
            RepositoryWorkspaceResult::Archive([0; 32]),
        ),
        RepositoryWorkspaceMode::Process => (
            RepositoryWorkspaceOperation::Execute(
                RepositoryWorkspaceExecute::new(
                    Arc::from([RepositoryWorkspaceArgument::String(
                        "trusted-fixture".into(),
                    )]),
                    RepositoryEnvironmentSnapshot::empty(),
                    SmallMap::new(),
                    600,
                    true,
                    HostRepositoryLabelPathValue::new(
                        reservation.display_root().clone(),
                        PathObservationNamespace::Materialization(reservation.instance()),
                    )
                    .unwrap(),
                    slug_bzlmod_v2::RepositoryWorkspaceReadViewManifest::empty(&reservation),
                )
                .unwrap(),
            ),
            RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                0,
                Arc::from([]),
                Arc::from([]),
            )),
        ),
    };
    initial
        .complete(
            initial.request(operation).unwrap(),
            result,
            mode,
            vec![
                RepositoryWorkspaceEntry::new("".into(), 0o755, RepositoryWorkspaceNode::Directory)
                    .unwrap(),
                RepositoryWorkspaceEntry::new(
                    "dir".into(),
                    0o755,
                    RepositoryWorkspaceNode::Directory,
                )
                .unwrap(),
                RepositoryWorkspaceEntry::new(
                    "dir/body".into(),
                    0o644,
                    RepositoryWorkspaceNode::File(Arc::from(body)),
                )
                .unwrap(),
                RepositoryWorkspaceEntry::new(
                    "link".into(),
                    0o777,
                    RepositoryWorkspaceNode::Link("dir/body".into()),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .realize(1)
        .unwrap()
}

fn closed(fd: i32) {
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
}

#[test]
#[ignore = "requires the explicit trusted bwrap tmpfs/SCM_RIGHTS fixture"]
fn native_archive_and_process_roots_seal_same_backing_and_readers_own_last_release() {
    for (index, mode) in [
        RepositoryWorkspaceMode::Archive,
        RepositoryWorkspaceMode::Process,
    ]
    .into_iter()
    .enumerate()
    {
        let input = native_fixture::root(mode);
        let raw = input.as_raw_fd();
        let fixture = tempfile::tempdir().unwrap();
        let display = fixture.path().join("logical-repo");
        let reservation = reservation(&display, 811 + index as u64);
        let terminal = realization(reservation.clone(), mode, BODY);
        let resources = resources();
        let baseline = resources.snapshot().retained_bytes;
        let scope = scope();
        let builder = RepositoryPhysicalRootBuilder::from_quiescent_tmpfs(
            input,
            reservation.clone(),
            mode,
            resources.clone(),
            &scope,
            &|| true,
        )
        .unwrap();
        assert_eq!(resources.snapshot().native_fds, 1);
        assert_eq!(resources.snapshot().backing_bytes, quota(mode).0);
        let root = builder.seal(&terminal, &scope, &|| true).unwrap();
        assert_eq!(root.file.as_raw_fd(), raw);
        assert!(Arc::ptr_eq(root.state(), terminal.final_state()));
        assert_eq!(root.reservation(), &reservation);
        assert_eq!(root.anchor().root(), reservation.display_root());
        let clone = root.clone();
        assert!(Arc::ptr_eq(&clone, &root));
        assert_eq!(resources.snapshot().native_fds, 1);
        let bytes = PathObservationDemand::new(
            root.anchor().namespace(),
            NormalizedAbsolutePath::new(display.join("dir/body")).unwrap(),
            PathObservationOperation::FileBytes,
        );
        let before = scope.snapshot();
        let observed = root.observe(&[bytes.clone()], &scope, &|| true).unwrap();
        let RepositoryWorkspaceNode::File(canonical) =
            root.state().entry("dir/body").unwrap().node()
        else {
            panic!("sealed fixture owns the regular body")
        };
        assert!(
            matches!(observed.get(&bytes).unwrap().as_ref(), PathObservationResult::FileBytes(PathOperationResult::Present(body)) if body.as_ref() == BODY && Arc::ptr_eq(body,canonical))
        );
        assert_eq!(scope.snapshot().source_attempts, 0);
        assert_eq!(
            scope.snapshot().decoded_bytes - before.decoded_bytes,
            BODY.len() as u64
        );
        // The selected spelling can later name a foreign Host tree. Neither
        // sealed membership nor reader IO may reopen that tree.
        std::fs::create_dir_all(display.join("dir")).unwrap();
        std::fs::write(display.join("dir/body"), b"foreign-host").unwrap();
        assert!(matches!(
            root.open_member("link", &scope, &|| true),
            Err(PhysicalRootError::Snapshot)
        ));
        assert!(matches!(
            root.open_member("dir", &scope, &|| true),
            Err(PhysicalRootError::Snapshot)
        ));
        assert!(matches!(
            root.open_member("../outside", &scope, &|| true),
            Err(PhysicalRootError::Snapshot)
        ));
        assert!(matches!(
            root.open_member("dir/body", &scope, &|| false),
            Err(PhysicalRootError::Inactive)
        ));
        let mut reader = root.open_member("dir/body", &scope, &|| true).unwrap();
        assert_eq!(resources.snapshot().native_fds, 2);
        assert_eq!(resources.snapshot().backing_bytes, quota(mode).0);
        let weak = Arc::downgrade(&root);
        drop(clone);
        drop(root);
        assert!(weak.upgrade().is_some());
        let mut actual = Vec::new();
        reader.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, BODY);
        assert_eq!(reader.seek(SeekFrom::Start(6)).unwrap(), 6);
        let mut tail = Vec::new();
        reader.read_to_end(&mut tail).unwrap();
        assert_eq!(tail, BODY[6..]);
        assert!(reader.seek(SeekFrom::Current(-100)).is_err());
        drop(reader);
        closed(raw);
        assert!(weak.upgrade().is_none());
        let final_usage = resources.snapshot();
        assert_eq!(
            (
                final_usage.retained_bytes,
                final_usage.native_fds,
                final_usage.backing_bytes
            ),
            (baseline, 0, 0)
        );
        assert!(final_usage.peak_native_fds >= 5);
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    }
}

#[test]
#[ignore = "requires the explicit trusted bwrap tmpfs/SCM_RIGHTS fixture"]
fn native_seal_rejects_foreign_reservation_mode_and_body_without_retaining_root() {
    let fixture = tempfile::tempdir().unwrap();
    let display = fixture.path().join("logical-repo");
    let selected = reservation(&display, 821);
    let foreign = reservation(&display, 822);
    // This is an actual filesystem quota mismatch at factory admission, not
    // merely a mismatch between the builder and realization mode enums.
    let input = native_fixture::root(RepositoryWorkspaceMode::Archive);
    let raw = input.as_raw_fd();
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let owner = scope();
    assert!(matches!(
        RepositoryPhysicalRootBuilder::from_quiescent_tmpfs(
            input,
            selected.clone(),
            RepositoryWorkspaceMode::Process,
            bank.clone(),
            &owner,
            &|| true,
        ),
        Err(PhysicalRootError::Backing)
    ));
    closed(raw);
    assert_eq!(
        (
            bank.snapshot().retained_bytes,
            bank.snapshot().native_fds,
            bank.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    // A bind-mounted subdirectory has MOUNT_ROOT and the correct superblock
    // quotas but cannot stand in for the complete tmpfs root inode.
    let input = native_fixture::bound_subdirectory(RepositoryWorkspaceMode::Process);
    let raw = input.as_raw_fd();
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let owner = scope();
    assert!(matches!(
        RepositoryPhysicalRootBuilder::from_quiescent_tmpfs(
            input,
            selected.clone(),
            RepositoryWorkspaceMode::Process,
            bank.clone(),
            &owner,
            &|| true,
        ),
        Err(PhysicalRootError::RootIdentity)
    ));
    closed(raw);
    assert_eq!(
        (
            bank.snapshot().retained_bytes,
            bank.snapshot().native_fds,
            bank.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let mut changed_body = BODY.to_vec();
    changed_body[0] ^= 1;
    for mismatch in 0..3 {
        let input = native_fixture::root(RepositoryWorkspaceMode::Archive);
        let raw = input.as_raw_fd();
        let resources = resources();
        let baseline = resources.snapshot().retained_bytes;
        let scope = scope();
        let terminal = realization(
            if mismatch == 0 {
                foreign.clone()
            } else {
                selected.clone()
            },
            if mismatch == 1 {
                RepositoryWorkspaceMode::Process
            } else {
                RepositoryWorkspaceMode::Archive
            },
            if mismatch == 2 { &changed_body } else { BODY },
        );
        let builder = RepositoryPhysicalRootBuilder::from_quiescent_tmpfs(
            input,
            selected.clone(),
            RepositoryWorkspaceMode::Archive,
            resources.clone(),
            &scope,
            &|| true,
        )
        .unwrap();
        let error = builder.seal(&terminal, &scope, &|| true).unwrap_err();
        assert!(matches!(
            (mismatch, error),
            (0, PhysicalRootError::Reservation)
                | (1, PhysicalRootError::Mode)
                | (2, PhysicalRootError::Snapshot)
        ));
        closed(raw);
        assert_eq!(
            (
                resources.snapshot().retained_bytes,
                resources.snapshot().native_fds,
                resources.snapshot().backing_bytes
            ),
            (baseline, 0, 0)
        );
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    }
}

#[test]
#[ignore = "requires the explicit trusted bwrap tmpfs/SCM_RIGHTS fixture"]
fn native_reader_cancellation_is_nonretryable_and_last_drop_releases_all_credits() {
    let input = native_fixture::root(RepositoryWorkspaceMode::Process);
    let raw = input.as_raw_fd();
    let fixture = tempfile::tempdir().unwrap();
    let reservation = reservation(&fixture.path().join("logical-repo"), 831);
    let terminal = realization(reservation.clone(), RepositoryWorkspaceMode::Process, BODY);
    let resources = resources();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope();
    let root = RepositoryPhysicalRootBuilder::from_quiescent_tmpfs(
        input,
        reservation,
        RepositoryWorkspaceMode::Process,
        resources.clone(),
        &scope,
        &|| true,
    )
    .unwrap()
    .seal(&terminal, &scope, &|| true)
    .unwrap();
    let mut reader = root.open_member("dir/body", &scope, &|| true).unwrap();
    drop(root);
    scope.cancel();
    let started = Instant::now();
    let mut bytes = Vec::new();
    let error = reader.read_to_end(&mut bytes).unwrap_err();
    assert_ne!(error.kind(), io::ErrorKind::Interrupted);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(bytes.is_empty());
    assert_eq!(resources.snapshot().native_fds, 2);
    drop(reader);
    closed(raw);
    assert_eq!(
        (
            resources.snapshot().retained_bytes,
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
}
