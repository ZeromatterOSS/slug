use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use dice::DetectCycles;
use dice::Dice;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;
use slug_loading_v2::load_label::LoadLabel;

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-loading-bzl-{name}-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn directory_snapshot(root: &Path) -> WorkspaceDirectorySnapshot {
    let mut pending = vec![root.to_path_buf()];
    let mut directories = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let file_type = entry.file_type().unwrap();
            let kind = if file_type.is_file() {
                WorkspaceDirectoryEntryKind::RegularFile
            } else if file_type.is_dir() {
                pending.push(entry.path());
                WorkspaceDirectoryEntryKind::Directory
            } else if file_type.is_symlink() {
                WorkspaceDirectoryEntryKind::Symlink
            } else {
                WorkspaceDirectoryEntryKind::Other
            };
            entries.push(WorkspaceDirectoryEntry {
                name: entry.file_name().to_str().unwrap().into(),
                kind,
            });
        }
        directories.push((directory, WorkspaceDirectoryValue::present(entries)));
    }
    WorkspaceDirectorySnapshot {
        directories: Arc::new(directories.into_iter().collect()),
    }
}

fn load_package(
    dice: &Arc<Dice>,
    runtime: &tokio::runtime::Runtime,
    workspace: &Path,
    package: &Path,
    bzl_paths: &[PathBuf],
) -> anyhow::Result<slug_loading_v2::LoadedPackage> {
    let paths = [
        vec![
            workspace.join("MODULE.bazel"),
            workspace.join("BUILD.bazel"),
            package.join("BUILD.bazel"),
            package.join("BUILD"),
        ],
        bzl_paths.to_vec(),
    ]
    .concat();
    let files: starlark_map::sorted_map::SortedMap<PathBuf, WorkspaceFileValue> = paths
        .into_iter()
        .map(|path| {
            let value = match fs::read_to_string(&path) {
                Ok(source) => WorkspaceFileValue::Present(Arc::new(source)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    WorkspaceFileValue::Absent
                }
                Err(error) => WorkspaceFileValue::ReadError(Arc::new(error.to_string())),
            };
            (path, value)
        })
        .collect();
    let evaluator = BzlModuleEvaluator::new(workspace)?;
    runtime.block_on(async {
        let mut updater = dice.updater();
        updater.changed_to(vec![(
            (WorkspaceSnapshotKey {
                workspace: workspace.to_path_buf(),
            }),
            Arc::new(WorkspaceSnapshot {
                files: Arc::new(files),
            }),
        )])?;
        updater.changed_to(vec![(
            (WorkspaceDirectorySnapshotKey {
                workspace: workspace.to_path_buf(),
            }),
            Arc::new(directory_snapshot(workspace)),
        )])?;
        let mut transaction = updater.commit().await;
        evaluator.evaluate_package(&mut transaction, package).await
    })
}

fn evaluate_load(
    dice: &Arc<Dice>,
    runtime: &tokio::runtime::Runtime,
    workspace: &Path,
    package: &Path,
    bzl_paths: &[PathBuf],
    load: &str,
) -> anyhow::Result<slug_loading_v2::EvaluatedBzlModule> {
    let paths = [
        vec![
            workspace.join("MODULE.bazel"),
            workspace.join("BUILD.bazel"),
            package.join("BUILD.bazel"),
            package.join("BUILD"),
        ],
        bzl_paths.to_vec(),
    ]
    .concat();
    let files = paths
        .into_iter()
        .map(|path| {
            let value = match fs::read_to_string(&path) {
                Ok(source) => WorkspaceFileValue::Present(Arc::new(source)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    WorkspaceFileValue::Absent
                }
                Err(error) => WorkspaceFileValue::ReadError(Arc::new(error.to_string())),
            };
            (path, value)
        })
        .collect();
    let evaluator = BzlModuleEvaluator::new(workspace)?;
    runtime.block_on(async {
        let mut updater = dice.updater();
        updater.changed_to(vec![(
            (WorkspaceSnapshotKey {
                workspace: workspace.to_path_buf(),
            }),
            Arc::new(WorkspaceSnapshot {
                files: Arc::new(files),
            }),
        )])?;
        updater.changed_to(vec![(
            (WorkspaceDirectorySnapshotKey {
                workspace: workspace.to_path_buf(),
            }),
            Arc::new(directory_snapshot(workspace)),
        )])?;
        let mut transaction = updater.commit().await;
        evaluator
            .evaluate_load(&mut transaction, package, load)
            .await
    })
}

#[test]
fn load_label_must_point_to_bzl_file() {
    let load = LoadLabel::parse("//pkg:defs.bzl").unwrap();
    assert_eq!(load.label().to_string(), "//pkg:defs.bzl");
    assert!(LoadLabel::parse("@repo//pkg:defs.bzl").is_ok());
    assert!(LoadLabel::parse("//pkg:not_defs.txt").is_err());
}

#[test]
fn injected_bzl_create_edit_delete_replays_the_loaded_package() {
    let workspace = scratch("workspace-file-input");
    let package = workspace.join("pkg");
    let definitions = package.join("defs.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"declare\")\ndeclare()\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let missing = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    );
    assert!(missing.unwrap_err().to_string().contains("absent"));

    write(
        &definitions,
        "def declare():\n    native.filegroup(name = \"before\", srcs = [])\n",
    );
    let initial = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    )
    .unwrap();
    assert_eq!(initial.targets[0].name, "before");

    write(
        &definitions,
        "def declare():\n    native.filegroup(name = \"after\", srcs = [])\n",
    );
    let edited = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    )
    .unwrap();
    assert_eq!(edited.targets[0].name, "after");

    fs::remove_file(&definitions).unwrap();
    let deleted = load_package(&dice, &runtime, &workspace, &package, &[definitions]);
    assert!(deleted.unwrap_err().to_string().contains("absent"));

    write(
        &package.join("defs.bzl"),
        "def declare():\n    native.filegroup(name = \"recreated\", srcs = [])\n",
    );
    let recreated = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[package.join("defs.bzl")],
    )
    .unwrap();
    assert_eq!(recreated.targets[0].name, "recreated");
}

#[test]
fn injected_build_primary_absence_selects_build_fallback() {
    let workspace = scratch("build-fallback");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD"),
        "filegroup(name = \"fallback\", srcs = [])\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let loaded = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();
    assert_eq!(loaded.targets[0].name, "fallback");
}

#[test]
fn local_loader_rejects_external_repository_before_mapping_exists() {
    let workspace = scratch("external");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\"@other//:defs.bzl\", \"declare\")\ndeclare()\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let error = load_package(&dice, &runtime, &workspace, &package, &[])
        .unwrap_err()
        .to_string();
    assert!(error.contains("external repository load"), "{error}");
}

#[test]
fn package_manifest_preserves_direct_edges_and_first_seen_diamond_closure() {
    let workspace = scratch("manifest-diamond");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":a.bzl\", \"declare_a\")\nload(\":b.bzl\", \"declare_b\")\ndeclare_a()\ndeclare_b()\n",
    );
    write(
        &package.join("a.bzl"),
        "load(\":shared.bzl\", \"first\")\nload(\":shared.bzl\", \"second\")\nload(\":other.bzl\", \"other\")\ndef declare_a():\n    native.filegroup(name = \"a\", srcs = [])\n",
    );
    write(
        &package.join("b.bzl"),
        "load(\":shared.bzl\", \"shared\")\ndef declare_b():\n    native.filegroup(name = \"b\", srcs = [])\n",
    );
    write(
        &package.join("shared.bzl"),
        "first = 1\nsecond = 2\nshared = 3\n",
    );
    write(&package.join("other.bzl"), "other = 3\n");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let loaded = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[
            package.join("a.bzl"),
            package.join("b.bzl"),
            package.join("shared.bzl"),
            package.join("other.bzl"),
        ],
    )
    .unwrap();

    assert_eq!(
        loaded
            .direct_load_roots
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        vec!["@@//pkg:a.bzl", "@@//pkg:b.bzl"]
    );
    assert_eq!(
        loaded
            .reachable_loads
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        vec![
            "@@//pkg:a.bzl",
            "@@//pkg:shared.bzl",
            "@@//pkg:other.bzl",
            "@@//pkg:b.bzl",
        ]
    );
    assert_eq!(loaded.direct_load_roots.len(), 2);
    assert_eq!(loaded.reachable_loads.len(), 4);
    let a = evaluate_load(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[
            package.join("a.bzl"),
            package.join("shared.bzl"),
            package.join("other.bzl"),
        ],
        ":a.bzl",
    )
    .unwrap();
    assert_eq!(
        a.manifest
            .direct_children
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        vec!["@@//pkg:shared.bzl", "@@//pkg:other.bzl"]
    );
    assert_eq!(
        a.manifest
            .reachable
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        vec!["@@//pkg:a.bzl", "@@//pkg:shared.bzl", "@@//pkg:other.bzl",]
    );
}

#[test]
fn manifest_changes_when_leaf_content_or_load_edge_changes_without_target_change() {
    let workspace = scratch("manifest-equality");
    let package = workspace.join("pkg");
    let build = package.join("BUILD.bazel");
    let defs = package.join("defs.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(&build, "load(\":defs.bzl\", \"declare\")\ndeclare()\n");
    write(
        &defs,
        "def declare():\n    native.filegroup(name = \"same\", srcs = [])\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let initial = load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();

    write(
        &defs,
        "# semantic declaration unchanged\ndef declare():\n    native.filegroup(name = \"same\", srcs = [])\n",
    );
    let leaf_edited = load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_eq!(initial.targets, leaf_edited.targets);
    assert_ne!(initial, leaf_edited);

    let shared = package.join("shared.bzl");
    write(&shared, "marker = 1\n");
    write(
        &defs,
        "load(\":shared.bzl\", \"marker\")\ndef declare():\n    native.filegroup(name = \"same\", srcs = [])\n",
    );
    let transitive_edge_changed = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[defs.clone(), shared.clone()],
    )
    .unwrap();
    assert_eq!(initial.targets, transitive_edge_changed.targets);
    assert_ne!(leaf_edited, transitive_edge_changed);

    let alternate = package.join("alternate.bzl");
    write(
        &alternate,
        "def declare():\n    native.filegroup(name = \"same\", srcs = [])\n",
    );
    write(&build, "load(\":alternate.bzl\", \"declare\")\ndeclare()\n");
    let edge_changed = load_package(&dice, &runtime, &workspace, &package, &[alternate]).unwrap();
    assert_eq!(initial.targets, edge_changed.targets);
    assert_ne!(transitive_edge_changed, edge_changed);
}

#[test]
fn same_dice_load_edges_invalidate_and_restore_without_target_changes() {
    let workspace = scratch("same-dice-load-edges");
    let package = workspace.join("pkg");
    let build = package.join("BUILD.bazel");
    let defs = package.join("defs.bzl");
    let shared = package.join("shared.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let no_direct_load = "filegroup(name = \"same\", srcs = [])\n";
    write(&build, no_direct_load);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let direct_absent = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();

    write(&defs, "marker = 1\n");
    let direct_load = "load(\":defs.bzl\", \"marker\")\nfilegroup(name = \"same\", srcs = [])\n";
    write(&build, direct_load);
    let direct_created =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_eq!(direct_absent.targets, direct_created.targets);
    assert_ne!(direct_absent, direct_created);

    write(&build, no_direct_load);
    let direct_deleted = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();
    assert_eq!(direct_absent, direct_deleted);

    write(&build, direct_load);
    let direct_recreated =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_eq!(direct_created, direct_recreated);

    let transitive_build = "load(\":defs.bzl\", \"declare\")\ndeclare()\n";
    let no_transitive_load = "def declare():\n    native.filegroup(name = \"same\", srcs = [])\n";
    write(&build, transitive_build);
    write(&defs, no_transitive_load);
    let transitive_absent =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();

    write(&shared, "marker = 1\n");
    let with_transitive_load = "load(\":shared.bzl\", \"marker\")\ndef declare():\n    native.filegroup(name = \"same\", srcs = [])\n";
    write(&defs, with_transitive_load);
    let transitive_created = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[defs.clone(), shared.clone()],
    )
    .unwrap();
    assert_eq!(transitive_absent.targets, transitive_created.targets);
    assert_ne!(transitive_absent, transitive_created);

    write(&defs, no_transitive_load);
    let transitive_deleted =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_eq!(transitive_absent, transitive_deleted);

    write(&defs, with_transitive_load);
    let transitive_recreated =
        load_package(&dice, &runtime, &workspace, &package, &[defs, shared]).unwrap();
    assert_eq!(transitive_created, transitive_recreated);
}

#[test]
fn build_comment_and_whitespace_edits_do_not_change_loaded_package() {
    let workspace = scratch("build-comment-equality");
    let package = workspace.join("pkg");
    let build = package.join("BUILD.bazel");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(&build, "filegroup(name = \"same\", srcs = [])\n");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let initial = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();
    write(
        &build,
        "# formatting-only edit\nfilegroup( name = \"same\", srcs = [] )\n",
    );
    let formatted = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();
    assert_eq!(initial, formatted);
}

#[test]
fn package_equality_ignores_distinct_frozen_module_handles() {
    let workspace = scratch("manifest-frozen-equality");
    let package = workspace.join("pkg");
    let defs = package.join("defs.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"declare\")\ndeclare()\n",
    );
    write(
        &defs,
        "def declare():\n    native.filegroup(name = \"same\", srcs = [])\n",
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let left_dice = Dice::builder().build(DetectCycles::Enabled);
    let right_dice = Dice::builder().build(DetectCycles::Enabled);
    let left = load_package(&left_dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    let right = load_package(&right_dice, &runtime, &workspace, &package, &[defs]).unwrap();
    assert_eq!(left, right);
}

async fn discover_companion(
    dice: &Arc<Dice>,
    workspace: &Path,
    package: &Path,
) -> anyhow::Result<Option<slug_loading_v2::BuildFileCompanion>> {
    let mut updater = dice.updater();
    updater.changed_to(vec![(
        (WorkspaceDirectorySnapshotKey {
            workspace: workspace.to_path_buf(),
        }),
        Arc::new(directory_snapshot(workspace)),
    )])?;
    let mut transaction = updater.commit().await;
    BzlModuleEvaluator::new(workspace)?
        .discover_build_file_companion(&mut transaction, package)
        .await
}

#[test]
fn companion_lookup_uses_only_directory_observation_and_never_loads_build_contents() {
    let workspace = scratch("build-companion");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let runtime = tokio::runtime::Runtime::new().unwrap();

    write(&package.join("BUILD"), "this is broken BUILD syntax");
    let fallback = runtime
        .block_on(discover_companion(&dice, &workspace, &package))
        .unwrap();
    let fallback = fallback.expect("fallback BUILD is present");
    assert_eq!(fallback.label.to_string(), "@@//pkg:BUILD");
    assert_eq!(fallback.path, package.join("BUILD"));

    write(
        &package.join("BUILD.bazel"),
        "also broken, but never parsed",
    );
    let primary = runtime
        .block_on(discover_companion(&dice, &workspace, &package))
        .unwrap();
    let primary = primary.expect("primary BUILD is present");
    assert_eq!(primary.label.to_string(), "@@//pkg:BUILD.bazel");

    write(&workspace.join("BUILD.bazel"), "broken root BUILD syntax");
    let root_primary = runtime
        .block_on(discover_companion(&dice, &workspace, &workspace))
        .unwrap();
    assert_eq!(
        root_primary
            .expect("root BUILD is present")
            .label
            .to_string(),
        "@@//:BUILD.bazel"
    );

    fs::remove_file(package.join("BUILD.bazel")).unwrap();
    fs::remove_file(package.join("BUILD")).unwrap();
    assert!(
        runtime
            .block_on(discover_companion(&dice, &workspace, &package))
            .unwrap()
            .is_none()
    );

    runtime.block_on(async {
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                WorkspaceDirectorySnapshotKey {
                    workspace: workspace.clone(),
                },
                Arc::new(WorkspaceDirectorySnapshot {
                    directories: Arc::new(
                        vec![(
                            (package.clone()),
                            WorkspaceDirectoryValue::ReadError(Arc::new("denied".to_owned())),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                }),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let error = BzlModuleEvaluator::new(&workspace)
            .unwrap()
            .discover_build_file_companion(&mut transaction, &package)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("denied"), "{error}");
    });
}

#[test]
fn companion_lookup_accepts_injected_symlinks_and_rejects_non_normalized_paths() {
    let workspace = scratch("build-companion-symlink");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let runtime = tokio::runtime::Runtime::new().unwrap();

    runtime.block_on(async {
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                (WorkspaceDirectorySnapshotKey {
                    workspace: workspace.clone(),
                }),
                Arc::new(WorkspaceDirectorySnapshot {
                    directories: Arc::new(
                        vec![(
                            package.clone(),
                            WorkspaceDirectoryValue::present(vec![WorkspaceDirectoryEntry {
                                name: "BUILD".into(),
                                kind: WorkspaceDirectoryEntryKind::Symlink,
                            }]),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                }),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let evaluator = BzlModuleEvaluator::new(&workspace).unwrap();
        let fallback = evaluator
            .discover_build_file_companion(&mut transaction, &package)
            .await
            .unwrap()
            .expect("injected fallback symlink is eligible");
        assert_eq!(fallback.label.to_string(), "@@//pkg:BUILD");
        drop(transaction);

        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                (WorkspaceDirectorySnapshotKey {
                    workspace: workspace.clone(),
                }),
                Arc::new(WorkspaceDirectorySnapshot {
                    directories: Arc::new(
                        vec![(
                            package.clone(),
                            WorkspaceDirectoryValue::present(vec![
                                WorkspaceDirectoryEntry {
                                    name: "BUILD".into(),
                                    kind: WorkspaceDirectoryEntryKind::RegularFile,
                                },
                                WorkspaceDirectoryEntry {
                                    name: "BUILD.bazel".into(),
                                    kind: WorkspaceDirectoryEntryKind::Symlink,
                                },
                            ]),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                }),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let primary = evaluator
            .discover_build_file_companion(&mut transaction, &package)
            .await
            .unwrap()
            .expect("injected primary symlink is eligible");
        assert_eq!(primary.label.to_string(), "@@//pkg:BUILD.bazel");

        for invalid in [
            PathBuf::from(format!("{}/.", package.display())),
            PathBuf::from(format!("{}/nested/..", package.display())),
        ] {
            let error = evaluator
                .discover_build_file_companion(&mut transaction, invalid)
                .await
                .unwrap_err()
                .to_string();
            assert!(error.contains("normalized absolute path"), "{error}");
        }
    });
}
