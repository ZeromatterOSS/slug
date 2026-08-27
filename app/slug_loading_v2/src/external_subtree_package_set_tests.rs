#![cfg(unix)]

use std::ffi::OsString;
use std::hash::Hash;
use std::hash::Hasher;
use std::os::unix::ffi::OsStringExt;
use std::sync::Arc;

use dice::DetectCycles;
use dice::Dice;
use dice::Key;
use dupe::Dupe;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
use slug_bzlmod_v2::HostExternalPackageBoundaryKind;
use slug_bzlmod_v2::HostRepositoryDirectoryListingKey;
use slug_bzlmod_v2::HostRepositoryLocalPathPolicy;
use slug_bzlmod_v2::HostRepositoryMaterializationDisposition;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::RepoRuleId;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
use slug_bzlmod_v2::RepositoryMaterializationResult;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::RepositoryMaterializationSuccess;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::RootRepositoryRouteKey;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::host_repository_materialization_request;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryListing;
use slug_workspace_v2::PathDirectoryName;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::ExternalSubtreePackageSet;
use super::ExternalSubtreePackageSetErrorKind;
use super::ExternalSubtreePackageSetKey;
use super::ExternalSubtreePackageSetObservationKey;
use super::child_packages;
use super::listing_entries;
use super::merge_observations;

fn path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(value).unwrap()
}

fn package(value: &str) -> PackagePath {
    PackagePath::parse(value).unwrap()
}

fn entry(name: &str, kind: PathDirectoryEntryKind) -> PathDirectoryEntry {
    PathDirectoryEntry::new(PathDirectoryName::new(name).unwrap(), kind)
}

fn generated_route(workspace: &str, bytes: &'static [u8]) -> RootRepositoryRoute {
    let plan =
        GeneratedRepositoryFileEffectPlan::build([("seed".into(), Arc::<[u8]>::from(bytes), true)])
            .unwrap();
    RootRepositoryRoute::for_generated_repo_spec(
        path(workspace),
        ApparentRepoName::new("generated").unwrap(),
        CanonicalRepoName::new("extension+generated").unwrap(),
        RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@extension+repo//:defs.bzl").unwrap(),
                rule_name: "generated_repository".into(),
            },
            attributes: Arc::default(),
        },
        HostRepositoryLocalPathPolicy::LocalUnsupported,
        plan,
    )
    .unwrap()
}

fn local_epoch(child_build: Option<bool>) -> PathObservationEpoch {
    let demand = |value, operation| {
        PathObservationDemand::new(PathObservationNamespace::Host, path(value), operation)
    };
    let lstat = |value, kind| {
        (
            demand(value, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, 1, 1, 1, 1, 0o755,
            ))),
        )
    };
    let listing = |value, entries| {
        (
            demand(value, PathObservationOperation::DirectoryEntries),
            PathObservationResult::DirectoryEntries(PathOperationResult::Present(
                PathDirectoryEntries::new(entries),
            )),
        )
    };
    let mut observations = vec![
        lstat("/", PathNodeKind::Directory),
        lstat("/workspace", PathNodeKind::Directory),
        lstat("/workspace/MODULE.bazel", PathNodeKind::RegularFile),
        (
            demand(
                "/workspace/MODULE.bazel",
                PathObservationOperation::FileBytes,
            ),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                &b"module(name='root')\nbazel_dep(name='dep')\nlocal_path_override(module_name='dep', path='dep')\n"[..],
            ))),
        ),
        lstat("/workspace/dep", PathNodeKind::Directory),
        (
            demand(
                "/workspace/dep/BUILD.bazel",
                PathObservationOperation::Lstat,
            ),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        ),
        (
            demand("/workspace/dep/BUILD", PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        ),
        (
            demand("/workspace/dep/REPO.bazel", PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        ),
        lstat("/workspace/dep/.bazelignore", PathNodeKind::RegularFile),
        (
            demand(
                "/workspace/dep/.bazelignore",
                PathObservationOperation::FileBytes,
            ),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                &b"ignored\n"[..],
            ))),
        ),
        listing(
            "/workspace/dep",
            vec![
                entry(".bazelignore", PathDirectoryEntryKind::File),
                entry("deleted", PathDirectoryEntryKind::Directory),
                entry("ignored", PathDirectoryEntryKind::Directory),
                entry("pkg", PathDirectoryEntryKind::Directory),
            ],
        ),
        lstat("/workspace/dep/deleted", PathNodeKind::Directory),
        listing(
            "/workspace/dep/deleted",
            vec![entry("child", PathDirectoryEntryKind::Directory)],
        ),
        lstat("/workspace/dep/deleted/child", PathNodeKind::Directory),
        lstat("/workspace/dep/pkg", PathNodeKind::Directory),
        lstat(
            "/workspace/dep/pkg/BUILD.bazel",
            PathNodeKind::RegularFile,
        ),
        listing(
            "/workspace/dep/pkg",
            vec![entry("BUILD.bazel", PathDirectoryEntryKind::File)],
        ),
    ];
    observations.push((
        demand(
            "/workspace/dep/deleted/child/BUILD.bazel",
            PathObservationOperation::Lstat,
        ),
        PathObservationResult::Lstat(PathOperationResult::Missing),
    ));
    observations.push(if child_build.unwrap_or(true) {
        lstat(
            "/workspace/dep/deleted/child/BUILD",
            PathNodeKind::RegularFile,
        )
    } else {
        (
            demand(
                "/workspace/dep/deleted/child/BUILD",
                PathObservationOperation::Lstat,
            ),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        )
    });
    if let Some(child_build) = child_build {
        observations.push(listing(
            "/workspace/dep/deleted/child",
            child_build
                .then(|| entry("BUILD", PathDirectoryEntryKind::File))
                .into_iter()
                .collect::<Vec<_>>(),
        ));
    }
    PathObservationEpoch::new(observations).unwrap()
}

async fn local_observed(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
) -> <ExternalSubtreePackageSetObservationKey as Key>::Value {
    let workspace = path("/workspace");
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            workspace.dupe(),
            vec![workspace.dupe()],
            ["@dep+//deleted"],
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        workspace.as_path(),
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let SourcePreparationOutcome::Complete(route) = transaction
        .compute(
            &RootRepositoryRouteKey::new(workspace, ApparentRepoName::new("dep").unwrap()).unwrap(),
        )
        .await
        .unwrap()
    else {
        panic!("local route observations must be complete");
    };
    let route = route.as_ref().as_ref().unwrap().clone();
    let HostRepositoryMaterializationDisposition::Request(request) =
        host_repository_materialization_request(&route.source_capability()).unwrap()
    else {
        panic!("direct-local route requires runtime materialization admission");
    };
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: route.workspace().dupe(),
            },
            RepositoryMaterializationResultEpoch::new(
                route.workspace().dupe(),
                [RepositoryMaterializationEpochEntry {
                    request,
                    result: RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Local,
                    ),
                }],
            )
            .unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    transaction
        .compute(&ExternalSubtreePackageSetObservationKey::new(
            route,
            PackagePath::root(),
        ))
        .await
        .unwrap()
}

#[test]
fn children_are_lexical_and_files_are_not_candidates() {
    let entries = PathDirectoryEntries::new([
        entry("z", PathDirectoryEntryKind::Directory),
        entry("BUILD", PathDirectoryEntryKind::File),
        entry("a", PathDirectoryEntryKind::Directory),
        entry("a", PathDirectoryEntryKind::Directory),
    ]);
    assert_eq!(
        child_packages(&package("pkg"), &entries).unwrap(),
        [package("pkg/a"), package("pkg/z")]
    );
    assert_eq!(
        child_packages(&PackagePath::root(), &entries).unwrap(),
        [package("a"), package("z")]
    );
}

#[test]
fn symlink_unknown_and_non_unicode_entries_fail_closed_and_redacted() {
    for kind in [
        PathDirectoryEntryKind::Symlink,
        PathDirectoryEntryKind::Unknown,
    ] {
        let error = child_packages(
            &package("pkg"),
            &PathDirectoryEntries::new([entry("secret-child", kind)]),
        )
        .unwrap_err();
        assert!(matches!(
            error.kind,
            ExternalSubtreePackageSetErrorKind::UnsupportedEntryKind {
                ref parent,
                kind: actual,
            } if parent == &package("pkg") && actual == kind
        ));
        assert!(!format!("{error:?}").contains("secret-child"));
    }

    let name = PathDirectoryName::new(OsString::from_vec(vec![b'x', 0xff])).unwrap();
    let error = child_packages(
        &package("pkg"),
        &PathDirectoryEntries::new([PathDirectoryEntry::new(
            name,
            PathDirectoryEntryKind::Directory,
        )]),
    )
    .unwrap_err();
    assert!(matches!(
        error.kind,
        ExternalSubtreePackageSetErrorKind::NonUnicodeDirectoryName { ref parent }
            if parent == &package("pkg")
    ));
    assert!(!format!("{error:?}").contains("/workspace"));
}

#[test]
fn key_identity_retains_route_source_and_prefix_a_b_a() {
    fn key_hash(value: &impl Hash) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }
    let key = |bytes, prefix| {
        ExternalSubtreePackageSetKey::new(generated_route("/workspace", bytes), package(prefix))
    };
    let a = key(b"a", "pkg");
    let b = key(b"b", "pkg");
    let restored = key(b"a", "pkg");
    assert_eq!(a, restored);
    assert_ne!(a, b);
    assert_ne!(a, key(b"a", "other"));
    assert_eq!(key_hash(&a), key_hash(&restored));
    assert_ne!(key_hash(&a), key_hash(&b));
}

#[test]
fn epochs_share_results_and_complete_only_keys_reject_need() {
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        path("/workspace/pkg"),
        PathObservationOperation::Lstat,
    );
    let result = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
    let epoch = PathObservationEpoch::from_shared([(demand.dupe(), result.dupe())]).unwrap();
    let merged = merge_observations(&PathObservationEpoch::empty(), &epoch).unwrap();
    assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &result));
    let conflicting = PathObservationEpoch::from_shared([(
        demand.dupe(),
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
            PathLstat::new(PathNodeKind::Directory, 2, 2, 2, 2, 0o755),
        ))),
    )])
    .unwrap();
    assert!(merge_observations(&epoch, &conflicting).is_err());

    let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
        NeedPathObservations::singleton(demand),
    ));
    assert!(!ExternalSubtreePackageSetKey::validity(&need));
    assert!(!ExternalSubtreePackageSetKey::equality(&need, &need));
    let complete = SourcePreparationOutcome::Complete(Arc::new(Ok(ExternalSubtreePackageSet {
        packages: Arc::from([]),
    })));
    assert!(ExternalSubtreePackageSetKey::validity(&complete));
    let observed_need: <ExternalSubtreePackageSetObservationKey as Key>::Value =
        need.map(|_| unreachable!());
    assert!(!ExternalSubtreePackageSetObservationKey::validity(
        &observed_need
    ));
}

#[test]
fn package_with_missing_listing_is_a_typed_inconsistency() {
    let package = package("pkg");
    let error = listing_entries(
        &package,
        HostExternalPackageBoundaryKind::Package,
        PathDirectoryListing::Missing,
    )
    .unwrap_err();
    assert!(matches!(
        error.kind(),
        ExternalSubtreePackageSetErrorKind::MissingPackageDirectory { package: actual }
            if actual == &package
    ));
    assert!(
        listing_entries(
            &package,
            HostExternalPackageBoundaryKind::NoPackage,
            PathDirectoryListing::Missing,
        )
        .unwrap()
        .is_none()
    );
}

#[tokio::test]
async fn observed_local_tree_prunes_ignored_retains_deleted_children_and_restores() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        path("/workspace/dep/deleted/child"),
        PathObservationOperation::DirectoryEntries,
    );
    let epoch_a = local_epoch(Some(true));
    let shared = epoch_a.get(&demand).unwrap().dupe();
    let a = local_observed(&dice, epoch_a.dupe()).await;
    let SourcePreparationOutcome::Complete(Ok(a)) = a else {
        panic!("complete local tree must not request ignored-directory observations: {a:?}");
    };
    assert_eq!(
        a.result()
            .as_ref()
            .as_ref()
            .unwrap()
            .packages()
            .iter()
            .map(|package| package.as_str())
            .collect::<Vec<_>>(),
        ["deleted/child", "pkg"]
    );
    assert!(Arc::ptr_eq(a.observations().get(&demand).unwrap(), &shared));
    assert!(!a.observations().observations().keys().any(|demand| {
        demand
            .path()
            .as_path()
            .starts_with("/workspace/dep/ignored")
    }));

    let b = local_observed(&dice, local_epoch(Some(false))).await;
    let SourcePreparationOutcome::Complete(Ok(b)) = b else {
        panic!("edited local tree must complete");
    };
    assert_eq!(
        b.result()
            .as_ref()
            .as_ref()
            .unwrap()
            .packages()
            .iter()
            .map(|package| package.as_str())
            .collect::<Vec<_>>(),
        ["pkg"]
    );

    let restored = local_observed(&dice, local_epoch(Some(true))).await;
    let SourcePreparationOutcome::Complete(Ok(restored)) = restored else {
        panic!("restored local tree must complete");
    };
    assert_eq!(restored.result(), a.result());
    assert_eq!(restored.observations(), a.observations());

    let SourcePreparationOutcome::Need(need) = local_observed(&dice, local_epoch(None)).await
    else {
        panic!("missing child listing observation must remain transient");
    };
    assert_eq!(
        need.path_observations().unwrap().demands(),
        [PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/workspace/dep/deleted/child"),
            PathObservationOperation::DirectoryEntries,
        )]
    );
}

#[tokio::test]
async fn boundary_failure_precedes_any_listing_need() {
    let key =
        ExternalSubtreePackageSetKey::new(generated_route("/workspace", b"a"), package("pkg"));
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = dice.updater().commit().await;
    let SourcePreparationOutcome::Complete(outcome) = transaction.compute(&key).await.unwrap()
    else {
        panic!("missing policy must fail before materialization Need");
    };
    assert!(matches!(
        outcome.as_ref().as_ref().unwrap_err().kind,
        ExternalSubtreePackageSetErrorKind::Boundary { ref package, .. }
            if package == &PackagePath::parse("pkg").unwrap()
    ));
}

#[tokio::test]
async fn deleted_package_still_consumes_its_listing() {
    let route = generated_route("/workspace", b"a");
    let prefix = package("pkg");
    let key = ExternalSubtreePackageSetKey::new(route.clone(), prefix.clone());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            path("/workspace"),
            Vec::<NormalizedAbsolutePath>::new(),
            ["@extension+generated//pkg"],
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    updater
        .changed_to(vec![(
            HostRepositoryDirectoryListingKey::new(route, prefix),
            SourcePreparationOutcome::Complete(Ok(PathDirectoryListing::Present(
                PathDirectoryEntries::new([]),
            ))),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let SourcePreparationOutcome::Complete(outcome) = transaction.compute(&key).await.unwrap()
    else {
        panic!("injected deleted package must complete");
    };
    assert!(outcome.as_ref().as_ref().unwrap().packages().is_empty());
}

#[tokio::test]
async fn deleted_package_listing_failure_is_typed_and_redacted() {
    let route = generated_route("/workspace", b"secret-source");
    let key = ExternalSubtreePackageSetKey::new(route.clone(), package("pkg"));
    let HostRepositoryMaterializationDisposition::Request(request) =
        host_repository_materialization_request(&route.source_capability()).unwrap()
    else {
        panic!("generated route requires runtime materialization admission");
    };
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            path("/workspace"),
            Vec::<NormalizedAbsolutePath>::new(),
            ["@extension+generated//pkg"],
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: route.workspace().dupe(),
            },
            RepositoryMaterializationResultEpoch::new(
                route.workspace().dupe(),
                [RepositoryMaterializationEpochEntry {
                    request,
                    result: RepositoryMaterializationResult::SpecError(
                        "private materialization failure".into(),
                    ),
                }],
            )
            .unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let SourcePreparationOutcome::Complete(outcome) = transaction.compute(&key).await.unwrap()
    else {
        panic!("listing infrastructure failure is semantic");
    };
    let error = outcome.as_ref().as_ref().unwrap_err();
    assert!(matches!(
        error.kind(),
        ExternalSubtreePackageSetErrorKind::Listing { package, .. }
            if package == &PackagePath::parse("pkg").unwrap()
    ));
    let debug = format!("{error:?}");
    assert!(!debug.contains("/workspace"));
    assert!(!debug.contains("secret-source"));
}

#[tokio::test]
async fn real_builtin_catalog_discovers_root_and_prefixed_package_sets() {
    let workspace = path("/workspace");
    let demand = |value, operation| {
        PathObservationDemand::new(PathObservationNamespace::Host, path(value), operation)
    };
    let lstat = |value, kind| {
        (
            demand(value, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, 1, 1, 1, 1, 0o755,
            ))),
        )
    };
    let epoch = PathObservationEpoch::new([
        lstat("/", PathNodeKind::Directory),
        lstat("/workspace", PathNodeKind::Directory),
        lstat("/workspace/MODULE.bazel", PathNodeKind::RegularFile),
        (
            demand(
                "/workspace/MODULE.bazel",
                PathObservationOperation::FileBytes,
            ),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                &b"module(name='root')\n"[..],
            ))),
        ),
    ])
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            workspace.dupe(),
            vec![workspace.dupe()],
            Vec::<&str>::new(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        workspace.as_path(),
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let SourcePreparationOutcome::Complete(route) = transaction
        .compute(
            &RootRepositoryRouteKey::new(workspace, ApparentRepoName::new("bazel_tools").unwrap())
                .unwrap(),
        )
        .await
        .unwrap()
    else {
        panic!("root module observations are complete");
    };
    let route = route.as_ref().as_ref().unwrap().clone();

    for (prefix, expected) in [
        ("", vec!["src/conditions", "tools/test"]),
        ("tools", vec!["tools/test"]),
    ] {
        let prefix = package(prefix);
        let SourcePreparationOutcome::Complete(outcome) = transaction
            .compute(&ExternalSubtreePackageSetKey::new(
                route.clone(),
                prefix.clone(),
            ))
            .await
            .unwrap()
        else {
            panic!("immutable built-in traversal must complete");
        };
        assert_eq!(
            outcome
                .as_ref()
                .as_ref()
                .unwrap()
                .packages()
                .iter()
                .map(|package| package.as_str())
                .collect::<Vec<_>>(),
            expected
        );

        let SourcePreparationOutcome::Complete(Ok(observed)) = transaction
            .compute(&ExternalSubtreePackageSetObservationKey::new(
                route.clone(),
                prefix,
            ))
            .await
            .unwrap()
        else {
            panic!("observed immutable built-in traversal must complete");
        };
        assert_eq!(observed.result(), &outcome);
        assert!(observed.observations().observations().is_empty());
    }

    let SourcePreparationOutcome::Complete(wrong_kind) = transaction
        .compute(&ExternalSubtreePackageSetKey::new(
            route,
            package("MODULE.bazel"),
        ))
        .await
        .unwrap()
    else {
        panic!("built-in file-as-prefix failure must complete");
    };
    let error = wrong_kind.as_ref().as_ref().unwrap_err();
    assert!(matches!(
        error.kind(),
        ExternalSubtreePackageSetErrorKind::Boundary { package, .. }
            if package == &PackagePath::parse("MODULE.bazel").unwrap()
    ));
    assert!(!format!("{error:?}").contains("/workspace"));
}

#[test]
fn ignored_branch_precedes_listing_and_no_fallback_owner_exists() {
    let source = include_str!("external_subtree_package_set.rs");
    let driver = &source[source
        .find("async fn compute_external_subtree_packages")
        .unwrap()..];
    assert!(driver.find("IgnoredDirectory").unwrap() < driver.find("listing(ctx").unwrap());
    for forbidden in [
        "PathDirectoryListingKey",
        "PathDirectoryListingObservationKey",
        "RepositoryMaterializationResultKey",
        "HostRepositoryPathKey",
        "HostRepositorySourceFileKey",
        "CanonicalDeletedPackagesProjectionKey",
    ] {
        assert!(!source.contains(forbidden), "forbidden edge: {forbidden}");
    }
}
