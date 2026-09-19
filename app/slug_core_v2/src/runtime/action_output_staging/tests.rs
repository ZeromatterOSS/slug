use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::path::PathBuf;

use slug_build_api_v2::ActionOutputKind;
use slug_configuration_v2::StarlarkOption;
use slug_configuration_v2::StarlarkOptionScope;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_identity_v2::CanonicalLabel;

use super::*;

pub(super) struct Workspace(tempfile::TempDir);
impl Workspace {
    pub(super) fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }
    pub(super) fn path(&self) -> &Path {
        self.0.path()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fn writable(path: &Path) {
            if fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()) {
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
                for child in fs::read_dir(path).unwrap() {
                    writable(&child.unwrap().path());
                }
            }
        }
        writable(self.path());
    }
}
pub(super) fn configurations() -> (SlugConfiguration, SlugConfiguration) {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    let base = SlugConfiguration::default_target(&host).unwrap();
    let other = base.with_starlark_option(StarlarkOption::string(
        CanonicalLabel::parse("@@//:setting").unwrap(),
        "transitioned",
        StarlarkOptionScope::Default,
    ));
    (base, other)
}
fn outputs() -> Vec<ActionOutput> {
    vec![
        ActionOutput::new("pkg/file", ActionOutputKind::File),
        ActionOutput::new("pkg/tree", ActionOutputKind::Directory),
    ]
}
pub(super) fn root(workspace: &Workspace, config: &SlugConfiguration) -> PathBuf {
    super::super::configured_output::configured_output_root(workspace.path(), config)
}
fn fill(stage: &ActionOutputStaging, bytes: &[u8]) {
    stage.create_file(0, "").unwrap().write_all(bytes).unwrap();
    stage.create_directory(1, "nested/empty").unwrap();
    stage
        .create_file(1, "nested/child")
        .unwrap()
        .write_all(bytes)
        .unwrap();
}
pub(super) fn scratch(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.parent().unwrap().to_path_buf()];
    while let Some(parent) = pending.pop() {
        for child in fs::read_dir(parent).unwrap() {
            let child = child.unwrap();
            if child
                .file_name()
                .to_string_lossy()
                .starts_with(".slug-output-stage-")
            {
                found.push(child.path());
            } else if child.file_type().unwrap().is_dir() {
                pending.push(child.path());
            }
        }
    }
    found
}

fn old_outputs(root: &Path) {
    fs::create_dir_all(root.join("pkg/tree")).unwrap();
    fs::write(root.join("pkg/file"), b"old file").unwrap();
    fs::write(root.join("pkg/tree/stale"), b"old tree").unwrap();
    fs::write(root.join("pkg/unrelated"), b"keep").unwrap();
}

#[test]
fn selected_file_tree_replacement_modes_empty_dirs_and_configuration_a_b_a() {
    let workspace = Workspace::new();
    let (a, b) = configurations();
    let aroot = root(&workspace, &a);
    old_outputs(&aroot);
    for (config, bytes) in [
        (&a, b"first".as_slice()),
        (&b, b"other".as_slice()),
        (&a, b"last".as_slice()),
    ] {
        let mut stage = ActionOutputStaging::new(workspace.path(), config, &outputs()).unwrap();
        fill(&stage, bytes);
        stage.seal().unwrap();
        let published = stage.publish().unwrap();
        assert_eq!(published.root(), root(&workspace, config));
        assert_eq!(published.outputs(), outputs());
        let path = published.root();
        for child in [
            "pkg/file",
            "pkg/tree",
            "pkg/tree/nested",
            "pkg/tree/nested/empty",
            "pkg/tree/nested/child",
        ] {
            assert_eq!(
                fs::metadata(path.join(child)).unwrap().permissions().mode() & 0o777,
                0o555
            );
        }
        assert_eq!(fs::read(path.join("pkg/file")).unwrap(), bytes);
        assert_eq!(fs::read(path.join("pkg/tree/nested/child")).unwrap(), bytes);
        assert!(!path.join("pkg/tree/stale").exists());
        assert!(stage.create_file(0, "").is_err());
        assert!(stage.publish().is_err());
        drop(stage);
        assert!(scratch(path).is_empty());
    }
    assert_eq!(fs::read(aroot.join("pkg/file")).unwrap(), b"last");
    assert_eq!(
        fs::read(root(&workspace, &b).join("pkg/file")).unwrap(),
        b"other"
    );
    assert_eq!(fs::read(aroot.join("pkg/unrelated")).unwrap(), b"keep");
    assert_ne!(
        fs::metadata(aroot.join("pkg"))
            .unwrap()
            .permissions()
            .mode()
            & 0o200,
        0
    );
    let marker = workspace
        .path()
        .join("bazel-out/.slug-configurations")
        .join(format!("{}.canonical", a.projection().path_component()));
    assert_eq!(fs::read(&marker).unwrap(), a.canonical_bytes());
    fs::write(marker, b.canonical_bytes()).unwrap();
    let error = ActionOutputStaging::new(workspace.path(), &a, &outputs()).unwrap_err();
    assert!(error.to_string().contains("collision"));
    assert_eq!(fs::read(aroot.join("pkg/file")).unwrap(), b"last");
}

#[test]
fn staging_rejects_escape_type_conflicts_missing_outputs_and_cleans_sealed_trees() {
    let workspace = Workspace::new();
    let (config, _) = configurations();
    let mut stage = ActionOutputStaging::new(workspace.path(), &config, &outputs()).unwrap();
    for path in [
        "../outside",
        "/absolute",
        "a//b",
        "a/./b",
        "a/../b",
        "a\\b",
        "a\0b",
    ] {
        assert!(stage.create_file(1, path).is_err(), "{path:?}");
        assert!(stage.create_directory(1, path).is_err(), "{path:?}");
    }
    assert!(stage.create_file(2, "").is_err());
    assert!(stage.create_file(0, "child").is_err());
    assert!(stage.create_directory(0, "").is_err());
    assert!(stage.create_file(1, "").is_err());
    assert!(stage.publish().is_err());
    assert!(stage.seal().is_err());
    fill(&stage, b"new");
    assert!(stage.create_file(1, "nested").is_err());
    assert!(stage.create_directory(1, "nested/child/invalid").is_err());
    stage.seal().unwrap();
    assert_eq!(scratch(&root(&workspace, &config)).len(), 2);
    drop(stage);
    assert!(scratch(&root(&workspace, &config)).is_empty());
    assert!(!root(&workspace, &config).join("pkg/file").exists());
}

#[test]
fn namespace_symlinks_are_rejected_without_touching_outside() {
    let (config, _) = configurations();
    for boundary in [
        "workspace",
        "bazel",
        "markers",
        "marker",
        "configuration",
        "bin",
        "parent",
        "leaf",
    ] {
        let workspace = Workspace::new();
        let outside = Workspace::new();
        fs::write(outside.path().join("sentinel"), b"untouched").unwrap();
        let output_root = root(&workspace, &config);
        let link = match boundary {
            "workspace" => workspace.path().join("alias"),
            "bazel" => workspace.path().join("bazel-out"),
            "markers" => workspace.path().join("bazel-out/.slug-configurations"),
            "marker" => workspace
                .path()
                .join("bazel-out/.slug-configurations")
                .join(format!(
                    "{}.canonical",
                    config.projection().path_component()
                )),
            "configuration" => output_root.parent().unwrap().to_path_buf(),
            "bin" => output_root.clone(),
            "parent" => output_root.join("pkg"),
            "leaf" => output_root.join("pkg/file"),
            _ => unreachable!(),
        };
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(outside.path(), &link).unwrap();
        let requested = if boundary == "workspace" {
            link.as_path()
        } else {
            workspace.path()
        };
        assert!(
            ActionOutputStaging::new(requested, &config, &outputs()).is_err(),
            "{boundary}"
        );
        assert_eq!(
            fs::read(outside.path().join("sentinel")).unwrap(),
            b"untouched"
        );
        assert_eq!(
            fs::read_dir(outside.path()).unwrap().count(),
            1,
            "{boundary}"
        );
    }
}

#[test]
fn changed_destination_or_root_fails_preflight_before_any_output_changes() {
    let (config, _) = configurations();
    for replace_root in [false, true] {
        let workspace = Workspace::new();
        let output_root = root(&workspace, &config);
        old_outputs(&output_root);
        let mut stage = ActionOutputStaging::new(workspace.path(), &config, &outputs()).unwrap();
        fill(&stage, b"new");
        stage.seal().unwrap();
        let changed = if replace_root {
            output_root.clone()
        } else {
            output_root.join("pkg/tree")
        };
        let retired = changed.with_extension("retired");
        fs::rename(&changed, &retired).unwrap();
        fs::create_dir(&changed).unwrap();
        let error = stage.publish().unwrap_err();
        assert!(error.to_string().contains("changed during staging"));
        let file = if replace_root {
            retired.join("pkg/file")
        } else {
            output_root.join("pkg/file")
        };
        assert_eq!(fs::read(file).unwrap(), b"old file");
        drop(stage);
        assert!(scratch(&output_root).is_empty());
    }
}

#[test]
fn late_rename_failure_reports_partial_publication_and_cleans_retired_outputs() {
    let workspace = Workspace::new();
    let (config, _) = configurations();
    let output_root = root(&workspace, &config);
    old_outputs(&output_root);
    let declared = vec![
        ActionOutput::new("pkg/file", ActionOutputKind::File),
        ActionOutput::new("pkg/unrelated", ActionOutputKind::File),
    ];
    let mut stage = ActionOutputStaging::new(workspace.path(), &config, &declared).unwrap();
    for i in 0..2 {
        stage
            .create_file(i, "")
            .unwrap()
            .write_all(if i == 0 { b"new" } else { b"second" })
            .unwrap();
    }
    stage.seal().unwrap();
    // A deterministic late filesystem failure after the first successful swap.
    let second = scratch(&output_root)
        .into_iter()
        .find(|path| fs::read(path).unwrap() == b"second")
        .unwrap();
    fs::remove_file(second).unwrap();
    let error = stage.publish().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("earlier outputs may have changed")
    );
    assert_eq!(fs::read(output_root.join("pkg/file")).unwrap(), b"new");
    assert_eq!(
        fs::read(output_root.join("pkg/unrelated")).unwrap(),
        b"keep"
    );
    assert!(stage.publish().is_err());
    drop(stage);
    assert!(scratch(&output_root).is_empty());
}

#[test]
fn retired_tree_cleanup_unlinks_symlinks_without_following_them() {
    let workspace = Workspace::new();
    let outside = Workspace::new();
    let (config, _) = configurations();
    let output_root = root(&workspace, &config);
    old_outputs(&output_root);
    fs::write(outside.path().join("sentinel"), b"untouched").unwrap();
    let outside_mode = fs::metadata(outside.path()).unwrap().permissions().mode();
    symlink(outside.path(), output_root.join("pkg/tree/link")).unwrap();
    fs::set_permissions(
        output_root.join("pkg/tree"),
        fs::Permissions::from_mode(0o555),
    )
    .unwrap();
    let mut stage = ActionOutputStaging::new(workspace.path(), &config, &outputs()).unwrap();
    fill(&stage, b"new");
    stage.seal().unwrap();
    stage.publish().unwrap();
    drop(stage);
    assert!(scratch(&output_root).is_empty());
    assert_eq!(
        fs::read(outside.path().join("sentinel")).unwrap(),
        b"untouched"
    );
    assert_eq!(
        fs::metadata(outside.path()).unwrap().permissions().mode(),
        outside_mode
    );
}

#[test]
fn tree_relative_depth_limit_excludes_private_staging_prefix() {
    let workspace = Workspace::new();
    let (config, _) = configurations();
    let mut stage = ActionOutputStaging::new(workspace.path(), &config, &outputs()).unwrap();
    stage
        .create_file(0, "")
        .unwrap()
        .write_all(b"file")
        .unwrap();
    let deepest = vec!["d"; 256].join("/");
    stage.create_directory(1, &deepest).unwrap();
    let deepest_file = format!("{}/f", vec!["d"; 255].join("/"));
    stage
        .create_file(1, &deepest_file)
        .unwrap()
        .write_all(b"deep")
        .unwrap();
    assert!(
        stage
            .create_directory(1, &format!("{deepest}/too-deep"))
            .is_err()
    );
    stage.seal().unwrap();
    let published = stage.publish().unwrap();
    assert!(published.root().join("pkg/tree").join(deepest).is_dir());
    assert_eq!(
        fs::read(published.root().join("pkg/tree").join(deepest_file)).unwrap(),
        b"deep"
    );
    drop(stage);
    assert!(scratch(published.root()).is_empty());
}

#[test]
fn empty_selected_output_set_seals_and_publishes_without_artifact_writes() {
    let workspace = Workspace::new();
    let (config, _) = configurations();
    let output_root = root(&workspace, &config);
    old_outputs(&output_root);
    let mut stage = ActionOutputStaging::new(workspace.path(), &config, &[]).unwrap();
    assert!(stage.create_file(0, "").is_err());
    stage.seal().unwrap();
    let published = stage.publish().unwrap();
    assert!(published.outputs().is_empty());
    drop(stage);
    assert_eq!(fs::read(output_root.join("pkg/file")).unwrap(), b"old file");
    assert!(scratch(&output_root).is_empty());
}
