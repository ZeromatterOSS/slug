use super::*;

#[test]
fn complete_depth_is_checked_on_hits_and_overflow_before_guard_growth() {
    let root = composite(vec![SharedPublicationInput::new(fact(4))]);
    let input = SharedPublicationInput::new(root.clone());
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_shared_input(&input, 0).unwrap();
    b.audit_shared_input(&input, DEPTH_LIMIT - root.full.depth)
        .unwrap();
    let refs = Arc::strong_count(&root);
    assert!(
        b.audit_shared_input(&input, DEPTH_LIMIT - root.full.depth + 1)
            .is_err()
    );
    assert_eq!(Arc::strong_count(&root), refs);
    assert_eq!(b.shared_seen.len(), 2);
    let mut b = PublishedValueArenaBuilder::new();
    assert!(matches!(
        b.audit_shared_input(&input, usize::MAX),
        Err(PublishedValueError::Limit("closure depth"))
    ));
    assert!(b.shared_seen.is_empty());
    assert!(b.shared_active.is_empty());
}

#[test]
fn child_failure_never_completes_parent_and_keeps_paid_partial_guards_until_drop() {
    let good = fact(5);
    let bad = Arc::new(Fact {
        semantic_id: 6,
        shape: PublicationShape {
            bytes: BYTE_LIMIT + 1,
            ..good.shape
        },
    });
    let weak_good = Arc::downgrade(&good);
    let weak_bad = Arc::downgrade(&bad);
    let root = composite(vec![
        SharedPublicationInput::new(good.clone()),
        SharedPublicationInput::new(bad.clone()),
    ]);
    let weak_root = Arc::downgrade(&root);
    let input = SharedPublicationInput::new(root.clone());
    let bad_refs = Arc::strong_count(&bad);
    let mut b = PublishedValueArenaBuilder::new();
    assert!(b.audit_shared_input(&input, 0).is_err());
    assert_eq!(Arc::strong_count(&bad), bad_refs);
    assert_eq!(b.shared_seen.len(), 1);
    assert!(!b.shared_seen.contains_key(&input.storage_identity()));
    assert!(b.shared_active.is_empty());
    assert_eq!(b.shared_payload.visits, 105);
    assert!(b.audit_shared_input(&input, 0).is_err());
    drop((input, root, good, bad));
    assert!(weak_root.upgrade().is_none());
    assert!(weak_bad.upgrade().is_none());
    assert!(weak_good.upgrade().is_some());
    drop(b);
    assert!(weak_good.upgrade().is_none());
}

#[test]
fn default_leaf_and_composite_boundaries_preserve_work_storage_and_guard_preflight() {
    // Existing default trait implementation still has its old full-body gate.
    let huge = fact_with_visits(7, NODE_LIMIT + 1);
    let input = SharedPublicationInput::new(huge.clone());
    let refs = Arc::strong_count(&huge);
    let mut b = PublishedValueArenaBuilder::new();
    assert!(matches!(
        b.audit_shared_input(&input, 0),
        Err(PublishedValueError::Limit("storage visits"))
    ));
    assert_eq!(Arc::strong_count(&huge), refs);
    assert!(b.shared_seen.is_empty());

    // Two occurrence credits precede raw replay; exact W and W+1 remain N.
    for extra in [0, 1] {
        let f = fact_with_visits(8, NODE_LIMIT - 2 + extra);
        let mut b = PublishedValueArenaBuilder::new();
        assert_eq!(
            b.audit_shared_input(&SharedPublicationInput::new(f), 0)
                .is_ok(),
            extra == 0
        );
        assert_eq!(b.charged_visits(), NODE_LIMIT);
    }
    // Measure the real fixed traversal prefix, then test exact byte boundaries.
    let f = fact(9);
    let mut reference = PublishedValueArenaBuilder::new();
    reference
        .audit_shared_input(&SharedPublicationInput::new(f.clone()), 0)
        .unwrap();
    let prefix = reference.charged_bytes() - f.shape.bytes;
    for extra in [0, 1] {
        let f = Arc::new(Fact {
            semantic_id: 9,
            shape: PublicationShape {
                bytes: BYTE_LIMIT - prefix + extra,
                ..f.shape
            },
        });
        let input = SharedPublicationInput::new(f.clone());
        let refs = Arc::strong_count(&f);
        let mut b = PublishedValueArenaBuilder::new();
        assert_eq!(b.audit_shared_input(&input, 0).is_ok(), extra == 0);
        assert_eq!(Arc::strong_count(&f), refs + usize::from(extra == 0));
    }
    for resource in 0..2 {
        for extra in [0, 1] {
            let (nodes, edges) = if resource == 0 {
                (NODE_LIMIT + extra, 0)
            } else {
                (0, EDGE_LIMIT - 1 + extra)
            };
            let f = Arc::new(Fact {
                semantic_id: 10,
                shape: PublicationShape {
                    nodes,
                    edges,
                    ..f.shape
                },
            });
            let mut b = PublishedValueArenaBuilder::new();
            assert_eq!(
                b.audit_shared_input(&SharedPublicationInput::new(f), 0)
                    .is_ok(),
                extra == 0
            );
            assert_eq!(b.shared_seen.is_empty(), extra == 1);
            assert!(b.shared_active.is_empty());
        }
    }
    let input = SharedPublicationInput::new(f.clone());
    let refs = Arc::strong_count(&f);
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_bytes(BYTE_LIMIT - super::super::SHARED_FRAME_BYTES)
        .unwrap();
    assert!(b.audit_shared_input(&input, 0).is_err()); // active growth cannot allocate
    assert_eq!(Arc::strong_count(&f), refs);
    assert!(b.shared_active.is_empty());
    assert!(b.shared_seen.is_empty());
    assert_eq!(b.shared_payload.visits, 0);
}

#[derive(Debug, Allocative)]
struct AdversarialCycle {
    children: std::sync::OnceLock<[SharedPublicationInput; 1]>,
}
impl ImmutablePublicationInput for AdversarialCycle {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        self.intrinsic_shape()
    }
    fn intrinsic_shape(&self) -> PublicationShape {
        PublicationShape {
            visits: 1,
            bytes: std::mem::size_of::<Self>(),
            ..PublicationShape::default()
        }
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        self.children.get().unwrap()
    }
}
#[test]
fn adversarial_owned_cycle_is_rejected_without_completing_or_reusing_partial_body() {
    // Deliberately violate the immutable DAG contract. This tiny caller-created
    // strong cycle is intentionally leaked by the fixture, not by the builder;
    // legal success/partial-error lifetime tests above prove normal teardown.
    let a = Arc::new(AdversarialCycle {
        children: std::sync::OnceLock::new(),
    });
    let b = Arc::new(AdversarialCycle {
        children: std::sync::OnceLock::new(),
    });
    a.children
        .set([SharedPublicationInput::new(b.clone())])
        .unwrap();
    b.children
        .set([SharedPublicationInput::new(a.clone())])
        .unwrap();
    let root = SharedPublicationInput::new(a);
    let mut audit = PublishedValueArenaBuilder::new();
    assert!(matches!(
        audit.audit_shared_input(&root, 0),
        Err(PublishedValueError::Invalid("shared input cycle"))
    ));
    assert!(audit.shared_seen.is_empty());
    assert!(audit.shared_active.is_empty());
    assert_eq!(audit.shared_payload.visits, 2);
    assert!(audit.seal(Arc::new(Originals(vec![]))).is_err());
}
