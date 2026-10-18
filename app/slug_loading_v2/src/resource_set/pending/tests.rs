//! Actual collector lifecycle and independent typed pre-copy cost boundaries.
use std::mem::size_of;

use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::none::NoneType;
use starlark::values::tuple::TupleRef;

use super::registry::INITIAL_WORK;
use super::registry::REGISTER_WORK;
use super::registry::Registry;
use super::*;
use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
use crate::bzl_module::semantic_evaluation::budget::Budget;
use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
use crate::subrule_invocation::clear_args_snapshots;
use crate::subrule_invocation::pending_args;

#[test]
fn collector_typed_layout_and_growth_refuse_before_retention_and_poison() {
    let bytes = size_of::<pending_args::RegisteredSnapshot<'_>>()
        + size_of::<Registry<Value<'_>>>()
        + size_of::<ResourceSetRegistration>()
        + 16 * size_of::<Value<'_>>()
        + 32 * size_of::<usize>();
    let exact = ResourceRegistry::<Value<'_>>::new(
        Budget {
            records: RECORD_LIMIT - INITIAL_WORK,
            bytes: BYTE_LIMIT - bytes,
            depth: 0,
        },
        size_of::<pending_args::RegisteredSnapshot<'_>>(),
    )
    .unwrap();
    let state = exact.registry.lock().unwrap();
    assert_eq!(
        (state.work.records, state.work.bytes),
        (RECORD_LIMIT, BYTE_LIMIT)
    );
    assert!(state.values.is_empty());
    drop(state);
    for (records, initial_bytes) in [
        (RECORD_LIMIT - INITIAL_WORK + 1, 0),
        (0, BYTE_LIMIT - bytes + 1),
    ] {
        assert!(
            ResourceRegistry::<Value<'_>>::new(
                Budget {
                    records,
                    bytes: initial_bytes,
                    depth: 0
                },
                size_of::<pending_args::RegisteredSnapshot<'_>>()
            )
            .is_err()
        );
    }
    let module = Module::new();
    let collector = pending_args::collector(&module).unwrap();
    assert_eq!(
        TupleRef::from_value(module.extra_value().unwrap())
            .unwrap()
            .content()
            .len(),
        4
    );
    let first = size_of::<ResourceSetRegistration>()
        + 16 * size_of::<Value<'_>>()
        + size_of::<Vec<Value<'_>>>();
    let mut state = collector.resources.registry.lock().unwrap();
    for (records, initial_bytes, ok) in [
        (RECORD_LIMIT - REGISTER_WORK - 1, BYTE_LIMIT - first, true),
        (RECORD_LIMIT - REGISTER_WORK, 0, false),
        (0, BYTE_LIMIT - first + 1, false),
    ] {
        state.work = Budget {
            records,
            bytes: initial_bytes,
            depth: 0,
        };
        state.phase = Phase::Registering;
        // No token or Value is retained before BOTH admissions succeed.
        assert_eq!(state.prepare_push().is_ok(), ok);
        assert!(state.values.is_empty());
        if !ok {
            assert!(matches!(state.phase, Phase::Poisoned));
            assert!(state.prepare_push().is_err());
        }
        // Reset only this isolated test state; production never resets credit.
        state.vector_credit = 0;
    }
}

#[test]
fn actual_collector_transfers_once_and_post_transfer_lookups_spend_only_publisher() {
    let module = Module::new();
    let token = register_resource_set(&module, Value::new_none()).unwrap();
    assert!(clear_args_snapshots(&module).is_err());
    let collector = pending_args::existing_collector(&module).unwrap().unwrap();
    assert_eq!(
        collector.resources.registry.lock().unwrap().work.records,
        2 + INITIAL_WORK + REGISTER_WORK + 1
    );
    let mut publisher = Budget::default();
    transfer_resource_sets(&module, |r, b| publisher.charge(r, b)).unwrap();
    let transferred = collector.resources.registry.lock().unwrap().work;
    assert_eq!(publisher.records, TRANSFER_WORK + transferred.records);
    assert_eq!(
        transferred.records,
        2 + INITIAL_WORK + REGISTER_WORK + 1 + TRANSFER_WORK
    );
    assert!(
        resource_set_value(&module, &token, |r, b| publisher.charge(r, b))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        publisher.records,
        TRANSFER_WORK + transferred.records + LOOKUP_WORK
    );
    let after = collector.resources.registry.lock().unwrap().work;
    assert_eq!(
        (after.records, after.bytes, after.depth),
        (transferred.records, transferred.bytes, transferred.depth)
    );
    let mut refused = Budget {
        records: RECORD_LIMIT - LOOKUP_WORK + 1,
        ..Budget::default()
    };
    assert!(resource_set_value(&module, &token, |r, b| refused.charge(r, b)).is_err());
    assert_eq!(
        collector.resources.registry.lock().unwrap().work.records,
        transferred.records
    );
    assert!(clear_args_snapshots(&module).is_err());
    assert!(transfer_resource_sets(&module, |r, b| publisher.charge(r, b)).is_err());
    assert!(matches!(
        collector.resources.registry.lock().unwrap().phase,
        Phase::Poisoned
    ));
    assert!(resource_set_value(&module, &token, |r, b| publisher.charge(r, b)).is_err());
}

#[test]
fn args_only_creation_pays_sidecar_and_transfers_before_any_snapshot_publication() {
    let module = Module::new();
    let args = crate::subrule_invocation::register_args_snapshot(
        &module,
        crate::subrule_invocation::EvaluatorArgsSnapshot {
            calls: Vec::new(),
            format: slug_build_api_v2::RetainedParamFileFormat::Shell,
            param_file: None,
        },
    )
    .unwrap();
    let collector = pending_args::existing_collector(&module).unwrap().unwrap();
    let before = collector.resources.registry.lock().unwrap().work;
    assert_eq!(before.records, 2 + INITIAL_WORK);
    let mut publisher = Budget::default();
    transfer_resource_sets(&module, |r, b| publisher.charge(r, b)).unwrap();
    assert_eq!(publisher.records, 2 + INITIAL_WORK + 2 * TRANSFER_WORK);
    assert!(
        crate::subrule_invocation::args_snapshot(&module, &args)
            .unwrap()
            .calls
            .is_empty()
    );
    assert!(clear_args_snapshots(&module).is_err());
}

#[test]
fn failed_or_reentrant_transfer_preserves_poisoned_roots_and_cannot_reopen() {
    for reentrant in [false, true] {
        let module = Module::new();
        let token = register_resource_set(&module, Value::new_none()).unwrap();
        let mut calls = 0;
        let result = transfer_resource_sets(&module, |_, _| {
            calls += 1;
            if calls == 2 {
                if reentrant {
                    assert!(register_resource_set(&module, Value::new_none()).is_err());
                } else {
                    return Err("actual Publisher quota refusal".into());
                }
            }
            Ok(())
        });
        assert!(result.is_err());
        let collector = pending_args::existing_collector(&module).unwrap().unwrap();
        let state = collector.resources.registry.lock().unwrap();
        assert!(matches!(state.phase, Phase::Poisoned));
        assert_eq!(state.values.len(), 1);
        drop(state);
        assert!(resource_set_value(&module, &token, |_, _| Ok(())).is_err());
        assert!(clear_args_snapshots(&module).is_err());
    }
}

#[starlark::starlark_module]
fn gc_helper(builder: &mut GlobalsBuilder) {
    fn collect(eval: &mut Evaluator<'_, '_, '_>) -> anyhow::Result<NoneType> {
        unsafe {
            eval.garbage_collect()
                .map_err(|error| error.into_anyhow())?
        };
        Ok(NoneType)
    }
}
#[test]
fn resource_only_sidecar_is_traced_without_args_and_four_old_slots_are_preserved() {
    for poison in [false, true] {
        let module = Module::new();
        Evaluator::new(&module)
            .eval_module(
                AstModule::parse(
                    "callback.bzl",
                    "def callback(os, count): return os + str(count)".to_owned(),
                    &Dialect::Bazel,
                )
                .unwrap(),
                &GlobalsBuilder::standard().build(),
            )
            .unwrap();
        let token = register_resource_set(&module, module.get("callback").unwrap()).unwrap();
        module.set("callback", Value::new_none());
        if poison {
            let collector = pending_args::existing_collector(&module).unwrap().unwrap();
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _locked = collector.resources.registry.lock().unwrap();
                    panic!("test-only host failure while locked");
                }))
                .is_err()
            );
        }
        Evaluator::new(&module)
            .eval_module(
                AstModule::parse("gc.bzl", "collect()".to_owned(), &Dialect::Bazel).unwrap(),
                &GlobalsBuilder::standard().with(gc_helper).build(),
            )
            .unwrap();
        let roots = TupleRef::from_value(module.extra_value().unwrap()).unwrap();
        assert_eq!(roots.content().len(), 4);
        assert!(roots.content()[..3].iter().all(|v| v.is_none()));
        let mut publisher = Budget::default();
        let callable = if poison {
            assert!(transfer_resource_sets(&module, |r, b| publisher.charge(r, b)).is_err());
            assert!(clear_args_snapshots(&module).is_err());
            // Private rooting oracle only: poisoned state is never publishable.
            // The GC trace must nonetheless retain/update every moving Value.
            let collector = pending_args::existing_collector(&module).unwrap().unwrap();
            collector
                .resources
                .registry
                .lock()
                .unwrap_err()
                .into_inner()
                .values[0]
        } else {
            transfer_resource_sets(&module, |r, b| publisher.charge(r, b)).unwrap();
            resource_set_value(&module, &token, |r, b| publisher.charge(r, b)).unwrap()
        };
        assert_eq!(
            Evaluator::new(&module)
                .eval_function(
                    callable,
                    &[module.heap().alloc("linux"), module.heap().alloc(7)],
                    &[]
                )
                .unwrap()
                .unpack_str(),
            Some("linux7")
        );
        assert!(
            module.freeze().is_err(),
            "collector cannot escape unaudited through freeze"
        );
    }
}
