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
use slug_bzlmod_v2::HostBuiltinBazelToolsRepositoryMappingError;
use slug_bzlmod_v2::HostBuiltinBazelToolsRepositoryMappingKey;
use slug_bzlmod_v2::HostBuiltinBazelToolsRepositoryMappingObservationError;
use slug_bzlmod_v2::HostBuiltinBazelToolsRepositoryMappingObservationKey;
use slug_bzlmod_v2::HostCanonicalRepositoryRoute;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionError;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionErrorDisposition;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionKey;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionObservationError;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionObservationKey;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use crate::generated_repository_definition::HostGeneratedRepositoryDefinitionError;
use crate::generated_repository_definition::HostGeneratedRepositoryDefinitionErrorKind;
use crate::generated_repository_definition::HostGeneratedRepositoryDefinitionKey;
use crate::generated_repository_definition::HostGeneratedRepositoryDefinitionObservationError;
use crate::generated_repository_definition::HostGeneratedRepositoryDefinitionObservationKey;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) enum HostCanonicalRepositoryRouteErrorKind {
    Builtin(HostBuiltinBazelToolsRepositoryMappingError),
    BuiltinCompute(Arc<str>),
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

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositoryRouteError {
    pub(super) canonical_repo: CanonicalRepoName,
    pub(super) kind: HostCanonicalRepositoryRouteErrorKind,
}

impl HostCanonicalRepositoryRouteError {
    pub fn is_missing(&self) -> bool {
        matches!(
            self.kind,
            HostCanonicalRepositoryRouteErrorKind::Missing { .. }
        )
    }
}

impl fmt::Display for HostCanonicalRepositoryRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "canonical repository route '{}': {:?}",
            self.canonical_repo, self.kind
        )
    }
}

impl std::error::Error for HostCanonicalRepositoryRouteError {}

#[doc(hidden)]
pub type HostCanonicalRepositoryRouteOutcome = SourcePreparationOutcome<
    Arc<Result<HostCanonicalRepositoryRoute, HostCanonicalRepositoryRouteError>>,
>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostCanonicalRepositoryRouteKey {
    workspace: NormalizedAbsolutePath,
    canonical_repo: CanonicalRepoName,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostCanonicalRepositoryRouteObservationKey(HostCanonicalRepositoryRouteKey);

impl HostCanonicalRepositoryRouteKey {
    pub fn new(workspace: NormalizedAbsolutePath, canonical_repo: CanonicalRepoName) -> Self {
        Self {
            workspace,
            canonical_repo,
        }
    }
}

impl HostCanonicalRepositoryRouteObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath, canonical_repo: CanonicalRepoName) -> Self {
        Self(HostCanonicalRepositoryRouteKey::new(
            workspace,
            canonical_repo,
        ))
    }
}

impl fmt::Display for HostCanonicalRepositoryRouteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-canonical-repository-route:{}:{}",
            self.workspace, self.canonical_repo
        )
    }
}

impl fmt::Display for HostCanonicalRepositoryRouteObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type CanonicalRepositoryRouteResult =
    Arc<Result<HostCanonicalRepositoryRoute, HostCanonicalRepositoryRouteError>>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostCanonicalRepositoryRoute {
    result: CanonicalRepositoryRouteResult,
    observations: PathObservationEpoch,
}

impl ObservedHostCanonicalRepositoryRoute {
    pub fn result(&self) -> &CanonicalRepositoryRouteResult {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) enum CanonicalRepositoryRouteObservationError {
    Builtin(HostBuiltinBazelToolsRepositoryMappingObservationError),
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

impl Dupe for CanonicalRepositoryRouteObservationError {}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositoryRouteObservationError(
    pub(super) CanonicalRepositoryRouteObservationError,
);

impl Dupe for HostCanonicalRepositoryRouteObservationError {}

pub(super) fn complete_route_driver(
    value: Result<HostCanonicalRepositoryRoute, HostCanonicalRepositoryRouteError>,
    observations: PathObservationEpoch,
) -> CanonicalRepositoryRouteDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations)))
}

pub(super) fn merge_route_observations(
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
enum CanonicalRepositoryRouteMode {
    Legacy,
    Observed,
}

type CanonicalRepositoryRouteDriverOutcome = SourcePreparationOutcome<
    Result<
        (CanonicalRepositoryRouteResult, PathObservationEpoch),
        CanonicalRepositoryRouteObservationError,
    >,
>;

#[rustfmt::skip]
async fn compute_canonical_repository_route(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositoryRouteKey,
    mode: CanonicalRepositoryRouteMode,
) -> CanonicalRepositoryRouteDriverOutcome {
    if key.canonical_repo.as_str() == "bazel_tools" {
        let (mapping, observations) = match mode {
            CanonicalRepositoryRouteMode::Legacy => match ctx.compute(&HostBuiltinBazelToolsRepositoryMappingKey::new(key.workspace.clone())).await {
                Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
                Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
                Err(error) => return complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::BuiltinCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
            },
            CanonicalRepositoryRouteMode::Observed => match ctx.compute(&HostBuiltinBazelToolsRepositoryMappingObservationKey::new(key.workspace.clone())).await {
                Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
                Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(CanonicalRepositoryRouteObservationError::Builtin(error))),
                Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
                Err(error) => return complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::BuiltinCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
            },
        };
        return match mapping.as_ref() {
            Ok(mapping) => complete_route_driver(Ok(HostCanonicalRepositoryRoute::builtin(key.workspace.clone(), mapping.clone())), observations),
            Err(error) => complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::Builtin(error.clone()) }), observations),
        };
    }
    let (selected, selected_observations) = match mode {
        CanonicalRepositoryRouteMode::Legacy => match ctx.compute(&HostCanonicalSelectedModuleDefinitionKey::new(key.workspace.clone(), key.canonical_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
            Err(error) => return complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::SelectedCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
        },
        CanonicalRepositoryRouteMode::Observed => match ctx.compute(&HostCanonicalSelectedModuleDefinitionObservationKey::new(key.workspace.clone(), key.canonical_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(CanonicalRepositoryRouteObservationError::Selected(error))),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
            Err(error) => return complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::SelectedCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
        },
    };
    let selected_missing = match selected.as_ref() {
        Ok(value) => return complete_route_driver(Ok(HostCanonicalRepositoryRoute::from_selected(key.workspace.clone(), value.clone())), selected_observations),
        Err(error) if error.disposition() == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing => error.clone(),
        Err(error) => return complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::Selected(error.clone()) }), selected_observations),
    };
    let (generated, generated_observations) = match mode {
        CanonicalRepositoryRouteMode::Legacy => match ctx.compute(&HostGeneratedRepositoryDefinitionKey::new(key.workspace.clone(), key.canonical_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
            Err(error) => return complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::GeneratedCompute { selected_missing, message: error.to_string().into() } }), selected_observations),
        },
        CanonicalRepositoryRouteMode::Observed => match ctx.compute(&HostGeneratedRepositoryDefinitionObservationKey::new(key.workspace.clone(), key.canonical_repo.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(CanonicalRepositoryRouteObservationError::Generated { selected_missing, error })),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
            Err(error) => return complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::GeneratedCompute { selected_missing, message: error.to_string().into() } }), selected_observations),
        },
    };
    let observations = match merge_route_observations(&selected_observations, &generated_observations) {
        Ok(observations) => observations,
        Err(error) => return SourcePreparationOutcome::Complete(Err(CanonicalRepositoryRouteObservationError::Merge { selected_missing, error })),
    };
    let value = match generated.as_ref() {
        Ok(value) => Ok(value.clone()),
        Err(error) if matches!(error.kind, HostGeneratedRepositoryDefinitionErrorKind::Missing { .. }) => Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::Missing { selected_missing, generated_missing: error.clone() } }),
        Err(error) => Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::Generated { selected_missing, error: error.clone() } }),
    };
    complete_route_driver(value, observations)
}

#[async_trait]
impl Key for HostCanonicalRepositoryRouteKey {
    type Value = HostCanonicalRepositoryRouteOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_canonical_repository_route(ctx, self, CanonicalRepositoryRouteMode::Legacy)
            .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy canonical route has no observed outer")
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
impl Key for HostCanonicalRepositoryRouteObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostCanonicalRepositoryRoute, HostCanonicalRepositoryRouteObservationError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_canonical_repository_route(
            ctx,
            &self.0,
            CanonicalRepositoryRouteMode::Observed,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(
                Err(HostCanonicalRepositoryRouteObservationError(error)),
            ),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostCanonicalRepositoryRoute {
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
