use super::*;

#[test]
fn expanded_diamond_keeps_complete_shape_but_admits_only_actual_unique_bodies() {
    let f = fact_with_visits(1, NODE_LIMIT / 2);
    let root = composite(vec![SharedPublicationInput::new(f.clone()); 2]);
    assert!(root.full.visits > NODE_LIMIT);
    let input = SharedPublicationInput::new(root.clone());
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_shared_input(&input, 0).unwrap();
    assert_eq!(b.shared_payload.visits, 5 + NODE_LIMIT / 2);
    assert_eq!(b.charged_visits(), 5 + NODE_LIMIT / 2 + 6);
    assert_eq!(b.shared_seen.len(), 2);
    assert!(b.shared_active.is_empty());
    assert_eq!(
        shape_fields(input.retained_shape()),
        shape_fields(root.full)
    );

    // Equal semantic leaf content is not the same actual accounting guard.
    let foreign = fact_with_visits(1, NODE_LIMIT / 2);
    let foreign_root = composite(vec![
        SharedPublicationInput::new(f),
        SharedPublicationInput::new(foreign),
    ]);
    let mut b = PublishedValueArenaBuilder::new();
    assert!(matches!(
        b.audit_shared_input(&SharedPublicationInput::new(foreign_root), 0),
        Err(PublishedValueError::Limit("shared visits"))
    ));
    assert_eq!(b.shared_seen.len(), 1); // first leaf only; parent incomplete
    assert_eq!(b.shared_payload.visits, 5 + NODE_LIMIT / 2);
    assert!(b.shared_active.is_empty());
    assert!(b.seal(Arc::new(Originals(vec![]))).is_err());
}

#[test]
fn composite_strict_certified_orders_import_and_reseal_keep_full_facts() {
    let f = fact(2);
    let root = composite(vec![SharedPublicationInput::new(f.clone()); 2]);
    let input = SharedPublicationInput::new(root);
    let owner = composite_owner(input.clone());
    let mut strict = PublishedValueArenaBuilder::new_strict_shared_audit();
    strict.audit_owner_shape(&owner).unwrap();
    assert_eq!(
        (strict.shared_payload.visits, strict.certified_shared_visits),
        (105, 0)
    );
    assert_eq!(strict.charged_visits(), 113); // 105 + six encounters + owner/root
    for raw_first in [false, true] {
        let mut b = PublishedValueArenaBuilder::new();
        if raw_first {
            b.audit_shared_input(&input, 0).unwrap();
        }
        b.audit_owner_shape(&owner).unwrap();
        b.audit_shared_input(&input, 0).unwrap();
        assert_eq!(b.shared_payload.visits, 105);
        assert_eq!(b.certified_shared_visits, if raw_first { 0 } else { 105 });
        assert_eq!(b.shared_seen.len(), 2);
        let prior = seal(b);
        let mut imported = PublishedValueArenaBuilder::new();
        imported.import(prior).unwrap();
        imported.audit_owner_shape(&owner).unwrap();
        assert_eq!(
            (
                imported.shared_payload.visits,
                imported.certified_shared_visits
            ),
            (105, 105)
        );
        assert_eq!(imported.shared_seen.len(), 2);
        assert!(seal(imported).shape().visits >= 105);
    }
    // Mixed paths classify each actual node independently; no raw body refund.
    let mut child_raw = PublishedValueArenaBuilder::new();
    child_raw
        .audit_shared_input(&SharedPublicationInput::new(f.clone()), 0)
        .unwrap();
    child_raw.audit_owner_shape(&owner).unwrap();
    assert_eq!(
        (
            child_raw.shared_payload.visits,
            child_raw.certified_shared_visits
        ),
        (105, 5)
    );
    let mut child_certified = PublishedValueArenaBuilder::new();
    child_certified
        .audit_owner_shape(&super::owner(&f))
        .unwrap();
    child_certified.audit_shared_input(&input, 0).unwrap();
    assert_eq!(
        (
            child_certified.shared_payload.visits,
            child_certified.certified_shared_visits
        ),
        (105, 100)
    );
    // Strict recursion through a completed owner/import still replays raw S.
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_owner_shape(&owner).unwrap();
    let arena = seal(b);
    let mut strict = PublishedValueArenaBuilder::new_strict_shared_audit();
    strict.import(arena).unwrap();
    assert_eq!(
        (strict.shared_payload.visits, strict.certified_shared_visits),
        (105, 0)
    );
}

#[test]
fn extracted_composite_retains_closed_guards_and_releases_after_last_audit() {
    let f = fact(3);
    let weak = Arc::downgrade(&f);
    let root = composite(vec![SharedPublicationInput::new(f.clone())]);
    let weak_root = Arc::downgrade(&root);
    let input = SharedPublicationInput::new(root.clone());
    let owner = composite_owner(input.clone());
    drop((owner, root, f));
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_shared_input(&input, 0).unwrap();
    assert_eq!(b.shared_seen.len(), 2);
    drop(input);
    assert!(weak_root.upgrade().is_some());
    assert!(weak.upgrade().is_some());
    drop(b);
    assert!(weak_root.upgrade().is_none());
    assert!(weak.upgrade().is_none());
}
