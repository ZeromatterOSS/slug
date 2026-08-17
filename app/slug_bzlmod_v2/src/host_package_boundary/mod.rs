/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

//! The public, root-repository package-boundary projection for Host loading.
//!
//! This is deliberately a projection rather than another package lookup: the
//! private lookup owns marker ordering and policy, while this key exposes only
//! the decision a directory traversal needs to make.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathOutcome;

use crate::host_package::HostRootPackageLookup;
use crate::host_package::HostRootPackageLookupError;
use crate::host_package::HostRootPackageLookupKey;
use crate::host_package::HostRootPackageLookupObservationKey;
use crate::repository_ignore::HostRepositoryIgnoreError;
use crate::repository_ignore::HostRepositoryIgnoreKey;
use crate::repository_ignore::HostRepositoryIgnoreObservationKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum HostRootPackageBoundaryKind {
    /// The candidate is not a package and traversal continues.
    NoPackage,
    /// A deleted package is unsuccessful and traversal continues.
    DeletedPackage,
    /// Repository ignore policy stops traversal before package lookup.
    IgnoredDirectory,
    /// A package marker selected a package-path root and stops traversal.
    Package,
}

#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
enum HostRootPackageBoundaryState {
    NoPackage,
    DeletedPackage,
    IgnoredDirectory,
    Package {
        selected_package_root: NormalizedAbsolutePath,
    },
}

/// An opaque package-boundary result for one root-repository candidate.
#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct HostRootPackageBoundary {
    state: HostRootPackageBoundaryState,
}

impl HostRootPackageBoundary {
    fn no_package() -> Self {
        Self {
            state: HostRootPackageBoundaryState::NoPackage,
        }
    }

    fn deleted_package() -> Self {
        Self {
            state: HostRootPackageBoundaryState::DeletedPackage,
        }
    }

    fn ignored_directory() -> Self {
        Self {
            state: HostRootPackageBoundaryState::IgnoredDirectory,
        }
    }

    fn package(selected_package_root: NormalizedAbsolutePath) -> Self {
        Self {
            state: HostRootPackageBoundaryState::Package {
                selected_package_root,
            },
        }
    }

    pub fn kind(&self) -> HostRootPackageBoundaryKind {
        match self.state {
            HostRootPackageBoundaryState::NoPackage => HostRootPackageBoundaryKind::NoPackage,
            HostRootPackageBoundaryState::DeletedPackage => {
                HostRootPackageBoundaryKind::DeletedPackage
            }
            HostRootPackageBoundaryState::IgnoredDirectory => {
                HostRootPackageBoundaryKind::IgnoredDirectory
            }
            HostRootPackageBoundaryState::Package { .. } => HostRootPackageBoundaryKind::Package,
        }
    }

    pub fn selected_package_root(&self) -> Option<&NormalizedAbsolutePath> {
        match &self.state {
            HostRootPackageBoundaryState::Package {
                selected_package_root,
            } => Some(selected_package_root),
            HostRootPackageBoundaryState::NoPackage
            | HostRootPackageBoundaryState::DeletedPackage
            | HostRootPackageBoundaryState::IgnoredDirectory => None,
        }
    }
}

impl fmt::Debug for HostRootPackageBoundary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HostRootPackageBoundary")
    }
}

#[derive(Clone, PartialEq, Eq, Allocative)]
enum HostRootPackageBoundaryErrorInner {
    RepositoryIgnore(HostRepositoryIgnoreError),
    PackageLookup(HostRootPackageLookupError),
}

/// An opaque typed failure from the retained ignore or package-lookup owner.
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct HostRootPackageBoundaryError {
    inner: HostRootPackageBoundaryErrorInner,
}

impl HostRootPackageBoundaryError {
    fn repository_ignore(error: HostRepositoryIgnoreError) -> Self {
        Self {
            inner: HostRootPackageBoundaryErrorInner::RepositoryIgnore(error),
        }
    }

    fn package_lookup(error: HostRootPackageLookupError) -> Self {
        Self {
            inner: HostRootPackageBoundaryErrorInner::PackageLookup(error),
        }
    }
}

impl fmt::Debug for HostRootPackageBoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HostRootPackageBoundaryError")
    }
}

impl fmt::Display for HostRootPackageBoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            HostRootPackageBoundaryErrorInner::RepositoryIgnore(error) => error.fmt(f),
            HostRootPackageBoundaryErrorInner::PackageLookup(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for HostRootPackageBoundaryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner {
            HostRootPackageBoundaryErrorInner::RepositoryIgnore(error) => {
                std::error::Error::source(error)
            }
            HostRootPackageBoundaryErrorInner::PackageLookup(error) => {
                std::error::Error::source(error)
            }
        }
    }
}

/// The semantic root-repository package-boundary identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostRootPackageBoundaryKey {
    workspace: NormalizedAbsolutePath,
    package: PackagePath,
}

impl HostRootPackageBoundaryKey {
    pub fn new(workspace: NormalizedAbsolutePath, package: PackagePath) -> Self {
        Self { workspace, package }
    }
}

impl fmt::Display for HostRootPackageBoundaryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-root-package-boundary:{}//{}",
            self.workspace, self.package
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostRootPackageBoundary {
    result: Arc<Result<HostRootPackageBoundary, HostRootPackageBoundaryError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostRootPackageBoundary {
    pub fn result(&self) -> &Result<HostRootPackageBoundary, HostRootPackageBoundaryError> {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostRootPackageBoundaryObservationKey(HostRootPackageBoundaryKey);

impl HostRootPackageBoundaryObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath, package: PackagePath) -> Self {
        Self(HostRootPackageBoundaryKey::new(workspace, package))
    }
}

impl fmt::Display for HostRootPackageBoundaryObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type ObservedHostRootPackageBoundaryOutcome =
    PathOutcome<Result<ObservedHostRootPackageBoundary, ObservedPathFrontierError>>;

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host package-boundary DICE invariant failed: {error:?}"))
}

async fn compute_boundary(
    ctx: &mut DiceComputations<'_>,
    key: &HostRootPackageBoundaryKey,
    observed_mode: bool,
) -> ObservedHostRootPackageBoundaryOutcome {
    let complete = |result, observations| {
        PathOutcome::Complete(Ok(ObservedHostRootPackageBoundary {
            result: Arc::new(result),
            observations,
        }))
    };
    let (repository_ignore, mut observations) = if observed_mode {
        match dice_invariant(
            ctx.compute(&HostRepositoryIgnoreObservationKey::new(
                key.workspace.dupe(),
            ))
            .await,
        ) {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(Err(error)) => return PathOutcome::Complete(Err(error)),
            PathOutcome::Complete(Ok(observed)) => {
                (observed.result().clone(), observed.observations().dupe())
            }
        }
    } else {
        match dice_invariant(
            ctx.compute(&HostRepositoryIgnoreKey::new(key.workspace.dupe()))
                .await,
        ) {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(value) => (value.as_ref().clone(), PathObservationEpoch::empty()),
        }
    };
    let repository_ignore = match repository_ignore {
        Ok(value) => value,
        Err(error) => {
            return complete(
                Err(HostRootPackageBoundaryError::repository_ignore(error)),
                observations,
            );
        }
    };
    if repository_ignore.matching_entry(&key.package).is_some() {
        let ignored = HostRootPackageBoundary::ignored_directory();
        return complete(Ok(ignored), observations);
    }

    let lookup = if observed_mode {
        match dice_invariant(
            ctx.compute(&HostRootPackageLookupObservationKey::new(
                key.workspace.dupe(),
                key.package.clone(),
            ))
            .await,
        ) {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(Err(error)) => return PathOutcome::Complete(Err(error)),
            PathOutcome::Complete(Ok(observed)) => {
                observations = match PathObservationEpoch::from_shared(
                    observations
                        .observations()
                        .iter()
                        .map(|(demand, result)| (demand.dupe(), result.dupe()))
                        .chain(
                            observed
                                .observations()
                                .observations()
                                .iter()
                                .map(|(demand, result)| (demand.dupe(), result.dupe())),
                        ),
                ) {
                    Ok(observations) => observations,
                    Err(error) => return PathOutcome::Complete(Err(error.into())),
                };
                observed.result().clone()
            }
        }
    } else {
        match dice_invariant(
            ctx.compute(&HostRootPackageLookupKey::new(
                key.workspace.dupe(),
                key.package.clone(),
            ))
            .await,
        ) {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(value) => value.as_ref().clone(),
        }
    };
    complete(
        match lookup {
            Err(error) => Err(HostRootPackageBoundaryError::package_lookup(error)),
            Ok(HostRootPackageLookup::Package(package)) => Ok(HostRootPackageBoundary::package(
                package.package_root().dupe(),
            )),
            Ok(HostRootPackageLookup::Deleted) => Ok(HostRootPackageBoundary::deleted_package()),
            Ok(HostRootPackageLookup::NoBuildFile)
            | Ok(HostRootPackageLookup::InvalidPackageName { .. }) => {
                Ok(HostRootPackageBoundary::no_package())
            }
        },
        observations,
    )
}

#[async_trait]
impl Key for HostRootPackageBoundaryKey {
    type Value = PathOutcome<Arc<Result<HostRootPackageBoundary, HostRootPackageBoundaryError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match compute_boundary(ctx, self, false).await {
            PathOutcome::Need(need) => PathOutcome::Need(need),
            PathOutcome::Complete(Ok(observed)) => PathOutcome::Complete(observed.result),
            PathOutcome::Complete(Err(error)) => {
                panic!("legacy package-boundary frontier invariant failed: {error}")
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
impl Key for HostRootPackageBoundaryObservationKey {
    type Value = ObservedHostRootPackageBoundaryOutcome;

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
