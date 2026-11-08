//! Inactive WP425 immutable workspace protocol. No native resource or permission
//! is carried by these values; Core must validate physical completion separately.
//! Exact admitted facts are original ordering and normal-exit bytes/effects.
//! Names, quotas, Host volatility and infrastructure refusal are Slug-native.

use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationInstanceId;

use crate::HostSelectedExtensionOwner;
use crate::RepoSpec;

mod demand;
mod observation;
mod operation;
mod read_views;
mod sequence;
mod state;
mod state_path;
mod state_sources;

pub use demand::*;
pub use observation::*;
pub use operation::*;
pub use read_views::*;
pub use sequence::*;
pub use state::*;
pub use state_path::RepositoryWorkspaceReadExpectation;

const REPOSITORY_WORKSPACE_EXECUTES: usize = 128;

pub const REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES: u64 = 64 * 1024 * 1024;
pub const REPOSITORY_WORKSPACE_PROCESS_ENTRIES: usize = 8192;
pub const REPOSITORY_WORKSPACE_PROCESS_NAMES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, dupe::Dupe)]
pub enum RepositoryWorkspaceError {
    InvalidPolicy,
    InvalidReservation,
    InvalidPath,
    InvalidMode,
    MissingRoot,
    NonCanonicalEntries,
    MissingParent,
    StateLimit,
    RetentionLimit,
    InvalidOperation,
    InvalidResult,
    ForeignReservation,
    ForeignPredecessor,
    WrongIndex,
    Revoked,
    ReplayConflict,
    UnusedRecords,
}

impl fmt::Display for RepositoryWorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository workspace protocol: {self:?}")
    }
}
impl std::error::Error for RepositoryWorkspaceError {}

/// Required command policy; deliberately has no default or hidden production
/// selection. Retained history is distinct from archive decode/full-tree caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, dupe::Dupe)]
pub struct RepositoryWorkspaceRetentionLimits {
    max_roots: usize,
    metadata_bytes: usize,
    archive_body_bytes: u64,
}

impl RepositoryWorkspaceRetentionLimits {
    pub fn new(
        max_roots: usize,
        metadata_bytes: usize,
        archive_body_bytes: u64,
    ) -> Result<Self, RepositoryWorkspaceError> {
        if max_roots == 0
            || metadata_bytes < RepositoryWorkspaceState::minimum_metadata_bytes()?
            || archive_body_bytes == 0
            || usize::try_from(archive_body_bytes).is_err()
            || max_roots
                .checked_mul(std::mem::size_of::<RepositoryWorkspaceTranscript>())
                .is_none()
        {
            return Err(RepositoryWorkspaceError::InvalidPolicy);
        }
        Ok(Self {
            max_roots,
            metadata_bytes,
            archive_body_bytes,
        })
    }
    pub fn max_roots(self) -> usize {
        self.max_roots
    }
    pub fn metadata_bytes(self) -> usize {
        self.metadata_bytes
    }
    pub fn archive_body_bytes(self) -> u64 {
        self.archive_body_bytes
    }
}

#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceReservation {
    workspace: NormalizedAbsolutePath,
    owner: Arc<HostSelectedExtensionOwner>,
    repository: CanonicalRepoName,
    ordinal: usize,
    expected_repo_spec: Arc<RepoSpec>,
    display_root: NormalizedAbsolutePath,
    instance: PathObservationInstanceId,
    lease: u64,
    limits: RepositoryWorkspaceRetentionLimits,
}

impl fmt::Debug for RepositoryWorkspaceReservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryWorkspaceReservation")
            .field("workspace", &self.workspace)
            .field("owner", &self.owner)
            .field("repository", &self.repository)
            .field("ordinal", &self.ordinal)
            .field("expected_repo_spec", &"<redacted>")
            .field("display_root", &self.display_root)
            .field("instance", &self.instance)
            .field("lease", &self.lease)
            .field("limits", &self.limits)
            .finish()
    }
}

impl Hash for RepositoryWorkspaceReservation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Routing only. RepoSpec has full structural Eq but deliberately no Hash.
        self.workspace.hash(state);
        self.owner.hash(state);
        self.repository.hash(state);
        self.ordinal.hash(state);
        self.display_root.hash(state);
        self.instance.hash(state);
        self.lease.hash(state);
        self.limits.hash(state);
    }
}

impl RepositoryWorkspaceReservation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        workspace: NormalizedAbsolutePath,
        owner: Arc<HostSelectedExtensionOwner>,
        repository: CanonicalRepoName,
        ordinal: usize,
        expected_repo_spec: Arc<RepoSpec>,
        display_root: NormalizedAbsolutePath,
        instance: PathObservationInstanceId,
        lease: u64,
        limits: RepositoryWorkspaceRetentionLimits,
    ) -> Result<Self, RepositoryWorkspaceError> {
        if instance.value() == 0
            || lease == 0
            || repository.is_root()
            || display_root.as_path().parent().is_none()
            || display_root
                .as_path()
                .to_str()
                .is_none_or(|p| p.len() > 4096 || p.contains('\0'))
        {
            return Err(RepositoryWorkspaceError::InvalidReservation);
        }
        expected_repo_spec.workspace_retained_storage_bytes()?;
        Ok(Self {
            workspace,
            owner,
            repository,
            ordinal,
            expected_repo_spec,
            display_root,
            instance,
            lease,
            limits,
        })
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.owner
    }
    pub fn repository(&self) -> &CanonicalRepoName {
        &self.repository
    }
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn expected_repo_spec(&self) -> &Arc<RepoSpec> {
        &self.expected_repo_spec
    }
    pub fn display_root(&self) -> &NormalizedAbsolutePath {
        &self.display_root
    }
    pub fn instance(&self) -> PathObservationInstanceId {
        self.instance
    }
    pub fn lease(&self) -> u64 {
        self.lease
    }
    pub fn limits(&self) -> RepositoryWorkspaceRetentionLimits {
        self.limits
    }

    /// Conservative complete reservation guard, including nested shared owners
    /// and spare string capacity. Equal foreign guards are not assumed shared.
    #[doc(hidden)]
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        self.metadata()
    }

    fn metadata(&self) -> Result<usize, RepositoryWorkspaceError> {
        add(
            add(
                array_bytes::<Self>(1)?,
                self.expected_repo_spec.workspace_retained_storage_bytes()?,
            )?,
            add(
                self.repository.retained_capacity_bytes(),
                add(
                    self.owner
                        .retained_storage_bytes()
                        .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
                    add(
                        self.workspace
                            .retained_storage_bytes()
                            .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
                        self.display_root
                            .retained_storage_bytes()
                            .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
                    )?,
                )?,
            )?,
        )
    }
    fn same_slot(&self, other: &Self) -> bool {
        self.workspace == other.workspace
            && self.owner == other.owner
            && self.repository == other.repository
            && self.ordinal == other.ordinal
    }
}

fn add(left: usize, right: usize) -> Result<usize, RepositoryWorkspaceError> {
    left.checked_add(right)
        .ok_or(RepositoryWorkspaceError::RetentionLimit)
}
fn array_bytes<T>(count: usize) -> Result<usize, RepositoryWorkspaceError> {
    let items = std::alloc::Layout::array::<T>(count)
        .map_err(|_| RepositoryWorkspaceError::RetentionLimit)?;
    let (owner, _) = std::alloc::Layout::new::<[usize; 2]>()
        .extend(items)
        .map_err(|_| RepositoryWorkspaceError::RetentionLimit)?;
    Ok(owner.pad_to_align().size())
}

fn valid_relative(path: &str, root_allowed: bool) -> bool {
    (root_allowed && path.is_empty())
        || (!path.is_empty()
            && !path.contains(['\0', '\\'])
            && path
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".." && part.len() <= 255))
}

#[cfg(test)]
mod tests;
