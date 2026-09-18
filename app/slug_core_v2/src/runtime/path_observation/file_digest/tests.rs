use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::*;

fn hex(digest: FileContentDigest) -> String {
    digest
        .sha256()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn standard_vectors_short_interrupted_and_bounded_reads() {
    for (bytes, expected) in [
        (
            &b""[..],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            &b"abc"[..],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
    ] {
        let digest = digest_reader(&mut &*bytes).unwrap();
        assert_eq!(digest.size_bytes(), bytes.len() as u64);
        assert_eq!(hex(digest), expected);
    }
    struct Repeated {
        remaining: usize,
        interrupted: bool,
        calls: usize,
    }
    impl Read for Repeated {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            assert!(buffer.len() <= BUFFER_BYTES);
            self.calls += 1;
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::ErrorKind::Interrupted.into());
            }
            let count = self.remaining.min(buffer.len()).min(4093);
            buffer[..count].fill(b'a');
            self.remaining -= count;
            Ok(count)
        }
    }
    let mut reader = Repeated {
        remaining: 1_000_000,
        interrupted: false,
        calls: 0,
    };
    let digest = digest_reader(&mut reader).unwrap();
    assert_eq!(digest.size_bytes(), 1_000_000);
    assert_eq!(
        hex(digest),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
    assert!(reader.calls > 200);
    assert_eq!(add_size(i64::MAX as u64 - 1, 1).unwrap(), i64::MAX as u64);
    assert_eq!(
        add_size(i64::MAX as u64, 1).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert!(add_size(u64::MAX, 1).is_err());
}

#[test]
fn read_failure_discards_partial_digest() {
    struct Failing(bool);
    impl Read for Failing {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.0 {
                return Err(io::Error::from_raw_os_error(5));
            }
            self.0 = true;
            buffer[..3].copy_from_slice(b"abc");
            Ok(3)
        }
    }
    assert_eq!(
        digest_reader(&mut Failing(false))
            .unwrap_err()
            .raw_os_error(),
        Some(5)
    );
}

#[cfg(unix)]
#[test]
fn native_digest_dispatch_file_link_missing_directory_and_fifo() {
    use std::os::unix::fs::symlink;

    use super::super::observe_native;
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("file");
    let link = temp.path().join("link");
    std::fs::write(&file, b"abc").unwrap();
    symlink("file", &link).unwrap();
    let fifo = temp.path().join("fifo");
    nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRUSR).unwrap();
    for (path, expected) in [
        (
            file,
            PathOperationResult::Present(digest_reader(&mut &b"abc"[..]).unwrap()),
        ),
        (
            link,
            PathOperationResult::Present(digest_reader(&mut &b"abc"[..]).unwrap()),
        ),
        (temp.path().join("missing"), PathOperationResult::Missing),
        (
            temp.path().to_owned(),
            PathOperationResult::Error(PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: PathNodeKind::Directory,
            }),
        ),
        (
            fifo,
            PathOperationResult::Error(PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: PathNodeKind::SpecialFile,
            }),
        ),
    ] {
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            PathObservationOperation::FileDigest,
        );
        let epoch = observe_native(&(), [], [demand.clone()]).unwrap();
        assert_eq!(
            epoch.get(&demand).unwrap().as_ref(),
            &PathObservationResult::FileDigest(expected)
        );
    }
}

#[test]
fn digest_validation_rejects_changed_size_content_and_errors() {
    use slug_workspace_v2::PathIoErrorKind;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathObservationEpoch;

    use crate::runtime::repository_io::validation_epoch_is_dirty;
    let path = NormalizedAbsolutePath::new("/source").unwrap();
    let file = PathObservationDemand::new(
        PathObservationNamespace::Host,
        path.clone(),
        PathObservationOperation::FileDigest,
    );
    let stat = PathObservationDemand::new(
        PathObservationNamespace::Host,
        path,
        PathObservationOperation::Lstat,
    );
    let content = PathObservationResult::FileDigest(PathOperationResult::Present(
        digest_reader(&mut &b"abc"[..]).unwrap(),
    ));
    let first = vec![
        (file.clone(), content.clone()),
        (
            stat.clone(),
            PathObservationResult::Lstat(PathOperationResult::Present(
                slug_workspace_v2::PathLstat::new(PathNodeKind::RegularFile, 3, 1, 2, 3, 0o644),
            )),
        ),
    ];
    let epoch = PathObservationEpoch::new(first.clone()).unwrap();
    assert!(!validation_epoch_is_dirty(&first, &epoch));
    let mut same = first.clone();
    same[1].1 = PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
        PathNodeKind::RegularFile,
        3,
        5,
        6,
        7,
        0o755,
    )));
    assert!(!validation_epoch_is_dirty(
        &first,
        &PathObservationEpoch::new(same.clone()).unwrap()
    ));
    for replacement in [
        PathObservationResult::FileDigest(PathOperationResult::Present(
            digest_reader(&mut &b"abd"[..]).unwrap(),
        )),
        PathObservationResult::FileDigest(PathOperationResult::Missing),
        PathObservationResult::FileDigest(PathOperationResult::Error(PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        })),
    ] {
        same[0].1 = replacement;
        assert!(validation_epoch_is_dirty(
            &first,
            &PathObservationEpoch::new(same.clone()).unwrap()
        ));
    }
    same[0].1 = content;
    for metadata in [
        PathLstat::new(PathNodeKind::RegularFile, 4, 1, 2, 3, 0o644),
        PathLstat::new(PathNodeKind::Directory, 3, 1, 2, 3, 0o644),
    ] {
        same[1].1 = PathObservationResult::Lstat(PathOperationResult::Present(metadata));
        assert!(validation_epoch_is_dirty(
            &first,
            &PathObservationEpoch::new(same.clone()).unwrap()
        ));
    }
    assert!(validation_epoch_is_dirty(
        &first,
        &PathObservationEpoch::empty()
    ));
}
