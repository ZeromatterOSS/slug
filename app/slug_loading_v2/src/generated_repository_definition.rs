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
use slug_bzlmod_v2::HostCanonicalRepositoryRoute;
use slug_bzlmod_v2::HostSelectedExtensionDemand;
use slug_bzlmod_v2::HostSelectedExtensionDemandError;
use slug_bzlmod_v2::HostSelectedExtensionDemandErrorDisposition;
use slug_bzlmod_v2::HostSelectedExtensionDemandKey;
use slug_bzlmod_v2::HostSelectedExtensionDemandObservationError;
use slug_bzlmod_v2::HostSelectedExtensionDemandObservationKey;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use crate::HostSelectedExtensionOwnerCertificateError;
use crate::HostSelectedExtensionOwnerCertificateKey;
use crate::HostSelectedExtensionOwnerCertificateObservationError;
use crate::HostSelectedExtensionOwnerCertificateObservationKey;

type HostGeneratedRepositoryDefinition = HostCanonicalRepositoryRoute;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) enum HostGeneratedRepositoryDefinitionErrorKind {
    Demand(HostSelectedExtensionDemandError),
    DemandCompute(Arc<str>),
    Loading(HostSelectedExtensionOwnerCertificateError),
    LoadingCompute(Arc<str>),
    Missing {},
    Duplicate { first: usize, conflicting: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostGeneratedRepositoryDefinitionError {
    pub(super) requested: CanonicalRepoName,
    pub(super) kind: HostGeneratedRepositoryDefinitionErrorKind,
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

pub(super) type HostGeneratedRepositoryDefinitionOutcome = SourcePreparationOutcome<
    Arc<Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>>,
>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostGeneratedRepositoryDefinitionKey {
    workspace: NormalizedAbsolutePath,
    canonical_repo: CanonicalRepoName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostGeneratedRepositoryDefinitionObservationKey(
    HostGeneratedRepositoryDefinitionKey,
);

impl HostGeneratedRepositoryDefinitionObservationKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        canonical_repo: CanonicalRepoName,
    ) -> Self {
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
pub(super) struct ObservedHostGeneratedRepositoryDefinition {
    result: Arc<Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostGeneratedRepositoryDefinition {
    pub(super) fn result(
        &self,
    ) -> &Arc<Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>>
    {
        &self.result
    }

    pub(super) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) enum HostGeneratedRepositoryDefinitionObservationError {
    Demand(HostSelectedExtensionDemandObservationError),
    Validation {
        demand: Arc<HostSelectedExtensionDemand>,
        error: HostSelectedExtensionOwnerCertificateObservationError,
    },
    Merge {
        demand: Arc<HostSelectedExtensionDemand>,
        error: ObservedPathFrontierError,
    },
}

impl HostGeneratedRepositoryDefinitionObservationError {
    pub(super) fn selected_frontier(&self) -> slug_bzlmod_v2::HostSelectedObservationFrontier {
        match self {
            Self::Demand(error) => error.selected_frontier(),
            Self::Validation { error, .. } => error.selected_frontier(),
            Self::Merge { error, .. } => {
                slug_bzlmod_v2::HostSelectedObservationFrontier::Path(error.clone())
            }
        }
    }
}

impl HostGeneratedRepositoryDefinitionKey {
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

pub(super) fn complete_generated_driver(
    value: Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>,
    observations: PathObservationEpoch,
) -> GeneratedRepositoryDefinitionDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations)))
}

fn merge_generated_observations(
    demand: &PathObservationEpoch,
    owner: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        demand
            .observations()
            .iter()
            .chain(owner.observations().iter())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(ObservedPathFrontierError::from)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UniqueOrdinalError {
    Missing,
    Duplicate { first: usize, conflicting: usize },
}

pub(super) fn find_unique_ordinal<'a>(
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
    let (demand, observations) = match mode {
        GeneratedRepositoryDefinitionMode::Legacy => match ctx.compute(&HostSelectedExtensionDemandKey::new(key.workspace.clone(), key.canonical_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
            Err(error) => return complete_generated_driver(Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::DemandCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
        },
        GeneratedRepositoryDefinitionMode::Observed => match ctx.compute(&HostSelectedExtensionDemandObservationKey::new(key.workspace.clone(), key.canonical_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(HostGeneratedRepositoryDefinitionObservationError::Demand(error))),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
            Err(error) => return complete_generated_driver(Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::DemandCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
        },
    };
    let demand = match demand.as_ref() {
        Ok(value) => Arc::new(value.clone()),
        Err(error) if error.disposition() == HostSelectedExtensionDemandErrorDisposition::Missing => return complete_generated_driver(Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::Missing {} }), observations),
        Err(error) => return complete_generated_driver(Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::Demand(error.clone()) }), observations),
    };
    let (result, owner_observations) = match mode {
        GeneratedRepositoryDefinitionMode::Legacy => match ctx.compute(&HostSelectedExtensionOwnerCertificateKey::new(key.workspace.clone(), demand.owner().clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
            Err(error) => return complete_generated_driver(Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::LoadingCompute(error.to_string().into()) }), observations),
        },
        GeneratedRepositoryDefinitionMode::Observed => match ctx.compute(&HostSelectedExtensionOwnerCertificateObservationKey::new(key.workspace.clone(), demand.owner().clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(HostGeneratedRepositoryDefinitionObservationError::Validation { demand, error })),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
            Err(error) => return complete_generated_driver(Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::LoadingCompute(error.to_string().into()) }), observations),
        },
    };
    let observations = match merge_generated_observations(&observations, &owner_observations) {
        Ok(value) => value,
        Err(error) => return SourcePreparationOutcome::Complete(Err(HostGeneratedRepositoryDefinitionObservationError::Merge { demand, error })),
    };
    let certificate = match result.as_ref() {
        Ok(value) => value,
        Err(error) => return complete_generated_driver(Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(error.clone()) }), observations),
    };
    let value = match find_unique_ordinal(&key.canonical_repo, certificate.iter().map(|(canonical, _, _, _)| canonical)) {
        Ok(ordinal) => {
            let (canonical, repo_spec, internal_name, mapping) = certificate
                .iter()
                .nth(ordinal)
                .expect("the unique generated repository ordinal remains present");
            Ok(HostCanonicalRepositoryRoute::generated(
                key.workspace.clone(),
                canonical.clone(),
                demand.owner().clone(),
                ordinal,
                internal_name,
                repo_spec.clone(),
                mapping.context_repo().clone(),
                mapping.entries().clone(),
            )
            .expect("validated generated repository rows have canonical polarity"))
        }
        Err(UniqueOrdinalError::Missing) => Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::Missing {} }),
        Err(UniqueOrdinalError::Duplicate { first, conflicting }) => Err(HostGeneratedRepositoryDefinitionError { requested: key.canonical_repo.clone(), kind: HostGeneratedRepositoryDefinitionErrorKind::Duplicate { first, conflicting } }),
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
