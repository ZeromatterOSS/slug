//! Tracked root repository mapping for ordinary Build-context `.bzl` modules.
use super::*;

fn failure(
    message: impl fmt::Display,
    observations: &PathObservationEpoch,
) -> HostBzlModuleDriverOutcome {
    host_bzl_complete(
        Err(HostBzlModuleError::Evaluation(LoadingError::new(format!(
            "root .bzl repository mapping: {message}"
        )))),
        observations.dupe(),
    )
}

pub(super) async fn load(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: HostBzlModuleMode,
    observations: &mut PathObservationEpoch,
) -> Result<Arc<[(ApparentRepoName, CanonicalRepoName)]>, HostBzlModuleDriverOutcome> {
    let result = match mode {
        HostBzlModuleMode::Legacy => match ctx
            .compute(&HostRootRepositoryMappingKey::new(workspace.dupe()))
            .await
        {
            Err(error) => return Err(failure(error, observations)),
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(result)) => result,
        },
        HostBzlModuleMode::Observed => match ctx
            .compute(&HostRootRepositoryMappingObservationKey::new(
                workspace.dupe(),
            ))
            .await
        {
            Err(error) => return Err(failure(error, observations)),
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(match error.selected_frontier() {
                    HostSelectedObservationFrontier::Path(error) => {
                        SourcePreparationOutcome::Complete(Err(error))
                    }
                    HostSelectedObservationFrontier::Infrastructure(message) => {
                        failure(message, observations)
                    }
                });
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                *observations = union_host_observations(observations, observed.observations())
                    .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
                observed.result().dupe()
            }
        },
    };
    let mapping = result
        .as_ref()
        .as_ref()
        .map_err(|error| failure(error, observations))?;
    let view = mapping
        .view()
        .ok_or_else(|| failure("selected mapping has no root view", observations))?;
    if !view.canonical_repo().is_root() || !view.mapping_context().is_root() {
        return Err(failure(
            "selected mapping has a nonroot context",
            observations,
        ));
    }
    // The view exposes borrowed entries, not a reusable Arc slice. Bound the
    // complete copy before cloning any names or allocating its output array.
    // Include both the Vec scratch and final Arc slots in the peak forecast.
    let mut count = 0usize;
    let mut visits = 1usize;
    let mut bytes = std::mem::size_of::<Arc<[(ApparentRepoName, CanonicalRepoName)]>>()
        + std::mem::size_of::<Vec<(ApparentRepoName, CanonicalRepoName)>>();
    for (apparent, canonical) in view.mapping() {
        count = count.saturating_add(1);
        visits = visits.saturating_add(3);
        bytes = bytes
            .saturating_add(2 * std::mem::size_of::<(ApparentRepoName, CanonicalRepoName)>())
            .saturating_add(apparent.as_str().len())
            .saturating_add(canonical.as_str().len());
        if visits > 65536 || count > 65536 || bytes > 64 * 1024 * 1024 {
            return Err(failure(
                format!("mapping copy shape limit: entries={count} visits={visits} bytes={bytes}"),
                observations,
            ));
        }
    }
    let mut entries = Vec::with_capacity(count);
    entries.extend(
        view.mapping()
            .map(|(apparent, canonical)| (apparent.clone(), canonical.clone())),
    );
    Ok(entries.into())
}
