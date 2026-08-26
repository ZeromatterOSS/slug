/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the root directory of this source tree. You may select,
 * at your option, one of the above-listed licenses.
 */

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryName;
use slug_workspace_v2::PathIoErrorKind;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationError;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::package::PackageTargetKind;

type ScriptEntry = (PathObservationDemand, PathObservationResult);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostPackageAttemptTestKey {
    package: PackagePath,
    source: Arc<String>,
    macro_source: Option<Arc<String>>,
}

impl std::fmt::Display for HostPackageAttemptTestKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "test-host-package-attempt:{}", self.package)
    }
}

#[async_trait]
impl Key for HostPackageAttemptTestKey {
    type Value = HostPackageAttemptOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let loaded_modules = self
            .macro_source
            .as_ref()
            .map(|source| vec![(":defs.bzl".to_owned(), frozen_macro(source))])
            .unwrap_or_default();
        evaluate_host_package_attempts(
            ctx,
            HostPackageAttemptInput {
                workspace: path("/workspace"),
                logical_package_root: path("/workspace"),
                package: self.package.clone(),
                package_dir: package_dir(&self.package),
                build_file: package_dir(&self.package).join("BUILD.bazel"),
                source: self.source.clone(),
                package_label: self.package.as_str().into(),
                loaded_modules: &loaded_modules,
                capture_events: true,
            },
        )
        .await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

fn path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(value).unwrap()
}

fn package_dir(package: &PackagePath) -> PathBuf {
    PathBuf::from("/workspace").join(package.as_str())
}

fn raw_child(parent: &str, child: &[u8]) -> NormalizedAbsolutePath {
    let mut path = PathBuf::from(parent);
    path.push(OsString::from_vec(child.to_vec()));
    NormalizedAbsolutePath::new(path).unwrap()
}

fn demand(
    path: NormalizedAbsolutePath,
    operation: PathObservationOperation,
) -> PathObservationDemand {
    PathObservationDemand::new(PathObservationNamespace::Host, path, operation)
}

fn present(value: &str, kind: PathNodeKind) -> ScriptEntry {
    present_path(path(value), kind)
}

fn present_path(path: NormalizedAbsolutePath, kind: PathNodeKind) -> ScriptEntry {
    (
        demand(path, PathObservationOperation::Lstat),
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            kind, 1, 2, 3, 4, 0o755,
        ))),
    )
}

fn missing(value: &str) -> ScriptEntry {
    (
        demand(path(value), PathObservationOperation::Lstat),
        PathObservationResult::Lstat(PathOperationResult::Missing),
    )
}

fn lstat_error(value: &str) -> ScriptEntry {
    (
        demand(path(value), PathObservationOperation::Lstat),
        PathObservationResult::Lstat(PathOperationResult::Error(PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        })),
    )
}

fn listing(value: &str, entries: Vec<(&[u8], PathDirectoryEntryKind)>) -> ScriptEntry {
    let entries = PathDirectoryEntries::new(entries.into_iter().map(|(name, kind)| {
        PathDirectoryEntry::new(
            PathDirectoryName::new(OsString::from_vec(name.to_vec())).unwrap(),
            kind,
        )
    }));
    (
        demand(path(value), PathObservationOperation::DirectoryEntries),
        PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries)),
    )
}

fn prelude() -> Vec<ScriptEntry> {
    vec![
        present("/", PathNodeKind::Directory),
        present("/workspace", PathNodeKind::Directory),
        missing("/workspace/REPO.bazel"),
        missing("/workspace/.bazelignore"),
    ]
}

fn policy() -> RootPackagePolicyInputs {
    RootPackagePolicyInputs::new(
        path("/workspace"),
        vec![path("/workspace")],
        std::iter::empty::<&str>(),
        None,
        Some("warning"),
    )
    .unwrap()
}

fn key(package: &str, source: &str) -> HostPackageAttemptTestKey {
    HostPackageAttemptTestKey {
        package: PackagePath::parse(package).unwrap(),
        source: Arc::new(source.to_owned()),
        macro_source: None,
    }
}

fn frozen_macro(source: &str) -> FrozenBzlModule {
    let ast = AstModule::parse(
        "/workspace/pkg/defs.bzl",
        source.to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    let module = Module::new();
    let context = BzlEvaluationContext::new("@@//pkg:defs.bzl");
    {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        evaluator.eval_module(ast, &loading_globals()).unwrap();
    }
    let module = module.freeze().unwrap();
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@//pkg:defs.bzl").unwrap(),
        workspace_path: PathBuf::from("/workspace/pkg/defs.bzl"),
        repository_mapping: Arc::from([]),
    };
    FrozenBzlModule {
        module,
        path: PathBuf::from("/workspace/pkg/defs.bzl"),
        loads: Vec::new(),
        manifest: BzlLoadManifest {
            root: identity.clone(),
            direct_children: Arc::from([]),
            reachable: Arc::from([identity]),
            fingerprint: digest(source),
        },
        retained_bzl_modules: Arc::from([]),
    }
}

fn events(terminal: &HostPackageAttemptTerminal) -> Vec<&str> {
    terminal
        .event_batch
        .events()
        .iter()
        .map(|event| match event {
            EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
            EvaluationEvent::Diagnostic { .. } => panic!("unexpected diagnostic event"),
        })
        .collect()
}

fn complete(outcome: &HostPackageAttemptOutcome) -> &HostPackageAttemptTerminal {
    let SourcePreparationOutcome::Complete(terminal) = outcome else {
        panic!("expected complete attempt outcome: {outcome:?}")
    };
    terminal
}

fn filegroup_srcs(package: &LoadedPackage, name: &str) -> Vec<String> {
    let target = package
        .targets
        .iter()
        .find(|target| target.name == name)
        .unwrap();
    let PackageTargetKind::Filegroup { srcs, .. } = &target.kind else {
        panic!("expected filegroup target {name}")
    };
    srcs.iter().map(ToString::to_string).collect()
}

async fn new_transaction(script: Vec<ScriptEntry>) -> dice::DiceTransaction {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(&mut updater, policy()).unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(script).unwrap(),
        )])
        .unwrap();
    updater.commit().await
}

#[tokio::test]
async fn composes_loaded_macro_repeated_requests_operations_and_final_state() {
    let mut script = prelude();
    script.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![
                (b"@lead", PathDirectoryEntryKind::File),
                (b"a.txt", PathDirectoryEntryKind::File),
                (b"b.txt", PathDirectoryEntryKind::File),
                (b"dir", PathDirectoryEntryKind::Directory),
                (b"excluded.txt", PathDirectoryEntryKind::File),
            ],
        ),
        present("/workspace/pkg/@lead", PathNodeKind::RegularFile),
        present("/workspace/pkg/a.txt", PathNodeKind::RegularFile),
        present("/workspace/pkg/b.txt", PathNodeKind::RegularFile),
        present("/workspace/pkg/dir", PathNodeKind::Directory),
        present("/workspace/pkg/excluded.txt", PathNodeKind::RegularFile),
        missing("/workspace/pkg/dir/BUILD.bazel"),
        missing("/workspace/pkg/dir/BUILD"),
    ]);
    let source = r#"
load(":defs.bzl", "macro_glob")
print("attempt")
filegroup(name = "partial")
filegroup(name = "macro", srcs = macro_glob())
filegroup(
    name = "txt",
    srcs = glob(["*.txt", "*.txt"], exclude = ["excluded.txt"]),
)
filegroup(name = "files", srcs = glob(["*"]))
filegroup(name = "all", srcs = glob(["*"], exclude_directories = 0))
filegroup(name = "lead", srcs = glob(["@lead"]))
print("done")
"#;
    let mut key = key("pkg", source);
    key.macro_source = Some(Arc::new(
        "def macro_glob():\n    print(\"macro\")\n    return glob([\"*.txt\"])\n".to_owned(),
    ));
    let mut transaction = new_transaction(script).await;
    let outcome = transaction.compute(&key).await.unwrap();
    let terminal = complete(&outcome);
    assert_eq!(events(terminal), ["attempt", "macro", "done"]);
    let package = terminal.result.as_ref().unwrap();
    assert_eq!(
        package
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["partial", "macro", "txt", "files", "all", "lead"]
    );
    assert_eq!(package.used_globs.len(), 5);
    assert_eq!(
        filegroup_srcs(package, "txt"),
        ["@@//pkg:a.txt", "@@//pkg:b.txt"]
    );
    assert_eq!(
        filegroup_srcs(package, "files"),
        [
            "@@//pkg:@lead",
            "@@//pkg:a.txt",
            "@@//pkg:b.txt",
            "@@//pkg:excluded.txt",
        ]
    );
    assert_eq!(
        filegroup_srcs(package, "all"),
        [
            "@@//pkg:@lead",
            "@@//pkg:a.txt",
            "@@//pkg:b.txt",
            "@@//pkg:dir",
            "@@//pkg:excluded.txt",
        ]
    );
    assert_eq!(filegroup_srcs(package, "lead"), ["@@//pkg:@lead"]);
}

#[tokio::test]
async fn forwards_need_and_restores_outputs_in_one_dice_graph() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(&mut updater, policy()).unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new([]).unwrap(),
        )])
        .unwrap();
    let source = "print(\"final\")\nexports_files(glob([\"entry\"], allow_empty = True))\n";
    let key = key("pkg", source);
    let mut transaction = updater.commit().await;
    let need = transaction.compute(&key).await.unwrap();
    let SourcePreparationOutcome::Need(need) = need else {
        panic!("missing observations must remain Need")
    };
    assert!(need.path_observations().is_some());

    let script = |present_entry| {
        let mut script = prelude();
        script.push(present("/workspace/pkg", PathNodeKind::Directory));
        script.push(if present_entry {
            present("/workspace/pkg/entry", PathNodeKind::RegularFile)
        } else {
            missing("/workspace/pkg/entry")
        });
        script
    };
    for (present_entry, expected_targets) in [
        (true, vec!["entry"]),
        (false, Vec::new()),
        (true, vec!["entry"]),
    ] {
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(script(present_entry)).unwrap(),
            )])
            .unwrap();
        transaction = updater.commit().await;
        let outcome = transaction.compute(&key).await.unwrap();
        let terminal = complete(&outcome);
        assert_eq!(events(terminal), ["final"]);
        let package = terminal.result.as_ref().unwrap();
        assert_eq!(
            package
                .targets
                .iter()
                .map(|target| target.name.as_str())
                .collect::<Vec<_>>(),
            expected_targets
        );
    }
}

#[tokio::test]
async fn distinguishes_per_include_and_all_excluded_diagnostics() {
    let mut script = prelude();
    script.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        present("/workspace/pkg/entry", PathNodeKind::RegularFile),
        missing("/workspace/pkg/missing"),
    ]);
    let mut transaction = new_transaction(script).await;
    let per_include = transaction
        .compute(&key(
            "pkg",
            "print(\"prefix\")\nglob([\"entry\", \"missing\"])\n",
        ))
        .await
        .unwrap();
    let terminal = complete(&per_include);
    assert_eq!(events(terminal), ["prefix"]);
    let Err(HostPackageAttemptError::Loading(error)) = &terminal.result else {
        panic!("expected ordinary glob diagnostic: {:?}", terminal.result)
    };
    assert!(
        error.message.contains(
            "glob pattern 'missing' didn't match anything, but allow_empty is set to False"
        )
    );

    let all_excluded = transaction
        .compute(&key(
            "pkg",
            "print(\"prefix\")\nglob([\"entry\"], exclude = [\"entry\"])\n",
        ))
        .await
        .unwrap();
    let terminal = complete(&all_excluded);
    assert_eq!(events(terminal), ["prefix"]);
    let Err(HostPackageAttemptError::Loading(error)) = &terminal.result else {
        panic!("expected ordinary glob diagnostic: {:?}", terminal.result)
    };
    assert!(
        error
            .message
            .contains("all files in the glob have been excluded, but allow_empty is set to False")
    );
}

#[tokio::test]
async fn build_environment_does_not_expose_bzl_struct_builtin() {
    let mut transaction = new_transaction(prelude()).await;
    let outcome = transaction
        .compute(&key("pkg", "VALUE = struct(enabled = True)\n"))
        .await
        .unwrap();
    let terminal = complete(&outcome);
    let Err(HostPackageAttemptError::Loading(error)) = &terminal.result else {
        panic!("expected BUILD evaluation failure: {:?}", terminal.result)
    };
    assert!(
        error.message.contains("Variable `struct` not found"),
        "{}",
        error.message
    );
}

#[tokio::test]
async fn terminal_failures_retain_typed_payload_and_only_the_print_prefix() {
    let source =
        "print(\"prefix\")\nfilegroup(name = \"partial\")\nglob([\"bad\"])\nprint(\"after\")\n";

    let mut traversal_script = prelude();
    traversal_script.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        lstat_error("/workspace/pkg/bad"),
    ]);
    let mut transaction = new_transaction(traversal_script).await;
    let outcome = transaction.compute(&key("pkg", source)).await.unwrap();
    let terminal = complete(&outcome);
    assert_eq!(events(terminal), ["prefix"]);
    let Err(HostPackageAttemptError::Glob(HostGlobAttemptError::Traversal(error))) =
        &terminal.result
    else {
        panic!("expected typed traversal terminal: {:?}", terminal.result)
    };
    assert!(format!("{error:?}").contains("raw_os_error: Some(13)"));

    let mut input_transaction = new_transaction(Vec::new()).await;
    let outcome = input_transaction
        .compute(&key("pkg/\u{100}", source))
        .await
        .unwrap();
    let terminal = complete(&outcome);
    assert_eq!(events(terminal), ["prefix"]);
    let Err(HostPackageAttemptError::Input(error)) = &terminal.result else {
        panic!("expected typed input terminal: {:?}", terminal.result)
    };
    assert!(format!("{error:?}").contains("NonLatin1PackagePathScalar"));

    let raw = b"\xff";
    let mut non_utf8_script = prelude();
    non_utf8_script.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing("/workspace/pkg", vec![(raw, PathDirectoryEntryKind::File)]),
        present_path(raw_child("/workspace/pkg", raw), PathNodeKind::RegularFile),
    ]);
    let non_utf8_source =
        "print(\"prefix\")\nfilegroup(name = \"partial\")\nglob([\"*\"])\nprint(\"after\")\n";
    let mut non_utf8_transaction = new_transaction(non_utf8_script).await;
    let outcome = non_utf8_transaction
        .compute(&key("pkg", non_utf8_source))
        .await
        .unwrap();
    let terminal = complete(&outcome);
    assert_eq!(events(terminal), ["prefix"]);
    assert!(matches!(
        &terminal.result,
        Err(HostPackageAttemptError::Glob(
            HostGlobAttemptError::UnsupportedPath { path }
        )) if path.as_ref() == raw
    ));
}
