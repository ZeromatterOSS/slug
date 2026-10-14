use super::*;

#[test]
fn local_source_payload_counts_follow_guarded_allocations_not_history() {
    let shared = source(3);
    let build = |fresh: bool| {
        let mut b = PublishedValueArenaBuilder::new();
        code(&mut b, shared.clone());
        let second = if fresh {
            CallableSourceAuthority::new(shared.data().clone()).unwrap()
        } else {
            shared.clone()
        };
        code(&mut b, second);
        seal(b, 0)
    };
    let reused = build(false);
    let fresh = build(true);
    assert_eq!(reused.sources().len(), 1);
    assert_eq!(fresh.sources().len(), 2);
    assert_eq!(
        fresh.0.local_shape.visits - reused.0.local_shape.visits,
        shared.fact_count() + 1
    ); // Additional full payload plus one stored reference.
    let mut b = PublishedValueArenaBuilder::new();
    code(&mut b, source(128));
    let large = seal(b, 0);
    assert_eq!(large.0.local_shape.nodes, 1);
    assert!(
        large.0.local_shape.visits > 250,
        "full source closure, not row count"
    );
}

#[test]
fn canonical_local_sources_preserve_import_order_independence_and_prior_lifetime() {
    let shared = source(2);
    let mut b = PublishedValueArenaBuilder::new();
    code(&mut b, shared.clone());
    let prior = seal(b, 0);
    let prior_weak = Arc::downgrade(&prior.0);
    let build = |import_first: bool| {
        let mut b = PublishedValueArenaBuilder::new();
        if import_first {
            b.import(prior.clone()).unwrap();
        }
        code(&mut b, shared.clone());
        if !import_first {
            b.import(prior.clone()).unwrap();
        }
        seal(b, 0)
    };
    let a = build(true);
    let c = build(false);
    assert_eq!(a.0.local_shape.visits, c.0.local_shape.visits);
    assert_eq!(a.shape().visits, c.shape().visits);
    assert_eq!(a.sources().len(), 1);
    assert_eq!(a.sources()[0].storage_identity(), shared.storage_identity());
    drop(a);
    drop(c);
    let mut imported_only = PublishedValueArenaBuilder::new();
    imported_only.import(prior.clone()).unwrap();
    let retained = seal(imported_only, 0);
    assert!(
        retained.sources().is_empty(),
        "imported-only guards stay in prior DAG"
    );
    assert_eq!(retained.imports()[0].sources()[0], shared);
    drop(prior);
    assert!(prior_weak.upgrade().is_some());
    drop(retained);
    assert!(prior_weak.upgrade().is_none());
}

#[test]
fn source_byte_forecast_includes_optional_mapping_id_in_every_identity_slot() {
    use crate::callable_graph::*;
    let plain = source(0);
    let mapping = slug_identity_v2::RepositoryMapping::new(
        slug_identity_v2::RepositoryMappingId::new("full-label-mapping-id").unwrap(),
    );
    let mapped_label = slug_identity_v2::ApparentLabel::parse("//pkg:source.bzl")
        .unwrap()
        .resolve(&mapping);
    for slot in 0..3 {
        let mut a = plain.data().clone();
        a.direct_children = Arc::from([a.root.clone()]);
        let mut b = a.clone();
        match slot {
            0 => b.root.label = mapped_label.clone(),
            1 => {
                let mut child = b.direct_children[0].clone();
                child.label = mapped_label.clone();
                b.direct_children = Arc::from([child]);
            }
            _ => {
                let mut row = b.reachable[0].clone();
                row.identity.label = mapped_label.clone();
                b.reachable = Arc::from([row]);
            }
        }
        let a = CallableSourceAuthority::new(a).unwrap();
        let b = CallableSourceAuthority::new(b).unwrap();
        assert_ne!(a, b);
        assert_eq!(b.byte_cost() - a.byte_cost(), mapping.id().as_str().len());
        let remaining = a.byte_cost() + 4 * std::mem::size_of::<CallableSourceAuthority>();
        for (authority, expected_success) in [(a, true), (b, false)] {
            let mut builder = PublishedValueArenaBuilder::new();
            let id = builder.reserve().unwrap();
            let dependencies = builder.append_edges(&[]).unwrap();
            let fill = BYTE_LIMIT - builder.charged_bytes() - remaining;
            builder.charge_bytes(fill).unwrap();
            let result = builder.set_node(
                id,
                PublishedNode::Code(PublishedCode {
                    source: authority,
                    name: "".into(),
                    definition: crate::CallableDefinitionSpan { start: 0, end: 1 },
                    dependencies,
                    globals: Arc::from([]),
                }),
            );
            assert_eq!(result.is_ok(), expected_success);
            if !expected_success {
                assert!(builder.charge_bytes(0).is_err());
            }
        }
    }
}
