use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::eval::inspect_function_symbol;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

#[test]
fn owner_functions_unwrap_only_actual_rule_and_transitive_subrule_implementations() {
    let module = Module::new();
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                "unrelated.bzl",
                "def first(): return 41\ndef second(): return 42\n".to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::standard(),
        )
        .unwrap();
    let frozen = module.freeze().unwrap();
    let first = frozen.get("first").unwrap();
    let second = frozen.get("second").unwrap();
    let first = first.value().unpack_frozen().unwrap();
    let second = second.value().unpack_frozen().unwrap();
    assert_eq!(owner_rule_function(first), Some(first));
    let heap = starlark::values::FrozenHeap::new();
    let wrapped = heap.alloc_simple(DeferredSubruleRuleImplementation {
        implementation: first,
        first_subrule: Some("parent".into()),
        first_late_bound_attribute: None,
    });
    assert_eq!(owner_rule_function(wrapped), Some(first));
    let subrule = |name: &str, implementation, children: Arc<[FrozenValue]>| {
        heap.alloc_simple(FrozenSubruleDefinition {
            implementation,
            identity: Arc::new(SubruleIdentity {
                defining_label: CanonicalLabel::parse("@@//:unrelated.bzl").unwrap(),
                exported_name: name.into(),
            }),
            attributes: Arc::from([]),
            toolchains: Arc::from([]),
            fragments: Arc::new(SmallSet::new()),
            direct_subrules: children,
        })
    };
    let leaf = subrule("leaf", second, Arc::from([]));
    let parent = subrule("parent", first, Arc::from([leaf]));
    assert_eq!(owner_subrule_function(parent), Some(first));
    let child = parent
        .to_value()
        .downcast_ref::<FrozenSubruleDefinition>()
        .unwrap()
        .direct_subrules[0];
    assert_eq!(owner_subrule_function(child), Some(second));
    assert_ne!(
        inspect_function_symbol(first.to_value())
            .unwrap()
            .creation_ordinal(),
        inspect_function_symbol(second.to_value())
            .unwrap()
            .creation_ordinal()
    );
    assert!(owner_rule_function(leaf).is_none());
    assert!(owner_subrule_function(first).is_none());
    let unknown = heap.alloc("not a function");
    assert!(owner_rule_function(unknown).is_none());
    assert!(owner_subrule_function(unknown).is_none());
    let module = Module::new();
    let error = Evaluator::new(&module)
        .eval_function(wrapped.to_value(), &[], &[])
        .unwrap_err();
    assert!(error.to_string().contains("subrule 'parent'"));
}
