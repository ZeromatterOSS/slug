//! Normal owner roots share each phase transaction and its activation closure.
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_loading_v2::SelectedScratchPreparationDeadline;

use super::*;
use crate::runtime::input_image;

pub(in crate::runtime::dice) async fn owner(
    runtime: &WorkspaceRuntime,
    guard: &mut NativeDemandAbortGuard<'_>,
    key: &SelectedPreparationOwnerKey,
    deadline: std::time::Instant,
) -> Result<
    selected_tool_source::attempt::Attempt<ObservedSelectedPreparationOwner>,
    NativeDemandSessionError,
> {
    let mut data = guard.attempt_user_computation_data()?;
    data.data
        .set(SelectedScratchPreparationDeadline::new(deadline));
    let mut updater = runtime.dice.updater_with_data(data);
    guard.inject_attempt(&mut updater)?;
    let mut tx = runtime.request_revision.commit(updater).await;
    let observed = match slug_loading_v2::compute_selected_preparation_owner(&mut tx, key)
        .await
        .map_err(|e| NativeDemandSessionError::Computation(anyhow::Error::new(e)))?
    {
        SourcePreparationOutcome::Need(need) => {
            guard.seal_retry()?;
            return Ok(selected_tool_source::attempt::Attempt::Need(need));
        }
        SourcePreparationOutcome::Complete(value) => {
            value.map_err(|e| NativeDemandSessionError::Computation(anyhow::Error::new(e)))?
        }
    };
    let selected = guard.seal_terminal()?.select(&tx).await?;
    Ok(selected_tool_source::attempt::Attempt::Complete(
        observed, selected,
    ))
}

pub(in crate::runtime::dice) async fn authorize(
    tx: &mut dice::DiceTransaction,
    owner: Option<&OwnerAuthorization<'_>>,
) -> Result<Option<slug_bzlmod_v2::SourcePreparationNeeds>, NativeDemandSessionError> {
    let Some(owner) = owner else { return Ok(None) };
    #[cfg(test)]
    {
        // Read the real raw input producers in each lower/final transaction;
        // the returned wrapper pin alone would not prove injection fidelity.
        let workspace = owner.key.workspace().as_path().to_path_buf();
        let expected = owner.expected_inputs;
        assert_eq!(
            tx.compute(&RootModuleCommandPolicyKey {
                workspace: workspace.clone()
            })
            .await
            .unwrap(),
            slug_bzlmod_v2::RootModuleCommandPolicy::from(expected.command_policy.clone())
        );
        assert_eq!(
            tx.compute(&RootModuleEnvironmentPolicyKey {
                workspace: workspace.clone()
            })
            .await
            .unwrap(),
            slug_bzlmod_v2::RootModuleEnvironmentPolicy::from(expected.environment_policy.clone())
        );
        assert_eq!(
            tx.compute(&RootModuleLockfileModeKey {
                workspace: workspace.clone()
            })
            .await
            .unwrap(),
            slug_bzlmod_v2::RootModuleLockfileMode::from(expected.lockfile_mode.clone())
        );
        assert_eq!(
            tx.compute(&RootModuleRegistryUrlsKey { workspace })
                .await
                .unwrap(),
            slug_bzlmod_v2::RootModuleRegistryUrls::from(expected.registry_urls.dupe())
        );
    }
    match slug_loading_v2::compute_selected_preparation_owner(tx, &owner.key)
        .await
        .map_err(|e| NativeDemandSessionError::Computation(anyhow::Error::new(e)))?
    {
        SourcePreparationOutcome::Need(need) => Ok(Some(need)),
        SourcePreparationOutcome::Complete(value) => {
            let value =
                value.map_err(|e| NativeDemandSessionError::Computation(anyhow::Error::new(e)))?;
            if value != owner.observed {
                return Err(refusal("selected source owner changed"));
            }
            Ok(None)
        }
    }
}

pub(in crate::runtime::dice) fn verify(
    command: &NativeDemandCommand<'_>,
    owner: Option<&OwnerAuthorization<'_>>,
    reachable: &SelectedWorkspaceDemands,
    deadline: std::time::Instant,
) -> Result<(), NativeDemandSessionError> {
    let Some(owner) = owner else { return Ok(()) };
    selected_scratch_source::active(command, deadline)?;
    if !owner.observed.matches_key(&owner.key)
        || !owner
            .observed
            .same_resources(&command.runtime.scratch_resources)
    {
        return Err(refusal("foreign selected source owner"));
    }
    for (path, value) in owner.observed.observations().observations() {
        let used = reachable.unscoped_paths().contains(path)
            || reachable.repository_validations().iter().any(|v| {
                v.paths().contains(path) && reachable.repository_requests().contains(v.request())
            });
        if !used || command.path_observations.get(path) != Some(value) {
            return Err(refusal("owner frontier lacks current reachable proof"));
        }
    }
    let _credit = selected_tool_source::frontier_credit(command, owner.observed.observations())?;
    let fresh = input_image_bridge::observe_command_paths(
        command,
        owner
            .observed
            .observations()
            .observations()
            .keys()
            .cloned()
            .collect(),
    )?;
    if &fresh != owner.observed.observations() {
        return Err(NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::StaleSource,
        ));
    }
    selected_scratch_source::active(command, deadline)
}
