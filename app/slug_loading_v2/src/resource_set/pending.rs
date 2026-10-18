//! Opaque tokens into the existing four-slot, GC-traced pending Args collector.
use std::mem::size_of;
use std::sync::Arc;

use allocative::Allocative;
use starlark::environment::Module;
use starlark::values::Value;

use crate::subrule_invocation::pending_args;

mod registry;
use registry::LOOKUP_WORK;
use registry::Phase;
pub(crate) use registry::ResourceRegistry;
use registry::TRANSFER_WORK;

#[derive(Debug, Clone, Allocative)]
pub struct ResourceSetRegistration {
    owner: Arc<()>,
    index: usize,
}
/// Prepay eligibility inspection on the invocation's same cumulative ledger.
/// The lock ends here, before the caller inspects code or original bindings.
pub(super) fn prepay_validation(
    module: &Module,
    records: usize,
    bytes: usize,
) -> anyhow::Result<()> {
    let collector = pending_args::collector(module)?;
    collector
        .resources
        .pay(records, bytes)
        .map_err(anyhow::Error::msg)
}
/// The bound original is retained only after prepaid index/cookie/vector work.
pub fn register_resource_set<'v>(
    module: &'v Module,
    value: Value<'v>,
) -> anyhow::Result<ResourceSetRegistration> {
    let collector = pending_args::collector(module)?;
    let mut registry = collector
        .resources
        .registry
        .lock()
        .map_err(|_| anyhow::anyhow!("resource_set collector poisoned"))?;
    registry.prepare_push()?;
    let token = ResourceSetRegistration {
        owner: collector.owner.clone(),
        index: registry.values.len(),
    };
    registry.values.push(value);
    Ok(token)
}
/// Admit exactly once, before provider iteration/capture. The charge closure is
/// a synchronous native Publisher debit; it runs with no collector lock held.
pub fn transfer_resource_sets(
    module: &Module,
    mut charge: impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<(), String> {
    let control = size_of::<crate::bzl_module::semantic_evaluation::budget::Budget>()
        + 8 * size_of::<usize>();
    charge(TRANSFER_WORK, control)?;
    let Some(collector) = pending_args::existing_collector(module).map_err(|e| e.to_string())?
    else {
        return Ok(());
    };
    let (records, bytes) = {
        let mut state = collector
            .resources
            .registry
            .lock()
            .map_err(|_| "resource_set collector poisoned")?;
        state.pay(TRANSFER_WORK, control)?;
        // Transferring is closed to every registration/read/clear. Any reentry
        // poisons it; no fresh allowance or successful action can escape.
        state.phase = Phase::Transferring;
        (state.work.records, state.work.bytes)
    };
    let admitted = charge(records, bytes);
    let mut state = collector
        .resources
        .registry
        .lock()
        .map_err(|_| "resource_set collector poisoned")?;
    if let Err(error) = admitted {
        state.phase = Phase::Poisoned;
        return Err(error);
    };
    if !matches!(state.phase, Phase::Transferring) {
        state.phase = Phase::Poisoned;
        return Err("resource_set collector changed during transfer".into());
    }
    state.phase = Phase::Transferred;
    Ok(())
}
/// Every post-transfer read spends only the SAME Publisher ledger, never the
/// already transferred Registry Budget. Release the lock before graph work.
pub fn resource_set_value<'v>(
    module: &'v Module,
    token: &ResourceSetRegistration,
    mut charge: impl FnMut(usize, usize) -> Result<(), String>,
) -> anyhow::Result<Value<'v>> {
    charge(
        LOOKUP_WORK,
        size_of::<Value>() + size_of::<ResourceSetRegistration>(),
    )
    .map_err(anyhow::Error::msg)?;
    let collector = pending_args::existing_collector(module)?
        .ok_or_else(|| anyhow::anyhow!("resource_set collector unavailable"))?;
    anyhow::ensure!(
        Arc::ptr_eq(&collector.owner, &token.owner),
        "resource_set registration belongs to another module"
    );
    let registry = collector
        .resources
        .registry
        .lock()
        .map_err(|_| anyhow::anyhow!("resource_set collector poisoned"))?;
    anyhow::ensure!(
        matches!(registry.phase, Phase::Transferred),
        "resource_set publication ledger is not transferred"
    );
    registry
        .values
        .get(token.index)
        .copied()
        .ok_or_else(|| anyhow::anyhow!("unknown resource_set registration"))
}

#[cfg(test)]
mod tests;
