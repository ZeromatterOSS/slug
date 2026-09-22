//! Owned native-provider bridge; constructor ABI has Loading/integration tests.
use slug_build_api_v2::ProviderId;
use slug_build_api_v2::TemplateVariableInfo;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

fn template() -> TemplateVariableInfo {
    TemplateVariableInfo::new(
        [("Z", "last"), ("λ", "雪")]
            .into_iter()
            .map(|(key, value)| (CompactString::new(key), CompactString::new(value)))
            .collect(),
    )
}

fn graph(info: TemplateVariableInfo, nested: TemplateVariableInfo) -> AnalysisValue {
    let carrier = ProviderOccurrence::new(
        ProviderIdentity::user(ProviderId::new("//:template.bzl", "Carrier").unwrap()),
        [(
            "template",
            AnalysisValue::template_variable_info(nested.dupe()),
        )],
    );
    let target = ConfiguredTargetValue::new(
        AnalysisTargetIdentity::null(CanonicalLabel::parse("@@//:target").unwrap()),
        ProviderCollection::from_values(
            vec![
                ProviderValue::TemplateVariableInfo(info.dupe()),
                ProviderValue::Occurrence(carrier.clone()),
            ],
            false,
        )
        .unwrap(),
    );
    AnalysisValue::tuple(vec![
        AnalysisValue::configured_target(target),
        AnalysisValue::template_variable_info(info),
        AnalysisValue::provider(carrier),
        AnalysisValue::list(vec![AnalysisValue::template_variable_info(nested)]),
    ])
}

#[test]
fn template_variable_roundtrip_retains_cross_provider_aliases_after_freeze_gc_and_heap_drop() {
    let retained = {
        let heap = FrozenHeap::new();
        let info = template();
        let expected = graph(info.dupe(), info);
        let allocated = AnalysisValueMaterializer::new(&heap)
            .value(&expected)
            .unwrap();
        let lowered = AnalysisValueLowerer::default()
            .lower(allocated.to_value(), "$source")
            .unwrap();
        assert!(expected.publication_eq(&lowered));
        lowered
    };
    let module = Module::new();
    let restored = AnalysisValueMaterializer::new(module.frozen_heap())
        .value(&retained)
        .unwrap();
    module.set("values", restored.to_value());
    module.set(
        "Template",
        slug_loading_v2::provider::alloc_starlark_provider_callable(
            module.frozen_heap(),
            "TemplateVariableInfo",
        )
        .unwrap()
        .to_value(),
    );
    let AnalysisValueKind::Tuple(elements) = retained.kind() else {
        panic!("tuple")
    };
    let AnalysisValueKind::TemplateVariableInfo(info) = elements[1].kind() else {
        panic!("template")
    };
    // A second facade allocation must not issue a new native occurrence.
    module.set(
        "same",
        slug_loading_v2::alloc_frozen_template_variable_info(module.frozen_heap(), info.dupe())
            .to_value(),
    );
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                "template.bzl",
                r#"
target, direct, carrier, nested = values
copy = direct.variables
copy["Z"] = "mutated"
RESULT = [target[Template] == direct, carrier.template == direct, nested[0] == direct,
          direct == same, same == direct, direct.variables["Z"], copy["Z"]]
"#
                .to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::standard(),
        )
        .unwrap();
    assert_eq!(
        module.get("RESULT").unwrap().to_repr(),
        "[True, True, True, True, True, \"last\", \"mutated\"]"
    );
    let lowered = AnalysisValueLowerer::default()
        .lower(restored.to_value(), "$restored")
        .unwrap();
    assert!(retained.publication_eq(&lowered));
    let frozen = module.freeze().unwrap();
    let mut gc = starlark::assert::Assert::new();
    gc.disable_static_typechecking();
    gc.module_add("retained", frozen);
    gc.pass(
        r#"
load("retained", "values", "same", "Template")
target, direct, carrier, nested = values
fresh = Template(vars = direct.variables)
lookup = {direct: "kept", fresh: "fresh"}
copied = direct.variables
copied["Z"] = "fresh evaluator mutation"
garbage_collect()
assert_eq(Template in target, True)
assert_eq(target[Template], carrier.template)
assert_eq(nested[0], same)
assert_eq(same, nested[0])
assert_eq(direct == fresh, False)
assert_eq(len(lookup), 2)
assert_eq(lookup[same], "kept")
assert_eq(direct.variables.keys(), ["Z", "λ"])
assert_eq(direct.variables, {"Z": "last", "λ": "雪"})
assert_eq(copied["Z"], "fresh evaluator mutation")
"#,
    );
}

#[test]
fn template_variable_publication_compares_alias_topology_across_native_and_nested_roots() {
    let a = template();
    let original = graph(a.dupe(), a);
    let b = template();
    let equivalent = graph(b.dupe(), b);
    assert!(original.publication_eq(&equivalent));
    let split = graph(template(), template());
    assert!(!original.publication_eq(&split));
    assert!(!split.publication_eq(&original));
    let heap = FrozenHeap::new();
    for value in [&original, &equivalent, &split] {
        let materialized = AnalysisValueMaterializer::new(&heap).value(value).unwrap();
        let lowered = AnalysisValueLowerer::default()
            .lower(materialized.to_value(), "$roundtrip")
            .unwrap();
        assert!(value.publication_eq(&lowered));
    }
}
