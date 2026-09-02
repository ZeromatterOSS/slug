/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostExternalPackageBoundary;
use slug_bzlmod_v2::HostExternalPackageBoundaryError;
use slug_bzlmod_v2::HostExternalPackageBoundaryKey;
use slug_bzlmod_v2::HostExternalPackageBoundaryKind;
use slug_bzlmod_v2::HostExternalPackageBoundaryObservationKey;
use slug_bzlmod_v2::HostRepositoryDirectoryListingError;
use slug_bzlmod_v2::HostRepositoryDirectoryListingKey;
use slug_bzlmod_v2::HostRepositoryDirectoryListingObservationKey;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::HostRootPackageBoundary;
use slug_bzlmod_v2::HostRootPackageBoundaryError;
use slug_bzlmod_v2::HostRootPackageBoundaryKey;
use slug_bzlmod_v2::HostRootPackageBoundaryKind;
use slug_bzlmod_v2::HostRootPackageBoundaryObservationKey;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryListing;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathOutcome;
use starlark_map::small_set::SmallSet;

use super::HexBytes;
use super::HostGlobSegmentCandidateKind;
use super::HostGlobSegmentCandidates;
use super::HostGlobSegmentCandidatesKey;
use super::HostGlobSegmentCandidatesObservationKey;
use super::HostGlobSegmentError;
use super::HostGlobSegmentPattern;
use super::dice_invariant;
use super::logical_child;
use super::union_observation_epochs;
use crate::glob::GlobPattern;
use crate::glob::glob_segment_matches;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(super) enum HostGlobTraversalOperation {
    Files,
    FilesAndDirs,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum HostGlobBoundaryScope {
    Root,
    External(HostRepositorySourceRoute),
    BuiltinCatalog(HostRepositorySourceRoute),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) enum HostGlobTraversalKeyError {
    NonLatin1PackagePathScalar {
        #[allocative(skip)]
        scalar: char,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostGlobTraversalKey {
    workspace: NormalizedAbsolutePath,
    boundary_scope: HostGlobBoundaryScope,
    logical_package_root: NormalizedAbsolutePath,
    package: PackagePath,
    package_bytes: Arc<[u8]>,
    pattern: GlobPattern,
    operation: HostGlobTraversalOperation,
}

impl HostGlobTraversalKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        boundary_scope: HostGlobBoundaryScope,
        logical_package_root: NormalizedAbsolutePath,
        package: PackagePath,
        pattern: GlobPattern,
        operation: HostGlobTraversalOperation,
    ) -> Result<Self, HostGlobTraversalKeyError> {
        let mut package_bytes = Vec::with_capacity(package.as_str().len());
        for scalar in package.as_str().chars() {
            let scalar = scalar as u32;
            let Some(byte) = u8::try_from(scalar).ok() else {
                return Err(HostGlobTraversalKeyError::NonLatin1PackagePathScalar {
                    scalar: char::from_u32(scalar).expect("a char scalar remains valid"),
                });
            };
            package_bytes.push(byte);
        }
        Ok(Self {
            workspace,
            boundary_scope,
            logical_package_root,
            package,
            package_bytes: package_bytes.into(),
            pattern,
            operation,
        })
    }
}

impl fmt::Display for HostGlobTraversalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-glob-traversal:{}//{}:{}",
            self.workspace,
            self.package,
            HexBytes(self.pattern.raw().as_bytes())
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct HostGlobTraversalMatch {
    pub(super) relative_path: Arc<[u8]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct HostGlobTraversal {
    matches: Arc<[HostGlobTraversalMatch]>,
}

impl HostGlobTraversal {
    pub(super) fn from_paths(mut paths: Vec<Arc<[u8]>>) -> Self {
        paths.sort();
        paths.dedup();
        Self {
            matches: paths
                .into_iter()
                .map(|relative_path| HostGlobTraversalMatch { relative_path })
                .collect::<Vec<_>>()
                .into(),
        }
    }

    pub(super) fn matches(&self) -> &[HostGlobTraversalMatch] {
        &self.matches
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) enum HostGlobTraversalError {
    UnsupportedHost,
    Segment {
        logical_directory: NormalizedAbsolutePath,
        fragment_index: usize,
        error: HostGlobSegmentError,
    },
    Boundary {
        candidate_package: PackagePath,
        error: HostGlobBoundaryError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) enum HostGlobBoundaryError {
    Root(HostRootPackageBoundaryError),
    External(HostExternalPackageBoundaryError),
}

pub(super) type HostGlobTraversalOutcome =
    SourcePreparationOutcome<Arc<Result<HostGlobTraversal, HostGlobTraversalError>>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostGlobTraversalObservationKey(HostGlobTraversalKey);

impl HostGlobTraversalObservationKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        boundary_scope: HostGlobBoundaryScope,
        logical_package_root: NormalizedAbsolutePath,
        package: PackagePath,
        pattern: GlobPattern,
        operation: HostGlobTraversalOperation,
    ) -> Result<Self, HostGlobTraversalKeyError> {
        HostGlobTraversalKey::new(
            workspace,
            boundary_scope,
            logical_package_root,
            package,
            pattern,
            operation,
        )
        .map(Self)
    }
}

impl fmt::Display for HostGlobTraversalObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct ObservedHostGlobTraversal {
    result: Arc<Result<HostGlobTraversal, HostGlobTraversalError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostGlobTraversal {
    pub(super) fn result(&self) -> &Arc<Result<HostGlobTraversal, HostGlobTraversalError>> {
        &self.result
    }

    pub(super) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

type HostGlobTraversalDriverOutcome = SourcePreparationOutcome<
    Result<
        (
            Arc<Result<HostGlobTraversal, HostGlobTraversalError>>,
            PathObservationEpoch,
        ),
        ObservedPathFrontierError,
    >,
>;
type ObservedHostGlobTraversalOutcome =
    SourcePreparationOutcome<Result<ObservedHostGlobTraversal, ObservedPathFrontierError>>;

#[derive(Clone, Copy)]
enum HostGlobTraversalMode {
    Legacy,
    Observed,
}

struct TraversalChild<T, E> {
    result: Arc<Result<T, E>>,
    observations: PathObservationEpoch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
enum HostGlobBoundary {
    Continue,
    Stop,
}

type TraversalChildOutcome<T, E> =
    SourcePreparationOutcome<Result<TraversalChild<T, E>, ObservedPathFrontierError>>;

#[derive(Clone)]
struct TraversalState {
    logical_directory: NormalizedAbsolutePath,
    package: PackagePath,
    relative_path: Arc<[u8]>,
    fragment_index: usize,
    ordinal: usize,
}

type TraversalVisited = Option<SmallSet<(Arc<[u8]>, usize)>>;

fn complete_driver(
    result: Result<HostGlobTraversal, HostGlobTraversalError>,
    observations: PathObservationEpoch,
) -> HostGlobTraversalDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn driver_need(need: SourcePreparationNeeds) -> HostGlobTraversalDriverOutcome {
    SourcePreparationOutcome::Need(need)
}

struct TraversalTerminals {
    needs: Option<SourcePreparationNeeds>,
    observations: PathObservationEpoch,
    first_outer: Option<ObservedPathFrontierError>,
    first_error: Option<((usize, usize), HostGlobTraversalError, PathObservationEpoch)>,
}

impl TraversalTerminals {
    fn new() -> Self {
        Self {
            needs: None,
            observations: PathObservationEpoch::empty(),
            first_outer: None,
            first_error: None,
        }
    }

    fn add_need(&mut self, next: SourcePreparationNeeds) {
        self.needs = Some(match self.needs.take() {
            Some(current) => current
                .try_union(&next)
                .expect("Host glob traversal path needs cannot conflict"),
            None => next,
        });
    }

    fn record_outer(&mut self, error: ObservedPathFrontierError) {
        if self.first_error.is_none() && self.first_outer.is_none() {
            self.first_outer = Some(error);
        }
    }

    fn merge_completed(&mut self, next: &PathObservationEpoch) {
        if self.first_error.is_some() || self.first_outer.is_some() {
            return;
        }
        match union_observation_epochs(&self.observations, next) {
            Ok(observations) => self.observations = observations,
            Err(error) => self.first_outer = Some(error),
        }
    }

    fn record_error(&mut self, rank: (usize, usize), error: HostGlobTraversalError) {
        if self.first_error.is_none() && self.first_outer.is_none() {
            self.first_error = Some((rank, error, self.observations.dupe()));
        }
    }

    fn finish(self, paths: Vec<Arc<[u8]>>) -> HostGlobTraversalDriverOutcome {
        if let Some(error) = self.first_outer {
            SourcePreparationOutcome::Complete(Err(error))
        } else if let Some((_, error, observations)) = self.first_error {
            complete_driver(Err(error), observations)
        } else if let Some(need) = self.needs {
            driver_need(need)
        } else {
            complete_driver(Ok(HostGlobTraversal::from_paths(paths)), self.observations)
        }
    }
}

impl HostGlobTraversalMode {
    async fn segment(
        self,
        ctx: &mut DiceComputations<'_>,
        scope: &HostGlobBoundaryScope,
        logical_directory: NormalizedAbsolutePath,
        package: PackagePath,
        pattern: HostGlobSegmentPattern,
    ) -> TraversalChildOutcome<HostGlobSegmentCandidates, HostGlobSegmentError> {
        if let HostGlobBoundaryScope::BuiltinCatalog(route) = scope {
            return self.catalog_segment(ctx, route, package, pattern).await;
        }
        match self {
            Self::Legacy => dice_invariant(
                ctx.compute(&HostGlobSegmentCandidatesKey::new(
                    logical_directory,
                    pattern,
                ))
                .await,
            )
            .map(|result| {
                Ok(TraversalChild {
                    result,
                    observations: PathObservationEpoch::empty(),
                })
            }),
            Self::Observed => dice_invariant(
                ctx.compute(&HostGlobSegmentCandidatesObservationKey::new(
                    logical_directory,
                    pattern,
                ))
                .await,
            )
            .map(|observed| {
                observed.map(|observed| TraversalChild {
                    result: observed.result,
                    observations: observed.observations,
                })
            }),
        }
    }

    async fn catalog_segment(
        self,
        ctx: &mut DiceComputations<'_>,
        route: &HostRepositorySourceRoute,
        package: PackagePath,
        pattern: HostGlobSegmentPattern,
    ) -> TraversalChildOutcome<HostGlobSegmentCandidates, HostGlobSegmentError> {
        let key = match route {
            HostRepositorySourceRoute::Root(route) => {
                HostRepositoryDirectoryListingKey::new(route.clone(), package.clone())
            }
            HostRepositorySourceRoute::Canonical(input) => {
                HostRepositoryDirectoryListingKey::new_canonical(input.clone(), package.clone())
            }
        };
        let observed_key = match route {
            HostRepositorySourceRoute::Root(route) => {
                HostRepositoryDirectoryListingObservationKey::new(route.clone(), package)
            }
            HostRepositorySourceRoute::Canonical(input) => {
                HostRepositoryDirectoryListingObservationKey::new_canonical(input.clone(), package)
            }
        };
        match self {
            Self::Legacy => match dice_invariant(ctx.compute(&key).await) {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(result) => {
                    SourcePreparationOutcome::Complete(Ok(TraversalChild {
                        result: Arc::new(project_catalog_listing(&result, &pattern)),
                        observations: PathObservationEpoch::empty(),
                    }))
                }
            },
            Self::Observed => match dice_invariant(ctx.compute(&observed_key).await) {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(Err(error)) => {
                    SourcePreparationOutcome::Complete(Err(error))
                }
                SourcePreparationOutcome::Complete(Ok(observed)) => {
                    SourcePreparationOutcome::Complete(Ok(TraversalChild {
                        result: Arc::new(project_catalog_listing(
                            observed.result().as_ref(),
                            &pattern,
                        )),
                        observations: observed.observations().dupe(),
                    }))
                }
            },
        }
    }

    async fn boundary(
        self,
        ctx: &mut DiceComputations<'_>,
        scope: &HostGlobBoundaryScope,
        workspace: NormalizedAbsolutePath,
        package: PackagePath,
    ) -> TraversalChildOutcome<HostGlobBoundary, HostGlobBoundaryError> {
        match scope {
            HostGlobBoundaryScope::Root => self.root_boundary(ctx, workspace, package).await,
            HostGlobBoundaryScope::External(route)
            | HostGlobBoundaryScope::BuiltinCatalog(route) => {
                self.external_boundary(ctx, route, package).await
            }
        }
    }

    async fn root_boundary(
        self,
        ctx: &mut DiceComputations<'_>,
        workspace: NormalizedAbsolutePath,
        package: PackagePath,
    ) -> TraversalChildOutcome<HostGlobBoundary, HostGlobBoundaryError> {
        let outcome = match self {
            Self::Legacy => dice_invariant(
                ctx.compute(&HostRootPackageBoundaryKey::new(workspace, package))
                    .await,
            )
            .map(|result| {
                Ok(TraversalChild {
                    result: Arc::new(project_root_boundary(result.as_ref())),
                    observations: PathObservationEpoch::empty(),
                })
            }),
            Self::Observed => dice_invariant(
                ctx.compute(&HostRootPackageBoundaryObservationKey::new(
                    workspace, package,
                ))
                .await,
            )
            .map(|observed| {
                observed.map(|observed| TraversalChild {
                    result: Arc::new(project_root_boundary(observed.result())),
                    observations: observed.observations().dupe(),
                })
            }),
        };
        match outcome {
            PathOutcome::Need(need) => {
                SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need))
            }
            PathOutcome::Complete(value) => SourcePreparationOutcome::Complete(value),
        }
    }

    async fn external_boundary(
        self,
        ctx: &mut DiceComputations<'_>,
        route: &HostRepositorySourceRoute,
        package: PackagePath,
    ) -> TraversalChildOutcome<HostGlobBoundary, HostGlobBoundaryError> {
        let key = match route {
            HostRepositorySourceRoute::Root(route) => {
                HostExternalPackageBoundaryKey::new(route.clone(), package.clone())
            }
            HostRepositorySourceRoute::Canonical(input) => {
                HostExternalPackageBoundaryKey::new_canonical(input.clone(), package.clone())
            }
        };
        let observed_key = match route {
            HostRepositorySourceRoute::Root(route) => {
                HostExternalPackageBoundaryObservationKey::new(route.clone(), package)
            }
            HostRepositorySourceRoute::Canonical(input) => {
                HostExternalPackageBoundaryObservationKey::new_canonical(input.clone(), package)
            }
        };
        match self {
            Self::Legacy => match dice_invariant(ctx.compute(&key).await) {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(result) => {
                    SourcePreparationOutcome::Complete(Ok(TraversalChild {
                        result: Arc::new(project_external_boundary(result.as_ref())),
                        observations: PathObservationEpoch::empty(),
                    }))
                }
            },
            Self::Observed => match dice_invariant(ctx.compute(&observed_key).await) {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(Err(error)) => {
                    SourcePreparationOutcome::Complete(Err(error))
                }
                SourcePreparationOutcome::Complete(Ok(observed)) => {
                    SourcePreparationOutcome::Complete(Ok(TraversalChild {
                        result: Arc::new(project_external_boundary(observed.result())),
                        observations: observed.observations().dupe(),
                    }))
                }
            },
        }
    }
}

fn catalog_component_bytes(component: &OsStr) -> Result<Arc<[u8]>, HostGlobSegmentError> {
    let component = component
        .to_str()
        .ok_or(HostGlobSegmentError::CatalogComponentEncoding)?;
    let mut bytes = Vec::with_capacity(component.chars().count());
    for scalar in component.chars() {
        bytes.push(
            u8::try_from(scalar as u32)
                .map_err(|_| HostGlobSegmentError::CatalogComponentEncoding)?,
        );
    }
    Ok(bytes.into())
}

fn project_catalog_listing(
    result: &Result<PathDirectoryListing, HostRepositoryDirectoryListingError>,
    pattern: &HostGlobSegmentPattern,
) -> Result<HostGlobSegmentCandidates, HostGlobSegmentError> {
    let listing = result
        .as_ref()
        .map_err(|error| HostGlobSegmentError::RepositoryDirectoryListing(error.dupe()))?;
    let PathDirectoryListing::Present(entries) = listing else {
        return Err(HostGlobSegmentError::CatalogDirectoryMissing);
    };
    let mut candidates = Vec::new();
    for entry in entries.entries() {
        let component = catalog_component_bytes(entry.name().as_os_str())?;
        let kind = match entry.kind() {
            PathDirectoryEntryKind::File => HostGlobSegmentCandidateKind::NonDirectory,
            PathDirectoryEntryKind::Directory => HostGlobSegmentCandidateKind::Directory,
            PathDirectoryEntryKind::Symlink | PathDirectoryEntryKind::Unknown => {
                return Err(HostGlobSegmentError::CatalogEntryKind(entry.kind()));
            }
        };
        if glob_segment_matches(pattern, &component) {
            candidates.push(super::HostGlobSegmentCandidate { component, kind });
        }
    }
    Ok(HostGlobSegmentCandidates::from_vec(candidates))
}

fn project_root_boundary(
    result: &Result<HostRootPackageBoundary, HostRootPackageBoundaryError>,
) -> Result<HostGlobBoundary, HostGlobBoundaryError> {
    result
        .as_ref()
        .map(|boundary| match boundary.kind() {
            HostRootPackageBoundaryKind::IgnoredDirectory
            | HostRootPackageBoundaryKind::Package => HostGlobBoundary::Stop,
            HostRootPackageBoundaryKind::NoPackage
            | HostRootPackageBoundaryKind::DeletedPackage => HostGlobBoundary::Continue,
        })
        .map_err(|error| HostGlobBoundaryError::Root(error.clone()))
}

fn project_external_boundary(
    result: &Result<HostExternalPackageBoundary, HostExternalPackageBoundaryError>,
) -> Result<HostGlobBoundary, HostGlobBoundaryError> {
    result
        .as_ref()
        .map(|boundary| match boundary.kind() {
            HostExternalPackageBoundaryKind::IgnoredDirectory
            | HostExternalPackageBoundaryKind::Package => HostGlobBoundary::Stop,
            HostExternalPackageBoundaryKind::InvalidPackageName
            | HostExternalPackageBoundaryKind::DeletedPackage
            | HostExternalPackageBoundaryKind::NoPackage => HostGlobBoundary::Continue,
        })
        .map_err(|error| HostGlobBoundaryError::External(*error))
}

fn raw_child(parent: &[u8], child: &[u8]) -> Arc<[u8]> {
    let mut result =
        Vec::with_capacity(parent.len() + usize::from(!parent.is_empty()) + child.len());
    result.extend_from_slice(parent);
    if !parent.is_empty() {
        result.push(b'/');
    }
    result.extend_from_slice(child);
    result.into()
}

fn package_child(parent: &PackagePath, child: &[u8]) -> PackagePath {
    let mut value = parent.as_str().to_owned();
    if !value.is_empty() {
        value.push('/');
    }
    value.extend(child.iter().map(|byte| char::from(*byte)));
    PackagePath::parse(&value).expect("a raw host component makes a valid package component")
}

fn enqueue(
    frontier: &mut VecDeque<TraversalState>,
    visited: &mut TraversalVisited,
    next_ordinal: &mut usize,
    mut state: TraversalState,
) {
    if let Some(visited) = visited
        && !visited.insert((state.relative_path.dupe(), state.fragment_index))
    {
        return;
    }
    state.ordinal = *next_ordinal;
    *next_ordinal += 1;
    frontier.push_back(state);
}

impl HostGlobTraversalKey {
    async fn compute_driver(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: HostGlobTraversalMode,
    ) -> HostGlobTraversalDriverOutcome {
        let has_multiple_recursive = self.pattern.recursive_count() > 1;
        let recursive_candidate_pattern = GlobPattern::include("*")
            .expect("a fixed simple wildcard is valid")
            .segment(0)
            .expect("a fixed simple wildcard has one segment");
        let logical_directory = logical_child(&self.logical_package_root, &self.package_bytes);
        let mut visited = has_multiple_recursive.then(SmallSet::new);
        let mut frontier = VecDeque::from([TraversalState {
            logical_directory,
            package: self.package.clone(),
            relative_path: Arc::from([]),
            fragment_index: 0,
            ordinal: 0,
        }]);
        if let Some(visited) = &mut visited {
            visited.insert((Arc::from([]), 0));
        }
        let mut next_ordinal = 1;
        let mut paths = Vec::new();
        let mut terminals = TraversalTerminals::new();

        while let Some(state) = frontier.pop_front() {
            let recursive = self.pattern.is_recursive(state.fragment_index);
            let candidate_pattern = if recursive {
                let last = state.fragment_index + 1 == self.pattern.len();
                if last {
                    if self.operation == HostGlobTraversalOperation::FilesAndDirs
                        && !state.relative_path.is_empty()
                    {
                        paths.push(state.relative_path.dupe());
                    }
                } else {
                    enqueue(
                        &mut frontier,
                        &mut visited,
                        &mut next_ordinal,
                        TraversalState {
                            logical_directory: state.logical_directory.dupe(),
                            package: state.package.clone(),
                            relative_path: state.relative_path.dupe(),
                            fragment_index: state.fragment_index + 1,
                            ordinal: 0,
                        },
                    );
                }
                recursive_candidate_pattern.dupe()
            } else {
                self.pattern
                    .segment(state.fragment_index)
                    .expect("a non-recursive fragment is a segment")
            };
            let candidates = match mode
                .segment(
                    ctx,
                    &self.boundary_scope,
                    state.logical_directory.dupe(),
                    state.package.clone(),
                    candidate_pattern,
                )
                .await
            {
                SourcePreparationOutcome::Need(need) => {
                    terminals.add_need(need);
                    continue;
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    terminals.record_outer(error);
                    continue;
                }
                SourcePreparationOutcome::Complete(Ok(child)) => child,
            };
            terminals.merge_completed(&candidates.observations);
            let candidates = match candidates.result.as_ref() {
                Ok(value) => value.dupe(),
                Err(error) => {
                    terminals.record_error(
                        (state.ordinal, 0),
                        HostGlobTraversalError::Segment {
                            logical_directory: state.logical_directory.dupe(),
                            fragment_index: state.fragment_index,
                            error: error.dupe(),
                        },
                    );
                    continue;
                }
            };
            let last = state.fragment_index + 1 == self.pattern.len();
            for (slot, candidate) in candidates.candidates().iter().enumerate() {
                let relative_path = raw_child(&state.relative_path, &candidate.component);
                if candidate.kind == HostGlobSegmentCandidateKind::NonDirectory {
                    if last {
                        paths.push(relative_path);
                    }
                    continue;
                }
                let candidate_package = package_child(&state.package, &candidate.component);
                let boundary = match mode
                    .boundary(
                        ctx,
                        &self.boundary_scope,
                        self.workspace.dupe(),
                        candidate_package.clone(),
                    )
                    .await
                {
                    SourcePreparationOutcome::Need(need) => {
                        terminals.add_need(need);
                        continue;
                    }
                    SourcePreparationOutcome::Complete(Err(error)) => {
                        terminals.record_outer(error);
                        continue;
                    }
                    SourcePreparationOutcome::Complete(Ok(child)) => child,
                };
                terminals.merge_completed(&boundary.observations);
                let boundary = match boundary.result.as_ref() {
                    Ok(value) => value.dupe(),
                    Err(error) => {
                        terminals.record_error(
                            (state.ordinal, slot + 1),
                            HostGlobTraversalError::Boundary {
                                candidate_package,
                                error: error.clone(),
                            },
                        );
                        continue;
                    }
                };
                if boundary == HostGlobBoundary::Stop {
                    continue;
                }
                if last && self.operation == HostGlobTraversalOperation::FilesAndDirs && !recursive
                {
                    paths.push(relative_path.dupe());
                }
                if recursive || !last {
                    enqueue(
                        &mut frontier,
                        &mut visited,
                        &mut next_ordinal,
                        TraversalState {
                            logical_directory: logical_child(
                                &state.logical_directory,
                                &candidate.component,
                            ),
                            package: candidate_package,
                            relative_path,
                            fragment_index: if recursive {
                                state.fragment_index
                            } else {
                                state.fragment_index + 1
                            },
                            ordinal: 0,
                        },
                    );
                }
            }
        }
        terminals.finish(paths)
    }
}

#[async_trait]
impl Key for HostGlobTraversalKey {
    type Value = HostGlobTraversalOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        #[cfg(not(unix))]
        if !matches!(
            self.boundary_scope,
            HostGlobBoundaryScope::BuiltinCatalog(_)
        ) {
            return SourcePreparationOutcome::Complete(Arc::new(Err(
                HostGlobTraversalError::UnsupportedHost,
            )));
        }
        match self
            .compute_driver(ctx, HostGlobTraversalMode::Legacy)
            .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy Host glob traversal produced frontier error: {error}")
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
impl Key for HostGlobTraversalObservationKey {
    type Value = ObservedHostGlobTraversalOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        #[cfg(not(unix))]
        if !matches!(
            self.0.boundary_scope,
            HostGlobBoundaryScope::BuiltinCatalog(_)
        ) {
            return SourcePreparationOutcome::Complete(Ok(ObservedHostGlobTraversal {
                result: Arc::new(Err(HostGlobTraversalError::UnsupportedHost)),
                observations: PathObservationEpoch::empty(),
            }));
        }
        let outcome = self
            .0
            .compute_driver(ctx, HostGlobTraversalMode::Observed)
            .await;
        match outcome {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostGlobTraversal {
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

#[cfg(all(test, unix))]
#[path = "traversal_tests.rs"]
mod traversal_tests;
