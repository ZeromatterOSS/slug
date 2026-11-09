use std::io::Cursor;
use std::io::Write;
use std::time::Duration;
use std::time::Instant;

use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::repository_physical_root::RepositoryRootResourceLimits;
use crate::runtime::repository_physical_root::RepositoryRootResources;

fn encoded(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 1);
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}
fn scope() -> Arc<SourceServiceScope> {
    let now = Instant::now();
    SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/xz-kernel").unwrap(),
        now + Duration::from_secs(5),
        now + Duration::from_secs(6),
        SourceServiceLimits::default(),
    )
    .unwrap()
}
fn bank() -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(RepositoryRootResourceLimits::new(1024 * 1024, 8, 1).unwrap())
        .unwrap()
}
fn command(bank: &Arc<RepositoryRootResources>) -> Arc<SourceServiceScope> {
    let now = Instant::now();
    SourceServiceScope::new_command(
        NormalizedAbsolutePath::new("/xz-kernel").unwrap(),
        now + Duration::from_secs(5),
        now + Duration::from_secs(6),
        SourceServiceLimits::default(),
        bank.clone(),
    )
    .unwrap()
}

#[test]
fn xz_concatenation_padding_footer_and_repeated_eof_match_stream_contract() {
    let mut compressed = encoded(b"one");
    compressed.extend_from_slice(&[0; 4]);
    compressed.extend(encoded(b"two"));
    compressed.extend_from_slice(&[0; 8]);
    let scope = scope();
    let mut reader = SourceXzDecoder::new(Cursor::new(compressed), Some(&scope)).unwrap();
    assert_eq!(scope.snapshot().live_decoder_bytes, 136 << 20);
    assert_eq!(reader.read(&mut []).unwrap(), 0);
    let mut result = Vec::new();
    let mut buffer = [0; 2];
    loop {
        let count = reader.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        result.extend_from_slice(&buffer[..count]);
    }
    assert_eq!(result, b"onetwo");
    for _ in 0..3 {
        assert_eq!(reader.read(&mut buffer).unwrap(), 0);
        assert_eq!(reader.read(&mut []).unwrap(), 0);
    }
    assert!(reader.stream.next_in.is_null() && reader.stream.next_out.is_null());
    assert_eq!(reader.stream.avail_in, 0);
    assert_eq!(reader.stream.avail_out, 0);
    drop(reader);
    assert_eq!(scope.snapshot().live_decoder_bytes, 0);
    assert_eq!(scope.snapshot().peak_decoder_bytes, 136 << 20);
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        scope.snapshot().decoded_bytes,
        0,
        "outer counted reader owns decoded history"
    );
    assert!(scope.check().is_ok());
}

#[test]
fn xz_rejects_invalid_late_corruption_truncation_and_trailing_junk_without_source_guessing() {
    let valid = encoded(b"payload payload");
    let mut corrupt = valid.clone();
    let index = corrupt.len() - 8;
    corrupt[index] ^= 1;
    let mut junk = valid.clone();
    junk.extend_from_slice(b"junk");
    for input in [
        b"invalid xz".to_vec(),
        corrupt,
        valid[..valid.len() - 5].to_vec(),
        junk,
    ] {
        let scope = scope();
        let mut reader = SourceXzDecoder::new(Cursor::new(input), Some(&scope)).unwrap();
        assert!(reader.read_to_end(&mut Vec::new()).is_err());
        assert!(reader.stream.next_in.is_null() && reader.stream.next_out.is_null());
        assert!(
            scope.check().is_ok(),
            "intrinsic codec failures do not fabricate Source allocation errors"
        );
        drop(reader);
        assert_eq!(scope.snapshot().live_decoder_bytes, 0);
    }
}

#[test]
fn xz_scope_refusal_and_partial_init_or_decode_allocation_cleanup_are_exact() {
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let scope = command(&resources);
    let before = resources.snapshot().retained_bytes;
    type Reader = SourceXzDecoder<Cursor<Vec<u8>>>;
    let control = size_of::<Reader>()
        + AllocatorOwner::context_bytes()
        + size_of::<SourceDecoderReservation>();
    let held = resources
        .reserve(1024 * 1024 - before - control - 1, 0, 0)
        .unwrap();
    let held_bytes = resources.snapshot().retained_bytes;
    assert!(Reader::new(Cursor::new(encoded(b"payload")), Some(&scope)).is_err());
    assert_eq!(scope.check(), Err(SourceServiceError::Allocation));
    assert_eq!(scope.snapshot().live_decoder_bytes, 0);
    assert_eq!(resources.snapshot().retained_bytes, held_bytes);
    drop((held, scope));
    assert_eq!(resources.snapshot().retained_bytes, baseline);

    // Initialization fits; the genuine level-1 dictionary cannot coexist in
    // this unchanged tiny bank. End must free its earlier successful blocks.
    let scope = command(&resources);
    let mut reader = Reader::new(Cursor::new(encoded(b"payload")), Some(&scope)).unwrap();
    assert!(reader.read_to_end(&mut Vec::new()).is_err());
    assert_eq!(scope.check(), Err(SourceServiceError::Allocation));
    drop(reader);
    assert_eq!(scope.snapshot().live_decoder_bytes, 0);
    drop(scope);
    assert_eq!(resources.snapshot().retained_bytes, baseline);
    assert_eq!(resources.snapshot().native_fds, 0);
    assert_eq!(resources.snapshot().backing_bytes, 0);
}

#[test]
fn xz_logical_allowance_and_preexisting_cancel_remain_sticky_and_retire() {
    let scope = scope();
    let generic = scope.reserve_decoder(1).unwrap();
    assert!(SourceXzDecoder::new(Cursor::new(encoded(b"data")), Some(&scope)).is_err());
    assert_eq!(scope.check(), Err(SourceServiceError::DecoderLimit));
    assert_eq!(scope.snapshot().live_decoder_bytes, 1);
    drop(generic);
    assert_eq!(scope.snapshot().live_decoder_bytes, 0);

    let scope = self::scope();
    let mut reader = SourceXzDecoder::new(Cursor::new(encoded(b"data")), Some(&scope)).unwrap();
    scope.cancel();
    assert!(reader.read(&mut [0; 1]).is_err());
    // An earlier Source refusal outranks a later local callback denial.
    assert_eq!(
        publish(Some(&scope), Failure::Allocation)
            .get_ref()
            .unwrap()
            .downcast_ref::<SourceServiceError>(),
        Some(&SourceServiceError::Cancelled)
    );
    assert_eq!(scope.check(), Err(SourceServiceError::Cancelled));
    drop(reader);
    assert_eq!(scope.snapshot().live_decoder_bytes, 0);
}

#[test]
fn xz_interrupted_refill_cannot_replay_previously_consumed_input() {
    struct InterruptedAfterFirst {
        reader: Cursor<Vec<u8>>,
        reads: usize,
    }
    impl Read for InterruptedAfterFirst {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            self.reads += 1;
            if self.reads == 2 {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let limit = out.len().min(13);
            self.reader.read(&mut out[..limit])
        }
    }
    let payload = b"exact complete output after interrupted compressed refill";
    let mut reader = SourceXzDecoder::new(
        InterruptedAfterFirst {
            reader: Cursor::new(encoded(payload)),
            reads: 0,
        },
        None,
    )
    .unwrap();
    let mut output = vec![0; payload.len()];
    reader.read_exact(&mut output).unwrap();
    assert_eq!(output, payload);
    assert_eq!(reader.read(&mut [0; 1]).unwrap(), 0);
    assert!(reader.reader.reads > 2);
}

#[test]
fn xz_deadline_refuses_before_next_codec_call_and_still_frees_allocations() {
    let now = Instant::now();
    let scope = SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/xz-kernel").unwrap(),
        now + Duration::from_millis(100),
        now + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap();
    let mut reader = SourceXzDecoder::new(Cursor::new(encoded(b"data")), Some(&scope)).unwrap();
    std::thread::sleep(Duration::from_millis(120));
    assert!(reader.read(&mut [0; 1]).is_err());
    assert_eq!(scope.check(), Err(SourceServiceError::WorkDeadline));
    drop(reader);
    assert_eq!(scope.snapshot().live_decoder_bytes, 0);
}
