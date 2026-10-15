//! Actual selected exports with an independently equal seed catalog.
use starlark::values::ValueLike;

use super::*;

/// Explicit engine/version ownership for tests that compare exact Arc counts.
/// Keeping both active transactions prevents detached state teardown from
/// dropping the catalog key's cached result during the measured interval.
pub(crate) struct MixedCatalogGuard {
    _transactions: [dice::DiceTransaction; 2],
    _engines: [Arc<dice::Dice>; 2],
}
pub(crate) async fn mixed_catalog_pair() -> (PreparedBzlEnvironment, PreparedBzlEnvironment) {
    let (a, mixed, guard) = mixed_catalog_pair_with_guard().await;
    // Preserve the original helper's teardown behavior for all existing callers.
    drop(guard);
    (a, mixed)
}
pub(crate) async fn mixed_catalog_pair_with_guard() -> (
    PreparedBzlEnvironment,
    PreparedBzlEnvironment,
    MixedCatalogGuard,
) {
    let left = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let right = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let mut lt = left.updater().commit().await;
    let mut rt = right.updater().commit().await;
    let key = BuiltinSourceCatalogKey::Bundled(BuiltinCatalogSnapshot::Bazel9_2);
    let mapping = BuiltinRepositoryMapping::new(Arc::from([])).unwrap();
    let a = prepare_build_environment(&mut lt, &key, &mapping)
        .await
        .unwrap();
    let reused = prepare_build_environment(&mut lt, &key, &mapping)
        .await
        .unwrap();
    assert!(Arc::ptr_eq(
        a.inputs.source_proof().unwrap(),
        reused.inputs.source_proof().unwrap()
    ));
    assert!(
        a.exports[0]
            .value
            .value()
            .ptr_eq(reused.exports[0].value.value())
    );
    let foreign = prepare_build_environment(&mut rt, &key, &mapping)
        .await
        .unwrap();
    let ca = a.inputs.catalog.as_ref().unwrap();
    let cb = foreign.inputs.catalog.as_ref().unwrap();
    assert_eq!(ca, cb);
    assert!(!Arc::ptr_eq(ca, cb));
    let seed_b = PreparedBzlEnvironment::seed(BzlEnvironmentMode::Builtin, Some(cb)).unwrap();
    // This exact original is a successfully reused DICE product. select already
    // accepts the seed and module separately; no producer/certificate is forged.
    let mixed = load::select(
        &seed_b,
        &reused.exports[0].module,
        "apple_common",
        "apple_common_bazel",
        BzlEnvironmentMode::Build,
        seed_b.inputs.budget(),
    )
    .unwrap();
    assert!(Arc::ptr_eq(mixed.inputs.catalog.as_ref().unwrap(), cb));
    assert!(Arc::ptr_eq(
        mixed.inputs.source_proof().unwrap(),
        a.inputs.source_proof().unwrap()
    ));
    assert!(a.inputs == mixed.inputs);
    (
        a,
        mixed,
        MixedCatalogGuard {
            _transactions: [lt, rt],
            _engines: [left, right],
        },
    )
}
