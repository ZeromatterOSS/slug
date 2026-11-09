//! Ordinary allocation/memo custody only. Pure fixture states and body leases
//! mint no physical root, request provenance, or native IO authority.
use super::*;

fn counters(scope: &SourceServiceScope) -> ([u64; 6], [usize; 4]) {
    let value = scope.snapshot();
    (
        [
            value.registry_attempts,
            value.registry_admitted_bytes,
            value.source_attempts,
            value.transferred_bytes,
            value.decoded_bytes,
            value.written_bytes,
        ],
        [
            value.live_buffer_bytes,
            value.peak_buffer_bytes,
            value.live_decoder_bytes,
            value.peak_decoder_bytes,
        ],
    )
}

#[test]
fn comparison_memo_retains_body_coverage_after_frontier_arguments_retire() {
    let bank = super::super::tests::resources();
    let scope = super::super::tests::scope();
    let baseline = bank.snapshot().retained_bytes;
    let source_before = counters(&scope);
    let active = || true;
    let mut comparison = ViewComparison::new(&bank, &scope, &active).unwrap();
    let (left_weak, right_weak, left_state_weak, right_state_weak) = {
        let reservation = super::super::native_tests::reservation(
            std::path::Path::new("/workspace/comparison-custody"),
            3412,
        );
        let payload = vec![8; 128 * 1024];
        let left_owner = super::super::native_tests::realization(
            reservation.clone(),
            RepositoryWorkspaceMode::Archive,
            &payload,
        );
        let right_owner = super::super::native_tests::realization(
            reservation.clone(),
            RepositoryWorkspaceMode::Archive,
            &payload,
        );
        let left = left_owner.final_state().clone();
        let right = right_owner.final_state().clone();
        assert_eq!(left, right);
        assert!(!Arc::ptr_eq(&left, &right));
        let body = |state: &RepositoryWorkspaceState| {
            let RepositoryWorkspaceNode::File(body) = state.entry("dir/body").unwrap().node()
            else {
                panic!("fixture regular body")
            };
            body.clone()
        };
        let left_body = body(&left);
        let right_body = body(&right);
        assert!(!Arc::ptr_eq(&left_body, &right_body));
        let weak = (
            Arc::downgrade(&left_body),
            Arc::downgrade(&right_body),
            Arc::downgrade(&left),
            Arc::downgrade(&right),
        );
        let ledger = NativeBodyLedger::new(&bank, &scope, &reservation, &active).unwrap();
        let mut candidates = ledger.candidates().unwrap();
        for state in [&left, &right] {
            state
                .try_visit_body_allocations(&mut |body| {
                    drop(candidates.retain(body, &active)?);
                    Ok::<_, PhysicalRootError>(())
                })
                .unwrap();
        }
        let control = left
            .body_covered_storage_partition()
            .unwrap()
            .total_bytes()
            .unwrap()
            .checked_add(
                right
                    .body_covered_storage_partition()
                    .unwrap()
                    .total_bytes()
                    .unwrap(),
            )
            .unwrap();
        let credit = bank.reserve(control, 0, 0).unwrap();
        let dependencies = NativeReadViewDependencies::empty(&bank, 0).unwrap();
        comparison.prepare_workspace_coverage().unwrap();
        comparison.hold_workspace_coverage(candidates, dependencies, credit);
        assert!(comparison.state_equal(&left, &right).unwrap());
        assert!(comparison.body_equal(&left_body, &right_body).unwrap());
        assert_eq!(comparison.compared_bytes, payload.len());
        assert_eq!(comparison.states.len(), 1);
        assert_eq!(comparison.bodies.len(), 1);
        // All frontier argument owners, raw body handles, ledger handles and
        // pure fixture realizations retire here; only the memo handoff remains.
        weak
    };
    assert!(left_weak.upgrade().is_some());
    assert!(right_weak.upgrade().is_some());
    assert!(left_state_weak.upgrade().is_some());
    assert!(right_state_weak.upgrade().is_some());
    assert!(bank.snapshot().retained_bytes >= baseline + 2 * 128 * 1024);
    assert_eq!(bank.snapshot().native_fds, 0);
    assert_eq!(bank.snapshot().backing_bytes, 0);
    assert_eq!(counters(&scope), source_before);
    scope.check().unwrap();
    drop(comparison);
    assert!(left_weak.upgrade().is_none());
    assert!(right_weak.upgrade().is_none());
    assert!(left_state_weak.upgrade().is_none());
    assert!(right_state_weak.upgrade().is_none());
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    assert_eq!(bank.snapshot().native_fds, 0);
    assert_eq!(bank.snapshot().backing_bytes, 0);
    assert_eq!(counters(&scope), source_before);
    scope.check().unwrap();
}
