/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::Mutex;

use dice::ActivationData;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DynKey;
use dice::Key;
use slug_bzlmod_v2::HostCanonicalRepositoryRouteKind;
use slug_bzlmod_v2::HostRepositoryLocalPathPolicy;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::HostSelectedObservationFrontier;
use slug_bzlmod_v2::RootRepositoryRouteKey;
use slug_bzlmod_v2::RootRepositoryRouteObservationError;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::*;
use crate::canonical_repository_load_route_tests::tests::LEAF_MAPPING_A;
use crate::canonical_repository_load_route_tests::tests::PARENT_MODULE;
use crate::canonical_repository_load_route_tests::tests::ROOT_MODULE;
use crate::canonical_repository_load_route_tests::tests::SOURCE_A;
use crate::canonical_repository_load_route_tests::tests::registry_dice;
use crate::canonical_repository_route_tests::tests::EXTENSION_A;
use crate::canonical_repository_route_tests::tests::MODULE;
use crate::canonical_repository_route_tests::tests::WORKSPACE;
use crate::canonical_repository_route_tests::tests::transaction;

#[derive(Default)]
struct RouteTrace(Mutex<Vec<(String, Vec<String>)>>);

impl ActivationTracker for RouteTrace {
    fn key_activated(
        &self,
        key: &DynKey,
        dependencies: &mut dyn Iterator<Item = &DynKey>,
        _: ActivationData,
    ) {
        self.0.lock().unwrap().push((
            key.to_string(),
            dependencies.map(ToString::to_string).collect(),
        ));
    }
}

impl RouteTrace {
    fn dependencies(&self, key: &str) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|(candidate, dependencies)| (candidate == key).then(|| dependencies.clone()))
            .unwrap_or_default()
    }

    fn contains(&self, key: &str) -> bool {
        self.0
            .lock()
            .unwrap()
            .iter()
            .any(|(candidate, _)| candidate == key)
    }
}

fn observed_route(
    value: &<HostRootRepositoryLoadRouteObservationKey as Key>::Value,
) -> &ObservedHostRootRepositoryLoadRoute {
    let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("observed root load route must complete: {value:?}")
    };
    observed
}

fn hash<T: Hash>(value: &T) -> u64 {
    let mut state = DefaultHasher::new();
    value.hash(&mut state);
    state.finish()
}

#[test]
fn root_repository_load_route_keys_preserve_admission_identity_and_reject_root() {
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let apparent = ApparentRepoName::new("generated").unwrap();
    let ordinary =
        HostRootRepositoryLoadRouteKey::new(workspace.clone(), apparent.clone()).unwrap();
    let same = HostRootRepositoryLoadRouteKey::new(workspace.clone(), apparent.clone()).unwrap();
    let root_build =
        HostRootRepositoryLoadRouteKey::for_root_build(workspace.clone(), apparent.clone())
            .unwrap();
    let observed =
        HostRootRepositoryLoadRouteObservationKey::new(workspace.clone(), apparent.clone())
            .unwrap();
    let observed_root_build =
        HostRootRepositoryLoadRouteObservationKey::for_root_build(workspace.clone(), apparent)
            .unwrap();

    assert_eq!(ordinary, same);
    assert_eq!(hash(&ordinary), hash(&same));
    assert_ne!(ordinary, root_build);
    assert_ne!(hash(&ordinary), hash(&root_build));
    assert_eq!(
        ordinary.to_string(),
        "root-repository-load-route:\"/workspace\":@generated"
    );
    assert_eq!(
        root_build.to_string(),
        "root-build-repository-load-route:\"/workspace\":@generated"
    );
    assert_eq!(observed.to_string(), format!("observed-{ordinary}"));
    assert_eq!(
        observed_root_build.to_string(),
        format!("observed-{root_build}")
    );
    assert!(
        HostRootRepositoryLoadRouteKey::new(workspace.clone(), ApparentRepoName::root(),).is_err()
    );
    assert!(
        HostRootRepositoryLoadRouteObservationKey::for_root_build(
            workspace,
            ApparentRepoName::root(),
        )
        .is_err()
    );
    let _: fn(&<HostRootRepositoryLoadRouteKey as Key>::Value) -> bool =
        HostRootRepositoryLoadRouteKey::validity;
}

#[tokio::test]
async fn shared_route_publishes_generated_canonical_input_and_observed_epoch() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let apparent = ApparentRepoName::new("first").unwrap();
    let key = HostRootRepositoryLoadRouteKey::new(workspace.clone(), apparent.clone()).unwrap();
    let observed_key =
        HostRootRepositoryLoadRouteObservationKey::new(workspace.clone(), apparent.clone())
            .unwrap();
    let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;

    let SourcePreparationOutcome::Complete(legacy) = tx.compute(&key).await.unwrap() else {
        panic!("generated shared route must complete")
    };
    let route = legacy.as_ref().as_ref().unwrap();
    assert_eq!(route.apparent_repo(), &apparent);
    let HostRepositorySourceRoute::Canonical(input) = route.source() else {
        panic!("generated route fabricated a root apparent route")
    };
    assert_eq!(
        input.view().route().view().kind(),
        HostCanonicalRepositoryRouteKind::Generated
    );
    assert_eq!(
        route.canonical_repo(),
        input.view().route().view().canonical_repo()
    );

    let SourcePreparationOutcome::Complete(Ok(observed)) = tx.compute(&observed_key).await.unwrap()
    else {
        panic!("observed generated shared route must complete")
    };
    assert_eq!(observed.result(), &legacy);
    assert!(!observed.observations().observations().is_empty());
}

#[tokio::test]
async fn shared_route_preserves_root_success_and_missing_diagnostic() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;

    let builtin = ApparentRepoName::new("bazel_tools").unwrap();
    let SourcePreparationOutcome::Complete(builtin_route) = tx
        .compute(&HostRootRepositoryLoadRouteKey::new(workspace.clone(), builtin.clone()).unwrap())
        .await
        .unwrap()
    else {
        panic!("built-in route must complete")
    };
    assert!(matches!(
        builtin_route.as_ref(),
        Ok(route) if matches!(route.source(), HostRepositorySourceRoute::Root(root) if root.is_builtin_bazel_tools())
    ));

    let missing = ApparentRepoName::new("missing").unwrap();
    let SourcePreparationOutcome::Complete(original) = tx
        .compute(&RootRepositoryRouteKey::new(workspace.clone(), missing.clone()).unwrap())
        .await
        .unwrap()
    else {
        panic!("original missing route must complete")
    };
    let SourcePreparationOutcome::Complete(shared) = tx
        .compute(&HostRootRepositoryLoadRouteKey::new(workspace, missing).unwrap())
        .await
        .unwrap()
    else {
        panic!("shared missing route must complete")
    };
    assert_eq!(
        shared.as_ref().as_ref().unwrap_err().to_string(),
        original.as_ref().as_ref().unwrap_err().to_string()
    );
}

#[tokio::test]
async fn root_build_and_existing_root_routes_do_not_activate_canonical_fallback() {
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let local_module = "module(name='bazel_tools')\nbazel_dep(name='local', version='1')\nlocal_path_override(module_name='local', path='local')\n";
    let local_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let local_trace = Arc::new(RouteTrace::default());
    let mut local_tx = transaction(
        &local_dice,
        local_module,
        EXTENSION_A,
        true,
        Some(local_trace.clone() as Arc<dyn ActivationTracker>),
    )
    .await;
    let local_key = HostRootRepositoryLoadRouteKey::new(
        workspace.clone(),
        ApparentRepoName::new("local").unwrap(),
    )
    .unwrap();
    let SourcePreparationOutcome::Complete(local) = local_tx.compute(&local_key).await.unwrap()
    else {
        panic!("direct-local route must complete")
    };
    assert!(matches!(
        local.as_ref(),
        Ok(route) if matches!(
            route.source(),
            HostRepositorySourceRoute::Root(root)
                if root.source_capability().local_path_policy()
                    == Some(HostRepositoryLocalPathPolicy::WorkspaceRelative)
        )
    ));
    let dependencies = local_trace.dependencies(&local_key.to_string());
    assert_eq!(dependencies.len(), 1);
    assert!(dependencies[0].starts_with("root-repository-route:"));
    assert!(dependencies.iter().all(|dependency| {
        !dependency.starts_with("host-canonical-repository-apparent-mapping:")
            && !dependency.starts_with("host-canonical-repository-load-route:")
    }));

    let selected_dice = registry_dice(PARENT_MODULE, LEAF_MAPPING_A, SOURCE_A);
    let selected_trace = Arc::new(RouteTrace::default());
    let mut selected_tx = transaction(
        &selected_dice,
        ROOT_MODULE,
        EXTENSION_A,
        true,
        Some(selected_trace.clone() as Arc<dyn ActivationTracker>),
    )
    .await;
    let apparent = ApparentRepoName::new("parent_alias").unwrap();
    let admitted_key =
        HostRootRepositoryLoadRouteKey::for_root_build(workspace.clone(), apparent.clone())
            .unwrap();
    let SourcePreparationOutcome::Complete(admitted) =
        selected_tx.compute(&admitted_key).await.unwrap()
    else {
        panic!("admitted selected route must complete")
    };
    assert!(matches!(
        admitted.as_ref(),
        Ok(route) if route.canonical_repo().as_str() == "parent+" && matches!(route.source(), HostRepositorySourceRoute::Root(_))
    ));
    let dependencies = selected_trace.dependencies(&admitted_key.to_string());
    assert_eq!(dependencies.len(), 1);
    assert!(dependencies[0].starts_with("root-build-repository-route:"));
    assert!(dependencies.iter().all(|dependency| {
        !dependency.starts_with("host-canonical-repository-apparent-mapping:")
            && !dependency.starts_with("host-canonical-repository-load-route:")
    }));

    let ordinary_root_key =
        RootRepositoryRouteKey::new(workspace.clone(), apparent.clone()).unwrap();
    let SourcePreparationOutcome::Complete(original) =
        selected_tx.compute(&ordinary_root_key).await.unwrap()
    else {
        panic!("ordinary selected route must complete with its existing error")
    };
    let original = original.as_ref().as_ref().unwrap_err();
    let ordinary_key = HostRootRepositoryLoadRouteKey::new(workspace, apparent).unwrap();
    let SourcePreparationOutcome::Complete(neutral) =
        selected_tx.compute(&ordinary_key).await.unwrap()
    else {
        panic!("mapped non-generated route must complete neutrally")
    };
    assert!(matches!(
        neutral.as_ref().as_ref().unwrap_err(),
        HostRootRepositoryLoadRouteError { kind: HostRootRepositoryLoadRouteErrorKind::Root(error), .. }
            if error == original
    ));
    let dependencies = selected_trace.dependencies(&ordinary_key.to_string());
    assert_eq!(dependencies.len(), 3);
    assert!(dependencies[0].starts_with("root-repository-route:"));
    assert!(dependencies[1].starts_with("host-canonical-repository-apparent-mapping:"));
    assert!(dependencies[2].starts_with("host-canonical-repository-load-route:"));
}

#[tokio::test]
async fn semantic_child_failures_and_observed_frontiers_remain_typed() {
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let valid_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let missing = ApparentRepoName::new("missing").unwrap();
    let mut valid_tx = transaction(&valid_dice, MODULE, EXTENSION_A, true, None).await;
    let SourcePreparationOutcome::Complete(original) = valid_tx
        .compute(&RootRepositoryRouteKey::new(workspace.clone(), missing).unwrap())
        .await
        .unwrap()
    else {
        panic!("original missing route must complete")
    };
    let original = original.as_ref().as_ref().unwrap_err().clone();

    let bad_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut bad_tx = transaction(
        &bad_dice,
        "this is not valid Starlark\n",
        EXTENSION_A,
        true,
        None,
    )
    .await;
    let key = HostRootRepositoryLoadRouteKey::new(
        workspace.clone(),
        ApparentRepoName::new("first").unwrap(),
    )
    .unwrap();
    let mapped = mapping_predecessor(
        &mut bad_tx,
        &key,
        FallbackState {
            original,
            observations: PathObservationEpoch::empty(),
        },
        LoadRouteMode::Legacy,
    )
    .await;
    let Ok(ControlFlow::Break(SourcePreparationOutcome::Complete(Ok((mapped, _))))) = mapped else {
        panic!("mapping semantic failure must be a completed adapter terminal")
    };
    assert!(matches!(
        mapped.as_ref().as_ref().unwrap_err().kind,
        HostRootRepositoryLoadRouteErrorKind::Mapping(_)
    ));

    let missing_extension_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut missing_extension_tx =
        transaction(&missing_extension_dice, MODULE, EXTENSION_A, false, None).await;
    let SourcePreparationOutcome::Complete(load_failure) =
        missing_extension_tx.compute(&key).await.unwrap()
    else {
        panic!("canonical load failure must complete")
    };
    assert!(matches!(
        load_failure.as_ref().as_ref().unwrap_err().kind,
        HostRootRepositoryLoadRouteErrorKind::LoadRoute(_)
    ));

    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/workspace/MODULE.bazel").unwrap(),
        PathObservationOperation::Lstat,
    );
    let left = PathObservationEpoch::from_shared([(
        demand.clone(),
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing)),
    )])
    .unwrap();
    let right = PathObservationEpoch::from_shared([(
        demand,
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
            PathLstat::new(PathNodeKind::RegularFile, 1, 1, 1, 1, 0o644),
        ))),
    )])
    .unwrap();
    let conflict = merge_observations(&left, &right).unwrap_err();
    assert!(matches!(
        conflict,
        HostRootRepositoryLoadRouteObservationError::Merge(_)
    ));
    assert!(matches!(
        conflict.selected_frontier(),
        HostSelectedObservationFrontier::Path(_)
    ));
    let infrastructure = HostRootRepositoryLoadRouteObservationError::Root(
        RootRepositoryRouteObservationError::Infrastructure("selected graph compute".into()),
    );
    assert!(matches!(
        infrastructure.selected_frontier(),
        HostSelectedObservationFrontier::Infrastructure(message)
            if message.as_ref() == "selected graph compute"
    ));
}

#[tokio::test]
async fn observed_route_need_warm_cancellation_and_generated_missing_generated_are_exact() {
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let key = HostRootRepositoryLoadRouteObservationKey::new(
        workspace,
        ApparentRepoName::new("first").unwrap(),
    )
    .unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let trace = Arc::new(RouteTrace::default());
    let mut cancelled = transaction(
        &dice,
        MODULE,
        EXTENSION_A,
        true,
        Some(trace.clone() as Arc<dyn ActivationTracker>),
    )
    .await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(!trace.contains(&key.to_string()));

    let mut recovered = transaction(
        &dice,
        MODULE,
        EXTENSION_A,
        true,
        Some(trace.clone() as Arc<dyn ActivationTracker>),
    )
    .await;
    let first_value = recovered.compute(&key).await.unwrap();
    let first = observed_route(&first_value).clone();
    assert!(first.result().as_ref().is_ok());
    assert!(trace.contains(&key.to_string()));
    let warm_value = recovered.compute(&key).await.unwrap();
    let warm = observed_route(&warm_value);
    assert!(HostRootRepositoryLoadRouteObservationKey::equality(
        &first_value,
        &warm_value
    ));
    assert!(Arc::ptr_eq(first.result(), warm.result()));

    let mut updater = recovered.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::empty(),
        )])
        .unwrap();
    let mut need_tx = updater.commit().await;
    let need = need_tx.compute(&key).await.unwrap();
    assert!(matches!(need, SourcePreparationOutcome::Need(_)));
    assert!(!HostRootRepositoryLoadRouteObservationKey::validity(&need));
    assert!(!HostRootRepositoryLoadRouteObservationKey::equality(
        &need, &need
    ));

    let missing_module = "module(name='bazel_tools')\n";
    let mut missing_tx = transaction(&dice, missing_module, EXTENSION_A, true, None).await;
    let missing_value = missing_tx.compute(&key).await.unwrap();
    let missing = observed_route(&missing_value).clone();
    assert!(matches!(
        missing.result().as_ref(),
        Err(HostRootRepositoryLoadRouteError {
            kind: HostRootRepositoryLoadRouteErrorKind::Root(_),
            ..
        })
    ));

    let mut restored_tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
    let restored_value = restored_tx.compute(&key).await.unwrap();
    let restored = observed_route(&restored_value);
    assert_ne!(first.result(), missing.result());
    assert_eq!(first.result(), restored.result());
    assert!(!HostRootRepositoryLoadRouteObservationKey::equality(
        &first_value,
        &missing_value
    ));
    assert!(HostRootRepositoryLoadRouteObservationKey::equality(
        &first_value,
        &restored_value
    ));
}
