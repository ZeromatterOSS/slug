//! Real same-module registration and metadata transactions; no recipe token mint.
use slug_build_api_v2::ActionOutputMetadata;
use slug_build_api_v2::BuildInfoTransformHeader;
use slug_build_api_v2::BuildInfoTransformKind;
use slug_build_api_v2::ProviderId;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::RegisterableAction;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::FrozenHeap;

use super::*;
use crate::starlark_rule::args_callbacks::Publisher;
use crate::starlark_rule::args_callbacks::pending::PendingRegisteredAction;
use crate::starlark_rule::context_lifecycle_tests::configuration;
use crate::starlark_rule::context_lifecycle_tests::declaration_sink;
fn callback(module: &Module) -> Value<'_> {
    Evaluator::new(module)
        .eval_module(
            AstModule::parse(
                "genuine_original.bzl",
                "def original(s): return {'@':'actual'}\n".to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::standard(),
        )
        .unwrap();
    module.get("original").unwrap()
}
fn request<'v>(
    module: &'v Module,
    kind: BuildInfoTransformKind,
    name: &'v str,
) -> BuildInfoTransformRequest<'v> {
    BuildInfoTransformRequest {
        module,
        kind,
        callback: slug_loading_v2::build_info_transform::register_callback(
            module,
            callback(module),
        )
        .unwrap(),
        template: AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//:unrelated.template").unwrap(),
        ),
        output_file_name: name,
    }
}
fn snapshot(sink: &SynchronousAnalysisActionSink) -> Vec<(Vec<ActionOutput>, String)> {
    sink.actions
        .lock()
        .unwrap()
        .registry()
        .actions()
        .iter()
        .map(|a| (a.outputs().to_vec(), a.mnemonic().to_owned()))
        .collect()
}
#[test]
fn actual_sink_preserves_status_output_metadata_order_and_failed_registration_atomicity() {
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@external+//pkg:translator").unwrap(),
        configuration(),
    );
    let sink = declaration_sink(&key);
    let module = Module::new();
    let volatile = sink
        .transform_build_info(request(
            &module,
            BuildInfoTransformKind::Version,
            "volatile.h",
        ))
        .unwrap();
    let stable = sink
        .transform_build_info(request(&module, BuildInfoTransformKind::Info, "stable.h"))
        .unwrap();
    for (artifact, metadata) in [
        (volatile.artifact(), ActionOutputMetadata::Constant),
        (stable.artifact(), ActionOutputMetadata::Ordinary),
    ] {
        let AnalysisArtifact::Derived { output, .. } = artifact else {
            panic!("derived")
        };
        assert_eq!(output.metadata(), metadata);
    }
    let before = snapshot(&sink);
    assert_eq!(before.len(), 2);
    assert!(before.iter().all(|a| a.1 == "TranslateBuildInfo"));
    let declarations = sink.declared_outputs.lock().unwrap().clone();
    for (kind, name) in [
        (BuildInfoTransformKind::Info, "volatile.h"),
        (BuildInfoTransformKind::Version, "stable.h"),
        (BuildInfoTransformKind::Version, "volatile.h"),
        (BuildInfoTransformKind::Info, "../bad"),
        (BuildInfoTransformKind::Info, ""),
    ] {
        assert!(
            sink.transform_build_info(request(&module, kind, name))
                .is_err()
        );
        assert_eq!(before, snapshot(&sink));
        assert_eq!(declarations, *sink.declared_outputs.lock().unwrap());
    }
    assert!(sink.declare_file("volatile.h", None).is_err());
    sink.declare_directory("tree.h").unwrap();
    assert!(
        sink.transform_build_info(request(&module, BuildInfoTransformKind::Version, "tree.h"))
            .is_err()
    );
    let ordinary = sink.declare_file("ordinary.h", None).unwrap();
    let heap = module.frozen_heap();
    sink.write(
        &module,
        heap.alloc(ordinary).to_value(),
        heap.alloc("body").to_value(),
        false,
    )
    .unwrap();
    let before = snapshot(&sink);
    assert!(
        sink.transform_build_info(request(&module, BuildInfoTransformKind::Info, "ordinary.h"))
            .is_err()
    );
    assert_eq!(before, snapshot(&sink));
}
#[tokio::test]
async fn build_info_callback_and_provider_alias_use_same_publication_graph_and_original_slot() {
    let loaded = super::super::args_callbacks::LoadedSource::new(
        "def original(s): return {'@':'actual'}\ndef entry(ctx): return original\n",
    )
    .await;
    let module = Module::new();
    let original = loaded.invoke(&module, Value::new_none());
    let token =
        slug_loading_v2::build_info_transform::register_callback(&module, original).unwrap();
    let resource_token =
        slug_loading_v2::resource_set::register_resource_set(&module, original).unwrap();
    let mut publisher = Publisher::with_resource_sets(&module).unwrap();
    let mut lowerer = AnalysisValueLowerer::from_module(&module);
    let first = publisher.capture(original, &mut lowerer).unwrap();
    let draft = prepare(&mut publisher, &token, &mut lowerer).unwrap();
    let resource_draft =
        super::super::resource_set::prepare(&mut publisher, &resource_token, &mut lowerer).unwrap();
    let graph = publisher.seal(&mut lowerer).unwrap();
    drop(lowerer);
    let frozen = module.freeze().unwrap();
    let roots = frozen.owned_extra_value().unwrap();
    let publisher = super::super::args_callbacks::FrozenPublisher::new(graph, roots).unwrap();
    let retained = complete(&publisher, draft).unwrap();
    let (same, _) = publisher.resource_original(first).unwrap();
    let resource = super::super::resource_set::complete(&publisher, resource_draft).unwrap();
    for other in [&same, resource.published_callable()] {
        assert!(retained.published_callable().same_original(other));
        assert_eq!(retained.published_callable().node_id(), other.node_id());
        assert_eq!(
            retained.published_callable().original_slot(),
            other.original_slot()
        );
        assert!(std::ptr::eq(
            retained.published_callable().node(),
            other.node()
        ));
    }
    struct Context;
    impl slug_build_api_v2::BuildInfoEvaluationContext for Context {
        fn is_cancelled(&self) -> bool {
            false
        }
        fn print(&self, _: &str) -> Result<(), slug_build_api_v2::BuildInfoTransformError> {
            Ok(())
        }
        fn mnemonic(&self) -> &str {
            "publication"
        }
    }
    drop(frozen);
    drop(publisher);
    drop(loaded);
    assert_eq!(retained.render(&[], b"@", &Context).unwrap(), b"actual");
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
        let registration = Module::new();
        let stable = sink
            .transform_build_info(request(
                &registration,
                BuildInfoTransformKind::Info,
                "stable.h",
            ))
            .unwrap();
        let constant = sink
            .transform_build_info(request(
                &registration,
                BuildInfoTransformKind::Version,
                "volatile.h",
            ))
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
