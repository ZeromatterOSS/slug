//! Workspace-global status descriptors, independent of future runtime contents.
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_build_api_v2::WorkspaceStatusArtifacts;
use slug_workspace_v2::NormalizedAbsolutePath;

/// The workspace alone determines these names and metadata policies. This key
/// does not observe status bytes, environment, time, or a requesting configuration.
#[derive(Debug, Clone, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct WorkspaceStatusArtifactsKey(pub NormalizedAbsolutePath);

impl std::fmt::Display for WorkspaceStatusArtifactsKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "workspace status descriptors {}", self.0)
    }
}

#[async_trait]
impl Key for WorkspaceStatusArtifactsKey {
    type Value = Arc<WorkspaceStatusArtifacts>;

    async fn compute(&self, _: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        Arc::new(WorkspaceStatusArtifacts::new(self.0.dupe()))
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left == right
    }
}

#[cfg(test)]
mod tests;
