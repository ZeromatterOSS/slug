//! Normal source requests and ELF certificates for actual typed checkpoint tools.
//! Native storage and runnable runtime authority remain separate owners.
use std::time::Duration;
use std::time::Instant;

use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::HostRepositoryLabelPathSource;
use slug_bzlmod_v2::SelectedLabelSourceKey;
use slug_bzlmod_v2::SelectedScratchElfMetadataRequest;
use slug_bzlmod_v2::SelectedScratchElfMetadataResult;
use slug_bzlmod_v2::SelectedScratchElfMetadataSelector;
use slug_bzlmod_v2::SelectedScratchElfMetadataTransaction;
use slug_bzlmod_v2::SelectedScratchResources;
use slug_bzlmod_v2::SelectedToolOperand;
use slug_bzlmod_v2::SelectedToolSourceError;
use slug_bzlmod_v2::SelectedToolSourceSelection;
use slug_bzlmod_v2::SelectedToolSourcesPolicy;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ElfMetadataError;
use slug_workspace_v2::ElfMetadataOutcome;
use slug_workspace_v2::ElfSourceDigestBinding;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::ImageDescriptorReservation;
use slug_workspace_v2::ObservedElfMetadata;
use slug_workspace_v2::ObservedElfMetadataKey;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::VerifiedElfMetadata;

use super::selected_scratch::HostSelectedExtensionScratchPrefixKey;
use super::selected_scratch::ObservedSelectedExtensionScratchPrefix;
use super::selected_scratch::SelectedScratchPrefixError;
use super::selected_scratch::SelectedScratchPreparationDeadline;
use super::*;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct SelectedExtensionToolSourceRequestsKey {
    checkpoint: HostSelectedExtensionScratchPrefixKey,
    policy: SelectedToolSourcesPolicy,
}
impl SelectedExtensionToolSourceRequestsKey {
    pub fn new(
        checkpoint: HostSelectedExtensionScratchPrefixKey,
        policy: SelectedToolSourcesPolicy,
    ) -> Self {
        Self { checkpoint, policy }
    }
    pub fn checkpoint(&self) -> &HostSelectedExtensionScratchPrefixKey {
        &self.checkpoint
    }
    pub fn policy(&self) -> SelectedToolSourcesPolicy {
        self.policy
    }
}
impl fmt::Display for SelectedExtensionToolSourceRequestsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selected-tool-requests:{}", self.checkpoint)
    }
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct SelectedExtensionToolSourcesKey(SelectedExtensionToolSourceRequestsKey);
impl SelectedExtensionToolSourcesKey {
    pub fn new(
        checkpoint: HostSelectedExtensionScratchPrefixKey,
        policy: SelectedToolSourcesPolicy,
    ) -> Self {
        Self(SelectedExtensionToolSourceRequestsKey::new(
            checkpoint, policy,
        ))
    }
    pub fn requests_key(&self) -> &SelectedExtensionToolSourceRequestsKey {
        &self.0
    }
    pub fn checkpoint(&self) -> &HostSelectedExtensionScratchPrefixKey {
        self.0.checkpoint()
    }
    pub fn policy(&self) -> SelectedToolSourcesPolicy {
        self.0.policy()
    }
}
impl fmt::Display for SelectedExtensionToolSourcesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selected-tool-sources:{}", self.0)
    }
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum SelectedToolSourcesError {
    Shared(SelectedToolSourceError),
    Prefix(SelectedScratchPrefixError),
    Label(SelectedLabelSourceError),
    Elf(ElfMetadataError),
    Operational,
    UnsupportedSource,
    NotExecutable,
    RejectedElf,
}
impl SelectedToolSourcesError {
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Shared(e) => e.is_transient(),
            Self::Prefix(e) => e.is_transient(),
            Self::Label(e) => e.is_transient(),
            Self::Elf(e) => e.is_transient(),
            Self::Operational => true,
            _ => false,
        }
    }
    pub(crate) fn operational() -> Self {
        Self::Operational
    }
}
impl fmt::Debug for SelectedToolSourcesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedToolSourcesError")
            .field("transient", &self.is_transient())
            .finish()
    }
}
impl fmt::Display for SelectedToolSourcesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("selected tool source preparation refused")
    }
}
impl std::error::Error for SelectedToolSourcesError {}
impl From<SelectedToolSourceError> for SelectedToolSourcesError {
    fn from(e: SelectedToolSourceError) -> Self {
        Self::Shared(e)
    }
}
impl From<SelectedScratchPrefixError> for SelectedToolSourcesError {
    fn from(e: SelectedScratchPrefixError) -> Self {
        Self::Prefix(e)
    }
}
impl From<SelectedLabelSourceError> for SelectedToolSourcesError {
    fn from(e: SelectedLabelSourceError) -> Self {
        Self::Label(e)
    }
}
impl From<ElfMetadataError> for SelectedToolSourcesError {
    fn from(e: ElfMetadataError) -> Self {
        Self::Elf(e)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum SelectedToolSourceRequest {
    Scratch(SelectedScratchElfMetadataRequest),
    Label {
        selection: SelectedToolSourceSelection,
        selected: slug_bzlmod_v2::ObservedSelectedLabelSource,
    },
}
impl SelectedToolSourceRequest {
    pub fn selection(&self) -> &SelectedToolSourceSelection {
        match self {
            Self::Scratch(r) => r.selection(),
            Self::Label { selection, .. } => selection,
        }
    }
    pub fn operand(&self) -> SelectedToolOperand {
        self.selection().operand()
    }
    pub fn scratch_request(&self) -> Option<&SelectedScratchElfMetadataRequest> {
        match self {
            Self::Scratch(r) => Some(r),
            _ => None,
        }
    }
    pub fn label_selection(&self) -> Option<&slug_bzlmod_v2::ObservedSelectedLabelSource> {
        match self {
            Self::Label { selected, .. } => Some(selected),
            _ => None,
        }
    }
}
#[derive(Debug, PartialEq, Eq, Allocative)]
struct RequestsInner {
    prefix: ObservedSelectedExtensionScratchPrefix,
    selector: SelectedScratchElfMetadataSelector,
    requests: [Option<SelectedToolSourceRequest>; 3],
    length: usize,
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedSelectedExtensionToolSourceRequests(BudgetedImageBlock<RequestsInner>);
impl ObservedSelectedExtensionToolSourceRequests {
    pub fn prefix(&self) -> &ObservedSelectedExtensionScratchPrefix {
        &self.0.get().prefix
    }
    pub fn selector(&self) -> &SelectedScratchElfMetadataSelector {
        &self.0.get().selector
    }
    pub fn policy(&self) -> SelectedToolSourcesPolicy {
        self.selector().policy()
    }
    pub fn requests(&self) -> impl ExactSizeIterator<Item = &SelectedToolSourceRequest> {
        self.0.get().requests[..self.0.get().length]
            .iter()
            .map(|r| r.as_ref().expect("filled role"))
    }
    pub fn matches_key(&self, key: &SelectedExtensionToolSourceRequestsKey) -> bool {
        self.prefix().matches_key(key.checkpoint()) && self.policy() == key.policy()
    }
    pub fn same_resources(&self, r: &SelectedScratchResources) -> bool {
        self.0.same_budget(r.descriptor_budget())
            && self.prefix().same_resources(r)
            && self.selector().same_resources(r)
            && self.requests().all(|request| {
                request.selection().same_resources(r)
                    && request
                        .label_selection()
                        .is_none_or(|l| l.same_budget(r.descriptor_budget()))
            })
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum ObservedSelectedToolSource {
    Scratch(SelectedScratchElfMetadataResult),
    Label {
        selection: SelectedToolSourceSelection,
        selected: slug_bzlmod_v2::ObservedSelectedLabelSource,
        metadata: ObservedElfMetadata,
    },
}
impl ObservedSelectedToolSource {
    pub fn selection(&self) -> &SelectedToolSourceSelection {
        match self {
            Self::Scratch(r) => r.request().selection(),
            Self::Label { selection, .. } => selection,
        }
    }
    pub fn operand(&self) -> SelectedToolOperand {
        self.selection().operand()
    }
    pub fn metadata(&self) -> &VerifiedElfMetadata {
        match self {
            Self::Scratch(r) => r.verified(),
            Self::Label { metadata, .. } => metadata.verified(),
        }
    }
    pub fn scratch_result(&self) -> Option<&SelectedScratchElfMetadataResult> {
        match self {
            Self::Scratch(r) => Some(r),
            _ => None,
        }
    }
    pub fn label_selection(&self) -> Option<&slug_bzlmod_v2::ObservedSelectedLabelSource> {
        match self {
            Self::Label { selected, .. } => Some(selected),
            _ => None,
        }
    }
    pub fn label_metadata(&self) -> Option<&ObservedElfMetadata> {
        match self {
            Self::Label { metadata, .. } => Some(metadata),
            _ => None,
        }
    }
}
#[derive(Debug, PartialEq, Eq, Allocative)]
struct SourcesInner {
    requests: ObservedSelectedExtensionToolSourceRequests,
    sources: [Option<ObservedSelectedToolSource>; 3],
    observations: PathObservationEpoch,
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedSelectedExtensionToolSources(BudgetedImageBlock<SourcesInner>);
impl ObservedSelectedExtensionToolSources {
    pub fn requests(&self) -> &ObservedSelectedExtensionToolSourceRequests {
        &self.0.get().requests
    }
    pub fn prefix(&self) -> &ObservedSelectedExtensionScratchPrefix {
        self.requests().prefix()
    }
    pub fn policy(&self) -> SelectedToolSourcesPolicy {
        self.requests().policy()
    }
    pub fn sources(&self) -> impl ExactSizeIterator<Item = &ObservedSelectedToolSource> {
        self.0.get().sources[..self.policy().operands().len()]
            .iter()
            .map(|r| r.as_ref().expect("certified role"))
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.0.get().observations
    }
    pub fn matches_key(&self, key: &SelectedExtensionToolSourcesKey) -> bool {
        self.requests().matches_key(key.requests_key())
    }
    pub fn same_resources(&self, r: &SelectedScratchResources) -> bool {
        self.0.same_budget(r.descriptor_budget())
            && self.requests().same_resources(r)
            && self.sources().all(|source| {
                source.selection().same_resources(r)
                    && source.metadata().same_budget(r.descriptor_budget())
                    && source.scratch_result().is_none_or(|v| v.same_resources(r))
                    && source
                        .label_metadata()
                        .is_none_or(|v| v.same_budget(r.descriptor_budget()))
            })
    }
    /// Userdata authorization is not a DICE dependency. Check it on warm reuse too.
    pub fn authorized_by(
        &self,
        tx: &SelectedScratchElfMetadataTransaction,
        r: &SelectedScratchResources,
    ) -> bool {
        tx.same_resources(r)
            && tx.authorizes_selector(self.requests().selector())
            && self.sources().all(|source| {
                source.scratch_result().is_none_or(|result| {
                    tx.is_authorized(result.request()) && tx.get(result.request()) == Some(result)
                })
            })
    }
}

type RequestsValue = SourcePreparationOutcome<
    Result<ObservedSelectedExtensionToolSourceRequests, SelectedToolSourcesError>,
>;
type SourcesValue = SourcePreparationOutcome<
    Result<ObservedSelectedExtensionToolSources, SelectedToolSourcesError>,
>;
macro_rules! tool_key {
    ($key:ty, $value:ty, $compute:ident) => {
        #[async_trait]
        impl Key for $key {
            type Value = $value;
            async fn compute(
                &self,
                ctx: &mut DiceComputations,
                cancellation: &CancellationContext,
            ) -> Self::Value {
                match $compute(self, ctx, cancellation).await {
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
tool_key!(
    SelectedExtensionToolSourceRequestsKey,
    RequestsValue,
    compute_requests
);
tool_key!(
    SelectedExtensionToolSourcesKey,
    SourcesValue,
    compute_sources
);

fn deadline(ctx: &DiceComputations<'_>) -> Instant {
    let deadline = ctx
        .per_transaction_data()
        .data
        .get::<SelectedScratchPreparationDeadline>()
        .map_or_else(
            |_| Instant::now() + Duration::from_secs(60),
            |d| d.deadline(),
        );
    deadline
}
fn require_active(active: &dyn Fn() -> bool) -> Result<(), SelectedToolSourcesError> {
    if active() {
        Ok(())
    } else {
        Err(SelectedToolSourcesError::Operational)
    }
}
fn resources(
    ctx: &DiceComputations<'_>,
) -> Result<SelectedScratchResources, SelectedToolSourcesError> {
    ctx.per_transaction_data()
        .data
        .get::<SelectedScratchResources>()
        .map(Dupe::dupe)
        .map_err(|_| SelectedToolSourcesError::Operational)
}
fn reserve(
    r: &SelectedScratchResources,
) -> Result<ImageDescriptorReservation, SelectedToolSourcesError> {
    r.descriptor_budget()
        .try_reserve(0)
        .map_err(|_| SelectedToolSourcesError::Operational)
}
fn digest(
    bytes: &[u8],
    active: &dyn Fn() -> bool,
) -> Result<ElfSourceDigestBinding, SelectedToolSourcesError> {
    let mut digest = Sha256::new();
    for chunk in bytes.chunks(64 << 10) {
        require_active(active)?;
        digest.update(chunk);
    }
    require_active(active)?;
    let content = FileContentDigest::new(digest.finalize().into(), bytes.len() as u64)
        .ok_or(SelectedToolSourcesError::Operational)?;
    Ok(ElfSourceDigestBinding::from_digest(content))
}

async fn compute_requests(
    key: &SelectedExtensionToolSourceRequestsKey,
    ctx: &mut DiceComputations<'_>,
    cancellation: &CancellationContext,
) -> Result<RequestsValue, SelectedToolSourcesError> {
    let deadline = deadline(ctx);
    let active = || !cancellation.is_cancelled() && Instant::now() < deadline;
    require_active(&active)?;
    let r = resources(ctx)?;
    let prefix =
        match crate::dependency_cycle::compute_selected_scratch_prefix(ctx, key.checkpoint())
            .await?
        {
            SourcePreparationOutcome::Need(need) => {
                return Ok(SourcePreparationOutcome::Need(need));
            }
            SourcePreparationOutcome::Complete(value) => value?,
        };
    let credit = reserve(&r)?;
    let selector =
        SelectedScratchElfMetadataSelector::from_prefix(prefix.prefix(), key.policy(), &r)?;
    let mut requests = [None, None, None];
    for (index, operand) in key.policy().operands().iter().enumerate() {
        require_active(&active)?;
        let selection =
            SelectedToolSourceSelection::from_prefix(prefix.prefix(), key.policy(), *operand, &r)?;
        requests[index] = Some(if let Some((_, _, blob, _)) = selection.scratch_entry() {
            SelectedToolSourceRequest::Scratch(SelectedScratchElfMetadataRequest::new(
                &selection,
                digest(blob.bytes(), &active)?,
                &r,
            )?)
        } else {
            let token = selection
                .external_path()
                .ok_or(SelectedToolSourcesError::Operational)?;
            let recorded = prefix
                .label_sources()
                .iter()
                .find(|l| l.event_index() == token.event_index())
                .ok_or(SelectedToolSourcesError::Operational)?;
            if recorded.selection().selected_file().as_path().to_str() != Some(token.spelling()) {
                return Err(SelectedToolSourcesError::Operational);
            }
            let selected = match crate::dependency_cycle::compute_selected_label_source(
                ctx,
                &SelectedLabelSourceKey::new(recorded.selection().recipe().dupe()),
            )
            .await?
            {
                SourcePreparationOutcome::Need(need) => {
                    return Ok(SourcePreparationOutcome::Need(need));
                }
                SourcePreparationOutcome::Complete(value) => value?,
            };
            if &selected != recorded.selection() {
                return Err(SelectedToolSourcesError::Operational);
            }
            if let HostRepositoryLabelPathSource::External(route) = selected.recipe().source() {
                if !route.is_immutable_materialization() {
                    return Err(SelectedToolSourcesError::UnsupportedSource);
                }
            }
            SelectedToolSourceRequest::Label {
                selection,
                selected,
            }
        });
    }
    require_active(&active)?;
    Ok(SourcePreparationOutcome::Complete(Ok(
        ObservedSelectedExtensionToolSourceRequests(
            credit
                .freeze(RequestsInner {
                    prefix,
                    selector,
                    requests,
                    length: key.policy().operands().len(),
                })
                .map_err(|_| SelectedToolSourcesError::Operational)?,
        ),
    )))
}

async fn compute_sources(
    key: &SelectedExtensionToolSourcesKey,
    ctx: &mut DiceComputations<'_>,
    cancellation: &CancellationContext,
) -> Result<SourcesValue, SelectedToolSourcesError> {
    let deadline = deadline(ctx);
    let active = || !cancellation.is_cancelled() && Instant::now() < deadline;
    require_active(&active)?;
    let r = resources(ctx)?;
    let requests = match crate::dependency_cycle::compute_selected_tool_source_requests(
        ctx,
        key.requests_key(),
    )
    .await?
    {
        SourcePreparationOutcome::Need(need) => return Ok(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(value) => value?,
    };
    let transaction = ctx
        .per_transaction_data()
        .data
        .get::<SelectedScratchElfMetadataTransaction>()
        .map_err(|_| SelectedToolSourcesError::Operational)?
        .dupe();
    if !transaction.same_resources(&r) || !transaction.authorizes_selector(requests.selector()) {
        return Err(SelectedToolSourcesError::Operational);
    }
    let mut credit = reserve(&r)?;
    let mut observations = requests.prefix().observations().dupe();
    let mut sources = [None, None, None];
    for (index, request) in requests.requests().enumerate() {
        require_active(&active)?;
        let source = match request {
            SelectedToolSourceRequest::Scratch(request) => {
                let result =
                    match slug_bzlmod_v2::lookup_selected_scratch_elf_metadata(ctx, request)
                        .await
                        .map_err(|_| SelectedToolSourcesError::Operational)?
                    {
                        SourcePreparationOutcome::Need(need) => {
                            return Ok(SourcePreparationOutcome::Need(need));
                        }
                        SourcePreparationOutcome::Complete(value) => value?,
                    };
                ObservedSelectedToolSource::Scratch(result)
            }
            SelectedToolSourceRequest::Label {
                selection,
                selected,
            } => {
                let elf_key = ObservedElfMetadataKey::from_charged(
                    selected.image_key().dupe(),
                    selected.selected_file().dupe(),
                    key.policy().elf_policy(),
                )?;
                let metadata = match ctx
                    .compute(&elf_key)
                    .await
                    .map_err(|_| SelectedToolSourcesError::Operational)?
                {
                    PathOutcome::Need(need) => {
                        return Ok(SourcePreparationOutcome::Need(
                            SourcePreparationNeeds::path(need),
                        ));
                    }
                    PathOutcome::Complete(value) => value?,
                };
                let executable = metadata.image().entries().iter().any(|e| {
                    e.path() == selected.selected_file()
                        && e.file_digest().is_some()
                        && e.mode() & 0o111 != 0
                });
                if !executable {
                    return Err(SelectedToolSourcesError::NotExecutable);
                }
                observations = super::selected_scratch::merge_checkpoint_frontier(
                    &mut credit,
                    &observations,
                    metadata.observations(),
                )?;
                ObservedSelectedToolSource::Label {
                    selection: selection.dupe(),
                    selected: selected.dupe(),
                    metadata,
                }
            }
        };
        if !matches!(source.metadata().outcome(), ElfMetadataOutcome::Parsed(_)) {
            return Err(SelectedToolSourcesError::RejectedElf);
        }
        sources[index] = Some(source);
    }
    require_active(&active)?;
    let value = ObservedSelectedExtensionToolSources(
        credit
            .freeze(SourcesInner {
                requests,
                sources,
                observations,
            })
            .map_err(|_| SelectedToolSourcesError::Operational)?,
    );
    if !value.same_resources(&r) || !value.authorized_by(&transaction, &r) {
        return Err(SelectedToolSourcesError::Operational);
    }
    Ok(SourcePreparationOutcome::Complete(Ok(value)))
}
