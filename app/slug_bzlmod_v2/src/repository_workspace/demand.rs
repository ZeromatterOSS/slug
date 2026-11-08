//! Resource-free original-evaluation demands. Native authority stays in Core.
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;

use super::*;
use crate::RepositoryMaterializationEpochEntry;
use crate::source_preparation::RepositoryWorkspaceStableKey;

/// Trusted runtime capability, observed before original/prepared reuse. Enabling
/// this policy never selects a caller, source hash, tool or argument grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, dupe::Dupe)]
pub enum RepositoryWorkspaceExecutionPolicy {
    Disabled,
    LinuxReadOnlyHostV1,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryWorkspaceExecutionPolicyKey {
    workspace: NormalizedAbsolutePath,
}
impl RepositoryWorkspaceExecutionPolicyKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}
impl fmt::Display for RepositoryWorkspaceExecutionPolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-workspace-policy:{}", self.workspace)
    }
}
#[async_trait]
impl Key for RepositoryWorkspaceExecutionPolicyKey {
    type Value = RepositoryWorkspaceExecutionPolicy;
    async fn compute(&self, _: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        RepositoryWorkspaceExecutionPolicy::Disabled
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceReservationDemand {
    key: Arc<RepositoryWorkspaceStableKey>,
    repo_spec: Arc<RepoSpec>,
}
impl fmt::Debug for RepositoryWorkspaceReservationDemand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryWorkspaceReservationDemand")
            .field("key", &self.key)
            .field("repo_spec", &"<redacted>")
            .finish()
    }
}
impl RepositoryWorkspaceReservationDemand {
    pub fn new(
        key: Arc<RepositoryWorkspaceStableKey>,
        repo_spec: Arc<RepoSpec>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        key.retained_storage_bytes()?;
        repo_spec.workspace_retained_storage_bytes()?;
        Ok(Self { key, repo_spec })
    }
    pub fn key(&self) -> &Arc<RepositoryWorkspaceStableKey> {
        &self.key
    }
    pub fn repo_spec(&self) -> &Arc<RepoSpec> {
        &self.repo_spec
    }
    pub fn matches(&self, reservation: &RepositoryWorkspaceReservation) -> bool {
        self.same_slot(reservation) && self.repo_spec == *reservation.expected_repo_spec()
    }
    fn same_slot(&self, reservation: &RepositoryWorkspaceReservation) -> bool {
        self.key.workspace() == reservation.workspace()
            && self.key.owner() == reservation.owner()
            && self.key.repository() == reservation.repository()
            && self.key.ordinal() == reservation.ordinal()
    }
    /// The epoch is the only returned binding. A same-slot different Spec is a
    /// refusal rather than a second reservation or an invented display path.
    pub fn find_in<'a>(
        &self,
        epoch: &'a RepositoryWorkspaceEpoch,
    ) -> Result<Option<&'a RepositoryWorkspaceTranscript>, RepositoryWorkspaceError> {
        if epoch.workspace().ok_or(RepositoryWorkspaceError::Revoked)? != self.key.workspace() {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        for transcript in epoch
            .transcripts()
            .ok_or(RepositoryWorkspaceError::Revoked)?
        {
            let reservation = transcript.reservation();
            if self.same_slot(reservation) {
                if !self.matches(reservation) || Some(reservation.lease()) != epoch.lease() {
                    return Err(RepositoryWorkspaceError::ForeignReservation);
                }
                return Ok(Some(transcript));
            }
        }
        Ok(None)
    }
}

/// A conflict slot, deliberately excluding mutable request contents and policy
/// facts. Two requests for this same command/root/index must agree in full.
#[derive(Debug, Clone, Allocative)]
pub struct RepositoryWorkspaceOperationId {
    reservation: Arc<RepositoryWorkspaceReservation>,
    index: usize,
}
impl RepositoryWorkspaceOperationId {
    pub fn new(request: &RepositoryWorkspaceRequest) -> Self {
        Self {
            reservation: request.reservation().clone(),
            index: request.index(),
        }
    }
    pub fn reservation(&self) -> &Arc<RepositoryWorkspaceReservation> {
        &self.reservation
    }
    pub fn index(&self) -> usize {
        self.index
    }
}
impl PartialEq for RepositoryWorkspaceOperationId {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.reservation.same_slot(&other.reservation)
            && self.reservation.instance() == other.reservation.instance()
            && self.reservation.lease() == other.reservation.lease()
    }
}
impl Eq for RepositoryWorkspaceOperationId {}
impl Hash for RepositoryWorkspaceOperationId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.reservation.workspace().hash(state);
        self.reservation.owner().hash(state);
        self.reservation.repository().hash(state);
        self.reservation.ordinal().hash(state);
        self.reservation.instance().hash(state);
        self.reservation.lease().hash(state);
        self.index.hash(state);
    }
}

/// Complete prefix declarations are independent of the operation's manifest.
/// They are transient Need data, never a field of a retained transcript.
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceOperationDemand {
    request: Arc<RepositoryWorkspaceRequest>,
    selected: Arc<[RepositoryMaterializationEpochEntry]>,
}
impl fmt::Debug for RepositoryWorkspaceOperationDemand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryWorkspaceOperationDemand")
            .field("request", &self.request)
            .field("selected_count", &self.selected.len())
            .finish()
    }
}
impl RepositoryWorkspaceOperationDemand {
    pub fn new(
        request: Arc<RepositoryWorkspaceRequest>,
        selected: Arc<[RepositoryMaterializationEpochEntry]>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        if selected.len() > REPOSITORY_WORKSPACE_MAX_READ_VIEWS {
            return Err(RepositoryWorkspaceError::RetentionLimit);
        }
        if selected.iter().any(|entry| {
            entry.request.id.workspace != *request.reservation().workspace()
                || entry.request.id.canonical_repo == *request.reservation().repository()
        }) {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        // At most128 rows: validate uniqueness before any normalization copy.
        for (index, entry) in selected.iter().enumerate() {
            if selected[..index].iter().any(|previous| {
                previous.request.id.canonical_repo == entry.request.id.canonical_repo
            }) {
                return Err(RepositoryWorkspaceError::ForeignReservation);
            }
        }
        let selected = if selected
            .windows(2)
            .all(|pair| pair[0].request.id.canonical_repo < pair[1].request.id.canonical_repo)
        {
            selected
        } else {
            let mut ordered = selected.to_vec();
            ordered.sort_by(|a, b| {
                a.request
                    .id
                    .canonical_repo
                    .cmp(&b.request.id.canonical_repo)
            });
            ordered.into()
        };
        Ok(Self { request, selected })
    }
    pub fn request(&self) -> &Arc<RepositoryWorkspaceRequest> {
        &self.request
    }
    pub fn selected_entries(&self) -> &[RepositoryMaterializationEpochEntry] {
        &self.selected
    }
}
