//! Exact observation cutoffs, not native admission or a reduced command epoch.
use dice::DiceProjectionComputations;
use dice::ProjectionKey;

use super::*;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryWorkspaceContext {
    workspace: NormalizedAbsolutePath,
    lease: u64,
    limits: RepositoryWorkspaceRetentionLimits,
}
impl RepositoryWorkspaceContext {
    /// Observe the actual command context without constructing or retaining an
    /// epoch. A revoked or foreign epoch never becomes an empty command.
    pub fn from_epoch(
        epoch: &RepositoryWorkspaceEpoch,
        workspace: &NormalizedAbsolutePath,
    ) -> Result<Arc<Self>, RepositoryWorkspaceError> {
        let actual = epoch.workspace().ok_or(RepositoryWorkspaceError::Revoked)?;
        if actual != workspace {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        Ok(Arc::new(Self {
            workspace: actual.clone(),
            lease: epoch.lease().ok_or(RepositoryWorkspaceError::Revoked)?,
            limits: epoch.limits().ok_or(RepositoryWorkspaceError::Revoked)?,
        }))
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
    pub fn lease(&self) -> u64 {
        self.lease
    }
    pub fn limits(&self) -> RepositoryWorkspaceRetentionLimits {
        self.limits
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryWorkspaceContextProjection {
    workspace: NormalizedAbsolutePath,
}
impl RepositoryWorkspaceContextProjection {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}
impl fmt::Display for RepositoryWorkspaceContextProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-workspace-context:{}", self.workspace)
    }
}
impl ProjectionKey for RepositoryWorkspaceContextProjection {
    type DeriveFromKey = RepositoryWorkspaceEpochKey;
    type Value = Result<Arc<RepositoryWorkspaceContext>, RepositoryWorkspaceError>;
    fn compute(
        &self,
        epoch: &RepositoryWorkspaceEpoch,
        _: &DiceProjectionComputations,
    ) -> Self::Value {
        RepositoryWorkspaceContext::from_epoch(epoch, &self.workspace)
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceTranscriptObservation {
    context: Arc<RepositoryWorkspaceContext>,
    transcript: Option<RepositoryWorkspaceTranscript>,
}
impl RepositoryWorkspaceTranscriptObservation {
    pub fn from_epoch(
        epoch: &RepositoryWorkspaceEpoch,
        demand: &RepositoryWorkspaceReservationDemand,
    ) -> Result<Arc<Self>, RepositoryWorkspaceError> {
        let context = RepositoryWorkspaceContext::from_epoch(epoch, demand.key().workspace())?;
        let transcript = demand.find_in(epoch)?.cloned();
        Ok(Arc::new(Self {
            context,
            transcript,
        }))
    }
    pub fn context(&self) -> &Arc<RepositoryWorkspaceContext> {
        &self.context
    }
    /// Requests reconstructed from this exact transcript are UNADMITTED. Core
    /// must check its actual full command epoch before any new native effect.
    pub fn transcript(&self) -> Option<&RepositoryWorkspaceTranscript> {
        self.transcript.as_ref()
    }
}
impl fmt::Debug for RepositoryWorkspaceTranscriptObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryWorkspaceTranscriptObservation")
            .field("context", &self.context)
            .field(
                "records",
                &self.transcript.as_ref().map(|t| t.records().len()),
            )
            .finish()
    }
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceTranscriptProjection {
    demand: Arc<RepositoryWorkspaceReservationDemand>,
}
impl RepositoryWorkspaceTranscriptProjection {
    pub fn new(demand: Arc<RepositoryWorkspaceReservationDemand>) -> Self {
        Self { demand }
    }
}
impl Hash for RepositoryWorkspaceTranscriptProjection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.demand.key().hash(state);
    }
}
impl fmt::Debug for RepositoryWorkspaceTranscriptProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryWorkspaceTranscriptProjection")
            .field("workspace", self.demand.key().workspace())
            .field("repository", self.demand.key().repository())
            .field("ordinal", &self.demand.key().ordinal())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for RepositoryWorkspaceTranscriptProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-workspace-transcript:{}",
            self.demand.key().repository()
        )
    }
}
impl ProjectionKey for RepositoryWorkspaceTranscriptProjection {
    type DeriveFromKey = RepositoryWorkspaceEpochKey;
    type Value = Result<Arc<RepositoryWorkspaceTranscriptObservation>, RepositoryWorkspaceError>;
    fn compute(
        &self,
        epoch: &RepositoryWorkspaceEpoch,
        _: &DiceProjectionComputations,
    ) -> Self::Value {
        RepositoryWorkspaceTranscriptObservation::from_epoch(epoch, &self.demand)
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;
