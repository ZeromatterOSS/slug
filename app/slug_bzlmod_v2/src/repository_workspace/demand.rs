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
    prefix_source_states: Arc<[Arc<crate::RepositorySourceSnapshotState>]>,
}
impl fmt::Debug for RepositoryWorkspaceOperationDemand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryWorkspaceOperationDemand")
            .field("request", &self.request)
            .field("selected_count", &self.selected.len())
            .field("source_count", &self.prefix_source_states.len())
            .finish()
    }
}
impl RepositoryWorkspaceOperationDemand {
    pub fn new(
        request: Arc<RepositoryWorkspaceRequest>,
        selected: Arc<[RepositoryMaterializationEpochEntry]>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        Self::new_with_sources(request, selected, Arc::from([]))
    }
    pub fn new_with_sources(
        request: Arc<RepositoryWorkspaceRequest>,
        selected: Arc<[RepositoryMaterializationEpochEntry]>,
        prefix_source_states: Arc<[Arc<crate::RepositorySourceSnapshotState>]>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        if selected
            .len()
            .checked_add(prefix_source_states.len())
            .is_none_or(|n| n > REPOSITORY_WORKSPACE_MAX_READ_VIEWS)
        {
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
        let executor = request.reservation();
        let overlaps =
            |a: &std::path::Path, b: &std::path::Path| a.starts_with(b) || b.starts_with(a);
        fn binding(
            entry: &RepositoryMaterializationEpochEntry,
        ) -> Option<(PathObservationInstanceId, &std::path::Path)> {
            if let crate::RepositoryMaterializationKind::GeneralWorkspace(realization) =
                &entry.request.kind
            {
                Some((
                    realization.reservation().instance(),
                    realization.reservation().display_root().as_path(),
                ))
            } else if let crate::RepositoryMaterializationResult::Success(
                crate::RepositoryMaterializationSuccess::Immutable {
                    observation_instance,
                    generation_root,
                    ..
                },
            ) = &entry.result
            {
                Some((*observation_instance, generation_root.as_path()))
            } else {
                None
            }
        }
        for (index, entry) in selected.iter().enumerate() {
            if binding(entry).is_some_and(|(instance, path)| {
                instance == executor.instance()
                    || overlaps(path, executor.display_root().as_path())
                    || selected[..index].iter().any(|previous| {
                        binding(previous).is_some_and(|(previous_instance, previous_path)| {
                            instance == previous_instance || overlaps(path, previous_path)
                        })
                    })
            }) {
                return Err(RepositoryWorkspaceError::ForeignReservation);
            }
            if let crate::RepositoryMaterializationKind::GeneralWorkspace(realization) =
                &entry.request.kind
            {
                entry.request.validate_general_workspace_binding()?;
                if realization.reservation().lease() != executor.lease() {
                    return Err(RepositoryWorkspaceError::ForeignReservation);
                }
            }
        }
        for (index, state) in prefix_source_states.iter().enumerate() {
            let source = state.reservation();
            state.retained_storage_bytes()?;
            if source.workspace() != executor.workspace()
                || source.lease() != executor.lease()
                || source.repository() == executor.repository()
                || source.instance() == executor.instance()
                || overlaps(
                    source.display_root().as_path(),
                    executor.display_root().as_path(),
                )
                || selected.iter().any(|entry| {
                    entry.request.id.canonical_repo == *source.repository()
                        || binding(entry).is_some_and(|(instance, path)| {
                            instance == source.instance()
                                || overlaps(path, source.display_root().as_path())
                        })
                })
                || prefix_source_states[..index].iter().any(|previous| {
                    let previous = previous.reservation();
                    previous.repository() == source.repository()
                        || previous.instance() == source.instance()
                        || overlaps(
                            previous.display_root().as_path(),
                            source.display_root().as_path(),
                        )
                })
            {
                return Err(RepositoryWorkspaceError::ForeignReservation);
            }
        }
        if let RepositoryWorkspaceOperation::Execute(execute) = request.operation() {
            let manifest = execute.read_views();
            manifest.validate_for(executor)?;
            let sources = manifest
                .views()
                .iter()
                .filter_map(|view| view.source_state());
            if sources.clone().count() != prefix_source_states.len()
                || sources.into_iter().any(|state| {
                    !prefix_source_states
                        .iter()
                        .any(|declared| declared == state)
                })
            {
                return Err(RepositoryWorkspaceError::ReplayConflict);
            }
        }
        if let RepositoryWorkspaceOperation::Symlink {
            source: Some(view), ..
        } = request.operation()
        {
            if !prefix_source_states
                .iter()
                .any(|declared| view.source_state() == Some(declared))
            {
                return Err(RepositoryWorkspaceError::ReplayConflict);
            }
        }
        let prefix_source_states = if prefix_source_states
            .windows(2)
            .all(|pair| pair[0].reservation().repository() < pair[1].reservation().repository())
        {
            prefix_source_states
        } else {
            let mut rows = prefix_source_states.to_vec();
            rows.sort_unstable_by(|a, b| {
                a.reservation()
                    .repository()
                    .cmp(b.reservation().repository())
            });
            rows.into()
        };
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
        Ok(Self {
            request,
            selected,
            prefix_source_states,
        })
    }
    pub fn request(&self) -> &Arc<RepositoryWorkspaceRequest> {
        &self.request
    }
    pub fn selected_entries(&self) -> &[RepositoryMaterializationEpochEntry] {
        &self.selected
    }
    pub fn declared_sources(&self) -> &[Arc<crate::RepositorySourceSnapshotState>] {
        &self.prefix_source_states
    }
}
