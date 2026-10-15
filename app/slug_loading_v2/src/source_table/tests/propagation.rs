use slug_build_api_v2::published_value::PublishedValueArenaBuilder;

use super::*;
use crate::bzl_environment::BzlEnvironmentMode;
use crate::bzl_environment::PreparedBzlEnvironment;
fn manifest(raw: &Rows) -> crate::BzlLoadManifest {
    crate::BzlLoadManifest {
        root: raw[0].1.identity.clone(),
        direct_children: Arc::from([]),
        reachable: raw
            .iter()
            .map(|(_, p)| p.clone())
            .collect::<Vec<_>>()
            .into(),
        fingerprint: [8; 32],
    }
}
fn audit_after(first: &CertifiedSourceTable, second: &CertifiedSourceTable) -> usize {
    let mut audit = PublishedValueArenaBuilder::new();
    audit
        .audit_shared_input_at_depth(&first.declarations()[0], 1)
        .unwrap();
    let before = audit.local_shape().visits;
    audit
        .audit_shared_input_at_depth(&second.declarations()[0], 1)
        .unwrap();
    audit.local_shape().visits - before
}
#[test]
fn genuine_child_context_propagates_first_actual_pair_without_retaining_upstream_table() {
    let env = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let raw = rows();
    let manifest = manifest(&raw);
    let child = crate::BzlModuleSourceContext::new(&manifest, &env, std::iter::empty()).unwrap();
    let other = crate::BzlModuleSourceContext::new(&manifest, &env, std::iter::empty()).unwrap();
    let child_weak = Arc::downgrade(child.sources());
    let mapping_weak = Arc::downgrade(&raw[0].1.identity.repository_mapping);
    let parent =
        crate::BzlModuleSourceContext::new(&manifest, &env, [&child, &other, &child].into_iter())
            .unwrap();
    let p = parent.certified_sources();
    assert_eq!(parent, child);
    assert_eq!(parent.fingerprint(), child.fingerprint());
    assert!(!Arc::ptr_eq(parent.sources(), child.sources()));
    let overhead = p.declarations()[0].intrinsic_shape().visits + 4;
    assert_eq!(audit_after(child.certified_sources(), p), overhead);
    assert_eq!(
        audit_after(other.certified_sources(), p),
        overhead + super::oracle::mapping(&raw[0].1.identity.repository_mapping).visits
    );
    let mut changed = manifest.clone();
    let mut sources = changed.reachable.to_vec();
    sources[0].source_digest[31] ^= 1;
    changed.reachable = sources.into();
    let before = Arc::strong_count(&raw[0].1.identity.repository_mapping);
    assert!(crate::BzlModuleSourceContext::new(&changed, &env, std::iter::once(&child)).is_err());
    assert_eq!(
        Arc::strong_count(&raw[0].1.identity.repository_mapping),
        before
    );
    let retained = parent
        .certified_sources()
        .capture(&mut Budget::default())
        .unwrap();
    drop(parent);
    drop(child);
    drop(other);
    assert!(
        child_weak.upgrade().is_none(),
        "selected pair never retains the upstream table"
    );
    drop(raw);
    drop(manifest);
    drop(changed);
    drop(env);
    assert!(mapping_weak.upgrade().is_some());
    drop(retained);
    assert!(mapping_weak.upgrade().is_none());
}
#[test]
fn empty_duplicate_contexts_pay_every_advance_and_equal_foreign_root_stays_separate() {
    use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
    let env = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let raw = rows();
    let manifest = manifest(&raw);
    let empty = crate::BzlLoadManifest {
        reachable: Arc::from([]),
        ..manifest.clone()
    };
    let context = crate::BzlModuleSourceContext::new(&empty, &env, std::iter::empty()).unwrap();
    assert!(context.sources().is_empty());
    let mut none = Budget::default();
    drop(inherited::prepare(0, 0, std::iter::empty(), &env, &mut none).unwrap());
    let mut duplicate = Budget::default();
    drop(inherited::prepare(0, 0, [&context, &context].into_iter(), &env, &mut duplicate).unwrap());
    assert_eq!(duplicate.records, none.records + 10);
    let mut limit = Budget {
        records: RECORD_LIMIT - duplicate.records + 1,
        bytes: 0,
        depth: 0,
    };
    assert!(inherited::prepare(0, 0, [&context, &context].into_iter(), &env, &mut limit).is_err());
    let child = crate::BzlModuleSourceContext::new(&manifest, &env, std::iter::empty()).unwrap();
    let mut foreign = manifest.clone();
    let mut sources = foreign.reachable.to_vec();
    sources[0].identity.repository_mapping = sources[0].identity.repository_mapping.to_vec().into();
    foreign.reachable = sources.into();
    foreign.root = foreign.reachable[0].identity.clone();
    let parent =
        crate::BzlModuleSourceContext::new(&foreign, &env, std::iter::once(&child)).unwrap();
    assert_eq!(parent, child);
    let p = parent.certified_sources();
    assert_eq!(
        audit_after(child.certified_sources(), p),
        p.declarations()[0].intrinsic_shape().visits
            + 4
            + super::oracle::mapping(&foreign.reachable[0].identity.repository_mapping).visits
    );
}
#[tokio::test]
async fn original_environment_defining_contexts_supply_actual_mapping_guards() {
    use dice::DetectCycles;
    use dice::Dice;

    use crate::bzl_environment::BuiltinCatalogSnapshot;
    use crate::bzl_environment::BuiltinRepositoryMapping;
    use crate::bzl_environment::BuiltinSourceCatalogKey;
    use crate::bzl_environment::prepare_build_environment;
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    let mapping = BuiltinRepositoryMapping::new(Arc::from([])).unwrap();
    let env = prepare_build_environment(
        &mut tx,
        &BuiltinSourceCatalogKey::Bundled(BuiltinCatalogSnapshot::Bazel9_2),
        &mapping,
    )
    .await
    .unwrap();
    let raw = rows();
    let manifest = manifest(&raw);
    let context = crate::BzlModuleSourceContext::new(&manifest, &env, std::iter::empty()).unwrap();
    let module = env.modules().next().unwrap();
    let defining = module
        .module
        .client_data()
        .and_then(|d| crate::starlark_label::defining_module_source_context(d.to_value()))
        .unwrap();
    let mut audit = PublishedValueArenaBuilder::new();
    audit
        .audit_shared_input_at_depth(&defining.certified_sources().declarations()[0], 1)
        .unwrap();
    let before = audit.local_shape().visits;
    audit
        .audit_shared_input_at_depth(&context.certified_sources().declarations()[0], 1)
        .unwrap();
    let mut expected = context.certified_sources().declarations()[0]
        .intrinsic_shape()
        .visits
        + 2 * (1 + context.sources().len());
    for pair in context.certified_sources().mapping_pairs() {
        if !defining
            .certified_sources()
            .mapping_pairs()
            .iter()
            .any(|p| p.pointer() == pair.pointer())
        {
            expected += pair.shape().visits;
        }
    }
    assert_eq!(audit.local_shape().visits - before, expected);
    assert!(context.effective_reachable().len() > manifest.reachable.len());
    assert_eq!(context.environment_bindings().len(), 1);
}
