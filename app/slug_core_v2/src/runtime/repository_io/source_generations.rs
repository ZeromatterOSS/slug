//! Command-owned leases for exact selected native repository generations.
//!
//! These resources never enter DICE values. They preserve temporary root lifetime,
//! not source freshness or durable backing for published runfiles links.

use starlark_map::small_set::SmallSet;

use super::*;
use crate::runtime::SourceArtifactInput;

#[derive(Clone, Debug, Default)]
pub(in crate::runtime) struct NativeSourceGenerations {
    // The only capability is retaining these owners. No path-list constructor or
    // generic filesystem access is exposed to callers.
    _roots: Arc<[NativeSourceGeneration]>,
}

#[derive(Debug)]
struct NativeSourceGeneration {
    _instance: PathObservationInstanceId,
    _root: Arc<tempfile::TempDir>,
}

struct SourceGenerationInput<'a> {
    repo: &'a CanonicalRepoName,
    namespace: PathObservationNamespace,
    requested_path: &'a NormalizedAbsolutePath,
}

impl RepositoryMaterializer {
    pub(in crate::runtime) fn retain_source_generations<'a>(
        &self,
        token: RepositorySessionToken,
        selected_requests: &[Arc<RepositoryMaterializationRequest>],
        selected_results: &RepositoryMaterializationResultEpoch,
        sources: impl Iterator<Item = &'a SourceArtifactInput>,
    ) -> Result<NativeSourceGenerations, Arc<str>> {
        // Evaluate the caller's iterator before taking the resource-owner lock.
        let sources = sources
            .map(|source| SourceGenerationInput {
                repo: source.label().package().repo(),
                namespace: source.namespace(),
                requested_path: source.requested_path(),
            })
            .collect::<Vec<_>>();
        self.retain_source_generation_inputs(token, selected_requests, selected_results, &sources)
    }

    fn retain_source_generation_inputs(
        &self,
        token: RepositorySessionToken,
        selected_requests: &[Arc<RepositoryMaterializationRequest>],
        selected_results: &RepositoryMaterializationResultEpoch,
        sources: &[SourceGenerationInput<'_>],
    ) -> Result<NativeSourceGenerations, Arc<str>> {
        let state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_validated_active(&state, token).map_err(|error| {
            Arc::<str>::from(format!("retaining source generations: {error:?}"))
        })?;
        let mut selected_repos = SmallSet::new();
        let mut entries = Vec::with_capacity(selected_requests.len());
        for request in selected_requests {
            if !selected_repos.insert(&request.id.canonical_repo) {
                return Err("duplicate selected repository for native source generation".into());
            }
            let entry = active
                .entries
                .iter()
                .find(|entry| entry.request.id == request.id)
                .ok_or("native source generation repository is absent from the active session")?;
            if entry.request != *request {
                return Err(
                    "native source generation selected request differs from active request".into(),
                );
            }
            entries.push(entry.clone());
        }
        let exact_results = complete_epoch(&self.workspace, &entries).map_err(|error| {
            Arc::<str>::from(format!("retaining source generations: {error:?}"))
        })?;
        if &exact_results != selected_results {
            return Err(
                "native source generation selected result epoch differs from active results".into(),
            );
        }

        let mut seen = SmallSet::new();
        let mut roots = Vec::new();
        for source in sources {
            let PathObservationNamespace::Materialization(instance) = source.namespace else {
                continue;
            };
            let entry = entries
                .iter()
                .find(|entry| &entry.request.id.canonical_repo == source.repo)
                .ok_or("materialized source repository is not selected")?;
            let RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    observation_instance,
                    generation_root,
                    ..
                },
            ) = &entry.result
            else {
                return Err("materialized source lacks a selected immutable success".into());
            };
            if instance.value() == 0 || instance != *observation_instance {
                return Err("materialized source instance differs from selected generation".into());
            }
            let owned = state
                .accepted_roots
                .iter()
                .chain(&active.provisional_roots)
                .find(|owned| owned.observation_instance == instance)
                .ok_or("materialized source has no native generation owner")?;
            if owned.root.path() != generation_root {
                return Err("materialized source generation root differs from native owner".into());
            }
            if !source
                .requested_path
                .as_path()
                .starts_with(owned.root.path())
            {
                return Err(
                    "materialized source requested path is outside its native generation".into(),
                );
            }
            // Validate each source path even when multiple sources share a lease.
            if seen.insert(instance) {
                roots.push(NativeSourceGeneration {
                    _instance: instance,
                    _root: owned.root.clone(),
                });
            }
        }
        Ok(NativeSourceGenerations {
            _roots: roots.into(),
        })
    }
}

#[cfg(test)]
mod tests;
