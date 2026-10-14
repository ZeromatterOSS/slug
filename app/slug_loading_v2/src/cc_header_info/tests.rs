use std::sync::Arc;

use sha2::Digest;
use sha2::Sha256;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::ValueLike;

use super::*;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;
use crate::subrule_invocation::AnalysisEvaluationContext;

fn internal() -> FrozenModule {
    let module = Module::new();
    let source = "i = cc_common.internal_DO_NOT_USE()";
    let identity = crate::BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_cc+//cc/private:header_test.bzl").unwrap(),
        workspace_path: "/header-fixture/cc/private/header_test.bzl".into(),
        repository_mapping: Arc::from([]),
    };
    let digest: [u8; 32] = Sha256::digest(source.as_bytes()).into();
    let manifest = crate::BzlLoadManifest {
        root: identity.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([crate::BzlModuleSourceProvenance::new(identity, digest)]),
        fingerprint: digest,
    };
    crate::starlark_label::attach_defining_module_manifest(&module, &manifest).unwrap();
    let context = BzlEvaluationContext::from_manifest(&manifest);
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse("header_test.bzl", source.to_owned(), &Dialect::Bazel).unwrap(),
        &loading_globals(),
    )
    .unwrap();
    drop(eval);
    module.freeze().unwrap()
}
fn evaluate(body: &str, host: Option<ActionEnvironmentHostOs>) -> anyhow::Result<FrozenModule> {
    let module = Module::new();
    module.import_public_symbols(&internal());
    for name in ["a.h", "b.h", "move.inc", "UPPER.INC"] {
        module.set(
            &name.replace('.', "_"),
            module
                .heap()
                .alloc(AnalysisArtifactValue::new(AnalysisArtifact::Source(
                    CanonicalLabel::parse(&format!("@@//:{name}")).unwrap(),
                ))),
        );
    }
    let owner =
        AnalysisConfiguredTargetKey::new(CanonicalLabel::parse("@@//:producer").unwrap(), vec![1]);
    module.set(
        "pcm",
        module
            .heap()
            .alloc(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
                owner,
                output: ActionOutput::new(
                    crate::artifact_root::test_root(),
                    "module.pcm",
                    ActionOutputKind::Directory,
                ),
            })),
    );
    let context = AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        CanonicalLabel::parse("@@//:consumer").unwrap(),
        Arc::new(crate::cc_common::solib_tests::RecordingSink::default()),
        FrozenValue::new_none(),
        Arc::from([]),
    )
    .with_host_os(host);
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse("header_test.bzl", body.to_owned(), &Dialect::Bazel)
            .map_err(starlark::Error::into_anyhow)?,
        &loading_globals(),
    )
    .map_err(starlark::Error::into_anyhow)?;
    drop(eval);
    if let Some(value) = module.get("h") {
        if let Some(wrapper) = HeaderInfoValue::from_value(value) {
            assert!(std::ptr::eq(cc_header_info_ref(value).unwrap(), &wrapper.0));
        }
    }
    Ok(module.freeze()?)
}

#[test]
fn constructors_use_observed_host_without_cpp_fragment_and_fail_closed_without_host() {
    let body = "h = i.create_header_info(modular_public_headers = [a_h, UPPER_INC, move_inc, a_h])";
    for (os, expected) in [
        (ActionEnvironmentHostOs::Linux, 2),
        (ActionEnvironmentHostOs::Windows, 1),
    ] {
        let result = evaluate(body, Some(os)).unwrap();
        let h = cc_header_info(result.get("h").unwrap().value()).unwrap();
        assert_eq!(h.model().modular_public_headers.len(), expected);
        assert_eq!(h.model().textual_headers.len(), 3 - expected);
    }
    assert!(
        evaluate(body, None)
            .unwrap_err()
            .to_string()
            .contains("observed analysis Host")
    );
    evaluate("h = i.create_header_info(header_module = None, modular_public_headers = range(0))\nx = i.create_header_info_with_deps(header_info = h)", None).unwrap();
}

#[test]
fn constructors_preserve_module_files_merge_order_and_immutable_projections() {
    let result = evaluate(r#"
a = i.create_header_info(modular_public_headers = [a_h, move_inc, a_h], modular_private_headers = (a_h, b_h), textual_headers = [b_h], separate_module_headers = [move_inc, move_inc], header_module = pcm, pic_header_module = pcm, separate_module = pcm, separate_pic_module = pcm)
b = i.create_header_info(modular_public_headers = [b_h])
h = i.create_header_info_with_deps(header_info = a, deps = [b, b], merged_deps = [b])
alias = h
fresh = i.create_header_info_with_deps(header_info = a, deps = [b, b], merged_deps = [b])
checks = (h == alias, h != fresh, type(h) == "HeaderInfo", h.header_module == pcm, h.pic_header_module == pcm, h.separate_module == pcm, h.separate_pic_module == pcm, h.modular_public_headers == [a_h, b_h], h.modular_private_headers == [a_h, b_h], h.textual_headers == [b_h, move_inc], h.separate_module_headers == [move_inc, move_inc], not hasattr(h, "deps"))
lookup = {h: "yes"}
hashed = lookup[alias]
projected = h.modular_public_headers
"#, Some(ActionEnvironmentHostOs::Linux)).unwrap();
    let checks = result.get("checks").unwrap();
    let checks = TupleRef::from_value(checks.value()).unwrap();
    assert!(checks.iter().all(|value| value.unpack_bool() == Some(true)));
    assert_eq!(result.get("hashed").unwrap().unpack_str(), Some("yes"));
    let h = cc_header_info(result.get("h").unwrap().value()).unwrap();
    assert_eq!(h.model().deps.len(), 2);
    assert_eq!(h.model().deps[0], h.model().deps[1]);
    // The owning module is frozen; projections remain real Files in immutable lists.
    assert_eq!(
        ListRef::from_value(result.get("projected").unwrap().value())
            .unwrap()
            .len(),
        2
    );
    for mutation in [
        "h.modular_public_headers.append(a_h)",
        "h.modular_public_headers[0] = b_h",
        "h.textual_headers.clear()",
    ] {
        let body = format!("h = i.create_header_info(modular_public_headers = [a_h])\n{mutation}");
        assert!(
            evaluate(&body, Some(ActionEnvironmentHostOs::Linux)).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn header_constructor_binding_rejects_wrong_sequence_artifact_and_node_shapes() {
    for call in [
        "i.create_header_info(None)",
        "i.create_header_info(unknown = None)",
        "i.create_header_info(modular_public_headers = None)",
        "i.create_header_info(modular_public_headers = depset())",
        "i.create_header_info(modular_public_headers = {})",
        "i.create_header_info(modular_public_headers = 'file')",
        "i.create_header_info(modular_public_headers = range(1))",
        "i.create_header_info(modular_public_headers = range(1000000000))",
        "i.create_header_info(modular_public_headers = [1])",
        "i.create_header_info(header_module = a_h)",
        "i.create_header_info(pic_header_module = a_h)",
        "i.create_header_info(separate_module = a_h)",
        "i.create_header_info(separate_pic_module = a_h)",
        "i.create_header_info(separate_module = pcm)",
        "i.create_header_info(separate_pic_module = pcm)",
        "i.create_header_info_with_deps()",
        "i.create_header_info_with_deps(header_info = a_h)",
        "i.create_header_info_with_deps(header_info = h, deps = [a_h])",
        "i.create_header_info_with_deps(header_info = h, merged_deps = [a_h])",
        "i.create_header_info_with_deps(header_info = h, deps = depset())",
        "i.create_header_info_with_deps(header_info = h, deps = range(1000000000))",
        "i.create_header_info_with_deps(header_info = h, merged_deps = range(1000000000))",
    ] {
        assert!(
            evaluate(
                &format!("h = i.create_header_info()\nx = {call}"),
                Some(ActionEnvironmentHostOs::Linux)
            )
            .is_err(),
            "{call}"
        );
    }
}

#[test]
fn frozen_model_facade_preserves_occurrence_after_producer_heap_disposal() {
    let result = evaluate(
        "h = i.create_header_info(modular_public_headers = [a_h], header_module = pcm)",
        Some(ActionEnvironmentHostOs::Linux),
    )
    .unwrap();
    let original = result.get("h").unwrap();
    let borrowed = cc_header_info_ref(original.value()).unwrap();
    assert!(std::ptr::eq(
        borrowed,
        &HeaderInfoValue::from_value(original.value()).unwrap().0
    ));
    let info = cc_header_info(original.value()).unwrap();
    assert!(std::ptr::eq(borrowed.model(), info.model()));
    drop(original);
    drop(result);
    let heap = FrozenHeap::new();
    let value = alloc_frozen_cc_header_info(&heap, info.clone());
    let alias = alloc_frozen_cc_header_info(&heap, info.clone());
    assert!(value.to_value().equals(alias.to_value()).unwrap());
    assert_eq!(cc_header_info(value.to_value()).unwrap(), info);
    assert!(std::ptr::eq(
        cc_header_info_ref(value.to_value()).unwrap().model(),
        info.model()
    ));
    assert!(cc_header_info_ref(Value::new_none()).is_none());
    let projection_heap = Module::new();
    let headers = value
        .to_value()
        .get_attr("modular_public_headers", projection_heap.heap())
        .unwrap()
        .unwrap();
    let file = ListRef::from_value(headers).unwrap().iter().next().unwrap();
    assert_eq!(
        AnalysisArtifactValue::from_starlark(file)
            .unwrap()
            .artifact(),
        &info.model().modular_public_headers[0]
    );
}

#[test]
fn live_header_info_and_artifact_projections_survive_supported_gc_modes() {
    let retained = evaluate(
        "h = i.create_header_info(modular_public_headers = [a_h], header_module = pcm)",
        Some(ActionEnvironmentHostOs::Linux),
    )
    .unwrap();
    static CONTEXT: std::sync::LazyLock<AnalysisEvaluationContext> =
        std::sync::LazyLock::new(|| {
            AnalysisEvaluationContext::new(
                Arc::from([]),
                [],
                CanonicalLabel::parse("@@//:gc").unwrap(),
                Arc::new(crate::cc_common::solib_tests::RecordingSink::default()),
                FrozenValue::new_none(),
                Arc::from([]),
            )
            .with_host_os(Some(ActionEnvironmentHostOs::Linux))
        });
    let mut gc = starlark::assert::Assert::new();
    gc.disable_static_typechecking();
    gc.setup_eval(|eval| eval.extra = Some(&*CONTEXT));
    gc.module_add("retained", retained);
    gc.module_add("internal", internal());
    // Assert runs Auto/Never/Always, collecting only at safe bytecode boundaries.
    gc.pass(r#"
load("retained", "h")
load("internal", "i")
live = i.create_header_info(modular_public_headers=h.modular_public_headers, header_module=h.header_module)
alias = live
headers = live.modular_public_headers
garbage_collect()
assert_eq(headers, h.modular_public_headers)
assert_eq(live.header_module, h.header_module)
assert_eq(live, alias)
assert_ne(live, h)
assert_eq({live: 7}[alias], 7)
"#);
}
