use super::*;

fn real_struct(count: usize, changed: bool) -> PublishedValueArenaBuilder {
    let mut b = PublishedValueArenaBuilder::new();
    let child = atom(&mut b, AnalysisValue::integer(7));
    let range = b
        .append_edges(&vec![PublishedEdge::Local(child); count])
        .unwrap();
    let id = b.reserve().unwrap();
    let mut fields = names(count).to_vec();
    if changed && count != 0 {
        fields[count - 1] = "different_final_field".into();
    }
    b.set_node(
        id,
        PublishedNode::Struct {
            names: fields.into(),
            values: range,
        },
    )
    .unwrap();
    b.set_original_slot(id, PublishedRootSlot::new(1).unwrap())
        .unwrap();
    b
}

#[test]
fn real_many_name_seal_uses_scalar_work_and_retains_validation_and_equality() {
    let count = 128;
    // Two rows have two slots: scalar slot work1 replaces their old work2.
    // Non-backing8 - old slots2 + scalar1, plus view8 and FULL backing1.
    // Full slot/name/backing facts remain unchanged; exact/+1 W stays strict.
    let needed = (8 - 2 + 1) + 8 + 1;
    for (remaining, succeeds) in [(needed, true), (needed - 1, false)] {
        let mut b = real_struct(count, false);
        while b.charged_visits() < NODE_LIMIT - remaining {
            b.charge_visit().unwrap();
        }
        let result = finish(b);
        assert_eq!(result.is_ok(), succeeds);
        if let Ok(arena) = result {
            assert_eq!(arena.0.local_shape.visits, 7 + 2 * count);
            assert_eq!(arena.shape().visits, arena.0.local_shape.visits);
            let same = finish(real_struct(count, false)).unwrap();
            let changed = finish(real_struct(count, true)).unwrap();
            let id = PublishedNodeId::new(1).unwrap();
            assert!(
                arena
                    .value(id)
                    .unwrap()
                    .publication_eq(&same.value(id).unwrap())
            );
            assert!(
                !arena
                    .value(id)
                    .unwrap()
                    .publication_eq(&changed.value(id).unwrap())
            );
        } else {
            assert!(matches!(
                result,
                Err(PublishedValueError::RetainedCertificateWork(_))
            ));
        }
    }
    // No array-length batching bypasses actual producer validation.
    for (fields, edges, error) in [
        (vec!["same", "same"], 2, "duplicate field"),
        (vec!["one", "two"], 1, "field count"),
    ] {
        let mut b = PublishedValueArenaBuilder::new();
        let id = b.reserve().unwrap();
        let range = b
            .append_edges(&vec![PublishedEdge::Local(id); edges])
            .unwrap();
        assert_eq!(
            b.set_node(
                id,
                PublishedNode::Struct {
                    names: fields
                        .into_iter()
                        .map(Into::into)
                        .collect::<Vec<_>>()
                        .into(),
                    values: range,
                }
            )
            .unwrap_err(),
            PublishedValueError::Invalid(error)
        );
    }
    let mut b = PublishedValueArenaBuilder::new();
    let id = b.reserve().unwrap();
    let constructor = |names| {
        PublishedNode::ProviderConstructor(PublishedProviderConstructor {
            id: crate::ProviderId::new("//ordinary:defs.bzl", "Info").unwrap(),
            schema: PublishedProviderSchema::List(names),
            init: None,
        })
    };
    assert_eq!(
        b.set_node(id, constructor(Arc::from(["same".into(), "same".into()])))
            .unwrap_err(),
        PublishedValueError::Invalid("constructor schema")
    );
    assert_eq!(
        b.set_node(
            id,
            PublishedNode::ProviderConstructor(PublishedProviderConstructor {
                id: crate::ProviderId::new("//ordinary:defs.bzl", "Info").unwrap(),
                schema: PublishedProviderSchema::Initialized(Arc::from([])),
                init: None,
            })
        )
        .unwrap_err(),
        PublishedValueError::Invalid("constructor initializer")
    );
    // Struct range validation stays BEFORE the newly paid count operation.
    let mut work = NODE_LIMIT - 2;
    assert_eq!(
        local_visits(
            &[Some(PublishedNode::Struct {
                names: names(1),
                values: PublishedEdgeRange { start: 1, len: 1 }
            })],
            &[],
            &[None],
            &[],
            0,
            0,
            &mut work,
            &mut 0,
            &mut 0,
            &mut false
        )
        .unwrap_err(),
        PublishedValueError::Invalid("retained range")
    );
}
