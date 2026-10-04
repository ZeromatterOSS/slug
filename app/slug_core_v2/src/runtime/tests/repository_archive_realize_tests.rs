use std::cell::Cell;
use std::io::Cursor;

use flate2::Compression;
use flate2::write::GzEncoder;

use super::super::repository_archive::SelectedBcrOverlay;
use super::super::repository_archive::SelectedBcrPatch;
use super::super::repository_io::ArchiveFailureStage;
use super::*;

fn plan(archive: [u8; 32], module: [u8; 32]) -> SelectedBcrArchive {
    SelectedBcrArchive {
        format: SelectedBcrArchiveFormat::TarGz,
        urls: vec!["https://registry.test/archive.tar.gz".into()].into_boxed_slice(),
        integrity: archive,
        strip_prefix: None,
        patches: Box::new([]),
        overlays: Box::new([]),
        patch_strip: 0,
        module_url: "https://registry.test/MODULE.bazel".into(),
        module_integrity: module,
    }
}

fn transform_plan(
    prefix: &str,
    patches: &[[u8; 32]],
    overlays: &[(&str, [u8; 32])],
    patch_strip: usize,
) -> SelectedBcrArchive {
    SelectedBcrArchive {
        format: SelectedBcrArchiveFormat::TarGz,
        urls: vec!["https://registry.test/archive.tgz".into()].into_boxed_slice(),
        integrity: [1; 32],
        strip_prefix: Some(prefix.into()),
        patches: patches
            .iter()
            .enumerate()
            .map(|(index, integrity)| SelectedBcrPatch {
                url: format!("https://registry.test/{index}.patch"),
                integrity: *integrity,
            })
            .collect(),
        overlays: overlays
            .iter()
            .map(|(destination, integrity)| SelectedBcrOverlay {
                destination: (*destination).into(),
                urls: vec![format!("https://registry.test/{destination}")].into_boxed_slice(),
                integrity: *integrity,
            })
            .collect(),
        patch_strip,
        module_url: "https://registry.test/MODULE.bazel".into(),
        module_integrity: [2; 32],
    }
}

fn capture(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(bytes).unwrap();
    file.flush().unwrap();
    file
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn header(name: &[u8], kind: u8, mode: u32, size: u64, mtime: u64) -> [u8; BLOCK] {
    assert!(name.len() <= 100);
    let mut block = [0u8; BLOCK];
    block[..name.len()].copy_from_slice(name);
    octal(&mut block[100..108], u64::from(mode));
    octal(&mut block[108..116], 1001);
    octal(&mut block[116..124], 1001);
    octal(&mut block[124..136], size);
    octal(&mut block[136..148], mtime);
    block[148..156].fill(b' ');
    block[156] = kind;
    block[257..263].copy_from_slice(b"ustar\0");
    block[263..265].copy_from_slice(b"00");
    let checksum = block.iter().map(|byte| u64::from(*byte)).sum();
    octal(&mut block[148..156], checksum);
    block
}

fn octal(field: &mut [u8], value: u64) {
    field.fill(b'0');
    field[field.len() - 1] = 0;
    let encoded = format!("{value:o}");
    let start = field.len() - 1 - encoded.len();
    field[start..start + encoded.len()].copy_from_slice(encoded.as_bytes());
}

fn append(raw: &mut Vec<u8>, name: &[u8], kind: u8, mode: u32, mtime: u64, data: &[u8]) {
    raw.extend_from_slice(&header(name, kind, mode, data.len() as u64, mtime));
    raw.extend_from_slice(data);
    raw.resize(raw.len().next_multiple_of(BLOCK), 0);
}

fn pax_record(key: &str, value: &str) -> Vec<u8> {
    let body = format!("{key}={value}\n");
    let mut length = body.len() + 2;
    loop {
        let record = format!("{length} {body}").into_bytes();
        if record.len() == length {
            return record;
        }
        length = record.len();
    }
}

fn finish(raw: &mut Vec<u8>) {
    raw.extend_from_slice(&[0; BLOCK * 2]);
}

fn append_symlink(raw: &mut Vec<u8>, name: &str, target: &[u8], mode: u32, payload: &[u8]) {
    assert!(target.len() <= 100);
    let mut block = header(name.as_bytes(), b'2', mode, payload.len() as u64, 999);
    block[157..157 + target.len()].copy_from_slice(target);
    block[148..156].fill(b' ');
    let checksum = block.iter().map(|byte| u64::from(*byte)).sum();
    octal(&mut block[148..156], checksum);
    raw.extend_from_slice(&block);
    raw.extend_from_slice(payload);
    raw.resize(raw.len().next_multiple_of(BLOCK), 0);
}

#[cfg(unix)]
#[test]
fn selected_bcr_preserves_relative_file_symlink_text_after_prefix_and_file_order() {
    for link_first in [false, true] {
        let mut raw = Vec::new();
        let link = "pkg/tests/integration/bzlmod_lock_files/.bazelversion";
        let target = b"../../../.bazelversion";
        if link_first {
            append_symlink(&mut raw, link, target, 0o777, b"");
        }
        append(
            &mut raw,
            b"pkg/.bazelversion",
            b'0',
            0o644,
            123,
            b"version\n",
        );
        if !link_first {
            append_symlink(&mut raw, link, target, 0o777, b"");
        }
        finish(&mut raw);
        for prefix in [None, Some("pkg")] {
            let root = tempfile::tempdir().unwrap();
            extract(
                capture(&gzip(&raw)).as_file(),
                root.path(),
                prefix,
                &|| true,
                None,
            )
            .unwrap();
            let link = root
                .path()
                .join(if prefix.is_some() { &link[4..] } else { link });
            assert!(
                std::fs::symlink_metadata(&link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(
                std::fs::read_link(&link).unwrap(),
                Path::new("../../../.bazelversion")
            );
            assert_eq!(std::fs::read(&link).unwrap(), b"version\n");
            let metadata = std::fs::metadata(&link).unwrap();
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(metadata.permissions().mode() & 0o7777, 0o644);
            assert_eq!(
                metadata
                    .modified()
                    .unwrap()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                123
            );
        }
    }
}

#[test]
fn selected_bcr_symlinks_refuse_unsupported_targets_modes_and_payloads() {
    for (target, expected) in [
        (b"/absolute".as_slice(), "absolute"),
        (b"../../escape", "escapes stripped root"),
        (b"../missing", "not an extracted regular file"),
        (b"../dir", "not an extracted regular file"),
        (b"../nested/link", "not an extracted regular file"),
        (b"../missing/../target", "interior dot components"),
        (b"../target/../target", "interior dot components"),
        (b"../dir/./target", "interior dot components"),
        (b"../dir//target", "not normalized"),
        (b"..", "no bounded file tail"),
        (b"../target\\name", "invalid path"),
        (b"\xff", "not valid Unicode"),
    ] {
        let mut raw = Vec::new();
        append(&mut raw, b"target", b'0', 0o644, 0, b"ordinary");
        append(&mut raw, b"dir/", b'5', 0o755, 0, b"");
        append_symlink(&mut raw, "nested/link", target, 0o777, b"");
        finish(&mut raw);
        assert!(
            extraction_error(&raw).contains(expected),
            "target={target:?}"
        );
    }
    for (mode, payload, expected) in [
        (0o644, b"".as_slice(), "unsupported entry mode"),
        (0o4777, b"", "unsupported entry mode"),
        (0o777, b"payload", "symbolic link has a payload"),
    ] {
        let mut raw = Vec::new();
        append_symlink(&mut raw, "link", b"target", mode, payload);
        finish(&mut raw);
        assert!(extraction_error(&raw).contains(expected));
    }
    let mut chain = Vec::new();
    append(&mut chain, b"target", b'0', 0o644, 0, b"ordinary");
    append_symlink(&mut chain, "one", b"target", 0o777, b"");
    append_symlink(&mut chain, "two", b"one", 0o777, b"");
    finish(&mut chain);
    assert!(extraction_error(&chain).contains("not an extracted regular file"));
}

#[test]
fn selected_bcr_symlink_namespace_collisions_refuse_both_archive_orders() {
    for link_first in [false, true] {
        for (child, kind) in [("link/child", b'0'), ("link/child/", b'5'), ("link", b'0')] {
            let mut raw = Vec::new();
            append(&mut raw, b"target", b'0', 0o644, 0, b"ordinary");
            if link_first {
                append_symlink(&mut raw, "link", b"target", 0o777, b"");
            }
            append(
                &mut raw,
                child.as_bytes(),
                kind,
                if kind == b'5' { 0o755 } else { 0o644 },
                0,
                b"",
            );
            if !link_first {
                append_symlink(&mut raw, "link", b"target", 0o777, b"");
            }
            finish(&mut raw);
            let error = extraction_error(&raw);
            assert!(
                error.contains("collision") || error.contains("duplicate"),
                "{error}"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn selected_bcr_checks_session_before_link_installation_and_rejects_linked_transforms() {
    let mut raw = Vec::new();
    append(&mut raw, b"target", b'0', 0o644, 0, b"ordinary\n");
    append_symlink(&mut raw, "link", b"./target", 0o777, b"");
    finish(&mut raw);
    let calls = Cell::new(0);
    let root = tempfile::tempdir().unwrap();
    extract(
        capture(&gzip(&raw)).as_file(),
        root.path(),
        None,
        &|| {
            calls.set(calls.get() + 1);
            true
        },
        None,
    )
    .unwrap();
    let final_call = calls.get();
    let cutoff = Cell::new(0);
    let cancelled = tempfile::tempdir().unwrap();
    assert!(
        extract(
            capture(&gzip(&raw)).as_file(),
            cancelled.path(),
            None,
            &|| {
                cutoff.set(cutoff.get() + 1);
                cutoff.get() < final_call
            },
            None
        )
        .unwrap_err()
        .message
        .contains("no longer active")
    );
    assert_eq!(
        std::fs::read(cancelled.path().join("target")).unwrap(),
        b"ordinary\n"
    );
    assert!(std::fs::symlink_metadata(cancelled.path().join("link")).is_err());
    let overlay =
        place_overlay(capture(b"overlay"), root.path(), "link", &|| true, None).unwrap_err();
    assert!(overlay.message.contains("not a regular file"));
    let patch = b"--- a/link\n+++ b/link\n@@ -1,1 +1,1 @@\n-ordinary\n+changed\n";
    assert!(
        super::super::repository_archive_patch::apply_selected_bcr_patch(
            root.path(),
            patch,
            1,
            &|| true,
            None
        )
        .is_err()
    );
    let mut module_raw = Vec::new();
    append(&mut module_raw, b"target", b'0', 0o644, 0, b"ordinary");
    append_symlink(&mut module_raw, "MODULE.bazel", b"target", 0o777, b"");
    finish(&mut module_raw);
    let archive = capture(&gzip(&module_raw));
    let archive_path = archive.path().to_path_buf();
    let module = capture(b"registry module");
    let module_path = module.path().to_path_buf();
    let error = realize_selected_bcr(
        &plan([1; 32], [2; 32]),
        archive,
        vec![],
        vec![],
        module,
        &|| true,
        None,
    )
    .err()
    .unwrap();
    assert!(
        error
            .message
            .contains("MODULE target is not a regular file")
    );
    assert!(!archive_path.exists() && !module_path.exists());
}

#[cfg(unix)]
#[test]
#[ignore = "root-supervised hash-verified local rules_jvm_external6.7 archive probe"]
fn selected_bcr_realizes_complete_rules_jvm_external_archive() {
    let path = std::env::var_os("SLUG_BCR_ARCHIVE_PROBE").expect("explicit pinned archive path");
    let archive = File::open(path).unwrap();
    let root = tempfile::tempdir().unwrap();
    extract(
        &archive,
        root.path(),
        Some("rules_jvm_external-6.7"),
        &|| true,
        None,
    )
    .unwrap();
    let link = root
        .path()
        .join("tests/integration/bzlmod_lock_files/.bazelversion");
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        Path::new("../../../.bazelversion")
    );
    assert_eq!(
        std::fs::read(link).unwrap(),
        std::fs::read(root.path().join(".bazelversion")).unwrap()
    );
    assert!(root.path().join("extensions.bzl").is_file());
}

fn extraction_error(raw: &[u8]) -> String {
    let root = tempfile::tempdir().unwrap();
    extract(
        capture(&gzip(raw)).as_file(),
        root.path(),
        None,
        &|| true,
        None,
    )
    .unwrap_err()
    .message
}

#[test]
fn selected_bcr_realizes_streamed_files_gnu_name_modes_mtime_and_module() {
    let long = format!("nested/{}/file.txt", "segment".repeat(15));
    assert!(long.len() > 100 && long.len() < PATH_LIMIT);
    let mut raw = Vec::new();
    append(&mut raw, b"./", b'5', 0o755, 1, b"");
    append(&mut raw, b"./bin/", b'5', 0o755, 2, b"");
    append(&mut raw, b"./bin/tool", b'0', 0o755, 123, b"tool bytes");
    let mut long_payload = long.as_bytes().to_vec();
    long_payload.push(0);
    append(&mut raw, b"././@LongLink", b'L', 0o644, 0, &long_payload);
    append(&mut raw, b"placeholder", b'0', 0o444, 456, b"long bytes");
    append(
        &mut raw,
        b"./MODULE.bazel",
        b'0',
        0o644,
        789,
        b"archive module",
    );
    finish(&mut raw);
    let archive = capture(&gzip(&raw));
    let archive_path = archive.path().to_path_buf();
    let module = capture(b"registry module\n");
    let module_path = module.path().to_path_buf();
    let plan = plan([7; 32], [9; 32]);

    let Materialized::AssociatedImmutable {
        source_identity,
        root,
    } = realize_selected_bcr(
        &plan,
        archive,
        Vec::new(),
        Vec::new(),
        module,
        &|| true,
        None,
    )
    .unwrap()
    else {
        panic!("selected BCR must be associated immutable")
    };
    assert_eq!(
        std::fs::read(root.path().join("bin/tool")).unwrap(),
        b"tool bytes"
    );
    assert_eq!(
        std::fs::read(root.path().join(&long)).unwrap(),
        b"long bytes"
    );
    assert_eq!(
        std::fs::read(root.path().join("MODULE.bazel")).unwrap(),
        b"registry module\n"
    );
    assert_eq!(
        source_identity,
        selected_bcr_source_association(&plan),
        "association covers both verified content digests"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(root.path().join("bin/tool"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        assert_eq!(
            std::fs::metadata(root.path().join(&long))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o444
        );
        assert_eq!(
            std::fs::metadata(root.path().join("MODULE.bazel"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o644
        );
    }
    assert_eq!(
        std::fs::metadata(root.path().join("bin/tool"))
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        123
    );
    assert_eq!(
        std::fs::metadata(root.path().join(&long))
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        456
    );
    assert!(!archive_path.exists());
    assert!(!module_path.exists());
}

// Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
// CompressedTarFunction.java:142-152 retains regular mode | 0400 and mtime.
// platforms 1.0.0 archive SHA-256:
// 3384eb1c30762704fbe38e440204e114154086c8fc8a8c2e3e28441028c019a8
// has eleven regular 0640 entries and three directory 0750 entries.
#[cfg(unix)]
#[test]
fn selected_bcr_admits_regular_0640_without_changing_bytes_mode_or_mtime() {
    use std::os::unix::fs::PermissionsExt;

    let mut raw = Vec::new();
    append(&mut raw, b"file", b'0', 0o640, 123, b"probe bytes");
    finish(&mut raw);
    let root = tempfile::tempdir().unwrap();
    extract(
        capture(&gzip(&raw)).as_file(),
        root.path(),
        None,
        &|| true,
        None,
    )
    .unwrap();
    let path = root.path().join("file");
    assert_eq!(std::fs::read(&path).unwrap(), b"probe bytes");
    let metadata = std::fs::metadata(path).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o7777, 0o640);
    assert_eq!(
        metadata
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        123
    );
}

#[cfg(unix)]
#[test]
fn selected_bcr_realizes_regular_0555_build_and_nested_source_with_bytes_mode_mtime_and_association()
 {
    use std::os::unix::fs::PermissionsExt;

    use sha2::Digest;

    const BUILD: &[u8] = b"exports_files(['src/defs.bzl'])\n";
    const SOURCE: &[u8] = b"VALUE = 'mode-0555'\n";
    const MODULE: &[u8] = b"module(name = 'mode_probe')\n";
    let mut raw = Vec::new();
    append(&mut raw, b"BUILD.bazel", b'0', 0o555, 123, BUILD);
    assert_eq!(&raw[100..108], b"0000555\0");
    append(&mut raw, b"src/", b'5', 0o755, 124, b"");
    append(&mut raw, b"src/defs.bzl", b'0', 0o555, 125, SOURCE);
    finish(&mut raw);
    let archive = gzip(&raw);
    let plan = plan(
        sha2::Sha256::digest(&archive).into(),
        sha2::Sha256::digest(MODULE).into(),
    );

    let Materialized::AssociatedImmutable {
        source_identity,
        root,
    } = realize_selected_bcr(
        &plan,
        capture(&archive),
        Vec::new(),
        Vec::new(),
        capture(MODULE),
        &|| true,
        None,
    )
    .unwrap()
    else {
        panic!("regular 0555 BCR archive must publish an associated immutable root")
    };
    assert_eq!(source_identity, selected_bcr_source_association(&plan));
    for (path, bytes, mtime) in [("BUILD.bazel", BUILD, 123), ("src/defs.bzl", SOURCE, 125)] {
        let path = root.path().join(path);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let metadata = std::fs::metadata(path).unwrap();
        assert!(metadata.file_type().is_file());
        assert_eq!(metadata.permissions().mode() & 0o7777, 0o555);
        assert_eq!(
            metadata
                .modified()
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            mtime
        );
    }
    assert_eq!(
        std::fs::read(root.path().join("MODULE.bazel")).unwrap(),
        MODULE
    );
    assert!(!root.path().join("REPO.bazel").exists());
}

#[cfg(unix)]
#[test]
fn selected_bcr_admits_directory_0750_with_native_normalization_and_prefix() {
    use std::os::unix::fs::PermissionsExt;

    // Bazel creates directories without archive chmod (same pinned source:110-112).
    // Slug intentionally keeps its own deterministic 0755 directory projection.
    let mut raw = Vec::new();
    append(&mut raw, b"./", b'5', 0o750, 0, b"");
    append(&mut raw, b"pkg/", b'5', 0o750, 0, b"");
    append(&mut raw, b"pkg/nested/", b'5', 0o750, 0, b"");
    append(&mut raw, b"pkg/nested/file", b'0', 0o644, 0, b"child");
    finish(&mut raw);
    for prefix in [None, Some("pkg")] {
        let root = tempfile::tempdir().unwrap();
        extract(
            capture(&gzip(&raw)).as_file(),
            root.path(),
            prefix,
            &|| true,
            None,
        )
        .unwrap();
        let directory = root.path().join(if prefix.is_some() {
            "nested"
        } else {
            "pkg/nested"
        });
        assert_eq!(std::fs::read(directory.join("file")).unwrap(), b"child");
        assert_eq!(
            std::fs::metadata(directory).unwrap().permissions().mode() & 0o7777,
            0o755
        );
    }
}

#[test]
fn selected_bcr_prefix_overlay_patch_module_order_and_modes_are_exact() {
    let mut raw = Vec::new();
    append(&mut raw, b"outside.txt", b'0', 0o664, 1, b"outside\n");
    append(&mut raw, b"pkg/", b'5', 0o775, 2, b"");
    append(&mut raw, b"pkg/value.txt", b'0', 0o664, 3, b"archive\n");
    append(&mut raw, b"pkg/bin/", b'5', 0o775, 4, b"");
    append(&mut raw, b"pkg/bin/tool", b'0', 0o775, 5, b"tool\n");
    append(
        &mut raw,
        b"pkg/MODULE.bazel",
        b'0',
        0o664,
        6,
        b"archive module\n",
    );
    finish(&mut raw);
    let patch = b"--- a/value.txt\n+++ b/value.txt\n@@ -1,1 +1,1 @@\n-overlay\n+patched\n--- a/MODULE.bazel\n+++ b/MODULE.bazel\n@@ -1,1 +1,1 @@\n-archive module\n+patched module\n";
    let plan = transform_plan("pkg", &[[3; 32]], &[("value.txt", [4; 32])], 1);
    let Materialized::AssociatedImmutable { root, .. } = realize_selected_bcr(
        &plan,
        capture(&gzip(&raw)),
        vec![capture(b"overlay\n")],
        vec![capture(patch)],
        capture(b"registry module\n"),
        &|| true,
        None,
    )
    .unwrap() else {
        panic!("selected BCR transform must publish an immutable root")
    };
    assert!(!root.path().join("outside.txt").exists());
    assert_eq!(
        std::fs::read(root.path().join("value.txt")).unwrap(),
        b"patched\n"
    );
    assert_eq!(
        std::fs::read(root.path().join("MODULE.bazel")).unwrap(),
        b"registry module\n"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(root.path().join("bin/tool"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o775
        );
        assert_eq!(
            std::fs::metadata(root.path().join("value.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755,
            "verified overlays are executable"
        );
    }

    let missing = tempfile::tempdir().unwrap();
    assert!(
        extract(
            capture(&gzip(&raw)).as_file(),
            missing.path(),
            Some("missing"),
            &|| true,
            None
        )
        .unwrap_err()
        .message
        .contains("strip_prefix was not found")
    );
}

#[test]
fn selected_bcr_consumes_only_well_formed_global_pax_comment_metadata() {
    let authentic = b"52 comment=e071f45e209f3e154210faed3d0e60c29aef3b4a\n".to_vec();
    let mut comments = authentic.clone();
    comments.extend(pax_record("comment", "second"));
    let mut raw = Vec::new();
    append(&mut raw, b"pax_global_header", b'g', 0o666, 0, &comments);
    append(
        &mut raw,
        b"another_global_header",
        b'g',
        0o666,
        0,
        &pax_record("comment", "third"),
    );
    append(&mut raw, b"file", b'0', 0o664, 1, b"bytes");
    finish(&mut raw);
    let root = tempfile::tempdir().unwrap();
    extract(
        capture(&gzip(&raw)).as_file(),
        root.path(),
        None,
        &|| true,
        None,
    )
    .unwrap();
    assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"bytes");

    let mut malformed_length = pax_record("comment", "x");
    malformed_length[..2].copy_from_slice(b"12");
    let mut malformed_utf8 = pax_record("comment", "x");
    let value = malformed_utf8.len() - 2;
    malformed_utf8[value] = 0xff;
    let mut nul = pax_record("comment", "x");
    nul[value] = 0;
    let mut no_lf = pax_record("comment", "x");
    *no_lf.last_mut().unwrap() = b'x';
    for (payload, expected) in [
        (malformed_length, "record lacks final LF"),
        (malformed_utf8, "NUL-free UTF-8"),
        (nul, "NUL-free UTF-8"),
        (no_lf, "record lacks final LF"),
        (pax_record("path", "file"), "key is unsupported"),
        (pax_record("vendor.example", "x"), "key is unsupported"),
    ] {
        let mut raw = Vec::new();
        append(&mut raw, b"pax", b'g', 0o666, 0, &payload);
        finish(&mut raw);
        let error = extraction_error(&raw);
        assert!(error.contains(expected), "{expected}: {error}");
    }

    let mut local = Vec::new();
    append(
        &mut local,
        b"pax",
        b'x',
        0o666,
        0,
        &pax_record("comment", "x"),
    );
    finish(&mut local);
    assert!(extraction_error(&local).contains("local PAX"));

    let mut cancelled = Vec::new();
    append(&mut cancelled, b"pax", b'g', 0o666, 0, &authentic);
    finish(&mut cancelled);
    let calls = Cell::new(0usize);
    let active = || {
        calls.set(calls.get() + 1);
        calls.get() < 2
    };
    let root = tempfile::tempdir().unwrap();
    assert!(
        extract(
            capture(&gzip(&cancelled)).as_file(),
            root.path(),
            None,
            &active,
            None,
        )
        .unwrap_err()
        .message
        .contains("no longer active")
    );
}

#[test]
fn selected_bcr_rejects_hidden_extension_allocation_and_name_state() {
    let cases = [
        (b'x', ENTRY_LIMIT, "local PAX"),
        (b'g', ENTRY_LIMIT, "global PAX payload exceeds"),
        (b'K', ENTRY_LIMIT, "GNU long link"),
        (b'S', ENTRY_LIMIT, "sparse entry"),
    ];
    for (kind, declared, expected) in cases {
        let raw = header(b"extension", kind, 0o644, declared, 0).to_vec();
        let error = extraction_error(&raw);
        assert!(error.contains(expected), "kind {kind}: {error}");
    }

    let mut oversized = header(b"long", b'L', 0o644, PATH_LIMIT as u64 + 1, 0).to_vec();
    finish(&mut oversized);
    assert!(extraction_error(&oversized).contains("GNU long name exceeds"));

    let mut orphan = Vec::new();
    append(&mut orphan, b"long", b'L', 0o644, 0, b"name\0");
    finish(&mut orphan);
    assert!(extraction_error(&orphan).contains("orphan GNU long name"));

    let mut doubled = Vec::new();
    append(&mut doubled, b"long", b'L', 0o644, 0, b"one\0");
    append(&mut doubled, b"long", b'L', 0o644, 0, b"two\0");
    finish(&mut doubled);
    assert!(extraction_error(&doubled).contains("doubled GNU long names"));
}

#[test]
fn selected_bcr_rejects_paths_namespace_modes_and_malformed_streams() {
    let invalid_paths = [
        "../escape",
        "/absolute",
        "a/./b",
        "a//b",
        "a//",
        "a\\b",
        "a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z/aa/bb/cc/dd/ee/ff/g",
    ];
    for path in invalid_paths {
        let mut raw = Vec::new();
        append(&mut raw, path.as_bytes(), b'0', 0o644, 0, b"x");
        finish(&mut raw);
        assert!(extraction_error(&raw).contains("path"), "{path}");
    }

    let mut duplicate = Vec::new();
    append(&mut duplicate, b"same", b'0', 0o644, 0, b"a");
    append(&mut duplicate, b"same", b'0', 0o644, 0, b"b");
    finish(&mut duplicate);
    assert!(extraction_error(&duplicate).contains("duplicate"));

    let mut collision = Vec::new();
    append(&mut collision, b"file", b'0', 0o644, 0, b"a");
    append(&mut collision, b"file/child", b'0', 0o644, 0, b"b");
    finish(&mut collision);
    assert!(extraction_error(&collision).contains("ancestor collision"));

    for mode in [0o600, 0o750, 0o4640, 0o2640, 0o1640] {
        let mut raw = Vec::new();
        append(&mut raw, b"file", b'0', mode, 0, b"a");
        finish(&mut raw);
        assert!(
            extraction_error(&raw).contains("unsupported entry mode"),
            "{mode:o}"
        );
    }

    for mode in [0o700, 0o640, 0o4750, 0o2750, 0o1750] {
        let mut raw = Vec::new();
        append(&mut raw, b"dir/", b'5', mode, 0, b"");
        finish(&mut raw);
        assert!(
            extraction_error(&raw).contains("unsupported entry mode"),
            "{mode:o}"
        );
    }

    let mut checksum = Vec::new();
    append(&mut checksum, b"file", b'0', 0o644, 0, b"a");
    checksum[0] ^= 1;
    finish(&mut checksum);
    assert!(extraction_error(&checksum).contains("checksum mismatch"));

    let truncated = header(b"file", b'0', 0o644, 5, 0).to_vec();
    assert!(extraction_error(&truncated).contains("unexpected end"));

    let mut trailing = Vec::new();
    finish(&mut trailing);
    trailing.push(1);
    assert!(extraction_error(&trailing).contains("nonzero trailing"));
}

#[test]
fn selected_bcr_enforces_physical_decompressed_entry_and_payload_limits() {
    let mut headers = Vec::new();
    for index in 0..=HEADER_LIMIT {
        append(
            &mut headers,
            format!("d{index}").as_bytes(),
            b'5',
            0o755,
            0,
            b"",
        );
    }
    finish(&mut headers);
    assert!(extraction_error(&headers).contains("physical header limit"));

    let mut entry = header(b"large", b'0', 0o644, ENTRY_LIMIT + 1, 0).to_vec();
    finish(&mut entry);
    assert!(extraction_error(&entry).contains("entry exceeds size limit"));

    assert!(checked_payload(PAYLOAD_LIMIT, 1).is_err());

    let mut reader = BoundedTarReader {
        reader: Cursor::new([1u8]),
        active: &|| true,
        scope: None,
        read: DECOMPRESSED_LIMIT,
    };
    assert!(
        reader
            .finish_zero_padding()
            .unwrap_err()
            .message
            .contains("decompressed limit")
    );
}

#[test]
fn selected_bcr_active_cutoff_and_source_association_are_session_semantic() {
    let mut raw = Vec::new();
    append(&mut raw, b"file", b'0', 0o644, 0, b"bytes");
    finish(&mut raw);
    let calls = Cell::new(0);
    let active = || {
        calls.set(calls.get() + 1);
        calls.get() < 3
    };
    let root = tempfile::tempdir().unwrap();
    assert!(
        extract(
            capture(&gzip(&raw)).as_file(),
            root.path(),
            None,
            &active,
            None
        )
        .unwrap_err()
        .message
        .contains("no longer active")
    );

    let a = plan([1; 32], [2; 32]);
    let b = plan([3; 32], [2; 32]);
    let module_changed = plan([1; 32], [4; 32]);
    assert_eq!(
        selected_bcr_source_association(&a),
        selected_bcr_source_association(&plan([1; 32], [2; 32]))
    );
    assert_ne!(
        selected_bcr_source_association(&a),
        selected_bcr_source_association(&b)
    );
    assert_ne!(
        selected_bcr_source_association(&a),
        selected_bcr_source_association(&module_changed)
    );

    let ordered = transform_plan("pkg", &[[5; 32], [6; 32]], &[("x", [7; 32])], 1);
    let reordered = transform_plan("pkg", &[[6; 32], [5; 32]], &[("x", [7; 32])], 1);
    let prefix_changed = transform_plan("other", &[[5; 32], [6; 32]], &[("x", [7; 32])], 1);
    let overlay_changed = transform_plan("pkg", &[[5; 32], [6; 32]], &[("y", [7; 32])], 1);
    let strip_changed = transform_plan("pkg", &[[5; 32], [6; 32]], &[("x", [7; 32])], 2);
    for changed in [
        &reordered,
        &prefix_changed,
        &overlay_changed,
        &strip_changed,
    ] {
        assert_ne!(
            selected_bcr_source_association(&ordered),
            selected_bcr_source_association(changed)
        );
    }
}

#[test]
fn selected_bcr_cancellation_between_transforms_drops_every_capture() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Boundary {
        ArchiveOverlay,
        OverlayPatch,
        PatchModule,
        ModulePublish,
    }

    for boundary in [
        Boundary::ArchiveOverlay,
        Boundary::OverlayPatch,
        Boundary::PatchModule,
        Boundary::ModulePublish,
    ] {
        let mut raw = Vec::new();
        append(&mut raw, b"pkg/", b'5', 0o755, 0, b"");
        append(&mut raw, b"pkg/value", b'0', 0o644, 0, b"old\n");
        finish(&mut raw);
        let archive = capture(&gzip(&raw));
        let archive_path = archive.path().to_path_buf();
        let overlay = capture(b"overlay\n");
        let overlay_path = overlay.path().to_path_buf();
        let patch = capture(b"--- a/value\n+++ b/value\n@@ -1,1 +1,1 @@\n-overlay\n+patched\n");
        let patch_path = patch.path().to_path_buf();
        let module = capture(b"module\n");
        let module_path = module.path().to_path_buf();
        let patch_apply_checks = Cell::new(2usize);
        let active = || {
            let capture_exists = match boundary {
                Boundary::ArchiveOverlay => archive_path.exists(),
                Boundary::OverlayPatch => overlay_path.exists(),
                Boundary::PatchModule => patch_path.exists(),
                Boundary::ModulePublish => module_path.exists(),
            };
            if capture_exists {
                return true;
            }
            if boundary == Boundary::PatchModule && patch_apply_checks.get() != 0 {
                patch_apply_checks.set(patch_apply_checks.get() - 1);
                return true;
            }
            false
        };
        let result = realize_selected_bcr(
            &transform_plan("pkg", &[[3; 32]], &[("value", [4; 32])], 1),
            archive,
            vec![overlay],
            vec![patch],
            module,
            &active,
            None,
        );
        let Err(error) = result else {
            panic!("cancellation at transform boundary must reject publication")
        };
        assert!(error.message.contains("no longer active"), "{boundary:?}");
        assert!(!archive_path.exists(), "{boundary:?}");
        assert!(!overlay_path.exists(), "{boundary:?}");
        assert!(!patch_path.exists(), "{boundary:?}");
        assert!(!module_path.exists(), "{boundary:?}");
    }
}

#[test]
fn selected_bcr_module_and_cancellation_failures_drop_captures() {
    let mut raw = Vec::new();
    append(&mut raw, b"file", b'0', 0o644, 0, b"bytes");
    finish(&mut raw);
    let archive = capture(&gzip(&raw));
    let archive_path = archive.path().to_path_buf();
    let cancelled_module = capture(b"unused");
    let cancelled_module_path = cancelled_module.path().to_path_buf();
    let result = realize_selected_bcr(
        &plan([1; 32], [2; 32]),
        archive,
        Vec::new(),
        Vec::new(),
        cancelled_module,
        &|| false,
        None,
    );
    let Err(error) = result else {
        panic!("MODULE transport failure must reject realization")
    };
    assert_eq!(error.stage, ArchiveFailureStage::Materialization);
    assert!(!archive_path.exists());
    assert!(!cancelled_module_path.exists());

    let archive = capture(&gzip(&raw));
    let module = capture(&vec![0; MODULE_LIMIT as usize + 1]);
    let module_path = module.path().to_path_buf();
    let result = realize_selected_bcr(
        &plan([1; 32], [2; 32]),
        archive,
        Vec::new(),
        Vec::new(),
        module,
        &|| true,
        None,
    );
    let Err(error) = result else {
        panic!("oversized MODULE must reject realization")
    };
    assert_eq!(error.stage, ArchiveFailureStage::Materialization);
    assert!(error.message.contains("MODULE exceeds size limit"));
    assert!(!module_path.exists());
}

#[cfg(unix)]
#[test]
#[ignore = "requires disposable audited rules_rust archive and registry MODULE"]
fn selected_bcr_disposable_artifact_matches_bazel_manifest() {
    let archive_path = std::env::var("SLUG_BCR_AUDIT_ARCHIVE").unwrap();
    let module_path = std::env::var("SLUG_BCR_AUDIT_MODULE").unwrap();
    let bazel_root = std::env::var("SLUG_BCR_AUDIT_BAZEL_ROOT").unwrap();
    let archive = capture_file(&archive_path);
    let module = capture_file(&module_path);
    let plan = plan(
        hex::decode("2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2")
            .unwrap()
            .try_into()
            .unwrap(),
        hex::decode("25e3b077128612754c4add1b4c90d20a6be06566b623dee6e32038d0e8f93062")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let Materialized::AssociatedImmutable { root, .. } = realize_selected_bcr(
        &plan,
        archive,
        Vec::new(),
        Vec::new(),
        module,
        &|| true,
        None,
    )
    .unwrap() else {
        panic!("selected BCR must be associated immutable")
    };
    let mut rows = Vec::new();
    audit_manifest(root.path(), root.path(), &mut rows);
    rows.sort();
    assert_eq!(rows.len(), 4493);
    assert_eq!(
        rows.iter().filter(|row| row.starts_with("F\t")).count(),
        3544
    );
    assert_eq!(
        rows.iter().filter(|row| row.starts_with("D\t")).count(),
        949
    );
    let bazel_root = Path::new(&bazel_root);
    let mut bazel_rows = Vec::new();
    audit_manifest(bazel_root, bazel_root, &mut bazel_rows);
    bazel_rows.sort();
    assert_eq!(rows, bazel_rows);
    let manifest = rows.join("\n");
    assert_eq!(
        hex::encode(Sha256::digest(manifest.as_bytes())),
        "3196a914273a4debfe9c8fe65c4a74d116cd0f2c4b94f7d08e372d267382b0ee"
    );
}

#[cfg(unix)]
fn capture_file(path: &str) -> tempfile::NamedTempFile {
    let mut capture = tempfile::NamedTempFile::new().unwrap();
    std::io::copy(
        &mut std::fs::File::open(path).unwrap(),
        capture.as_file_mut(),
    )
    .unwrap();
    capture
}

#[cfg(unix)]
fn audit_manifest(root: &Path, directory: &Path, rows: &mut Vec<String>) {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;

    for entry in std::fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        if metadata.is_dir() {
            rows.push(format!("D\t{relative}"));
            audit_manifest(root, &path, rows);
        } else if metadata.is_file() {
            let mut digest = Sha256::new();
            std::io::copy(&mut std::fs::File::open(&path).unwrap(), &mut digest).unwrap();
            let mtime = if relative == "MODULE.bazel" {
                "-".to_owned()
            } else {
                metadata.mtime().to_string()
            };
            rows.push(format!(
                "F\t{relative}\t{:o}\t{mtime}\t{}",
                metadata.permissions().mode() & 0o777,
                hex::encode(digest.finalize())
            ));
        } else {
            panic!("special filesystem entry at {relative}");
        }
    }
}

#[test]
fn scoped_bcr_counts_headers_overlay_patch_and_final_module_replacements() {
    use std::time::Duration;
    use std::time::Instant;

    use slug_workspace_v2::NormalizedAbsolutePath;

    use crate::runtime::dice::source_service::SourceServiceLimits;
    use crate::runtime::dice::source_service::SourceServiceScope;
    let mut raw = Vec::new();
    append(&mut raw, b"file", b'0', 0o644, 1, b"old\n");
    finish(&mut raw);
    let patch = b"--- a/file\n+++ b/file\n@@ -1,1 +1,1 @@\n-one\n+two\n";
    let module = b"module\n";
    let mut plan = transform_plan("", &[[2; 32]], &[("file", [3; 32])], 1);
    plan.strip_prefix = None;
    for cap in [19, 18] {
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
        let result = realize_selected_bcr(
            &plan,
            capture(&gzip(&raw)),
            vec![capture(b"one\n")],
            vec![capture(patch)],
            capture(module),
            &|| true,
            Some(&scope),
        );
        assert_eq!(result.is_ok(), cap == 19);
        assert_eq!(
            scope.snapshot().decoded_bytes,
            raw.len() as u64,
            "headers/padding/end blocks all count"
        );
        if let Ok(Materialized::AssociatedImmutable { root, .. }) = result {
            assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"two\n");
            assert_eq!(
                std::fs::read(root.path().join("MODULE.bazel")).unwrap(),
                module
            );
            assert_eq!(scope.snapshot().written_bytes, 19);
        } else {
            assert!(scope.check().is_err());
        }
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    }
}

#[test]
fn scoped_capture_fallible_reservation_preserves_bytes_poison_and_cleanup() {
    use std::time::Instant;

    use slug_workspace_v2::NormalizedAbsolutePath;
    use source_service::SourceServiceError;

    use crate::runtime::dice::source_service::SourceServiceLimits;

    let new_scope = || {
        let work = Instant::now() + Duration::from_secs(2);
        SourceServiceScope::new_for_test(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            work,
            work + Duration::from_secs(1),
            SourceServiceLimits::default(),
        )
        .unwrap()
    };
    let payload = b"capture\0\xff\n";
    let scope = new_scope();
    let credit = scope
        .reserve_buffer_at(
            payload.len(),
            source_service::SourceBufferSite::PatchCapture,
        )
        .unwrap();
    assert_eq!(
        read_capture(capture(payload), "patch", 1024, &|| true, Some(&scope)).unwrap(),
        payload
    );
    assert_eq!(scope.check(), Ok(()));
    assert_eq!(scope.snapshot().live_buffer_bytes, payload.len());
    assert_eq!(scope.snapshot().written_bytes, 0);
    drop(credit);
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);

    // Overflow is a real TryReserveError; oversizing exercises the credit guard.
    // Cancellation before/inside reservation must retain the first cause.
    for mode in 0..4 {
        let scope = new_scope();
        let credit = scope
            .reserve_buffer_at(
                payload.len(),
                source_service::SourceBufferSite::PatchCapture,
            )
            .unwrap();
        let mut file = capture(payload);
        file.as_file_mut().rewind().unwrap();
        let path = file.path().to_owned();
        let mut observed = file.as_file().try_clone().unwrap();
        let calls = Cell::new(0);
        let invoked = Cell::new(false);
        if mode == 3 {
            scope.cancel();
        }
        let error = read_capture_with_reserve(
            file,
            "patch",
            1024,
            &|| {
                calls.set(calls.get() + 1);
                true
            },
            Some(&scope),
            |bytes, capacity| {
                invoked.set(true);
                assert_eq!(capacity, payload.len());
                assert!(bytes.is_empty());
                assert_eq!(bytes.capacity(), 0);
                assert_eq!(
                    scope.snapshot().live_buffer_bytes,
                    payload.len() + 64 * 1024
                );
                if mode == 1 {
                    scope.cancel();
                }
                bytes.try_reserve_exact(if mode == 2 { capacity + 1 } else { usize::MAX })
            },
        )
        .unwrap_err();
        let expected = if mode == 1 || mode == 3 {
            SourceServiceError::Cancelled
        } else {
            SourceServiceError::Allocation
        };
        assert!(error.message.contains(&expected.to_string()), "{error:?}");
        assert_eq!(scope.check(), Err(expected));
        assert_eq!(invoked.get(), mode != 3);
        assert_eq!(calls.get(), 1, "copy never began");
        assert_eq!(observed.stream_position().unwrap(), 0);
        assert!(!path.exists(), "failed capture must retire");
        assert_eq!(scope.snapshot().written_bytes, 0);
        assert_eq!(scope.snapshot().live_buffer_bytes, payload.len());
        drop(credit);
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    }
}
