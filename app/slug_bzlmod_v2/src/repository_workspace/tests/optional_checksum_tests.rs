//! Pure request/history proof only; Core separately verifies actual captures.
use super::*;
use crate::GeneratedRepositoryArchiveEffect;
use crate::GeneratedRepositoryArchiveFormat;

fn archive(expected: Option<[u8; 32]>) -> GeneratedRepositoryArchiveEffect {
    GeneratedRepositoryArchiveEffect::new(
        Arc::from(["https://ordinary.test/archive.tar.gz".into()]),
        expected,
        "".into(),
        GeneratedRepositoryArchiveFormat::TarGz,
        "".into(),
        "".into(),
    )
    .unwrap()
}
fn complete(
    transcript: &RepositoryWorkspaceTranscript,
    expected: Option<[u8; 32]>,
    actual: [u8; 32],
) -> Result<RepositoryWorkspaceTranscript, RepositoryWorkspaceError> {
    transcript.complete(
        transcript.request(RepositoryWorkspaceOperation::Archive(archive(expected)))?,
        RepositoryWorkspaceResult::Archive(actual),
        RepositoryWorkspaceMode::Archive,
        vec![
            root(),
            file(
                "out",
                RepositoryWorkspaceBody::from_arc(Arc::from(&b"same extracted content"[..])),
            ),
        ],
    )
}

#[test]
fn general_archive_omission_retains_actual_digest_and_exact_history_replay() {
    let before = transcript();
    let request = before
        .request(RepositoryWorkspaceOperation::Archive(archive(None)))
        .unwrap();
    let completed = complete(&before, None, [9; 32]).unwrap();
    assert_eq!(completed.records().len(), 1);
    assert_eq!(completed.current().mode(), RepositoryWorkspaceMode::Archive);
    assert_eq!(
        completed.records()[0].result(),
        &RepositoryWorkspaceResult::Archive([9; 32])
    );
    assert_eq!(
        completed
            .request_at(0, RepositoryWorkspaceOperation::Archive(archive(None)))
            .unwrap(),
        request
    );
    assert_eq!(
        completed.replay(&request).unwrap().unwrap().result(),
        &RepositoryWorkspaceResult::Archive([9; 32])
    );
    let changed = completed
        .request_at(
            0,
            RepositoryWorkspaceOperation::Archive(archive(Some([9; 32]))),
        )
        .unwrap();
    assert!(
        completed.replay(&changed).is_err(),
        "declared expectation absence remains request identity"
    );
    assert_eq!(
        complete(&before, Some([8; 32]), [9; 32]),
        Err(RepositoryWorkspaceError::InvalidResult)
    );
    assert!(before.records().is_empty());
    let alternate = complete(&before, None, [10; 32]).unwrap();
    assert_ne!(
        completed, alternate,
        "actual compressed digests remain full history identity"
    );
    assert_eq!(completed.current(), alternate.current());
    assert_eq!(
        completed.realize(1).unwrap().source_identity(),
        alternate.realize(1).unwrap().source_identity(),
        "final-content SHA remains based on the complete extracted tree"
    );
    assert_eq!(
        completed.retention().body_bytes(),
        alternate.retention().body_bytes()
    );
}

#[test]
fn archive_expected_option_layout_is_included_in_operation_metadata() {
    #[allow(dead_code)]
    struct OriginalArchive {
        urls: Arc<[CompactString]>,
        sha256: [u8; 32],
        strip_prefix: CompactString,
        format: GeneratedRepositoryArchiveFormat,
        output: crate::GeneratedRepositoryArchiveOutput,
        canonical_id: CompactString,
        declared_integrity: CompactString,
    }
    assert!(std::mem::size_of::<Option<[u8; 32]>>() > std::mem::size_of::<[u8; 32]>());
    assert!(
        std::mem::size_of::<GeneratedRepositoryArchiveEffect>()
            >= std::mem::size_of::<OriginalArchive>()
    );
    let none = RepositoryWorkspaceOperation::Archive(archive(None));
    let pinned = RepositoryWorkspaceOperation::Archive(archive(Some([0; 32])));
    assert!(none.metadata().unwrap() >= std::mem::size_of::<GeneratedRepositoryArchiveEffect>());
    assert_eq!(none.metadata().unwrap(), pinned.metadata().unwrap());
}
