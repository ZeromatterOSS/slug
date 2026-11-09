//! General omission observes bytes; these ordinary rooted kernels admit no mount.
use super::*;

fn unpinned(urls: Arc<[compact_str::CompactString]>, instance: u64) -> RepositoryWorkspaceRequest {
    RepositoryWorkspaceTranscript::new(
        reservation(Path::new("/absent/optional-archive-root"), instance),
        0o755,
    )
    .unwrap()
    .request(RepositoryWorkspaceOperation::Archive(
        GeneratedRepositoryArchiveEffect::new(
            urls,
            None,
            "prefix".into(),
            GeneratedRepositoryArchiveFormat::TarGz,
            "observed-archive".into(),
            "".into(),
        )
        .unwrap(),
    ))
    .unwrap()
}

#[test]
fn omitted_checksum_observes_same_file_url_a_b_a_and_bypasses_distdir() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("source.tar.gz");
    let distdir = fixture.path().join("distdir");
    fs::create_dir(&distdir).unwrap();
    fs::write(
        distdir.join("source.tar.gz"),
        compressed(b"wrong distdir body"),
    )
    .unwrap();
    let hints =
        ArchiveDistdirPolicy::from_request(fixture.path(), &[distdir.to_str().unwrap().to_owned()])
            .unwrap();
    let url = url::Url::from_file_path(&source).unwrap().to_string();
    let bank = resources(16, 2 * CAPTURE_BYTES);
    let baseline = bank.snapshot().retained_bytes;
    let runtime = runtime();
    let mut observed = Vec::new();
    for (index, body) in [b"A".as_slice(), b"B".as_slice(), b"A".as_slice()]
        .into_iter()
        .enumerate()
    {
        let bytes = compressed(body);
        let actual: [u8; 32] = Sha256::digest(&bytes).into();
        fs::write(&source, &bytes).unwrap();
        let request = unpinned(Arc::from([url.clone().into()]), 880 + index as u64);
        let scope = scope();
        let mut capture = VerifiedWorkspaceArchiveCapture::capture(
            &request,
            &bank,
            &runtime,
            &hints,
            &scope,
            &|| true,
        )
        .unwrap();
        assert_eq!(capture.sha256, actual);
        assert_eq!(capture.frame.as_ref().unwrap().len(), bytes.len() as u64);
        assert_eq!(scope.snapshot().source_attempts, 1);
        let temporary = capture.capture.as_ref().unwrap().path().to_owned();
        let fd = capture.capture.as_ref().unwrap().as_file().as_raw_fd();
        // Exact captured bytes survive a later source mutation; a later command
        // still has to reopen the URL and observe that command's current bytes.
        fs::write(&source, b"changed after capture").unwrap();
        let destination = tempfile::tempdir_in(fixture.path()).unwrap();
        let root = File::open(destination.path()).unwrap();
        let io = RootIo::new(
            &root,
            io_limits(RepositoryWorkspaceMode::Archive, request.reservation()),
        )
        .unwrap();
        let mut decoded = 0;
        {
            let _decode = bank.reserve(IO_SCRATCH + DECODE_CONTROL, 4, 0).unwrap();
            assert_eq!(
                capture
                    .extract_at(&io, &mut decoded, &scope, &|| true)
                    .unwrap(),
                RepositoryWorkspaceResult::Archive(actual)
            );
        }
        assert!(decoded > 0);
        assert_eq!(fs::read(destination.path().join("dir/body")).unwrap(), body);
        drop(capture);
        closed(fd);
        assert!(!temporary.exists());
        assert_eq!(
            (
                bank.snapshot().retained_bytes,
                bank.snapshot().native_fds,
                bank.snapshot().backing_bytes
            ),
            (baseline, 0, 0)
        );
        observed.push(actual);
    }
    assert_ne!(observed[0], observed[1]);
    assert_eq!(observed[0], observed[2]);
}

#[test]
fn omitted_expectation_keeps_full_fd_hash_eof_frame_and_cancellation_checks() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("source.tar.gz");
    let bytes = compressed(b"data");
    fs::write(&source, &bytes).unwrap();
    let request = unpinned(
        Arc::from([url::Url::from_file_path(&source)
            .unwrap()
            .to_string()
            .into()]),
        884,
    );
    let bank = resources(16, 2 * CAPTURE_BYTES);
    let baseline = bank.snapshot().retained_bytes;
    let runtime = runtime();
    let live = scope();
    assert!(
        VerifiedWorkspaceArchiveCapture::capture(
            &request,
            &bank,
            &runtime,
            &ArchiveDistdirPolicy::default(),
            &live,
            &|| false,
        )
        .is_err()
    );
    assert_eq!(live.snapshot().source_attempts, 0);
    let mut capture = VerifiedWorkspaceArchiveCapture::capture(
        &request,
        &bank,
        &runtime,
        &ArchiveDistdirPolicy::default(),
        &live,
        &|| true,
    )
    .unwrap();
    let file = capture.capture.as_ref().unwrap().as_file();
    let frame = file.metadata().unwrap();
    let actual: [u8; 32] = Sha256::digest(&bytes).into();
    assert_eq!(
        verify_capture(file, &frame, None, &live, &|| true).unwrap(),
        actual
    );
    assert!(verify_capture(file, &frame, Some([0; 32]), &live, &|| true).is_err());
    let destination = tempfile::tempdir_in(fixture.path()).unwrap();
    let root = File::open(destination.path()).unwrap();
    let io = RootIo::new(
        &root,
        io_limits(RepositoryWorkspaceMode::Archive, request.reservation()),
    )
    .unwrap();
    let mut decoded = 0;
    assert!(
        capture
            .extract_at(&io, &mut decoded, &live, &|| false)
            .is_err()
    );
    assert_eq!(decoded, 0);
    // Neither absence of an expectation nor a refreshed frame can conceal EOF
    // mismatch against the already verified capture frame.
    let file = capture.capture.as_ref().unwrap().as_file();
    file.set_len(1).unwrap();
    assert!(verify_capture(file, &frame, None, &live, &|| true).is_err());
    let fd = file.as_raw_fd();
    assert!(
        capture
            .extract_at(&io, &mut decoded, &live, &|| true)
            .is_err()
    );
    assert_eq!(decoded, 0);
    drop(capture);
    closed(fd);
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
fn omitted_expectation_cannot_select_an_expected_hash_offline_backing() {
    use std::net::TcpListener;

    use crate::runtime::dice::source_service::OfflineArchiveBacking;
    // The existing backing API takes a static original URL. Reserve a bounded
    // choice of static loopback ports, then close the selected listener.
    let (url, listener) = [
        ("https://127.0.0.1:39761/archive.tar.gz", "127.0.0.1:39761"),
        ("https://127.0.0.1:39762/archive.tar.gz", "127.0.0.1:39762"),
        ("https://127.0.0.1:39763/archive.tar.gz", "127.0.0.1:39763"),
    ]
    .into_iter()
    .find_map(|(url, address)| {
        TcpListener::bind(address)
            .ok()
            .map(|listener| (url, listener))
    })
    .expect("one loopback test port available");
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("offline.tar.gz");
    let bytes = compressed(b"offline only with a declared digest");
    fs::write(&source, &bytes).unwrap();
    let actual: [u8; 32] = Sha256::digest(&bytes).into();
    let backing =
        OfflineArchiveBacking::new(url, actual, NormalizedAbsolutePath::new(source).unwrap());
    let fresh = || {
        let deadline = Instant::now() + Duration::from_secs(2);
        SourceServiceScope::new_for_test_with_offline_archive(
            NormalizedAbsolutePath::new(fixture.path()).unwrap(),
            deadline,
            deadline + Duration::from_secs(1),
            SourceServiceLimits::default(),
            None,
            Some(backing.clone()),
            false,
        )
        .unwrap()
    };
    let runtime = runtime();
    let urls = vec![url.to_owned()];
    let pinned = fresh();
    let capture = capture_workspace_generated_archive(
        &urls,
        Some(actual),
        &runtime,
        &|| true,
        &pinned,
        &ArchiveDistdirPolicy::default(),
    )
    .unwrap();
    assert_eq!(fs::read(capture.path()).unwrap(), bytes);
    capture.close().unwrap();
    assert_eq!(pinned.snapshot().source_attempts, 1);
    drop(listener);
    let omitted = fresh();
    assert!(
        capture_workspace_generated_archive(
            &urls,
            None,
            &runtime,
            &|| true,
            &omitted,
            &ArchiveDistdirPolicy::default()
        )
        .is_err()
    );
    assert_eq!(omitted.snapshot().source_attempts, 1);
    assert_eq!(omitted.snapshot().transferred_bytes, 0);
    assert_eq!(omitted.snapshot().live_buffer_bytes, 0);
}

#[test]
fn direct_legacy_archive_request_refuses_omission_before_acquisition() {
    use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
    use slug_bzlmod_v2::RepositoryFilesystemCallId;
    use slug_bzlmod_v2::RepositoryFilesystemOperation;
    use slug_bzlmod_v2::RepositoryFilesystemRequest;
    let owner = crate::runtime::dice::physical_root_owner_for_test();
    let request = Arc::new(RepositoryFilesystemRequest::new(
        RepositoryFilesystemCallId::new(owner, 0, 0),
        "legacy".into(),
        RepositoryFilesystemOperation::Archive(
            GeneratedRepositoryArchiveEffect::new(
                Arc::from(["file:///does-not-exist.tar.gz".into()]),
                None,
                "".into(),
                GeneratedRepositoryArchiveFormat::TarGz,
                "".into(),
                "".into(),
            )
            .unwrap(),
        ),
        GeneratedRepositoryFileEffectPlan::builder().finish(),
    ));
    let mut progress =
        crate::runtime::repository_filesystem_progress::RepositoryFilesystemProgress::default();
    let scope = scope();
    let error = progress
        .complete_call(
            &request,
            &runtime(),
            &|| true,
            Some(&scope),
            &ArchiveDistdirPolicy::default(),
        )
        .unwrap_err();
    assert!(error.message.contains("command-fresh General workspace"));
    assert_eq!(scope.snapshot().source_attempts, 0);
    assert!(progress.provisional_paths().is_empty());
}
