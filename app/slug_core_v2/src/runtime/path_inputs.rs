//! Command-scoped revocation ledger for exact raw path inputs.
//!
//! This retains demand identities only. DICE owns each observed value; it no
//! longer retains a copy of every accumulated map for every demand retry.

use std::sync::Mutex;

use dice::DiceTransactionUpdater;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use starlark_map::small_set::SmallSet;

pub(super) struct NativePathInputLedger {
    staged: Mutex<SmallSet<PathObservationDemand>>,
}

impl NativePathInputLedger {
    pub(super) fn new(prior: &PathObservationEpoch) -> Self {
        Self {
            staged: Mutex::new(prior.observations().keys().cloned().collect()),
        }
    }

    pub(super) fn inject(
        &self,
        updater: &mut DiceTransactionUpdater,
        desired: &PathObservationEpoch,
    ) -> anyhow::Result<()> {
        // Keep a conservative union even if staging fails or a prepared
        // updater is discarded. Acceptance and abort must revoke every input
        // that could have been published by this command. No guard survives
        // this synchronous staging call; commit and compute run unlocked.
        let mut staged = self.staged.lock().expect("path input ledger poisoned");
        for demand in desired.observations().keys() {
            staged.insert(demand.clone());
        }
        slug_workspace_v2::inject_path_observation_inputs(
            updater,
            desired,
            staged.iter().cloned(),
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "path_inputs/tests.rs"]
mod tests;
