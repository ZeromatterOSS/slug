//! One rich genuine native model exercises complete feature-owned storage.
use compact_str::CompactString;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::*;
use crate::cc_features::*;

fn rich(name: CompactString, mapped: bool) -> CcToolchainFeatures {
    let label = if mapped {
        ApparentLabel::parse("//pkg:tool")
            .unwrap()
            .resolve(&RepositoryMapping::new(
                RepositoryMappingId::new("full-tool-mapping-identity".repeat(32)).unwrap(),
            ))
    } else {
        CanonicalLabel::parse("@@//pkg:tool").unwrap()
    };
    let predicate = || {
        [CcWithFeatureSet {
            features: ["feature".into()].into_iter().collect(),
            not_features: CcSet::new(),
        }]
        .into_iter()
        .collect()
    };
    let inner = CcFlagGroup {
        expandables: Box::new([CcExpandable::Flag(Box::new([
            CcStringChunk::Literal("--flag=".into()),
            CcStringChunk::Variable("value".into()),
            CcStringChunk::RelativePath("relative/path".into()),
        ]))]),
        iterate_over: None,
        expand_if_available: Some("present".into()),
        expand_if_not_available: None,
        expand_if_true: None,
        expand_if_false: None,
        expand_if_equal: Some(CcVariableWithValue {
            name: "mode".into(),
            value: "active".into(),
        }),
    };
    let group = CcFlagGroup {
        expandables: Box::new([CcExpandable::Group(inner)]),
        iterate_over: None,
        expand_if_available: None,
        expand_if_not_available: None,
        expand_if_true: None,
        expand_if_false: None,
        expand_if_equal: None,
    };
    CcToolchainFeatures::new(
        vec![CcFeature {
            name,
            enabled: false,
            flag_sets: Box::new([CcFlagSet {
                actions: (0..33).map(|i| format!("action{i}").into()).collect(),
                with_features: predicate(),
                flag_groups: Box::new([group]),
            }]),
            env_sets: Box::new([CcEnvSet {
                actions: ["compile".into()].into_iter().collect(),
                with_features: predicate(),
                env_entries: Box::new([CcEnvEntry {
                    key: "VARIABLE".into(),
                    value: Box::new([CcStringChunk::Literal("environment".into())]),
                    expand_if_available: Some("optional_environment".into()),
                }]),
            }]),
            requires: Box::new([["feature".into()].into_iter().collect()]),
            implies: Box::new([]),
            provides: Box::new(["capability".into()]),
        }],
        vec![CcActionConfig {
            action_name: "compile".into(),
            enabled: false,
            flag_sets: Box::new([]),
            implies: Box::new(["feature".into()]),
            tools: Box::new([CcTool::new(
                None,
                Some(crate::AnalysisArtifact::Source(label)),
                predicate(),
                ["requirement".into()].into_iter().collect(),
                CcExecOs::Linux,
            )
            .unwrap()]),
        }],
        vec![CcArtifactNamePattern::new("static_library", "custom_", ".a").unwrap()],
        "tools",
        CcExecOs::Linux,
    )
    .unwrap()
}

#[test]
fn complete_storage_and_last_tool_mapping_survive_proof_hits_and_precopy_refusal() {
    let mut name = CompactString::with_capacity(4096);
    name.push_str("feature");
    let spare = rich(name, false);
    let plain = rich("feature".into(), false);
    assert_eq!(spare.model(), plain.model());
    let mut large = PublishedValueArenaBuilder::new();
    native::features(&mut large, spare.model(), 0).unwrap();
    let mut small = PublishedValueArenaBuilder::new();
    native::features(&mut small, plain.model(), 0).unwrap();
    assert_eq!(large.charged_visits(), small.charged_visits());
    assert!(large.charged_bytes() >= small.charged_bytes() + 4000);
    let set = &spare.model().features[0].flag_sets[0].actions;
    let backing = set.retained_storage_bytes_upper_bound().unwrap();
    assert!(backing > set.len() * size_of::<CompactString>());
    let array_bytes = spare.model().features.len() * size_of::<CcFeature>();
    assert!(
        large.charged_bytes()
            >= size_of::<CcToolchainFeatureModel>() + array_bytes + 4096 + backing
    );
    let mapped = rich("feature".into(), true);
    assert_ne!(mapped.model(), plain.model());
    let mapping_length = mapped.model().action_configs[0].tools[0]
        .artifact()
        .unwrap();
    let crate::AnalysisArtifact::Source(label) = mapping_length else {
        panic!("source tool")
    };
    let length = label.mapping_id().unwrap().as_str().len();
    let value = AnalysisValue::cc_toolchain_features(mapped.clone());
    let mut reference = PublishedValueArenaBuilder::new();
    native::audit(&mut reference, &value, 0).unwrap();
    let mut without = PublishedValueArenaBuilder::new();
    native::audit(
        &mut without,
        &AnalysisValue::cc_toolchain_features(plain),
        0,
    )
    .unwrap();
    assert_eq!(reference.charged_visits(), without.charged_visits() + 1);
    assert_eq!(
        reference.local_shape().edges,
        without.local_shape().edges + 1
    );
    assert_eq!(
        reference.charged_bytes() - without.charged_bytes(),
        length + size_of::<CompactString>()
    );
    let mut b = PublishedValueArenaBuilder::new();
    let first = b.native_atom_borrowed(&value).unwrap();
    let before = b.local_shape();
    let prior_scratch = b.producer_scratch_bytes;
    let hit = b.native_atom_borrowed(&value).unwrap();
    assert_eq!(first.retained_visits(), reference.charged_visits());
    assert_eq!(hit.retained_visits(), first.retained_visits());
    assert_eq!(hit.ordinary_depth(), reference.maximum_depth);
    assert_eq!(
        b.producer_scratch_bytes - prior_scratch,
        size_of::<ModelPlan<'_>>()
    );
    assert_eq!(
        b.charged_bytes() - before.bytes - (b.producer_scratch_bytes - prior_scratch),
        reference.charged_bytes() + size_of::<NativeCertificate>(),
        "full spare capacity, tool mapping and native certificate are retained"
    );
    assert_eq!(
        b.charged_bytes() - before.bytes,
        reference.charged_bytes() + size_of::<NativeCertificate>() + size_of::<ModelPlan<'_>>()
    );
    assert_eq!(
        b.local_shape().edges - before.edges,
        reference.local_shape().edges
    );
    assert!(b.charged_visits() - before.visits < reference.charged_visits());
    drop(first);
    drop(hit);
    let refs = Arc::strong_count(mapped.model());
    let allowance =
        reference.charged_bytes() + size_of::<NativeCertificate>() + size_of::<ModelPlan<'_>>();
    b.charge_bytes(BYTE_LIMIT - b.charged_bytes() - allowance + 1)
        .unwrap();
    assert!(b.native_atom_borrowed(&value).is_err());
    assert_eq!(Arc::strong_count(mapped.model()), refs);
    assert_eq!(b.feature_models.committed.len(), 1);
    assert!(b.charge_visit().is_err());
    // Same full first-miss budget refusal must not create a guard or atom.
    let mut fresh = PublishedValueArenaBuilder::new();
    fresh.charge_bytes(BYTE_LIMIT - 4095).unwrap();
    let raw = AnalysisValue::cc_toolchain_features(spare.clone());
    let refs = Arc::strong_count(spare.model());
    assert!(fresh.native_atom_borrowed(&raw).is_err());
    assert_eq!(Arc::strong_count(spare.model()), refs);
    assert!(fresh.feature_models.committed.is_empty());
}
