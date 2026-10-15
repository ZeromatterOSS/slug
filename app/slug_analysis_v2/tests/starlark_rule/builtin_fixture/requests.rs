//! Complete input preparation for convenience fixture requests only.
//! Explicit raw/caller-owned transactions do not pass through this adapter.
use std::path::Path;
use std::sync::Arc;

use dice::ActivationTracker;
use dice::DiceTransaction;
use dice::UserComputationData;
use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
use slug_events_v2::CaptureEvaluationEvents;

use super::RepositoryHostInputs;

pub(in super::super) fn data(
    workspace: &Path,
    tracker: Option<Arc<dyn ActivationTracker>>,
    capture_events: bool,
) -> (UserComputationData, RepositoryHostInputs) {
    let mut data = UserComputationData {
        activation_tracker: tracker,
        cycle_detector: Some(super::super::analysis_cycle_detector()),
        ..Default::default()
    };
    if capture_events {
        data.data.set(CaptureEvaluationEvents);
    }
    let host = RepositoryHostInputs::prepare(workspace, &mut data);
    (data, host)
}

pub(in super::super) async fn ready(
    transaction: DiceTransaction,
    workspace: &Path,
    entries: Vec<RepositoryMaterializationEpochEntry>,
    tracker: Option<Arc<dyn ActivationTracker>>,
    capture_events: bool,
    saved: super::generated_registration::SavedInputs,
) -> Result<DiceTransaction, String> {
    // The caller supplied the exact snapshot/epoch/entries once. No filesystem
    // or root_epoch rescan, consumer query, retry, or tracker clearing occurs.
    let prepared =
        super::generated_registration::prepare_saved(transaction, workspace, entries, saved)
            .await?;
    // Only known convenience fields are constructed here, never opaque caller
    // userdata. Host defaults match the inputs injected by data() before prep.
    let (data, _) = data(workspace, tracker, capture_events);
    Ok(prepared.into_updater().commit_with_data(data).await)
}
