use slug_identity_v2::ApparentLabel;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::*;

fn fixture() -> BzlLoadManifest {
    let mut manifest = super::super::tests::manifest("@@//pkg:source.bzl", [3; 32]);
    manifest.direct_children = Arc::from([manifest.root.clone()]);
    manifest
}

fn cost(manifest: &BzlLoadManifest) -> usize {
    let mut builder = PublishedValueArenaBuilder::new();
    convert(
        manifest,
        &mut MappingAudits::default(),
        &mut builder,
        &mut 65536,
    )
    .unwrap();
    builder.charged_bytes()
}

// Observe the ordinary unprepared native binding path, not a second copy of
// the private authority layout formula. Row/name and four handle costs stay
// independently admitted; the remaining delta is the native payload Q.
pub(super) fn native_cost(authority: &CallableSourceAuthority) -> usize {
    use slug_build_api_v2::published_value::PublishedCode;
    use slug_build_api_v2::published_value::PublishedNode;
    let mut builder = PublishedValueArenaBuilder::new();
    let node = builder.reserve().unwrap();
    let dependencies = builder.append_edges(&[]).unwrap();
    let before = builder.charged_bytes();
    builder
        .set_node(
            node,
            PublishedNode::Code(PublishedCode {
                source: authority.clone(),
                name: "source".into(),
                definition: slug_build_api_v2::CallableDefinitionSpan { start: 0, end: 1 },
                dependencies,
                globals: Arc::from([]),
            }),
        )
        .unwrap();
    builder.charged_bytes() - before - "source".len() - 4 * size_of::<CallableSourceAuthority>()
}

#[test]
fn mapping_ids_in_each_source_position_refuse_before_conversion_clones() {
    let plain = fixture();
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("m".repeat(257)).unwrap());
    let mapped = ApparentLabel::parse("//pkg:source.bzl")
        .unwrap()
        .resolve(&mapping);
    let plain_cost = cost(&plain);
    for slot in 0..3 {
        let mut changed = plain.clone();
        match slot {
            0 => changed.root.label = mapped.clone(),
            1 => {
                let mut child = changed.direct_children[0].clone();
                child.label = mapped.clone();
                changed.direct_children = Arc::from([child]);
            }
            _ => {
                let mut source = changed.reachable[0].clone();
                source.identity.label = mapped.clone();
                changed.reachable = Arc::from([source]);
            }
        }
        assert_eq!(cost(&changed) - plain_cost, mapping.id().as_str().len());
        let mut relation_builder = PublishedValueArenaBuilder::new();
        let mut relation_audits = MappingAudits::default();
        let plan = forecast(
            &changed,
            &relation_audits,
            &mut relation_builder,
            &mut 65536,
        )
        .unwrap();
        let output = plan.output_bytes;
        drop(plan);
        let authority = convert(
            &changed,
            &mut relation_audits,
            &mut relation_builder,
            &mut 65536,
        )
        .unwrap();
        let transfer = std::mem::size_of_val(authority.data().direct_children.as_ref())
            + std::mem::size_of_val(authority.data().reachable.as_ref());
        assert_eq!(
            output,
            native_cost(&authority) + transfer,
            "complete P=Q+transfer with optional mappingID at source position {slot}"
        );
        for (manifest, succeeds) in [(&plain, true), (&changed, false)] {
            let mut builder = PublishedValueArenaBuilder::new();
            builder
                .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - plain_cost)
                .unwrap();
            let references = Arc::strong_count(&manifest.root.repository_mapping);
            let result = convert(
                manifest,
                &mut MappingAudits::default(),
                &mut builder,
                &mut 65536,
            );
            assert_eq!(result.is_ok(), succeeds, "source position {slot}");
            drop(result);
            assert_eq!(
                Arc::strong_count(&manifest.root.repository_mapping),
                references
                    + if succeeds {
                        1 + manifest.direct_children.len() + manifest.reachable.len()
                    } else {
                        0
                    }
            );
            if !succeeds {
                assert!(builder.charge_bytes(0).is_err());
            }
            drop(builder);
            assert_eq!(
                Arc::strong_count(&manifest.root.repository_mapping),
                references
            );
        }
    }
}

#[test]
fn complete_authority_and_array_transfer_peak_is_admitted_before_allocation() {
    let manifest = fixture();
    let bytes = cost(&manifest);
    let authority = convert(
        &manifest,
        &mut MappingAudits::default(),
        &mut PublishedValueArenaBuilder::new(),
        &mut 65536,
    )
    .unwrap();
    // Besides the retained payload, one full temporary array of each actual
    // element type and all three Arc allocations coexist during conversion.
    let arrays = std::mem::size_of_val(authority.data().direct_children.as_ref())
        + std::mem::size_of_val(authority.data().reachable.as_ref());
    let allocation_floor = size_of::<CallableSourceAuthorityData>()
        + CallableSourceAuthority::storage_overhead_bytes()
        + 4 * size_of::<usize>()
        + 2 * arrays;
    assert!(bytes >= allocation_floor);
    for (allowance, succeeds) in [(bytes - 1, false), (bytes, true)] {
        let mut builder = PublishedValueArenaBuilder::new();
        builder
            .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - allowance)
            .unwrap();
        let references = Arc::strong_count(&manifest.root.repository_mapping);
        let result = convert(
            &manifest,
            &mut MappingAudits::default(),
            &mut builder,
            &mut 65536,
        );
        assert_eq!(result.is_ok(), succeeds);
        drop(result);
        assert_eq!(
            Arc::strong_count(&manifest.root.repository_mapping),
            references
                + if succeeds {
                    1 + manifest.direct_children.len() + manifest.reachable.len()
                } else {
                    0
                }
        );
        if !succeeds {
            assert!(builder.charge_bytes(0).is_err());
        }
        drop(builder);
        assert_eq!(
            Arc::strong_count(&manifest.root.repository_mapping),
            references
        );
    }
    assert!(array::<CallableModuleIdentity>(usize::MAX).is_err());
}

#[test]
fn private_source_envelope_is_prepaid_before_any_conversion_guard_clone() {
    let manifest = fixture();
    let full = cost(&manifest);
    let added = CallableSourceAuthority::storage_overhead_bytes() - 2 * size_of::<usize>();
    assert!(added >= size_of::<usize>());
    let references = Arc::strong_count(&manifest.root.repository_mapping);
    let mut builder = PublishedValueArenaBuilder::new();
    // The entire previous pre-copy allowance lacks the newly retained envelope.
    builder
        .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - (full - added))
        .unwrap();
    assert!(
        convert(
            &manifest,
            &mut MappingAudits::default(),
            &mut builder,
            &mut 65536
        )
        .is_err()
    );
    assert_eq!(
        Arc::strong_count(&manifest.root.repository_mapping),
        references
    );
    assert!(builder.charge_bytes(0).is_err());
}
