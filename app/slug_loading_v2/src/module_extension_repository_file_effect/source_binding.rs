//! Exact tracked rows for contextual access. Opaque epochs never escape here.
use slug_bzlmod_v2::RepositoryMaterializationObservation;
use slug_bzlmod_v2::RepositoryMaterializationProjection;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::RepositorySourceSnapshotEpochKey;
use slug_bzlmod_v2::RepositorySourceSnapshotObservation;
use slug_bzlmod_v2::RepositorySourceSnapshotProjection;
use slug_bzlmod_v2::RepositorySourceSnapshotRequest;
use slug_bzlmod_v2::RepositoryWorkspaceContext;

use super::*;

pub(super) async fn source(
    ctx: &mut DiceComputations<'_>,
    context: Arc<RepositoryWorkspaceContext>,
    request: Arc<RepositorySourceSnapshotRequest>,
) -> Result<Arc<RepositorySourceSnapshotObservation>, String> {
    let epoch = ctx
        .compute_opaque(&RepositorySourceSnapshotEpochKey::new(
            context.workspace().clone(),
        ))
        .await
        .map_err(|e| e.to_string())?;
    let projection =
        RepositorySourceSnapshotProjection::new(context, request).map_err(|e| e.to_string())?;
    ctx.projection(&epoch, &projection)
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
pub(super) async fn materialized(
    ctx: &mut DiceComputations<'_>,
    context: Arc<RepositoryWorkspaceContext>,
    request: Arc<RepositoryMaterializationRequest>,
) -> Result<Arc<RepositoryMaterializationObservation>, String> {
    let epoch = ctx
        .compute_opaque(&RepositoryMaterializationResultEpochKey {
            workspace: context.workspace().clone(),
        })
        .await
        .map_err(|e| e.to_string())?;
    let projection =
        RepositoryMaterializationProjection::new(context, request).map_err(|e| e.to_string())?;
    ctx.projection(&epoch, &projection)
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
