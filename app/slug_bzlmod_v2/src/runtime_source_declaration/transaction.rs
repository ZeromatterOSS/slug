use std::fmt;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorBudgetError;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::RuntimeSourceDeclarationError;
use super::RuntimeSourceDeclarationPolicy;

/// Raw command configuration. Only the existing normalized workspace handle
/// enters the engine's key catalogue; policy arrays are retained VALUES.
/// The root-path/key allocation remains its existing baseline DICE owner, so
/// ordinary None injection consumes no new image descriptor credit.
#[derive(Debug, Clone, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct RuntimeSourceDeclarationPolicyKey {
    workspace: NormalizedAbsolutePath,
}
impl RuntimeSourceDeclarationPolicyKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
}
impl fmt::Display for RuntimeSourceDeclarationPolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("runtime-source-declaration-policy")
    }
}
#[async_trait]
impl Key for RuntimeSourceDeclarationPolicyKey {
    type Value = Option<RuntimeSourceDeclarationPolicy>;
    async fn compute(&self, _: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        None
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
    // None is valid raw absence data: the SAME updater can restore/revoke it.
    // Normal consumers own MissingDeclaration and current-authority validity.
}

#[derive(Allocative)]
struct TransactionInner {
    workspace: ImageChargedPath,
    current: Option<RuntimeSourceDeclarationPolicy>,
    budget: ImageDescriptorBudget,
}
impl PartialEq for TransactionInner {
    fn eq(&self, other: &Self) -> bool {
        self.workspace == other.workspace && self.current == other.current
    }
}
impl Eq for TransactionInner {}
/// Current immutable command input and actual workspace descriptor service.
/// This configuration witness alone grants no native source access.
#[derive(Clone, Dupe, Eq, PartialEq, Allocative)]
pub struct RuntimeSourceDeclarationTransaction(BudgetedImageBlock<TransactionInner>);
impl RuntimeSourceDeclarationTransaction {
    pub fn new(
        workspace: ImageChargedPath,
        current: Option<RuntimeSourceDeclarationPolicy>,
        budget: ImageDescriptorBudget,
    ) -> Result<Self, RuntimeSourceDeclarationError> {
        if !workspace.same_budget(&budget)
            || current.as_ref().is_some_and(|p| !p.same_budget(&budget))
        {
            return Err(ImageDescriptorBudgetError::ForeignBudget.into());
        }
        if current
            .as_ref()
            .is_some_and(|p| p.workspace() != &workspace)
        {
            return Err(RuntimeSourceDeclarationError::UnauthorizedCurrentPolicy);
        }
        Ok(Self(budget.try_reserve(0)?.freeze(TransactionInner {
            workspace,
            current,
            budget: budget.dupe(),
        })?))
    }
    pub fn workspace(&self) -> &ImageChargedPath {
        &self.0.get().workspace
    }
    pub fn current(&self) -> Option<&RuntimeSourceDeclarationPolicy> {
        self.0.get().current.as_ref()
    }
    pub fn authorizes(&self, policy: &RuntimeSourceDeclarationPolicy) -> bool {
        self.current() == Some(policy)
            && self.workspace() == policy.workspace()
            && self.same_budget(&self.0.get().budget)
            && policy.same_budget(&self.0.get().budget)
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.0.same_budget(budget)
            && self.workspace().same_budget(budget)
            && self.current().is_none_or(|p| p.same_budget(budget))
    }
}
impl fmt::Debug for RuntimeSourceDeclarationTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeSourceDeclarationTransaction")
            .field("present", &self.current().is_some())
            .finish_non_exhaustive()
    }
}
