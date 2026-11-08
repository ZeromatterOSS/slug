//! Constructor-scoped prior actual inputs, never consumer or global state.
use std::mem::size_of;
use std::path::Path;
use std::sync::Arc;

use dice::DiceTransaction;
use dice::DiceTransactionUpdater;
use dice_error::DiceError;
use dice_error::DiceErrorImpl;
use dice_error::result::CancellationReason;
use slug_bzlmod_v2::RepositoryMaterialization;
use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationResult;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::RepositoryMaterializationSuccess;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::source_preparation::compute_repository_materialization_result;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationNamespace;

use super::Budget;
#[path = "stable_inputs/initial_inputs.rs"]
mod initial_inputs;
#[path = "stable_inputs/preflight.rs"]
mod preflight;
pub(crate) use initial_inputs::FixtureObservations;
pub(super) use initial_inputs::admit_inventory;
pub(super) use preflight::request as admit_request;

pub(crate) struct SavedInputs {
    pub(super) prior: Option<PriorInputs>,
    pub(super) budget: Budget,
    pub(super) floor: u64,
    pub(super) explicit: Vec<slug_identity_v2::CanonicalRepoName>,
}
pub(super) struct PriorInputs {
    transaction: DiceTransaction,
    observations: PathObservationEpoch,
    materializations: RepositoryMaterializationResultEpoch,
}
pub(super) fn admit_transfer(
    observations: &PathObservationEpoch,
    entries: &[RepositoryMaterializationEpochEntry],
    budget: &mut Budget,
) -> Result<(), String> {
    preflight::transfer(observations, entries, budget)
}
pub(super) type Reused = (
    RepositoryMaterializationEpochEntry,
    Vec<(
        slug_workspace_v2::PathObservationDemand,
        Arc<slug_workspace_v2::PathObservationResult>,
    )>,
);

pub(super) fn injected<T>(result: Result<T, DiceError>) -> Result<Option<T>, String> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error)
            if matches!(
                error.0.as_ref(),
                DiceErrorImpl::Cancelled(CancellationReason::Rejected)
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error.to_string()),
    }
}
impl SavedInputs {
    pub(super) fn empty() -> Self {
        Self {
            prior: None,
            budget: Budget::default(),
            floor: 0,
            explicit: Vec::new(),
        }
    }
}
/// Called before the caller writes ANY current input to its updater.
pub(crate) async fn capture(
    updater: &DiceTransactionUpdater,
    workspace: &Path,
) -> Result<SavedInputs, String> {
    let mut saved = SavedInputs::empty();
    saved.budget.charge(
        2,
        size_of::<SavedInputs>()
            .checked_add(size_of::<PriorInputs>())
            .and_then(|n| n.checked_add(workspace.as_os_str().len().checked_mul(4)?))
            .ok_or("saved fixture input header overflow")?,
    )?;
    let root = NormalizedAbsolutePath::new(workspace.to_path_buf()).map_err(|e| e.to_string())?;
    let mut transaction = updater.existing_state().await;
    let observations = injected(transaction.compute(&PathObservationEpochKey).await)?;
    let materializations = injected(
        transaction
            .compute(&RepositoryMaterializationResultEpochKey {
                workspace: root.clone(),
            })
            .await,
    )?;
    if let Some(materializations) = &materializations {
        saved.floor = retained_floor(materializations, &root, &mut saved.budget)?;
    }
    if let Some(observations) = &observations {
        for (demand, _) in observations.observations() {
            saved.budget.charge(1, 0)?;
            if let PathObservationNamespace::Materialization(instance) = demand.namespace() {
                saved.floor = saved.floor.max(instance.value());
            }
        }
    }
    if let (Some(observations), Some(materializations)) = (observations, materializations) {
        saved.prior = Some(PriorInputs {
            transaction,
            observations,
            materializations,
        });
    }
    Ok(saved)
}

/// Retired immutable namespaces still reserve their actual instance IDs.
pub(super) fn retained_floor(
    epoch: &RepositoryMaterializationResultEpoch,
    root: &NormalizedAbsolutePath,
    budget: &mut Budget,
) -> Result<u64, String> {
    let retained = epoch
        .borrowed_retained_generated(root, 65_536)
        .map_err(|e| format!("saved fixture inventory floor: {e:?}"))?;
    budget.charge(retained.len(), 0)?;
    Ok(retained
        .iter()
        .filter_map(|entry| match &entry.result {
            RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    observation_instance,
                    ..
                },
            ) => Some(observation_instance.value()),
            _ => None,
        })
        .max()
        .unwrap_or(0))
}

impl PriorInputs {
    // The caller first obtained this request from CURRENT registration sources.
    // The real materialization owner then checks full request Eq at prior state.
    pub(super) async fn reuse(
        &mut self,
        request: &Arc<RepositoryMaterializationRequest>,
        current: &PathObservationEpoch,
        budget: &mut Budget,
    ) -> Result<Option<Reused>, String> {
        preflight::request(request, budget)?;
        // Inspect only borrowed prior namespace owners before the real lookup
        // so its returned generation-root copy is prepaid at its actual width.
        let bound = self.observations.observations().len();
        let mut last = None;
        let mut candidate = None;
        for (demand, _) in self.observations.observations() {
            budget.charge(1, 0)?;
            let PathObservationNamespace::Materialization(instance) = demand.namespace() else {
                continue;
            };
            if last == Some(instance) {
                continue;
            }
            last = Some(instance);
            budget.charge(bound, 0)?;
            let view = self
                .materializations
                .borrowed_instance_for_namespace(&request.id.workspace, instance, bound)
                .map_err(|e| format!("saved fixture namespace: {e:?}"))?;
            let Some(view) = view else { continue };
            if view.request().id == request.id {
                budget.charge(
                    1,
                    view.generation_root()
                        .as_os_str()
                        .len()
                        .checked_mul(4)
                        .and_then(|n| n.checked_add(size_of::<RepositoryMaterialization>() * 4))
                        .ok_or("saved fixture result forecast overflow")?,
                )?;
                candidate = Some(instance);
                break;
            }
        }
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        let result =
            compute_repository_materialization_result(&mut self.transaction, request.clone())
                .await
                .map_err(|e| e.to_string())?;
        let SourcePreparationOutcome::Complete(result) = result else {
            return Ok(None);
        };
        let Ok(RepositoryMaterialization::Immutable {
            canonical_repo,
            source_identity,
            generation_root,
            observation_instance,
            ..
        }) = result.as_ref()
        else {
            return Ok(None);
        };
        if canonical_repo != &request.id.canonical_repo {
            return Err("saved fixture canonical mismatch".into());
        }
        if *observation_instance != candidate {
            return Err("saved fixture materialization changed namespace".into());
        }
        // The original namespace owner confirms the exact full request/root.
        budget.charge(bound, 0)?;
        let view = self
            .materializations
            .borrowed_instance_for_namespace(&request.id.workspace, *observation_instance, bound)
            .map_err(|e| format!("saved fixture namespace: {e:?}"))?;
        let Some(view) = view else { return Ok(None) };
        // The second full Eq is covered by the same preflight's complete bound.
        if view.request() != request.as_ref() || view.generation_root() != generation_root {
            return Err("saved fixture namespace/request mismatch".into());
        }
        preflight::observations(&self.observations, current, *observation_instance, budget)?;
        budget.charge(
            1,
            generation_root
                .as_os_str()
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(size_of::<RepositoryMaterializationEpochEntry>() * 4))
                .ok_or("saved fixture result storage overflow")?,
        )?;
        let observations = self
            .observations
            .observations()
            .iter()
            .filter(|(demand, _)| {
                demand.namespace()
                    == PathObservationNamespace::Materialization(*observation_instance)
                    && current.get(demand).is_none()
            })
            .map(|(demand, value)| (demand.clone(), value.clone()))
            .collect();
        Ok(Some((
            RepositoryMaterializationEpochEntry {
                request: request.clone(),
                result: RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Immutable {
                        path_policy: slug_bzlmod_v2::RepositoryMaterializationPathPolicy::LegacyUnbounded,
                        source_identity: source_identity.clone(),
                        generation_root: generation_root.clone(),
                        observation_instance: *observation_instance,
                    },
                ),
            },
            observations,
        )))
    }
}

#[cfg(test)]
#[path = "stable_inputs/tests.rs"]
mod tests;
