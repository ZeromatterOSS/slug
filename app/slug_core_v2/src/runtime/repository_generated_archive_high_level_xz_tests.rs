//! Exercise the real high-level fallback reader on Linux.
//! This creates no Source/native authority and does not replace allocator tests.
#![cfg(target_os = "linux")]

use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;

use xz2::read::XzDecoder;
use xz2::stream::CONCATENATED;
use xz2::stream::Stream;

use super::tests::append_entry;
use super::*;

const CONTENT: &[u8] = b"high-level fallback preserves the complete authored file\n";

fn authored_tar() -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    append_entry(
        &mut tar,
        "prefix/nested/tool",
        tar::EntryType::Regular,
        0o755,
        CONTENT,
    );
    tar.into_inner().unwrap()
}

fn encode(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 1);
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn decoder(bytes: Vec<u8>) -> XzDecoder<Cursor<Vec<u8>>> {
    let stream = Stream::new_stream_decoder(XZ_DECODER_MEMORY_LIMIT, CONCATENATED).unwrap();
    XzDecoder::new_stream(Cursor::new(bytes), stream)
}

#[test]
fn high_level_xz_extracts_concatenated_padded_tar_and_repeats_eof_and_empty_reads() {
    let tar = authored_tar();
    // Split within the file payload, not at a tar or stream boundary supplied
    // by the consumer. Each half is a separately finished XZ stream.
    let split = 512 + CONTENT.len() / 2;
    assert!(split < tar.len());
    let mut compressed = encode(&tar[..split]);
    compressed.extend_from_slice(&[0; 4]);
    compressed.extend(encode(&tar[split..]));
    compressed.extend_from_slice(&[0; 8]);
    let compressed_len = compressed.len() as u64;
    let mut decoder = decoder(compressed);
    assert_eq!(decoder.read(&mut []).unwrap(), 0);
    assert_eq!(decoder.total_out(), 0);
    let root = tempfile::tempdir().unwrap();
    let mut decoded = 0;
    extract_tar(
        &mut decoder,
        &PathDestination(root.path()),
        "prefix",
        &|| true,
        &mut decoded,
        GENERATED_DECODE_LIMITS,
        None,
    )
    .unwrap();
    assert_eq!(decoded, tar.len() as u64);
    assert_eq!(decoder.total_out(), tar.len() as u64);
    assert_eq!(decoder.total_in(), compressed_len);
    let path = root.path().join("nested/tool");
    assert_eq!(std::fs::read(&path).unwrap(), CONTENT);
    let metadata = std::fs::metadata(&path).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o7777, 0o755);
    assert_eq!(metadata.mtime(), 1_600_000_000);
    assert_eq!(metadata.mtime_nsec(), 0);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    assert_eq!(
        std::fs::read_dir(root.path().join("nested"))
            .unwrap()
            .count(),
        1
    );
    for _ in 0..3 {
        assert_eq!(decoder.read(&mut [0; 1]).unwrap(), 0);
        assert_eq!(decoder.read(&mut []).unwrap(), 0);
    }
    assert_eq!(decoder.total_out(), tar.len() as u64);
    assert_eq!(decoder.total_in(), compressed_len);
    // Empty decoded XZ also exercises the actual high-level EOF path; it is
    // not a valid tar and is deliberately not passed off as an archive.
    let mut empty = self::decoder(encode(&[]));
    assert_eq!(empty.read(&mut []).unwrap(), 0);
    for _ in 0..3 {
        assert_eq!(empty.read(&mut [0; 1]).unwrap(), 0);
        assert_eq!(empty.read(&mut []).unwrap(), 0);
    }
    assert_eq!(empty.total_out(), 0);
}

#[test]
fn high_level_xz_tar_extraction_refuses_late_footer_truncation_and_trailing_data() {
    let tar = authored_tar();
    let valid = encode(&tar);
    assert!(valid.len() > 12);
    let mut footer = valid.clone();
    let last = footer.len() - 1;
    footer[last] ^= 1;
    let truncated = valid[..valid.len() - 5].to_vec();
    let mut junk = valid.clone();
    junk.extend_from_slice(b"not another XZ stream");
    let mut incomplete_padding = valid;
    incomplete_padding.push(0);
    for compressed in [footer, truncated, junk, incomplete_padding] {
        let root = tempfile::tempdir().unwrap();
        let mut decoded = 0;
        // Extraction is unpublished staging work: late failure may have
        // written the file, but must never return successful completion.
        assert!(
            extract_tar(
                decoder(compressed),
                &PathDestination(root.path()),
                "prefix",
                &|| true,
                &mut decoded,
                GENERATED_DECODE_LIMITS,
                None,
            )
            .is_err()
        );
        assert!(decoded <= tar.len() as u64);
    }
}
