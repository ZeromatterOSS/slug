use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::PackageTarget;
use slug_loading_v2::PackageTargetKind;
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

#[test]
fn load_label_must_point_to_bzl_file() {
    let load = LoadLabel::parse("//pkg:defs.bzl").unwrap();
    assert_eq!(load.label().to_string(), "//pkg:defs.bzl");
    assert!(LoadLabel::parse("@repo//pkg:defs.bzl").is_ok());
    assert!(LoadLabel::parse("//pkg:not_defs.txt").is_err());
}

#[test]
fn evaluator_requires_a_bzlmod_workspace_root() {
    let workspace = scratch("missing-module");
    let error = BzlModuleEvaluator::new(&workspace).err().unwrap();
    assert!(error.to_string().contains("missing MODULE.bazel"));
}

#[test]
fn transitive_bzl_load_is_cached_then_invalidated_through_dice() {
    let workspace = scratch("invalidation");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("defs.bzl"),
        "load(\":dep.bzl\", \"value\")\nanswer = value\n",
    );
    let dependency = package.join("dep.bzl");
    write(&dependency, "value = 1\n");

    let loader = BzlModuleEvaluator::new(&workspace).unwrap();
    let evaluated = loader.evaluate_load(&package, ":defs.bzl").unwrap();
    assert_eq!(evaluated.path, package.join("defs.bzl"));
    assert_eq!(evaluated.loads, vec![":dep.bzl"]);

    write(&dependency, "value = (\n");
    assert!(loader.evaluate_load(&package, ":defs.bzl").is_ok());

    loader.invalidate_path(&dependency).unwrap();
    let error = loader.evaluate_load(&package, ":defs.bzl").unwrap_err();
    assert!(error.to_string().contains("error"));
}

#[test]
fn invalidating_a_loaded_bzl_recomputes_its_dependent_package() {
    let workspace = scratch("package-invalidation");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let definitions = package.join("defs.bzl");
    write(
        &definitions,
        "def declare():\n    native.filegroup(name = \"before\", srcs = [])\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"declare\")\ndeclare()\n",
    );

    let loader = BzlModuleEvaluator::new(&workspace).unwrap();
    let initial = loader.evaluate_package(&package).unwrap();
    assert_eq!(
        initial.targets,
        vec![PackageTarget {
            name: "before".to_owned(),
            kind: PackageTargetKind::Filegroup { srcs: Vec::new() },
        }]
    );

    write(
        &definitions,
        "def declare():\n    native.filegroup(name = \"after\", srcs = [])\n",
    );
    loader.invalidate_path(&definitions).unwrap();
    let recomputed = loader.evaluate_package(&package).unwrap();
    assert_eq!(
        recomputed.targets,
        vec![PackageTarget {
            name: "after".to_owned(),
            kind: PackageTargetKind::Filegroup { srcs: Vec::new() },
        }]
    );
}

#[test]
fn invalidating_a_package_recomputes_its_build_file() {
    let workspace = scratch("build-file-invalidation");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let build_file = package.join("BUILD.bazel");
    write(&build_file, "filegroup(name = \"before\", srcs = [])\n");

    let loader = BzlModuleEvaluator::new(&workspace).unwrap();
    let initial = loader.evaluate_package(&package).unwrap();
    assert_eq!(initial.targets[0].name, "before");

    write(&build_file, "filegroup(name = \"after\", srcs = [])\n");
    assert_eq!(
        loader.evaluate_package(&package).unwrap().targets[0].name,
        "before"
    );

    loader.invalidate_package(&package).unwrap();
    assert_eq!(
        loader.evaluate_package(&package).unwrap().targets[0].name,
        "after"
    );
}

#[test]
fn local_loader_rejects_external_repository_before_mapping_exists() {
    let workspace = scratch("external");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    fs::create_dir_all(&package).unwrap();

    let loader = BzlModuleEvaluator::new(&workspace).unwrap();
    let error = loader
        .evaluate_load(&package, "@other//:defs.bzl")
        .unwrap_err();
    assert!(error.to_string().contains("external repository load"));
}
