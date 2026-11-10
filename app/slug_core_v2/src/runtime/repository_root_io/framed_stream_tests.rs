//! Ordinary FD kernels only; these fixtures grant no native root authority.
use std::cell::Cell;
use std::fs;
use std::os::unix::fs::symlink;

use super::*;

fn limits() -> RootIoLimits {
    RootIoLimits {
        max_path_bytes: 256,
        max_name_bytes: 255,
        max_depth: 32,
        max_entries: 1024,
        max_directory_name_bytes: 64 * 1024,
        max_file_bytes: 256 * 1024,
        max_link_bytes: 256,
    }
}

#[test]
fn contents_and_required_digest_streams_preserve_all_facts_before_zero_sha_gate() {
    let fixture = tempfile::tempdir().unwrap();
    let root = File::open(fixture.path()).unwrap();
    let io = RootIo::new(&root, limits()).unwrap();
    let work = read_work_for_test::begin();
    let payload = (0..2 * 65536 + 13)
        .map(|n| (n % 251) as u8)
        .collect::<Vec<_>>();
    for bytes in [b"".as_slice(), payload.as_slice()] {
        fs::write(fixture.path().join("data"), bytes).unwrap();
        let before = io.lstat(Path::new("data")).unwrap();
        let mut contents = Vec::new();
        let frame = io
            .read_file_contents(Path::new("data"), &|| true, |chunk| {
                contents.extend_from_slice(chunk);
                Ok(())
            })
            .unwrap();
        assert_eq!(contents, bytes);
        assert!(unchanged(&before, &frame));
        assert_eq!(frame.st_size as usize, bytes.len());
        let mut required_bytes = Vec::new();
        let required = io
            .read_file(Path::new("data"), &|| true, |chunk| {
                required_bytes.extend_from_slice(chunk);
                Ok(())
            })
            .unwrap();
        assert_eq!(required_bytes, bytes);
        assert!(unchanged(&before, &required.metadata));
        let expected: [u8; 32] = Sha256::digest(bytes).into();
        assert_eq!(required.digest, expected);
        let digest_only = io.digest_file(Path::new("data"), &|| true).unwrap();
        assert!(unchanged(&before, &digest_only.metadata));
        assert_eq!(digest_only.digest, expected);
    }
    let counts = work.snapshot();
    assert_eq!(counts.contents_payload, payload.len() as u64);
    assert_eq!(counts.digest_payload, 2 * payload.len() as u64);
    assert_eq!(counts.digest_sha, counts.digest_payload);
    // RED has already passed all independent byte/frame/digest assertions.
    assert_eq!(counts.contents_sha, 0);
}

#[test]
fn contents_stream_refuses_name_replacement_truncation_and_growth() {
    for change in ["replace", "truncate", "grow"] {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("data");
        let body = vec![0x6b; 65536 + 13];
        fs::write(&path, &body).unwrap();
        let root = File::open(fixture.path()).unwrap();
        let io = RootIo::new(&root, limits()).unwrap();
        let changed = Cell::new(false);
        let mut observed = 0;
        assert!(
            io.read_file_contents(Path::new("data"), &|| true, |chunk| {
                observed += chunk.len();
                if !changed.replace(true) {
                    match change {
                        "replace" => {
                            fs::rename(&path, fixture.path().join("old"))?;
                            fs::write(&path, &body)?;
                        }
                        "truncate" => fs::OpenOptions::new().write(true).open(&path)?.set_len(1)?,
                        "grow" => fs::OpenOptions::new()
                            .write(true)
                            .open(&path)?
                            .set_len(body.len() as u64 + 1)?,
                        _ => unreachable!(),
                    }
                }
                Ok(())
            })
            .is_err()
        );
        assert!(changed.get());
        assert!(observed > 0);
    }
}

#[test]
fn contents_stream_size_kind_and_link_bounds_precede_payload_callbacks() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("data");
    fs::write(&path, b"four").unwrap();
    let root = File::open(fixture.path()).unwrap();
    let mut bound = limits();
    bound.max_file_bytes = 3;
    let io = RootIo::new(&root, bound).unwrap();
    let calls = Cell::new(0);
    let mut visit = |_: &[u8]| -> io::Result<()> {
        calls.set(calls.get() + 1);
        Ok(())
    };
    assert!(
        io.read_file_contents(Path::new("data"), &|| true, &mut visit)
            .is_err()
    );
    fs::write(&path, b"yes").unwrap();
    fs::hard_link(&path, fixture.path().join("hardlink")).unwrap();
    assert!(
        io.read_file_contents(Path::new("data"), &|| true, &mut visit)
            .is_err()
    );
    fs::remove_file(fixture.path().join("hardlink")).unwrap();
    symlink("data", fixture.path().join("link")).unwrap();
    assert!(
        io.read_file_contents(Path::new("link"), &|| true, &mut visit)
            .is_err()
    );
    assert!(
        io.read_file_contents(Path::new("."), &|| true, &mut visit)
            .is_err()
    );
    assert_eq!(calls.get(), 0);
}

// Reuse the existing test's fixture-specific dev/inode census pattern, rather
// than process-wide FD totals which race unrelated test threads.
fn descriptors(root: &File, file: &File) -> usize {
    let root = linux::stat(root).unwrap();
    let file = linux::stat(file).unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| {
            let fd = entry.ok()?.file_name().to_str()?.parse::<i32>().ok()?;
            let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
            // SAFETY: fstat borrows a numeric descriptor in this process and
            // initializes the output on success; unrelated churn is ignored.
            if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
                return None;
            }
            let stat = unsafe { stat.assume_init() };
            ((stat.st_dev, stat.st_ino) == (root.st_dev, root.st_ino)
                || (stat.st_dev, stat.st_ino) == (file.st_dev, file.st_ino))
                .then_some(())
        })
        .count()
}

#[test]
fn contents_callback_refusal_cancel_and_unwind_close_temporary_descriptors() {
    let fixture = tempfile::tempdir().unwrap();
    let length = 65536 + 7;
    fs::write(fixture.path().join("data"), vec![0x6b; length]).unwrap();
    let root = File::open(fixture.path()).unwrap();
    let file = File::open(fixture.path().join("data")).unwrap();
    let io = RootIo::new(&root, limits()).unwrap();
    let before = descriptors(&root, &file);
    assert!(
        io.read_file_contents(Path::new("data"), &|| true, |_| {
            Err(io::Error::other("test capture refusal"))
        })
        .is_err()
    );
    assert_eq!(descriptors(&root, &file), before);
    assert!(
        io.read_file_contents(Path::new("data"), &|| false, |_| Ok(()))
            .is_err()
    );
    assert_eq!(descriptors(&root, &file), before);
    let checks = Cell::new(0);
    let observed = Cell::new(0);
    let canceled = io.read_file_contents(
        Path::new("data"),
        &|| {
            let n = checks.get();
            checks.set(n + 1);
            n < 2
        },
        |chunk| {
            observed.set(observed.get() + chunk.len());
            Ok(())
        },
    );
    assert!(canceled.is_err());
    assert!(observed.get() > 0 && observed.get() < length);
    assert_eq!(descriptors(&root, &file), before);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _work = read_work_for_test::begin();
        io.read_file_contents(Path::new("data"), &|| true, |_| {
            panic!("test contents callback unwind")
        })
    }));
    let panic = caught.expect_err("callback fault must unwind");
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"test contents callback unwind")
    );
    assert_eq!(descriptors(&root, &file), before);
    let work = read_work_for_test::begin();
    assert_eq!(work.snapshot(), read_work_for_test::Snapshot::default());
    assert!(std::mem::size_of::<Option<Sha256>>() <= 256);
}
