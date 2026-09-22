use slug_identity_v2::PackagePath;

use super::*;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::ProviderCollection;
use crate::ProviderValue;

fn package(repo: &str, path: &str) -> PackageIdentifier {
    PackageIdentifier::new(
        if repo.is_empty() {
            CanonicalRepoName::root()
        } else {
            CanonicalRepoName::new(repo).unwrap()
        },
        PackagePath::parse(path).unwrap(),
    )
}

fn exact(path: &str) -> Arc<PackageGroupContents> {
    Arc::new(PackageGroupContents::new(PackageGroupContentsInput {
        exact_positive: [package("", path)].into_iter().collect(),
        ..Default::default()
    }))
}

fn info(path: &str) -> PackageSpecificationInfo {
    PackageSpecificationInfo::new(exact(path), []).unwrap()
}

#[test]
fn membership_preserves_boundaries_negatives_and_repository_rebinding() {
    let contents = PackageGroupContents::new(PackageGroupContentsInput {
        exact_positive: [package("", "one")].into_iter().collect(),
        subtree_positive: [package("", "tree")].into(),
        exact_negative: [package("", "tree/no")].into_iter().collect(),
        subtree_negative: [package("", "tree/excluded")].into(),
        ..Default::default()
    });
    for allowed in ["one", "tree", "tree/child", "tree/nothing"] {
        assert!(
            contents.contains_package(&package("", allowed)),
            "{allowed}"
        );
    }
    for denied in [
        "one/child",
        "trees",
        "tree/no",
        "tree/excluded",
        "tree/excluded/child",
    ] {
        assert!(!contents.contains_package(&package("", denied)), "{denied}");
    }
    assert!(!contents.contains_package(&package("other+", "one")));
    let repo = CanonicalRepoName::new("dep+").unwrap();
    let rebound = contents.in_repository_context(&repo).unwrap();
    assert!(rebound.contains_package(&package("dep+", "tree/child")));
    assert!(!rebound.contains_package(&package("", "tree/child")));
    assert!(!rebound.contains_package(&package("dep+", "tree/excluded")));
    assert_eq!(rebound, rebound.in_repository_context(&repo).unwrap());
    assert!(
        rebound
            .in_repository_context(&CanonicalRepoName::new("other+").unwrap())
            .is_err()
    );
    assert!(
        contents
            .in_repository_context(&CanonicalRepoName::root())
            .is_err()
    );
    for has_private in [false, true] {
        let empty = PackageGroupContents::new(PackageGroupContentsInput {
            has_private,
            ..Default::default()
        });
        assert!(!empty.contains_package(&package("", "")));
        assert!(!empty.contains_package(&package("other+", "one")));
    }
    let public = PackageGroupContents::new(PackageGroupContentsInput {
        positive_all: true,
        exact_negative: [package("", "excluded")].into_iter().collect(),
        ..Default::default()
    });
    assert!(public.contains_package(&package("other+", "excluded")));
    assert!(!public.contains_package(&package("", "excluded")));
}

#[test]
fn contents_hash_matches_unordered_sets_and_ordered_subtree_payloads() {
    let build = |reverse: bool| {
        let mut exact = vec![package("", "a"), package("", "b")];
        if reverse {
            exact.reverse();
        }
        PackageGroupContents::new(PackageGroupContentsInput {
            exact_positive: exact.iter().cloned().collect(),
            exact_negative: exact.into_iter().collect(),
            subtree_positive: [package("", "tree/a"), package("", "tree/b")].into(),
            ..Default::default()
        })
    };
    assert_eq!(build(false), build(true));
    assert_eq!(fxhash::hash64(&build(false)), fxhash::hash64(&build(true)));
    let mut reversed = build(false);
    reversed.subtree_positive = reversed.subtree_positive.iter().rev().cloned().collect();
    assert_ne!(build(false), reversed);
    assert_ne!(fxhash::hash64(&build(false)), fxhash::hash64(&reversed));
}

#[test]
fn includes_keep_local_negatives_separate_and_share_dense_diamond_graph() {
    let leaf = info("included");
    let left = PackageSpecificationInfo::new(exact("left"), [leaf.clone()]).unwrap();
    let right = PackageSpecificationInfo::new(exact("right"), [leaf.clone()]).unwrap();
    let local = Arc::new(PackageGroupContents::new(PackageGroupContentsInput {
        exact_positive: [package("", "local")].into_iter().collect(),
        exact_negative: [package("", "included")].into_iter().collect(),
        ..Default::default()
    }));
    assert!(!local.contains_package(&package("", "included")));
    let root = PackageSpecificationInfo::new(local, [left, right]).unwrap();
    for allowed in ["included", "left", "right", "local"] {
        assert!(root.contains_package(&package("", allowed)));
    }
    assert!(!root.contains_package(&package("", "other")));
    assert!(root.groups().shares_node_with(root.clone().groups()));
    let groups = root.groups().to_list();
    assert_eq!(groups.len(), 4);
    assert!(Arc::ptr_eq(&groups[0], &leaf.groups().to_list()[0]));
    assert!(groups[0].contains_package(&package("", "included")));
    assert!(groups[1].contains_package(&package("", "left")));
    assert!(groups[2].contains_package(&package("", "right")));
    assert!(groups[3].contains_package(&package("", "local")));
}

#[test]
fn native_equality_hash_and_dedup_are_distinct_from_full_publication() {
    let left = AnalysisValue::package_specification_info(info("left"));
    let right = AnalysisValue::package_specification_info(info("right"));
    assert_eq!(left, right);
    assert_eq!(
        left.starlark_hash().unwrap(),
        right.starlark_hash().unwrap()
    );
    assert!(!left.publication_eq(&right));
    assert!(left.publication_eq(&AnalysisValue::package_specification_info(info("left"))));
    assert!(left.is_starlark_immutable());
    assert!(left.is_valid_depset_leaf());
    let merged = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![left.clone(), right.clone()],
        vec![],
    )
    .unwrap();
    assert_eq!(merged.to_list().len(), 1);
    let leaves = merged.to_list();
    let AnalysisValueKind::PackageSpecificationInfo(first) = leaves[0].kind() else {
        panic!("provider leaf")
    };
    assert!(first.contains_package(&package("", "left")));
    assert!(!first.contains_package(&package("", "right")));
    assert!(
        AnalysisValue::dictionary([
            (left.clone(), AnalysisValue::none()),
            (right.clone(), AnalysisValue::none())
        ])
        .is_err()
    );
    let nested = |value: AnalysisValue| {
        AnalysisValue::strukt([
            ("list", AnalysisValue::list(vec![value.clone()])),
            (
                "key",
                AnalysisValue::dictionary([(value.clone(), AnalysisValue::none())]).unwrap(),
            ),
            (
                "value",
                AnalysisValue::dictionary([(AnalysisValue::string("key"), value.clone())]).unwrap(),
            ),
            (
                "graph",
                AnalysisValue::depset(
                    AnalysisDepset::new(DepsetOrder::Default, vec![value], vec![]).unwrap(),
                ),
            ),
        ])
    };
    let a = nested(left);
    let b = nested(right);
    assert!(!a.publication_eq(&b));
    assert!(
        a.publication_eq(&nested(AnalysisValue::package_specification_info(info(
            "left"
        ))))
    );
    // Check each container independently so an earlier unequal field cannot
    // mask a missing nested-depset or dictionary publication dispatch.
    let AnalysisValueKind::Struct(a) = a.kind() else {
        unreachable!()
    };
    let AnalysisValueKind::Struct(b) = b.kind() else {
        unreachable!()
    };
    for (name, a) in a {
        assert!(!a.publication_eq(b.get(name).unwrap()), "{name}");
    }
}

#[test]
fn typed_provider_collection_and_hidden_payload_changes_invalidate() {
    let collection = |info| {
        ProviderCollection::from_values(vec![ProviderValue::PackageSpecificationInfo(info)], false)
            .unwrap()
    };
    let a = collection(info("a"));
    assert!(
        a.package_specification_info()
            .unwrap()
            .contains_package(&package("", "a"))
    );
    assert_ne!(a, collection(info("b")));
    assert_eq!(a, collection(info("a")));
    let empty =
        PackageSpecificationInfo::new(Arc::new(PackageGroupContents::default()), []).unwrap();
    let private = PackageSpecificationInfo::new(
        Arc::new(PackageGroupContents::new(PackageGroupContentsInput {
            has_private: true,
            ..Default::default()
        })),
        [],
    )
    .unwrap();
    assert!(!empty.contains_package(&package("", "a")));
    assert!(!private.contains_package(&package("", "a")));
    assert_ne!(collection(empty), collection(private));
}
