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

#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceReadView {
    repo_spec: RepoSpec,
    state: Arc<RepositoryWorkspaceState>,
}
impl fmt::Debug for RepositoryWorkspaceReadView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryWorkspaceReadView")
            .field("repository", self.state.reservation().repository())
            .field("instance", &self.state.reservation().instance())
            .field("mode", &self.state.mode())
            .field("entries", &self.state.entries().len())
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
        Ok(Self { repo_spec, state })
    }
    pub(super) fn try_eq_with_states<E>(
        &self,
        other: &Self,
        compare: &mut impl FnMut(
            &Arc<RepositoryWorkspaceState>,
            &Arc<RepositoryWorkspaceState>,
        ) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let Self { repo_spec, state } = self;
        let Self {
            repo_spec: other_spec,
            state: other_state,
        } = other;
        if repo_spec != other_spec {
            return Ok(false);
        }
        compare(state, other_state)
    }
    pub fn repo_spec(&self) -> &RepoSpec {
        &self.repo_spec
    }
    pub fn state(&self) -> &Arc<RepositoryWorkspaceState> {
        &self.state
    }
    /// Source state includes its own reservation/control/entry/body allocations;
    /// none of those allocations belong to mutable Process history.
    #[doc(hidden)]
    pub fn storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        RepositoryWorkspaceStoragePartition::new(
            add(
                std::mem::size_of::<Self>(),
                self.repo_spec.workspace_retained_storage_bytes()?,
            )?,
            self.state.retained_storage_bytes()?,
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
            &Arc<RepositoryWorkspaceState>,
            &Arc<RepositoryWorkspaceState>,
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
            a.state
                .reservation()
                .display_root
                .cmp(&b.state.reservation().display_root)
                .then_with(|| {
                    a.state
                        .reservation()
                        .repository
                        .cmp(&b.state.reservation().repository)
                })
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
            let source = view.state.reservation();
            if source.workspace != executor.workspace
                || source.lease != executor.lease
                || source.limits != executor.limits
                || source.instance == executor.instance
                || source.repository == executor.repository
                || overlaps(
                    source.display_root.as_path(),
                    executor.display_root.as_path(),
                )
                || views[..index].iter().any(|prior| {
                    let prior = prior.state.reservation();
                    prior.repository == source.repository
                        || prior.instance == source.instance
                        || overlaps(prior.display_root.as_path(), source.display_root.as_path())
                })
            {
                return Err(RepositoryWorkspaceError::ForeignReservation);
            }
            // One shared depth/edge budget across all specs, including repeated
            // shared attribute occurrences. Equality never depends on a hash.
            view.repo_spec
                .workspace_retained_storage_bytes_with(&mut walk)?;
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
            bytes = add(
                bytes,
                view.repo_spec
                    .workspace_retained_storage_bytes_with(&mut walk)?,
            )?;
        }
        Ok(bytes)
    }
    #[doc(hidden)]
    pub fn storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let mut sources = 0;
        for view in self.views.iter() {
            sources = add(sources, view.state.retained_storage_bytes()?)?
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
        bytes = bytes.and_then(|old| add(old, view.state.retained_storage_bytes()?));
    });
    bytes
}
