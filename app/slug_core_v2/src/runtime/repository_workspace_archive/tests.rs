//! Actual file-URL acquisition and private rooted decoder controls. Ordinary
//! directory destinations do not pass physical-root admission or activate one.
use std::cell::Cell;
use std::fs;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use flate2::Compression;
use flate2::write::GzEncoder;
use slug_bzlmod_v2::GeneratedRepositoryArchiveFormat;
use slug_bzlmod_v2::RepositoryWorkspaceTranscript;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::super::native_tests::reservation;
use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;

fn scope() -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(5);
    SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/archive-capture-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap()
}
fn resources(fds: usize, backing: u64) -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(8 * 1024 * 1024, fds, backing).unwrap(),
    )
    .unwrap()
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
fn compressed(body: &[u8]) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_path("prefix/dir/body").unwrap();
    header.set_size(body.len() as u64);
    header.set_mode(0o755);
    header.set_mtime(1_600_000_000);
    header.set_cksum();
    tar.append(&header, body).unwrap();
    let tar = tar.into_inner().unwrap();
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&tar).unwrap();
    gz.finish().unwrap()
}
fn request(path: &Path, expected: [u8; 32]) -> RepositoryWorkspaceRequest {
    let initial = RepositoryWorkspaceTranscript::new(
        reservation(Path::new("/absent/archive-root"), 801),
        0o755,
    )
    .unwrap();
    initial
        .request(RepositoryWorkspaceOperation::Archive(
            GeneratedRepositoryArchiveEffect::new(
                Arc::from([url::Url::from_file_path(path).unwrap().to_string().into()]),
                expected,
                "prefix".into(),
                GeneratedRepositoryArchiveFormat::TarGz,
                "actual-archive-capture".into(),
                "".into(),
            )
            .unwrap(),
        ))
        .unwrap()
}
fn closed(fd: i32) {
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
}

#[test]
fn archive_capture_moves_actual_verified_file_into_rooted_extract_without_host_alias() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("source.tar.gz");
    let compressed = compressed(b"actual\0\xff");
    let sha: [u8; 32] = Sha256::digest(&compressed).into();
    fs::write(&source, &compressed).unwrap();
    let request = request(&source, sha);
    let bank = resources(16, 2 * CAPTURE_BYTES);
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    let runtime = runtime();
    let mut capture = VerifiedWorkspaceArchiveCapture::capture(
        &request,
        &bank,
        &runtime,
        &ArchiveDistdirPolicy::default(),
        &scope,
        &|| true,
    )
    .unwrap();
    let temp = capture.capture.as_ref().unwrap().path().to_owned();
    let capture_fd = capture.capture.as_ref().unwrap().as_file().as_raw_fd();
    assert_eq!(capture.sha256, sha);
    assert_eq!(
        capture.frame.as_ref().unwrap().len(),
        compressed.len() as u64
    );
    assert_eq!(bank.snapshot().native_fds, 8);
    assert_eq!(bank.snapshot().backing_bytes, CAPTURE_BYTES);
    // The original source may change after capture. Extraction retains the
    // verified actual capture; it never returns to that source pathname.
    fs::write(&source, b"changed after actual capture").unwrap();
    let display = fixture.path().join("display");
    let backing = fixture.path().join("backing");
    fs::create_dir(&display).unwrap();
    let fd = File::open(&display).unwrap();
    fs::rename(&display, &backing).unwrap();
    fs::create_dir(&display).unwrap();
    fs::write(display.join("untouched"), b"foreign").unwrap();
    let root = RootIo::new(
        &fd,
        io_limits(RepositoryWorkspaceMode::Archive, request.reservation()),
    )
    .unwrap();
    let mut decoded = 0;
    {
        let _decode = bank.reserve(IO_SCRATCH + DECODE_CONTROL, 4, 0).unwrap();
        assert_eq!(
            capture
                .extract_at(&root, &mut decoded, &scope, &|| true)
                .unwrap(),
            RepositoryWorkspaceResult::Archive(sha)
        );
    }
    assert!(decoded > 0);
    assert_eq!(fs::read(backing.join("dir/body")).unwrap(), b"actual\0\xff");
    assert_eq!(
        fs::metadata(backing.join("dir/body"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert_eq!(
        fs::metadata(backing.join("dir/body")).unwrap().mtime(),
        1_600_000_000
    );
    assert_eq!(fs::read(display.join("untouched")).unwrap(), b"foreign");
    capture.close_capture().unwrap();
    drop(capture);
    closed(capture_fd);
    assert!(!temp.exists());
    assert_eq!(
        (
            bank.snapshot().retained_bytes,
            bank.snapshot().native_fds,
            bank.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    assert_eq!(scope.snapshot().live_decoder_bytes, 0);
}

#[test]
fn archive_capture_wrong_sha_and_non_archive_never_mint_a_verified_owner() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("source.tar.gz");
    fs::write(&source, compressed(b"body")).unwrap();
    let wrong = request(&source, [0; 32]);
    let bank = resources(16, CAPTURE_BYTES);
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    let runtime = runtime();
    assert!(
        VerifiedWorkspaceArchiveCapture::capture(
            &wrong,
            &bank,
            &runtime,
            &ArchiveDistdirPolicy::default(),
            &scope,
            &|| true
        )
        .is_err()
    );
    assert_eq!(scope.snapshot().source_attempts, 1);
    assert_eq!(
        (
            bank.snapshot().retained_bytes,
            bank.snapshot().native_fds,
            bank.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
    let initial = RepositoryWorkspaceTranscript::new(wrong.reservation().clone(), 0o755).unwrap();
    let ordinary = initial
        .request(RepositoryWorkspaceOperation::Exists("".into()))
        .unwrap();
    let fresh = super::tests::scope();
    assert!(matches!(
        VerifiedWorkspaceArchiveCapture::capture(
            &ordinary,
            &bank,
            &runtime,
            &ArchiveDistdirPolicy::default(),
            &fresh,
            &|| true
        ),
        Err(PhysicalRootError::UnsupportedOperation)
    ));
    assert_eq!(fresh.snapshot().source_attempts, 0);
    assert_eq!(bank.snapshot().native_fds, 0);
}

#[test]
fn archive_capture_fd_and_backing_refusals_precede_source_io() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("source.tar.gz");
    let bytes = compressed(b"body");
    fs::write(&source, &bytes).unwrap();
    let request = request(&source, Sha256::digest(&bytes).into());
    let runtime = runtime();
    for bank in [resources(7, CAPTURE_BYTES), resources(8, CAPTURE_BYTES - 1)] {
        let baseline = bank.snapshot().retained_bytes;
        let scope = scope();
        assert!(matches!(
            VerifiedWorkspaceArchiveCapture::capture(
                &request,
                &bank,
                &runtime,
                &ArchiveDistdirPolicy::default(),
                &scope,
                &|| true
            ),
            Err(PhysicalRootError::Resource)
        ));
        assert_eq!(scope.snapshot().source_attempts, 0);
        assert_eq!(scope.snapshot().transferred_bytes, 0);
        assert_eq!(
            (
                bank.snapshot().retained_bytes,
                bank.snapshot().native_fds,
                bank.snapshot().backing_bytes
            ),
            (baseline, 0, 0)
        );
    }
}

#[test]
fn archive_capture_verifier_rejects_truncation_growth_wrong_bytes_and_hardlinks() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("capture");
    for changed in [b"short".as_slice(), b"original plus one", b"ORIGINAL"] {
        fs::write(&path, b"original").unwrap();
        let file = File::open(&path).unwrap();
        let frame = file.metadata().unwrap();
        fs::write(&path, changed).unwrap();
        assert!(
            verify_capture(
                &file,
                &frame,
                Sha256::digest(b"original").into(),
                &scope(),
                &|| true
            )
            .is_err()
        );
    }
    fs::write(&path, b"original").unwrap();
    fs::hard_link(&path, fixture.path().join("alias")).unwrap();
    let file = File::open(&path).unwrap();
    let frame = file.metadata().unwrap();
    assert!(
        verify_capture(
            &file,
            &frame,
            Sha256::digest(b"original").into(),
            &scope(),
            &|| true
        )
        .is_err()
    );
    let directory = File::open(fixture.path()).unwrap();
    assert!(
        verify_capture(
            &directory,
            &directory.metadata().unwrap(),
            [0; 32],
            &scope(),
            &|| true
        )
        .is_err()
    );
}

#[test]
fn archive_capture_verifier_accepts_exact_bytes_above_filebytes_cap_and_refuses_metadata_oversize()
{
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("capture");
    let bytes = vec![0xab; 1024 * 1024 + 1];
    fs::write(&path, &bytes).unwrap();
    let file = File::open(&path).unwrap();
    let frame = file.metadata().unwrap();
    let expected = Sha256::digest(&bytes).into();
    assert_eq!(
        verify_capture(&file, &frame, expected, &scope(), &|| true).unwrap(),
        expected
    );
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(CAPTURE_BYTES + 1)
        .unwrap();
    let large = file.metadata().unwrap();
    let calls = Cell::new(0);
    let active = || {
        calls.set(calls.get() + 1);
        true
    };
    assert!(verify_capture(&file, &large, [0; 32], &scope(), &active).is_err());
    assert_eq!(calls.get(), 1);
}

#[test]
fn archive_capture_mutation_after_verification_and_foreign_scope_refuse_before_root_writes() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("source.tar.gz");
    let bytes = compressed(b"body");
    fs::write(&source, &bytes).unwrap();
    let request = request(&source, Sha256::digest(&bytes).into());
    let bank = resources(16, CAPTURE_BYTES);
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    let runtime = runtime();
    let mut capture = VerifiedWorkspaceArchiveCapture::capture(
        &request,
        &bank,
        &runtime,
        &ArchiveDistdirPolicy::default(),
        &scope,
        &|| true,
    )
    .unwrap();
    let destination = fixture.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let fd = File::open(&destination).unwrap();
    let root = RootIo::new(
        &fd,
        io_limits(RepositoryWorkspaceMode::Archive, request.reservation()),
    )
    .unwrap();
    assert!(matches!(
        capture.extract_at(&root, &mut 0, &super::tests::scope(), &|| true),
        Err(PhysicalRootError::SourceService(
            SourceServiceError::ForeignScope
        ))
    ));
    capture
        .capture
        .as_mut()
        .unwrap()
        .as_file_mut()
        .write_all(b"mutation")
        .unwrap();
    assert!(capture.extract_at(&root, &mut 0, &scope, &|| true).is_err());
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    drop(capture);
    assert_eq!(
        (
            bank.snapshot().retained_bytes,
            bank.snapshot().native_fds,
            bank.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
}

#[test]
fn archive_capture_cancellation_at_acquisition_verification_and_decode_retires_owners() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("source.tar.gz");
    let bytes = compressed(b"body");
    fs::write(&source, &bytes).unwrap();
    let request = request(&source, Sha256::digest(&bytes).into());
    let bank = resources(16, CAPTURE_BYTES);
    let baseline = bank.snapshot().retained_bytes;
    let runtime = runtime();
    let canceled = scope();
    canceled.cancel();
    assert!(
        VerifiedWorkspaceArchiveCapture::capture(
            &request,
            &bank,
            &runtime,
            &ArchiveDistdirPolicy::default(),
            &canceled,
            &|| true
        )
        .is_err()
    );
    let scope = scope();
    assert!(
        VerifiedWorkspaceArchiveCapture::capture(
            &request,
            &bank,
            &runtime,
            &ArchiveDistdirPolicy::default(),
            &scope,
            &|| false
        )
        .is_err()
    );
    let mut capture = VerifiedWorkspaceArchiveCapture::capture(
        &request,
        &bank,
        &runtime,
        &ArchiveDistdirPolicy::default(),
        &scope,
        &|| true,
    )
    .unwrap();
    let temp = capture.capture.as_ref().unwrap().path().to_owned();
    let capture_fd = capture.capture.as_ref().unwrap().as_file().as_raw_fd();
    let fd = File::open(fixture.path()).unwrap();
    let root = RootIo::new(
        &fd,
        io_limits(RepositoryWorkspaceMode::Archive, request.reservation()),
    )
    .unwrap();
    scope.cancel();
    assert!(capture.extract_at(&root, &mut 0, &scope, &|| true).is_err());
    drop(capture);
    closed(capture_fd);
    assert!(!temp.exists());
    assert_eq!(
        (
            bank.snapshot().retained_bytes,
            bank.snapshot().native_fds,
            bank.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
    let file = File::open(&source).unwrap();
    let frame = file.metadata().unwrap();
    let live = super::tests::scope();
    let calls = Cell::new(0);
    let active = || {
        let n = calls.get();
        calls.set(n + 1);
        if n == 1 {
            live.cancel();
        }
        true
    };
    assert!(verify_capture(&file, &frame, Sha256::digest(&bytes).into(), &live, &active).is_err());
}

#[test]
fn archive_capture_failed_unlink_releases_descriptors_and_control_but_keeps_storage_allowance() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("source.tar.gz");
    let bytes = compressed(b"body");
    fs::write(&source, &bytes).unwrap();
    let request = request(&source, Sha256::digest(&bytes).into());
    let bank = resources(16, CAPTURE_BYTES);
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    let runtime = runtime();
    let mut capture = VerifiedWorkspaceArchiveCapture::capture(
        &request,
        &bank,
        &runtime,
        &ArchiveDistdirPolicy::default(),
        &scope,
        &|| true,
    )
    .unwrap();
    let temp = capture.capture.as_ref().unwrap().path().to_owned();
    let capture_fd = capture.capture.as_ref().unwrap().as_file().as_raw_fd();
    fs::remove_file(&temp).unwrap();
    fs::create_dir(&temp).unwrap();
    assert!(capture.close_capture().is_err());
    drop(capture);
    closed(capture_fd);
    assert_eq!(
        (
            bank.snapshot().retained_bytes,
            bank.snapshot().native_fds,
            bank.snapshot().backing_bytes
        ),
        (baseline, 0, CAPTURE_BYTES)
    );
    // The contract has no orphan retry owner. This test removes its deliberate
    // path obstruction, but the conservatively forgotten allowance stays paid.
    fs::remove_dir(temp).unwrap();
}
