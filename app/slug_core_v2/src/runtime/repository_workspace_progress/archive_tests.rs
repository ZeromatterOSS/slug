//! Native same-root integration. Tiny real archives exercise ownership/counters;
//! no full-envelope or equivalent-command memory assertion is made here.
use std::io::Write;
use std::os::fd::AsRawFd;

use sha2::Digest;
use slug_bzlmod_v2::GeneratedRepositoryArchiveEffect;
use slug_bzlmod_v2::GeneratedRepositoryArchiveFormat;

use super::tests::apply;
use super::tests::assert_released;
use super::tests::empty_builder;
use super::tests::write;
use super::*;
use crate::runtime::repository_physical_root::native_tests::reservation;
use crate::runtime::repository_physical_root::tests::resources;
use crate::runtime::repository_physical_root::tests::scope as new_scope;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
fn input(
    directory: &Path,
    corrupt: bool,
) -> (tempfile::NamedTempFile, RepositoryWorkspaceOperation) {
    let mut source = tempfile::NamedTempFile::new_in(directory).unwrap();
    let bytes = if corrupt {
        b"not a gzip stream".to_vec()
    } else {
        let mut tar = tar::Builder::new(Vec::new());
        for (path, body, mode) in [
            ("prefix/body", b"archive replacement".as_slice(), 0o755),
            ("prefix/sub/kept", b"new nested file".as_slice(), 0o644),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_path(path).unwrap();
            header.set_mode(mode);
            header.set_size(body.len() as u64);
            header.set_mtime(1_600_000_000);
            header.set_cksum();
            tar.append(&header, body).unwrap();
        }
        let tar = tar.into_inner().unwrap();
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(&tar).unwrap();
        gzip.finish().unwrap()
    };
    source.write_all(&bytes).unwrap();
    source.flush().unwrap();
    let url = url::Url::from_file_path(source.path()).unwrap().to_string();
    let operation = RepositoryWorkspaceOperation::Archive(
        GeneratedRepositoryArchiveEffect::new(
            Arc::from([url.into()]),
            sha2::Sha256::digest(&bytes).into(),
            "prefix".into(),
            GeneratedRepositoryArchiveFormat::TarGz,
            "".into(),
            "".into(),
        )
        .unwrap(),
    );
    (source, operation)
}
fn extract(
    progress: RepositoryWorkspaceProgress,
    operation: RepositoryWorkspaceOperation,
    runtime: &tokio::runtime::Runtime,
    scope: &Arc<SourceServiceScope>,
) -> RepositoryWorkspaceProgress {
    let request = progress.transcript().request(operation).unwrap();
    progress
        .apply_archive(
            request,
            runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            scope,
            &|| true,
        )
        .unwrap()
}

#[test]
#[ignore = "requires trusted Rust tmpfs fixture and real archive acquisition"]
fn native_archive_progress_uses_same_root_and_replays_without_reacquisition() {
    let fixture = tempfile::tempdir().unwrap();
    let display = fixture.path().join("foreign-display");
    std::fs::create_dir(&display).unwrap();
    std::fs::write(display.join("sentinel"), b"foreign").unwrap();
    let selected = reservation(&display, 1501);
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let incoming = bank.reserve(1024 * 1024, 0, 0).unwrap();
    let scope = new_scope();
    let builder = empty_builder(selected, bank.clone(), &scope);
    let fd = builder.file.as_raw_fd();
    let progress = RepositoryWorkspaceProgress::new(builder, &scope, &|| true).unwrap();
    let progress = apply(progress, write("body", b"before"), &scope);
    let (source, archive) = input(fixture.path(), false);
    let request = progress.transcript().request(archive.clone()).unwrap();
    let runtime = runtime();
    let progress = progress
        .apply_archive(
            request.clone(),
            &runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            &scope,
            &|| true,
        )
        .unwrap();
    assert_eq!(
        progress
            .transcript()
            .current()
            .entry("body")
            .unwrap()
            .mode(),
        0o755
    );
    assert!(
        matches!(progress.transcript().current().entry("body").unwrap().node(), RepositoryWorkspaceNode::File(body) if body.as_ref()==b"archive replacement")
    );
    let decoded = progress.decoded;
    let counters = scope.snapshot();
    assert!(decoded > 0);
    // Removing the original input makes any reacquisition observably fail.
    source.close().unwrap();
    let progress = progress
        .apply_archive(
            request,
            &runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            &scope,
            &|| true,
        )
        .unwrap();
    assert_eq!(progress.decoded, decoded);
    assert_eq!(scope.snapshot().source_attempts, counters.source_attempts);
    assert_eq!(scope.snapshot().decoded_bytes, counters.decoded_bytes);
    let progress = apply(
        progress,
        RepositoryWorkspaceOperation::Read("body".into()),
        &scope,
    );
    let progress = apply(
        progress,
        RepositoryWorkspaceOperation::Delete("sub".into()),
        &scope,
    );
    let source = slug_bzlmod_v2::HostRepositoryLabelPathValue::new(
        slug_workspace_v2::NormalizedAbsolutePath::new("/tracked/template").unwrap(),
        PathObservationNamespace::Host,
    )
    .unwrap();
    let progress = apply(
        progress,
        RepositoryWorkspaceOperation::Template {
            path: "final".into(),
            body: Arc::from(b"template".as_slice()),
            mode: 0o644,
            source,
            source_body: Arc::from(b"template".as_slice()),
            substitutions: starlark_map::small_map::SmallMap::new(),
        },
        &scope,
    );
    assert_eq!(progress.decoded, decoded);
    let count = progress.transcript().records().len();
    let sealed = progress.seal(count, &scope, &|| true).unwrap();
    assert!(sealed.state().entry("sub").is_none());
    assert!(sealed.state().entry("final").is_some());
    assert_eq!(std::fs::read(display.join("sentinel")).unwrap(), b"foreign");
    assert!(!display.join("body").exists());
    drop(sealed);
    drop(incoming);
    assert_released(&bank, baseline, fd);
}

#[test]
#[ignore = "requires trusted Rust tmpfs fixture and real archive acquisition"]
fn native_archive_progress_refuses_sha_decode_counter_and_scope_failures() {
    let fixture = tempfile::tempdir().unwrap();
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let incoming = bank.reserve(1024 * 1024, 0, 0).unwrap();
    let runtime = runtime();
    for corrupt in [false, true] {
        let scope = new_scope();
        let selected = reservation(&fixture.path().join("absent"), 1502 + u64::from(corrupt));
        let builder = empty_builder(selected, bank.clone(), &scope);
        let fd = builder.file.as_raw_fd();
        let progress = RepositoryWorkspaceProgress::new(builder, &scope, &|| true).unwrap();
        let (_source, archive) = input(fixture.path(), corrupt);
        let archive = if corrupt {
            archive
        } else {
            let RepositoryWorkspaceOperation::Archive(actual) = archive else {
                unreachable!()
            };
            RepositoryWorkspaceOperation::Archive(
                GeneratedRepositoryArchiveEffect::new(
                    actual.urls().into(),
                    [0; 32],
                    actual.strip_prefix().into(),
                    actual.format(),
                    "".into(),
                    "".into(),
                )
                .unwrap(),
            )
        };
        let request = progress.transcript().request(archive).unwrap();
        assert!(
            progress
                .apply_archive(
                    request,
                    &runtime,
                    &crate::runtime::ArchiveDistdirPolicy::default(),
                    &scope,
                    &|| true
                )
                .is_err()
        );
        assert_eq!(bank.snapshot().native_fds, 0);
        assert_eq!(bank.snapshot().backing_bytes, 0);
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    }
    // Preseed only the aggregate counter to discriminate exact/+1 at the real
    // 1GiB policy without allocating that envelope in this tiny trusted gate.
    let scope = new_scope();
    let selected = reservation(&fixture.path().join("absent-counter"), 1504);
    let builder = empty_builder(selected, bank.clone(), &scope);
    let fd = builder.file.as_raw_fd();
    let progress = RepositoryWorkspaceProgress::new(builder, &scope, &|| true).unwrap();
    let (_source, archive) = input(fixture.path(), false);
    let mut progress = extract(progress, archive.clone(), &runtime, &scope);
    let delta = progress.decoded;
    progress.decoded = 1024 * 1024 * 1024 - delta;
    let progress = extract(progress, archive.clone(), &runtime, &scope);
    assert_eq!(progress.decoded, 1024 * 1024 * 1024);
    let progress = apply(
        progress,
        RepositoryWorkspaceOperation::Delete("sub".into()),
        &scope,
    );
    assert_eq!(progress.decoded, 1024 * 1024 * 1024);
    let request = progress.transcript().request(archive).unwrap();
    assert!(
        progress
            .apply_archive(
                request,
                &runtime,
                &crate::runtime::ArchiveDistdirPolicy::default(),
                &scope,
                &|| true
            )
            .is_err()
    );
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    assert_eq!(bank.snapshot().backing_bytes, 0);
    // A structurally equal fresh scope cannot reset source/decode counters,
    // including through a completed prefix replay or terminal sealing.
    for boundary in [0, 1, 2, 3, 4] {
        let original = new_scope();
        let foreign = new_scope();
        let selected = reservation(&fixture.path().join("absent-scope"), 1510 + boundary);
        let builder = empty_builder(selected, bank.clone(), &original);
        let fd = builder.file.as_raw_fd();
        let progress = RepositoryWorkspaceProgress::new(builder, &original, &|| true).unwrap();
        if boundary >= 3 {
            let (source, operation) = input(fixture.path(), false);
            let request = progress.transcript().request(operation).unwrap();
            let progress = if boundary == 4 {
                let progress = progress
                    .apply_archive(
                        request.clone(),
                        &runtime,
                        &crate::runtime::ArchiveDistdirPolicy::default(),
                        &original,
                        &|| true,
                    )
                    .unwrap();
                source.close().unwrap();
                progress
            } else {
                drop(source);
                progress
            };
            let before = original.snapshot();
            assert!(matches!(
                progress.apply_archive(
                    request,
                    &runtime,
                    &crate::runtime::ArchiveDistdirPolicy::default(),
                    &foreign,
                    &|| true
                ),
                Err(PhysicalRootError::SourceService(
                    SourceServiceError::ForeignScope
                ))
            ));
            assert_eq!(foreign.snapshot().source_attempts, 0);
            assert_eq!(foreign.snapshot().decoded_bytes, 0);
            assert_eq!(original.snapshot().source_attempts, before.source_attempts);
            assert_eq!(original.snapshot().decoded_bytes, before.decoded_bytes);
            assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
            continue;
        }
        let request = progress
            .transcript()
            .request(write("out", b"body"))
            .unwrap();
        if boundary == 0 {
            assert!(matches!(
                progress.apply(request, &foreign, &|| true),
                Err(PhysicalRootError::SourceService(
                    SourceServiceError::ForeignScope
                ))
            ));
        } else {
            let progress = progress
                .apply(request.clone(), &original, &|| true)
                .unwrap();
            if boundary == 1 {
                assert!(matches!(
                    progress.apply(request, &foreign, &|| true),
                    Err(PhysicalRootError::SourceService(
                        SourceServiceError::ForeignScope
                    ))
                ));
            } else {
                assert!(matches!(
                    progress.seal(1, &foreign, &|| true),
                    Err(PhysicalRootError::SourceService(
                        SourceServiceError::ForeignScope
                    ))
                ));
            }
        }
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    }
    drop(incoming);
    assert_eq!(
        (
            bank.snapshot().retained_bytes,
            bank.snapshot().native_fds,
            bank.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
}
