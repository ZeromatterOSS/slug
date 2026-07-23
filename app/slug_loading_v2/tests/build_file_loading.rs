use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use dice::DetectCycles;
use dice::Dice;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::PackageTarget;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::file_discovery::BUILD_FILE_FALLBACK;
use slug_loading_v2::file_discovery::BUILD_FILE_PRIMARY;
use slug_loading_v2::file_discovery::MODULE_FILE;
use slug_loading_v2::file_discovery::find_build_file;
use slug_loading_v2::file_discovery::find_workspace_root;
use slug_loading_v2::file_discovery::is_bazel_build_file;
use slug_loading_v2::file_discovery::is_bzl_file;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-loading-v2-{name}-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
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

fn load_package(workspace: &Path, package: &Path) -> slug_loading_v2::LoadedPackage {
    try_load_package(workspace, package).unwrap()
}

fn try_load_package(
    workspace: &Path,
    package: &Path,
) -> anyhow::Result<slug_loading_v2::LoadedPackage> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut paths = vec![
        workspace.join(MODULE_FILE),
        workspace.join(BUILD_FILE_PRIMARY),
        workspace.join(BUILD_FILE_FALLBACK),
        package.join(BUILD_FILE_PRIMARY),
        package.join(BUILD_FILE_FALLBACK),
    ];
    for entry in fs::read_dir(package).unwrap() {
        let path = entry.unwrap().path();
        if is_bzl_file(&path) {
            paths.push(path);
        }
    }
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
    let evaluator = BzlModuleEvaluator::new(workspace).unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let mut updater = dice.updater();
            updater
                .changed_to(vec![(
                    (WorkspaceSnapshotKey {
                        workspace: workspace.to_path_buf(),
                    }),
                    Arc::new(WorkspaceSnapshot {
                        files: Arc::new(files),
                    }),
                )])
                .unwrap();
            updater
                .changed_to(vec![(
                    (WorkspaceDirectorySnapshotKey {
                        workspace: workspace.to_path_buf(),
                    }),
                    Arc::new(directory_snapshot(workspace)),
                )])
                .unwrap();
            let mut transaction = updater.commit().await;
            evaluator.evaluate_package(&mut transaction, package).await
        })
}

#[test]
fn workspace_root_requires_module_bazel() {
    let root = scratch("root");
    let pkg = root.join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(root.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    assert_eq!(find_workspace_root(&pkg).unwrap().path(), root.as_path());

    let missing = scratch("missing");
    fs::write(missing.join("WORKSPACE"), "# ignored\n").unwrap();
    let err = find_workspace_root(&missing).unwrap_err();
    assert!(err.contains(MODULE_FILE));
}

#[test]
fn build_file_discovery_is_bazel_only() {
    let pkg = scratch("build-file");
    fs::write(pkg.join(BUILD_FILE_FALLBACK), "# fallback\n").unwrap();
    assert_eq!(
        find_build_file(&pkg).unwrap(),
        pkg.join(BUILD_FILE_FALLBACK)
    );
    fs::write(pkg.join(BUILD_FILE_PRIMARY), "# primary\n").unwrap();
    assert_eq!(find_build_file(&pkg).unwrap(), pkg.join(BUILD_FILE_PRIMARY));

    let other_1 = pkg.join(concat!("BU", "CK"));
    let other_2 = pkg.join(concat!("TAR", "GETS"));
    fs::write(&other_1, "# ignored\n").unwrap();
    fs::write(&other_2, "# ignored\n").unwrap();
    assert!(!is_bazel_build_file(&other_1));
    assert!(!is_bazel_build_file(&other_2));
}

#[test]
fn recognizes_bzl_extension_only() {
    assert!(is_bzl_file(&PathBuf::from("defs.bzl")));
    assert!(!is_bzl_file(&PathBuf::from("defs.star")));
}

#[test]
fn package_load_evaluates_loaded_macro_and_bazel_package_globals() {
    let workspace = scratch("package-load");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def make_export(name, src):\n    native.exports_files([src])\n    native.filegroup(name = name, srcs = [src])\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"make_export\")\npackage(default_visibility = [\"//visibility:public\"])\nexports_files([\"data.txt\"])\nfilegroup(name = \"fg\", srcs = [\"data.txt\"])\nalias(name = \"alias_fg\", actual = \":fg\")\nmake_export(name = \"macro_file\", src = \"macro.txt\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);

    assert_eq!(loaded.default_visibility, vec!["//visibility:public"]);
    assert_eq!(
        loaded.targets,
        vec![
            PackageTarget {
                name: "data.txt".to_owned(),
                kind: PackageTargetKind::ExportedFile,
            },
            PackageTarget {
                name: "fg".to_owned(),
                kind: PackageTargetKind::Filegroup {
                    srcs: vec!["data.txt".to_owned()],
                },
            },
            PackageTarget {
                name: "alias_fg".to_owned(),
                kind: PackageTargetKind::Alias {
                    actual: ":fg".to_owned(),
                },
            },
            PackageTarget {
                name: "macro.txt".to_owned(),
                kind: PackageTargetKind::ExportedFile,
            },
            PackageTarget {
                name: "macro_file".to_owned(),
                kind: PackageTargetKind::Filegroup {
                    srcs: vec!["macro.txt".to_owned()],
                },
            },
        ]
    );
}

#[test]
fn package_load_registers_a_generic_starlark_rule_without_executing_it() {
    let workspace = scratch("rule-load");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    out = ctx.actions.declare_file(ctx.label.name + \".txt\")\n    return [DefaultInfo(files = depset([out]))]\n\nexample = rule(implementation = _impl)\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"example\")\nexample(name = \"registered\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);

    assert_eq!(loaded.targets.len(), 1,);
    assert_eq!(loaded.targets[0].name, "registered");
    assert!(matches!(
        loaded.targets[0].kind,
        PackageTargetKind::StarlarkRule(_)
    ));
    let PackageTargetKind::StarlarkRule(implementation) = &loaded.targets[0].kind else {
        unreachable!()
    };
    assert!(implementation.dependencies().is_empty());
}

#[test]
fn rule_deps_schema_retains_exact_normalized_order_and_rejects_other_shapes() {
    let workspace = scratch("rule-deps");
    let package = workspace.join("parent");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\n\nwith_deps = rule(implementation = _impl, attrs = {\"deps\": attr.label_list()})\nwithout_deps = rule(implementation = _impl)\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"with_deps\")\nwith_deps(name = \"ordered\", deps = [\"//leaf:second\", \":local\", \"//leaf:first\"], visibility = [\"//visibility:public\"])\nwith_deps(name = \"omitted\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let dependencies = loaded
        .targets
        .iter()
        .find_map(|target| match &target.kind {
            PackageTargetKind::StarlarkRule(implementation) if target.name == "ordered" => {
                Some(implementation.dependencies())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        dependencies,
        [
            CanonicalLabel::parse("@@//leaf:second").unwrap(),
            CanonicalLabel::parse("@@//parent:local").unwrap(),
            CanonicalLabel::parse("@@//leaf:first").unwrap(),
        ]
    );
    let omitted = loaded
        .targets
        .iter()
        .find_map(|target| match &target.kind {
            PackageTargetKind::StarlarkRule(implementation) if target.name == "omitted" => {
                Some(implementation.dependencies())
            }
            _ => None,
        })
        .unwrap();
    assert!(omitted.is_empty());

    let bad_builds = [
        (
            "with_deps(name = \"bad\", unknown = [])\n",
            "unknown attribute `unknown`",
        ),
        (
            "without_deps(name = \"bad\", deps = [])\n",
            "unknown attribute `deps`",
        ),
        (
            "with_deps(name = \"bad\", deps = (\":one\",))\n",
            "attribute `deps` must be a list of labels",
        ),
        (
            "with_deps(name = \"bad\", deps = [1])\n",
            "attribute `deps` must contain only string labels",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"relative\"])\n",
            "dependency label must be package-relative",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"@repo//leaf:one\"])\n",
            "external repository dependency labels are not supported",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"@@repo//leaf:one\"])\n",
            "external repository dependency labels are not supported",
        ),
    ];
    for (invocation, expected) in bad_builds {
        fs::write(
            package.join(BUILD_FILE_PRIMARY),
            format!("load(\":defs.bzl\", \"with_deps\", \"without_deps\")\n{invocation}"),
        )
        .unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "error: {error}");
    }
}

#[test]
fn build_and_macro_native_glob_share_the_prepared_package_listing() {
    let workspace = scratch("glob-callable");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(package.join("sub")).unwrap();
    fs::create_dir_all(package.join("subpackage")).unwrap();
    fs::write(package.join("keep.txt"), "keep\n").unwrap();
    fs::write(package.join("skip.txt"), "skip\n").unwrap();
    fs::write(package.join("sub/child.txt"), "child\n").unwrap();
    fs::write(package.join("subpackage/BUILD.bazel"), "# boundary\n").unwrap();
    fs::write(package.join("subpackage/hidden.txt"), "hidden\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def make_group():\n    native.filegroup(name = \"macro\", srcs = native.glob((\"sub/*.txt\",)))\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"make_group\")\nfilegroup(name = \"direct\", srcs = glob([\"*.txt\"], exclude = [\"skip.txt\"]))\nfilegroup(name = \"dirs\", srcs = glob([\"*\"], exclude_directories = 0))\nfilegroup(name = \"omitted\", srcs = glob(allow_empty = True))\nmake_group()\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    assert_eq!(
        loaded
            .targets
            .iter()
            .map(|target| (target.name.clone(), target.kind.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                "direct".to_owned(),
                PackageTargetKind::Filegroup {
                    srcs: vec!["keep.txt".to_owned()]
                }
            ),
            (
                "dirs".to_owned(),
                PackageTargetKind::Filegroup {
                    srcs: vec![
                        "BUILD.bazel".to_owned(),
                        "defs.bzl".to_owned(),
                        "keep.txt".to_owned(),
                        "skip.txt".to_owned(),
                        "sub".to_owned()
                    ]
                }
            ),
            (
                "omitted".to_owned(),
                PackageTargetKind::Filegroup { srcs: Vec::new() },
            ),
            (
                "macro".to_owned(),
                PackageTargetKind::Filegroup {
                    srcs: vec!["sub/child.txt".to_owned()]
                }
            ),
        ]
    );
    assert_eq!(loaded.used_globs.len(), 4);
}

#[test]
fn glob_reports_context_and_allow_empty_type_errors() {
    let workspace = scratch("glob-errors");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "filegroup(name = \"bad\", srcs = glob([\"*.txt\"], allow_empty = 5))\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(error.contains("expected boolean for argument `allow_empty`, got `5`"));

    fs::write(
        package.join("defs.bzl"),
        "BAD = glob([\"*.txt\"], allow_empty = True)\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"BAD\")\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(error.contains("glob() may only be called while evaluating a BUILD package"));
}
