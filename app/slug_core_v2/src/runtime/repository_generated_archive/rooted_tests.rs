use std::fs::File;
use std::fs::{self};
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;

use super::tests::append_entry;
use super::tests::archive_with_format;
use super::*;
use crate::runtime::repository_root_io::RootIo;
use crate::runtime::repository_root_io::RootIoLimits;

fn limits() -> RootIoLimits {
    RootIoLimits {
        max_path_bytes: 4096,
        max_name_bytes: 255,
        max_depth: 256,
        max_entries: 131_072,
        max_directory_name_bytes: 4 * 1024 * 1024,
        max_file_bytes: 1024 * 1024 * 1024,
        max_link_bytes: 4096,
    }
}

#[test]
fn rooted_archives_replace_on_retained_backing_without_touching_foreign_display_tree() {
    for format in [
        GeneratedRepositoryArchiveFormat::TarXz,
        GeneratedRepositoryArchiveFormat::TarGz,
    ] {
        let parent = tempfile::tempdir().unwrap();
        let display = parent.path().join("display");
        let retained = parent.path().join("retained");
        fs::create_dir(&display).unwrap();
        let descriptor = File::open(&display).unwrap();
        fs::rename(&display, &retained).unwrap();
        fs::create_dir(&display).unwrap();
        fs::write(display.join("untouched"), b"foreign tree").unwrap();
        let root = RootIo::new(&descriptor, limits()).unwrap();
        let first = archive_with_format(format, |tar| {
            append_entry(
                tar,
                "prefix/nested/file",
                tar::EntryType::Regular,
                0o644,
                b"long original body",
            );
            append_entry(
                tar,
                "prefix/kept",
                tar::EntryType::Regular,
                0o644,
                b"retained",
            );
        });
        let second = archive_with_format(format, |tar| {
            append_entry(
                tar,
                "prefix/nested/file",
                tar::EntryType::Regular,
                0o755,
                b"new",
            );
        });
        let mut decoded = 0;
        for capture in [&first, &second] {
            extract_generated_archive_at(
                capture.as_file(),
                &root,
                "prefix",
                format,
                &|| true,
                &mut decoded,
                None,
            )
            .unwrap();
        }
        assert_eq!(fs::read(retained.join("nested/file")).unwrap(), b"new");
        assert_eq!(fs::read(retained.join("kept")).unwrap(), b"retained");
        // Both lanes request mkdir(0777), leaving process umask in force.
        assert_eq!(
            fs::metadata(retained.join("nested"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            fs::metadata(&retained).unwrap().permissions().mode() & 0o777,
        );
        let meta = fs::metadata(retained.join("nested/file")).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o755);
        assert_eq!(meta.mtime(), 1_600_000_000);
        assert_eq!(
            fs::read(display.join("untouched")).unwrap(),
            b"foreign tree"
        );
        assert_eq!(fs::read_dir(&display).unwrap().count(), 1);
        assert!(decoded > 0);
    }
}

#[test]
fn rooted_archive_refuses_symlink_ancestors_and_leaf_without_touching_target() {
    for leaf in [false, true] {
        let parent = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("victim"), b"outside").unwrap();
        if leaf {
            symlink(outside.path().join("victim"), parent.path().join("victim")).unwrap();
        } else {
            symlink(outside.path(), parent.path().join("parent")).unwrap();
        }
        let path = if leaf { "victim" } else { "parent/victim" };
        let capture = archive_with_format(GeneratedRepositoryArchiveFormat::TarGz, |tar| {
            append_entry(tar, path, tar::EntryType::Regular, 0o644, b"overwritten");
        });
        let descriptor = File::open(parent.path()).unwrap();
        let root = RootIo::new(&descriptor, limits()).unwrap();
        assert!(
            extract_generated_archive_at(
                capture.as_file(),
                &root,
                "",
                GeneratedRepositoryArchiveFormat::TarGz,
                &|| true,
                &mut 0,
                None
            )
            .is_err()
        );
        assert_eq!(fs::read(outside.path().join("victim")).unwrap(), b"outside");
    }
}

#[test]
fn rooted_archive_admits_file_size_before_truncation_and_shares_decoded_limits() {
    let parent = tempfile::tempdir().unwrap();
    let descriptor = File::open(parent.path()).unwrap();
    fs::write(parent.path().join("file"), b"keep").unwrap();
    let capture = archive_with_format(GeneratedRepositoryArchiveFormat::TarGz, |tar| {
        append_entry(tar, "file", tar::EntryType::Regular, 0o644, b"data");
    });
    let mut small = limits();
    small.max_file_bytes = 3;
    let root = RootIo::new(&descriptor, small).unwrap();
    assert!(
        extract_generated_archive_at(
            capture.as_file(),
            &root,
            "",
            GeneratedRepositoryArchiveFormat::TarGz,
            &|| true,
            &mut 0,
            None
        )
        .is_err()
    );
    assert_eq!(fs::read(parent.path().join("file")).unwrap(), b"keep");
    small.max_file_bytes = 4;
    let root = RootIo::new(&descriptor, small).unwrap();
    let mut exact = 0;
    extract_with_destination_and_limits(
        capture.as_file(),
        &root,
        "",
        GeneratedRepositoryArchiveFormat::TarGz,
        &|| true,
        &mut exact,
        DecodeLimits {
            per_call: 2048,
            aggregate: 2048,
        },
        None,
    )
    .unwrap();
    assert_eq!(exact, 2048);
    assert_eq!(fs::read(parent.path().join("file")).unwrap(), b"data");
    let mut short = 0;
    assert!(
        extract_with_destination_and_limits(
            capture.as_file(),
            &root,
            "",
            GeneratedRepositoryArchiveFormat::TarGz,
            &|| true,
            &mut short,
            DecodeLimits {
                per_call: 2047,
                aggregate: 2048
            },
            None
        )
        .is_err()
    );
    assert_eq!(short, 2047);
}

#[test]
fn rooted_archive_cancelled_before_decode_preserves_existing_file() {
    let parent = tempfile::tempdir().unwrap();
    fs::write(parent.path().join("file"), b"keep").unwrap();
    let descriptor = File::open(parent.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let capture = archive_with_format(GeneratedRepositoryArchiveFormat::TarGz, |tar| {
        append_entry(tar, "file", tar::EntryType::Regular, 0o644, b"changed");
    });
    let mut decoded = 0;
    assert!(
        extract_generated_archive_at(
            capture.as_file(),
            &root,
            "",
            GeneratedRepositoryArchiveFormat::TarGz,
            &|| false,
            &mut decoded,
            None
        )
        .is_err()
    );
    assert_eq!(decoded, 0);
    assert_eq!(fs::read(parent.path().join("file")).unwrap(), b"keep");
    assert!(root.lstat(Path::new("")).is_ok());
}
