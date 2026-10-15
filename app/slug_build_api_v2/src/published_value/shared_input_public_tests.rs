//! Public raw admission retains the existing authority and lifetime boundary.
use super::*;

#[test]
fn public_raw_delegate_preserves_full_body_work_foreign_guards_and_owner_order() {
    let f = fact(21);
    let weak = Arc::downgrade(&f);
    let input = SharedPublicationInput::new(f.clone());
    let own = owner(&f);
    for strict in [false, true] {
        for raw_first in [false, true] {
            let mut b = if strict {
                PublishedValueArenaBuilder::new_strict_shared_audit()
            } else {
                PublishedValueArenaBuilder::new()
            };
            if raw_first {
                let (): () = b.audit_shared_input_at_depth(&input, 0).unwrap();
                assert_eq!(
                    (
                        b.charged_visits(),
                        b.shared_payload.visits,
                        b.certified_shared_visits
                    ),
                    (102, 100, 0)
                );
                assert!(b.owner_seen.is_empty() && b.owners().is_empty());
            }
            b.audit_owner_shape(&own).unwrap();
            if !raw_first {
                b.audit_shared_input_at_depth(&input, 0).unwrap();
            }
            let expected_c = if !strict && !raw_first { 100 } else { 0 };
            assert_eq!(b.certified_shared_visits, expected_c);
            assert_eq!(b.charged_visits(), 106 - expected_c);
            assert_eq!(
                (
                    b.shared_payload.nodes,
                    b.shared_payload.edges,
                    b.shared_payload.bytes,
                    b.shared_payload.visits
                ),
                (f.shape.nodes, f.shape.edges, f.shape.bytes, f.shape.visits)
            );
            assert_eq!(b.local_shape().depth, own.retained_shape().depth);
            assert_eq!(b.local_shape().visits, 108);
            assert_eq!(b.intrinsic_shape().visits, 6);
            assert_eq!(b.shared_seen.len(), 1);
            let foreign = fact(21);
            let foreign_input = SharedPublicationInput::new(foreign.clone());
            let foreign_owner = owner(&foreign);
            assert_eq!(own, foreign_owner);
            let before = b.charged_visits();
            b.audit_shared_input_at_depth(&foreign_input, 0).unwrap();
            assert_eq!(b.charged_visits(), before + 102);
            assert_eq!(b.shared_seen.len(), 2);
            assert_eq!(b.shared_payload.visits, 200);
            assert_eq!(b.certified_shared_visits, expected_c);
            b.audit_owner_shape(&foreign_owner).unwrap();
            assert_eq!(b.charged_visits(), before + 106);
            assert_eq!(
                b.certified_shared_visits, expected_c,
                "a later owner cannot promote a raw foreign body"
            );
            assert_eq!(b.owner_payload.visits, 4);
            assert_eq!(b.shared_seen.len(), 2);
        }
    }
    drop((own, input, f));
    assert!(weak.upgrade().is_none());
}

#[test]
fn public_raw_delegate_checks_full_depth_before_hits_and_poison_before_new_guards() {
    let f = fact(22);
    let weak = Arc::downgrade(&f);
    let input = SharedPublicationInput::new(f.clone());
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_shared_input_at_depth(&input, DEPTH_LIMIT - f.shape.depth)
        .unwrap();
    assert_eq!(b.shared_payload.visits, 100);
    assert_eq!(b.certified_shared_visits, 0);
    assert_eq!(b.local_shape().depth, DEPTH_LIMIT);
    let before = (
        b.charged_visits(),
        b.shared_seen.len(),
        Arc::strong_count(&f),
    );
    assert_eq!(
        b.audit_shared_input_at_depth(&input, DEPTH_LIMIT - f.shape.depth + 1),
        Err(PublishedValueError::Limit("closure depth"))
    );
    assert_eq!(
        (
            b.charged_visits(),
            b.shared_seen.len(),
            Arc::strong_count(&f)
        ),
        (before.0 + 2, before.1, before.2)
    );
    assert_eq!(b.shared_payload.visits, 100);
    assert_eq!(b.certified_shared_visits, 0);
    assert!(b.owner_seen.is_empty() && b.owners().is_empty());
    assert!(b.audit_shared_input_at_depth(&input, 0).is_err());
    assert!(b.seal(Arc::new(Originals(vec![]))).is_err());
    drop((input, f));
    assert!(weak.upgrade().is_none());

    let f = fact(23);
    let weak = Arc::downgrade(&f);
    let input = SharedPublicationInput::new(f.clone());
    // Measure actual frame/table admission, including minimum active buckets.
    // The new public entry must still reach the exact old SharedBody refusal.
    let mut reference = PublishedValueArenaBuilder::new();
    reference.audit_shared_input_at_depth(&input, 0).unwrap();
    let prefix = reference.charged_bytes() - f.shape.bytes;
    drop(reference);
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_bytes(BYTE_LIMIT - prefix - (f.shape.bytes - 1))
        .unwrap();
    let refs = Arc::strong_count(&f);
    let PublishedValueError::ByteBudget(report) =
        b.audit_shared_input_at_depth(&input, 0).unwrap_err()
    else {
        panic!("expected body bytes");
    };
    assert_eq!(report.context(), PublicationByteContext::SharedBody);
    assert_eq!(report.requested_bytes(), f.shape.bytes);
    assert_eq!(report.checked_total_bytes(), Some(BYTE_LIMIT + 1));
    assert_eq!(report.producer_work(), Some(2));
    assert_eq!(
        (
            b.shared_payload.visits,
            b.certified_shared_visits,
            b.shared_seen.len()
        ),
        (0, 0, 0)
    );
    assert_eq!(Arc::strong_count(&f), refs);
    assert!(b.owner_seen.is_empty() && b.owners().is_empty());
    assert!(b.audit_shared_input_at_depth(&input, 0).is_err());
    assert!(b.charge_visit().is_err());
    drop((b, input, f));
    assert!(weak.upgrade().is_none());
}
