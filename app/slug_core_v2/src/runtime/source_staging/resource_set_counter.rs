//! Closed feature-only instrumentation of the already selected original callback.
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use super::PreparedSourceActionInputs;

impl PreparedSourceActionInputs {
    /// Clone this valid prepared selection and count only explicit numeric calls.
    /// No replacement callable, runtime, action, source or certificate is accepted.
    pub fn with_resource_set_invocation_counter(&self) -> Result<(Self, Arc<AtomicUsize>), String> {
        let original = self
            .evaluation
            .as_ref()
            .as_ref()
            .map_err(|error| error.to_string())?;
        let selected = BTreeSet::from([(self.owner, self.action)]);
        let calls = Arc::new(AtomicUsize::new(0));
        let evaluation =
            super::super::resource_set_counter_support::instrument(original, &selected, &calls)?;
        let mut result = self.clone();
        result.evaluation = Arc::new(Ok(evaluation));
        if result != *self {
            return Err("counter changed prepared source inputs".into());
        }
        Ok((result, calls))
    }
}
