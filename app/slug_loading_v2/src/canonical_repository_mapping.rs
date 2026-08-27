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
use slug_bzlmod_v2::HostRootRepositoryMapping;
use slug_bzlmod_v2::HostRootRepositoryMappingError;
use slug_bzlmod_v2::HostRootRepositoryMappingKey;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationError;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;

use crate::canonical_repository_route::HostCanonicalRepositoryRouteError;
use crate::canonical_repository_route::HostCanonicalRepositoryRouteKey;
use crate::canonical_repository_route::HostCanonicalRepositoryRouteObservationError;
use crate::canonical_repository_route::HostCanonicalRepositoryRouteObservationKey;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositoryApparentMapping {
    pub(super) predecessor: ApparentMappingPredecessor,
    pub(super) apparent_repo: ApparentRepoName,
}

impl HostCanonicalRepositoryApparentMapping {
    pub fn resolved_target(&self) -> Option<&CanonicalRepoName> {
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
pub(super) enum ApparentMappingPredecessor {
    Root(HostRootRepositoryMapping),
    Canonical(HostCanonicalRepositoryRoute),
}

impl ApparentMappingPredecessor {
    fn contexts(&self) -> Option<(&CanonicalRepoName, &CanonicalRepoName)> {
        match self {
            Self::Root(predecessor) => predecessor
                .view()
                .map(|view| (view.canonical_repo(), view.mapping_context())),
            Self::Canonical(predecessor) => {
                let view = predecessor.view();
                Some((view.canonical_repo(), view.mapping_context()))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) enum HostCanonicalRepositoryApparentMappingErrorKind {
    RootApparent,
    RootMapping(HostRootRepositoryMappingError),
    RootMappingCompute(Arc<str>),
    Route(HostCanonicalRepositoryRouteError),
    RouteCompute(Arc<str>),
    ContextMismatch {
        predecessor: ApparentMappingPredecessor,
    },
    Missing {
        predecessor: ApparentMappingPredecessor,
    },
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCanonicalRepositoryApparentMappingErrorDisposition {
    Missing,
    ContextMismatch,
    Other,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositoryApparentMappingError {
    pub(super) context_repo: CanonicalRepoName,
    pub(super) apparent_repo: ApparentRepoName,
    pub(super) kind: HostCanonicalRepositoryApparentMappingErrorKind,
}

impl HostCanonicalRepositoryApparentMappingError {
    pub fn disposition(&self) -> HostCanonicalRepositoryApparentMappingErrorDisposition {
        match self.kind {
            HostCanonicalRepositoryApparentMappingErrorKind::Missing { .. } => {
                HostCanonicalRepositoryApparentMappingErrorDisposition::Missing
            }
            HostCanonicalRepositoryApparentMappingErrorKind::ContextMismatch { .. } => {
                HostCanonicalRepositoryApparentMappingErrorDisposition::ContextMismatch
            }
            _ => HostCanonicalRepositoryApparentMappingErrorDisposition::Other,
        }
    }
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

#[doc(hidden)]
pub type HostCanonicalRepositoryApparentMappingOutcome = SourcePreparationOutcome<
    Arc<
        Result<HostCanonicalRepositoryApparentMapping, HostCanonicalRepositoryApparentMappingError>,
    >,
>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostCanonicalRepositoryApparentMappingKey {
    workspace: NormalizedAbsolutePath,
    context_repo: CanonicalRepoName,
    apparent_repo: ApparentRepoName,
}

impl HostCanonicalRepositoryApparentMappingKey {
    pub fn new(
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

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostCanonicalRepositoryApparentMappingObservationKey(
    pub(super) HostCanonicalRepositoryApparentMappingKey,
);

impl HostCanonicalRepositoryApparentMappingObservationKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        context_repo: CanonicalRepoName,
        apparent_repo: ApparentRepoName,
    ) -> Self {
        Self(HostCanonicalRepositoryApparentMappingKey::new(
            workspace,
            context_repo,
            apparent_repo,
        ))
    }
}

impl fmt::Display for HostCanonicalRepositoryApparentMappingObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type CanonicalRepositoryApparentMappingResult = Arc<
    Result<HostCanonicalRepositoryApparentMapping, HostCanonicalRepositoryApparentMappingError>,
>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostCanonicalRepositoryApparentMapping {
    pub(super) result: CanonicalRepositoryApparentMappingResult,
    pub(super) observations: PathObservationEpoch,
}

impl ObservedHostCanonicalRepositoryApparentMapping {
    pub fn result(&self) -> &CanonicalRepositoryApparentMappingResult {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) enum CanonicalRepositoryApparentMappingObservationError {
    RootMapping(HostRootRepositoryMappingObservationError),
    Route(HostCanonicalRepositoryRouteObservationError),
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct HostCanonicalRepositoryApparentMappingObservationError(
    pub(super) CanonicalRepositoryApparentMappingObservationError,
);

#[derive(Clone, Copy)]
enum CanonicalRepositoryApparentMappingMode {
    Legacy,
    Observed,
}

pub(super) enum CanonicalRepositoryApparentMappingChildOutcome {
    Need(SourcePreparationNeeds),
    Outer(CanonicalRepositoryApparentMappingObservationError),
    Complete {
        result: Result<ApparentMappingPredecessor, HostCanonicalRepositoryApparentMappingErrorKind>,
        observations: PathObservationEpoch,
    },
}

type CanonicalRepositoryApparentMappingDriverOutcome = SourcePreparationOutcome<
    Result<
        (
            CanonicalRepositoryApparentMappingResult,
            PathObservationEpoch,
        ),
        CanonicalRepositoryApparentMappingObservationError,
    >,
>;

fn complete_mapping_driver(
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
async fn root_mapping_child(
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
async fn canonical_route_child(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositoryApparentMappingKey,
    mode: CanonicalRepositoryApparentMappingMode,
) -> CanonicalRepositoryApparentMappingChildOutcome {
    let (result, observations) = match mode {
        CanonicalRepositoryApparentMappingMode::Legacy => match ctx.compute(&HostCanonicalRepositoryRouteKey::new(key.workspace.clone(), key.context_repo.clone())).await {
            Err(error) => return CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Err(HostCanonicalRepositoryApparentMappingErrorKind::RouteCompute(error.to_string().into())), observations: PathObservationEpoch::empty() },
            Ok(SourcePreparationOutcome::Need(need)) => return CanonicalRepositoryApparentMappingChildOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
        },
        CanonicalRepositoryApparentMappingMode::Observed => match ctx.compute(&HostCanonicalRepositoryRouteObservationKey::new(key.workspace.clone(), key.context_repo.clone())).await {
            Err(error) => return CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Err(HostCanonicalRepositoryApparentMappingErrorKind::RouteCompute(error.to_string().into())), observations: PathObservationEpoch::empty() },
            Ok(SourcePreparationOutcome::Need(need)) => return CanonicalRepositoryApparentMappingChildOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return CanonicalRepositoryApparentMappingChildOutcome::Outer(CanonicalRepositoryApparentMappingObservationError::Route(error)),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().clone(), observed.observations().clone()),
        },
    };
    let result = match result.as_ref() {
        Ok(value) => Ok(ApparentMappingPredecessor::Canonical(value.clone())),
        Err(error) => Err(HostCanonicalRepositoryApparentMappingErrorKind::Route(error.clone())),
    };
    CanonicalRepositoryApparentMappingChildOutcome::Complete { result, observations }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MappingLookupStatus {
    ContextMismatch,
    Missing,
    Found,
}

pub(super) fn mapping_lookup_status(
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

pub(super) fn finish_mapping(
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
        } => return complete_mapping_driver(key, Err(kind), observations),
        CanonicalRepositoryApparentMappingChildOutcome::Complete {
            result: Ok(predecessor),
            observations,
        } => (predecessor, observations),
    };
    let Some((canonical_repo, mapping_context)) = predecessor.contexts() else {
        return complete_mapping_driver(
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
    complete_mapping_driver(key, value, observations)
}

async fn compute_mapping(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositoryApparentMappingKey,
    mode: CanonicalRepositoryApparentMappingMode,
) -> CanonicalRepositoryApparentMappingDriverOutcome {
    if key.apparent_repo.is_root() && !key.context_repo.is_root() {
        return complete_mapping_driver(
            key,
            Err(HostCanonicalRepositoryApparentMappingErrorKind::RootApparent),
            PathObservationEpoch::empty(),
        );
    }
    let child = if key.context_repo.is_root() {
        root_mapping_child(ctx, key, mode).await
    } else {
        canonical_route_child(ctx, key, mode).await
    };
    finish_mapping(key, child)
}

fn project_legacy(
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
        project_legacy(
            compute_mapping(ctx, self, CanonicalRepositoryApparentMappingMode::Legacy).await,
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
        match compute_mapping(
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
