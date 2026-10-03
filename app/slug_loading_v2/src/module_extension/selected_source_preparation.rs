//! Normal root-import selection for private source preparation; no producer evaluation.
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
use slug_bzlmod_v2::HostSelectedExtensionDemandErrorDisposition;
use slug_bzlmod_v2::HostSelectedExtensionDemandObservationKey;
use slug_bzlmod_v2::HostSelectedExtensionOwnerKind;
use slug_bzlmod_v2::ObservedHostRootRepositoryMapping;
use slug_bzlmod_v2::ObservedHostSelectedExtensionDemand;
use slug_bzlmod_v2::SelectedScratchResources;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageDescriptorBudget;

use super::selected_scratch::SelectedScratchPreparationDeadline;
use super::*;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct SelectedPreparationOwnerKey {
    workspace: NormalizedAbsolutePath,
    apparent: BudgetedImageBlock<ApparentRepoName>,
}
impl SelectedPreparationOwnerKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        apparent: &ApparentRepoName,
        budget: &ImageDescriptorBudget,
    ) -> Result<Self, SelectedPreparationOwnerError> {
        if apparent.as_str().is_empty() || apparent.as_str().len() > 4096 {
            return Err(SelectedPreparationOwnerError::InvalidSelector);
        }
        let credit = budget
            .try_reserve(
                apparent
                    .as_str()
                    .len()
                    .checked_add(128)
                    .ok_or(SelectedPreparationOwnerError::Operational)?,
            )
            .map_err(|_| SelectedPreparationOwnerError::Operational)?;
        Ok(Self {
            workspace,
            apparent: credit
                .freeze(apparent.clone())
                .map_err(|_| SelectedPreparationOwnerError::Operational)?,
        })
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
    pub fn apparent(&self) -> &ApparentRepoName {
        self.apparent.get()
    }
    pub(crate) fn same_resources(&self, r: &SelectedScratchResources) -> bool {
        self.apparent.same_budget(r.descriptor_budget())
    }
}
impl fmt::Display for SelectedPreparationOwnerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "selected-preparation-owner:{}:{}",
            self.workspace,
            self.apparent().as_str()
        )
    }
}
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum SelectedPreparationOwnerError {
    InvalidSelector,
    MissingImport,
    NotGenerated,
    WrongOwnerFamily,
    SourceSelection,
    Operational,
}
impl SelectedPreparationOwnerError {
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::SourceSelection | Self::Operational)
    }
    pub(crate) fn operational() -> Self {
        Self::Operational
    }
}
impl fmt::Display for SelectedPreparationOwnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selected source owner refused: {self:?}")
    }
}
impl std::error::Error for SelectedPreparationOwnerError {}

#[derive(Debug, PartialEq, Eq, Allocative)]
struct OwnerInner {
    key: SelectedPreparationOwnerKey,
    requested: BudgetedImageBlock<CanonicalRepoName>,
    mapping: ObservedHostRootRepositoryMapping,
    demand: ObservedHostSelectedExtensionDemand,
    observations: PathObservationEpoch,
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedSelectedPreparationOwner(BudgetedImageBlock<OwnerInner>);
impl ObservedSelectedPreparationOwner {
    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        self.0
            .get()
            .demand
            .result()
            .as_ref()
            .as_ref()
            .expect("normal owner success")
            .owner()
    }
    pub fn requested(&self) -> &CanonicalRepoName {
        self.0.get().requested.get()
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.0.get().observations
    }
    pub fn matches_key(&self, key: &SelectedPreparationOwnerKey) -> bool {
        self.0.get().key == *key
    }
    pub fn same_resources(&self, r: &SelectedScratchResources) -> bool {
        self.0.same_budget(r.descriptor_budget())
            && self.0.get().key.same_resources(r)
            && self.0.get().requested.same_budget(r.descriptor_budget())
    }
}
pub(crate) fn deadline(ctx: &DiceComputations<'_>) -> Instant {
    ctx.per_transaction_data()
        .data
        .get::<SelectedScratchPreparationDeadline>()
        .map_or_else(
            |_| Instant::now() + Duration::from_secs(60),
            |d| d.deadline(),
        )
}
type Value = SourcePreparationOutcome<
    Result<ObservedSelectedPreparationOwner, SelectedPreparationOwnerError>,
>;
async fn compute(
    key: &SelectedPreparationOwnerKey,
    ctx: &mut DiceComputations<'_>,
    cancel: &CancellationContext,
) -> Result<Value, SelectedPreparationOwnerError> {
    let deadline = deadline(ctx);
    let active = || !cancel.is_cancelled() && Instant::now() < deadline;
    if !active() {
        return Err(SelectedPreparationOwnerError::Operational);
    }
    let resources = ctx
        .per_transaction_data()
        .data
        .get::<SelectedScratchResources>()
        .map_err(|_| SelectedPreparationOwnerError::Operational)?
        .dupe();
    if !key.same_resources(&resources) {
        return Err(SelectedPreparationOwnerError::Operational);
    }
    let mut credit = resources
        .descriptor_budget()
        .try_reserve(std::mem::size_of::<OwnerInner>() + 256)
        .map_err(|_| SelectedPreparationOwnerError::Operational)?;
    let mapping = match crate::dependency_cycle::compute_guarded(
        ctx,
        &HostRootRepositoryMappingObservationKey::new(key.workspace.dupe()),
    )
    .await
    .map_err(|_| SelectedPreparationOwnerError::Operational)?
    {
        SourcePreparationOutcome::Need(need) => return Ok(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(value) => {
            value.map_err(|_| SelectedPreparationOwnerError::SourceSelection)?
        }
    };
    let view = mapping
        .result()
        .as_ref()
        .as_ref()
        .map_err(|_| SelectedPreparationOwnerError::SourceSelection)?
        .view()
        .ok_or(SelectedPreparationOwnerError::SourceSelection)?;
    let requested = view
        .mapping()
        .find(|(name, _)| *name == key.apparent())
        .map(|(_, repo)| repo)
        .ok_or(SelectedPreparationOwnerError::MissingImport)?;
    let name_bytes = requested
        .as_str()
        .len()
        .checked_add(128)
        .ok_or(SelectedPreparationOwnerError::Operational)?;
    // New wrapper name credit survives its full lifetime. Temporary child-local
    // clone credit does not claim the baseline engine-interned child key copy.
    let name = resources
        .descriptor_budget()
        .try_reserve(name_bytes)
        .map_err(|_| SelectedPreparationOwnerError::Operational)?
        .freeze(requested.clone())
        .map_err(|_| SelectedPreparationOwnerError::Operational)?;
    let temporary = resources
        .descriptor_budget()
        .try_reserve(name_bytes)
        .map_err(|_| SelectedPreparationOwnerError::Operational)?;
    let child =
        HostSelectedExtensionDemandObservationKey::new(key.workspace.dupe(), name.get().clone());
    let outcome = crate::dependency_cycle::compute_guarded(ctx, &child)
        .await
        .map_err(|_| SelectedPreparationOwnerError::Operational)?;
    drop(child);
    drop(temporary);
    let demand = match outcome {
        SourcePreparationOutcome::Need(need) => return Ok(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(value) => {
            value.map_err(|_| SelectedPreparationOwnerError::SourceSelection)?
        }
    };
    let selected = demand.result().as_ref().as_ref().map_err(|e| {
        if e.disposition() == HostSelectedExtensionDemandErrorDisposition::Missing {
            SelectedPreparationOwnerError::NotGenerated
        } else {
            SelectedPreparationOwnerError::SourceSelection
        }
    })?;
    if selected.owner().kind() != HostSelectedExtensionOwnerKind::ModuleExtension {
        return Err(SelectedPreparationOwnerError::WrongOwnerFamily);
    }
    let observations = super::selected_scratch::merge_checkpoint_frontier(
        &mut credit,
        mapping.observations(),
        demand.observations(),
    )
    .map_err(|_| SelectedPreparationOwnerError::Operational)?;
    if !active() {
        return Err(SelectedPreparationOwnerError::Operational);
    }
    let value = ObservedSelectedPreparationOwner(
        credit
            .freeze(OwnerInner {
                key: key.dupe(),
                requested: name,
                mapping,
                demand,
                observations,
            })
            .map_err(|_| SelectedPreparationOwnerError::Operational)?,
    );
    Ok(SourcePreparationOutcome::Complete(Ok(value)))
}
#[async_trait]
impl Key for SelectedPreparationOwnerKey {
    type Value = Value;
    async fn compute(&self, ctx: &mut DiceComputations, cancel: &CancellationContext) -> Value {
        match compute(self, ctx, cancel).await {
            Ok(value) => value,
            Err(error) => SourcePreparationOutcome::Complete(Err(error)),
        }
    }
    fn validity(value: &Value) -> bool {
        match value {
            SourcePreparationOutcome::Need(_) => false,
            SourcePreparationOutcome::Complete(v) => {
                v.as_ref().err().is_none_or(|e| !e.is_transient())
            }
        }
    }
    fn equality(a: &Value, b: &Value) -> bool {
        Self::validity(a) && Self::validity(b) && a.complete_eq(b)
    }
}
