//! Immutable model routing. Synthetic metadata is private to this synchronous
//! resolver and must never be installed in a PathObservationEpoch.
use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::AnchoredResolvedPathKey;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::MaterializationPathAnchor;
use slug_workspace_v2::MaterializationTerminalAnchor;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::ResolvedPath;
use slug_workspace_v2::ResolvedPathState;

use super::*;
use crate::RepositorySourceSnapshotNode;

#[derive(Debug)]
pub enum RepositoryWorkspaceReadExpectation<'a> {
    Owned(&'a Arc<[u8]>),
    Source(FileContentDigest),
}
impl RepositoryWorkspaceReadExpectation<'_> {
    pub fn validate(&self, body: &[u8]) -> Result<(), RepositoryWorkspaceError> {
        if body.len() > 2 * 1024 * 1024 {
            return Err(RepositoryWorkspaceError::InvalidResult);
        }
        let matches = match self {
            Self::Owned(expected) => expected.as_ref() == body,
            Self::Source(expected) => {
                expected.size_bytes() == body.len() as u64
                    && expected.sha256() == &<[u8; 32]>::from(Sha256::digest(body))
            }
        };
        if matches {
            Ok(())
        } else {
            Err(RepositoryWorkspaceError::InvalidResult)
        }
    }
}
impl RepositoryWorkspaceState {
    /// Conservative construction peak, including owned path clones and both
    /// temporary Vec and retained Arc rows. No allocation occurs in this walk.
    pub fn resolution_anchor_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        let mut bytes = add(
            std::mem::size_of::<MaterializationPathAnchor>(),
            self.reservation()
                .display_root()
                .retained_storage_bytes()
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
        )?;
        let table = array_bytes::<MaterializationTerminalAnchor>(self.source_bindings().len())?;
        bytes = add(bytes, add(table, table)?)?;
        for view in self.source_bindings() {
            bytes = add(
                bytes,
                view.display_root()
                    .retained_storage_bytes()
                    .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
            )?;
        }
        Ok(bytes)
    }
    pub fn resolution_anchor(&self) -> Result<MaterializationPathAnchor, RepositoryWorkspaceError> {
        self.resolution_anchor_storage_bytes()?;
        let anchor = MaterializationPathAnchor::new(
            PathObservationNamespace::Materialization(self.reservation().instance()),
            self.reservation().display_root().clone(),
        )
        .map_err(|_| RepositoryWorkspaceError::InvalidPath)?;
        let sources = self
            .source_bindings()
            .iter()
            .map(|view| {
                MaterializationTerminalAnchor::new(
                    PathObservationNamespace::Materialization(view.instance()),
                    view.display_root().clone(),
                )
                .map_err(|_| RepositoryWorkspaceError::InvalidPath)
            })
            .collect::<Result<Vec<_>, _>>()?;
        anchor
            .with_source_anchors(sources)
            .map_err(|_| RepositoryWorkspaceError::InvalidPath)
    }
    /// Computed model only: native observers must independently resolve actual
    /// FDs and may compare the terminal route, never publish these fake stats.
    pub fn resolve_path(&self, relative: &str) -> Result<ResolvedPath, RepositoryWorkspaceError> {
        match self.resolve_path_with(relative, |_| Ok::<_, std::convert::Infallible>(())) {
            Ok(result) => result,
            Err(never) => match never {},
        }
    }
    /// Calls the native caller's guard before every synthetic model fact. The
    /// caller must prepay resolver/anchor scratch before entering this method.
    /// Guard errors stop immediately and remain distinct from model refusal.
    pub fn resolve_path_with<E>(
        &self,
        relative: &str,
        before_demand: impl FnMut(&slug_workspace_v2::PathObservationDemand) -> Result<(), E>,
    ) -> Result<Result<ResolvedPath, RepositoryWorkspaceError>, E> {
        self.resolve_path_with_work(relative, before_demand, |_| Ok(()))
    }
    /// Native adapter checks retained high-water storage and cancellation at
    /// every resolver allocation phase, independently of demand-count bounds.
    pub fn resolve_path_with_work<E>(
        &self,
        relative: &str,
        mut before_demand: impl FnMut(&slug_workspace_v2::PathObservationDemand) -> Result<(), E>,
        mut before_work: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Result<ResolvedPath, RepositoryWorkspaceError>, E> {
        if !valid_relative(relative, true) {
            return Ok(Err(RepositoryWorkspaceError::InvalidPath));
        }
        let anchor = match self.resolution_anchor() {
            Ok(anchor) => anchor,
            Err(error) => return Ok(Err(error)),
        };
        let key = match AnchoredResolvedPathKey::new(
            anchor,
            self.reservation().display_root().as_path().join(relative),
        ) {
            Ok(key) => key,
            Err(_) => return Ok(Err(RepositoryWorkspaceError::InvalidPath)),
        };
        match key.resolve_sync_with_work(
            |demand| {
                before_demand(demand).map_err(ModelFailure::Guard)?;
                self.observe_model(demand).map_err(ModelFailure::Model)
            },
            |bytes| before_work(bytes).map_err(ModelFailure::Guard),
        ) {
            Ok(result) => Ok(result.map_err(|error| match error {
                slug_workspace_v2::AnchoredPathResolutionError::WorkingStorageRefused => {
                    RepositoryWorkspaceError::RetentionLimit
                }
                _ => RepositoryWorkspaceError::InvalidPath,
            })),
            Err(ModelFailure::Guard(error)) => Err(error),
            Err(ModelFailure::Model(error)) => Ok(Err(error)),
        }
    }
    fn observe_model(
        &self,
        demand: &slug_workspace_v2::PathObservationDemand,
    ) -> Result<PathObservationResult, RepositoryWorkspaceError> {
        let ns = demand.namespace();
        let path = demand.path().as_path();
        let (kind, size, mode, link) =
            if ns == PathObservationNamespace::Materialization(self.reservation().instance()) {
                let relative = path
                    .strip_prefix(self.reservation().display_root().as_path())
                    .ok()
                    .and_then(|p| p.to_str())
                    .ok_or(RepositoryWorkspaceError::InvalidPath)?;
                let Some(entry) = self.entry(relative) else {
                    return missing(demand.operation());
                };
                match entry.node() {
                    RepositoryWorkspaceNode::Directory => {
                        (PathNodeKind::Directory, 0, entry.mode(), None)
                    }
                    RepositoryWorkspaceNode::File(body) => (
                        PathNodeKind::RegularFile,
                        body.len() as i64,
                        entry.mode(),
                        None,
                    ),
                    RepositoryWorkspaceNode::Link(link) => (
                        PathNodeKind::Symlink,
                        link.len() as i64,
                        entry.mode(),
                        Some(link),
                    ),
                }
            } else {
                let view = self
                    .source_bindings()
                    .iter()
                    .find(|v| ns == PathObservationNamespace::Materialization(v.instance()))
                    .ok_or(RepositoryWorkspaceError::ForeignReservation)?;
                let relative = path
                    .strip_prefix(view.display_root().as_path())
                    .ok()
                    .and_then(|p| p.to_str())
                    .ok_or(RepositoryWorkspaceError::InvalidPath)?;
                let Some(entry) = view.source_state().unwrap().entry(relative) else {
                    return missing(demand.operation());
                };
                match entry.node() {
                    RepositorySourceSnapshotNode::Directory => {
                        (PathNodeKind::Directory, 0, entry.mode(), None)
                    }
                    RepositorySourceSnapshotNode::File(digest) => (
                        PathNodeKind::RegularFile,
                        digest.size_bytes() as i64,
                        entry.mode(),
                        None,
                    ),
                    RepositorySourceSnapshotNode::Link(link) => (
                        PathNodeKind::Symlink,
                        link.len() as i64,
                        entry.mode(),
                        Some(link),
                    ),
                }
            };
        Ok(match demand.operation() {
            PathObservationOperation::Lstat => PathObservationResult::Lstat(
                PathOperationResult::Present(PathLstat::new(kind, size, 0, 0, 0, mode as i32)),
            ),
            PathObservationOperation::ReadLink => PathObservationResult::ReadLink(
                PathOperationResult::Present(Arc::new(std::path::PathBuf::from(
                    link.ok_or(RepositoryWorkspaceError::InvalidResult)?
                        .as_str(),
                ))),
            ),
            _ => return Err(RepositoryWorkspaceError::InvalidOperation),
        })
    }

    pub fn exists(&self, relative: &str) -> Result<bool, RepositoryWorkspaceError> {
        Ok(matches!(
            self.resolve_path(relative)?.state(),
            ResolvedPathState::Present(_)
        ))
    }
    pub fn read_expectation(
        &self,
        relative: &str,
    ) -> Result<RepositoryWorkspaceReadExpectation<'_>, RepositoryWorkspaceError> {
        let resolved = self.resolve_path(relative)?;
        self.read_expectation_for_resolved(&resolved)
    }
    /// Reuses a caller-bounded modeled resolution. This checks immutable logical
    /// context and terminal membership; it grants no native read authority.
    pub fn read_expectation_for_resolved(
        &self,
        resolved: &ResolvedPath,
    ) -> Result<RepositoryWorkspaceReadExpectation<'_>, RepositoryWorkspaceError> {
        let namespace = PathObservationNamespace::Materialization(self.reservation().instance());
        let Some(anchor) = resolved.anchor() else {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        };
        if resolved.namespace() != namespace
            || anchor.namespace() != namespace
            || anchor.root() != self.reservation().display_root()
            || !resolved
                .requested_path()
                .as_path()
                .starts_with(anchor.root().as_path())
            || anchor.source_anchors().len() != self.source_bindings().len()
            || anchor
                .source_anchors()
                .iter()
                .zip(self.source_bindings())
                .any(|(anchor, view)| {
                    anchor.namespace() != PathObservationNamespace::Materialization(view.instance())
                        || anchor.root() != view.display_root()
                })
        {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        let ResolvedPathState::Present(stat) = resolved.state() else {
            return Err(RepositoryWorkspaceError::InvalidResult);
        };
        if stat.kind() != PathNodeKind::RegularFile
            || stat.size() < 0
            || stat.size() > 2 * 1024 * 1024
        {
            return Err(RepositoryWorkspaceError::InvalidResult);
        }
        let ns = resolved.content_origin().namespace();
        if ns == PathObservationNamespace::Materialization(self.reservation().instance()) {
            let rel = resolved
                .real_path()
                .as_path()
                .strip_prefix(self.reservation().display_root().as_path())
                .ok()
                .and_then(|p| p.to_str())
                .ok_or(RepositoryWorkspaceError::InvalidPath)?;
            let Some(RepositoryWorkspaceNode::File(body)) = self.entry(rel).map(|e| e.node())
            else {
                return Err(RepositoryWorkspaceError::InvalidResult);
            };
            if body.len() > 2 * 1024 * 1024 {
                return Err(RepositoryWorkspaceError::InvalidResult);
            }
            Ok(RepositoryWorkspaceReadExpectation::Owned(body))
        } else {
            let view = self
                .source_bindings()
                .iter()
                .find(|v| ns == PathObservationNamespace::Materialization(v.instance()))
                .ok_or(RepositoryWorkspaceError::ForeignReservation)?;
            let rel = resolved
                .real_path()
                .as_path()
                .strip_prefix(view.display_root().as_path())
                .ok()
                .and_then(|p| p.to_str())
                .ok_or(RepositoryWorkspaceError::InvalidPath)?;
            let Some(RepositorySourceSnapshotNode::File(digest)) =
                view.source_state().unwrap().entry(rel).map(|e| e.node())
            else {
                return Err(RepositoryWorkspaceError::InvalidResult);
            };
            if digest.size_bytes() > 2 * 1024 * 1024 {
                return Err(RepositoryWorkspaceError::InvalidResult);
            }
            Ok(RepositoryWorkspaceReadExpectation::Source(*digest))
        }
    }
}
fn missing(
    operation: PathObservationOperation,
) -> Result<PathObservationResult, RepositoryWorkspaceError> {
    match operation {
        PathObservationOperation::Lstat => {
            Ok(PathObservationResult::Lstat(PathOperationResult::Missing))
        }
        PathObservationOperation::ReadLink => Ok(PathObservationResult::ReadLink(
            PathOperationResult::Missing,
        )),
        _ => Err(RepositoryWorkspaceError::InvalidOperation),
    }
}

enum ModelFailure<E> {
    Guard(E),
    Model(RepositoryWorkspaceError),
}
