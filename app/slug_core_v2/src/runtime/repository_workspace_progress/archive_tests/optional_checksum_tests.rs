//! The actual same-root owner replays an observed digest without reacquiring.
use super::*;

#[test]
#[ignore = "requires trusted Rust tmpfs fixture and actual archive acquisition"]
fn native_optional_archive_replay_retains_actual_digest_without_reacquisition() {
    let fixture = tempfile::tempdir().unwrap();
    let display = fixture.path().join("unmounted-display");
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let incoming = bank.reserve(1024 * 1024, 0, 0).unwrap();
    let scope = new_scope();
    let builder = empty_builder(reservation(&display, 1591), bank.clone(), &scope);
    let fd = builder.file.as_raw_fd();
    let progress = RepositoryWorkspaceProgress::new(builder, &scope, &|| true).unwrap();
    let (source, pinned) = input(fixture.path(), false);
    let actual: [u8; 32] = sha2::Sha256::digest(std::fs::read(source.path()).unwrap()).into();
    let RepositoryWorkspaceOperation::Archive(pinned) = pinned else {
        unreachable!()
    };
    let operation = RepositoryWorkspaceOperation::Archive(
        GeneratedRepositoryArchiveEffect::new(
            Arc::from(pinned.urls()),
            None,
            pinned.strip_prefix().into(),
            pinned.format(),
            pinned.canonical_id().into(),
            "".into(),
        )
        .unwrap(),
    );
    let request = progress.transcript().request(operation).unwrap();
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
        progress.transcript().records().last().unwrap().result(),
        &RepositoryWorkspaceResult::Archive(actual)
    );
    assert!(
        matches!(progress.transcript().current().entry("body").unwrap().node(),
        RepositoryWorkspaceNode::File(body) if body.as_ref()==b"archive replacement")
    );
    let counters = scope.snapshot();
    let decoded = progress.decoded;
    assert!(decoded > 0);
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
    let after = scope.snapshot();
    assert_eq!(after.source_attempts, counters.source_attempts);
    assert_eq!(after.transferred_bytes, counters.transferred_bytes);
    assert_eq!(after.written_bytes, counters.written_bytes);
    assert_eq!(after.decoded_bytes, counters.decoded_bytes);
    assert_eq!(progress.decoded, decoded);
    assert_eq!(progress.transcript().records().len(), 1);
    let sealed = progress.seal(1, &scope, &|| true).unwrap();
    assert!(!display.exists());
    drop(sealed);
    drop(incoming);
    assert_released(&bank, baseline, fd);
}
