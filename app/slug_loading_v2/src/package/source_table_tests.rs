//! Real definition allocation/freeze preserves the original producer pair.
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
#[test]
fn two_rule_definitions_freeze_with_one_producer_guard_and_release_last_raw_table() {
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@//:defs.bzl").unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace("//:defs.bzl".into()),
        repository_mapping: Arc::from([]),
    };
    let raw: Arc<[(CompactString, BzlModuleSourceProvenance)]> = Arc::from([(
        "//:defs.bzl".into(),
        BzlModuleSourceProvenance::new(identity.clone(), [9; 32]),
    )]);
    let weak = Arc::downgrade(&raw);
    let context = BzlEvaluationContext::macro_runtime_context(identity, raw.clone());
    assert_eq!(
        Arc::strong_count(&raw),
        2,
        "caller and one pure descriptor; pair owns typed descriptor handles"
    );
    let module = Module::new();
    let manifest = crate::BzlLoadManifest {
        root: context.source_identity().clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([raw[0].1.clone()]),
        fingerprint: [3; 32],
    };
    crate::starlark_label::attach_defining_module_manifest(&module, &manifest).unwrap();
    let ast = AstModule::parse(
        "//:defs.bzl",
        "def impl(ctx): pass\nR = rule(implementation=impl)\nS = rule(implementation=impl)\n"
            .into(),
        &Dialect::Standard,
    )
    .unwrap();
    {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        evaluator.eval_module(ast, &loading_globals()).unwrap();
    }
    assert_eq!(
        Arc::strong_count(&raw),
        2,
        "definitions clone the typed pair, never the raw backing"
    );
    let frozen = module.freeze().unwrap();
    assert_eq!(
        Arc::strong_count(&raw),
        2,
        "freeze transfers the typed pairs"
    );
    let r = frozen
        .get("R")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    let s = frozen
        .get("S")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    assert!(Arc::ptr_eq(
        r.source_identities_by_filename.raw(),
        s.source_identities_by_filename.raw()
    ));
    assert!(Arc::ptr_eq(r.source_identities_by_filename.raw(), &raw));
    assert_eq!(
        r.source_identities_by_filename.descriptor_strong_count(),
        6,
        "context plus two definitions each retain two descriptor handles"
    );
    let retained = r
        .source_identities_by_filename
        .capture(&mut crate::bzl_module::semantic_evaluation::budget::Budget::default())
        .unwrap();
    drop(r);
    drop(s);
    drop(frozen);
    drop(context);
    drop(raw);
    assert!(weak.upgrade().is_some());
    let guard = retained.declarations()[0].clone();
    drop(retained);
    assert!(
        weak.upgrade().is_some(),
        "last standalone raw guard keeps the source body"
    );
    drop(guard);
    assert!(weak.upgrade().is_none());
}

#[path = "source_table_tests/nested.rs"]
mod nested;
