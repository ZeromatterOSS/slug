//! Resource-free closed source images. File digests describe captured content;
//! native sealing and current source association establish acquisition authority.
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationInstanceId;

use crate::BuiltinBazelToolsRouteIdentity;
use crate::HostCanonicalRepositoryRouteKind;
use crate::HostCanonicalRepositorySourceInput;
use crate::HostRepositoryLocalPathPolicy;
use crate::HostRepositorySourceInputDispositionView;
use crate::HostRootRepositoryMapping;
use crate::RepoSpec;
use crate::RepositoryMaterializationKind;
use crate::RepositoryMaterializationRequest;
use crate::RepositoryWorkspaceError;
use crate::source_preparation::workspace_identity::WorkspaceSpecStorageWalk;

mod epoch;
mod state;
pub use epoch::*;
pub use state::*;

pub const REPOSITORY_SOURCE_SNAPSHOT_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const REPOSITORY_SOURCE_SNAPSHOT_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
pub const REPOSITORY_SOURCE_SNAPSHOT_ENTRIES: usize = 8192;
pub const REPOSITORY_SOURCE_SNAPSHOT_NAMES: usize = 1024 * 1024;
pub const REPOSITORY_SOURCE_SNAPSHOT_PATH: usize = 256;
pub const REPOSITORY_SOURCE_SNAPSHOT_DEPTH: usize = 32;
pub const REPOSITORY_SOURCE_SNAPSHOT_METADATA: usize = 8 * 1024 * 1024;

fn add(a: usize, b: usize) -> Result<usize, RepositoryWorkspaceError> {
    a.checked_add(b)
        .ok_or(RepositoryWorkspaceError::RetentionLimit)
}
fn array<T>(n: usize) -> Result<usize, RepositoryWorkspaceError> {
    add(
        n.checked_mul(std::mem::size_of::<T>())
            .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
        2 * std::mem::size_of::<usize>() + std::mem::align_of::<T>() + 64,
    )
}
fn path_bytes(path: &NormalizedAbsolutePath) -> Result<usize, RepositoryWorkspaceError> {
    path.retained_storage_bytes()
        .ok_or(RepositoryWorkspaceError::RetentionLimit)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum RepositorySourceSnapshotKind {
    Workspace,
    Local,
    Immutable,
    Builtin,
}

#[derive(Clone, PartialEq, Eq, Allocative)]
enum Subject {
    Workspace,
    Local {
        request: Arc<RepositoryMaterializationRequest>,
        policy: HostRepositoryLocalPathPolicy,
    },
    Immutable {
        request: Arc<RepositoryMaterializationRequest>,
        policy: HostRepositoryLocalPathPolicy,
    },
    Builtin(BuiltinBazelToolsRouteIdentity),
}

/// A natural source projection, including the selected lookup and evaluated
/// declaration identities. Constructors reject generated/history producers.
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositorySourceSnapshotRequest {
    workspace: NormalizedAbsolutePath,
    repository: CanonicalRepoName,
    lease: u64,
    subject: Subject,
    selected_identity: Option<(CompactString, CompactString)>,
    module_metadata: Option<(CompactString, CompactString)>,
    selected_registry: bool,
    mapping_context: CanonicalRepoName,
    mapping: Arc<[(ApparentRepoName, CanonicalRepoName)]>,
}
impl fmt::Debug for RepositorySourceSnapshotRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositorySourceSnapshotRequest")
            .field("repository", &self.repository)
            .field("lease", &self.lease)
            .field("kind", &self.kind())
            .field("mapping_entries", &self.mapping.len())
            .finish()
    }
}
impl Hash for RepositorySourceSnapshotRequest {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.workspace.hash(h);
        self.repository.hash(h);
        self.lease.hash(h);
        self.kind().hash(h);
    }
}
impl RepositorySourceSnapshotRequest {
    pub fn from_canonical(
        input: &HostCanonicalRepositorySourceInput,
        lease: u64,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let view = input.view();
        let route = view.route().view();
        let subject = match (route.kind(), view.disposition()) {
            (
                HostCanonicalRepositoryRouteKind::Builtin,
                HostRepositorySourceInputDispositionView::Builtin(identity),
            ) if route.builtin_identity().as_ref() == Some(identity) => Subject::Builtin(*identity),
            (
                HostCanonicalRepositoryRouteKind::SelectedRegistry
                | HostCanonicalRepositoryRouteKind::SelectedNonregistry,
                HostRepositorySourceInputDispositionView::Request(request),
            ) => {
                if request.id.workspace != *route.workspace()
                    || request.id.canonical_repo != *route.canonical_repo()
                    || route.repo_spec() != Some(&request.repo_spec)
                {
                    return Err(RepositoryWorkspaceError::ForeignReservation);
                }
                let policy = route
                    .local_path_policy()
                    .ok_or(RepositoryWorkspaceError::InvalidReservation)?;
                match &request.kind {
                    RepositoryMaterializationKind::Local { .. } => Subject::Local {
                        request: request.clone(),
                        policy,
                    },
                    RepositoryMaterializationKind::Immutable => Subject::Immutable {
                        request: request.clone(),
                        policy,
                    },
                    _ => return Err(RepositoryWorkspaceError::InvalidReservation),
                }
            }
            _ => return Err(RepositoryWorkspaceError::InvalidReservation),
        };
        let selected_identity = match route.selected_module_identity() {
            Some(crate::selected_repo_spec::HostCanonicalSelectedModuleIdentity::Module {
                name,
                normalized_version,
            }) => Some((name.into(), normalized_version.into())),
            Some(crate::selected_repo_spec::HostCanonicalSelectedModuleIdentity::Root) => {
                return Err(RepositoryWorkspaceError::InvalidReservation);
            }
            None => None,
        };
        let request = Self {
            workspace: route.workspace().clone(),
            repository: route.canonical_repo().clone(),
            lease,
            subject,
            selected_identity,
            module_metadata: route
                .associated_module_metadata()
                .map(|(n, v)| (n.into(), v.into())),
            selected_registry: route.kind() == HostCanonicalRepositoryRouteKind::SelectedRegistry,
            mapping_context: route.mapping_context().clone(),
            mapping: view.route().bzl_repository_mapping(),
        };
        request.validate()?;
        Ok(request)
    }
    pub fn from_workspace(
        workspace: NormalizedAbsolutePath,
        mapping: &HostRootRepositoryMapping,
        lease: u64,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let mapping = mapping
            .view()
            .ok_or(RepositoryWorkspaceError::InvalidReservation)?;
        if !mapping.canonical_repo().is_root() || !mapping.mapping_context().is_root() {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        let request = Self {
            workspace,
            repository: mapping.canonical_repo().clone(),
            lease,
            subject: Subject::Workspace,
            selected_identity: None,
            module_metadata: None,
            selected_registry: false,
            mapping_context: mapping.mapping_context().clone(),
            mapping: mapping
                .mapping()
                .map(|(a, c)| (a.clone(), c.clone()))
                .collect::<Vec<_>>()
                .into(),
        };
        request.validate()?;
        Ok(request)
    }
    fn validate(&self) -> Result<(), RepositoryWorkspaceError> {
        if self.lease == 0
            || self.workspace.as_path().as_os_str().len() > 4096
            || self.mapping.len() > 131072
        {
            return Err(RepositoryWorkspaceError::InvalidReservation);
        }
        if self.mapping_context != self.repository {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        self.retained_storage_bytes()?;
        Ok(())
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
    pub fn repository(&self) -> &CanonicalRepoName {
        &self.repository
    }
    pub fn lease(&self) -> u64 {
        self.lease
    }
    pub fn kind(&self) -> RepositorySourceSnapshotKind {
        match self.subject {
            Subject::Workspace => RepositorySourceSnapshotKind::Workspace,
            Subject::Local { .. } => RepositorySourceSnapshotKind::Local,
            Subject::Immutable { .. } => RepositorySourceSnapshotKind::Immutable,
            Subject::Builtin(_) => RepositorySourceSnapshotKind::Builtin,
        }
    }
    pub fn materialization_request(&self) -> Option<&Arc<RepositoryMaterializationRequest>> {
        match &self.subject {
            Subject::Local { request, .. } | Subject::Immutable { request, .. } => Some(request),
            _ => None,
        }
    }
    pub fn repo_spec(&self) -> Option<&RepoSpec> {
        self.materialization_request().map(|r| &r.repo_spec)
    }
    pub fn local_root(&self) -> Option<&NormalizedAbsolutePath> {
        match self.materialization_request().map(|r| &r.kind) {
            Some(RepositoryMaterializationKind::Local { logical_root }) => Some(logical_root),
            _ => None,
        }
    }
    pub fn builtin_identity(&self) -> Option<BuiltinBazelToolsRouteIdentity> {
        match self.subject {
            Subject::Builtin(i) => Some(i),
            _ => None,
        }
    }
    pub fn mapping_context(&self) -> &CanonicalRepoName {
        &self.mapping_context
    }
    pub fn mapping(&self) -> &[(ApparentRepoName, CanonicalRepoName)] {
        &self.mapping
    }
    pub fn selected_identity(&self) -> Option<(&str, &str)> {
        self.selected_identity
            .as_ref()
            .map(|(n, v)| (n.as_str(), v.as_str()))
    }
    pub fn module_metadata(&self) -> Option<(&str, &str)> {
        self.module_metadata
            .as_ref()
            .map(|(n, v)| (n.as_str(), v.as_str()))
    }
    pub fn local_path_policy(&self) -> Option<HostRepositoryLocalPathPolicy> {
        match self.subject {
            Subject::Local { policy, .. } | Subject::Immutable { policy, .. } => Some(policy),
            _ => None,
        }
    }
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        self.retained_storage_bytes_with(&mut WorkspaceSpecStorageWalk::default())
    }
    pub(crate) fn retained_storage_bytes_with(
        &self,
        walk: &mut WorkspaceSpecStorageWalk,
    ) -> Result<usize, RepositoryWorkspaceError> {
        let mut bytes = add(array::<Self>(1)?, path_bytes(&self.workspace)?)?;
        bytes = add(
            bytes,
            add(
                self.repository.retained_capacity_bytes(),
                self.mapping_context.retained_capacity_bytes(),
            )?,
        )?;
        bytes = add(
            bytes,
            array::<(ApparentRepoName, CanonicalRepoName)>(self.mapping.len())?,
        )?;
        for (a, c) in self.mapping.iter() {
            bytes = add(
                bytes,
                add(a.retained_capacity_bytes(), c.retained_capacity_bytes())?,
            )?;
        }
        for (n, v) in self
            .selected_identity
            .iter()
            .chain(self.module_metadata.iter())
        {
            bytes = add(bytes, add(n.capacity(), v.capacity())?)?;
        }
        if let Some(request) = self.materialization_request() {
            bytes = add(bytes, array::<RepositoryMaterializationRequest>(1)?)?;
            bytes = add(
                bytes,
                add(
                    path_bytes(&request.id.workspace)?,
                    request.id.canonical_repo.retained_capacity_bytes(),
                )?,
            )?;
            bytes = add(
                bytes,
                request
                    .repo_spec
                    .workspace_retained_storage_bytes_with(walk)?,
            )?;
            if let RepositoryMaterializationKind::Local { logical_root } = &request.kind {
                bytes = add(bytes, path_bytes(logical_root)?)?;
            }
        }
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositorySourceSnapshotReservation {
    request: Arc<RepositorySourceSnapshotRequest>,
    display_root: NormalizedAbsolutePath,
    instance: PathObservationInstanceId,
}
impl RepositorySourceSnapshotReservation {
    pub fn new(
        request: Arc<RepositorySourceSnapshotRequest>,
        display_root: NormalizedAbsolutePath,
        instance: PathObservationInstanceId,
    ) -> Result<Self, RepositoryWorkspaceError> {
        request.validate()?;
        let path = display_root.as_path();
        if instance.value() == 0
            || !path.starts_with("/__slug_repositories__")
            || path == std::path::Path::new("/__slug_repositories__")
            || path.as_os_str().len() > 4096
        {
            return Err(RepositoryWorkspaceError::InvalidReservation);
        }
        Ok(Self {
            request,
            display_root,
            instance,
        })
    }
    pub fn request(&self) -> &Arc<RepositorySourceSnapshotRequest> {
        &self.request
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        self.request.workspace()
    }
    pub fn repository(&self) -> &CanonicalRepoName {
        self.request.repository()
    }
    pub fn display_root(&self) -> &NormalizedAbsolutePath {
        &self.display_root
    }
    pub fn instance(&self) -> PathObservationInstanceId {
        self.instance
    }
    pub fn lease(&self) -> u64 {
        self.request.lease()
    }
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        add(
            add(array::<Self>(1)?, path_bytes(&self.display_root)?)?,
            self.request.retained_storage_bytes()?,
        )
    }
}

#[cfg(test)]
mod tests;
