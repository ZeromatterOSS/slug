use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use std::time::UNIX_EPOCH;

use super::extract_selected_generated_archive;

fn append_entry(
    builder: &mut tar::Builder<Vec<u8>>,
    path: &str,
    entry_type: tar::EntryType,
    mode: u32,
    bytes: &[u8],
) {
    let mut header = tar::Header::new_gnu();
    header.set_path(path).unwrap();
    header.set_entry_type(entry_type);
    header.set_mode(mode);
    header.set_mtime(1_600_000_000);
    header.set_size(bytes.len() as u64);
    header.set_cksum();
    builder.append(&header, bytes).unwrap();
}

fn archive(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> tempfile::NamedTempFile {
    let mut builder = tar::Builder::new(Vec::new());
    build(&mut builder);
    let tar = builder.into_inner().unwrap();
    let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 1);
    encoder.write_all(&tar).unwrap();
    let compressed = encoder.finish().unwrap();
    let mut capture = tempfile::NamedTempFile::new().unwrap();
    capture.write_all(&compressed).unwrap();
    capture
}

fn extract(
    capture: &tempfile::NamedTempFile,
    root: &Path,
    strip_prefix: &str,
    aggregate: &mut u64,
) -> Result<(), super::ArchiveMaterializationError> {
    extract_selected_generated_archive(capture.as_file(), root, strip_prefix, &|| true, aggregate)
}

#[test]
fn ordered_archives_replace_regular_files_and_preserve_executable_mode_and_mtime() {
    let first = archive(|tar| {
        append_entry(
            tar,
            "a/component/shared.txt",
            tar::EntryType::Regular,
            0o644,
            b"first\n",
        );
        append_entry(
            tar,
            "a/component/only-a.txt",
            tar::EntryType::Regular,
            0o644,
            b"only a\n",
        );
        append_entry(
            tar,
            "a/ignored.txt",
            tar::EntryType::Regular,
            0o644,
            b"ignored\n",
        );
    });
    let second = archive(|tar| {
        append_entry(
            tar,
            "b/component/shared.txt",
            tar::EntryType::Regular,
            0o755,
            b"second\n",
        );
        append_entry(
            tar,
            "b/component/tool.sh",
            tar::EntryType::Regular,
            0o755,
            b"#!/bin/sh\n",
        );
    });
    let root = tempfile::tempdir().unwrap();
    let mut aggregate = 0;
    extract(&first, root.path(), "a/component", &mut aggregate).unwrap();
    let after_first = aggregate;
    extract(&second, root.path(), "b/component", &mut aggregate).unwrap();
    assert!(after_first > 1024 && aggregate > after_first);
    assert_eq!(
        fs::read(root.path().join("shared.txt")).unwrap(),
        b"second\n"
    );
    assert_eq!(
        fs::read(root.path().join("only-a.txt")).unwrap(),
        b"only a\n"
    );
    assert!(!root.path().join("ignored.txt").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(root.path().join("shared.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        assert_eq!(
            fs::metadata(root.path().join("tool.sh"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }
    assert_eq!(
        fs::metadata(root.path().join("shared.txt"))
            .unwrap()
            .modified()
            .unwrap(),
        UNIX_EPOCH + Duration::from_secs(1_600_000_000)
    );
}

#[test]
fn gnu_long_name_header_is_applied_to_following_regular_entry() {
    let long_name = format!("long_{}.txt", "x".repeat(104));
    let archive_path = format!("component/{long_name}");
    let capture = archive(|tar| {
        let mut payload = archive_path.as_bytes().to_vec();
        payload.push(0);
        append_entry(
            tar,
            "././@LongLink",
            tar::EntryType::new(b'L'),
            0o644,
            &payload,
        );
        append_entry(
            tar,
            "component/placeholder",
            tar::EntryType::Regular,
            0o644,
            b"long payload\n",
        );
    });
    let root = tempfile::tempdir().unwrap();
    extract(&capture, root.path(), "component", &mut 0).unwrap();
    assert_eq!(
        fs::read(root.path().join(long_name)).unwrap(),
        b"long payload\n"
    );
    assert!(!root.path().join("placeholder").exists());
}

#[test]
fn path_escape_links_and_file_directory_collisions_fail_closed() {
    let traversal = archive(|tar| {
        // tar::Header::set_path correctly refuses "..", so write the raw name
        // field to exercise our extractor's independent confinement check.
        let mut header = tar::Header::new_gnu();
        header.set_path("component/escape").unwrap();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_mtime(1_600_000_000);
        header.set_size(3);
        let name = b"component/../escape";
        header.as_mut_bytes()[..100].fill(0);
        header.as_mut_bytes()[..name.len()].copy_from_slice(name);
        header.set_cksum();
        tar.append(&header, &b"bad"[..]).unwrap();
    });
    let root = tempfile::tempdir().unwrap();
    let error = extract(&traversal, root.path(), "component", &mut 0).unwrap_err();
    assert!(error.message.contains("not normalized"));
    assert!(!root.path().join("escape").exists());

    let link = archive(|tar| {
        append_entry(tar, "component/link", tar::EntryType::Symlink, 0o755, b"");
    });
    let error = extract(&link, root.path(), "component", &mut 0).unwrap_err();
    assert!(error.message.contains("unsupported entry type"));

    let collision = archive(|tar| {
        append_entry(
            tar,
            "component/node",
            tar::EntryType::Regular,
            0o644,
            b"file",
        );
        append_entry(
            tar,
            "component/node/child",
            tar::EntryType::Regular,
            0o644,
            b"child",
        );
    });
    let error = extract(&collision, root.path(), "component", &mut 0).unwrap_err();
    assert!(error.message.contains("file/directory collision"));
}

#[test]
fn aggregate_cap_and_cancellation_stop_extraction() {
    let capture = archive(|tar| {
        append_entry(
            tar,
            "component/file",
            tar::EntryType::Regular,
            0o644,
            b"payload",
        );
    });
    let root = tempfile::tempdir().unwrap();
    let mut near_cap = 1024 * 1024 * 1024 - 512;
    let error = extract(&capture, root.path(), "component", &mut near_cap).unwrap_err();
    assert!(error.message.contains("aggregate decoded limit"));
    let error = extract_selected_generated_archive(
        capture.as_file(),
        root.path(),
        "component",
        &|| false,
        &mut 0,
    )
    .unwrap_err();
    assert!(error.message.contains("no longer active"));
}

#[test]
fn within_archive_duplicate_destination_is_rejected_but_prior_archive_is_replaceable() {
    let first = archive(|tar| {
        append_entry(
            tar,
            "a/component/shared",
            tar::EntryType::Regular,
            0o644,
            b"first",
        );
    });
    let duplicate = archive(|tar| {
        append_entry(
            tar,
            "b/component/shared",
            tar::EntryType::Regular,
            0o644,
            b"second",
        );
        append_entry(
            tar,
            "b/component/shared",
            tar::EntryType::Regular,
            0o644,
            b"third",
        );
    });
    let root = tempfile::tempdir().unwrap();
    let mut aggregate = 0;
    extract(&first, root.path(), "a/component", &mut aggregate).unwrap();
    let error = extract(&duplicate, root.path(), "b/component", &mut aggregate).unwrap_err();
    assert!(error.message.contains("repeats a stripped destination"));
    // The enclosing materializer owns root discard on error; this direct
    // extractor test only verifies that prior-call overwrite was admitted.
    assert_eq!(fs::read(root.path().join("shared")).unwrap(), b"second");
}

#[test]
fn per_call_decoded_cap_counts_tar_headers_before_payload() {
    let capture = archive(|tar| {
        append_entry(
            tar,
            "component/file",
            tar::EntryType::Regular,
            0o644,
            b"payload",
        );
    });
    let root = tempfile::tempdir().unwrap();
    let mut aggregate = 0;
    let error = super::extract_with_limits(
        capture.as_file(),
        root.path(),
        "component",
        &|| true,
        &mut aggregate,
        super::DecodeLimits {
            per_call: 512,
            aggregate: super::AGGREGATE_DECODED_LIMIT,
        },
    )
    .unwrap_err();
    assert!(error.message.contains("per-call decoded limit"));
    assert_eq!(aggregate, 512);
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

#[test]
fn oversized_xz_dictionary_is_rejected_before_extracting_a_tar_header() {
    let ordinary = archive(|tar| {
        append_entry(
            tar,
            "component/file",
            tar::EntryType::Regular,
            0o644,
            b"payload",
        );
    });
    let mut compressed = fs::read(ordinary.path()).unwrap();
    // An xz2 level-1 encoder emits a one-filter LZMA2 block header here.
    // Change only its dictionary property to 0x20 (256 MiB) and repair the
    // block-header CRC32; no large dictionary is allocated by this test.
    assert_eq!(&compressed[..6], b"\xfd7zXZ\0");
    let header_len = (usize::from(compressed[12]) + 1) * 4;
    assert!(header_len >= 12);
    assert_eq!(&compressed[14..16], &[0x21, 0x01]);
    compressed[16] = 0x20;
    let checksum_start = 12 + header_len - 4;
    let checksum = crc32(&compressed[12..checksum_start]);
    compressed[checksum_start..checksum_start + 4].copy_from_slice(&checksum.to_le_bytes());

    let mut oversized = tempfile::NamedTempFile::new().unwrap();
    oversized.write_all(&compressed).unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut aggregate = 0;
    assert_eq!(super::XZ_DECODER_MEMORY_LIMIT, 136 * 1024 * 1024);
    let error = extract(&oversized, root.path(), "component", &mut aggregate).unwrap_err();
    assert!(error.message.contains("memory limit reached"), "{error:?}");
    assert_eq!(aggregate, 0);
    assert!(!root.path().join("file").exists());
}
