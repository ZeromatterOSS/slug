//! Disabled coverage uses the existing native empty provider and configuration owner.
use slug_build_api_v2::ProviderIdentity;
use starlark::values::FrozenHeap;

use super::*;

impl SynchronousAnalysisActionSink {
    pub(super) fn materialize_empty_instrumented_files(
        &self,
        ctx: Value<'_>,
        heap: &FrozenHeap,
    ) -> anyhow::Result<FrozenValue> {
        let ctx = AnalysisContext::from_value(ctx).ok_or_else(|| {
            anyhow::anyhow!("coverage_common.instrumented_files_info ctx must be a rule context")
        })?;
        anyhow::ensure!(
            std::ptr::addr_eq(ctx.action_sink.as_ref(), self as &dyn AnalysisActionSink),
            "coverage_common.instrumented_files_info ctx belongs to a different analysis sink"
        );
        ctx.token
            .require_active("instrumented_files_info", "rule context")?;
        ctx.cc_context
            .require_disabled_coverage("coverage_common.instrumented_files_info")?;
        let empty = AnalysisValue::depset(AnalysisDepset::empty(DepsetOrder::Default));
        let occurrence = ProviderOccurrence::new(
            ProviderIdentity::builtin("InstrumentedFilesInfo"),
            [
                ("instrumented_files", empty.clone()),
                ("metadata_files", empty),
            ],
        );
        // The caller owns the frozen allocation; the sink retains no phase Value.
        AnalysisValueMaterializer::new(heap)
            .value(&AnalysisValue::provider(occurrence))
            .map_err(anyhow::Error::msg)
    }
}
