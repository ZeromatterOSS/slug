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
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
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
    macro_load: Arc<str>,
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
            .map(|source| vec![(self.macro_load.to_string(), frozen_macro(source))])
            .unwrap_or_default();
        evaluate_host_package_attempts(
            ctx,
            HostPackageAttemptInput {
                repository_features: Arc::from(["repo".into()]),
                associated_module_metadata: Some(("root".into(), "".into())),
                workspace: path("/workspace"),
                logical_package_root: path("/workspace"),
                package: self.package.clone(),
                package_identifier: PackageIdentifier::new(
                    CanonicalRepoName::root(),
                    self.package.clone(),
                ),
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

#[test]
fn build_module_metadata_accepts_empty_root_projection() {
    let package = PackagePath::parse("pkg").unwrap();
    let input = HostPackageAttemptInput {
        repository_features: Arc::from([]),
        associated_module_metadata: Some(("".into(), "".into())),
        workspace: path("/workspace"),
        logical_package_root: path("/workspace"),
        package: package.clone(),
        package_identifier: PackageIdentifier::new(CanonicalRepoName::root(), package.clone()),
        package_dir: package_dir(&package),
        build_file: package_dir(&package).join("BUILD.bazel"),
        source: Arc::new(
            "filegroup(name='empty_' + module_name() + module_version())\n".to_owned(),
        ),
        package_label: package.as_str().into(),
        loaded_modules: &[],
        capture_events: false,
    };
    let HostPackageAttemptStep::Terminal(terminal) = evaluate_host_package_attempt(
        &input,
        Arc::new(SmallMap::new()),
        Arc::new(RunfilesRepositoryMapping::empty()),
    ) else {
        panic!("BUILD module metadata must not request a glob");
    };
    let loaded = terminal.result.unwrap();
    assert_eq!(
        loaded
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["empty_"]
    );
}

#[test]
fn build_module_metadata_calls_fail_closed_for_generated_and_builtin_repositories() {
    // Generated and builtin routes project no associated module metadata. Keep
    // both BUILD globals callable, but reject their use rather than treating
    // the absent declaration as an empty name or version.
    for repository in ["+extension+generated", "bazel_tools"] {
        for call in ["module_name()", "module_version()"] {
            let package = PackagePath::root();
            let input = HostPackageAttemptInput {
                repository_features: Arc::from([]),
                associated_module_metadata: None,
                workspace: path("/workspace"),
                logical_package_root: path("/workspace"),
                package: package.clone(),
                package_identifier: PackageIdentifier::new(
                    CanonicalRepoName::new(repository).unwrap(),
                    package.clone(),
                ),
                package_dir: PathBuf::from("/workspace"),
                build_file: PathBuf::from("/workspace/BUILD.bazel"),
                source: Arc::new(format!("filegroup(name={call})\n")),
                package_label: package.as_str().into(),
                loaded_modules: &[],
                capture_events: false,
            };
            let HostPackageAttemptStep::Terminal(terminal) = evaluate_host_package_attempt(
                &input,
                Arc::new(SmallMap::new()),
                Arc::new(RunfilesRepositoryMapping::empty()),
            ) else {
                panic!("metadata call must terminate without requesting a glob");
            };
            let Err(HostPackageAttemptError::Loading(error)) = terminal.result else {
                panic!("{repository} {call} must fail closed");
            };
            assert!(
                error
                    .message
                    .contains("unsupported for this repository source"),
                "{repository} {call}: {}",
                error.message
            );
        }
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

fn read_link(value: &str, target: &str) -> ScriptEntry {
    (
        demand(path(value), PathObservationOperation::ReadLink),
        PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(PathBuf::from(
            target,
        )))),
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
        macro_load: Arc::from(":defs.bzl"),
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
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(PathBuf::from(
            "/workspace/pkg/defs.bzl",
        )),
        repository_mapping: Arc::from([]),
    };
    FrozenBzlModule {
        semantic_inputs: None,
        module,
        path: PathBuf::from("/workspace/pkg/defs.bzl"),
        loads: Vec::new(),
        manifest: BzlLoadManifest {
            root: identity.clone(),
            direct_children: Arc::from([]),
            reachable: [identity]
                .into_iter()
                .map(|identity| crate::BzlModuleSourceProvenance::new(identity, [0; 32]))
                .collect(),
            fingerprint: digest(source),
        },
        retained_bzl_modules: Arc::from([]),
        bzl_load_visibility: context.bzl_load_visibility(),
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
            EvaluationEvent::RepositoryExecuteOutput { .. } => {
                panic!("unexpected repository process output")
            }
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
async fn callable_recursive_patterns_and_in_memory_excludes_share_host_traversal() {
    let mut script = prelude();
    script.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![
                (b"root.txt", PathDirectoryEntryKind::File),
                (b"nested", PathDirectoryEntryKind::Directory),
            ],
        ),
        present("/workspace/pkg/root.txt", PathNodeKind::RegularFile),
        missing("/workspace/pkg/deep"),
        present("/workspace/pkg/nested", PathNodeKind::Directory),
        missing("/workspace/pkg/nested/BUILD.bazel"),
        missing("/workspace/pkg/nested/BUILD"),
        missing("/workspace/pkg/nested/nested"),
        missing("/workspace/pkg/nested/last.txt"),
        listing(
            "/workspace/pkg/nested",
            vec![
                (b"leaf.txt", PathDirectoryEntryKind::File),
                (b"deep", PathDirectoryEntryKind::Directory),
            ],
        ),
        present("/workspace/pkg/nested/leaf.txt", PathNodeKind::RegularFile),
        present("/workspace/pkg/nested/deep", PathNodeKind::Directory),
        missing("/workspace/pkg/nested/deep/deep"),
        missing("/workspace/pkg/nested/deep/BUILD.bazel"),
        missing("/workspace/pkg/nested/deep/BUILD"),
        missing("/workspace/pkg/nested/deep/nested"),
        listing(
            "/workspace/pkg/nested/deep",
            vec![(b"last.txt", PathDirectoryEntryKind::File)],
        ),
        present(
            "/workspace/pkg/nested/deep/last.txt",
            PathNodeKind::RegularFile,
        ),
    ]);
    let source = r#"
filegroup(name = "all_txt", srcs = glob(["**/*.txt"], exclude = ["absent/**"]))
filegroup(name = "middle", srcs = glob(["**/deep/**"], exclude_directories = 0))
filegroup(name = "multiple", srcs = glob(["**/nested/**/last.txt"]))
"#;
    let mut transaction = new_transaction(script).await;
    let outcome = transaction.compute(&key("pkg", source)).await.unwrap();
    let terminal = complete(&outcome);
    let package = terminal.result.as_ref().unwrap();
    assert_eq!(
        filegroup_srcs(package, "all_txt"),
        [
            "@@//pkg:nested/deep/last.txt",
            "@@//pkg:nested/leaf.txt",
            "@@//pkg:root.txt",
        ]
    );
    assert_eq!(
        filegroup_srcs(package, "middle"),
        ["@@//pkg:nested/deep", "@@//pkg:nested/deep/last.txt"]
    );
    assert_eq!(
        filegroup_srcs(package, "multiple"),
        ["@@//pkg:nested/deep/last.txt"]
    );
}

#[tokio::test]
async fn both_bindings_accept_arbitrary_integer_exclude_directories_and_reject_other_types() {
    let mut script = prelude();
    script.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![
                (b"entry", PathDirectoryEntryKind::File),
                (b"dir", PathDirectoryEntryKind::Directory),
            ],
        ),
        present("/workspace/pkg/entry", PathNodeKind::RegularFile),
        present("/workspace/pkg/dir", PathNodeKind::Directory),
        missing("/workspace/pkg/dir/BUILD.bazel"),
        missing("/workspace/pkg/dir/BUILD"),
    ]);
    let source = r#"
filegroup(name = "global_zero", srcs = glob(["*"], exclude_directories = 0))
filegroup(name = "native_zero", srcs = native.glob(["*"], exclude_directories = 0))
filegroup(name = "global_default", srcs = glob(["*"]))
filegroup(name = "native_default", srcs = native.glob(["*"]))
filegroup(name = "global_big", srcs = glob(["*"], exclude_directories = 999999999999999999999999))
filegroup(name = "global_negative", srcs = glob(["*"], exclude_directories = -999999999999999999999999))
filegroup(name = "native_big", srcs = native.glob(["*"], exclude_directories = 999999999999999999999999))
filegroup(name = "native_negative", srcs = native.glob(["*"], exclude_directories = -999999999999999999999999))
"#;
    let mut transaction = new_transaction(script).await;
    let outcome = transaction.compute(&key("pkg", source)).await.unwrap();
    let package = complete(&outcome).result.as_ref().unwrap();
    for name in ["global_zero", "native_zero"] {
        assert_eq!(
            filegroup_srcs(package, name),
            ["@@//pkg:dir", "@@//pkg:entry"]
        );
    }
    for name in [
        "global_default",
        "native_default",
        "global_big",
        "global_negative",
        "native_big",
        "native_negative",
    ] {
        assert_eq!(filegroup_srcs(package, name), ["@@//pkg:entry"]);
    }

    let mut invalid_transaction = new_transaction(prelude()).await;
    for callable in ["glob", "native.glob"] {
        for value in ["True", "\"1\"", "None"] {
            let source = format!("{callable}([], exclude_directories = {value})\n");
            let outcome = invalid_transaction
                .compute(&key("pkg", &source))
                .await
                .unwrap();
            let Err(HostPackageAttemptError::Loading(error)) = &complete(&outcome).result else {
                panic!("expected typed binding rejection for {callable}({value})")
            };
            assert!(error.message.contains("int"), "{}", error.message);
        }
    }
}

#[tokio::test]
async fn callable_cycles_fail_only_when_matched_and_recursive_expansion_stays_typed() {
    for (pattern, include_listing) in [("self", false), ("*", true), ("**/self", true)] {
        let mut script = prelude();
        script.push(present("/workspace/pkg", PathNodeKind::Directory));
        if include_listing {
            script.push(listing(
                "/workspace/pkg",
                vec![(b"self", PathDirectoryEntryKind::Symlink)],
            ));
        }
        script.extend([
            present("/workspace/pkg/self", PathNodeKind::Symlink),
            read_link("/workspace/pkg/self", "self"),
        ]);
        let mut transaction = new_transaction(script).await;
        let source = format!("glob([\"{pattern}\"])\n");
        let outcome = transaction.compute(&key("pkg", &source)).await.unwrap();
        assert!(matches!(
            &complete(&outcome).result,
            Err(HostPackageAttemptError::Glob(
                HostGlobAttemptError::Traversal(_)
            ))
        ));
    }

    let mut unmatched = prelude();
    unmatched.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![
                (b"self", PathDirectoryEntryKind::Symlink),
                (b"entry.txt", PathDirectoryEntryKind::File),
            ],
        ),
        present("/workspace/pkg/entry.txt", PathNodeKind::RegularFile),
    ]);
    let mut transaction = new_transaction(unmatched).await;
    let outcome = transaction
        .compute(&key(
            "pkg",
            "filegroup(name = \"result\", srcs = glob([\"*.txt\"]))\n",
        ))
        .await
        .unwrap();
    let package = complete(&outcome).result.as_ref().unwrap();
    assert_eq!(filegroup_srcs(package, "result"), ["@@//pkg:entry.txt"]);

    let mut expansion = prelude();
    expansion.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![(b"match", PathDirectoryEntryKind::Symlink)],
        ),
        present("/workspace/pkg/match", PathNodeKind::Symlink),
        read_link("/workspace/pkg/match", "/a/a"),
        present("/a", PathNodeKind::Directory),
        present("/a/a", PathNodeKind::Symlink),
        read_link("/a/a", "../a"),
    ]);
    let mut transaction = new_transaction(expansion).await;
    let outcome = transaction
        .compute(&key("pkg", "glob([\"**\"])\n"))
        .await
        .unwrap();
    assert!(matches!(
        &complete(&outcome).result,
        Err(HostPackageAttemptError::Glob(
            HostGlobAttemptError::Traversal(_)
        ))
    ));
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
    let source = "package(features=['-repo', 'package'])\nprint(\"final\")\nexports_files(glob([\"entry\"], allow_empty = True))\n";
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
        assert_eq!(package.feature_defaults.repository(), ["repo"]);
        assert!(package.feature_defaults.package().is_some());
        let features = package.feature_defaults.resolve(&[], &[]);
        assert_eq!(features.enabled(), ["package"]);
        assert_eq!(features.disabled(), ["repo"]);
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

    let include_before_exclude = transaction
        .compute(&key(
            "pkg",
            "glob([\"entry\", \"missing\"], exclude = [\"bad/**x*\"])\n",
        ))
        .await
        .unwrap();
    let Err(HostPackageAttemptError::Loading(error)) = &complete(&include_before_exclude).result
    else {
        panic!("expected per-include error before deferred exclude validation")
    };
    assert!(
        error
            .message
            .contains("glob pattern 'missing' didn't match")
    );

    let invalid_exclude = transaction
        .compute(&key("pkg", "glob([\"entry\"], exclude = [\"bad/**x*\"])\n"))
        .await
        .unwrap();
    let Err(HostPackageAttemptError::Loading(error)) = &complete(&invalid_exclude).result else {
        panic!("expected deferred complex-exclude validation error")
    };
    assert!(
        error
            .message
            .contains("recursive wildcard must be its own segment")
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
async fn bzl_visibility_rejects_cross_package_host_attempt_before_build_evaluation() {
    let mut key = key(
        "consumer",
        concat!(
            "load(\"//pkg:defs.bzl\", \"EXPORTED\")\n",
            "fail(\"BUILD evaluation must not start after a denied load\")\n",
        ),
    );
    key.macro_load = Arc::from("//pkg:defs.bzl");
    key.macro_source = Some(Arc::new(
        "visibility(\"private\")\nEXPORTED = 1\n".to_owned(),
    ));
    let mut transaction = new_transaction(prelude()).await;
    let outcome = transaction.compute(&key).await.unwrap();
    let terminal = complete(&outcome);
    assert!(events(terminal).is_empty());
    let Err(HostPackageAttemptError::Loading(error)) = &terminal.result else {
        panic!("expected visibility denial: {:?}", terminal.result)
    };
    assert!(
        error.message.contains("@@//pkg:defs.bzl is not visible"),
        "{}",
        error.message
    );
    assert!(error.message.contains("package @@//consumer"));
    assert!(!error.message.contains("BUILD evaluation must not start"));
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

    let mut excluded_non_utf8 = prelude();
    excluded_non_utf8.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing("/workspace/pkg", vec![(raw, PathDirectoryEntryKind::File)]),
        present_path(raw_child("/workspace/pkg", raw), PathNodeKind::RegularFile),
    ]);
    let mut excluded_transaction = new_transaction(excluded_non_utf8).await;
    let outcome = excluded_transaction
        .compute(&key(
            "pkg",
            "filegroup(name = \"empty\", srcs = glob([\"*\"], exclude = [\"*\"], allow_empty = True))\n",
        ))
        .await
        .unwrap();
    let package = complete(&outcome).result.as_ref().unwrap();
    assert!(filegroup_srcs(package, "empty").is_empty());
}

#[tokio::test]
async fn package_attempt_call_sources_share_within_driver_scope() {
    fn attempt_input<'a>(
        modules: &'a [(String, FrozenBzlModule)],
        source: &str,
        package: &str,
    ) -> HostPackageAttemptInput<'a> {
        let package = PackagePath::parse(package).unwrap();
        HostPackageAttemptInput {
            repository_features: Arc::from([]),
            associated_module_metadata: Some(("root".into(), "".into())),
            workspace: path("/workspace"),
            logical_package_root: path("/workspace"),
            package: package.clone(),
            package_identifier: PackageIdentifier::new(CanonicalRepoName::root(), package.clone()),
            package_dir: package_dir(&package),
            build_file: package_dir(&package).join("BUILD.bazel"),
            source: Arc::new(source.to_owned()),
            package_label: package.as_str().into(),
            loaded_modules: modules,
            capture_events: true,
        }
    }
    const MACRO: &str = "def macro_glob():\n    print('macro')\n    return glob(['*.txt'])\n";
    const BUILD: &str = "load(':defs.bzl', 'macro_glob')\nprint('attempt')\nfilegroup(name='macro', srcs=macro_glob())\nfilegroup(name='literal', srcs=glob(['a.txt']))\nprint('done')\n";
    let macro_module = frozen_macro(MACRO);
    // First fixture keeps its existing [0;32] marker; the duplicate supplies
    // source-derived provenance so retaining the original first row is decisive.
    let mut duplicate = macro_module.clone();
    duplicate.manifest.reachable = Arc::from([BzlModuleSourceProvenance::new(
        duplicate.manifest.root.clone(),
        digest(MACRO),
    )]);
    let modules = vec![
        (":defs.bzl".to_owned(), macro_module),
        (":alias.bzl".to_owned(), duplicate),
    ];
    let input = attempt_input(&modules, BUILD, "pkg");
    let mut script = prelude();
    script.extend([
        present("/workspace/pkg", PathNodeKind::Directory),
        listing(
            "/workspace/pkg",
            vec![
                (b"a.txt", PathDirectoryEntryKind::File),
                (b"b.txt", PathDirectoryEntryKind::File),
            ],
        ),
        present("/workspace/pkg/a.txt", PathNodeKind::RegularFile),
        present("/workspace/pkg/b.txt", PathNodeKind::RegularFile),
    ]);
    let mut transaction = new_transaction(script).await;
    let mapping = Arc::new(RunfilesRepositoryMapping::empty());
    let mut prepared = Arc::new(SmallMap::new());
    let mut sources: PackageBzlCallSourcesSlot = None;
    let mut witness = None;
    let mut retries = 0;
    let terminal = loop {
        let step = evaluate_host_package_attempt_with_call_sources(
            &input,
            prepared.dupe(),
            mapping.dupe(),
            &mut sources,
        );
        let table = sources.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].0.as_str(), "/workspace/pkg/defs.bzl");
        assert_eq!(table[0].1, modules[0].1.manifest.reachable[0]);
        assert_eq!(
            Arc::strong_count(table),
            1,
            "attempt recorder releases its table handle"
        );
        let weak = witness.get_or_insert_with(|| Arc::downgrade(table));
        assert!(
            std::ptr::eq(weak.as_ptr(), Arc::as_ptr(table)),
            "every real attempt uses the same scoped table"
        );
        match step {
            HostPackageAttemptStep::Terminal(terminal) => break terminal,
            HostPackageAttemptStep::Pending {
                request,
                event_batch,
            } => {
                assert_eq!(
                    event_batch.events().len(),
                    2,
                    "discarded attempt has only its print prefix"
                );
                retries += 1;
                assert!(retries <= 2, "two distinct actual glob requests settle");
                let outcome = compute_host_glob_request(
                    &mut transaction,
                    input.workspace.dupe(),
                    HostGlobBoundaryScope::Root,
                    input.logical_package_root.dupe(),
                    input.package.clone(),
                    request.dupe(),
                    false,
                )
                .await
                .unwrap();
                let SourcePreparationOutcome::Complete(Ok((value, observations))) = outcome else {
                    panic!("fixture must supply the genuine prepared glob result");
                };
                assert!(observations.observations().is_empty());
                assert!(value.as_ref().is_ok());
                assert!(
                    Arc::make_mut(&mut prepared)
                        .insert(request, value)
                        .is_none()
                );
            }
        }
    };
    assert_eq!(retries, 2);
    assert_eq!(events(&terminal), ["attempt", "macro", "done"]);
    let package = terminal.result.as_ref().unwrap();
    assert_eq!(
        package
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["macro", "literal"]
    );
    assert_eq!(
        filegroup_srcs(package, "macro"),
        ["@@//pkg:a.txt", "@@//pkg:b.txt"]
    );
    assert_eq!(filegroup_srcs(package, "literal"), ["@@//pkg:a.txt"]);
    let witness = witness.unwrap();
    drop(terminal);
    drop(prepared);
    drop(sources);
    assert!(
        witness.upgrade().is_none(),
        "table retires while original loaded modules and DICE remain alive"
    );

    // frozen_macro uses a [0;32] reachable marker. Replace the changed module's
    // actual provenance with the digest of its genuine fixture evaluation source.
    const CHANGED: &str = "def macro_glob():\n    print('macro')\n    return glob(['a.txt'])\n";
    let mut changed = frozen_macro(CHANGED);
    changed.manifest.reachable = Arc::from([BzlModuleSourceProvenance::new(
        changed.manifest.root.clone(),
        digest(CHANGED),
    )]);
    let changed_modules = vec![(":defs.bzl".to_owned(), changed)];
    let changed_input = attempt_input(&changed_modules, BUILD, "pkg");
    let mut fresh = None;
    let pending = evaluate_host_package_attempt_with_call_sources(
        &changed_input,
        Arc::new(SmallMap::new()),
        mapping.dupe(),
        &mut fresh,
    );
    assert!(matches!(pending, HostPackageAttemptStep::Pending { .. }));
    let table = fresh.as_ref().unwrap().as_ref().unwrap();
    assert_ne!(table[0].1, modules[0].1.manifest.reachable[0]);
    assert_eq!(table[0].1, changed_modules[0].1.manifest.reachable[0]);
    let fresh_weak = Arc::downgrade(table);
    // Original scope is already retired; compare identities while both table allocations live.
    let mut independent = None;
    let original_pending = evaluate_host_package_attempt_with_call_sources(
        &input,
        Arc::new(SmallMap::new()),
        mapping.dupe(),
        &mut independent,
    );
    assert!(matches!(
        original_pending,
        HostPackageAttemptStep::Pending { .. }
    ));
    assert!(!Arc::ptr_eq(
        table,
        independent.as_ref().unwrap().as_ref().unwrap()
    ));
    drop(pending);
    drop(original_pending);
    drop(independent);
    drop(fresh);
    assert!(fresh_weak.upgrade().is_none());

    let no_loads = attempt_input(&[], "filegroup(name='empty')\n", "pkg");
    let mut empty = None;
    let HostPackageAttemptStep::Terminal(terminal) =
        evaluate_host_package_attempt_with_call_sources(
            &no_loads,
            Arc::new(SmallMap::new()),
            mapping.dupe(),
            &mut empty,
        )
    else {
        panic!("no-load BUILD terminates without a glob");
    };
    assert!(terminal.result.is_ok());
    assert!(
        matches!(empty, Some(None)),
        "valid empty provenance is initialized once"
    );

    let private = vec![(
        "//pkg:defs.bzl".to_owned(),
        frozen_macro("visibility('private')\nEXPORTED = 1\n"),
    )];
    for (source, visibility) in [
        ("[", false),
        (
            "load('//pkg:defs.bzl', 'EXPORTED')\nprint('must not run')\n",
            true,
        ),
    ] {
        let denied = attempt_input(&private, source, "consumer");
        let mut untouched = None;
        let HostPackageAttemptStep::Terminal(terminal) =
            evaluate_host_package_attempt_with_call_sources(
                &denied,
                Arc::new(SmallMap::new()),
                mapping.dupe(),
                &mut untouched,
            )
        else {
            panic!("parse/visibility failure cannot request a glob");
        };
        assert!(
            untouched.is_none(),
            "parse and visibility checks precede lazy construction"
        );
        assert!(events(&terminal).is_empty());
        let Err(HostPackageAttemptError::Loading(error)) = &terminal.result else {
            panic!("original typed loading error required");
        };
        assert_eq!(error.message.contains("is not visible"), visibility);
        assert!(!error.message.contains("must not run"));
    }
    println!(
        "WP1099_LAYOUT slot={}:{} arc={}:{} provenance_row={}:{} rows=1 retries={}",
        std::mem::size_of::<PackageBzlCallSourcesSlot>(),
        std::mem::align_of::<PackageBzlCallSourcesSlot>(),
        std::mem::size_of::<Arc<[(CompactString, BzlModuleSourceProvenance)]>>(),
        std::mem::align_of::<Arc<[(CompactString, BzlModuleSourceProvenance)]>>(),
        std::mem::size_of::<(CompactString, BzlModuleSourceProvenance)>(),
        std::mem::align_of::<(CompactString, BzlModuleSourceProvenance)>(),
        retries
    );
}
