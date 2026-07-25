/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory.
 */

//! Operational, observation-backed path resolution.

use std::ffi::OsStr;
use std::fmt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;

use crate::NormalizedAbsolutePath;
use crate::PathLstat;
use crate::PathNodeKind;
use crate::PathObservationDemand;
use crate::PathObservationError;
use crate::PathObservationKey;
use crate::PathObservationNamespace;
use crate::PathObservationOperation;
use crate::PathObservationResult;
use crate::PathOperationResult;
use crate::PathOutcome;
use crate::PathResult;

/// One physical symlink followed while resolving a logical path.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ResolvedSymlink {
    path: NormalizedAbsolutePath,
    target: Arc<PathBuf>,
}

impl ResolvedSymlink {
    pub fn path(&self) -> &NormalizedAbsolutePath {
        &self.path
    }

    pub fn target(&self) -> &Path {
        self.target.as_path()
    }
}

/// A Bazel-shaped ordered split into the path before and the cyclic/expanding chain.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct PathResolutionChain {
    path_to: Arc<[NormalizedAbsolutePath]>,
    chain: Arc<[NormalizedAbsolutePath]>,
}

impl PathResolutionChain {
    fn split(route: &[NormalizedAbsolutePath], at: usize) -> Self {
        Self {
            path_to: Arc::from(&route[..at]),
            chain: Arc::from(&route[at..]),
        }
    }

    pub fn path_to(&self) -> &[NormalizedAbsolutePath] {
        &self.path_to
    }

    pub fn chain(&self) -> &[NormalizedAbsolutePath] {
        &self.chain
    }
}

/// The final no-follow state after resolving all symlinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum ResolvedPathState {
    Present(PathLstat),
    Missing,
}

/// One complete operational resolution.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ResolvedPath {
    namespace: PathObservationNamespace,
    requested_path: NormalizedAbsolutePath,
    real_path: NormalizedAbsolutePath,
    state: ResolvedPathState,
    route: Arc<[NormalizedAbsolutePath]>,
    symlinks: Arc<[ResolvedSymlink]>,
    ancestor_expansion: Option<PathResolutionChain>,
}

impl ResolvedPath {
    pub const fn namespace(&self) -> PathObservationNamespace {
        self.namespace
    }

    pub fn requested_path(&self) -> &NormalizedAbsolutePath {
        &self.requested_path
    }

    pub fn real_path(&self) -> &NormalizedAbsolutePath {
        &self.real_path
    }

    pub const fn state(&self) -> ResolvedPathState {
        self.state
    }

    pub fn route(&self) -> &[NormalizedAbsolutePath] {
        &self.route
    }

    pub fn symlinks(&self) -> &[ResolvedSymlink] {
        &self.symlinks
    }

    pub fn ancestor_expansion(&self) -> Option<&PathResolutionChain> {
        self.ancestor_expansion.as_ref()
    }
}

/// An exact operational resolver failure.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum PathResolutionError {
    Observation {
        namespace: PathObservationNamespace,
        requested_path: NormalizedAbsolutePath,
        demand: PathObservationDemand,
        error: PathObservationError,
    },
    InconsistentState {
        namespace: PathObservationNamespace,
        requested_path: NormalizedAbsolutePath,
        demand: PathObservationDemand,
        before: Option<PathLstat>,
        after: Option<PathLstat>,
    },
    Cycle {
        namespace: PathObservationNamespace,
        requested_path: NormalizedAbsolutePath,
        chain: PathResolutionChain,
    },
    InfiniteExpansion {
        namespace: PathObservationNamespace,
        requested_path: NormalizedAbsolutePath,
        chain: PathResolutionChain,
    },
}

impl PathResolutionError {
    pub const fn namespace(&self) -> PathObservationNamespace {
        match self {
            Self::Observation { namespace, .. }
            | Self::InconsistentState { namespace, .. }
            | Self::Cycle { namespace, .. }
            | Self::InfiniteExpansion { namespace, .. } => *namespace,
        }
    }

    pub fn requested_path(&self) -> &NormalizedAbsolutePath {
        match self {
            Self::Observation { requested_path, .. }
            | Self::InconsistentState { requested_path, .. }
            | Self::Cycle { requested_path, .. }
            | Self::InfiniteExpansion { requested_path, .. } => requested_path,
        }
    }
}

/// Resolves an exact logical path without filesystem IO.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct ResolvedPathKey {
    namespace: PathObservationNamespace,
    logical_path: NormalizedAbsolutePath,
}

impl ResolvedPathKey {
    pub fn new(namespace: PathObservationNamespace, logical_path: NormalizedAbsolutePath) -> Self {
        Self {
            namespace,
            logical_path,
        }
    }

    pub const fn namespace(&self) -> PathObservationNamespace {
        self.namespace
    }

    pub fn logical_path(&self) -> &NormalizedAbsolutePath {
        &self.logical_path
    }
}

impl fmt::Display for ResolvedPathKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "resolved-path:{:?}:{:?}",
            self.namespace,
            self.logical_path.as_path()
        )
    }
}

#[derive(Debug, Clone)]
enum ResolverPhase {
    Begin,
    AwaitParent {
        basename: Arc<OsStr>,
    },
    ReplayParentRoute {
        entries: Vec<NormalizedAbsolutePath>,
        next: usize,
        real_child: NormalizedAbsolutePath,
    },
    AwaitLstat {
        real_path: NormalizedAbsolutePath,
    },
    AwaitReadLink {
        physical_link: NormalizedAbsolutePath,
        lstat: PathLstat,
    },
}

#[derive(Debug, Clone)]
struct ResolverFrame {
    original_path: NormalizedAbsolutePath,
    current_path: NormalizedAbsolutePath,
    phase: ResolverPhase,
    logical_chain: Vec<NormalizedAbsolutePath>,
    sorted_logical_chain: Vec<NormalizedAbsolutePath>,
    symlinks: Vec<ResolvedSymlink>,
    ancestor_expansion: Option<PathResolutionChain>,
}

impl ResolverFrame {
    fn new(requested_path: NormalizedAbsolutePath) -> Self {
        Self {
            original_path: requested_path.dupe(),
            current_path: requested_path,
            phase: ResolverPhase::Begin,
            logical_chain: Vec::new(),
            sorted_logical_chain: Vec::new(),
            symlinks: Vec::new(),
            ancestor_expansion: None,
        }
    }

    fn completion(
        &self,
        namespace: PathObservationNamespace,
        real_path: NormalizedAbsolutePath,
        state: ResolvedPathState,
    ) -> ResolvedPath {
        ResolvedPath {
            namespace,
            requested_path: self.original_path.dupe(),
            real_path,
            state,
            route: Arc::from(self.logical_chain.as_slice()),
            symlinks: Arc::from(self.symlinks.as_slice()),
            ancestor_expansion: self.ancestor_expansion.dupe(),
        }
    }

    fn check_only(&mut self, candidate: NormalizedAbsolutePath) -> RouteAdmission {
        self.route_candidate(candidate, false)
    }

    fn admit(&mut self, candidate: NormalizedAbsolutePath) -> RouteAdmission {
        self.route_candidate(candidate, true)
    }

    fn route_candidate(
        &mut self,
        candidate: NormalizedAbsolutePath,
        admit: bool,
    ) -> RouteAdmission {
        match self.sorted_logical_chain.binary_search(&candidate) {
            Ok(sorted_index) => {
                let repeated = &self.sorted_logical_chain[sorted_index];
                let at = self
                    .logical_chain
                    .iter()
                    .position(|path| path == repeated)
                    .expect("sorted and ordered logical chains have identical members");
                RouteAdmission::Terminal(ResolutionCause::Cycle(PathResolutionChain::split(
                    &self.logical_chain,
                    at,
                )))
            }
            Err(sorted_index) => {
                if let Some(predecessor) = sorted_index
                    .checked_sub(1)
                    .and_then(|index| self.sorted_logical_chain.get(index))
                {
                    if is_strict_descendant(&candidate, predecessor) {
                        let at = self
                            .logical_chain
                            .iter()
                            .position(|path| path == predecessor)
                            .expect("sorted and ordered logical chains have identical members");
                        let mut combined = self.logical_chain.clone();
                        combined.push(candidate);
                        return RouteAdmission::Terminal(ResolutionCause::InfiniteExpansion(
                            PathResolutionChain::split(&combined, at),
                        ));
                    }
                }
                if let Some(successor) = self.sorted_logical_chain.get(sorted_index) {
                    if is_strict_descendant(successor, &candidate)
                        && self.ancestor_expansion.is_none()
                    {
                        let at = self
                            .logical_chain
                            .iter()
                            .position(|path| path == successor)
                            .expect("sorted and ordered logical chains have identical members");
                        let mut combined = self.logical_chain.clone();
                        combined.push(candidate.dupe());
                        self.ancestor_expansion = Some(PathResolutionChain::split(&combined, at));
                    }
                }
                if admit {
                    self.sorted_logical_chain
                        .insert(sorted_index, candidate.dupe());
                    self.logical_chain.push(candidate);
                }
                RouteAdmission::Continue
            }
        }
    }
}

#[derive(Debug)]
enum ResolutionCause {
    Observation {
        demand: PathObservationDemand,
        error: PathObservationError,
    },
    InconsistentState {
        demand: PathObservationDemand,
        before: Option<PathLstat>,
        after: Option<PathLstat>,
    },
    Cycle(PathResolutionChain),
    InfiniteExpansion(PathResolutionChain),
}

impl ResolutionCause {
    fn with_outer_context(
        self,
        namespace: PathObservationNamespace,
        requested_path: NormalizedAbsolutePath,
    ) -> PathResolutionError {
        match self {
            Self::Observation { demand, error } => PathResolutionError::Observation {
                namespace,
                requested_path,
                demand,
                error,
            },
            Self::InconsistentState {
                demand,
                before,
                after,
            } => PathResolutionError::InconsistentState {
                namespace,
                requested_path,
                demand,
                before,
                after,
            },
            Self::Cycle(chain) => PathResolutionError::Cycle {
                namespace,
                requested_path,
                chain,
            },
            Self::InfiniteExpansion(chain) => PathResolutionError::InfiniteExpansion {
                namespace,
                requested_path,
                chain,
            },
        }
    }
}

enum RouteAdmission {
    Continue,
    Terminal(ResolutionCause),
}

#[derive(Debug)]
enum MachineStep {
    PushParent(NormalizedAbsolutePath),
    Observe(PathObservationDemand),
    Complete(Result<ResolvedPath, ResolutionCause>),
}

/// The pure state machine. Only the async adapter below performs DICE awaits.
struct ResolutionMachine {
    namespace: PathObservationNamespace,
    frames: Vec<ResolverFrame>,
}

impl ResolutionMachine {
    fn new(namespace: PathObservationNamespace, requested_path: NormalizedAbsolutePath) -> Self {
        Self {
            namespace,
            frames: vec![ResolverFrame::new(requested_path)],
        }
    }

    fn demand(
        &self,
        path: NormalizedAbsolutePath,
        operation: PathObservationOperation,
    ) -> MachineStep {
        MachineStep::Observe(PathObservationDemand::new(self.namespace, path, operation))
    }

    fn transition(&mut self) -> MachineStep {
        loop {
            let frame = self
                .frames
                .last_mut()
                .expect("resolution machine always has an active frame");
            match &mut frame.phase {
                ResolverPhase::Begin => {
                    if let Some((parent, basename)) = parent_and_basename(&frame.current_path) {
                        frame.phase = ResolverPhase::AwaitParent { basename };
                        return MachineStep::PushParent(parent);
                    }
                    match frame.admit(frame.current_path.dupe()) {
                        RouteAdmission::Continue => {
                            let root = frame.current_path.dupe();
                            frame.phase = ResolverPhase::AwaitLstat {
                                real_path: root.dupe(),
                            };
                            return self.demand(root, PathObservationOperation::Lstat);
                        }
                        RouteAdmission::Terminal(cause) => {
                            return MachineStep::Complete(Err(cause));
                        }
                    }
                }
                ResolverPhase::ReplayParentRoute {
                    entries,
                    next,
                    real_child,
                } => {
                    if *next == entries.len() {
                        let real_path = real_child.dupe();
                        frame.phase = ResolverPhase::AwaitLstat {
                            real_path: real_path.dupe(),
                        };
                        return self.demand(real_path, PathObservationOperation::Lstat);
                    }
                    let candidate = entries[*next].dupe();
                    *next += 1;
                    if let RouteAdmission::Terminal(cause) = frame.admit(candidate) {
                        return MachineStep::Complete(Err(cause));
                    }
                }
                ResolverPhase::AwaitParent { .. }
                | ResolverPhase::AwaitLstat { .. }
                | ResolverPhase::AwaitReadLink { .. } => {
                    unreachable!("transition requires a pending parent or observation result")
                }
            }
        }
    }

    fn push_parent(&mut self, parent: NormalizedAbsolutePath) {
        self.frames.push(ResolverFrame::new(parent));
    }

    fn finish_frame(
        &mut self,
        result: Result<ResolvedPath, ResolutionCause>,
    ) -> Option<MachineStep> {
        self.frames
            .pop()
            .expect("frame completion requires a frame");
        let Some(caller) = self.frames.last_mut() else {
            return Some(MachineStep::Complete(result));
        };
        let ResolverPhase::AwaitParent { basename } = &caller.phase else {
            unreachable!("only a suspended caller receives parent completion")
        };
        let basename = basename.dupe();
        match result {
            Err(cause) => Some(MachineStep::Complete(Err(cause))),
            Ok(parent) => {
                caller.symlinks.extend(parent.symlinks.iter().cloned());
                let real_child = join_normalized(parent.real_path(), basename.as_ref());
                match parent.state() {
                    ResolvedPathState::Missing => Some(MachineStep::Complete(Ok(
                        caller.completion(self.namespace, real_child, ResolvedPathState::Missing)
                    ))),
                    ResolvedPathState::Present(lstat)
                        if lstat.kind() != PathNodeKind::Directory =>
                    {
                        Some(MachineStep::Complete(Ok(caller.completion(
                            self.namespace,
                            real_child,
                            ResolvedPathState::Missing,
                        ))))
                    }
                    ResolvedPathState::Present(_) => {
                        let entries = parent
                            .route()
                            .iter()
                            .map(|path| join_normalized(path, basename.as_ref()))
                            .collect();
                        caller.phase = ResolverPhase::ReplayParentRoute {
                            entries,
                            next: 0,
                            real_child,
                        };
                        None
                    }
                }
            }
        }
    }

    fn observe(
        &mut self,
        demand: PathObservationDemand,
        result: Arc<PathObservationResult>,
    ) -> Option<MachineStep> {
        let namespace = self.namespace;
        let frame = self
            .frames
            .last_mut()
            .expect("observation requires an active frame");
        match (frame.phase.clone(), result.as_ref()) {
            (ResolverPhase::AwaitLstat { real_path }, PathObservationResult::Lstat(result)) => {
                match result {
                    PathOperationResult::Present(lstat)
                        if lstat.kind() == PathNodeKind::Symlink =>
                    {
                        frame.phase = ResolverPhase::AwaitReadLink {
                            physical_link: real_path.dupe(),
                            lstat: *lstat,
                        };
                        Some(MachineStep::Observe(PathObservationDemand::new(
                            namespace,
                            real_path,
                            PathObservationOperation::ReadLink,
                        )))
                    }
                    PathOperationResult::Present(lstat) => Some(MachineStep::Complete(Ok(
                        frame.completion(namespace, real_path, ResolvedPathState::Present(*lstat))
                    ))),
                    PathOperationResult::Missing => Some(MachineStep::Complete(Ok(
                        frame.completion(namespace, real_path, ResolvedPathState::Missing)
                    ))),
                    PathOperationResult::Error(error) => {
                        Some(MachineStep::Complete(Err(ResolutionCause::Observation {
                            demand,
                            error: *error,
                        })))
                    }
                }
            }
            (
                ResolverPhase::AwaitReadLink {
                    physical_link,
                    lstat,
                },
                PathObservationResult::ReadLink(result),
            ) => match result {
                PathOperationResult::Present(target) => {
                    let target_path = normalize_link_target(&physical_link, target.as_path());
                    frame.symlinks.push(ResolvedSymlink {
                        path: physical_link,
                        target: target.dupe(),
                    });
                    match frame.check_only(target_path.dupe()) {
                        RouteAdmission::Terminal(cause) => Some(MachineStep::Complete(Err(cause))),
                        RouteAdmission::Continue => {
                            frame.current_path = target_path;
                            frame.phase = ResolverPhase::Begin;
                            None
                        }
                    }
                }
                PathOperationResult::Missing => Some(MachineStep::Complete(Err(
                    ResolutionCause::InconsistentState {
                        demand,
                        before: Some(lstat),
                        after: None,
                    },
                ))),
                PathOperationResult::Error(error) => {
                    Some(MachineStep::Complete(Err(ResolutionCause::Observation {
                        demand,
                        error: *error,
                    })))
                }
            },
            _ => unreachable!("observation operation must match the pending machine phase"),
        }
    }
}

fn is_strict_descendant(path: &NormalizedAbsolutePath, ancestor: &NormalizedAbsolutePath) -> bool {
    path != ancestor && path.as_path().starts_with(ancestor.as_path())
}

fn parent_and_basename(
    path: &NormalizedAbsolutePath,
) -> Option<(NormalizedAbsolutePath, Arc<OsStr>)> {
    let parent = path.as_path().parent()?;
    let basename = path.as_path().file_name()?;
    Some((
        NormalizedAbsolutePath::new(parent.to_path_buf())
            .expect("parent of an absolute path is absolute"),
        Arc::from(basename),
    ))
}

fn filesystem_root(path: &Path) -> PathBuf {
    let mut root = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => root.push(prefix.as_os_str()),
            Component::RootDir => {
                root.push(component.as_os_str());
                break;
            }
            Component::CurDir | Component::ParentDir | Component::Normal(_) => break,
        }
    }
    root
}

fn join_normalized(parent: &NormalizedAbsolutePath, basename: &OsStr) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(parent.as_path().join(basename))
        .expect("an absolute parent joined with a basename is absolute")
}

fn normalize_link_target(
    physical_link: &NormalizedAbsolutePath,
    target: &Path,
) -> NormalizedAbsolutePath {
    let path = if target.is_absolute() {
        target.to_path_buf()
    } else {
        physical_link
            .as_path()
            .parent()
            .map(|parent| parent.join(target))
            .unwrap_or_else(|| filesystem_root(physical_link.as_path()).join(target))
    };
    NormalizedAbsolutePath::new(path)
        .expect("a link target resolved from an absolute base is absolute")
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("path-resolution DICE invariant failed: {error:?}"))
}

#[async_trait]
impl Key for ResolvedPathKey {
    type Value = PathResult<ResolvedPath, PathResolutionError>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let mut machine = ResolutionMachine::new(self.namespace, self.logical_path.dupe());
        let mut next = None;
        loop {
            let step = next.take().unwrap_or_else(|| machine.transition());
            match step {
                MachineStep::PushParent(parent) => machine.push_parent(parent),
                MachineStep::Observe(demand) => {
                    let observed =
                        dice_invariant(ctx.compute(&PathObservationKey::new(demand.dupe())).await);
                    match observed {
                        PathOutcome::Need(need) => return PathOutcome::Need(need),
                        PathOutcome::Complete(result) => next = machine.observe(demand, result),
                    }
                }
                MachineStep::Complete(result) => {
                    if let Some(step) = machine.finish_frame(result) {
                        if machine.frames.is_empty() {
                            let MachineStep::Complete(result) = step else {
                                unreachable!("root-frame completion is terminal")
                            };
                            return PathOutcome::Complete(result.map_err(|cause| {
                                cause.with_outer_context(self.namespace, self.logical_path.dupe())
                            }));
                        }
                        next = Some(step);
                    }
                }
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
mod tests {
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;

    use dice::DetectCycles;
    use dice::Dice;
    use dice::Key;
    use dupe::Dupe;

    use super::PathResolutionChain;
    use super::PathResolutionError;
    use super::ResolvedPath;
    use super::ResolvedPathKey;
    use super::ResolvedPathState;
    use crate::NeedPathObservations;
    use crate::NormalizedAbsolutePath;
    use crate::PathIoErrorKind;
    use crate::PathLstat;
    use crate::PathNodeKind;
    use crate::PathObservationDemand;
    use crate::PathObservationEpoch;
    use crate::PathObservationEpochKey;
    use crate::PathObservationError;
    use crate::PathObservationInstanceId;
    use crate::PathObservationNamespace;
    use crate::PathObservationOperation;
    use crate::PathObservationResult;
    use crate::PathOperationResult;
    use crate::PathOutcome;

    type ScriptEntry = (PathObservationDemand, PathObservationResult);

    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }

    fn lstat(kind: PathNodeKind) -> PathLstat {
        PathLstat::new(kind, 1, 2, 3, 4, 0o755)
    }

    fn demand(
        namespace: PathObservationNamespace,
        value: &str,
        operation: PathObservationOperation,
    ) -> PathObservationDemand {
        PathObservationDemand::new(namespace, path(value), operation)
    }

    fn observed_lstat(
        namespace: PathObservationNamespace,
        value: &str,
        result: PathOperationResult<PathLstat>,
    ) -> ScriptEntry {
        (
            demand(namespace, value, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(result),
        )
    }

    fn present(
        namespace: PathObservationNamespace,
        value: &str,
        kind: PathNodeKind,
    ) -> ScriptEntry {
        observed_lstat(namespace, value, PathOperationResult::Present(lstat(kind)))
    }

    fn missing(namespace: PathObservationNamespace, value: &str) -> ScriptEntry {
        observed_lstat(namespace, value, PathOperationResult::Missing)
    }

    fn lstat_error(
        namespace: PathObservationNamespace,
        value: &str,
        error: PathObservationError,
    ) -> ScriptEntry {
        observed_lstat(namespace, value, PathOperationResult::Error(error))
    }

    fn read_link(
        namespace: PathObservationNamespace,
        value: &str,
        target: impl Into<PathBuf>,
    ) -> ScriptEntry {
        (
            demand(namespace, value, PathObservationOperation::ReadLink),
            PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(target.into()))),
        )
    }

    fn read_link_result(
        namespace: PathObservationNamespace,
        value: &str,
        result: PathOperationResult<Arc<PathBuf>>,
    ) -> ScriptEntry {
        (
            demand(namespace, value, PathObservationOperation::ReadLink),
            PathObservationResult::ReadLink(result),
        )
    }

    async fn resolve_script(
        namespace: PathObservationNamespace,
        logical_path: &str,
        script: &[ScriptEntry],
    ) -> Result<ResolvedPath, PathResolutionError> {
        let key = ResolvedPathKey::new(namespace, path(logical_path));
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater().commit().await;

        for prefix_len in 0..=script.len() {
            let epoch = PathObservationEpoch::new(
                script[..prefix_len]
                    .iter()
                    .map(|(demand, result)| (demand.dupe(), result.dupe())),
            )
            .unwrap();
            let mut updater = transaction.into_updater();
            updater
                .changed_to(vec![(PathObservationEpochKey, epoch)])
                .unwrap();
            transaction = updater.commit().await;

            let outcome = transaction.compute(&key).await.unwrap();
            if prefix_len < script.len() {
                let PathOutcome::Need(need) = &outcome else {
                    panic!(
                        "script for {logical_path:?} completed after {prefix_len} of {} observations",
                        script.len()
                    );
                };
                assert_eq!(
                    need.demands(),
                    &[script[prefix_len].0.dupe()],
                    "unexpected demand after script prefix {prefix_len} for {logical_path:?}"
                );
                assert!(!ResolvedPathKey::validity(&outcome));
                assert!(!ResolvedPathKey::equality(&outcome, &outcome));
            } else {
                let PathOutcome::Complete(result) = outcome else {
                    panic!("full script for {logical_path:?} did not complete");
                };
                assert!(ResolvedPathKey::validity(&PathOutcome::Complete(
                    result.dupe()
                )));
                assert!(ResolvedPathKey::equality(
                    &PathOutcome::Complete(result.dupe()),
                    &PathOutcome::Complete(result.dupe())
                ));
                return result;
            }
        }
        unreachable!("inclusive prefix loop always reaches the full script")
    }

    fn assert_paths(actual: &[NormalizedAbsolutePath], expected: &[&str]) {
        assert_eq!(
            actual.iter().map(|path| path.as_path()).collect::<Vec<_>>(),
            expected.iter().map(Path::new).collect::<Vec<_>>()
        );
    }

    fn assert_complete(
        result: &ResolvedPath,
        namespace: PathObservationNamespace,
        requested: &str,
        real: &str,
        state: ResolvedPathState,
        route: &[&str],
    ) {
        assert_eq!(result.namespace(), namespace);
        assert_eq!(result.requested_path().as_path(), Path::new(requested));
        assert_eq!(result.real_path().as_path(), Path::new(real));
        assert_eq!(result.state(), state);
        assert_paths(result.route(), route);
    }

    fn assert_chain(chain: &PathResolutionChain, path_to: &[&str], cyclic: &[&str]) {
        assert_paths(chain.path_to(), path_to);
        assert_paths(chain.chain(), cyclic);
    }

    fn assert_error_context(
        error: &PathResolutionError,
        namespace: PathObservationNamespace,
        requested_path: &str,
    ) {
        assert_eq!(error.namespace(), namespace);
        assert_eq!(error.requested_path().as_path(), Path::new(requested_path));
    }

    fn assert_symlinks(resolved: &ResolvedPath, expected: &[(&str, &str)]) {
        assert_eq!(resolved.symlinks().len(), expected.len());
        for (actual, (physical, target)) in resolved.symlinks().iter().zip(expected) {
            assert_eq!(actual.path().as_path(), Path::new(physical));
            assert_eq!(actual.target(), Path::new(target));
        }
    }

    fn assert_error_not_equal(left: &PathResolutionError, right: PathResolutionError) {
        let left = PathOutcome::Complete(Err::<ResolvedPath, _>(left.dupe()));
        let right = PathOutcome::Complete(Err::<ResolvedPath, _>(right));
        assert!(!ResolvedPathKey::equality(&left, &right));
    }

    fn host_root() -> ScriptEntry {
        present(PathObservationNamespace::Host, "/", PathNodeKind::Directory)
    }

    #[test]
    fn path_resolution_public_schema_roots_and_pure_route_admission() {
        let host = PathObservationNamespace::Host;
        let materialized =
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(9));
        let host_key = ResolvedPathKey::new(host, path("/x"));
        let materialized_key = ResolvedPathKey::new(materialized, path("/x"));
        assert_eq!(host_key.namespace(), host);
        assert_eq!(host_key.logical_path().as_path(), Path::new("/x"));
        assert_ne!(host_key, materialized_key);
        assert!(host_key.to_string().contains("resolved-path"));

        let mut frame = super::ResolverFrame::new(path("/prefix"));
        assert!(matches!(
            frame.admit(path("/prefix")),
            super::RouteAdmission::Continue
        ));
        let super::RouteAdmission::Terminal(super::ResolutionCause::Cycle(chain)) =
            frame.check_only(path("/prefix"))
        else {
            panic!("an exact repeat must be a cycle");
        };
        assert_chain(&chain, &[], &["/prefix"]);

        let mut frame = super::ResolverFrame::new(path("/prefix"));
        assert!(matches!(
            frame.admit(path("/prefix")),
            super::RouteAdmission::Continue
        ));
        let super::RouteAdmission::Terminal(super::ResolutionCause::InfiniteExpansion(chain)) =
            frame.check_only(path("/prefix/child"))
        else {
            panic!("a descendant expansion must be terminal");
        };
        assert_chain(&chain, &[], &["/prefix", "/prefix/child"]);

        let mut frame = super::ResolverFrame::new(path("/a/a"));
        assert!(matches!(
            frame.admit(path("/a/a")),
            super::RouteAdmission::Continue
        ));
        assert!(matches!(
            frame.check_only(path("/a")),
            super::RouteAdmission::Continue
        ));
        assert_chain(
            frame.ancestor_expansion.as_ref().unwrap(),
            &[],
            &["/a/a", "/a"],
        );

        assert!(super::parent_and_basename(&path("/")).is_none());
        let (parent, basename) = super::parent_and_basename(&path("/a")).unwrap();
        assert_eq!(parent.as_path(), Path::new("/"));
        assert_eq!(basename.as_ref(), std::ffi::OsStr::new("a"));
        assert_eq!(
            super::normalize_link_target(&path("/repo/link"), Path::new("../../outside")).as_path(),
            Path::new("/outside")
        );
        assert_eq!(
            super::normalize_link_target(&path("/clamp"), Path::new("/../../target")).as_path(),
            Path::new("/target")
        );
    }

    #[test]
    fn path_resolution_complete_equality_is_fully_structural_and_need_is_transient() {
        let ns = PathObservationNamespace::Host;
        let symlink = super::ResolvedSymlink {
            path: path("/link"),
            target: Arc::new(PathBuf::from("target")),
        };
        assert_eq!(symlink.path().as_path(), Path::new("/link"));
        assert_eq!(symlink.target(), Path::new("target"));
        let marker = PathResolutionChain {
            path_to: Arc::from([path("/link")]),
            chain: Arc::from([path("/target/child"), path("/target")]),
        };
        let baseline = ResolvedPath {
            namespace: ns,
            requested_path: path("/link"),
            real_path: path("/target"),
            state: ResolvedPathState::Present(lstat(PathNodeKind::RegularFile)),
            route: Arc::from([path("/link"), path("/target")]),
            symlinks: Arc::from([symlink]),
            ancestor_expansion: Some(marker),
        };
        let complete = PathOutcome::Complete(Ok::<_, PathResolutionError>(baseline.dupe()));
        assert!(ResolvedPathKey::equality(&complete, &complete));
        assert!(ResolvedPathKey::validity(&complete));

        let mut variants = Vec::new();
        let mut changed = baseline.dupe();
        changed.namespace =
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(1));
        variants.push(changed);
        let mut changed = baseline.dupe();
        changed.requested_path = path("/other");
        variants.push(changed);
        let mut changed = baseline.dupe();
        changed.real_path = path("/other");
        variants.push(changed);
        let mut changed = baseline.dupe();
        changed.state = ResolvedPathState::Missing;
        variants.push(changed);
        let mut changed = baseline.dupe();
        changed.route = Arc::from([path("/link"), path("/other")]);
        variants.push(changed);
        let mut changed = baseline.dupe();
        changed.symlinks = Arc::from([super::ResolvedSymlink {
            path: path("/link"),
            target: Arc::new(PathBuf::from("other")),
        }]);
        variants.push(changed);
        let mut changed = baseline.dupe();
        changed.ancestor_expansion = None;
        variants.push(changed);
        for changed in variants {
            assert!(!ResolvedPathKey::equality(
                &complete,
                &PathOutcome::Complete(Ok(changed))
            ));
        }

        let observation_error = PathResolutionError::Observation {
            namespace: ns,
            requested_path: path("/logical"),
            demand: demand(ns, "/physical", PathObservationOperation::Lstat),
            error: PathObservationError::Io {
                kind: PathIoErrorKind::PermissionDenied,
                raw_os_error: Some(13),
            },
        };
        let error_complete =
            PathOutcome::Complete(Err::<ResolvedPath, _>(observation_error.dupe()));
        assert!(ResolvedPathKey::equality(
            &error_complete,
            &PathOutcome::Complete(Err(observation_error.dupe()))
        ));
        for changed in [
            PathResolutionError::Observation {
                namespace: PathObservationNamespace::Materialization(
                    PathObservationInstanceId::new(2),
                ),
                requested_path: path("/logical"),
                demand: demand(ns, "/physical", PathObservationOperation::Lstat),
                error: PathObservationError::Io {
                    kind: PathIoErrorKind::PermissionDenied,
                    raw_os_error: Some(13),
                },
            },
            PathResolutionError::Observation {
                namespace: ns,
                requested_path: path("/other"),
                demand: demand(ns, "/physical", PathObservationOperation::Lstat),
                error: PathObservationError::Io {
                    kind: PathIoErrorKind::PermissionDenied,
                    raw_os_error: Some(13),
                },
            },
            PathResolutionError::Observation {
                namespace: ns,
                requested_path: path("/logical"),
                demand: demand(
                    PathObservationNamespace::Materialization(PathObservationInstanceId::new(3)),
                    "/physical",
                    PathObservationOperation::Lstat,
                ),
                error: PathObservationError::Io {
                    kind: PathIoErrorKind::PermissionDenied,
                    raw_os_error: Some(13),
                },
            },
            PathResolutionError::Observation {
                namespace: ns,
                requested_path: path("/logical"),
                demand: demand(ns, "/other", PathObservationOperation::Lstat),
                error: PathObservationError::Io {
                    kind: PathIoErrorKind::PermissionDenied,
                    raw_os_error: Some(13),
                },
            },
            PathResolutionError::Observation {
                namespace: ns,
                requested_path: path("/logical"),
                demand: demand(ns, "/physical", PathObservationOperation::ReadLink),
                error: PathObservationError::Io {
                    kind: PathIoErrorKind::PermissionDenied,
                    raw_os_error: Some(13),
                },
            },
            PathResolutionError::Observation {
                namespace: ns,
                requested_path: path("/logical"),
                demand: demand(ns, "/physical", PathObservationOperation::Lstat),
                error: PathObservationError::Io {
                    kind: PathIoErrorKind::TimedOut,
                    raw_os_error: Some(13),
                },
            },
            PathResolutionError::Observation {
                namespace: ns,
                requested_path: path("/logical"),
                demand: demand(ns, "/physical", PathObservationOperation::Lstat),
                error: PathObservationError::Io {
                    kind: PathIoErrorKind::PermissionDenied,
                    raw_os_error: Some(5),
                },
            },
        ] {
            assert_error_not_equal(&observation_error, changed);
        }

        let inconsistent = PathResolutionError::InconsistentState {
            namespace: ns,
            requested_path: path("/logical"),
            demand: demand(ns, "/physical", PathObservationOperation::ReadLink),
            before: Some(lstat(PathNodeKind::Symlink)),
            after: None,
        };
        assert_error_not_equal(
            &inconsistent,
            PathResolutionError::InconsistentState {
                namespace: ns,
                requested_path: path("/logical"),
                demand: demand(ns, "/physical", PathObservationOperation::ReadLink),
                before: Some(lstat(PathNodeKind::RegularFile)),
                after: None,
            },
        );
        assert_error_not_equal(
            &inconsistent,
            PathResolutionError::InconsistentState {
                namespace: ns,
                requested_path: path("/logical"),
                demand: demand(ns, "/physical", PathObservationOperation::ReadLink),
                before: Some(lstat(PathNodeKind::Symlink)),
                after: Some(lstat(PathNodeKind::RegularFile)),
            },
        );

        let cycle = PathResolutionError::Cycle {
            namespace: ns,
            requested_path: path("/logical"),
            chain: PathResolutionChain {
                path_to: Arc::from([path("/prefix")]),
                chain: Arc::from([path("/a"), path("/b")]),
            },
        };
        for changed in [
            PathResolutionError::Cycle {
                namespace: PathObservationNamespace::Materialization(
                    PathObservationInstanceId::new(4),
                ),
                requested_path: path("/logical"),
                chain: PathResolutionChain {
                    path_to: Arc::from([path("/prefix")]),
                    chain: Arc::from([path("/a"), path("/b")]),
                },
            },
            PathResolutionError::Cycle {
                namespace: ns,
                requested_path: path("/other"),
                chain: PathResolutionChain {
                    path_to: Arc::from([path("/prefix")]),
                    chain: Arc::from([path("/a"), path("/b")]),
                },
            },
            PathResolutionError::InfiniteExpansion {
                namespace: ns,
                requested_path: path("/logical"),
                chain: PathResolutionChain {
                    path_to: Arc::from([path("/prefix")]),
                    chain: Arc::from([path("/a"), path("/b")]),
                },
            },
            PathResolutionError::Cycle {
                namespace: ns,
                requested_path: path("/logical"),
                chain: PathResolutionChain {
                    path_to: Arc::from([path("/prefix")]),
                    chain: Arc::from([path("/a"), path("/other")]),
                },
            },
            PathResolutionError::Cycle {
                namespace: ns,
                requested_path: path("/logical"),
                chain: PathResolutionChain {
                    path_to: Arc::from([]),
                    chain: Arc::from([path("/prefix"), path("/a"), path("/b")]),
                },
            },
            PathResolutionError::Cycle {
                namespace: ns,
                requested_path: path("/logical"),
                chain: PathResolutionChain {
                    path_to: Arc::from([path("/other")]),
                    chain: Arc::from([path("/a"), path("/b")]),
                },
            },
            PathResolutionError::Cycle {
                namespace: ns,
                requested_path: path("/logical"),
                chain: PathResolutionChain {
                    path_to: Arc::from([path("/prefix")]),
                    chain: Arc::from([path("/b"), path("/a")]),
                },
            },
        ] {
            assert_error_not_equal(&cycle, changed);
        }

        let need = PathOutcome::<Result<ResolvedPath, PathResolutionError>>::Need(
            NeedPathObservations::singleton(demand(
                ns,
                "/physical",
                PathObservationOperation::Lstat,
            )),
        );
        assert!(!ResolvedPathKey::validity(&need));
        assert!(!ResolvedPathKey::equality(&need, &need));
        assert!(!ResolvedPathKey::equality(&complete, &need));
    }

    #[tokio::test]
    async fn path_resolution_basics_suppress_children_and_preserve_routes() {
        let ns = PathObservationNamespace::Host;

        let ordinary = resolve_script(
            ns,
            "/a/b/c",
            &[
                host_root(),
                present(ns, "/a", PathNodeKind::Directory),
                present(ns, "/a/b", PathNodeKind::Directory),
                present(ns, "/a/b/c", PathNodeKind::RegularFile),
            ],
        )
        .await
        .unwrap();
        assert_complete(
            &ordinary,
            ns,
            "/a/b/c",
            "/a/b/c",
            ResolvedPathState::Present(lstat(PathNodeKind::RegularFile)),
            &["/a/b/c"],
        );
        assert!(ordinary.symlinks().is_empty());
        assert!(ordinary.ancestor_expansion().is_none());

        let root_missing = resolve_script(ns, "/a/b", &[missing(ns, "/")])
            .await
            .unwrap();
        assert_complete(
            &root_missing,
            ns,
            "/a/b",
            "/a/b",
            ResolvedPathState::Missing,
            &[],
        );

        let non_directory = resolve_script(
            ns,
            "/a/b",
            &[host_root(), present(ns, "/a", PathNodeKind::RegularFile)],
        )
        .await
        .unwrap();
        assert_complete(
            &non_directory,
            ns,
            "/a/b",
            "/a/b",
            ResolvedPathState::Missing,
            &[],
        );

        let special = resolve_script(
            ns,
            "/special",
            &[
                host_root(),
                present(ns, "/special", PathNodeKind::SpecialFile),
            ],
        )
        .await
        .unwrap();
        assert_complete(
            &special,
            ns,
            "/special",
            "/special",
            ResolvedPathState::Present(lstat(PathNodeKind::SpecialFile)),
            &["/special"],
        );
    }

    #[tokio::test]
    async fn path_resolution_follows_direct_transitive_parent_and_escaping_links() {
        let ns = PathObservationNamespace::Host;

        for (logical, target) in [("/relative", "target"), ("/absolute", "/target")] {
            let resolved = resolve_script(
                ns,
                logical,
                &[
                    host_root(),
                    present(ns, logical, PathNodeKind::Symlink),
                    read_link(ns, logical, target),
                    present(ns, "/target", PathNodeKind::RegularFile),
                ],
            )
            .await
            .unwrap();
            assert_complete(
                &resolved,
                ns,
                logical,
                "/target",
                ResolvedPathState::Present(lstat(PathNodeKind::RegularFile)),
                &[logical, "/target"],
            );
            assert_symlinks(&resolved, &[(logical, target)]);
        }

        let escaped = resolve_script(
            ns,
            "/repo/link",
            &[
                host_root(),
                present(ns, "/repo", PathNodeKind::Directory),
                present(ns, "/repo/link", PathNodeKind::Symlink),
                read_link(ns, "/repo/link", "../../outside"),
                present(ns, "/outside", PathNodeKind::RegularFile),
            ],
        )
        .await
        .unwrap();
        assert_complete(
            &escaped,
            ns,
            "/repo/link",
            "/outside",
            ResolvedPathState::Present(lstat(PathNodeKind::RegularFile)),
            &["/repo/link", "/outside"],
        );
        assert_symlinks(&escaped, &[("/repo/link", "../../outside")]);

        let clamped = resolve_script(
            ns,
            "/clamp",
            &[
                host_root(),
                present(ns, "/clamp", PathNodeKind::Symlink),
                read_link(ns, "/clamp", "/../../target"),
                present(ns, "/target", PathNodeKind::RegularFile),
            ],
        )
        .await
        .unwrap();
        assert_complete(
            &clamped,
            ns,
            "/clamp",
            "/target",
            ResolvedPathState::Present(lstat(PathNodeKind::RegularFile)),
            &["/clamp", "/target"],
        );
        assert_symlinks(&clamped, &[("/clamp", "/../../target")]);

        let transitive = resolve_script(
            ns,
            "/a",
            &[
                host_root(),
                present(ns, "/a", PathNodeKind::Symlink),
                read_link(ns, "/a", "b"),
                present(ns, "/b", PathNodeKind::Symlink),
                read_link(ns, "/b", "c"),
                present(ns, "/c", PathNodeKind::RegularFile),
            ],
        )
        .await
        .unwrap();
        assert_complete(
            &transitive,
            ns,
            "/a",
            "/c",
            ResolvedPathState::Present(lstat(PathNodeKind::RegularFile)),
            &["/a", "/b", "/c"],
        );
        assert_symlinks(&transitive, &[("/a", "b"), ("/b", "c")]);

        let parent_link = resolve_script(
            ns,
            "/link",
            &[
                host_root(),
                present(ns, "/link", PathNodeKind::Symlink),
                read_link(ns, "/link", "/x/y"),
                present(ns, "/x", PathNodeKind::Symlink),
                read_link(ns, "/x", "/z"),
                present(ns, "/z", PathNodeKind::Directory),
                present(ns, "/z/y", PathNodeKind::RegularFile),
            ],
        )
        .await
        .unwrap();
        assert_complete(
            &parent_link,
            ns,
            "/link",
            "/z/y",
            ResolvedPathState::Present(lstat(PathNodeKind::RegularFile)),
            &["/link", "/x/y", "/z/y"],
        );
        assert_symlinks(&parent_link, &[("/link", "/x/y"), ("/x", "/z")]);
    }

    #[tokio::test]
    async fn path_resolution_detects_exact_cycles_and_expansion_chains() {
        let ns = PathObservationNamespace::Host;

        let self_cycle = resolve_script(
            ns,
            "/self",
            &[
                host_root(),
                present(ns, "/self", PathNodeKind::Symlink),
                read_link(ns, "/self", "self"),
            ],
        )
        .await
        .unwrap_err();
        assert_error_context(&self_cycle, ns, "/self");
        let PathResolutionError::Cycle {
            namespace,
            requested_path,
            chain,
        } = self_cycle
        else {
            panic!("expected self-cycle");
        };
        assert_eq!(namespace, ns);
        assert_eq!(requested_path.as_path(), Path::new("/self"));
        assert_chain(&chain, &[], &["/self"]);

        let two_cycle = resolve_script(
            ns,
            "/a",
            &[
                host_root(),
                present(ns, "/a", PathNodeKind::Symlink),
                read_link(ns, "/a", "b"),
                present(ns, "/b", PathNodeKind::Symlink),
                read_link(ns, "/b", "a"),
            ],
        )
        .await
        .unwrap_err();
        assert_error_context(&two_cycle, ns, "/a");
        let PathResolutionError::Cycle { chain, .. } = two_cycle else {
            panic!("expected two-link cycle");
        };
        assert_chain(&chain, &[], &["/a", "/b"]);

        let prefixed = resolve_script(
            ns,
            "/prefix",
            &[
                host_root(),
                present(ns, "/prefix", PathNodeKind::Symlink),
                read_link(ns, "/prefix", "a"),
                present(ns, "/a", PathNodeKind::Symlink),
                read_link(ns, "/a", "b"),
                present(ns, "/b", PathNodeKind::Symlink),
                read_link(ns, "/b", "a"),
            ],
        )
        .await
        .unwrap_err();
        assert_error_context(&prefixed, ns, "/prefix");
        let PathResolutionError::Cycle { chain, .. } = prefixed else {
            panic!("expected prefixed cycle");
        };
        assert_chain(&chain, &["/prefix"], &["/a", "/b"]);

        for target in ["a/child", "/a/child"] {
            let descendant = resolve_script(
                ns,
                "/prefix",
                &[
                    host_root(),
                    present(ns, "/prefix", PathNodeKind::Symlink),
                    read_link(ns, "/prefix", "a"),
                    present(ns, "/a", PathNodeKind::Symlink),
                    read_link(ns, "/a", target),
                ],
            )
            .await
            .unwrap_err();
            assert_error_context(&descendant, ns, "/prefix");
            let PathResolutionError::InfiniteExpansion { chain, .. } = descendant else {
                panic!("expected descendant expansion");
            };
            assert_chain(&chain, &["/prefix"], &["/a", "/a/child"]);
        }

        let root_cycle = resolve_script(
            ns,
            "/",
            &[
                present(ns, "/", PathNodeKind::Symlink),
                read_link(ns, "/", "/"),
            ],
        )
        .await
        .unwrap_err();
        assert_error_context(&root_cycle, ns, "/");
        let PathResolutionError::Cycle { chain, .. } = root_cycle else {
            panic!("expected root cycle");
        };
        assert_chain(&chain, &[], &["/"]);
    }

    #[tokio::test]
    async fn path_resolution_retains_first_marker_and_parent_provenance() {
        let ns = PathObservationNamespace::Host;
        let ancestor = resolve_script(
            ns,
            "/prefix",
            &[
                host_root(),
                present(ns, "/prefix", PathNodeKind::Symlink),
                read_link(ns, "/prefix", "/a/a"),
                present(ns, "/a", PathNodeKind::Directory),
                present(ns, "/a/a", PathNodeKind::Symlink),
                read_link(ns, "/a/a", "../a"),
            ],
        )
        .await
        .unwrap();
        assert_complete(
            &ancestor,
            ns,
            "/prefix",
            "/a",
            ResolvedPathState::Present(lstat(PathNodeKind::Directory)),
            &["/prefix", "/a/a", "/a"],
        );
        assert_chain(
            ancestor.ancestor_expansion().unwrap(),
            &["/prefix"],
            &["/a/a", "/a"],
        );
        assert_symlinks(&ancestor, &[("/prefix", "/a/a"), ("/a/a", "../a")]);

        let first_marker = resolve_script(
            ns,
            "/f",
            &[
                host_root(),
                present(ns, "/f", PathNodeKind::Symlink),
                read_link(ns, "/f", "/a/b/c"),
                present(ns, "/a", PathNodeKind::Symlink),
                read_link(ns, "/a", "/x"),
                present(ns, "/x", PathNodeKind::Directory),
                present(ns, "/x/b", PathNodeKind::Directory),
                present(ns, "/x/b/c", PathNodeKind::Symlink),
                read_link(ns, "/x/b/c", "/a/b"),
            ],
        )
        .await
        .unwrap();
        assert_complete(
            &first_marker,
            ns,
            "/f",
            "/x/b",
            ResolvedPathState::Present(lstat(PathNodeKind::Directory)),
            &["/f", "/a/b/c", "/x/b/c", "/a/b", "/x/b"],
        );
        assert_chain(
            first_marker.ancestor_expansion().unwrap(),
            &["/f"],
            &["/a/b/c", "/x/b/c", "/a/b"],
        );
        assert_symlinks(
            &first_marker,
            &[
                ("/f", "/a/b/c"),
                ("/a", "/x"),
                ("/x/b/c", "/a/b"),
                ("/a", "/x"),
            ],
        );

        let requested_ancestor = resolve_script(
            ns,
            "/alias/leaf",
            &[
                host_root(),
                present(ns, "/alias", PathNodeKind::Symlink),
                read_link(ns, "/alias", "/real"),
                present(ns, "/real", PathNodeKind::Directory),
                present(ns, "/real/leaf", PathNodeKind::RegularFile),
            ],
        )
        .await
        .unwrap();
        assert_complete(
            &requested_ancestor,
            ns,
            "/alias/leaf",
            "/real/leaf",
            ResolvedPathState::Present(lstat(PathNodeKind::RegularFile)),
            &["/alias/leaf", "/real/leaf"],
        );
        assert_symlinks(&requested_ancestor, &[("/alias", "/real")]);

        let nested = resolve_script(
            ns,
            "/a/a/b",
            &[
                host_root(),
                present(ns, "/a", PathNodeKind::Directory),
                present(ns, "/a/a", PathNodeKind::Symlink),
                read_link(ns, "/a/a", "../a"),
                present(ns, "/a/b", PathNodeKind::RegularFile),
            ],
        )
        .await
        .unwrap();
        assert_complete(
            &nested,
            ns,
            "/a/a/b",
            "/a/b",
            ResolvedPathState::Present(lstat(PathNodeKind::RegularFile)),
            &["/a/a/b", "/a/b"],
        );
        assert_symlinks(&nested, &[("/a/a", "../a")]);
        assert!(nested.ancestor_expansion().is_none());
    }

    #[tokio::test]
    async fn path_resolution_distinguishes_dangling_inconsistent_and_io_errors() {
        let ns = PathObservationNamespace::Materialization(PathObservationInstanceId::new(17));
        let dangling = resolve_script(
            ns,
            "/dangling",
            &[
                present(ns, "/", PathNodeKind::Directory),
                present(ns, "/dangling", PathNodeKind::Symlink),
                read_link(ns, "/dangling", "missing"),
                missing(ns, "/missing"),
            ],
        )
        .await
        .unwrap();
        assert_complete(
            &dangling,
            ns,
            "/dangling",
            "/missing",
            ResolvedPathState::Missing,
            &["/dangling", "/missing"],
        );
        assert_symlinks(&dangling, &[("/dangling", "missing")]);

        let inconsistent = resolve_script(
            ns,
            "/bad",
            &[
                present(ns, "/", PathNodeKind::Directory),
                present(ns, "/bad", PathNodeKind::Symlink),
                read_link_result(ns, "/bad", PathOperationResult::Missing),
            ],
        )
        .await
        .unwrap_err();
        assert_eq!(inconsistent.namespace(), ns);
        assert_eq!(inconsistent.requested_path().as_path(), Path::new("/bad"));
        let PathResolutionError::InconsistentState {
            demand,
            before,
            after,
            ..
        } = inconsistent
        else {
            panic!("missing readlink must be inconsistent");
        };
        assert_eq!(demand.namespace(), ns);
        assert_eq!(demand.path().as_path(), Path::new("/bad"));
        assert_eq!(demand.operation(), PathObservationOperation::ReadLink);
        assert_eq!(before, Some(lstat(PathNodeKind::Symlink)));
        assert_eq!(after, None);

        let io = PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        };
        let lstat_failure = resolve_script(
            ns,
            "/file/child",
            &[
                present(ns, "/", PathNodeKind::Directory),
                lstat_error(ns, "/file", io),
            ],
        )
        .await
        .unwrap_err();
        assert_error_context(&lstat_failure, ns, "/file/child");
        let PathResolutionError::Observation {
            namespace,
            requested_path,
            demand,
            error,
        } = lstat_failure
        else {
            panic!("expected lstat observation error");
        };
        assert_eq!(namespace, ns);
        assert_eq!(requested_path.as_path(), Path::new("/file/child"));
        assert_eq!(demand.namespace(), ns);
        assert_eq!(demand.path().as_path(), Path::new("/file"));
        assert_eq!(demand.operation(), PathObservationOperation::Lstat);
        assert_eq!(error, io);

        let readlink_failure = resolve_script(
            ns,
            "/denied",
            &[
                present(ns, "/", PathNodeKind::Directory),
                present(ns, "/denied", PathNodeKind::Symlink),
                read_link_result(ns, "/denied", PathOperationResult::Error(io)),
            ],
        )
        .await
        .unwrap_err();
        assert_error_context(&readlink_failure, ns, "/denied");
        let PathResolutionError::Observation {
            namespace,
            requested_path,
            demand,
            error,
        } = readlink_failure
        else {
            panic!("expected readlink observation error");
        };
        assert_eq!(namespace, ns);
        assert_eq!(requested_path.as_path(), Path::new("/denied"));
        assert_eq!(demand.namespace(), ns);
        assert_eq!(demand.path().as_path(), Path::new("/denied"));
        assert_eq!(demand.operation(), PathObservationOperation::ReadLink);
        assert_eq!(error, io);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn path_resolution_preserves_non_utf8_raw_link_targets() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::ffi::OsStringExt;

        let ns = PathObservationNamespace::Host;
        let target = OsString::from_vec(vec![b't', 0xff]);
        let raw_target = Arc::new(PathBuf::from(target));
        let raw_absolute = NormalizedAbsolutePath::new(Path::new("/").join(raw_target.as_path()))
            .expect("raw target resolves below the root");
        let raw_demand =
            PathObservationDemand::new(ns, raw_absolute.dupe(), PathObservationOperation::Lstat);
        let resolved = resolve_script(
            ns,
            "/raw",
            &[
                host_root(),
                present(ns, "/raw", PathNodeKind::Symlink),
                read_link_result(ns, "/raw", PathOperationResult::Present(raw_target.dupe())),
                (
                    raw_demand,
                    PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                        PathNodeKind::RegularFile,
                    ))),
                ),
            ],
        )
        .await
        .unwrap();
        assert_eq!(resolved.namespace(), ns);
        assert_eq!(resolved.requested_path().as_path(), Path::new("/raw"));
        assert_eq!(
            resolved.state(),
            ResolvedPathState::Present(lstat(PathNodeKind::RegularFile))
        );
        assert_eq!(resolved.route().len(), 2);
        assert_eq!(
            resolved.route()[0].as_path().as_os_str().as_bytes(),
            b"/raw"
        );
        assert_eq!(
            resolved.route()[1].as_path().as_os_str().as_bytes(),
            raw_absolute.as_path().as_os_str().as_bytes()
        );
        assert_eq!(resolved.symlinks()[0].path().as_path(), Path::new("/raw"));
        assert_eq!(
            raw_target.as_os_str().as_bytes(),
            resolved.symlinks()[0].target().as_os_str().as_bytes()
        );
        assert_eq!(
            raw_absolute.as_path().as_os_str().as_bytes(),
            resolved.real_path().as_path().as_os_str().as_bytes()
        );
    }

    #[test]
    #[should_panic(expected = "path-resolution DICE invariant failed")]
    fn path_resolution_dice_dependency_errors_fail_fast() {
        let _: () = super::dice_invariant::<(), _>(Err("injected dependency failure"));
    }

    #[cfg(windows)]
    #[test]
    fn path_resolution_preserves_windows_roots() {
        assert_eq!(
            super::filesystem_root(Path::new(r"C:\workspace\file")),
            PathBuf::from(r"C:\")
        );
        assert_eq!(
            super::filesystem_root(Path::new(r"\\server\share\workspace")),
            PathBuf::from(r"\\server\share\")
        );
        assert_eq!(
            super::filesystem_root(Path::new(r"\\?\C:\workspace\file")),
            PathBuf::from(r"\\?\C:\")
        );
    }
}
