use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutputMetadata;
use slug_build_api_v2::BuildInfoCallbackRecipe;
use slug_build_api_v2::ProviderId;
use slug_build_api_v2::ProviderIdentity;
use slug_identity_v2::CanonicalLabel;
use starlark::values::FrozenHeap;

use super::super::context_lifecycle_tests::configuration;
use super::super::context_lifecycle_tests::declaration_sink;
use super::*;

fn request(kind: BuildInfoTransformKind, name: &str) -> BuildInfoTransformRequest<'_> {
    BuildInfoTransformRequest {
        callback: BuildInfoCallbackRecipe::new(
            kind,
            CanonicalLabel::parse(BuildInfoCallbackRecipe::SOURCE_LABEL).unwrap(),
            BuildInfoCallbackRecipe::SOURCE_DIGEST,
        )
        .unwrap(),
        template: AnalysisArtifact::Source(
            CanonicalLabel::parse(
                "@@bazel_tools//tools/build_defs/build_info/templates:template.h",
            )
            .unwrap(),
        ),
        output_file_name: name,
    }
}

#[test]
fn actual_sink_preserves_status_output_metadata_order_and_failed_registration_atomicity() {
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@external+//pkg:translator").unwrap(),
        configuration(),
    );
    let sink = declaration_sink(&key);
    let volatile = sink
        .transform_build_info(request(BuildInfoTransformKind::Version, "volatile.h"))
        .unwrap();
    let stable = sink
        .transform_build_info(request(BuildInfoTransformKind::Info, "stable.h"))
        .unwrap();
    let AnalysisArtifact::Derived { owner, output } = volatile.artifact() else {
        panic!("derived")
    };
    assert_eq!(owner, &key.artifact_owner());
    assert_eq!(
        output.root_relative_path(),
        "external/external+/pkg/volatile.h"
    );
    assert_eq!(output.metadata(), ActionOutputMetadata::Constant);
    assert_eq!(
        output.root().configuration(),
        key.configuration().slug_configuration().unwrap()
    );
    let AnalysisArtifact::Derived { output, .. } = stable.artifact() else {
        panic!("derived")
    };
    assert_eq!(output.metadata(), ActionOutputMetadata::Ordinary);
    let snapshot = sink.actions.lock().unwrap().clone();
    assert_eq!(snapshot.registry().actions().len(), 2);
    for (action, expected) in snapshot.registry().actions().iter().zip([
        sink.workspace_status.volatile(),
        sink.workspace_status.stable(),
    ]) {
        assert_eq!(action.kind(), &ActionKind::BuildInfoTransform);
        assert_eq!(
            action.build_info_transform_spec().unwrap().status(),
            expected
        );
    }
    let declarations = sink.declared_outputs.lock().unwrap().clone();
    for (kind, name) in [
        (BuildInfoTransformKind::Info, "volatile.h"),
        (BuildInfoTransformKind::Version, "stable.h"),
        (BuildInfoTransformKind::Version, "volatile.h"),
        (BuildInfoTransformKind::Info, "../bad"),
        (BuildInfoTransformKind::Info, ""),
    ] {
        assert!(sink.transform_build_info(request(kind, name)).is_err());
        assert_eq!(snapshot, *sink.actions.lock().unwrap());
        assert_eq!(declarations, *sink.declared_outputs.lock().unwrap());
    }
    assert!(
        sink.declare_file("volatile.h").is_err(),
        "ordinary redeclaration cannot erase constant metadata"
    );
    sink.declare_directory("tree.h").unwrap();
    assert!(
        sink.transform_build_info(request(BuildInfoTransformKind::Version, "tree.h"))
            .is_err()
    );
    let ordinary = sink.declare_file("ordinary.h").unwrap();
    let heap = FrozenHeap::new();
    sink.write(
        heap.alloc(ordinary).to_value(),
        heap.alloc("body").to_value(),
        false,
    )
    .unwrap();
    let before = sink.actions.lock().unwrap().clone();
    assert!(
        sink.transform_build_info(request(BuildInfoTransformKind::Info, "ordinary.h"))
            .is_err()
    );
    assert_eq!(before, *sink.actions.lock().unwrap());
}

#[test]
fn bare_native_and_user_infos_reuse_existing_lowering_without_promoting_structs() {
    let heap = FrozenHeap::new();
    let user = AnalysisValue::provider(ProviderOccurrence::new(
        ProviderIdentity::user(ProviderId::new("//:bare.bzl", "P").unwrap()),
        [("field", AnalysisValue::string("value"))],
    ));
    let native = AnalysisValue::provider(ProviderOccurrence::new(
        ProviderIdentity::builtin("ToolchainInfo"),
        [("field", AnalysisValue::string("value"))],
    ));
    for owned in [&user, &native] {
        let value = AnalysisValueMaterializer::new(&heap).value(owned).unwrap();
        let bare = returned_providers(value.to_value());
        assert_eq!(bare.len(), 1);
        let lowered = AnalysisValueLowerer::default()
            .lower(bare[0], "$[0]")
            .unwrap();
        assert!(matches!(lowered.kind(), AnalysisValueKind::Provider(_)));
        assert!(owned.publication_eq(&lowered));
    }
    assert!(
        returned_providers(Value::new_none()).is_empty(),
        "bare None is an empty provider return"
    );
    let list_none = AnalysisValue::list(vec![AnalysisValue::none()]);
    let list_none = AnalysisValueMaterializer::new(&heap)
        .value(&list_none)
        .unwrap();
    let contained_none = returned_providers(list_none.to_value());
    assert_eq!(contained_none.len(), 1);
    assert!(contained_none[0].is_none());
    let lowered = AnalysisValueLowerer::default()
        .lower(contained_none[0], "$[0]")
        .unwrap();
    assert!(
        !matches!(lowered.kind(), AnalysisValueKind::Provider(_)),
        "None inside a provider list remains an invalid candidate"
    );
    for owned in [
        AnalysisValue::strukt([("field", AnalysisValue::string("value"))]),
        AnalysisValue::string("not provider"),
    ] {
        let value = AnalysisValueMaterializer::new(&heap).value(&owned).unwrap();
        let lowered = AnalysisValueLowerer::default()
            .lower(returned_providers(value.to_value())[0], "$[0]")
            .unwrap();
        assert!(
            !matches!(lowered.kind(), AnalysisValueKind::Provider(_)),
            "normalization cannot give non-Info a provider identity"
        );
    }
}

#[test]
fn actual_build_info_outputs_retain_policy_and_owner_after_heap_drop_freeze_and_gc() {
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@external+//pkg:translator").unwrap(),
        configuration(),
    );
    let (expected, retained) = {
        let sink = declaration_sink(&key);
        let stable = sink
            .transform_build_info(request(BuildInfoTransformKind::Info, "stable.h"))
            .unwrap();
        let constant = sink
            .transform_build_info(request(BuildInfoTransformKind::Version, "volatile.h"))
            .unwrap();
        let expected = AnalysisValue::tuple(vec![
            AnalysisValue::artifact(stable.artifact().clone()),
            AnalysisValue::artifact(constant.artifact().clone()),
        ]);
        let producer = starlark::environment::Module::new();
        let allocated = AnalysisValueMaterializer::new(producer.frozen_heap())
            .value(&expected)
            .unwrap();
        producer.set("outputs", allocated.to_value());
        let retained = AnalysisValueLowerer::from_module(&producer)
            .lower(allocated.to_value(), "$producer.outputs")
            .unwrap();
        assert!(expected.publication_eq(&retained));
        drop(producer.freeze().unwrap());
        (expected, retained)
    }; // The real sink and producer heap are both disposed before restoration.
    let restored = starlark::environment::Module::new();
    let allocated = AnalysisValueMaterializer::new(restored.frozen_heap())
        .value(&retained)
        .unwrap();
    restored.set("outputs", allocated.to_value());
    let mut gc = starlark::assert::Assert::new();
    gc.disable_static_typechecking();
    gc.module_add("retained", restored.freeze().unwrap());
    // Assert runs its supported GC modes; garbage_collect is invoked at a safe
    // Starlark statement boundary in a fresh evaluator, not after its frame exits.
    let after_gc = gc.pass(
        r#"
load("retained", "outputs")
stable, constant = outputs
lookup = {stable: "stable", constant: "constant"}
garbage_collect()
assert_eq(type(stable), "File")
assert_eq(type(constant), "File")
assert_eq(stable.root, constant.root)
assert_eq(stable.owner, constant.owner)
assert_eq(stable.is_directory, False)
assert_eq(constant.is_directory, False)
assert_eq(stable.path.endswith("/external/external+/pkg/stable.h"), True)
assert_eq(constant.path.endswith("/external/external+/pkg/volatile.h"), True)
assert_eq(lookup[stable], "stable")
assert_eq(lookup[constant], "constant")
outputs
"#,
    );
    let lowered = AnalysisValueLowerer::default()
        .lower(after_gc.value(), "$after_gc.outputs")
        .unwrap();
    assert!(expected.publication_eq(&lowered));
    let AnalysisValueKind::Tuple(outputs) = lowered.kind() else {
        panic!("output tuple")
    };
    for (value, metadata) in outputs.iter().zip([
        ActionOutputMetadata::Ordinary,
        ActionOutputMetadata::Constant,
    ]) {
        let AnalysisValueKind::Artifact(AnalysisArtifact::Derived { owner, output }) = value.kind()
        else {
            panic!("derived output File")
        };
        assert_eq!(owner, &key.artifact_owner());
        assert_eq!(output.metadata(), metadata);
        assert_eq!(output.kind(), ActionOutputKind::File);
        assert_eq!(
            output.root().configuration(),
            key.configuration().slug_configuration().unwrap()
        );
    }
}
