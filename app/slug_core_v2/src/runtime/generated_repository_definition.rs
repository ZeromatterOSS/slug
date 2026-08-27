/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

#[cfg(test)]
pub(super) mod tests {
    use std::sync::Arc;

    use dice::ActivationTracker;
    use dice::Dice;
    use dice::UserComputationData;
    use dupe::Dupe;
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::RegistryRequestGeneration;
    use slug_bzlmod_v2::RegistryUrls;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_bzlmod_v2::SourcePreparationOutcome;
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_identity_v2::CanonicalRepoName;
    use slug_loading_v2::HostValidatedGeneratedRepositorySpecsOutcome;
    use slug_loading_v2::HostValidatedModuleExtensionRepositoriesKey;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use starlark_map::sorted_map::SortedMap;

    pub(in crate::runtime) const WORKSPACE: &str = "/generated-repository-definition";
    pub(in crate::runtime) const MODULE: &str = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, first='first', second='second')\n";
    pub(in crate::runtime) const EXTENSION_A: &str = r#"
repo=repository_rule(implementation=lambda ctx: None, attrs={'value':attr.string(), 'target':attr.label()})
def impl(ctx):
    repo(name='first', value='one', target=':local')
    repo(name='second', value='two', target='@first//:item')
ext=module_extension(implementation=impl)
"#;

    pub(in crate::runtime) async fn transaction(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        extension_present: bool,
        tracker: Option<Arc<dyn ActivationTracker>>,
    ) -> dice::DiceTransaction {
        transaction_with_policy(
            dice,
            module,
            extension,
            extension_present,
            tracker,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        )
        .await
    }

    pub(in crate::runtime) async fn transaction_with_command_override(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        module_name: &str,
    ) -> dice::DiceTransaction {
        let override_value = format!("{module_name}={WORKSPACE}/local");
        transaction_with_policy(
            dice,
            module,
            extension,
            true,
            None,
            BzlmodCommandPolicyKey::from_flags_with_module_overrides(
                None,
                false,
                NormalizedAbsolutePath::new(WORKSPACE).unwrap().as_path(),
                [override_value.as_str()],
            )
            .unwrap(),
        )
        .await
    }

    fn generated_definition_observation_epoch(
        module: &str,
        extension: &str,
        extension_present: bool,
    ) -> PathObservationEpoch {
        let demand = |path: &str, operation| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        };
        let lstat = |kind, stamp, mode| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, stamp, 1, 1, 1, mode,
            )))
        };
        let path = |name: &str| format!("{WORKSPACE}/{name}");
        let mut observations = Vec::new();
        for (stamp, directory) in [(1, "/"), (2, WORKSPACE)] {
            observations.push((
                demand(directory, PathObservationOperation::Lstat),
                lstat(PathNodeKind::Directory, stamp, 0o755),
            ));
        }
        for name in ["REPO.bazel", ".bazelignore", "BUILD", "MODULE.bazel.lock"] {
            observations.push((
                demand(&path(name), PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ));
        }
        for (name, kind, stamp, mode) in [
            ("MODULE.bazel", PathNodeKind::RegularFile, 9, 0o644),
            ("BUILD.bazel", PathNodeKind::RegularFile, 10, 0o644),
            ("local", PathNodeKind::Directory, 12, 0o755),
            ("local/MODULE.bazel", PathNodeKind::RegularFile, 13, 0o644),
        ] {
            observations.push((
                demand(&path(name), PathObservationOperation::Lstat),
                lstat(kind, stamp, mode),
            ));
        }
        observations.push((
            demand(&path("MODULE.bazel"), PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                module.as_bytes(),
            ))),
        ));
        observations.push((
            demand(
                &path("local/MODULE.bazel"),
                PathObservationOperation::FileBytes,
            ),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                &b"module(name='local')\n"[..],
            ))),
        ));
        let extension_lstat = if extension_present {
            PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                11,
                1,
                1,
                1,
                0o644,
            ))
        } else {
            PathOperationResult::Missing
        };
        observations.push((
            demand(&path("ext.bzl"), PathObservationOperation::Lstat),
            PathObservationResult::Lstat(extension_lstat),
        ));
        let extension_bytes = if extension_present {
            PathOperationResult::Present(Arc::from(extension.as_bytes()))
        } else {
            PathOperationResult::Missing
        };
        observations.push((
            demand(&path("ext.bzl"), PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(extension_bytes),
        ));
        PathObservationEpoch::new(observations).unwrap()
    }

    async fn transaction_with_policy(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        extension_present: bool,
        tracker: Option<Arc<dyn ActivationTracker>>,
        command_policy: BzlmodCommandPolicyKey,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut user_data = UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            activation_tracker: tracker,
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(SortedMap::from_iter([
                        (
                            workspace.as_path().join("MODULE.bazel"),
                            WorkspaceFileValue::Present(Arc::new(module.to_owned())),
                        ),
                        (
                            workspace.as_path().join("ext.bzl"),
                            if extension_present {
                                WorkspaceFileValue::Present(Arc::new(extension.to_owned()))
                            } else {
                                WorkspaceFileValue::Absent
                            },
                        ),
                        (
                            workspace.as_path().join("BUILD.bazel"),
                            WorkspaceFileValue::Present(Arc::new(String::new())),
                        ),
                    ])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel.lock"),
                        WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        slug_bzlmod_v2::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            command_policy,
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        slug_bzlmod_v2::inject_registry_request_inputs(
            &mut updater,
            workspace.as_path(),
            RegistryUrls::new(["https://registry.invalid"]),
            RegistryRequestGeneration(1),
        )
        .unwrap();
        slug_bzlmod_v2::inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                workspace.dupe(),
                Arc::from([workspace.dupe()]),
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(workspace.dupe(), []).unwrap(),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                generated_definition_observation_epoch(module, extension, extension_present),
            )])
            .unwrap();
        updater.commit().await
    }

    pub(in crate::runtime) async fn validated(
        transaction: &mut dice::DiceTransaction,
    ) -> HostValidatedGeneratedRepositorySpecsOutcome {
        transaction
            .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    pub(in crate::runtime) fn names(
        value: &HostValidatedGeneratedRepositorySpecsOutcome,
    ) -> Vec<CanonicalRepoName> {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("validation must complete")
        };
        value
            .as_ref()
            .as_ref()
            .unwrap()
            .iter()
            .map(|(name, _, _, _)| name.clone())
            .collect()
    }
}
