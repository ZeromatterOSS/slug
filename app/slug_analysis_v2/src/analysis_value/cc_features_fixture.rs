//! A real //test module owns the restricted feature calls in the round-trip test.
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use dice::DetectCycles;
use dice::Dice;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::package::LegacyLoadedPackage;
use slug_loading_v2::package::PackageTargetKind;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::WorkspaceDirectoryEntry;
use slug_workspace_v2::WorkspaceDirectoryEntryKind;
use slug_workspace_v2::WorkspaceDirectorySnapshot;
use slug_workspace_v2::WorkspaceDirectorySnapshotKey;
use slug_workspace_v2::WorkspaceDirectoryValue;
use slug_workspace_v2::WorkspaceFileValue;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use slug_workspace_v2::WorkspaceSnapshot;
use slug_workspace_v2::WorkspaceSnapshotKey;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark_map::sorted_map::SortedMap;

pub(super) struct LoadedSource {
    package: LegacyLoadedPackage,
    path: std::path::PathBuf,
}
impl Drop for LoadedSource {
    fn drop(&mut self) {
        std::fs::remove_dir(&self.path).unwrap();
    }
}
impl LoadedSource {
    pub(super) async fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "slug-cc-feature-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let path = path.canonicalize().unwrap();
        let source = format!("{source}\nfixture = rule(implementation = entry)\n");
        let build = "load(':feature_retention.bzl','fixture')\nfixture(name='subject')\n";
        let dice = Dice::builder().build(DetectCycles::Disabled);
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: path.clone(),
                },
                Arc::new(WorkspaceSnapshot {
                    files: Arc::new(SortedMap::from_iter([
                        (
                            path.join("MODULE.bazel"),
                            WorkspaceFileValue::Present(Arc::new(
                                "module(name='published_fixture')\n".into(),
                            )),
                        ),
                        (
                            path.join("test/BUILD.bazel"),
                            WorkspaceFileValue::Present(Arc::new(build.into())),
                        ),
                        (
                            path.join("test/feature_retention.bzl"),
                            WorkspaceFileValue::Present(Arc::new(source)),
                        ),
                    ])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceDirectorySnapshotKey {
                    workspace: path.clone(),
                },
                Arc::new(WorkspaceDirectorySnapshot {
                    directories: Arc::new(SortedMap::from_iter([
                        (
                            path.clone(),
                            WorkspaceDirectoryValue::present(vec![
                                WorkspaceDirectoryEntry {
                                    name: "MODULE.bazel".into(),
                                    kind: WorkspaceDirectoryEntryKind::RegularFile,
                                },
                                WorkspaceDirectoryEntry {
                                    name: "test".into(),
                                    kind: WorkspaceDirectoryEntryKind::Directory,
                                },
                            ]),
                        ),
                        (
                            path.join("test"),
                            WorkspaceDirectoryValue::present(
                                ["BUILD.bazel", "feature_retention.bzl"]
                                    .into_iter()
                                    .map(|name| WorkspaceDirectoryEntry {
                                        name: name.into(),
                                        kind: WorkspaceDirectoryEntryKind::RegularFile,
                                    })
                                    .collect(),
                            ),
                        ),
                    ])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceRawSnapshotKey {
                    workspace: path.clone(),
                },
                Arc::new(WorkspaceRawSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        path.join("MODULE.bazel.lock"),
                        WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        let workspace = NormalizedAbsolutePath::new(path.clone()).unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.clone(),
                },
                RepositoryMaterializationResultEpoch::new(workspace.clone(), []).unwrap(),
            )])
            .unwrap();
        inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                workspace.clone(),
                [workspace],
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            &path,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        let package = BzlModuleEvaluator::new(&path)
            .unwrap()
            .evaluate_package(&mut transaction, &path.join("test"))
            .await
            .unwrap();
        Self { package, path }
    }
    fn rule(&self) -> &slug_loading_v2::package::StarlarkRuleImplementation {
        let PackageTargetKind::StarlarkRule(rule) = &self.package.targets[0].kind else {
            panic!("genuine loaded rule")
        };
        rule
    }

    pub(super) fn context(&self) -> super::AnalysisEvaluationContext {
        let rule = self.rule();
        super::AnalysisEvaluationContext::new(
            Arc::from([]),
            [],
            rule.definition_source().label.clone(),
            Arc::new(super::NoActions),
            starlark::values::FrozenValue::new_none(),
            rule.source_identities_by_filename().clone(),
        )
    }

    pub(super) fn invoke<'v>(
        &self,
        module: &'v Module,
        input: Value<'v>,
        context: &super::AnalysisEvaluationContext,
    ) -> Value<'v> {
        self.package.retain_bzl_heaps(module.frozen_heap());
        let callable = self.rule().frozen_value().to_value();
        let mut evaluator = Evaluator::new(module);
        evaluator.extra = Some(context);
        evaluator.eval_function(callable, &[input], &[]).unwrap()
    }
}
