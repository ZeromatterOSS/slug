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
fn rooted_patch_streams_cross_chunk_utf8_preserves_suffix_mode_and_exact_written_charge() {
    use std::os::unix::fs::PermissionsExt;
    let first = format!("{}é\n", "a".repeat(65535));
    let source = format!("{first}before\nsuffix\n");
    let expected = format!("{first}after\nsuffix\n");
    let patch = b"--- a/file\n+++ b/file\n@@ -2,1 +2,1 @@\n-before\n+after\n";
    let old = root_file("file", source.as_bytes());
    let target = root_file("file", source.as_bytes());
    std::fs::set_permissions(
        target.path().join("file"),
        std::fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    apply_selected_bcr_patch(old.path(), patch, 1, &|| true, None).unwrap();
    let scope = stream_scope(expected.len() as u64);
    let fd = std::fs::File::open(target.path()).unwrap();
    let io = RootIo::new(&fd, limits()).unwrap();
    apply_selected_bcr_patch_at(&io, patch, 1, &|| true, Some(&scope)).unwrap();
    assert_eq!(
        std::fs::read(target.path().join("file")).unwrap(),
        expected.as_bytes()
    );
    assert_eq!(
        std::fs::read(old.path().join("file")).unwrap(),
        expected.as_bytes()
    );
    assert_eq!(scope.snapshot().written_bytes, expected.len() as u64);
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        std::fs::metadata(target.path().join("file"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o640
    );
    only_target_remains(target.path());
}
#[test]
fn rooted_patch_validates_complete_text_and_context_before_creating_staging_state() {
    let patch = b"--- a/file\n+++ b/file\n@@ -1,1 +1,1 @@\n-a\n+b\n";
    for source in [
        b"a\ninvalid\xff\n".as_slice(),
        b"a\nmissing-LF".as_slice(),
        b"a\nzero\0\n".as_slice(),
        b"different\n".as_slice(),
    ] {
        let target = root_file("file", source);
        let fd = std::fs::File::open(target.path()).unwrap();
        let io = RootIo::new(&fd, limits()).unwrap();
        let scope = stream_scope(1024 * 1024);
        assert!(apply_selected_bcr_patch_at(&io, patch, 1, &|| true, Some(&scope)).is_err());
        assert_eq!(std::fs::read(target.path().join("file")).unwrap(), source);
        assert_eq!(scope.snapshot().written_bytes, 0);
        only_target_remains(target.path());
    }
    let target = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), target.path().join("file")).unwrap();
    let fd = std::fs::File::open(target.path()).unwrap();
    let io = RootIo::new(&fd, limits()).unwrap();
    assert!(apply_selected_bcr_patch_at(&io, patch, 1, &|| true, None).is_err());
    assert!(outside.as_file().metadata().unwrap().len() == 0);
}
