use std::io::Read;
use std::os::fd::AsRawFd;
use std::time::Duration;
use std::time::Instant;

use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;

fn scope(limits: SourceServiceLimits) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(2);
    SourceServiceScope::new(
        NormalizedAbsolutePath::new("/regular-file-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        limits,
    )
    .unwrap()
}

fn host_with(
    owner: &Arc<SourceServiceScope>,
    path: &Path,
    hook: impl FnMut(ReadPoint, &std::fs::File),
    read: impl FnMut(&mut std::fs::File, &mut [u8]) -> io::Result<usize>,
) -> Result<Arc<[u8]>, RegularFileError> {
    host_with_policy(owner, path, ReadPolicy::Host, hook, read)
}

fn host_with_policy(
    owner: &Arc<SourceServiceScope>,
    path: &Path,
    policy: ReadPolicy,
    mut hook: impl FnMut(ReadPoint, &std::fs::File),
    read: impl FnMut(&mut std::fs::File, &mut [u8]) -> io::Result<usize>,
) -> Result<Arc<[u8]>, RegularFileError> {
    owner.charge(SourceCounter::SourceAttempts, 1)?;
    let mut fds = [None, None];
    let result = read_regular_with(
        owner,
        path,
        policy,
        |point, file| {
            if point == ReadPoint::Pinned {
                fds[0] = Some(file.as_raw_fd());
            }
            if point == ReadPoint::BeforeRead {
                fds[1] = Some(file.as_raw_fd());
            }
            hook(point, file);
        },
        read,
    );
    // No opens occur between the read return and these exact FD probes.
    for fd in fds.into_iter().flatten() {
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
    }
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    result
}

#[test]
fn scoped_host_regular_bytes_empty_and_maximum_keep_accounting_and_owner_domains() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(".bazelignore");
    for (policy, length) in [
        (ReadPolicy::Host, 0),
        (ReadPolicy::Host, 257),
        (ReadPolicy::Host, HOST_FILE_OBJECT_BYTES),
        (ReadPolicy::KnownShas, KNOWN_SHAS_OBJECT_BYTES),
    ] {
        let bytes = (0..length).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        std::fs::write(&path, &bytes).unwrap();
        let owner = scope(SourceServiceLimits::default());
        let value = host_with_policy(
            &owner,
            &path,
            policy,
            |_, _| {},
            |file, bytes| file.read(bytes),
        )
        .unwrap();
        assert_eq!(value.as_ref(), bytes.as_slice());
        let counters = owner.snapshot();
        assert_eq!(
            (counters.source_attempts, counters.decoded_bytes),
            (1, length as u64)
        );
        assert_eq!(
            (counters.registry_attempts, counters.registry_admitted_bytes),
            (0, 0)
        );
        assert_eq!((counters.transferred_bytes, counters.written_bytes), (0, 0));
        assert_eq!(
            counters.peak_buffer_bytes,
            length * 2 + std::mem::size_of::<Vec<u8>>() + CHUNK_BYTES + 64
        );
        let weak = Arc::downgrade(&value);
        let retained = Arc::clone(&value);
        assert!(Arc::ptr_eq(&value, &retained));
        drop(value);
        assert!(weak.upgrade().is_some());
        let scope_weak = Arc::downgrade(&owner);
        drop(owner);
        assert!(scope_weak.upgrade().is_none());
        // Body history is cumulatively bounded, not tied to a per-Arc bank guard.
        assert_eq!(retained.as_ref(), bytes.as_slice());
        drop(retained);
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn scoped_host_missing_shapes_and_object_limit_are_typed() {
    use std::os::unix::ffi::OsStrExt;
    let directory = tempfile::tempdir().unwrap();
    let owner = scope(SourceServiceLimits::default());
    assert!(
        matches!(read_host_file(&owner, &directory.path().join("missing")),
        Err(RegularFileError::Io { stage: RegularFileStage::Pin, error, .. }) if error.kind() == io::ErrorKind::NotFound)
    );
    assert_eq!(owner.check(), Ok(()));
    let plain = directory.path().join("plain");
    std::fs::write(&plain, b"x").unwrap();
    assert!(matches!(read_host_file(&owner, &plain.join("child")),
        Err(RegularFileError::Io { stage: RegularFileStage::Pin, error, .. }) if error.raw_os_error() == Some(libc::ENOTDIR)));
    assert!(matches!(
        read_host_file(&owner, directory.path()),
        Err(RegularFileError::NonRegular {
            kind: PathNodeKind::Directory
        })
    ));
    assert_eq!(owner.check(), Ok(()));
    let fifo = directory.path().join("fifo");
    let cpath = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    assert!(matches!(
        read_host_file(&owner, &fifo),
        Err(RegularFileError::NonRegular {
            kind: PathNodeKind::SpecialFile
        })
    ));
    assert_eq!(owner.snapshot().source_attempts, 4);
    assert_eq!(owner.snapshot().decoded_bytes, 0);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let over = directory.path().join("over");
    std::fs::File::create(&over)
        .unwrap()
        .set_len(HOST_FILE_OBJECT_BYTES as u64 + 1)
        .unwrap();
    let owner = scope(SourceServiceLimits::default());
    assert!(matches!(
        read_host_file(&owner, &over),
        Err(RegularFileError::Scope(SourceServiceError::Limit(
            SourceCounter::DecodedBytes
        )))
    ));
    assert_eq!(owner.snapshot().decoded_bytes, 0);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        owner.check(),
        Err(SourceServiceError::Limit(SourceCounter::DecodedBytes))
    );
}

#[test]
fn scoped_host_inode_and_current_name_mutation_refuse_without_credit_leaks() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(".bazelignore");
    for mutation in 0..4 {
        std::fs::write(&path, b"original").unwrap();
        let owner = scope(SourceServiceLimits::default());
        let mut changed = false;
        let result = host_with(
            &owner,
            &path,
            |point, _| {
                let wanted = if mutation < 2 {
                    ReadPoint::BeforeRead
                } else {
                    ReadPoint::BeforeConversion
                };
                if point == wanted && !changed {
                    changed = true;
                    match mutation {
                        0 => std::fs::OpenOptions::new()
                            .write(true)
                            .open(&path)
                            .unwrap()
                            .set_len(100)
                            .unwrap(),
                        1 => std::fs::OpenOptions::new()
                            .write(true)
                            .open(&path)
                            .unwrap()
                            .set_len(2)
                            .unwrap(),
                        2 => {
                            let replacement = directory.path().join("replacement");
                            std::fs::write(&replacement, b"original").unwrap();
                            std::fs::rename(replacement, &path).unwrap();
                        }
                        3 => std::fs::remove_file(&path).unwrap(),
                        _ => unreachable!(),
                    }
                }
            },
            |file, bytes| file.read(bytes),
        );
        assert!(changed);
        assert!(matches!(
            result,
            Err(RegularFileError::Changed { before_read: false })
        ));
        assert_eq!(owner.snapshot().source_attempts, 1);
        assert!(owner.snapshot().decoded_bytes <= 8);
    }
}

#[test]
fn scoped_host_interrupted_reads_are_bounded_and_scope_failure_wins() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("body");
    std::fs::write(&path, b"a\0b").unwrap();
    let owner = scope(SourceServiceLimits::default());
    let mut reads = 0;
    let value = host_with(
        &owner,
        &path,
        |_, _| {},
        |file, bytes| {
            reads += 1;
            if reads == 1 {
                Err(io::Error::from(io::ErrorKind::Interrupted))
            } else {
                file.read(bytes)
            }
        },
    )
    .unwrap();
    assert_eq!(value.as_ref(), b"a\0b");
    assert_eq!(reads, 3);
    assert_eq!(owner.snapshot().decoded_bytes, 3);
    let owner = scope(SourceServiceLimits::default());
    let mut reads = 0;
    assert!(matches!(
        host_with(
            &owner,
            &path,
            |_, _| {},
            |_, _| {
                reads += 1;
                if reads == 3 {
                    owner.cancel();
                }
                Err(io::Error::from(io::ErrorKind::Interrupted))
            }
        ),
        Err(RegularFileError::Scope(SourceServiceError::Cancelled))
    ));
    assert_eq!(reads, 3);
    assert_eq!(owner.snapshot().decoded_bytes, 0);

    // Every failed Host IO phase postchecks actual scope state before IO projection.
    for stage in [
        RegularFileStage::Pin,
        RegularFileStage::Stat,
        RegularFileStage::Reopen,
        RegularFileStage::Read,
        RegularFileStage::Identity,
    ] {
        let owner = scope(SourceServiceLimits::default());
        let result = checked_io(&owner, ReadPolicy::Host, stage, "test IO", || {
            owner.cancel();
            std::fs::metadata(directory.path().join("missing"))
        });
        assert!(matches!(
            result,
            Err(RegularFileError::Scope(SourceServiceError::Cancelled))
        ));
    }
    for refusal in [
        SourceServiceError::Cancelled,
        SourceServiceError::AcquisitionClosed,
        SourceServiceError::Allocation,
    ] {
        let owner = scope(SourceServiceLimits::default());
        match refusal {
            SourceServiceError::Cancelled => owner.cancel(),
            SourceServiceError::AcquisitionClosed => owner.close_acquisition().unwrap(),
            _ => {
                owner.poison(refusal);
            }
        }
        assert!(
            matches!(read_host_file(&owner, &path), Err(RegularFileError::Scope(error)) if error == refusal)
        );
        assert_eq!(owner.snapshot().source_attempts, 0);
        assert_eq!(owner.snapshot().decoded_bytes, 0);
        assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    }
}

#[test]
fn scoped_host_conversion_overlap_and_cumulative_history_are_bounded() {
    let owner = scope(SourceServiceLimits::default());
    let mut capture = Capture::with_capacity(&owner, ReadPolicy::Host, 2).unwrap();
    assert_eq!(
        capture.append(&owner, b"abc"),
        Err(SourceServiceError::Allocation)
    );
    assert_eq!(capture.bytes.capacity(), 2);
    assert!(capture.bytes.is_empty());
    assert_eq!(owner.snapshot().decoded_bytes, 0);
    drop(capture);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("body");
    std::fs::write(&path, b"abc").unwrap();
    let owner = scope(SourceServiceLimits {
        decoded_bytes: 5,
        ..SourceServiceLimits::default()
    });
    let held = read_host_file(&owner, &path).unwrap();
    assert!(matches!(
        host_with(&owner, &path, |_, _| {}, |file, bytes| file.read(bytes)),
        Err(RegularFileError::Scope(SourceServiceError::Limit(
            SourceCounter::DecodedBytes
        )))
    ));
    assert_eq!(held.as_ref(), b"abc");
    assert_eq!(
        (
            owner.snapshot().source_attempts,
            owner.snapshot().decoded_bytes
        ),
        (2, 3)
    );
    assert_eq!(owner.snapshot().registry_attempts, 0);
    let length = 4096;
    std::fs::write(&path, vec![0x5a; length]).unwrap();
    for policy in [ReadPolicy::Host, ReadPolicy::KnownShas] {
        let ceiling = length * 2 + std::mem::size_of::<Vec<u8>>() + CHUNK_BYTES + 63;
        let owner = scope(SourceServiceLimits {
            live_buffer_bytes: ceiling,
            ..SourceServiceLimits::default()
        });
        assert!(matches!(
            host_with_policy(
                &owner,
                &path,
                policy,
                |_, _| {},
                |file, bytes| file.read(bytes)
            ),
            Err(RegularFileError::Scope(SourceServiceError::BufferLimit))
        ));
        assert_eq!(owner.snapshot().decoded_bytes, length as u64);
        assert_eq!(
            owner.snapshot().peak_buffer_bytes,
            length + std::mem::size_of::<Vec<u8>>() + CHUNK_BYTES
        );
        assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    }
}

#[test]
fn registry_capture_keeps_fixed_capacity_and_refusal_domains() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("registry");
    std::fs::write(&path, b"12345678").unwrap();
    let growth_owner = scope(SourceServiceLimits {
        registry_object_bytes: 8,
        ..SourceServiceLimits::default()
    });
    let mut grew = false;
    let mut requested = 0;
    let result = read_regular_with(
        &growth_owner,
        &path,
        ReadPolicy::Registry,
        |point, _| {
            if point == ReadPoint::BeforeRead && !grew {
                grew = true;
                std::fs::write(&path, b"123456789").unwrap();
            }
        },
        |file, bytes| {
            requested = bytes.len();
            file.read(bytes)
        },
    );
    assert!(matches!(
        result,
        Err(RegularFileError::Scope(SourceServiceError::Limit(
            SourceCounter::RegistryBytes
        )))
    ));
    assert_eq!(requested, CHUNK_BYTES);
    assert_eq!(growth_owner.snapshot().registry_admitted_bytes, 0);
    assert_eq!(growth_owner.snapshot().live_buffer_bytes, 0);

    // Existing registry stat failures remain primary even if the scope became
    // cancelled at that IO phase; Host checks this same failed result first.
    let stat_owner = scope(SourceServiceLimits::default());
    stat_owner.cancel();
    assert!(
        matches!(checked_io(&stat_owner, ReadPolicy::Registry, RegularFileStage::Stat, "scoped registry final stat failed", || std::fs::metadata(directory.path().join("missing"))),
        Err(RegularFileError::Io { stage: RegularFileStage::Stat, error, message }) if error.kind() == io::ErrorKind::NotFound && message == "scoped registry final stat failed")
    );
    assert_eq!(stat_owner.check(), Err(SourceServiceError::Cancelled));

    let owner = scope(SourceServiceLimits {
        registry_object_bytes: 8,
        ..SourceServiceLimits::default()
    });
    let mut capture = Capture::new(&owner, ReadPolicy::Registry).unwrap();
    assert_eq!(capture.bytes.capacity(), 8);
    capture.append(&owner, b"a\0bc").unwrap();
    let value = capture.finish(&owner).unwrap();
    assert_eq!(value.as_ref(), b"a\0bc");
    assert_eq!(owner.snapshot().registry_admitted_bytes, 4);
    assert_eq!(
        (
            owner.snapshot().source_attempts,
            owner.snapshot().decoded_bytes
        ),
        (0, 0)
    );
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let mut capture = Capture::new(&owner, ReadPolicy::Registry).unwrap();
    assert_eq!(
        capture.append(&owner, b"123456789"),
        Err(SourceServiceError::Limit(SourceCounter::RegistryBytes))
    );
    assert_eq!(owner.snapshot().registry_admitted_bytes, 4);
    drop(capture);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}
