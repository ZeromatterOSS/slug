use super::*;
use crate::published_value::builder::import_accounting::tests::leaf;
use crate::published_value::builder::retained_facts::tests::code;
use crate::published_value::builder::retained_facts::tests::seal;
use crate::published_value::builder::retained_facts::tests::source;

#[derive(Debug, Allocative)]
pub(super) struct Part {
    id: u8,
    visits: usize,
    owners: Vec<FunctionEvaluationOwner>,
    arenas: Vec<PublishedValueArena>,
}
impl StructuralFunctionOwnerPart for Part {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().downcast_ref::<Self>().is_some_and(|o| {
            self.id == o.id
                && self.visits == o.visits
                && self.owners == o.owners
                && self.arenas.len() == o.arenas.len()
                && self
                    .arenas
                    .iter()
                    .zip(&o.arenas)
                    .all(|(a, b)| a.identity() == b.identity())
        })
    }
    fn hash_part(&self, h: &mut dyn std::hash::Hasher) {
        h.write_u8(self.id);
    }
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + (self.owners.len() + self.arenas.len()) * std::mem::size_of::<usize>()
    }
    fn retained_shape(&self) -> PublicationShape {
        PublicationShape {
            bytes: self.retained_bytes(),
            visits: self.visits,
            ..PublicationShape::default()
        }
    }
    fn retained_owners(&self) -> &[FunctionEvaluationOwner] {
        &self.owners
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.arenas
    }
}
pub(super) fn part(
    id: u8,
    visits: usize,
    owners: Vec<FunctionEvaluationOwner>,
    arenas: Vec<PublishedValueArena>,
) -> Arc<Part> {
    Arc::new(Part {
        id,
        visits,
        owners,
        arenas,
    })
}
fn owner(domain: FunctionOwnerDomain, id: u8, visits: usize) -> FunctionEvaluationOwner {
    FunctionEvaluationOwner::new(
        domain,
        part(id, visits, vec![], vec![]),
        part(id, visits, vec![], vec![]),
    )
    .unwrap()
}
fn domain_draft() -> PublishedValueArenaBuilder {
    let bzl = owner(FunctionOwnerDomain::BzlLoad, 1, 1);
    let foreign = owner(FunctionOwnerDomain::BzlLoad, 1, 1);
    assert_eq!(bzl, foreign);
    assert_ne!(bzl.storage_identity(), foreign.storage_identity());
    let zero = owner(FunctionOwnerDomain::BzlLoad, 8, 0);
    let rule = owner(FunctionOwnerDomain::ConfiguredRule, 2, 2);
    let aspect = owner(FunctionOwnerDomain::ConfiguredAspect, 3, 3);
    let parent = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::ConfiguredRule,
        part(4, 1, vec![], vec![]),
        part(
            5,
            1,
            vec![bzl.clone(), bzl, foreign, zero, rule, aspect],
            vec![],
        ),
    )
    .unwrap();
    let mut b = PublishedValueArenaBuilder::new_strict_shared_audit();
    b.audit_owner_shape(&parent).unwrap();
    b.audit_owner_shape(&parent).unwrap();
    b
}
#[test]
fn completed_domain_census_counts_actual_diamonds_and_equal_foreign_owners() {
    let mut b = domain_draft();
    let total = b.complete_shape().unwrap().visits;
    let census = collect(&b, total).unwrap();
    for (domain, count, sum, max) in [
        (FunctionOwnerDomain::BzlLoad, 3, 4, 2),
        (FunctionOwnerDomain::ConfiguredRule, 2, 6, 4),
        (FunctionOwnerDomain::ConfiguredAspect, 1, 6, 6),
    ] {
        let row = census.owners(domain);
        assert_eq!(
            (
                row.count(),
                row.intrinsic_visits(),
                row.maximum_intrinsic_visits()
            ),
            (count, sum, max)
        );
    }
    assert_eq!(b.owner_payload.visits, 16);
    assert_eq!(census.imported_arenas(), 0);
    assert_eq!(census.imported_local_visits(), 0);
    assert_eq!(census.source_references(), 0);
    assert_eq!(census.unique_sources(), 0);
}

#[test]
fn real_final_refusal_census_preserves_local_source_bodies_foreign_guards_and_drop() {
    let authority = source(4);
    let foreign = source(4);
    assert_eq!(authority, foreign);
    assert!(!authority.shares_storage_with(&foreign));
    let source_facts = authority.fact_count();
    let source_weak = Arc::downgrade(&authority.data().root.repository_mapping);
    let foreign_weak = Arc::downgrade(&foreign.data().root.repository_mapping);
    let mut a = PublishedValueArenaBuilder::new();
    code(&mut a, authority.clone());
    code(&mut a, authority.clone());
    let a = seal(a, 0);
    assert_eq!(
        a.sources().len(),
        1,
        "repeated code rows share one LOCAL source body"
    );
    let mut b = PublishedValueArenaBuilder::new();
    b.import(a.clone()).unwrap();
    code(&mut b, authority.clone());
    let b = seal(b, 0);
    assert_eq!(
        b.sources().len(),
        1,
        "a parent local source list does not flatten imported lists"
    );
    let mut c = PublishedValueArenaBuilder::new();
    code(&mut c, foreign.clone());
    let c = seal(c, 0);
    let mut imported_only = PublishedValueArenaBuilder::new();
    imported_only.import(a.clone()).unwrap();
    let imported_only = seal(imported_only, 0);
    assert!(imported_only.sources().is_empty());
    let source_local = a.0.local_shape.visits
        + b.0.local_shape.visits
        + c.0.local_shape.visits
        + imported_only.0.local_shape.visits;
    // Two intrinsic part records; seven explicit arena encounters, two nested
    // diamond encounters and three LOCAL source lookups. No fabricated shape.
    let work = 14usize + 22; // complete positive constructor-count prepayment
    let padding = RETAINED_FACT_LIMIT + 1 - 2 - 7 - source_local - (NODE_LIMIT - 1);
    let arena_weak = Arc::downgrade(&a.0);
    let key = part(6, 1, vec![], vec![]);
    let validity = part(
        7,
        1,
        vec![],
        vec![
            a.clone(),
            b.clone(),
            c.clone(),
            imported_only.clone(),
            a.clone(),
            leaf(NODE_LIMIT - 1),
            leaf(padding),
        ],
    );
    let key_weak = Arc::downgrade(&key);
    let validity_weak = Arc::downgrade(&validity);
    let error = FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredRule, key, validity)
        .unwrap_err();
    let PublishedValueError::OwnerRetainedVisitBudget(report) = error else {
        panic!("expected final owner refusal, got {error:?}");
    };
    assert_eq!(report.producer_work(), Some(work));
    assert_eq!(report.declaration_reference_visits(), 7);
    assert_eq!(report.complete_visits(), RETAINED_FACT_LIMIT + 1);
    let census = report.closure_census().unwrap();
    assert_eq!(census.imported_arenas(), 6);
    assert_eq!(census.imported_local_visits(), report.imported_visits());
    assert_eq!(census.source_references(), 3);
    assert_eq!(census.imported_source_body_facts(), 3 * source_facts);
    assert_eq!(census.unique_sources(), 2);
    assert_eq!(census.unique_source_body_facts(), 2 * source_facts);
    assert_eq!(
        census.imported_source_body_facts() - census.unique_source_body_facts(),
        source_facts
    );
    for domain in [
        FunctionOwnerDomain::BzlLoad,
        FunctionOwnerDomain::ConfiguredRule,
        FunctionOwnerDomain::ConfiguredAspect,
    ] {
        assert_eq!(
            census.owners(domain),
            &PublicationOwnerDomainCensus::default()
        );
    }
    drop((authority, foreign, a, b, c, imported_only));
    let cloned = report.clone();
    assert_eq!(report, cloned);
    assert!(key_weak.upgrade().is_none() && validity_weak.upgrade().is_none());
    assert!(arena_weak.upgrade().is_none());
    assert!(
        source_weak.upgrade().is_none() && foreign_weak.upgrade().is_none(),
        "scalar report retains no actual source payload"
    );
}

#[test]
fn observer_exact_envelope_and_capacity_preguard_leave_all_ledgers_and_guards_unchanged() {
    let mut b = domain_draft();
    let total = b.complete_shape().unwrap().visits;
    let before = (
        b.visits,
        b.import_visits,
        b.owner_payload.visits,
        b.shared_payload.visits,
        b.certified_shared_visits,
        b.failed_budget,
    );
    let guards = b
        .owner_seen
        .values()
        .map(|v| v.storage_identity())
        .collect::<Vec<_>>();
    let reads = b.owner_seen.len();
    let mut exact = Observer { remaining: reads };
    assert!(observe(&b, &mut exact).is_some());
    assert_eq!(exact.remaining, 0);
    let mut short = Observer {
        remaining: reads - 1,
    };
    assert!(observe(&b, &mut short).is_none());
    assert_eq!(short.remaining, 0);
    assert!(short.before_read().is_none());
    assert!(collect(&b, total).is_some());
    assert_eq!(
        (
            b.visits,
            b.import_visits,
            b.owner_payload.visits,
            b.shared_payload.visits,
            b.certified_shared_visits,
            b.failed_budget
        ),
        before
    );
    assert_eq!(
        b.owner_seen
            .values()
            .map(|v| v.storage_identity())
            .collect::<Vec<_>>(),
        guards
    );
    // Arena, LOCAL source-reference and unique-source reads each need their
    // own record; none can borrow a later record or return partial totals.
    let mut source_builder = PublishedValueArenaBuilder::new();
    code(&mut source_builder, source(2));
    let arena = seal(source_builder, 0);
    let mut source_draft = PublishedValueArenaBuilder::new_strict_shared_audit();
    source_draft.import(arena).unwrap();
    let source_total = source_draft.complete_shape().unwrap().visits;
    assert!(collect(&source_draft, source_total).is_some());
    let mut exact = Observer { remaining: 3 };
    assert!(observe(&source_draft, &mut exact).is_some());
    assert_eq!(exact.remaining, 0);
    for remaining in 0..3 {
        let mut short = Observer { remaining };
        assert!(observe(&source_draft, &mut short).is_none());
        assert_eq!(short.remaining, 0);
    }
    b.owner_seen.reserve(4 * NODE_LIMIT + 1);
    assert!(b.owner_seen.capacity() > 4 * NODE_LIMIT);
    assert!(
        envelope(&b, total).is_none(),
        "capacity refusal precedes creation of any table iterator"
    );
    assert!(collect(&b, total).is_none());
    assert_eq!(
        (
            b.visits,
            b.import_visits,
            b.owner_payload.visits,
            b.shared_payload.visits,
            b.certified_shared_visits,
            b.failed_budget
        ),
        before
    );
}

#[test]
fn unavailable_census_never_returns_partial_or_inconsistent_private_state() {
    let mut b = domain_draft();
    let total = b.complete_shape().unwrap().visits;
    b.owner_payload.visits += 1;
    assert!(collect(&b, total).is_none());
    b.owner_payload.visits -= 1;
    b.import_visits = 1;
    assert!(collect(&b, total + 1).is_none());
    b.import_visits = 0;
    b.visits = usize::MAX;
    assert!(collect(&b, usize::MAX).is_none());
    let mut row = PublicationOwnerDomainCensus {
        count: usize::MAX,
        ..PublicationOwnerDomainCensus::default()
    };
    assert!(row.add(1).is_none());
    let mut row = PublicationOwnerDomainCensus {
        intrinsic_visits: usize::MAX,
        ..PublicationOwnerDomainCensus::default()
    };
    assert!(row.add(1).is_none());
    let mut b = PublishedValueArenaBuilder::new_strict_shared_audit();
    b.visits = 1;
    b.rows.push(None);
    assert!(collect(&b, 1).is_none());
    b.rows.clear();
    b.certified_shared_visits = 1;
    assert!(collect(&b, 1).is_none());
    b.certified_shared_visits = 0;
    b.strict_shared_audit = false;
    assert!(collect(&b, 1).is_none());
}

#[test]
fn complete_fixed_census_formatting_and_layout_remain_bounded() {
    use std::mem::align_of;
    use std::mem::size_of;
    assert_eq!(
        size_of::<PublicationOwnerDomainCensus>(),
        3 * size_of::<usize>()
    );
    assert_eq!(
        size_of::<PublicationClosureCensus>(),
        15 * size_of::<usize>() + size_of::<Option<PublicationNativeRetentionCensus>>()
    );
    assert_eq!(align_of::<PublicationClosureCensus>(), align_of::<usize>());
    let domain = PublicationOwnerDomainCensus {
        count: usize::MAX,
        intrinsic_visits: usize::MAX,
        maximum_intrinsic_visits: usize::MAX,
    };
    let census = PublicationClosureCensus {
        bzl_load: domain.clone(),
        configured_rule: domain.clone(),
        configured_aspect: domain,
        imported_arenas: usize::MAX,
        imported_local_visits: usize::MAX,
        source_references: usize::MAX,
        imported_source_body_facts: usize::MAX,
        unique_sources: usize::MAX,
        unique_source_body_facts: usize::MAX,
        native_retention: None,
    };
    // Scalar-only formatting fixture, never a successful observed certificate.
    let mut b = PublishedValueArenaBuilder::new_strict_shared_audit();
    b.visits = usize::MAX;
    let error = b.owner_retained_visit_refusal(
        FunctionOwnerDomain::ConfiguredAspect,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );
    let PublishedValueError::OwnerRetainedVisitBudget(mut report) = error else {
        unreachable!()
    };
    report.closure_census = Some(census.clone());
    assert_eq!(report.closure_census(), Some(&census));
    let error = PublishedValueError::OwnerRetainedVisitBudget(report);
    assert_eq!(error, error.clone());
    assert!(error.is_limit());
    assert!(error.to_string().len() <= 2048);
    assert!(format!("{error:?}").len() <= 2048);
    assert!(error.to_string().contains("owner retained visits"));
}
