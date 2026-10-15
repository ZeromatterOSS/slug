use super::*;
#[derive(Debug, allocative::Allocative)]
struct Dependencies {
    owners: Vec<FunctionEvaluationOwner>,
    arenas: Vec<PublishedValueArena>,
}
impl StructuralFunctionOwnerPart for Dependencies {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|p| p.owners == self.owners)
    }
    fn hash_part(&self, h: &mut dyn std::hash::Hasher) {
        use std::hash::Hasher;
        for o in &self.owners {
            let mut nested = fxhash::FxHasher::default();
            o.hash_fields(&mut nested);
            h.write_u64(nested.finish());
        }
    }
    fn retained_bytes(&self) -> usize {
        32 + self.owners.len() * std::mem::size_of::<FunctionEvaluationOwner>()
            + self.arenas.len() * std::mem::size_of::<PublishedValueArena>()
    }
    fn retained_owners(&self) -> &[FunctionEvaluationOwner] {
        &self.owners
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.arenas
    }
}
fn dependent(
    owners: Vec<FunctionEvaluationOwner>,
    arenas: Vec<PublishedValueArena>,
) -> FunctionEvaluationOwner {
    FunctionEvaluationOwner::new(
        FunctionOwnerDomain::ConfiguredRule,
        Arc::new(ShapePart {
            id: 7,
            shape: PublicationShape {
                bytes: 1,
                visits: 1,
                ..PublicationShape::default()
            },
        }),
        Arc::new(Dependencies { owners, arenas }),
    )
    .unwrap()
}
fn owner_payload(b: &PublishedValueArenaBuilder) -> (usize, usize) {
    let all = b.local_shape();
    let own = b.intrinsic_shape();
    (all.nodes - own.nodes, all.visits - own.visits)
}
#[test]
fn actual_owner_dependencies_share_payload_in_both_orders_without_semantic_shortcuts() {
    let child = counted_owner();
    let parent = dependent(vec![child.clone(), child.clone()], vec![]);
    for child_first in [false, true] {
        let mut b = PublishedValueArenaBuilder::new();
        if child_first {
            b.audit_owner_shape(&child).unwrap();
        }
        b.audit_owner_shape(&parent).unwrap();
        b.audit_owner_shape(&child).unwrap();
        assert_eq!(owner_payload(&b), (2, 8));
        let fresh = counted_owner();
        assert_eq!(child, fresh);
        b.audit_owner_shape(&fresh).unwrap();
        assert_eq!(owner_payload(&b), (4, 14));
    }
    assert_ne!(parent, dependent(vec![dependent(vec![], vec![])], vec![]));
}
#[test]
fn imported_arenas_and_owners_share_one_guarded_accounting_graph() {
    let child = counted_owner();
    let mut a = PublishedValueArenaBuilder::new();
    a.audit_owner_shape(&child).unwrap();
    let arena = seal(a, 0);
    assert_eq!(arena.owners().len(), 1);
    let parent = dependent(vec![child.clone()], vec![arena.clone()]);
    for arena_first in [false, true] {
        let mut b = PublishedValueArenaBuilder::new();
        if arena_first {
            b.import(arena.clone()).unwrap();
        }
        b.audit_owner_shape(&parent).unwrap();
        b.audit_owner_shape(&child).unwrap();
        b.import(arena.clone()).unwrap();
        assert_eq!(owner_payload(&b), (2, 8));
        assert_eq!(b.imports().len(), 1);
        assert!(b.local_shape().depth >= child.retained_shape().depth);
    }
}
#[test]
fn deeper_owner_and_arena_memo_hits_recheck_depth_and_poison_failure() {
    let owner = counted_owner();
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_owner_shape(&owner).unwrap();
    assert_eq!(b.intrinsic_shape().depth, 0);
    assert!(b.audit_owner_shape_at_depth(&owner, DEPTH_LIMIT).is_err());
    assert!(
        b.seal(Arc::new(Originals {
            count: 0,
            imports: vec![]
        }))
        .is_err()
    );
    let mut a = PublishedValueArenaBuilder::new();
    a.audit_owner_shape(&owner).unwrap();
    let a = seal(a, 0);
    let mut b = PublishedValueArenaBuilder::new();
    b.import(a.clone()).unwrap();
    // Exercise the completed arena memo directly, not merely a fresh parent's
    // cached-depth gate. The exact maximum context is admitted, the next fails.
    b.audit_arena_closure(&a, DEPTH_LIMIT - a.shape().depth)
        .unwrap();
    assert!(
        b.audit_arena_closure(&a, DEPTH_LIMIT - a.shape().depth + 1)
            .is_err()
    );
    assert!(
        b.seal(Arc::new(Originals {
            count: 0,
            imports: vec![a.clone()]
        }))
        .is_err()
    );
    let mut b = PublishedValueArenaBuilder::new();
    // A new immutable owner may retain an arena which retains an owner. Its
    // complete cached depth includes every dependency hop, not just its body.
    let p = dependent(vec![], vec![a]);
    assert!(p.retained_shape().depth > owner.retained_shape().depth);
    assert!(
        b.audit_owner_shape_at_depth(&p, DEPTH_LIMIT - owner.retained_shape().depth)
            .is_err()
    );
}
#[test]
fn constructor_preflights_complete_dependency_closure_before_publishing_owner() {
    // Two D=N-22 bodies plus a genuine D44 body keep full O=F while
    // each constructor retains positive completion-work headroom.
    let part = |id, visits| {
        Arc::new(ShapePart {
            id,
            shape: PublicationShape {
                visits,
                bytes: 1,
                ..PublicationShape::default()
            },
        })
    };
    let large = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::BzlLoad,
        part(1, (NODE_LIMIT - 22) / 2),
        part(2, (NODE_LIMIT - 22) / 2),
    )
    .unwrap();
    let second = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::BzlLoad,
        part(4, (NODE_LIMIT - 22) / 2),
        part(5, (NODE_LIMIT - 22) / 2),
    )
    .unwrap();
    let tail = FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, part(6, 22), part(7, 22))
        .unwrap();
    assert_eq!(
        large.intrinsic_shape().visits
            + second.intrinsic_shape().visits
            + tail.intrinsic_shape().visits,
        RETAINED_FACT_LIMIT
    );
    let key = Arc::new(ShapePart {
        id: 3,
        shape: PublicationShape {
            bytes: 1,
            visits: 1,
            ..PublicationShape::default()
        },
    });
    let dependencies = Arc::new(Dependencies {
        owners: vec![large, second, tail],
        arenas: vec![],
    });
    let key_weak = Arc::downgrade(&key);
    let dependencies_weak = Arc::downgrade(&dependencies);
    let error =
        FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredRule, key, dependencies)
            .unwrap_err();
    let PublishedValueError::OwnerRetainedVisitBudget(report) = error else {
        panic!("expected final owner refusal, got {error:?}");
    };
    assert_eq!(
        (
            report.domain(),
            report.stable_key_visits(),
            report.input_validity_visits(),
            report.intrinsic_visits(),
            report.legacy_local_visits(),
            report.owner_visits(),
            report.imported_visits(),
            report.shared_visits(),
            report.certified_shared_visits(),
            report.producer_work(),
            report.complete_visits(),
            report.limit_visits()
        ),
        (
            FunctionOwnerDomain::ConfiguredRule,
            1,
            1,
            2,
            RETAINED_FACT_LIMIT + 60,
            RETAINED_FACT_LIMIT,
            0,
            0,
            0,
            Some(60),
            RETAINED_FACT_LIMIT + 5,
            RETAINED_FACT_LIMIT
        )
    );
    assert_eq!(report.declaration_reference_visits(), 3);
    let cloned = report.clone();
    assert_eq!(report, cloned);
    assert!(key_weak.upgrade().is_none());
    assert!(
        dependencies_weak.upgrade().is_none(),
        "numeric errors retain no owner declarations"
    );
}
#[test]
fn dependency_guards_outlive_producer_and_release_with_unpublished_builder() {
    let part = Arc::new(Dependencies {
        owners: vec![],
        arenas: vec![],
    });
    let weak = Arc::downgrade(&part);
    let child =
        FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, part.clone(), part.clone())
            .unwrap();
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_owner_shape(&child).unwrap();
    drop(child);
    drop(part);
    assert!(weak.upgrade().is_some());
    drop(b);
    assert!(weak.upgrade().is_none());
}

#[test]
fn maximum_mixed_owner_arena_depth_is_cached_and_next_hop_is_refused() {
    let mut owner = counted_owner();
    while owner.retained_shape().depth < DEPTH_LIMIT - 2 {
        owner = dependent(vec![owner], vec![]);
    }
    assert_eq!(owner.retained_shape().depth, DEPTH_LIMIT - 2);
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_owner_shape(&owner).unwrap();
    let arena = seal(b, 0);
    assert_eq!(arena.shape().depth, DEPTH_LIMIT - 1);
    let owner = dependent(vec![], vec![arena]);
    assert_eq!(owner.retained_shape().depth, DEPTH_LIMIT);
    let key = Arc::new(ShapePart {
        id: 9,
        shape: PublicationShape {
            bytes: 1,
            ..PublicationShape::default()
        },
    });
    assert!(
        FunctionEvaluationOwner::new(
            FunctionOwnerDomain::ConfiguredRule,
            key,
            Arc::new(Dependencies {
                owners: vec![owner],
                arenas: vec![]
            })
        )
        .is_err()
    );
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_bytes(BYTE_LIMIT).unwrap();
    assert!(
        b.seal(Arc::new(Originals {
            count: 0,
            imports: vec![]
        }))
        .is_err()
    );
}
