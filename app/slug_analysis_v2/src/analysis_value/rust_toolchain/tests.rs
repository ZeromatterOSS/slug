use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::ConfiguredTargetValue;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::RustBzlModuleIdentity;
use slug_build_api_v2::RustBzlModuleSource;
use slug_build_api_v2::RustToolchainCallable;
use slug_build_api_v2::RustToolchainCallableModel;
use slug_build_api_v2::RustToolchainHelperManifest;
use slug_identity_v2::CanonicalLabel;

use super::*;

fn target(fields: Vec<(&str, AnalysisValue)>) -> AnalysisValue {
    AnalysisValue::configured_target(ConfiguredTargetValue::new(
        AnalysisConfiguredTargetKey::new(CanonicalLabel::parse("@@//:std").unwrap(), vec![1]),
        ProviderCollection::new(vec![
            ProviderValue::DefaultInfo(DefaultInfo::empty()),
            ProviderValue::Occurrence(ProviderOccurrence::new(
                ProviderIdentity::builtin("ToolchainInfo"),
                fields,
            )),
        ])
        .unwrap(),
    ))
}
fn authority(id: u8) -> Arc<RustToolchainCallableSourceAuthority> {
    let identity = RustBzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_rust+//rust/private:toolchain.bzl").unwrap(),
        workspace_path: "/workspace/toolchain.bzl".into(),
        repository_mapping: Arc::from([]),
    };
    Arc::new(RustToolchainCallableSourceAuthority {
        caller: RustBzlModuleSource {
            identity: identity.clone(),
            digest: [id; 32],
        },
        helper: RustToolchainHelperManifest {
            root: identity,
            direct_children: Arc::from([]),
            reachable: Arc::from([]),
            fingerprint: [id; 32],
        },
    })
}
fn callable(id: u8) -> AnalysisValue {
    AnalysisValue::rust_toolchain_callable(
        RustToolchainCallable::new(RustToolchainCallableModel {
            source: authority(id),
            identity: Arc::new(()),
            name_hash: 42,
            cc_toolchain: AnalysisValue::none(),
            feature_configuration: AnalysisValue::none(),
            experimental_link_std_dylib: AnalysisValue::boolean(false),
            rust_std: target(vec![]),
        })
        .unwrap(),
    )
}
fn depset(value: AnalysisValue) -> AnalysisValue {
    AnalysisValue::depset(AnalysisDepset::new(DepsetOrder::Default, vec![value], vec![]).unwrap())
}
#[test]
fn discovery_visits_all_container_provider_target_depset_and_cc_capture_roots() {
    use slug_build_api_v2::cc_toolchain_callable::CcToolchainCallable;
    use slug_build_api_v2::cc_toolchain_callable::CcToolchainCallableKind;
    use slug_build_api_v2::cc_toolchain_callable::CcToolchainCallableModel;
    use slug_build_api_v2::cc_toolchain_callable::CcToolchainCallableSource;
    use slug_build_api_v2::cc_variables::CcToolchainVariables;
    use slug_build_api_v2::cc_variables::CcVariableCacheEntry;
    use slug_build_api_v2::cc_variables::CcVariablesKind;
    use slug_build_api_v2::cc_variables::CcVariablesModel;
    use slug_build_api_v2::cc_variables::CcVariablesOccurrence;
    let cc = AnalysisValue::cc_toolchain_callable(
        CcToolchainCallable::new(CcToolchainCallableModel {
            kind: CcToolchainCallableKind::StaticRuntime,
            source: CcToolchainCallableSource {
                label: CanonicalLabel::parse("@@cc+//:helper.bzl").unwrap(),
                digest: [1; 32],
            },
            identity: Arc::new(()),
            name_hash: 42,
            capture: depset(callable(6)),
        })
        .unwrap(),
    );
    let parent = CcToolchainVariables::new(CcVariablesModel {
        kind: CcVariablesKind::Map,
        occurrence: CcVariablesOccurrence::new(),
        parent: None,
        locals: vec![("parent".into(), callable(7))].into_boxed_slice(),
        cache: Box::new([]),
    });
    let vars = AnalysisValue::cc_toolchain_variables(CcToolchainVariables::new(CcVariablesModel {
        kind: CcVariablesKind::Map,
        occurrence: CcVariablesOccurrence::new(),
        parent: Some(parent),
        locals: vec![("local".into(), callable(8))].into_boxed_slice(),
        cache: vec![("cache".into(), CcVariableCacheEntry::Value(callable(9)))].into_boxed_slice(),
    }));
    let child = match depset(callable(5)).kind() {
        AnalysisValueKind::Depset(child) => child.clone(),
        _ => unreachable!(),
    };
    let nested = AnalysisValue::rust_toolchain_callable(
        RustToolchainCallable::new(RustToolchainCallableModel {
            source: authority(10),
            identity: Arc::new(()),
            name_hash: 42,
            cc_toolchain: AnalysisValue::provider(ProviderOccurrence::new(
                ProviderIdentity::builtin("ToolchainInfo"),
                [("capture", callable(11))],
            )),
            feature_configuration: AnalysisValue::none(),
            experimental_link_std_dylib: AnalysisValue::boolean(false),
            rust_std: target(vec![("capture", callable(12))]),
        })
        .unwrap(),
    );
    let graph = AnalysisValue::tuple([
        AnalysisValue::list([callable(1)]),
        AnalysisValue::dictionary([(callable(2), callable(3))]).unwrap(),
        AnalysisValue::strukt([("nested", target(vec![("helper", callable(4))]))]),
        AnalysisValue::depset(
            AnalysisDepset::new(DepsetOrder::Default, vec![], vec![child.clone(), child]).unwrap(),
        ),
        cc,
        vars,
        callable(1),
        nested,
    ]);
    let mut collector = RustHelperAuthorities::default();
    collector.value(&graph);
    let mut ids = collector
        .into_authorities()
        .map(|source| source.caller.digest[0])
        .collect::<Vec<_>>();
    ids.sort();
    assert_eq!(ids, (1..=12).collect::<Vec<_>>());
}
#[test]
fn rematerialization_refuses_an_unprepared_authority_before_installing_code() {
    let heap = starlark::values::FrozenHeap::new();
    let error = crate::analysis_value::AnalysisValueMaterializer::new(&heap)
        .value(&callable(1))
        .unwrap_err();
    assert!(error.contains("freshly prepared helper binding"), "{error}");
}
#[test]
fn provenance_merge_refuses_filename_and_canonical_label_collisions() {
    let label = CanonicalLabel::parse("@@rules_rust+//:helper.bzl").unwrap();
    let source = BzlModuleSourceProvenance::new(
        slug_loading_v2::BzlModuleIdentity {
            label,
            workspace_path: "/workspace/helper.bzl".into(),
            repository_mapping: Arc::from([]),
        },
        [1; 32],
    );
    let row = (CompactString::new("helper.bzl"), source.clone());
    let same = merge_helper_sources(&[row.clone(), row.clone()], &[]).unwrap();
    assert_eq!(same.as_ref(), &[row.clone()]);
    let mut different = source.clone();
    different.source_digest[0] ^= 1;
    assert!(
        merge_helper_sources(&[row.clone(), ("different.bzl".into(), different)], &[])
            .unwrap_err()
            .contains("canonical source")
    );
    let mut different = source;
    different.identity.label = CanonicalLabel::parse("@@other+//:helper.bzl").unwrap();
    assert!(
        merge_helper_sources(&[row, ("helper.bzl".into(), different)], &[])
            .unwrap_err()
            .contains("filename")
    );
}

#[test]
fn registered_nested_function_lowers_four_captures_and_aliases_before_producer_disposal() {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;
    use starlark::values::ValueLike;
    use starlark::values::function::StarlarkFunctionIdentity;
    use starlark::values::tuple::AllocTuple;

    use crate::analysis_value::AnalysisValueLowerer;
    use crate::analysis_value::AnalysisValueMaterializer;
    let files = depset(AnalysisValue::artifact(
        slug_build_api_v2::AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//:runtime.a").unwrap(),
        ),
    ));
    let cc = AnalysisValue::provider(ProviderOccurrence::new(
        ProviderIdentity::builtin("ToolchainInfo"),
        [("files", files.clone())],
    ));
    let standard = target(vec![("files", files.clone())]);
    let retained = {
        let module = Module::new();
        let mut materializer = AnalysisValueMaterializer::new(module.frozen_heap());
        let cc_value = materializer.value(&cc).unwrap();
        let std_value = materializer.value(&standard).unwrap();
        module.set("cc_value", cc_value.to_value());
        Evaluator::new(&module).eval_module(AstModule::parse("authored_lowering.bzl", "def build():\n    capture = cc_value\n    def make_ccinfo(label, actions, allocator_library, std): return capture\n    return make_ccinfo\nf = build()\n".to_owned(), &Dialect::Bazel).unwrap(), &Globals::standard()).unwrap();
        let function = module.get("f").unwrap();
        let identity = function
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .unwrap();
        let token = identity.issue_identity();
        // Authentication/registry tracing are separate Loading gates. This
        // synthetic descriptor isolates Analysis's owned four-capture lowering.
        let registration = slug_loading_v2::RustCallableRegistration {
            source: authority(1),
            identity: token.clone(),
            name_hash: identity.name_hash().get(),
        };
        let captures = module.heap().alloc(AllocTuple([
            cc_value.to_value(),
            starlark::values::Value::new_none(),
            starlark::values::Value::new_bool(false),
            std_value.to_value(),
        ]));
        let files_value = materializer.value(&files).unwrap();
        let graph = module
            .heap()
            .alloc(AllocTuple([function, function, files_value.to_value()]));
        let mut lowerer = AnalysisValueLowerer::from_module(&module);
        lowerer
            .rust_callables
            .insert(function.identity(), (registration, captures));
        let retained = lowerer.lower(graph, "producer").unwrap();
        let AnalysisValueKind::Tuple(values) = retained.kind() else {
            panic!("tuple")
        };
        let AnalysisValueKind::RustToolchainCallable(first) = values[0].kind() else {
            panic!("callable")
        };
        assert!(Arc::ptr_eq(&token, &first.model().identity));
        retained
    };
    let AnalysisValueKind::Tuple(values) = retained.kind() else {
        panic!("tuple")
    };
    assert_eq!(values[0], values[1]);
    let AnalysisValueKind::RustToolchainCallable(callable) = values[0].kind() else {
        panic!("callable")
    };
    assert_eq!(callable.model().cc_toolchain, cc);
    assert_eq!(callable.model().rust_std, standard);
    assert!(matches!(
        callable.model().feature_configuration.kind(),
        AnalysisValueKind::None
    ));
    assert!(matches!(
        callable.model().experimental_link_std_dylib.kind(),
        AnalysisValueKind::Boolean(false)
    ));
    let AnalysisValueKind::Provider(cc) = callable.model().cc_toolchain.kind() else {
        panic!("provider")
    };
    assert_eq!(cc.field("files").unwrap(), &values[2]);
    let AnalysisValueKind::Depset(files) = values[2].kind() else {
        panic!("depset")
    };
    assert_eq!(files.to_list().len(), 1);
}
