//! Fresh native attempts invoke guarded normal producers; no completed injection.
use selected_tool_source::attempt::Attempt;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_loading_v2::SelectedScratchPreparationDeadline;

use super::*;

fn complete<T>(
    outcome: SourcePreparationOutcome<Result<T, slug_loading_v2::RuntimeSourcesError>>,
) -> Result<Result<T, SourcePreparationNeeds>, NativeDemandSessionError> {
    match outcome {
        SourcePreparationOutcome::Need(need) => Ok(Err(need)),
        SourcePreparationOutcome::Complete(value) => value
            .map(Ok)
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::Error::new(error))),
    }
}
pub(in crate::runtime::dice) async fn plan(
    runtime: &WorkspaceRuntime,
    guard: &mut NativeDemandAbortGuard<'_>,
    key: &RuntimeSourceProfileKey,
    deadline: std::time::Instant,
) -> Result<Attempt<ObservedRuntimeSourceRequests>, NativeDemandSessionError> {
    let mut data = guard.attempt_user_computation_data()?;
    data.data
        .set(SelectedScratchPreparationDeadline::new(deadline));
    let mut updater = runtime.dice.updater_with_data(data);
    guard.inject_attempt(&mut updater)?;
    let mut transaction = runtime.request_revision.commit(updater).await;
    let observed = match complete(
        slug_loading_v2::compute_runtime_source_requests(&mut transaction, key.requests_key())
            .await
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::Error::new(error)))?,
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
    key: &RuntimeSourceProfileKey,
    expected: &ObservedRuntimeSourceRequests,
    catalog: &mut selected_tool_source::declarations::Catalog,
    deadline: std::time::Instant,
) -> Result<Attempt<ObservedRuntimeSourceProfile>, NativeDemandSessionError> {
    let mut transaction =
        match source_transaction(runtime, guard, key, expected, catalog, deadline).await? {
            Ok(transaction) => transaction,
            Err(need) => return Ok(Attempt::Need(need)),
        };
    let profile = match complete(
        slug_loading_v2::compute_runtime_source_profile(&mut transaction, key)
            .await
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::Error::new(error)))?,
    )? {
        Err(need) => {
            guard.seal_retry()?;
            return Ok(Attempt::Need(need));
        }
        Ok(value) => value,
    };
    let selected = guard.seal_terminal()?.select(&transaction).await?;
    Ok(Attempt::Complete(profile, selected))
}

pub(in crate::runtime::dice) async fn source_transaction(
    runtime: &WorkspaceRuntime,
    guard: &mut NativeDemandAbortGuard<'_>,
    key: &RuntimeSourceProfileKey,
    expected: &ObservedRuntimeSourceRequests,
    catalog: &mut selected_tool_source::declarations::Catalog,
    deadline: std::time::Instant,
) -> Result<Result<dice::DiceTransaction, SourcePreparationNeeds>, NativeDemandSessionError> {
    let mut data = guard.attempt_user_computation_data()?;
    data.data
        .set(SelectedScratchPreparationDeadline::new(deadline));
    let mut updater = runtime.dice.updater_with_data(data);
    guard.inject_attempt(&mut updater)?;
    let mut transaction = runtime.request_revision.commit(updater).await;
    let current = match complete(
        slug_loading_v2::compute_runtime_source_requests(&mut transaction, key.requests_key())
            .await
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::Error::new(error)))?,
    )? {
        Err(need) => {
            guard.seal_retry()?;
            return Ok(Err(need));
        }
        Ok(value) => value,
    };
    if &current != expected {
        return Err(refusal("runtime requests changed within attempt"));
    }
    if let Some(need) = selected_tool_source::attempt::prepare_tool_images(
        runtime,
        &mut transaction,
        &key.tool_sources_key(),
        current.tool_requests(),
        catalog,
    )
    .await?
    {
        guard.seal_retry()?;
        return Ok(Err(need));
    }
    Ok(Ok(transaction))
}
