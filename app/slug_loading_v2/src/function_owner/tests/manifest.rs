//! Canonical local facts preserve every validated expanded provenance field.
use super::*;

fn leaf_manifest(name: usize) -> BzlLoadManifest {
    let mut value = manifest();
    value.root.label = slug_identity_v2::CanonicalLabel::parse(&format!(
        "@@unrelated+//moved:canonical{name}.bzl"
    ))
    .unwrap();
    value.root.workspace_path = format!("/same/canonical{name}.bzl").into();
    value.reachable = Arc::from([crate::BzlModuleSourceProvenance::new(
        value.root.clone(),
        [0; 32],
    )]);
    value
}
fn joined_manifest(name: usize, children: &[&BzlLoadManifest]) -> BzlLoadManifest {
    let mut value = leaf_manifest(name);
    let mut direct = Vec::new();
    let mut reachable = value.reachable.to_vec();
    for child in children {
        if !direct
            .iter()
            .any(|identity: &BzlModuleIdentity| identity.label == child.root.label)
        {
            direct.push(child.root.clone());
        }
        for source in child.reachable.iter() {
            if !reachable
                .iter()
                .any(|previous| previous.identity.label == source.identity.label)
            {
                reachable.push(source.clone());
            }
        }
    }
    value.direct_children = direct.into();
    value.reachable = reachable.into();
    value
}
fn local(owner: &FunctionEvaluationOwner) -> &BzlInputValidity {
    owner.input_validity().as_any().downcast_ref().unwrap()
}

#[test]
fn canonical_manifest_order_preserves_raw_child_multiplicity_and_modes() {
    let leaf_manifest = leaf_manifest(0);
    let leaf = owner(&leaf_manifest, "leaf", false, []).unwrap();
    let left_manifest = joined_manifest(1, &[&leaf_manifest]);
    let left = owner(&left_manifest, "left", false, [leaf.clone()]).unwrap();
    let right_manifest = joined_manifest(2, &[&leaf_manifest]);
    let right = owner(&right_manifest, "right", false, [leaf.clone()]).unwrap();
    let top_manifest = joined_manifest(3, &[&left_manifest, &right_manifest, &left_manifest]);
    let top = owner(&top_manifest, "top", false, [left.clone(), right, left]).unwrap();
    let rows = &local(&top).rows;
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].children.len(), 3);
    assert!(Arc::ptr_eq(&rows[0].children[0], &rows[0].children[2]));
    assert_eq!(
        rows.iter().map(|row| &row.key.identity).collect::<Vec<_>>(),
        top_manifest
            .reachable
            .iter()
            .map(|source| &source.identity)
            .collect::<Vec<_>>()
    );
    // Same module identity under different load modes remains two complete
    // local facts; label-first original provenance does not erase those modes.
    let bzlmod = owner(&leaf_manifest, "leaf", true, []).unwrap();
    let mixed_manifest = joined_manifest(4, &[&leaf_manifest, &leaf_manifest]);
    let mixed = owner(&mixed_manifest, "mixed", false, [leaf, bzlmod]).unwrap();
    assert_eq!(local(&mixed).rows.len(), 3);
    assert_eq!(local(&mixed).rows[0].children.len(), 2);
    assert_ne!(local(&mixed).rows[1].key, local(&mixed).rows[2].key);
}

#[test]
fn canonical_manifest_validation_refuses_missing_extra_reordered_and_changed_full_facts() {
    let a_manifest = leaf_manifest(0);
    let b_manifest = leaf_manifest(1);
    let a = owner(&a_manifest, "a", false, []).unwrap();
    let b = owner(&b_manifest, "b", false, []).unwrap();
    let valid = joined_manifest(2, &[&a_manifest, &b_manifest]);
    owner(&valid, "parent", false, [a.clone(), b.clone()]).unwrap();
    let mut variants = Vec::new();
    let mut changed = valid.clone();
    changed.reachable = Arc::from([]);
    variants.push(changed);
    let mut changed = valid.clone();
    changed.root.workspace_path = "/different/root.bzl".into();
    variants.push(changed);
    let mut changed = valid.clone();
    changed.direct_children = Arc::from([b_manifest.root.clone(), a_manifest.root.clone()]);
    variants.push(changed);
    let mut changed = valid.clone();
    changed.direct_children = Arc::from([a_manifest.root.clone()]);
    variants.push(changed);
    let mut changed = valid.clone();
    changed.direct_children = Arc::from([
        a_manifest.root.clone(),
        b_manifest.root.clone(),
        a_manifest.root.clone(),
    ]);
    variants.push(changed);
    for operation in 0..6 {
        let mut changed = valid.clone();
        let mut sources = changed.reachable.to_vec();
        match operation {
            0 => {
                sources.remove(1);
            }
            1 => sources.push(sources[1].clone()),
            2 => sources.swap(1, 2),
            3 => sources[1].source_digest = [9; 32],
            4 => sources[1].identity.workspace_path = "/different/child.bzl".into(),
            5 => {
                sources[1].identity.repository_mapping = Arc::from([(
                    slug_identity_v2::ApparentRepoName::new("alias").unwrap(),
                    slug_identity_v2::CanonicalRepoName::new("different+").unwrap(),
                )])
            }
            _ => unreachable!(),
        }
        changed.reachable = sources.into();
        variants.push(changed);
    }
    for (index, variant) in variants.iter().enumerate() {
        let error = owner(variant, "parent", false, [a.clone(), b.clone()]).unwrap_err();
        assert!(
            error.to_string().contains("Bzl manifest"),
            "variant {index}: {error}"
        );
    }
}

#[test]
fn canonical_integrity_metadata_remains_observable_with_independent_full_source_equality() {
    let original = leaf_manifest(0);
    let a = owner(&original, "def f(): return 1", false, []).unwrap();
    assert_eq!(a, owner(&original, "def f(): return 1", false, []).unwrap());
    assert_ne!(a, owner(&original, "def f(): return 2", false, []).unwrap());
    let mut digest = original.clone();
    digest.reachable = Arc::from([crate::BzlModuleSourceProvenance::new(
        digest.root.clone(),
        [1; 32],
    )]);
    assert_ne!(a, owner(&digest, "def f(): return 1", false, []).unwrap());
    let mut fingerprint = original.clone();
    fingerprint.fingerprint = [2; 32];
    assert_ne!(
        a,
        owner(&fingerprint, "def f(): return 1", false, []).unwrap()
    );
    let mut mapping = original.clone();
    mapping.root.repository_mapping = Arc::from([(
        slug_identity_v2::ApparentRepoName::new("alias").unwrap(),
        slug_identity_v2::CanonicalRepoName::new("changed+").unwrap(),
    )]);
    mapping.reachable = Arc::from([crate::BzlModuleSourceProvenance::new(
        mapping.root.clone(),
        [0; 32],
    )]);
    assert_ne!(a, owner(&mapping, "def f(): return 1", false, []).unwrap());
}

#[test]
fn canonical_local_visits_are_linear_while_all_original_metadata_bytes_and_edges_remain() {
    let mut current_manifest = leaf_manifest(0);
    let mut current = owner(&current_manifest, "leaf", false, []).unwrap();
    for level in 1..=48 {
        current_manifest = joined_manifest(level, &[&current_manifest]);
        current = owner(&current_manifest, "parent", false, [current]).unwrap();
        let rows = &local(&current).rows;
        assert_eq!(rows.len(), level + 1);
        assert_eq!(current.retained_shape().nodes, rows.len());
        let semantic_visits: usize = rows.iter().map(|row| row.shape.visits).sum();
        let original_bytes: usize = rows.iter().map(|row| row.original_metadata.bytes).sum();
        let original_edges: usize = rows.iter().map(|row| row.original_metadata.edges).sum();
        assert!(rows.iter().all(|row| row.original_metadata.visits == 0));
        // All non-leaf rows have the same local semantic shape regardless of
        // how many descendants their original client-data manifest retains.
        assert!(
            rows[..level]
                .iter()
                .all(|row| row.shape.visits == rows[0].shape.visits)
        );
        assert_eq!(
            semantic_visits,
            level * rows[0].shape.visits + rows[level].shape.visits
        );
        assert!(current.retained_shape().bytes >= original_bytes);
        assert!(current.retained_shape().edges >= original_edges);
        assert!(rows[0].original_metadata.bytes > rows[level].original_metadata.bytes);
        assert!(rows[0].original_metadata.edges > rows[level].original_metadata.edges);
    }
}

#[test]
fn oversized_original_manifest_refuses_before_retaining_child_handles_or_copying_local_facts() {
    let mut oversized = leaf_manifest(0);
    oversized.reachable = vec![oversized.reachable[0].clone(); 65_536].into();
    let child_manifest = leaf_manifest(1);
    let child = owner(&child_manifest, "child", false, []).unwrap();
    let consumed = std::cell::Cell::new(0);
    let children = std::iter::from_fn(|| {
        consumed.set(consumed.get() + 1);
        Some(child.clone())
    });
    assert!(
        owner(&oversized, "root", false, children)
            .unwrap_err()
            .to_string()
            .contains("shape limit")
    );
    assert_eq!(consumed.get(), 0);
}
