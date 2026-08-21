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
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinition;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionError;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionErrorDisposition;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionKey;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionObservationError;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionObservationKey;
use slug_bzlmod_v2::HostRepositoryLocalPathPolicy;
use slug_bzlmod_v2::HostRootRepositoryMapping;
use slug_bzlmod_v2::HostRootRepositoryMappingError;
use slug_bzlmod_v2::HostRootRepositoryMappingKey;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationError;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_loading_v2::HostGeneratedRepositoryMapping;
use slug_loading_v2::HostValidatedGeneratedRepositorySpecs;
use slug_loading_v2::HostValidatedGeneratedRepositorySpecsError;
use slug_loading_v2::HostValidatedModuleExtensionRepositoriesKey;
use slug_loading_v2::HostValidatedModuleExtensionRepositoriesObservationError;
use slug_loading_v2::HostValidatedModuleExtensionRepositoriesObservationKey;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostGeneratedRepositoryDefinition {
    certificate: Arc<HostValidatedGeneratedRepositorySpecs>,
    ordinal: usize,
}

#[derive(Debug, Clone, Copy)]
struct HostGeneratedRepositoryDefinitionView<'a> {
    canonical_name: &'a CanonicalRepoName,
    internal_name: &'a str,
    repo_spec: &'a RepoSpec,
    mapping: HostGeneratedRepositoryMapping<'a>,
}

impl HostGeneratedRepositoryDefinition {
    fn view(&self) -> Option<HostGeneratedRepositoryDefinitionView<'_>> {
        self.certificate.iter().nth(self.ordinal).map(
            |(canonical_name, repo_spec, internal_name, mapping)| {
                HostGeneratedRepositoryDefinitionView {
                    canonical_name,
                    internal_name,
                    repo_spec,
                    mapping,
                }
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostGeneratedRepositoryDefinitionErrorKind {
    Loading(HostValidatedGeneratedRepositorySpecsError),
    LoadingCompute(Arc<str>),
    Missing {
        certificate: Arc<HostValidatedGeneratedRepositorySpecs>,
    },
    Duplicate {
        certificate: Arc<HostValidatedGeneratedRepositorySpecs>,
        first: usize,
        conflicting: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostGeneratedRepositoryDefinitionError {
    requested: CanonicalRepoName,
    kind: HostGeneratedRepositoryDefinitionErrorKind,
}

impl fmt::Display for HostGeneratedRepositoryDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "generated repository '{}': {:?}",
            self.requested, self.kind
        )
    }
}

impl std::error::Error for HostGeneratedRepositoryDefinitionError {}

type HostGeneratedRepositoryDefinitionOutcome = SourcePreparationOutcome<
    Arc<Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>>,
>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostGeneratedRepositoryDefinitionKey {
    workspace: NormalizedAbsolutePath,
    canonical_repo: CanonicalRepoName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostGeneratedRepositoryDefinitionObservationKey(HostGeneratedRepositoryDefinitionKey);

impl HostGeneratedRepositoryDefinitionObservationKey {
    fn new(workspace: NormalizedAbsolutePath, canonical_repo: CanonicalRepoName) -> Self {
        Self(HostGeneratedRepositoryDefinitionKey::new(
            workspace,
            canonical_repo,
        ))
    }
}

impl fmt::Display for HostGeneratedRepositoryDefinitionObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct ObservedHostGeneratedRepositoryDefinition {
    result: Arc<Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostGeneratedRepositoryDefinition {
    fn result(
        &self,
    ) -> &Arc<Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>>
    {
        &self.result
    }

    fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum HostGeneratedRepositoryDefinitionObservationError {
    Validation(HostValidatedModuleExtensionRepositoriesObservationError),
}

impl HostGeneratedRepositoryDefinitionKey {
    fn new(workspace: NormalizedAbsolutePath, canonical_repo: CanonicalRepoName) -> Self {
        Self {
            workspace,
            canonical_repo,
        }
    }
}

impl fmt::Display for HostGeneratedRepositoryDefinitionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-generated-repository-definition:{}:{}",
            self.workspace, self.canonical_repo
        )
    }
}

#[derive(Clone, Copy)]
enum GeneratedRepositoryDefinitionMode {
    Legacy,
    Observed,
}

type GeneratedRepositoryDefinitionResult =
    Arc<Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>>;
type GeneratedRepositoryDefinitionDriverOutcome = SourcePreparationOutcome<
    Result<
        (GeneratedRepositoryDefinitionResult, PathObservationEpoch),
        HostGeneratedRepositoryDefinitionObservationError,
    >,
>;

fn complete_generated_driver(
    value: Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>,
    observations: PathObservationEpoch,
) -> GeneratedRepositoryDefinitionDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UniqueOrdinalError {
    Missing,
    Duplicate { first: usize, conflicting: usize },
}

fn find_unique_ordinal<'a>(
    requested: &CanonicalRepoName,
    names: impl Iterator<Item = &'a CanonicalRepoName>,
) -> Result<usize, UniqueOrdinalError> {
    let mut first = None;
    let mut conflicting = None;
    for (ordinal, name) in names.enumerate() {
        if name != requested {
            continue;
        }
        if let Some(first) = first {
            conflicting.get_or_insert((first, ordinal));
        } else {
            first = Some(ordinal);
        }
    }
    match (first, conflicting) {
        (_, Some((first, conflicting))) => {
            Err(UniqueOrdinalError::Duplicate { first, conflicting })
        }
        (Some(first), None) => Ok(first),
        (None, None) => Err(UniqueOrdinalError::Missing),
    }
}

#[rustfmt::skip]
async fn compute_generated_repository_definition(
    ctx: &mut DiceComputations<'_>,
    key: &HostGeneratedRepositoryDefinitionKey,
    mode: GeneratedRepositoryDefinitionMode,
) -> GeneratedRepositoryDefinitionDriverOutcome {
    let (result, observations) = match mode {
        GeneratedRepositoryDefinitionMode::Legacy => match ctx.compute(&HostValidatedModuleExtensionRepositoriesKey::new(key.workspace.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
            Err(error) => return complete_generated_driver(Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::LoadingCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
        },
        GeneratedRepositoryDefinitionMode::Observed => match ctx.compute(&HostValidatedModuleExtensionRepositoriesObservationKey::new(key.workspace.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(HostGeneratedRepositoryDefinitionObservationError::Validation(error))),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
            Err(error) => return complete_generated_driver(Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::LoadingCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
        },
    };
    let certificate = match result.as_ref() {
        Ok(value) => Arc::new(value.clone()),
        Err(error) => return complete_generated_driver(Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(error.clone()) }), observations),
    };
    let value = match find_unique_ordinal(&key.canonical_repo, certificate.iter().map(|(canonical, _, _, _)| canonical)) {
        Ok(ordinal) => Ok(HostGeneratedRepositoryDefinition { certificate, ordinal }),
        Err(UniqueOrdinalError::Missing) => Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::Missing { certificate } }),
        Err(UniqueOrdinalError::Duplicate { first, conflicting }) => Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::Duplicate { certificate, first, conflicting } }),
    };
    complete_generated_driver(value, observations)
}

#[async_trait]
impl Key for HostGeneratedRepositoryDefinitionKey {
    type Value = HostGeneratedRepositoryDefinitionOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_generated_repository_definition(
            ctx,
            self,
            GeneratedRepositoryDefinitionMode::Legacy,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy generated definition has no observed outer")
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
impl Key for HostGeneratedRepositoryDefinitionObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostGeneratedRepositoryDefinition,
            HostGeneratedRepositoryDefinitionObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_generated_repository_definition(
            ctx,
            &self.0,
            GeneratedRepositoryDefinitionMode::Observed,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostGeneratedRepositoryDefinition {
                    result,
                    observations,
                }))
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

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostCanonicalRepositoryDefinitionSource {
    Selected(HostCanonicalSelectedModuleDefinition),
    Generated(HostGeneratedRepositoryDefinition),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostCanonicalRepositoryDefinition {
    source: HostCanonicalRepositoryDefinitionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HostCanonicalRepositoryDefinitionKind {
    Root,
    SelectedRegistry,
    SelectedNonregistry,
    Generated,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HostCanonicalRepositoryDefinitionView<'a> {
    kind: HostCanonicalRepositoryDefinitionKind,
    canonical_repo: &'a CanonicalRepoName,
    mapping_context: &'a CanonicalRepoName,
    repo_spec: Option<&'a RepoSpec>,
    local_path_policy: Option<HostRepositoryLocalPathPolicy>,
}

impl HostCanonicalRepositoryDefinition {
    pub(super) fn view(&self) -> Option<HostCanonicalRepositoryDefinitionView<'_>> {
        let (kind, canonical_repo, mapping_context, repo_spec, local_path_policy) =
            match &self.source {
                HostCanonicalRepositoryDefinitionSource::Selected(value) => {
                    let view = value.view();
                    let kind = match view.kind() {
                        slug_bzlmod_v2::HostCanonicalSelectedModuleKind::Root => {
                            HostCanonicalRepositoryDefinitionKind::Root
                        }
                        slug_bzlmod_v2::HostCanonicalSelectedModuleKind::SelectedRegistry => {
                            HostCanonicalRepositoryDefinitionKind::SelectedRegistry
                        }
                        slug_bzlmod_v2::HostCanonicalSelectedModuleKind::SelectedNonregistry => {
                            HostCanonicalRepositoryDefinitionKind::SelectedNonregistry
                        }
                    };
                    (
                        kind,
                        view.canonical_repo(),
                        view.mapping_context(),
                        view.repo_spec(),
                        view.local_path_policy(),
                    )
                }
                HostCanonicalRepositoryDefinitionSource::Generated(value) => {
                    let view = value.view()?;
                    (
                        HostCanonicalRepositoryDefinitionKind::Generated,
                        view.canonical_name,
                        view.mapping.context_repo(),
                        Some(view.repo_spec),
                        Some(HostRepositoryLocalPathPolicy::LocalUnsupported),
                    )
                }
            };
        Some(HostCanonicalRepositoryDefinitionView {
            kind,
            canonical_repo,
            mapping_context,
            repo_spec,
            local_path_policy,
        })
    }

    fn mapping_target(&self, apparent_repo: &ApparentRepoName) -> Option<&CanonicalRepoName> {
        match &self.source {
            HostCanonicalRepositoryDefinitionSource::Selected(value) => value
                .view()
                .mapping()
                .find_map(|(apparent, canonical)| (apparent == apparent_repo).then_some(canonical)),
            HostCanonicalRepositoryDefinitionSource::Generated(value) => {
                value.view()?.mapping.entries().get(apparent_repo)
            }
        }
    }
}

impl<'a> HostCanonicalRepositoryDefinitionView<'a> {
    pub(super) fn kind(self) -> HostCanonicalRepositoryDefinitionKind {
        self.kind
    }

    pub(super) fn canonical_repo(self) -> &'a CanonicalRepoName {
        self.canonical_repo
    }

    pub(super) fn mapping_context(self) -> &'a CanonicalRepoName {
        self.mapping_context
    }

    pub(super) fn repo_spec(self) -> Option<&'a RepoSpec> {
        self.repo_spec
    }

    pub(super) fn local_path_policy(self) -> Option<HostRepositoryLocalPathPolicy> {
        self.local_path_policy
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostCanonicalRepositoryDefinitionErrorKind {
    Selected(HostCanonicalSelectedModuleDefinitionError),
    SelectedCompute(Arc<str>),
    Generated {
        selected_missing: HostCanonicalSelectedModuleDefinitionError,
        error: HostGeneratedRepositoryDefinitionError,
    },
    GeneratedCompute {
        selected_missing: HostCanonicalSelectedModuleDefinitionError,
        message: Arc<str>,
    },
    Missing {
        selected_missing: HostCanonicalSelectedModuleDefinitionError,
        generated_missing: HostGeneratedRepositoryDefinitionError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostCanonicalRepositoryDefinitionError {
    canonical_repo: CanonicalRepoName,
    kind: HostCanonicalRepositoryDefinitionErrorKind,
}

impl HostCanonicalRepositoryDefinitionError {
    pub(super) fn is_missing(&self) -> bool {
        matches!(
            self.kind,
            HostCanonicalRepositoryDefinitionErrorKind::Missing { .. }
        )
    }
}

impl fmt::Display for HostCanonicalRepositoryDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "canonical repository '{}': {:?}",
            self.canonical_repo, self.kind
        )
    }
}

impl std::error::Error for HostCanonicalRepositoryDefinitionError {}

pub(super) type HostCanonicalRepositoryDefinitionOutcome = SourcePreparationOutcome<
    Arc<Result<HostCanonicalRepositoryDefinition, HostCanonicalRepositoryDefinitionError>>,
>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostCanonicalRepositoryDefinitionKey {
    workspace: NormalizedAbsolutePath,
    canonical_repo: CanonicalRepoName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostCanonicalRepositoryDefinitionObservationKey(HostCanonicalRepositoryDefinitionKey);

impl HostCanonicalRepositoryDefinitionObservationKey {
    fn new(workspace: NormalizedAbsolutePath, canonical_repo: CanonicalRepoName) -> Self {
        Self(HostCanonicalRepositoryDefinitionKey::new(
            workspace,
            canonical_repo,
        ))
    }
}

impl fmt::Display for HostCanonicalRepositoryDefinitionObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type CanonicalRepositoryDefinitionResult =
    Arc<Result<HostCanonicalRepositoryDefinition, HostCanonicalRepositoryDefinitionError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct ObservedHostCanonicalRepositoryDefinition {
    result: CanonicalRepositoryDefinitionResult,
    observations: PathObservationEpoch,
}

impl ObservedHostCanonicalRepositoryDefinition {
    fn result(&self) -> &CanonicalRepositoryDefinitionResult {
        &self.result
    }

    fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostCanonicalRepositoryDefinitionObservationError {
    Selected(HostCanonicalSelectedModuleDefinitionObservationError),
    Generated {
        selected_missing: HostCanonicalSelectedModuleDefinitionError,
        error: HostGeneratedRepositoryDefinitionObservationError,
    },
    Merge {
        selected_missing: HostCanonicalSelectedModuleDefinitionError,
        error: ObservedPathFrontierError,
    },
}

impl Dupe for HostCanonicalRepositoryDefinitionObservationError {}

impl HostCanonicalRepositoryDefinitionKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        canonical_repo: CanonicalRepoName,
    ) -> Self {
        Self {
            workspace,
            canonical_repo,
        }
    }
}

impl fmt::Display for HostCanonicalRepositoryDefinitionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-canonical-repository-definition:{}:{}",
            self.workspace, self.canonical_repo
        )
    }
}

fn complete_canonical_driver(
    value: Result<HostCanonicalRepositoryDefinition, HostCanonicalRepositoryDefinitionError>,
    observations: PathObservationEpoch,
) -> CanonicalRepositoryDefinitionDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations)))
}

fn merge_canonical_observations(
    selected: &PathObservationEpoch,
    generated: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        selected
            .observations()
            .iter()
            .chain(generated.observations().iter())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(ObservedPathFrontierError::from)
}

#[derive(Clone, Copy)]
enum CanonicalRepositoryDefinitionMode {
    Legacy,
    Observed,
}

type CanonicalRepositoryDefinitionDriverOutcome = SourcePreparationOutcome<
    Result<
        (CanonicalRepositoryDefinitionResult, PathObservationEpoch),
        HostCanonicalRepositoryDefinitionObservationError,
    >,
>;

#[rustfmt::skip]
async fn compute_canonical_repository_definition(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositoryDefinitionKey,
    mode: CanonicalRepositoryDefinitionMode,
) -> CanonicalRepositoryDefinitionDriverOutcome {
    let (selected, selected_observations) = match mode {
        CanonicalRepositoryDefinitionMode::Legacy => match ctx.compute(&HostCanonicalSelectedModuleDefinitionKey::new(key.workspace.clone(), key.canonical_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
            Err(error) => return complete_canonical_driver(Err(HostCanonicalRepositoryDefinitionError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryDefinitionErrorKind::SelectedCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
        },
        CanonicalRepositoryDefinitionMode::Observed => match ctx.compute(&HostCanonicalSelectedModuleDefinitionObservationKey::new(key.workspace.clone(), key.canonical_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(HostCanonicalRepositoryDefinitionObservationError::Selected(error))),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
            Err(error) => return complete_canonical_driver(Err(HostCanonicalRepositoryDefinitionError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryDefinitionErrorKind::SelectedCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
        },
    };
    let selected_missing = match selected.as_ref() {
        Ok(value) => return complete_canonical_driver(Ok(HostCanonicalRepositoryDefinition { source: HostCanonicalRepositoryDefinitionSource::Selected(value.clone()) }), selected_observations),
        Err(error) if error.disposition() == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing => error.clone(),
        Err(error) => return complete_canonical_driver(Err(HostCanonicalRepositoryDefinitionError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryDefinitionErrorKind::Selected(error.clone()) }), selected_observations),
    };
    let (generated, generated_observations) = match mode {
        CanonicalRepositoryDefinitionMode::Legacy => match ctx.compute(&HostGeneratedRepositoryDefinitionKey::new(key.workspace.clone(), key.canonical_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
            Err(error) => return complete_canonical_driver(Err(HostCanonicalRepositoryDefinitionError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryDefinitionErrorKind::GeneratedCompute { selected_missing, message: error.to_string().into() } }), selected_observations),
        },
        CanonicalRepositoryDefinitionMode::Observed => match ctx.compute(&HostGeneratedRepositoryDefinitionObservationKey::new(key.workspace.clone(), key.canonical_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(HostCanonicalRepositoryDefinitionObservationError::Generated { selected_missing, error })),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
            Err(error) => return complete_canonical_driver(Err(HostCanonicalRepositoryDefinitionError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryDefinitionErrorKind::GeneratedCompute { selected_missing, message: error.to_string().into() } }), selected_observations),
        },
    };
    let observations = match merge_canonical_observations(&selected_observations, &generated_observations) {
        Ok(observations) => observations,
        Err(error) => return SourcePreparationOutcome::Complete(Err(HostCanonicalRepositoryDefinitionObservationError::Merge { selected_missing, error })),
    };
    let value = match generated.as_ref() {
        Ok(value) => Ok(HostCanonicalRepositoryDefinition { source: HostCanonicalRepositoryDefinitionSource::Generated(value.clone()) }),
        Err(error) if matches!(error.kind, HostGeneratedRepositoryDefinitionErrorKind::Missing { .. }) => Err(HostCanonicalRepositoryDefinitionError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryDefinitionErrorKind::Missing { selected_missing, generated_missing: error.clone() } }),
        Err(error) => Err(HostCanonicalRepositoryDefinitionError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryDefinitionErrorKind::Generated { selected_missing, error: error.clone() } }),
    };
    complete_canonical_driver(value, observations)
}

#[async_trait]
impl Key for HostCanonicalRepositoryDefinitionKey {
    type Value = HostCanonicalRepositoryDefinitionOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_canonical_repository_definition(
            ctx,
            self,
            CanonicalRepositoryDefinitionMode::Legacy,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy canonical definition has no observed outer")
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
impl Key for HostCanonicalRepositoryDefinitionObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostCanonicalRepositoryDefinition,
            HostCanonicalRepositoryDefinitionObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_canonical_repository_definition(
            ctx,
            &self.0,
            CanonicalRepositoryDefinitionMode::Observed,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostCanonicalRepositoryDefinition {
                    result,
                    observations,
                }))
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

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostCanonicalRepositoryApparentMapping {
    predecessor: ApparentMappingPredecessor,
    apparent_repo: ApparentRepoName,
}

impl HostCanonicalRepositoryApparentMapping {
    pub(super) fn resolved_target(&self) -> Option<&CanonicalRepoName> {
        match &self.predecessor {
            ApparentMappingPredecessor::Root(predecessor) => predecessor
                .view()?
                .mapping()
                .find_map(|(name, target)| (name == &self.apparent_repo).then_some(target)),
            ApparentMappingPredecessor::Canonical(predecessor) => {
                predecessor.mapping_target(&self.apparent_repo)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum ApparentMappingPredecessor {
    Root(HostRootRepositoryMapping),
    Canonical(HostCanonicalRepositoryDefinition),
}

impl ApparentMappingPredecessor {
    fn contexts(&self) -> Option<(&CanonicalRepoName, &CanonicalRepoName)> {
        match self {
            Self::Root(predecessor) => predecessor
                .view()
                .map(|view| (view.canonical_repo(), view.mapping_context())),
            Self::Canonical(predecessor) => predecessor
                .view()
                .map(|view| (view.canonical_repo(), view.mapping_context())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostCanonicalRepositoryApparentMappingErrorKind {
    RootApparent,
    RootMapping(HostRootRepositoryMappingError),
    RootMappingCompute(Arc<str>),
    Definition(HostCanonicalRepositoryDefinitionError),
    DefinitionCompute(Arc<str>),
    ContextMismatch {
        predecessor: ApparentMappingPredecessor,
    },
    Missing {
        predecessor: ApparentMappingPredecessor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostCanonicalRepositoryApparentMappingError {
    context_repo: CanonicalRepoName,
    apparent_repo: ApparentRepoName,
    kind: HostCanonicalRepositoryApparentMappingErrorKind,
}

impl fmt::Display for HostCanonicalRepositoryApparentMappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "canonical repository '{}' apparent mapping '{}': {:?}",
            self.context_repo, self.apparent_repo, self.kind
        )
    }
}

impl std::error::Error for HostCanonicalRepositoryApparentMappingError {}

pub(super) type HostCanonicalRepositoryApparentMappingOutcome = SourcePreparationOutcome<
    Arc<
        Result<HostCanonicalRepositoryApparentMapping, HostCanonicalRepositoryApparentMappingError>,
    >,
>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostCanonicalRepositoryApparentMappingKey {
    workspace: NormalizedAbsolutePath,
    context_repo: CanonicalRepoName,
    apparent_repo: ApparentRepoName,
}

impl HostCanonicalRepositoryApparentMappingKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        context_repo: CanonicalRepoName,
        apparent_repo: ApparentRepoName,
    ) -> Self {
        Self {
            workspace,
            context_repo,
            apparent_repo,
        }
    }
}

impl fmt::Display for HostCanonicalRepositoryApparentMappingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-canonical-repository-apparent-mapping:{}:{}:{}",
            self.workspace, self.context_repo, self.apparent_repo
        )
    }
}

#[rustfmt::skip]
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostCanonicalRepositoryApparentMappingObservationKey(HostCanonicalRepositoryApparentMappingKey);

#[rustfmt::skip]
#[allow(dead_code)]
impl HostCanonicalRepositoryApparentMappingObservationKey {
    pub(super) fn new(workspace: NormalizedAbsolutePath, context_repo: CanonicalRepoName, apparent_repo: ApparentRepoName) -> Self { Self(HostCanonicalRepositoryApparentMappingKey::new(workspace, context_repo, apparent_repo)) }
}

impl fmt::Display for HostCanonicalRepositoryApparentMappingObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[rustfmt::skip]
type CanonicalRepositoryApparentMappingResult = Arc<Result<HostCanonicalRepositoryApparentMapping, HostCanonicalRepositoryApparentMappingError>>;

#[rustfmt::skip]
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct ObservedHostCanonicalRepositoryApparentMapping { result: CanonicalRepositoryApparentMappingResult, observations: PathObservationEpoch }

#[rustfmt::skip]
#[allow(dead_code)]
impl ObservedHostCanonicalRepositoryApparentMapping {
    pub(super) fn result(
        &self,
    ) -> &Arc<
        Result<
            HostCanonicalRepositoryApparentMapping,
            HostCanonicalRepositoryApparentMappingError,
        >,
    > { &self.result }
    pub(super) fn observations(&self) -> &PathObservationEpoch { &self.observations }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum CanonicalRepositoryApparentMappingObservationError {
    RootMapping(HostRootRepositoryMappingObservationError),
    Definition(HostCanonicalRepositoryDefinitionObservationError),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct HostCanonicalRepositoryApparentMappingObservationError(
    CanonicalRepositoryApparentMappingObservationError,
);

#[rustfmt::skip]
#[allow(dead_code)]
#[derive(Clone, Copy)]
enum CanonicalRepositoryApparentMappingMode { Legacy, Observed }

enum CanonicalRepositoryApparentMappingChildOutcome {
    Need(SourcePreparationNeeds),
    Outer(CanonicalRepositoryApparentMappingObservationError),
    Complete {
        result: Result<ApparentMappingPredecessor, HostCanonicalRepositoryApparentMappingErrorKind>,
        observations: PathObservationEpoch,
    },
}

#[rustfmt::skip]
type CanonicalRepositoryApparentMappingDriverOutcome = SourcePreparationOutcome<Result<(CanonicalRepositoryApparentMappingResult, PathObservationEpoch), CanonicalRepositoryApparentMappingObservationError>>;

fn complete_canonical_apparent_mapping_driver(
    key: &HostCanonicalRepositoryApparentMappingKey,
    value: Result<
        HostCanonicalRepositoryApparentMapping,
        HostCanonicalRepositoryApparentMappingErrorKind,
    >,
    observations: PathObservationEpoch,
) -> CanonicalRepositoryApparentMappingDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((
        Arc::new(
            value.map_err(|kind| HostCanonicalRepositoryApparentMappingError {
                context_repo: key.context_repo.clone(),
                apparent_repo: key.apparent_repo.clone(),
                kind,
            }),
        ),
        observations,
    )))
}

#[rustfmt::skip]
async fn root_mapping_apparent_mapping_child(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositoryApparentMappingKey,
    mode: CanonicalRepositoryApparentMappingMode,
) -> CanonicalRepositoryApparentMappingChildOutcome {
    let (result, observations) = match mode {
        CanonicalRepositoryApparentMappingMode::Legacy => match ctx.compute(&HostRootRepositoryMappingKey::new(key.workspace.clone())).await {
            Err(error) => return CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Err(HostCanonicalRepositoryApparentMappingErrorKind::RootMappingCompute(error.to_string().into())), observations: PathObservationEpoch::empty() },
            Ok(SourcePreparationOutcome::Need(need)) => return CanonicalRepositoryApparentMappingChildOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
        },
        CanonicalRepositoryApparentMappingMode::Observed => match ctx.compute(&HostRootRepositoryMappingObservationKey::new(key.workspace.clone())).await {
            Err(error) => return CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Err(HostCanonicalRepositoryApparentMappingErrorKind::RootMappingCompute(error.to_string().into())), observations: PathObservationEpoch::empty() },
            Ok(SourcePreparationOutcome::Need(need)) => return CanonicalRepositoryApparentMappingChildOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return CanonicalRepositoryApparentMappingChildOutcome::Outer(CanonicalRepositoryApparentMappingObservationError::RootMapping(error)),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
        },
    };
    let result = match result.as_ref() {
        Ok(value) => Ok(ApparentMappingPredecessor::Root(value.clone())),
        Err(error) => Err(HostCanonicalRepositoryApparentMappingErrorKind::RootMapping(error.clone())),
    };
    CanonicalRepositoryApparentMappingChildOutcome::Complete { result, observations }
}

#[rustfmt::skip]
async fn canonical_definition_apparent_mapping_child(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositoryApparentMappingKey,
    mode: CanonicalRepositoryApparentMappingMode,
) -> CanonicalRepositoryApparentMappingChildOutcome {
    let (result, observations) = match mode {
        CanonicalRepositoryApparentMappingMode::Legacy => match ctx.compute(&HostCanonicalRepositoryDefinitionKey::new(key.workspace.clone(), key.context_repo.clone())).await {
            Err(error) => return CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Err(HostCanonicalRepositoryApparentMappingErrorKind::DefinitionCompute(error.to_string().into())), observations: PathObservationEpoch::empty() },
            Ok(SourcePreparationOutcome::Need(need)) => return CanonicalRepositoryApparentMappingChildOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
        },
        CanonicalRepositoryApparentMappingMode::Observed => match ctx.compute(&HostCanonicalRepositoryDefinitionObservationKey::new(key.workspace.clone(), key.context_repo.clone())).await {
            Err(error) => return CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Err(HostCanonicalRepositoryApparentMappingErrorKind::DefinitionCompute(error.to_string().into())), observations: PathObservationEpoch::empty() },
            Ok(SourcePreparationOutcome::Need(need)) => return CanonicalRepositoryApparentMappingChildOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return CanonicalRepositoryApparentMappingChildOutcome::Outer(CanonicalRepositoryApparentMappingObservationError::Definition(error)),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
        },
    };
    let result = match result.as_ref() {
        Ok(value) => Ok(ApparentMappingPredecessor::Canonical(value.clone())),
        Err(error) => Err(HostCanonicalRepositoryApparentMappingErrorKind::Definition(
            error.clone(),
        )),
    };
    CanonicalRepositoryApparentMappingChildOutcome::Complete { result, observations }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MappingLookupStatus {
    ContextMismatch,
    Missing,
    Found,
}

fn mapping_lookup_status(
    requested: &CanonicalRepoName,
    published: &CanonicalRepoName,
    mapping_context: &CanonicalRepoName,
    has_target: impl FnOnce() -> bool,
) -> MappingLookupStatus {
    if published != requested || mapping_context != requested {
        MappingLookupStatus::ContextMismatch
    } else if has_target() {
        MappingLookupStatus::Found
    } else {
        MappingLookupStatus::Missing
    }
}

fn finish_canonical_repository_apparent_mapping(
    key: &HostCanonicalRepositoryApparentMappingKey,
    child: CanonicalRepositoryApparentMappingChildOutcome,
) -> CanonicalRepositoryApparentMappingDriverOutcome {
    let (predecessor, observations) = match child {
        CanonicalRepositoryApparentMappingChildOutcome::Need(need) => {
            return SourcePreparationOutcome::Need(need);
        }
        CanonicalRepositoryApparentMappingChildOutcome::Outer(error) => {
            return SourcePreparationOutcome::Complete(Err(error));
        }
        CanonicalRepositoryApparentMappingChildOutcome::Complete {
            result: Err(kind),
            observations,
        } => {
            return complete_canonical_apparent_mapping_driver(key, Err(kind), observations);
        }
        CanonicalRepositoryApparentMappingChildOutcome::Complete {
            result: Ok(predecessor),
            observations,
        } => (predecessor, observations),
    };
    let Some((canonical_repo, mapping_context)) = predecessor.contexts() else {
        return complete_canonical_apparent_mapping_driver(
            key,
            Err(HostCanonicalRepositoryApparentMappingErrorKind::ContextMismatch { predecessor }),
            observations,
        );
    };
    let status = mapping_lookup_status(&key.context_repo, canonical_repo, mapping_context, || {
        match &predecessor {
            ApparentMappingPredecessor::Root(value) => value
                .view()
                .is_some_and(|view| view.mapping().any(|(name, _)| name == &key.apparent_repo)),
            ApparentMappingPredecessor::Canonical(value) => {
                value.mapping_target(&key.apparent_repo).is_some()
            }
        }
    });
    let value = match status {
        MappingLookupStatus::ContextMismatch => {
            Err(HostCanonicalRepositoryApparentMappingErrorKind::ContextMismatch { predecessor })
        }
        MappingLookupStatus::Missing => {
            Err(HostCanonicalRepositoryApparentMappingErrorKind::Missing { predecessor })
        }
        MappingLookupStatus::Found => Ok(HostCanonicalRepositoryApparentMapping {
            predecessor,
            apparent_repo: key.apparent_repo.clone(),
        }),
    };
    complete_canonical_apparent_mapping_driver(key, value, observations)
}

async fn compute_canonical_repository_apparent_mapping(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositoryApparentMappingKey,
    mode: CanonicalRepositoryApparentMappingMode,
) -> CanonicalRepositoryApparentMappingDriverOutcome {
    if key.apparent_repo.is_root() && !key.context_repo.is_root() {
        return complete_canonical_apparent_mapping_driver(
            key,
            Err(HostCanonicalRepositoryApparentMappingErrorKind::RootApparent),
            PathObservationEpoch::empty(),
        );
    }
    let child = if key.context_repo.is_root() {
        root_mapping_apparent_mapping_child(ctx, key, mode).await
    } else {
        canonical_definition_apparent_mapping_child(ctx, key, mode).await
    };
    finish_canonical_repository_apparent_mapping(key, child)
}

fn project_legacy_canonical_repository_apparent_mapping(
    outcome: CanonicalRepositoryApparentMappingDriverOutcome,
) -> HostCanonicalRepositoryApparentMappingOutcome {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, observations))) => {
            debug_assert!(observations.observations().is_empty());
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy canonical apparent mapping has no observed outer")
        }
    }
}

#[async_trait]
impl Key for HostCanonicalRepositoryApparentMappingKey {
    type Value = HostCanonicalRepositoryApparentMappingOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_canonical_repository_apparent_mapping(
            compute_canonical_repository_apparent_mapping(
                ctx,
                self,
                CanonicalRepositoryApparentMappingMode::Legacy,
            )
            .await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostCanonicalRepositoryApparentMappingObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostCanonicalRepositoryApparentMapping,
            HostCanonicalRepositoryApparentMappingObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_canonical_repository_apparent_mapping(
            ctx,
            &self.0,
            CanonicalRepositoryApparentMappingMode::Observed,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(
                    HostCanonicalRepositoryApparentMappingObservationError(error),
                ))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(
                    ObservedHostCanonicalRepositoryApparentMapping {
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
    use std::cell::Cell;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use dupe::Dupe;
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::HostRepositorySourceFileKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::RegistryFileKey;
    use slug_bzlmod_v2::RegistryRequestGeneration;
    use slug_bzlmod_v2::RegistryUrls;
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationKey;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_bzlmod_v2::RepositoryPackageSourceKey;
    use slug_bzlmod_v2::RepositorySourceFileKey;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_bzlmod_v2::RootRepositoryRouteKey;
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_events_v2::EvaluationEvent;
    use slug_events_v2::EventBatch;
    use slug_loading_v2::HostValidatedGeneratedRepositorySpecsOutcome;
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

    use super::*;

    pub(in crate::runtime) const WORKSPACE: &str = "/generated-repository-definition";
    pub(in crate::runtime) const MODULE: &str = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, first='first', second='second')\n";
    pub(in crate::runtime) const EXTENSION_A: &str = r#"
repo=repository_rule(implementation=lambda ctx: None, attrs={'value':attr.string(), 'target':attr.label()})
def impl(ctx):
    repo(name='first', value='one', target=':local')
    repo(name='second', value='two', target='@first//:item')
ext=module_extension(implementation=impl)
"#;
    const EXTENSION_B: &str = r#"
other=repository_rule(implementation=lambda ctx: None, attrs={'value':attr.string(), 'target':attr.label()})
def impl(ctx):
    other(name='first', value='one', target=':local')
    other(name='second', value='two', target='@first//:item')
ext=module_extension(implementation=impl)
"#;

    #[derive(Default)]
    struct LookupTracker {
        canonical: Mutex<Vec<(ActivationKind, bool)>>,
        selected: Mutex<Vec<(ActivationKind, bool)>>,
        lookup: Mutex<Vec<(ActivationKind, bool)>>,
        apparent: Mutex<Vec<(ActivationKind, bool)>>,
        root_mapping: Mutex<Vec<(ActivationKind, bool)>>,
        forbidden: Mutex<Vec<&'static str>>,
        activations: Mutex<Vec<(String, ActivationKind, Option<EventBatch>)>>,
        dependencies: Mutex<Vec<(String, Vec<String>)>>,
    }

    impl ActivationTracker for LookupTracker {
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
            let batch = activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe);
            self.activations
                .lock()
                .unwrap()
                .push((key.to_string(), activation.kind(), batch));
            if key
                .downcast_ref::<HostCanonicalRepositoryDefinitionKey>()
                .is_some()
            {
                self.canonical
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key
                .downcast_ref::<HostCanonicalSelectedModuleDefinitionKey>()
                .is_some()
            {
                self.selected
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key
                .downcast_ref::<HostGeneratedRepositoryDefinitionKey>()
                .is_some()
            {
                self.lookup
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key
                .downcast_ref::<HostCanonicalRepositoryApparentMappingKey>()
                .is_some()
            {
                self.apparent
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key.downcast_ref::<HostRootRepositoryMappingKey>().is_some() {
                self.root_mapping
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
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

    async fn lookup(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        requested: Option<&CanonicalRepoName>,
    ) -> HostGeneratedRepositoryDefinitionOutcome {
        let mut tx = transaction(dice, module, extension, true, None).await;
        let mut generated = names(&validated(&mut tx).await);
        let name = requested.cloned().unwrap_or_else(|| generated.remove(0));
        tx.compute(&HostGeneratedRepositoryDefinitionKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            name,
        ))
        .await
        .unwrap()
    }

    async fn observed_lookup(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        extension_present: bool,
        requested: CanonicalRepoName,
        tracker: Option<Arc<LookupTracker>>,
    ) -> <HostGeneratedRepositoryDefinitionObservationKey as Key>::Value {
        transaction(
            dice,
            module,
            extension,
            extension_present,
            tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
        )
        .await
        .compute(&HostGeneratedRepositoryDefinitionObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            requested,
        ))
        .await
        .unwrap()
    }

    fn observed_carrier(
        value: &<HostGeneratedRepositoryDefinitionObservationKey as Key>::Value,
    ) -> &ObservedHostGeneratedRepositoryDefinition {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("expected observed generated definition carrier: {value:?}"),
        }
    }

    async fn observed_canonical_lookup(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        extension_present: bool,
        requested: CanonicalRepoName,
        tracker: Option<Arc<LookupTracker>>,
    ) -> <HostCanonicalRepositoryDefinitionObservationKey as Key>::Value {
        transaction(
            dice,
            module,
            extension,
            extension_present,
            tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
        )
        .await
        .compute(&HostCanonicalRepositoryDefinitionObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            requested,
        ))
        .await
        .unwrap()
    }

    fn observed_canonical_carrier(
        value: &<HostCanonicalRepositoryDefinitionObservationKey as Key>::Value,
    ) -> &ObservedHostCanonicalRepositoryDefinition {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("expected observed canonical definition carrier: {value:?}"),
        }
    }

    fn observed_apparent_mapping_carrier(
        value: &<HostCanonicalRepositoryApparentMappingObservationKey as Key>::Value,
    ) -> &ObservedHostCanonicalRepositoryApparentMapping {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("expected observed apparent mapping carrier: {value:?}"),
        }
    }

    fn assert_apparent_epoch_current(epoch: &PathObservationEpoch, global: &PathObservationEpoch) {
        for (demand, result) in epoch.observations() {
            assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
        }
    }

    fn activation_dependencies(tracker: &LookupTracker, key: &str) -> Vec<String> {
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

    fn starlark_print_owners(tracker: &LookupTracker) -> Vec<(String, String)> {
        tracker
            .activations
            .lock()
            .unwrap()
            .iter()
            .flat_map(|(name, _, batch)| {
                batch
                    .iter()
                    .flat_map(EventBatch::events)
                    .filter_map(|event| match event {
                        EvaluationEvent::StarlarkPrint { text, .. } => {
                            Some((name.clone(), text.to_string()))
                        }
                        _ => None,
                    })
            })
            .collect()
    }

    fn assert_activation_families_absent(tracker: &LookupTracker, families: &[&str]) {
        let activations = tracker.activations.lock().unwrap();
        let dependencies = tracker.dependencies.lock().unwrap();
        for family in families {
            assert!(
                activations
                    .iter()
                    .all(|(name, _, _)| !name.starts_with(family))
            );
            assert!(dependencies.iter().all(|(name, children)| {
                !name.starts_with(family) && children.iter().all(|child| !child.starts_with(family))
            }));
        }
    }

    fn snapshot(value: &HostGeneratedRepositoryDefinitionOutcome) -> Vec<String> {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("lookup must complete")
        };
        let view = value.as_ref().as_ref().unwrap().view().unwrap();
        vec![
            view.canonical_name.as_str().to_owned(),
            view.internal_name.to_owned(),
            view.repo_spec.rule_id.rule_name.to_string(),
            format!("{:?}", view.repo_spec.attributes),
            view.mapping.context_repo().as_str().to_owned(),
            format!("{:?}", view.mapping.entries()),
        ]
    }

    fn mapping(
        value: &HostCanonicalRepositoryApparentMappingOutcome,
    ) -> &HostCanonicalRepositoryApparentMapping {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("mapping must complete")
        };
        value.as_ref().as_ref().unwrap()
    }

    fn target(value: &HostCanonicalRepositoryApparentMappingOutcome) -> CanonicalRepoName {
        mapping(value).resolved_target().unwrap().clone()
    }

    async fn canonical_lookup(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        canonical_repo: CanonicalRepoName,
    ) -> HostCanonicalRepositoryDefinitionOutcome {
        transaction(dice, module, extension, true, None)
            .await
            .compute(&HostCanonicalRepositoryDefinitionKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                canonical_repo,
            ))
            .await
            .unwrap()
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum SyntheticDomainOutcome {
        Need,
        Success,
        Missing,
        Terminal,
    }

    fn synthetic_composition(
        selected: SyntheticDomainOutcome,
        generated: SyntheticDomainOutcome,
        generated_calls: &std::cell::Cell<usize>,
    ) -> SyntheticDomainOutcome {
        match selected {
            SyntheticDomainOutcome::Success
            | SyntheticDomainOutcome::Terminal
            | SyntheticDomainOutcome::Need => selected,
            SyntheticDomainOutcome::Missing => {
                generated_calls.set(generated_calls.get() + 1);
                generated
            }
        }
    }

    #[test]
    fn composition_branch_matrix_is_missing_only_and_selected_first() {
        use SyntheticDomainOutcome as O;

        for selected in [O::Success, O::Terminal, O::Need] {
            let calls = std::cell::Cell::new(0);
            assert_eq!(
                synthetic_composition(selected, O::Success, &calls),
                selected
            );
            assert_eq!(calls.get(), 0, "same-canonical generated candidate ran");
        }
        for (generated, expected) in [
            (O::Success, O::Success),
            (O::Terminal, O::Terminal),
            (O::Missing, O::Missing),
            (O::Need, O::Need),
        ] {
            let calls = std::cell::Cell::new(0);
            assert_eq!(
                synthetic_composition(O::Missing, generated, &calls),
                expected
            );
            assert_eq!(calls.get(), 1);
        }
    }

    #[test]
    fn complete_scan_rejects_missing_and_duplicate() {
        use std::cell::Cell;

        let requested = CanonicalRepoName::new("wanted").unwrap();
        let other = CanonicalRepoName::new("other").unwrap();
        assert_eq!(
            find_unique_ordinal(&requested, [].iter()),
            Err(UniqueOrdinalError::Missing)
        );
        assert_eq!(
            find_unique_ordinal(&requested, [&other, &requested].into_iter()),
            Ok(1)
        );
        let consumed = Cell::new(0);
        let names = [&requested, &other, &requested, &other];
        assert_eq!(
            find_unique_ordinal(
                &requested,
                names
                    .into_iter()
                    .inspect(|_| consumed.set(consumed.get() + 1)),
            ),
            Err(UniqueOrdinalError::Duplicate {
                first: 0,
                conflicting: 2
            })
        );
        assert_eq!(consumed.get(), names.len());
    }
    #[test]
    fn mapping_context_mismatch_precedes_target_lookup() {
        let requested = CanonicalRepoName::new("requested").unwrap();
        let other = CanonicalRepoName::new("other").unwrap();
        let target_checks = Cell::new(0);
        let has_target = || {
            target_checks.set(target_checks.get() + 1);
            false
        };
        assert_eq!(
            mapping_lookup_status(&requested, &other, &requested, has_target),
            MappingLookupStatus::ContextMismatch
        );
        assert_eq!(target_checks.get(), 0);
        assert_eq!(
            mapping_lookup_status(&requested, &requested, &other, has_target),
            MappingLookupStatus::ContextMismatch
        );
        assert_eq!(target_checks.get(), 0);
        assert_eq!(
            mapping_lookup_status(&requested, &requested, &requested, has_target),
            MappingLookupStatus::Missing
        );
        assert_eq!(target_checks.get(), 1);
        assert_eq!(
            mapping_lookup_status(&requested, &requested, &requested, || true),
            MappingLookupStatus::Found
        );
    }
    #[tokio::test]
    async fn canonical_definition_selects_before_missing_only_generated_fallback() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let generated = names(&validated(&mut tx).await);
        tracker.canonical.lock().unwrap().clear();
        tracker.selected.lock().unwrap().clear();
        tracker.lookup.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = |canonical_repo| {
            HostCanonicalRepositoryDefinitionKey::new(workspace.clone(), canonical_repo)
        };
        let root_key = key(CanonicalRepoName::root());
        let root = tx.compute(&root_key).await.unwrap();
        let SourcePreparationOutcome::Complete(root_value) = &root else {
            panic!("root definition must complete")
        };
        let root_definition = root_value.as_ref().as_ref().unwrap();
        let root_view = root_definition.view().unwrap();
        assert_eq!(
            root_view.kind(),
            HostCanonicalRepositoryDefinitionKind::Root
        );
        let HostCanonicalRepositoryDefinitionSource::Selected(root_certificate) =
            &root_definition.source
        else {
            panic!("root must be selected")
        };
        let root_view = root_certificate.view();
        assert_eq!(root_view.canonical_repo(), &CanonicalRepoName::root());
        assert!(root_view.repo_spec().is_none());
        assert!(tracker.lookup.lock().unwrap().is_empty());
        let mut selected_error_tx = transaction(
            &dice,
            "module(name='root')\nbazel_dep(name='missing', version='1')\n",
            EXTENSION_A,
            true,
            Some(tracker.clone()),
        )
        .await;
        let selected_error = selected_error_tx
            .compute(&key(CanonicalRepoName::root()))
            .await
            .unwrap();
        assert!(matches!(
            selected_error,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryDefinitionError {
                        kind: HostCanonicalRepositoryDefinitionErrorKind::Selected(error),
                        ..
                    }) if error.disposition()
                        == HostCanonicalSelectedModuleDefinitionErrorDisposition::Terminal
                )
        ));
        assert!(tracker.lookup.lock().unwrap().is_empty());
        let selected_mapping_terminal = selected_error_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                CanonicalRepoName::new("missing+").unwrap(),
                ApparentRepoName::new("bazel_tools").unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(value) = selected_mapping_terminal else {
            panic!("selected terminal must complete")
        };
        assert!(matches!(
            value.as_ref(),
            Err(HostCanonicalRepositoryApparentMappingError {
                kind: HostCanonicalRepositoryApparentMappingErrorKind::Definition(_),
                ..
            })
        ));
        assert!(tracker.lookup.lock().unwrap().is_empty());
        tracker.forbidden.lock().unwrap().clear();
        let generated_key = key(generated[0].clone());
        let generated_value = tx.compute(&generated_key).await.unwrap();
        let SourcePreparationOutcome::Complete(value) = &generated_value else {
            panic!("generated definition must complete")
        };
        let generated_definition = value.as_ref().as_ref().unwrap();
        let generated_view = generated_definition.view().unwrap();
        assert_eq!(
            generated_view.kind(),
            HostCanonicalRepositoryDefinitionKind::Generated
        );
        let HostCanonicalRepositoryDefinitionSource::Generated(generated_certificate) =
            &generated_definition.source
        else {
            panic!("generated repository must use generated domain")
        };
        let generated_view = generated_certificate.view().unwrap();
        assert_eq!(generated_view.canonical_name, &generated[0]);
        assert_eq!(generated_view.internal_name, "first");
        assert_eq!(generated_view.repo_spec.rule_id.rule_name, "repo");
        assert_eq!(generated_view.mapping.context_repo(), &generated[0]);
        let generated_terminal = transaction(&dice, MODULE, EXTENSION_A, false, None)
            .await
            .compute(&generated_key)
            .await
            .unwrap();
        assert!(matches!(
            &generated_terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryDefinitionError {
                        canonical_repo,
                        kind: HostCanonicalRepositoryDefinitionErrorKind::Generated {
                            selected_missing,
                            error,
                        },
                    }) if canonical_repo == &generated[0]
                        && selected_missing.disposition()
                            == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing
                        && matches!(
                            error.kind,
                            HostGeneratedRepositoryDefinitionErrorKind::Loading(_)
                        )
                )
        ));
        assert!(!HostCanonicalRepositoryDefinitionKey::equality(
            &generated_value,
            &generated_terminal,
        ));
        let missing_key = key(CanonicalRepoName::new("missing").unwrap());
        let missing = tx.compute(&missing_key).await.unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryDefinitionError {
                        canonical_repo,
                        kind: HostCanonicalRepositoryDefinitionErrorKind::Missing {
                            selected_missing,
                            generated_missing,
                        },
                    }) if canonical_repo.as_str() == "missing"
                        && selected_missing.disposition()
                            == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing
                        && matches!(
                            generated_missing.kind,
                            HostGeneratedRepositoryDefinitionErrorKind::Missing { .. }
                        )
                )
        ));
        let warm = tx.compute(&root_key).await.unwrap();
        assert!(HostCanonicalRepositoryDefinitionKey::equality(&root, &warm));
        assert_eq!(
            *tracker.canonical.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert_eq!(tracker.lookup.lock().unwrap().len(), 2);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let generated_b = canonical_lookup(&dice, MODULE, EXTENSION_B, generated[0].clone()).await;
        assert!(!HostCanonicalRepositoryDefinitionKey::equality(
            &generated_value,
            &generated_b,
        ));
        assert!(HostCanonicalRepositoryDefinitionKey::equality(
            &generated_value,
            &canonical_lookup(&dice, MODULE, EXTENSION_A, generated[0].clone()).await,
        ));
        let selected_b = canonical_lookup(
            &dice,
            &MODULE.replace("name='bazel_tools'", "name='root', repo_name='changed'"),
            EXTENSION_A,
            CanonicalRepoName::root(),
        )
        .await;
        assert!(!HostCanonicalRepositoryDefinitionKey::equality(
            &root,
            &selected_b
        ));
        assert!(HostCanonicalRepositoryDefinitionKey::equality(
            &root,
            &canonical_lookup(&dice, MODULE, EXTENSION_A, CanonicalRepoName::root(),).await,
        ));
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let need = updater
            .commit()
            .await
            .compute(&generated_key)
            .await
            .unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostCanonicalRepositoryDefinitionKey::validity(&need));
        assert!(!HostCanonicalRepositoryDefinitionKey::equality(
            &need, &need
        ));
    }
    #[tokio::test]
    async fn canonical_definition_borrows_nonregistry_selected_view() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let local_module = "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let local_key = HostCanonicalRepositoryDefinitionKey::new(
            workspace.clone(),
            CanonicalRepoName::new("local+").unwrap(),
        );
        let local_mapping_key = HostCanonicalRepositoryApparentMappingKey::new(
            workspace.clone(),
            CanonicalRepoName::new("local+").unwrap(),
            ApparentRepoName::new("bazel_tools").unwrap(),
        );
        let local_need = transaction(
            &dice,
            local_module,
            EXTENSION_A,
            true,
            Some(tracker.clone()),
        )
        .await
        .compute(&local_mapping_key)
        .await
        .unwrap();
        assert!(!HostCanonicalRepositoryApparentMappingKey::validity(
            &local_need
        ));
        assert!(tracker.lookup.lock().unwrap().is_empty());
        let SourcePreparationOutcome::Need(need) = local_need else {
            panic!("local definition must first request materialization")
        };
        let request = need
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .clone();
        let mut updater = dice.updater_with_data(UserComputationData {
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.clone(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace,
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
        let mut local_tx = updater.commit().await;
        let local = local_tx.compute(&local_key).await.unwrap();
        let SourcePreparationOutcome::Complete(local) = &local else {
            panic!("local definition must complete: {local:?}")
        };
        let HostCanonicalRepositoryDefinitionSource::Selected(local) =
            &local.as_ref().as_ref().unwrap().source
        else {
            panic!("local definition must be selected")
        };
        let local_view = local.view();
        assert_eq!(
            local_view.kind(),
            slug_bzlmod_v2::HostCanonicalSelectedModuleKind::SelectedNonregistry
        );
        assert_eq!(local_view.canonical_repo().as_str(), "local+");
        assert_eq!(local_view.mapping_context().as_str(), "local+");
        assert_eq!(
            local_view.repo_spec().unwrap().rule_id.rule_name,
            "local_repository"
        );
        tracker.canonical.lock().unwrap().clear();
        tracker.lookup.lock().unwrap().clear();
        tracker.apparent.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let mapping_value = local_tx.compute(&local_mapping_key).await.unwrap();
        let selected_target = local_view
            .mapping()
            .find_map(|(apparent, canonical)| {
                (apparent.as_str() == "bazel_tools").then_some(canonical)
            })
            .unwrap();
        assert_eq!(target(&mapping_value), selected_target.clone());
        let borrowed_target = mapping(&mapping_value).resolved_target().unwrap();
        assert!(std::ptr::eq(selected_target, borrowed_target));
        assert_eq!(
            *tracker.canonical.lock().unwrap(),
            [(ActivationKind::Reused, false)]
        );
        assert!(tracker.lookup.lock().unwrap().is_empty());
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [(ActivationKind::Evaluated, false)]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        tracker.canonical.lock().unwrap().clear();
        tracker.root_mapping.lock().unwrap().clear();
        let root_local = local_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("local_alias").unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(target(&root_local).as_str(), "local+");
        assert!(tracker.canonical.lock().unwrap().is_empty());
        assert_eq!(
            *tracker.root_mapping.lock().unwrap(),
            [(ActivationKind::Evaluated, false)]
        );
    }

    #[tokio::test]
    async fn real_lookup_borrows_exact_definition_and_restores() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let validation = validated(&mut tx).await;
        let generated = names(&validation);
        assert_eq!(generated.len(), 2);
        tracker.forbidden.lock().unwrap().clear();

        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let first_key =
            HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), generated[0].clone());
        let second_key =
            HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), generated[1].clone());
        let first = tx.compute(&first_key).await.unwrap();
        let second = tx.compute(&second_key).await.unwrap();
        let warm = tx.compute(&first_key).await.unwrap();
        assert!(HostGeneratedRepositoryDefinitionKey::validity(&first));
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &first, &warm
        ));

        let SourcePreparationOutcome::Complete(first_value) = &first else {
            panic!("lookup must complete")
        };
        let SourcePreparationOutcome::Complete(second_value) = &second else {
            panic!("lookup must complete")
        };
        let first_view = first_value.as_ref().as_ref().unwrap().view().unwrap();
        let second_view = second_value.as_ref().as_ref().unwrap().view().unwrap();
        assert_eq!(first_view.canonical_name, &generated[0]);
        assert_eq!(first_view.internal_name, "first");
        assert_eq!(first_view.repo_spec.rule_id.rule_name.as_str(), "repo");
        assert_eq!(second_view.internal_name, "second");
        assert!(matches!(
            second_view.repo_spec.attributes.get("value"),
            Some(slug_bzlmod_v2::OverrideAttributeValue::String(value)) if value == "two"
        ));
        assert!(std::ptr::eq(
            first_view.mapping.entries(),
            second_view.mapping.entries()
        ));
        assert_eq!(first_view.mapping.context_repo(), &generated[0]);
        assert_eq!(second_view.mapping.context_repo(), &generated[1]);

        let missing_key = HostGeneratedRepositoryDefinitionKey::new(
            workspace.clone(),
            CanonicalRepoName::new("missing").unwrap(),
        );
        let missing = tx.compute(&missing_key).await.unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostGeneratedRepositoryDefinitionError {
                        kind: HostGeneratedRepositoryDefinitionErrorKind::Missing { .. },
                        ..
                    })
                )
        ));

        let baseline = snapshot(&first);
        for (case, (module, extension, changed_fields)) in [
            (MODULE.to_owned(), EXTENSION_B.to_owned(), &[2][..]),
            (
                MODULE.replace("first='first'", "first='renamed'"),
                EXTENSION_A.replacen("name='first'", "name='renamed'", 1),
                &[0, 1, 4, 5],
            ),
            (MODULE.to_owned(), EXTENSION_A.replace("value", "renamed_value"), &[3][..]),
            (MODULE.to_owned(), EXTENSION_A.replace("value='one'", "value='changed'"), &[3][..]),
            (
                MODULE.to_owned(),
                EXTENSION_A.replace("value='one', target=':local'", "target=':local', value='one'"),
                &[3][..],
            ),
            (MODULE.to_owned(), EXTENSION_A.replace("target=':local'", "target=':changed'"), &[3][..]),
            (
                MODULE.to_owned(),
                EXTENSION_A.replace(
                    "repo(name='first', value='one', target=':local')\n    repo(name='second', value='two', target='@first//:item')",
                    "repo(name='second', value='two', target='@first//:item')\n    repo(name='first', value='one', target=':local')",
                ),
                &[0, 1, 3, 4],
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let b = lookup(&dice, &module, &extension, None).await;
            assert!(!HostGeneratedRepositoryDefinitionKey::equality(&first, &b));
            let changed = snapshot(&b);
            assert!(
                changed_fields.iter().all(|index| baseline[*index] != changed[*index]),
                "case {case}: {baseline:?} == {changed:?}"
            );
            let a2 = lookup(&dice, MODULE, EXTENSION_A, None).await;
            assert!(HostGeneratedRepositoryDefinitionKey::equality(&first, &a2));
        }

        let inject_a = format!(
            "{MODULE}inject_repo(e, injected='bazel_tools')\ninject_repo(e, other='bazel_tools')\n"
        );
        let inject_b = format!(
            "{MODULE}inject_repo(e, other='bazel_tools')\ninject_repo(e, injected='bazel_tools')\n"
        );
        let mapping_a = lookup(&dice, &inject_a, EXTENSION_A, None).await;
        let mapping_b = lookup(&dice, &inject_b, EXTENSION_A, None).await;
        assert_ne!(snapshot(&mapping_a)[5], snapshot(&mapping_b)[5]);
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &mapping_a,
            &lookup(&dice, &inject_a, EXTENSION_A, None).await,
        ));
        let overridden = lookup(
            &dice,
            &format!("{MODULE}override_repo(e, first='bazel_tools')\n"),
            EXTENSION_A,
            None,
        )
        .await;
        assert_ne!(baseline[5], snapshot(&overridden)[5]);
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &first,
            &lookup(&dice, MODULE, EXTENSION_A, None).await,
        ));

        let multi_extension = EXTENSION_A.replace(
            "ext=module_extension(implementation=impl)",
            "first=module_extension(implementation=impl)\nsecond=module_extension(implementation=impl)",
        );
        let request_a = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\n";
        let request_b = "module(name='bazel_tools')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\n";
        let order_a = lookup(&dice, request_a, &multi_extension, None).await;
        let fixed = CanonicalRepoName::new(&snapshot(&order_a)[0]).unwrap();
        let order_b = lookup(&dice, request_b, &multi_extension, Some(&fixed)).await;
        assert_eq!(&snapshot(&order_a)[..2], &snapshot(&order_b)[..2]);
        assert!(!HostGeneratedRepositoryDefinitionKey::equality(
            &order_a, &order_b
        ));
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &order_a,
            &lookup(&dice, request_a, &multi_extension, Some(&fixed)).await,
        ));
        assert_eq!(
            *tracker.lookup.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
                (ActivationKind::Evaluated, false),
            ]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn real_apparent_mapping_borrows_effective_target_and_restores() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let generated = names(&validated(&mut tx).await);
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let definition_key =
            HostCanonicalRepositoryDefinitionKey::new(workspace.clone(), generated[0].clone());
        tx.compute(&definition_key).await.unwrap();
        tracker.canonical.lock().unwrap().clear();
        tracker.lookup.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let key = |context: CanonicalRepoName, apparent: &str| {
            HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                context,
                ApparentRepoName::new(apparent).unwrap(),
            )
        };
        let self_key = key(generated[0].clone(), "first");
        let sibling_key = key(generated[0].clone(), "second");
        let host_key = key(generated[0].clone(), "bazel_tools");
        let self_mapping = tx.compute(&self_key).await.unwrap();
        let sibling_mapping = tx.compute(&sibling_key).await.unwrap();
        let host_mapping = tx.compute(&host_key).await.unwrap();
        let warm = tx.compute(&self_key).await.unwrap();
        assert_eq!(target(&self_mapping), generated[0]);
        assert_eq!(target(&sibling_mapping), generated[1]);
        assert_eq!(target(&host_mapping), CanonicalRepoName::root());
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &sibling_mapping,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &warm,
        ));
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert_eq!(
            *tracker.canonical.lock().unwrap(),
            [
                (ActivationKind::Reused, false),
                (ActivationKind::Reused, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert!(tracker.lookup.lock().unwrap().is_empty());
        assert!(tracker.root_mapping.lock().unwrap().is_empty());
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let missing = tx
            .compute(&key(generated[0].clone(), "missing"))
            .await
            .unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Missing { .. },
                        ..
                    })
                )
        ));
        let predecessor_activations = tracker.canonical.lock().unwrap().len();
        let root_apparent = tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                generated[0].clone(),
                ApparentRepoName::root(),
            ))
            .await
            .unwrap();
        let root_context = tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("first").unwrap(),
            ))
            .await
            .unwrap();
        let root_self = tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                CanonicalRepoName::root(),
                ApparentRepoName::root(),
            ))
            .await
            .unwrap();
        assert_eq!(
            tracker.canonical.lock().unwrap().len(),
            predecessor_activations
        );
        assert!(matches!(
            root_apparent,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::RootApparent,
                        ..
                    })
                )
        ));
        assert_eq!(target(&root_context), generated[0]);
        assert_eq!(target(&root_self), CanonicalRepoName::root());
        let ApparentMappingPredecessor::Root(root_predecessor) =
            &mapping(&root_context).predecessor
        else {
            panic!("root lookup must retain root mapping predecessor")
        };
        let published_target = root_predecessor
            .view()
            .unwrap()
            .mapping()
            .find_map(|(name, target)| (name.as_str() == "first").then_some(target))
            .unwrap();
        assert!(std::ptr::eq(
            published_target,
            mapping(&root_context).resolved_target().unwrap(),
        ));
        tx.compute(&HostRootRepositoryMappingKey::new(workspace.clone()))
            .await
            .unwrap();
        tracker.root_mapping.lock().unwrap().clear();
        tracker.canonical.lock().unwrap().clear();
        tracker.apparent.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let root_builtin_key = HostCanonicalRepositoryApparentMappingKey::new(
            workspace.clone(),
            CanonicalRepoName::root(),
            ApparentRepoName::new("bazel_tools").unwrap(),
        );
        let root_builtin = tx.compute(&root_builtin_key).await.unwrap();
        assert_eq!(target(&root_builtin), CanonicalRepoName::root());
        assert_eq!(
            *tracker.root_mapping.lock().unwrap(),
            [(ActivationKind::Reused, false)]
        );
        assert!(tracker.canonical.lock().unwrap().is_empty());
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [(ActivationKind::Evaluated, false)]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let root_builtin_warm = tx.compute(&root_builtin_key).await.unwrap();
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_builtin,
            &root_builtin_warm,
        ));
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert_eq!(tracker.root_mapping.lock().unwrap().len(), 1);
        assert!(tracker.canonical.lock().unwrap().is_empty());
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        async fn resolve(
            dice: &Arc<Dice>,
            module: &str,
            apparent: &str,
        ) -> HostCanonicalRepositoryApparentMappingOutcome {
            let mut tx = transaction(dice, module, EXTENSION_A, true, None).await;
            let context = names(&validated(&mut tx).await).remove(0);
            tx.compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                context,
                ApparentRepoName::new(apparent).unwrap(),
            ))
            .await
            .unwrap()
        }
        async fn resolve_root(
            dice: &Arc<Dice>,
            module: &str,
            apparent: &str,
        ) -> HostCanonicalRepositoryApparentMappingOutcome {
            transaction(dice, module, EXTENSION_A, true, None)
                .await
                .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                    CanonicalRepoName::root(),
                    ApparentRepoName::new(apparent).unwrap(),
                ))
                .await
                .unwrap()
        }
        let override_module = format!("{MODULE}override_repo(e, first='bazel_tools')\n");
        let overridden = resolve(&dice, &override_module, "first").await;
        assert_eq!(target(&overridden), CanonicalRepoName::root());
        let SourcePreparationOutcome::Complete(overridden_value) = &overridden else {
            panic!("override mapping must complete")
        };
        let ApparentMappingPredecessor::Canonical(overridden_predecessor) =
            &overridden_value.as_ref().as_ref().unwrap().predecessor
        else {
            panic!("overridden mapping must retain canonical predecessor")
        };
        let HostCanonicalRepositoryDefinitionSource::Generated(overridden_certificate) =
            &overridden_predecessor.source
        else {
            panic!("overridden generated definition must stay generated")
        };
        let overridden_view = overridden_certificate.view().unwrap();
        assert_eq!(overridden_view.repo_spec.rule_id.rule_name.as_str(), "repo");
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &overridden,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &resolve(&dice, MODULE, "first").await,
        ));
        let root_a = resolve_root(&dice, MODULE, "first").await;
        let root_b = resolve_root(&dice, &override_module, "first").await;
        assert_eq!(target(&root_a), generated[0]);
        assert_eq!(target(&root_b), CanonicalRepoName::root());
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a, &root_b,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let order_module = "module(name='bazel_tools')\n\
            e=use_extension('//:ext.bzl','ext')\n\
            use_repo(e, second='second', first='first')\n";
        let root_order = resolve_root(&dice, order_module, "first").await;
        assert_eq!(target(&root_order), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &root_order,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let named_root_module = MODULE.replacen(
            "module(name='bazel_tools')",
            "module(name='bazel_tools', repo_name='root_self')",
            1,
        );
        let named_root = resolve_root(&dice, &named_root_module, "first").await;
        assert_eq!(target(&named_root), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &named_root,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let alternate_extension = EXTENSION_A.replace(
            "ext=module_extension(implementation=impl)",
            "ext=module_extension(implementation=impl)\nother=module_extension(implementation=impl)",
        );
        let alternate_module = "module(name='bazel_tools')\n\
            e=use_extension('//:ext.bzl','other')\n\
            use_repo(e, first='first', second='second')\n";
        let alternate_root = transaction(&dice, alternate_module, &alternate_extension, true, None)
            .await
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("first").unwrap(),
            ))
            .await
            .unwrap();
        assert_ne!(target(&alternate_root), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &alternate_root,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let mut invalid_tx = transaction(
            &dice,
            "this is not valid Starlark\n",
            EXTENSION_A,
            true,
            None,
        )
        .await;
        let direct_error = invalid_tx
            .compute(&HostRootRepositoryMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(direct_error) = direct_error else {
            panic!("invalid root mapping must complete")
        };
        let direct_error = direct_error.as_ref().as_ref().unwrap_err().clone();
        let root_terminal = invalid_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("first").unwrap(),
            ))
            .await
            .unwrap();
        assert!(matches!(
            &root_terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        context_repo,
                        apparent_repo,
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::RootMapping(error),
                        ..
                    }) if context_repo.is_root()
                        && apparent_repo.as_str() == "first"
                        && error == &direct_error
                )
        ));
        let root_injected = resolve_root(
            &dice,
            &format!("{MODULE}inject_repo(e, injected='bazel_tools')\n"),
            "first",
        )
        .await;
        assert_eq!(target(&root_injected), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &root_injected,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &self_mapping,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let root_missing = resolve_root(&dice, MODULE, "missing").await;
        assert!(matches!(
            root_missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Missing {
                            predecessor: ApparentMappingPredecessor::Root(_),
                        },
                        ..
                    })
                )
        ));
        let injected = resolve(
            &dice,
            &format!("{MODULE}inject_repo(e, injected='bazel_tools')\n"),
            "injected",
        )
        .await;
        assert_eq!(target(&injected), CanonicalRepoName::root());
        let injected_context = {
            let SourcePreparationOutcome::Complete(value) = &injected else {
                panic!("injected mapping must complete")
            };
            let ApparentMappingPredecessor::Canonical(predecessor) =
                &value.as_ref().as_ref().unwrap().predecessor
            else {
                panic!("injected mapping must retain canonical predecessor")
            };
            predecessor.view().unwrap().canonical_repo().clone()
        };
        let invalid_override_module = format!("{MODULE}override_repo(e, injected='bazel_tools')\n");
        let mut invalid_override_tx =
            transaction(&dice, &invalid_override_module, EXTENSION_A, true, None).await;
        let invalid_override = invalid_override_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                injected_context,
                ApparentRepoName::new("injected").unwrap(),
            ))
            .await
            .unwrap();
        assert!(matches!(
            &invalid_override,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Definition(_),
                        ..
                    })
                )
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &injected,
            &invalid_override,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &injected,
            &resolve(
                &dice,
                &format!("{MODULE}inject_repo(e, injected='bazel_tools')\n"),
                "injected",
            )
            .await,
        ));
        let inject_a = format!(
            "{MODULE}inject_repo(e, injected='bazel_tools')\ninject_repo(e, other='bazel_tools')\n"
        );
        let inject_b = format!(
            "{MODULE}inject_repo(e, other='bazel_tools')\ninject_repo(e, injected='bazel_tools')\n"
        );
        let order_a = resolve(&dice, &inject_a, "injected").await;
        let order_b = resolve(&dice, &inject_b, "injected").await;
        assert_eq!(target(&order_a), target(&order_b));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &order_a, &order_b,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &order_a,
            &resolve(&dice, &inject_a, "injected").await,
        ));
        let multi_extension = EXTENSION_A.replace(
            "ext=module_extension(implementation=impl)",
            "first=module_extension(implementation=impl)\nsecond=module_extension(implementation=impl)",
        );
        let multi_module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first_a='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, first_b='first')\n";
        let mut multi_tx = transaction(&dice, multi_module, &multi_extension, true, None).await;
        let contexts = names(&validated(&mut multi_tx).await);
        let first_context = HostCanonicalRepositoryApparentMappingKey::new(
            workspace.clone(),
            contexts[0].clone(),
            ApparentRepoName::new("first").unwrap(),
        );
        let second_context = HostCanonicalRepositoryApparentMappingKey::new(
            workspace,
            contexts[2].clone(),
            ApparentRepoName::new("first").unwrap(),
        );
        let isolated_a = multi_tx.compute(&first_context).await.unwrap();
        let isolated_b = multi_tx.compute(&second_context).await.unwrap();
        assert_eq!(target(&isolated_a), contexts[0]);
        assert_eq!(target(&isolated_b), contexts[2]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &isolated_a,
            &isolated_b,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &isolated_a,
            &multi_tx.compute(&first_context).await.unwrap(),
        ));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_generated_definition_identity_scan_and_terminal_algebra() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let requested = CanonicalRepoName::new("generated").unwrap();
        let key = HostGeneratedRepositoryDefinitionObservationKey::new(NormalizedAbsolutePath::new("/workspace").unwrap(), requested.clone());
        let same = HostGeneratedRepositoryDefinitionObservationKey::new(NormalizedAbsolutePath::new("/workspace").unwrap(), requested.clone());
        let other = HostGeneratedRepositoryDefinitionObservationKey::new(NormalizedAbsolutePath::new("/workspace").unwrap(), CanonicalRepoName::new("other").unwrap());
        let hash = |key: &HostGeneratedRepositoryDefinitionObservationKey| {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(key.to_string(), "observed-host-generated-repository-definition:\"/workspace\":@@generated");
        assert_eq!(key, same);
        assert_ne!(key, other);
        assert_eq!(hash(&key), hash(&same));
        assert_ne!(hash(&key), hash(&other));

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut initial = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let generated = names(&validated(&mut initial).await);
        let success = observed_lookup(&dice, MODULE, EXTENSION_A, true, generated[0].clone(), None).await;
        let carrier = observed_carrier(&success);
        assert!(carrier.result().is_ok());
        assert!(!carrier.observations().observations().is_empty());
        assert!(HostGeneratedRepositoryDefinitionObservationKey::validity(&success));
        assert!(HostGeneratedRepositoryDefinitionObservationKey::equality(&success, &success));

        let missing_name = CanonicalRepoName::new("missing").unwrap();
        let missing = observed_lookup(&dice, MODULE, EXTENSION_A, true, missing_name.clone(), None).await;
        assert!(matches!(
            observed_carrier(&missing).result().as_ref(),
            Err(HostGeneratedRepositoryDefinitionError {
                requested,
                kind: HostGeneratedRepositoryDefinitionErrorKind::Missing { certificate },
            }) if requested == &missing_name && certificate.iter().len() == 2
        ));
        let loading = observed_lookup(&dice, MODULE, EXTENSION_A, false, generated[0].clone(), None).await;
        assert!(matches!(
            observed_carrier(&loading).result().as_ref(),
            Err(HostGeneratedRepositoryDefinitionError {
                kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(_),
                ..
            })
        ));

        let certificate = carrier.result().as_ref().as_ref().unwrap().certificate.clone();
        let duplicate = complete_generated_driver(
            Err(HostGeneratedRepositoryDefinitionError {
                requested: generated[0].clone(),
                kind: HostGeneratedRepositoryDefinitionErrorKind::Duplicate {
                    certificate: certificate.clone(),
                    first: 0,
                    conflicting: 2,
                },
            }),
            carrier.observations().clone(),
        );
        assert!(matches!(
            duplicate,
            SourcePreparationOutcome::Complete(Ok((value, observations)))
                if matches!(value.as_ref(), Err(HostGeneratedRepositoryDefinitionError {
                    kind: HostGeneratedRepositoryDefinitionErrorKind::Duplicate { first: 0, conflicting: 2, .. },
                    ..
                })) && observations == *carrier.observations()
        ));
        let compute = complete_generated_driver(
            Err(HostGeneratedRepositoryDefinitionError {
                requested: generated[0].clone(),
                kind: HostGeneratedRepositoryDefinitionErrorKind::LoadingCompute("failed".into()),
            }),
            PathObservationEpoch::empty(),
        );
        assert!(matches!(compute, SourcePreparationOutcome::Complete(Ok((value, observations)))
            if matches!(value.as_ref(), Err(HostGeneratedRepositoryDefinitionError {
                kind: HostGeneratedRepositoryDefinitionErrorKind::LoadingCompute(message), ..
            }) if message.as_ref() == "failed") && observations.observations().is_empty()));

        let consumed = Cell::new(0);
        let unrelated = CanonicalRepoName::new("unrelated").unwrap();
        let scan = [&generated[0], &unrelated, &generated[0], &unrelated];
        assert_eq!(
            find_unique_ordinal(&generated[0], scan.into_iter().inspect(|_| consumed.set(consumed.get() + 1)),),
            Err(UniqueOrdinalError::Duplicate { first: 0, conflicting: 2 })
        );
        assert_eq!(consumed.get(), scan.len());

        let tracker = Arc::new(LookupTracker::default());
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        });
        updater.changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())]).unwrap();
        let observed_key = HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), generated[0].clone());
        let need = updater.commit().await.compute(&observed_key).await.unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostGeneratedRepositoryDefinitionObservationKey::validity(&need));
        assert!(!HostGeneratedRepositoryDefinitionObservationKey::equality(&need, &need));
        assert_eq!(
            tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &observed_key.to_string()).unwrap().1,
            [HostValidatedModuleExtensionRepositoriesObservationKey::new(workspace).to_string()]
        );

        let source = include_str!("generated_repository_definition.rs");
        let producer = &source[source.find("type GeneratedRepositoryDefinitionResult").unwrap()..source.find("enum HostCanonicalRepositoryDefinitionSource").unwrap()];
        assert_eq!(producer.matches("HostValidatedModuleExtensionRepositoriesObservationKey::new").count(), 1);
        assert!(producer.contains("HostGeneratedRepositoryDefinitionObservationError::Validation(error)"));
        assert!(!producer.contains("HostCanonicalSelectedModuleDefinitionKey"));
        assert!(!producer.contains("union_"));
        assert!(!producer.contains("store_evaluation_data"));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_generated_definition_real_order_events_and_parity() {
        let extension = r#"print('load')
repo=repository_rule(implementation=lambda ctx: None)
def first_impl(ctx):
    print('invoke-first')
    repo(name='first')
def second_impl(ctx):
    print('invoke-second')
    repo(name='second')
first=module_extension(implementation=first_impl)
second=module_extension(implementation=second_impl)
"#;
        let module =
            "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\n";
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, module, extension, true, None).await;
        let requested = names(&validated(&mut prep_tx).await).remove(0);
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, module, extension, true, Some(tracker.clone())).await;
        let observed_key = HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), requested.clone());
        let observed = tx.compute(&observed_key).await.unwrap();
        let carrier = observed_carrier(&observed);
        let legacy_key = HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), requested.clone());
        assert_eq!(
            tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &observed_key.to_string()).unwrap().1,
            [HostValidatedModuleExtensionRepositoriesObservationKey::new(workspace.clone()).to_string()]
        );
        assert!(tracker.selected.lock().unwrap().is_empty());
        let activations = tracker.activations.lock().unwrap();
        let parent = activations.iter().find(|(name, _, _)| name == &observed_key.to_string()).unwrap();
        assert_eq!(parent.1, ActivationKind::Evaluated);
        assert!(parent.2.is_none());
        let prints = activations
            .iter()
            .filter_map(|(_, _, batch)| batch.as_ref())
            .flat_map(EventBatch::events)
            .filter_map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(prints, ["load", "invoke-first", "invoke-second"], "activations: {activations:#?}");
        assert!(
            activations
                .iter()
                .filter(|(name, _, _)| {
                    name.contains("instantiated-module-extension-repositories:") || name.contains("validated-module-extension-repositories:") || name == &observed_key.to_string()
                })
                .all(|(_, _, batch)| batch.is_none())
        );
        drop(activations);

        let legacy = tx.compute(&legacy_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("legacy generated definition must complete")
        };
        assert_eq!(legacy.as_ref(), carrier.result().as_ref());
        let snapshot = snapshot(&SourcePreparationOutcome::Complete(legacy.clone()));
        assert_eq!(snapshot[0], requested.as_str());
        assert_eq!(&snapshot[1..3], ["first", "repo"]);
        assert_eq!(snapshot[4], requested.as_str());

        tracker.activations.lock().unwrap().clear();
        let warm = tx.compute(&observed_key).await.unwrap();
        assert!(HostGeneratedRepositoryDefinitionObservationKey::equality(&observed, &warm));
        assert!(Arc::ptr_eq(carrier.result(), observed_carrier(&warm).result()));
        assert!(
            tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .any(|(name, kind, batch)| { name == &observed_key.to_string() && *kind == ActivationKind::Reused && batch.is_none() })
        );
        assert!(tracker.activations.lock().unwrap().iter().all(|(_, _, batch)| batch.is_none()));

        for (present, requested_case, expected) in [(false, requested.clone(), "Loading"), (true, CanonicalRepoName::new("missing").unwrap(), "Missing")] {
            let case_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let observed = observed_lookup(&case_dice, module, extension, present, requested_case.clone(), None).await;
            let mut legacy_tx = transaction(&case_dice, module, extension, present, None).await;
            let legacy = legacy_tx
                .compute(&HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), requested_case))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(legacy) = legacy else {
                panic!("legacy terminal must complete")
            };
            assert_eq!(legacy.as_ref(), observed_carrier(&observed).result().as_ref());
            assert!(format!("{:?}", observed_carrier(&observed).result()).contains(expected));
        }

        let terminal_tracker = Arc::new(LookupTracker::default());
        let _ = observed_lookup(
            &Dice::builder().build(DetectCycles::Enabled),
            module,
            extension,
            false,
            requested,
            Some(terminal_tracker.clone()),
        )
        .await;
        assert!(
            terminal_tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .filter_map(|(_, _, batch)| batch.as_ref())
                .flat_map(EventBatch::events)
                .all(|event| !matches!(event, EvaluationEvent::StarlarkPrint { .. }))
        );
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_generated_definition_lifecycle_cancellation_and_nonactivation() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, MODULE, EXTENSION_A, true, None).await;
        let requested = names(&validated(&mut prep_tx).await).remove(0);
        let order = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, second='second', first='first')\n";
        let mapping = format!("{MODULE}override_repo(e, first='bazel_tools')\n");
        let same_semantic = format!("{MODULE}\n");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), requested.clone());
        let mut held = Vec::new();
        for (module, extension) in [
            (MODULE, EXTENSION_A),
            (MODULE, EXTENSION_B),
            (MODULE, EXTENSION_A),
            (order, EXTENSION_A),
            (MODULE, EXTENSION_A),
            (mapping.as_str(), EXTENSION_A),
            (MODULE, EXTENSION_A),
            (same_semantic.as_str(), EXTENSION_A),
        ] {
            let mut tx = transaction(&dice, module, extension, true, None).await;
            let global = tx.compute(&PathObservationEpochKey).await.unwrap();
            let carrier = observed_carrier(&tx.compute(&key).await.unwrap()).clone();
            let child = tx.compute(&HostValidatedModuleExtensionRepositoriesObservationKey::new(workspace.clone())).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child else {
                panic!("observed validation child must complete")
            };
            assert_eq!(carrier.observations(), child.observations());
            for (demand, result) in carrier.observations().observations() {
                assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
            }
            held.push(carrier);
        }
        for (a, b, restored) in [(0, 1, 2), (2, 3, 4), (4, 5, 6)] {
            assert_ne!(held[a].result(), held[b].result());
            assert_eq!(held[a].result(), held[restored].result());
        }
        assert_eq!(held[0].result(), held[7].result());
        assert_ne!(held[0].observations(), held[7].observations());

        let warm_tracker = Arc::new(LookupTracker::default());
        let mut warm_tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(warm_tracker.clone())).await;
        let first = observed_carrier(&warm_tx.compute(&key).await.unwrap()).clone();
        warm_tracker.activations.lock().unwrap().clear();
        let reused = observed_carrier(&warm_tx.compute(&key).await.unwrap()).clone();
        assert!(Arc::ptr_eq(first.result(), reused.result()));
        assert!(
            warm_tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .any(|(name, kind, batch)| { name == &key.to_string() && *kind == ActivationKind::Reused && batch.is_none() })
        );

        let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let cancel_tracker = Arc::new(LookupTracker::default());
        let mut cancelled = transaction(&cancel_dice, MODULE, EXTENSION_A, true, Some(cancel_tracker.clone())).await;
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(cancel_tracker.activations.lock().unwrap().iter().all(|(name, _, _)| name != &key.to_string()));
        assert!(cancel_tracker.dependencies.lock().unwrap().iter().all(|(name, _)| name != &key.to_string()));

        let mut recovery = transaction(&cancel_dice, MODULE, EXTENSION_A, true, Some(cancel_tracker.clone())).await;
        let global = recovery.compute(&PathObservationEpochKey).await.unwrap();
        let recovered = observed_carrier(&recovery.compute(&key).await.unwrap()).clone();
        assert_eq!(recovered.result(), held[0].result());
        for (demand, result) in recovered.observations().observations() {
            assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
        }
        let activations = cancel_tracker.activations.lock().unwrap();
        let dependencies = cancel_tracker.dependencies.lock().unwrap();
        let forbidden_captures = cancel_tracker.forbidden.lock().unwrap();
        assert!(!forbidden_captures.is_empty());
        assert!(forbidden_captures.iter().all(|capture| *capture == "filesystem"));
        let legacy = HostGeneratedRepositoryDefinitionKey::new(workspace, requested).to_string();
        assert!(activations.iter().all(|(name, _, _)| name != &legacy));
        assert!(dependencies.iter().all(|(name, children)| name != &legacy && children.iter().all(|child| child != &legacy)));
        for forbidden in [
            "host-canonical-selected-module-definition:",
            "host-canonical-repository-definition:",
            "host-canonical-repository-apparent-mapping:",
            "host-root-repository-mapping:",
            "host-root-apparent-repository-definition:",
            "HostRootApparentRepositoryRouteKey",
            "HostRootApparentRepositorySourceInputKey",
            "HostRootApparentRepositorySourceObservationKey",
            "HostRootApparentRepositorySourcePathInputKey",
            "root-repository-route:",
            "repository-package-source:",
            "repository-source-file:",
            "host-repository-source-file:",
            "build-command-root:",
        ] {
            assert!(activations.iter().all(|(name, _, _)| !name.contains(forbidden)));
            assert!(
                dependencies
                    .iter()
                    .all(|(name, children)| !name.contains(forbidden) && children.iter().all(|child| !child.contains(forbidden)))
            );
        }
        let source = include_str!("generated_repository_definition.rs");
        let producer = &source[source.find("type GeneratedRepositoryDefinitionResult").unwrap()..source.find("enum HostCanonicalRepositoryDefinitionSource").unwrap()];
        assert!(!producer.contains("RootModuleBootstrap"));
        assert!(!producer.contains("HostCanonicalRepositoryDefinitionKey::new"));
        assert!(!producer.contains("HostCanonicalRepositoryApparentMappingKey::new"));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_definition_identity_staging_and_terminal_algebra() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let requested = CanonicalRepoName::new("requested").unwrap();
        let key = HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), requested.clone());
        let same = HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), requested.clone());
        let other = HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), CanonicalRepoName::new("other").unwrap());
        let hash = |key: &HostCanonicalRepositoryDefinitionObservationKey| { let mut hasher = DefaultHasher::new(); key.hash(&mut hasher); hasher.finish() };
        assert_eq!(key.to_string(), format!("observed-host-canonical-repository-definition:{workspace}:{requested}"));
        assert_eq!(key, same); assert_ne!(key, other); assert_eq!(hash(&key), hash(&same)); assert_ne!(hash(&key), hash(&other));

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let root = observed_canonical_lookup(&dice, MODULE, EXTENSION_A, true, CanonicalRepoName::root(), None).await;
        let root_carrier = observed_canonical_carrier(&root);
        assert!(matches!(root_carrier.result().as_ref(), Ok(HostCanonicalRepositoryDefinition { source: HostCanonicalRepositoryDefinitionSource::Selected(_) })));
        assert!(!root_carrier.observations().observations().is_empty());
        assert!(HostCanonicalRepositoryDefinitionObservationKey::validity(&root));
        assert!(HostCanonicalRepositoryDefinitionObservationKey::equality(&root, &root));

        let mut prep = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let generated_name = names(&validated(&mut prep).await).remove(0);
        let generated = observed_canonical_lookup(&dice, MODULE, EXTENSION_A, true, generated_name.clone(), None).await;
        assert!(matches!(observed_canonical_carrier(&generated).result().as_ref(), Ok(HostCanonicalRepositoryDefinition { source: HostCanonicalRepositoryDefinitionSource::Generated(_) })));
        let missing_name = CanonicalRepoName::new("missing").unwrap();
        let missing = observed_canonical_lookup(&dice, MODULE, EXTENSION_A, true, missing_name.clone(), None).await;
        assert!(matches!(observed_canonical_carrier(&missing).result().as_ref(), Err(HostCanonicalRepositoryDefinitionError { canonical_repo, kind: HostCanonicalRepositoryDefinitionErrorKind::Missing { selected_missing, generated_missing } }) if canonical_repo == &missing_name && selected_missing.disposition() == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing && matches!(generated_missing.kind, HostGeneratedRepositoryDefinitionErrorKind::Missing { .. })));
        let loading = observed_canonical_lookup(&dice, MODULE, EXTENSION_A, false, generated_name, None).await;
        assert!(matches!(observed_canonical_carrier(&loading).result().as_ref(), Err(HostCanonicalRepositoryDefinitionError { kind: HostCanonicalRepositoryDefinitionErrorKind::Generated { selected_missing, error: HostGeneratedRepositoryDefinitionError { kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(_), .. } }, .. }) if selected_missing.disposition() == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing));
        let selected_terminal = observed_canonical_lookup(&dice, "module(name='root')\nbazel_dep(name='missing', version='1')\n", EXTENSION_A, true, CanonicalRepoName::root(), None).await;
        assert!(matches!(observed_canonical_carrier(&selected_terminal).result().as_ref(), Err(HostCanonicalRepositoryDefinitionError { kind: HostCanonicalRepositoryDefinitionErrorKind::Selected(error), .. }) if error.disposition() == HostCanonicalSelectedModuleDefinitionErrorDisposition::Terminal));

        let mut selected_tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let selected_key = HostCanonicalSelectedModuleDefinitionObservationKey::new(workspace.clone(), missing_name.clone());
        let selected_value = selected_tx.compute(&selected_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(selected_carrier)) = &selected_value else { panic!("selected Missing carrier must complete") };
        let selected_missing = selected_carrier.result().as_ref().as_ref().unwrap_err().clone();
        let selected_epoch = selected_carrier.observations().clone();
        assert_eq!(selected_missing.disposition(), HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing);

        let selected_need_tracker = Arc::new(LookupTracker::default());
        let mut updater = dice.updater_with_data(UserComputationData { cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()), activation_tracker: Some(selected_need_tracker.clone()), ..Default::default() });
        updater.changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())]).unwrap();
        let selected_need = updater.commit().await.compute(&key).await.unwrap();
        assert!(matches!(selected_need, SourcePreparationOutcome::Need(_)));
        assert_eq!(selected_need_tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &key.to_string()).unwrap().1, [HostCanonicalSelectedModuleDefinitionObservationKey::new(workspace.clone(), requested.clone()).to_string()]);

        let generated_need_tracker = Arc::new(LookupTracker::default());
        let mut updater = dice.updater_with_data(UserComputationData { cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()), activation_tracker: Some(generated_need_tracker.clone()), ..Default::default() });
        updater.changed_to(vec![(PathObservationEpochKey, selected_epoch.clone())]).unwrap();
        let generated_need_key = HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), missing_name.clone());
        let generated_need = updater.commit().await.compute(&generated_need_key).await.unwrap();
        assert!(matches!(generated_need, SourcePreparationOutcome::Need(_)));
        assert!(!HostCanonicalRepositoryDefinitionObservationKey::validity(&generated_need));
        assert!(!HostCanonicalRepositoryDefinitionObservationKey::equality(&generated_need, &generated_need));
        assert_eq!(generated_need_tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &generated_need_key.to_string()).unwrap().1, [selected_key.to_string(), HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), missing_name.clone()).to_string()]);

        let selected_compute = complete_canonical_driver(Err(HostCanonicalRepositoryDefinitionError { canonical_repo: requested.clone(), kind: HostCanonicalRepositoryDefinitionErrorKind::SelectedCompute("selected-dice".into()) }), PathObservationEpoch::empty());
        assert!(matches!(selected_compute, SourcePreparationOutcome::Complete(Ok((result, observations))) if matches!(result.as_ref(), Err(HostCanonicalRepositoryDefinitionError { canonical_repo, kind: HostCanonicalRepositoryDefinitionErrorKind::SelectedCompute(message) }) if canonical_repo == &requested && message.as_ref() == "selected-dice") && observations.observations().is_empty()));
        let generated_compute = complete_canonical_driver(Err(HostCanonicalRepositoryDefinitionError { canonical_repo: missing_name.clone(), kind: HostCanonicalRepositoryDefinitionErrorKind::GeneratedCompute { selected_missing: selected_missing.clone(), message: "generated-dice".into() } }), selected_epoch.clone());
        assert!(matches!(generated_compute, SourcePreparationOutcome::Complete(Ok((result, observations))) if matches!(result.as_ref(), Err(HostCanonicalRepositoryDefinitionError { canonical_repo, kind: HostCanonicalRepositoryDefinitionErrorKind::GeneratedCompute { selected_missing: prefix, message } }) if canonical_repo == &missing_name && prefix == &selected_missing && message.as_ref() == "generated-dice") && observations == selected_epoch));

        let demand = PathObservationDemand::new(PathObservationNamespace::Host, NormalizedAbsolutePath::new("/merge").unwrap(), PathObservationOperation::Lstat);
        let first = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let left = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let equal = PathObservationEpoch::from_shared([(demand.dupe(), Arc::new(first.as_ref().clone()))]).unwrap();
        let merged = merge_canonical_observations(&left, &equal).unwrap();
        assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first));
        let conflict = PathObservationEpoch::from_shared([(demand.dupe(), Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(PathNodeKind::RegularFile, 1, 1, 1, 1, 0o644))))) ]).unwrap();
        assert!(matches!(merge_canonical_observations(&left, &conflict), Err(ObservedPathFrontierError::Epoch(slug_workspace_v2::PathObservationEpochError::ConflictingDemand(found))) if found == demand));

        let source = include_str!("generated_repository_definition.rs");
        let producer = &source[source.find("type CanonicalRepositoryDefinitionResult").unwrap()..source.find("pub(super) struct HostCanonicalRepositoryApparentMapping").unwrap()];
        assert_eq!(producer.matches("HostCanonicalSelectedModuleDefinitionObservationKey::new").count(), 1);
        assert_eq!(producer.matches("HostGeneratedRepositoryDefinitionObservationKey::new").count(), 1);
        assert!(producer.find("HostCanonicalSelectedModuleDefinitionObservationKey::new").unwrap() < producer.find("HostGeneratedRepositoryDefinitionObservationKey::new").unwrap());
        assert!(producer.contains("HostCanonicalRepositoryDefinitionObservationError::Selected(error)"));
        assert!(producer.contains("HostCanonicalRepositoryDefinitionObservationError::Generated { selected_missing, error }"));
        assert!(producer.contains("HostCanonicalRepositoryDefinitionObservationError::Merge { selected_missing, error }"));
        assert!(producer.contains("Err(error) => return complete_canonical_driver(Err(HostCanonicalRepositoryDefinitionError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryDefinitionErrorKind::SelectedCompute"));
        assert!(producer.contains("Err(error) => return complete_canonical_driver(Err(HostCanonicalRepositoryDefinitionError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryDefinitionErrorKind::GeneratedCompute { selected_missing"));
        assert!(producer.contains("return SourcePreparationOutcome::Complete(Err(HostCanonicalRepositoryDefinitionObservationError::Selected(error)))"));
        assert!(producer.contains("return SourcePreparationOutcome::Complete(Err(HostCanonicalRepositoryDefinitionObservationError::Generated { selected_missing, error }))"));
        assert!(!producer.contains("store_evaluation_data")); assert!(!producer.contains("union_")); assert!(!producer.contains("HostCanonicalRepositoryApparentMappingKey::new"));
        let selected_source = include_str!("../../../slug_bzlmod_v2/src/selected_repo_spec.rs");
        let selected_proof = &selected_source[selected_source.find("observed_canonical_selected_definition_identity_scan_and_terminal_algebra").unwrap()..selected_source.find("observed_canonical_selected_definition_real_order_events_and_parity").unwrap()];
        assert!(selected_proof.contains("PathObservationEpochError::OperationMismatch"));
        assert!(selected_proof.contains("RepoSpecChild::Outer(HostSelectedModuleRoutesObservationError::Graph"));
        assert!(selected_proof.contains("observed route outer must remain carrierless"));
        let pure_source = include_str!("../../../slug_loading_v2/src/module_extension.rs");
        let pure_proof = &pure_source[pure_source.find("observed_pure_identity_finisher_and_prefix_algebra").unwrap()..];
        assert!(pure_proof.contains("PathObservationEpochError::OperationMismatch"));
        assert!(pure_proof.contains("assert_observed_pure_outer_stages(&prepared, lower_error, merge_error)"));
        let instantiation_source = include_str!("../../../slug_loading_v2/src/module_extension_repository_instantiation.rs");
        let validation_source = include_str!("../../../slug_loading_v2/src/module_extension_repository_validation.rs");
        assert!(instantiation_source.contains("Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(InstantiatedModuleExtensionRepositoriesObservationError::Pure(error)))"));
        assert!(validation_source.contains("Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(ValidatedModuleExtensionRepositoriesObservationError::Instantiation(error)))"));
        assert!(source.contains("Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(HostGeneratedRepositoryDefinitionObservationError::Validation(error)))"));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_definition_real_order_events_and_parity() {
        let extension = r#"print('load')
repo=repository_rule(implementation=lambda ctx: None)
def first_impl(ctx):
    print('invoke-first')
    repo(name='first')
def second_impl(ctx):
    print('invoke-second')
    repo(name='second')
first=module_extension(implementation=first_impl)
second=module_extension(implementation=second_impl)
"#;
        let module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\n";
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, module, extension, true, None).await;
        let generated = names(&validated(&mut prep_tx).await).remove(0);
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let selected_failure = "module(name='root')\nbazel_dep(name='missing', version='1')\n";
        let empty: &[&str] = &[];
        let generated_prints = ["load", "invoke-first", "invoke-second"];
        for (case, case_module, case_extension, present, requested, generated_stage, expected_prints) in [
            ("selected-success", MODULE, EXTENSION_A, true, CanonicalRepoName::root(), false, empty),
            ("selected-failure", selected_failure, EXTENSION_A, true, CanonicalRepoName::root(), false, empty),
            ("generated-success", module, extension, true, generated.clone(), true, generated_prints.as_slice()),
            ("generated-failure", module, extension, false, generated.clone(), true, empty),
            ("generated-missing", module, extension, true, CanonicalRepoName::new("missing").unwrap(), true, generated_prints.as_slice()),
        ] {
            let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let tracker = Arc::new(LookupTracker::default());
            let mut tx = transaction(&dice, case_module, case_extension, present, Some(tracker.clone())).await;
            let key = HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), requested.clone());
            let observed = tx.compute(&key).await.unwrap();
            let carrier = observed_canonical_carrier(&observed);
            match case {
                "selected-success" => assert!(matches!(carrier.result().as_ref(), Ok(HostCanonicalRepositoryDefinition { source: HostCanonicalRepositoryDefinitionSource::Selected(_) }))),
                "selected-failure" => assert!(matches!(carrier.result().as_ref(), Err(HostCanonicalRepositoryDefinitionError { kind: HostCanonicalRepositoryDefinitionErrorKind::Selected(_), .. }))),
                "generated-success" => assert!(matches!(carrier.result().as_ref(), Ok(HostCanonicalRepositoryDefinition { source: HostCanonicalRepositoryDefinitionSource::Generated(_) }))),
                "generated-failure" => assert!(matches!(carrier.result().as_ref(), Err(HostCanonicalRepositoryDefinitionError { kind: HostCanonicalRepositoryDefinitionErrorKind::Generated { .. }, .. }))),
                "generated-missing" => assert!(matches!(carrier.result().as_ref(), Err(HostCanonicalRepositoryDefinitionError { kind: HostCanonicalRepositoryDefinitionErrorKind::Missing { selected_missing, generated_missing }, .. }) if selected_missing.disposition() == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing && matches!(generated_missing.kind, HostGeneratedRepositoryDefinitionErrorKind::Missing { .. }))),
                _ => unreachable!(),
            }
            let mut expected_children = vec![HostCanonicalSelectedModuleDefinitionObservationKey::new(workspace.clone(), requested.clone()).to_string()];
            if generated_stage { expected_children.push(HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), requested.clone()).to_string()); }
            assert_eq!(tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &key.to_string()).unwrap().1, expected_children);
            let activations = tracker.activations.lock().unwrap();
            let parent = activations.iter().find(|(name, _, _)| name == &key.to_string()).unwrap();
            assert_eq!(parent.1, ActivationKind::Evaluated); assert!(parent.2.is_none());
            for child in &expected_children { assert!(activations.iter().any(|(name, _, batch)| name == child && batch.is_none()), "{case}: {child}"); }
            let prints = activations.iter().filter_map(|(_, _, batch)| batch.as_ref()).flat_map(EventBatch::events).filter_map(|event| match event { EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()), _ => None }).collect::<Vec<_>>();
            assert_eq!(prints, expected_prints, "{case}: {activations:#?}");
            drop(activations);
            let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let mut legacy_tx = transaction(&legacy_dice, case_module, case_extension, present, None).await;
            let SourcePreparationOutcome::Complete(legacy) = legacy_tx.compute(&HostCanonicalRepositoryDefinitionKey::new(workspace.clone(), requested)).await.unwrap() else { panic!("{case}: legacy must complete") };
            assert_eq!(legacy.as_ref(), carrier.result().as_ref(), "{case}");
            tracker.activations.lock().unwrap().clear();
            let warm = tx.compute(&key).await.unwrap();
            assert!(HostCanonicalRepositoryDefinitionObservationKey::equality(&observed, &warm));
            assert!(Arc::ptr_eq(carrier.result(), observed_canonical_carrier(&warm).result()));
            let warm_activations = tracker.activations.lock().unwrap();
            assert!(warm_activations.iter().any(|(name, kind, batch)| name == &key.to_string() && *kind == ActivationKind::Reused && batch.is_none()));
            assert!(warm_activations.iter().all(|(_, _, batch)| batch.is_none()));
            drop(warm_activations);
            for forbidden in ["host-canonical-selected-module-definition:", "host-generated-repository-definition:", "host-canonical-repository-definition:", "host-canonical-repository-apparent-mapping:", "host-root-repository-mapping:", "host-root-apparent-repository-definition:", "root-repository-route:", "repository-package-source:", "repository-source-file:", "host-repository-source-file:", "build-command-root:"] {
                assert!(tracker.activations.lock().unwrap().iter().all(|(name, _, _)| !name.starts_with(forbidden)), "{case}: {forbidden}");
                assert!(tracker.dependencies.lock().unwrap().iter().all(|(name, children)| !name.starts_with(forbidden) && children.iter().all(|child| !child.starts_with(forbidden))), "{case}: {forbidden}");
            }
        }
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_definition_lifecycle_cancellation_and_nonactivation() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, MODULE, EXTENSION_A, true, None).await;
        let generated = names(&validated(&mut prep_tx).await).remove(0);
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let selected_tracker = Arc::new(LookupTracker::default());
        let selected_parent_key = HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), CanonicalRepoName::root());
        let selected_child_key = HostCanonicalSelectedModuleDefinitionObservationKey::new(workspace.clone(), CanonicalRepoName::root());
        let selected_b = MODULE.replacen("bazel_tools", "root", 1);
        let mut selected_held = Vec::new();
        for module in [MODULE, selected_b.as_str(), MODULE] {
            let mut tx = transaction(&dice, module, EXTENSION_A, true, Some(selected_tracker.clone())).await;
            let global = tx.compute(&PathObservationEpochKey).await.unwrap();
            let parent = observed_canonical_carrier(&tx.compute(&selected_parent_key).await.unwrap()).clone();
            let child_value = tx.compute(&selected_child_key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child_value else { panic!("selected child must complete") };
            assert_eq!(parent.observations(), child.observations());
            for carrier_epoch in [parent.observations(), child.observations()] {
                for (demand, result) in carrier_epoch.observations() { assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref()); }
            }
            selected_held.push((parent, child));
        }
        assert_ne!(selected_held[0].0.result(), selected_held[1].0.result());
        assert_eq!(selected_held[0].0.result(), selected_held[2].0.result());
        assert_ne!(selected_held[0].1.result(), selected_held[1].1.result());
        assert_eq!(selected_held[0].1.result(), selected_held[2].1.result());

        let generated_tracker = Arc::new(LookupTracker::default());
        let generated_parent_key = HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), generated.clone());
        let generated_child_key = HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), generated.clone());
        let same_semantic = format!("{MODULE}\n");
        let mut generated_held = Vec::new();
        for (module, extension) in [(MODULE, EXTENSION_A), (MODULE, EXTENSION_B), (MODULE, EXTENSION_A), (same_semantic.as_str(), EXTENSION_A)] {
            let mut tx = transaction(&dice, module, extension, true, Some(generated_tracker.clone())).await;
            let global = tx.compute(&PathObservationEpochKey).await.unwrap();
            let parent = observed_canonical_carrier(&tx.compute(&generated_parent_key).await.unwrap()).clone();
            let child_value = tx.compute(&generated_child_key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child_value else { panic!("generated child must complete") };
            for carrier_epoch in [parent.observations(), child.observations()] {
                for (demand, result) in carrier_epoch.observations() { assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref()); }
            }
            generated_held.push((parent, child));
        }
        assert_ne!(generated_held[0].0.result(), generated_held[1].0.result());
        assert_eq!(generated_held[0].0.result(), generated_held[2].0.result());
        assert_ne!(generated_held[0].1.result(), generated_held[1].1.result());
        assert_eq!(generated_held[0].1.result(), generated_held[2].1.result());
        assert_eq!(generated_held[0].0.result(), generated_held[3].0.result());
        assert_ne!(generated_held[0].0.observations(), generated_held[3].0.observations());
        assert_eq!(generated_held[0].1.result(), generated_held[3].1.result());
        assert_ne!(generated_held[0].1.observations(), generated_held[3].1.observations());

        let selected_failure = "module(name='root')\nbazel_dep(name='missing', version='1')\n";
        let mut trackers = vec![selected_tracker, generated_tracker];
        for (case, module, requested, generated_stage) in [
            ("selected-terminal", selected_failure, CanonicalRepoName::root(), false),
            ("missing-fallback", MODULE, CanonicalRepoName::new("missing").unwrap(), true),
        ] {
            let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let tracker = Arc::new(LookupTracker::default());
            let mut cancelled = transaction(&cancel_dice, module, EXTENSION_A, true, Some(tracker.clone())).await;
            let key = HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), requested.clone());
            let mut future = Box::pin(cancelled.compute(&key));
            std::future::poll_fn(|context| { assert!(std::future::Future::poll(future.as_mut(), context).is_pending()); std::task::Poll::Ready(()) }).await;
            drop(future); drop(cancelled);
            assert!(tracker.activations.lock().unwrap().iter().all(|(name, _, _)| name != &key.to_string()));
            assert!(tracker.dependencies.lock().unwrap().iter().all(|(name, _)| name != &key.to_string()));

            let mut recovery = transaction(&cancel_dice, module, EXTENSION_A, true, Some(tracker.clone())).await;
            let global = recovery.compute(&PathObservationEpochKey).await.unwrap();
            let recovered_value = recovery.compute(&key).await.unwrap();
            let recovered = observed_canonical_carrier(&recovered_value);
            match case {
                "selected-terminal" => assert!(matches!(recovered.result().as_ref(), Err(HostCanonicalRepositoryDefinitionError { kind: HostCanonicalRepositoryDefinitionErrorKind::Selected(_), .. }))),
                "missing-fallback" => assert!(matches!(recovered.result().as_ref(), Err(HostCanonicalRepositoryDefinitionError { kind: HostCanonicalRepositoryDefinitionErrorKind::Missing { .. }, .. }))),
                _ => unreachable!(),
            }
            for (demand, result) in recovered.observations().observations() { assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref()); }
            let clean_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let clean = observed_canonical_lookup(&clean_dice, module, EXTENSION_A, true, requested.clone(), None).await;
            assert_eq!(recovered.result(), observed_canonical_carrier(&clean).result(), "{case}");
            let mut expected_children = vec![HostCanonicalSelectedModuleDefinitionObservationKey::new(workspace.clone(), requested.clone()).to_string()];
            if generated_stage { expected_children.push(HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), requested).to_string()); }
            assert_eq!(tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &key.to_string()).unwrap().1, expected_children);
            let activations = tracker.activations.lock().unwrap();
            assert!(activations.iter().any(|(name, kind, batch)| name == &key.to_string() && *kind == ActivationKind::Evaluated && batch.is_none()));
            for child in &expected_children { assert!(activations.iter().any(|(name, _, batch)| name == child && batch.is_none()), "{case}: {child}"); }
            drop(activations);
            trackers.push(tracker);
        }

        for tracker in trackers {
            let activations = tracker.activations.lock().unwrap();
            let dependencies = tracker.dependencies.lock().unwrap();
            for family in ["host-canonical-selected-module-definition:", "host-generated-repository-definition:", "host-canonical-repository-definition:"] {
                assert!(activations.iter().all(|(name, _, _)| !name.starts_with(family)));
                assert!(dependencies.iter().all(|(name, children)| !name.starts_with(family) && children.iter().all(|child| !child.starts_with(family))));
            }
            for upper in ["host-canonical-repository-apparent-mapping:", "host-root-repository-mapping:", "host-root-apparent-repository-definition:", "HostRootApparentRepositoryRouteKey", "HostRootApparentRepositorySourceInputKey", "HostRootApparentRepositorySourceObservationKey", "HostRootApparentRepositorySourcePathInputKey", "root-repository-route:", "repository-package-source:", "repository-source-file:", "host-repository-source-file:", "build-command-root:"] {
                assert!(activations.iter().all(|(name, _, _)| !name.contains(upper)));
                assert!(dependencies.iter().all(|(name, children)| !name.contains(upper) && children.iter().all(|child| !child.contains(upper))));
            }
        }
        let source = include_str!("generated_repository_definition.rs");
        let producer = &source[source.find("type CanonicalRepositoryDefinitionResult").unwrap()..source.find("pub(super) struct HostCanonicalRepositoryApparentMapping").unwrap()];
        assert!(!producer.contains("HostCanonicalRepositoryDefinitionKey::new"));
        assert!(!producer.contains("HostCanonicalRepositoryApparentMappingKey::new"));
        assert!(!producer.contains("HostRootRepositoryMappingKey"));
        assert!(!producer.contains("RootModuleBootstrap"));
        assert!(!producer.contains("CaptureEvaluationEvents"));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_apparent_mapping_identity_branch_and_terminal_algebra() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let root = CanonicalRepoName::root();
        let first = ApparentRepoName::new("first").unwrap();
        let key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), root.clone(), first.clone());
        let same = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), root.clone(), first.clone());
        let other = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), root.clone(), ApparentRepoName::new("second").unwrap());
        let hash = |value: &HostCanonicalRepositoryApparentMappingObservationKey| { let mut state = DefaultHasher::new(); value.hash(&mut state); state.finish() };
        assert_eq!(key, same); assert_ne!(key, other); assert_eq!(hash(&key), hash(&same)); assert_ne!(hash(&key), hash(&other));
        assert_eq!(key.to_string(), "observed-host-canonical-repository-apparent-mapping:\"/generated-repository-definition\":@@:@first");

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let generated = names(&validated(&mut tx).await).remove(0);
        tracker.dependencies.lock().unwrap().clear();
        let root_value = tx.compute(&key).await.unwrap();
        let root_carrier = observed_apparent_mapping_carrier(&root_value);
        let root_child_key = HostRootRepositoryMappingObservationKey::new(workspace.clone());
        let root_child_value = tx.compute(&root_child_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(root_child)) = root_child_value else { panic!("root child must complete") };
        assert_eq!(root_carrier.observations(), root_child.observations());
        assert_eq!(activation_dependencies(&tracker, &key.to_string()), [root_child_key.to_string()]);
        assert!(matches!(root_carrier.result().as_ref(), Ok(HostCanonicalRepositoryApparentMapping { predecessor: ApparentMappingPredecessor::Root(_), .. })));

        let nonroot_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), generated.clone(), first.clone());
        let nonroot_value = tx.compute(&nonroot_key).await.unwrap();
        let nonroot_carrier = observed_apparent_mapping_carrier(&nonroot_value);
        let definition_child_key = HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), generated.clone());
        let definition_child_value = tx.compute(&definition_child_key).await.unwrap();
        let definition_child = observed_canonical_carrier(&definition_child_value);
        assert_eq!(nonroot_carrier.observations(), definition_child.observations());
        assert_eq!(activation_dependencies(&tracker, &nonroot_key.to_string()), [definition_child_key.to_string()]);
        assert!(matches!(nonroot_carrier.result().as_ref(), Ok(HostCanonicalRepositoryApparentMapping { predecessor: ApparentMappingPredecessor::Canonical(_), .. })));
        assert!(HostCanonicalRepositoryApparentMappingObservationKey::validity(&nonroot_value));
        assert!(HostCanonicalRepositoryApparentMappingObservationKey::equality(&nonroot_value, &nonroot_value));

        let epoch_only = SourcePreparationOutcome::Complete(Ok(ObservedHostCanonicalRepositoryApparentMapping { result: nonroot_carrier.result().clone(), observations: PathObservationEpoch::empty() }));
        assert!(!HostCanonicalRepositoryApparentMappingObservationKey::equality(&nonroot_value, &epoch_only));
        let root_apparent_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), generated.clone(), ApparentRepoName::root());
        let root_apparent = tx.compute(&root_apparent_key).await.unwrap();
        assert!(matches!(observed_apparent_mapping_carrier(&root_apparent).result().as_ref(), Err(HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::RootApparent, .. })));
        assert!(observed_apparent_mapping_carrier(&root_apparent).observations().observations().is_empty());
        assert!(activation_dependencies(&tracker, &root_apparent_key.to_string()).is_empty());

        let need_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let _ = transaction(&need_dice, MODULE, EXTENSION_A, true, None).await;
        let mut updater = need_dice.updater();
        updater.changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())]).unwrap();
        let need = updater.commit().await.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Need(needs) = &need else { panic!("observed parent must need") };
        assert!(!HostCanonicalRepositoryApparentMappingObservationKey::validity(&need));
        assert!(!HostCanonicalRepositoryApparentMappingObservationKey::equality(&need, &need));
        assert!(matches!(finish_canonical_repository_apparent_mapping(&key.0, CanonicalRepositoryApparentMappingChildOutcome::Need(needs.clone())), SourcePreparationOutcome::Need(_)));

        for kind in [HostCanonicalRepositoryApparentMappingErrorKind::RootMappingCompute("root-dice".into()), HostCanonicalRepositoryApparentMappingErrorKind::DefinitionCompute("definition-dice".into())] {
            let complete = finish_canonical_repository_apparent_mapping(&key.0, CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Err(kind), observations: PathObservationEpoch::empty() });
            assert!(matches!(complete, SourcePreparationOutcome::Complete(Ok((result, observations))) if result.is_err() && observations.observations().is_empty()));
        }
        let mismatch_key = HostCanonicalRepositoryApparentMappingKey::new(workspace.clone(), root.clone(), first.clone());
        let predecessor = definition_child.result().as_ref().as_ref().unwrap().clone();
        let mismatch = finish_canonical_repository_apparent_mapping(&mismatch_key, CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Ok(ApparentMappingPredecessor::Canonical(predecessor.clone())), observations: definition_child.observations().clone() });
        assert!(matches!(mismatch, SourcePreparationOutcome::Complete(Ok((result, observations))) if matches!(result.as_ref(), Err(HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::ContextMismatch { predecessor: ApparentMappingPredecessor::Canonical(_) }, .. })) && observations == *definition_child.observations()));
        let missing_key = HostCanonicalRepositoryApparentMappingKey::new(workspace.clone(), generated.clone(), ApparentRepoName::new("missing").unwrap());
        let missing = finish_canonical_repository_apparent_mapping(&missing_key, CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Ok(ApparentMappingPredecessor::Canonical(predecessor.clone())), observations: definition_child.observations().clone() });
        assert!(matches!(missing, SourcePreparationOutcome::Complete(Ok((result, observations))) if matches!(result.as_ref(), Err(HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::Missing { predecessor: ApparentMappingPredecessor::Canonical(_) }, .. })) && observations == *definition_child.observations()));
        let success = finish_canonical_repository_apparent_mapping(&nonroot_key.0, CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Ok(ApparentMappingPredecessor::Canonical(predecessor)), observations: definition_child.observations().clone() });
        assert!(matches!(success, SourcePreparationOutcome::Complete(Ok((result, observations))) if result.as_ref().as_ref().unwrap().resolved_target() == nonroot_carrier.result().as_ref().as_ref().unwrap().resolved_target() && observations == *definition_child.observations()));

        let bad_root_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut bad_root_tx = transaction(&bad_root_dice, "this is not valid Starlark\n", EXTENSION_A, true, None).await;
        let bad_root = bad_root_tx.compute(&key).await.unwrap();
        assert!(matches!(observed_apparent_mapping_carrier(&bad_root).result().as_ref(), Err(HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::RootMapping(_), .. })));
        let bad_definition_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut bad_definition_tx = transaction(&bad_definition_dice, MODULE, EXTENSION_A, false, None).await;
        let bad_definition = bad_definition_tx.compute(&nonroot_key).await.unwrap();
        assert!(matches!(observed_apparent_mapping_carrier(&bad_definition).result().as_ref(), Err(HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::Definition(_), .. })));

        let missing_definition = tx.compute(&HostCanonicalRepositoryDefinitionKey::new(workspace.clone(), CanonicalRepoName::new("missing").unwrap())).await.unwrap();
        let SourcePreparationOutcome::Complete(missing_definition) = missing_definition else { panic!("missing definition must complete") };
        let HostCanonicalRepositoryDefinitionErrorKind::Missing { selected_missing, .. } = &missing_definition.as_ref().as_ref().unwrap_err().kind else { panic!("missing definition terminal expected") };
        let conflict = merge_canonical_observations(&generated_definition_observation_epoch(MODULE, EXTENSION_A, true), &generated_definition_observation_epoch(&format!("{MODULE}\n"), EXTENSION_A, true)).unwrap_err();
        let outer = finish_canonical_repository_apparent_mapping(&nonroot_key.0, CanonicalRepositoryApparentMappingChildOutcome::Outer(CanonicalRepositoryApparentMappingObservationError::Definition(HostCanonicalRepositoryDefinitionObservationError::Merge { selected_missing: selected_missing.clone(), error: conflict })));
        let outer_value: <HostCanonicalRepositoryApparentMappingObservationKey as Key>::Value = match outer { SourcePreparationOutcome::Complete(Err(error @ CanonicalRepositoryApparentMappingObservationError::Definition(_))) => SourcePreparationOutcome::Complete(Err(HostCanonicalRepositoryApparentMappingObservationError(error))), _ => panic!("definition outer expected") };
        assert!(HostCanonicalRepositoryApparentMappingObservationKey::validity(&outer_value));
        assert!(HostCanonicalRepositoryApparentMappingObservationKey::equality(&outer_value, &outer_value));

        let source = include_str!("generated_repository_definition.rs");
        let producer = &source[source.find("type CanonicalRepositoryApparentMappingResult").unwrap()..source.find("#[cfg(test)]").unwrap()];
        assert_eq!(producer.matches("HostRootRepositoryMappingObservationKey::new").count(), 1);
        assert_eq!(producer.matches("HostCanonicalRepositoryDefinitionObservationKey::new").count(), 1);
        assert_eq!(producer.matches("CanonicalRepositoryApparentMappingObservationError::RootMapping(error)").count(), 1);
        assert_eq!(producer.matches("CanonicalRepositoryApparentMappingObservationError::Definition(error)").count(), 1);
        assert_eq!(producer.matches("HostCanonicalRepositoryApparentMappingObservationError(error)").count(), 1);
        for forbidden in ["merge_canonical_observations", "union_", "store_evaluation_data", "HostRootApparentRepositoryDefinitionKey"] { assert!(!producer.contains(forbidden)); }
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_apparent_mapping_real_branches_events_and_parity() {
        let extension = r#"print('load')
repo=repository_rule(implementation=lambda ctx: None)
def first_impl(ctx):
    print('invoke-first')
    repo(name='first')
def second_impl(ctx):
    print('invoke-second')
    repo(name='second')
first=module_extension(implementation=first_impl)
second=module_extension(implementation=second_impl)
"#;
        let module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\n";
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, module, extension, true, None).await;
        let generated = names(&validated(&mut prep_tx).await);
        for (case, case_module, present, context, expected) in [
            ("root-success", module, true, CanonicalRepoName::root(), "success"),
            ("root-error", "this is not valid Starlark\n", true, CanonicalRepoName::root(), "root-error"),
            ("nonroot-success", module, true, generated[0].clone(), "success"),
            ("nonroot-error", module, false, generated[0].clone(), "definition-error"),
        ] {
            let observed_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let observed_tracker = Arc::new(LookupTracker::default());
            let mut observed_tx = transaction(&observed_dice, case_module, extension, present, Some(observed_tracker.clone())).await;
            let observed_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), context.clone(), ApparentRepoName::new("first").unwrap());
            let observed_value = observed_tx.compute(&observed_key).await.unwrap();
            let carrier = observed_apparent_mapping_carrier(&observed_value);
            let (observed_child, legacy_child) = if context.is_root() {
                (HostRootRepositoryMappingObservationKey::new(workspace.clone()).to_string(), HostRootRepositoryMappingKey::new(workspace.clone()).to_string())
            } else {
                (HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), context.clone()).to_string(), HostCanonicalRepositoryDefinitionKey::new(workspace.clone(), context.clone()).to_string())
            };
            assert_eq!(activation_dependencies(&observed_tracker, &observed_key.to_string()), [observed_child.clone()], "{case}");
            assert!(observed_tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &observed_key.to_string()).unwrap().1.iter().all(|child| child == &observed_child));
            let observed_parent = observed_tracker.activations.lock().unwrap().iter().find(|(name, _, _)| name == &observed_key.to_string()).cloned().unwrap();
            assert_eq!(observed_parent.1, ActivationKind::Evaluated); assert!(observed_parent.2.is_none());
            let observed_prints = starlark_print_owners(&observed_tracker);

            let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let legacy_tracker = Arc::new(LookupTracker::default());
            let mut legacy_tx = transaction(&legacy_dice, case_module, extension, present, Some(legacy_tracker.clone())).await;
            let legacy_key = HostCanonicalRepositoryApparentMappingKey::new(workspace.clone(), context.clone(), ApparentRepoName::new("first").unwrap());
            let legacy_value = legacy_tx.compute(&legacy_key).await.unwrap();
            assert_eq!(activation_dependencies(&legacy_tracker, &legacy_key.to_string()), [legacy_child], "{case}");
            let SourcePreparationOutcome::Complete(legacy_result) = &legacy_value else { panic!("{case}: legacy must complete") };
            assert_eq!(legacy_result.as_ref(), carrier.result().as_ref(), "{case}");
            let child_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let child_tracker = Arc::new(LookupTracker::default());
            let mut child_tx = transaction(&child_dice, case_module, extension, present, Some(child_tracker.clone())).await;
            if context.is_root() {
                let _ = child_tx.compute(&HostRootRepositoryMappingObservationKey::new(workspace.clone())).await.unwrap();
            } else {
                let _ = child_tx.compute(&HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), context.clone())).await.unwrap();
            }
            assert_eq!(observed_prints, starlark_print_owners(&child_tracker), "{case}: lower event owner/payload");
            match expected {
                "success" => {
                    let value = carrier.result().as_ref().as_ref().unwrap();
                    assert_eq!(value.resolved_target(), Some(&generated[0]), "{case}");
                    let borrowed = match &value.predecessor {
                        ApparentMappingPredecessor::Root(predecessor) => predecessor.view().unwrap().mapping().find_map(|(name, target)| (name.as_str() == "first").then_some(target)).unwrap(),
                        ApparentMappingPredecessor::Canonical(predecessor) => predecessor.mapping_target(&ApparentRepoName::new("first").unwrap()).unwrap(),
                    };
                    assert!(std::ptr::eq(borrowed, value.resolved_target().unwrap()), "{case}");
                }
                "root-error" => assert!(matches!(carrier.result().as_ref(), Err(HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::RootMapping(_), .. }))),
                "definition-error" => assert!(matches!(carrier.result().as_ref(), Err(HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::Definition(_), .. }))),
                _ => unreachable!(),
            }
            observed_tracker.activations.lock().unwrap().clear();
            let warm = observed_tx.compute(&observed_key).await.unwrap();
            assert!(HostCanonicalRepositoryApparentMappingObservationKey::equality(&observed_value, &warm));
            assert!(Arc::ptr_eq(carrier.result(), observed_apparent_mapping_carrier(&warm).result()));
            let warm_rows = observed_tracker.activations.lock().unwrap();
            assert!(warm_rows.iter().any(|(name, kind, batch)| name == &observed_key.to_string() && *kind == ActivationKind::Reused && batch.is_none()), "{case}");
            assert!(warm_rows.iter().all(|(_, _, batch)| batch.is_none()), "{case}");
            drop(warm_rows);
            assert!(starlark_print_owners(&observed_tracker).is_empty(), "{case}: warm replay");
            let unchosen = if context.is_root() { "observed-host-canonical-repository-definition:" } else { "observed-host-root-repository-mapping:" };
            assert_activation_families_absent(&observed_tracker, &[unchosen, "host-root-apparent-repository-definition:", "build-command-root:"]);
        }
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_apparent_mapping_lifecycle_cancellation_and_nonactivation() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, MODULE, EXTENSION_A, true, None).await;
        let generated = names(&validated(&mut prep_tx).await).remove(0);
        let root_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), CanonicalRepoName::root(), ApparentRepoName::new("first").unwrap());
        let nonroot_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), generated.clone(), ApparentRepoName::new("first").unwrap());
        let root_b = format!("{MODULE}override_repo(e, first='bazel_tools')\n");
        let same_semantic = format!("{MODULE}\n");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));

        let mut root_held = Vec::new();
        for module in [MODULE, root_b.as_str(), MODULE, same_semantic.as_str()] {
            let mut tx = transaction(&dice, module, EXTENSION_A, true, None).await;
            let global = tx.compute(&PathObservationEpochKey).await.unwrap();
            let parent = observed_apparent_mapping_carrier(&tx.compute(&root_key).await.unwrap()).clone();
            let child_value = tx.compute(&HostRootRepositoryMappingObservationKey::new(workspace.clone())).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child_value else { panic!("root child must complete") };
            assert_eq!(parent.observations(), child.observations());
            assert_apparent_epoch_current(parent.observations(), &global);
            assert_apparent_epoch_current(child.observations(), &global);
            root_held.push((parent, child));
        }
        assert_ne!(root_held[0].0.result(), root_held[1].0.result()); assert_eq!(root_held[0].0.result(), root_held[2].0.result());
        assert_ne!(root_held[0].1.result(), root_held[1].1.result()); assert_eq!(root_held[0].1.result(), root_held[2].1.result());
        assert_eq!(root_held[0].0.result(), root_held[3].0.result()); assert_ne!(root_held[0].0.observations(), root_held[3].0.observations());
        assert_eq!(root_held[0].1.result(), root_held[3].1.result()); assert_ne!(root_held[0].1.observations(), root_held[3].1.observations());

        let mut nonroot_held = Vec::new();
        for (module, extension) in [(MODULE, EXTENSION_A), (MODULE, EXTENSION_B), (MODULE, EXTENSION_A), (same_semantic.as_str(), EXTENSION_A)] {
            let mut tx = transaction(&dice, module, extension, true, None).await;
            let global = tx.compute(&PathObservationEpochKey).await.unwrap();
            let parent = observed_apparent_mapping_carrier(&tx.compute(&nonroot_key).await.unwrap()).clone();
            let child_value = tx.compute(&HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), generated.clone())).await.unwrap();
            let child = observed_canonical_carrier(&child_value).clone();
            assert_eq!(parent.observations(), child.observations());
            assert_apparent_epoch_current(parent.observations(), &global);
            assert_apparent_epoch_current(child.observations(), &global);
            nonroot_held.push((parent, child));
        }
        assert_ne!(nonroot_held[0].0.result(), nonroot_held[1].0.result()); assert_eq!(nonroot_held[0].0.result(), nonroot_held[2].0.result());
        assert_ne!(nonroot_held[0].1.result(), nonroot_held[1].1.result()); assert_eq!(nonroot_held[0].1.result(), nonroot_held[2].1.result());
        assert_eq!(nonroot_held[0].0.result(), nonroot_held[3].0.result()); assert_ne!(nonroot_held[0].0.observations(), nonroot_held[3].0.observations());
        assert_eq!(nonroot_held[0].1.result(), nonroot_held[3].1.result()); assert_ne!(nonroot_held[0].1.observations(), nonroot_held[3].1.observations());

        for key in [&root_key, &nonroot_key] {
            let tracker = Arc::new(LookupTracker::default());
            let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
            let first = observed_apparent_mapping_carrier(&tx.compute(key).await.unwrap()).clone();
            tracker.activations.lock().unwrap().clear();
            let reused = observed_apparent_mapping_carrier(&tx.compute(key).await.unwrap()).clone();
            assert!(Arc::ptr_eq(first.result(), reused.result()));
            assert!(tracker.activations.lock().unwrap().iter().any(|(name, kind, batch)| name == &key.to_string() && *kind == ActivationKind::Reused && batch.is_none()));
        }

        let mut lifecycle_trackers = Vec::new();
        for (case, key, child) in [
            ("root", root_key.clone(), HostRootRepositoryMappingObservationKey::new(workspace.clone()).to_string()),
            ("nonroot", nonroot_key.clone(), HostCanonicalRepositoryDefinitionObservationKey::new(workspace.clone(), generated.clone()).to_string()),
        ] {
            let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let tracker = Arc::new(LookupTracker::default());
            let mut cancelled = transaction(&cancel_dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
            let mut future = Box::pin(cancelled.compute(&key));
            std::future::poll_fn(|context| { assert!(std::future::Future::poll(future.as_mut(), context).is_pending()); std::task::Poll::Ready(()) }).await;
            drop(future); drop(cancelled);
            assert!(tracker.activations.lock().unwrap().iter().all(|(name, _, _)| name != &key.to_string()), "{case}");
            assert!(tracker.dependencies.lock().unwrap().iter().all(|(name, _)| name != &key.to_string()), "{case}");

            let mut recovery = transaction(&cancel_dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
            let global = recovery.compute(&PathObservationEpochKey).await.unwrap();
            let recovered = observed_apparent_mapping_carrier(&recovery.compute(&key).await.unwrap()).clone();
            let expected = if case == "root" { root_held[0].0.result() } else { nonroot_held[0].0.result() };
            assert_eq!(recovered.result(), expected, "{case}");
            assert_apparent_epoch_current(recovered.observations(), &global);
            assert_eq!(activation_dependencies(&tracker, &key.to_string()), [child]);
            assert!(tracker.activations.lock().unwrap().iter().any(|(name, kind, batch)| name == &key.to_string() && *kind == ActivationKind::Evaluated && batch.is_none()));
            lifecycle_trackers.push((case, tracker));
        }

        let inactive = ["host-canonical-repository-apparent-mapping:", "host-root-apparent-repository-definition:", "HostRootApparentRepositoryRouteKey", "HostRootApparentRepositorySourceInputKey", "HostRootApparentRepositorySourceObservationKey", "HostRootApparentRepositorySourcePathInputKey", "root-repository-route:", "repository-package-source:", "repository-source-file:", "host-repository-source-file:", "repository-materialization:", "build-command-root:"];
        for (case, tracker) in lifecycle_trackers {
            assert_activation_families_absent(&tracker, &inactive);
            let unchosen = if case == "root" { "observed-host-canonical-repository-definition:" } else { "observed-host-root-repository-mapping:" };
            assert_activation_families_absent(&tracker, &[unchosen]);
        }
        let source = include_str!("generated_repository_definition.rs");
        let producer = &source[source.find("type CanonicalRepositoryApparentMappingResult").unwrap()..source.find("#[cfg(test)]").unwrap()];
        for forbidden in ["HostRootApparentRepositoryDefinitionKey", "HostRootApparentRepositoryRouteKey", "HostRootApparentRepositorySourceInputKey", "HostRootApparentRepositorySourceObservationKey", "HostRootApparentRepositorySourcePathInputKey", "RepositoryMaterializationKey", "BuildCommandRootKey", "RootModuleBootstrap", "store_evaluation_data", "Mutex", "spawn"] { assert!(!producer.contains(forbidden)); }
    }

    #[tokio::test]
    async fn predecessor_need_and_error_precede_lookup() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut initial = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let generated = names(&validated(&mut initial).await);
        let key = HostGeneratedRepositoryDefinitionKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            generated[0].clone(),
        );
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let mut need_tx = updater.commit().await;
        let need = need_tx.compute(&key).await.unwrap();
        assert!(!HostGeneratedRepositoryDefinitionKey::validity(&need));
        assert!(!HostGeneratedRepositoryDefinitionKey::equality(
            &need, &need
        ));
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        let mapping_key = HostCanonicalRepositoryApparentMappingKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            generated[0].clone(),
            ApparentRepoName::new("first").unwrap(),
        );
        let mapping_need = need_tx.compute(&mapping_key).await.unwrap();
        assert!(!HostCanonicalRepositoryApparentMappingKey::validity(
            &mapping_need
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &mapping_need,
            &mapping_need,
        ));
        let root_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let _root_tx = transaction(
            &root_dice,
            "module(name='bazel_tools')\n\
             local_path_override(module_name='local', path='local')\n\
             bazel_dep(name='local', version='1', repo_name='local_alias')\n",
            EXTENSION_A,
            true,
            None,
        )
        .await;
        let mut root_updater = root_dice.updater();
        root_updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let root_mapping_need = root_updater
            .commit()
            .await
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::root(),
            ))
            .await
            .unwrap();
        assert!(matches!(
            root_mapping_need,
            SourcePreparationOutcome::Need(_)
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::validity(
            &root_mapping_need,
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_mapping_need,
            &root_mapping_need,
        ));

        let mut missing_source = transaction(&dice, MODULE, EXTENSION_A, false, None).await;
        let terminal = missing_source.compute(&key).await.unwrap();
        assert!(HostGeneratedRepositoryDefinitionKey::validity(&terminal));
        assert!(matches!(
            terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostGeneratedRepositoryDefinitionError {
                        kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(_),
                        ..
                    })
                )
        ));
        let mapping_terminal = missing_source.compute(&mapping_key).await.unwrap();
        assert!(matches!(
            mapping_terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Definition(_),
                        ..
                    })
                )
        ));
    }
}
