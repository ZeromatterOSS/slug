//! Real same-collector Publisher quotas, distinct from whole-capture costs.
use std::alloc::Layout;
use std::mem::size_of;

use slug_build_api_v2::actions::RetainedSpawnResourceSet;
use slug_build_api_v2::published_value::BYTE_LIMIT;
use slug_build_api_v2::published_value::NODE_LIMIT;
use slug_loading_v2::resource_set::ResourceSetRegistration;
use slug_loading_v2::resource_set::register_resource_set;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::values::OwnedFrozenValue;
use starlark::values::Value;
use starlark::values::ValueLike;

use crate::analysis_value::AnalysisValueLowerer;
use crate::starlark_rule::args_callbacks::Publisher;
use crate::starlark_rule::resource_set::DraftResourceSet;
use crate::starlark_rule::resource_set::RuntimeAllocationType;
use crate::starlark_rule::resource_set::{self};

struct Cost {
    facts: usize,
    descriptor_bytes: usize,
    lookup_bytes: usize,
    read_bytes: usize,
    draft_bytes: usize,
}
fn cost() -> Cost {
    let (descriptor_facts, descriptor_bytes) = RetainedSpawnResourceSet::allocation_layout();
    // Independent typed composition: no cached prepare total or body discount.
    let (runtime, _) = Layout::new::<[usize; 2]>()
        .extend(Layout::new::<RuntimeAllocationType>())
        .unwrap();
    let runtime_bytes = runtime.pad_to_align().size();
    let runtime_facts =
        runtime_bytes / size_of::<usize>() + usize::from(runtime_bytes % size_of::<usize>() != 0);
    Cost {
        facts: descriptor_facts + runtime_facts + 8,
        descriptor_bytes: descriptor_bytes + runtime_bytes + size_of::<OwnedFrozenValue>(),
        lookup_bytes: size_of::<Value<'_>>() + size_of::<ResourceSetRegistration>(),
        read_bytes: 2 * size_of::<usize>(),
        draft_bytes: 5 * size_of::<DraftResourceSet>(),
    }
}
fn callback(globals: &Globals) -> Value<'_> {
    // A genuine admitted native callable, whose numeric invocation would error
    // on two arguments; publication/registration must never evaluate it.
    globals
        .iter()
        .find(|(name, _)| *name == "str")
        .unwrap()
        .1
        .to_value()
}
fn snapshot(publisher: &Publisher<'_>) -> (usize, usize, usize, usize) {
    (
        publisher.graph.roots.len(),
        publisher.graph.roots.capacity(),
        publisher.graph.values.len(),
        publisher.graph.remaining,
    )
}
#[test]
fn full_descriptor_runtime_prepare_exact_and_plus_one_share_actual_publisher_quota() {
    let c = cost();
    let full_work = 8 + 2 + c.facts + 4 + 1;
    let full_bytes = c.lookup_bytes + c.read_bytes + c.descriptor_bytes + c.draft_bytes;
    for (extra_work, extra_bytes, ok) in [(0, 0, true), (1, 0, false), (0, 1, false)] {
        let globals = Globals::standard();
        let module = Module::new();
        let value = callback(&globals);
        let token = register_resource_set(&module, value).unwrap();
        let mut publisher = Publisher::with_resource_sets(&module).unwrap();
        let mut lowerer = AnalysisValueLowerer::from_module(&module);
        let original = publisher.capture(value, &mut lowerer).unwrap();
        let before = snapshot(&publisher);
        let work = publisher.graph.graph.charged_visits();
        let bytes = publisher.graph.graph.charged_bytes();
        publisher
            .resource_storage(
                NODE_LIMIT - work - full_work + extra_work,
                BYTE_LIMIT - bytes - full_bytes + extra_bytes,
                true,
            )
            .unwrap();
        let result = resource_set::prepare(&mut publisher, &token, &mut lowerer);
        assert_eq!(result.is_ok(), ok);
        assert_eq!(
            (
                publisher.graph.roots.len(),
                publisher.graph.roots.capacity(),
                publisher.graph.values.len()
            ),
            (before.0, before.1, before.2)
        );
        if ok {
            assert_eq!(result.unwrap().node, original);
            assert_eq!(publisher.graph.remaining, before.3 - 1);
            assert_eq!(publisher.graph.graph.charged_visits(), NODE_LIMIT);
            assert_eq!(publisher.graph.graph.charged_bytes(), BYTE_LIMIT);
        } else {
            assert_eq!(
                publisher.graph.remaining, before.3,
                "failure precedes the actual capture encounter"
            );
            assert!(
                publisher.graph.graph.charge_visit().is_err(),
                "quota failure remains poisoned"
            );
        }
    }
}
#[test]
fn descriptor_precharge_exact_admission_is_not_whole_prepare_and_refuses_before_capture() {
    let c = cost();
    let pre_work = 8 + 2 + c.facts;
    let pre_bytes = c.lookup_bytes + c.read_bytes + c.descriptor_bytes;
    for bytes_boundary in [false, true] {
        for excess in [0, 1] {
            let globals = Globals::standard();
            let module = Module::new();
            let token = register_resource_set(&module, callback(&globals)).unwrap();
            let mut publisher = Publisher::with_resource_sets(&module).unwrap();
            let mut lowerer = AnalysisValueLowerer::from_module(&module);
            let before = snapshot(&publisher);
            assert_eq!((before.0, before.2), (0, 0));
            let w0 = publisher.graph.graph.charged_visits();
            let b0 = publisher.graph.graph.charged_bytes();
            if bytes_boundary {
                publisher
                    .resource_storage(0, BYTE_LIMIT - b0 - pre_bytes + excess, false)
                    .unwrap();
            } else {
                publisher
                    .resource_storage(NODE_LIMIT - w0 - pre_work + excess, 0, false)
                    .unwrap();
            }
            let seeded_bytes = publisher.graph.graph.charged_bytes();
            let error = resource_set::prepare(&mut publisher, &token, &mut lowerer).unwrap_err();
            assert_eq!(
                snapshot(&publisher),
                before,
                "no map/root/slot/remaining mutation before capture"
            );
            if bytes_boundary {
                assert!(error.to_string().contains("bytes"));
                assert_eq!(
                    publisher.graph.graph.charged_bytes(),
                    if excess == 0 {
                        BYTE_LIMIT
                    } else {
                        seeded_bytes + c.lookup_bytes + c.read_bytes
                    }
                );
                assert_eq!(
                    publisher.graph.graph.charged_visits(),
                    w0 + pre_work + if excess == 0 { 4 } else { 0 }
                );
            } else {
                assert!(error.to_string().contains("visits"));
                assert_eq!(publisher.graph.graph.charged_visits(), NODE_LIMIT);
                assert_eq!(
                    publisher.graph.graph.charged_bytes(),
                    seeded_bytes
                        + c.lookup_bytes
                        + c.read_bytes
                        + if excess == 0 { c.descriptor_bytes } else { 0 }
                );
            }
            assert!(publisher.graph.graph.charge_visit().is_err());
        }
    }
}
