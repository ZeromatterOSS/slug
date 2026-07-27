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
use slug_workspace_v2::PathOutcome;

use crate::host_package::HostRootPackageLookup;
use crate::host_package::HostRootPackageLookupError;
use crate::host_package::HostRootPackageLookupKey;
use crate::repository_ignore::HostRepositoryIgnoreError;
use crate::repository_ignore::HostRepositoryIgnoreKey;

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

type HostRootPackageBoundaryCarrier =
    Arc<Result<HostRootPackageBoundary, HostRootPackageBoundaryError>>;
type HostRootPackageBoundaryOutcome = PathOutcome<HostRootPackageBoundaryCarrier>;

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host package-boundary DICE invariant failed: {error:?}"))
}

fn complete_success(boundary: HostRootPackageBoundary) -> HostRootPackageBoundaryOutcome {
    PathOutcome::Complete(Arc::new(Ok(boundary)))
}

fn complete_error(error: HostRootPackageBoundaryError) -> HostRootPackageBoundaryOutcome {
    PathOutcome::Complete(Arc::new(Err(error)))
}

#[async_trait]
impl Key for HostRootPackageBoundaryKey {
    type Value = PathOutcome<Arc<Result<HostRootPackageBoundary, HostRootPackageBoundaryError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let repository_ignore = match dice_invariant(
            ctx.compute(&HostRepositoryIgnoreKey::new(self.workspace.dupe()))
                .await,
        ) {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(value) => match value.as_ref() {
                Ok(value) => value.dupe(),
                Err(error) => {
                    return complete_error(HostRootPackageBoundaryError::repository_ignore(
                        error.clone(),
                    ));
                }
            },
        };
        if repository_ignore.matching_entry(&self.package).is_some() {
            return complete_success(HostRootPackageBoundary::ignored_directory());
        }

        match dice_invariant(
            ctx.compute(&HostRootPackageLookupKey::new(
                self.workspace.dupe(),
                self.package.clone(),
            ))
            .await,
        ) {
            PathOutcome::Need(need) => PathOutcome::Need(need),
            PathOutcome::Complete(value) => match value.as_ref() {
                Err(error) => {
                    complete_error(HostRootPackageBoundaryError::package_lookup(error.clone()))
                }
                Ok(HostRootPackageLookup::Package(package)) => complete_success(
                    HostRootPackageBoundary::package(package.package_root().dupe()),
                ),
                Ok(HostRootPackageLookup::Deleted) => {
                    complete_success(HostRootPackageBoundary::deleted_package())
                }
                Ok(HostRootPackageLookup::NoBuildFile)
                | Ok(HostRootPackageLookup::InvalidPackageName { .. }) => {
                    complete_success(HostRootPackageBoundary::no_package())
                }
            },
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
mod tests;
