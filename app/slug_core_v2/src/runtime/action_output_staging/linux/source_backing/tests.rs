use std::fs;
use std::io::Seek;
use std::io::SeekFrom;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;

use super::*;

fn digest(bytes: &[u8]) -> FileContentDigest {
    FileContentDigest::new(Sha256::digest(bytes).into(), bytes.len() as u64).unwrap()
}
fn source(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut source = tempfile::NamedTempFile::new().unwrap();
    source.write_all(bytes).unwrap();
    source.seek(SeekFrom::Start(0)).unwrap();
    source
}
fn no_private(workspace: &Path) {
    let cache = workspace.join("bazel-out/.slug-runfiles-sources/v1");
    if cache.exists() {
        assert!(fs::read_dir(cache).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".slug-output-stage-")
        }));
    }
}

#[test]
fn source_backing_verifies_bytes_preserves_execute_bits_and_repairs_corruption_or_loss() {
    let workspace = tempfile::tempdir().unwrap();
    for (permissions, expected) in [(0o644, 0o444), (0o755, 0o555), (0o710, 0o554)] {
        let identity = digest(b"verified");
        let expected_path =
            SourceBacking::target_path(workspace.path(), identity, permissions).unwrap();
        let mut input = source(b"verified");
        let mut stage =
            SourceBacking::new(workspace.path(), identity, permissions, input.as_file_mut())
                .unwrap();
        assert_eq!(stage.target(), expected_path);
        assert!(stage.preflight().is_err());
        stage.seal().unwrap();
        stage.preflight().unwrap();
        stage.publish().unwrap();
        assert!(stage.publish().is_err());
        drop(stage);
        drop(input);
        assert_eq!(fs::read(&expected_path).unwrap(), b"verified");
        assert_eq!(
            fs::metadata(&expected_path).unwrap().permissions().mode() & 0o777,
            expected
        );
        for corrupt in [true, false] {
            if corrupt {
                fs::set_permissions(&expected_path, fs::Permissions::from_mode(0o600)).unwrap();
                fs::write(&expected_path, b"corrupt").unwrap();
            } else {
                fs::remove_file(&expected_path).unwrap();
            }
            let mut input = source(b"verified");
            let mut repair =
                SourceBacking::new(workspace.path(), identity, permissions, input.as_file_mut())
                    .unwrap();
            repair.seal().unwrap();
            repair.publish().unwrap();
            drop(repair);
            assert_eq!(fs::read(&expected_path).unwrap(), b"verified");
            assert_eq!(
                fs::metadata(&expected_path).unwrap().permissions().mode() & 0o777,
                expected
            );
        }
        no_private(workspace.path());
    }
}

#[test]
fn source_backing_rejects_digest_mode_and_stale_namespace_without_publishing() {
    let workspace = tempfile::tempdir().unwrap();
    let identity = digest(b"expected");
    for bytes in [b"wrong!!!".as_slice(), b"short", b"longer than expected"] {
        let mut input = source(bytes);
        assert!(
            SourceBacking::new(workspace.path(), identity, 0o644, input.as_file_mut())
                .unwrap_err()
                .to_string()
                .contains("digest or size mismatch")
        );
        no_private(workspace.path());
    }
    assert!(SourceBacking::target_path(workspace.path(), identity, -1).is_err());
    let mut input = source(b"expected");
    let mut stage =
        SourceBacking::new(workspace.path(), identity, 0o644, input.as_file_mut()).unwrap();
    stage.seal().unwrap();
    let target = stage.target().to_path_buf();
    fs::write(&target, b"concurrent destination").unwrap();
    assert!(stage.preflight().is_err());
    assert!(stage.publish().is_err());
    drop(stage);
    assert_eq!(fs::read(&target).unwrap(), b"concurrent destination");
    no_private(workspace.path());

    let mut input = source(b"expected");
    let mut stage =
        SourceBacking::new(workspace.path(), identity, 0o644, input.as_file_mut()).unwrap();
    stage.seal().unwrap();
    let version = target.parent().unwrap();
    let moved = version.with_file_name("old-v1");
    fs::rename(version, &moved).unwrap();
    fs::create_dir(version).unwrap();
    assert!(
        stage
            .preflight()
            .unwrap_err()
            .to_string()
            .contains("namespace changed")
    );
    drop(stage);
    assert_eq!(
        fs::read(moved.join(target.file_name().unwrap())).unwrap(),
        b"concurrent destination"
    );
    assert!(fs::read_dir(&moved).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".slug-output-stage-")
    }));
}

#[test]
fn source_backing_confines_cache_parents_and_never_follows_destination_symlinks() {
    for parent in [
        "bazel-out",
        "bazel-out/.slug-runfiles-sources",
        "bazel-out/.slug-runfiles-sources/v1",
    ] {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let path = workspace.path().join(parent);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(outside.path(), &path).unwrap();
        let mut input = source(b"source");
        assert!(
            SourceBacking::new(
                workspace.path(),
                digest(b"source"),
                0o644,
                input.as_file_mut()
            )
            .is_err()
        );
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    }
    let workspace = tempfile::tempdir().unwrap();
    let outside = source(b"outside");
    let target = SourceBacking::target_path(workspace.path(), digest(b"source"), 0o644).unwrap();
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    symlink(outside.path(), &target).unwrap();
    let mut input = source(b"source");
    assert!(
        SourceBacking::new(
            workspace.path(),
            digest(b"source"),
            0o644,
            input.as_file_mut()
        )
        .is_err()
    );
    assert_eq!(fs::read(outside.path()).unwrap(), b"outside");
    assert_eq!(fs::read_link(target).unwrap(), outside.path());
    no_private(workspace.path());
}
