use super::*;
use crate::bzl_environment::references::EnvironmentReferencePlan;
use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
use crate::function_owner::tests::manifest;
use crate::function_owner::tests::owner;
use crate::function_owner::tests::owner_with_environment;
use crate::function_owner::tests::parent_manifest;

fn validity(owner: &FunctionEvaluationOwner) -> &BzlInputValidity {
    owner.input_validity().as_any().downcast_ref().unwrap()
}
fn credits(b: Budget) -> (usize, usize, usize) {
    (b.records, b.bytes, b.depth)
}
fn hash(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
// Independent positive reconstruction of the unchanged pre-plan accumulator
// from actual accepted native fields. No caller-supplied Budget or fake row.
fn admitted_base(v: &BzlInputValidity) -> Budget {
    let mut shape = fact(
        std::mem::size_of::<BzlInputValidity>() + 4 * std::mem::size_of::<usize>(),
        2,
        1,
    );
    add_shape(
        &mut shape,
        fact(
            v.environment_owners.len() * std::mem::size_of::<FunctionEvaluationOwner>()
                + 2 * std::mem::size_of::<usize>(),
            v.environment_owners.len(),
            1,
        ),
        0,
    )
    .unwrap();
    for row in &*v.rows {
        add_shape(&mut shape, row.shape, 1).unwrap();
        add_shape(
            &mut shape,
            fact(
                std::mem::size_of::<Arc<LocalBzlInput>>()
                    + std::mem::size_of::<SharedPublicationInput>(),
                2,
                1,
            ),
            0,
        )
        .unwrap();
    }
    Budget {
        records: shape.visits,
        bytes: shape.bytes,
        depth: shape.depth,
    }
}
fn complement(plan: &EnvironmentReferencePlan) -> Budget {
    let (mut b, _) = plan.owner_table_budget().unwrap();
    for payload in plan.physical_payloads() {
        let whole = payload.physical_budget();
        b.charge(whole.records, whole.bytes).unwrap();
        b.depth = b.depth.max(whole.depth);
    }
    b
}
fn assert_same_cache(a: &EnvironmentReferencePlan, b: &EnvironmentReferencePlan) {
    assert_eq!(a, b);
    assert_eq!(hash(a), hash(b));
    assert_eq!(credits(a.budget()), credits(b.budget()));
    assert_eq!(credits(a.local_budget()), credits(b.local_budget()));
    assert_eq!(credits(a.physical_budget()), credits(b.physical_budget()));
    assert_eq!(
        credits(a.physical_local_budget()),
        credits(b.physical_local_budget())
    );
    assert!(b.construction_budget().records >= a.construction_budget().records);
    assert!(b.construction_budget().bytes >= a.construction_budget().bytes);
}

#[test]
fn carrier_fixed_work_and_iterator_envelope_refuse_without_guard_clones() {
    assert_eq!(
        std::mem::size_of::<OwnerReferenceInputs<'_>>(),
        5 * std::mem::size_of::<usize>()
    );
    let iterator = environment_publication::uses(&[]);
    assert!(
        std::mem::size_of_val(&iterator)
            <= OwnerReferenceInputs::ITERATOR_WORDS * std::mem::size_of::<usize>()
    );
    let o = owner(&manifest(), "def f(): return 1", false, []).unwrap();
    let v = validity(&o);
    let before = Arc::strong_count(&v.rows[0]);
    let mut cost = zero_shape();
    OwnerReferenceInputs::prepay(&mut cost).unwrap();
    assert_eq!(cost.visits, 12);
    for extra in [0, 1] {
        let mut work = PublicationShape {
            visits: RECORD_LIMIT - cost.visits + extra,
            ..zero_shape()
        };
        assert_eq!(OwnerReferenceInputs::prepay(&mut work).is_ok(), extra == 0);
        assert_eq!(work.visits, RECORD_LIMIT + extra);
        let mut work = PublicationShape {
            bytes: BYTE_LIMIT - cost.bytes + extra,
            ..zero_shape()
        };
        assert_eq!(OwnerReferenceInputs::prepay(&mut work).is_ok(), extra == 0);
        assert_eq!(work.bytes, BYTE_LIMIT + extra);
    }
    assert_eq!(Arc::strong_count(&v.rows[0]), before);
}

#[tokio::test]
async fn carrier_genuine_stream_preserves_foreign_catalogs_caches_refusals_and_last_drop() {
    let (weak_env, weak_proof) = {
        let (a, mixed, guard) =
            crate::bzl_environment::environment_publication_tests::mixed_catalog_pair_with_guard()
                .await;
        let m = manifest();
        let child = owner_with_environment(&m, "def f(): return 1", &mixed, []).unwrap();
        let pm = parent_manifest(1, &m);
        let parent = owner_with_environment(
            &pm,
            "def parent(): return 1",
            &a,
            [child.clone(), child.clone()],
        )
        .unwrap();
        let v = validity(&parent);
        let expected: Vec<_> = environment_publication::uses(&v.rows).collect();
        let before = (
            Arc::strong_count(a.inputs()),
            Arc::strong_count(mixed.inputs()),
        );
        let base = admitted_base(v);
        let mut paid = zero_shape();
        OwnerReferenceInputs::prepay(&mut paid).unwrap();
        // Test-only literal is derived from this genuine completed producer;
        // production has only the parent's inline mint, with no factory API.
        let inputs = OwnerReferenceInputs {
            rows: &v.rows,
            base,
        };
        assert_eq!(credits(inputs.base()), credits(base));
        let actual: Vec<_> = inputs.environments().collect();
        assert_eq!(actual.len(), expected.len());
        assert!(
            actual
                .iter()
                .zip(expected.iter())
                .all(|(a, b)| Arc::ptr_eq(a, b))
        );
        assert!(Arc::ptr_eq(actual[0], a.inputs()));
        assert!(
            actual
                .iter()
                .skip(1)
                .all(|e| Arc::ptr_eq(e, mixed.inputs()))
        );
        let ordinary = EnvironmentReferencePlan::prepare(expected.iter().copied()).unwrap();
        let covered = EnvironmentReferencePlan::prepare_covered_owner(
            inputs,
            Budget {
                records: paid.visits,
                bytes: paid.bytes,
                depth: paid.depth,
            },
        )
        .unwrap();
        assert_same_cache(&ordinary, &covered);
        assert_same_cache(&ordinary, &v.references);
        assert!(covered.construction_budget().records > ordinary.construction_budget().records);
        drop(covered);
        drop(ordinary);
        assert_eq!(
            (
                Arc::strong_count(a.inputs()),
                Arc::strong_count(mixed.inputs())
            ),
            before
        );
        let proof = mixed.inputs().source_proof().unwrap();
        let weak = (Arc::downgrade(mixed.inputs()), Arc::downgrade(proof));
        let guards_before = Arc::strong_count(mixed.inputs());
        let mut exhausted = PublicationShape {
            visits: RECORD_LIMIT - OwnerReferenceInputs::READS,
            ..zero_shape()
        };
        OwnerReferenceInputs::prepay(&mut exhausted).unwrap();
        let refused = EnvironmentReferencePlan::prepare_covered_owner(
            OwnerReferenceInputs {
                rows: &v.rows,
                base,
            },
            Budget {
                records: exhausted.visits,
                bytes: exhausted.bytes,
                depth: exhausted.depth,
            },
        );
        assert!(refused.is_err());
        assert_eq!(Arc::strong_count(mixed.inputs()), guards_before);
        drop(actual);
        drop(expected);
        drop(parent);
        drop(child);
        drop(a);
        drop(mixed);
        drop(guard);
        weak
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while weak_env.strong_count() != 0 || weak_proof.strong_count() != 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "covered inputs outlive complete DICE teardown"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(weak_env.upgrade().is_none());
    assert!(weak_proof.upgrade().is_none());
}

mod native_boundary;
