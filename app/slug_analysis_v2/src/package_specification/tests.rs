use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::PackageGroupContentsInput;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::analysis_value::AnalysisValueLowerer;
use crate::analysis_value::AnalysisValueMaterializer;

fn info(package: &str) -> PackageSpecificationInfo {
    let label = CanonicalLabel::parse(package).unwrap();
    PackageSpecificationInfo::new(
        Arc::new(PackageGroupContents::new(PackageGroupContentsInput {
            exact_positive: [label.package().clone()].into_iter().collect(),
            ..Default::default()
        })),
        [],
    )
    .unwrap()
}

fn evaluate(module: &Module, source: &str) -> Result<(), String> {
    Evaluator::new(module)
        .eval_module(
            AstModule::parse(
                "package_specification_test.bzl",
                source.to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::standard(),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[test]
fn native_membership_uses_canonical_packages_but_equality_excludes_callable_fields() {
    let heap = FrozenHeap::new();
    let module = Module::new();
    module.set("main", alloc(&heap, info("@@//p:one")).to_value());
    module.set(
        "external",
        alloc(&heap, info("@@actual+//p:one")).to_value(),
    );
    module.set(
        "external_root",
        alloc(&heap, info("@@actual+//:actual+")).to_value(),
    );
    module.set(
        "label",
        slug_loading_v2::provider::alloc_frozen_starlark_label(
            &heap,
            CanonicalLabel::parse("@@actual+//p:other").unwrap(),
        )
        .to_value(),
    );
    evaluate(
        &module,
        r#"
lookup = {main: 1}
lookup[external] = 2
RESULT = [type(main), repr(main), dir(main), main.contains("//p:other"),
          main.contains("//other:x"), external.contains("@actual+//p:x"),
          external.contains("@@actual+//p:x"), external.contains(label),
          external.contains("@apparent//p:x"), external.contains("//p:x"),
          main == external, len(lookup), lookup[main]]
"#,
    )
    .unwrap();
    assert_eq!(
        module.get("RESULT").unwrap().to_repr(),
        "[\"PackageSpecificationInfo\", \"struct()\", [\"contains\"], True, False, True, True, True, False, False, True, 1, 2]"
    );
    evaluate(&module, "SPELLINGS = [main.contains('//p'), external.contains('@actual+//p'), external_root.contains('@actual+'), external_root.contains('@@actual+')]").unwrap();
    assert_eq!(
        module.get("SPELLINGS").unwrap().to_repr(),
        "[True, True, True, True]"
    );
    for source in [
        "RESULT = main.contains()",
        "RESULT = main.contains('//p:x', '//p:y')",
        "RESULT = main.contains(target='//p:x')",
        "RESULT = main.contains(1)",
        "RESULT = main.contains(None)",
        "RESULT = main.contains(':relative')",
        "RESULT = main.contains('p:relative')",
        "RESULT = main.contains('//p/...')",
        "RESULT = main.contains('//p/...:target')",
        "RESULT = main.contains('@actual+//p/...')",
        "RESULT = main()",
        "main.contains = None",
    ] {
        assert!(
            evaluate(&module, source).is_err(),
            "unexpected admission: {source}"
        );
    }
}

#[test]
fn frozen_nested_publication_preserves_membership_beyond_native_equality() {
    let retained = {
        let heap = FrozenHeap::new();
        let module = Module::new();
        module.set("a", alloc(&heap, info("@@//a:x")).to_value());
        module.set("b", alloc(&heap, info("@@//b:x")).to_value());
        evaluate(&module, "RESULT = [a, (b,), {'a': a, 'b': b}]").unwrap();
        // Borrowed frozen values are retained through the owning heap reference.
        module.frozen_heap().add_reference(&heap.into_ref());
        let module = module.freeze().unwrap();
        let result = module.get("RESULT").unwrap();
        AnalysisValueLowerer::default()
            .lower(result.value(), "$result")
            .unwrap()
    };
    let heap = FrozenHeap::new();
    let restored = AnalysisValueMaterializer::new(&heap)
        .value(&retained)
        .unwrap();
    let module = Module::new();
    module.set("values", restored.to_value());
    evaluate(&module, "RESULT = [values[0].contains('//a:other'), values[1][0].contains('//b:other'), values[2]['a'] == values[2]['b']]").unwrap();
    assert_eq!(
        module.get("RESULT").unwrap().to_repr(),
        "[True, True, True]"
    );
    let lowered = AnalysisValueLowerer::default()
        .lower(restored.to_value(), "$restored")
        .unwrap();
    assert!(retained.publication_eq(&lowered));
    let a = AnalysisValue::package_specification_info(info("@@//a:x"));
    let b = AnalysisValue::package_specification_info(info("@@//b:x"));
    assert_eq!(a, b);
    assert!(!a.publication_eq(&b));

    // Assert runs its supported collection modes, including a statement-safe
    // explicit collection. No evaluator GC is invoked outside an active frame.
    module.frozen_heap().add_reference(&heap.into_ref());
    let frozen = module.freeze().unwrap();
    let mut gc = starlark::assert::Assert::new();
    gc.disable_static_typechecking();
    gc.module_add("retained", frozen);
    gc.pass(
        r#"
load("retained", "values")
lookup = {values[0]: "first"}
lookup[values[1][0]] = "second"
garbage_collect()
assert_eq(type(values[0]), "PackageSpecificationInfo")
assert_eq(values[0].contains("//a:after_gc"), True)
assert_eq(values[0].contains("//b:after_gc"), False)
assert_eq(values[1][0].contains("//b:after_gc"), True)
assert_eq(values[2]["a"].contains("//a:after_gc"), True)
assert_eq(len(lookup), 1)
assert_eq(lookup[values[0]], "second")
"#,
    );
}
