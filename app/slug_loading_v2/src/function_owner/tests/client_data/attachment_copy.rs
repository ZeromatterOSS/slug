//! Fixed effective attachment copying stays on the original owner ledger.
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;

use super::*;
use crate::bzl_module::semantic_evaluation::budget::Budget;
use crate::function_owner::manifest::prepay_effective_attachment_copy;
use crate::source_table::CertifiedSourceTable;

fn copy_bytes() -> usize {
    std::mem::size_of::<CertifiedSourceTable>()
        + std::mem::size_of::<Budget>()
        + 2 * std::mem::size_of::<usize>()
}

#[test]
fn effective_attachment_fixed_copy_exact_allowances_precede_actual_context_clone() {
    let raw = mapped_manifest();
    let env = bootstrap();
    let context = crate::BzlModuleSourceContext::new(&raw, &env, std::iter::empty()).unwrap();
    let meta = Metadata::EffectiveContext(&context);
    let owner = native_owner(meta, &env, vec![]);
    let guards = Arc::strong_count(context.sources());
    let descriptor_guards = context.certified_sources().descriptor_strong_count();
    let (retained, actual_work) = super::super::super::manifest::original_storage(meta).unwrap();
    let identity = identity_shape(&raw.root).unwrap();
    // Enumerate the old initial header/root and literal zero-byte read, then
    // the new fixed8. Retained payload certificate is independently unchanged.
    assert_eq!(actual_work.visits, 1 + identity.visits + 1 + 8);
    assert_eq!(
        actual_work.bytes,
        Metadata::header_bytes() + identity.bytes + copy_bytes()
    );
    assert_eq!(actual_work.edges, 4 + identity.edges);
    assert_eq!(retained.visits, 0);
    assert_eq!(
        retained.bytes,
        Metadata::header_bytes() + identity.bytes + context.retention().bytes
    );
    assert_eq!(Arc::strong_count(context.sources()), guards);
    for bytes in [false, true] {
        for extra in [0, 1] {
            // Isolated allowance unit on the SAME PublicationShape/admission
            // primitive. The real whole-owner path above includes this helper
            // exactly once; this is not an extra production allowance/debit.
            let mut work = if bytes {
                PublicationShape {
                    bytes: 64 * 1024 * 1024 - copy_bytes() + extra,
                    ..zero_shape()
                }
            } else {
                PublicationShape {
                    visits: 65536 - 8 + extra,
                    ..zero_shape()
                }
            };
            let before = work;
            let module = Module::new();
            let result = prepay_effective_attachment_copy(&mut work)
                .and_then(|()| meta.attach(&module, owner.clone()));
            assert_eq!(result.is_ok(), extra == 0);
            assert_eq!(work.visits, before.visits + 8);
            assert_eq!(work.bytes, before.bytes + copy_bytes());
            if extra == 0 {
                assert_eq!(Arc::strong_count(context.sources()), guards);
                assert_eq!(
                    context.certified_sources().descriptor_strong_count(),
                    descriptor_guards + 2
                );
                let data = module.client_data().unwrap();
                let attached = crate::starlark_label::defining_module_source_context(data).unwrap();
                assert!(Arc::ptr_eq(attached.sources(), context.sources()));
                let mut identity_audit = PublishedValueArenaBuilder::new();
                identity_audit
                    .audit_shared_input_at_depth(&context.certified_sources().declarations()[0], 1)
                    .unwrap();
                let prior = identity_audit.charged_visits();
                identity_audit
                    .audit_shared_input_at_depth(&attached.certified_sources().declarations()[0], 1)
                    .unwrap();
                assert_eq!(identity_audit.charged_visits() - prior, 2);
            } else {
                assert!(module.client_data().is_none());
                assert_eq!(
                    Arc::strong_count(context.sources()),
                    guards,
                    "refusal precedes the actual pair/context clone"
                );
                // add_shape deliberately keeps admitted/failed counters; it
                // has no reset or separate poison flag. Reuse still refuses.
                assert!(prepay_effective_attachment_copy(&mut work).is_err());
            }
            drop(module);
            assert_eq!(Arc::strong_count(context.sources()), guards);
            assert_eq!(
                context.certified_sources().descriptor_strong_count(),
                descriptor_guards
            );
        }
    }
}

#[test]
fn effective_attachment_freeze_keeps_same_descriptor_until_last_module_and_audit_drop() {
    let raw = mapped_manifest();
    let env = bootstrap();
    let context = crate::BzlModuleSourceContext::new(&raw, &env, std::iter::empty()).unwrap();
    let weak = Arc::downgrade(context.sources());
    let meta = Metadata::EffectiveContext(&context);
    let owner = native_owner(meta, &env, vec![]);
    let mut audit = PublishedValueArenaBuilder::new();
    audit
        .audit_shared_input_at_depth(&context.certified_sources().declarations()[0], 1)
        .unwrap();
    let before = audit.charged_visits();
    let frozen = freeze_with(meta, owner.clone());
    let data = frozen.client_data().unwrap().to_value();
    let attached = crate::starlark_label::defining_module_source_context(data).unwrap();
    assert!(Arc::ptr_eq(attached.sources(), context.sources()));
    assert_eq!(attached, &context);
    audit
        .audit_shared_input_at_depth(&attached.certified_sources().declarations()[0], 1)
        .unwrap();
    assert_eq!(
        audit.charged_visits() - before,
        2,
        "actual descriptor guard, not an equal newly allocated table"
    );
    let function = frozen.get("f").unwrap();
    assert_eq!(
        snapshot_function_symbol(function.value())
            .unwrap()
            .unwrap()
            .owner(),
        &owner
    );
    drop(function);
    drop(context);
    drop(owner);
    drop(raw);
    assert!(weak.upgrade().is_some());
    drop(frozen);
    assert!(
        weak.upgrade().is_some(),
        "the raw audit still owns the actual descriptor"
    );
    drop(audit);
    assert!(weak.upgrade().is_none());
}

#[test]
fn bootstrap_effective_attachment_carries_complete_mapping_table_depth_before_copy() {
    let raw = mapped_manifest();
    let env = bootstrap();
    assert!(
        env.modules().next().is_none(),
        "no deeper builtin owner can mask this path"
    );
    let context = crate::BzlModuleSourceContext::new(&raw, &env, std::iter::empty()).unwrap();
    let table = &context.certified_sources().declarations()[0];
    assert_eq!(table.retained_shape().depth, 5);
    assert_eq!(context.retention().depth, 6);
    assert_eq!(context.producer_work().depth, 6);
    let meta = Metadata::EffectiveContext(&context);
    let (retained, _) = super::super::super::manifest::original_storage(meta).unwrap();
    assert_eq!(retained.depth, 7);
    let owner = native_owner(meta, &env, vec![]);
    let row = &owner
        .input_validity()
        .as_any()
        .downcast_ref::<BzlInputValidity>()
        .unwrap()
        .rows[0];
    assert_eq!(row.original_metadata.depth, 7);
    assert_eq!(row.retained_shape().depth, 8);
    assert_eq!(owner.retained_shape().depth, 9);
    let raw_guards = Arc::strong_count(context.sources());
    let descriptor_guards = context.certified_sources().descriptor_strong_count();
    let module = Module::new();
    let mut audit = PublishedValueArenaBuilder::new();
    let result = audit
        .audit_shared_input_at_depth(
            table,
            slug_build_api_v2::published_value::DEPTH_LIMIT - table.retained_shape().depth + 1,
        )
        .map_err(anyhow::Error::new)
        .and_then(|()| meta.attach(&module, owner.clone()));
    assert!(result.is_err());
    assert!(module.client_data().is_none());
    assert_eq!(Arc::strong_count(context.sources()), raw_guards);
    assert_eq!(
        context.certified_sources().descriptor_strong_count(),
        descriptor_guards
    );
    assert!(audit.charge_visit().is_err());
    meta.attach(&module, owner).unwrap();
    let attached =
        crate::starlark_label::defining_module_source_context(module.client_data().unwrap())
            .unwrap();
    assert_eq!(attached.retention().depth, 6);
    assert!(Arc::ptr_eq(attached.sources(), context.sources()));
    assert_eq!(Arc::strong_count(context.sources()), raw_guards);
    assert_eq!(
        context.certified_sources().descriptor_strong_count(),
        descriptor_guards + 2
    );
}
