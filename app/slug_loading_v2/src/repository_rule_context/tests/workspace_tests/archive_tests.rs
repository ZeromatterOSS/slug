//! Original Starlark with supplied pure-protocol results; actual compressed
//! capture/extraction and final publication remain separate native Core gates.
//! Pinned Bazel9.2 starlark_repository_test.sh1159+ returns actual SHA/SRI when
//! download_and_extract omits both expected-checksum parameters.
use sha2::Digest;

use super::*;

const ORIGINAL: &str = r#"
def implementation(ctx):
    result = ctx.download_and_extract('file:///independent/archive.tar.gz', type='tar.gz', strip_prefix='prefix')
    if not result.success: fail('archive result failed')
    if ctx.read('extracted') != 'observed extracted body': fail('extracted state unavailable')
    ctx.file('result', result.sha256 + '|' + result.integrity, executable=False)
"#;

#[tokio::test]
async fn general_original_archive_without_checksum_replays_observed_digest_and_state() {
    let (mut epoch, demand) = binding(41).await;
    let first = attempt(ORIGINAL, &epoch, &demand);
    let Err(RepositoryRuleInvocationError::WorkspaceNeed(need)) = first.result else {
        panic!("checksum-omitted original must suspend at its actual Archive operation");
    };
    let request = need.request();
    let RepositoryWorkspaceOperation::Archive(effect) = request.operation() else {
        panic!("original must emit Archive before reading its returned result");
    };
    assert_eq!(request.index(), 0);
    assert_eq!(effect.expected_sha256(), None);
    assert_eq!(
        effect.urls()[0].as_str(),
        "file:///independent/archive.tar.gz"
    );
    assert_eq!(effect.strip_prefix(), "prefix");
    // This is a protocol result fixture, not a physical archive receipt. The
    // observed digest is independent of an expected hash in the intent.
    let observed_digest: [u8; 32] = sha2::Sha256::digest(b"captured archive bytes").into();
    let extracted: Arc<[u8]> = Arc::from(b"observed extracted body".as_slice());
    let mut archive_entries = request.predecessor().entries().to_vec();
    archive_entries.push(entry(
        "extracted",
        0o644,
        RepositoryWorkspaceNode::File(extracted.clone()),
    ));
    epoch = epoch
        .complete(
            request.as_ref().clone(),
            RepositoryWorkspaceResult::Archive(observed_digest),
            RepositoryWorkspaceMode::Archive,
            archive_entries,
        )
        .unwrap();
    let second = attempt(ORIGINAL, &epoch, &demand);
    let Err(RepositoryRuleInvocationError::WorkspaceNeed(read)) = second.result else {
        panic!("archive success must continue to read the accepted extracted state");
    };
    assert!(
        matches!(read.request().operation(), RepositoryWorkspaceOperation::Read(path) if path == "extracted")
    );
    epoch = epoch
        .complete(
            read.request().as_ref().clone(),
            RepositoryWorkspaceResult::Read(extracted),
            RepositoryWorkspaceMode::Archive,
            read.request().predecessor().entries().to_vec(),
        )
        .unwrap();
    let third = attempt(ORIGINAL, &epoch, &demand);
    let Err(RepositoryRuleInvocationError::WorkspaceNeed(file)) = third.result else {
        panic!("continued original must write its actual digest return fields");
    };
    let RepositoryWorkspaceOperation::File { path, body, mode } = file.request().operation() else {
        panic!("expected authored digest output");
    };
    let expected = format!(
        "{}|{}",
        slug_bzlmod_v2::generated_repository_archive_sha256_hex(observed_digest),
        slug_bzlmod_v2::generated_repository_archive_sha256_sri(observed_digest),
    );
    assert_eq!(path, "result");
    assert_eq!(body.as_ref(), expected.as_bytes());
    assert_eq!(*mode, 0o644);
    let mut entries = file.request().predecessor().entries().to_vec();
    entries.push(entry(
        path,
        *mode,
        RepositoryWorkspaceNode::File(body.clone()),
    ));
    epoch = epoch
        .complete(
            file.request().as_ref().clone(),
            RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            entries,
        )
        .unwrap();
    let complete = attempt(ORIGINAL, &epoch, &demand).result.unwrap();
    let realized = complete.workspace_realization.unwrap();
    assert_eq!(realized.transcript().records().len(), 3);
    assert!(complete.plan.effects().is_empty());
}

#[test]
fn legacy_original_archive_without_checksum_refuses_before_filesystem_need() {
    assert!(matches!(
        invoke(ORIGINAL),
        Err(RepositoryRuleInvocationError::Plan(
            slug_bzlmod_v2::GeneratedRepositoryFileEffectPlanError::MissingArchiveChecksum
        )),
    ));
}
