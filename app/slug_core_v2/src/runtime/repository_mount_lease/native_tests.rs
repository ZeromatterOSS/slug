use std::os::fd::AsRawFd;

use slug_bzlmod_v2::HostRepositoryLabelPathValue;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryExecuteResult;
use slug_bzlmod_v2::RepositoryWorkspaceArgument;
use slug_bzlmod_v2::RepositoryWorkspaceEntry;
use slug_bzlmod_v2::RepositoryWorkspaceExecute;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_bzlmod_v2::RepositoryWorkspaceTranscript;
use starlark_map::small_map::SmallMap;

use super::super::native_tests::reservation;
use super::super::snapshot::CreditedWorkspaceTranscript;
use super::super::snapshot::capture;
use super::super::tests::resources;
use super::super::tests::scope;
use super::*;

fn execute(reservation: &RepositoryWorkspaceReservation) -> RepositoryWorkspaceOperation {
    RepositoryWorkspaceOperation::Execute(
        RepositoryWorkspaceExecute::new(
            Arc::from([RepositoryWorkspaceArgument::String(
                "trusted-rolling-fixture".into(),
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
            slug_bzlmod_v2::RepositoryWorkspaceReadViewManifest::empty(reservation),
        )
        .unwrap(),
    )
}
fn result() -> RepositoryWorkspaceResult {
    RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
        0,
        Arc::from([]),
        Arc::from([]),
    ))
}

#[test]
#[ignore = "requires trusted Rust bwrap three-launch rolling mount proof"]
fn native_three_launch_rolling_lease_handoff_waits_for_real_snapshot_and_record() {
    let fixture = tempfile::Builder::new()
        .prefix("slug-wp425-rolling-")
        .tempdir()
        .unwrap();
    let display = fixture.path().join("root");
    std::fs::create_dir(&display).unwrap();
    let reservation = reservation(&display, 1201);
    let resources = resources();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope();
    let incoming = resources.reserve(1024 * 1024, 0, 0).unwrap();
    let initial_credit = resources
        .reserve(
            RepositoryWorkspaceTranscript::initial_storage_bytes(&reservation, 0o755).unwrap(),
            0,
            0,
        )
        .unwrap();
    let mut current = CreditedWorkspaceTranscript {
        transcript: RepositoryWorkspaceTranscript::new(reservation.clone(), 0o755).unwrap(),
        dependencies: None,
        credit: initial_credit,
    };
    let mut root: Option<RepositoryPhysicalRootBuilder> = None;
    let mut lease: Option<RepositoryMountLease> = None;
    let mut original = -1;
    for phase in 0..3 {
        let request = current.transcript().request(execute(&reservation)).unwrap();
        assert_eq!(request.index(), phase);
        let attempt = lease.take().map(|lease| {
            lease
                .begin(root.as_ref().unwrap(), request.clone(), &scope, &|| true)
                .unwrap()
        });
        let (receipt, inflight) =
            native_fixture::launch(phase, &display, attempt, root.as_ref(), &resources, &scope)
                .unwrap();
        let native_fixture::Receipt {
            root: received,
            successor,
            backing,
            _receipt: receipt_credit,
        } = receipt;
        if phase == 0 {
            original = received.as_raw_fd();
            root = Some(
                RepositoryPhysicalRootBuilder::from_quiescent_tmpfs_with_backing(
                    received,
                    reservation.clone(),
                    RepositoryWorkspaceMode::Process,
                    resources.clone(),
                    backing.clone(),
                    &scope,
                    &|| true,
                )
                .unwrap(),
            );
        } else {
            assert!(same_backing(
                identity(&received).unwrap(),
                root.as_ref().unwrap().identity
            ));
            drop(received);
        }
        let selected = root.as_ref().unwrap();
        if phase == 0 {
            let retained = resources.snapshot().retained_bytes;
            selected
                .backing
                .bind(selected.identity, &reservation)
                .unwrap();
            let wrong_mount = RootIdentity {
                mount: selected.identity.mount.checked_add(1).unwrap(),
                ..selected.identity
            };
            assert!(matches!(
                selected.backing.bind(wrong_mount, &reservation),
                Err(PhysicalRootError::Backing)
            ));
            let foreign = Arc::new(
                RepositoryWorkspaceReservation::new(
                    reservation.workspace().clone(),
                    reservation.owner().clone(),
                    reservation.repository().clone(),
                    reservation.ordinal().checked_add(1).unwrap(),
                    reservation.expected_repo_spec().clone(),
                    reservation.display_root().clone(),
                    reservation.instance(),
                    reservation.lease(),
                    reservation.limits(),
                )
                .unwrap(),
            );
            assert!(matches!(
                selected.backing.bind(selected.identity, &foreign),
                Err(PhysicalRootError::Backing)
            ));
            assert_eq!(resources.snapshot().retained_bytes, retained);
            assert_eq!(resources.snapshot().backing_bytes, PROCESS_BYTES);
        }
        let pending = if let Some(inflight) = inflight {
            inflight.stage(successor, &scope, &|| true).unwrap()
        } else {
            PendingMountLease::bootstrap(selected, request.clone(), successor, &scope, &|| true)
                .unwrap()
        };
        // launch returned only after actual namespace/helper reap. Capture reads
        // the ORIGINAL pinned descriptor, not the latest child display path.
        let captured = capture(selected, current.transcript().current(), &scope, &|| true).unwrap();
        let completed = captured
            .complete(current.transcript(), request, result())
            .unwrap();
        let record = completed.transcript().records().last().unwrap();
        assert!(
            matches!(record.next().entry("out").unwrap().node(),RepositoryWorkspaceNode::File(body)if body.as_ref()==native_fixture::BODIES[phase])
        );
        let next = pending
            .accept(
                QuiescedMountCompletion {
                    record: record.as_ref(),
                    root: selected.identity,
                },
                &scope,
                &|| true,
            )
            .unwrap();
        assert_eq!(next.index, phase + 1);
        assert_eq!(next.predecessor, record.next().clone());
        if phase == 0 {
            // Same reservation/index, different complete predecessor: a digest
            // or path-only comparison must not authorize this branch.
            let initial = RepositoryWorkspaceTranscript::new(reservation.clone(), 0o755).unwrap();
            let branch = initial
                .complete(
                    initial.request(execute(&reservation)).unwrap(),
                    result(),
                    RepositoryWorkspaceMode::Process,
                    vec![
                        RepositoryWorkspaceEntry::new(
                            "".into(),
                            0o755,
                            RepositoryWorkspaceNode::Directory,
                        )
                        .unwrap(),
                        RepositoryWorkspaceEntry::new(
                            "out".into(),
                            0o644,
                            RepositoryWorkspaceNode::File(Arc::from(&b"wrong"[..])),
                        )
                        .unwrap(),
                    ],
                )
                .unwrap();
            assert!(matches!(
                next.validate_request(
                    selected,
                    &branch.request(execute(&reservation)).unwrap(),
                    &scope,
                    &|| true
                ),
                Err(MountLeaseError::Request)
            ));
        }
        let replay = completed
            .transcript()
            .request_at(0, execute(&reservation))
            .unwrap();
        assert!(matches!(
            next.validate_request(selected, &replay, &scope, &|| true),
            Err(MountLeaseError::Request)
        ));
        assert!(matches!(
            next.validate_request(
                selected,
                &completed
                    .transcript()
                    .request(execute(&reservation))
                    .unwrap(),
                &scope,
                &|| false
            ),
            Err(MountLeaseError::Root(PhysicalRootError::Inactive))
        ));
        lease = Some(next);
        current = completed;
        drop(receipt_credit);
        drop(backing);
        assert_eq!(resources.snapshot().native_fds, 3);
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        if phase == 0 {
            std::fs::write(display.join("out"), b"foreign-host").unwrap();
        }
    }
    assert_eq!(std::fs::read(display.join("out")).unwrap(), b"foreign-host");
    scope.cancel();
    let request = current.transcript().request(execute(&reservation)).unwrap();
    assert!(matches!(
        lease
            .as_ref()
            .unwrap()
            .validate_request(root.as_ref().unwrap(), &request, &scope, &|| true),
        Err(MountLeaseError::Root(PhysicalRootError::SourceService(
            SourceServiceError::Cancelled
        )))
    ));
    drop(request);
    drop(root);
    assert_eq!(unsafe { libc::fcntl(original, libc::F_GETFD) }, -1);
    // The live detached token still pins this backing after the original root
    // descriptor is gone; its shared allowance cannot retire early.
    assert_eq!(resources.snapshot().native_fds, 2);
    assert_eq!(resources.snapshot().backing_bytes, PROCESS_BYTES);
    drop(current);
    drop(incoming);
    drop(lease);
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

#[test]
#[ignore = "requires trusted rolling bootstrap and retained-bank shortage proof"]
fn native_pending_accept_credit_shortage_retires_successor_and_payload_owners() {
    let fixture = tempfile::Builder::new()
        .prefix("slug-wp425-rolling-")
        .tempdir()
        .unwrap();
    let display = fixture.path().join("root");
    std::fs::create_dir(&display).unwrap();
    let reservation = reservation(&display, 1202);
    let resources = resources();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope();
    let incoming = resources.reserve(1024 * 1024, 0, 0).unwrap();
    let initial_credit = resources
        .reserve(
            RepositoryWorkspaceTranscript::initial_storage_bytes(&reservation, 0o755).unwrap(),
            0,
            0,
        )
        .unwrap();
    let initial = CreditedWorkspaceTranscript {
        transcript: RepositoryWorkspaceTranscript::new(reservation.clone(), 0o755).unwrap(),
        dependencies: None,
        credit: initial_credit,
    };
    let request = initial.transcript().request(execute(&reservation)).unwrap();
    let (receipt, inflight) =
        native_fixture::launch(0, &display, None, None, &resources, &scope).unwrap();
    assert!(inflight.is_none());
    let native_fixture::Receipt {
        root: input,
        successor,
        backing,
        _receipt: receipt_credit,
    } = receipt;
    let builder = RepositoryPhysicalRootBuilder::from_quiescent_tmpfs_with_backing(
        input,
        reservation,
        RepositoryWorkspaceMode::Process,
        resources.clone(),
        backing.clone(),
        &scope,
        &|| true,
    )
    .unwrap();
    let pending =
        PendingMountLease::bootstrap(&builder, request.clone(), successor, &scope, &|| true)
            .unwrap();
    let token = pending.successor.token.as_raw_fd();
    let namespace = pending.successor.owner.as_raw_fd();
    let completed = capture(&builder, initial.transcript().current(), &scope, &|| true)
        .unwrap()
        .complete(initial.transcript(), request, result())
        .unwrap();
    let record = completed.transcript().records().last().unwrap();
    drop(receipt_credit);
    drop(backing);
    // The shared test bank has exactly8MiB. Admission must fail while Pending
    // still owns its request/reservation and their last retained credit guard.
    let pressure = resources
        .reserve(8 * 1024 * 1024 - resources.snapshot().retained_bytes, 0, 0)
        .unwrap();
    assert!(matches!(
        pending.accept(
            QuiescedMountCompletion {
                record: record.as_ref(),
                root: builder.identity
            },
            &scope,
            &|| true
        ),
        Err(MountLeaseError::Root(PhysicalRootError::Resource))
    ));
    for fd in [token, namespace] {
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
    }
    assert_eq!(resources.snapshot().native_fds, 1);
    assert_eq!(resources.snapshot().backing_bytes, PROCESS_BYTES);
    drop(pressure);
    drop(completed);
    drop(initial);
    drop(builder);
    drop(incoming);
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
