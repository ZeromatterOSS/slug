//! Private-kernel oracle, not a manufactured native coverage certificate.
use super::*;
use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
fn fields(b: Budget) -> (usize, usize, usize) {
    (b.records, b.bytes, b.depth)
}
fn hash(p: &EnvironmentReferencePlan) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    p.hash(&mut h);
    h.finish()
}

#[tokio::test]
async fn actual_alias_foreign_payloads_preserve_all_caches_and_refuse_before_guard_clones() {
    let (left, mixed, guard) =
        crate::bzl_environment::environment_publication_tests::mixed_catalog_pair_with_guard()
            .await;
    let (foreign, _, foreign_guard) =
        crate::bzl_environment::environment_publication_tests::mixed_catalog_pair_with_guard()
            .await;
    assert!(!Arc::ptr_eq(&left.inputs, &mixed.inputs));
    assert!(!Arc::ptr_eq(
        left.inputs.source_proof().unwrap(),
        foreign.inputs.source_proof().unwrap()
    ));
    assert!(left.inputs == mixed.inputs && left.inputs == foreign.inputs);
    let envs = [&left.inputs, &mixed.inputs, &foreign.inputs, &left.inputs];
    // A positive env-body base for this isolated kernel oracle. No row-coverage
    // carrier or publication owner is minted here; genuine producer tests are separate.
    let mut base = Budget::default();
    for env in [&left.inputs, &mixed.inputs, &foreign.inputs] {
        let own = env.local_budget();
        base.charge(own.records, own.bytes).unwrap();
    }
    let full = EnvironmentReferencePlan::prepare_with_budget(
        envs.into_iter(),
        base,
        Budget::default(),
        PeakOwner::FunctionOwner,
    )
    .unwrap();
    let covered = EnvironmentReferencePlan::prepare_indexed_physical::<Covered>(
        envs.into_iter(),
        |h| h,
        Budget::default(),
        base,
        PeakOwner::FunctionOwner,
    )
    .unwrap();
    assert_eq!(covered, full);
    assert_eq!(hash(&covered), hash(&full));
    for (a, b) in [
        (covered.budget(), full.budget()),
        (covered.local_budget(), full.local_budget()),
        (covered.physical_budget(), full.physical_budget()),
        (
            covered.physical_local_budget(),
            full.physical_local_budget(),
        ),
    ] {
        assert_eq!(fields(a), fields(b));
    }
    assert_eq!(covered.ordinals.as_ref(), &[0, 0, 0, 0]);
    assert_eq!(covered.physical_payloads().len(), 2);
    assert!(covered.construction_budget().records > full.construction_budget().records);
    assert!(covered.construction_budget().bytes > full.construction_budget().bytes);
    let mut positive = table_budget(
        envs.len(),
        PeakContext::new(
            PeakOwner::FunctionOwner,
            PeakPhase::ReferenceTables,
            FirstLedger::Local,
        ),
    )
    .unwrap();
    for payload in full.physical_payloads() {
        let body = payload.physical_budget();
        positive.charge(body.records, body.bytes).unwrap();
    }
    assert!(positive.records < full.physical_budget().records);
    drop(covered);
    drop(full);
    // The boundary oracle uses two actual descriptors sharing ONE actual proof;
    // separately equal foreign payloads above remain charged whole, not omitted.
    let boundary_envs = [&left.inputs, &mixed.inputs, &left.inputs];
    let ordinary = EnvironmentReferencePlan::prepare(boundary_envs.into_iter()).unwrap();
    let mut positive = table_budget(
        boundary_envs.len(),
        PeakContext::new(
            PeakOwner::FunctionOwner,
            PeakPhase::ReferenceTables,
            FirstLedger::Local,
        ),
    )
    .unwrap();
    assert_eq!(ordinary.physical_payloads().len(), 1);
    for payload in ordinary.physical_payloads() {
        let body = payload.physical_budget();
        positive.charge(body.records, body.bytes).unwrap();
    }
    let edge_base = Budget {
        records: RECORD_LIMIT - ordinary.budget().records,
        ..Budget::default()
    };
    assert!(edge_base.records + positive.records <= RECORD_LIMIT);
    assert!(edge_base.records + ordinary.physical_budget().records > RECORD_LIMIT);
    drop(ordinary);
    let counts = || {
        [
            Arc::strong_count(&left.inputs),
            Arc::strong_count(&mixed.inputs),
            Arc::strong_count(&foreign.inputs),
            Arc::strong_count(left.inputs.source_proof().unwrap()),
            Arc::strong_count(foreign.inputs.source_proof().unwrap()),
        ]
    };
    let before = counts();
    let error = EnvironmentReferencePlan::prepare_with_budget(
        boundary_envs.into_iter(),
        edge_base,
        Budget::default(),
        PeakOwner::FunctionOwner,
    )
    .unwrap_err();
    assert!(error.contains("first_ledger=physical-forecast"));
    assert_eq!(before, counts());
    let accepted = EnvironmentReferencePlan::prepare_indexed_physical::<Covered>(
        boundary_envs.into_iter(),
        |h| h,
        Budget::default(),
        edge_base,
        PeakOwner::FunctionOwner,
    )
    .unwrap();
    assert_eq!(accepted.ordinals.as_ref(), &[0, 0, 0]);
    drop(accepted);
    assert_eq!(before, counts());
    let error = EnvironmentReferencePlan::prepare_indexed_physical::<Covered>(
        boundary_envs.into_iter(),
        |h| h,
        Budget::default(),
        Budget {
            records: edge_base.records + 1,
            ..edge_base
        },
        PeakOwner::FunctionOwner,
    )
    .unwrap_err();
    assert!(error.contains("first_ledger=logical-facts"));
    assert_eq!(before, counts());
    drop(guard);
    drop(foreign_guard);
}
