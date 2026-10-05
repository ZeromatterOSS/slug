use slug_bzlmod_v2::RootRepositoryRouteKey;
use slug_identity_v2::ApparentRepoName;
use slug_workspace_v2::PathObservationEpochKey;

use super::*;
use crate::canonical_repository_route_tests::tests::EXTENSION_A;
use crate::canonical_repository_route_tests::tests::WORKSPACE;
use crate::canonical_repository_route_tests::tests::builtin_graph_dice;
use crate::canonical_repository_route_tests::tests::builtin_graph_module;
use crate::canonical_repository_route_tests::tests::transaction;

async fn admitted_keys() -> (
    RepositoryPackageInventoryKey,
    RepositoryPackageInventoryKey,
    PathObservationEpoch,
) {
    let dice = builtin_graph_dice();
    let module = builtin_graph_module();
    let mut tx = transaction(&dice, &module, EXTENSION_A, true, None).await;
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let SourcePreparationOutcome::Complete(root) = tx
        .compute(
            &RootRepositoryRouteKey::for_root_build(
                workspace.clone(),
                ApparentRepoName::new("bazel_tools").unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap()
    else {
        panic!("builtin root route must complete");
    };
    let root = root.as_ref().as_ref().unwrap().clone();
    let SourcePreparationOutcome::Complete(canonical) = tx
        .compute(&HostCanonicalRepositoryLoadRouteKey::new(
            workspace,
            CanonicalRepoName::new("rules_cc+").unwrap(),
        ))
        .await
        .unwrap()
    else {
        panic!("canonical route must complete");
    };
    let input = canonical.as_ref().as_ref().unwrap().input().clone();
    let observations = tx.compute(&PathObservationEpochKey).await.unwrap();
    assert!(!observations.observations().is_empty());
    let package = PackagePath::parse("pkg").unwrap();
    (
        RepositoryPackageInventoryKey::new(HostRepositorySourceRoute::root(root), package.clone()),
        RepositoryPackageInventoryKey::new(HostRepositorySourceRoute::canonical(input), package),
        observations.dupe(),
    )
}

fn prepare(
    key: &RepositoryPackageInventoryKey,
    loads: Vec<String>,
    observations: &PathObservationEpoch,
) -> Result<Vec<RepositoryPackageChildLoad>, RepositoryPackageInventoryDriverOutcome> {
    let repo = CompactString::new(key.route.canonical_repo().as_str());
    prepare_repository_package_child_loads(key, loads, &repo, observations)
}

fn raw_name(child: &RepositoryPackageChildLoad) -> &String {
    match child {
        RepositoryPackageChildLoad::Canonical(name)
        | RepositoryPackageChildLoad::Resolved(name, _) => name,
    }
}

fn assert_transferred(
    key: &RepositoryPackageInventoryKey,
    names: &[&str],
    observations: &PathObservationEpoch,
) {
    let loads: Vec<_> = names
        .iter()
        .map(|name| {
            let mut name = name.to_string();
            name.reserve(512);
            name
        })
        .collect();
    let allocations: Vec<_> = loads
        .iter()
        .map(|name| (name.as_ptr(), name.capacity()))
        .collect();
    let children = prepare(key, loads, observations).unwrap();
    assert_eq!(children.len(), names.len());
    for ((child, expected), (pointer, capacity)) in children.iter().zip(names).zip(allocations) {
        let name = raw_name(child);
        assert_eq!(name, expected);
        assert_eq!(name.as_ptr(), pointer);
        assert_eq!(name.capacity(), capacity);
        match child {
            RepositoryPackageChildLoad::Resolved(_, resolved) => {
                assert_eq!(resolved.route, key.route);
                assert_eq!(
                    resolved.label,
                    resolve_external_load_label(&key.package, expected).unwrap()
                );
            }
            RepositoryPackageChildLoad::Canonical(_) => {
                assert!(matches!(key.route, HostRepositorySourceRoute::Canonical(_)));
            }
        }
    }
}

#[tokio::test]
async fn root_prepared_loads_transfer_original_string_buffers() {
    let (root, _, observations) = admitted_keys().await;
    assert_transferred(
        &root,
        &[":first.bzl", "//pkg:second.bzl", ":first.bzl"],
        &observations,
    );
}

#[tokio::test]
async fn canonical_prepared_loads_transfer_original_string_buffers() {
    let (_, canonical, observations) = admitted_keys().await;
    assert_transferred(
        &canonical,
        &["@alias//:é.bzl", "//other:defs.bzl", "@alias//:é.bzl"],
        &observations,
    );
}

#[tokio::test]
async fn prepared_loads_preserve_order_variants_and_first_error_observations() {
    let (root, canonical, observations) = admitted_keys().await;
    for (key, names) in [
        (&root, vec![":first.bzl", "//pkg:second.bzl", ":first.bzl"]),
        (&canonical, vec!["", "\0世界", "@other//:defs.bzl", ""]),
    ] {
        assert!(prepare(key, Vec::new(), &observations).unwrap().is_empty());
        let children = prepare(
            key,
            names.iter().map(|x| x.to_string()).collect(),
            &observations,
        )
        .unwrap();
        assert_eq!(children.len(), names.len());
        for (child, expected) in children.iter().zip(names) {
            assert_eq!(raw_name(child), expected);
            assert_eq!(
                matches!(child, RepositoryPackageChildLoad::Resolved(..)),
                matches!(key.route, HostRepositorySourceRoute::Root(_)),
            );
        }
    }
    let failure = prepare(
        &root,
        [
            ":visited.bzl",
            "@other//:wrong.bzl",
            "//elsewhere:later.bzl",
        ]
        .map(String::from)
        .into(),
        &observations,
    );
    let Err(SourcePreparationOutcome::Complete(Ok((result, returned)))) = failure else {
        panic!("invalid root load must retain typed error and observations");
    };
    let Err(error) = result.as_ref() else {
        panic!("expected load label failure");
    };
    let RepositoryPackageLoadErrorInner::LoadLabel {
        canonical_repo,
        package,
        error,
    } = &error.inner
    else {
        panic!("expected load label error");
    };
    assert_eq!(
        canonical_repo.as_str(),
        root.route.canonical_repo().as_str()
    );
    assert_eq!(package, &root.package);
    assert_eq!(
        error,
        &ExternalLoadLabelError::Repository {
            load: Arc::from("@other//:wrong.bzl"),
        },
    );
    assert_eq!(returned, observations);
}
