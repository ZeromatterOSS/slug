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
use dupe::Dupe;
use slug_bzlmod_v2::HostRepositoryLocalPathPolicy;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use super::generated_repository_definition::HostCanonicalRepositoryApparentMapping;
use super::generated_repository_definition::HostCanonicalRepositoryApparentMappingError;
use super::generated_repository_definition::HostCanonicalRepositoryApparentMappingKey;
use super::generated_repository_definition::HostCanonicalRepositoryApparentMappingObservationError;
use super::generated_repository_definition::HostCanonicalRepositoryApparentMappingObservationKey;
use super::generated_repository_definition::HostCanonicalRepositoryDefinition;
use super::generated_repository_definition::HostCanonicalRepositoryDefinitionError;
use super::generated_repository_definition::HostCanonicalRepositoryDefinitionKey;
use super::generated_repository_definition::HostCanonicalRepositoryDefinitionKind;
use super::generated_repository_definition::HostCanonicalRepositoryDefinitionObservationError;
use super::generated_repository_definition::HostCanonicalRepositoryDefinitionObservationKey;
use super::generated_repository_definition::HostGeneratedRepositoryEffectSeed;

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
    generated_effect_seed: Option<HostGeneratedRepositoryEffectSeed<'a>>,
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
            generated_effect_seed: definition.generated_effect_seed(),
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
    pub(super) fn generated_effect_seed(self) -> Option<HostGeneratedRepositoryEffectSeed<'a>> {
        self.generated_effect_seed
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostRootApparentRepositoryDefinitionObservationKey(
    HostRootApparentRepositoryDefinitionKey,
);

impl HostRootApparentRepositoryDefinitionObservationKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Option<Self> {
        HostRootApparentRepositoryDefinitionKey::new(workspace, apparent_repo).map(Self)
    }
}

impl fmt::Display for HostRootApparentRepositoryDefinitionObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type RootApparentRepositoryDefinitionResult =
    Arc<Result<HostRootApparentRepositoryDefinition, HostRootApparentRepositoryDefinitionError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct ObservedHostRootApparentRepositoryDefinition {
    result: RootApparentRepositoryDefinitionResult,
    observations: PathObservationEpoch,
}

impl ObservedHostRootApparentRepositoryDefinition {
    pub(super) fn result(
        &self,
    ) -> &Arc<Result<HostRootApparentRepositoryDefinition, HostRootApparentRepositoryDefinitionError>>
    {
        &self.result
    }

    pub(super) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RootApparentRepositoryDefinitionObservationError {
    Mapping(HostCanonicalRepositoryApparentMappingObservationError),
    Definition {
        mapping: HostCanonicalRepositoryApparentMapping,
        error: HostCanonicalRepositoryDefinitionObservationError,
    },
    Merge {
        mapping: HostCanonicalRepositoryApparentMapping,
        error: ObservedPathFrontierError,
    },
}

impl Dupe for RootApparentRepositoryDefinitionObservationError {}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositoryDefinitionObservationError(
    RootApparentRepositoryDefinitionObservationError,
);

impl Dupe for HostRootApparentRepositoryDefinitionObservationError {}

#[derive(Clone, Copy)]
enum RootApparentRepositoryDefinitionMode {
    Legacy,
    Observed,
}

type RootApparentRepositoryDefinitionDriverOutcome = SourcePreparationOutcome<
    Result<
        (RootApparentRepositoryDefinitionResult, PathObservationEpoch),
        RootApparentRepositoryDefinitionObservationError,
    >,
>;

fn complete_root_apparent_repository_definition_driver(
    key: &HostRootApparentRepositoryDefinitionKey,
    value: Result<
        HostRootApparentRepositoryDefinition,
        HostRootApparentRepositoryDefinitionErrorKind,
    >,
    observations: PathObservationEpoch,
) -> RootApparentRepositoryDefinitionDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((
        Arc::new(
            value.map_err(|kind| HostRootApparentRepositoryDefinitionError {
                apparent_repo: key.apparent_repo.clone(),
                kind,
            }),
        ),
        observations,
    )))
}

fn merge_root_apparent_repository_definition_observations(
    mapping: &PathObservationEpoch,
    definition: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        mapping
            .observations()
            .iter()
            .chain(definition.observations().iter())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(ObservedPathFrontierError::from)
}

fn finish_root_apparent_repository_definition(
    key: &HostRootApparentRepositoryDefinitionKey,
    mapping: HostCanonicalRepositoryApparentMapping,
    target: CanonicalRepoName,
    definition: Arc<
        Result<HostCanonicalRepositoryDefinition, HostCanonicalRepositoryDefinitionError>,
    >,
    observations: PathObservationEpoch,
) -> RootApparentRepositoryDefinitionDriverOutcome {
    let definition = match definition.as_ref() {
        Ok(definition) => definition.clone(),
        Err(error) if error.is_missing() => {
            return complete_root_apparent_repository_definition_driver(
                key,
                Err(HostRootApparentRepositoryDefinitionErrorKind::Missing {
                    mapping,
                    error: error.clone(),
                }),
                observations,
            );
        }
        Err(error) => {
            return complete_root_apparent_repository_definition_driver(
                key,
                Err(HostRootApparentRepositoryDefinitionErrorKind::Definition {
                    mapping,
                    error: error.clone(),
                }),
                observations,
            );
        }
    };
    let context_matches = definition.view().is_some_and(|view| {
        definition_context_matches(&target, view.canonical_repo(), view.mapping_context())
    });
    if !context_matches {
        return complete_root_apparent_repository_definition_driver(
            key,
            Err(
                HostRootApparentRepositoryDefinitionErrorKind::ContextMismatch {
                    mapping,
                    definition,
                },
            ),
            observations,
        );
    }
    complete_root_apparent_repository_definition_driver(
        key,
        Ok(HostRootApparentRepositoryDefinition {
            mapping,
            definition,
            apparent_repo: key.apparent_repo.clone(),
        }),
        observations,
    )
}

#[rustfmt::skip]
async fn compute_root_apparent_repository_definition(
    ctx: &mut DiceComputations<'_>,
    key: &HostRootApparentRepositoryDefinitionKey,
    mode: RootApparentRepositoryDefinitionMode,
) -> RootApparentRepositoryDefinitionDriverOutcome {
    let (mapping_result, mapping_observations) = match mode {
        RootApparentRepositoryDefinitionMode::Legacy => match ctx.compute(&HostCanonicalRepositoryApparentMappingKey::new(key.workspace.clone(), CanonicalRepoName::root(), key.apparent_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
            Err(error) => return complete_root_apparent_repository_definition_driver(key, Err(HostRootApparentRepositoryDefinitionErrorKind::MappingCompute(error.to_string().into())), PathObservationEpoch::empty()),
        },
        RootApparentRepositoryDefinitionMode::Observed => match ctx.compute(&HostCanonicalRepositoryApparentMappingObservationKey::new(key.workspace.clone(), CanonicalRepoName::root(), key.apparent_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(RootApparentRepositoryDefinitionObservationError::Mapping(error))),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
            Err(error) => return complete_root_apparent_repository_definition_driver(key, Err(HostRootApparentRepositoryDefinitionErrorKind::MappingCompute(error.to_string().into())), PathObservationEpoch::empty()),
        },
    };
    let mapping = match mapping_result.as_ref() {
        Ok(mapping) => mapping.clone(),
        Err(error) => return complete_root_apparent_repository_definition_driver(key, Err(HostRootApparentRepositoryDefinitionErrorKind::Mapping(error.clone())), mapping_observations),
    };
    let target = mapping.resolved_target().expect("successful apparent mapping retains its target").clone();
    match target_disposition(&target) {
        TargetDisposition::MainDeferred => return complete_root_apparent_repository_definition_driver(key, Err(HostRootApparentRepositoryDefinitionErrorKind::MainDeferred { mapping }), mapping_observations),
        TargetDisposition::BuiltinDeferred => return complete_root_apparent_repository_definition_driver(key, Err(HostRootApparentRepositoryDefinitionErrorKind::BuiltinDeferred { mapping }), mapping_observations),
        TargetDisposition::Definition => {}
    }
    let (definition_result, definition_observations) = match mode {
        RootApparentRepositoryDefinitionMode::Legacy => match ctx.compute(&HostCanonicalRepositoryDefinitionKey::new(key.workspace.clone(), target.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
            Err(error) => return complete_root_apparent_repository_definition_driver(key, Err(HostRootApparentRepositoryDefinitionErrorKind::DefinitionCompute { mapping, message: error.to_string().into() }), mapping_observations),
        },
        RootApparentRepositoryDefinitionMode::Observed => match ctx.compute(&HostCanonicalRepositoryDefinitionObservationKey::new(key.workspace.clone(), target.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(RootApparentRepositoryDefinitionObservationError::Definition { mapping, error })),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
            Err(error) => return complete_root_apparent_repository_definition_driver(key, Err(HostRootApparentRepositoryDefinitionErrorKind::DefinitionCompute { mapping, message: error.to_string().into() }), mapping_observations),
        },
    };
    let observations = match merge_root_apparent_repository_definition_observations(&mapping_observations, &definition_observations) {
        Ok(observations) => observations,
        Err(error) => return SourcePreparationOutcome::Complete(Err(RootApparentRepositoryDefinitionObservationError::Merge { mapping, error })),
    };
    finish_root_apparent_repository_definition(key, mapping, target, definition_result, observations)
}

#[async_trait]
impl Key for HostRootApparentRepositoryDefinitionKey {
    type Value = HostRootApparentRepositoryDefinitionOutcome;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_root_apparent_repository_definition(
            ctx,
            self,
            RootApparentRepositoryDefinitionMode::Legacy,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy root apparent definition has no observed outer")
            }
        }
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostRootApparentRepositoryDefinitionObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostRootApparentRepositoryDefinition,
            HostRootApparentRepositoryDefinitionObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_root_apparent_repository_definition(
            ctx,
            &self.0,
            RootApparentRepositoryDefinitionMode::Observed,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(
                Err(HostRootApparentRepositoryDefinitionObservationError(error)),
            ),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(
                    ObservedHostRootApparentRepositoryDefinition {
                        result,
                        observations,
                    },
                ))
            }
        }
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
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Arc;
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
    use slug_events_v2::EventBatch;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;

    use super::super::generated_repository_definition::HostCanonicalRepositoryApparentMappingObservationError;
    use super::super::generated_repository_definition::HostCanonicalRepositoryApparentMappingObservationKey;
    use super::super::generated_repository_definition::HostCanonicalRepositoryDefinitionObservationError;
    use super::super::generated_repository_definition::HostCanonicalRepositoryDefinitionObservationKey;
    use super::super::generated_repository_definition::ObservedHostCanonicalRepositoryApparentMapping;
    use super::super::generated_repository_definition::ObservedHostCanonicalRepositoryDefinition;
    use super::super::generated_repository_definition::tests::EXTENSION_A;
    use super::super::generated_repository_definition::tests::MODULE;
    use super::super::generated_repository_definition::tests::WORKSPACE;
    use super::super::generated_repository_definition::tests::names;
    use super::super::generated_repository_definition::tests::transaction;
    use super::super::generated_repository_definition::tests::validated;
    use super::*;

    #[test]
    fn canonical_repository_definition_observation_surface_is_sibling_usable() {
        let key = HostCanonicalRepositoryDefinitionObservationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            CanonicalRepoName::new("requested").unwrap(),
        );
        assert_eq!(
            key.to_string(),
            "observed-host-canonical-repository-definition:\"/workspace\":@@requested"
        );

        fn inspect(
            _: &<HostCanonicalRepositoryDefinitionObservationKey as Key>::Value,
            observed: &ObservedHostCanonicalRepositoryDefinition,
            _: &HostCanonicalRepositoryDefinitionObservationError,
        ) {
            let _: &Arc<
                Result<HostCanonicalRepositoryDefinition, HostCanonicalRepositoryDefinitionError>,
            > = observed.result();
            let _: &PathObservationEpoch = observed.observations();
        }
        let _ = inspect
            as fn(
                &SourcePreparationOutcome<
                    Result<
                        ObservedHostCanonicalRepositoryDefinition,
                        HostCanonicalRepositoryDefinitionObservationError,
                    >,
                >,
                &ObservedHostCanonicalRepositoryDefinition,
                &HostCanonicalRepositoryDefinitionObservationError,
            );
    }

    #[test]
    fn canonical_repository_apparent_mapping_observation_surface_is_sibling_usable() {
        let key = HostCanonicalRepositoryApparentMappingObservationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            CanonicalRepoName::root(),
            ApparentRepoName::new("first").unwrap(),
        );
        assert_eq!(
            key.to_string(),
            "observed-host-canonical-repository-apparent-mapping:\"/workspace\":@@:@first"
        );

        fn inspect(
            _: &<HostCanonicalRepositoryApparentMappingObservationKey as Key>::Value,
            observed: &ObservedHostCanonicalRepositoryApparentMapping,
            _: &HostCanonicalRepositoryApparentMappingObservationError,
        ) {
            let _: &Arc<
                Result<
                    HostCanonicalRepositoryApparentMapping,
                    HostCanonicalRepositoryApparentMappingError,
                >,
            > = observed.result();
            let _: &PathObservationEpoch = observed.observations();
        }
        let _ = inspect
            as fn(
                &SourcePreparationOutcome<
                    Result<
                        ObservedHostCanonicalRepositoryApparentMapping,
                        HostCanonicalRepositoryApparentMappingObservationError,
                    >,
                >,
                &ObservedHostCanonicalRepositoryApparentMapping,
                &HostCanonicalRepositoryApparentMappingObservationError,
            );
    }

    #[derive(Default)]
    pub(in crate::runtime) struct CompositionTracker {
        composition: Mutex<Vec<ActivationKind>>,
        mapping: Mutex<Vec<ActivationKind>>,
        definition: Mutex<Vec<ActivationKind>>,
        observed_composition: Mutex<Vec<ActivationKind>>,
        observed_mapping: Mutex<Vec<ActivationKind>>,
        observed_definition: Mutex<Vec<ActivationKind>>,
        activations: Mutex<Vec<(String, ActivationKind, Option<EventBatch>)>>,
        dependencies: Mutex<Vec<(String, Vec<String>)>>,
        events: Mutex<usize>,
        forbidden: Mutex<Vec<&'static str>>,
    }
    impl ActivationTracker for CompositionTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            dependencies: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            self.dependencies.lock().unwrap().push((
                key.to_string(),
                dependencies.map(ToString::to_string).collect(),
            ));
        }
        fn tracks_rich_activations(&self) -> bool {
            true
        }
        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            let kind = activation.kind();
            let batch = activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe);
            self.activations
                .lock()
                .unwrap()
                .push((key.to_string(), kind, batch));
            if key
                .downcast_ref::<HostRootApparentRepositoryDefinitionObservationKey>()
                .is_some()
            {
                self.observed_composition.lock().unwrap().push(kind);
            } else if key
                .downcast_ref::<HostRootApparentRepositoryDefinitionKey>()
                .is_some()
            {
                self.composition.lock().unwrap().push(kind);
                *self.events.lock().unwrap() += usize::from(activation.evaluation_data().is_some());
            } else if key
                .downcast_ref::<HostCanonicalRepositoryApparentMappingObservationKey>()
                .is_some()
            {
                self.observed_mapping.lock().unwrap().push(kind);
            } else if key
                .downcast_ref::<HostCanonicalRepositoryApparentMappingKey>()
                .is_some()
            {
                self.mapping.lock().unwrap().push(kind);
            } else if key
                .downcast_ref::<HostCanonicalRepositoryDefinitionObservationKey>()
                .is_some()
            {
                self.observed_definition.lock().unwrap().push(kind);
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
            self.observed_composition.lock().unwrap().clear();
            self.observed_mapping.lock().unwrap().clear();
            self.observed_definition.lock().unwrap().clear();
            self.activations.lock().unwrap().clear();
            self.dependencies.lock().unwrap().clear();
            *self.events.lock().unwrap() = 0;
            self.forbidden.lock().unwrap().clear();
        }
    }

    fn observed_value(
        outcome: &<HostRootApparentRepositoryDefinitionObservationKey as Key>::Value,
    ) -> &ObservedHostRootApparentRepositoryDefinition {
        match outcome {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("observed root definition must have a carrier: {value:?}"),
        }
    }

    fn dependency_row(tracker: &CompositionTracker, key: &str) -> Vec<String> {
        tracker
            .dependencies
            .lock()
            .unwrap()
            .iter()
            .find(|(name, _)| name == key)
            .unwrap_or_else(|| panic!("missing dependency row for {key}"))
            .1
            .clone()
    }

    fn event_rows(tracker: &CompositionTracker) -> Vec<(String, EventBatch)> {
        tracker
            .activations
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(owner, _, batch)| batch.dupe().map(|batch| (owner.clone(), batch)))
            .collect()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RealRootDefinitionFamily {
        Generated,
        SelectedNonregistry,
        MappingFailure,
        MainDeferred,
        BuiltinDeferred,
    }

    async fn real_family_transaction(
        dice: &Arc<Dice>,
        family: RealRootDefinitionFamily,
        tracker: Arc<CompositionTracker>,
    ) -> dice::DiceTransaction {
        if family == RealRootDefinitionFamily::BuiltinDeferred {
            let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
            let _ = builtin_outcome(dice, &workspace, tracker.clone()).await;
            tracker.clear();
            return dice
                .updater_with_data(UserComputationData {
                    cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
                    activation_tracker: Some(tracker),
                    ..Default::default()
                })
                .commit()
                .await;
        }
        let (module, extension, present) = match family {
            RealRootDefinitionFamily::Generated => (MODULE, EXTENSION_A, true),
            RealRootDefinitionFamily::SelectedNonregistry => (
                "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n",
                EXTENSION_A,
                true,
            ),
            RealRootDefinitionFamily::MappingFailure => {
                ("this is not valid Starlark\n", EXTENSION_A, true)
            }
            RealRootDefinitionFamily::MainDeferred => (
                "module(name='bazel_tools', repo_name='root_self')\n",
                EXTENSION_A,
                true,
            ),
            RealRootDefinitionFamily::BuiltinDeferred => unreachable!(),
        };
        transaction(dice, module, extension, present, Some(tracker)).await
    }

    pub(in crate::runtime) async fn local_materialized_transaction(
        dice: &Arc<Dice>,
        workspace: &NormalizedAbsolutePath,
        request: Arc<RepositoryMaterializationRequest>,
        tracker: Arc<CompositionTracker>,
        success: RepositoryMaterializationSuccess,
    ) -> dice::DiceTransaction {
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            activation_tracker: Some(tracker),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.clone(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace.clone(),
                    [RepositoryMaterializationEpochEntry {
                        request,
                        result: RepositoryMaterializationResult::Success(success),
                    }],
                )
                .unwrap(),
            )])
            .unwrap();
        updater.commit().await
    }

    async fn observed_transaction_state(
        transaction: &mut dice::DiceTransaction,
        key: &HostRootApparentRepositoryDefinitionObservationKey,
        mapping_key: &HostCanonicalRepositoryApparentMappingObservationKey,
    ) -> (
        ObservedHostRootApparentRepositoryDefinition,
        ObservedHostCanonicalRepositoryApparentMapping,
        ObservedHostCanonicalRepositoryDefinition,
    ) {
        let parent_outcome = transaction.compute(key).await.unwrap();
        let parent = observed_value(&parent_outcome).dupe();
        let target = parent
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .view()
            .unwrap()
            .canonical_repo()
            .clone();
        let mapping_outcome = transaction.compute(mapping_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(mapping)) = mapping_outcome else {
            panic!("mapping child carrier expected")
        };
        let definition_outcome = transaction
            .compute(&HostCanonicalRepositoryDefinitionObservationKey::new(
                key.0.workspace.clone(),
                target,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(definition)) = definition_outcome else {
            panic!("definition child carrier expected")
        };
        let global = transaction.compute(&PathObservationEpochKey).await.unwrap();
        assert_epoch_subset(mapping.observations(), parent.observations());
        assert_epoch_subset(definition.observations(), parent.observations());
        assert_epoch_subset(parent.observations(), &global);
        (parent, mapping, definition)
    }

    fn assert_epoch_subset(subset: &PathObservationEpoch, superset: &PathObservationEpoch) {
        for (demand, result) in subset.observations() {
            assert_eq!(result.as_ref(), superset.get(demand).unwrap().as_ref());
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

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_root_apparent_repository_definition_identity_staging_and_terminal_algebra() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = ApparentRepoName::new("first").unwrap();
        let key = HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), apparent.clone()).unwrap();
        let same = HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), apparent.clone()).unwrap();
        let other = HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), ApparentRepoName::new("second").unwrap()).unwrap();
        let hash = |value: &HostRootApparentRepositoryDefinitionObservationKey| { let mut state = DefaultHasher::new(); value.hash(&mut state); state.finish() };
        assert_eq!(key.to_string(), "observed-host-root-apparent-repository-definition:\"/generated-repository-definition\":@first");
        assert!(HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), ApparentRepoName::root()).is_none());
        assert_eq!(key, same); assert_ne!(key, other); assert_eq!(hash(&key), hash(&same)); assert_ne!(hash(&key), hash(&other));

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(CompositionTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let target = names(&validated(&mut tx).await)[0].clone();
        tracker.clear();
        let outcome = tx.compute(&key).await.unwrap();
        let carrier = observed_value(&outcome);
        assert!(carrier.result().as_ref().is_ok()); assert!(!carrier.observations().observations().is_empty());
        assert!(HostRootApparentRepositoryDefinitionObservationKey::validity(&outcome));
        assert!(HostRootApparentRepositoryDefinitionObservationKey::equality(&outcome, &outcome));
        let mapping_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), CanonicalRepoName::root(), apparent.clone());
        let definition_key = HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), target.clone());
        assert_eq!(dependency_row(&tracker, &key.to_string()), [mapping_key.to_string(), definition_key.to_string()]);
        let mapping_outcome = tx.compute(&mapping_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(mapping_carrier)) = mapping_outcome else { panic!("mapping carrier expected") };
        let mapping = mapping_carrier.result().as_ref().as_ref().unwrap().clone();
        let definition_outcome = tx.compute(&definition_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(definition_carrier)) = definition_outcome else { panic!("definition carrier expected") };
        assert_epoch_subset(mapping_carrier.observations(), carrier.observations());
        assert_epoch_subset(definition_carrier.observations(), carrier.observations());

        let main_tracker = Arc::new(CompositionTracker::default());
        let mut main_tx = transaction(&dice, "module(name='bazel_tools', repo_name='root_self')\n", EXTENSION_A, true, Some(main_tracker.clone())).await;
        let main_key = HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), ApparentRepoName::new("root_self").unwrap()).unwrap();
        let main = main_tx.compute(&main_key).await.unwrap();
        assert!(matches!(observed_value(&main).result().as_ref(), Err(HostRootApparentRepositoryDefinitionError { kind: HostRootApparentRepositoryDefinitionErrorKind::MainDeferred { .. }, .. })));
        assert_eq!(dependency_row(&main_tracker, &main_key.to_string()).len(), 1);
        assert!(main_tracker.observed_definition.lock().unwrap().is_empty());

        let need_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let _ = transaction(&need_dice, MODULE, EXTENSION_A, true, None).await;
        let need_tracker = Arc::new(CompositionTracker::default());
        let mut updater = need_dice.updater_with_data(UserComputationData { cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()), activation_tracker: Some(need_tracker), ..Default::default() });
        updater.changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())]).unwrap();
        let need = updater.commit().await.compute(&key).await.unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostRootApparentRepositoryDefinitionObservationKey::validity(&need));
        assert!(!HostRootApparentRepositoryDefinitionObservationKey::equality(&need, &need));

        let absent_key = HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), CanonicalRepoName::new("absent+").unwrap());
        let absent = tx.compute(&absent_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(absent_carrier)) = absent else { panic!("missing definition carrier expected") };
        let missing_observations = merge_root_apparent_repository_definition_observations(mapping_carrier.observations(), absent_carrier.observations()).unwrap();
        assert!(!absent_carrier.observations().observations().is_empty()); assert_epoch_subset(mapping_carrier.observations(), &missing_observations); assert_epoch_subset(absent_carrier.observations(), &missing_observations);
        let missing = finish_root_apparent_repository_definition(&key.0, mapping.clone(), target.clone(), absent_carrier.result().clone(), missing_observations.clone());
        assert!(matches!(missing, SourcePreparationOutcome::Complete(Ok((result, observations))) if matches!(result.as_ref(), Err(HostRootApparentRepositoryDefinitionError { kind: HostRootApparentRepositoryDefinitionErrorKind::Missing { .. }, .. })) && observations == missing_observations));
        let mismatch = finish_root_apparent_repository_definition(&key.0, mapping.clone(), CanonicalRepoName::new("other+").unwrap(), definition_carrier.result().clone(), carrier.observations().clone());
        assert!(matches!(mismatch, SourcePreparationOutcome::Complete(Ok((result, _))) if matches!(result.as_ref(), Err(HostRootApparentRepositoryDefinitionError { kind: HostRootApparentRepositoryDefinitionErrorKind::ContextMismatch { .. }, .. }))));
        let definition_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut definition_tx = transaction(&definition_dice, MODULE, EXTENSION_A, false, None).await;
        let definition_terminal = definition_tx.compute(&definition_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(definition_terminal)) = definition_terminal else { panic!("definition terminal carrier") };
        let definition_terminal = finish_root_apparent_repository_definition(&key.0, mapping.clone(), target.clone(), definition_terminal.result().clone(), carrier.observations().clone());
        assert!(matches!(definition_terminal, SourcePreparationOutcome::Complete(Ok((result, _))) if matches!(result.as_ref(), Err(HostRootApparentRepositoryDefinitionError { kind: HostRootApparentRepositoryDefinitionErrorKind::Definition { .. }, .. }))));
        let bad_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut bad_tx = transaction(&bad_dice, "this is not valid Starlark\n", EXTENSION_A, true, None).await;
        let bad_mapping = bad_tx.compute(&mapping_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(bad_mapping)) = bad_mapping else { panic!("mapping terminal carrier") };
        let mapping_terminal = complete_root_apparent_repository_definition_driver(&key.0, Err(HostRootApparentRepositoryDefinitionErrorKind::Mapping(bad_mapping.result().as_ref().as_ref().unwrap_err().clone())), bad_mapping.observations().clone());
        assert!(matches!(mapping_terminal, SourcePreparationOutcome::Complete(Ok((result, _))) if matches!(result.as_ref(), Err(HostRootApparentRepositoryDefinitionError { kind: HostRootApparentRepositoryDefinitionErrorKind::Mapping(_), .. }))));
        let mapping_compute = complete_root_apparent_repository_definition_driver(&key.0, Err(HostRootApparentRepositoryDefinitionErrorKind::MappingCompute("mapping-dice".into())), PathObservationEpoch::empty());
        let definition_compute = complete_root_apparent_repository_definition_driver(&key.0, Err(HostRootApparentRepositoryDefinitionErrorKind::DefinitionCompute { mapping: mapping.clone(), message: "definition-dice".into() }), carrier.observations().clone());
        assert!(matches!(mapping_compute, SourcePreparationOutcome::Complete(Ok((result, observations))) if matches!(result.as_ref(), Err(HostRootApparentRepositoryDefinitionError { kind: HostRootApparentRepositoryDefinitionErrorKind::MappingCompute(message), .. }) if message.as_ref() == "mapping-dice") && observations.observations().is_empty()));
        assert!(matches!(definition_compute, SourcePreparationOutcome::Complete(Ok((result, observations))) if matches!(result.as_ref(), Err(HostRootApparentRepositoryDefinitionError { kind: HostRootApparentRepositoryDefinitionErrorKind::DefinitionCompute { message, .. }, .. }) if message.as_ref() == "definition-dice") && observations == *carrier.observations()));

        let demand = PathObservationDemand::new(PathObservationNamespace::Host, NormalizedAbsolutePath::new("/merge").unwrap(), PathObservationOperation::Lstat);
        let left_result = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let left = PathObservationEpoch::from_shared([(demand.dupe(), left_result.dupe())]).unwrap();
        let equal = PathObservationEpoch::from_shared([(demand.dupe(), Arc::new(left_result.as_ref().clone()))]).unwrap();
        let merged = merge_root_apparent_repository_definition_observations(&left, &equal).unwrap();
        assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &left_result));
        let conflict_epoch = PathObservationEpoch::from_shared([(demand.dupe(), Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(PathNodeKind::RegularFile, 1, 1, 1, 1, 0o644))))) ]).unwrap();
        let conflict = merge_root_apparent_repository_definition_observations(&left, &conflict_epoch).unwrap_err();
        let outer: <HostRootApparentRepositoryDefinitionObservationKey as Key>::Value = SourcePreparationOutcome::Complete(Err(HostRootApparentRepositoryDefinitionObservationError(RootApparentRepositoryDefinitionObservationError::Merge { mapping, error: conflict })));
        assert!(HostRootApparentRepositoryDefinitionObservationKey::validity(&outer));
        assert!(HostRootApparentRepositoryDefinitionObservationKey::equality(&outer, &outer));

        let source = include_str!("root_apparent_repository_definition.rs");
        let producer = &source[source.find("struct HostRootApparentRepositoryDefinitionObservationKey").unwrap()..source.find("#[cfg(test)]").unwrap()];
        assert_eq!(producer.matches("HostCanonicalRepositoryApparentMappingObservationKey::new").count(), 1);
        assert_eq!(producer.matches("HostCanonicalRepositoryDefinitionObservationKey::new").count(), 1);
        assert!(producer.find("HostCanonicalRepositoryApparentMappingObservationKey::new").unwrap() < producer.find("HostCanonicalRepositoryDefinitionObservationKey::new").unwrap());
        assert_eq!(producer.matches("RootApparentRepositoryDefinitionObservationError::Mapping(error)").count(), 1);
        assert_eq!(producer.matches("RootApparentRepositoryDefinitionObservationError::Definition { mapping, error }").count(), 1);
        assert_eq!(producer.matches("RootApparentRepositoryDefinitionObservationError::Merge { mapping, error }").count(), 1);
        assert_eq!(producer.matches("HostRootApparentRepositoryDefinitionObservationError(error)").count(), 1);
        assert!(!producer.contains("OperationMismatch")); assert!(!producer.contains("EventBatch")); assert!(!producer.contains("store_evaluation_data"));

        let selected_source = include_str!("../../../slug_bzlmod_v2/src/selected_repo_spec.rs");
        let selected_real = &selected_source[selected_source.find("async fn observed_canonical_selected_definition_real_order_events_and_parity").unwrap()..selected_source.find("async fn observed_canonical_selected_definition_lifecycle_cancellation_and_nonactivation").unwrap()];
        for evidence in ["const MODULE_URL", "CanonicalRepoName::new(\"dep+\")", "observed_events", "legacy_events", "ActivationKind::Reused"] { assert!(selected_real.contains(evidence), "missing accepted selected-registry evidence: {evidence}"); }
        let canonical_source = include_str!("generated_repository_definition.rs");
        let canonical_chain = &canonical_source[canonical_source.find("enum HostCanonicalRepositoryDefinitionSource").unwrap()..canonical_source.find("#[cfg(test)]").unwrap()];
        assert!(canonical_chain.contains("HostCanonicalSelectedModuleDefinitionObservationKey::new"));
        assert!(canonical_chain.contains("source: HostCanonicalRepositoryDefinitionSource::Selected(value.clone())"));
        assert!(canonical_chain.contains("HostCanonicalSelectedModuleKind::SelectedRegistry"));
        assert!(canonical_chain.contains("HostCanonicalRepositoryDefinitionKind::SelectedRegistry"));
        let canonical_real = &canonical_source[canonical_source.find("async fn observed_canonical_repository_definition_real_order_events_and_parity").unwrap()..canonical_source.find("async fn observed_canonical_repository_definition_lifecycle_cancellation_and_nonactivation").unwrap()];
        for evidence in ["generated-missing", "HostCanonicalRepositoryDefinitionErrorKind::Missing", "expected_prints", "ActivationKind::Reused"] { assert!(canonical_real.contains(evidence), "missing accepted canonical-Missing evidence: {evidence}"); }
        let root_forwarding = &source[source.find("impl HostRootApparentRepositoryDefinition {").unwrap()..source.find("enum HostRootApparentRepositoryDefinitionErrorKind").unwrap()];
        for projection in ["HostCanonicalRepositoryDefinitionKind::SelectedRegistry", "HostRootApparentRepositoryDefinitionKind::SelectedRegistry", "canonical_repo: definition.canonical_repo()", "repo_spec: definition.repo_spec()", "local_path_policy"] { assert!(root_forwarding.contains(projection), "missing root selected forwarding: {projection}"); }
        assert!(definition_policy_matches(HostRootApparentRepositoryDefinitionKind::SelectedRegistry, HostRepositoryLocalPathPolicy::LocalUnsupported));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_root_apparent_repository_definition_real_order_events_and_parity() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        for family in [RealRootDefinitionFamily::Generated, RealRootDefinitionFamily::SelectedNonregistry, RealRootDefinitionFamily::MappingFailure, RealRootDefinitionFamily::MainDeferred, RealRootDefinitionFamily::BuiltinDeferred] {
            let apparent = ApparentRepoName::new(match family { RealRootDefinitionFamily::Generated | RealRootDefinitionFamily::MappingFailure => "first", RealRootDefinitionFamily::SelectedNonregistry => "local_alias", RealRootDefinitionFamily::MainDeferred => "root_self", RealRootDefinitionFamily::BuiltinDeferred => "bazel_tools" }).unwrap();
            let key = HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), apparent.clone()).unwrap();
            let observed_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let observed_tracker = Arc::new(CompositionTracker::default());
            let mut observed_tx = real_family_transaction(&observed_dice, family, observed_tracker.clone()).await;
            let mut observed = observed_tx.compute(&key).await.unwrap();
            if let SourcePreparationOutcome::Need(need) = &observed {
                assert_eq!(family, RealRootDefinitionFamily::SelectedNonregistry);
                let request = need.repository_materializations().values().next().unwrap().clone();
                observed_tx = local_materialized_transaction(&observed_dice, &workspace, request, observed_tracker.clone(), RepositoryMaterializationSuccess::Local).await;
                observed_tracker.clear(); observed = observed_tx.compute(&key).await.unwrap();
            }
            let carrier = observed_value(&observed);
            match family {
                RealRootDefinitionFamily::Generated => assert!(matches!(carrier.result().as_ref(), Ok(value) if value.view().unwrap().kind() == HostRootApparentRepositoryDefinitionKind::Generated)),
                RealRootDefinitionFamily::SelectedNonregistry => assert!(matches!(carrier.result().as_ref(), Ok(value) if value.view().unwrap().kind() == HostRootApparentRepositoryDefinitionKind::SelectedNonregistry)),
                RealRootDefinitionFamily::MappingFailure => assert!(matches!(carrier.result().as_ref(), Err(HostRootApparentRepositoryDefinitionError { kind: HostRootApparentRepositoryDefinitionErrorKind::Mapping(_), .. }))),
                RealRootDefinitionFamily::MainDeferred => assert!(matches!(carrier.result().as_ref(), Err(error) if matches!(error.kind, HostRootApparentRepositoryDefinitionErrorKind::MainDeferred { .. }) && error.is_deferred())),
                RealRootDefinitionFamily::BuiltinDeferred => assert!(matches!(carrier.result().as_ref(), Err(error) if matches!(error.kind, HostRootApparentRepositoryDefinitionErrorKind::BuiltinDeferred { .. }) && error.is_deferred())),
            }
            let target = match carrier.result().as_ref() {
                Ok(value) => value.mapping.resolved_target().cloned(),
                Err(error) => match &error.kind {
                    HostRootApparentRepositoryDefinitionErrorKind::MainDeferred { mapping } | HostRootApparentRepositoryDefinitionErrorKind::BuiltinDeferred { mapping } => mapping.resolved_target().cloned(),
                    HostRootApparentRepositoryDefinitionErrorKind::Mapping(_) | HostRootApparentRepositoryDefinitionErrorKind::MappingCompute(_) => None,
                    _ => unreachable!("unexpected real family terminal: {family:?}"),
                },
            };
            let definition_edge = matches!(family, RealRootDefinitionFamily::Generated | RealRootDefinitionFamily::SelectedNonregistry);
            let mapping_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), CanonicalRepoName::root(), apparent.clone());
            let mut expected_children = vec![mapping_key.to_string()];
            if definition_edge { expected_children.push(HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), target.clone().unwrap()).to_string()); }
            assert_eq!(dependency_row(&observed_tracker, &key.to_string()), expected_children, "{family:?}");
            let activations = observed_tracker.activations.lock().unwrap();
            let parent = activations.iter().find(|(name, _, _)| name == &key.to_string()).unwrap();
            assert_eq!(parent.1, ActivationKind::Evaluated); assert!(parent.2.is_none()); drop(activations);
            let parent_events = event_rows(&observed_tracker);

            let direct_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let direct_tracker = Arc::new(CompositionTracker::default());
            let mut direct_tx = real_family_transaction(&direct_dice, family, direct_tracker.clone()).await;
            let mut direct_mapping = direct_tx.compute(&mapping_key).await.unwrap();
            if let SourcePreparationOutcome::Need(need) = &direct_mapping {
                assert_eq!(family, RealRootDefinitionFamily::SelectedNonregistry);
                let request = need.repository_materializations().values().next().unwrap().clone();
                direct_tx = local_materialized_transaction(&direct_dice, &workspace, request, direct_tracker.clone(), RepositoryMaterializationSuccess::Local).await;
                direct_tracker.clear(); direct_mapping = direct_tx.compute(&mapping_key).await.unwrap();
            }
            assert!(matches!(direct_mapping, SourcePreparationOutcome::Complete(Ok(_))));
            if definition_edge { assert!(matches!(direct_tx.compute(&HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), target.clone().unwrap())).await.unwrap(), SourcePreparationOutcome::Complete(Ok(_)))); }
            assert_eq!(parent_events, event_rows(&direct_tracker), "{family:?}");

            let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let legacy_tracker = Arc::new(CompositionTracker::default());
            let mut legacy_tx = real_family_transaction(&legacy_dice, family, legacy_tracker.clone()).await;
            let legacy_key = HostRootApparentRepositoryDefinitionKey::new(workspace.clone(), apparent).unwrap();
            let mut legacy = legacy_tx.compute(&legacy_key).await.unwrap();
            if let SourcePreparationOutcome::Need(need) = &legacy {
                assert_eq!(family, RealRootDefinitionFamily::SelectedNonregistry);
                let request = need.repository_materializations().values().next().unwrap().clone();
                legacy_tx = local_materialized_transaction(&legacy_dice, &workspace, request, legacy_tracker, RepositoryMaterializationSuccess::Local).await;
                legacy = legacy_tx.compute(&legacy_key).await.unwrap();
            }
            let SourcePreparationOutcome::Complete(legacy_result) = legacy else { panic!("{family:?}: legacy must complete") };
            assert_eq!(legacy_result.as_ref(), carrier.result().as_ref(), "{family:?}");

            observed_tracker.clear();
            let warm = observed_tx.compute(&key).await.unwrap();
            assert!(Arc::ptr_eq(observed_value(&warm).result(), carrier.result()));
            let warm_activations = observed_tracker.activations.lock().unwrap();
            assert!(!warm_activations.is_empty()); assert!(warm_activations.iter().all(|(_, kind, batch)| *kind == ActivationKind::Reused && batch.is_none()), "{family:?}: {warm_activations:#?}");
            drop(warm_activations); assert!(event_rows(&observed_tracker).is_empty());
        }
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_root_apparent_repository_definition_lifecycle_cancellation_and_nonactivation() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = ApparentRepoName::new("first").unwrap();
        let key = HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), apparent.clone()).unwrap();
        let mapping_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), CanonicalRepoName::root(), apparent);
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(CompositionTracker::default());
        let mut a_tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        let (a, a_mapping, a_definition) = observed_transaction_state(&mut a_tx, &key, &mapping_key).await;
        let a_result = a.result().clone(); let a_mapping_result = a_mapping.result().clone(); let a_definition_result = a_definition.result().clone();
        let a_observations = a.observations().clone(); let a_mapping_observations = a_mapping.observations().clone(); let a_definition_observations = a_definition.observations().clone();

        tracker.clear();
        let (warm, warm_mapping, warm_definition) = observed_transaction_state(&mut a_tx, &key, &mapping_key).await;
        assert!(Arc::ptr_eq(warm.result(), a.result())); assert!(Arc::ptr_eq(warm_mapping.result(), a_mapping.result())); assert!(Arc::ptr_eq(warm_definition.result(), a_definition.result()));
        assert_eq!(tracker.observed_composition.lock().unwrap().as_slice(), [ActivationKind::Reused]); assert_eq!(tracker.observed_mapping.lock().unwrap().as_slice(), [ActivationKind::Reused]); assert_eq!(tracker.observed_definition.lock().unwrap().as_slice(), [ActivationKind::Reused]);
        assert!(tracker.activations.lock().unwrap().iter().all(|(_, _, batch)| batch.is_none())); assert!(event_rows(&tracker).is_empty());

        let mapping_b_module = MODULE.replacen("first='first', second='second'", "first='second', second='first'", 1);
        let mut mapping_b_tx = transaction(&dice, &mapping_b_module, EXTENSION_A, true, None).await;
        let (mapping_b, mapping_b_child, mapping_b_definition) = observed_transaction_state(&mut mapping_b_tx, &key, &mapping_key).await;
        assert_ne!(mapping_b_child.result(), a_mapping.result()); assert_ne!(mapping_b.result(), a.result());
        let mut mapping_restore_tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let (mapping_restored, mapping_restored_child, mapping_restored_definition) = observed_transaction_state(&mut mapping_restore_tx, &key, &mapping_key).await;
        assert_eq!(mapping_restored.result(), a.result()); assert_eq!(mapping_restored_child.result(), a_mapping.result()); assert_eq!(mapping_restored_definition.result(), a_definition.result());

        let extension_b = EXTENSION_A.replacen("value='one'", "value='changed'", 1);
        let mut definition_b_tx = transaction(&dice, MODULE, &extension_b, true, None).await;
        let (definition_b, definition_b_mapping, definition_b_child) = observed_transaction_state(&mut definition_b_tx, &key, &mapping_key).await;
        assert_eq!(definition_b_mapping.result(), a_mapping.result()); assert_ne!(definition_b_child.result(), a_definition.result()); assert_ne!(definition_b.result(), a.result());
        let mut definition_restore_tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let (definition_restored, definition_restored_mapping, definition_restored_child) = observed_transaction_state(&mut definition_restore_tx, &key, &mapping_key).await;
        assert_eq!(definition_restored.result(), a.result()); assert_eq!(definition_restored_mapping.result(), a_mapping.result()); assert_eq!(definition_restored_child.result(), a_definition.result());

        let neutral_module = format!("{MODULE}\n");
        let mut neutral_tx = transaction(&dice, &neutral_module, EXTENSION_A, true, None).await;
        let (neutral, neutral_mapping, neutral_definition) = observed_transaction_state(&mut neutral_tx, &key, &mapping_key).await;
        assert_eq!(neutral.result(), a.result()); assert_eq!(neutral_mapping.result(), a_mapping.result()); assert_eq!(neutral_definition.result(), a_definition.result());
        assert_ne!(neutral.observations(), a.observations()); assert_ne!(neutral_mapping.observations(), a_mapping.observations()); assert_ne!(neutral_definition.observations(), a_definition.observations()); assert_ne!(neutral, a); assert_ne!(neutral_mapping, a_mapping); assert_ne!(neutral_definition, a_definition);
        assert_eq!(a.result(), &a_result); assert_eq!(a_mapping.result(), &a_mapping_result); assert_eq!(a_definition.result(), &a_definition_result);
        assert_eq!(a.observations(), &a_observations); assert_eq!(a_mapping.observations(), &a_mapping_observations); assert_eq!(a_definition.observations(), &a_definition_observations);
        assert_ne!(mapping_b_definition.result(), a_definition.result());

        let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let cancel_tracker = Arc::new(CompositionTracker::default());
        let mut cancelled = transaction(&cancel_dice, MODULE, EXTENSION_A, true, Some(cancel_tracker.clone())).await;
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| { assert!(std::future::Future::poll(future.as_mut(), context).is_pending()); std::task::Poll::Ready(()) }).await;
        drop(future);
        assert!(cancel_tracker.activations.lock().unwrap().iter().all(|(name, _, _)| name != &key.to_string()));
        assert!(cancel_tracker.dependencies.lock().unwrap().iter().all(|(name, _)| name != &key.to_string()));
        let mut recovery = transaction(&cancel_dice, MODULE, EXTENSION_A, true, Some(cancel_tracker.clone())).await;
        let (recovered, recovered_mapping, recovered_definition) = observed_transaction_state(&mut recovery, &key, &mapping_key).await;
        assert_eq!(recovered.result(), a.result()); assert_eq!(recovered_mapping.result(), a_mapping.result()); assert_eq!(recovered_definition.result(), a_definition.result());

        let activations = cancel_tracker.activations.lock().unwrap();
        let dependencies = cancel_tracker.dependencies.lock().unwrap();
        assert!(activations.iter().all(|(name, _, _)| !name.starts_with("host-root-apparent-repository-definition:")));
        assert!(dependencies.iter().all(|(name, children)| !name.starts_with("host-root-apparent-repository-definition:") && children.iter().all(|child| !child.starts_with("host-root-apparent-repository-definition:"))));
        for forbidden in ["HostRootApparentRepositoryRouteKey", "HostRootApparentRepositorySourceInputKey", "HostRootApparentRepositorySourceObservationKey", "HostRootApparentRepositorySourcePathInputKey", "root-repository-route:", "repository-package-source:", "repository-source-file:", "host-repository-source-file:", "build-command-root:"] {
            assert!(activations.iter().all(|(name, _, _)| !name.contains(forbidden)));
            assert!(dependencies.iter().all(|(name, children)| !name.contains(forbidden) && children.iter().all(|child| !child.contains(forbidden))));
        }
        assert!(!a_mapping.observations().observations().is_empty()); assert!(!a_definition.observations().observations().is_empty());
        assert!(!mapping_b_child.observations().observations().is_empty()); assert!(!mapping_b_definition.observations().observations().is_empty());
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
            present(
                NormalizedAbsolutePath::new(format!("{WORKSPACE}/MODULE.bazel")).unwrap(),
                PathNodeKind::RegularFile,
                3,
            ),
            (
                demand(
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/MODULE.bazel")).unwrap(),
                    PathObservationOperation::FileBytes,
                ),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    module.clone().into_bytes(),
                ))),
            ),
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
