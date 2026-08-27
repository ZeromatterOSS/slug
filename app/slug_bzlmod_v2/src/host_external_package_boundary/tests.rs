#![cfg(unix)]

use std::error::Error;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use dice::DetectCycles;
use dice::Dice;
use dice::Key;
use dupe::Dupe;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochError;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::HostExternalPackageBoundary;
use super::HostExternalPackageBoundaryError;
use super::HostExternalPackageBoundaryKey;
use super::HostExternalPackageBoundaryKind;
use super::HostExternalPackageBoundaryObservationKey;
use crate::RootPackagePolicyInputs;
use crate::RootRepositoryRoute;
use crate::SourcePreparationNeeds;
use crate::SourcePreparationOutcome;
use crate::host_package::ExternalRepositoryPackageLookup;
use crate::host_package::ExternalRepositoryPackageLookupKey;
use crate::host_package::ExternalRepositoryPackageLookupObservationKey;
use crate::host_package::HostBuildFileName;
use crate::host_package::ObservedExternalRepositoryPackageLookup;
use crate::inject_root_package_policy_inputs;

fn path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(value).unwrap()
}

fn package(value: &str) -> PackagePath {
    PackagePath::parse(value).unwrap()
}

fn route(workspace: &str) -> RootRepositoryRoute {
    RootRepositoryRoute::builtin_for_test(path(workspace))
}

fn private_key(
    route: &RootRepositoryRoute,
    package: &PackagePath,
) -> ExternalRepositoryPackageLookupKey {
    ExternalRepositoryPackageLookupKey::new(
        route.clone(),
        PackageIdentifier::new(route.canonical_repo().clone(), package.clone()),
    )
    .unwrap()
}

fn private_observed_key(
    route: &RootRepositoryRoute,
    package: &PackagePath,
) -> ExternalRepositoryPackageLookupObservationKey {
    ExternalRepositoryPackageLookupObservationKey::new(
        route.clone(),
        PackageIdentifier::new(route.canonical_repo().clone(), package.clone()),
    )
    .unwrap()
}

fn complete(
    value: &SourcePreparationOutcome<
        Arc<Result<HostExternalPackageBoundary, HostExternalPackageBoundaryError>>,
    >,
) -> &HostExternalPackageBoundary {
    let SourcePreparationOutcome::Complete(value) = value else {
        panic!("expected complete boundary");
    };
    value.as_ref().as_ref().unwrap()
}

async fn project(terminal: ExternalRepositoryPackageLookup) -> HostExternalPackageBoundary {
    let route = route("/workspace");
    let package = package("tools/test");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            private_key(&route, &package),
            SourcePreparationOutcome::Complete(Arc::new(Ok(terminal))),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    complete(
        &transaction
            .compute(&HostExternalPackageBoundaryKey::new(route, package))
            .await
            .unwrap(),
    )
    .clone()
}

async fn real_builtin_boundary(
    package_name: &str,
    deleted: &[&str],
) -> SourcePreparationOutcome<
    Arc<Result<HostExternalPackageBoundary, HostExternalPackageBoundaryError>>,
> {
    let workspace = path("/workspace");
    let route = RootRepositoryRoute::builtin_for_test(workspace.dupe());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            workspace,
            Vec::<NormalizedAbsolutePath>::new(),
            deleted,
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    transaction
        .compute(&HostExternalPackageBoundaryKey::new(
            route,
            package(package_name),
        ))
        .await
        .unwrap()
}

#[tokio::test]
async fn real_builtin_route_reaches_private_policy_ignore_listing_and_marker_owner() {
    let package_value = real_builtin_boundary("tools/test", &[]).await;
    assert_eq!(
        complete(&package_value).kind(),
        HostExternalPackageBoundaryKind::Package
    );
    assert_eq!(
        complete(&package_value).selected_build_file_name(),
        Some("BUILD")
    );

    let root = real_builtin_boundary("", &[]).await;
    assert_eq!(
        complete(&root).kind(),
        HostExternalPackageBoundaryKind::NoPackage
    );
    let deleted = real_builtin_boundary("tools/test", &["@bazel_tools//tools/test"]).await;
    assert_eq!(
        complete(&deleted).kind(),
        HostExternalPackageBoundaryKind::DeletedPackage
    );

    let wrong_kind = real_builtin_boundary("MODULE.bazel", &[]).await;
    let SourcePreparationOutcome::Complete(wrong_kind) = wrong_kind else {
        panic!("wrong-kind listing must complete");
    };
    assert_eq!(
        wrong_kind.as_ref(),
        &Err(HostExternalPackageBoundaryError::RepositoryListing)
    );
}

#[tokio::test]
async fn real_private_policy_failure_projects_to_a_payload_free_tag() {
    let route = route("/workspace");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = dice.updater().commit().await;
    let outcome = transaction
        .compute(&HostExternalPackageBoundaryKey::new(
            route,
            package("tools/test"),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(outcome) = outcome else {
        panic!("policy projection failure must complete");
    };
    assert_eq!(
        outcome.as_ref(),
        &Err(HostExternalPackageBoundaryError::PolicyInput)
    );
}

#[tokio::test]
async fn projects_all_five_terminals_and_both_marker_spellings() {
    for (private, kind, marker) in [
        (
            ExternalRepositoryPackageLookup::InvalidPackageName {
                message: Arc::from("private diagnostic"),
            },
            HostExternalPackageBoundaryKind::InvalidPackageName,
            None,
        ),
        (
            ExternalRepositoryPackageLookup::Deleted,
            HostExternalPackageBoundaryKind::DeletedPackage,
            None,
        ),
        (
            ExternalRepositoryPackageLookup::IgnoredDirectory,
            HostExternalPackageBoundaryKind::IgnoredDirectory,
            None,
        ),
        (
            ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel),
            HostExternalPackageBoundaryKind::Package,
            Some("BUILD.bazel"),
        ),
        (
            ExternalRepositoryPackageLookup::Package(HostBuildFileName::Build),
            HostExternalPackageBoundaryKind::Package,
            Some("BUILD"),
        ),
        (
            ExternalRepositoryPackageLookup::NoBuildFile,
            HostExternalPackageBoundaryKind::NoPackage,
            None,
        ),
    ] {
        let boundary = project(private).await;
        assert_eq!(boundary.kind(), kind);
        assert_eq!(boundary.selected_build_file_name(), marker);
        assert!(!format!("{boundary:?}").contains("private diagnostic"));
    }
}

#[test]
fn route_and_package_identity_restore_a_b_a() {
    fn key_hash(value: &impl Hash) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }
    let package = package("tools/test");
    let a = HostExternalPackageBoundaryKey::new(route("/workspace-a"), package.clone());
    let b = HostExternalPackageBoundaryKey::new(route("/workspace-b"), package.clone());
    let restored = HostExternalPackageBoundaryKey::new(route("/workspace-a"), package);
    assert_eq!(a, restored);
    assert_ne!(a, b);
    assert_eq!(key_hash(&a), key_hash(&restored));
    assert_ne!(key_hash(&a), key_hash(&b));
}

#[tokio::test]
async fn legacy_projection_restores_private_marker_a_b_a() {
    let route = route("/workspace");
    let package = package("tools/test");
    let private = private_key(&route, &package);
    let public = HostExternalPackageBoundaryKey::new(route, package);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = dice.updater().commit().await;

    for (marker, expected) in [
        (HostBuildFileName::BuildDotBazel, "BUILD.bazel"),
        (HostBuildFileName::Build, "BUILD"),
        (HostBuildFileName::BuildDotBazel, "BUILD.bazel"),
    ] {
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                private.clone(),
                SourcePreparationOutcome::Complete(Arc::new(Ok(
                    ExternalRepositoryPackageLookup::Package(marker),
                ))),
            )])
            .unwrap();
        transaction = updater.commit().await;
        let value = transaction.compute(&public).await.unwrap();
        assert_eq!(complete(&value).selected_build_file_name(), Some(expected));
    }
}

fn observation() -> (
    PathObservationDemand,
    Arc<PathObservationResult>,
    PathObservationEpoch,
) {
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        path("/workspace/tools/test/BUILD"),
        PathObservationOperation::Lstat,
    );
    let result = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
    let epoch = PathObservationEpoch::from_shared([(demand.dupe(), result.dupe())]).unwrap();
    (demand, result, epoch)
}

#[tokio::test]
async fn observed_projection_forwards_exact_epoch_and_terminal() {
    let route = route("/workspace");
    let package = package("tools/test");
    let (demand, result, epoch) = observation();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            private_observed_key(&route, &package),
            SourcePreparationOutcome::Complete(Ok(
                ObservedExternalRepositoryPackageLookup::for_test(
                    Ok(ExternalRepositoryPackageLookup::IgnoredDirectory),
                    epoch,
                ),
            )),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let value = transaction
        .compute(&HostExternalPackageBoundaryObservationKey::new(
            route, package,
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("expected observed completion");
    };
    assert_eq!(
        observed.result().as_ref().unwrap().kind(),
        HostExternalPackageBoundaryKind::IgnoredDirectory
    );
    assert!(Arc::ptr_eq(
        observed.observations().get(&demand).unwrap(),
        &result
    ));
}

#[tokio::test]
async fn observed_outer_error_and_need_are_forwarded_before_projection() {
    let route = route("/workspace");
    let package = package("tools/test");
    let observed_key = private_observed_key(&route, &package);
    let public = HostExternalPackageBoundaryObservationKey::new(route, package);
    let (_, _, epoch) = observation();
    let demand = epoch.observations().keys().next().unwrap().dupe();
    let need = SourcePreparationNeeds::path(NeedPathObservations::singleton(demand.dupe()));
    let outer = ObservedPathFrontierError::Epoch(PathObservationEpochError::OperationMismatch {
        demand,
        result_operation: PathObservationOperation::FileBytes,
    });
    let pending: <HostExternalPackageBoundaryObservationKey as Key>::Value =
        SourcePreparationOutcome::Need(need.dupe());
    assert!(matches!(&pending, SourcePreparationOutcome::Need(actual) if actual == &need));
    assert!(!HostExternalPackageBoundaryObservationKey::validity(
        &pending
    ));

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            observed_key,
            SourcePreparationOutcome::Complete(Err(outer.dupe())),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let failed = transaction.compute(&public).await.unwrap();
    assert!(matches!(&failed, SourcePreparationOutcome::Complete(Err(actual)) if actual == &outer));
    assert!(HostExternalPackageBoundaryObservationKey::validity(&failed));
}

#[test]
fn public_errors_are_payload_free_and_redacted() {
    for error in [
        HostExternalPackageBoundaryError::PolicyInput,
        HostExternalPackageBoundaryError::RepositoryIgnore,
        HostExternalPackageBoundaryError::RepositoryListing,
        HostExternalPackageBoundaryError::SourcePath,
    ] {
        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(!debug.contains("/workspace"));
        assert!(!display.contains("/workspace"));
        assert!(error.source().is_none());
    }
}

#[test]
fn projection_has_only_the_private_lookup_dependency() {
    let source = include_str!("mod.rs");
    assert!(source.contains("ctx.compute(&key.lookup_key())"));
    assert!(source.contains("ctx.compute(&key.lookup_observation_key())"));
    for forbidden in [
        "CanonicalDeletedPackagesProjectionKey",
        "HostRouteRepositoryIgnoreKey",
        "HostRepositoryDirectoryListingKey",
        "HostRepositoryPathKey",
        "HostRepositorySourceFileKey",
        "RepositoryMaterializationResultKey",
    ] {
        assert!(!source.contains(forbidden), "forbidden edge: {forbidden}");
    }
}
