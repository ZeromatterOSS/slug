/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use slug_bzlmod_v2::HostRepositoryLocalPathPolicy;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::generated_repository_definition::HostCanonicalRepositoryApparentMapping;
use super::generated_repository_definition::HostCanonicalRepositoryApparentMappingError;
use super::generated_repository_definition::HostCanonicalRepositoryApparentMappingKey;
use super::generated_repository_definition::HostCanonicalRepositoryDefinition;
use super::generated_repository_definition::HostCanonicalRepositoryDefinitionError;
use super::generated_repository_definition::HostCanonicalRepositoryDefinitionKey;
use super::generated_repository_definition::HostCanonicalRepositoryDefinitionKind;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositoryDefinition {
    mapping: HostCanonicalRepositoryApparentMapping,
    definition: HostCanonicalRepositoryDefinition,
    apparent_repo: ApparentRepoName,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HostRootApparentRepositoryDefinitionKind {
    SelectedRegistry,
    SelectedNonregistry,
    Generated,
}
#[derive(Debug, Clone, Copy)]
pub(super) struct HostRootApparentRepositoryDefinitionView<'a> {
    apparent_repo: &'a ApparentRepoName,
    canonical_repo: &'a CanonicalRepoName,
    kind: HostRootApparentRepositoryDefinitionKind,
    repo_spec: Option<&'a RepoSpec>,
    local_path_policy: HostRepositoryLocalPathPolicy,
}
fn definition_policy_matches(
    kind: HostRootApparentRepositoryDefinitionKind,
    policy: HostRepositoryLocalPathPolicy,
) -> bool {
    match kind {
        HostRootApparentRepositoryDefinitionKind::SelectedRegistry
        | HostRootApparentRepositoryDefinitionKind::Generated => {
            policy == HostRepositoryLocalPathPolicy::LocalUnsupported
        }
        HostRootApparentRepositoryDefinitionKind::SelectedNonregistry => matches!(
            policy,
            HostRepositoryLocalPathPolicy::WorkspaceRelative
                | HostRepositoryLocalPathPolicy::CommandAbsolute
        ),
    }
}
impl HostRootApparentRepositoryDefinition {
    pub(super) fn view(&self) -> Option<HostRootApparentRepositoryDefinitionView<'_>> {
        let definition = self.definition.view()?;
        let kind = match definition.kind() {
            HostCanonicalRepositoryDefinitionKind::Root => return None,
            HostCanonicalRepositoryDefinitionKind::SelectedRegistry => {
                HostRootApparentRepositoryDefinitionKind::SelectedRegistry
            }
            HostCanonicalRepositoryDefinitionKind::SelectedNonregistry => {
                HostRootApparentRepositoryDefinitionKind::SelectedNonregistry
            }
            HostCanonicalRepositoryDefinitionKind::Generated => {
                HostRootApparentRepositoryDefinitionKind::Generated
            }
        };
        let local_path_policy = definition.local_path_policy()?;
        definition_policy_matches(kind, local_path_policy).then_some(())?;
        Some(HostRootApparentRepositoryDefinitionView {
            apparent_repo: &self.apparent_repo,
            canonical_repo: definition.canonical_repo(),
            kind,
            repo_spec: definition.repo_spec(),
            local_path_policy,
        })
    }
}
impl<'a> HostRootApparentRepositoryDefinitionView<'a> {
    pub(super) fn apparent_repo(&self) -> &'a ApparentRepoName {
        self.apparent_repo
    }
    pub(super) fn canonical_repo(&self) -> &'a CanonicalRepoName {
        self.canonical_repo
    }
    pub(super) fn kind(&self) -> HostRootApparentRepositoryDefinitionKind {
        self.kind
    }
    pub(super) fn repo_spec(&self) -> Option<&'a RepoSpec> {
        self.repo_spec
    }
    pub(super) fn local_path_policy(self) -> HostRepositoryLocalPathPolicy {
        self.local_path_policy
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostRootApparentRepositoryDefinitionErrorKind {
    Mapping(HostCanonicalRepositoryApparentMappingError),
    MappingCompute(Arc<str>),
    MainDeferred {
        mapping: HostCanonicalRepositoryApparentMapping,
    },
    BuiltinDeferred {
        mapping: HostCanonicalRepositoryApparentMapping,
    },
    Definition {
        mapping: HostCanonicalRepositoryApparentMapping,
        error: HostCanonicalRepositoryDefinitionError,
    },
    DefinitionCompute {
        mapping: HostCanonicalRepositoryApparentMapping,
        message: Arc<str>,
    },
    Missing {
        mapping: HostCanonicalRepositoryApparentMapping,
        error: HostCanonicalRepositoryDefinitionError,
    },
    ContextMismatch {
        mapping: HostCanonicalRepositoryApparentMapping,
        definition: HostCanonicalRepositoryDefinition,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositoryDefinitionError {
    apparent_repo: ApparentRepoName,
    kind: HostRootApparentRepositoryDefinitionErrorKind,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HostRootApparentRepositoryDeferredKind {
    Main,
    Builtin,
}
#[derive(Debug, Clone, Copy)]
pub(super) struct HostRootApparentRepositoryDeferredView<'a> {
    apparent_repo: &'a ApparentRepoName,
    canonical_repo: &'a CanonicalRepoName,
    kind: HostRootApparentRepositoryDeferredKind,
}
impl HostRootApparentRepositoryDefinitionError {
    pub(super) fn is_deferred(&self) -> bool {
        matches!(
            self.kind,
            HostRootApparentRepositoryDefinitionErrorKind::MainDeferred { .. }
                | HostRootApparentRepositoryDefinitionErrorKind::BuiltinDeferred { .. }
        )
    }
    pub(super) fn deferred_view(&self) -> Option<HostRootApparentRepositoryDeferredView<'_>> {
        let (mapping, kind) = match &self.kind {
            HostRootApparentRepositoryDefinitionErrorKind::MainDeferred { mapping } => {
                (mapping, HostRootApparentRepositoryDeferredKind::Main)
            }
            HostRootApparentRepositoryDefinitionErrorKind::BuiltinDeferred { mapping } => {
                (mapping, HostRootApparentRepositoryDeferredKind::Builtin)
            }
            _ => return None,
        };
        Some(HostRootApparentRepositoryDeferredView {
            apparent_repo: &self.apparent_repo,
            canonical_repo: mapping.resolved_target()?,
            kind,
        })
    }
}
impl<'a> HostRootApparentRepositoryDeferredView<'a> {
    pub(super) fn apparent_repo(self) -> &'a ApparentRepoName {
        self.apparent_repo
    }
    pub(super) fn canonical_repo(self) -> &'a CanonicalRepoName {
        self.canonical_repo
    }
    pub(super) fn kind(self) -> HostRootApparentRepositoryDeferredKind {
        self.kind
    }
}
impl fmt::Display for HostRootApparentRepositoryDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "root apparent repository '{}': {:?}",
            self.apparent_repo, self.kind
        )
    }
}
impl std::error::Error for HostRootApparentRepositoryDefinitionError {}
pub(super) type HostRootApparentRepositoryDefinitionOutcome = SourcePreparationOutcome<
    Arc<Result<HostRootApparentRepositoryDefinition, HostRootApparentRepositoryDefinitionError>>,
>;
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostRootApparentRepositoryDefinitionKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
}
impl HostRootApparentRepositoryDefinitionKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Option<Self> {
        (!apparent_repo.is_root()).then_some(Self {
            workspace,
            apparent_repo,
        })
    }
}
impl fmt::Display for HostRootApparentRepositoryDefinitionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-root-apparent-repository-definition:{}:@{}",
            self.workspace,
            self.apparent_repo.as_str()
        )
    }
}
fn complete(
    value: Result<HostRootApparentRepositoryDefinition, HostRootApparentRepositoryDefinitionError>,
) -> HostRootApparentRepositoryDefinitionOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetDisposition {
    MainDeferred,
    BuiltinDeferred,
    Definition,
}
fn target_disposition(target: &CanonicalRepoName) -> TargetDisposition {
    if target.is_root() {
        TargetDisposition::MainDeferred
    } else if target.as_str() == "bazel_tools" {
        TargetDisposition::BuiltinDeferred
    } else {
        TargetDisposition::Definition
    }
}
fn definition_context_matches(
    target: &CanonicalRepoName,
    canonical_repo: &CanonicalRepoName,
    mapping_context: &CanonicalRepoName,
) -> bool {
    canonical_repo == target && mapping_context == target
}
#[async_trait]
impl Key for HostRootApparentRepositoryDefinitionKey {
    type Value = HostRootApparentRepositoryDefinitionOutcome;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let terminal = |kind| {
            complete(Err(HostRootApparentRepositoryDefinitionError {
                apparent_repo: self.apparent_repo.clone(),
                kind,
            }))
        };
        let mapping = match ctx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                self.workspace.clone(),
                CanonicalRepoName::root(),
                self.apparent_repo.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => value.clone(),
                Err(error) => {
                    return terminal(HostRootApparentRepositoryDefinitionErrorKind::Mapping(
                        error.clone(),
                    ));
                }
            },
            Err(error) => {
                return terminal(
                    HostRootApparentRepositoryDefinitionErrorKind::MappingCompute(
                        error.to_string().into(),
                    ),
                );
            }
        };
        let target = mapping
            .resolved_target()
            .expect("successful apparent mapping retains its target")
            .clone();
        match target_disposition(&target) {
            TargetDisposition::MainDeferred => {
                return terminal(
                    HostRootApparentRepositoryDefinitionErrorKind::MainDeferred { mapping },
                );
            }
            TargetDisposition::BuiltinDeferred => {
                return terminal(
                    HostRootApparentRepositoryDefinitionErrorKind::BuiltinDeferred { mapping },
                );
            }
            TargetDisposition::Definition => {}
        }
        let definition = match ctx
            .compute(&HostCanonicalRepositoryDefinitionKey::new(
                self.workspace.clone(),
                target.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => value.clone(),
                Err(error) if error.is_missing() => {
                    return terminal(HostRootApparentRepositoryDefinitionErrorKind::Missing {
                        mapping,
                        error: error.clone(),
                    });
                }
                Err(error) => {
                    return terminal(HostRootApparentRepositoryDefinitionErrorKind::Definition {
                        mapping,
                        error: error.clone(),
                    });
                }
            },
            Err(error) => {
                return terminal(
                    HostRootApparentRepositoryDefinitionErrorKind::DefinitionCompute {
                        mapping,
                        message: error.to_string().into(),
                    },
                );
            }
        };
        let Some(view) = definition.view() else {
            return terminal(
                HostRootApparentRepositoryDefinitionErrorKind::ContextMismatch {
                    mapping,
                    definition,
                },
            );
        };
        if !definition_context_matches(&target, view.canonical_repo(), view.mapping_context()) {
            return terminal(
                HostRootApparentRepositoryDefinitionErrorKind::ContextMismatch {
                    mapping,
                    definition,
                },
            );
        }
        complete(Ok(HostRootApparentRepositoryDefinition {
            mapping,
            definition,
            apparent_repo: self.apparent_repo.clone(),
        }))
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
#[cfg(test)]
pub(super) mod tests {
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use slug_bzlmod_v2::HostRepositorySourceFileKey;
    use slug_bzlmod_v2::RegistryFileKey;
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationKey;
    use slug_bzlmod_v2::RepositoryMaterializationRequest;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_bzlmod_v2::RepositoryPackageSourceKey;
    use slug_bzlmod_v2::RepositorySourceFileKey;
    use slug_bzlmod_v2::RootRepositoryRouteKey;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;

    use super::super::generated_repository_definition::tests::EXTENSION_A;
    use super::super::generated_repository_definition::tests::MODULE;
    use super::super::generated_repository_definition::tests::WORKSPACE;
    use super::super::generated_repository_definition::tests::names;
    use super::super::generated_repository_definition::tests::transaction;
    use super::super::generated_repository_definition::tests::validated;
    use super::*;
    #[derive(Default)]
    struct CompositionTracker {
        composition: Mutex<Vec<ActivationKind>>,
        mapping: Mutex<Vec<ActivationKind>>,
        definition: Mutex<Vec<ActivationKind>>,
        events: Mutex<usize>,
        forbidden: Mutex<Vec<&'static str>>,
    }
    impl ActivationTracker for CompositionTracker {
        fn key_activated(
            &self,
            _: &DynKey,
            _: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
        }
        fn tracks_rich_activations(&self) -> bool {
            true
        }
        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            let kind = activation.kind();
            if key
                .downcast_ref::<HostRootApparentRepositoryDefinitionKey>()
                .is_some()
            {
                self.composition.lock().unwrap().push(kind);
                *self.events.lock().unwrap() += usize::from(activation.evaluation_data().is_some());
            } else if key
                .downcast_ref::<HostCanonicalRepositoryApparentMappingKey>()
                .is_some()
            {
                self.mapping.lock().unwrap().push(kind);
            } else if key
                .downcast_ref::<HostCanonicalRepositoryDefinitionKey>()
                .is_some()
            {
                self.definition.lock().unwrap().push(kind);
            } else if key.downcast_ref::<RootRepositoryRouteKey>().is_some() {
                self.forbidden.lock().unwrap().push("root-route");
            } else if key.downcast_ref::<RegistryFileKey>().is_some() {
                self.forbidden.lock().unwrap().push("registry");
            } else if key.downcast_ref::<RepositoryMaterializationKey>().is_some() {
                self.forbidden.lock().unwrap().push("materialization");
            } else if key.downcast_ref::<RepositoryPackageSourceKey>().is_some()
                || key.downcast_ref::<RepositorySourceFileKey>().is_some()
                || key.downcast_ref::<HostRepositorySourceFileKey>().is_some()
            {
                self.forbidden.lock().unwrap().push("source");
            } else if key.downcast_ref::<PathObservationEpochKey>().is_some() {
                self.forbidden.lock().unwrap().push("filesystem");
            }
        }
    }
    impl CompositionTracker {
        fn clear(&self) {
            self.composition.lock().unwrap().clear();
            self.mapping.lock().unwrap().clear();
            self.definition.lock().unwrap().clear();
            *self.events.lock().unwrap() = 0;
            self.forbidden.lock().unwrap().clear();
        }
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ProbeOutcome {
        Need,
        Terminal,
        Success,
    }
    #[test]
    fn request_shape_and_target_precedence_are_total() {
        let workspace = NormalizedAbsolutePath::new("/root-definition").unwrap();
        let root = HostRootApparentRepositoryDefinitionKey::new(
            workspace.clone(),
            ApparentRepoName::root(),
        );
        let dep = HostRootApparentRepositoryDefinitionKey::new(
            workspace,
            ApparentRepoName::new("dep").unwrap(),
        );
        assert!(root.is_none() && dep.is_some());
        assert_eq!(
            target_disposition(&CanonicalRepoName::root()),
            TargetDisposition::MainDeferred,
        );
        assert_eq!(
            target_disposition(&CanonicalRepoName::new("bazel_tools").unwrap()),
            TargetDisposition::BuiltinDeferred,
        );
        assert_eq!(
            target_disposition(&CanonicalRepoName::new("dep+").unwrap()),
            TargetDisposition::Definition,
        );
        let target = CanonicalRepoName::new("dep+").unwrap();
        assert!(definition_context_matches(&target, &target, &target));
        assert!(!definition_context_matches(
            &target,
            &CanonicalRepoName::new("other+").unwrap(),
            &target,
        ));
        for kind in [
            HostRootApparentRepositoryDefinitionKind::SelectedRegistry,
            HostRootApparentRepositoryDefinitionKind::SelectedNonregistry,
            HostRootApparentRepositoryDefinitionKind::Generated,
        ] {
            for policy in [
                HostRepositoryLocalPathPolicy::WorkspaceRelative,
                HostRepositoryLocalPathPolicy::CommandAbsolute,
                HostRepositoryLocalPathPolicy::LocalUnsupported,
            ] {
                assert_eq!(
                    definition_policy_matches(kind, policy),
                    match kind {
                        HostRootApparentRepositoryDefinitionKind::SelectedNonregistry => {
                            policy != HostRepositoryLocalPathPolicy::LocalUnsupported
                        }
                        _ => policy == HostRepositoryLocalPathPolicy::LocalUnsupported,
                    }
                );
            }
        }
        assert!(!definition_context_matches(
            &target,
            &target,
            &CanonicalRepoName::new("other+").unwrap(),
        ));
        use ProbeOutcome::*;
        use TargetDisposition::*;
        let probe_calls = |mapping, target, definition| {
            if mapping != Success {
                (1, 0, mapping)
            } else if target != Definition {
                (1, 0, Terminal)
            } else {
                (1, 1, definition)
            }
        };
        for mapping in [Need, Terminal, Success] {
            for target in [MainDeferred, BuiltinDeferred, Definition] {
                for definition in [Need, Terminal, Success] {
                    let expected = if mapping != Success {
                        (1, 0, mapping)
                    } else if target != Definition {
                        (1, 0, Terminal)
                    } else {
                        (1, 1, definition)
                    };
                    assert_eq!(probe_calls(mapping, target, definition), expected);
                }
            }
        }
    }
    fn value(
        outcome: &HostRootApparentRepositoryDefinitionOutcome,
    ) -> &HostRootApparentRepositoryDefinition {
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("definition must complete: {outcome:?}")
        };
        value.as_ref().as_ref().unwrap()
    }
    async fn builtin_outcome(
        dice: &Arc<Dice>,
        workspace: &NormalizedAbsolutePath,
        tracker: Arc<CompositionTracker>,
    ) -> HostRootApparentRepositoryDefinitionOutcome {
        const LOCALS: &str = "rules_license,buildozer,platforms,zlib,protobuf,rules_java,rules_cc,rules_python,rules_shell,apple_support,bazel_features,rules_apple,rules_swift,abseil-cpp";
        let mut module = "module(name='root')\n".to_owned();
        for name in LOCALS.split(',') {
            module.push_str(&format!(
                "local_path_override(module_name='{name}', path='{name}')\n"
            ));
        }
        for pair in
            "bazel_features=1.42.1,rules_apple=4.1.0,rules_swift=3.1.2,abseil-cpp=20250814.1"
                .split(',')
        {
            let (name, version) = pair.split_once('=').unwrap();
            module.push_str(&format!("bazel_dep(name='{name}', version='{version}')\n"));
        }
        let _ = transaction(dice, &module, EXTENSION_A, true, Some(tracker.clone())).await;
        let demand = |path, operation| {
            PathObservationDemand::new(PathObservationNamespace::Host, path, operation)
        };
        let present = |path, kind, id| {
            (
                demand(path, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    kind, id, 1, 1, 1, 0o755,
                ))),
            )
        };
        let missing = |path| {
            (
                demand(path, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            )
        };
        let mut observations = vec![
            present(
                NormalizedAbsolutePath::new("/").unwrap(),
                PathNodeKind::Directory,
                1,
            ),
            present(workspace.clone(), PathNodeKind::Directory, 2),
        ];
        for (index, name) in LOCALS.split(',').enumerate() {
            let root = NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}")).unwrap();
            let module =
                NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}/MODULE.bazel")).unwrap();
            observations.extend([
                present(root, PathNodeKind::Directory, 10 + index as i64 * 2),
                present(
                    module.clone(),
                    PathNodeKind::RegularFile,
                    11 + index as i64 * 2,
                ),
                (
                    demand(module, PathObservationOperation::FileBytes),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        format!("module(name='{name}')\n").into_bytes(),
                    ))),
                ),
            ]);
            for leaf in ["REPO.bazel", ".bazelignore"] {
                observations.push(missing(
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}/{leaf}")).unwrap(),
                ));
            }
        }
        for leaf in ["REPO.bazel", ".bazelignore", "BUILD", "MODULE.bazel.lock"] {
            observations.push(missing(
                NormalizedAbsolutePath::new(format!("{WORKSPACE}/{leaf}")).unwrap(),
            ));
        }
        let mut updater = dice.updater_with_data(UserComputationData {
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(observations).unwrap(),
            )])
            .unwrap();
        let key = HostRootApparentRepositoryDefinitionKey::new(
            workspace.clone(),
            ApparentRepoName::new("bazel_tools").unwrap(),
        )
        .unwrap();
        let mut tx = updater.commit().await;
        let mut outcome = tx.compute(&key).await.unwrap();
        let mut requests = Vec::<Arc<RepositoryMaterializationRequest>>::new();
        while let SourcePreparationOutcome::Need(need) = &outcome {
            for request in need.repository_materializations().values() {
                if !requests.iter().any(|seen| seen.id == request.id) {
                    requests.push(request.clone());
                }
            }
            let entries =
                requests
                    .iter()
                    .cloned()
                    .map(|request| RepositoryMaterializationEpochEntry {
                        request,
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Local,
                        ),
                    });
            let mut updater = dice.updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            });
            updater
                .changed_to(vec![(
                    RepositoryMaterializationResultEpochKey {
                        workspace: workspace.clone(),
                    },
                    RepositoryMaterializationResultEpoch::new(workspace.clone(), entries).unwrap(),
                )])
                .unwrap();
            tx = updater.commit().await;
            outcome = tx.compute(&key).await.unwrap();
        }
        outcome
    }
    pub(in crate::runtime) async fn prepare_builtin(
        dice: &Arc<Dice>,
        workspace: &NormalizedAbsolutePath,
    ) {
        let outcome =
            builtin_outcome(dice, workspace, Arc::new(CompositionTracker::default())).await;
        assert!(matches!(outcome, SourcePreparationOutcome::Complete(_)));
    }
    #[tokio::test]
    async fn real_generated_selected_and_deferred_domains_are_structural() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut generated_tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let generated_names = names(&validated(&mut generated_tx).await);
        let generated_key = HostRootApparentRepositoryDefinitionKey::new(
            workspace.clone(),
            ApparentRepoName::new("first").unwrap(),
        )
        .unwrap();
        let generated = generated_tx.compute(&generated_key).await.unwrap();
        let generated_view = value(&generated).view().unwrap();
        assert_eq!(generated_view.apparent_repo().as_str(), "first");
        assert_eq!(generated_view.canonical_repo(), &generated_names[0]);
        assert_eq!(
            generated_view.kind(),
            HostRootApparentRepositoryDefinitionKind::Generated
        );
        assert_eq!(
            generated_view
                .repo_spec()
                .unwrap()
                .rule_id
                .rule_name
                .as_str(),
            "repo"
        );
        let local_module = "module(name='bazel_tools')\n\
            local_path_override(module_name='local', path='local')\n\
            bazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let local_tracker = Arc::new(CompositionTracker::default());
        let mut local_tx = transaction(
            &dice,
            local_module,
            EXTENSION_A,
            true,
            Some(local_tracker.clone()),
        )
        .await;
        let local_key = HostRootApparentRepositoryDefinitionKey::new(
            workspace.clone(),
            ApparentRepoName::new("local_alias").unwrap(),
        )
        .unwrap();
        let local_need = local_tx.compute(&local_key).await.unwrap();
        assert!(!HostRootApparentRepositoryDefinitionKey::validity(
            &local_need
        ));
        assert!(!HostRootApparentRepositoryDefinitionKey::equality(
            &local_need,
            &local_need
        ));
        let SourcePreparationOutcome::Need(need) = local_need else {
            panic!("local definition must first request materialization")
        };
        assert!(!local_tracker.mapping.lock().unwrap().is_empty());
        assert!(local_tracker.definition.lock().unwrap().is_empty());
        let request = need
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .clone();
        let mut updater = dice.updater_with_data(UserComputationData::default());
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.clone(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace.clone(),
                    [RepositoryMaterializationEpochEntry {
                        request,
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Local,
                        ),
                    }],
                )
                .unwrap(),
            )])
            .unwrap();
        local_tx = updater.commit().await;
        let local = local_tx.compute(&local_key).await.unwrap();
        let local_view = value(&local).view().unwrap();
        assert_eq!(local_view.canonical_repo().as_str(), "local+");
        assert_eq!(
            local_view.kind(),
            HostRootApparentRepositoryDefinitionKind::SelectedNonregistry,
        );
        assert_eq!(
            local_view.repo_spec().unwrap().rule_id.rule_name.as_str(),
            "local_repository",
        );
        let mut main_tx = transaction(
            &dice,
            "module(name='bazel_tools', repo_name='root_self')\n",
            EXTENSION_A,
            true,
            None,
        )
        .await;
        let main = main_tx
            .compute(
                &HostRootApparentRepositoryDefinitionKey::new(
                    workspace.clone(),
                    ApparentRepoName::new("root_self").unwrap(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(main) = main else {
            unreachable!()
        };
        assert!(main.as_ref().as_ref().unwrap_err().is_deferred());
        let tracker = Arc::new(CompositionTracker::default());
        let builtin = builtin_outcome(&dice, &workspace, tracker.clone()).await;
        assert!(
            matches!(
                &builtin,
                SourcePreparationOutcome::Complete(value)
                    if matches!(
                        &value.as_ref().as_ref().unwrap_err().kind,
                        HostRootApparentRepositoryDefinitionErrorKind::BuiltinDeferred { .. }
                    ) && value.as_ref().as_ref().unwrap_err().is_deferred()
            ),
            "builtin outcome: {builtin:?}"
        );
        assert!(tracker.definition.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn lifecycle_identity_and_mapping_precedence_are_structural() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let tracker = Arc::new(CompositionTracker::default());
        let mut a_tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let canonical = names(&validated(&mut a_tx).await)[0].clone();
        let mapping_key = HostCanonicalRepositoryApparentMappingKey::new(
            workspace.clone(),
            CanonicalRepoName::root(),
            ApparentRepoName::new("first").unwrap(),
        );
        a_tx.compute(&mapping_key).await.unwrap();
        a_tx.compute(&HostCanonicalRepositoryDefinitionKey::new(
            workspace.clone(),
            canonical.clone(),
        ))
        .await
        .unwrap();
        tracker.clear();
        let key = HostRootApparentRepositoryDefinitionKey::new(
            workspace.clone(),
            ApparentRepoName::new("first").unwrap(),
        )
        .unwrap();
        let a = a_tx.compute(&key).await.unwrap();
        assert_eq!(*tracker.mapping.lock().unwrap(), [ActivationKind::Reused]);
        assert_eq!(
            *tracker.definition.lock().unwrap(),
            [ActivationKind::Reused]
        );
        assert_eq!(
            *tracker.composition.lock().unwrap(),
            [ActivationKind::Evaluated]
        );
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let certificate = value(&a);
        let view = certificate.view().unwrap();
        let definition_view = certificate.definition.view().unwrap();
        assert_eq!(view.canonical_repo(), &canonical);
        assert!(std::ptr::eq(
            view.repo_spec().unwrap(),
            definition_view.repo_spec().unwrap(),
        ));
        tracker.clear();
        let warm = a_tx.compute(&key).await.unwrap();
        assert!(HostRootApparentRepositoryDefinitionKey::equality(&a, &warm));
        assert_eq!(
            *tracker.composition.lock().unwrap(),
            [ActivationKind::Reused]
        );
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let mapping = certificate.mapping.clone();
        let definition = certificate.definition.clone();
        let missing_outcome = a_tx
            .compute(&HostCanonicalRepositoryDefinitionKey::new(
                workspace.clone(),
                CanonicalRepoName::new("absent+").unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(missing_outcome) = missing_outcome else {
            panic!("missing definition must complete")
        };
        let missing = missing_outcome.as_ref().as_ref().unwrap_err().clone();
        let apparent = ApparentRepoName::new("first").unwrap();
        let typed = [
            HostRootApparentRepositoryDefinitionErrorKind::Definition {
                mapping: mapping.clone(),
                error: missing.clone(),
            },
            HostRootApparentRepositoryDefinitionErrorKind::Missing {
                mapping: mapping.clone(),
                error: missing,
            },
            HostRootApparentRepositoryDefinitionErrorKind::ContextMismatch {
                mapping,
                definition,
            },
        ]
        .map(|kind| HostRootApparentRepositoryDefinitionError {
            apparent_repo: apparent.clone(),
            kind,
        });
        assert!(typed.windows(2).all(|pair| pair[0] != pair[1]));
        assert!(typed.iter().all(|error| error.apparent_repo == apparent));
        let extension_b = EXTENSION_A.replacen("value='one'", "value='changed'", 1);
        for (module, extension) in [
            (
                MODULE.replace(
                    "first='first', second='second'",
                    "second='second', first='first'",
                ),
                EXTENSION_A,
            ),
            (
                format!("{MODULE}override_repo(e, first='bazel_tools')\n"),
                EXTENSION_A,
            ),
            (
                format!("{MODULE}inject_repo(e, injected='bazel_tools')\n"),
                EXTENSION_A,
            ),
            (MODULE.to_owned(), extension_b.as_str()),
        ] {
            let changed = transaction(&dice, &module, extension, true, None)
                .await
                .compute(&key)
                .await
                .unwrap();
            assert!(!HostRootApparentRepositoryDefinitionKey::equality(
                &a, &changed
            ));
            let restored = transaction(&dice, MODULE, EXTENSION_A, true, None)
                .await
                .compute(&key)
                .await
                .unwrap();
            assert!(HostRootApparentRepositoryDefinitionKey::equality(
                &a, &restored
            ));
        }
        tracker.clear();
        let terminal_key = HostRootApparentRepositoryDefinitionKey::new(
            workspace,
            ApparentRepoName::new("absent").unwrap(),
        )
        .unwrap();
        let terminal = transaction(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='missing', version='1', repo_name='absent')\n",
            EXTENSION_A,
            true,
            Some(tracker.clone()),
        )
        .await
        .compute(&terminal_key)
        .await
        .unwrap();
        let SourcePreparationOutcome::Complete(error) = terminal else {
            panic!("mapping failure must be complete")
        };
        let error = error.as_ref().as_ref().unwrap_err();
        assert_eq!(error.apparent_repo.as_str(), "absent");
        assert!(!error.is_deferred());
        assert!(matches!(
            error.kind,
            HostRootApparentRepositoryDefinitionErrorKind::Mapping(_)
        ));
        assert!(tracker.definition.lock().unwrap().is_empty());
        tracker.clear();
        let missing = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone()))
            .await
            .compute(&terminal_key)
            .await
            .unwrap();
        assert!(matches!(missing, SourcePreparationOutcome::Complete(value)
            if matches!(value.as_ref().as_ref().unwrap_err().kind,
                HostRootApparentRepositoryDefinitionErrorKind::Mapping(_))));
        assert!(tracker.definition.lock().unwrap().is_empty());
    }
}
