//! Pure child-visible repository inputs. These projections contain neither
//! upstream transcripts nor native root, mount, namespace or reader authority.
use super::*;
use crate::RepoSpec;
use crate::source_preparation::workspace_identity::WorkspaceSpecStorageWalk;

pub const REPOSITORY_WORKSPACE_MAX_READ_VIEWS: usize = 128;

/// Native owners may use owned_bytes only while separate canonical source-data
/// guards cover every referenced projection. The complete total remains the
/// conservative bound for foreign values without those lifetime guards.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceStoragePartition {
    owned_bytes: usize,
    source_state_bytes: usize,
}
impl RepositoryWorkspaceStoragePartition {
    pub(crate) fn new(
        owned_bytes: usize,
        source_state_bytes: usize,
    ) -> Result<Self, RepositoryWorkspaceError> {
        add(owned_bytes, source_state_bytes)?;
        Ok(Self {
            owned_bytes,
            source_state_bytes,
        })
    }
    pub fn owned_bytes(self) -> usize {
        self.owned_bytes
    }
    pub fn source_state_bytes(self) -> usize {
        self.source_state_bytes
    }
    pub fn total_bytes(self) -> Result<usize, RepositoryWorkspaceError> {
        add(self.owned_bytes, self.source_state_bytes)
    }
    pub(crate) fn add_owned(self, bytes: usize) -> Result<Self, RepositoryWorkspaceError> {
        Self::new(add(self.owned_bytes, bytes)?, self.source_state_bytes)
    }
    pub(crate) fn add_partition(self, other: Self) -> Result<Self, RepositoryWorkspaceError> {
        Self::new(
            add(self.owned_bytes, other.owned_bytes)?,
            add(self.source_state_bytes, other.source_state_bytes)?,
        )
    }
}

/// Borrowed state edges for scope-bounded exact comparison. No source can be
/// represented by a fabricated mutable workspace state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryWorkspaceReadViewStateRef<'a> {
    Workspace(&'a Arc<RepositoryWorkspaceState>),
    Source(&'a Arc<crate::RepositorySourceSnapshotState>),
}

#[derive(Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryWorkspaceReadView {
    Workspace {
        repo_spec: RepoSpec,
        state: Arc<RepositoryWorkspaceState>,
    },
    Source {
        state: Arc<crate::RepositorySourceSnapshotState>,
    },
}
impl fmt::Debug for RepositoryWorkspaceReadView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryWorkspaceReadView")
            .field("repository", self.repository())
            .field("instance", &self.instance())
            .field("repo_spec", &"<redacted>")
            .finish()
    }
}
impl RepositoryWorkspaceReadView {
    pub fn new(
        repo_spec: RepoSpec,
        state: Arc<RepositoryWorkspaceState>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        repo_spec.workspace_retained_storage_bytes()?;
        state.retained_metadata_bytes()?;
        if &repo_spec != state.reservation().expected_repo_spec().as_ref() {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        Ok(Self::Workspace { repo_spec, state })
    }
    pub fn from_source(
        state: Arc<crate::RepositorySourceSnapshotState>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        state.retained_storage_bytes()?;
        Ok(Self::Source { state })
    }
    pub(super) fn try_eq_with_states<E>(
        &self,
        other: &Self,
        compare: &mut impl FnMut(
            RepositoryWorkspaceReadViewStateRef<'_>,
            RepositoryWorkspaceReadViewStateRef<'_>,
        ) -> Result<bool, E>,
    ) -> Result<bool, E> {
        match (self, other) {
            (
                Self::Workspace { repo_spec, state },
                Self::Workspace {
                    repo_spec: other_spec,
                    state: other_state,
                },
            ) => {
                if repo_spec != other_spec {
                    return Ok(false);
                }
                compare(
                    RepositoryWorkspaceReadViewStateRef::Workspace(state),
                    RepositoryWorkspaceReadViewStateRef::Workspace(other_state),
                )
            }
            (Self::Source { state }, Self::Source { state: other_state }) => compare(
                RepositoryWorkspaceReadViewStateRef::Source(state),
                RepositoryWorkspaceReadViewStateRef::Source(other_state),
            ),
            _ => Ok(false),
        }
    }
    pub fn workspace_state(&self) -> Option<&Arc<RepositoryWorkspaceState>> {
        match self {
            Self::Workspace { state, .. } => Some(state),
            _ => None,
        }
    }
    pub fn source_state(&self) -> Option<&Arc<crate::RepositorySourceSnapshotState>> {
        match self {
            Self::Source { state } => Some(state),
            _ => None,
        }
    }
    pub fn repo_spec(&self) -> Option<&RepoSpec> {
        match self {
            Self::Workspace { repo_spec, .. } => Some(repo_spec),
            Self::Source { state } => state.reservation().request().repo_spec(),
        }
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        match self {
            Self::Workspace { state, .. } => state.reservation().workspace(),
            Self::Source { state } => state.reservation().workspace(),
        }
    }
    pub fn repository(&self) -> &CanonicalRepoName {
        match self {
            Self::Workspace { state, .. } => state.reservation().repository(),
            Self::Source { state } => state.reservation().repository(),
        }
    }
    pub fn display_root(&self) -> &NormalizedAbsolutePath {
        match self {
            Self::Workspace { state, .. } => state.reservation().display_root(),
            Self::Source { state } => state.reservation().display_root(),
        }
    }
    pub fn instance(&self) -> PathObservationInstanceId {
        match self {
            Self::Workspace { state, .. } => state.reservation().instance(),
            Self::Source { state } => state.reservation().instance(),
        }
    }
    pub fn lease(&self) -> u64 {
        match self {
            Self::Workspace { state, .. } => state.reservation().lease(),
            Self::Source { state } => state.reservation().lease(),
        }
    }
    pub(super) fn owned_metadata_bytes_with(
        &self,
        walk: &mut WorkspaceSpecStorageWalk,
    ) -> Result<usize, RepositoryWorkspaceError> {
        match self {
            Self::Workspace { repo_spec, .. } => {
                repo_spec.workspace_retained_storage_bytes_with(walk)
            }
            Self::Source { state } => state
                .reservation()
                .request()
                .retained_storage_bytes_with(walk),
        }
    }
    fn source_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        match self {
            Self::Workspace { state, .. } => state.retained_storage_bytes(),
            Self::Source { state } => state.retained_storage_bytes(),
        }
    }
    /// Complete reachable allocation bound. Source sizes constrain physical
    /// backing separately; digest-only states allocate no file payloads.
    #[doc(hidden)]
    pub fn storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        RepositoryWorkspaceStoragePartition::new(
            add(
                std::mem::size_of::<Self>(),
                self.owned_metadata_bytes_with(&mut WorkspaceSpecStorageWalk::default())?,
            )?,
            self.source_storage_bytes()?,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceReadViewManifest {
    workspace: NormalizedAbsolutePath,
    lease: u64,
    views: Arc<[RepositoryWorkspaceReadView]>,
}
impl RepositoryWorkspaceReadViewManifest {
    pub(super) fn try_eq_with_states<E>(
        &self,
        other: &Self,
        compare: &mut impl FnMut(
            RepositoryWorkspaceReadViewStateRef<'_>,
            RepositoryWorkspaceReadViewStateRef<'_>,
        ) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let Self {
            workspace,
            lease,
            views,
        } = self;
        let Self {
            workspace: other_workspace,
            lease: other_lease,
            views: other_views,
        } = other;
        if workspace != other_workspace || lease != other_lease || views.len() != other_views.len()
        {
            return Ok(false);
        }
        for (view, other) in views.iter().zip(other_views.iter()) {
            if !view.try_eq_with_states(other, compare)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    /// Canonicalize the declaration set at THIS Execute prefix. Discoveries at
    /// later prefixes must not rewrite a prior completed operation's manifest.
    pub fn new(
        executor: &RepositoryWorkspaceReservation,
        mut views: Vec<RepositoryWorkspaceReadView>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        if views.len() > REPOSITORY_WORKSPACE_MAX_READ_VIEWS {
            return Err(RepositoryWorkspaceError::StateLimit);
        }
        Self::validate_rows(executor, &views)?;
        views.sort_unstable_by(|a, b| {
            a.display_root()
                .cmp(b.display_root())
                .then_with(|| a.repository().cmp(b.repository()))
        });
        Ok(Self {
            workspace: executor.workspace.clone(),
            lease: executor.lease,
            views: views.into(),
        })
    }
    pub fn empty(executor: &RepositoryWorkspaceReservation) -> Self {
        Self {
            workspace: executor.workspace.clone(),
            lease: executor.lease,
            views: Arc::from([]),
        }
    }
    pub fn views(&self) -> &[RepositoryWorkspaceReadView] {
        &self.views
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
    pub fn lease(&self) -> u64 {
        self.lease
    }
    pub fn validate_for(
        &self,
        executor: &RepositoryWorkspaceReservation,
    ) -> Result<(), RepositoryWorkspaceError> {
        if self.workspace != executor.workspace || self.lease != executor.lease {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        Self::validate_rows(executor, &self.views)
    }
    fn validate_rows(
        executor: &RepositoryWorkspaceReservation,
        views: &[RepositoryWorkspaceReadView],
    ) -> Result<(), RepositoryWorkspaceError> {
        if views.len() > REPOSITORY_WORKSPACE_MAX_READ_VIEWS {
            return Err(RepositoryWorkspaceError::StateLimit);
        }
        let mut walk = WorkspaceSpecStorageWalk::default();
        for (index, view) in views.iter().enumerate() {
            if view.workspace() != &executor.workspace
                || view.lease() != executor.lease
                || view
                    .workspace_state()
                    .is_some_and(|s| s.reservation().limits != executor.limits)
                || view.instance() == executor.instance
                || view.repository() == &executor.repository
                || overlaps(
                    view.display_root().as_path(),
                    executor.display_root.as_path(),
                )
                || views[..index].iter().any(|p| {
                    p.repository() == view.repository()
                        || p.instance() == view.instance()
                        || overlaps(p.display_root().as_path(), view.display_root().as_path())
                })
            {
                return Err(RepositoryWorkspaceError::ForeignReservation);
            }
            if let RepositoryWorkspaceReadView::Workspace { repo_spec, state } = view {
                if repo_spec != state.reservation().expected_repo_spec().as_ref() {
                    return Err(RepositoryWorkspaceError::ForeignReservation);
                }
            }
            view.owned_metadata_bytes_with(&mut walk)?;
        }
        Ok(())
    }
    pub(super) fn owned_metadata_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        let mut bytes = add(
            std::mem::size_of::<Self>(),
            array_bytes::<RepositoryWorkspaceReadView>(self.views.len())?,
        )?;
        bytes = add(
            bytes,
            self.workspace
                .retained_storage_bytes()
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
        )?;
        let mut walk = WorkspaceSpecStorageWalk::default();
        for view in self.views.iter() {
            bytes = add(bytes, view.owned_metadata_bytes_with(&mut walk)?)?;
        }
        Ok(bytes)
    }
    #[doc(hidden)]
    pub fn storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let mut sources = 0;
        for view in self.views.iter() {
            sources = add(sources, view.source_storage_bytes()?)?
        }
        RepositoryWorkspaceStoragePartition::new(self.owned_metadata_bytes()?, sources)
    }
}
fn overlaps(a: &std::path::Path, b: &std::path::Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

pub(super) fn source_storage_bytes(
    mut visit: impl FnMut(&mut dyn FnMut(&RepositoryWorkspaceReadView)),
) -> Result<usize, RepositoryWorkspaceError> {
    let mut bytes = Ok(0usize);
    visit(&mut |view| {
        bytes = bytes.and_then(|old| add(old, view.source_storage_bytes()?));
    });
    bytes
}
