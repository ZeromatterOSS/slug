use slug_bzlmod_v2::HostRepositoryLabelPathValue;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryExecuteResult;
use slug_bzlmod_v2::RepositoryWorkspaceArgument;
use slug_bzlmod_v2::RepositoryWorkspaceExecute;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use starlark_map::small_map::SmallMap;

use super::super::super::native_tests::reservation;
use super::super::super::progress::command::RepositoryWorkspaceCommand;
use super::super::super::tests::resources;
use super::super::super::tests::scope;
use super::*;

#[test]
fn physical_process_admission_checks_allocated_bytes_and_inodes_independently() {
    let exact = QuotaFacts {
        bytes: ARCHIVE_BYTES,
        inodes: 262144,
        allocated: PROCESS_BYTES,
        used_inodes: 8192,
    };
    assert!(admit_physical(exact).is_ok());
    assert!(matches!(
        admit_physical(QuotaFacts {
            allocated: PROCESS_BYTES + 1,
            ..exact
        }),
        Err(PhysicalRootError::Backing)
    ));
    assert!(matches!(
        admit_physical(QuotaFacts {
            used_inodes: 8193,
            ..exact
        }),
        Err(PhysicalRootError::Backing)
    ));
}

fn execute(reservation: &RepositoryWorkspaceReservation) -> RepositoryWorkspaceOperation {
    RepositoryWorkspaceOperation::Execute(
        RepositoryWorkspaceExecute::new(
            Arc::from([RepositoryWorkspaceArgument::String(
                "trusted-quota-fixture".into(),
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

fn command_with_first_file(
    reservation: &Arc<RepositoryWorkspaceReservation>,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
) -> RepositoryWorkspaceCommand {
    let (builder, initial) =
        initialized_archive_for_test(reservation.clone(), resources, scope).unwrap();
    let mut command = RepositoryWorkspaceCommand::new(
        reservation.workspace().clone(),
        reservation.lease(),
        reservation.limits(),
        resources.clone(),
        scope.clone(),
        &|| true,
    )
    .unwrap()
    .reserve_root(builder, initial, scope, &|| true)
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let request = command
        .request(
            reservation,
            RepositoryWorkspaceOperation::File {
                path: "out".into(),
                body: Arc::from(super::super::native_fixture::BODIES[0]),
                mode: 0o644,
            },
            scope,
            &|| true,
        )
        .unwrap();
    command = command
        .apply(
            request,
            &runtime,
            &crate::runtime::ArchiveDistdirPolicy::from_request(
                reservation.workspace().as_path(),
                &[],
            )
            .unwrap(),
            scope,
            &|| true,
        )
        .unwrap();
    command
}

#[test]
#[ignore = "requires trusted Archive→Process same-backing bwrap quota proof"]
fn native_archive_process_shrink_is_shared_and_verified_before_fixed_payload() {
    let fixture = tempfile::Builder::new()
        .prefix("slug-wp425-rolling-")
        .tempdir()
        .unwrap();
    let display = fixture.path().join("root");
    std::fs::create_dir(&display).unwrap();
    let reservation = reservation(&display, 1401);
    let resources = resources();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope();
    let producer = resources.reserve(1024 * 1024, 0, 0).unwrap();
    let command = command_with_first_file(&reservation, &resources, &scope);
    let prior = command.epoch().transcripts().unwrap()[0].clone();
    let request = command
        .request(&reservation, execute(&reservation), &scope, &|| true)
        .unwrap();
    assert_eq!(request.index(), 1);
    let admission = command
        .admit_first_execute(request, &scope, &|| true)
        .unwrap();
    let mut transition = ProcessQuotaTransition::new(admission, &|| true).unwrap();
    assert_eq!(resources.snapshot().backing_bytes, ARCHIVE_BYTES);
    let original = transition.builder().file.as_raw_fd();
    super::super::native_fixture::quota_transition(&display, &mut transition, &resources, &scope)
        .unwrap();
    assert_eq!(resources.snapshot().backing_bytes, PROCESS_BYTES);
    let ready = transition.into_ready().unwrap();
    assert_eq!(ready.builder().file.as_raw_fd(), original);
    assert_eq!(ready.builder().mode, RepositoryWorkspaceMode::Process);
    assert_eq!(
        facts(&ready.builder().file, RepositoryWorkspaceMode::Process)
            .unwrap()
            .bytes,
        PROCESS_BYTES
    );
    let captured =
        super::super::super::snapshot::capture(ready.builder(), prior.current(), &scope, &|| true)
            .unwrap();
    let completed = captured
        .complete(&prior, ready.request().clone(), result())
        .unwrap();
    let record = completed.transcript().records().last().unwrap();
    assert!(
        matches!(record.next().entry("out").unwrap().node(),RepositoryWorkspaceNode::File(body)if body.as_ref()==super::super::native_fixture::BODIES[1])
    );
    let ProcessQuotaReady {
        pending_mount,
        owner,
    } = ready;
    let lease = pending_mount
        .accept(
            QuiescedMountCompletion {
                record: record.as_ref(),
                root: owner.builder().identity,
            },
            &scope,
            &|| true,
        )
        .unwrap();
    let next = completed
        .transcript()
        .request(execute(&reservation))
        .unwrap();
    let attempt = lease
        .begin(owner.builder(), next.clone(), &scope, &|| true)
        .unwrap();
    let (receipt, inflight) = super::super::native_fixture::launch(
        2,
        &display,
        Some(attempt),
        Some(owner.builder()),
        &resources,
        &scope,
    )
    .unwrap();
    let super::super::native_fixture::Receipt {
        root: received,
        successor,
        backing,
        _receipt: receipt_credit,
    } = receipt;
    assert_eq!(
        facts(&received, RepositoryWorkspaceMode::Process)
            .unwrap()
            .inodes,
        8192
    );
    drop(received);
    let pending = inflight
        .unwrap()
        .stage(successor, &scope, &|| true)
        .unwrap();
    let captured = super::super::super::snapshot::capture(
        owner.builder(),
        completed.transcript().current(),
        &scope,
        &|| true,
    )
    .unwrap();
    let final_record = captured
        .complete(completed.transcript(), next, result())
        .unwrap();
    let record = final_record.transcript().records().last().unwrap();
    assert!(
        matches!(record.next().entry("out").unwrap().node(),RepositoryWorkspaceNode::File(body)if body.as_ref()==super::super::native_fixture::BODIES[2])
    );
    let lease = pending
        .accept(
            QuiescedMountCompletion {
                record: record.as_ref(),
                root: owner.builder().identity,
            },
            &scope,
            &|| true,
        )
        .unwrap();
    drop(receipt_credit);
    drop(backing);
    drop(owner);
    assert_eq!(unsafe { libc::fcntl(original, libc::F_GETFD) }, -1);
    assert_eq!(resources.snapshot().backing_bytes, PROCESS_BYTES);
    drop(lease);
    drop(final_record);
    drop(completed);
    drop(prior);
    drop(producer);
    assert_eq!(
        (
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes,
            resources.snapshot().retained_bytes
        ),
        (0, 0, baseline)
    );
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
}

#[test]
#[ignore = "requires actual Archive tmpfs physical page and inode admission facts"]
fn native_physical_precheck_rejects_page_slack_and_actual_inode_charge_crossing() {
    use std::os::fd::FromRawFd;
    let fixture = tempfile::Builder::new()
        .prefix("slug-wp425-rolling-")
        .tempdir()
        .unwrap();
    let display = fixture.path().join("root");
    std::fs::create_dir(&display).unwrap();
    let resources = resources();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope();
    let receipt = super::super::native_fixture::archive_seed(&display, &resources, &scope).unwrap();
    // Partial movement preserves the receipt's remaining field drop order:
    // actual root File precedes its backing guard even on a failed assertion.
    drop(receipt.successor);
    let scratch = resources.reserve(IO_SCRATCH, 2, 0).unwrap();
    let before = facts(&receipt.root, RepositoryWorkspaceMode::Archive).unwrap();
    let create = |name: &std::ffi::CStr, length: u64| {
        let fd = unsafe {
            libc::openat(
                receipt.root.as_raw_fd(),
                name.as_ptr(),
                libc::O_CREAT | libc::O_EXCL | libc::O_RDWR | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        assert!(fd >= 0, "{}", io::Error::last_os_error());
        let file = unsafe { File::from_raw_fd(fd) };
        if length != 0 {
            assert_eq!(
                unsafe { libc::ftruncate(file.as_raw_fd(), length as i64) },
                0
            );
            assert_eq!(
                unsafe { libc::fallocate(file.as_raw_fd(), 0, 0, length as i64) },
                0,
                "{}",
                io::Error::last_os_error()
            );
        }
        file
    };
    let left = create(c"page-a", PROCESS_BYTES / 2 + 1);
    let right = create(c"page-b", PROCESS_BYTES / 2 - 1);
    assert_eq!(
        left.metadata().unwrap().len() + right.metadata().unwrap().len(),
        PROCESS_BYTES
    );
    drop(left);
    drop(right);
    let slack = facts(&receipt.root, RepositoryWorkspaceMode::Archive).unwrap();
    assert_eq!(slack.allocated - before.allocated, PROCESS_BYTES + 4096);
    assert!(matches!(
        admit_physical(slack),
        Err(PhysicalRootError::Backing)
    ));
    for name in [c"page-a", c"page-b"] {
        assert_eq!(
            unsafe { libc::unlinkat(receipt.root.as_raw_fd(), name.as_ptr(), 0) },
            0
        );
    }
    let cleaned = facts(&receipt.root, RepositoryWorkspaceMode::Archive).unwrap();
    assert_eq!(cleaned.allocated, before.allocated);
    assert_eq!(cleaned.used_inodes, before.used_inodes);
    // tmpfs also charges metadata/security-xattr storage against nr_inodes.
    // Count actual charged units after every create; a named node is not a
    // promise of exactly one statvfs unit on the running kernel.
    let mut previous = cleaned;
    let mut crossing = None;
    for index in 0..8192 {
        scope.check().unwrap();
        let name = std::ffi::CString::new(format!("inode-{index}")).unwrap();
        drop(create(&name, 0));
        let observed = facts(&receipt.root, RepositoryWorkspaceMode::Archive).unwrap();
        assert!(
            observed.used_inodes > previous.used_inodes,
            "nonpositive tmpfs charge at named entry {index}: {previous:?} -> {observed:?}"
        );
        assert_eq!((observed.bytes, observed.inodes), (ARCHIVE_BYTES, 262144));
        if observed.used_inodes > 8192 {
            crossing = Some((index + 1, previous, observed));
            break;
        }
        previous = observed;
    }
    let (created, accepted, refused) =
        crossing.expect("bounded actual inode charge must cross Process allowance");
    eprintln!(
        "Archive actual inode charge: created={created}, last_admitted={accepted:?}, first_refused={refused:?}"
    );
    assert!(accepted.used_inodes <= 8192);
    assert!(refused.used_inodes > 8192);
    assert!(admit_physical(accepted).is_ok());
    assert!(matches!(
        admit_physical(refused),
        Err(PhysicalRootError::Backing)
    ));
    assert_eq!(resources.snapshot().backing_bytes, ARCHIVE_BYTES);
    drop(scratch);
    drop(receipt.root);
    drop(receipt.backing);
    drop(receipt._receipt);
    assert_eq!(
        (
            resources.snapshot().retained_bytes,
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
}

#[test]
#[ignore = "requires actual full Process tmpfs page and inode exhaustion"]
fn native_process_quota_rejects_one_more_page_and_measured_inode_exhaustion() {
    use std::os::fd::FromRawFd;
    let resources = resources();
    let baseline = resources.snapshot().retained_bytes;
    for inode_case in [false, true] {
        let fixture = tempfile::Builder::new()
            .prefix("slug-wp425-rolling-")
            .tempdir()
            .unwrap();
        let display = fixture.path().join("root");
        std::fs::create_dir(&display).unwrap();
        let scope = scope();
        let (receipt, inflight) =
            super::super::native_fixture::launch(0, &display, None, None, &resources, &scope)
                .unwrap();
        assert!(inflight.is_none());
        drop(receipt.successor);
        let scratch = resources.reserve(IO_SCRATCH, 1, 0).unwrap();
        assert_eq!(
            unsafe { libc::unlinkat(receipt.root.as_raw_fd(), c"out".as_ptr(), 0) },
            0
        );
        let empty = facts(&receipt.root, RepositoryWorkspaceMode::Process).unwrap();
        assert_eq!(
            (
                empty.bytes,
                empty.inodes,
                empty.allocated,
                empty.used_inodes
            ),
            (PROCESS_BYTES, 8192, 0, 1)
        );
        let create = |name: &std::ffi::CStr| {
            let fd = unsafe {
                libc::openat(
                    receipt.root.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_RDWR
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(unsafe { File::from_raw_fd(fd) })
            }
        };
        if inode_case {
            let mut previous = empty;
            let mut exhausted = None;
            for index in 0..8193 {
                scope.check().unwrap();
                let name = std::ffi::CString::new(format!("inode-{index}")).unwrap();
                match create(&name) {
                    Ok(file) => {
                        drop(file);
                        let observed =
                            facts(&receipt.root, RepositoryWorkspaceMode::Process).unwrap();
                        assert!(
                            observed.used_inodes > previous.used_inodes,
                            "nonpositive tmpfs charge at named entry {index}: {previous:?} -> {observed:?}"
                        );
                        assert!(observed.used_inodes <= 8192);
                        previous = observed;
                    }
                    Err(error) => {
                        assert_eq!(error.raw_os_error(), Some(libc::ENOSPC));
                        assert_eq!(
                            facts(&receipt.root, RepositoryWorkspaceMode::Process).unwrap(),
                            previous
                        );
                        exhausted = Some((index, previous));
                        break;
                    }
                }
            }
            let (created, exhausted) =
                exhausted.expect("bounded actual inode charge must exhaust Process quota");
            eprintln!("Process actual inode exhaustion: created={created}, final={exhausted:?}");
            assert_eq!(exhausted.inodes, 8192);
            assert!(exhausted.used_inodes <= exhausted.inodes);
            // A failed creation must roll back its inode and security metadata,
            // even when charged-unit granularity leaves less than one node's
            // complete charge free. Repeated refusal preserves exact facts.
            for _ in 0..2 {
                let error = create(c"inode-extra").unwrap_err();
                assert_eq!(error.raw_os_error(), Some(libc::ENOSPC));
                assert_eq!(
                    facts(&receipt.root, RepositoryWorkspaceMode::Process).unwrap(),
                    exhausted
                );
            }
        } else {
            let file = create(c"pages").unwrap();
            assert_eq!(
                unsafe { libc::fallocate(file.as_raw_fd(), 0, 0, PROCESS_BYTES as i64) },
                0,
                "{}",
                io::Error::last_os_error()
            );
            let exact = facts(&receipt.root, RepositoryWorkspaceMode::Process).unwrap();
            assert_eq!(exact.allocated, PROCESS_BYTES);
            assert_eq!(file.metadata().unwrap().len(), PROCESS_BYTES);
            let byte = [1u8];
            assert_eq!(
                unsafe {
                    libc::pwrite(
                        file.as_raw_fd(),
                        byte.as_ptr().cast(),
                        1,
                        PROCESS_BYTES as i64,
                    )
                },
                -1
            );
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(libc::ENOSPC)
            );
            assert_eq!(file.metadata().unwrap().len(), PROCESS_BYTES);
            assert_eq!(
                facts(&receipt.root, RepositoryWorkspaceMode::Process).unwrap(),
                exact
            );
            drop(file);
        }
        assert_eq!(resources.snapshot().backing_bytes, PROCESS_BYTES);
        drop(scratch);
        drop(receipt.root);
        drop(receipt.backing);
        drop(receipt._receipt);
        assert_eq!(
            (
                resources.snapshot().retained_bytes,
                resources.snapshot().native_fds,
                resources.snapshot().backing_bytes
            ),
            (baseline, 0, 0)
        );
    }
}

#[test]
#[ignore = "requires trusted failure after actual quota shrink before payload ACK"]
fn native_failure_after_quota_commit_poisoned_after_reap_keeps_guard_until_disposal() {
    use std::os::fd::FromRawFd;
    use std::os::unix::fs::FileExt;
    let fixture = tempfile::Builder::new()
        .prefix("slug-wp425-rolling-")
        .tempdir()
        .unwrap();
    let display = fixture.path().join("root");
    std::fs::create_dir(&display).unwrap();
    let reservation = reservation(&display, 1402);
    let resources = resources();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope();
    let producer = resources.reserve(1024 * 1024, 0, 0).unwrap();
    let command = command_with_first_file(&reservation, &resources, &scope);
    let request = command
        .request(&reservation, execute(&reservation), &scope, &|| true)
        .unwrap();
    let admission = command
        .admit_first_execute(request, &scope, &|| true)
        .unwrap();
    let mut transition = ProcessQuotaTransition::new(admission, &|| true).unwrap();
    let original = transition.builder().file.as_raw_fd();
    let error = super::super::native_fixture::quota_transition_fail_after_commit(
        &display,
        &mut transition,
        &resources,
        &scope,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("after verified quota commit before ACK")
    );
    assert!(transition.confirmed);
    assert!(transition.poisoned);
    assert!(transition.pending.is_none());
    assert!(transition.attempt.is_none());
    assert!(transition.inflight.is_none());
    assert!(matches!(
        transition.namespace(),
        Err(PhysicalRootError::Mode)
    ));
    assert!(matches!(
        transition.transfer(unsafe { BorrowedFd::borrow_raw(0) }, &[0; 16], &|| true),
        Err(PhysicalRootError::Mode)
    ));
    // The poisoned transition retains the data FD and its exclusive command's
    // domain FD until disposal; all usable rolling tokens have retired.
    assert_eq!(resources.snapshot().native_fds, 2);
    assert_eq!(transition.builder().mode, RepositoryWorkspaceMode::Process);
    assert_eq!(
        facts(&transition.builder().file, RepositoryWorkspaceMode::Process)
            .unwrap()
            .bytes,
        PROCESS_BYTES
    );
    assert_eq!(resources.snapshot().backing_bytes, PROCESS_BYTES);
    assert_ne!(unsafe { libc::fcntl(original, libc::F_GETFD) }, -1);
    // The fixed payload was blocked awaiting ACK; the actual original FD still
    // names the accepted predecessor after the failed helper is killed/reaped.
    let scratch = resources.reserve(IO_SCRATCH, 1, 0).unwrap();
    let raw = unsafe {
        libc::openat(
            original,
            c"out".as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    assert!(raw >= 0, "{}", io::Error::last_os_error());
    let file = unsafe { File::from_raw_fd(raw) };
    let mut bytes = [0u8; 4];
    let count = file.read_at(&mut bytes, 0).unwrap();
    assert_eq!(&bytes[..count], super::super::native_fixture::BODIES[0]);
    drop(file);
    drop(scratch);
    assert!(matches!(
        transition.into_ready(),
        Err(PhysicalRootError::Mode)
    ));
    assert_eq!(unsafe { libc::fcntl(original, libc::F_GETFD) }, -1);
    drop(producer);
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
