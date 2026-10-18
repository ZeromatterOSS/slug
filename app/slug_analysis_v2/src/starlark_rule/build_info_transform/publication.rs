//! One original in the SAME provider/Args/resource publication graph.
use std::alloc::Layout;
use std::mem::size_of;
use std::sync::Arc;

use slug_build_api_v2::BuildInfoTransformSpec;
use slug_build_api_v2::RetainedBuildInfoTransform;
use slug_build_api_v2::published_value::PublishedNodeId;
use slug_loading_v2::build_info_transform::BuildInfoCallbackRegistration;
use starlark::values::OwnedFrozenValue;

use super::super::args_callbacks::FrozenPublisher;
use super::super::args_callbacks::Publisher;
use super::runtime::StarlarkBuildInfoRuntime;
use crate::analysis_value::AnalysisValueLowerer;
#[derive(Debug)]
pub(in crate::starlark_rule) struct DraftBuildInfo {
    node: PublishedNodeId,
}
pub(in crate::starlark_rule) fn prepare<'v>(
    publisher: &mut Publisher<'v>,
    token: &BuildInfoCallbackRegistration,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<DraftBuildInfo> {
    let value = publisher.build_info_value(token)?;
    publisher.resource_storage(4, 8 * size_of::<usize>(), false)?;
    let (descriptor_facts, descriptor_bytes) = RetainedBuildInfoTransform::allocation_layout();
    let (spec_facts, spec_bytes) = BuildInfoTransformSpec::allocation_layout();
    let (runtime, _) = Layout::new::<[usize; 2]>()
        .extend(Layout::new::<StarlarkBuildInfoRuntime>())
        .map_err(|_| anyhow::anyhow!("build-info runtime layout overflow"))?;
    let runtime_bytes = runtime.pad_to_align().size();
    let bytes = descriptor_bytes
        .checked_add(spec_bytes)
        .and_then(|b| b.checked_add(runtime_bytes))
        .and_then(|b| b.checked_add(size_of::<OwnedFrozenValue>()))
        .and_then(|b| b.checked_add(5 * size_of::<DraftBuildInfo>()))
        .ok_or_else(|| anyhow::anyhow!("build-info publication layout overflow"))?;
    let facts = descriptor_facts
        .checked_add(spec_facts)
        .and_then(|n| n.checked_add(runtime_bytes.div_ceil(size_of::<usize>())))
        .and_then(|n| n.checked_add(12))
        .ok_or_else(|| anyhow::anyhow!("build-info publication facts overflow"))?;
    publisher.resource_storage(facts, bytes, false)?;
    Ok(DraftBuildInfo {
        node: publisher.capture_resource_callable(value, lowerer)?,
    })
}
pub(in crate::starlark_rule) fn complete(
    publisher: &FrozenPublisher,
    draft: DraftBuildInfo,
) -> anyhow::Result<RetainedBuildInfoTransform> {
    let (published, original) = publisher.resource_original(draft.node)?;
    Ok(RetainedBuildInfoTransform::from_published(
        published,
        Arc::new(StarlarkBuildInfoRuntime::new(original)),
    )?)
}
