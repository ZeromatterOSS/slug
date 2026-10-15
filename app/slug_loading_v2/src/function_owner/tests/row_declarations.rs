//! Actual old-array oracle: no replacement factory or detached certificate.
use slug_build_api_v2::published_value::DEPTH_LIMIT;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;

use super::*;

fn validity(owner: &FunctionEvaluationOwner) -> &BzlInputValidity {
    owner.input_validity().as_any().downcast_ref().unwrap()
}
fn old_declarations(v: &BzlInputValidity) -> (Vec<SharedPublicationInput>, usize, usize, usize) {
    let mut old: Vec<_> = v
        .rows
        .iter()
        .cloned()
        .map(|row| SharedPublicationInput::new(row))
        .collect();
    let mut k = 0;
    for payload in v.references.physical_payloads() {
        let before = old.len();
        payload.append_shared(&mut old);
        assert_eq!(old.len() - before, payload.shared_count());
        k += old.len() - before;
    }
    let e = environment_publication::uses(&v.rows).count();
    let c = environment_publication::uses(&v.rows)
        .filter(|env| env.own_catalog().is_some())
        .count();
    environment_publication::append(&v.rows, &mut old);
    assert_eq!(old.len(), v.rows.len() + k + e + c);
    assert_eq!(v.shared_inputs.len(), v.rows.len());
    (old, k, e, c)
}
fn body(b: &PublishedValueArenaBuilder) -> (usize, usize, usize, usize) {
    let full = b.local_shape();
    let intrinsic = b.intrinsic_shape();
    // Raw admission supplies no OwnerSharedAudit: C remains zero. Subtraction
    // here only observes the public positive S accounting, never admits facts.
    assert_eq!(b.charged_visits(), full.visits);
    (
        full.nodes - intrinsic.nodes,
        full.edges - intrinsic.edges,
        full.bytes - intrinsic.bytes,
        full.visits - intrinsic.visits,
    )
}
fn audit(inputs: &[SharedPublicationInput]) -> PublishedValueArenaBuilder {
    let mut b = PublishedValueArenaBuilder::new();
    for input in inputs {
        b.audit_shared_input_at_depth(input, 1).unwrap();
    }
    b
}
fn same_closed_body(new: &[SharedPublicationInput], old: &[SharedPublicationInput]) {
    for (first, second) in [(new, old), (old, new)] {
        let mut b = audit(first);
        let expected = body(&b);
        let before = b.charged_visits();
        for input in second {
            b.audit_shared_input_at_depth(input, 1).unwrap();
        }
        assert_eq!(body(&b), expected);
        assert_eq!(b.charged_visits() - before, 2 * second.len());
        assert_eq!(expected, body(&audit(second)));
    }
}

#[tokio::test]
async fn row_only_actual_array_closes_foreign_catalogs_with_exact_reference_cell_algebra() {
    let (a, mixed, producers) =
        crate::bzl_environment::environment_publication_tests::mixed_catalog_pair_with_guard()
            .await;
    let m = manifest();
    let child = owner_with_environment(&m, "def child(): return 1", &mixed, []).unwrap();
    let pm = parent_manifest(1, &m);
    let parent = owner_with_environment(&pm, "parent", &a, [child.clone(), child.clone()]).unwrap();
    let v = validity(&parent);
    let l = v.rows.len();
    assert_eq!(l, 2);
    let own_b = mixed.inputs().own_catalog().unwrap();
    let crate::bzl_environment::references::PayloadRef::Catalog(cb) = own_b else {
        unreachable!()
    };
    let proof = mixed.inputs().source_proof().unwrap();
    assert!(Arc::ptr_eq(proof, a.inputs().source_proof().unwrap()));
    assert!(proof.nested_payloads().iter().all(|p| match p {
        crate::bzl_environment::references::Payload::Catalog(c) => !Arc::ptr_eq(c, cb),
        _ => false,
    }));
    // Holding both actual DICE producers makes these direct Arc deltas exact.
    let env_before = Arc::strong_count(mixed.inputs());
    let catalog_before = Arc::strong_count(cb);
    let (old, k, e, c) = old_declarations(v);
    assert!(k > 0);
    assert_eq!((e, c), (4, 4));
    assert_eq!(Arc::strong_count(mixed.inputs()), env_before + 3);
    assert_eq!(Arc::strong_count(cb), catalog_before + 3);
    let new = v.shared_inputs.to_vec();
    same_closed_body(&new, &old);
    // Every actual row is the corresponding stored guard, not equal content.
    let mut b = audit(&new);
    for row in &*v.rows {
        let before = b.charged_visits();
        b.audit_shared_input_at_depth(&SharedPublicationInput::new(row.clone()), 1)
            .unwrap();
        assert_eq!(b.charged_visits() - before, 2);
    }
    drop(b);
    // Positive old/new cell equations. The retained K group remains one
    // conservative bulk fact; only E+C intrinsic handles disappear. R counts
    // every removed occurrence, K included. No body count is subtracted.
    let a_cells = k + e + c;
    let old_d = parent.intrinsic_shape().visits + e + c;
    let new_r = parent.stable_key().shared_inputs().len() + new.len();
    let old_r = parent.stable_key().shared_inputs().len() + old.len();
    assert_eq!(old_r - new_r, a_cells);
    assert_eq!(
        (old_d + old_r) - (parent.intrinsic_shape().visits + new_r),
        k + 2 * e + 2 * c
    );
    // Independent typed local-storage oracle: owner handles, both row arrays,
    // canonical table and conservative K bulk group stay; E+C is absent.
    let table = v.references.owner_table_budget().unwrap().0;
    let owners = v.environment_owners.len();
    assert_eq!(v.shape.nodes, 0);
    assert_eq!(v.shape.visits, 3 + l + table.records);
    assert_eq!(v.shape.edges, 2 + owners + 2 * l + table.records + k);
    assert_eq!(
        v.shape.bytes,
        std::mem::size_of::<BzlInputValidity>()
            + 6 * std::mem::size_of::<usize>()
            + owners * std::mem::size_of::<FunctionEvaluationOwner>()
            + l * (std::mem::size_of::<Arc<LocalBzlInput>>()
                + std::mem::size_of::<SharedPublicationInput>())
            + table.bytes
            + k * std::mem::size_of::<SharedPublicationInput>()
    );
    assert_eq!(v.shape.depth, 1usize.max(table.depth).max(k));
    let weak_row = Arc::downgrade(&v.rows[0]);
    let weak_env = Arc::downgrade(mixed.inputs());
    let weak_catalog = Arc::downgrade(cb);
    let weak_proof = Arc::downgrade(proof);
    drop(old);
    assert_eq!(Arc::strong_count(mixed.inputs()), env_before);
    assert_eq!(Arc::strong_count(cb), catalog_before);
    // Raw row declarations alone own the entire closure after producer and
    // stable-owner disposal; no owner witness or outer sibling is available.
    drop(parent);
    drop(child);
    drop(a);
    drop(mixed);
    drop(producers);
    let retained = audit(&new);
    drop(new);
    assert!(weak_row.upgrade().is_some());
    assert!(weak_env.upgrade().is_some());
    assert!(weak_catalog.upgrade().is_some());
    assert!(weak_proof.upgrade().is_some());
    drop(retained);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while (weak_env.upgrade().is_some()
        || weak_catalog.upgrade().is_some()
        || weak_proof.upgrade().is_some())
        && std::time::Instant::now() < until
    {
        tokio::task::yield_now().await;
    }
    assert!(weak_row.upgrade().is_none());
    assert!(weak_env.upgrade().is_none());
    assert!(weak_catalog.upgrade().is_none());
    assert!(weak_proof.upgrade().is_none());
}

#[test]
fn row_only_uninjected_root_keeps_complete_depth_before_every_memo_hit() {
    let o = owner(&manifest(), "def f(): return 1", false, []).unwrap();
    let v = validity(&o);
    let (old, k, e, c) = old_declarations(v);
    assert_eq!((v.rows.len(), k, e, c), (1, 0, 1, 0));
    same_closed_body(&v.shared_inputs, &old);
    let input = &v.shared_inputs[0];
    let full = input.retained_shape();
    assert_eq!(full.depth, v.rows[0].retained_shape().depth);
    let exact = DEPTH_LIMIT - full.depth;
    let mut b = audit(&v.shared_inputs);
    let before = b.charged_visits();
    b.audit_shared_input_at_depth(input, exact).unwrap();
    assert_eq!(b.charged_visits() - before, 2);
    assert_eq!(b.local_shape().depth, DEPTH_LIMIT);
    assert!(b.audit_shared_input_at_depth(input, exact + 1).is_err());
    // Failure poisons even a shallow already-completed actual guard.
    assert!(b.audit_shared_input_at_depth(input, 1).is_err());
}
