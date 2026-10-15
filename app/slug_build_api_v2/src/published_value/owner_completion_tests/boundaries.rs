//! Exact final facts and independent unchanged producer work/byte admission.
use super::*;

#[test]
fn complete_fact_boundary_counts_real_import_and_default_reference_cell_with_same_work() {
    let mut actual_work = None;
    for extra in [0, 1] {
        let prior = arena(RETAINED_FACT_LIMIT - 5 + extra);
        let weak = Arc::downgrade(&prior.0);
        let a = empty(20);
        let b = part(21, vec![], vec![prior], vec![]);
        let parts = (Arc::downgrade(&a), Arc::downgrade(&b));
        let mut audit = raw_audit(&*a, &*b);
        let completion = audit.complete_owner_shape(&*a, &*b, 2).unwrap();
        assert_eq!(completion.declaration_references, 1);
        assert_eq!(completion.shape.visits, RETAINED_FACT_LIMIT + extra);
        assert_eq!(audit.import_visits, RETAINED_FACT_LIMIT - 3 + extra);
        assert_eq!(audit.certified_shared_visits, 0);
        if let Some(work) = actual_work {
            assert_eq!(audit.charged_visits(), work);
        }
        actual_work = Some(audit.charged_visits());
        drop(audit);
        let result = FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredRule, a, b);
        if extra == 0 {
            let actual = result.unwrap();
            assert_eq!(actual.intrinsic_shape().visits, 2);
            assert_eq!(actual.retained_shape().visits, RETAINED_FACT_LIMIT);
            assert!(weak.upgrade().is_some());
            drop(actual);
        } else {
            let PublishedValueError::OwnerRetainedVisitBudget(report) = result.unwrap_err() else {
                panic!("wrong final refusal");
            };
            assert_eq!(report.declaration_reference_visits(), 1);
            assert_eq!(report.intrinsic_visits(), 2);
            assert_eq!(report.imported_visits(), RETAINED_FACT_LIMIT - 2);
            assert_eq!(report.complete_visits(), RETAINED_FACT_LIMIT + 1);
            assert_eq!(report.producer_work(), actual_work);
            assert_eq!(report.legacy_local_visits(), actual_work.unwrap());
            let census = report.closure_census().unwrap();
            assert_eq!(census.imported_arenas(), 1);
            assert_eq!(census.imported_local_visits(), RETAINED_FACT_LIMIT - 2);
            assert_eq!(census.source_references(), 0);
            assert_eq!(report, report.clone());
        }
        assert!(weak.upgrade().is_none());
        assert!(parts.0.upgrade().is_none() && parts.1.upgrade().is_none());
    }
}

#[test]
fn actual_aliases_still_obey_strict_work_and_complete_depth_before_final_policy() {
    let child_part = empty(40);
    let weak = Arc::downgrade(&child_part);
    let child =
        FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredRule, child_part, empty(40))
            .unwrap();
    let a = part(41, vec![child; NODE_LIMIT / 2], vec![], vec![]);
    let a_weak = Arc::downgrade(&a);
    let error = FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredRule, a, empty(42))
        .unwrap_err();
    assert_eq!(error, PublishedValueError::Limit("visits"));
    assert!(a_weak.upgrade().is_none() && weak.upgrade().is_none());
    let child = owner(43);
    let mut audit = PublishedValueArenaBuilder::new_strict_shared_audit();
    audit.audit_owner_shape(&child).unwrap();
    let depth = child.retained_shape().depth;
    audit
        .audit_owner_shape_at_depth(&child, DEPTH_LIMIT - depth)
        .unwrap();
    assert!(
        audit
            .audit_owner_shape_at_depth(&child, DEPTH_LIMIT - depth + 1)
            .is_err()
    );
    assert!(audit.charge_visit().is_err());
}
