//! Independent adversarial composition checks; basic cache/storage tests are separate.
use super::*;
use crate::cc_features::*;

fn model() -> CcToolchainFeatures {
    let group = CcFlagGroup {
        expandables: vec![CcExpandable::Flag(
            vec![CcStringChunk::Literal("-DRETAIN".into())].into(),
        )]
        .into(),
        iterate_over: None,
        expand_if_available: None,
        expand_if_not_available: None,
        expand_if_true: None,
        expand_if_false: None,
        expand_if_equal: None,
    };
    CcToolchainFeatures::new(
        vec![CcFeature {
            name: "selected".into(),
            enabled: false,
            flag_sets: vec![CcFlagSet {
                actions: ["compile".into()].into_iter().collect(),
                with_features: CcSet::new(),
                flag_groups: vec![group].into(),
            }]
            .into(),
            env_sets: vec![].into(),
            requires: vec![].into(),
            implies: vec![].into(),
            provides: vec![].into(),
        }],
        vec![],
        vec![],
        "tools",
        CcExecOs::Linux,
    )
    .unwrap()
}
#[derive(Debug)]
struct Originals {
    roots: usize,
    imports: Vec<PublishedValueArena>,
}
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn root_count(&self) -> usize {
        self.roots
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
}
fn row(b: &mut PublishedValueArenaBuilder, node: PublishedNode) -> PublishedNodeId {
    let id = b.reserve().unwrap();
    b.set_node(id, node).unwrap();
    b.set_original_slot(id, PublishedRootSlot::new(id.index()).unwrap())
        .unwrap();
    id
}
fn finish(b: PublishedValueArenaBuilder) -> Result<PublishedValueArena, PublishedValueError> {
    let originals = Originals {
        roots: b.rows.len(),
        imports: b.imports.clone(),
    };
    b.seal(Arc::new(originals))
}
fn reference(value: &AnalysisValue) -> (usize, usize) {
    let mut b = PublishedValueArenaBuilder::new();
    native::audit(&mut b, value, 0).unwrap();
    (b.charged_visits(), b.maximum_depth)
}
fn graph(
    separate_config_model: bool,
    split_wrapper_alias: bool,
) -> (PublishedValueArena, PublishedNodeId) {
    let features = model();
    let config_model = if separate_config_model {
        model()
    } else {
        features.clone()
    };
    let alias = if split_wrapper_alias {
        model()
    } else {
        features.clone()
    };
    let values = [
        AnalysisValue::cc_toolchain_features(features),
        AnalysisValue::cc_feature_configuration(config_model.configure_features(vec![]).unwrap()),
        AnalysisValue::cc_toolchain_features(alias),
    ];
    let mut b = PublishedValueArenaBuilder::new();
    let mut edges = Vec::new();
    for value in values {
        let complete = reference(&value);
        let atom = b.native_atom_borrowed(&value).unwrap();
        assert_eq!((atom.retained_visits(), atom.ordinary_depth()), complete);
        edges.push(PublishedEdge::Local(row(
            &mut b,
            PublishedNode::Native(atom),
        )));
    }
    assert_eq!(
        b.feature_models.committed.len(),
        1 + usize::from(separate_config_model) + usize::from(split_wrapper_alias)
    );
    let range = b.append_edges(&edges).unwrap();
    let root = row(&mut b, PublishedNode::Tuple(range));
    (finish(b).unwrap(), root)
}
fn imported(
    arena: PublishedValueArena,
    root: PublishedNodeId,
) -> (PublishedValueArena, PublishedNodeId) {
    let mut b = PublishedValueArenaBuilder::new();
    let expected = arena.shape().visits;
    let expected_bytes = arena.shape().bytes;
    let index = b.import(arena).unwrap();
    assert_eq!(
        b.import_bytes, expected_bytes,
        "full retained model bytes survive import"
    );
    assert_eq!(
        b.import_visits, expected,
        "full sealed facts remain admitted"
    );
    assert_eq!(
        b.charged_visits(),
        2,
        "one closure lookup and one import registration"
    );
    assert_eq!(b.complete_shape().unwrap().visits, expected + 2);
    let range = b
        .append_edges(&[PublishedEdge::Imported {
            arena: index,
            node: root,
        }])
        .unwrap();
    let root = row(&mut b, PublishedNode::Tuple(range));
    (finish(b).unwrap(), root)
}

#[test]
fn reused_model_certificates_keep_full_sealed_import_counts_and_joint_wrapper_aliases() {
    let (shared, a) = graph(false, false);
    let (separate, b) = graph(true, false);
    let (split, c) = graph(false, true);
    assert_eq!(shared.shape().visits, separate.shape().visits);
    assert_eq!(shared.shape().visits, split.shape().visits);
    assert_eq!(shared.shape().bytes, separate.shape().bytes);
    assert_eq!(shared.shape().bytes, split.shape().bytes);
    assert!(
        shared
            .value(a)
            .unwrap()
            .publication_eq(&separate.value(b).unwrap())
    );
    assert!(
        !shared
            .value(a)
            .unwrap()
            .publication_eq(&split.value(c).unwrap())
    );
    assert!(
        !split
            .value(c)
            .unwrap()
            .publication_eq(&shared.value(a).unwrap())
    );
    let (shared, a) = imported(shared, a);
    let (separate, b) = imported(separate, b);
    assert_eq!(shared.shape().visits, separate.shape().visits);
    assert_eq!(shared.shape().bytes, separate.shape().bytes);
    assert!(
        shared
            .value(a)
            .unwrap()
            .publication_eq(&separate.value(b).unwrap())
    );
}

#[test]
fn cached_feature_height_survives_enclosing_containers_and_refuses_the_next_level() {
    for extra in [0, 1] {
        let features = model();
        let value = AnalysisValue::cc_toolchain_features(features);
        let complete = reference(&value);
        assert!(complete.1 > 1);
        let mut b = PublishedValueArenaBuilder::new();
        drop(b.native_atom_borrowed(&value).unwrap());
        let before = b.charged_visits();
        let hit = b.native_atom_borrowed(&value).unwrap();
        assert!(b.charged_visits() - before < complete.0);
        assert_eq!((hit.retained_visits(), hit.ordinary_depth()), complete);
        let mut edge = PublishedEdge::Local(row(&mut b, PublishedNode::Native(hit)));
        for _ in 0..DEPTH_LIMIT - complete.1 + extra {
            let range = b.append_edges(&[edge]).unwrap();
            edge = PublishedEdge::Local(row(&mut b, PublishedNode::Tuple(range)));
        }
        let result = finish(b);
        if extra == 0 {
            assert_eq!(result.unwrap().shape().depth, DEPTH_LIMIT);
        } else {
            assert!(matches!(
                result,
                Err(PublishedValueError::Limit("projection depth"))
            ));
        }
    }
}

fn warm(features: &CcToolchainFeatures) -> PublishedValueArenaBuilder {
    let mut b = PublishedValueArenaBuilder::new();
    drop(
        b.native_atom(AnalysisValue::cc_toolchain_features(features.clone()))
            .unwrap(),
    );
    b
}
#[test]
fn warmed_model_does_not_hide_suffix_refusal_or_restore_a_failed_builder() {
    let features = model();
    let empty =
        AnalysisValue::cc_feature_configuration(features.configure_features(vec![]).unwrap());
    let changed = AnalysisValue::cc_feature_configuration(
        features
            .configure_features(vec!["unknown_request".repeat(64).into()])
            .unwrap(),
    );
    let mut probe = warm(&features);
    let before = probe.charged_bytes();
    drop(probe.native_atom_borrowed(&empty).unwrap());
    let empty_allowance = probe.charged_bytes() - before;
    let mut b = warm(&features);
    b.charge_bytes(BYTE_LIMIT - b.charged_bytes() - empty_allowance)
        .unwrap();
    b.owner_audit = true;
    b.maximum_depth = 11;
    let references = Arc::strong_count(features.model());
    assert!(matches!(
        b.native_atom_borrowed(&changed),
        Err(PublishedValueError::ByteBudget(refusal))
            if !refusal.prior_poisoned()
                && refusal.limit_bytes() == BYTE_LIMIT
                && refusal.checked_total_bytes().is_some_and(|total| total > BYTE_LIMIT)
    ));
    assert_eq!(Arc::strong_count(features.model()), references);
    assert_eq!(b.feature_models.committed.len(), 1);
    assert!(b.owner_audit);
    assert_eq!(b.maximum_depth, 11);
    assert!(b.failed_budget);
    assert!(b.native_atom_borrowed(&empty).is_err());
    assert_eq!(b.feature_models.committed.len(), 1);
}
