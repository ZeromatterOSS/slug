//! Namespace is computed normally in the same final source transaction.
use slug_bzlmod_v2::SourcePreparationOutcome;

use super::*;
pub(in crate::runtime::dice) async fn sources(
    runtime: &WorkspaceRuntime,
    guard: &mut NativeDemandAbortGuard<'_>,
    key: &SelectedGuestNamespaceFactsKey,
    expected: &slug_loading_v2::ObservedRuntimeSourceRequests,
    catalog: &mut selected_tool_source::declarations::Catalog,
    deadline: std::time::Instant,
) -> Result<
    selected_tool_source::attempt::Attempt<ObservedGuestNamespaceFacts>,
    NativeDemandSessionError,
> {
    let mut tx = match runtime_sources::attempt::source_transaction(
        runtime,
        guard,
        key.profile_key(),
        expected,
        catalog,
        deadline,
    )
    .await?
    {
        Ok(tx) => tx,
        Err(need) => return Ok(selected_tool_source::attempt::Attempt::Need(need)),
    };
    let facts = match slug_loading_v2::compute_selected_guest_namespace_facts(&mut tx, key)
        .await
        .map_err(|e| NativeDemandSessionError::Computation(anyhow::Error::new(e)))?
    {
        SourcePreparationOutcome::Need(need) => {
            guard.seal_retry()?;
            return Ok(selected_tool_source::attempt::Attempt::Need(need));
        }
        SourcePreparationOutcome::Complete(facts) => {
            facts.map_err(|e| NativeDemandSessionError::Computation(anyhow::Error::new(e)))?
        }
    };
    let selected = guard.seal_terminal()?.select(&tx).await?;
    Ok(selected_tool_source::attempt::Attempt::Complete(
        facts, selected,
    ))
}
