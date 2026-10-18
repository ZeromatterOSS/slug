//! Closed feature-only instrumentation of callbacks in this existing plan.
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use super::PreparedActionChainInputs;

impl PreparedActionChainInputs {
    /// Clone this valid plan, preserving its original selection and certificates.
    /// All coordinates come from this plan; no caller injection is accepted.
    pub fn with_resource_set_invocation_counter(&self) -> Result<(Self, Arc<AtomicUsize>), String> {
        let original = self.evaluation().map_err(|error| error.to_string())?;
        let plan = self.plan().map_err(|error| error.to_string())?;
        let mut selected = BTreeSet::new();
        for step in plan.actions() {
            let action = step.action();
            if !action
                .spawn_spec()
                .is_some_and(|spawn| spawn.resource_set().is_some())
            {
                continue;
            }
            let mut coordinate = None;
            for (owner_index, owner) in original.action_closure.owners().iter().enumerate() {
                for (action_index, candidate) in owner.actions().iter().enumerate() {
                    if std::ptr::eq(candidate, action) {
                        if coordinate.replace((owner_index, action_index)).is_some() {
                            return Err(
                                "counter action has multiple actual closure coordinates".into()
                            );
                        }
                    }
                }
            }
            selected.insert(coordinate.ok_or("counter plan action absent from original closure")?);
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let evaluation =
            super::super::resource_set_counter_support::instrument(original, &selected, &calls)?;
        let mut result = self.clone();
        result.evaluation = Arc::new(Ok(evaluation));
        if result != *self {
            return Err("counter changed prepared chain inputs".into());
        }
        Ok((result, calls))
    }
}
