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
use slug_bzlmod_v2::HostCanonicalRepositoryRouteKind;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::HostSelectedObservationFrontier;
use slug_bzlmod_v2::RootRepositoryRouteError;
use slug_bzlmod_v2::RootRepositoryRouteKey;
use slug_bzlmod_v2::RootRepositoryRouteObservationError;
use slug_bzlmod_v2::RootRepositoryRouteObservationKey;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use crate::HostCanonicalRepositoryApparentMappingError;
use crate::HostCanonicalRepositoryApparentMappingErrorDisposition;
use crate::HostCanonicalRepositoryApparentMappingKey;
use crate::HostCanonicalRepositoryApparentMappingObservationError;
use crate::HostCanonicalRepositoryApparentMappingObservationKey;
use crate::HostCanonicalRepositoryLoadRouteError;
use crate::HostCanonicalRepositoryLoadRouteKey;
use crate::HostCanonicalRepositoryLoadRouteObservationError;
use crate::HostCanonicalRepositoryLoadRouteObservationKey;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostRootRepositoryLoadRoute {
    apparent_repo: ApparentRepoName,
    source: HostRepositorySourceRoute,
}

impl HostRootRepositoryLoadRoute {
    pub fn apparent_repo(&self) -> &ApparentRepoName {
        &self.apparent_repo
    }

    pub fn canonical_repo(&self) -> &CanonicalRepoName {
        self.source.canonical_repo()
    }

    pub fn source(&self) -> &HostRepositorySourceRoute {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostRootRepositoryLoadRouteErrorKind {
    Root(RootRepositoryRouteError),
    RootCompute(Arc<str>),
    Mapping(HostCanonicalRepositoryApparentMappingError),
    MappingCompute(Arc<str>),
    LoadRoute(HostCanonicalRepositoryLoadRouteError),
    LoadRouteCompute(Arc<str>),
    Projection(Arc<str>),
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRootRepositoryLoadRouteError {
    apparent_repo: ApparentRepoName,
    kind: HostRootRepositoryLoadRouteErrorKind,
}

impl fmt::Display for HostRootRepositoryLoadRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            HostRootRepositoryLoadRouteErrorKind::Root(error) => error.fmt(f),
            kind => write!(
                f,
                "root repository load route '@{}': {kind:?}",
                self.apparent_repo.as_str()
            ),
        }
    }
}

impl std::error::Error for HostRootRepositoryLoadRouteError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
enum RootRepositoryLoadAdmission {
    Ordinary,
    RootBuild,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostRootRepositoryLoadRouteKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    admission: RootRepositoryLoadAdmission,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostRootRepositoryLoadRouteObservationKey(HostRootRepositoryLoadRouteKey);

impl HostRootRepositoryLoadRouteKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Result<Self, String> {
        if apparent_repo.is_root() {
            return Err("external repository load route requires a nonroot apparent name".into());
        }
        Ok(Self {
            workspace,
            apparent_repo,
            admission: RootRepositoryLoadAdmission::Ordinary,
        })
    }

    pub fn for_root_build(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Result<Self, String> {
        let mut key = Self::new(workspace, apparent_repo)?;
        key.admission = RootRepositoryLoadAdmission::RootBuild;
        Ok(key)
    }

    fn root_key(&self) -> RootRepositoryRouteKey {
        match self.admission {
            RootRepositoryLoadAdmission::Ordinary => {
                RootRepositoryRouteKey::new(self.workspace.dupe(), self.apparent_repo.clone())
            }
            RootRepositoryLoadAdmission::RootBuild => RootRepositoryRouteKey::for_root_build(
                self.workspace.dupe(),
                self.apparent_repo.clone(),
            ),
        }
        .expect("load-route keys already reject the root apparent name")
    }

    fn observed_root_key(&self) -> RootRepositoryRouteObservationKey {
        match self.admission {
            RootRepositoryLoadAdmission::Ordinary => RootRepositoryRouteObservationKey::new(
                self.workspace.dupe(),
                self.apparent_repo.clone(),
            ),
            RootRepositoryLoadAdmission::RootBuild => {
                RootRepositoryRouteObservationKey::for_root_build(
                    self.workspace.dupe(),
                    self.apparent_repo.clone(),
                )
            }
        }
        .expect("load-route keys already reject the root apparent name")
    }
}

impl HostRootRepositoryLoadRouteObservationKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Result<Self, String> {
        HostRootRepositoryLoadRouteKey::new(workspace, apparent_repo).map(Self)
    }

    pub fn for_root_build(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Result<Self, String> {
        HostRootRepositoryLoadRouteKey::for_root_build(workspace, apparent_repo).map(Self)
    }
}

impl fmt::Display for HostRootRepositoryLoadRouteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:@{}",
            match self.admission {
                RootRepositoryLoadAdmission::Ordinary => "root-repository-load-route",
                RootRepositoryLoadAdmission::RootBuild => "root-build-repository-load-route",
            },
            self.workspace,
            self.apparent_repo.as_str()
        )
    }
}

impl fmt::Display for HostRootRepositoryLoadRouteObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type LoadRouteResult = Arc<Result<HostRootRepositoryLoadRoute, HostRootRepositoryLoadRouteError>>;

#[doc(hidden)]
pub type HostRootRepositoryLoadRouteOutcome = SourcePreparationOutcome<LoadRouteResult>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostRootRepositoryLoadRoute {
    result: LoadRouteResult,
    observations: PathObservationEpoch,
}

impl ObservedHostRootRepositoryLoadRoute {
    pub fn result(&self) -> &LoadRouteResult {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum HostRootRepositoryLoadRouteObservationError {
    Root(RootRepositoryRouteObservationError),
    Mapping(HostCanonicalRepositoryApparentMappingObservationError),
    LoadRoute(HostCanonicalRepositoryLoadRouteObservationError),
    Merge(ObservedPathFrontierError),
}

impl HostRootRepositoryLoadRouteObservationError {
    pub fn selected_frontier(&self) -> HostSelectedObservationFrontier {
        match self {
            Self::Root(error) => error.clone().selected_frontier(),
            Self::Mapping(error) => error.selected_frontier(),
            Self::LoadRoute(error) => error.selected_frontier(),
            Self::Merge(error) => HostSelectedObservationFrontier::Path(error.clone()),
        }
    }
}

impl fmt::Display for HostRootRepositoryLoadRouteObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root repository load route observation failed: {self:?}")
    }
}

impl std::error::Error for HostRootRepositoryLoadRouteObservationError {}

#[derive(Clone, Copy)]
enum LoadRouteMode {
    Legacy,
    Observed,
}

struct FallbackState {
    original: RootRepositoryRouteError,
    observations: PathObservationEpoch,
}

type Driver = SourcePreparationOutcome<
    Result<(LoadRouteResult, PathObservationEpoch), HostRootRepositoryLoadRouteObservationError>,
>;

fn complete(
    key: &HostRootRepositoryLoadRouteKey,
    value: Result<HostRootRepositoryLoadRoute, HostRootRepositoryLoadRouteErrorKind>,
    observations: PathObservationEpoch,
) -> Driver {
    SourcePreparationOutcome::Complete(Ok((
        Arc::new(value.map_err(|kind| HostRootRepositoryLoadRouteError {
            apparent_repo: key.apparent_repo.clone(),
            kind,
        })),
        observations,
    )))
}

fn merge_observations(
    left: &PathObservationEpoch,
    right: &PathObservationEpoch,
) -> Result<PathObservationEpoch, HostRootRepositoryLoadRouteObservationError> {
    PathObservationEpoch::from_shared(
        left.observations()
            .iter()
            .chain(right.observations().iter())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(|error| {
        HostRootRepositoryLoadRouteObservationError::Merge(ObservedPathFrontierError::from(error))
    })
}

async fn root_predecessor(
    ctx: &mut DiceComputations<'_>,
    key: &HostRootRepositoryLoadRouteKey,
    mode: LoadRouteMode,
) -> Result<ControlFlow<Driver, FallbackState>, Driver> {
    let (result, observations) = match mode {
        LoadRouteMode::Legacy => match ctx.compute(&key.root_key()).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(result)) => {
                (result, PathObservationEpoch::empty())
            }
            Err(error) => {
                return Ok(ControlFlow::Break(complete(
                    key,
                    Err(HostRootRepositoryLoadRouteErrorKind::RootCompute(
                        error.to_string().into(),
                    )),
                    PathObservationEpoch::empty(),
                )));
            }
        },
        LoadRouteMode::Observed => match ctx.compute(&key.observed_root_key()).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(
                    HostRootRepositoryLoadRouteObservationError::Root(error),
                )));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().clone(), observed.observations().dupe())
            }
            Err(error) => {
                return Ok(ControlFlow::Break(complete(
                    key,
                    Err(HostRootRepositoryLoadRouteErrorKind::RootCompute(
                        error.to_string().into(),
                    )),
                    PathObservationEpoch::empty(),
                )));
            }
        },
    };
    match result.as_ref() {
        Ok(route) => Ok(ControlFlow::Break(complete(
            key,
            Ok(HostRootRepositoryLoadRoute {
                apparent_repo: key.apparent_repo.clone(),
                source: HostRepositorySourceRoute::root(route.clone()),
            }),
            observations,
        ))),
        Err(error) if error.is_generated_route_fallback() => {
            Ok(ControlFlow::Continue(FallbackState {
                original: error.clone(),
                observations,
            }))
        }
        Err(error) => Ok(ControlFlow::Break(complete(
            key,
            Err(HostRootRepositoryLoadRouteErrorKind::Root(error.clone())),
            observations,
        ))),
    }
}

async fn mapping_predecessor(
    ctx: &mut DiceComputations<'_>,
    key: &HostRootRepositoryLoadRouteKey,
    fallback: FallbackState,
    mode: LoadRouteMode,
) -> Result<ControlFlow<Driver, (FallbackState, CanonicalRepoName)>, Driver> {
    let mapping_key = HostCanonicalRepositoryApparentMappingKey::new(
        key.workspace.dupe(),
        CanonicalRepoName::root(),
        key.apparent_repo.clone(),
    );
    let (result, incoming) = match mode {
        LoadRouteMode::Legacy => match ctx.compute(&mapping_key).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(result)) => {
                (result, PathObservationEpoch::empty())
            }
            Err(error) => {
                return Ok(ControlFlow::Break(complete(
                    key,
                    Err(HostRootRepositoryLoadRouteErrorKind::MappingCompute(
                        error.to_string().into(),
                    )),
                    fallback.observations,
                )));
            }
        },
        LoadRouteMode::Observed => match ctx
            .compute(&HostCanonicalRepositoryApparentMappingObservationKey::new(
                key.workspace.dupe(),
                CanonicalRepoName::root(),
                key.apparent_repo.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(
                    HostRootRepositoryLoadRouteObservationError::Mapping(error),
                )));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().clone(), observed.observations().dupe())
            }
            Err(error) => {
                return Ok(ControlFlow::Break(complete(
                    key,
                    Err(HostRootRepositoryLoadRouteErrorKind::MappingCompute(
                        error.to_string().into(),
                    )),
                    fallback.observations,
                )));
            }
        },
    };
    let observations = merge_observations(&fallback.observations, &incoming)
        .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
    let fallback = FallbackState {
        original: fallback.original,
        observations,
    };
    match result.as_ref() {
        Err(error)
            if error.disposition()
                == HostCanonicalRepositoryApparentMappingErrorDisposition::Missing =>
        {
            let FallbackState {
                original,
                observations,
            } = fallback;
            Ok(ControlFlow::Break(complete(
                key,
                Err(HostRootRepositoryLoadRouteErrorKind::Root(original)),
                observations,
            )))
        }
        Err(error) => Ok(ControlFlow::Break(complete(
            key,
            Err(HostRootRepositoryLoadRouteErrorKind::Mapping(error.clone())),
            fallback.observations,
        ))),
        Ok(mapping) => match mapping.resolved_target().cloned() {
            Some(target) => Ok(ControlFlow::Continue((fallback, target))),
            None => Ok(ControlFlow::Break(complete(
                key,
                Err(HostRootRepositoryLoadRouteErrorKind::Projection(
                    "successful apparent mapping has no resolved target".into(),
                )),
                fallback.observations,
            ))),
        },
    }
}

async fn load_predecessor(
    ctx: &mut DiceComputations<'_>,
    key: &HostRootRepositoryLoadRouteKey,
    fallback: FallbackState,
    target: CanonicalRepoName,
    mode: LoadRouteMode,
) -> Driver {
    let (result, incoming) = match mode {
        LoadRouteMode::Legacy => match ctx
            .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                key.workspace.dupe(),
                target.clone(),
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
                return complete(
                    key,
                    Err(HostRootRepositoryLoadRouteErrorKind::LoadRouteCompute(
                        error.to_string().into(),
                    )),
                    fallback.observations,
                );
            }
        },
        LoadRouteMode::Observed => match ctx
            .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                key.workspace.dupe(),
                target,
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    HostRootRepositoryLoadRouteObservationError::LoadRoute(error),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().clone(), observed.observations().dupe())
            }
            Err(error) => {
                return complete(
                    key,
                    Err(HostRootRepositoryLoadRouteErrorKind::LoadRouteCompute(
                        error.to_string().into(),
                    )),
                    fallback.observations,
                );
            }
        },
    };
    let observations = match merge_observations(&fallback.observations, &incoming) {
        Ok(observations) => observations,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    match result.as_ref() {
        Err(error) => complete(
            key,
            Err(HostRootRepositoryLoadRouteErrorKind::LoadRoute(
                error.clone(),
            )),
            observations,
        ),
        Ok(route) if route.route().view().kind() == HostCanonicalRepositoryRouteKind::Generated => {
            complete(
                key,
                Ok(HostRootRepositoryLoadRoute {
                    apparent_repo: key.apparent_repo.clone(),
                    source: HostRepositorySourceRoute::canonical(route.input().clone()),
                }),
                observations,
            )
        }
        Ok(_) => complete(
            key,
            Err(HostRootRepositoryLoadRouteErrorKind::Root(
                fallback.original,
            )),
            observations,
        ),
    }
}

async fn compute_load_route(
    ctx: &mut DiceComputations<'_>,
    key: &HostRootRepositoryLoadRouteKey,
    mode: LoadRouteMode,
) -> Driver {
    let fallback = match root_predecessor(ctx, key, mode).await {
        Err(outcome) | Ok(ControlFlow::Break(outcome)) => return outcome,
        Ok(ControlFlow::Continue(fallback)) => fallback,
    };
    let (fallback, target) = match mapping_predecessor(ctx, key, fallback, mode).await {
        Err(outcome) | Ok(ControlFlow::Break(outcome)) => return outcome,
        Ok(ControlFlow::Continue(next)) => next,
    };
    load_predecessor(ctx, key, fallback, target, mode).await
}

#[async_trait]
impl Key for HostRootRepositoryLoadRouteKey {
    type Value = HostRootRepositoryLoadRouteOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_load_route(ctx, self, LoadRouteMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy root repository load route has no observed outer")
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
impl Key for HostRootRepositoryLoadRouteObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostRootRepositoryLoadRoute, HostRootRepositoryLoadRouteObservationError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_load_route(ctx, &self.0, LoadRouteMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostRootRepositoryLoadRoute {
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

#[cfg(test)]
#[path = "root_repository_load_route_tests.rs"]
mod tests;
