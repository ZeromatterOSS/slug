use std::collections::hash_map::DefaultHasher;

use dice::DetectCycles;
use dice::Dice;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::*;

fn identity(name: &str) -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse(&format!("@@//:{name}.bzl")).unwrap(),
        workspace_path: format!("/semantic-inputs/{name}.bzl").into(),
        repository_mapping: Arc::from([]),
    }
}

fn leaf_key(identity: &BzlModuleIdentity, source: &Arc<[u8]>) -> SemanticBzlEvaluationKey {
    prepare(
        identity,
        "/semantic-inputs/source.bzl",
        source,
        BzlModuleContext::Build,
        std::iter::empty(),
    )
    .unwrap()
}

fn hash(key: &SemanticBzlEvaluationKey) -> u64 {
    let mut state = DefaultHasher::new();
    key.hash(&mut state);
    state.finish()
}

#[test]
fn complete_source_mapping_label_provenance_and_modes_determine_keys() {
    let identity = identity("root");
    let source: Arc<[u8]> = Arc::from(b"value = 1\n".as_slice());
    let equal_source: Arc<[u8]> = Arc::from(b"value = 1\n".as_slice());
    assert!(!Arc::ptr_eq(&source, &equal_source));
    let first = leaf_key(&identity, &source);
    let equal = leaf_key(&identity, &equal_source);
    assert_eq!(first, equal);
    assert_eq!(hash(&first), hash(&equal));
    assert_eq!(first.0.budget.records, equal.0.budget.records);
    assert_eq!(first.0.budget.bytes, equal.0.budget.bytes);
    assert_ne!(
        first,
        leaf_key(&identity, &Arc::from(b"value = 2\n".as_slice()))
    );
    assert_ne!(
        first,
        prepare(
            &identity,
            "/semantic-inputs/source.bzl",
            &source,
            BzlModuleContext::Bzlmod,
            std::iter::empty()
        )
        .unwrap()
    );
    assert_ne!(
        first,
        prepare(
            &identity,
            "/semantic-inputs/other.bzl",
            &source,
            BzlModuleContext::Build,
            std::iter::empty()
        )
        .unwrap()
    );

    let mut changed_mapping = identity.clone();
    changed_mapping.repository_mapping = Arc::from([(
        ApparentRepoName::new("dep").unwrap(),
        CanonicalRepoName::new("dep+").unwrap(),
    )]);
    assert_ne!(first, leaf_key(&changed_mapping, &source));
    let mut provenance_a = identity.clone();
    let mut provenance_b = identity;
    provenance_a.label =
        ApparentLabel::parse("//:root.bzl")
            .unwrap()
            .resolve(&RepositoryMapping::new(
                RepositoryMappingId::new("a").unwrap(),
            ));
    provenance_b.label =
        ApparentLabel::parse("//:root.bzl")
            .unwrap()
            .resolve(&RepositoryMapping::new(
                RepositoryMappingId::new("b").unwrap(),
            ));
    assert_ne!(
        leaf_key(&provenance_a, &source),
        leaf_key(&provenance_b, &source)
    );
}

#[tokio::test]
async fn ordered_repeated_loads_and_conflicting_complete_child_rows_are_checked() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = dice.updater().commit().await;
    let source_a: Arc<[u8]> = Arc::from(b"value = 1\n".as_slice());
    let source_b: Arc<[u8]> = Arc::from(b"value = 2\n".as_slice());
    let child_identity = identity("child");
    let a = transaction
        .compute(&leaf_key(&child_identity, &source_a))
        .await
        .unwrap();
    let b = transaction
        .compute(&leaf_key(&child_identity, &source_b))
        .await
        .unwrap();
    let other = transaction
        .compute(&leaf_key(&identity("other"), &source_a))
        .await
        .unwrap();
    let a = a.result().unwrap();
    let b = b.result().unwrap();
    let other = other.result().unwrap();
    let root = identity("root");
    let make = |children: &[(&str, &FrozenBzlModule)]| {
        prepare(
            &root,
            "root.bzl",
            &source_a,
            BzlModuleContext::Build,
            children.iter().copied(),
        )
    };
    let forward = make(&[(":child.bzl", a), (":other.bzl", other)]).unwrap();
    let reverse = make(&[(":other.bzl", other), (":child.bzl", a)]).unwrap();
    assert_ne!(forward, reverse);
    let repeated = make(&[(":child.bzl", a), (":child.bzl", a)]).unwrap();
    assert_ne!(repeated, make(&[(":child.bzl", a)]).unwrap());
    assert_eq!(
        repeated.0.rows.len(),
        2,
        "flat source rows deduplicate while both ordered edges remain"
    );
    let error = make(&[(":first.bzl", a), (":second.bzl", b)]).unwrap_err();
    assert!(
        error.contains("conflicting complete local facts"),
        "{error}"
    );

    // Repeated stable-key comparison visits the complete child mapping at
    // every edge, even though all references share its immutable allocation.
    let mut wide_identity = identity("wide");
    wide_identity.repository_mapping = (0..128)
        .map(|index| {
            (
                ApparentRepoName::new(format!("dep{index}")).unwrap(),
                CanonicalRepoName::new(format!("dep{index}+")).unwrap(),
            )
        })
        .collect::<Vec<_>>()
        .into();
    let wide = transaction
        .compute(&leaf_key(&wide_identity, &source_a))
        .await
        .unwrap();
    let wide = wide.result().unwrap();
    let wide_inputs = wide.semantic_inputs.as_ref().unwrap();
    assert!(
        facts::row_budget(
            &root,
            "root.bzl",
            &source_a,
            std::iter::repeat((":wide.bzl", wide_inputs.rows[0].key.as_ref())).take(600)
        )
        .is_err()
    );
    let wide_refs = Arc::strong_count(wide_inputs);
    assert!(
        prepare(
            &root,
            "root.bzl",
            &source_a,
            BzlModuleContext::Build,
            std::iter::repeat((":wide.bzl", wide)).take(600)
        )
        .is_err()
    );
    assert_eq!(Arc::strong_count(wide_inputs), wide_refs);

    let source_refs = Arc::strong_count(&source_a);
    let child_refs = Arc::strong_count(a.semantic_inputs.as_ref().unwrap());
    let error = prepare(
        &root,
        "root.bzl",
        &source_a,
        BzlModuleContext::Build,
        std::iter::repeat((":child.bzl", a)).take(budget::RECORD_LIMIT),
    )
    .unwrap_err();
    assert!(error.contains("resource limit"), "{error}");
    assert_eq!(Arc::strong_count(&source_a), source_refs);
    assert_eq!(
        Arc::strong_count(a.semantic_inputs.as_ref().unwrap()),
        child_refs
    );
}

#[test]
fn input_budget_checks_byte_record_overflow_and_depth_boundaries() {
    let mut records = budget::Budget::default();
    records.charge(budget::RECORD_LIMIT, 0).unwrap();
    assert!(records.charge(1, 0).is_err());
    let mut bytes = budget::Budget::default();
    bytes.charge(0, budget::BYTE_LIMIT).unwrap();
    assert!(bytes.charge(0, 1).is_err());
    let mut overflow = budget::Budget::default();
    assert!(overflow.charge(usize::MAX, 0).is_err());
    let mut depth = budget::Budget::default();
    depth.depth(budget::DEPTH_LIMIT).unwrap();
    assert!(depth.depth(budget::DEPTH_LIMIT + 1).is_err());
}
