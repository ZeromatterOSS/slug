//! Closed full-history seal receipt. A final tree/count alone cannot authorize
//! materialization adoption. This capability is minted by the consuming command
//! after matching its exact immutable epoch and verifying the actual same root.
use slug_bzlmod_v2::RepositoryMaterializationRequest;

use super::*;

pub(in crate::runtime) struct SealedWorkspaceMaterialization {
    request: Arc<RepositoryMaterializationRequest>,
    root: Arc<RepositoryPhysicalRoot>,
    // Full request/history guard follows both native and immutable owners.
    credit: Arc<NativeWorkspaceHistoryGuard>,
}
impl SealedWorkspaceMaterialization {
    pub(in crate::runtime::repository_physical_root) fn from_command(
        request: Arc<RepositoryMaterializationRequest>,
        root: Arc<RepositoryPhysicalRoot>,
        credit: Arc<NativeWorkspaceHistoryGuard>,
    ) -> Self {
        Self {
            request,
            root,
            credit,
        }
    }
    pub(in crate::runtime) fn request(&self) -> &Arc<RepositoryMaterializationRequest> {
        &self.request
    }
    pub(in crate::runtime) fn root(&self) -> &Arc<RepositoryPhysicalRoot> {
        &self.root
    }
    pub(in crate::runtime) fn into_parts(
        self,
    ) -> (
        Arc<RepositoryMaterializationRequest>,
        Arc<RepositoryPhysicalRoot>,
        Arc<NativeWorkspaceHistoryGuard>,
    ) {
        (self.request, self.root, self.credit)
    }
}
