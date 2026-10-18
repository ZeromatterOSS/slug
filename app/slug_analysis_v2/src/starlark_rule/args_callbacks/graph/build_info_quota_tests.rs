//! Genuine Loader original, SAME Publisher and typed descriptor pre-copy quotas.
use std::alloc::Layout;
use std::mem::size_of;

use slug_build_api_v2::BuildInfoTransformSpec;
use slug_build_api_v2::RetainedBuildInfoTransform;
use slug_build_api_v2::published_value::BYTE_LIMIT;
use slug_build_api_v2::published_value::NODE_LIMIT;
use starlark::environment::Module;
use starlark::values::OwnedFrozenValue;
use starlark::values::Value;

use crate::analysis_value::AnalysisValueLowerer;
use crate::starlark_rule::args_callbacks::LoadedSource;
use crate::starlark_rule::args_callbacks::Publisher;
use crate::starlark_rule::build_info_transform::DraftBuildInfo;
use crate::starlark_rule::build_info_transform::RuntimeAllocationType;
use crate::starlark_rule::build_info_transform::{self};
#[tokio::test]
async fn descriptor_spec_runtime_full_precharge_exact_plus_one_preserves_original_memo() {
    let loaded =
        LoadedSource::new("def original(s): return {'x':'y'}\ndef entry(ctx): return original\n")
            .await;
    let (df, db) = RetainedBuildInfoTransform::allocation_layout();
    let (sf, sb) = BuildInfoTransformSpec::allocation_layout();
    let (rt, _) = Layout::new::<[usize; 2]>()
        .extend(Layout::new::<RuntimeAllocationType>())
        .unwrap();
    let rb = rt.pad_to_align().size();
    let facts = df + sf + rb.div_ceil(size_of::<usize>()) + 12;
    let work = 8 + 4 + facts + 1; // lookup, layout/control, full positive allocations, warm memo encounter.
    let retained = 8 * size_of::<usize>()
        + db
        + sb
        + rb
        + size_of::<OwnedFrozenValue>()
        + 5 * size_of::<DraftBuildInfo>();
    let lookup = size_of::<Value<'_>>()
        + size_of::<slug_loading_v2::resource_set::ResourceSetRegistration>();
    for (extra_work, extra_bytes, ok) in [(0, 0, true), (1, 0, false), (0, 1, false)] {
        let module = Module::new();
        let original = loaded.invoke(&module, Value::new_none());
        let token =
            slug_loading_v2::build_info_transform::register_callback(&module, original).unwrap();
        let mut publisher = Publisher::with_resource_sets(&module).unwrap();
        let mut lowerer = AnalysisValueLowerer::from_module(&module);
        let first = publisher.capture(original, &mut lowerer).unwrap();
        let before = (
            publisher.graph.roots.len(),
            publisher.graph.roots.capacity(),
            publisher.graph.values.len(),
            publisher.graph.remaining,
        );
        let w = publisher.graph.graph.charged_visits();
        let b = publisher.graph.graph.charged_bytes();
        publisher
            .resource_storage(
                NODE_LIMIT - w - work + extra_work,
                BYTE_LIMIT - b - retained - lookup + extra_bytes,
                false,
            )
            .unwrap();
        let prepared = build_info_transform::prepare(&mut publisher, &token, &mut lowerer);
        assert_eq!(prepared.is_ok(), ok);
        assert_eq!(
            (
                publisher.graph.roots.len(),
                publisher.graph.roots.capacity(),
                publisher.graph.values.len()
            ),
            (before.0, before.1, before.2)
        );
        if ok {
            drop(prepared.unwrap());
            assert_eq!(publisher.graph.graph.charged_visits(), NODE_LIMIT);
            assert_eq!(publisher.capture(original, &mut lowerer).is_err(), true);
        } else {
            assert!(publisher.graph.graph.charge_visit().is_err());
            assert_eq!(publisher.graph.remaining, before.3);
        }
        let _ = first;
    }
}
