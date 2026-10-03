//! Normal configured source-tree profiles; execution and full namespace facts are separate.
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::ObservedSelectedLabelSource;
use slug_bzlmod_v2::RuntimeSourceDeclarationError;
use slug_bzlmod_v2::RuntimeSourceDeclarationPolicy;
use slug_bzlmod_v2::RuntimeSourceDeclarationTransaction;
use slug_bzlmod_v2::RuntimeSourceGuestPrefix;
use slug_bzlmod_v2::SelectedLabelSourceError;
use slug_bzlmod_v2::SelectedScratchElfMetadataTransaction;
use slug_bzlmod_v2::SelectedScratchResources;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageDescriptorReservation;
use slug_workspace_v2::InputImageError;
use slug_workspace_v2::ObservedInputImage;
use slug_workspace_v2::ObservedInputImageKey;

use super::selected_scratch::SelectedScratchPrefixError;
use super::selected_scratch::SelectedScratchPreparationDeadline;
use super::selected_tools::ObservedSelectedExtensionToolSourceRequests;
use super::selected_tools::ObservedSelectedExtensionToolSources;
use super::selected_tools::SelectedExtensionToolSourceRequestsKey;
use super::selected_tools::SelectedExtensionToolSourcesKey;
use super::selected_tools::SelectedToolSourcesError;
use super::*;
mod covered_tool;
mod profile;
mod requests;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RuntimeSourceRequestsKey(SelectedExtensionToolSourceRequestsKey);
impl RuntimeSourceRequestsKey {
    pub fn new(tools: SelectedExtensionToolSourceRequestsKey) -> Self {
        Self(tools)
    }
    pub fn tool_requests_key(&self) -> &SelectedExtensionToolSourceRequestsKey {
        &self.0
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        self.0.checkpoint().workspace()
    }
}
impl fmt::Display for RuntimeSourceRequestsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "runtime-source-requests:{}", self.0)
    }
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RuntimeSourceProfileKey(RuntimeSourceRequestsKey);
impl RuntimeSourceProfileKey {
    pub fn new(tools: SelectedExtensionToolSourceRequestsKey) -> Self {
        Self(RuntimeSourceRequestsKey::new(tools))
    }
    pub fn requests_key(&self) -> &RuntimeSourceRequestsKey {
        &self.0
    }
    pub fn tool_sources_key(&self) -> SelectedExtensionToolSourcesKey {
        SelectedExtensionToolSourcesKey::new(self.0.0.checkpoint().clone(), self.0.0.policy())
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        self.0.workspace()
    }
}
impl fmt::Display for RuntimeSourceProfileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "runtime-source-profile:{}", self.0)
    }
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RuntimeSourcesError {
    Declaration(RuntimeSourceDeclarationError),
    Tool(SelectedToolSourcesError),
    Label(SelectedLabelSourceError),
    Image(InputImageError),
    Prefix(SelectedScratchPrefixError),
    MissingDeclaration,
    UnsupportedPlatform,
    InvalidScope,
    WrongLabelRole,
    AggregateLimit,
    Operational,
}
impl RuntimeSourcesError {
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Declaration(e) => e.is_transient(),
            Self::Tool(e) => e.is_transient(),
            Self::Label(e) => e.is_transient(),
            Self::Image(e) => e.is_transient(),
            Self::Prefix(e) => e.is_transient(),
            Self::Operational => true,
            _ => false,
        }
    }
    pub(crate) fn operational() -> Self {
        Self::Operational
    }
}
impl fmt::Debug for RuntimeSourcesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Declaration(_) => "declaration",
            Self::Tool(_) => "tool",
            Self::Label(_) => "label",
            Self::Image(_) => "image",
            Self::Prefix(_) => "prefix",
            Self::MissingDeclaration => "missing declaration",
            Self::UnsupportedPlatform => "platform",
            Self::InvalidScope => "scope",
            Self::WrongLabelRole => "label role",
            Self::AggregateLimit => "aggregate limit",
            Self::Operational => "operational",
        };
        f.debug_struct("RuntimeSourcesError")
            .field("kind", &name)
            .field("transient", &self.is_transient())
            .finish()
    }
}
impl fmt::Display for RuntimeSourcesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("configured runtime source preparation refused")
    }
}
impl std::error::Error for RuntimeSourcesError {}
macro_rules! error_from {
    ($type:ty, $variant:ident) => {
        impl From<$type> for RuntimeSourcesError {
            fn from(e: $type) -> Self {
                Self::$variant(e)
            }
        }
    };
}
error_from!(RuntimeSourceDeclarationError, Declaration);
error_from!(SelectedToolSourcesError, Tool);
error_from!(SelectedLabelSourceError, Label);
error_from!(InputImageError, Image);
error_from!(SelectedScratchPrefixError, Prefix);

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RuntimeSourceRequest {
    policy: RuntimeSourceDeclarationPolicy,
    index: usize,
    image: ObservedInputImageKey,
    selected: Option<ObservedSelectedLabelSource>,
    guest: RuntimeSourceGuestPrefix,
}
impl RuntimeSourceRequest {
    pub fn declaration_index(&self) -> usize {
        self.index
    }
    pub fn image_key(&self) -> &ObservedInputImageKey {
        &self.image
    }
    pub fn selected_label_source(&self) -> Option<&ObservedSelectedLabelSource> {
        self.selected.as_ref()
    }
    pub fn guest_prefix(&self) -> &RuntimeSourceGuestPrefix {
        &self.guest
    }
}
#[derive(Debug, PartialEq, Eq, Allocative)]
struct RequestsInner {
    policy: RuntimeSourceDeclarationPolicy,
    tools: ObservedSelectedExtensionToolSourceRequests,
    requests: Vec<RuntimeSourceRequest>,
    observations: PathObservationEpoch,
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedRuntimeSourceRequests(BudgetedImageBlock<RequestsInner>);
impl ObservedRuntimeSourceRequests {
    pub fn policy(&self) -> &RuntimeSourceDeclarationPolicy {
        &self.0.get().policy
    }
    pub fn tool_requests(&self) -> &ObservedSelectedExtensionToolSourceRequests {
        &self.0.get().tools
    }
    pub fn requests(&self) -> impl ExactSizeIterator<Item = &RuntimeSourceRequest> {
        self.0.get().requests.iter()
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.0.get().observations
    }
    pub fn matches_key(&self, key: &RuntimeSourceRequestsKey) -> bool {
        self.policy().workspace().normalized_ref() == key.workspace()
            && self.tool_requests().matches_key(key.tool_requests_key())
    }
    pub fn same_resources(&self, r: &SelectedScratchResources) -> bool {
        let b = r.descriptor_budget();
        self.0.same_budget(b)
            && self.policy().same_budget(b)
            && self.tool_requests().same_resources(r)
            && self.requests().all(|row| {
                row.policy == *self.policy()
                    && row.policy.same_budget(b)
                    && row.guest.same_budget(b)
                    && row.selected.as_ref().is_none_or(|s| s.same_budget(b))
            })
    }
    pub fn authorized_by(&self, tx: &RuntimeSourceDeclarationTransaction) -> bool {
        tx.authorizes(self.policy())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub struct RuntimeSourceTotals {
    pub entries: usize,
    pub file_bytes: u64,
    pub descriptor_bytes: usize,
}
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum RuntimeSourceImageBinding {
    Captured,
    CoveredTool {
        role: usize,
        root_entry: usize,
        entry_count: usize,
    },
}
#[derive(Debug, PartialEq, Eq, Allocative)]
struct ProfileInner {
    requests: ObservedRuntimeSourceRequests,
    tools: ObservedSelectedExtensionToolSources,
    images: Vec<ObservedInputImage>,
    bindings: Vec<RuntimeSourceImageBinding>,
    observations: PathObservationEpoch,
    totals: RuntimeSourceTotals,
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedRuntimeSourceProfile(BudgetedImageBlock<ProfileInner>);
impl ObservedRuntimeSourceProfile {
    pub fn requests(&self) -> &ObservedRuntimeSourceRequests {
        &self.0.get().requests
    }
    pub fn tool_sources(&self) -> &ObservedSelectedExtensionToolSources {
        &self.0.get().tools
    }
    pub fn images(&self) -> impl ExactSizeIterator<Item = &ObservedInputImage> {
        self.0.get().images.iter()
    }
    pub fn image_binding(&self, declaration: usize) -> Option<RuntimeSourceImageBinding> {
        self.0.get().bindings.get(declaration).copied()
    }
    pub fn tool_root_member(&self, declaration: usize) -> Option<(usize, usize)> {
        match self.image_binding(declaration)? {
            RuntimeSourceImageBinding::CoveredTool {
                role, root_entry, ..
            } => Some((role, root_entry)),
            RuntimeSourceImageBinding::Captured => None,
        }
    }
    pub fn tool_member(&self, declaration: usize, entry: usize) -> Option<(usize, usize)> {
        match self.image_binding(declaration)? {
            RuntimeSourceImageBinding::CoveredTool {
                role,
                root_entry,
                entry_count,
            } if entry < entry_count => Some((role, root_entry.checked_add(entry)?)),
            _ => None,
        }
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.0.get().observations
    }
    pub fn totals(&self) -> RuntimeSourceTotals {
        self.0.get().totals
    }
    pub fn matches_key(&self, key: &RuntimeSourceProfileKey) -> bool {
        self.requests().matches_key(key.requests_key())
            && self.tool_sources().matches_key(&key.tool_sources_key())
            && self.tool_sources().requests() == self.requests().tool_requests()
            && self.0.get().bindings.len() == self.0.get().images.len()
            && self.0.get().images.len() == self.requests().requests().len()
    }
    pub fn same_resources(&self, r: &SelectedScratchResources) -> bool {
        self.0.same_budget(r.descriptor_budget())
            && self.requests().same_resources(r)
            && self.tool_sources().same_resources(r)
            && self
                .images()
                .all(|image| image.same_budget(r.descriptor_budget()))
    }
    pub fn authorized_by(
        &self,
        tx: &RuntimeSourceDeclarationTransaction,
        scratch: &SelectedScratchElfMetadataTransaction,
        r: &SelectedScratchResources,
    ) -> bool {
        self.requests().authorized_by(tx) && self.tool_sources().authorized_by(scratch, r)
    }
}

#[cfg(test)]
pub(super) fn certify_runtime_test(
    row: &RuntimeSourceRequest,
    image: &ObservedInputImage,
    tools: &ObservedSelectedExtensionToolSources,
) -> Result<RuntimeSourceImageBinding, RuntimeSourcesError> {
    covered_tool::certify(row, image, tools, &|| true)
}

type RequestsValue =
    SourcePreparationOutcome<Result<ObservedRuntimeSourceRequests, RuntimeSourcesError>>;
type ProfileValue =
    SourcePreparationOutcome<Result<ObservedRuntimeSourceProfile, RuntimeSourcesError>>;
macro_rules! runtime_key {
    ($key:ty,$value:ty,$compute:path) => {
        #[async_trait]
        impl Key for $key {
            type Value = $value;
            async fn compute(
                &self,
                ctx: &mut DiceComputations,
                cancel: &CancellationContext,
            ) -> Self::Value {
                match $compute(self, ctx, cancel).await {
                    Ok(v) => v,
                    Err(e) => SourcePreparationOutcome::Complete(Err(e)),
                }
            }
            fn equality(a: &Self::Value, b: &Self::Value) -> bool {
                Self::validity(a) && Self::validity(b) && a.complete_eq(b)
            }
            fn validity(v: &Self::Value) -> bool {
                match v {
                    SourcePreparationOutcome::Need(_) => false,
                    SourcePreparationOutcome::Complete(v) => {
                        v.as_ref().err().is_none_or(|e| !e.is_transient())
                    }
                }
            }
        }
    };
}
runtime_key!(RuntimeSourceRequestsKey, RequestsValue, requests::compute);
runtime_key!(RuntimeSourceProfileKey, ProfileValue, profile::compute);
fn deadline(ctx: &DiceComputations<'_>) -> Instant {
    ctx.per_transaction_data()
        .data
        .get::<SelectedScratchPreparationDeadline>()
        .map_or_else(
            |_| Instant::now() + Duration::from_secs(60),
            |d| d.deadline(),
        )
}
fn resources(ctx: &DiceComputations<'_>) -> Result<SelectedScratchResources, RuntimeSourcesError> {
    ctx.per_transaction_data()
        .data
        .get::<SelectedScratchResources>()
        .map(Dupe::dupe)
        .map_err(|_| RuntimeSourcesError::Operational)
}
fn require_active(active: &dyn Fn() -> bool) -> Result<(), RuntimeSourcesError> {
    if active() {
        Ok(())
    } else {
        Err(RuntimeSourcesError::Operational)
    }
}
fn declaration_transaction(
    ctx: &DiceComputations<'_>,
) -> Result<RuntimeSourceDeclarationTransaction, RuntimeSourcesError> {
    ctx.per_transaction_data()
        .data
        .get::<RuntimeSourceDeclarationTransaction>()
        .map(Dupe::dupe)
        .map_err(|_| RuntimeSourcesError::Operational)
}
fn reserve_rows<T>(
    credit: &mut ImageDescriptorReservation,
    count: usize,
) -> Result<Vec<T>, RuntimeSourcesError> {
    credit
        .try_grow(
            count
                .checked_mul(std::mem::size_of::<T>())
                .and_then(|n| n.checked_add(256))
                .ok_or(RuntimeSourcesError::Operational)?,
        )
        .map_err(|_| RuntimeSourcesError::Operational)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| RuntimeSourcesError::Operational)?;
    Ok(rows)
}
fn merge(
    credit: &mut ImageDescriptorReservation,
    left: &PathObservationEpoch,
    right: &PathObservationEpoch,
) -> Result<PathObservationEpoch, RuntimeSourcesError> {
    Ok(super::selected_scratch::merge_checkpoint_frontier(
        credit, left, right,
    )?)
}
