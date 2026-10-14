//! Borrowed target/key certificates cover execution-platform identity and providers.
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::*;
use crate::AnalysisConfiguredTargetKey;
use crate::AnalysisTargetIdentity;
use crate::ConfiguredTargetValue;
use crate::ProviderCollection;
use crate::ProviderIdentity;
use crate::ProviderValue;

fn platform(mapped: bool) -> CanonicalLabel {
    if mapped {
        ApparentLabel::parse("//platform:exec")
            .unwrap()
            .resolve(&RepositoryMapping::new(
                RepositoryMappingId::new("platform-map-".repeat(64)).unwrap(),
            ))
    } else {
        CanonicalLabel::parse("@@//platform:exec").unwrap()
    }
}
fn key(platform: CanonicalLabel) -> AnalysisConfiguredTargetKey {
    AnalysisConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//pkg:owner").unwrap(),
        b"cfg".as_slice(),
    )
    .with_toolchain_execution_platform(Arc::new(platform))
}
fn target(
    key: impl Into<AnalysisTargetIdentity>,
    providers: Vec<ProviderValue>,
) -> ConfiguredTargetValue {
    ConfiguredTargetValue::new(
        key,
        ProviderCollection::from_values(providers, false).unwrap(),
    )
}
fn target_shape(target: &ConfiguredTargetValue, depth: usize) -> PublicationShape {
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_owner_configured_target_at_depth(target, depth)
        .unwrap();
    b.local_shape()
}
fn derived(key: AnalysisConfiguredTargetKey) -> AnalysisValue {
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
    let config = slug_configuration_v2::SlugConfiguration::default_target(&host).unwrap();
    AnalysisValue::artifact(AnalysisArtifact::Derived {
        owner: key,
        output: crate::ActionOutput::new(
            crate::DerivedArtifactRoot::bin(config).unwrap(),
            "pkg/result",
            crate::ActionOutputKind::File,
        ),
    })
}
#[test]
fn configured_keys_forecast_full_platform_mapping_id_for_artifacts_and_targets() {
    let plain_key = key(platform(false));
    let mapped_key = key(platform(true));
    let length = platform(true).mapping_id().unwrap().as_str().len();
    let plain = target(plain_key.clone(), vec![]);
    let mapped = target(mapped_key.clone(), vec![]);
    let before = target_shape(&plain, 0);
    let after = target_shape(&mapped, 0);
    assert_eq!(
        after.bytes - before.bytes,
        length + std::mem::size_of::<compact_str::CompactString>()
    );
    assert_eq!(after.visits - before.visits, 1);
    assert_eq!(after.edges - before.edges, 1);
    let artifact_shape = |value: &AnalysisValue| {
        let mut b = PublishedValueArenaBuilder::new();
        b.audit_native(value).unwrap();
        b.local_shape()
    };
    assert_eq!(
        artifact_shape(&derived(mapped_key)).bytes - artifact_shape(&derived(plain_key)).bytes,
        after.bytes - before.bytes
    );
    let remaining = before.bytes + length / 2;
    for (value, succeeds) in [(&plain, true), (&mapped, false)] {
        let mut b = PublishedValueArenaBuilder::new();
        b.charge_bytes(BYTE_LIMIT - remaining).unwrap();
        assert_eq!(
            b.audit_owner_configured_target_at_depth(value, 0).is_ok(),
            succeeds
        );
        assert!(!b.owner_audit);
        if !succeeds {
            assert!(b.charge_bytes(0).is_err());
        }
    }
    let mut old_path = PublishedValueArenaBuilder::new();
    old_path
        .audit_owner_native(&AnalysisValue::configured_target(mapped))
        .unwrap();
    let old = old_path.local_shape();
    assert_eq!(
        (old.nodes, old.edges, old.bytes, old.visits, old.depth),
        (
            after.nodes,
            after.edges,
            after.bytes,
            after.visits,
            after.depth
        ),
    ); // Identical outer header coverage.
}

#[test]
fn borrowed_target_audit_preserves_null_depth_offsets_mode_and_failure_poison() {
    let null = target(
        AnalysisTargetIdentity::null(CanonicalLabel::parse("@@//pkg:source").unwrap()),
        vec![],
    );
    assert_eq!(target_shape(&null, 17).depth, 17);
    let provider = crate::ProviderOccurrence::new(
        ProviderIdentity::builtin("Ordinary"),
        [("value", AnalysisValue::string("payload"))],
    );
    let value = target(
        key(platform(false)),
        vec![ProviderValue::Occurrence(provider)],
    );
    for prior in [false, true] {
        let mut b = PublishedValueArenaBuilder::new();
        b.owner_audit = prior;
        b.audit_owner_configured_target_at_depth(&value, 253)
            .unwrap();
        assert_eq!(b.local_shape().depth, 256);
        assert_eq!(b.owner_audit, prior);
        let mut b = PublishedValueArenaBuilder::new();
        b.owner_audit = prior;
        assert!(matches!(
            b.audit_owner_configured_target_at_depth(&value, 254),
            Err(PublishedValueError::Limit("depth"))
        ));
        assert_eq!(b.owner_audit, prior);
        assert!(b.charge_bytes(0).is_err());
    }
}

#[derive(Debug)]
struct Originals;
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        1
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &[]
    }
}
#[test]
fn borrowed_target_audit_walks_complete_providers_and_declared_prior_imports() {
    let mut prior = PublishedValueArenaBuilder::new();
    let node = prior.reserve().unwrap();
    let empty = prior.append_edges(&[]).unwrap();
    prior
        .set_node(
            node,
            PublishedNode::Provider(PublishedProvider {
                identity: ProviderIdentity::builtin("Original"),
                has_schema: false,
                names: Arc::from([]),
                values: empty,
                files_to_run: None,
            }),
        )
        .unwrap();
    prior
        .set_original_slot(node, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    let arena = prior.seal(Arc::new(Originals)).unwrap();
    let provider = arena
        .value(node)
        .unwrap()
        .project_provider(std::iter::empty::<(&str, AnalysisValue)>())
        .unwrap();
    let plain = target(key(platform(false)), vec![]);
    let imported = target(
        key(platform(false)),
        vec![ProviderValue::Occurrence(provider)],
    );
    let mut audit = PublishedValueArenaBuilder::new();
    audit
        .audit_owner_configured_target_at_depth(&imported, 2)
        .unwrap();
    assert_eq!(audit.imports().len(), 1);
    assert_eq!(audit.imports()[0].identity(), arena.identity());
    assert!(audit.charged_visits() > target_shape(&plain, 2).visits);
    assert!(!audit.owner_audit);
    let mut limited = PublishedValueArenaBuilder::new();
    limited
        .charge_bytes(BYTE_LIMIT - target_shape(&plain, 0).bytes)
        .unwrap();
    assert!(matches!(
        limited.audit_owner_configured_target_at_depth(&imported, 0),
        Err(PublishedValueError::ByteBudget(refusal))
            if !refusal.prior_poisoned()
                && refusal.limit_bytes() == BYTE_LIMIT
                && refusal.checked_total_bytes().is_some_and(|total| total > BYTE_LIMIT)
    ));
    assert!(limited.imports().is_empty());
    assert!(!limited.owner_audit);
    assert!(limited.charge_bytes(0).is_err());
}
