use super::*;

#[derive(Debug, Allocative)]
struct Fact {
    semantic_id: u8,
    shape: PublicationShape,
}
impl ImmutablePublicationInput for Fact {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        self.shape
    }
}
fn fact(id: u8) -> Arc<Fact> {
    Arc::new(Fact {
        semantic_id: id,
        shape: PublicationShape {
            nodes: 1,
            edges: 2,
            bytes: 128,
            visits: 100,
            depth: 3,
        },
    })
}
#[derive(Debug, Allocative)]
struct Inputs {
    semantic_ids: Vec<u8>,
    inputs: Vec<SharedPublicationInput>,
}
impl StructuralFunctionOwnerPart for Inputs {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|o| self.semantic_ids == o.semantic_ids)
    }
    fn hash_part(&self, h: &mut dyn std::hash::Hasher) {
        h.write(&self.semantic_ids);
    }
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.semantic_ids.len()
            + self.inputs.len() * std::mem::size_of::<SharedPublicationInput>()
            + 4 * std::mem::size_of::<usize>()
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &self.inputs
    }
}
fn owner(f: &Arc<Fact>) -> FunctionEvaluationOwner {
    let part = Arc::new(Inputs {
        semantic_ids: vec![f.semantic_id],
        inputs: vec![SharedPublicationInput::new(f.clone())],
    });
    let empty = Arc::new(Inputs {
        semantic_ids: vec![],
        inputs: vec![],
    });
    FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, empty, part).unwrap()
}
#[derive(Debug)]
struct Originals(Vec<PublishedValueArena>);
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        0
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.0
    }
}
fn seal(b: PublishedValueArenaBuilder) -> PublishedValueArena {
    let imports = b.imports().to_vec();
    b.seal(Arc::new(Originals(imports))).unwrap()
}
#[test]
fn shared_leaf_payload_is_counted_once_across_distinct_owners_in_both_orders() {
    let f = fact(1);
    let a = owner(&f);
    let b = owner(&f);
    assert_eq!(a, b);
    for (first, second) in [(&a, &b), (&b, &a)] {
        let mut audit = PublishedValueArenaBuilder::new();
        audit.audit_owner_shape(first).unwrap();
        audit.audit_owner_shape(second).unwrap();
        assert_eq!(audit.shared_seen.len(), 1);
        assert_eq!(audit.shared_payload.visits, 100);
        assert_eq!(audit.local_shape().nodes - audit.intrinsic_shape().nodes, 1);
        let separately_allocated = fact(1);
        let c = owner(&separately_allocated);
        assert_eq!(a, c);
        audit.audit_owner_shape(&c).unwrap();
        assert_eq!(audit.shared_seen.len(), 2);
        assert_eq!(audit.shared_payload.visits, 200);
    }
}
#[test]
fn shared_leaf_guards_and_imported_owner_roots_use_the_same_memo() {
    let f = fact(2);
    let weak = Arc::downgrade(&f);
    let a = owner(&f);
    let mut prior = PublishedValueArenaBuilder::new();
    prior.audit_owner_shape(&a).unwrap();
    let prior = seal(prior);
    let b = owner(&f);
    for import_first in [true, false] {
        let mut audit = PublishedValueArenaBuilder::new();
        if import_first {
            audit.import(prior.clone()).unwrap();
        }
        audit.audit_owner_shape(&b).unwrap();
        audit.import(prior.clone()).unwrap();
        assert_eq!(audit.shared_seen.len(), 1);
        assert_eq!(audit.shared_payload.visits, 100);
        assert_eq!(audit.shared_payload.bytes, 128);
    }
    let mut audit = PublishedValueArenaBuilder::new();
    audit.audit_owner_shape(&b).unwrap();
    drop(a);
    drop(b);
    drop(prior);
    drop(f);
    assert!(weak.upgrade().is_some());
    drop(audit);
    assert!(weak.upgrade().is_none());
}
#[test]
fn shared_leaf_quota_and_deeper_memo_hits_refuse_before_guard_growth() {
    let f = fact(3);
    let input = SharedPublicationInput::new(f.clone());
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_shared_input(&input, 0).unwrap();
    b.audit_shared_input(&input, DEPTH_LIMIT - f.shape.depth)
        .unwrap();
    assert_eq!(b.shared_seen.len(), 1);
    assert!(
        b.audit_shared_input(&input, DEPTH_LIMIT - f.shape.depth + 1)
            .is_err()
    );
    assert!(b.seal(Arc::new(Originals(vec![]))).is_err());
    let huge = SharedPublicationInput::new(Arc::new(Fact {
        semantic_id: 4,
        shape: PublicationShape {
            bytes: BYTE_LIMIT + 1,
            ..PublicationShape::default()
        },
    }));
    let mut b = PublishedValueArenaBuilder::new();
    assert!(b.audit_shared_input(&huge, 0).is_err());
    assert!(b.shared_seen.is_empty());
    assert!(b.audit_shared_input(&input, 0).is_err());
    assert!(b.shared_seen.is_empty());
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_shared_input(&input, 0).unwrap();
    while b.charged_visits() < NODE_LIMIT {
        b.charge_visit().unwrap();
    }
    assert!(b.audit_shared_input(&input, 0).is_err());
    assert_eq!(b.shared_seen.len(), 1); // A hit still consumes reference work.
}
#[test]
fn shared_leaf_constructor_preflight_keeps_full_isolated_closure_bounds() {
    let giant = fact(5);
    let giant = Arc::new(Fact {
        semantic_id: giant.semantic_id,
        shape: PublicationShape {
            visits: NODE_LIMIT,
            ..giant.shape
        },
    });
    let part = Arc::new(Inputs {
        semantic_ids: vec![5],
        inputs: vec![SharedPublicationInput::new(giant)],
    });
    let empty = Arc::new(Inputs {
        semantic_ids: vec![],
        inputs: vec![],
    });
    assert!(FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, empty, part).is_err());
}

fn fact_with_visits(id: u8, visits: usize) -> Arc<Fact> {
    let original = fact(id);
    Arc::new(Fact {
        semantic_id: id,
        shape: PublicationShape {
            visits,
            ..original.shape
        },
    })
}
fn shape_fields(s: PublicationShape) -> (usize, usize, usize, usize, usize) {
    (s.nodes, s.edges, s.bytes, s.visits, s.depth)
}
fn pad_work(b: &mut PublishedValueArenaBuilder, remaining: usize) {
    while b.charged_visits() < NODE_LIMIT - remaining {
        b.charge_visit().unwrap();
    }
}

#[test]
fn completed_owner_and_raw_orders_keep_full_facts_without_refunding_paid_work() {
    let f = fact(6);
    let own = owner(&f);
    let same_owner = owner(&f);
    let input = SharedPublicationInput::new(f.clone());
    let mut raw_first = PublishedValueArenaBuilder::new();
    raw_first.audit_shared_input(&input, 0).unwrap();
    raw_first.audit_owner_shape(&own).unwrap();
    let mut certified_first = PublishedValueArenaBuilder::new();
    certified_first.audit_owner_shape(&own).unwrap();
    certified_first.audit_shared_input(&input, 0).unwrap();
    assert_eq!(raw_first.certified_shared_visits, 0);
    assert_eq!(raw_first.charged_visits(), 106);
    assert_eq!(certified_first.certified_shared_visits, 100);
    assert_eq!(certified_first.charged_visits(), 6);
    assert_eq!(
        shape_fields(raw_first.local_shape()),
        shape_fields(certified_first.local_shape())
    );
    assert_eq!(
        shape_fields(raw_first.intrinsic_shape()),
        shape_fields(certified_first.intrinsic_shape())
    );
    assert_eq!(raw_first.shared_payload.visits, 100);
    assert_eq!(certified_first.shared_payload.visits, 100);
    let raw_arena = seal(raw_first);
    let certified_arena = seal(certified_first);
    assert_eq!(
        shape_fields(raw_arena.shape()),
        shape_fields(certified_arena.shape())
    );
    assert_eq!(own, same_owner);
    use std::hash::Hash;
    use std::hash::Hasher;
    let hash = |v: &FunctionEvaluationOwner| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        v.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(&own), hash(&same_owner));
    let mut imported = PublishedValueArenaBuilder::new();
    imported.import(certified_arena.clone()).unwrap();
    imported.audit_owner_shape(&same_owner).unwrap();
    imported.import(raw_arena).unwrap();
    assert_eq!(
        (
            imported.shared_seen.len(),
            imported.shared_payload.visits,
            imported.certified_shared_visits
        ),
        (1, 100, 100)
    );
    // Equal semantic input is not the actual guard carried by that owner.
    let foreign = fact(6);
    imported
        .audit_shared_input(&SharedPublicationInput::new(foreign.clone()), 0)
        .unwrap();
    assert_eq!(
        (
            imported.shared_seen.len(),
            imported.shared_payload.visits,
            imported.certified_shared_visits
        ),
        (2, 200, 100)
    );
    let foreign_owner = owner(&foreign);
    imported.audit_owner_shape(&foreign_owner).unwrap();
    assert_eq!(
        imported.certified_shared_visits, 100,
        "a raw body is never promoted later"
    );
    let weak = Arc::downgrade(&f);
    drop((input, own, same_owner, f, certified_arena));
    assert!(weak.upgrade().is_some());
    drop(imported);
    assert!(weak.upgrade().is_none());
}

#[derive(Debug, Allocative)]
struct Links {
    owners: Vec<FunctionEvaluationOwner>,
    imports: Vec<PublishedValueArena>,
}
impl StructuralFunctionOwnerPart for Links {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().downcast_ref::<Self>().is_some_and(|p| {
            self.owners == p.owners
                && self.imports.len() == p.imports.len()
                && self
                    .imports
                    .iter()
                    .zip(&p.imports)
                    .all(|(a, b)| a.identity() == b.identity())
        })
    }
    fn hash_part(&self, h: &mut dyn std::hash::Hasher) {
        h.write_usize(self.owners.len());
        for own in &self.owners {
            own.stable_key().hash_part(h);
            own.input_validity().hash_part(h);
        }
        h.write_usize(self.imports.len());
        for arena in &self.imports {
            h.write_usize(Arc::as_ptr(&arena.0) as usize);
        }
    }
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.owners.capacity() * std::mem::size_of::<FunctionEvaluationOwner>()
            + self.imports.capacity() * std::mem::size_of::<PublishedValueArena>()
    }
    fn retained_owners(&self) -> &[FunctionEvaluationOwner] {
        &self.owners
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
}
#[test]
fn strict_constructor_replays_shared_facts_through_recursive_and_imported_owners() {
    let f = fact(7);
    let own = owner(&f);
    let mut strict = PublishedValueArenaBuilder::new_strict_shared_audit();
    strict.audit_owner_shape(&own).unwrap();
    assert_eq!(
        (strict.charged_visits(), strict.certified_shared_visits),
        (104, 0)
    );
    let mut normal = PublishedValueArenaBuilder::new();
    normal.audit_owner_shape(&own).unwrap();
    assert_eq!(
        (normal.charged_visits(), normal.certified_shared_visits),
        (4, 100)
    );
    assert_eq!(
        shape_fields(strict.local_shape()),
        shape_fields(normal.local_shape())
    );
    // Raw strict body+encounter4 and positive completion29 stay W<=N.
    let heavy = fact_with_visits(8, NODE_LIMIT - 33);
    let heavy_owner = owner(&heavy); // Original strict prefix + completion exactly N.
    let mut consumer = PublishedValueArenaBuilder::new();
    consumer.audit_owner_shape(&heavy_owner).unwrap();
    assert_eq!(
        (consumer.charged_visits(), consumer.certified_shared_visits),
        (4, NODE_LIMIT - 33)
    );
    let arena = seal(consumer);
    for (owners, imports) in [
        (vec![heavy_owner.clone()], vec![]),
        (vec![], vec![arena.clone()]),
    ] {
        let part = Arc::new(Links { owners, imports });
        let empty = Arc::new(Inputs {
            semantic_ids: vec![],
            inputs: vec![],
        });
        let refs = Arc::strong_count(&part);
        assert!(
            FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, empty, part.clone())
                .is_err()
        );
        assert_eq!(
            Arc::strong_count(&part),
            refs,
            "failed strict audit publishes no owner/borrow witness"
        );
    }
}

#[test]
fn certified_shared_work_shared_facts_and_joint_cached_facts_have_independent_caps() {
    let a = owner(&fact_with_visits(9, NODE_LIMIT / 2));
    let b = owner(&fact_with_visits(10, NODE_LIMIT / 2));
    let mut full = PublishedValueArenaBuilder::new();
    full.audit_owner_shape(&a).unwrap();
    full.audit_owner_shape(&b).unwrap();
    assert_eq!(
        (
            full.charged_visits(),
            full.shared_payload.visits,
            full.certified_shared_visits
        ),
        (8, NODE_LIMIT, NODE_LIMIT)
    );
    assert!(seal(full).shape().visits >= NODE_LIMIT);
    let mut refused = PublishedValueArenaBuilder::new();
    refused.audit_owner_shape(&a).unwrap();
    refused.audit_owner_shape(&b).unwrap();
    let extra = owner(&fact_with_visits(11, 1));
    assert!(matches!(
        refused.audit_owner_shape(&extra),
        Err(PublishedValueError::Limit("shared visits"))
    ));
    assert_eq!(
        (
            refused.shared_seen.len(),
            refused.shared_payload.visits,
            refused.certified_shared_visits
        ),
        (2, NODE_LIMIT, NODE_LIMIT)
    );
    assert!(refused.charge_visit().is_err());
    for remaining in [4, 3, 2, 1] {
        let mut tight = PublishedValueArenaBuilder::new();
        pad_work(&mut tight, remaining);
        assert_eq!(tight.audit_owner_shape(&a).is_ok(), remaining == 4);
        assert_eq!(tight.charged_visits(), NODE_LIMIT);
        assert_eq!(
            tight.certified_shared_visits,
            if remaining >= 3 { NODE_LIMIT / 2 } else { 0 }
        );
        if remaining < 4 {
            assert!(tight.charge_bytes(0).is_err());
        }
    }
    use crate::published_value::builder::import_accounting::tests::leaf;
    // Keep the SAME aggregate S=N-4 using two genuinely constructible
    // certificates; each pays the full strict body plus its completion work.
    let heavy = owner(&fact_with_visits(12, (NODE_LIMIT - 4) / 2));
    let tail = owner(&fact_with_visits(15, (NODE_LIMIT - 4) / 2));
    // Cached certificates omit actual stored reference facts. Final seal still includes both
    // the import and owner-root references, independently of W/R/S admission.
    for extra in [0, 1] {
        let mut final_bound = PublishedValueArenaBuilder::new();
        final_bound.import(leaf(NODE_LIMIT - 3 + extra)).unwrap();
        final_bound.audit_owner_shape(&heavy).unwrap();
        final_bound.audit_owner_shape(&tail).unwrap();
        assert_eq!(final_bound.charged_visits(), 10);
        assert_eq!(final_bound.shared_payload.visits, NODE_LIMIT - 4);
        assert_eq!(
            final_bound.checked_cached_visits(),
            Some(RETAINED_FACT_LIMIT - 3 + extra)
        );
        let imports = final_bound.imports().to_vec();
        let result = final_bound.seal(Arc::new(Originals(imports)));
        if extra == 0 {
            assert_eq!(result.unwrap().shape().visits, RETAINED_FACT_LIMIT);
        } else {
            let Err(PublishedValueError::RetainedFactBudget(report)) = result else {
                panic!("expected full canonical refusal, got {result:?}");
            };
            assert_eq!(
                (
                    report.imported_facts(),
                    report.owner_facts(),
                    report.shared_facts(),
                    report.local_facts()
                ),
                (NODE_LIMIT - 2, 4, NODE_LIMIT - 4, 3)
            );
            assert_eq!(report.certified_shared_facts(), NODE_LIMIT - 4);
            // Entry10 + view8 + source setup1 + empty backing1 + empty slots1 + refs1.
            // R=0 adds positive1; all S/I/local exact-F and F+1 assertions stay.
            assert_eq!(report.producer_work(), 22);
            assert_eq!(
                (
                    report.local_source_facts(),
                    report.local_native_facts(),
                    report.local_other_facts()
                ),
                (0, 0, 3)
            );
            assert_eq!(report.checked_total_facts(), Some(RETAINED_FACT_LIMIT + 1));
            assert_eq!(report.limit_facts(), RETAINED_FACT_LIMIT);
        }
    }
    let prior = leaf(NODE_LIMIT - 1);
    let later = leaf(4);
    let mut cached = PublishedValueArenaBuilder::new();
    cached.import(prior.clone()).unwrap();
    cached.audit_owner_shape(&heavy).unwrap();
    cached.audit_owner_shape(&tail).unwrap();
    assert_eq!(
        cached.checked_cached_visits(),
        Some(RETAINED_FACT_LIMIT - 1)
    );
    let accepted = (
        cached.import_visits,
        cached.certified_shared_visits,
        cached.shared_seen.len(),
    );
    assert!(matches!(
        cached.import(later.clone()),
        Err(PublishedValueError::Limit("import visits"))
    ));
    assert_eq!(
        (
            cached.import_visits,
            cached.certified_shared_visits,
            cached.shared_seen.len()
        ),
        accepted
    );
    let mut reverse = PublishedValueArenaBuilder::new();
    reverse.import(prior).unwrap();
    reverse.import(later).unwrap();
    reverse.audit_owner_shape(&heavy).unwrap();
    let accepted_reverse = (
        reverse.certified_shared_visits,
        reverse.shared_payload.visits,
        reverse.shared_seen.len(),
    );
    assert_eq!(
        accepted_reverse,
        ((NODE_LIMIT - 4) / 2, (NODE_LIMIT - 4) / 2, 1)
    );
    assert!(matches!(
        reverse.audit_owner_shape(&tail),
        Err(PublishedValueError::Limit("shared certificate visits"))
    ));
    assert_eq!(
        (
            reverse.certified_shared_visits,
            reverse.shared_payload.visits,
            reverse.shared_seen.len()
        ),
        accepted_reverse
    );
    assert!(reverse.charge_visit().is_err());
}

#[test]
fn nonzero_certified_shared_facts_preserve_native_deltas_and_byte_failure_work() {
    use crate::AnalysisValue;
    use crate::cc_features::CcExecOs;
    use crate::cc_features::CcToolchainFeatures;
    let own = owner(&fact(13));
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_owner_shape(&own).unwrap();
    let mut reference = PublishedValueArenaBuilder::new();
    let plain = reference.native_atom(AnalysisValue::none()).unwrap();
    let before = b.visits;
    let with_shared = b.native_atom(AnalysisValue::none()).unwrap();
    assert_eq!(with_shared.retained_visits(), plain.retained_visits());
    assert_eq!(b.visits - before, plain.retained_visits());
    let features =
        CcToolchainFeatures::new(vec![], vec![], vec![], "tools", CcExecOs::Linux).unwrap();
    let value = AnalysisValue::cc_toolchain_features(features);
    let first = b.native_atom_borrowed(&value).unwrap();
    let hit = b.native_atom_borrowed(&value).unwrap();
    let reference = reference.native_atom_borrowed(&value).unwrap();
    assert_eq!(first.retained_visits(), reference.retained_visits());
    assert_eq!(hit.retained_visits(), first.retained_visits());
    assert_eq!(
        (
            b.certified_shared_visits,
            b.owner_payload.visits,
            b.shared_payload.visits
        ),
        (100, 2, 100)
    );
    let work = b.charged_visits();
    b.charge_bytes(BYTE_LIMIT - b.charged_bytes()).unwrap();
    let error = b.charge_bytes(1).unwrap_err();
    let PublishedValueError::ByteBudget(report) = error else {
        panic!("expected byte refusal")
    };
    assert_eq!(report.producer_work(), Some(work));
    assert!(!report.prior_poisoned());
    assert_eq!(report.shared_bytes(), 128);
    assert_eq!(b.shared_seen.len(), 1);
    assert!(b.native_atom(AnalysisValue::none()).is_err());
}

#[test]
fn completed_shared_body_refusal_preserves_guard_counts_and_poisons_before_reuse() {
    let f = fact(14);
    let own = owner(&f);
    let mut reference = PublishedValueArenaBuilder::new();
    reference
        .audit_shared_input(&SharedPublicationInput::new(f.clone()), 0)
        .unwrap();
    let prefix_bytes = reference.charged_bytes() - f.shape.bytes;
    drop(reference);
    for resource in 0..4 {
        let mut b = PublishedValueArenaBuilder::new();
        match resource {
            0 => b.charge_bytes(BYTE_LIMIT - prefix_bytes - 127).unwrap(),
            1 => b.charge_owner_edges(EDGE_LIMIT - 2).unwrap(),
            2 => b
                .charge_storage(crate::DepsetStorageCharge {
                    nodes: NODE_LIMIT,
                    edges: 0,
                    bytes: 0,
                    visits: 0,
                })
                .unwrap(),
            _ => pad_work(&mut b, 0),
        }
        let refs = Arc::strong_count(&f);
        let witness = own.certified_shared_inputs().next().unwrap();
        let error = b.audit_owner_shared_input(witness, 0).unwrap_err();
        if resource == 0 {
            let PublishedValueError::ByteBudget(report) = error else {
                panic!("expected body byte refusal")
            };
            assert_eq!(report.context(), PublicationByteContext::SharedBody);
            assert_eq!(report.producer_work(), Some(2));
        } else if resource == 1 {
            let PublishedValueError::EdgeBudget(report) = error else {
                panic!("expected body edge refusal")
            };
            assert_eq!(report.context(), PublicationEdgeContext::SharedBody);
        }
        assert_eq!(
            (
                b.certified_shared_visits,
                b.shared_payload.visits,
                b.shared_seen.len()
            ),
            (0, 0, 0)
        );
        assert_eq!(Arc::strong_count(&f), refs);
        assert!(b.audit_owner_shape(&own).is_err());
        assert_eq!(Arc::strong_count(&f), refs);
        assert!(b.seal(Arc::new(Originals(vec![]))).is_err());
    }
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_owner_shape(&own).unwrap();
    let refs = Arc::strong_count(&f);
    let witness = own.certified_shared_inputs().next().unwrap();
    assert!(
        b.audit_owner_shared_input(witness, DEPTH_LIMIT - f.shape.depth + 1)
            .is_err()
    );
    assert_eq!(
        (
            b.certified_shared_visits,
            b.shared_payload.visits,
            b.shared_seen.len()
        ),
        (100, 100, 1)
    );
    assert_eq!(
        Arc::strong_count(&f),
        refs,
        "a refused hit neither drops nor clones prior guards"
    );
    assert!(b.charge_visit().is_err());
}

#[path = "shared_input_dag_tests.rs"]
mod dag_tests;

#[path = "shared_input_public_tests.rs"]
mod public_tests;
