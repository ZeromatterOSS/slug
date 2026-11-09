//! Ordinary real-codec/accounting integration, without NativeRoot/IO authority
//! constructors. Fixture encoding happens before the command clock/bank owners.
#![cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use slug_workspace_v2::NormalizedAbsolutePath;
use xz2::stream::Check;
use xz2::stream::Filters;
use xz2::stream::LzmaOptions;
use xz2::stream::Stream;

use super::tests::append_entry;
use super::*;
use crate::runtime::dice::source_service::SourceServiceError;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::dice::source_service::SourceServiceScope;
use crate::runtime::repository_io::ArchiveFailureStage;
use crate::runtime::repository_physical_root::RepositoryRootResourceLimits;
use crate::runtime::repository_physical_root::RepositoryRootResources;
use crate::runtime::repository_physical_root::ResourceCredit;

const BANK_BYTES: usize = 16 * 1024 * 1024;
const HELD_BYTES: usize = 4 * 1024 * 1024;
const ALLOWANCE: usize = 136 * 1024 * 1024;
const CONTENT: &[u8] = b"real xz output under a shared finite native byte bank\n";

struct HeldBody {
    body: Vec<u8>,
    // Payload-before-credit order, including refusal/cancellation unwinding.
    _credit: ResourceCredit,
}
impl HeldBody {
    fn new(resources: &Arc<RepositoryRootResources>) -> Self {
        let bytes = HELD_BYTES
            .checked_add(std::mem::size_of::<Self>())
            .unwrap()
            .checked_add(64)
            .unwrap();
        let credit = resources.reserve(bytes, 0, 0).unwrap();
        let mut body = Vec::new();
        body.try_reserve_exact(HELD_BYTES).unwrap();
        assert!(body.capacity() <= HELD_BYTES);
        body.resize(HELD_BYTES, 0x5a);
        Self {
            body,
            _credit: credit,
        }
    }
    fn check(&self) {
        assert_eq!(self.body.len(), HELD_BYTES);
        assert_eq!(self.body.first(), Some(&0x5a));
        assert_eq!(self.body.last(), Some(&0x5a));
    }
}
fn bank() -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(RepositoryRootResourceLimits::new(BANK_BYTES, 16, 1).unwrap())
        .unwrap()
}
fn scope(
    resources: &Arc<RepositoryRootResources>,
    decoder_allowance: usize,
) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(5);
    SourceServiceScope::new_command(
        NormalizedAbsolutePath::new("/xz-allocation-fixture").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits {
            live_decoder_bytes: decoder_allowance,
            ..SourceServiceLimits::default()
        },
        resources.clone(),
    )
    .unwrap()
}
fn archive(dictionary: u32) -> (tempfile::NamedTempFile, u64) {
    let mut tar = tar::Builder::new(Vec::new());
    append_entry(
        &mut tar,
        "prefix/file",
        tar::EntryType::Regular,
        0o755,
        CONTENT,
    );
    let bytes = tar.into_inner().unwrap();
    let decoded = bytes.len() as u64;
    let mut options = LzmaOptions::new_preset(1).unwrap();
    options.dict_size(dictionary);
    let mut filters = Filters::new();
    filters.lzma2(&options);
    let stream = Stream::new_stream_encoder(&filters, Check::Crc64).unwrap();
    let mut capture = tempfile::NamedTempFile::new().unwrap();
    {
        let mut encoder = xz2::write::XzEncoder::new_stream(capture.as_file_mut(), stream);
        encoder.write_all(&bytes).unwrap();
        encoder.finish().unwrap();
    }
    capture.flush().unwrap();
    (capture, decoded)
}
fn assert_baseline(resources: &RepositoryRootResources, retained: usize) {
    let now = resources.snapshot();
    assert_eq!(
        (now.retained_bytes, now.native_fds, now.backing_bytes),
        (retained, 0, 0)
    );
}
fn assert_output(root: &std::path::Path) {
    let file = root.join("file");
    assert_eq!(std::fs::read(&file).unwrap(), CONTENT);
    assert_eq!(
        std::fs::metadata(file).unwrap().permissions().mode() & 0o7777,
        0o755
    );
}

#[test]
fn genuine_small_xz_dictionary_fits_shared_native_bank_with_retained_body() {
    let (capture, expected_decoded) = archive(1024 * 1024);
    let root = tempfile::tempdir().unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let held = HeldBody::new(&resources);
    let with_body = resources.snapshot().retained_bytes;
    let scope = scope(&resources, ALLOWANCE);
    assert!(Arc::ptr_eq(scope.command_resources().unwrap(), &resources));
    let mut decoded = 0;
    // Causal RED on the former flat136MiB native debit: this real small
    // dictionary needs to fit alongside the positively retained4MiB payload.
    extract_generated_archive(
        capture.as_file(),
        root.path(),
        "prefix",
        GeneratedRepositoryArchiveFormat::TarXz,
        &|| true,
        &mut decoded,
        Some(&scope),
    )
    .unwrap();
    assert_output(root.path());
    assert_eq!(decoded, expected_decoded);
    let counts = scope.snapshot();
    assert_eq!(counts.decoded_bytes, expected_decoded);
    assert_eq!(counts.written_bytes, CONTENT.len() as u64);
    assert_eq!((counts.source_attempts, counts.transferred_bytes), (0, 0));
    assert_eq!(
        (counts.live_decoder_bytes, counts.live_buffer_bytes),
        (0, 0)
    );
    assert_eq!(counts.peak_decoder_bytes, ALLOWANCE);
    assert!(scope.check().is_ok());
    held.check();
    drop(scope);
    assert_baseline(&resources, with_body);
    drop(held);
    assert_baseline(&resources, baseline);
}

#[test]
fn genuine_larger_xz_dictionary_and_source_allowance_refuse_distinct_limits() {
    let (capture, expected_decoded) = archive(16 * 1024 * 1024);
    // Prove the larger dictionary stream is genuine and below the unchanged
    // codec ceiling, independently of native admission. No command bank exists
    // for this ordinary unbound Source scope.
    let deadline = Instant::now() + Duration::from_secs(5);
    let unbound = SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/xz-allocation-fixture").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap();
    let control_root = tempfile::tempdir().unwrap();
    let mut decoded = 0;
    extract_generated_archive(
        capture.as_file(),
        control_root.path(),
        "prefix",
        GeneratedRepositoryArchiveFormat::TarXz,
        &|| true,
        &mut decoded,
        Some(&unbound),
    )
    .unwrap();
    assert_eq!(decoded, expected_decoded);
    assert_output(control_root.path());
    assert_eq!(
        (
            unbound.snapshot().live_decoder_bytes,
            unbound.snapshot().peak_decoder_bytes
        ),
        (0, ALLOWANCE)
    );
    drop(unbound);
    for (limit, expected) in [
        (ALLOWANCE, SourceServiceError::Allocation),
        (1024, SourceServiceError::DecoderLimit),
    ] {
        let resources = bank();
        let baseline = resources.snapshot().retained_bytes;
        let held = HeldBody::new(&resources);
        let with_body = resources.snapshot().retained_bytes;
        let scope = scope(&resources, limit);
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("file"), b"previous").unwrap();
        let mut decoded = 0;
        let error = extract_generated_archive(
            capture.as_file(),
            root.path(),
            "prefix",
            GeneratedRepositoryArchiveFormat::TarXz,
            &|| true,
            &mut decoded,
            Some(&scope),
        )
        .unwrap_err();
        assert_eq!(error.stage, ArchiveFailureStage::Materialization);
        // Typed sticky cause, never an inference from a formatted codec error.
        assert_eq!(scope.check(), Err(expected));
        assert_eq!(decoded, 0);
        assert_eq!(
            std::fs::read(root.path().join("file")).unwrap(),
            b"previous"
        );
        let counts = scope.snapshot();
        assert_eq!((counts.decoded_bytes, counts.written_bytes), (0, 0));
        assert_eq!(
            (counts.live_decoder_bytes, counts.live_buffer_bytes),
            (0, 0)
        );
        assert_eq!(
            counts.peak_decoder_bytes,
            if limit == ALLOWANCE { ALLOWANCE } else { 0 }
        );
        held.check();
        drop(scope);
        assert_baseline(&resources, with_body);
        drop(held);
        assert_baseline(&resources, baseline);
    }
}
