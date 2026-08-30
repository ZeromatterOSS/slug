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

fn extraction_error(raw: &[u8]) -> String {
    let root = tempfile::tempdir().unwrap();
    extract(capture(&gzip(raw)).as_file(), root.path(), None, &|| true)
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
    append(&mut raw, b"placeholder", b'0', 0o644, 456, b"long bytes");
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
    } = realize_selected_bcr(&plan, archive, Vec::new(), Vec::new(), module, &|| true).unwrap()
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
            0o644
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
    assert!(!archive_path.exists());
    assert!(!module_path.exists());
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
            &|| true
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
    extract(capture(&gzip(&raw)).as_file(), root.path(), None, &|| true).unwrap();
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

    let mut mode = Vec::new();
    append(&mut mode, b"file", b'0', 0o600, 0, b"a");
    finish(&mut mode);
    assert!(extraction_error(&mode).contains("unsupported entry mode"));

    let mut directory_mode = Vec::new();
    append(&mut directory_mode, b"dir/", b'5', 0o700, 0, b"");
    finish(&mut directory_mode);
    assert!(extraction_error(&directory_mode).contains("unsupported entry mode"));

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
        extract(capture(&gzip(&raw)).as_file(), root.path(), None, &active)
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
    let Materialized::AssociatedImmutable { root, .. } =
        realize_selected_bcr(&plan, archive, Vec::new(), Vec::new(), module, &|| true).unwrap()
    else {
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
