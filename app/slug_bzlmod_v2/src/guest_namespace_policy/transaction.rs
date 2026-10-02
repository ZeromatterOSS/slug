use std::fmt;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::GuestNamespacePolicy;
use super::GuestNamespacePolicyError;

/// Compact raw input. Workspace Arc/key containers keep existing baseline owners;
/// inline Some/None policy introduces no retained paths, profiles or blobs.
#[derive(Debug, Clone, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct RawGuestNamespacePolicyKey {
    workspace: NormalizedAbsolutePath,
}
impl RawGuestNamespacePolicyKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
}
impl fmt::Display for RawGuestNamespacePolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("raw-guest-namespace-policy")
    }
}
#[async_trait]
impl Key for RawGuestNamespacePolicyKey {
    type Value = Option<GuestNamespacePolicy>;
    async fn compute(&self, _: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        None
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
    // None is valid absence data, including SAME-updater restoration.
}

#[derive(Allocative)]
struct TransactionInner {
    workspace: NormalizedAbsolutePath,
    current: Option<GuestNamespacePolicy>,
    budget: ImageDescriptorBudget,
}
impl PartialEq for TransactionInner {
    fn eq(&self, other: &Self) -> bool {
        self.workspace == other.workspace && self.current == other.current
    }
}
impl Eq for TransactionInner {}

/// Current command witness; only normal consumers/native factories grant access.
#[derive(Clone, Dupe, Eq, PartialEq, Allocative)]
pub struct GuestNamespaceTransaction(BudgetedImageBlock<TransactionInner>);
impl GuestNamespaceTransaction {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        current: Option<GuestNamespacePolicy>,
        budget: ImageDescriptorBudget,
    ) -> Result<Self, GuestNamespacePolicyError> {
        // freeze precharges the new block/header before allocating its Arc.
        Ok(Self(budget.try_reserve(0)?.freeze(TransactionInner {
            workspace,
            current,
            budget: budget.dupe(),
        })?))
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.0.get().workspace
    }
    pub fn current(&self) -> Option<GuestNamespacePolicy> {
        self.0.get().current
    }
    /// Consumers additionally compare workspace() to their key and same_budget()
    /// to current services. Copy policy has no embedded workspace/service identity.
    pub fn authorizes(&self, policy: &GuestNamespacePolicy) -> bool {
        self.current() == Some(*policy) && self.same_budget(&self.0.get().budget)
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.0.same_budget(budget)
    }
}
impl fmt::Debug for GuestNamespaceTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuestNamespaceTransaction")
            .field("present", &self.current().is_some())
            .finish_non_exhaustive()
    }
}
