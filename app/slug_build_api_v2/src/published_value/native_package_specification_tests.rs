//! Complete hidden membership storage is audited before retaining native handles.
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use starlark_map::small_set::SmallSet;

use super::*;
use crate::PackageGroupContents;
use crate::PackageGroupContentsInput;
use crate::PackageSpecificationInfo;

fn package(path: &str) -> PackageIdentifier {
    PackageIdentifier::new(CanonicalRepoName::root(), PackagePath::parse(path).unwrap())
}
fn info(input: PackageGroupContentsInput) -> PackageSpecificationInfo {
    PackageSpecificationInfo::new(Arc::new(PackageGroupContents::new(input)), []).unwrap()
}
fn shape(value: &PackageSpecificationInfo, depth: usize) -> PublicationShape {
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_package_specification_at_depth(value, depth)
        .unwrap();
    b.local_shape()
}

#[test]
fn package_storage_forecast_overflow_poisons_the_shared_strict_path() {
    assert_eq!(
        owner::package_subtree_storage_bytes(0),
        Some(2 * std::mem::size_of::<usize>())
    );
    let overflow = owner::package_subtree_storage_bytes(usize::MAX);
    assert_eq!(overflow, None);
    let mut b = PublishedValueArenaBuilder::new();
    assert!(owner::charge_package_group_storage(&mut b, overflow).is_err());
    assert!(b.audit_native(&AnalysisValue::none()).is_err());
    assert!(b.charge_bytes(0).is_err());
}

#[test]
fn empty_reserved_and_cloned_package_sets_refuse_before_handle_retention() {
    let plain = info(Default::default());
    let reserved = SmallSet::with_capacity(128);
    let cloned = reserved.clone();
    assert_eq!(cloned.capacity(), 0);
    for set in [reserved, cloned] {
        let group = Arc::new(PackageGroupContents::new(PackageGroupContentsInput {
            exact_positive: set,
            ..Default::default()
        }));
        let value = PackageSpecificationInfo::new(group.clone(), []).unwrap();
        let before = Arc::strong_count(&group);
        assert!(shape(&value, 0).bytes > shape(&plain, 0).bytes);
        let mut b = PublishedValueArenaBuilder::new();
        b.charge_bytes(BYTE_LIMIT - shape(&plain, 0).bytes).unwrap();
        assert!(b.audit_package_specification_at_depth(&value, 0).is_err());
        assert_eq!(Arc::strong_count(&group), before);
        assert!(b.charge_bytes(0).is_err(), "refusal poisons the builder");
        assert!(b.imports().is_empty());
    }
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_bytes(BYTE_LIMIT - shape(&plain, 0).bytes).unwrap();
    b.audit_package_specification_at_depth(&plain, 0).unwrap();
}

#[test]
fn package_audit_covers_both_sets_arrays_and_hidden_publication() {
    let input = PackageGroupContentsInput {
        exact_positive: [package("one")].into_iter().collect(),
        subtree_positive: [package("tree")].into(),
        exact_negative: [package("tree/no")].into_iter().collect(),
        subtree_negative: [package("tree/excluded")].into(),
        positive_all: true,
        has_private: true,
    };
    let expected_sets = input
        .exact_positive
        .retained_storage_bytes_upper_bound()
        .unwrap()
        + input
            .exact_negative
            .retained_storage_bytes_upper_bound()
            .unwrap();
    let value = info(input);
    let plain = info(Default::default());
    let text_bytes: usize = ["one", "tree", "tree/no", "tree/excluded"]
        .into_iter()
        .map(|p| p.len() + 2 * std::mem::size_of::<compact_str::CompactString>())
        .sum();
    assert_eq!(
        shape(&value, 0).bytes - shape(&plain, 0).bytes,
        expected_sets + 2 * std::mem::size_of::<PackageIdentifier>() + text_bytes
    );
    assert!(value.contains_package(&package("other")));
    assert!(!value.contains_package(&package("tree/no")));
    let a = AnalysisValue::package_specification_info(value);
    let b = AnalysisValue::package_specification_info(plain);
    assert_eq!(a, b, "native language equality excludes hidden membership");
    let mut builder = PublishedValueArenaBuilder::new();
    let a = builder.native_atom_borrowed(&a).unwrap();
    let b = builder.native_atom_borrowed(&b).unwrap();
    assert!(!a.value().publication_eq(b.value()));
}

#[test]
fn package_audit_preserves_transitive_depth_and_mode_on_poison() {
    let child = info(PackageGroupContentsInput {
        exact_positive: [package("child")].into_iter().collect(),
        ..Default::default()
    });
    let value =
        PackageSpecificationInfo::new(Arc::new(PackageGroupContents::default()), [child]).unwrap();
    assert!(value.contains_package(&package("child")));
    assert_eq!(value.groups().depth(), 2);
    // The provider header and direct group payload are ordinary containment;
    // typed transitive group edges retain their separate logical DAG depth.
    assert_eq!(shape(&value, 17).depth, 17 + 2);
    let outer = shape(&value, 0);
    let mut long = value.clone();
    for _ in 0..31 {
        long = PackageSpecificationInfo::new(Arc::new(PackageGroupContents::default()), [long])
            .unwrap();
    }
    assert_eq!(long.groups().depth(), 33);
    assert!(long.contains_package(&package("child")));
    let long_shape = shape(&long, 17);
    assert_eq!(long_shape.depth, 17 + 2);
    assert!(long_shape.bytes > outer.bytes);
    assert!(long_shape.visits > outer.visits);
    let mut native = PublishedValueArenaBuilder::new();
    native
        .audit_native(&AnalysisValue::package_specification_info(value.clone()))
        .unwrap();
    let actual = native.local_shape();
    assert_eq!(
        (
            actual.nodes,
            actual.edges,
            actual.bytes,
            actual.visits,
            actual.depth
        ),
        (
            outer.nodes,
            outer.edges,
            outer.bytes,
            outer.visits,
            outer.depth
        ),
        "borrowed audit pays the ordinary native header"
    );
    for value in [&value, &long] {
        for prior in [false, true] {
            let mut allowed = PublishedValueArenaBuilder::new();
            allowed.owner_audit = prior;
            allowed
                .audit_package_specification_at_depth(value, DEPTH_LIMIT - 2)
                .unwrap();
            assert_eq!(allowed.local_shape().depth, DEPTH_LIMIT);
            assert_eq!(allowed.owner_audit, prior);
            // First failure reaches the actual direct group payload; the second
            // preserves the existing enclosing-header depth refusal.
            for depth in [DEPTH_LIMIT - 1, DEPTH_LIMIT] {
                let mut b = PublishedValueArenaBuilder::new();
                b.owner_audit = prior;
                assert!(
                    b.audit_package_specification_at_depth(value, depth)
                        .is_err()
                );
                assert_eq!(b.owner_audit, prior);
                assert!(b.charge_bytes(0).is_err());
            }
        }
    }
}
