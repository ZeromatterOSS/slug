/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostRootPackageBoundaryError;
#[cfg(unix)]
use slug_bzlmod_v2::HostRootPackageBoundaryKey;
#[cfg(unix)]
use slug_bzlmod_v2::HostRootPackageBoundaryKind;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
#[cfg(unix)]
use slug_workspace_v2::PathOutcome;
use starlark_map::small_set::SmallSet;

use super::HexBytes;
use super::HostGlobDeferredPattern;
use super::HostGlobInvalidPattern;
#[cfg(unix)]
use super::HostGlobSegmentCandidateKind;
#[cfg(unix)]
use super::HostGlobSegmentCandidatesKey;
use super::HostGlobSegmentError;
use super::HostGlobSegmentPattern;
use super::HostGlobSegmentPatternError;
#[cfg(unix)]
use super::dice_invariant;
#[cfg(unix)]
use super::logical_child;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
enum HostGlobPatternFragment {
    Segment(HostGlobSegmentPattern),
    RecursiveWildcard,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) enum HostGlobPatternError {
    Invalid {
        pattern: Arc<[u8]>,
        fragment_index: Option<usize>,
        reason: HostGlobInvalidPattern,
    },
    Deferred {
        pattern: Arc<[u8]>,
        fragment_index: usize,
        reason: HostGlobDeferredPattern,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(super) struct HostGlobPattern {
    bytes: Arc<[u8]>,
    fragments: Arc<[HostGlobPatternFragment]>,
}

impl HostGlobPattern {
    pub(super) fn new(bytes: impl Into<Arc<[u8]>>) -> Result<Self, HostGlobPatternError> {
        let bytes = bytes.into();
        let invalid = |fragment_index, reason| HostGlobPatternError::Invalid {
            pattern: bytes.dupe(),
            fragment_index,
            reason,
        };
        if bytes.contains(&b'?') {
            return Err(invalid(None, HostGlobInvalidPattern::QuestionMarkForbidden));
        }
        if bytes.is_empty() {
            return Err(invalid(None, HostGlobInvalidPattern::Empty));
        }
        if bytes[0] == b'/' {
            return Err(invalid(None, HostGlobInvalidPattern::Absolute));
        }

        let mut fragments = Vec::new();
        for (fragment_index, fragment) in bytes.split(|byte| *byte == b'/').enumerate() {
            if fragment.is_empty() {
                return Err(invalid(
                    Some(fragment_index),
                    HostGlobInvalidPattern::EmptySegment,
                ));
            }
            if fragment == b"." {
                return Err(invalid(
                    Some(fragment_index),
                    HostGlobInvalidPattern::DotSegment,
                ));
            }
            if fragment == b".." {
                return Err(invalid(
                    Some(fragment_index),
                    HostGlobInvalidPattern::UpLevelSegment,
                ));
            }
            if super::contains_adjacent_stars(fragment) && fragment != b"**" {
                return Err(invalid(
                    Some(fragment_index),
                    HostGlobInvalidPattern::EmbeddedRecursiveWildcard,
                ));
            }
            if fragment == b"**" {
                fragments.push(HostGlobPatternFragment::RecursiveWildcard);
                continue;
            }
            match HostGlobSegmentPattern::new(Arc::<[u8]>::from(fragment)) {
                Ok(pattern) => fragments.push(HostGlobPatternFragment::Segment(pattern)),
                Err(HostGlobSegmentPatternError::Invalid { reason, .. }) => {
                    return Err(invalid(Some(fragment_index), reason));
                }
                Err(HostGlobSegmentPatternError::Deferred { reason, .. }) => {
                    return Err(HostGlobPatternError::Deferred {
                        pattern: bytes.dupe(),
                        fragment_index,
                        reason,
                    });
                }
            }
        }
        Ok(Self {
            bytes,
            fragments: fragments.into(),
        })
    }

    fn fragments(&self) -> &[HostGlobPatternFragment] {
        &self.fragments
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(super) enum HostGlobTraversalOperation {
    Files,
    FilesAndDirs,
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
    logical_package_root: NormalizedAbsolutePath,
    package: PackagePath,
    package_bytes: Arc<[u8]>,
    pattern: HostGlobPattern,
    operation: HostGlobTraversalOperation,
}

impl HostGlobTraversalKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        logical_package_root: NormalizedAbsolutePath,
        package: PackagePath,
        pattern: HostGlobPattern,
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
            HexBytes(&self.pattern.bytes)
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
        error: HostRootPackageBoundaryError,
    },
}

pub(super) type HostGlobTraversalOutcome =
    SourcePreparationOutcome<Arc<Result<HostGlobTraversal, HostGlobTraversalError>>>;

#[derive(Clone)]
struct TraversalState {
    logical_directory: NormalizedAbsolutePath,
    package: PackagePath,
    relative_path: Arc<[u8]>,
    fragment_index: usize,
    ordinal: usize,
}

fn complete_traversal(value: HostGlobTraversal) -> HostGlobTraversalOutcome {
    SourcePreparationOutcome::Complete(Arc::new(Ok(value)))
}

fn traversal_error(error: HostGlobTraversalError) -> HostGlobTraversalOutcome {
    SourcePreparationOutcome::Complete(Arc::new(Err(error)))
}

fn add_need(needs: &mut Option<SourcePreparationNeeds>, next: SourcePreparationNeeds) {
    *needs = Some(match needs.take() {
        Some(current) => current
            .try_union(&next)
            .expect("Host glob traversal path needs cannot conflict"),
        None => next,
    });
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

fn record_error(
    first: &mut Option<((usize, usize), HostGlobTraversalError)>,
    rank: (usize, usize),
    error: HostGlobTraversalError,
) {
    if first.as_ref().is_none_or(|(current, _)| rank < *current) {
        *first = Some((rank, error));
    }
}

fn enqueue(
    frontier: &mut VecDeque<TraversalState>,
    visited: &mut Option<SmallSet<(Arc<[u8]>, usize)>>,
    next_ordinal: &mut usize,
    mut state: TraversalState,
) {
    if let Some(visited) = visited {
        if !visited.insert((state.relative_path.dupe(), state.fragment_index)) {
            return;
        }
    }
    state.ordinal = *next_ordinal;
    *next_ordinal += 1;
    frontier.push_back(state);
}

#[cfg(unix)]
impl HostGlobTraversalKey {
    async fn compute_unix(&self, ctx: &mut DiceComputations<'_>) -> HostGlobTraversalOutcome {
        let has_multiple_recursive = self
            .pattern
            .fragments()
            .iter()
            .filter(|fragment| matches!(fragment, HostGlobPatternFragment::RecursiveWildcard))
            .nth(1)
            .is_some();
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
        let mut needs = None;
        let mut first_error = None;

        while let Some(state) = frontier.pop_front() {
            let fragment = &self.pattern.fragments()[state.fragment_index];
            let (candidate_pattern, recursive) = match fragment {
                HostGlobPatternFragment::Segment(pattern) => (pattern.dupe(), false),
                HostGlobPatternFragment::RecursiveWildcard => {
                    let last = state.fragment_index + 1 == self.pattern.fragments().len();
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
                    (
                        HostGlobSegmentPattern::new(Arc::<[u8]>::from(&b"*"[..]))
                            .expect("a fixed simple wildcard is valid"),
                        true,
                    )
                }
            };
            let candidates = dice_invariant(
                ctx.compute(&HostGlobSegmentCandidatesKey::new(
                    state.logical_directory.dupe(),
                    candidate_pattern,
                ))
                .await,
            );
            let candidates = match candidates {
                SourcePreparationOutcome::Need(need) => {
                    add_need(&mut needs, need);
                    continue;
                }
                SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                    Ok(value) => value.dupe(),
                    Err(error) => {
                        record_error(
                            &mut first_error,
                            (state.ordinal, 0),
                            HostGlobTraversalError::Segment {
                                logical_directory: state.logical_directory.dupe(),
                                fragment_index: state.fragment_index,
                                error: error.dupe(),
                            },
                        );
                        continue;
                    }
                },
            };
            let last = state.fragment_index + 1 == self.pattern.fragments().len();
            for (slot, candidate) in candidates.candidates().iter().enumerate() {
                let relative_path = raw_child(&state.relative_path, &candidate.component);
                if candidate.kind == HostGlobSegmentCandidateKind::NonDirectory {
                    if last {
                        paths.push(relative_path);
                    }
                    continue;
                }
                let candidate_package = package_child(&state.package, &candidate.component);
                let boundary = dice_invariant(
                    ctx.compute(&HostRootPackageBoundaryKey::new(
                        self.workspace.dupe(),
                        candidate_package.clone(),
                    ))
                    .await,
                );
                let boundary = match boundary {
                    PathOutcome::Need(need) => {
                        add_need(&mut needs, SourcePreparationNeeds::path(need));
                        continue;
                    }
                    PathOutcome::Complete(value) => match value.as_ref() {
                        Ok(value) => value.dupe(),
                        Err(error) => {
                            record_error(
                                &mut first_error,
                                (state.ordinal, slot + 1),
                                HostGlobTraversalError::Boundary {
                                    candidate_package,
                                    error: error.clone(),
                                },
                            );
                            continue;
                        }
                    },
                };
                if matches!(
                    boundary.kind(),
                    HostRootPackageBoundaryKind::IgnoredDirectory
                        | HostRootPackageBoundaryKind::Package
                ) {
                    continue;
                }
                if last {
                    if self.operation == HostGlobTraversalOperation::FilesAndDirs && !recursive {
                        paths.push(relative_path.dupe());
                    }
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
        if let Some((_, error)) = first_error {
            traversal_error(error)
        } else if let Some(need) = needs {
            SourcePreparationOutcome::Need(need)
        } else {
            complete_traversal(HostGlobTraversal::from_paths(paths))
        }
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
        #[cfg(unix)]
        {
            self.compute_unix(ctx).await
        }
        #[cfg(not(unix))]
        {
            let _ = ctx;
            traversal_error(HostGlobTraversalError::UnsupportedHost)
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
