//! Handcrafted gABI wire fixtures and real normal-image Need fulfillment.
use std::cell::Cell;
use std::fs::File;
use std::path::Path;

use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::ElfMalformedReason;
use slug_workspace_v2::ElfMetadataLimit;
use slug_workspace_v2::ElfMetadataLimits;
use slug_workspace_v2::ElfMetadataOutcome;
use slug_workspace_v2::ElfMetadataRejection;
use slug_workspace_v2::ElfUnsupportedReason;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageObservationRequest;
use slug_workspace_v2::InputImagePolicy;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationNamespace;

use super::*;

fn set16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn set32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn set64(bytes: &mut [u8], at: usize, value: u64) {
    bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
fn segment(bytes: &mut [u8], at: usize, kind: u32, offset: u64, address: u64, length: u64) {
    set32(bytes, at, kind);
    set32(bytes, at + 4, 5);
    set64(bytes, at + 8, offset);
    set64(bytes, at + 16, address);
    set64(bytes, at + 32, length);
    set64(bytes, at + 40, length);
    set64(bytes, at + 48, 1);
}
fn elf() -> Vec<u8> {
    let mut bytes = vec![0; 1024];
    bytes[..9].copy_from_slice(b"\x7fELF\x02\x01\x01\x00\x00");
    set16(&mut bytes, 16, 2);
    set16(&mut bytes, 18, 62);
    set32(&mut bytes, 20, 1);
    set64(&mut bytes, 32, 64);
    set16(&mut bytes, 52, 64);
    set16(&mut bytes, 54, 56);
    set16(&mut bytes, 56, 3);
    segment(&mut bytes, 64, 3, 256, 0, 7);
    segment(&mut bytes, 120, 1, 0, 0x400000, 1024);
    segment(&mut bytes, 176, 2, 320, 0x400140, 128);
    bytes[256..263].copy_from_slice(b"/ld.so\0");
    let strings = b"\0liba.so\0:$ORIGIN:\0\0";
    bytes[512..512 + strings.len()].copy_from_slice(strings);
    let entries = [
        (5, 0x400200),
        (10, strings.len() as u64),
        (1, 1),
        (1, 1),
        (15, 9),
        (29, 19),
        (0x70000000, 42),
        (0, 0),
    ];
    for (i, (tag, value)) in entries.into_iter().enumerate() {
        set64(&mut bytes, 320 + i * 16, tag);
        set64(&mut bytes, 328 + i * 16, value);
    }
    bytes
}
fn binding(bytes: &[u8]) -> ElfSourceDigestBinding {
    ElfSourceDigestBinding::from_digest(
        FileContentDigest::new(Sha256::digest(bytes).into(), bytes.len() as u64).unwrap(),
    )
}
fn policy(limits: ElfMetadataLimits) -> ElfMetadataPolicy {
    ElfMetadataPolicy::new(ElfMetadataPolicy::default().profile(), limits).unwrap()
}
fn parsed(bytes: &[u8], selected: ElfMetadataPolicy) -> VerifiedElfMetadata {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("elf");
    std::fs::write(&path, bytes).unwrap();
    let file = File::open(path).unwrap();
    let budget = ImageDescriptorBudget::new(16 << 20);
    let mut credit = budget.try_reserve(512).unwrap();
    let mut reader =
        proof::ProofReader::new(&file, bytes.len() as u64, selected, &budget, &|| true).unwrap();
    let outcome = parser::parse(&mut reader, selected, &mut credit).unwrap();
    reader.verify(binding(bytes)).unwrap();
    VerifiedElfMetadata::new(binding(bytes), selected, outcome, credit).unwrap()
}
fn rejection(bytes: &[u8], selected: ElfMetadataPolicy, expected: ElfMetadataRejection) {
    assert!(
        matches!(parsed(bytes, selected).outcome(), ElfMetadataOutcome::Rejected(actual) if *actual == expected)
    );
}

#[test]
fn sectionless_exec_and_dyn_preserve_order_duplicates_raw_literals_and_controls() {
    for object_type in [2, 3] {
        let mut bytes = elf();
        set16(&mut bytes, 16, object_type);
        let value = parsed(&bytes, ElfMetadataPolicy::default());
        let ElfMetadataOutcome::Parsed(facts) = value.outcome() else {
            panic!("valid sectionless wire fixture");
        };
        assert_eq!(facts.header().object_type, object_type);
        assert_eq!(facts.interpreter(), Some(&b"/ld.so"[..]));
        assert_eq!(
            facts
                .string_refs()
                .iter()
                .map(|s| (s.tag, s.bytes()))
                .collect::<Vec<_>>(),
            vec![
                (1, &b"liba.so"[..]),
                (1, &b"liba.so"[..]),
                (15, &b":$ORIGIN:"[..]),
                (29, &b""[..])
            ]
        );
        assert_eq!(facts.dynamic_entries()[6].tag, 0x70000000);
        assert_eq!(facts.dynamic_entries()[6].value, 42);
        assert_eq!(facts.logical_bytes(), Some(650));
    }
    let mut static_elf = elf();
    set16(&mut static_elf, 56, 1);
    segment(&mut static_elf, 64, 1, 0, 0x400000, 1024);
    assert!(
        matches!(parsed(&static_elf, ElfMetadataPolicy::default()).outcome(), ElfMetadataOutcome::Parsed(facts) if facts.interpreter().is_none() && facts.dynamic_entries().is_empty())
    );
    let mut raw = elf();
    raw[513] = 0xff;
    assert!(
        matches!(parsed(&raw, ElfMetadataPolicy::default()).outcome(), ElfMetadataOutcome::Parsed(facts) if facts.string_refs()[0].bytes()[0] == 0xff)
    );
}

#[test]
fn checked_format_profile_and_mapping_failures_are_authenticated_rejections() {
    let default = ElfMetadataPolicy::default();
    rejection(
        &elf()[..63],
        default,
        ElfMetadataRejection::Malformed(ElfMalformedReason::Header),
    );
    let mut bytes = elf();
    set64(&mut bytes, 32, u64::MAX);
    rejection(
        &bytes,
        default,
        ElfMetadataRejection::Malformed(ElfMalformedReason::ProgramTable),
    );
    for (at, value, reason) in [
        (4, 1, ElfUnsupportedReason::Class),
        (5, 2, ElfUnsupportedReason::Endian),
        (7, 9, ElfUnsupportedReason::Abi),
        (8, 1, ElfUnsupportedReason::Abi),
    ] {
        let mut bytes = elf();
        bytes[at] = value;
        rejection(&bytes, default, ElfMetadataRejection::Unsupported(reason));
    }
    let mut bytes = elf();
    set16(&mut bytes, 18, 3);
    rejection(
        &bytes,
        default,
        ElfMetadataRejection::Unsupported(ElfUnsupportedReason::Machine),
    );
    let mut bytes = elf();
    set16(&mut bytes, 56, 0xffff);
    rejection(
        &bytes,
        default,
        ElfMetadataRejection::Unsupported(ElfUnsupportedReason::ExtendedProgramCount),
    );
    let mut bytes = elf();
    set64(&mut bytes, 320 + 7 * 16, 1);
    rejection(
        &bytes,
        default,
        ElfMetadataRejection::Malformed(ElfMalformedReason::DynamicTable),
    );
    let mut bytes = elf();
    bytes[531] = b'x';
    rejection(
        &bytes,
        default,
        ElfMetadataRejection::Malformed(ElfMalformedReason::String),
    );
    let mut bytes = elf();
    set64(&mut bytes, 120 + 32, 128);
    rejection(
        &bytes,
        default,
        ElfMetadataRejection::Unsupported(ElfUnsupportedReason::Layout),
    );
    let mut bytes = elf();
    set16(&mut bytes, 56, 4);
    segment(&mut bytes, 232, 1, 0, 0x400000, 1024);
    set64(&mut bytes, 64 + 8, 900);
    bytes[900..907].copy_from_slice(b"/ld.so\0");
    rejection(
        &bytes,
        default,
        ElfMetadataRejection::Unsupported(ElfUnsupportedReason::AmbiguousLoadMapping),
    );
    // A second mapping covering only the table's interior is still ambiguous.
    segment(&mut bytes, 232, 1, 800, 0x400205, 5);
    rejection(
        &bytes,
        default,
        ElfMetadataRejection::Unsupported(ElfUnsupportedReason::AmbiguousLoadMapping),
    );
}

#[test]
fn independent_semantic_limits_have_inclusive_boundaries() {
    let bytes = elf();
    for (limit, positive, negative) in [
        (ElfMetadataLimit::ProgramHeaders, 3, 2),
        (ElfMetadataLimit::DynamicEntries, 8, 7),
        (ElfMetadataLimit::StringBytes, 10, 9),
        (ElfMetadataLimit::FactBytes, 650, 649),
    ] {
        for (n, valid) in [(positive, true), (negative, false)] {
            let mut limits = ElfMetadataLimits::default();
            match limit {
                ElfMetadataLimit::ProgramHeaders => limits.max_program_headers = n,
                ElfMetadataLimit::DynamicEntries => limits.max_dynamic_entries = n,
                ElfMetadataLimit::StringBytes => limits.max_string_bytes = n,
                ElfMetadataLimit::FactBytes => limits.max_fact_bytes = n,
                _ => unreachable!(),
            }
            if valid {
                assert!(matches!(
                    parsed(&bytes, policy(limits)).outcome(),
                    ElfMetadataOutcome::Parsed(_)
                ));
            } else {
                rejection(
                    &bytes,
                    policy(limits),
                    ElfMetadataRejection::PolicyLimit(limit),
                );
            }
        }
    }
}

#[test]
fn proof_occurrences_spans_and_overlap_normalization_are_bounded() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("data");
    std::fs::write(&path, b"0123456789").unwrap();
    let file = File::open(&path).unwrap();
    let budget = ImageDescriptorBudget::new(1 << 20);
    let mut limits = ElfMetadataLimits::default();
    limits.max_proof_bytes = 12;
    limits.max_proof_spans = 2;
    let mut reader = proof::ProofReader::new(&file, 10, policy(limits), &budget, &|| true).unwrap();
    reader.read(0, 6).unwrap();
    reader.read(4, 6).unwrap();
    reader.verify(binding(b"0123456789")).unwrap();
    assert_eq!(budget.used_bytes(), 0);
    limits.max_proof_bytes = 11;
    let mut reader = proof::ProofReader::new(&file, 10, policy(limits), &budget, &|| true).unwrap();
    reader.read(0, 6).unwrap();
    assert!(matches!(
        reader.read(4, 6),
        Err(proof::ReadError::Limit(ElfMetadataLimit::ProofBytes))
    ));
    drop(reader);
    assert_eq!(budget.used_bytes(), 0);
    limits.max_proof_bytes = 12;
    limits.max_proof_spans = 1;
    let mut reader = proof::ProofReader::new(&file, 10, policy(limits), &budget, &|| true).unwrap();
    reader.read(0, 1).unwrap();
    assert!(matches!(
        reader.read(2, 1),
        Err(proof::ReadError::Limit(ElfMetadataLimit::ProofSpans))
    ));
    drop(reader);
    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn matching_full_digest_cannot_bless_different_parser_bytes_or_conflicting_overlaps() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("data");
    let expected = elf();
    std::fs::write(&path, &expected).unwrap();
    let file = File::open(&path).unwrap();
    let budget = ImageDescriptorBudget::new(4 << 20);
    let selected = ElfMetadataPolicy::default();
    // Header-derived object type, a dynamic control value, literal data and
    // the NUL terminating a needed-library literal each influence real parse.
    for (offset, changed) in [(16, 3), (424, 43), (513, b'z'), (520, b'X')] {
        let mut transient = elf();
        transient[offset] = changed;
        std::fs::write(&path, &transient).unwrap();
        let mut credit = budget.try_reserve(512).unwrap();
        let mut reader =
            proof::ProofReader::new(&file, expected.len() as u64, selected, &budget, &|| true)
                .unwrap();
        let outcome = parser::parse(&mut reader, selected, &mut credit).unwrap();
        assert!(matches!(outcome, ElfMetadataOutcome::Parsed(_)));
        std::fs::write(&path, &expected).unwrap();
        assert_eq!(binding(&std::fs::read(&path).unwrap()), binding(&expected));
        assert!(
            matches!(
                reader.verify(binding(&expected)),
                Err(InputImageCaptureError::StaleSource)
            ),
            "unbound parser byte at {offset}"
        );
        drop(outcome);
        drop(credit);
        assert_eq!(budget.used_bytes(), 0);
    }
    let mut transient = elf();
    transient[513] = b'z';
    let mut reader =
        proof::ProofReader::new(&file, expected.len() as u64, selected, &budget, &|| true).unwrap();
    reader.read(513, 2).unwrap();
    std::fs::write(&path, &transient).unwrap();
    reader.read(513, 2).unwrap();
    std::fs::write(&path, &expected).unwrap();
    assert!(matches!(
        reader.verify(binding(&expected)),
        Err(InputImageCaptureError::StaleSource)
    ));
    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn proof_rejection_digest_eof_cancel_and_credit_failure_never_publish() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("data");
    std::fs::write(&path, b"not an elf").unwrap();
    let file = File::open(&path).unwrap();
    let budget = ImageDescriptorBudget::new(1 << 20);
    let selected = ElfMetadataPolicy::default();
    let mut credit = budget.try_reserve(512).unwrap();
    let mut reader = proof::ProofReader::new(&file, 10, selected, &budget, &|| true).unwrap();
    assert!(matches!(
        parser::parse(&mut reader, selected, &mut credit).unwrap(),
        ElfMetadataOutcome::Rejected(_)
    ));
    assert!(matches!(
        reader.verify(binding(b"XXXXXXXXXX")),
        Err(InputImageCaptureError::StaleSource)
    ));
    drop(credit);
    assert_eq!(budget.used_bytes(), 0);
    let mut reader = proof::ProofReader::new(&file, 10, selected, &budget, &|| true).unwrap();
    std::fs::write(&path, b"short").unwrap();
    assert!(matches!(
        reader.read(0, 10),
        Err(proof::ReadError::Operational(
            InputImageCaptureError::StaleSource
        ))
    ));
    drop(reader);
    assert_eq!(budget.used_bytes(), 0);
    let calls = Cell::new(0);
    let active = || {
        calls.set(calls.get() + 1);
        calls.get() < 2
    };
    let mut reader = proof::ProofReader::new(&file, 5, selected, &budget, &active).unwrap();
    assert!(matches!(
        reader.read(0, 5),
        Err(proof::ReadError::Operational(
            InputImageCaptureError::Cancelled
        ))
    ));
    drop(reader);
    assert_eq!(budget.used_bytes(), 0);
    let tiny = ImageDescriptorBudget::new(257);
    let mut reader = proof::ProofReader::new(&file, 5, selected, &tiny, &|| true).unwrap();
    assert!(matches!(
        reader.read(0, 5),
        Err(proof::ReadError::Operational(
            InputImageCaptureError::DescriptorBudget(_)
        ))
    ));
    drop(reader);
    assert_eq!(tiny.used_bytes(), 0);
}

struct FragmentedRead {
    bytes: &'static [u8],
    calls: Cell<usize>,
    interrupted: bool,
    zero: bool,
}
impl proof::ReadAt for FragmentedRead {
    fn read_at(&self, buffer: &mut [u8], offset: u64) -> std::io::Result<usize> {
        self.calls.set(self.calls.get() + 1);
        if self.interrupted {
            return Err(std::io::Error::from(std::io::ErrorKind::Interrupted));
        }
        if self.zero {
            return Ok(0);
        }
        let start = offset as usize;
        let count = buffer
            .len()
            .min(2)
            .min(self.bytes.len().saturating_sub(start));
        if count != 0 {
            buffer[..count].copy_from_slice(&self.bytes[start..start + count]);
        }
        Ok(count)
    }
}

#[test]
fn actual_proof_reader_handles_partial_progress_eof_and_single_interrupted_attempt() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let selected = ElfMetadataPolicy::default();
    let partial = FragmentedRead {
        bytes: b"0123456789",
        calls: Cell::new(0),
        interrupted: false,
        zero: false,
    };
    let mut reader = proof::ProofReader::new(&partial, 10, selected, &budget, &|| true).unwrap();
    assert_eq!(reader.read(0, 10).unwrap(), b"0123456789");
    reader.verify(binding(b"0123456789")).unwrap();
    assert!(partial.calls.get() > 2);
    assert_eq!(budget.used_bytes(), 0);
    let interrupted = FragmentedRead {
        bytes: b"data",
        calls: Cell::new(0),
        interrupted: true,
        zero: false,
    };
    let mut reader = proof::ProofReader::new(&interrupted, 4, selected, &budget, &|| true).unwrap();
    assert!(
        matches!(reader.read(0,4),Err(proof::ReadError::Operational(InputImageCaptureError::Io(error))) if error.kind()==std::io::ErrorKind::Interrupted)
    );
    assert_eq!(interrupted.calls.get(), 1);
    drop(reader);
    assert_eq!(budget.used_bytes(), 0);
    let reader = proof::ProofReader::new(&interrupted, 4, selected, &budget, &|| true).unwrap();
    assert!(
        matches!(reader.verify(binding(b"data")),Err(InputImageCaptureError::Io(error)) if error.kind()==std::io::ErrorKind::Interrupted)
    );
    assert_eq!(interrupted.calls.get(), 2);
    assert_eq!(budget.used_bytes(), 0);
    let zero = FragmentedRead {
        bytes: b"data",
        calls: Cell::new(0),
        interrupted: false,
        zero: true,
    };
    let mut reader = proof::ProofReader::new(&zero, 4, selected, &budget, &|| true).unwrap();
    assert!(matches!(
        reader.read(0, 4),
        Err(proof::ReadError::Operational(
            InputImageCaptureError::StaleSource
        ))
    ));
    assert_eq!(zero.calls.get(), 1);
    drop(reader);
    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn sparse_large_file_streaming_keeps_proof_credit_bounded_and_releases_it() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("data");
    let file = File::create(&path).unwrap();
    file.set_len(8 << 20).unwrap();
    drop(file);
    let file = File::open(&path).unwrap();
    let budget = ImageDescriptorBudget::new(4096);
    let mut reader = proof::ProofReader::new(
        &file,
        8 << 20,
        ElfMetadataPolicy::default(),
        &budget,
        &|| true,
    )
    .unwrap();
    assert_eq!(reader.read(4095, 2).unwrap(), &[0, 0]);
    assert!(budget.used_bytes() < 4096);
    let mut hash = Sha256::new();
    let buffer = [0u8; 64 * 1024];
    for _ in 0..128 {
        hash.update(buffer);
    }
    let expected = ElfSourceDigestBinding::from_digest(
        FileContentDigest::new(hash.finalize().into(), 8 << 20).unwrap(),
    );
    reader.verify(expected).unwrap();
    assert_eq!(budget.used_bytes(), 0);
}

struct CancellingStream<'a> {
    active: &'a Cell<bool>,
    armed: Cell<bool>,
    calls: Cell<usize>,
}
impl proof::ReadAt for CancellingStream<'_> {
    fn read_at(&self, buffer: &mut [u8], offset: u64) -> std::io::Result<usize> {
        self.calls.set(self.calls.get() + 1);
        let count = buffer
            .len()
            .min((128 * 1024u64).saturating_sub(offset) as usize);
        buffer[..count].fill(0);
        if self.armed.get() {
            self.active.set(false);
        }
        Ok(count)
    }
}

#[test]
fn actual_stream_cancellation_after_first_hash_read_releases_all_proof_credit() {
    let enabled = Cell::new(true);
    let source = CancellingStream {
        active: &enabled,
        armed: Cell::new(false),
        calls: Cell::new(0),
    };
    let budget = ImageDescriptorBudget::new(1 << 20);
    let baseline = budget.used_bytes();
    let active = || enabled.get();
    let mut reader = proof::ProofReader::new(
        &source,
        128 * 1024,
        ElfMetadataPolicy::default(),
        &budget,
        &active,
    )
    .unwrap();
    reader.read(0, 64).unwrap();
    let before_stream = source.calls.get();
    let mut hash = Sha256::new();
    let zeros = [0u8; 64 * 1024];
    hash.update(zeros);
    hash.update(zeros);
    let digest = ElfSourceDigestBinding::from_digest(
        FileContentDigest::new(hash.finalize().into(), 128 * 1024).unwrap(),
    );
    source.armed.set(true);
    assert!(matches!(
        reader.verify(digest),
        Err(InputImageCaptureError::Cancelled)
    ));
    assert_eq!(source.calls.get(), before_stream + 1);
    assert_eq!(budget.used_bytes(), baseline);
}

#[test]
fn generic_image_observer_rejects_elf_before_opening_a_missing_named_anchor() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("anchor");
    std::fs::create_dir(&root).unwrap();
    let approved = source(&root);
    let budget = ImageDescriptorBudget::new(1 << 20);
    let anchor =
        ImageChargedPath::pin_existing(NormalizedAbsolutePath::new(&root).unwrap(), &budget)
            .unwrap();
    let file = ImageChargedPath::pin_existing(
        NormalizedAbsolutePath::new(root.join("tool")).unwrap(),
        &budget,
    )
    .unwrap();
    let query = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        file,
        ImageObservationRequest::new(
            anchor,
            ImageObservationOperation::BoundedElfMetadata {
                expected_digest: binding(b"data"),
                policy: ElfMetadataPolicy::default(),
            },
        ),
    )
    .unwrap();
    std::fs::remove_dir(&root).unwrap();
    assert!(matches!(
        approved.observe(&query, &budget, &|| true),
        Err(InputImageCaptureError::Unauthorized)
    ));
    drop(query);
    assert_eq!(budget.used_bytes(), 0);
}

fn source(root: &Path) -> ScopedImageSource {
    let anchor = NormalizedAbsolutePath::new(root).unwrap();
    ScopedImageSource::new(
        PathObservationNamespace::Host,
        anchor.clone(),
        super::super::open_image_anchor(&anchor).unwrap(),
        None,
    )
    .unwrap()
}
async fn certificate(root: &Path, resources: &WorkspaceImageResources) -> ObservedInputImage {
    use dice::DetectCycles;
    use dice::Dice;
    use dice::UserComputationData;
    use slug_workspace_v2::ObservedInputImageKey;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathOutcome;
    use slug_workspace_v2::path_observation_shards;
    let source = source(root);
    let budget = resources.descriptor_budget();
    let key = ObservedInputImageKey::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(root).unwrap(),
        NormalizedAbsolutePath::new(root.join("input")).unwrap(),
        InputImagePolicy::plain(),
        budget,
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut rows = Vec::new();
    for _ in 0..64 {
        let mut data = UserComputationData::default();
        data.data.set(budget.clone());
        let mut updater = dice.updater_with_data(data);
        let epoch = PathObservationEpoch::new(rows.clone()).unwrap();
        updater.changed_to(path_observation_shards(&epoch)).unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        let mut transaction = updater.commit().await;
        let outcome = transaction.compute(&key).await.unwrap();
        drop(transaction);
        dice.wait_for_idle().await;
        match outcome {
            PathOutcome::Need(need) => {
                for demand in need.demands() {
                    assert!(rows.iter().all(|(old, _)| old != demand));
                    rows.push((
                        demand.clone(),
                        source.observe(demand, budget, &|| true).unwrap(),
                    ));
                }
            }
            PathOutcome::Complete(result) => return result.unwrap(),
        }
    }
    panic!("bounded normal image fixture made no progress")
}

#[tokio::test]
async fn actual_image_need_authorizes_only_exact_current_member_scope_and_budget() {
    let temporary = tempfile::tempdir().unwrap();
    std::fs::create_dir(temporary.path().join("input")).unwrap();
    std::fs::write(temporary.path().join("input/tool"), elf()).unwrap();
    let resources = WorkspaceImageResources::new(ImageDescriptorBudget::new(8 << 20), 2 << 30);
    let certificate = certificate(temporary.path(), &resources).await;
    let file = ImageChargedPath::pin_existing(
        NormalizedAbsolutePath::new(temporary.path().join("input/tool")).unwrap(),
        resources.descriptor_budget(),
    )
    .unwrap();
    let selected = ElfMetadataPolicy::default();
    let authorized = AuthorizedElfObservation::new(
        source(temporary.path()),
        certificate.clone(),
        file.clone(),
        selected,
        &resources,
    )
    .unwrap();
    let anchor = ImageChargedPath::pin_existing(
        NormalizedAbsolutePath::new(temporary.path()).unwrap(),
        resources.descriptor_budget(),
    )
    .unwrap();
    let query = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        file.clone(),
        ImageObservationRequest::new(
            anchor.clone(),
            ImageObservationOperation::BoundedElfMetadata {
                expected_digest: binding(&elf()),
                policy: selected,
            },
        ),
    )
    .unwrap();
    let result = observe_elf_metadata(&authorized, &query, &resources, &|| true).unwrap();
    assert!(
        matches!(result,PathObservationResult::InputImage(ref row) if matches!(row.data(),ImageObservationData::ElfMetadata(PathOperationResult::Present(value)) if matches!(value.outcome(),ElfMetadataOutcome::Parsed(_))))
    );
    assert!(matches!(
        observe_elf_metadata(&authorized, &query, &resources, &|| false),
        Err(InputImageCaptureError::Cancelled)
    ));
    let foreign = WorkspaceImageResources::new(ImageDescriptorBudget::new(8 << 20), 2 << 30);
    assert!(matches!(
        observe_elf_metadata(&authorized, &query, &foreign, &|| true),
        Err(InputImageCaptureError::Unauthorized)
    ));
    let wrong = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        file,
        ImageObservationRequest::new(
            anchor,
            ImageObservationOperation::BoundedElfMetadata {
                expected_digest: binding(b"different"),
                policy: selected,
            },
        ),
    )
    .unwrap();
    assert!(matches!(
        observe_elf_metadata(&authorized, &wrong, &resources, &|| true),
        Err(InputImageCaptureError::Unauthorized)
    ));
    std::fs::write(temporary.path().join("input/tool"), b"changed").unwrap();
    assert!(matches!(
        observe_elf_metadata(&authorized, &query, &resources, &|| true),
        Err(InputImageCaptureError::StaleSource)
    ));
    drop((authorized, certificate, query, wrong, result));
    assert_eq!(resources.descriptor_budget().used_bytes(), 0);
}

#[tokio::test]
async fn authenticated_rejection_history_retains_only_small_enum_credit() {
    let temporary = tempfile::tempdir().unwrap();
    std::fs::create_dir(temporary.path().join("input")).unwrap();
    let mut bytes = elf();
    bytes.resize(64 + 512 * 56, 0);
    set16(&mut bytes, 56, 512);
    // Valid unused program records grow partial facts before keyed refusal.
    bytes[64..].fill(0);
    std::fs::write(temporary.path().join("input/tool"), &bytes).unwrap();
    let resources = WorkspaceImageResources::new(ImageDescriptorBudget::new(8 << 20), 2 << 30);
    let certificate = certificate(temporary.path(), &resources).await;
    let file = ImageChargedPath::pin_existing(
        NormalizedAbsolutePath::new(temporary.path().join("input/tool")).unwrap(),
        resources.descriptor_budget(),
    )
    .unwrap();
    let anchor = ImageChargedPath::pin_existing(
        NormalizedAbsolutePath::new(temporary.path()).unwrap(),
        resources.descriptor_budget(),
    )
    .unwrap();
    let mut limits = ElfMetadataLimits::default();
    limits.max_fact_bytes = 192 + 256 * 56;
    let selected = policy(limits);
    let authorized = AuthorizedElfObservation::new(
        source(temporary.path()),
        certificate,
        file.clone(),
        selected,
        &resources,
    )
    .unwrap();
    let query = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        file,
        ImageObservationRequest::new(
            anchor,
            ImageObservationOperation::BoundedElfMetadata {
                expected_digest: binding(&bytes),
                policy: selected,
            },
        ),
    )
    .unwrap();
    let baseline = resources.descriptor_budget().used_bytes();
    // Independent small raw-observation control includes the real enum/block
    // layout (the enum can also hold Parsed facts). It models only the retained
    // rejection, without constructing or retaining any partial parser vectors.
    let small = VerifiedElfMetadata::new(
        binding(&bytes),
        selected,
        ElfMetadataOutcome::Rejected(ElfMetadataRejection::PolicyLimit(
            ElfMetadataLimit::FactBytes,
        )),
        resources.descriptor_budget().try_reserve(512).unwrap(),
    )
    .unwrap();
    let PathObservationOperation::InputImage(request) = query.operation() else {
        unreachable!()
    };
    let control = PathObservationResult::InputImage(
        ImageObservationResult::new(
            query.image_path().unwrap().clone(),
            request,
            ImageObservationData::ElfMetadata(PathOperationResult::Present(small)),
            resources.descriptor_budget().try_reserve(256).unwrap(),
        )
        .unwrap(),
    );
    let expected_retained = resources.descriptor_budget().used_bytes() - baseline;
    drop(control);
    assert_eq!(resources.descriptor_budget().used_bytes(), baseline);
    let result = observe_elf_metadata(&authorized, &query, &resources, &|| true).unwrap();
    assert!(
        matches!(&result,PathObservationResult::InputImage(row) if matches!(row.data(),ImageObservationData::ElfMetadata(PathOperationResult::Present(value)) if matches!(value.outcome(),ElfMetadataOutcome::Rejected(ElfMetadataRejection::PolicyLimit(ElfMetadataLimit::FactBytes)))))
    );
    let retained_bytes = resources.descriptor_budget().used_bytes() - baseline;
    assert_eq!(
        retained_bytes, expected_retained,
        "only the independently sized rejection container may remain; partial vector capacity must drop"
    );
    let history = result.clone();
    drop(result);
    assert_eq!(
        resources.descriptor_budget().used_bytes() - baseline,
        retained_bytes
    );
    drop(history);
    assert_eq!(resources.descriptor_budget().used_bytes(), baseline);
    drop((query, authorized));
    assert_eq!(resources.descriptor_budget().used_bytes(), 0);
}
