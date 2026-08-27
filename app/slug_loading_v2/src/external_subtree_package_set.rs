/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

//! Loading-owned recursive package discovery for authenticated repositories.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use slug_bzlmod_v2::HostCanonicalRepositorySourceInput;
use slug_bzlmod_v2::HostExternalPackageBoundary;
use slug_bzlmod_v2::HostExternalPackageBoundaryError;
use slug_bzlmod_v2::HostExternalPackageBoundaryKey;
use slug_bzlmod_v2::HostExternalPackageBoundaryKind;
use slug_bzlmod_v2::HostExternalPackageBoundaryObservationKey;
use slug_bzlmod_v2::HostRepositoryDirectoryListing;
use slug_bzlmod_v2::HostRepositoryDirectoryListingError;
use slug_bzlmod_v2::HostRepositoryDirectoryListingKey;
use slug_bzlmod_v2::HostRepositoryDirectoryListingObservationKey;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryListing;
use slug_workspace_v2::PathObservationEpoch;

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ExternalSubtreePackageSet {
    packages: Arc<[CompactString]>,
}

impl ExternalSubtreePackageSet {
    pub fn packages(&self) -> &Arc<[CompactString]> {
        &self.packages
    }
}

#[derive(Clone, Eq, PartialEq, Allocative)]
pub enum ExternalSubtreePackageSetErrorKind {
    Boundary {
        package: PackagePath,
        error: HostExternalPackageBoundaryError,
    },
    Listing {
        package: PackagePath,
        error: HostRepositoryDirectoryListingError,
    },
    MissingPackageDirectory {
        package: PackagePath,
    },
    UnsupportedEntryKind {
        parent: PackagePath,
        kind: PathDirectoryEntryKind,
    },
    NonUnicodeDirectoryName {
        parent: PackagePath,
    },
    InvalidChildPackage {
        parent: PackagePath,
    },
}

#[derive(Clone, Eq, PartialEq, Allocative)]
pub struct ExternalSubtreePackageSetError {
    kind: ExternalSubtreePackageSetErrorKind,
}

impl ExternalSubtreePackageSetError {
    pub fn kind(&self) -> &ExternalSubtreePackageSetErrorKind {
        &self.kind
    }
}

impl fmt::Debug for ExternalSubtreePackageSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ExternalSubtreePackageSetErrorKind::Boundary { package, error } => f
                .debug_struct("ExternalSubtreePackageSetError")
                .field("kind", &"boundary")
                .field("package", package)
                .field("error", error)
                .finish(),
            ExternalSubtreePackageSetErrorKind::Listing { package, error } => f
                .debug_struct("ExternalSubtreePackageSetError")
                .field("kind", &"listing")
                .field("package", package)
                .field("error", error)
                .finish(),
            ExternalSubtreePackageSetErrorKind::MissingPackageDirectory { package } => f
                .debug_struct("ExternalSubtreePackageSetError")
                .field("kind", &"missing-package-directory")
                .field("package", package)
                .finish(),
            ExternalSubtreePackageSetErrorKind::UnsupportedEntryKind { parent, kind } => f
                .debug_struct("ExternalSubtreePackageSetError")
                .field("kind", &"unsupported-entry-kind")
                .field("parent", parent)
                .field("entry_kind", kind)
                .finish(),
            ExternalSubtreePackageSetErrorKind::NonUnicodeDirectoryName { parent } => f
                .debug_struct("ExternalSubtreePackageSetError")
                .field("kind", &"non-unicode-directory-name")
                .field("parent", parent)
                .finish(),
            ExternalSubtreePackageSetErrorKind::InvalidChildPackage { parent } => f
                .debug_struct("ExternalSubtreePackageSetError")
                .field("kind", &"invalid-child-package")
                .field("parent", parent)
                .finish(),
        }
    }
}

impl fmt::Display for ExternalSubtreePackageSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for ExternalSubtreePackageSetError {}

type PackageSetValue = Arc<Result<ExternalSubtreePackageSet, ExternalSubtreePackageSetError>>;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct ExternalSubtreePackageSetKey {
    route: HostRepositorySourceRoute,
    prefix: PackagePath,
}

impl ExternalSubtreePackageSetKey {
    pub fn new(route: RootRepositoryRoute, prefix: PackagePath) -> Self {
        Self {
            route: HostRepositorySourceRoute::root(route),
            prefix,
        }
    }

    pub fn new_canonical(input: HostCanonicalRepositorySourceInput, prefix: PackagePath) -> Self {
        Self {
            route: HostRepositorySourceRoute::canonical(input),
            prefix,
        }
    }
}

impl fmt::Display for ExternalSubtreePackageSetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "external-subtree-package-set:{}//{}",
            self.route.canonical_repo(),
            self.prefix
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct ExternalSubtreePackageSetObservationKey(ExternalSubtreePackageSetKey);

impl ExternalSubtreePackageSetObservationKey {
    pub fn new(route: RootRepositoryRoute, prefix: PackagePath) -> Self {
        Self(ExternalSubtreePackageSetKey::new(route, prefix))
    }

    pub fn new_canonical(input: HostCanonicalRepositorySourceInput, prefix: PackagePath) -> Self {
        Self(ExternalSubtreePackageSetKey::new_canonical(input, prefix))
    }
}

impl fmt::Display for ExternalSubtreePackageSetObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative, Dupe)]
pub struct ObservedExternalSubtreePackageSet {
    result: PackageSetValue,
    observations: PathObservationEpoch,
}

impl ObservedExternalSubtreePackageSet {
    pub fn result(&self) -> &PackageSetValue {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ObservationMode {
    Legacy,
    Observed,
}

type ObservedPackageSetValue =
    SourcePreparationOutcome<Result<ObservedExternalSubtreePackageSet, ObservedPathFrontierError>>;
type BoundaryOutcome = SourcePreparationOutcome<
    Result<
        (
            Result<HostExternalPackageBoundary, HostExternalPackageBoundaryError>,
            PathObservationEpoch,
        ),
        ObservedPathFrontierError,
    >,
>;
type ListingOutcome = SourcePreparationOutcome<
    Result<
        (
            Result<HostRepositoryDirectoryListing, HostRepositoryDirectoryListingError>,
            PathObservationEpoch,
        ),
        ObservedPathFrontierError,
    >,
>;

fn merge_observations(
    current: &PathObservationEpoch,
    incoming: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        current
            .observations()
            .iter()
            .chain(incoming.observations())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(ObservedPathFrontierError::from)
}

fn complete(
    result: Result<ExternalSubtreePackageSet, ExternalSubtreePackageSetError>,
    observations: PathObservationEpoch,
) -> ObservedPackageSetValue {
    SourcePreparationOutcome::Complete(Ok(ObservedExternalSubtreePackageSet {
        result: Arc::new(result),
        observations,
    }))
}

async fn boundary(
    ctx: &mut DiceComputations<'_>,
    route: &HostRepositorySourceRoute,
    package: &PackagePath,
    mode: ObservationMode,
) -> BoundaryOutcome {
    match mode {
        ObservationMode::Legacy => match ctx
            .compute(&match route {
                HostRepositorySourceRoute::Root(route) => {
                    HostExternalPackageBoundaryKey::new(route.clone(), package.clone())
                }
                HostRepositorySourceRoute::Canonical(input) => {
                    HostExternalPackageBoundaryKey::new_canonical(input.clone(), package.clone())
                }
            })
            .await
            .expect("external package boundary DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(result) => SourcePreparationOutcome::Complete(Ok((
                result.as_ref().clone(),
                PathObservationEpoch::empty(),
            ))),
        },
        ObservationMode::Observed => match ctx
            .compute(&match route {
                HostRepositorySourceRoute::Root(route) => {
                    HostExternalPackageBoundaryObservationKey::new(route.clone(), package.clone())
                }
                HostRepositorySourceRoute::Canonical(input) => {
                    HostExternalPackageBoundaryObservationKey::new_canonical(
                        input.clone(),
                        package.clone(),
                    )
                }
            })
            .await
            .expect("observed external package boundary DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok(observed)) => SourcePreparationOutcome::Complete(
                Ok((observed.result().clone(), observed.observations().dupe())),
            ),
        },
    }
}

async fn listing(
    ctx: &mut DiceComputations<'_>,
    route: &HostRepositorySourceRoute,
    package: &PackagePath,
    mode: ObservationMode,
) -> ListingOutcome {
    match mode {
        ObservationMode::Legacy => match ctx
            .compute(&match route {
                HostRepositorySourceRoute::Root(route) => {
                    HostRepositoryDirectoryListingKey::new(route.clone(), package.clone())
                }
                HostRepositorySourceRoute::Canonical(input) => {
                    HostRepositoryDirectoryListingKey::new_canonical(input.clone(), package.clone())
                }
            })
            .await
            .expect("external repository listing DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(result) => {
                SourcePreparationOutcome::Complete(Ok((result, PathObservationEpoch::empty())))
            }
        },
        ObservationMode::Observed => match ctx
            .compute(&match route {
                HostRepositorySourceRoute::Root(route) => {
                    HostRepositoryDirectoryListingObservationKey::new(
                        route.clone(),
                        package.clone(),
                    )
                }
                HostRepositorySourceRoute::Canonical(input) => {
                    HostRepositoryDirectoryListingObservationKey::new_canonical(
                        input.clone(),
                        package.clone(),
                    )
                }
            })
            .await
            .expect("observed external repository listing DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok(observed)) => {
                SourcePreparationOutcome::Complete(Ok((
                    observed.result().as_ref().clone(),
                    observed.observations().dupe(),
                )))
            }
        },
    }
}

fn child_packages(
    parent: &PackagePath,
    entries: &PathDirectoryEntries,
) -> Result<Vec<PackagePath>, ExternalSubtreePackageSetError> {
    let mut children = Vec::new();
    for entry in entries.entries() {
        match entry.kind() {
            PathDirectoryEntryKind::File => continue,
            PathDirectoryEntryKind::Symlink | PathDirectoryEntryKind::Unknown => {
                return Err(ExternalSubtreePackageSetError {
                    kind: ExternalSubtreePackageSetErrorKind::UnsupportedEntryKind {
                        parent: parent.clone(),
                        kind: entry.kind(),
                    },
                });
            }
            PathDirectoryEntryKind::Directory => {}
        }
        let Some(name) = entry.name().as_os_str().to_str() else {
            return Err(ExternalSubtreePackageSetError {
                kind: ExternalSubtreePackageSetErrorKind::NonUnicodeDirectoryName {
                    parent: parent.clone(),
                },
            });
        };
        let child = if parent.as_str().is_empty() {
            name.to_owned()
        } else {
            format!("{}/{name}", parent.as_str())
        };
        let child = PackagePath::parse(&child).map_err(|_| ExternalSubtreePackageSetError {
            kind: ExternalSubtreePackageSetErrorKind::InvalidChildPackage {
                parent: parent.clone(),
            },
        })?;
        children.push(child);
    }
    children.sort_unstable_by(|left, right| left.as_str().cmp(right.as_str()));
    children.dedup();
    Ok(children)
}

fn listing_entries(
    package: &PackagePath,
    boundary: HostExternalPackageBoundaryKind,
    listing: HostRepositoryDirectoryListing,
) -> Result<Option<PathDirectoryEntries>, ExternalSubtreePackageSetError> {
    match listing {
        PathDirectoryListing::Missing => {
            if boundary == HostExternalPackageBoundaryKind::Package {
                Err(ExternalSubtreePackageSetError {
                    kind: ExternalSubtreePackageSetErrorKind::MissingPackageDirectory {
                        package: package.clone(),
                    },
                })
            } else {
                Ok(None)
            }
        }
        PathDirectoryListing::Present(entries) => Ok(Some(entries)),
    }
}

async fn compute_external_subtree_packages(
    ctx: &mut DiceComputations<'_>,
    key: &ExternalSubtreePackageSetKey,
    mode: ObservationMode,
) -> ObservedPackageSetValue {
    let mut observations = PathObservationEpoch::empty();
    let mut pending = vec![key.prefix.clone()];
    let mut packages = Vec::new();
    while let Some(package) = pending.pop() {
        let boundary = match boundary(ctx, &key.route, &package, mode).await {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok((result, epoch))) => {
                observations = match merge_observations(&observations, &epoch) {
                    Ok(observations) => observations,
                    Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
                };
                match result {
                    Ok(value) => value,
                    Err(error) => {
                        return complete(
                            Err(ExternalSubtreePackageSetError {
                                kind: ExternalSubtreePackageSetErrorKind::Boundary {
                                    package,
                                    error,
                                },
                            }),
                            observations,
                        );
                    }
                }
            }
        };
        if boundary.kind() == HostExternalPackageBoundaryKind::IgnoredDirectory {
            continue;
        }

        let listing = match listing(ctx, &key.route, &package, mode).await {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok((result, epoch))) => {
                observations = match merge_observations(&observations, &epoch) {
                    Ok(observations) => observations,
                    Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
                };
                match result {
                    Ok(value) => value,
                    Err(error) => {
                        return complete(
                            Err(ExternalSubtreePackageSetError {
                                kind: ExternalSubtreePackageSetErrorKind::Listing {
                                    package,
                                    error,
                                },
                            }),
                            observations,
                        );
                    }
                }
            }
        };
        let entries = match listing_entries(&package, boundary.kind(), listing) {
            Ok(Some(entries)) => entries,
            Ok(None) => continue,
            Err(error) => return complete(Err(error), observations),
        };
        if boundary.kind() == HostExternalPackageBoundaryKind::Package {
            packages.push(CompactString::new(package.as_str()));
        }
        let children = match child_packages(&package, &entries) {
            Ok(children) => children,
            Err(error) => return complete(Err(error), observations),
        };
        pending.extend(children.into_iter().rev());
    }
    packages.sort_unstable();
    packages.dedup();
    complete(
        Ok(ExternalSubtreePackageSet {
            packages: packages.into(),
        }),
        observations,
    )
}

#[async_trait]
impl Key for ExternalSubtreePackageSetKey {
    type Value = SourcePreparationOutcome<PackageSetValue>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match compute_external_subtree_packages(ctx, self, ObservationMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok(value)) => {
                SourcePreparationOutcome::Complete(value.result)
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy external subtree produced observed outer error: {error}")
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
impl Key for ExternalSubtreePackageSetObservationKey {
    type Value = ObservedPackageSetValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        compute_external_subtree_packages(ctx, &self.0, ObservationMode::Observed).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
#[path = "external_subtree_package_set_tests.rs"]
mod tests;
