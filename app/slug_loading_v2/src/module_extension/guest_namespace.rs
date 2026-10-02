//! Normal initial guest facts. Virtual metadata is explicit policy, never stage stat.
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::GuestNamespacePolicy;
use slug_bzlmod_v2::GuestNamespacePolicyError;
use slug_bzlmod_v2::GuestNamespaceTransaction;
use slug_bzlmod_v2::RawGuestNamespacePolicyKey;
use slug_bzlmod_v2::RuntimeSourceDeclarationTransaction;
use slug_bzlmod_v2::SelectedScratchElfMetadataTransaction;
use slug_bzlmod_v2::SelectedScratchResources;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::ImageDescriptorBudget;

use super::runtime_sources::ObservedRuntimeSourceProfile;
use super::runtime_sources::RuntimeSourceProfileKey;
use super::runtime_sources::RuntimeSourcesError;
use super::*;

mod compose;
mod resolver;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct SelectedGuestNamespaceFactsKey(RuntimeSourceProfileKey);
impl SelectedGuestNamespaceFactsKey {
    pub fn new(profile: RuntimeSourceProfileKey) -> Self {
        Self(profile)
    }
    pub fn profile_key(&self) -> &RuntimeSourceProfileKey {
        &self.0
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        self.0.workspace()
    }
}
impl fmt::Display for SelectedGuestNamespaceFactsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "guest-namespace:{}", self.0)
    }
}

#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum GuestNamespaceFactsError {
    Policy(GuestNamespacePolicyError),
    Runtime(RuntimeSourcesError),
    MissingDeclaration,
    Operational,
    Collision,
    UnsupportedSource,
    Limit,
}
impl GuestNamespaceFactsError {
    pub(crate) fn operational() -> Self {
        Self::Operational
    }
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Policy(e) => e.is_transient(),
            Self::Runtime(e) => e.is_transient(),
            Self::Operational => true,
            _ => false,
        }
    }
}
impl fmt::Debug for GuestNamespaceFactsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuestNamespaceFactsError")
            .field("transient", &self.is_transient())
            .finish()
    }
}
impl fmt::Display for GuestNamespaceFactsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("guest namespace preparation refused")
    }
}
impl std::error::Error for GuestNamespaceFactsError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum GuestNamespaceNodeKind {
    Directory,
    RegularFile(FileContentDigest),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum GuestNamespaceBinding {
    Scaffold,
    ScratchEntry { entry: usize },
    ToolImage { role: usize, entry: usize },
    RuntimeImage { declaration: usize, entry: usize },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub struct GuestNamespaceMetadata {
    pub device: u64,
    pub inode: u64,
    pub nlink: u64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub block_size: u64,
    pub blocks: u64,
    pub epoch_seconds: i64,
    pub epoch_nanoseconds: u32,
}
#[derive(Debug, PartialEq, Eq, Allocative)]
pub struct GuestNamespaceNode {
    index: usize,
    path: String,
    kind: GuestNamespaceNodeKind,
    mode: u32,
    binding: GuestNamespaceBinding,
    read_only: bool,
    parent: Option<usize>,
    first_child: usize,
    child_count: usize,
    metadata: GuestNamespaceMetadata,
}
impl GuestNamespaceNode {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn kind(&self) -> GuestNamespaceNodeKind {
        self.kind
    }
    pub fn mode(&self) -> u32 {
        self.mode
    }
    pub fn metadata(&self) -> GuestNamespaceMetadata {
        self.metadata
    }
    pub fn binding(&self) -> GuestNamespaceBinding {
        self.binding
    }
    pub fn read_only(&self) -> bool {
        self.read_only
    }
    pub fn is_owned_tree(&self) -> bool {
        self.binding != GuestNamespaceBinding::Scaffold
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum GuestNamespaceOperation {
    Lstat,
    Stat,
    Access(u8),
    ListDirectory,
    ReadLink,
    ReadRegular,
    Unsupported,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum GuestLookupError {
    Missing,
    NotDirectory,
    PermissionDenied,
    ReadOnly,
    NotSymlink,
    IsDirectory,
    Unowned,
    Unsupported,
    InvalidPath,
    PathLimit,
    ObservationRejected,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum GuestDirectoryCheck {
    Traverse,
    Membership,
    Metadata,
    Listing,
}

#[derive(Debug, Allocative)]
struct FactsInner {
    key: SelectedGuestNamespaceFactsKey,
    profile: ObservedRuntimeSourceProfile,
    policy: GuestNamespacePolicy,
    nodes: Vec<GuestNamespaceNode>,
    children: Vec<usize>,
    lookup_budget: ImageDescriptorBudget,
}
impl PartialEq for FactsInner {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.profile == other.profile
            && self.policy == other.policy
            && self.nodes == other.nodes
            && self.children == other.children
    }
}
impl Eq for FactsInner {}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedGuestNamespaceFacts(BudgetedImageBlock<FactsInner>);
impl ObservedGuestNamespaceFacts {
    pub fn profile(&self) -> &ObservedRuntimeSourceProfile {
        &self.0.get().profile
    }
    pub fn policy(&self) -> GuestNamespacePolicy {
        self.0.get().policy
    }
    pub fn nodes(&self) -> &[GuestNamespaceNode] {
        &self.0.get().nodes
    }
    pub fn node(&self, index: usize) -> Option<&GuestNamespaceNode> {
        self.nodes().get(index)
    }
    pub fn children(&self, index: usize) -> &[usize] {
        let Some(n) = self.node(index) else {
            return &[];
        };
        &self.0.get().children[n.first_child..n.first_child + n.child_count]
    }
    pub fn matches_key(&self, key: &SelectedGuestNamespaceFactsKey) -> bool {
        &self.0.get().key == key && self.profile().matches_key(key.profile_key())
    }
    pub fn same_resources(&self, r: &SelectedScratchResources) -> bool {
        self.0.same_budget(r.descriptor_budget())
            && self
                .0
                .get()
                .lookup_budget
                .same_instance(r.descriptor_budget())
            && self.profile().same_resources(r)
    }
    pub fn authorized_by(
        &self,
        guest: &GuestNamespaceTransaction,
        runtime: &RuntimeSourceDeclarationTransaction,
        scratch: &SelectedScratchElfMetadataTransaction,
        r: &SelectedScratchResources,
    ) -> bool {
        guest.workspace() == self.0.get().key.workspace()
            && guest.authorizes(&self.policy())
            && guest.same_budget(r.descriptor_budget())
            && self.same_resources(r)
            && self.profile().authorized_by(runtime, scratch, r)
    }
}

type Value =
    SourcePreparationOutcome<Result<ObservedGuestNamespaceFacts, GuestNamespaceFactsError>>;
#[async_trait]
impl Key for SelectedGuestNamespaceFactsKey {
    type Value = Value;
    async fn compute(&self, ctx: &mut DiceComputations, cancel: &CancellationContext) -> Value {
        match compute(self, ctx, cancel).await {
            Ok(v) => v,
            Err(e) => SourcePreparationOutcome::Complete(Err(e)),
        }
    }
    fn equality(a: &Value, b: &Value) -> bool {
        Self::validity(a) && Self::validity(b) && a.complete_eq(b)
    }
    fn validity(v: &Value) -> bool {
        match v {
            SourcePreparationOutcome::Need(_) => false,
            SourcePreparationOutcome::Complete(v) => {
                v.as_ref().err().is_none_or(|e| !e.is_transient())
            }
        }
    }
}
async fn compute(
    key: &SelectedGuestNamespaceFactsKey,
    ctx: &mut DiceComputations<'_>,
    cancel: &CancellationContext,
) -> Result<Value, GuestNamespaceFactsError> {
    let deadline = ctx
        .per_transaction_data()
        .data
        .get::<selected_scratch::SelectedScratchPreparationDeadline>()
        .map_or_else(
            |_| Instant::now() + Duration::from_secs(60),
            |d| d.deadline(),
        );
    let active = || !cancel.is_cancelled() && Instant::now() < deadline;
    if !active() {
        return Err(GuestNamespaceFactsError::Operational);
    }
    let policy = ctx
        .compute(&RawGuestNamespacePolicyKey::new(key.workspace().dupe()))
        .await
        .map_err(|_| GuestNamespaceFactsError::Operational)?
        .ok_or(GuestNamespaceFactsError::MissingDeclaration)?;
    let resources = ctx
        .per_transaction_data()
        .data
        .get::<SelectedScratchResources>()
        .map_err(|_| GuestNamespaceFactsError::Operational)?
        .dupe();
    let guest = ctx
        .per_transaction_data()
        .data
        .get::<GuestNamespaceTransaction>()
        .map_err(|_| GuestNamespaceFactsError::Operational)?
        .dupe();
    if guest.workspace() != key.workspace()
        || !guest.authorizes(&policy)
        || !guest.same_budget(resources.descriptor_budget())
    {
        return Err(GuestNamespaceFactsError::Operational);
    }
    let profile =
        match crate::dependency_cycle::compute_runtime_source_profile(ctx, key.profile_key())
            .await
            .map_err(GuestNamespaceFactsError::Runtime)?
        {
            SourcePreparationOutcome::Need(n) => return Ok(SourcePreparationOutcome::Need(n)),
            SourcePreparationOutcome::Complete(v) => {
                v.map_err(GuestNamespaceFactsError::Runtime)?
            }
        };
    let runtime = ctx
        .per_transaction_data()
        .data
        .get::<RuntimeSourceDeclarationTransaction>()
        .map_err(|_| GuestNamespaceFactsError::Operational)?
        .dupe();
    let scratch = ctx
        .per_transaction_data()
        .data
        .get::<SelectedScratchElfMetadataTransaction>()
        .map_err(|_| GuestNamespaceFactsError::Operational)?
        .dupe();
    let facts = compose::build(key, profile, policy, &resources, &active)?;
    if !active() || !facts.authorized_by(&guest, &runtime, &scratch, &resources) {
        return Err(GuestNamespaceFactsError::Operational);
    }
    Ok(SourcePreparationOutcome::Complete(Ok(facts)))
}
