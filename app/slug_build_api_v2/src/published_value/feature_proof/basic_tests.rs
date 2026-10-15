use compact_str::CompactString;

use super::*;
use crate::cc_features::*;

pub(in crate::published_value) fn features(name: CompactString) -> CcToolchainFeatures {
    CcToolchainFeatures::new(
        vec![CcFeature {
            name,
            enabled: false,
            flag_sets: Box::new([]),
            env_sets: Box::new([]),
            requires: Box::new([]),
            implies: Box::new([]),
            provides: Box::new([]),
        }],
        vec![],
        vec![],
        "tools",
        CcExecOs::Linux,
    )
    .unwrap()
}

#[test]
fn actual_model_hit_keeps_full_certificate_and_distinct_equal_model_misses() {
    let f = features("feature".into());
    let config = f.configure_features(vec!["feature".into()]).unwrap();
    let raw = AnalysisValue::cc_toolchain_features(f.clone());
    let configured = AnalysisValue::cc_feature_configuration(config);
    let mut b = PublishedValueArenaBuilder::new();
    b.observe_depth(200).unwrap();
    b.owner_audit = true;
    let first = b.native_atom_borrowed(&raw).unwrap();
    let first_live_bytes = b.charged_bytes();
    let first_scratch = b.producer_scratch_bytes;
    assert_eq!(
        first_scratch,
        size_of::<FeatureModelAudits>()
            + size_of::<ModelPlan<'_>>()
            + map_peak(1).unwrap()
            + size_of::<OwnedProof>(),
        "all four private proof forecasts stay fully paid and classified"
    );
    assert_eq!(b.feature_models.committed.len(), 1);
    let before = b.local_shape();
    let hit = b.native_atom_borrowed(&raw).unwrap();
    let after = b.local_shape();
    assert_eq!(
        b.producer_scratch_bytes - first_scratch,
        size_of::<ModelPlan<'_>>()
    );
    assert_eq!(hit.retained_visits(), first.retained_visits());
    assert_eq!(hit.ordinary_depth(), first.ordinary_depth());
    assert!(after.visits - before.visits < hit.retained_visits());
    let proof = b.feature_models.committed.values().next().unwrap().proof;
    let mut raw_reference = PublishedValueArenaBuilder::new();
    native::audit(&mut raw_reference, &raw, 0).unwrap();
    assert_eq!(
        after.bytes - before.bytes,
        raw_reference.charged_bytes() + size_of::<NativeCertificate>() + size_of::<ModelPlan<'_>>(),
        "hit re-admits the entire body plus its certificate and borrowed plan"
    );
    assert_eq!(
        first_live_bytes - first_scratch,
        raw_reference.charged_bytes() + size_of::<NativeCertificate>(),
        "first miss retains the same complete native body and certificate as a hit"
    );
    assert_eq!(
        after.bytes - before.bytes - (b.producer_scratch_bytes - first_scratch),
        raw_reference.charged_bytes() + size_of::<NativeCertificate>()
    );
    assert_eq!(after.edges - before.edges, proof.canonical.edges);
    let mut model_reference = PublishedValueArenaBuilder::new();
    native::features(&mut model_reference, f.model(), 1).unwrap();
    let full = model_reference.local_shape();
    assert_eq!(proof.canonical.bytes, full.bytes);
    assert_eq!(proof.canonical.edges, full.edges);
    assert_eq!(proof.canonical.nodes, full.nodes);
    assert_eq!(proof.canonical.visits, full.visits);
    assert_eq!(proof.canonical.depth, full.depth - 1);
    let mut reference = PublishedValueArenaBuilder::new();
    native::audit(&mut reference, &configured, 0).unwrap();
    let prior_bytes = b.charged_bytes();
    let prior_scratch = b.producer_scratch_bytes;
    let configured = b.native_atom_borrowed(&configured).unwrap();
    assert_eq!(
        b.producer_scratch_bytes - prior_scratch,
        size_of::<ModelPlan<'_>>()
    );
    assert_eq!(
        b.charged_bytes() - prior_bytes - (b.producer_scratch_bytes - prior_scratch),
        reference.charged_bytes() + size_of::<NativeCertificate>(),
        "configuration outer fields and suffix remain fully retained"
    );
    assert_eq!(configured.retained_visits(), reference.charged_visits());
    assert_eq!(b.feature_models.committed.len(), 1);
    assert_eq!(b.maximum_depth, 200);
    assert!(b.owner_audit);
    let separate = features("feature".into());
    assert_eq!(f.model(), separate.model());
    assert!(!Arc::ptr_eq(f.model(), separate.model()));
    let separate = AnalysisValue::cc_toolchain_features(separate);
    b.native_atom_borrowed(&separate).unwrap();
    assert_eq!(b.feature_models.committed.len(), 2);
    // Reverse first-use order must not mix the configuration's suffix/header
    // into the shared MODEL proof consumed by a later Features root.
    let mut reverse = PublishedValueArenaBuilder::new();
    let config_first = reverse.native_atom_borrowed(configured.value()).unwrap();
    assert_eq!(config_first.retained_visits(), reference.charged_visits());
    let features_second = reverse.native_atom_borrowed(&raw).unwrap();
    assert_eq!(features_second.retained_visits(), first.retained_visits());
    assert_eq!(features_second.ordinary_depth(), first.ordinary_depth());
    assert_eq!(reverse.feature_models.committed.len(), 1);
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
fn published_feature_origin_and_poison_refuse_before_a_warmed_cache_lookup() {
    let raw = AnalysisValue::cc_toolchain_features(features("feature".into()));
    let mut producer = PublishedValueArenaBuilder::new();
    let atom = producer.native_atom_borrowed(&raw).unwrap();
    let id = producer.reserve().unwrap();
    producer.set_node(id, PublishedNode::Native(atom)).unwrap();
    producer
        .set_original_slot(id, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    let arena = producer.seal(Arc::new(Originals)).unwrap();
    let origin =
        AnalysisValue::with_original_origin(raw.clone(), arena.value(id).unwrap()).unwrap();
    for borrowed in [false, true] {
        let mut b = PublishedValueArenaBuilder::new();
        b.native_atom_borrowed(&raw).unwrap();
        b.owner_audit = true;
        let before = b.charged_visits();
        let result = if borrowed {
            b.native_atom_borrowed(&origin)
        } else {
            b.native_atom(origin.clone())
        };
        assert_eq!(
            result.unwrap_err(),
            PublishedValueError::Invalid("arena-bearing native atom")
        );
        assert_eq!(
            b.charged_visits() - before,
            1,
            "only unchanged native header ran"
        );
        assert_eq!(b.feature_models.committed.len(), 1);
        assert!(b.imports().is_empty());
        assert!(b.owner_audit);
        let before = b.charged_visits();
        assert!(b.native_atom_borrowed(&raw).is_err());
        assert_eq!(
            b.charged_visits(),
            before,
            "poison refuses before any cache hit"
        );
    }
}

#[test]
fn first_and_growing_cache_refusal_clone_no_guard_and_owner_drop_releases_it() {
    let f = features("feature".into());
    let raw = AnalysisValue::cc_toolchain_features(f.clone());
    let refs = Arc::strong_count(f.model());
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_bytes(BYTE_LIMIT - 1).unwrap();
    assert!(b.native_atom_borrowed(&raw).is_err());
    assert_eq!(Arc::strong_count(f.model()), refs);
    assert!(b.feature_models.committed.is_empty());
    assert_eq!((b.producer_scratch_bytes, b.feature_models.credit), (0, 0));
    assert!(!b.feature_models.header_admitted);
    let mut b = PublishedValueArenaBuilder::new();
    for i in 0..4 {
        b.native_atom(AnalysisValue::cc_toolchain_features(features(
            format!("feature{i}").into(),
        )))
        .unwrap();
    }
    let mut reference = PublishedValueArenaBuilder::new();
    native::audit(&mut reference, &raw, 0).unwrap();
    let body =
        reference.charged_bytes() + size_of::<NativeCertificate>() + size_of::<ModelPlan<'_>>();
    b.charge_bytes(BYTE_LIMIT - b.charged_bytes() - body)
        .unwrap();
    b.owner_audit = true;
    let prior_scratch = b.producer_scratch_bytes;
    let prior_credit = b.feature_models.credit;
    assert!(b.native_atom_borrowed(&raw).is_err());
    assert_eq!(b.charged_bytes(), BYTE_LIMIT, "no live construction refund");
    assert_eq!(
        b.producer_scratch_bytes - prior_scratch,
        size_of::<ModelPlan<'_>>()
    );
    assert_eq!(
        b.feature_models.credit, prior_credit,
        "failed growth commits no credit"
    );
    assert_eq!(b.feature_models.committed.len(), 4);
    assert_eq!(Arc::strong_count(f.model()), refs);
    assert!(b.owner_audit);
    assert!(b.charge_visit().is_err());
    let required = {
        let mut probe = PublishedValueArenaBuilder::new();
        drop(probe.native_atom_borrowed(&raw).unwrap());
        probe.charged_bytes()
    };
    for extra in [0, 1] {
        let mut bounded = PublishedValueArenaBuilder::new();
        bounded.charge_bytes(BYTE_LIMIT - required + extra).unwrap();
        let refs = Arc::strong_count(f.model());
        let result = bounded.native_atom_borrowed(&raw);
        assert_eq!(result.is_ok(), extra == 0);
        drop(result);
        if extra == 0 {
            assert_eq!(bounded.charged_bytes(), BYTE_LIMIT);
            assert_eq!(bounded.feature_models.committed.len(), 1);
        } else {
            assert!(bounded.feature_models.committed.is_empty());
            assert_eq!(bounded.feature_models.credit, 0);
            assert_eq!(Arc::strong_count(f.model()), refs);
            assert_eq!(
                bounded.producer_scratch_bytes,
                size_of::<FeatureModelAudits>() + size_of::<ModelPlan<'_>>() + map_peak(1).unwrap(),
                "failed OwnedProof admission commits neither its bytes nor a guard"
            );
            assert!(bounded.charge_bytes(0).is_err());
        }
        drop(bounded);
        assert_eq!(Arc::strong_count(f.model()), refs);
    }
    let weak = Arc::downgrade(f.model());
    let mut owner = PublishedValueArenaBuilder::new();
    let atom = owner.native_atom_borrowed(&raw).unwrap();
    assert!(owner.producer_scratch_bytes > 0);
    drop(atom);
    drop(raw);
    drop(f);
    assert!(
        weak.upgrade().is_some(),
        "memo owns the immutable allocation guard"
    );
    drop(owner);
    assert!(weak.upgrade().is_none());
}

#[test]
fn guarded_model_mutation_detaches_and_is_a_fresh_full_audit() {
    let f = features("feature".into());
    let mut b = PublishedValueArenaBuilder::new();
    b.native_atom(AnalysisValue::cc_toolchain_features(f.clone()))
        .unwrap();
    let mut detached = Arc::clone(f.model());
    assert!(Arc::get_mut(&mut detached).is_none());
    Arc::make_mut(&mut detached).tools_directory = "changed_tools".into();
    assert!(!Arc::ptr_eq(&detached, f.model()));
    assert_eq!(f.model().tools_directory, "tools");
    let before = b.charged_visits();
    let plan = b.feature_model_plan(&detached, 1).unwrap();
    assert!(plan.pending);
    assert_eq!(b.charged_visits() - before, plan.proof.canonical.visits + 1);
    b.commit_feature_model(plan).unwrap();
    assert_eq!(b.feature_models.committed.len(), 2);
}
