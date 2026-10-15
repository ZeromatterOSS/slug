use slug_build_api_v2::published_value::BYTE_LIMIT;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;

use super::*;
use crate::attrs::owner_metadata as metadata;
use crate::bzl_module::semantic_evaluation::budget::Budget;
use crate::source_table::Rows;

fn retained_transition(
    raw: Rows,
) -> (
    starlark::environment::FrozenModule,
    LoadingTransitionDefinition,
) {
    let identity = raw[0].1.identity.clone();
    let context = BzlEvaluationContext::macro_runtime_context(identity.clone(), raw.clone());
    let module = Module::new();
    let manifest = crate::BzlLoadManifest {
        root: identity,
        direct_children: Arc::from([]),
        reachable: Arc::from([raw[0].1.clone()]),
        fingerprint: [3; 32],
    };
    crate::starlark_label::attach_defining_module_manifest(&module, &manifest).unwrap();
    let ast=AstModule::parse(raw[0].0.as_str(),"def impl(settings, attr): return {}\nT=transition(implementation=impl,inputs=[],outputs=[])\n".into(),&Dialect::Standard).unwrap();
    {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        evaluator.eval_module(ast, &loading_globals()).unwrap();
    }
    let frozen = module.freeze().unwrap();
    let definition = frozen
        .get("T")
        .unwrap()
        .downcast::<FrozenTransitionDefinition>()
        .unwrap();
    let value = LoadingTransitionDefinition::with_source_table(
        definition.implementation,
        definition.inputs.clone(),
        definition.outputs.clone(),
        definition.definition_source.clone(),
        definition
            .source_identities_by_filename
            .capture(&mut Budget::default())
            .unwrap(),
    );
    drop(definition);
    (frozen, value)
}
struct LoadedTransition(starlark::environment::FrozenModule);
impl starlark::eval::FileLoader for LoadedTransition {
    fn load(&self, path: &str) -> starlark::Result<starlark::environment::FrozenModule> {
        use dupe::Dupe;
        assert_eq!(path, ":transition.bzl");
        Ok(self.0.dupe())
    }
}
fn foreign_rule(
    raw: Rows,
    child: starlark::environment::FrozenModule,
) -> starlark::environment::FrozenModule {
    let context =
        BzlEvaluationContext::macro_runtime_context(raw[0].1.identity.clone(), raw.clone());
    let module = Module::new();
    let manifest = crate::BzlLoadManifest {
        root: raw[0].1.identity.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([raw[0].1.clone()]),
        fingerprint: [3; 32],
    };
    crate::starlark_label::attach_defining_module_manifest(&module, &manifest).unwrap();
    let loader = LoadedTransition(child);
    let ast=AstModule::parse(raw[0].0.as_str(),"load(\":transition.bzl\", \"T\")\ndef impl(ctx): pass\nR=rule(implementation=impl,cfg=T)\n".into(),&Dialect::Standard).unwrap();
    {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        evaluator.set_loader(&loader);
        evaluator.eval_module(ast, &loading_globals()).unwrap();
    }
    module.freeze().unwrap()
}
fn raw(spare: bool) -> Rows {
    let mut name = CompactString::with_capacity(if spare { 4096 } else { 24 });
    name.push_str("//:defs.bzl");
    let mut path = std::path::PathBuf::with_capacity(if spare { 4096 } else { 11 });
    path.push("//:defs.bzl");
    let mut a = String::with_capacity(if spare { 4096 } else { 1 });
    a.push('a');
    let mut c = String::with_capacity(if spare { 4096 } else { 2 });
    c.push_str("c+");
    Arc::from([(
        name,
        BzlModuleSourceProvenance::new(
            BzlModuleIdentity {
                label: CanonicalLabel::parse("@@//:defs.bzl").unwrap(),
                source: slug_build_api_v2::ModuleSourceOrigin::Workspace(path),
                repository_mapping: Arc::from([(
                    slug_identity_v2::ApparentRepoName::new(a).unwrap(),
                    slug_identity_v2::CanonicalRepoName::new(c).unwrap(),
                )]),
            },
            [9; 32],
        ),
    )])
}
fn charges(value: &LoadingTransitionDefinition) -> Vec<slug_build_api_v2::DepsetStorageCharge> {
    let mut rows = Vec::new();
    metadata::transition(value, &mut |c| {
        rows.push(c);
        Ok(())
    })
    .unwrap();
    rows
}
#[test]
fn foreign_nested_table_and_empty_table_keep_complete_positive_envelopes() {
    let ordinary = raw(false);
    let foreign = raw(true);
    assert_eq!(ordinary, foreign);
    let weak = Arc::downgrade(&foreign);
    let (module, value) = retained_transition(foreign.clone());
    let (other_module, other) = retained_transition(ordinary.clone());
    assert_eq!(value, other);
    use dupe::Dupe;
    let rule_module = foreign_rule(ordinary.clone(), module.dupe());
    let rule = rule_module
        .get("R")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    let nested = rule.incoming_transition.as_ref().unwrap();
    assert_eq!(
        rule.source_identities_by_filename.raw(),
        nested.source_identities_by_filename.raw()
    );
    assert!(
        !Arc::ptr_eq(
            rule.source_identities_by_filename.raw(),
            nested.source_identities_by_filename.raw()
        ),
        "actual foreign transition guard survives rule capture/freeze"
    );
    assert!(Arc::ptr_eq(
        nested.source_identities_by_filename.raw(),
        value.certified_source_table().raw()
    ));
    drop(rule);
    drop(rule_module);

    let actual = charges(&value);
    let plain = charges(&other);
    let body = value.certified_source_table().declarations()[0].retained_shape();
    let ordinary_body = other.certified_source_table().declarations()[0].retained_shape();
    // Original rows remain byte-exact, then pair/declaration controls and full body.
    let mut old = Vec::new();
    metadata::row::<LoadingTransitionDefinition>(&mut |c| {
        old.push(c);
        Ok(())
    })
    .unwrap();
    metadata::source(value.definition_source(), &mut |c| {
        old.push(c);
        Ok(())
    })
    .unwrap();
    metadata::sources(value.source_identities_by_filename(), &mut |c| {
        old.push(c);
        Ok(())
    })
    .unwrap();
    for (a, b) in actual.iter().zip(&old) {
        assert_eq!(
            (a.nodes, a.edges, a.bytes, a.visits),
            (b.nodes, b.edges, b.bytes, b.visits)
        );
    }
    assert_eq!(actual.len(), old.len() + 3);
    let allowance = 64 + std::mem::size_of::<usize>();
    assert_eq!(
        actual[old.len()].bytes,
        std::mem::size_of::<crate::source_table::CertifiedSourceTable>() + allowance
    );
    assert_eq!(
        actual[old.len() + 1].bytes,
        std::mem::size_of::<slug_build_api_v2::published_value::SharedPublicationInput>()
            + allowance
    );
    assert_eq!(
        (actual.last().unwrap().bytes, actual.last().unwrap().visits),
        (body.bytes, body.visits)
    );
    assert_eq!(body.depth, 5);
    assert_eq!(
        actual.iter().map(|c| c.bytes).sum::<usize>()
            - plain.iter().map(|c| c.bytes).sum::<usize>(),
        body.bytes - ordinary_body.bytes
    );
    let count = Arc::strong_count(&foreign);
    let total = actual.iter().map(|c| c.bytes).sum::<usize>();
    for extra in [0, 1] {
        let mut audit = PublishedValueArenaBuilder::new();
        audit
            .charge_bytes(BYTE_LIMIT - audit.local_shape().bytes - total + extra)
            .unwrap();
        let result = metadata::transition(&value, &mut |c| {
            audit.charge_storage(c).map_err(|e| e.to_string())
        });
        assert_eq!(result.is_ok(), extra == 0);
        assert_eq!(
            Arc::strong_count(&foreign),
            count,
            "metadata refusal cannot clone/retain a guard"
        );
    }
    let empty = LoadingTransitionDefinition::new(
        value.implementation(),
        Arc::from([]),
        Arc::from([]),
        value.definition_source().clone(),
        Arc::from([]),
    )
    .unwrap();
    let empty_rows = charges(&empty);
    let empty_full = empty.certified_source_table().declarations()[0].retained_shape();
    assert_eq!(
        (empty_full.visits, empty_full.edges, empty_full.depth),
        (6, 2, 5)
    );
    assert_eq!(empty_rows.last().unwrap().bytes, empty_full.bytes);
    let top = BzlEvaluationContext::new("//:top.bzl")
        .source_table()
        .unwrap();
    let mut depth = PublishedValueArenaBuilder::new();
    depth
        .audit_shared_input_at_depth(&top.declarations()[0], 1)
        .unwrap();
    assert_eq!(
        depth.local_shape().depth,
        6,
        "even an empty top guard covers the composite nested table depth"
    );
    drop(module);
    drop(other_module);
    drop(foreign);
    drop(empty);
    assert!(weak.upgrade().is_some());
    drop(value);
    assert!(weak.upgrade().is_none());
}
