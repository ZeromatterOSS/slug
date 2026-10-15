//! Complete borrowed source entries retain their forecasts under typed batching.
use slug_build_api_v2::DepsetStorageCharge;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;

use super::*;

fn identity() -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse("@@repo+//pkg:defs.bzl").unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(std::path::PathBuf::from(
            "/actual/source/defs.bzl",
        )),
        repository_mapping: Arc::from([
            (
                slug_identity_v2::ApparentRepoName::new("first").unwrap(),
                CanonicalRepoName::new("one+").unwrap(),
            ),
            (
                slug_identity_v2::ApparentRepoName::new("second_long").unwrap(),
                CanonicalRepoName::new("different_long+").unwrap(),
            ),
        ]),
    }
}
fn charges(value: &BzlModuleIdentity) -> Vec<DepsetStorageCharge> {
    let mut charges = Vec::new();
    source(value, &mut |charge| {
        charges.push(charge);
        Ok(())
    })
    .unwrap();
    charges
}
fn old_raw_rows(value: &BzlModuleIdentity) -> Vec<usize> {
    let mut rows = vec![
        std::mem::size_of::<BzlModuleIdentity>(),
        std::mem::size_of::<CanonicalLabel>(),
        value.label.package().repo().as_str().len(),
        value.label.package().package().as_str().len(),
        value.label.target().as_str().len(),
    ];
    if let Some(mapping) = value.label.mapping_id() {
        rows.push(mapping.as_str().len());
    }
    rows.push(value.source.retained_text_bytes().unwrap());
    for (apparent, canonical) in &*value.repository_mapping {
        rows.extend([
            std::mem::size_of::<(slug_identity_v2::ApparentRepoName, CanonicalRepoName)>(),
            apparent.as_str().len(),
            canonical.as_str().len(),
        ]);
    }
    rows
}

#[test]
fn source_entry_batching_preserves_all_bytes_edges_and_entry_order_with_less_work() {
    let value = identity();
    let old = old_raw_rows(&value);
    let actual = charges(&value);
    let allowance = 64 + std::mem::size_of::<usize>();
    assert_eq!(
        actual.iter().map(|c| c.bytes).sum::<usize>(),
        old.iter().sum::<usize>() + old.len() * allowance
    );
    assert_eq!(actual.iter().map(|c| c.edges).sum::<usize>(), old.len());
    assert_eq!(actual.iter().map(|c| c.nodes).sum::<usize>(), 0);
    assert_eq!(actual.len(), 1 + value.repository_mapping.len());
    assert_eq!(
        actual.iter().map(|c| c.visits).sum::<usize>(),
        actual.len() * 2
    );
    assert_eq!(
        old.len() * 2 - actual.len() * 2,
        10 + value.repository_mapping.len() * 4
    );
    assert_eq!(
        actual[0].bytes,
        old[..6].iter().sum::<usize>() + 6 * allowance
    );
    assert_eq!(actual[0].edges, 6);
    assert_eq!(actual[0].visits, 2);
    for (index, raw) in old[6..].chunks_exact(3).enumerate() {
        assert_eq!(
            actual[1 + index].bytes,
            raw.iter().sum::<usize>() + 3 * allowance
        );
        assert_eq!(actual[1 + index].edges, 3);
        assert_eq!(actual[1 + index].visits, 2);
    }
    assert_ne!(
        actual[1].bytes, actual[2].bytes,
        "actual entry lengths expose source order"
    );
}

#[test]
fn optional_label_mapping_provenance_is_forecast_outside_entry_batches() {
    let mut plain = identity();
    plain.label = CanonicalLabel::parse("@@//pkg:defs.bzl").unwrap();
    let mut mapped = plain.clone();
    let mapping = slug_identity_v2::RepositoryMapping::new(
        slug_identity_v2::RepositoryMappingId::new("complete-origin-mapping").unwrap(),
    );
    mapped.label = slug_identity_v2::ApparentLabel::parse("//pkg:defs.bzl")
        .unwrap()
        .resolve(&mapping);
    let actual = charges(&mapped);
    let old = old_raw_rows(&mapped);
    assert_eq!(
        actual.iter().map(|c| c.bytes).sum::<usize>(),
        old.iter().sum::<usize>() + old.len() * (64 + std::mem::size_of::<usize>())
    );
    let plain = charges(&plain);
    assert_eq!(
        actual[0].bytes,
        plain[0].bytes + mapping.id().as_str().len() + 64 + std::mem::size_of::<usize>()
    );
    assert_eq!(actual[0].edges, 7);
    assert_eq!(plain[0].edges, 6);
    assert_eq!(actual[0].visits, 2);
    assert_eq!(actual.len(), plain.len());
    for (mapped, plain) in actual[1..].iter().zip(&plain[1..]) {
        assert_eq!(
            (mapped.bytes, mapped.edges, mapped.visits),
            (plain.bytes, plain.edges, plain.visits)
        );
    }
}

#[test]
fn transition_source_rows_and_repeated_mapping_entries_keep_full_order_and_forecasts() {
    let value = identity();
    let direct = charges(&value);
    let transition = TransitionDefinition::new(
        FrozenValue::new_none(),
        Arc::from([]),
        Arc::from([]),
        Arc::new(value.clone()),
        Arc::from([(
            CompactString::new("actual-source-name"),
            BzlModuleSourceProvenance::new(value, [7; 32]),
        )]),
    )
    .unwrap();
    let mut actual = Vec::new();
    super::transition(&transition, &mut |c| {
        actual.push(c);
        Ok(())
    })
    .unwrap();
    assert_eq!(actual.len(), 1 + direct.len() * 2 + 2 + 3);
    let full = transition.source_identities_by_filename.declarations()[0].retained_shape();
    let last = actual.last().unwrap();
    assert_eq!(
        (last.nodes, last.edges, last.bytes, last.visits),
        (full.nodes, full.edges, full.bytes, full.visits)
    );
    for (got, expected) in actual[1..1 + direct.len()].iter().zip(&direct) {
        assert_eq!(
            (got.bytes, got.edges, got.visits),
            (expected.bytes, expected.edges, expected.visits)
        );
    }
    for (got, expected) in actual[3 + direct.len()..].iter().zip(&direct) {
        assert_eq!(
            (got.bytes, got.edges, got.visits),
            (expected.bytes, expected.edges, expected.visits)
        );
    }
}

#[test]
fn source_entry_budget_failure_stops_before_retention_and_rechecks_the_first_entry() {
    use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_EDGE_LIMIT;
    use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT;
    use slug_build_api_v2::published_value::BYTE_LIMIT;
    let value = identity();
    let expected = charges(&value);
    let prefix_bytes = expected[..1].iter().map(|c| c.bytes).sum::<usize>();
    let prefix_edges = expected[..1].iter().map(|c| c.edges).sum::<usize>();
    let prefix_visits = expected[..1].iter().map(|c| c.visits).sum::<usize>();
    let original_guards = Arc::strong_count(&value.repository_mapping);
    for dimension in 0..3 {
        let mut builder = PublishedValueArenaBuilder::new();
        match dimension {
            0 => builder
                .charge_bytes(BYTE_LIMIT - prefix_bytes - expected[1].bytes + 1)
                .unwrap(),
            1 => builder
                .charge_owner_edges(
                    CALLABLE_GRAPH_EDGE_LIMIT - prefix_edges - expected[1].edges + 1,
                )
                .unwrap(),
            _ => {
                for _ in 0..CALLABLE_GRAPH_NODE_LIMIT - prefix_visits - expected[1].visits + 1 {
                    builder.charge_visit().unwrap();
                }
            }
        }
        let mut callbacks = 0;
        assert!(
            source(&value, &mut |c| {
                callbacks += 1;
                builder.charge_storage(c).map_err(|e| e.to_string())
            })
            .is_err()
        );
        assert_eq!(callbacks, 2);
        assert!(builder.charge_visit().is_err());
        assert_eq!(
            Arc::strong_count(&value.repository_mapping),
            original_guards
        );
        let mut callbacks = 0;
        let error = source(&value, &mut |_| {
            callbacks += 1;
            if callbacks == 2 {
                Err("first entry again".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error, "first entry again");
        assert_eq!(callbacks, 2);
    }
}

#[test]
fn source_prefix_budget_refuses_before_any_mapping_entry_callback() {
    use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_EDGE_LIMIT;
    use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT;
    use slug_build_api_v2::published_value::BYTE_LIMIT;
    let value = identity();
    let expected = charges(&value);
    let prefix = &expected[0];
    let guards = Arc::strong_count(&value.repository_mapping);
    for dimension in 0..3 {
        let mut builder = PublishedValueArenaBuilder::new();
        match dimension {
            0 => builder.charge_bytes(BYTE_LIMIT - prefix.bytes + 1).unwrap(),
            1 => builder
                .charge_owner_edges(CALLABLE_GRAPH_EDGE_LIMIT - prefix.edges + 1)
                .unwrap(),
            _ => {
                for _ in 0..CALLABLE_GRAPH_NODE_LIMIT - prefix.visits + 1 {
                    builder.charge_visit().unwrap();
                }
            }
        }
        let mut callbacks = 0;
        assert!(
            source(&value, &mut |c| {
                callbacks += 1;
                assert_eq!(
                    (c.bytes, c.edges, c.visits),
                    (prefix.bytes, prefix.edges, prefix.visits)
                );
                builder.charge_storage(c).map_err(|e| e.to_string())
            })
            .is_err()
        );
        assert_eq!(callbacks, 1);
        assert!(builder.charge_visit().is_err());
        assert_eq!(Arc::strong_count(&value.repository_mapping), guards);
    }
}
