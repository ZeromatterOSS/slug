use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;

use super::*;
use crate::runtime::action_output_staging::tests::Workspace;
use crate::runtime::action_output_staging::tests::configurations;
use crate::runtime::action_output_staging::tests::root;
use crate::runtime::action_output_staging::tests::scratch;

fn output() -> ActionOutput {
    ActionOutput::new("pkg/tool.runfiles", ActionOutputKind::RunfilesTree)
}
fn reserved() -> SmallSet<&'static str> {
    ["pkg", "tool.runfiles"].into_iter().collect()
}
fn link(path: &str, target: &Path) -> (String, Option<String>) {
    (path.to_owned(), Some(target.to_str().unwrap().to_owned()))
}

#[test]
fn typed_runfiles_tree_replaces_links_and_preserves_targets_modes_and_workspace() {
    let workspace = Workspace::new();
    let (config, _) = configurations();
    let target = workspace.path().join("target");
    let manifest = workspace.path().join("input_manifest");
    fs::write(&target, b"source").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    fs::write(&manifest, b"manifest").unwrap();
    let output = output();
    let entries = vec![
        link("external/data", &target),
        link("MANIFEST", &manifest),
        ("external/empty".into(), None),
        ("external/back\\slash".into(), None),
    ];
    let mut stage = Staging::new_runfiles(
        workspace.path(),
        &config,
        &output,
        "main",
        &entries,
        &reserved(),
    )
    .unwrap();
    assert!(
        stage
            .create_file(std::slice::from_ref(&output), 0, "file")
            .is_err()
    );
    assert!(
        stage
            .create_directory(std::slice::from_ref(&output), 0, "dir")
            .is_err()
    );
    stage.seal(std::slice::from_ref(&output)).unwrap();
    stage.publish(std::slice::from_ref(&output)).unwrap();
    drop(stage);
    let tree = root(&workspace, &config).join(output.path());
    assert_eq!(fs::read_link(tree.join("external/data")).unwrap(), target);
    assert_eq!(fs::read_link(tree.join("MANIFEST")).unwrap(), manifest);
    for directory in [&tree, &tree.join("main"), &tree.join("external")] {
        assert_eq!(
            fs::metadata(directory).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
    for empty in ["external/empty", "external/back\\slash"] {
        assert_eq!(
            fs::metadata(tree.join(empty)).unwrap().permissions().mode() & 0o777,
            0o444
        );
        assert_eq!(fs::read(tree.join(empty)).unwrap(), b"");
    }
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o640
    );
    let mut replacement = Staging::new_runfiles(
        workspace.path(),
        &config,
        &output,
        "main",
        &[link("main/new", &target), link("MANIFEST", &manifest)],
        &reserved(),
    )
    .unwrap();
    replacement.seal(std::slice::from_ref(&output)).unwrap();
    replacement.publish(std::slice::from_ref(&output)).unwrap();
    drop(replacement);
    assert!(!tree.join("external").exists());
    assert_eq!(fs::read(tree.join("main/new")).unwrap(), b"source");
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert!(scratch(&root(&workspace, &config)).is_empty());
}

#[test]
fn runfiles_topology_rejects_leaf_prefix_manifest_and_workspace_conflicts_before_staging() {
    let absolute = Some("/observed/target".to_owned());
    for entries in [
        vec![("a".into(), absolute.clone()), ("a/child".into(), None)],
        vec![
            ("MANIFEST".into(), absolute.clone()),
            ("MANIFEST/child".into(), None),
        ],
        vec![("main".into(), absolute.clone())],
        vec![("same".into(), None), ("same".into(), absolute.clone())],
    ] {
        assert!(validate_runfiles("main", &entries).is_err());
    }
    for path in ["", "/absolute", "../escape", "a/./b", "a//b", "a\0b"] {
        assert!(validate_runfiles("main", &[(path.into(), None)]).is_err());
    }
    for target in ["relative", "/absolute\0bad"] {
        assert!(validate_runfiles("main", &[("main/link".into(), Some(target.into()))]).is_err());
    }
    assert!(validate_runfiles("main", &[("main/back\\slash".into(), None)]).is_ok());
    assert!(validate_runfiles("main", &[]).is_ok());
    let workspace = Workspace::new();
    let (config, _) = configurations();
    assert!(
        Staging::new_runfiles(
            workspace.path(),
            &config,
            &output(),
            "main",
            &[
                ("MANIFEST".into(), absolute),
                ("MANIFEST/child".into(), None)
            ],
            &reserved()
        )
        .is_err()
    );
    assert!(!workspace.path().join("bazel-out").exists());
}

#[test]
fn runfiles_seal_checks_exact_contents_and_preflight_rejects_changed_destinations() {
    let workspace = Workspace::new();
    let (config, _) = configurations();
    let outside = workspace.path().join("outside");
    fs::write(&outside, b"untouched").unwrap();
    let output = output();
    for mutation in 0..3 {
        let mut stage = Staging::new_runfiles(
            workspace.path(),
            &config,
            &output,
            "main",
            &[link("main/link", &outside), ("main/empty".into(), None)],
            &reserved(),
        )
        .unwrap();
        let private = fd_path(&stage.destinations[0].staged);
        match mutation {
            0 => {
                fs::remove_file(private.join("main/link")).unwrap();
                symlink("/unexpected", private.join("main/link")).unwrap();
            }
            1 => fs::remove_file(private.join("main/empty")).unwrap(),
            _ => fs::write(private.join("main/extra"), b"extra").unwrap(),
        }
        assert!(stage.seal(std::slice::from_ref(&output)).is_err());
        drop(stage);
    }
    let mut stage = Staging::new_runfiles(
        workspace.path(),
        &config,
        &output,
        "main",
        &[link("main/link", &outside)],
        &reserved(),
    )
    .unwrap();
    stage.seal(std::slice::from_ref(&output)).unwrap();
    let destination = root(&workspace, &config).join(output.path());
    symlink(&outside, &destination).unwrap();
    assert!(
        stage
            .preflight_publication(std::slice::from_ref(&output))
            .is_err()
    );
    assert!(stage.publish(std::slice::from_ref(&output)).is_err());
    drop(stage);
    assert_eq!(fs::read(&outside).unwrap(), b"untouched");
    assert_eq!(fs::read_link(destination).unwrap(), outside);
    assert!(scratch(&root(&workspace, &config)).is_empty());
}
