//! Actual borrowed RootIo sampling, not native-root admission or archive execution.
use super::*;
use crate::runtime::repository_root_io::read_work_for_test;

#[test]
fn fresh_raw_and_packed_contents_streams_preserve_identity_frames_and_retirement() {
    let payload = vec![b'K'; 2 * 64 * 1024 + 37];
    let mut observations = Vec::new();
    for covered in [false, true] {
        let fixture = tempfile::tempdir().unwrap();
        prepare(fixture.path());
        let path = fixture.path().join("file");
        fs::write(&path, &payload).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let selected = reservation(Path::new("/absent/framed-snapshot"), 239);
        let scope = scope();
        let bank = resources();
        let baseline = bank.snapshot().retained_bytes;
        let initial = RepositoryWorkspaceTranscript::new(selected.clone(), 0o755).unwrap();
        let ledger =
            covered.then(|| NativeBodyLedger::new(&bank, &scope, &selected, &|| true).unwrap());
        let mut candidates = ledger.as_ref().map(|ledger| ledger.candidates().unwrap());
        let descriptor = File::open(fixture.path()).unwrap();
        let io = RootIo::new(
            &descriptor,
            io_limits(RepositoryWorkspaceMode::Archive, &selected),
        )
        .unwrap();
        let scratch = bank
            .reserve(
                IO_SCRATCH + 2 * RepositoryWorkspaceBody::decode_scratch_bytes(),
                3,
                0,
            )
            .unwrap();
        let witness = read_work_for_test::begin();
        let captured = capture_at_with_sources(
            &io,
            &selected,
            RepositoryWorkspaceMode::Archive,
            initial.current(),
            &bank,
            &scope,
            &|| true,
            Limits::for_mode(RepositoryWorkspaceMode::Archive),
            None,
            candidates.as_mut(),
        )
        .unwrap();
        observations.push(witness.snapshot());
        drop(witness);
        let actual = body(captured.entries(), "file").clone();
        assert_eq!(actual.len(), payload.len());
        assert!(actual.equals_slice(&payload));
        assert_eq!(actual.raw_arc().is_none(), covered);
        if covered {
            assert!(actual.native_storage_bytes() < actual.len());
        }
        let weak = RepositoryWorkspaceBody::downgrade(&actual);
        let captured = match candidates.take() {
            Some(candidates) => captured.with_body_candidates(candidates),
            None => captured,
        };
        let completed = captured
            .complete_active(
                &initial,
                initial.request(archive()).unwrap(),
                RepositoryWorkspaceResult::Archive([0; 32]),
                None,
                &|| true,
            )
            .unwrap();
        let raw_reference = prior(
            selected.clone(),
            vec![root_entry(), file_entry("file", &payload)],
        );
        // Retention records physical owner control as well as logical bytes.
        // Build the expected representation independently from authored bytes;
        // never borrow the captured Body to manufacture its own oracle.
        let expected_body = if covered {
            let mut builder = RepositoryWorkspaceBodyBuilder::new(payload.len()).unwrap();
            for chunk in payload.chunks(64 * 1024) {
                builder.append(chunk, &mut |_| Ok::<(), ()>(())).unwrap();
            }
            builder.finish().unwrap()
        } else {
            Arc::<[u8]>::from(payload.as_slice()).into()
        };
        let reference = initial
            .complete(
                initial.request(archive()).unwrap(),
                RepositoryWorkspaceResult::Archive([0; 32]),
                RepositoryWorkspaceMode::Archive,
                vec![
                    root_entry(),
                    RepositoryWorkspaceEntry::new(
                        "file".into(),
                        0o644,
                        RepositoryWorkspaceNode::File(expected_body.clone()),
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
        let expected_metadata = raw_reference
            .retention()
            .metadata_bytes()
            .checked_add(expected_body.control_storage_bytes())
            .unwrap()
            .checked_sub(body(raw_reference.current().entries(), "file").control_storage_bytes())
            .unwrap();
        let retention = reference.retention();
        assert_eq!(retention.state_versions(), 2);
        assert_eq!(retention.executes(), 0);
        assert_eq!(retention.body_bytes(), payload.len() as u64);
        assert_eq!(retention.metadata_bytes(), expected_metadata);
        assert_eq!(completed.transcript(), &reference);
        assert_eq!(
            reference.realize(1).unwrap().source_identity(),
            raw_reference.realize(1).unwrap().source_identity()
        );
        assert_eq!(
            completed.transcript().realize(1).unwrap().source_identity(),
            reference.realize(1).unwrap().source_identity()
        );
        assert!(RepositoryWorkspaceBody::ptr_eq(
            body(completed.transcript().current().entries(), "file"),
            &actual
        ));
        if let Some(ledger) = &ledger {
            assert!(RepositoryWorkspaceBody::ptr_eq(
                ledger.lookup(&actual, &|| true).unwrap().unwrap().body(),
                &actual
            ));
        }
        drop((actual, completed));
        assert_eq!(weak.upgrade().is_some(), covered);

        // A stale pre-capture frame cannot publish a shortened file, including
        // the covered builder's partial block; every temporary debit retires.
        let framed = io.lstat(Path::new("file")).unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len((payload.len() - 1) as u64)
            .unwrap();
        let before_failure = bank.snapshot().retained_bytes;
        let mut failed_candidates = ledger.as_ref().map(|ledger| ledger.candidates().unwrap());
        let mut credit = bank.reserve(0, 0, 0).unwrap();
        assert!(matches!(
            capture_body_covered(
                &io,
                Path::new("file"),
                &framed,
                initial.current(),
                &mut credit,
                failed_candidates.as_mut(),
                &scope,
                &|| true,
            ),
            Err(PhysicalRootError::Snapshot)
        ));
        drop((failed_candidates, credit));
        assert_eq!(bank.snapshot().retained_bytes, before_failure);
        assert!(
            sample(
                &io,
                &selected,
                RepositoryWorkspaceMode::Archive,
                initial.current(),
                &bank,
                &scope,
                &|| false
            )
            .is_err()
        );
        drop(ledger);
        assert!(weak.upgrade().is_none());
        drop(scratch);
        drop(io);
        drop(descriptor);
        let retired = bank.snapshot();
        assert_eq!(
            (
                retired.retained_bytes,
                retired.native_fds,
                retired.backing_bytes
            ),
            (baseline, 0, 0)
        );
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        assert_eq!(scope.snapshot().decoded_bytes, 0);
    }
    // Both complete semantic/lifetime cases precede the causal work gate.
    for work in &observations {
        assert_eq!(work.contents_payload, payload.len() as u64);
        assert_eq!((work.digest_payload, work.digest_sha), (0, 0));
    }
    for work in observations {
        assert_eq!(
            work.contents_sha, 0,
            "fresh snapshot must not compute a discarded digest"
        );
    }
}
