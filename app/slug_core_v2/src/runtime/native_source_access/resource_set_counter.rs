//! Closed counter instrumentation preserves this exact native selection/access.
use std::sync::atomic::AtomicUsize;

use super::*;

impl NativeSourceActionInputs {
    pub fn with_resource_set_invocation_counter(&self) -> Result<(Self, Arc<AtomicUsize>), String> {
        let (prepared, counter) = self.prepared.with_resource_set_invocation_counter()?;
        Ok((
            Self {
                prepared: Arc::new(prepared),
                access: self.access.clone(),
            },
            counter,
        ))
    }
}
impl NativeActionChainInputs {
    pub fn with_resource_set_invocation_counter(&self) -> Result<(Self, Arc<AtomicUsize>), String> {
        let (prepared, counter) = self.prepared.with_resource_set_invocation_counter()?;
        Ok((
            Self {
                prepared: Arc::new(prepared),
                access: self.access.clone(),
            },
            counter,
        ))
    }
}
