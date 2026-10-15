//! Equal foreign proofs must retain complete facts and deterministic credits.
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use slug_build_api_v2::published_value::FunctionEvaluationOwner;
use starlark::values::ValueLike;

use super::*;
use crate::bzl_module::BzlModuleContext;
use crate::bzl_module::FrozenBzlModule;
use crate::bzl_module::semantic_evaluation;

fn hash(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
fn credits(b: Budget) -> (usize, usize, usize) {
    (b.records, b.bytes, b.depth)
}
fn key(env: &PreparedBzlEnvironment) -> semantic_evaluation::SemanticBzlEvaluationKey {
    key_with_context(env, BzlModuleContext::Build)
}
fn key_with_context(
    env: &PreparedBzlEnvironment,
    context: BzlModuleContext,
) -> semantic_evaluation::SemanticBzlEvaluationKey {
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@//pkg:reference_consumer.bzl").unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(
            "/fixture/pkg/reference_consumer.bzl".into(),
        ),
        repository_mapping: Arc::from([]),
    };
    semantic_evaluation::prepare(
        &identity,
        "reference_consumer.bzl",
        &Arc::from(b"print('reference event')\nVALUE = apple_common\n".as_slice()),
        context,
        env,
        std::iter::empty(),
    )
    .unwrap()
}
fn owner(module: &FrozenBzlModule) -> &FunctionEvaluationOwner {
    module
        .module
        .client_data()
        .and_then(|v| crate::starlark_label::defining_module_function_owner(v.to_value()))
        .unwrap()
}
fn mapping(last: &str) -> BuiltinRepositoryMapping {
    BuiltinRepositoryMapping::new(Arc::from([
        (
            ApparentRepoName::new("first").unwrap(),
            CanonicalRepoName::new("first+").unwrap(),
        ),
        (
            ApparentRepoName::new("last").unwrap(),
            CanonicalRepoName::new(last).unwrap(),
        ),
    ]))
    .unwrap()
}

#[tokio::test]
async fn foreign_equal_builtin_proofs_preserve_normal_form_owner_events_and_release() {
    let (left_weak, right_weak) = {
        let left = Dice::builder().build(DetectCycles::Enabled);
        let right = Dice::builder().build(DetectCycles::Enabled);
        let mut lt = left.updater().commit().await;
        let mut rt = right.updater().commit().await;
        let catalog = BuiltinSourceCatalogKey::Bundled(BuiltinCatalogSnapshot::Bazel9_2);
        let a = prepare_build_environment(&mut lt, &catalog, &mapping("last+"))
            .await
            .unwrap();
        let selected_again = prepare_build_environment(&mut lt, &catalog, &mapping("last+"))
            .await
            .unwrap();
        let foreign = prepare_build_environment(&mut rt, &catalog, &mapping("last+"))
            .await
            .unwrap();
        let ap = a.inputs.source_proof().unwrap();
        let fp = foreign.inputs.source_proof().unwrap();
        assert!(!Arc::ptr_eq(&a.inputs, &selected_again.inputs));
        assert!(Arc::ptr_eq(
            ap,
            selected_again.inputs.source_proof().unwrap()
        ));
        assert!(
            !Arc::ptr_eq(ap, fp),
            "independent engines must exercise full foreign equality"
        );
        assert!(a.inputs == foreign.inputs);
        assert_eq!(hash(&a.inputs), hash(&foreign.inputs));
        assert!(
            !a.exports[0]
                .value
                .value()
                .ptr_eq(foreign.exports[0].value.value())
        );
        let shared_plan = references::EnvironmentReferencePlan::prepare(
            [&a.inputs, &selected_again.inputs].into_iter(),
        )
        .unwrap();
        let foreign_plan =
            references::EnvironmentReferencePlan::prepare([&a.inputs, &foreign.inputs].into_iter())
                .unwrap();
        assert_eq!(
            credits(shared_plan.budget()),
            credits(foreign_plan.budget())
        );
        assert_eq!(
            credits(shared_plan.local_budget()),
            credits(foreign_plan.local_budget())
        );

        let ak = key(&a);
        let sk = key(&selected_again);
        let fk = key(&foreign);
        assert!(ak == sk && ak == fk);
        assert_eq!(hash(&ak), hash(&fk));
        let av = lt.compute(&ak).await.unwrap();
        let sv = lt.compute(&sk).await.unwrap();
        let fv = rt.compute(&fk).await.unwrap();
        let am = av.result().unwrap();
        let fm = fv.result().unwrap();
        assert!(
            Arc::ptr_eq(&av, &sv),
            "same complete semantic key reuses its actual producer"
        );
        assert!(owner(am) == owner(fm));
        assert_eq!(hash(owner(am)), hash(owner(fm)));
        assert_eq!(
            credits(am.semantic_input_facts().unwrap().logical_budget()),
            credits(fm.semantic_input_facts().unwrap().logical_budget())
        );
        assert_eq!(av.events(), fv.events());
        assert_eq!(av.events().events().len(), 1);
        assert!(
            !am.module
                .get("VALUE")
                .unwrap()
                .value()
                .ptr_eq(fm.module.get("VALUE").unwrap().value())
        );
        (Arc::downgrade(ap), Arc::downgrade(fp))
    };
    // DICE's state processor is a detached OS thread. Wait for its actual
    // cache/key destruction, not merely the last engine handle's Drop return.
    // A real proof cycle still fails this exact zero-owner condition.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while left_weak.strong_count() != 0 || right_weak.strong_count() != 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "proofs survive engine teardown: left={} right={}",
            left_weak.strong_count(),
            right_weak.strong_count(),
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        left_weak.upgrade().is_none(),
        "no proof remains after complete engine teardown"
    );
    assert!(
        right_weak.upgrade().is_none(),
        "foreign equality must not create persistent ownership"
    );
}

#[tokio::test]
async fn final_mapping_source_export_and_mode_fields_discriminate_and_restore() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    let catalog_key = BuiltinSourceCatalogKey::Bundled(BuiltinCatalogSnapshot::Bazel9_2);
    let a = prepare_build_environment(&mut tx, &catalog_key, &mapping("last+"))
        .await
        .unwrap();
    let initial_key = key(&a);
    let initial = tx.compute(&initial_key).await.unwrap();
    let changed_mapping = prepare_build_environment(&mut tx, &catalog_key, &mapping("changed+"))
        .await
        .unwrap();
    assert!(initial_key != key(&changed_mapping));
    let catalog = a.inputs.catalog.as_ref().unwrap();
    let changed_source = prepare_build_environment(
        &mut tx,
        &BuiltinSourceCatalogKey::Snapshot(super::tests::changed_catalog_at(
            catalog,
            catalog.entries.len() - 1,
        )),
        &mapping("last+"),
    )
    .await
    .unwrap();
    assert!(initial_key != key(&changed_source));
    let seed = PreparedBzlEnvironment::seed(BzlEnvironmentMode::Builtin, Some(catalog)).unwrap();
    let changed_export = load::select(
        &seed,
        &a.exports[0].module,
        "apple_common",
        "native_apple_common",
        BzlEnvironmentMode::Build,
        seed.inputs.budget(),
    )
    .unwrap();
    assert!(initial_key != key(&changed_export));
    let bzlmod = prepare_source_environment(
        &mut tx,
        &catalog_key,
        &mapping("last+"),
        BzlEnvironmentMode::Bzlmod,
    )
    .await
    .unwrap();
    let bootstrap =
        PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    assert!(
        key_with_context(&bzlmod, BzlModuleContext::Bzlmod)
            != key_with_context(&bootstrap, BzlModuleContext::Bzlmod)
    );
    let restored = prepare_build_environment(&mut tx, &catalog_key, &mapping("last+"))
        .await
        .unwrap();
    let restored_key = key(&restored);
    assert!(initial_key == restored_key);
    assert_eq!(hash(&initial_key), hash(&restored_key));
    let actual = tx.compute(&restored_key).await.unwrap();
    assert!(Arc::ptr_eq(&initial, &actual));
    assert!(
        a.exports[0]
            .value
            .value()
            .ptr_eq(restored.exports[0].value.value())
    );
}
