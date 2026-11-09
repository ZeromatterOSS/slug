//! Borrowed RootIo kernel evidence only; actual native admission/seal coverage
//! belongs to the command's scaled ignored archive fixture.
use super::*;

#[test]
fn covered_new_path_reuses_pending_bytes_but_mismatch_and_sticky_scope_stay_closed() {
    for case in 0..4 {
        let fixture = tempfile::tempdir().unwrap();
        prepare(fixture.path());
        let length = 2 * 1024 * 1024;
        let old: Arc<[u8]> = vec![b'a'; length].into();
        fs::write(
            fixture.path().join("new"),
            if case == 1 {
                vec![b'b'; length]
            } else {
                old.to_vec()
            },
        )
        .unwrap();
        let selected = reservation(Path::new("/absent/cross-path"), 231);
        let scope = scope();
        let bank = RepositoryRootResources::new(
            RepositoryRootResourceLimits::new(3 * 1024 * 1024, 16, 4 * ARCHIVE_BYTES).unwrap(),
        )
        .unwrap();
        let baseline = bank.snapshot().retained_bytes;
        let ledger = NativeBodyLedger::new(&bank, &scope, &selected, &|| true).unwrap();
        let mut candidates = ledger.candidates().unwrap();
        candidates.retain(&old, &|| true).unwrap();
        let initial = RepositoryWorkspaceTranscript::new(selected.clone(), 0o755).unwrap();
        let file = File::open(fixture.path()).unwrap();
        let io = RootIo::new(
            &file,
            io_limits(RepositoryWorkspaceMode::Archive, &selected),
        )
        .unwrap();
        let framed = io.lstat(Path::new("new")).unwrap();
        let mut scratch = bank.reserve(IO_SCRATCH, 3, 0).unwrap();
        if case == 2 {
            scope.cancel();
        } else if case == 3 {
            assert!(
                scope
                    .reserve_buffer(scope.limits().live_buffer_bytes + 1)
                    .is_err()
            );
        }
        let before = bank.snapshot().retained_bytes;
        let result = capture_body_covered(
            &io,
            Path::new("new"),
            &framed,
            initial.current(),
            &mut scratch,
            Some(&mut candidates),
            &scope,
            &|| true,
        );
        match case {
            0 => {
                let actual = result.unwrap();
                assert!(Arc::ptr_eq(&actual, &old));
                assert!(candidates.lookup(&actual, &|| true).unwrap().is_some());
            }
            1 => assert!(matches!(result, Err(PhysicalRootError::Resource))),
            2 => assert!(matches!(
                result,
                Err(PhysicalRootError::SourceService(
                    SourceServiceError::Cancelled
                ))
            )),
            3 => assert!(matches!(
                result,
                Err(PhysicalRootError::SourceService(
                    SourceServiceError::BufferLimit
                ))
            )),
            _ => unreachable!(),
        }
        assert_eq!(bank.snapshot().retained_bytes, before);
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        drop((candidates, ledger, scratch, old));
        assert_eq!(bank.snapshot().retained_bytes, baseline);
        assert_eq!(bank.snapshot().native_fds, 0);
        assert_eq!(bank.snapshot().backing_bytes, 0);
    }
}

#[test]
fn observed_recovery_bytes_require_current_fd_name_and_snapshot_frames() {
    for case in 0..3 {
        let fixture = tempfile::tempdir().unwrap();
        prepare(fixture.path());
        let path = fixture.path().join("new");
        fs::write(&path, b"aaa").unwrap();
        let selected = reservation(Path::new("/absent/recovery-frame"), 232);
        let scope = scope();
        let bank = resources();
        let baseline = bank.snapshot().retained_bytes;
        let scratch = bank.reserve(IO_SCRATCH, 3, 0).unwrap();
        let file = File::open(fixture.path()).unwrap();
        let io = RootIo::new(
            &file,
            io_limits(RepositoryWorkspaceMode::Archive, &selected),
        )
        .unwrap();
        let framed = io.lstat(Path::new("new")).unwrap();
        let calls = Cell::new(0);
        let changed = Cell::new(false);
        let active = || {
            let call = calls.get() + 1;
            calls.set(call);
            // For this three-byte regular file RootIo checks active at entry,
            // before its only read, then after payload BEFORE EOF/frame checks.
            if call == 3 {
                changed.set(true);
                match case {
                    0 => fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap(),
                    1 => {
                        fs::rename(&path, fixture.path().join("old-name")).unwrap();
                        fs::write(&path, b"aaa").unwrap();
                    }
                    2 => scope.cancel(),
                    _ => unreachable!(),
                }
            }
            true
        };
        let old: Arc<[u8]> = Arc::from(&b"aaa"[..]);
        let result =
            body_recovery::observed_equal(&io, Path::new("new"), &framed, &old, &scope, &active);
        assert!(changed.get(), "post-payload mutation must execute");
        if case == 2 {
            assert!(matches!(
                result,
                Err(PhysicalRootError::SourceService(
                    SourceServiceError::Cancelled
                ))
            ));
        } else {
            assert!(matches!(result, Err(PhysicalRootError::Io(_))));
        }
        drop(scratch);
        assert_eq!(bank.snapshot().retained_bytes, baseline);
        assert_eq!(bank.snapshot().native_fds, 0);
    }
}

#[test]
fn observed_equal_body_still_rejects_changed_snapshot_frame() {
    let fixture = tempfile::tempdir().unwrap();
    prepare(fixture.path());
    let path = fixture.path().join("new");
    fs::write(&path, b"aaa").unwrap();
    let selected = reservation(Path::new("/absent/recovery-stale"), 233);
    let scope = scope();
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let scratch = bank.reserve(IO_SCRATCH, 3, 0).unwrap();
    let file = File::open(fixture.path()).unwrap();
    let io = RootIo::new(
        &file,
        io_limits(RepositoryWorkspaceMode::Archive, &selected),
    )
    .unwrap();
    let framed = io.lstat(Path::new("new")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let old: Arc<[u8]> = Arc::from(&b"aaa"[..]);
    assert!(matches!(
        body_recovery::observed_equal(&io, Path::new("new"), &framed, &old, &scope, &|| true),
        Err(PhysicalRootError::Snapshot)
    ));
    drop(scratch);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}
