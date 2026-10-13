use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use std::time::UNIX_EPOCH;

use slug_bzlmod_v2::GeneratedRepositoryArchiveFormat;

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
    archive_with_format(GeneratedRepositoryArchiveFormat::TarXz, build)
}

fn tar_bytes(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    build(&mut builder);
    builder.into_inner().unwrap()
}

fn gzip_parts(parts: &[&[u8]]) -> Vec<u8> {
    let mut compressed = Vec::new();
    for part in parts {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(part).unwrap();
        compressed.extend(encoder.finish().unwrap());
    }
    compressed
}

fn capture_bytes(compressed: &[u8]) -> tempfile::NamedTempFile {
    let mut capture = tempfile::NamedTempFile::new().unwrap();
    capture.write_all(compressed).unwrap();
    capture
}

fn archive_with_format(
    format: GeneratedRepositoryArchiveFormat,
    build: impl FnOnce(&mut tar::Builder<Vec<u8>>),
) -> tempfile::NamedTempFile {
    let tar = tar_bytes(build);
    let compressed = match format {
        GeneratedRepositoryArchiveFormat::TarXz => {
            let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 1);
            encoder.write_all(&tar).unwrap();
            encoder.finish().unwrap()
        }
        GeneratedRepositoryArchiveFormat::TarGz => gzip_parts(&[&tar]),
    };
    capture_bytes(&compressed)
}

fn extract(
    capture: &tempfile::NamedTempFile,
    root: &Path,
    strip_prefix: &str,
    aggregate: &mut u64,
) -> Result<(), super::ArchiveMaterializationError> {
    extract_selected_generated_archive(
        capture.as_file(),
        root,
        strip_prefix,
        &|| true,
        aggregate,
        None,
    )
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
        None,
    )
    .unwrap_err();
    assert!(error.message.contains("no longer active"));
}

#[test]
fn payload_padding_quota_refusal_preserves_existing_files_and_counts_discarded_entries() {
    let capture = archive(|tar| {
        append_entry(
            tar,
            "component/file",
            tar::EntryType::Regular,
            0o644,
            &[b'x'; 513],
        );
    });
    for (prefix, limits, expected) in [
        (
            "component",
            super::DecodeLimits {
                per_call: 1025,
                aggregate: 4096,
            },
            "per-call decoded limit",
        ),
        (
            "component",
            super::DecodeLimits {
                per_call: 4096,
                aggregate: 1025,
            },
            "aggregate decoded limit",
        ),
        (
            "unrelated",
            super::DecodeLimits {
                per_call: 1025,
                aggregate: 4096,
            },
            "per-call decoded limit",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("file"), b"previous bytes").unwrap();
        let mut decoded = 0;
        let error = super::extract_with_limits(
            capture.as_file(),
            root.path(),
            prefix,
            &|| true,
            &mut decoded,
            limits,
            None,
        )
        .unwrap_err();
        assert!(error.message.contains(expected), "{}", error.message);
        assert_eq!(
            fs::read(root.path().join("file")).unwrap(),
            b"previous bytes"
        );
        // The payload itself fits the 513-byte remainder; its physical padding
        // does not. Admission refuses before reading or replacing any payload.
        assert_eq!(decoded, 512);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
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
        None,
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

#[test]
fn scoped_generated_archives_count_ordered_replacements_and_all_decoded_bytes() {
    use std::time::Instant;

    use slug_workspace_v2::NormalizedAbsolutePath;

    use crate::runtime::dice::source_service::SourceServiceLimits;
    use crate::runtime::dice::source_service::SourceServiceScope;
    let first = archive(|tar| append_entry(tar, "a/file", tar::EntryType::Regular, 0o644, b"one"));
    let second = archive(|tar| append_entry(tar, "b/file", tar::EntryType::Regular, 0o755, b"two"));
    for cap in [6, 5] {
        let work = Instant::now() + Duration::from_secs(2);
        let scope = SourceServiceScope::new_for_test(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            work,
            work + Duration::from_secs(1),
            SourceServiceLimits {
                written_bytes: cap,
                ..SourceServiceLimits::default()
            },
        )
        .unwrap();
        let root = tempfile::tempdir().unwrap();
        let mut aggregate = 0;
        extract_selected_generated_archive(
            first.as_file(),
            root.path(),
            "a",
            &|| true,
            &mut aggregate,
            Some(&scope),
        )
        .unwrap();
        let result = extract_selected_generated_archive(
            second.as_file(),
            root.path(),
            "b",
            &|| true,
            &mut aggregate,
            Some(&scope),
        );
        assert_eq!(result.is_ok(), cap == 6);
        assert_eq!(scope.snapshot().decoded_bytes, aggregate);
        assert_eq!(scope.snapshot().live_decoder_bytes, 0);
        assert_eq!(scope.snapshot().peak_decoder_bytes, 136 << 20);
        assert!(
            aggregate >= 4 * 512,
            "physical headers and end markers cannot be skipped"
        );
        if cap == 6 {
            assert_eq!(scope.snapshot().written_bytes, 6);
            assert_eq!(fs::read(root.path().join("file")).unwrap(), b"two");
        } else {
            assert!(scope.check().is_err());
        }
    }
}

#[test]
fn scoped_xz_reserves_before_decoding_and_retires_on_codec_failure_and_cancellation() {
    use std::time::Instant;

    use crate::runtime::dice::source_service::SourceServiceLimits;
    use crate::runtime::dice::source_service::SourceServiceScope;
    let valid = archive(|tar| append_entry(tar, "a/file", tar::EntryType::Regular, 0o644, b"new"));
    let invalid = capture_bytes(b"invalid xz bytes");
    for case in ["tiny", "invalid", "cancel"] {
        let deadline = Instant::now() + Duration::from_secs(5);
        let scope = SourceServiceScope::new_for_test(
            slug_workspace_v2::NormalizedAbsolutePath::new("/workspace").unwrap(),
            deadline,
            deadline + Duration::from_secs(1),
            SourceServiceLimits {
                live_decoder_bytes: if case == "tiny" { 1024 } else { 136 << 20 },
                ..SourceServiceLimits::default()
            },
        )
        .unwrap();
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("file"), b"previous").unwrap();
        let active = || {
            if case == "cancel" && scope.snapshot().live_decoder_bytes != 0 {
                scope.cancel();
            }
            true
        };
        let mut decoded = 0;
        let capture = if case == "invalid" { &invalid } else { &valid };
        let error = extract_selected_generated_archive(
            capture.as_file(),
            root.path(),
            "a",
            &active,
            &mut decoded,
            Some(&scope),
        )
        .unwrap_err();
        if case == "tiny" {
            assert!(error.message.contains("DecoderLimit"), "{}", error.message);
        } else if case == "cancel" {
            assert!(error.message.contains("Cancelled"), "{}", error.message);
        }
        assert_eq!(decoded, 0);
        assert_eq!(fs::read(root.path().join("file")).unwrap(), b"previous");
        assert_eq!(scope.snapshot().live_decoder_bytes, 0);
        assert_eq!(
            scope.snapshot().peak_decoder_bytes,
            if case == "tiny" { 0 } else { 136 << 20 }
        );
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        assert_eq!(
            scope.snapshot().peak_buffer_bytes,
            if case == "tiny" { 0 } else { 64 * 1024 + 1024 }
        );
        assert_eq!(scope.limits().live_buffer_bytes, 16 << 20);
    }
}

#[test]
fn general_tar_formats_share_empty_prefix_ordering_caps_and_path_rejection() {
    for format in [
        GeneratedRepositoryArchiveFormat::TarXz,
        GeneratedRepositoryArchiveFormat::TarGz,
    ] {
        let capture = archive_with_format(format, |tar| {
            append_entry(
                tar,
                "component/file",
                tar::EntryType::Regular,
                0o755,
                b"archive",
            );
        });
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("component")).unwrap();
        fs::write(root.path().join("component/file"), b"before").unwrap();
        let mut aggregate = 0;
        super::extract_generated_archive(
            capture.as_file(),
            root.path(),
            "",
            format,
            &|| true,
            &mut aggregate,
            None,
        )
        .unwrap();
        assert_eq!(
            fs::read(root.path().join("component/file")).unwrap(),
            b"archive"
        );
        assert!(aggregate >= 4 * 512);
        fs::write(root.path().join("component/file"), b"after").unwrap();
        assert_eq!(
            fs::read(root.path().join("component/file")).unwrap(),
            b"after"
        );
        let error = super::extract_with_format_and_limits(
            capture.as_file(),
            root.path(),
            "",
            format,
            &|| true,
            &mut 0,
            super::DecodeLimits {
                per_call: 512,
                aggregate: super::AGGREGATE_DECODED_LIMIT,
            },
            None,
        )
        .unwrap_err();
        assert!(
            error.message.contains("per-call decoded limit"),
            "{format:?}: {error:?}"
        );
        let error = super::extract_generated_archive(
            capture.as_file(),
            root.path(),
            "",
            format,
            &|| false,
            &mut 0,
            None,
        )
        .unwrap_err();
        assert!(error.message.contains("no longer active"));
        let link = archive_with_format(format, |tar| {
            append_entry(tar, "link", tar::EntryType::Symlink, 0o755, b"");
        });
        let error = super::extract_generated_archive(
            link.as_file(),
            root.path(),
            "",
            format,
            &|| true,
            &mut 0,
            None,
        )
        .unwrap_err();
        assert!(error.message.contains("unsupported entry type"));
        let escape = archive_with_format(format, |tar| {
            let mut header = tar::Header::new_gnu();
            header.set_path("escape").unwrap();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_mode(0o644);
            header.set_mtime(1_600_000_000);
            header.set_size(1);
            header.as_mut_bytes()[..100].fill(0);
            header.as_mut_bytes()[..8].copy_from_slice(b"../owned");
            header.set_cksum();
            tar.append(&header, &b"x"[..]).unwrap();
        });
        let error = super::extract_generated_archive(
            escape.as_file(),
            root.path(),
            "",
            format,
            &|| true,
            &mut 0,
            None,
        )
        .unwrap_err();
        assert!(error.message.contains("not normalized"));
    }
}

#[test]
fn gzip_consumes_concatenated_members_and_validates_trailers_before_success() {
    let tar = tar_bytes(|tar| {
        append_entry(tar, "file", tar::EntryType::Regular, 0o644, b"payload");
    });
    let valid = gzip_parts(&[&tar[..512], &tar[512..]]);
    let capture = capture_bytes(&valid);
    let root = tempfile::tempdir().unwrap();
    let mut aggregate = 0;
    super::extract_generated_archive(
        capture.as_file(),
        root.path(),
        "",
        GeneratedRepositoryArchiveFormat::TarGz,
        &|| true,
        &mut aggregate,
        None,
    )
    .unwrap();
    assert_eq!(aggregate, tar.len() as u64);
    assert_eq!(fs::read(root.path().join("file")).unwrap(), b"payload");
    let mut bad_crc = valid.clone();
    let trailer = bad_crc.len() - 8;
    bad_crc[trailer] ^= 1;
    let mut garbage_tail = valid.clone();
    garbage_tail.extend_from_slice(b"invalid-gzip-tail");
    for bytes in [bad_crc, valid[..valid.len() - 2].to_vec(), garbage_tail] {
        let capture = capture_bytes(&bytes);
        let root = tempfile::tempdir().unwrap();
        assert!(
            super::extract_generated_archive(
                capture.as_file(),
                root.path(),
                "",
                GeneratedRepositoryArchiveFormat::TarGz,
                &|| true,
                &mut 0,
                None
            )
            .is_err()
        );
    }
}
