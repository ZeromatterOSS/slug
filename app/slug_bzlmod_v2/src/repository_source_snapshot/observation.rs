//! Exact natural-request selection from the current Source publication.
use dice::DiceProjectionComputations;
use dice::ProjectionKey;

use super::*;
use crate::RepositoryWorkspaceContext;

fn validate_context(
    context: &RepositoryWorkspaceContext,
    request: &RepositorySourceSnapshotRequest,
) -> Result<(), RepositoryWorkspaceError> {
    if context.workspace() != request.workspace() || context.lease() != request.lease() {
        return Err(RepositoryWorkspaceError::ForeignReservation);
    }
    Ok(())
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositorySourceSnapshotObservation {
    context: Arc<RepositoryWorkspaceContext>,
    request: Arc<RepositorySourceSnapshotRequest>,
    state: Option<Arc<RepositorySourceSnapshotState>>,
}
impl RepositorySourceSnapshotObservation {
    pub fn from_epoch(
        epoch: &RepositorySourceSnapshotEpoch,
        context: Arc<RepositoryWorkspaceContext>,
        request: Arc<RepositorySourceSnapshotRequest>,
    ) -> Result<Arc<Self>, RepositoryWorkspaceError> {
        validate_context(&context, &request)?;
        // find() distinguishes active absence from revoked/foreign context and
        // a same-repository publication with a different full natural request.
        let state = epoch.find(&request)?.cloned();
        Ok(Arc::new(Self {
            context,
            request,
            state,
        }))
    }
    pub fn context(&self) -> &Arc<RepositoryWorkspaceContext> {
        &self.context
    }
    pub fn request(&self) -> &Arc<RepositorySourceSnapshotRequest> {
        &self.request
    }
    pub fn state(&self) -> Option<&Arc<RepositorySourceSnapshotState>> {
        self.state.as_ref()
    }
}
impl fmt::Debug for RepositorySourceSnapshotObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositorySourceSnapshotObservation")
            .field("context", &self.context)
            .field("repository", self.request.repository())
            .field("present", &self.state.is_some())
            .finish()
    }
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositorySourceSnapshotProjection {
    context: Arc<RepositoryWorkspaceContext>,
    request: Arc<RepositorySourceSnapshotRequest>,
}
impl RepositorySourceSnapshotProjection {
    pub fn new(
        context: Arc<RepositoryWorkspaceContext>,
        request: Arc<RepositorySourceSnapshotRequest>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        validate_context(&context, &request)?;
        Ok(Self { context, request })
    }
}
impl Hash for RepositorySourceSnapshotProjection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.context.hash(state);
        self.request.hash(state);
    }
}
impl fmt::Debug for RepositorySourceSnapshotProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositorySourceSnapshotProjection")
            .field("context", &self.context)
            .field("repository", self.request.repository())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for RepositorySourceSnapshotProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-source-selection:{}",
            self.request.repository()
        )
    }
}
impl ProjectionKey for RepositorySourceSnapshotProjection {
    type DeriveFromKey = RepositorySourceSnapshotEpochKey;
    type Value = Result<Arc<RepositorySourceSnapshotObservation>, RepositoryWorkspaceError>;
    fn compute(
        &self,
        epoch: &Arc<RepositorySourceSnapshotEpoch>,
        _: &DiceProjectionComputations,
    ) -> Self::Value {
        RepositorySourceSnapshotObservation::from_epoch(
            epoch,
            self.context.clone(),
            self.request.clone(),
        )
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;
