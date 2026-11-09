//! Real Archive capture/FD verification across paths. Allocation reuse is not
//! inferred from length, archive identity, digest, or semantic tree equality.
use super::*;

fn fixture() -> (
    tempfile::TempDir,
    Arc<RepositoryWorkspaceReservation>,
    Arc<RepositoryRootResources>,
    Arc<SourceServiceScope>,
    tokio::runtime::Runtime,
) {
    let fixture = tempfile::Builder::new()
        .prefix("slug-wp425-rolling-")
        .tempdir()
        .unwrap();
    let display = fixture.path().join("root");
    std::fs::create_dir(&display).unwrap();
    let selected = reservation(&display, 1893);
    let bank = bank();
    let scope = command_scope(&selected);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    (fixture, selected, bank, scope, runtime)
}

#[test]
#[ignore = "requires trusted Rust physical Archive factory and two actual48MiB different-path captures"]
fn native_deleted_history_body_is_recovered_at_new_path_through_read_and_materialization() {
    let (fixture, selected, bank, scope, runtime) = fixture();
    let baseline = bank.snapshot().retained_bytes;
    let (archive_a, operation_a) = archive_input(fixture.path(), "old/large", b'a');
    let (archive_b, operation_b) = archive_input(fixture.path(), "new/large", b'a');
    let (command, fd) = start(selected.clone(), bank.clone(), scope.clone());
    let command = apply(command, &selected, operation_a, &runtime, &scope);
    let old_body = {
        let transcript = &command.epoch().transcripts().unwrap()[0];
        let RepositoryWorkspaceNode::File(body) =
            transcript.current().entry("old/large").unwrap().node()
        else {
            panic!()
        };
        assert_eq!(body.len(), LARGE);
        Arc::downgrade(body)
    };
    let command = apply(
        command,
        &selected,
        RepositoryWorkspaceOperation::Delete("old/large".into()),
        &runtime,
        &scope,
    );
    {
        let transcript = &command.epoch().transcripts().unwrap()[0];
        assert!(transcript.current().entry("old/large").is_none());
        assert!(transcript.current().entry("new/large").is_none());
        assert_eq!(
            transcript.retention().body_bytes(),
            (LARGE + KEPT.len()) as u64
        );
    }
    // Baseline RED: direct same-path reuse has no entry at new/large, while
    // history's genuine48MiB allocation still occupies the64MiB native bank.
    let command = apply(command, &selected, operation_b, &runtime, &scope);
    {
        let transcript = &command.epoch().transcripts().unwrap()[0];
        assert_eq!(transcript.records().len(), 3);
        let RepositoryWorkspaceNode::File(new) =
            transcript.current().entry("new/large").unwrap().node()
        else {
            panic!()
        };
        assert!(
            Arc::ptr_eq(new, &old_body.upgrade().unwrap()),
            "fresh rooted comparison must recover the actual historical allocation"
        );
        assert_eq!(
            transcript.retention().body_bytes(),
            (LARGE + KEPT.len()) as u64
        );
    }
    assert_eq!(
        scope.snapshot().source_attempts,
        2,
        "both real archives were captured"
    );
    // Preserve the admitted2MiB ctx.read bound: use a real small following Read,
    // and stream the full large file only through the final native reader.
    let command = apply(
        command,
        &selected,
        RepositoryWorkspaceOperation::Read("kept".into()),
        &runtime,
        &scope,
    );
    {
        let transcript = &command.epoch().transcripts().unwrap()[0];
        let RepositoryWorkspaceResult::Read(read) = transcript.records().last().unwrap().result()
        else {
            panic!()
        };
        assert_eq!(read.as_ref(), KEPT);
        assert_eq!(transcript.records().len(), 4);
    }
    let request = final_request(&command, &selected);
    let (command, sealed) = command
        .seal_for_materialization(request, &scope, &|| true)
        .unwrap();
    command.check_epoch_body_coverage(&scope, &|| true).unwrap();
    let (request, root, history) = sealed.into_parts();
    let RepositoryWorkspaceNode::File(final_body) = root.state().entry("new/large").unwrap().node()
    else {
        panic!()
    };
    assert!(Arc::ptr_eq(final_body, &old_body.upgrade().unwrap()));
    let weak_root = Arc::downgrade(&root);
    let mut reader = root.open_member("new/large", &scope, &|| true).unwrap();
    drop((command, request, root));
    assert!(old_body.upgrade().is_some());
    drop(history);
    assert!(
        old_body.upgrade().is_some(),
        "final reader's filtered data coverage shares the historical allocation"
    );
    let mut scratch = [0; 64 * 1024];
    let mut count = 0usize;
    loop {
        let read = reader.read(&mut scratch).unwrap();
        if read == 0 {
            break;
        }
        assert!(scratch[..read].iter().all(|byte| *byte == b'a'));
        count += read;
    }
    assert_eq!(count, LARGE);
    assert!(weak_root.upgrade().is_some());
    drop(reader);
    assert!(weak_root.upgrade().is_none());
    assert!(old_body.upgrade().is_none());
    release(&bank, baseline, fd);
    drop((archive_a, archive_b, scope, runtime));
}

#[test]
#[ignore = "requires trusted Rust physical Archive factory and changed48MiB history/new-path overlap"]
fn native_deleted_history_different_bytes_at_new_path_still_refuse_full_overlap() {
    let (fixture, selected, bank, scope, runtime) = fixture();
    let baseline = bank.snapshot().retained_bytes;
    let (archive_a, operation_a) = archive_input(fixture.path(), "old/large", b'a');
    let (archive_b, operation_b) = archive_input(fixture.path(), "new/large", b'b');
    let (command, fd) = start(selected.clone(), bank.clone(), scope.clone());
    let command = apply(command, &selected, operation_a, &runtime, &scope);
    let old_body = {
        let transcript = &command.epoch().transcripts().unwrap()[0];
        let RepositoryWorkspaceNode::File(body) =
            transcript.current().entry("old/large").unwrap().node()
        else {
            panic!()
        };
        Arc::downgrade(body)
    };
    let command = apply(
        command,
        &selected,
        RepositoryWorkspaceOperation::Delete("old/large".into()),
        &runtime,
        &scope,
    );
    assert!(
        old_body.upgrade().is_some(),
        "deleted payload is still retained by full history"
    );
    let request = command
        .request(&selected, operation_b, &scope, &|| true)
        .unwrap();
    let result = command.apply(
        request,
        &runtime,
        &crate::runtime::ArchiveDistdirPolicy::default(),
        &scope,
        &|| true,
    );
    assert!(
        matches!(result, Err(PhysicalRootError::Resource)),
        "same length/new path cannot stand in for full rooted byte equality"
    );
    assert_eq!(scope.snapshot().source_attempts, 2);
    assert!(old_body.upgrade().is_none());
    release(&bank, baseline, fd);
    drop((archive_a, archive_b, scope, runtime));
}
