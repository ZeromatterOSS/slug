use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

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

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-loading-v2-{name}-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
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

    let evaluator = BzlModuleEvaluator::new(&workspace).unwrap();
    let loaded = evaluator.evaluate_package(&package).unwrap();

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
        "load(\":defs.bzl\", \"example\")\nexample(name = \"registered\", arbitrary_attribute = \"kept for analysis\")\n",
    )
    .unwrap();

    let evaluator = BzlModuleEvaluator::new(&workspace).unwrap();
    let loaded = evaluator.evaluate_package(&package).unwrap();

    assert_eq!(loaded.targets.len(), 1,);
    assert_eq!(loaded.targets[0].name, "registered");
    assert!(matches!(
        loaded.targets[0].kind,
        PackageTargetKind::StarlarkRule(_)
    ));
}
