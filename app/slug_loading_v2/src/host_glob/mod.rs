/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the package-aware Host glob owner lands.

use std::ffi::OsString;
use std::fmt;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostRepositoryDirectoryListingError;
#[cfg(unix)]
use slug_bzlmod_v2::HostRepositoryPathKey;
#[cfg(unix)]
use slug_bzlmod_v2::HostRepositoryPathObservationKey;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::RepositorySourceFileError;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
#[cfg(all(test, unix))]
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathDirectoryEntryKind;
#[cfg(unix)]
use slug_workspace_v2::PathDirectoryListing;
use slug_workspace_v2::PathDirectoryListingError;
#[cfg(unix)]
use slug_workspace_v2::PathDirectoryListingKey;
#[cfg(unix)]
use slug_workspace_v2::PathDirectoryListingObservationKey;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationError;
#[cfg(unix)]
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
#[cfg(unix)]
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::PathResolutionError;
#[cfg(unix)]
use slug_workspace_v2::ResolvedPath;
#[cfg(unix)]
use slug_workspace_v2::ResolvedPathKey;
#[cfg(unix)]
use slug_workspace_v2::ResolvedPathObservationKey;
#[cfg(unix)]
use slug_workspace_v2::ResolvedPathState;

use crate::glob::GlobSegmentPattern as HostGlobSegmentPattern;
#[cfg(unix)]
use crate::glob::GlobSegmentPatternKind as HostGlobSegmentPatternKind;
#[cfg(unix)]
use crate::glob::glob_segment_matches;

mod adapter;
pub(crate) use adapter::HostGlobLoadingOperation;
pub(crate) use adapter::HostGlobLoadingRequest;
pub(crate) use adapter::HostGlobPrepared;
pub(crate) use adapter::HostGlobRequestInputError;
#[cfg(test)]
pub(crate) use adapter::HostGlobRequestOutcome;
pub(crate) use adapter::HostGlobRequestTraversalError;
pub(crate) use adapter::compute_host_glob_request;
pub(crate) use traversal::HostGlobBoundaryScope;
#[cfg(all(test, unix))]
mod tests;
mod traversal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
enum HostGlobSegmentCandidateKind {
    NonDirectory,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct HostGlobSegmentCandidate {
    component: Arc<[u8]>,
    kind: HostGlobSegmentCandidateKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct HostGlobSegmentCandidates {
    candidates: Arc<[HostGlobSegmentCandidate]>,
}

impl HostGlobSegmentCandidates {
    fn empty() -> Self {
        Self {
            candidates: Arc::from([]),
        }
    }

    fn from_vec(mut candidates: Vec<HostGlobSegmentCandidate>) -> Self {
        candidates.sort_by(|left, right| left.component.cmp(&right.component));
        Self {
            candidates: candidates.into(),
        }
    }

    fn candidates(&self) -> &[HostGlobSegmentCandidate] {
        &self.candidates
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum HostGlobSegmentError {
    UnsupportedHost,
    DirectoryListing(PathDirectoryListingError),
    RepositoryDirectoryListing(HostRepositoryDirectoryListingError),
    RepositoryPath(Arc<RepositorySourceFileError>),
    CatalogDirectoryMissing,
    CatalogComponentEncoding,
    CatalogEntryKind(PathDirectoryEntryKind),
    DirectoryDisappeared {
        logical_directory: NormalizedAbsolutePath,
    },
    Observation {
        logical_directory: NormalizedAbsolutePath,
        component: Arc<[u8]>,
        operation: PathObservationOperation,
        error: PathObservationError,
    },
    InconsistentState {
        logical_directory: NormalizedAbsolutePath,
        component: Arc<[u8]>,
        operation: PathObservationOperation,
        before: Option<PathLstat>,
        after: Option<PathLstat>,
    },
    Cycle {
        logical_directory: NormalizedAbsolutePath,
        component: Arc<[u8]>,
    },
    InfiniteExpansion {
        logical_directory: NormalizedAbsolutePath,
        component: Arc<[u8]>,
    },
    ListingSymlinkResolutionMismatch {
        logical_directory: NormalizedAbsolutePath,
        component: Arc<[u8]>,
    },
}

impl fmt::Display for HostGlobSegmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedHost => f.write_str("Host glob segment names are unsupported here"),
            Self::DirectoryListing(error) => {
                write!(f, "Host glob directory listing failed: {error:?}")
            }
            Self::RepositoryDirectoryListing(error) => {
                write!(f, "repository glob directory listing failed: {error:?}")
            }
            Self::RepositoryPath(error) => write!(f, "repository glob path failed: {error:?}"),
            Self::CatalogDirectoryMissing => {
                f.write_str("built-in catalog glob directory disappeared")
            }
            Self::CatalogComponentEncoding => {
                f.write_str("built-in catalog glob component is not Latin-1")
            }
            Self::CatalogEntryKind(kind) => {
                write!(
                    f,
                    "built-in catalog glob entry kind is unsupported: {kind:?}"
                )
            }
            Self::DirectoryDisappeared { logical_directory } => write!(
                f,
                "Host glob directory disappeared: {:?}",
                logical_directory.as_path()
            ),
            Self::Observation {
                logical_directory,
                component,
                operation,
                error,
            } => write!(
                f,
                "Host glob observation failed at {:?}/{} for {operation:?}: {error:?}",
                logical_directory.as_path(),
                HexBytes(component)
            ),
            Self::InconsistentState {
                logical_directory,
                component,
                operation,
                ..
            } => write!(
                f,
                "Host glob observation became inconsistent at {:?}/{} for {operation:?}",
                logical_directory.as_path(),
                HexBytes(component)
            ),
            Self::Cycle {
                logical_directory,
                component,
            } => write!(
                f,
                "Host glob symlink cycle at {:?}/{}",
                logical_directory.as_path(),
                HexBytes(component)
            ),
            Self::InfiniteExpansion {
                logical_directory,
                component,
            } => write!(
                f,
                "Host glob infinite symlink expansion at {:?}/{}",
                logical_directory.as_path(),
                HexBytes(component)
            ),
            Self::ListingSymlinkResolutionMismatch {
                logical_directory,
                component,
            } => write!(
                f,
                "Host glob listing/resolution symlink mismatch at {:?}/{}",
                logical_directory.as_path(),
                HexBytes(component)
            ),
        }
    }
}

impl std::error::Error for HostGlobSegmentError {}

struct HexBytes<'a>(&'a [u8]);

impl fmt::Display for HexBytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
enum HostGlobSegmentSource {
    Host,
    Repository {
        route: Arc<HostRepositorySourceRoute>,
        directory: Arc<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
struct HostGlobSegmentCandidatesKey {
    source: HostGlobSegmentSource,
    logical_directory: NormalizedAbsolutePath,
    pattern: HostGlobSegmentPattern,
}

impl HostGlobSegmentCandidatesKey {
    fn new(logical_directory: NormalizedAbsolutePath, pattern: HostGlobSegmentPattern) -> Self {
        Self {
            source: HostGlobSegmentSource::Host,
            logical_directory,
            pattern,
        }
    }
    fn from_repository(
        route: Arc<HostRepositorySourceRoute>,
        directory: Arc<PathBuf>,
        logical_directory: NormalizedAbsolutePath,
        pattern: HostGlobSegmentPattern,
    ) -> Self {
        Self {
            source: HostGlobSegmentSource::Repository { route, directory },
            logical_directory,
            pattern,
        }
    }
}

impl fmt::Display for HostGlobSegmentCandidatesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-glob-segment-candidates:{:?}:{}",
            self.logical_directory.as_path(),
            HexBytes(self.pattern.bytes())
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
struct HostGlobSegmentCandidatesObservationKey(HostGlobSegmentCandidatesKey);

impl HostGlobSegmentCandidatesObservationKey {
    fn new(logical_directory: NormalizedAbsolutePath, pattern: HostGlobSegmentPattern) -> Self {
        Self(HostGlobSegmentCandidatesKey::new(
            logical_directory,
            pattern,
        ))
    }
}

impl fmt::Display for HostGlobSegmentCandidatesObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct ObservedHostGlobSegmentCandidates {
    result: Arc<Result<HostGlobSegmentCandidates, HostGlobSegmentError>>,
    observations: PathObservationEpoch,
}

type HostGlobSegmentOutcome =
    SourcePreparationOutcome<Arc<Result<HostGlobSegmentCandidates, HostGlobSegmentError>>>;

type HostGlobSegmentDriverOutcome = SourcePreparationOutcome<
    Result<
        (
            Arc<Result<HostGlobSegmentCandidates, HostGlobSegmentError>>,
            PathObservationEpoch,
        ),
        ObservedPathFrontierError,
    >,
>;
type ObservedHostGlobSegmentOutcome =
    SourcePreparationOutcome<Result<ObservedHostGlobSegmentCandidates, ObservedPathFrontierError>>;

#[derive(Clone, Copy)]
enum HostGlobSegmentMode {
    Legacy,
    Observed,
}

fn segment_complete(
    result: Result<HostGlobSegmentCandidates, HostGlobSegmentError>,
    observations: PathObservationEpoch,
) -> HostGlobSegmentDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn segment_need(need: SourcePreparationNeeds) -> HostGlobSegmentDriverOutcome {
    SourcePreparationOutcome::Need(need)
}

fn union_observation_epochs(
    left: &PathObservationEpoch,
    right: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        left.observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain(
                right
                    .observations()
                    .iter()
                    .map(|(demand, result)| (demand.dupe(), result.dupe())),
            ),
    )
    .map_err(ObservedPathFrontierError::from)
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host glob segment DICE invariant failed: {error:?}"))
}

#[cfg(unix)]
struct SegmentInput<T, E> {
    result: Result<T, E>,
    observations: PathObservationEpoch,
}

#[cfg(unix)]
type SegmentInputOutcome<T, E> =
    SourcePreparationOutcome<Result<SegmentInput<T, E>, ObservedPathFrontierError>>;

#[cfg(unix)]
fn path_segment_input<T, E>(
    outcome: PathOutcome<Result<SegmentInput<T, E>, ObservedPathFrontierError>>,
) -> SegmentInputOutcome<T, E> {
    match outcome {
        PathOutcome::Need(need) => {
            SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need))
        }
        PathOutcome::Complete(result) => SourcePreparationOutcome::Complete(result),
    }
}

#[cfg(unix)]
fn legacy_segment_input<T, E>(outcome: PathOutcome<Result<T, E>>) -> SegmentInputOutcome<T, E> {
    path_segment_input(outcome.map(|result| {
        Ok(SegmentInput {
            result,
            observations: PathObservationEpoch::empty(),
        })
    }))
}

#[cfg(unix)]
impl HostGlobSegmentMode {
    async fn resolve(
        self,
        ctx: &mut DiceComputations<'_>,
        key: &HostGlobSegmentCandidatesKey,
        component: Option<&[u8]>,
    ) -> SegmentInputOutcome<ResolvedPath, HostGlobSegmentError> {
        if let HostGlobSegmentSource::Repository { route, directory } = &key.source {
            let mut relative = directory.as_ref().clone();
            if let Some(component) = component {
                relative.push(std::ffi::OsStr::from_bytes(component));
            }
            return match self {
                Self::Legacy => {
                    let child = if relative.as_os_str().is_empty() {
                        HostRepositoryPathKey::for_repository_root(route.as_ref().clone())
                    } else {
                        HostRepositoryPathKey::from_source_route(route.as_ref().clone(), relative)
                    };
                    dice_invariant(ctx.compute(&child).await).map(|result| {
                        Ok(SegmentInput {
                            result: result
                                .as_ref()
                                .map(|value| value.resolved().dupe())
                                .map_err(|error| {
                                    HostGlobSegmentError::RepositoryPath(Arc::new(error.clone()))
                                }),
                            observations: PathObservationEpoch::empty(),
                        })
                    })
                }
                Self::Observed => {
                    let child = if relative.as_os_str().is_empty() {
                        HostRepositoryPathObservationKey::for_repository_root(
                            route.as_ref().clone(),
                        )
                    } else {
                        HostRepositoryPathObservationKey::from_source_route(
                            route.as_ref().clone(),
                            relative,
                        )
                    };
                    dice_invariant(ctx.compute(&child).await).map(|observed| {
                        observed.map(|observed| SegmentInput {
                            result: observed
                                .result()
                                .as_ref()
                                .as_ref()
                                .map(|value| value.resolved().dupe())
                                .map_err(|error| {
                                    HostGlobSegmentError::RepositoryPath(Arc::new(error.clone()))
                                }),
                            observations: observed.observations().dupe(),
                        })
                    })
                }
            };
        }
        let logical_path = component.map_or_else(
            || key.logical_directory.dupe(),
            |component| logical_child(&key.logical_directory, component),
        );
        let project_error = |error| {
            resolution_error(
                &key.logical_directory,
                component.map_or_else(|| Arc::from([]), Arc::from),
                error,
            )
        };
        match self {
            Self::Legacy => legacy_segment_input(dice_invariant(
                ctx.compute(&ResolvedPathKey::new(
                    PathObservationNamespace::Host,
                    logical_path,
                ))
                .await,
            ))
            .map(|result| {
                result.map(|input| SegmentInput {
                    result: input.result.map_err(project_error),
                    observations: input.observations,
                })
            }),
            Self::Observed => path_segment_input(
                dice_invariant(
                    ctx.compute(&ResolvedPathObservationKey::new(
                        PathObservationNamespace::Host,
                        logical_path,
                    ))
                    .await,
                )
                .map(|observed| {
                    observed.map(|observed| SegmentInput {
                        result: observed.result().clone().map_err(project_error),
                        observations: observed.observations().dupe(),
                    })
                }),
            ),
        }
    }

    async fn scoped_list(
        self,
        ctx: &mut DiceComputations<'_>,
        namespace: PathObservationNamespace,
        logical_path: NormalizedAbsolutePath,
    ) -> SegmentInputOutcome<PathDirectoryListing, PathDirectoryListingError> {
        match self {
            Self::Legacy => legacy_segment_input(dice_invariant(
                ctx.compute(&PathDirectoryListingKey::new(namespace, logical_path))
                    .await,
            )),
            Self::Observed => path_segment_input(
                dice_invariant(
                    ctx.compute(&PathDirectoryListingObservationKey::new(
                        namespace,
                        logical_path,
                    ))
                    .await,
                )
                .map(|observed| {
                    observed.map(|observed| SegmentInput {
                        result: observed.result().clone(),
                        observations: observed.observations().dupe(),
                    })
                }),
            ),
        }
    }

    async fn list(
        self,
        ctx: &mut DiceComputations<'_>,
        key: &HostGlobSegmentCandidatesKey,
    ) -> SegmentInputOutcome<PathDirectoryListing, HostGlobSegmentError> {
        let (namespace, logical_path, resolution_observations) = match &key.source {
            HostGlobSegmentSource::Host => (
                PathObservationNamespace::Host,
                key.logical_directory.dupe(),
                PathObservationEpoch::empty(),
            ),
            HostGlobSegmentSource::Repository { .. } => {
                let input = match self.resolve(ctx, key, None).await {
                    SourcePreparationOutcome::Need(need) => {
                        return SourcePreparationOutcome::Need(need);
                    }
                    SourcePreparationOutcome::Complete(Err(error)) => {
                        return SourcePreparationOutcome::Complete(Err(error));
                    }
                    SourcePreparationOutcome::Complete(Ok(input)) => input,
                };
                match input.result {
                    Err(error) => {
                        return SourcePreparationOutcome::Complete(Ok(SegmentInput {
                            result: Err(error),
                            observations: input.observations,
                        }));
                    }
                    Ok(resolved) => (
                        resolved.namespace(),
                        resolved.requested_path().dupe(),
                        input.observations,
                    ),
                }
            }
        };
        self.scoped_list(ctx, namespace, logical_path)
            .await
            .map(|result| {
                result.and_then(|input| {
                    Ok(SegmentInput {
                        result: input.result.map_err(HostGlobSegmentError::DirectoryListing),
                        observations: union_observation_epochs(
                            &resolution_observations,
                            &input.observations,
                        )?,
                    })
                })
            })
    }
}

#[cfg(unix)]
#[derive(Debug, Clone)]
struct PendingSymlink {
    slot: usize,
    component: Arc<[u8]>,
}

#[cfg(unix)]
struct PendingSymlinkBatch {
    slots: Vec<Option<HostGlobSegmentCandidate>>,
    pending: Vec<PendingSymlink>,
    directory_real_path: NormalizedAbsolutePath,
    observations: PathObservationEpoch,
}

#[cfg(unix)]
type PendingSymlinkOutcome = (
    PendingSymlink,
    SegmentInputOutcome<ResolvedPath, HostGlobSegmentError>,
);

#[cfg(unix)]
impl HostGlobSegmentCandidatesKey {
    async fn compute_unix(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: HostGlobSegmentMode,
    ) -> HostGlobSegmentDriverOutcome {
        match self.pattern.kind() {
            HostGlobSegmentPatternKind::Literal => self.compute_literal_unix(ctx, mode).await,
            HostGlobSegmentPatternKind::Wildcard => self.compute_wildcard_unix(ctx, mode).await,
        }
    }

    async fn compute_literal_unix(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: HostGlobSegmentMode,
    ) -> HostGlobSegmentDriverOutcome {
        let component: Arc<[u8]> = Arc::from(self.pattern.bytes());
        let resolved = match mode.resolve(ctx, self, Some(&component)).await {
            SourcePreparationOutcome::Need(need) => return segment_need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok(resolved)) => resolved,
        };
        let result = match resolved.result {
            Err(error) => Err(error),
            Ok(resolved) => match resolved.state() {
                ResolvedPathState::Missing => Ok(HostGlobSegmentCandidates::empty()),
                ResolvedPathState::Present(lstat) => Ok(HostGlobSegmentCandidates::from_vec(vec![
                    HostGlobSegmentCandidate {
                        component,
                        kind: candidate_kind(lstat.kind()),
                    },
                ])),
            },
        };
        segment_complete(result, resolved.observations)
    }

    async fn compute_wildcard_unix(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: HostGlobSegmentMode,
    ) -> HostGlobSegmentDriverOutcome {
        let listing = match mode.list(ctx, self).await {
            SourcePreparationOutcome::Need(need) => return segment_need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok(listing)) => listing,
        };
        let mut observations = listing.observations;
        let entries = match listing.result {
            Err(error) => {
                return segment_complete(Err(error), observations);
            }
            Ok(PathDirectoryListing::Missing) => {
                return segment_complete(
                    Err(HostGlobSegmentError::DirectoryDisappeared {
                        logical_directory: self.logical_directory.dupe(),
                    }),
                    observations,
                );
            }
            Ok(PathDirectoryListing::Present(entries)) => entries,
        };

        let mut slots = Vec::new();
        let mut pending = Vec::new();
        for entry in entries.entries() {
            let component: Arc<[u8]> = Arc::from(entry.name().as_os_str().as_bytes());
            if !glob_segment_matches(&self.pattern, &component) {
                continue;
            }
            let slot = slots.len();
            match entry.kind() {
                PathDirectoryEntryKind::Unknown => {}
                PathDirectoryEntryKind::File => {
                    slots.push(Some(HostGlobSegmentCandidate {
                        component,
                        kind: HostGlobSegmentCandidateKind::NonDirectory,
                    }));
                }
                PathDirectoryEntryKind::Directory => {
                    slots.push(Some(HostGlobSegmentCandidate {
                        component,
                        kind: HostGlobSegmentCandidateKind::Directory,
                    }));
                }
                PathDirectoryEntryKind::Symlink => {
                    slots.push(None);
                    pending.push(PendingSymlink { slot, component });
                }
            }
        }
        if pending.is_empty() {
            return segment_complete(
                Ok(HostGlobSegmentCandidates::from_vec(
                    slots.into_iter().flatten().collect(),
                )),
                observations,
            );
        }

        // The listing already resolved this base. Reuse that completed dependency
        // to identify the physical child recorded by ResolvedPath.
        let resolved_directory = match mode.resolve(ctx, self, None).await {
            SourcePreparationOutcome::Need(need) => return segment_need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok(resolved)) => resolved,
        };
        observations =
            match union_observation_epochs(&observations, &resolved_directory.observations) {
                Ok(observations) => observations,
                Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
            };
        let directory_real_path = match resolved_directory.result {
            Ok(resolved)
                if matches!(
                    resolved.state(),
                    ResolvedPathState::Present(lstat)
                        if lstat.kind() == PathNodeKind::Directory
                ) =>
            {
                resolved.real_path().dupe()
            }
            other => panic!(
                "completed Host directory listing must retain a completed directory resolution: \
                 {other:?}"
            ),
        };

        self.complete_pending_symlinks(
            ctx,
            mode,
            PendingSymlinkBatch {
                slots,
                pending,
                directory_real_path,
                observations,
            },
        )
        .await
    }

    async fn complete_pending_symlinks(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: HostGlobSegmentMode,
        batch: PendingSymlinkBatch,
    ) -> HostGlobSegmentDriverOutcome {
        let mut batch = batch;
        let pending = std::mem::take(&mut batch.pending);
        let outcomes = ctx
            .compute_join(pending, |ctx, pending| {
                Box::pin(async move {
                    let outcome = mode.resolve(ctx, self, Some(&pending.component)).await;
                    (pending, outcome)
                })
            })
            .await;

        self.finish_pending_symlinks(batch, outcomes)
    }

    fn finish_pending_symlinks(
        &self,
        batch: PendingSymlinkBatch,
        outcomes: Vec<PendingSymlinkOutcome>,
    ) -> HostGlobSegmentDriverOutcome {
        let mut all_need: Option<SourcePreparationNeeds> = None;
        let mut observations = batch.observations;
        let mut slots = batch.slots;
        for (pending, outcome) in outcomes {
            let resolved = match outcome {
                SourcePreparationOutcome::Need(need) => {
                    all_need = Some(match all_need {
                        Some(current) => current
                            .try_union(&need)
                            .expect("same repository segment needs cannot conflict"),
                        None => need,
                    });
                    continue;
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    return SourcePreparationOutcome::Complete(Err(error));
                }
                SourcePreparationOutcome::Complete(Ok(resolved)) => resolved,
            };
            observations = match union_observation_epochs(&observations, &resolved.observations) {
                Ok(observations) => observations,
                Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
            };
            let resolved = match resolved.result {
                Ok(resolved) => resolved,
                Err(error) => {
                    return segment_complete(Err(error), observations);
                }
            };
            let physical_child = logical_child(&batch.directory_real_path, &pending.component);
            if !resolved
                .symlinks()
                .iter()
                .any(|symlink| symlink.path() == &physical_child)
            {
                return segment_complete(
                    Err(HostGlobSegmentError::ListingSymlinkResolutionMismatch {
                        logical_directory: self.logical_directory.dupe(),
                        component: pending.component,
                    }),
                    observations,
                );
            }
            match resolved.state() {
                ResolvedPathState::Missing => {}
                ResolvedPathState::Present(lstat)
                    if lstat.kind() == PathNodeKind::Directory
                        && resolved.ancestor_expansion().is_some() =>
                {
                    return segment_complete(
                        Err(HostGlobSegmentError::InfiniteExpansion {
                            logical_directory: self.logical_directory.dupe(),
                            component: pending.component,
                        }),
                        observations,
                    );
                }
                ResolvedPathState::Present(lstat) => {
                    slots[pending.slot] = Some(HostGlobSegmentCandidate {
                        component: pending.component,
                        kind: candidate_kind(lstat.kind()),
                    });
                }
            }
        }

        if let Some(need) = all_need {
            return segment_need(need);
        }
        segment_complete(
            Ok(HostGlobSegmentCandidates::from_vec(
                slots.into_iter().flatten().collect(),
            )),
            observations,
        )
    }
}

fn logical_child(parent: &NormalizedAbsolutePath, component: &[u8]) -> NormalizedAbsolutePath {
    #[cfg(unix)]
    let component = OsString::from_vec(component.to_vec());
    #[cfg(not(unix))]
    let component = OsString::from(
        component
            .iter()
            .map(|byte| char::from(*byte))
            .collect::<String>(),
    );
    NormalizedAbsolutePath::new(parent.as_path().join(component))
        .expect("a validated component joined to an absolute directory remains absolute")
}

fn candidate_kind(kind: PathNodeKind) -> HostGlobSegmentCandidateKind {
    match kind {
        PathNodeKind::Directory => HostGlobSegmentCandidateKind::Directory,
        PathNodeKind::RegularFile | PathNodeKind::SpecialFile => {
            HostGlobSegmentCandidateKind::NonDirectory
        }
        PathNodeKind::Symlink => {
            panic!("ResolvedPath terminal state must not retain a symlink")
        }
    }
}

fn resolution_error(
    logical_directory: &NormalizedAbsolutePath,
    component: Arc<[u8]>,
    error: PathResolutionError,
) -> HostGlobSegmentError {
    match error {
        PathResolutionError::Observation { demand, error, .. } => {
            HostGlobSegmentError::Observation {
                logical_directory: logical_directory.dupe(),
                component,
                operation: demand.operation(),
                error,
            }
        }
        PathResolutionError::InconsistentState {
            demand,
            before,
            after,
            ..
        } => HostGlobSegmentError::InconsistentState {
            logical_directory: logical_directory.dupe(),
            component,
            operation: demand.operation(),
            before,
            after,
        },
        PathResolutionError::Cycle { .. } => HostGlobSegmentError::Cycle {
            logical_directory: logical_directory.dupe(),
            component,
        },
        PathResolutionError::InfiniteExpansion { .. } => HostGlobSegmentError::InfiniteExpansion {
            logical_directory: logical_directory.dupe(),
            component,
        },
    }
}

impl HostGlobSegmentCandidatesKey {
    async fn compute_driver(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: HostGlobSegmentMode,
    ) -> HostGlobSegmentDriverOutcome {
        #[cfg(unix)]
        {
            self.compute_unix(ctx, mode).await
        }
        #[cfg(not(unix))]
        {
            let _ = (ctx, mode);
            segment_complete(
                Err(HostGlobSegmentError::UnsupportedHost),
                PathObservationEpoch::empty(),
            )
        }
    }
}

#[async_trait]
impl Key for HostGlobSegmentCandidatesKey {
    type Value = HostGlobSegmentOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match self.compute_driver(ctx, HostGlobSegmentMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy Host glob segment produced frontier error: {error}")
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
impl Key for HostGlobSegmentCandidatesObservationKey {
    type Value = ObservedHostGlobSegmentOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match self
            .0
            .compute_driver(ctx, HostGlobSegmentMode::Observed)
            .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostGlobSegmentCandidates {
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
