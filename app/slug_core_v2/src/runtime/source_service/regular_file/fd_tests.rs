use std::fs::File;
use std::fs::Metadata;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;
use std::time::Instant;

use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;

fn scope(limits: SourceServiceLimits) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(2);
    SourceServiceScope::new(
        NormalizedAbsolutePath::new("/regular-fd-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        limits,
    )
    .unwrap()
}

fn capture_with(
    owner: &Arc<SourceServiceScope>,
    input: &mut File,
    expected: &Metadata,
    hook: impl FnMut(ReadPoint, &File),
    read: impl FnMut(&mut File, &mut [u8]) -> io::Result<usize>,
) -> Result<Arc<[u8]>, RegularFileError> {
    owner.charge(SourceCounter::SourceAttempts, 1)?;
    fd::read_open_with(
        owner,
        input,
        expected,
        ReadPolicy::Host,
        true,
        hook,
        read,
        || Ok(()),
    )
}

#[test]
fn open_fd_capture_ignores_cursor_and_foreign_name_and_releases_consumed_descriptor() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("body");
    std::fs::write(&path, b"original\0\xff").unwrap();
    let mut input = File::open(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"foreign").unwrap();
    // Capture the expected unlinked inode identity after its link count changed.
    let expected = input.metadata().unwrap();
    let mut shared_cursor = input.try_clone().unwrap();
    input.seek(SeekFrom::End(0)).unwrap();
    let raw = input.as_raw_fd();
    let owner = scope(SourceServiceLimits::default());
    owner.arm_copy_counts_for_test();
    let traffic = owner.copy_counts_for_test();
    let bytes = read_open_regular_file(&owner, input, &expected).unwrap();
    assert_eq!(unsafe { libc::fcntl(raw, libc::F_GETFD) }, -1);
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
    assert_eq!(bytes.as_ref(), b"original\0\xff");
    assert_eq!(shared_cursor.stream_position().unwrap(), expected.len());
    assert_eq!(owner.snapshot().source_attempts, 1);
    assert_eq!(owner.snapshot().decoded_bytes, expected.len());
    assert_eq!(owner.snapshot().registry_admitted_bytes, 0);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    assert_eq!(owner.copy_counts_for_test(), traffic);
    let held = bytes.clone();
    assert!(Arc::ptr_eq(&held, &bytes));
    let weak = Arc::downgrade(&bytes);
    drop(bytes);
    drop(owner);
    assert_eq!(held.as_ref(), b"original\0\xff");
    drop(held);
    assert!(weak.upgrade().is_none());
}

#[test]
fn open_fd_capture_refuses_foreign_expected_identity_before_allocation() {
    let directory = tempfile::tempdir().unwrap();
    let left = directory.path().join("left");
    let right = directory.path().join("right");
    std::fs::write(&left, b"same length").unwrap();
    std::fs::write(&right, b"same length").unwrap();
    let expected = File::open(left).unwrap().metadata().unwrap();
    let input = File::open(right).unwrap();
    let raw = input.as_raw_fd();
    let owner = scope(SourceServiceLimits::default());
    assert!(matches!(
        read_open_regular_file(&owner, input, &expected),
        Err(RegularFileError::Changed { before_read: true })
    ));
    assert_eq!(unsafe { libc::fcntl(raw, libc::F_GETFD) }, -1);
    assert_eq!(owner.snapshot().source_attempts, 1);
    assert_eq!(owner.snapshot().decoded_bytes, 0);
    assert_eq!(owner.snapshot().peak_buffer_bytes, 0);
    assert_eq!(owner.check(), Ok(()));
    let input = File::open(directory.path()).unwrap();
    let expected = input.metadata().unwrap();
    assert!(matches!(
        read_open_regular_file(&owner, input, &expected),
        Err(RegularFileError::NonRegular {
            kind: PathNodeKind::Directory
        })
    ));
}

#[test]
fn open_fd_capture_after_attempt_preserves_single_preopen_admission() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("body");
    std::fs::write(&path, b"body").unwrap();
    let owner = scope(SourceServiceLimits {
        source_attempts: 1,
        ..SourceServiceLimits::default()
    });
    owner.charge(SourceCounter::SourceAttempts, 1).unwrap();
    let input = File::open(path).unwrap();
    let expected = input.metadata().unwrap();
    let bytes = read_open_regular_file_after_attempt(&owner, input, &expected).unwrap();
    assert_eq!(bytes.as_ref(), b"body");
    assert_eq!(owner.snapshot().source_attempts, 1);
    assert_eq!(owner.snapshot().decoded_bytes, 4);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    assert_eq!(owner.check(), Ok(()));
}

#[test]
fn open_fd_capture_object_and_live_credit_exact_plus_one_keep_cumulative_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("body");
    for length in [0, HOST_FILE_OBJECT_BYTES, HOST_FILE_OBJECT_BYTES + 1] {
        std::fs::write(&path, vec![0x5a; length]).unwrap();
        let input = File::open(&path).unwrap();
        let expected = input.metadata().unwrap();
        let owner = scope(SourceServiceLimits::default());
        let result = read_open_regular_file(&owner, input, &expected);
        if length <= HOST_FILE_OBJECT_BYTES {
            let bytes = result.unwrap();
            assert_eq!(bytes.len(), length);
            assert!(bytes.iter().all(|byte| *byte == 0x5a));
            assert_eq!(owner.snapshot().decoded_bytes, length as u64);
            assert_eq!(
                owner.snapshot().peak_buffer_bytes,
                length + CHUNK_BYTES + 64
            );
        } else {
            assert!(matches!(
                result,
                Err(RegularFileError::Scope(SourceServiceError::Limit(
                    SourceCounter::DecodedBytes
                )))
            ));
            assert_eq!(owner.snapshot().decoded_bytes, 0);
            assert_eq!(owner.snapshot().peak_buffer_bytes, 0);
        }
        assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    }
    std::fs::write(&path, b"abc").unwrap();
    for shortage in [0, 1] {
        let owner = scope(SourceServiceLimits {
            live_buffer_bytes: 3 + CHUNK_BYTES + 64 - shortage,
            ..SourceServiceLimits::default()
        });
        let input = File::open(&path).unwrap();
        let expected = input.metadata().unwrap();
        let result = read_open_regular_file(&owner, input, &expected);
        if shortage == 0 {
            assert_eq!(result.unwrap().as_ref(), b"abc");
        } else {
            assert!(matches!(
                result,
                Err(RegularFileError::Scope(SourceServiceError::BufferLimit))
            ));
            assert_eq!(owner.snapshot().decoded_bytes, 0);
            assert_eq!(owner.snapshot().peak_buffer_bytes, 3 + 64);
        }
        assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    }
    let owner = scope(SourceServiceLimits {
        decoded_bytes: 5,
        ..SourceServiceLimits::default()
    });
    let read = || {
        let input = File::open(&path).unwrap();
        let expected = input.metadata().unwrap();
        read_open_regular_file(&owner, input, &expected)
    };
    let held = read().unwrap();
    assert!(matches!(
        read(),
        Err(RegularFileError::Scope(SourceServiceError::Limit(
            SourceCounter::DecodedBytes
        )))
    ));
    assert_eq!(held.as_ref(), b"abc");
    assert_eq!(owner.snapshot().source_attempts, 2);
    assert_eq!(owner.snapshot().decoded_bytes, 3);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}

#[test]
fn open_fd_capture_detects_inode_mutation_and_short_read_without_publication() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("body");
    for mutation in 0..3 {
        std::fs::write(&path, b"body").unwrap();
        let mut input = File::open(&path).unwrap();
        let expected = input.metadata().unwrap();
        let writer = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        let owner = scope(SourceServiceLimits::default());
        let mut changed = false;
        let result = capture_with(
            &owner,
            &mut input,
            &expected,
            |point, _| {
                if point == ReadPoint::BeforeRead && !changed {
                    changed = true;
                    if mutation < 2 {
                        writer.set_len(if mutation == 0 { 2 } else { 8 }).unwrap();
                    } else {
                        writer
                            .set_permissions(std::fs::Permissions::from_mode(
                                expected.permissions().mode() ^ 0o100,
                            ))
                            .unwrap();
                    }
                }
            },
            |file, bytes| file.read(bytes),
        );
        assert!(matches!(
            result,
            Err(RegularFileError::Changed { before_read: false })
        ));
        assert_eq!(owner.snapshot().live_buffer_bytes, 0);
        assert!(owner.snapshot().decoded_bytes <= 4);
    }
    std::fs::write(&path, b"body").unwrap();
    let mut input = File::open(&path).unwrap();
    let expected = input.metadata().unwrap();
    let owner = scope(SourceServiceLimits::default());
    let mut reads = 0;
    let result = capture_with(
        &owner,
        &mut input,
        &expected,
        |_, _| {},
        |file, bytes| {
            reads += 1;
            if reads == 1 {
                file.read(&mut bytes[..2])
            } else {
                Ok(0)
            }
        },
    );
    assert!(matches!(
        result,
        Err(RegularFileError::Changed { before_read: false })
    ));
    assert_eq!(owner.snapshot().decoded_bytes, 2);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}

#[test]
fn open_fd_capture_retries_interruption_and_cancelled_reads_release_all_credit() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("body");
    std::fs::write(&path, b"body").unwrap();
    for cancel in [false, true] {
        let mut input = File::open(&path).unwrap();
        let expected = input.metadata().unwrap();
        let owner = scope(SourceServiceLimits::default());
        let mut reads = 0;
        let result = capture_with(
            &owner,
            &mut input,
            &expected,
            |_, _| {},
            |file, bytes| {
                reads += 1;
                if reads == 1 {
                    if cancel {
                        owner.cancel();
                    }
                    Err(io::Error::from(io::ErrorKind::Interrupted))
                } else {
                    file.read(bytes)
                }
            },
        );
        if cancel {
            assert!(matches!(
                result,
                Err(RegularFileError::Scope(SourceServiceError::Cancelled))
            ));
            assert_eq!(reads, 1);
            assert_eq!(owner.snapshot().decoded_bytes, 0);
        } else {
            assert_eq!(result.unwrap().as_ref(), b"body");
            assert_eq!(reads, 3);
            assert_eq!(owner.snapshot().decoded_bytes, 4);
        }
        assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    }
    let owner = scope(SourceServiceLimits::default());
    owner.cancel();
    let input = File::open(&path).unwrap();
    let expected = input.metadata().unwrap();
    let raw = input.as_raw_fd();
    assert!(matches!(
        read_open_regular_file(&owner, input, &expected),
        Err(RegularFileError::Scope(SourceServiceError::Cancelled))
    ));
    assert_eq!(unsafe { libc::fcntl(raw, libc::F_GETFD) }, -1);
    assert_eq!(owner.snapshot().source_attempts, 0);
    assert_eq!(owner.snapshot().peak_buffer_bytes, 0);
}
