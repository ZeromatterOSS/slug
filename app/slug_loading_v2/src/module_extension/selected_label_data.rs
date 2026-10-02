//! Normal ordered issued-Label sources. No string lookup or parent-tree discovery.
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::GuestLabelDataPolicy;
use slug_bzlmod_v2::GuestNamespacePolicy;
use slug_bzlmod_v2::GuestNamespacePolicyError;
use slug_bzlmod_v2::GuestNamespaceTransaction;
use slug_bzlmod_v2::ObservedSelectedLabelSource;
use slug_bzlmod_v2::RawGuestNamespacePolicyKey;
use slug_bzlmod_v2::RuntimeSourceDeclarationTransaction;
use slug_bzlmod_v2::SelectedLabelSourceError;
use slug_bzlmod_v2::SelectedScratchElfMetadataTransaction;
use slug_bzlmod_v2::SelectedScratchResources;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageDescriptorReservation;
use slug_workspace_v2::InputImageError;
use slug_workspace_v2::ObservedInputImage;
use slug_workspace_v2::ObservedInputImageKey;

use super::runtime_sources::ObservedRuntimeSourceProfile;
use super::runtime_sources::ObservedRuntimeSourceRequests;
use super::runtime_sources::RuntimeSourceProfileKey;
use super::runtime_sources::RuntimeSourceRequestsKey;
use super::runtime_sources::RuntimeSourcesError;
use super::*;
mod requests;
mod sources;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct SelectedDataLabelRequestsKey(RuntimeSourceRequestsKey);
impl SelectedDataLabelRequestsKey {
    pub fn new(runtime: RuntimeSourceRequestsKey) -> Self {
        Self(runtime)
    }
    pub fn runtime_requests_key(&self) -> &RuntimeSourceRequestsKey {
        &self.0
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        self.0.workspace()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct SelectedDataLabelSourcesKey(RuntimeSourceProfileKey);
impl SelectedDataLabelSourcesKey {
    pub fn new(profile: RuntimeSourceProfileKey) -> Self {
        Self(profile)
    }
    pub fn profile_key(&self) -> &RuntimeSourceProfileKey {
        &self.0
    }
    pub fn requests_key(&self) -> SelectedDataLabelRequestsKey {
        SelectedDataLabelRequestsKey::new(self.0.requests_key().clone())
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        self.0.workspace()
    }
}
impl fmt::Display for SelectedDataLabelRequestsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "data-label-requests:{}", self.0)
    }
}
impl fmt::Display for SelectedDataLabelSourcesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "data-label-sources:{}", self.0)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum SelectedDataLabelError {
    Policy(GuestNamespacePolicyError),
    Runtime(RuntimeSourcesError),
    Label(SelectedLabelSourceError),
    Image(InputImageError),
    Disabled,
    MissingDeclaration,
    WitnessLimit,
    InvalidReuse,
    UnsupportedSource,
    Collision,
    AggregateLimit,
    Operational,
}
impl SelectedDataLabelError {
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Policy(e) => e.is_transient(),
            Self::Runtime(e) => e.is_transient(),
            Self::Label(e) => e.is_transient(),
            Self::Image(e) => e.is_transient(),
            Self::Operational => true,
            _ => false,
        }
    }
    pub(crate) fn operational() -> Self {
        Self::Operational
    }
}
impl fmt::Display for SelectedDataLabelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("issued Label data source preparation refused")
    }
}
impl std::error::Error for SelectedDataLabelError {}
impl From<RuntimeSourcesError> for SelectedDataLabelError {
    fn from(e: RuntimeSourcesError) -> Self {
        Self::Runtime(e)
    }
}
impl From<SelectedLabelSourceError> for SelectedDataLabelError {
    fn from(e: SelectedLabelSourceError) -> Self {
        Self::Label(e)
    }
}
impl From<InputImageError> for SelectedDataLabelError {
    fn from(e: InputImageError) -> Self {
        Self::Image(e)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct SelectedDataLabelRequest {
    index: usize,
    event: usize,
    selection: ObservedSelectedLabelSource,
    image: ObservedInputImageKey,
}
impl SelectedDataLabelRequest {
    pub fn occurrence_index(&self) -> usize {
        self.index
    }
    pub fn event_index(&self) -> usize {
        self.event
    }
    pub fn selection(&self) -> &ObservedSelectedLabelSource {
        &self.selection
    }
    pub fn leaf_image_key(&self) -> &ObservedInputImageKey {
        &self.image
    }
}
#[derive(Debug, PartialEq, Eq, Allocative)]
struct RequestsInner {
    key: SelectedDataLabelRequestsKey,
    policy: GuestNamespacePolicy,
    runtime: ObservedRuntimeSourceRequests,
    rows: Vec<SelectedDataLabelRequest>,
    observations: PathObservationEpoch,
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedSelectedDataLabelRequests(BudgetedImageBlock<RequestsInner>);
impl ObservedSelectedDataLabelRequests {
    pub fn policy(&self) -> GuestNamespacePolicy {
        self.0.get().policy
    }
    pub fn runtime_requests(&self) -> &ObservedRuntimeSourceRequests {
        &self.0.get().runtime
    }
    pub fn requests(&self) -> impl ExactSizeIterator<Item = &SelectedDataLabelRequest> {
        self.0.get().rows.iter()
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.0.get().observations
    }
    pub fn matches_key(&self, key: &SelectedDataLabelRequestsKey) -> bool {
        &self.0.get().key == key
            && self
                .runtime_requests()
                .matches_key(key.runtime_requests_key())
    }
    pub fn same_resources(&self, r: &SelectedScratchResources) -> bool {
        let b = r.descriptor_budget();
        self.0.same_budget(b)
            && self.runtime_requests().same_resources(r)
            && self.requests().all(|v| v.selection.same_budget(b))
    }
    pub fn authorized_by(
        &self,
        guest: &GuestNamespaceTransaction,
        runtime: &RuntimeSourceDeclarationTransaction,
        scratch: &SelectedScratchElfMetadataTransaction,
        r: &SelectedScratchResources,
    ) -> bool {
        guest.workspace() == self.0.get().key.workspace()
            && guest.same_budget(r.descriptor_budget())
            && guest.authorizes(&self.policy())
            && runtime.same_budget(r.descriptor_budget())
            && self.runtime_requests().authorized_by(runtime)
            && scratch.same_resources(r)
            && scratch.authorizes_selector(self.runtime_requests().tool_requests().selector())
            && self.same_resources(r)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum SelectedDataLabelBinding {
    Leaf { image: ObservedInputImage },
    ToolMember { role: usize, entry: usize },
    RuntimeMember { declaration: usize, entry: usize },
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct SelectedDataLabelSource {
    index: usize,
    binding: SelectedDataLabelBinding,
}
impl SelectedDataLabelSource {
    pub fn occurrence_index(&self) -> usize {
        self.index
    }
    pub fn binding(&self) -> &SelectedDataLabelBinding {
        &self.binding
    }
    pub fn leaf_image(&self) -> Option<&ObservedInputImage> {
        match &self.binding {
            SelectedDataLabelBinding::Leaf { image } => Some(image),
            _ => None,
        }
    }
}
#[derive(Debug, PartialEq, Eq, Allocative)]
struct SourcesInner {
    key: SelectedDataLabelSourcesKey,
    requests: ObservedSelectedDataLabelRequests,
    profile: ObservedRuntimeSourceProfile,
    rows: Vec<SelectedDataLabelSource>,
    observations: PathObservationEpoch,
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedSelectedDataLabelSources(BudgetedImageBlock<SourcesInner>);
impl ObservedSelectedDataLabelSources {
    pub fn requests(&self) -> &ObservedSelectedDataLabelRequests {
        &self.0.get().requests
    }
    pub fn profile(&self) -> &ObservedRuntimeSourceProfile {
        &self.0.get().profile
    }
    pub fn sources(&self) -> impl ExactSizeIterator<Item = &SelectedDataLabelSource> {
        self.0.get().rows.iter()
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.0.get().observations
    }
    pub fn matches_key(&self, key: &SelectedDataLabelSourcesKey) -> bool {
        &self.0.get().key == key
            && self.requests().matches_key(&key.requests_key())
            && self.profile().matches_key(key.profile_key())
    }
    pub fn same_resources(&self, r: &SelectedScratchResources) -> bool {
        self.0.same_budget(r.descriptor_budget())
            && self.requests().same_resources(r)
            && self.profile().same_resources(r)
            && self.sources().all(|s| {
                s.leaf_image()
                    .is_none_or(|i| i.same_budget(r.descriptor_budget()))
            })
    }
    pub fn authorized_by(
        &self,
        g: &GuestNamespaceTransaction,
        rt: &RuntimeSourceDeclarationTransaction,
        s: &SelectedScratchElfMetadataTransaction,
        r: &SelectedScratchResources,
    ) -> bool {
        self.requests().authorized_by(g, rt, s, r)
            && self.profile().authorized_by(rt, s, r)
            && self.same_resources(r)
    }
    /// Exact certified node used by composition; indices never escape retained producers.
    pub fn member<'a>(
        &'a self,
        source: &'a SelectedDataLabelSource,
    ) -> Option<(&'a ObservedInputImage, usize)> {
        match source.binding() {
            SelectedDataLabelBinding::Leaf { image } => Some((image, 0)),
            SelectedDataLabelBinding::ToolMember { role, entry } => Some((
                self.profile()
                    .tool_sources()
                    .sources()
                    .nth(*role)?
                    .label_metadata()?
                    .image(),
                *entry,
            )),
            SelectedDataLabelBinding::RuntimeMember { declaration, entry } => {
                Some((self.profile().images().nth(*declaration)?, *entry))
            }
        }
    }
}
pub type RequestsValue =
    SourcePreparationOutcome<Result<ObservedSelectedDataLabelRequests, SelectedDataLabelError>>;
pub type SourcesValue =
    SourcePreparationOutcome<Result<ObservedSelectedDataLabelSources, SelectedDataLabelError>>;
macro_rules! data_key {
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
            fn validity(v: &Self::Value) -> bool {
                match v {
                    SourcePreparationOutcome::Need(_) => false,
                    SourcePreparationOutcome::Complete(v) => {
                        v.as_ref().err().is_none_or(|e| !e.is_transient())
                    }
                }
            }
            fn equality(a: &Self::Value, b: &Self::Value) -> bool {
                Self::validity(a) && Self::validity(b) && a.complete_eq(b)
            }
        }
    };
}
data_key!(
    SelectedDataLabelRequestsKey,
    RequestsValue,
    requests::compute
);
data_key!(SelectedDataLabelSourcesKey, SourcesValue, sources::compute);
fn deadline(ctx: &DiceComputations<'_>) -> Instant {
    ctx.per_transaction_data()
        .data
        .get::<selected_scratch::SelectedScratchPreparationDeadline>()
        .map_or_else(
            |_| Instant::now() + Duration::from_secs(60),
            |v| v.deadline(),
        )
}
fn active(check: &dyn Fn() -> bool) -> Result<(), SelectedDataLabelError> {
    if check() {
        Ok(())
    } else {
        Err(SelectedDataLabelError::Operational)
    }
}
fn resources(
    ctx: &DiceComputations<'_>,
) -> Result<SelectedScratchResources, SelectedDataLabelError> {
    ctx.per_transaction_data()
        .data
        .get::<SelectedScratchResources>()
        .map(Dupe::dupe)
        .map_err(|_| SelectedDataLabelError::Operational)
}
fn reserve<T>(
    credit: &mut ImageDescriptorReservation,
    count: usize,
) -> Result<Vec<T>, SelectedDataLabelError> {
    credit
        .try_grow(
            count
                .checked_mul(std::mem::size_of::<T>())
                .and_then(|n| n.checked_add(256))
                .ok_or(SelectedDataLabelError::Operational)?,
        )
        .map_err(|_| SelectedDataLabelError::Operational)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| SelectedDataLabelError::Operational)?;
    Ok(rows)
}
fn merge(
    credit: &mut ImageDescriptorReservation,
    a: &PathObservationEpoch,
    b: &PathObservationEpoch,
) -> Result<PathObservationEpoch, SelectedDataLabelError> {
    selected_scratch::merge_checkpoint_frontier(credit, a, b)
        .map_err(|_| SelectedDataLabelError::Operational)
}
fn current<'a, T: 'static + Send + Sync>(
    ctx: &'a DiceComputations<'_>,
) -> Result<&'a T, SelectedDataLabelError> {
    ctx.per_transaction_data()
        .data
        .get::<T>()
        .map_err(|_| SelectedDataLabelError::Operational)
}
pub(crate) fn authorize_requests(
    ctx: &DiceComputations<'_>,
    v: &ObservedSelectedDataLabelRequests,
) -> Result<(), SelectedDataLabelError> {
    if v.authorized_by(current(ctx)?, current(ctx)?, current(ctx)?, current(ctx)?) {
        Ok(())
    } else {
        Err(SelectedDataLabelError::Operational)
    }
}
pub(crate) fn authorize_sources(
    ctx: &DiceComputations<'_>,
    v: &ObservedSelectedDataLabelSources,
) -> Result<(), SelectedDataLabelError> {
    if v.authorized_by(current(ctx)?, current(ctx)?, current(ctx)?, current(ctx)?) {
        Ok(())
    } else {
        Err(SelectedDataLabelError::Operational)
    }
}
