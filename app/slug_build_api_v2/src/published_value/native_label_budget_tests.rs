//! Full optional label provenance is forecast before native retention.
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::*;

fn labels() -> (CanonicalLabel, CanonicalLabel, usize) {
    let id = "retained-mapping-identity-".repeat(64);
    let mapping = RepositoryMapping::new(RepositoryMappingId::new(id.clone()).unwrap());
    let mapped = ApparentLabel::parse("//pkg:leaf")
        .unwrap()
        .resolve(&mapping);
    let plain = CanonicalLabel::parse("@@//pkg:leaf").unwrap();
    assert_eq!(mapped.package(), plain.package());
    assert_eq!(mapped.target(), plain.target());
    (plain, mapped, id.len())
}

fn shape(value: &AnalysisValue) -> PublicationShape {
    let mut builder = PublishedValueArenaBuilder::new();
    builder.audit_native(value).unwrap();
    builder.local_shape()
}

#[test]
fn optional_label_mapping_identity_charges_full_bytes_and_one_text_visit_edge() {
    let (plain, mapped, length) = labels();
    let plain = AnalysisValue::label(plain);
    let mapped = AnalysisValue::label(mapped);
    let before = shape(&plain);
    let after = shape(&mapped);
    assert_eq!(
        after.bytes - before.bytes,
        length + std::mem::size_of::<compact_str::CompactString>()
    );
    assert_eq!(after.visits - before.visits, 1);
    assert_eq!(after.edges - before.edges, 1);
    assert_eq!(after.nodes, before.nodes);
    assert_eq!(after.depth, before.depth);
    assert_ne!(plain, mapped); // Complete native label identity remains unchanged.
}

fn derived(label: CanonicalLabel) -> AnalysisValue {
    use slug_configuration_v2::native::host::AutoCpuToken;
    use slug_configuration_v2::native::host::HostConversionInputs;
    use slug_configuration_v2::native::host::HostPathFlavor;
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    let configuration = slug_configuration_v2::SlugConfiguration::default_target(&host).unwrap();
    AnalysisValue::artifact(AnalysisArtifact::Derived {
        owner: crate::AnalysisConfiguredTargetKey::new(label, b"cfg".as_slice()),
        output: crate::ActionOutput::new(
            crate::DerivedArtifactRoot::bin(configuration).unwrap(),
            "pkg/result",
            crate::ActionOutputKind::File,
        ),
    })
}

fn nested_source(label: CanonicalLabel) -> AnalysisValue {
    AnalysisValue::depset(
        crate::AnalysisDepset::new(
            crate::DepsetOrder::Default,
            vec![AnalysisValue::artifact(AnalysisArtifact::Source(label))],
            vec![],
        )
        .unwrap(),
    )
}

#[test]
fn label_source_derived_and_nested_artifact_ids_refuse_before_native_retention() {
    let constructors: [fn(CanonicalLabel) -> AnalysisValue; 4] = [
        AnalysisValue::label,
        |label| AnalysisValue::artifact(AnalysisArtifact::Source(label)),
        derived,
        nested_source,
    ];
    for construct in constructors {
        let (plain, mapped, length) = labels();
        let plain = construct(plain);
        let mapped = construct(mapped);
        let baseline = shape(&plain);
        let complete = shape(&mapped);
        assert_eq!(
            complete.bytes - baseline.bytes,
            length + std::mem::size_of::<compact_str::CompactString>()
        );
        // The small fixture leaves room for the complete spelling and only half
        // its separate mapping ID; no large allocation is needed to hit the cap.
        let remaining = baseline.bytes + length / 2;
        let mut control = PublishedValueArenaBuilder::new();
        control.charge_bytes(BYTE_LIMIT - remaining).unwrap();
        assert!(control.native_atom_borrowed(&plain).is_ok());
        let mut limited = PublishedValueArenaBuilder::new();
        limited.charge_bytes(BYTE_LIMIT - remaining).unwrap();
        assert!(matches!(
            limited.native_atom_borrowed(&mapped),
            Err(PublishedValueError::ByteBudget(refusal))
            if !refusal.prior_poisoned()
                && refusal.limit_bytes() == BYTE_LIMIT
                && refusal.checked_total_bytes().is_some_and(|total| total > BYTE_LIMIT)
        ));
        assert!(limited.charged_bytes() <= BYTE_LIMIT);
        assert_eq!(limited.local_shape().nodes, baseline.nodes);
        assert!(limited.charge_bytes(0).is_err()); // Refusal poisons reuse.
    }
}
