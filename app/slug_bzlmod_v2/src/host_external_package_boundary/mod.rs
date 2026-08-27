/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

//! Public semantic projection of the private external-package point lookup.
//!
//! This owner deliberately adds no policy, marker or source dependency. It
//! exposes only the decision a later recursive package producer needs.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use crate::HostCanonicalRepositorySourceInput;
use crate::RootRepositoryRoute;
use crate::SourcePreparationOutcome;
use crate::host_package::ExternalRepositoryPackageLookup;
use crate::host_package::ExternalRepositoryPackageLookupError;
use crate::host_package::ExternalRepositoryPackageLookupKey;
use crate::host_package::ExternalRepositoryPackageLookupObservationKey;
use crate::host_package::HostBuildFileName;
use crate::source_preparation::HostRepositorySourceRoute;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum HostExternalPackageBoundaryKind {
    InvalidPackageName,
    DeletedPackage,
    IgnoredDirectory,
    Package,
    NoPackage,
}

#[derive(Clone, Copy, PartialEq, Eq, Allocative)]
enum HostExternalPackageBoundaryState {
    InvalidPackageName,
    DeletedPackage,
    IgnoredDirectory,
    Package(HostBuildFileName),
    NoPackage,
}

/// The repository-relative package decision exposed to later traversal.
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct HostExternalPackageBoundary {
    state: HostExternalPackageBoundaryState,
}

impl HostExternalPackageBoundary {
    fn from_lookup(value: &ExternalRepositoryPackageLookup) -> Self {
        let state = match value {
            ExternalRepositoryPackageLookup::InvalidPackageName { .. } => {
                HostExternalPackageBoundaryState::InvalidPackageName
            }
            ExternalRepositoryPackageLookup::Deleted => {
                HostExternalPackageBoundaryState::DeletedPackage
            }
            ExternalRepositoryPackageLookup::IgnoredDirectory => {
                HostExternalPackageBoundaryState::IgnoredDirectory
            }
            ExternalRepositoryPackageLookup::Package(marker) => {
                HostExternalPackageBoundaryState::Package(*marker)
            }
            ExternalRepositoryPackageLookup::NoBuildFile => {
                HostExternalPackageBoundaryState::NoPackage
            }
        };
        Self { state }
    }

    pub fn kind(&self) -> HostExternalPackageBoundaryKind {
        match self.state {
            HostExternalPackageBoundaryState::InvalidPackageName => {
                HostExternalPackageBoundaryKind::InvalidPackageName
            }
            HostExternalPackageBoundaryState::DeletedPackage => {
                HostExternalPackageBoundaryKind::DeletedPackage
            }
            HostExternalPackageBoundaryState::IgnoredDirectory => {
                HostExternalPackageBoundaryKind::IgnoredDirectory
            }
            HostExternalPackageBoundaryState::Package(_) => {
                HostExternalPackageBoundaryKind::Package
            }
            HostExternalPackageBoundaryState::NoPackage => {
                HostExternalPackageBoundaryKind::NoPackage
            }
        }
    }

    pub fn selected_build_file_name(&self) -> Option<&'static str> {
        match self.state {
            HostExternalPackageBoundaryState::Package(marker) => Some(marker.as_str()),
            HostExternalPackageBoundaryState::InvalidPackageName
            | HostExternalPackageBoundaryState::DeletedPackage
            | HostExternalPackageBoundaryState::IgnoredDirectory
            | HostExternalPackageBoundaryState::NoPackage => None,
        }
    }
}

impl fmt::Debug for HostExternalPackageBoundary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostExternalPackageBoundary")
            .field("kind", &self.kind())
            .field("selected_build_file_name", &self.selected_build_file_name())
            .finish()
    }
}

/// Payload-free semantic class of a private point-lookup failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum HostExternalPackageBoundaryError {
    PolicyInput,
    RepositoryIgnore,
    RepositoryListing,
    SourcePath,
}

impl HostExternalPackageBoundaryError {
    fn from_lookup(error: &ExternalRepositoryPackageLookupError) -> Self {
        match error {
            ExternalRepositoryPackageLookupError::PolicyInput(_) => Self::PolicyInput,
            ExternalRepositoryPackageLookupError::RepositoryIgnore(_) => Self::RepositoryIgnore,
            ExternalRepositoryPackageLookupError::RepositoryListing(_) => Self::RepositoryListing,
            ExternalRepositoryPackageLookupError::Path(_) => Self::SourcePath,
        }
    }
}

impl fmt::Display for HostExternalPackageBoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::PolicyInput => "external package policy input failed",
            Self::RepositoryIgnore => "external repository ignore lookup failed",
            Self::RepositoryListing => "external repository listing failed",
            Self::SourcePath => "external package marker lookup failed",
        };
        f.write_str(message)
    }
}

impl std::error::Error for HostExternalPackageBoundaryError {}

/// Complete authenticated identity for one external package candidate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostExternalPackageBoundaryKey {
    route: HostRepositorySourceRoute,
    package: PackagePath,
}

impl HostExternalPackageBoundaryKey {
    pub fn new(route: RootRepositoryRoute, package: PackagePath) -> Self {
        Self {
            route: HostRepositorySourceRoute::root(route),
            package,
        }
    }

    pub fn new_canonical(input: HostCanonicalRepositorySourceInput, package: PackagePath) -> Self {
        Self {
            route: HostRepositorySourceRoute::canonical(input),
            package,
        }
    }

    fn lookup_key(&self) -> ExternalRepositoryPackageLookupKey {
        ExternalRepositoryPackageLookupKey::from_source_route(
            self.route.clone(),
            PackageIdentifier::new(self.route.canonical_repo().clone(), self.package.clone()),
        )
        .expect("boundary route and package repository identities agree")
    }

    fn lookup_observation_key(&self) -> ExternalRepositoryPackageLookupObservationKey {
        ExternalRepositoryPackageLookupObservationKey::from_source_route(
            self.route.clone(),
            PackageIdentifier::new(self.route.canonical_repo().clone(), self.package.clone()),
        )
        .expect("boundary route and package repository identities agree")
    }
}

impl fmt::Display for HostExternalPackageBoundaryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-external-package-boundary:{:?}//{}",
            self.route.canonical_repo(),
            self.package
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostExternalPackageBoundary {
    result: Arc<Result<HostExternalPackageBoundary, HostExternalPackageBoundaryError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostExternalPackageBoundary {
    pub fn result(&self) -> &Result<HostExternalPackageBoundary, HostExternalPackageBoundaryError> {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostExternalPackageBoundaryObservationKey(HostExternalPackageBoundaryKey);

impl HostExternalPackageBoundaryObservationKey {
    pub fn new(route: RootRepositoryRoute, package: PackagePath) -> Self {
        Self(HostExternalPackageBoundaryKey::new(route, package))
    }

    pub fn new_canonical(input: HostCanonicalRepositorySourceInput, package: PackagePath) -> Self {
        Self(HostExternalPackageBoundaryKey::new_canonical(
            input, package,
        ))
    }
}

impl fmt::Display for HostExternalPackageBoundaryObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type BoundaryResult = Arc<Result<HostExternalPackageBoundary, HostExternalPackageBoundaryError>>;
type ObservedBoundaryOutcome = SourcePreparationOutcome<
    Result<ObservedHostExternalPackageBoundary, ObservedPathFrontierError>,
>;

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| {
        panic!("external package-boundary DICE invariant failed: {error:?}")
    })
}

fn project_lookup(
    result: &Result<ExternalRepositoryPackageLookup, ExternalRepositoryPackageLookupError>,
) -> Result<HostExternalPackageBoundary, HostExternalPackageBoundaryError> {
    match result {
        Ok(value) => Ok(HostExternalPackageBoundary::from_lookup(value)),
        Err(error) => Err(HostExternalPackageBoundaryError::from_lookup(error)),
    }
}

async fn compute_boundary(
    ctx: &mut DiceComputations<'_>,
    key: &HostExternalPackageBoundaryKey,
    observed_mode: bool,
) -> ObservedBoundaryOutcome {
    if observed_mode {
        let observed = match dice_invariant(ctx.compute(&key.lookup_observation_key()).await) {
            SourcePreparationOutcome::Need(need) => {
                return SourcePreparationOutcome::Need(need);
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok(observed)) => observed,
        };
        return SourcePreparationOutcome::Complete(Ok(ObservedHostExternalPackageBoundary {
            result: Arc::new(project_lookup(observed.result().as_ref())),
            observations: observed.observations().dupe(),
        }));
    }

    match dice_invariant(ctx.compute(&key.lookup_key()).await) {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(result) => {
            SourcePreparationOutcome::Complete(Ok(ObservedHostExternalPackageBoundary {
                result: Arc::new(project_lookup(result.as_ref())),
                observations: PathObservationEpoch::empty(),
            }))
        }
    }
}

#[async_trait]
impl Key for HostExternalPackageBoundaryKey {
    type Value = SourcePreparationOutcome<BoundaryResult>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match compute_boundary(ctx, self, false).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok(observed)) => {
                SourcePreparationOutcome::Complete(observed.result)
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy external package-boundary frontier invariant failed: {error}")
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
impl Key for HostExternalPackageBoundaryObservationKey {
    type Value = ObservedBoundaryOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        compute_boundary(ctx, &self.0, true).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
mod tests;
