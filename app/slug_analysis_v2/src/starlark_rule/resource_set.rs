//! Actual resource callback publication, independent of command-line Args.
use std::alloc::Layout;
use std::mem::size_of;
use std::sync::Arc;

use slug_build_api_v2::actions::RetainedSpawnResourceSet;
use slug_build_api_v2::published_value::PublishedNodeId;
use slug_loading_v2::resource_set::ResourceSetRegistration;
use starlark::values::OwnedFrozenValue;

use super::args_callbacks::FrozenPublisher;
use super::args_callbacks::Publisher;
use crate::analysis_value::AnalysisValueLowerer;

mod runtime;
#[cfg(test)]
pub(super) use runtime::StarlarkResourceSetRuntime as RuntimeAllocationType;
#[derive(Debug, Clone)]
pub(super) struct DraftResourceSet {
    pub(super) node: PublishedNodeId,
}
pub(super) fn prepare<'v>(
    publisher: &mut Publisher<'v>,
    token: &ResourceSetRegistration,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<DraftResourceSet> {
    let value = publisher.resource_set_value(token)?;
    // Fixed control precedes layout reads. The runtime's own original handle
    // is separate from the descriptor's published-callable handle.
    publisher.resource_storage(2, size_of::<(usize, usize)>(), false)?;
    let (facts, bytes) = RetainedSpawnResourceSet::allocation_layout();
    let (runtime_layout, _) = Layout::new::<[usize; 2]>()
        .extend(Layout::new::<runtime::StarlarkResourceSetRuntime>())
        .map_err(|_| anyhow::anyhow!("resource_set runtime layout overflow"))?;
    // The descriptor layout already includes its trait-Arc and callable handle.
    // Runtime allocation includes its inline OwnedFrozenValue; a separate
    // original handle covers the temporary owned_root/finish transfer.
    let runtime_bytes = runtime_layout.pad_to_align().size();
    let total_bytes = bytes
        .checked_add(runtime_bytes)
        .and_then(|b| b.checked_add(size_of::<OwnedFrozenValue>()))
        .ok_or_else(|| anyhow::anyhow!("resource_set layout overflow"))?;
    let total_facts = facts
        .checked_add(runtime_bytes.div_ceil(size_of::<usize>()))
        .and_then(|n| n.checked_add(8))
        .ok_or_else(|| anyhow::anyhow!("resource_set layout overflow"))?;
    publisher.resource_storage(total_facts, total_bytes, false)?;
    // Drafts survive arena seal inside prepared actions. Conservatively retain
    // their full copy envelope; the admission still precedes every copy.
    publisher.resource_storage(4, 5 * size_of::<DraftResourceSet>(), false)?;
    Ok(DraftResourceSet {
        node: publisher.capture_resource_callable(value, lowerer)?,
    })
}
pub(super) fn complete(
    publisher: &FrozenPublisher,
    draft: DraftResourceSet,
) -> anyhow::Result<RetainedSpawnResourceSet> {
    let (published, original) = publisher.resource_original(draft.node)?;
    Ok(RetainedSpawnResourceSet::from_published(
        published,
        Arc::new(runtime::StarlarkResourceSetRuntime::new(original)),
    )?)
}
