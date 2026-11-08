//! Runfiles lifetime wrapper over the SAME checked selected-root façade used by
//! native source transport. No additional generation lookup or root map lives here.
use super::physical_roots::SelectedRepositoryRoots;
use super::*;
use crate::runtime::SourceArtifactInput;
use crate::runtime::native_source_access::NativeReaderResources;

#[derive(Clone, Debug, Default)]
pub(in crate::runtime) struct NativeSourceGenerations {
    _roots: SelectedRepositoryRoots,
}
impl RepositoryMaterializer {
    pub(in crate::runtime) fn retain_source_generations<'a>(
        &self,
        token: RepositorySessionToken,
        selected_requests: &[Arc<RepositoryMaterializationRequest>],
        selected_results: &RepositoryMaterializationResultEpoch,
        source_epoch: &slug_bzlmod_v2::RepositorySourceSnapshotEpoch,
        sources: impl Iterator<Item = &'a SourceArtifactInput>,
        resources: &Arc<NativeReaderResources>,
    ) -> Result<NativeSourceGenerations, Arc<str>> {
        let selected = self.pin_selected_roots(
            token,
            selected_requests,
            selected_results,
            source_epoch,
            resources,
        )?;
        Ok(NativeSourceGenerations {
            _roots: selected.retain_sources(sources, resources)?,
        })
    }
}
#[cfg(test)]
mod tests;
