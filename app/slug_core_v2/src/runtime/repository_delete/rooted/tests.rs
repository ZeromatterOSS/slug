use std::cell::Cell;
use std::fs::File;
use std::os::unix::fs::symlink;
use std::time::Duration;
use std::time::Instant;

use super::*;
use crate::runtime::repository_root_io::RootIoLimits;

fn limits() -> RootIoLimits {
    RootIoLimits {
        max_path_bytes: 4096,
        max_name_bytes: 255,
        max_depth: 2048,
        max_entries: MAX_ENTRIES,
        max_directory_name_bytes: MAX_PATH_BYTES,
        max_file_bytes: 1024 * 1024 * 1024,
        max_link_bytes: 4096,
    }
}

fn scope(root: &Path, buffer_bytes: Option<usize>) -> Arc<SourceServiceScope> {
    use crate::runtime::dice::source_service::SourceServiceLimits;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut limits = SourceServiceLimits::default();
    if let Some(bytes) = buffer_bytes {
        limits.live_buffer_bytes = bytes;
    }
    SourceServiceScope::new_for_test(
        slug_workspace_v2::NormalizedAbsolutePath::new(root).unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        limits,
    )
    .unwrap()
}

#[test]
fn rooted_delete_uses_original_descriptor_with_absent_and_foreign_display() {
    let fixture = tempfile::tempdir().unwrap();
    let display = fixture.path().join("display");
    let backing = fixture.path().join("backing");
    fs::create_dir_all(display.join("tree/inner")).unwrap();
    fs::write(display.join("tree/inner/file"), b"original").unwrap();
    fs::write(display.join("leaf"), b"also original").unwrap();
    let descriptor = File::open(&display).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    fs::rename(&display, &backing).unwrap();
    let mut work = DeleteWork::default();
    assert!(delete_at(&root, "leaf", &mut work, &|| true, None).unwrap());
    assert!(!display.exists());
    assert!(!backing.join("leaf").exists());
    fs::create_dir_all(display.join("tree/inner")).unwrap();
    fs::write(display.join("tree/inner/file"), b"foreign").unwrap();
    assert!(delete_at(&root, "tree", &mut work, &|| true, None).unwrap());
    assert!(!backing.join("tree").exists());
    assert_eq!(
        fs::read(display.join("tree/inner/file")).unwrap(),
        b"foreign"
    );
    assert!(!delete_at(&root, "tree", &mut work, &|| true, None).unwrap());
}

#[test]
fn rooted_delete_unlinks_internal_and_escaping_literal_links_without_traversal() {
    let fixture = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir_all(fixture.path().join("real")).unwrap();
    fs::create_dir(fixture.path().join("links")).unwrap();
    fs::write(fixture.path().join("real/file"), b"inside").unwrap();
    fs::write(outside.path().join("file"), b"outside").unwrap();
    symlink("../real", fixture.path().join("links/internal")).unwrap();
    symlink(outside.path(), fixture.path().join("links/escape")).unwrap();
    symlink(outside.path(), fixture.path().join("ancestor")).unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    assert!(
        delete_at(
            &root,
            "ancestor/file",
            &mut DeleteWork::default(),
            &|| true,
            None
        )
        .is_err()
    );
    assert!(delete_at(&root, "links", &mut DeleteWork::default(), &|| true, None).unwrap());
    assert_eq!(
        fs::read(fixture.path().join("real/file")).unwrap(),
        b"inside"
    );
    assert_eq!(fs::read(outside.path().join("file")).unwrap(), b"outside");
    // The legacy path consumer retains its existing unsupported link boundary.
    assert!(
        super::super::delete(
            fixture.path(),
            "ancestor",
            &mut DeleteWork::default(),
            &|| true,
            None
        )
        .is_err()
    );
    assert!(
        delete_at(
            &root,
            "ancestor",
            &mut DeleteWork::default(),
            &|| true,
            None
        )
        .unwrap()
    );
    assert_eq!(fs::read(outside.path().join("file")).unwrap(), b"outside");
}

#[test]
fn rooted_delete_work_exact_plus_one_accumulates_across_the_complete_tree() {
    let fixture = tempfile::tempdir().unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    fs::create_dir(fixture.path().join("dir")).unwrap();
    fs::write(fixture.path().join("dir/a"), b"body").unwrap();
    let mut exact = DeleteWork {
        entries: MAX_ENTRIES - 2,
        path_bytes: MAX_PATH_BYTES - 8,
    };
    assert!(delete_at(&root, "dir", &mut exact, &|| true, None).unwrap());
    assert_eq!(exact.entries, MAX_ENTRIES);
    assert_eq!(exact.path_bytes, MAX_PATH_BYTES);
    assert!(delete_at(&root, "absent", &mut exact, &|| true, None).is_err());

    fs::create_dir(fixture.path().join("dir")).unwrap();
    fs::write(fixture.path().join("dir/a"), b"a").unwrap();
    fs::write(fixture.path().join("dir/b"), b"b").unwrap();
    let mut entry_overflow = DeleteWork {
        entries: MAX_ENTRIES - 2,
        path_bytes: 0,
    };
    assert!(
        delete_at(&root, "dir", &mut entry_overflow, &|| true, None)
            .unwrap_err()
            .message
            .contains("work limits")
    );
    // Overflow during discovery occurs before ANY mutation, independently of
    // the native enumeration order of the two children.
    assert_eq!(fs::read(fixture.path().join("dir/a")).unwrap(), b"a");
    assert_eq!(fs::read(fixture.path().join("dir/b")).unwrap(), b"b");
    fs::remove_file(fixture.path().join("dir/b")).unwrap();
    let mut path_overflow = DeleteWork {
        entries: 0,
        path_bytes: MAX_PATH_BYTES - 7,
    };
    assert!(
        delete_at(&root, "dir", &mut path_overflow, &|| true, None)
            .unwrap_err()
            .message
            .contains("work limits")
    );
    assert_eq!(fs::read(fixture.path().join("dir/a")).unwrap(), b"a");
    assert_eq!(path_overflow.path_bytes, MAX_PATH_BYTES - 4);
}

#[test]
fn rooted_delete_depth_exact_plus_one_and_missing_ancestor_are_bounded() {
    let fixture = tempfile::tempdir().unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let exact = std::iter::repeat_n("d", MAX_DEPTH)
        .collect::<Vec<_>>()
        .join("/");
    fs::create_dir_all(fixture.path().join(&exact)).unwrap();
    // Legacy DFS limits descendant directories, allowing a file leaf one
    // level below directory 256. The general route preserves that behavior.
    fs::write(fixture.path().join(&exact).join("file"), b"deep leaf").unwrap();
    assert!(
        super::super::delete(
            fixture.path(),
            "d",
            &mut DeleteWork::default(),
            &|| true,
            None
        )
        .unwrap()
    );
    fs::create_dir_all(fixture.path().join(&exact)).unwrap();
    fs::write(fixture.path().join(&exact).join("file"), b"deep leaf").unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("keep"), b"outside").unwrap();
    symlink(outside.path(), fixture.path().join(&exact).join("link")).unwrap();
    assert!(delete_at(&root, "d", &mut DeleteWork::default(), &|| true, None).unwrap());
    assert_eq!(fs::read(outside.path().join("keep")).unwrap(), b"outside");
    let overflow = format!("{exact}/d");
    fs::create_dir_all(fixture.path().join(&overflow)).unwrap();
    assert!(delete_at(&root, "d", &mut DeleteWork::default(), &|| true, None).is_err());
    assert!(fixture.path().join(&overflow).is_dir());
    let mut work = DeleteWork::default();
    assert!(!delete_at(&root, "absent/child", &mut work, &|| true, None).unwrap());
    assert_eq!(work.entries, 1);
    assert_eq!(work.path_bytes, "absent".len());
    fs::write(fixture.path().join("plain"), b"data").unwrap();
    assert!(
        delete_at(
            &root,
            "plain/child",
            &mut DeleteWork::default(),
            &|| true,
            None
        )
        .unwrap_err()
        .message
        .contains("non-directory")
    );
}

#[test]
fn rooted_delete_rejects_hardlinks_special_nodes_and_invalid_demands_before_mutation() {
    let fixture = tempfile::tempdir().unwrap();
    fs::create_dir(fixture.path().join("tree")).unwrap();
    fs::write(fixture.path().join("tree/file"), b"protected").unwrap();
    fs::hard_link(
        fixture.path().join("tree/file"),
        fixture.path().join("alias"),
    )
    .unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    assert!(
        delete_at(&root, "tree", &mut DeleteWork::default(), &|| true, None)
            .unwrap_err()
            .message
            .contains("hardlinks")
    );
    assert_eq!(
        fs::read(fixture.path().join("alias")).unwrap(),
        b"protected"
    );
    fs::remove_file(fixture.path().join("alias")).unwrap();
    nix::unistd::mkfifo(
        &fixture.path().join("tree/fifo"),
        nix::sys::stat::Mode::S_IRUSR,
    )
    .unwrap();
    assert!(
        delete_at(&root, "tree", &mut DeleteWork::default(), &|| true, None)
            .unwrap_err()
            .message
            .contains("special")
    );
    assert_eq!(
        fs::read(fixture.path().join("tree/file")).unwrap(),
        b"protected"
    );
    for relative in [
        "", ".", "..", "../host", "/outside", "a//b", "a/./b", "a/", "a\\b", "C:drive", "a\0b",
    ] {
        assert!(delete_at(&root, relative, &mut DeleteWork::default(), &|| true, None).is_err());
    }
}

#[test]
fn rooted_delete_frontier_credits_release_on_success_refusal_and_cancellation() {
    let fixture = tempfile::tempdir().unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let resources = scope(fixture.path(), None);
    fs::create_dir_all(fixture.path().join("tree/child")).unwrap();
    fs::write(fixture.path().join("tree/child/file"), b"body").unwrap();
    assert!(
        delete_at(
            &root,
            "tree",
            &mut DeleteWork::default(),
            &|| true,
            Some(&resources)
        )
        .unwrap()
    );
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        resources.snapshot().peak_buffer_bytes,
        ROOTED_FRONTIER_CREDIT
    );
    fs::create_dir_all(fixture.path().join("tree/child")).unwrap();
    fs::write(fixture.path().join("tree/child/file"), b"body").unwrap();
    let calls = Cell::new(0usize);
    let active = || {
        let call = calls.get();
        calls.set(call + 1);
        call < 6
    };
    assert!(
        delete_at(
            &root,
            "tree",
            &mut DeleteWork::default(),
            &active,
            Some(&resources)
        )
        .is_err()
    );
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        fs::read(fixture.path().join("tree/child/file")).unwrap(),
        b"body"
    );
    let refused = scope(fixture.path(), Some(ROOTED_FRONTIER_CREDIT - 1));
    assert!(
        delete_at(
            &root,
            "tree",
            &mut DeleteWork::default(),
            &|| true,
            Some(&refused)
        )
        .is_err()
    );
    assert_eq!(refused.snapshot().live_buffer_bytes, 0);
    assert_eq!(refused.snapshot().peak_buffer_bytes, 0);
    assert_eq!(
        fs::read(fixture.path().join("tree/child/file")).unwrap(),
        b"body"
    );
    resources.cancel();
    assert!(
        delete_at(
            &root,
            "tree",
            &mut DeleteWork::default(),
            &|| true,
            Some(&resources)
        )
        .is_err()
    );
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
}
