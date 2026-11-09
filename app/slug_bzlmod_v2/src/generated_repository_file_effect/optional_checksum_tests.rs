//! Omitted expectations are General intents, never legacy verification facts.
use super::*;

#[derive(Default)]
struct Stream(Vec<u8>);
impl Hasher for Stream {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
}
fn stream(value: &impl Hash) -> Vec<u8> {
    let mut state = Stream::default();
    value.hash(&mut state);
    state.0
}
fn effect(expected: Option<[u8; 32]>) -> GeneratedRepositoryArchiveEffect {
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
#[derive(Hash)]
struct OriginalArchive<'a> {
    urls: &'a Arc<[CompactString]>,
    sha256: &'a [u8; 32],
    strip_prefix: &'a CompactString,
    format: &'a GeneratedRepositoryArchiveFormat,
    output: &'a GeneratedRepositoryArchiveOutput,
    canonical_id: &'a CompactString,
    declared_integrity: &'a CompactString,
}
fn original(value: &GeneratedRepositoryArchiveEffect) -> OriginalArchive<'_> {
    OriginalArchive {
        urls: &value.urls,
        sha256: value.expected_sha256.as_ref().unwrap(),
        strip_prefix: &value.strip_prefix,
        format: &value.format,
        output: &value.output,
        canonical_id: &value.canonical_id,
        declared_integrity: &value.declared_integrity,
    }
}

#[test]
fn optional_checksum_parser_keeps_strict_supplied_checksum_contract() {
    use GeneratedRepositoryFileEffectPlanError as E;
    let parse = generated_repository_archive_parse_sha256;
    let optional = generated_repository_archive_parse_optional_sha256;
    assert_eq!(parse("", ""), Err(E::MissingArchiveChecksum));
    assert_eq!(optional("", ""), Ok(None));
    let sha = generated_repository_archive_sha256_hex([7; 32]);
    let sri = generated_repository_archive_sha256_sri([7; 32]);
    for (sha, integrity) in [
        (&sha[..], ""),
        ("", &sri[..]),
        ("", sri.trim_end_matches('=')),
    ] {
        assert_eq!(optional(sha, integrity), parse(sha, integrity).map(Some));
        assert_eq!(optional(sha, integrity), Ok(Some([7; 32])));
    }
    for (sha, integrity) in [
        ("abc", ""),
        ("", "sha512-AA=="),
        ("", "sha256-AA=="),
        (&sha[..], &sri[..]),
    ] {
        assert_eq!(optional(sha, integrity), parse(sha, integrity).map(Some));
        assert!(optional(sha, integrity).is_err());
    }
    let build = |expected| {
        GeneratedRepositoryArchiveEffect::new(
            Arc::from(["https://ordinary.test/archive.tar.gz".into()]),
            expected,
            "".into(),
            GeneratedRepositoryArchiveFormat::TarGz,
            "".into(),
            sri.clone().into(),
        )
    };
    assert_eq!(build(None), Err(E::InvalidArchiveChecksum));
    assert_eq!(build(Some([8; 32])), Err(E::InvalidArchiveChecksum));
    assert!(build(Some([7; 32])).is_ok());
    let none = effect(None);
    assert_eq!(none.expected_sha256(), None);
    assert_eq!(none.expected_sha256_hex(), None);
    assert_eq!(none.expected_sha256_sri(), None);
    assert_eq!(
        GeneratedRepositoryArchiveEffect::new(
            Arc::from(["http://ordinary.test/archive.tar.gz".into()]),
            None,
            "".into(),
            GeneratedRepositoryArchiveFormat::TarGz,
            "".into(),
            "".into(),
        ),
        Err(E::InvalidArchiveUrl),
    );
}

#[test]
fn pinned_archive_hash_is_original_field_stream_and_none_is_distinct() {
    let base = effect(Some([7; 32]));
    let mut cases = vec![base.clone(), effect(Some([0; 32]))];
    let mut changed = base.clone();
    changed.urls = Arc::from([
        "https://ordinary.test/mirror.tar.gz".into(),
        "file:///tmp/archive.tar.gz".into(),
    ]);
    cases.push(changed.clone());
    changed.urls = changed.urls.iter().rev().cloned().collect();
    cases.push(changed);
    let mut changed = base.clone();
    changed.strip_prefix = "inside/source".into();
    cases.push(changed);
    let mut changed = base.clone();
    changed.format = GeneratedRepositoryArchiveFormat::TarXz;
    cases.push(changed);
    let mut changed = base.clone();
    changed.canonical_id = "independent id".into();
    cases.push(changed);
    let mut changed = base.clone();
    changed.declared_integrity = generated_repository_archive_sha256_sri([7; 32]).into();
    cases.push(changed);
    for value in &cases {
        assert_eq!(
            stream(value),
            stream(&original(value)),
            "retain original derived field writers, including the array length framing"
        );
        assert_eq!(stream(value), stream(&value.clone()));
    }
    let none = effect(None);
    let zero = effect(Some([0; 32]));
    assert_ne!(none, zero);
    assert_ne!(stream(&none), stream(&zero));
    assert!(stream(&none).starts_with(b"slug-generated-archive-unpinned-v1\0"));
    let mut changed = none.clone();
    changed.canonical_id = "other".into();
    assert_ne!(none, changed);
    assert_ne!(stream(&none), stream(&changed));
}

#[test]
fn legacy_plan_and_direct_transcript_reject_missing_expectation() {
    use crate::RepositoryFilesystemCallId;
    use crate::RepositoryFilesystemOperation;
    use crate::RepositoryFilesystemRecord;
    use crate::RepositoryFilesystemRequest;
    use crate::RepositoryFilesystemResult;
    use crate::RepositoryFilesystemTranscript;
    use crate::RepositoryFilesystemTranscriptError;
    let none = effect(None);
    let empty = GeneratedRepositoryFileEffectPlan::builder().finish();
    let mut builder = GeneratedRepositoryFileEffectPlan::builder();
    assert_eq!(
        builder.push_archive_effect(none.clone()),
        Err(GeneratedRepositoryFileEffectPlanError::MissingArchiveChecksum)
    );
    assert_eq!(builder.finish(), empty);
    assert!(empty.with_archive(none.clone()).is_err());
    let owner = Arc::new(crate::HostSelectedExtensionOwner::testing("ordinary"));
    let make = |archive| {
        RepositoryFilesystemRequest::new(
            RepositoryFilesystemCallId::new(owner.clone(), 0, 0),
            "ordinary+data".into(),
            RepositoryFilesystemOperation::Archive(archive),
            empty.clone(),
        )
    };
    let request = make(none);
    let id = request.id().clone();
    let transcript = RepositoryFilesystemTranscript::empty();
    assert_eq!(
        transcript.append(RepositoryFilesystemRecord::new(
            Arc::new(request),
            RepositoryFilesystemResult::Archive([9; 32])
        )),
        Err(RepositoryFilesystemTranscriptError::InvalidOperation(id))
    );
    assert!(transcript.records().is_empty());
    let zero = make(effect(Some([0; 32])));
    assert!(
        transcript
            .append(RepositoryFilesystemRecord::new(
                Arc::new(zero.clone()),
                RepositoryFilesystemResult::Archive([0; 32])
            ))
            .is_ok()
    );
    assert_eq!(
        transcript.append(RepositoryFilesystemRecord::new(
            Arc::new(zero.clone()),
            RepositoryFilesystemResult::Archive([1; 32])
        )),
        Err(RepositoryFilesystemTranscriptError::ChecksumMismatch(
            zero.id().clone()
        ))
    );
}

#[test]
fn unpinned_http_mirrors_remain_intent_but_require_eligible_source() {
    let build = |expected, urls: &[&str]| {
        GeneratedRepositoryArchiveEffect::new(
            urls.iter().map(|url| CompactString::new(url)).collect(),
            expected,
            "".into(),
            GeneratedRepositoryArchiveFormat::TarGz,
            "".into(),
            "".into(),
        )
    };
    let mixed = build(
        None,
        &[
            "http://ignored.invalid/type.zip",
            "file:///tmp/actual.tar.gz",
        ],
    )
    .unwrap();
    assert_eq!(
        mixed
            .urls()
            .iter()
            .map(CompactString::as_str)
            .collect::<Vec<_>>(),
        vec![
            "http://ignored.invalid/type.zip",
            "file:///tmp/actual.tar.gz"
        ]
    );
    assert_ne!(mixed, build(None, &["file:///tmp/actual.tar.gz"]).unwrap());
    assert!(build(None, &["http://ignored.invalid/type.zip"]).is_err());
    assert!(
        build(
            Some([7; 32]),
            &[
                "http://ignored.invalid/type.zip",
                "file:///tmp/actual.tar.gz"
            ]
        )
        .is_err()
    );
    for invalid in [
        "http://user@ignored.invalid/archive",
        "http://user:password@ignored.invalid/archive",
        "http://ignored.invalid/archive#fragment",
        "http://",
    ] {
        assert!(build(None, &[invalid, "file:///tmp/actual.tar.gz"]).is_err());
    }
}
