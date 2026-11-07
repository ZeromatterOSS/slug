use std::cell::Cell;
use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::time::Duration;
use std::time::Instant;

use super::*;
use crate::runtime::repository_root_io::RootIoLimits;

fn root_limits() -> RootIoLimits {
    RootIoLimits {
        max_path_bytes: PATH_LIMIT,
        max_name_bytes: 255,
        max_depth: 2048,
        max_entries: LIMITS.entries,
        max_directory_name_bytes: LIMITS.paths,
        max_file_bytes: LIMITS.bytes,
        max_link_bytes: PATH_LIMIT,
    }
}

fn scope() -> Arc<SourceServiceScope> {
    use crate::runtime::dice::source_service::SourceServiceLimits;
    let deadline = Instant::now() + Duration::from_secs(5);
    SourceServiceScope::new_for_test(
        slug_workspace_v2::NormalizedAbsolutePath::new("/workspace").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap()
}

fn same_manifest(root: &RootIo<'_>, path: &Path) -> Arc<str> {
    let plain = super::super::final_manifest(path, &|| true, None).unwrap();
    let rooted = final_manifest_at(root, path, &|| true, None).unwrap();
    assert_eq!(plain, rooted);
    rooted
}

#[test]
fn rooted_manifest_matches_path_framing_body_modes_and_dfs_restoration() {
    let fixture = tempfile::tempdir().unwrap();
    fs::create_dir(fixture.path().join("zdir")).unwrap();
    fs::create_dir(fixture.path().join("adir")).unwrap();
    fs::set_permissions(
        fixture.path().join("adir"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    fs::write(fixture.path().join("zdir/b"), b"b\0\xffbody").unwrap();
    let large = vec![b'x'; 70 * 1024];
    fs::write(fixture.path().join("adir/a"), &large).unwrap();
    fs::write(fixture.path().join("middle"), b"middle").unwrap();
    fs::set_permissions(
        fixture.path().join("adir/a"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, root_limits()).unwrap();
    let first = same_manifest(&root, fixture.path());
    assert!(first.starts_with("verified-generated-archive:"));

    fs::set_permissions(
        fixture.path().join("adir/a"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert_ne!(first, same_manifest(&root, fixture.path()));
    fs::set_permissions(
        fixture.path().join("adir/a"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert_eq!(first, same_manifest(&root, fixture.path()));
    fs::write(fixture.path().join("middle"), b"Middle").unwrap();
    assert_ne!(first, same_manifest(&root, fixture.path()));
    fs::write(fixture.path().join("middle"), b"middle").unwrap();
    assert_eq!(first, same_manifest(&root, fixture.path()));

    // Recreate siblings in the opposite insertion order. Physical inode/time
    // changes are excluded from the existing framing; sorted DFS is unchanged.
    fs::remove_file(fixture.path().join("middle")).unwrap();
    fs::remove_file(fixture.path().join("zdir/b")).unwrap();
    fs::write(fixture.path().join("middle"), b"middle").unwrap();
    fs::write(fixture.path().join("zdir/b"), b"b\0\xffbody").unwrap();
    assert_eq!(first, same_manifest(&root, fixture.path()));
    fs::set_permissions(
        fixture.path().join("adir"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    assert_ne!(first, same_manifest(&root, fixture.path()));
    fs::set_permissions(
        fixture.path().join("adir"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert_eq!(first, same_manifest(&root, fixture.path()));
}

#[test]
fn rooted_manifest_ignores_absent_and_foreign_host_display_tree() {
    let fixture = tempfile::tempdir().unwrap();
    let display = fixture.path().join("display");
    let backing = fixture.path().join("backing");
    fs::create_dir_all(display.join("dir")).unwrap();
    fs::write(display.join("dir/data"), b"original").unwrap();
    let descriptor = File::open(&display).unwrap();
    let root = RootIo::new(&descriptor, root_limits()).unwrap();
    fs::rename(&display, &backing).unwrap();
    let expected = super::super::final_manifest(&backing, &|| true, None).unwrap();
    assert_eq!(
        final_manifest_at(&root, &display, &|| true, None).unwrap(),
        expected
    );
    assert!(!display.exists());
    fs::create_dir_all(display.join("dir")).unwrap();
    fs::write(display.join("dir/data"), b"foreign").unwrap();
    assert_ne!(
        super::super::final_manifest(&display, &|| true, None).unwrap(),
        expected
    );
    assert_eq!(
        final_manifest_at(&root, &display, &|| true, None).unwrap(),
        expected
    );
    assert_eq!(fs::read(display.join("dir/data")).unwrap(), b"foreign");
}

#[test]
fn rooted_manifest_rejects_links_special_nodes_and_regular_hardlinks() {
    let fixture = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("data"), b"inside").unwrap();
    fs::write(outside.path().join("data"), b"outside").unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, root_limits()).unwrap();
    for target in [Path::new("data"), outside.path()] {
        symlink(target, fixture.path().join("link")).unwrap();
        assert!(final_manifest_at(&root, fixture.path(), &|| true, None).is_err());
        fs::remove_file(fixture.path().join("link")).unwrap();
    }
    assert_eq!(fs::read(outside.path().join("data")).unwrap(), b"outside");
    fs::hard_link(fixture.path().join("data"), fixture.path().join("alias")).unwrap();
    assert!(
        final_manifest_at(&root, fixture.path(), &|| true, None)
            .unwrap_err()
            .message
            .contains("hardlinks")
    );
    // New general-root admission does not alter the old Path-only policy.
    assert!(super::super::final_manifest(fixture.path(), &|| true, None).is_ok());
    fs::remove_file(fixture.path().join("alias")).unwrap();
    nix::unistd::mkfifo(&fixture.path().join("fifo"), nix::sys::stat::Mode::S_IRUSR).unwrap();
    assert!(
        final_manifest_at(&root, fixture.path(), &|| true, None)
            .unwrap_err()
            .message
            .contains("unsupported")
    );
    assert_eq!(fs::read(fixture.path().join("data")).unwrap(), b"inside");
}

// Use the shared, unchanged frontier to derive its precise platform-specific
// PathBuf capacity bill for this two-child tree. The consumers then have to
// enforce that exact total, rather than only a per-directory name-byte count.
fn two_child_path_budget(path: &Path) -> usize {
    let limits = Limits {
        entries: 3,
        ..LIMITS
    };
    let mut frontier = Frontier {
        paths: Vec::new(),
        discovered: 0,
        charged: 0,
        limits,
    };
    frontier.push(path.to_path_buf()).unwrap();
    let parent = frontier.paths.pop().unwrap();
    frontier.push(parent.join("a")).unwrap();
    frontier.push(parent.join("b")).unwrap();
    frontier.charged + PATH_SCRATCH
}

#[test]
fn rooted_manifest_complete_entry_byte_and_frontier_exact_plus_one() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("b"), b"b").unwrap();
    fs::write(fixture.path().join("a"), b"a").unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, root_limits()).unwrap();
    let exact = Limits {
        entries: 3,
        paths: two_child_path_budget(fixture.path()),
        bytes: 2,
    };
    let expected = super::super::capture(fixture.path(), &|| true, None, exact).unwrap();
    assert_eq!(
        capture_at(&root, fixture.path(), &|| true, None, exact).unwrap(),
        expected
    );
    for (limits, error) in [
        (
            Limits {
                entries: 2,
                ..exact
            },
            "entry budget",
        ),
        (Limits { bytes: 1, ..exact }, "byte budget"),
        (
            Limits {
                paths: exact.paths - 1,
                ..exact
            },
            "path storage budget",
        ),
    ] {
        let resources = scope();
        assert!(
            capture_at(&root, fixture.path(), &|| true, Some(&resources), limits)
                .unwrap_err()
                .message
                .contains(error)
        );
        assert!(
            super::super::capture(fixture.path(), &|| true, None, limits)
                .unwrap_err()
                .message
                .contains(error)
        );
        assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    }
    let mut local = root_limits();
    local.max_file_bytes = 0;
    let narrower = RootIo::new(&descriptor, local).unwrap();
    assert!(final_manifest_at(&narrower, fixture.path(), &|| true, None).is_err());
    assert_eq!(fs::read(fixture.path().join("a")).unwrap(), b"a");
}

fn semantic_display(length: usize) -> PathBuf {
    assert!(length > 1);
    let mut result = String::from("/");
    while length - result.len() > 255 {
        result.extend(std::iter::repeat_n('x', 254));
        result.push('/');
    }
    result.extend(std::iter::repeat_n('x', length - result.len()));
    assert_eq!(result.len(), length);
    result.into()
}

#[test]
fn rooted_manifest_preserves_absolute_4096_bound_and_archive_depth_above_256() {
    let fixture = tempfile::tempdir().unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, root_limits()).unwrap();
    let empty = same_manifest(&root, fixture.path());
    assert_eq!(
        final_manifest_at(&root, &semantic_display(4096), &|| true, None).unwrap(),
        empty
    );
    assert!(final_manifest_at(&root, &semantic_display(4097), &|| true, None).is_err());
    fs::write(fixture.path().join("a"), b"a").unwrap();
    let one = same_manifest(&root, fixture.path());
    assert_eq!(
        final_manifest_at(&root, &semantic_display(4094), &|| true, None).unwrap(),
        one
    );
    assert!(final_manifest_at(&root, &semantic_display(4095), &|| true, None).is_err());
    fs::remove_file(fixture.path().join("a")).unwrap();
    let deep = std::iter::repeat_n("d", 257).collect::<Vec<_>>().join("/");
    fs::create_dir_all(fixture.path().join(&deep)).unwrap();
    same_manifest(&root, fixture.path());
    let mut process = root_limits();
    process.max_depth = 64;
    let process_root = RootIo::new(&descriptor, process).unwrap();
    assert!(final_manifest_at(&process_root, fixture.path(), &|| true, None).is_err());
}

#[test]
fn rooted_manifest_cancellation_and_refusal_release_prepaid_buffers() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("data"), vec![b'x'; 70 * 1024]).unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, root_limits()).unwrap();
    let resources = scope();
    assert_eq!(
        final_manifest_at(&root, fixture.path(), &|| true, Some(&resources)).unwrap(),
        super::super::final_manifest(fixture.path(), &|| true, None).unwrap()
    );
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        resources.snapshot().peak_buffer_bytes,
        LIMITS.paths + 64 * 1024
    );
    let calls = Cell::new(0usize);
    let active = || {
        let count = calls.get();
        calls.set(count + 1);
        count < 10
    };
    assert!(final_manifest_at(&root, fixture.path(), &active, Some(&resources)).is_err());
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        fs::read(fixture.path().join("data")).unwrap(),
        vec![b'x'; 70 * 1024]
    );
    resources.cancel();
    assert!(final_manifest_at(&root, fixture.path(), &|| true, Some(&resources)).is_err());
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
}

#[test]
fn rooted_manifest_framing_metadata_refuses_changed_file_before_streaming() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("data"), b"old").unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, root_limits()).unwrap();
    let framed = root.lstat(Path::new("data")).unwrap();
    let mut digest = Sha256::new();
    digest.update(b"already-framed");
    let before = digest.clone().finalize();
    fs::write(fixture.path().join("data"), vec![b'x'; 70 * 1024]).unwrap();
    assert!(
        stream_regular(
            &root,
            Path::new("data"),
            &framed,
            3,
            &mut digest,
            &|| true,
            None
        )
        .unwrap_err()
        .message
        .contains("before body capture")
    );
    assert_eq!(digest.finalize(), before);
}

#[test]
fn rooted_manifest_stream_refuses_replaced_literal_link_before_reading_body() {
    let fixture = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("data"), b"inside").unwrap();
    fs::write(outside.path().join("data"), b"outside").unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, root_limits()).unwrap();
    let framed = root.lstat(Path::new("data")).unwrap();
    fs::remove_file(fixture.path().join("data")).unwrap();
    symlink(outside.path().join("data"), fixture.path().join("data")).unwrap();
    let mut digest = Sha256::new();
    digest.update(b"already-framed");
    let before = digest.clone().finalize();
    assert!(
        stream_regular(
            &root,
            Path::new("data"),
            &framed,
            6,
            &mut digest,
            &|| true,
            None,
        )
        .is_err()
    );
    assert_eq!(digest.finalize(), before);
    assert_eq!(fs::read(outside.path().join("data")).unwrap(), b"outside");
}
