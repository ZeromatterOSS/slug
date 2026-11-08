//! Request-bound current selection; missing is never a conflicting association.
use dice::DiceProjectionComputations;
use dice::ProjectionKey;

use super::*;
use crate::RepositoryWorkspaceContext;
use crate::RepositoryWorkspaceError;

fn validate_request(
    context: &RepositoryWorkspaceContext,
    request: &RepositoryMaterializationRequest,
) -> Result<(), RepositoryWorkspaceError> {
    if context.workspace() != &request.id.workspace {
        return Err(RepositoryWorkspaceError::ForeignReservation);
    }
    if let RepositoryMaterializationKind::GeneralWorkspace(realization) = &request.kind {
        request.validate_general_workspace_binding()?;
        if realization.reservation().lease() != context.lease()
            || realization.reservation().limits() != context.limits()
        {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
    }
    Ok(())
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryMaterializationObservation {
    context: Arc<RepositoryWorkspaceContext>,
    request: Arc<RepositoryMaterializationRequest>,
    entry: Option<RepositoryMaterializationEpochEntry>,
}
impl RepositoryMaterializationObservation {
    pub fn from_epoch(
        epoch: &RepositoryMaterializationResultEpoch,
        context: Arc<RepositoryWorkspaceContext>,
        request: Arc<RepositoryMaterializationRequest>,
    ) -> Result<Arc<Self>, RepositoryWorkspaceError> {
        validate_request(&context, &request)?;
        if epoch.workspace() != context.workspace() {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        let entry = epoch.entry_for_repository(&request.id.canonical_repo);
        if let Some(entry) = entry {
            if entry.request.id.workspace != request.id.workspace
                || entry.request.id.canonical_repo != request.id.canonical_repo
                || entry.request.as_ref() != request.as_ref()
            {
                return Err(RepositoryWorkspaceError::ReplayConflict);
            }
            validate_request(&context, &entry.request)?;
        }
        Ok(Arc::new(Self {
            context,
            request,
            entry: entry.cloned(),
        }))
    }
    pub fn context(&self) -> &Arc<RepositoryWorkspaceContext> {
        &self.context
    }
    pub fn request(&self) -> &Arc<RepositoryMaterializationRequest> {
        &self.request
    }
    pub fn entry(&self) -> Option<&RepositoryMaterializationEpochEntry> {
        self.entry.as_ref()
    }
}
impl fmt::Debug for RepositoryMaterializationObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryMaterializationObservation")
            .field("context", &self.context)
            .field("repository", &self.request.id.canonical_repo)
            .field("present", &self.entry.is_some())
            .finish()
    }
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryMaterializationProjection {
    context: Arc<RepositoryWorkspaceContext>,
    request: Arc<RepositoryMaterializationRequest>,
}
impl RepositoryMaterializationProjection {
    pub fn new(
        context: Arc<RepositoryWorkspaceContext>,
        request: Arc<RepositoryMaterializationRequest>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        validate_request(&context, &request)?;
        Ok(Self { context, request })
    }
}
impl Hash for RepositoryMaterializationProjection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.context.hash(state);
        self.request.id.hash(state);
    }
}
impl fmt::Debug for RepositoryMaterializationProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryMaterializationProjection")
            .field("context", &self.context)
            .field("repository", &self.request.id.canonical_repo)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for RepositoryMaterializationProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-materialization-selection:{}",
            self.request.id.canonical_repo
        )
    }
}
impl ProjectionKey for RepositoryMaterializationProjection {
    type DeriveFromKey = RepositoryMaterializationResultEpochKey;
    type Value = Result<Arc<RepositoryMaterializationObservation>, RepositoryWorkspaceError>;
    fn compute(
        &self,
        epoch: &RepositoryMaterializationResultEpoch,
        _: &DiceProjectionComputations,
    ) -> Self::Value {
        RepositoryMaterializationObservation::from_epoch(
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
#[path = "materialization_projection_tests.rs"]
mod tests;
