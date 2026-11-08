//! Opt-in integration probes use the real original evaluator and native owner.
//! This support exposes counters, never roots, descriptors or result injection.
use std::sync::Arc;

use crate::runtime::WorkspaceRuntime;
use crate::runtime::repository_physical_root::RepositoryRootResourceLimits;
use crate::runtime::repository_physical_root::RepositoryRootResourceSnapshot;
use crate::runtime::repository_physical_root::RepositoryRootResources;

/// Scalar instrumentation only. Peaks are historical; last-drop comparisons
/// compare retained_bytes/native_fds/backing_bytes with the baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepositoryWorkspaceIntegrationTestSnapshot {
    pub retained_bytes: usize,
    pub peak_retained_bytes: usize,
    pub native_fds: usize,
    pub peak_native_fds: usize,
    pub backing_bytes: u64,
    pub peak_backing_bytes: u64,
}
impl From<RepositoryRootResourceSnapshot> for RepositoryWorkspaceIntegrationTestSnapshot {
    fn from(value: RepositoryRootResourceSnapshot) -> Self {
        Self {
            retained_bytes: value.retained_bytes,
            peak_retained_bytes: value.peak_retained_bytes,
            native_fds: value.native_fds,
            peak_native_fds: value.peak_native_fds,
            backing_bytes: value.backing_bytes,
            peak_backing_bytes: value.peak_backing_bytes,
        }
    }
}

/// Retains only the existing bank's control owner for final refund inspection.
/// No native acquisition, observation or execution capability is exposed.
pub struct RepositoryWorkspaceIntegrationTestResources {
    resources: Arc<RepositoryRootResources>,
    baseline: RepositoryWorkspaceIntegrationTestSnapshot,
}
impl RepositoryWorkspaceIntegrationTestResources {
    pub fn snapshot(&self) -> RepositoryWorkspaceIntegrationTestSnapshot {
        self.resources.snapshot().into()
    }
    pub fn baseline(&self) -> RepositoryWorkspaceIntegrationTestSnapshot {
        self.baseline
    }
}
impl std::fmt::Debug for RepositoryWorkspaceIntegrationTestResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepositoryWorkspaceIntegrationTestResources")
            .field("snapshot", &self.snapshot())
            .finish_non_exhaustive()
    }
}

impl WorkspaceRuntime {
    /// Trusted test-only activation with the same fixed bounds as native owner
    /// probes. The opt-in feature also uses their existing SLUG_V2_BIN compiled
    /// CLI binding; default builds continue to select their own executable.
    pub fn enable_general_repository_workspaces_for_integration_test(
        &mut self,
    ) -> anyhow::Result<RepositoryWorkspaceIntegrationTestResources> {
        if self.source_scope.is_some() {
            anyhow::bail!(
                "integration repository workspace support requires an ordinary Native runtime"
            );
        }
        let limits =
            RepositoryRootResourceLimits::new(512 * 1024 * 1024, 512, 8 * 1024 * 1024 * 1024)
                .map_err(|error| {
                    anyhow::anyhow!("integration repository bank refused: {error:?}")
                })?;
        let resources = RepositoryRootResources::new(limits)
            .map_err(|error| anyhow::anyhow!("integration repository bank refused: {error:?}"))?;
        let baseline = resources.snapshot().into();
        let retention = slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits::new(
            128,
            16 * 1024 * 1024,
            1024 * 1024 * 1024,
        )?;
        self.configure_general_repository_workspaces(resources.clone(), retention)?;
        Ok(RepositoryWorkspaceIntegrationTestResources {
            resources,
            baseline,
        })
    }
}
