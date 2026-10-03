//! Ordered normal issued Label data, privately retained in the existing command.
use slug_loading_v2::ObservedSelectedDataLabelRequests;
use slug_loading_v2::ObservedSelectedDataLabelSources;
use slug_loading_v2::RuntimeSourceProfileKey;
use slug_loading_v2::SelectedDataLabelBinding;
use slug_loading_v2::SelectedDataLabelRequestsKey;
use slug_loading_v2::SelectedDataLabelSourcesKey;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ObservedInputImage;

use super::*;
use crate::runtime::input_image;

#[path = "selected_label_data/catalogue.rs"]
pub(super) mod catalogue;
#[path = "selected_label_data/prepare.rs"]
pub(super) mod prepare;
#[cfg(all(test, target_os = "linux"))]
#[path = "selected_label_data/tests.rs"]
pub(super) mod tests;

fn refusal(message: &'static str) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!(message))
}
fn image_error(error: slug_workspace_v2::ImageDescriptorBudgetError) -> NativeDemandSessionError {
    NativeDemandSessionError::InputImage(error.into())
}
pub(super) fn validate_requests(
    command: &NativeDemandCommand<'_>,
    key: &RuntimeSourceProfileKey,
    plan: &ObservedSelectedDataLabelRequests,
    reachable: &SelectedWorkspaceDemands,
    deadline: std::time::Instant,
) -> Result<(), NativeDemandSessionError> {
    selected_scratch_source::active(command, deadline)?;
    if command.inputs.request.guest_namespace != Some(plan.policy())
        || !plan.matches_key(&SelectedDataLabelRequestsKey::new(
            key.requests_key().clone(),
        ))
        || !plan.same_resources(&command.runtime.scratch_resources)
    {
        return Err(refusal("foreign data Label plan"));
    }
    runtime_sources::validate_plan(command, key, plan.runtime_requests(), reachable, deadline)?;
    let guest = slug_bzlmod_v2::GuestNamespaceTransaction::new(
        command.runtime.repository_materializer.workspace().dupe(),
        Some(plan.policy()),
        command.runtime.image_resources.descriptor_budget().dupe(),
    )
    .map_err(|_| refusal("data guest transaction unavailable"))?;
    let scratch = command
        .scratch_metadata
        .as_ref()
        .ok_or_else(|| refusal("data scratch selector unavailable"))?
        .transaction()?;
    if !plan.authorized_by(
        &guest,
        &runtime_sources::declaration_transaction(command)?,
        &scratch,
        &command.runtime.scratch_resources,
    ) {
        return Err(refusal("data current transaction mismatch"));
    }
    for row in plan.requests() {
        input_image_bridge::label_source::check_selection(command.runtime, row.selection())?;
        input_image_bridge::label_source::check_reachable(command, row.selection(), reachable)?;
        let leaf = row.leaf_image_key();
        let selected = row.selection();
        if !leaf.is_regular_file_root()
            || leaf.namespace() != selected.image_key().namespace()
            || leaf.anchor_root() != selected.image_key().anchor_root()
            || leaf.source_root().as_path() != selected.selected_file().as_path()
            || leaf.policy() != selected.image_key().policy()
        {
            return Err(refusal("foreign data Label leaf selector"));
        }
    }
    Ok(())
}
pub(super) fn validate_sources(
    command: &NativeDemandCommand<'_>,
    key: &RuntimeSourceProfileKey,
    plan: &ObservedSelectedDataLabelRequests,
    data: &ObservedSelectedDataLabelSources,
    reachable: &SelectedWorkspaceDemands,
    deadline: std::time::Instant,
) -> Result<(), NativeDemandSessionError> {
    validate_requests(command, key, plan, reachable, deadline)?;
    if data.requests() != plan
        || !data.same_resources(&command.runtime.scratch_resources)
        || !data.matches_key(&SelectedDataLabelSourcesKey::new(key.clone()))
        || data.sources().len() != plan.requests().len()
    {
        return Err(refusal("foreign data Label sources"));
    }
    for (index, (request, source)) in plan.requests().zip(data.sources()).enumerate() {
        if request.occurrence_index() != index || source.occurrence_index() != index {
            return Err(refusal("data Label occurrence changed"));
        }
        if let SelectedDataLabelBinding::Leaf { image } = source.binding() {
            catalogue::check_image(request.leaf_image_key(), image)?;
        }
    }
    Ok(())
}

pub(super) async fn plan(
    runtime: &WorkspaceRuntime,
    guard: &mut NativeDemandAbortGuard<'_>,
    key: &RuntimeSourceProfileKey,
    deadline: std::time::Instant,
    owner: Option<&super::source_preparation::OwnerAuthorization<'_>>,
) -> Result<
    selected_tool_source::attempt::Attempt<ObservedSelectedDataLabelRequests>,
    NativeDemandSessionError,
> {
    let mut data = guard.attempt_user_computation_data()?;
    data.data
        .set(slug_loading_v2::SelectedScratchPreparationDeadline::new(
            deadline,
        ));
    let mut updater = runtime.dice.updater_with_data(data);
    guard.inject_attempt(&mut updater)?;
    let mut tx = runtime.request_revision.commit(updater).await;
    if let Some(need) = super::source_preparation::attempt::authorize(&mut tx, owner).await? {
        guard.seal_retry()?;
        return Ok(selected_tool_source::attempt::Attempt::Need(need));
    }
    let plan = match slug_loading_v2::compute_selected_data_label_requests(
        &mut tx,
        &SelectedDataLabelRequestsKey::new(key.requests_key().clone()),
    )
    .await
    .map_err(|e| NativeDemandSessionError::Computation(anyhow::Error::new(e)))?
    {
        slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => {
            guard.seal_retry()?;
            return Ok(selected_tool_source::attempt::Attempt::Need(need));
        }
        slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) => {
            value.map_err(|e| NativeDemandSessionError::Computation(anyhow::Error::new(e)))?
        }
    };
    let selected = guard.seal_terminal()?.select(&tx).await?;
    Ok(selected_tool_source::attempt::Attempt::Complete(
        plan, selected,
    ))
}
