use super::FilesystemObservationAdapter;
use super::scan;

#[test]
fn metadata_counts_warm_create_change_delete_and_recreate() {
    let root = tempfile::tempdir().unwrap();
    let mut adapter = FilesystemObservationAdapter::default();
    assert_eq!(adapter.observe(root.path()).unwrap(), 0);
    let file = root.path().join("source");
    std::fs::write(&file, b"a").unwrap();
    assert_eq!(adapter.observe(root.path()).unwrap(), 1);
    assert_eq!(adapter.observe(root.path()).unwrap(), 0);
    std::fs::write(&file, b"longer").unwrap();
    assert_eq!(adapter.observe(root.path()).unwrap(), 1);
    std::fs::remove_file(&file).unwrap();
    assert_eq!(adapter.observe(root.path()).unwrap(), 1);
    std::fs::write(&file, b"longer").unwrap();
    assert_eq!(adapter.observe(root.path()).unwrap(), 1);
    assert_eq!(adapter.observe(root.path()).unwrap(), 0);
}

#[test]
fn sparse_binary_hidden_and_target_files_retain_only_metadata() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("target")).unwrap();
    let sparse = root.path().join("target/large-binary");
    let file = std::fs::File::create(&sparse).unwrap();
    file.set_len(1_u64 << 40).unwrap();
    std::fs::write(root.path().join(".hidden"), [0xff, 0xfe]).unwrap();
    let files = scan(root.path(), None).0;
    assert_eq!(files.len(), 2);
    assert_eq!(files[&sparse].length, 1_u64 << 40);
    assert_eq!(files[&root.path().join(".hidden")].length, 2);
}

#[test]
fn only_root_bazel_out_is_excluded() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("bazel-out")).unwrap();
    std::fs::create_dir_all(root.path().join("nested/bazel-out")).unwrap();
    std::fs::write(root.path().join("bazel-out/product"), b"ignored").unwrap();
    let nested = root.path().join("nested/bazel-out/source");
    std::fs::write(&nested, b"included").unwrap();
    let files = scan(root.path(), None).0;
    assert_eq!(files.len(), 1);
    assert!(files.contains_key(&nested));
}

#[cfg(unix)]
#[test]
fn symlink_files_and_directories_are_not_followed() {
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    std::fs::write(external.path().join("file"), b"external").unwrap();
    std::os::unix::fs::symlink(external.path(), root.path().join("directory-link")).unwrap();
    std::os::unix::fs::symlink(external.path().join("file"), root.path().join("file-link"))
        .unwrap();
    std::os::unix::fs::symlink(root.path(), root.path().join("cycle")).unwrap();
    assert!(scan(root.path(), None).0.is_empty());
}

#[test]
fn root_canonicalization_error_preserves_previous_reporting_state() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source"), b"present").unwrap();
    let mut adapter = FilesystemObservationAdapter::default();
    assert_eq!(adapter.observe(root.path()).unwrap(), 0);
    assert!(adapter.observe(&root.path().join("missing")).is_err());
    assert_eq!(adapter.previous.as_ref().unwrap().len(), 1);
    assert_eq!(adapter.observe(root.path()).unwrap(), 0);
}

#[test]
fn overlapping_and_disjoint_scans_count_each_change_once() {
    let root = tempfile::tempdir().unwrap();
    for name in ["same", "changed", "deleted"] {
        std::fs::write(root.path().join(name), b"old").unwrap();
    }
    let mut adapter = FilesystemObservationAdapter::default();
    assert_eq!(adapter.observe(root.path()).unwrap(), 0);
    std::fs::write(root.path().join("changed"), b"longer").unwrap();
    std::fs::remove_file(root.path().join("deleted")).unwrap();
    std::fs::write(root.path().join("new"), b"new").unwrap();
    assert_eq!(adapter.observe(root.path()).unwrap(), 3);
    assert_eq!(adapter.observe(root.path()).unwrap(), 0);
    for name in ["same", "changed", "new"] {
        std::fs::remove_file(root.path().join(name)).unwrap();
    }
    std::fs::write(root.path().join("replacement"), b"new").unwrap();
    assert_eq!(adapter.observe(root.path()).unwrap(), 4);
    assert_eq!(adapter.previous.as_ref().unwrap().len(), 1);
    assert_eq!(adapter.observe(root.path()).unwrap(), 0);
}
