//! Fresh trackers and explicit terminal transitions share one native command.
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_loading_v2::SelectedScratchPreparationDeadline;
use slug_workspace_v2::PathOutcome;

use super::*;

pub(in crate::runtime::dice) enum Attempt<T> {
    Need(slug_bzlmod_v2::SourcePreparationNeeds),
    Complete(T, NativeDemandTerminalSelection),
}
fn complete<T>(
    outcome: SourcePreparationOutcome<Result<T, slug_loading_v2::SelectedToolSourcesError>>,
) -> Result<Result<T, slug_bzlmod_v2::SourcePreparationNeeds>, NativeDemandSessionError> {
    match outcome {
        SourcePreparationOutcome::Need(need) => Ok(Err(need)),
        SourcePreparationOutcome::Complete(value) => value
            .map(Ok)
            .map_err(|_| refusal("normal tool source producer refused")),
    }
}
pub(in crate::runtime::dice) async fn plan(
    runtime: &WorkspaceRuntime,
    guard: &mut NativeDemandAbortGuard<'_>,
    key: &SelectedExtensionToolSourcesKey,
    deadline: Instant,
) -> Result<Attempt<ObservedSelectedExtensionToolSourceRequests>, NativeDemandSessionError> {
    let mut data = guard.attempt_user_computation_data()?;
    data.data
        .set(SelectedScratchPreparationDeadline::new(deadline));
    let mut updater = runtime.dice.updater_with_data(data);
    guard.inject_attempt(&mut updater)?;
    let mut transaction = runtime.request_revision.commit(updater).await;
    let observed = match complete(
        slug_loading_v2::compute_selected_tool_source_requests(
            &mut transaction,
            key.requests_key(),
        )
        .await
        .map_err(|_| refusal("normal tool request computation failed"))?,
    )? {
        Err(need) => {
            guard.seal_retry()?;
            return Ok(Attempt::Need(need));
        }
        Ok(value) => value,
    };
    let selected = guard.seal_terminal()?.select(&transaction).await?;
    Ok(Attempt::Complete(observed, selected))
}
pub(in crate::runtime::dice) async fn sources(
    runtime: &WorkspaceRuntime,
    guard: &mut NativeDemandAbortGuard<'_>,
    key: &SelectedExtensionToolSourcesKey,
    expected: &ObservedSelectedExtensionToolSourceRequests,
    catalog: &mut declarations::Catalog,
    deadline: Instant,
) -> Result<Attempt<ObservedSelectedExtensionToolSources>, NativeDemandSessionError> {
    let mut data = guard.attempt_user_computation_data()?;
    data.data
        .set(SelectedScratchPreparationDeadline::new(deadline));
    let mut updater = runtime.dice.updater_with_data(data);
    guard.inject_attempt(&mut updater)?;
    let mut transaction = runtime.request_revision.commit(updater).await;
    let current = match complete(
        slug_loading_v2::compute_selected_tool_source_requests(
            &mut transaction,
            key.requests_key(),
        )
        .await
        .map_err(|_| refusal("normal tool request computation failed"))?,
    )? {
        Err(need) => {
            guard.seal_retry()?;
            return Ok(Attempt::Need(need));
        }
        Ok(value) => value,
    };
    if &current != expected {
        return Err(refusal("tool request plan changed within attempt"));
    }
    if let Some(need) =
        prepare_tool_images(runtime, &mut transaction, key, &current, catalog).await?
    {
        guard.seal_retry()?;
        return Ok(Attempt::Need(need));
    }
    let certificate = match complete(
        slug_loading_v2::compute_selected_tool_sources(&mut transaction, key)
            .await
            .map_err(|_| refusal("normal tool source computation failed"))?,
    )? {
        Err(need) => {
            guard.seal_retry()?;
            return Ok(Attempt::Need(need));
        }
        Ok(value) => value,
    };
    let selected = guard.seal_terminal()?.select(&transaction).await?;
    Ok(Attempt::Complete(certificate, selected))
}

pub(in crate::runtime::dice) async fn prepare_tool_images(
    runtime: &WorkspaceRuntime,
    transaction: &mut dice::DiceTransaction,
    key: &SelectedExtensionToolSourcesKey,
    current: &ObservedSelectedExtensionToolSourceRequests,
    catalog: &mut declarations::Catalog,
) -> Result<Option<slug_bzlmod_v2::SourcePreparationNeeds>, NativeDemandSessionError> {
    // Declared selector scopes exist before any image Need can be emitted.
    for (index, request) in current.requests().enumerate() {
        let Some(selected) = request.label_selection() else {
            continue;
        };
        let image = match transaction
            .compute(selected.image_key())
            .await
            .map_err(|e| NativeDemandSessionError::Computation(e.into()))?
        {
            PathOutcome::Need(need) => {
                return Ok(Some(slug_bzlmod_v2::SourcePreparationNeeds::path(need)));
            }
            PathOutcome::Complete(value) => {
                value.map_err(|_| refusal("tool image producer refused"))?
            }
        };
        catalog.declare_elf(runtime, index, selected, image, key.policy().elf_policy())?;
    }
    Ok(None)
}
