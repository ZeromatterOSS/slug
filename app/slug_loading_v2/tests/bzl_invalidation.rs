use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use dice::DetectCycles;
use dice::Dice;
use slug_loading_v2::BzlModuleEvaluator;
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
        let mut transaction = updater.commit().await;
        evaluator.evaluate_package(&mut transaction, package).await
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
