/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

use std::fmt;
use std::ops::ControlFlow;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostCanonicalRepositoryRoute;
use slug_bzlmod_v2::HostCanonicalRepositorySourceInput;
use slug_bzlmod_v2::HostCanonicalRepositorySourceInputError;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::host_canonical_repository_source_input;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use crate::HostCanonicalRepositoryRouteError;
use crate::HostCanonicalRepositoryRouteKey;
use crate::HostCanonicalRepositoryRouteObservationError;
use crate::HostCanonicalRepositoryRouteObservationKey;
use crate::HostSelectedRepositoryFileEffectError;
use crate::HostSelectedRepositoryFileEffectKey;
use crate::HostSelectedRepositoryFileEffectObservationError;
use crate::HostSelectedRepositoryFileEffectObservationKey;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositoryLoadRoute {
    input: HostCanonicalRepositorySourceInput,
}

impl HostCanonicalRepositoryLoadRoute {
    pub fn input(&self) -> &HostCanonicalRepositorySourceInput {
        &self.input
    }

    pub fn route(&self) -> &Arc<HostCanonicalRepositoryRoute> {
        self.input.view().route()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostCanonicalRepositoryLoadRouteErrorKind {
    Route(HostCanonicalRepositoryRouteError),
    RouteCompute(Arc<str>),
    Effect(HostSelectedRepositoryFileEffectError),
    EffectCompute(Arc<str>),
    Projection(HostCanonicalRepositorySourceInputError),
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositoryLoadRouteError {
    canonical_repo: CanonicalRepoName,
    kind: HostCanonicalRepositoryLoadRouteErrorKind,
}

impl HostCanonicalRepositoryLoadRouteError {
    pub fn is_effect_error(&self) -> bool {
        matches!(
            self.kind,
            HostCanonicalRepositoryLoadRouteErrorKind::Effect(_)
                | HostCanonicalRepositoryLoadRouteErrorKind::EffectCompute(_)
        )
    }
}

impl fmt::Display for HostCanonicalRepositoryLoadRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "canonical repository load route '{}': {:?}",
            self.canonical_repo, self.kind
        )
    }
}

impl std::error::Error for HostCanonicalRepositoryLoadRouteError {}

#[doc(hidden)]
pub type HostCanonicalRepositoryLoadRouteOutcome = SourcePreparationOutcome<
    Arc<Result<HostCanonicalRepositoryLoadRoute, HostCanonicalRepositoryLoadRouteError>>,
>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostCanonicalRepositoryLoadRouteKey {
    workspace: NormalizedAbsolutePath,
    canonical_repo: CanonicalRepoName,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostCanonicalRepositoryLoadRouteObservationKey(HostCanonicalRepositoryLoadRouteKey);

impl HostCanonicalRepositoryLoadRouteKey {
    pub fn new(workspace: NormalizedAbsolutePath, canonical_repo: CanonicalRepoName) -> Self {
        Self {
            workspace,
            canonical_repo,
        }
    }
}

impl HostCanonicalRepositoryLoadRouteObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath, canonical_repo: CanonicalRepoName) -> Self {
        Self(HostCanonicalRepositoryLoadRouteKey::new(
            workspace,
            canonical_repo,
        ))
    }
}

impl fmt::Display for HostCanonicalRepositoryLoadRouteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-canonical-repository-load-route:{}:{}",
            self.workspace, self.canonical_repo
        )
    }
}

impl fmt::Display for HostCanonicalRepositoryLoadRouteObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type CanonicalLoadRouteResult =
    Arc<Result<HostCanonicalRepositoryLoadRoute, HostCanonicalRepositoryLoadRouteError>>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostCanonicalRepositoryLoadRoute {
    result: CanonicalLoadRouteResult,
    observations: PathObservationEpoch,
}

impl ObservedHostCanonicalRepositoryLoadRoute {
    pub fn result(&self) -> &CanonicalLoadRouteResult {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostCanonicalRepositoryLoadRouteObservationError {
    Route(HostCanonicalRepositoryRouteObservationError),
    Effect {
        route: Arc<HostCanonicalRepositoryRoute>,
        error: HostSelectedRepositoryFileEffectObservationError,
    },
    Merge {
        route: Arc<HostCanonicalRepositoryRoute>,
        error: ObservedPathFrontierError,
    },
}

impl Dupe for HostCanonicalRepositoryLoadRouteObservationError {}

#[derive(Clone, Copy)]
enum CanonicalLoadRouteMode {
    Legacy,
    Observed,
}

type CanonicalLoadRouteDriverOutcome = SourcePreparationOutcome<
    Result<
        (CanonicalLoadRouteResult, PathObservationEpoch),
        HostCanonicalRepositoryLoadRouteObservationError,
    >,
>;

fn complete_load_route(
    result: Result<HostCanonicalRepositoryLoadRoute, HostCanonicalRepositoryLoadRouteError>,
    observations: PathObservationEpoch,
) -> CanonicalLoadRouteDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn merge_load_route_observations(
    route: &PathObservationEpoch,
    effect: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        route
            .observations()
            .iter()
            .chain(effect.observations().iter())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(ObservedPathFrontierError::from)
}

async fn compute_route_predecessor(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositoryLoadRouteKey,
    mode: CanonicalLoadRouteMode,
) -> SourcePreparationOutcome<
    Result<
        ControlFlow<
            (CanonicalLoadRouteResult, PathObservationEpoch),
            (Arc<HostCanonicalRepositoryRoute>, PathObservationEpoch),
        >,
        HostCanonicalRepositoryLoadRouteObservationError,
    >,
> {
    let (result, observations) = match mode {
        CanonicalLoadRouteMode::Legacy => match ctx
            .compute(&HostCanonicalRepositoryRouteKey::new(
                key.workspace.clone(),
                key.canonical_repo.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(result)) => {
                (result, PathObservationEpoch::empty())
            }
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok(ControlFlow::Break((
                    Arc::new(Err(HostCanonicalRepositoryLoadRouteError {
                        canonical_repo: key.canonical_repo.clone(),
                        kind: HostCanonicalRepositoryLoadRouteErrorKind::RouteCompute(
                            error.to_string().into(),
                        ),
                    })),
                    PathObservationEpoch::empty(),
                ))));
            }
        },
        CanonicalLoadRouteMode::Observed => match ctx
            .compute(&HostCanonicalRepositoryRouteObservationKey::new(
                key.workspace.clone(),
                key.canonical_repo.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    HostCanonicalRepositoryLoadRouteObservationError::Route(error),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().clone(), observed.observations().dupe())
            }
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok(ControlFlow::Break((
                    Arc::new(Err(HostCanonicalRepositoryLoadRouteError {
                        canonical_repo: key.canonical_repo.clone(),
                        kind: HostCanonicalRepositoryLoadRouteErrorKind::RouteCompute(
                            error.to_string().into(),
                        ),
                    })),
                    PathObservationEpoch::empty(),
                ))));
            }
        },
    };
    match result.as_ref() {
        Ok(route) => SourcePreparationOutcome::Complete(Ok(ControlFlow::Continue((
            Arc::new(route.clone()),
            observations,
        )))),
        Err(error) => SourcePreparationOutcome::Complete(Ok(ControlFlow::Break((
            Arc::new(Err(HostCanonicalRepositoryLoadRouteError {
                canonical_repo: key.canonical_repo.clone(),
                kind: HostCanonicalRepositoryLoadRouteErrorKind::Route(error.clone()),
            })),
            observations,
        )))),
    }
}

async fn compute_generated_effect(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositoryLoadRouteKey,
    route: Arc<HostCanonicalRepositoryRoute>,
    route_observations: PathObservationEpoch,
    mode: CanonicalLoadRouteMode,
) -> SourcePreparationOutcome<
    Result<
        ControlFlow<
            (CanonicalLoadRouteResult, PathObservationEpoch),
            (
                Option<slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan>,
                PathObservationEpoch,
            ),
        >,
        HostCanonicalRepositoryLoadRouteObservationError,
    >,
> {
    let Some(seed) = route.view().generated_effect_seed() else {
        return SourcePreparationOutcome::Complete(Ok(ControlFlow::Continue((
            None,
            route_observations,
        ))));
    };
    let (result, effect_observations) = match mode {
        CanonicalLoadRouteMode::Legacy => match ctx
            .compute(&HostSelectedRepositoryFileEffectKey::new(
                key.workspace.clone(),
                seed.owner().clone(),
                seed.ordinal(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(result)) => {
                (result, PathObservationEpoch::empty())
            }
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok(ControlFlow::Break((
                    Arc::new(Err(HostCanonicalRepositoryLoadRouteError {
                        canonical_repo: key.canonical_repo.clone(),
                        kind: HostCanonicalRepositoryLoadRouteErrorKind::EffectCompute(
                            error.to_string().into(),
                        ),
                    })),
                    route_observations,
                ))));
            }
        },
        CanonicalLoadRouteMode::Observed => match ctx
            .compute(&HostSelectedRepositoryFileEffectObservationKey::new(
                key.workspace.clone(),
                seed.owner().clone(),
                seed.ordinal(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    HostCanonicalRepositoryLoadRouteObservationError::Effect { route, error },
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().clone(), observed.observations().dupe())
            }
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok(ControlFlow::Break((
                    Arc::new(Err(HostCanonicalRepositoryLoadRouteError {
                        canonical_repo: key.canonical_repo.clone(),
                        kind: HostCanonicalRepositoryLoadRouteErrorKind::EffectCompute(
                            error.to_string().into(),
                        ),
                    })),
                    route_observations,
                ))));
            }
        },
    };
    let observations =
        match merge_load_route_observations(&route_observations, &effect_observations) {
            Ok(value) => value,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    HostCanonicalRepositoryLoadRouteObservationError::Merge { route, error },
                ));
            }
        };
    match result.as_ref() {
        Ok(effect) => SourcePreparationOutcome::Complete(Ok(ControlFlow::Continue((
            Some(effect.plan().clone()),
            observations,
        )))),
        Err(error) => SourcePreparationOutcome::Complete(Ok(ControlFlow::Break((
            Arc::new(Err(HostCanonicalRepositoryLoadRouteError {
                canonical_repo: key.canonical_repo.clone(),
                kind: HostCanonicalRepositoryLoadRouteErrorKind::Effect(error.clone()),
            })),
            observations,
        )))),
    }
}

async fn compute_load_route(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositoryLoadRouteKey,
    mode: CanonicalLoadRouteMode,
) -> CanonicalLoadRouteDriverOutcome {
    let (route, observations) = match compute_route_predecessor(ctx, key, mode).await {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return SourcePreparationOutcome::Complete(Err(error));
        }
        SourcePreparationOutcome::Complete(Ok(ControlFlow::Continue(value))) => value,
        SourcePreparationOutcome::Complete(Ok(ControlFlow::Break(value))) => {
            return SourcePreparationOutcome::Complete(Ok(value));
        }
    };
    let (plan, observations) =
        match compute_generated_effect(ctx, key, route.clone(), observations, mode).await {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok(ControlFlow::Continue(value))) => value,
            SourcePreparationOutcome::Complete(Ok(ControlFlow::Break(value))) => {
                return SourcePreparationOutcome::Complete(Ok(value));
            }
        };
    let result = host_canonical_repository_source_input(route, plan)
        .map(|input| HostCanonicalRepositoryLoadRoute { input })
        .map_err(|error| HostCanonicalRepositoryLoadRouteError {
            canonical_repo: key.canonical_repo.clone(),
            kind: HostCanonicalRepositoryLoadRouteErrorKind::Projection(error),
        });
    complete_load_route(result, observations)
}

#[async_trait]
impl Key for HostCanonicalRepositoryLoadRouteKey {
    type Value = HostCanonicalRepositoryLoadRouteOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_load_route(ctx, self, CanonicalLoadRouteMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy canonical load route has no observed outer")
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
impl Key for HostCanonicalRepositoryLoadRouteObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostCanonicalRepositoryLoadRoute,
            HostCanonicalRepositoryLoadRouteObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_load_route(ctx, &self.0, CanonicalLoadRouteMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostCanonicalRepositoryLoadRoute {
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
