//! Reuse the admitted parser fixtures; only the destination changes.
use super::*;
use crate::runtime::repository_root_io::RootIo;
use crate::runtime::repository_root_io::RootIoLimits;
fn limits() -> RootIoLimits {
    RootIoLimits {
        max_path_bytes: 256,
        max_name_bytes: 255,
        max_depth: 32,
        max_entries: 8192,
        max_directory_name_bytes: 1024 * 1024,
        max_file_bytes: 64 * 1024 * 1024,
        max_link_bytes: 256,
    }
}
#[test]
fn bcr_rooted_destination_preserves_parser_bytes_modes_mtime_and_literal_links() {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;
    let mut raw = Vec::new();
    append(&mut raw, b"pkg/dir", b'5', 0o755, 11, b"");
    append(
        &mut raw,
        b"pkg/dir/body",
        b'0',
        0o640,
        123,
        b"exact body\0\xff\n",
    );
    append_symlink(&mut raw, "pkg/link", b"dir/body", 0o777, b"");
    finish(&mut raw);
    let captured = capture(&gzip(&raw));
    let old = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let fd = File::open(target.path()).unwrap();
    let io = RootIo::new(&fd, limits()).unwrap();
    extract(captured.as_file(), old.path(), Some("pkg"), &|| true, None).unwrap();
    extract_selected_bcr_at(captured.as_file(), &io, Some("pkg"), &|| true, None).unwrap();
    assert_eq!(
        std::fs::read(old.path().join("dir/body")).unwrap(),
        std::fs::read(target.path().join("dir/body")).unwrap()
    );
    for path in ["dir", "dir/body"] {
        let a = std::fs::metadata(old.path().join(path)).unwrap();
        let b = std::fs::metadata(target.path().join(path)).unwrap();
        assert_eq!(
            a.permissions().mode() & 0o7777,
            b.permissions().mode() & 0o7777
        );
        if path == "dir/body" {
            assert_eq!(a.mtime(), b.mtime());
        }
    }
    assert_eq!(
        std::fs::read_link(target.path().join("link")).unwrap(),
        Path::new("dir/body")
    );
}
#[test]
fn bcr_rooted_destination_retains_prefix_collision_and_extension_refusals() {
    let mut duplicate = Vec::new();
    append(&mut duplicate, b"pkg/body", b'0', 0o644, 0, b"first");
    append(&mut duplicate, b"pkg/body", b'0', 0o644, 0, b"second");
    finish(&mut duplicate);
    let mut escape = Vec::new();
    append_symlink(&mut escape, "pkg/link", b"../outside", 0o777, b"");
    finish(&mut escape);
    let mut local_pax = Vec::new();
    append(&mut local_pax, b"pax", b'x', 0o644, 0, b"");
    finish(&mut local_pax);
    for raw in [duplicate, escape, local_pax] {
        let captured = capture(&gzip(&raw));
        let old = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let fd = File::open(target.path()).unwrap();
        let io = RootIo::new(&fd, limits()).unwrap();
        let legacy =
            extract(captured.as_file(), old.path(), Some("pkg"), &|| true, None).unwrap_err();
        let rooted = extract_selected_bcr_at(captured.as_file(), &io, Some("pkg"), &|| true, None)
            .unwrap_err();
        assert_eq!(legacy.stage, rooted.stage);
        assert_eq!(legacy.message, rooted.message);
    }
}
