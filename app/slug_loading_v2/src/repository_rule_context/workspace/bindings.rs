//! Full tracked rows prepared by actual path/source accesses, never an epoch.
use slug_bzlmod_v2::RepositoryMaterializationSuccess;
use slug_bzlmod_v2::RepositorySourceSnapshotState;
use slug_workspace_v2::MaterializationPathAnchor;

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum RepositoryWorkspaceSourceBinding {
    Source(Arc<RepositorySourceSnapshotState>),
    Materialized(RepositoryMaterializationEpochEntry),
}
pub(crate) type PreparedRepositoryWorkspaceBindings =
    SmallMap<slug_identity_v2::CanonicalRepoName, RepositoryWorkspaceSourceBinding>;

impl RepositoryWorkspaceSourceBinding {
    pub(crate) fn repository(&self) -> &slug_identity_v2::CanonicalRepoName {
        match self {
            Self::Source(state) => state.reservation().repository(),
            Self::Materialized(entry) => &entry.request.id.canonical_repo,
        }
    }
    pub(crate) fn validate_path(
        &self,
        workspace: &NormalizedAbsolutePath,
        lease: u64,
        path: &HostRepositoryLabelPathValue,
    ) -> Result<(), RepositoryRuleInvocationError> {
        let invalid = RepositoryRuleInvocationError::WorkspaceReplay;
        match self {
            Self::Source(state) => {
                let root = state.reservation();
                if root.workspace() != workspace
                    || root.lease() != lease
                    || path.namespace()
                        != PathObservationNamespace::Materialization(root.instance())
                    || !path
                        .path()
                        .as_path()
                        .starts_with(root.display_root().as_path())
                {
                    return Err(invalid);
                }
            }
            Self::Materialized(entry) => {
                if &entry.request.id.workspace != workspace {
                    return Err(invalid);
                }
                match &entry.result {
                    slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Immutable {
                            generation_root,
                            observation_instance,
                            ..
                        },
                    ) => {
                        if path.namespace()
                            != PathObservationNamespace::Materialization(*observation_instance)
                            || !path.path().as_path().starts_with(generation_root)
                        {
                            return Err(invalid);
                        }
                        if let RepositoryMaterializationKind::GeneralWorkspace(realization) =
                            &entry.request.kind
                        {
                            let root = realization.reservation();
                            if root.lease() != lease
                                || root.instance() != *observation_instance
                                || root.display_root().as_path() != generation_root.as_path()
                            {
                                return Err(invalid);
                            }
                        }
                    }
                    slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Local,
                    ) if path.namespace() == PathObservationNamespace::Host => {}
                    _ => return Err(invalid),
                }
            }
        }
        Ok(())
    }
    pub(crate) fn anchor(
        &self,
        workspace: &NormalizedAbsolutePath,
        lease: u64,
        path: &HostRepositoryLabelPathValue,
    ) -> Result<MaterializationPathAnchor, RepositoryRuleInvocationError> {
        self.validate_path(workspace, lease, path)?;
        match self {
            Self::Source(state) => MaterializationPathAnchor::new(
                path.namespace(),
                state.reservation().display_root().clone(),
            )
            .map_err(|_| RepositoryRuleInvocationError::WorkspaceReplay),
            Self::Materialized(entry) => match (&entry.request.kind, &entry.result) {
                (RepositoryMaterializationKind::GeneralWorkspace(realization), _) => realization
                    .final_state()
                    .resolution_anchor()
                    .map_err(RepositoryRuleInvocationError::Workspace),
                (
                    _,
                    slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Immutable {
                            generation_root, ..
                        },
                    ),
                ) => MaterializationPathAnchor::new(
                    path.namespace(),
                    NormalizedAbsolutePath::new(generation_root)
                        .map_err(|_| RepositoryRuleInvocationError::WorkspaceReplay)?,
                )
                .map_err(|_| RepositoryRuleInvocationError::WorkspaceReplay),
                _ => Err(RepositoryRuleInvocationError::WorkspaceUnsupported),
            },
        }
    }
}
