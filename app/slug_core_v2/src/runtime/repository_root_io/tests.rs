use std::cell::Cell;
use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;

use super::*;

fn limits() -> RootIoLimits {
    RootIoLimits {
        max_path_bytes: 4096,
        max_name_bytes: 255,
        max_depth: 64,
        max_entries: 8192,
        max_directory_name_bytes: 1024 * 1024,
        max_file_bytes: 64 * 1024 * 1024,
        max_link_bytes: 4096,
    }
}

fn read(io: &RootIo<'_>, name: &str) -> Vec<u8> {
    let mut body = Vec::new();
    io.read_file(Path::new(name), &|| true, |bytes| {
        body.extend_from_slice(bytes);
        Ok(())
    })
    .unwrap();
    body
}

#[test]
fn renamed_root_and_foreign_display_preserve_actual_reads_and_mutations() {
    let fixture = tempfile::tempdir().unwrap();
    let display = fixture.path().join("display");
    let backing = fixture.path().join("backing");
    fs::create_dir(&display).unwrap();
    let root = File::open(&display).unwrap();
    let io = RootIo::new(&root, limits()).unwrap();
    let original = io.root_stat().unwrap();
    fs::rename(&display, &backing).unwrap();
    fs::create_dir(&display).unwrap();
    fs::write(display.join("data"), b"foreign").unwrap();

    io.ensure_directory(Path::new("nested/empty"), 0o755)
        .unwrap();
    io.write_file(
        Path::new("data"),
        b"first",
        0o644,
        WriteMode::CreateNew,
        &|| true,
    )
    .unwrap();
    let inode = io.lstat(Path::new("data")).unwrap().st_ino;
    io.write_file(
        Path::new("data"),
        b"next",
        0o755,
        WriteMode::ReplaceRegular,
        &|| true,
    )
    .unwrap();
    assert_eq!(io.lstat(Path::new("data")).unwrap().st_ino, inode);
    assert_eq!(read(&io, "data"), b"next");
    let capture = io.digest_file(Path::new("data"), &|| true).unwrap();
    assert_eq!(capture.digest, <[u8; 32]>::from(Sha256::digest(b"next")));
    assert_eq!(capture.metadata.st_size, 4);
    assert_eq!(capture.metadata.st_mode & 0o777, 0o755);
    io.set_mode(Path::new("data"), 0o640).unwrap();
    assert_eq!(io.lstat(Path::new("data")).unwrap().st_mode & 0o777, 0o640);

    let mut names = Vec::new();
    io.read_directory(Path::new(""), &|| true, |name, _| {
        names.push(name.to_owned());
        Ok(())
    })
    .unwrap();
    names.sort();
    assert_eq!(names, [OsStr::new("data"), OsStr::new("nested")]);
    io.remove_directory(Path::new("nested/empty")).unwrap();
    io.remove_directory(Path::new("nested")).unwrap();
    assert_eq!(fs::read(backing.join("data")).unwrap(), b"next");
    assert_eq!(fs::read(display.join("data")).unwrap(), b"foreign");
    let final_root = io.lstat(Path::new("")).unwrap();
    assert_eq!(
        (original.st_dev, original.st_ino),
        (final_root.st_dev, final_root.st_ino)
    );
    io.unlink(Path::new("data")).unwrap();
    assert!(!backing.join("data").exists());
    assert_eq!(fs::read(display.join("data")).unwrap(), b"foreign");
}

#[test]
fn ensure_directory_preserves_mkdir_umask_and_existing_modes() {
    let fixture = tempfile::tempdir().unwrap();
    fs::create_dir(fixture.path().join("reference")).unwrap();
    let reference_mode = fs::metadata(fixture.path().join("reference"))
        .unwrap()
        .mode()
        & 0o777;
    let root = File::open(fixture.path()).unwrap();
    let io = RootIo::new(&root, limits()).unwrap();
    io.ensure_directory(Path::new("observed"), 0o777).unwrap();
    assert_eq!(
        io.lstat(Path::new("observed")).unwrap().st_mode & 0o777,
        reference_mode
    );
    fs::set_permissions(
        fixture.path().join("observed"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    io.ensure_directory(Path::new("observed/child"), 0o777)
        .unwrap();
    assert_eq!(
        io.lstat(Path::new("observed")).unwrap().st_mode & 0o777,
        0o700
    );
    assert_eq!(
        io.lstat(Path::new("observed/child")).unwrap().st_mode & 0o777,
        reference_mode
    );
}

#[test]
fn removed_display_never_falls_back_to_new_host_directory() {
    let fixture = tempfile::tempdir().unwrap();
    let display = fixture.path().join("display");
    fs::create_dir(&display).unwrap();
    let root = File::open(&display).unwrap();
    let io = RootIo::new(&root, limits()).unwrap();
    let original = io.root_stat().unwrap();
    fs::remove_dir(&display).unwrap();
    fs::create_dir(&display).unwrap();
    fs::write(display.join("foreign"), b"protected").unwrap();
    let actual = io.root_stat().unwrap();
    assert_eq!(
        (actual.st_dev, actual.st_ino),
        (original.st_dev, original.st_ino)
    );
    assert_ne!(actual.st_ino, fs::metadata(&display).unwrap().ino());
    assert!(io.lstat(Path::new("foreign")).is_err());
    assert!(
        io.write_file(
            Path::new("foreign"),
            b"bad",
            0o644,
            WriteMode::ReplaceRegular,
            &|| true
        )
        .is_err()
    );
    assert_eq!(fs::read(display.join("foreign")).unwrap(), b"protected");
}

#[test]
fn literal_links_are_observed_and_unlinked_but_never_traversed() {
    let fixture = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(fixture.path().join("real")).unwrap();
    fs::write(fixture.path().join("real/data"), b"inside").unwrap();
    fs::write(outside.path().join("data"), b"outside").unwrap();
    symlink("real", fixture.path().join("internal")).unwrap();
    symlink(outside.path(), fixture.path().join("external")).unwrap();
    symlink("real/data", fixture.path().join("leaf")).unwrap();
    let root = File::open(fixture.path()).unwrap();
    let io = RootIo::new(&root, limits()).unwrap();
    assert_eq!(
        io.lstat(Path::new("leaf")).unwrap().st_mode & libc::S_IFMT,
        libc::S_IFLNK
    );
    let mut target = [0; 4097];
    assert_eq!(
        io.read_link(Path::new("leaf"), &mut target).unwrap(),
        b"real/data"
    );
    for name in ["internal/data", "external/data", "leaf"] {
        assert!(io.open_read_regular(Path::new(name)).is_err(), "{name}");
        assert!(
            io.open_write_regular(Path::new(name), WriteMode::ReplaceRegular)
                .is_err(),
            "{name}"
        );
    }
    assert!(
        io.ensure_directory(Path::new("external/new"), 0o755)
            .is_err()
    );
    assert!(io.remove_directory(Path::new("external")).is_err());
    assert!(io.set_mode(Path::new("leaf"), 0o600).is_err());
    assert_eq!(fs::read(outside.path().join("data")).unwrap(), b"outside");
    assert_eq!(
        fs::read(fixture.path().join("real/data")).unwrap(),
        b"inside"
    );
    assert!(!outside.path().join("new").exists());
    io.unlink(Path::new("external")).unwrap();
    assert!(outside.path().join("data").exists());
}

#[test]
fn body_link_and_create_new_exact_plus_one_are_checked_before_mutation() {
    let fixture = tempfile::tempdir().unwrap();
    let root = File::open(fixture.path()).unwrap();
    let mut bound = limits();
    bound.max_file_bytes = 3;
    bound.max_link_bytes = 3;
    let io = RootIo::new(&root, bound).unwrap();
    assert!(io.admit_file_size(3).is_ok());
    assert!(io.admit_file_size(4).is_err());
    io.write_file(
        Path::new("file"),
        b"abc",
        0o644,
        WriteMode::CreateNew,
        &|| true,
    )
    .unwrap();
    assert!(
        io.write_file(
            Path::new("file"),
            b"abcd",
            0o644,
            WriteMode::ReplaceRegular,
            &|| true
        )
        .is_err()
    );
    assert!(
        io.write_file(
            Path::new("file"),
            b"new",
            0o644,
            WriteMode::CreateNew,
            &|| true
        )
        .is_err()
    );
    assert_eq!(read(&io, "file"), b"abc");
    symlink("abc", fixture.path().join("exact")).unwrap();
    symlink("abcd", fixture.path().join("extra")).unwrap();
    let mut target = [0; 4];
    assert_eq!(
        io.read_link(Path::new("exact"), &mut target).unwrap(),
        b"abc"
    );
    assert!(io.read_link(Path::new("extra"), &mut target).is_err());
    assert!(io.read_link(Path::new("exact"), &mut [0; 3]).is_err());
    fs::write(fixture.path().join("file"), b"abcd").unwrap();
    let calls = Cell::new(0);
    assert!(
        io.read_file(Path::new("file"), &|| true, |_| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .is_err()
    );
    assert_eq!(calls.get(), 0);
}

#[test]
fn directory_entry_name_path_and_depth_bounds_precede_visitors() {
    let fixture = tempfile::tempdir().unwrap();
    let root = File::open(fixture.path()).unwrap();
    fs::write(fixture.path().join("a"), b"").unwrap();
    fs::write(fixture.path().join("b"), b"").unwrap();
    let mut bound = limits();
    bound.max_entries = 2;
    bound.max_directory_name_bytes = 2;
    let io = RootIo::new(&root, bound).unwrap();
    let visits = Cell::new(0);
    io.read_directory(Path::new(""), &|| true, |_, _| {
        visits.set(visits.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(visits.get(), 2);
    fs::write(fixture.path().join("c"), b"").unwrap();
    visits.set(0);
    assert!(
        io.read_directory(Path::new(""), &|| true, |_, _| {
            visits.set(visits.get() + 1);
            Ok(())
        })
        .is_err()
    );
    assert_eq!(visits.get(), 2);
    bound.max_entries = 3;
    bound.max_directory_name_bytes = 1;
    let io = RootIo::new(&root, bound).unwrap();
    visits.set(0);
    assert!(
        io.read_directory(Path::new(""), &|| true, |_, _| {
            visits.set(visits.get() + 1);
            Ok(())
        })
        .is_err()
    );
    assert_eq!(visits.get(), 1);
    bound.max_path_bytes = 5;
    bound.max_name_bytes = 2;
    bound.max_depth = 2;
    let io = RootIo::new(&root, bound).unwrap();
    io.ensure_directory(Path::new("aa/bb"), 0o755).unwrap();
    assert!(io.ensure_directory(Path::new("aa/bbb"), 0o755).is_err());
    assert!(io.ensure_directory(Path::new("aa/bb/c"), 0o755).is_err());
    assert!(!fixture.path().join("aa/bb/c").exists());
    // A child discovered beneath an exact-depth directory also refuses before
    // the visitor gets a name it cannot represent under the same bounds.
    fs::write(fixture.path().join("aa/bb/c"), b"").unwrap();
    visits.set(0);
    assert!(
        io.read_directory(Path::new("aa/bb"), &|| true, |_, _| {
            visits.set(visits.get() + 1);
            Ok(())
        })
        .is_err()
    );
    assert_eq!(visits.get(), 0);
}

#[test]
fn lexical_paths_special_modes_and_hardlinks_refuse_without_truncation() {
    let fixture = tempfile::tempdir().unwrap();
    let root = File::open(fixture.path()).unwrap();
    let io = RootIo::new(&root, limits()).unwrap();
    for path in [
        "",
        ".",
        "..",
        "/absolute",
        "a//b",
        "a/./b",
        "a/../b",
        "a/",
        "a\0b",
    ] {
        assert!(
            io.open_write_regular(Path::new(path), WriteMode::ReplaceRegular)
                .is_err(),
            "{path:?}"
        );
        assert!(io.unlink(Path::new(path)).is_err(), "{path:?}");
    }
    assert!(io.remove_directory(Path::new("")).is_err());
    assert!(
        io.write_file(
            Path::new("special"),
            b"data",
            0o4755,
            WriteMode::CreateNew,
            &|| true
        )
        .is_err()
    );
    assert!(!fixture.path().join("special").exists());
    fs::write(fixture.path().join("data"), b"protected").unwrap();
    fs::hard_link(fixture.path().join("data"), fixture.path().join("alias")).unwrap();
    assert!(io.open_read_regular(Path::new("data")).is_err());
    assert!(
        io.open_write_regular(Path::new("data"), WriteMode::ReplaceRegular)
            .is_err()
    );
    assert!(io.unlink(Path::new("alias")).is_err());
    assert_eq!(fs::read(fixture.path().join("data")).unwrap(), b"protected");
    let mut bound = limits();
    bound.max_entries = 131_073;
    assert!(RootIo::new(&root, bound).is_err());
    let nondirectory = File::open(fixture.path().join("data")).unwrap();
    assert!(RootIo::new(&nondirectory, limits()).is_err());
}

#[test]
fn capture_rejects_mutation_after_actual_bytes_and_digest() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("data"), b"first").unwrap();
    let root = File::open(fixture.path()).unwrap();
    let io = RootIo::new(&root, limits()).unwrap();
    let mut observed = Vec::new();
    let outcome = io.read_file(Path::new("data"), &|| true, |bytes| {
        observed.extend_from_slice(bytes);
        fs::rename(fixture.path().join("data"), fixture.path().join("old"))?;
        fs::write(fixture.path().join("data"), b"other")?;
        Ok(())
    });
    assert!(outcome.is_err());
    assert_eq!(observed, b"first");
    assert_eq!(read(&io, "data"), b"other");
}

// /proc is test-only FD inventory. No source file is opened through proc text;
// fstat inspects live numeric descriptors for this fixture's unique dev/inodes.
fn fixture_descriptor_count(root: &File, file: &File) -> usize {
    let root = linux::stat(root).unwrap();
    let file = linux::stat(file).unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| {
            let fd = entry.ok()?.file_name().to_str()?.parse::<i32>().ok()?;
            let mut result = std::mem::MaybeUninit::<libc::stat>::uninit();
            // SAFETY: fstat initializes result on success and borrows the numeric
            // FD only for this syscall. Parallel unrelated FD churn is ignored.
            if unsafe { libc::fstat(fd, result.as_mut_ptr()) } != 0 {
                return None;
            }
            let result = unsafe { result.assume_init() };
            ((result.st_dev, result.st_ino) == (root.st_dev, root.st_ino)
                || (result.st_dev, result.st_ino) == (file.st_dev, file.st_ino))
                .then_some(())
        })
        .count()
}

#[test]
fn callback_errors_cancellation_and_unwind_release_temporary_descriptors() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("data"), b"body").unwrap();
    let root = File::open(fixture.path()).unwrap();
    let file = File::open(fixture.path().join("data")).unwrap();
    let io = RootIo::new(&root, limits()).unwrap();
    let before = fixture_descriptor_count(&root, &file);
    for _ in 0..16 {
        assert!(
            io.read_file(Path::new("data"), &|| true, |_| Err(io::Error::other(
                "capture failed"
            )))
            .is_err()
        );
        assert!(
            io.read_directory(Path::new(""), &|| true, |_, _| Err(io::Error::other(
                "directory capture failed"
            )))
            .is_err()
        );
        assert!(
            io.read_file(Path::new("data"), &|| false, |_| Ok(()))
                .is_err()
        );
        assert!(
            io.write_file(
                Path::new("data"),
                b"bad",
                0o644,
                WriteMode::ReplaceRegular,
                &|| false
            )
            .is_err()
        );
    }
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = io.read_directory(Path::new(""), &|| true, |_, _| panic!("capture panic"));
    }));
    assert!(panic.is_err());
    assert_eq!(fixture_descriptor_count(&root, &file), before);
    assert_eq!(read(&io, "data"), b"body");
    assert!(nix::fcntl::fcntl(&root, nix::fcntl::FcntlArg::F_GETFD).is_ok());
    assert_ne!(root.as_raw_fd(), file.as_raw_fd());
}

#[test]
fn archive_paths_beyond_delete_depth_remain_rooted_and_bounded() {
    let fixture = tempfile::tempdir().unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let mut bound = limits();
    bound.max_depth = 258;
    let io = RootIo::new(&descriptor, bound).unwrap();
    let directory = vec!["part"; 257].join("/");
    io.ensure_directory(Path::new(&directory), 0o755).unwrap();
    let file = format!("{directory}/file");
    io.write_file(
        Path::new(&file),
        b"deep",
        0o644,
        WriteMode::CreateNew,
        &|| true,
    )
    .unwrap();
    assert_eq!(read(&io, &file), b"deep");
    assert!(
        io.ensure_directory(Path::new(&format!("{directory}/one/extra")), 0o755)
            .is_err()
    );
    bound.max_depth = 2049;
    assert!(RootIo::new(&descriptor, bound).is_err());
}
