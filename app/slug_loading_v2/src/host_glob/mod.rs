/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the package-aware Host glob owner lands.

#[cfg(unix)]
use std::ffi::OsString;
use std::fmt;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::NormalizedAbsolutePath;
#[cfg(unix)]
use slug_workspace_v2::PathDirectoryEntryKind;
#[cfg(unix)]
use slug_workspace_v2::PathDirectoryListing;
use slug_workspace_v2::PathDirectoryListingError;
#[cfg(unix)]
use slug_workspace_v2::PathDirectoryListingKey;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationError;
#[cfg(unix)]
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
#[cfg(unix)]
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::PathResolutionError;
#[cfg(unix)]
use slug_workspace_v2::ResolvedPathKey;
#[cfg(unix)]
use slug_workspace_v2::ResolvedPathState;

mod adapter;
pub(crate) use adapter::HostGlobLoadingOperation;
pub(crate) use adapter::HostGlobLoadingRequest;
pub(crate) use adapter::HostGlobPrepared;
pub(crate) use adapter::HostGlobRequestInputError;
pub(crate) use adapter::HostGlobRequestTraversalError;
pub(crate) use adapter::compute_host_glob_request;
#[cfg(all(test, unix))]
mod tests;
mod traversal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
enum HostGlobSegmentPatternKind {
    Literal,
    SimpleWildcard,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
struct HostGlobSegmentPattern {
    bytes: Arc<[u8]>,
    kind: HostGlobSegmentPatternKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
enum HostGlobInvalidPattern {
    QuestionMarkForbidden,
    Empty,
    Absolute,
    EmptySegment,
    DotSegment,
    UpLevelSegment,
    EmbeddedRecursiveWildcard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
enum HostGlobDeferredPattern {
    RecursiveWildcard,
    MultiSegment,
    NulPathByte,
    Parenthesis,
    Bracket,
    Brace,
    Backslash,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum HostGlobSegmentPatternError {
    Invalid {
        pattern: Arc<[u8]>,
        reason: HostGlobInvalidPattern,
    },
    Deferred {
        pattern: Arc<[u8]>,
        reason: HostGlobDeferredPattern,
    },
}

impl HostGlobSegmentPattern {
    fn new(bytes: impl Into<Arc<[u8]>>) -> Result<Self, HostGlobSegmentPatternError> {
        let bytes = bytes.into();
        let invalid = |reason| HostGlobSegmentPatternError::Invalid {
            pattern: bytes.dupe(),
            reason,
        };
        let deferred = |reason| HostGlobSegmentPatternError::Deferred {
            pattern: bytes.dupe(),
            reason,
        };

        // GlobValue checks this before UnixGlob validation.
        if bytes.contains(&b'?') {
            return Err(invalid(HostGlobInvalidPattern::QuestionMarkForbidden));
        }
        if bytes.is_empty() {
            return Err(invalid(HostGlobInvalidPattern::Empty));
        }
        if bytes[0] == b'/' {
            return Err(invalid(HostGlobInvalidPattern::Absolute));
        }

        let segments = bytes.split(|byte| *byte == b'/').collect::<Vec<_>>();
        for segment in &segments {
            if segment.is_empty() {
                return Err(invalid(HostGlobInvalidPattern::EmptySegment));
            }
            if *segment == b"." {
                return Err(invalid(HostGlobInvalidPattern::DotSegment));
            }
            if *segment == b".." {
                return Err(invalid(HostGlobInvalidPattern::UpLevelSegment));
            }
            if contains_adjacent_stars(segment) && *segment != b"**" {
                return Err(invalid(HostGlobInvalidPattern::EmbeddedRecursiveWildcard));
            }
        }
        if segments.len() != 1 {
            return Err(deferred(HostGlobDeferredPattern::MultiSegment));
        }

        let segment = segments[0];
        if segment == b"**" {
            return Err(deferred(HostGlobDeferredPattern::RecursiveWildcard));
        }
        if segment.contains(&0) {
            return Err(deferred(HostGlobDeferredPattern::NulPathByte));
        }
        for (needle, reason) in [
            (b'(', HostGlobDeferredPattern::Parenthesis),
            (b')', HostGlobDeferredPattern::Parenthesis),
            (b'[', HostGlobDeferredPattern::Bracket),
            (b']', HostGlobDeferredPattern::Bracket),
            (b'{', HostGlobDeferredPattern::Brace),
            (b'}', HostGlobDeferredPattern::Brace),
            (b'\\', HostGlobDeferredPattern::Backslash),
        ] {
            if segment.contains(&needle) {
                return Err(deferred(reason));
            }
        }

        let kind = if segment.contains(&b'*') {
            HostGlobSegmentPatternKind::SimpleWildcard
        } else {
            HostGlobSegmentPatternKind::Literal
        };
        Ok(Self { bytes, kind })
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn contains_adjacent_stars(bytes: &[u8]) -> bool {
    bytes.windows(2).any(|pair| pair == b"**")
}

fn simple_segment_matches(pattern: &[u8], candidate: &[u8]) -> bool {
    if pattern.is_empty() || candidate.is_empty() {
        return false;
    }
    if pattern == b"*" {
        return true;
    }
    if candidate[0] == b'.' && pattern[0] != b'.' {
        return false;
    }

    let mut pattern_index = 0;
    let mut candidate_index = 0;
    let mut last_star = None;
    let mut star_candidate_index = 0;
    while candidate_index < candidate.len() {
        if pattern_index < pattern.len() && pattern[pattern_index] == candidate[candidate_index] {
            pattern_index += 1;
            candidate_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            last_star = Some(pattern_index);
            pattern_index += 1;
            star_candidate_index = candidate_index;
        } else if let Some(star) = last_star {
            star_candidate_index += 1;
            candidate_index = star_candidate_index;
            pattern_index = star + 1;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

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
struct HostGlobSegmentCandidatesKey {
    logical_directory: NormalizedAbsolutePath,
    pattern: HostGlobSegmentPattern,
}

impl HostGlobSegmentCandidatesKey {
    fn new(logical_directory: NormalizedAbsolutePath, pattern: HostGlobSegmentPattern) -> Self {
        Self {
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

type HostGlobSegmentOutcome =
    SourcePreparationOutcome<Arc<Result<HostGlobSegmentCandidates, HostGlobSegmentError>>>;

fn terminal_ok(candidates: HostGlobSegmentCandidates) -> HostGlobSegmentOutcome {
    SourcePreparationOutcome::Complete(Arc::new(Ok(candidates)))
}

fn terminal_error(error: HostGlobSegmentError) -> HostGlobSegmentOutcome {
    SourcePreparationOutcome::Complete(Arc::new(Err(error)))
}

fn path_need(need: NeedPathObservations) -> HostGlobSegmentOutcome {
    SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need))
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host glob segment DICE invariant failed: {error:?}"))
}

#[cfg(unix)]
#[derive(Debug, Clone)]
struct PendingSymlink {
    slot: usize,
    component: Arc<[u8]>,
    logical_path: NormalizedAbsolutePath,
}

#[cfg(unix)]
impl HostGlobSegmentCandidatesKey {
    async fn compute_unix(&self, ctx: &mut DiceComputations<'_>) -> HostGlobSegmentOutcome {
        match self.pattern.kind {
            HostGlobSegmentPatternKind::Literal => self.compute_literal_unix(ctx).await,
            HostGlobSegmentPatternKind::SimpleWildcard => self.compute_wildcard_unix(ctx).await,
        }
    }

    async fn compute_literal_unix(&self, ctx: &mut DiceComputations<'_>) -> HostGlobSegmentOutcome {
        let component = self.pattern.bytes.dupe();
        let logical_path = logical_child(&self.logical_directory, &component);
        let resolved = dice_invariant(
            ctx.compute(&ResolvedPathKey::new(
                PathObservationNamespace::Host,
                logical_path,
            ))
            .await,
        );
        match resolved {
            PathOutcome::Need(need) => path_need(need),
            PathOutcome::Complete(Err(error)) => {
                terminal_error(resolution_error(&self.logical_directory, component, error))
            }
            PathOutcome::Complete(Ok(resolved)) => match resolved.state() {
                ResolvedPathState::Missing => terminal_ok(HostGlobSegmentCandidates::empty()),
                ResolvedPathState::Present(lstat) => {
                    terminal_ok(HostGlobSegmentCandidates::from_vec(vec![
                        HostGlobSegmentCandidate {
                            component,
                            kind: candidate_kind(lstat.kind()),
                        },
                    ]))
                }
            },
        }
    }

    async fn compute_wildcard_unix(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> HostGlobSegmentOutcome {
        let listing = dice_invariant(
            ctx.compute(&PathDirectoryListingKey::new(
                PathObservationNamespace::Host,
                self.logical_directory.dupe(),
            ))
            .await,
        );
        let entries = match listing {
            PathOutcome::Need(need) => return path_need(need),
            PathOutcome::Complete(Err(error)) => {
                return terminal_error(HostGlobSegmentError::DirectoryListing(error));
            }
            PathOutcome::Complete(Ok(PathDirectoryListing::Missing)) => {
                return terminal_error(HostGlobSegmentError::DirectoryDisappeared {
                    logical_directory: self.logical_directory.dupe(),
                });
            }
            PathOutcome::Complete(Ok(PathDirectoryListing::Present(entries))) => entries,
        };

        let mut slots = Vec::new();
        let mut pending = Vec::new();
        for entry in entries.entries() {
            let component: Arc<[u8]> = Arc::from(entry.name().as_os_str().as_bytes());
            if !simple_segment_matches(self.pattern.bytes(), &component) {
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
                    let logical_path = logical_child(&self.logical_directory, &component);
                    slots.push(None);
                    pending.push(PendingSymlink {
                        slot,
                        component,
                        logical_path,
                    });
                }
            }
        }
        if pending.is_empty() {
            return terminal_ok(HostGlobSegmentCandidates::from_vec(
                slots.into_iter().flatten().collect(),
            ));
        }

        // The listing already resolved this base. Reuse that completed dependency
        // to identify the physical child recorded by ResolvedPath.
        let resolved_directory = dice_invariant(
            ctx.compute(&ResolvedPathKey::new(
                PathObservationNamespace::Host,
                self.logical_directory.dupe(),
            ))
            .await,
        );
        let directory_real_path = match resolved_directory {
            PathOutcome::Complete(Ok(resolved))
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

        let outcomes = ctx
            .compute_join(pending, |ctx, pending| {
                Box::pin(async move {
                    let outcome = dice_invariant(
                        ctx.compute(&ResolvedPathKey::new(
                            PathObservationNamespace::Host,
                            pending.logical_path.dupe(),
                        ))
                        .await,
                    );
                    (pending, outcome)
                })
            })
            .await;

        let mut all_need: Option<NeedPathObservations> = None;
        let mut first_error = None;
        for (pending, outcome) in outcomes {
            match outcome {
                PathOutcome::Need(need) => {
                    all_need = Some(match all_need {
                        Some(current) => current.union(&need),
                        None => need,
                    });
                }
                PathOutcome::Complete(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(resolution_error(
                            &self.logical_directory,
                            pending.component,
                            error,
                        ));
                    }
                }
                PathOutcome::Complete(Ok(resolved)) => {
                    let physical_child = logical_child(&directory_real_path, &pending.component);
                    if !resolved
                        .symlinks()
                        .iter()
                        .any(|symlink| symlink.path() == &physical_child)
                    {
                        if first_error.is_none() {
                            first_error =
                                Some(HostGlobSegmentError::ListingSymlinkResolutionMismatch {
                                    logical_directory: self.logical_directory.dupe(),
                                    component: pending.component,
                                });
                        }
                        continue;
                    }
                    match resolved.state() {
                        ResolvedPathState::Missing => {}
                        ResolvedPathState::Present(lstat) => {
                            if lstat.kind() == PathNodeKind::Directory
                                && resolved.ancestor_expansion().is_some()
                            {
                                if first_error.is_none() {
                                    first_error = Some(HostGlobSegmentError::InfiniteExpansion {
                                        logical_directory: self.logical_directory.dupe(),
                                        component: pending.component,
                                    });
                                }
                            } else {
                                slots[pending.slot] = Some(HostGlobSegmentCandidate {
                                    component: pending.component,
                                    kind: candidate_kind(lstat.kind()),
                                });
                            }
                        }
                    }
                }
            }
        }

        if let Some(error) = first_error {
            return terminal_error(error);
        }
        if let Some(need) = all_need {
            return path_need(need);
        }
        terminal_ok(HostGlobSegmentCandidates::from_vec(
            slots.into_iter().flatten().collect(),
        ))
    }
}

#[cfg(unix)]
fn logical_child(parent: &NormalizedAbsolutePath, component: &[u8]) -> NormalizedAbsolutePath {
    let component = OsString::from_vec(component.to_vec());
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

#[async_trait]
impl Key for HostGlobSegmentCandidatesKey {
    type Value = HostGlobSegmentOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        #[cfg(unix)]
        {
            self.compute_unix(ctx).await
        }
        #[cfg(not(unix))]
        {
            let _ = ctx;
            terminal_error(HostGlobSegmentError::UnsupportedHost)
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
