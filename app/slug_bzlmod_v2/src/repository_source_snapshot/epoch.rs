use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;

use super::*;

/// Command-local publication in the source graph, never a resource registry.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositorySourceSnapshotEpoch {
    context: Option<(NormalizedAbsolutePath, u64)>,
    states: Arc<[Arc<RepositorySourceSnapshotState>]>,
}
impl RepositorySourceSnapshotEpoch {
    pub fn fresh(
        workspace: NormalizedAbsolutePath,
        lease: u64,
    ) -> Result<Self, RepositoryWorkspaceError> {
        if lease == 0 {
            return Err(RepositoryWorkspaceError::InvalidReservation);
        }
        Ok(Self {
            context: Some((workspace, lease)),
            states: Arc::from([]),
        })
    }
    pub fn revoked() -> Self {
        Self {
            context: None,
            states: Arc::from([]),
        }
    }
    pub fn workspace(&self) -> Option<&NormalizedAbsolutePath> {
        self.context.as_ref().map(|(w, _)| w)
    }
    pub fn lease(&self) -> Option<u64> {
        self.context.as_ref().map(|(_, l)| *l)
    }
    pub fn states(&self) -> Option<&[Arc<RepositorySourceSnapshotState>]> {
        self.context.as_ref().map(|_| self.states.as_ref())
    }
    pub fn find(
        &self,
        request: &RepositorySourceSnapshotRequest,
    ) -> Result<Option<&Arc<RepositorySourceSnapshotState>>, RepositoryWorkspaceError> {
        let (workspace, lease) = self
            .context
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        if workspace != request.workspace() || *lease != request.lease() {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        if let Some(state) = self
            .states
            .iter()
            .find(|s| s.reservation().repository() == request.repository())
        {
            if state.reservation().request().as_ref() != request {
                return Err(RepositoryWorkspaceError::ReplayConflict);
            }
            return Ok(Some(state));
        }
        Ok(None)
    }
    pub fn with_snapshot(
        &self,
        state: Arc<RepositorySourceSnapshotState>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let request = state.reservation().request();
        if let Some(previous) = self.find(request)? {
            return if previous == &state {
                Ok(self.clone())
            } else {
                Err(RepositoryWorkspaceError::ReplayConflict)
            };
        }
        if self.states.len() == crate::REPOSITORY_WORKSPACE_MAX_READ_VIEWS {
            return Err(RepositoryWorkspaceError::StateLimit);
        }
        if self.states.iter().any(|s| {
            let a = s.reservation();
            let b = state.reservation();
            a.instance() == b.instance()
                || a.display_root()
                    .as_path()
                    .starts_with(b.display_root().as_path())
                || b.display_root()
                    .as_path()
                    .starts_with(a.display_root().as_path())
        }) {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        let mut states = self.states.to_vec();
        states.push(state);
        states.sort_unstable_by(|a, b| {
            a.reservation()
                .repository()
                .cmp(b.reservation().repository())
        });
        Ok(Self {
            context: self.context.clone(),
            states: states.into(),
        })
    }
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        let mut bytes = add(
            std::mem::size_of::<Self>(),
            array::<Arc<RepositorySourceSnapshotState>>(self.states.len())?,
        )?;
        if let Some((w, _)) = &self.context {
            bytes = add(bytes, path_bytes(w)?)?;
        }
        for state in self.states.iter() {
            bytes = add(bytes, state.retained_storage_bytes()?)?;
        }
        Ok(bytes)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositorySourceSnapshotEpochKey {
    workspace: NormalizedAbsolutePath,
}
impl RepositorySourceSnapshotEpochKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}
impl fmt::Display for RepositorySourceSnapshotEpochKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-source-snapshot-epoch:{}", self.workspace)
    }
}
#[async_trait]
impl Key for RepositorySourceSnapshotEpochKey {
    type Value = Arc<RepositorySourceSnapshotEpoch>;
    async fn compute(&self, _: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        Arc::new(RepositorySourceSnapshotEpoch::revoked())
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
}
