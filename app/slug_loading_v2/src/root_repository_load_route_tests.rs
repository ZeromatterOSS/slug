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
use std::path::PathBuf;
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
use slug_bzlmod_v2::HostRepositorySourceObservationEpochKey;
use slug_bzlmod_v2::HostRepositorySourceObservationView;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::HostSelectedObservationFrontier;
use slug_bzlmod_v2::RootRepositoryRouteKey;
use slug_bzlmod_v2::RootRepositoryRouteObservationError;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::host_repository_relative_path;
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
use crate::canonical_repository_route_tests::tests::builtin_graph_dice;
use crate::canonical_repository_route_tests::tests::builtin_graph_module;
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

    fn contains_fragment(&self, fragment: &str) -> bool {
        self.0
            .lock()
            .unwrap()
            .iter()
            .any(|(candidate, _)| candidate.contains(fragment))
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
        .compute(
            &HostRootRepositoryLoadRouteKey::for_root_build(workspace.clone(), builtin.clone())
                .unwrap(),
        )
        .await
        .unwrap()
    else {
        panic!("root-build built-in route must complete")
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
async fn ordinary_builtin_route_uses_canonical_owner_but_root_build_keeps_root_bootstrap() {
    let dice = builtin_graph_dice();
    let module = builtin_graph_module();
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let apparent = ApparentRepoName::new("bazel_tools").unwrap();
    let ordinary_key =
        HostRootRepositoryLoadRouteKey::new(workspace.clone(), apparent.clone()).unwrap();
    let observed_key =
        HostRootRepositoryLoadRouteObservationKey::new(workspace.clone(), apparent.clone())
            .unwrap();
    let root_build_key =
        HostRootRepositoryLoadRouteKey::for_root_build(workspace.clone(), apparent.clone())
            .unwrap();
    let trace = Arc::new(RouteTrace::default());
    let mut tx = transaction(
        &dice,
        &module,
        EXTENSION_A,
        true,
        Some(trace.clone() as Arc<dyn ActivationTracker>),
    )
    .await;

    let SourcePreparationOutcome::Complete(ordinary) = tx.compute(&ordinary_key).await.unwrap()
    else {
        panic!("ordinary built-in route must complete")
    };
    let ordinary_route = ordinary.as_ref().as_ref().unwrap();
    assert_eq!(ordinary_route.apparent_repo(), &apparent);
    assert_eq!(ordinary_route.canonical_repo().as_str(), "bazel_tools");
    let HostRepositorySourceRoute::Canonical(input) = ordinary_route.source() else {
        panic!("ordinary built-in route must retain canonical source input")
    };
    assert_eq!(
        input.view().route().view().kind(),
        HostCanonicalRepositoryRouteKind::Builtin
    );
    let dependencies = trace.dependencies(&ordinary_key.to_string());
    assert_eq!(dependencies.len(), 2);
    assert!(dependencies[0].starts_with("root-repository-route:"));
    assert!(dependencies[1].starts_with("host-canonical-repository-load-route:"));

    let source_key = HostRepositorySourceObservationEpochKey::new_canonical(
        input.clone(),
        host_repository_relative_path(PathBuf::from("tools/cpp/BUILD")).unwrap(),
    );
    let SourcePreparationOutcome::Complete(Ok(source)) = tx.compute(&source_key).await.unwrap()
    else {
        panic!("root-visible canonical built-in BUILD source must complete")
    };
    let source_value = source.result().as_ref().as_ref().unwrap();
    let HostRepositorySourceObservationView::Builtin(catalog_file) = source_value.view() else {
        panic!("root-visible built-in BUILD source must use the catalog")
    };
    assert_eq!(catalog_file.path(), "tools/cpp/BUILD");
    assert_eq!(
        catalog_file.bytes(),
        include_bytes!("../../slug_bzlmod_v2/builtin/bazel_tools/tools/cpp/BUILD")
    );
    assert!(source.observations().observations().is_empty());
    assert_eq!(
        trace.dependencies(&source_key.to_string()),
        ["builtin-bazel-tools-source-file:tools/cpp/BUILD"]
    );

    let observed_value = tx.compute(&observed_key).await.unwrap();
    let observed = observed_route(&observed_value);
    assert_eq!(observed.result(), &ordinary);
    let observed_dependencies = trace.dependencies(&observed_key.to_string());
    assert_eq!(observed_dependencies.len(), 2);
    assert!(observed_dependencies[0].starts_with("observed-root-repository-route:"));
    assert!(observed_dependencies[1].starts_with("observed-host-canonical-repository-load-route:"));
    let SourcePreparationOutcome::Complete(Ok(root)) = tx
        .compute(
            &RootRepositoryRouteObservationKey::new(workspace.clone(), apparent.clone()).unwrap(),
        )
        .await
        .unwrap()
    else {
        panic!("observed root predecessor must complete")
    };
    let SourcePreparationOutcome::Complete(Ok(canonical)) = tx
        .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
            workspace.clone(),
            CanonicalRepoName::new("bazel_tools").unwrap(),
        ))
        .await
        .unwrap()
    else {
        panic!("observed canonical predecessor must complete")
    };
    assert_eq!(
        observed.observations(),
        &merge_observations(root.observations(), canonical.observations()).unwrap()
    );

    let SourcePreparationOutcome::Complete(root_build) = tx.compute(&root_build_key).await.unwrap()
    else {
        panic!("root-build built-in route must complete")
    };
    assert!(matches!(
        root_build.as_ref(),
        Ok(route) if route.apparent_repo() == &apparent
            && matches!(route.source(), HostRepositorySourceRoute::Root(root) if root.is_builtin_bazel_tools())
    ));
    let root_build_dependencies = trace.dependencies(&root_build_key.to_string());
    assert_eq!(root_build_dependencies.len(), 1);
    assert!(root_build_dependencies[0].starts_with("root-build-repository-route:"));
    assert!(!trace.contains_fragment("host-repository-source-file:@@bazel_tools:"));
    assert!(!trace.contains_fragment("repository-materialization-result:@@bazel_tools"));

    let warm = tx.compute(&observed_key).await.unwrap();
    assert!(HostRootRepositoryLoadRouteObservationKey::equality(
        &observed_value,
        &warm
    ));
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::empty(),
        )])
        .unwrap();
    let mut missing = updater.commit().await;
    let need = missing.compute(&observed_key).await.unwrap();
    assert!(matches!(need, SourcePreparationOutcome::Need(_)));
    assert!(!HostRootRepositoryLoadRouteObservationKey::validity(&need));
    assert!(!HostRootRepositoryLoadRouteObservationKey::equality(
        &need, &need
    ));
    let mut restored = transaction(&dice, &module, EXTENSION_A, true, None).await;
    let restored_value = restored.compute(&observed_key).await.unwrap();
    assert!(HostRootRepositoryLoadRouteObservationKey::equality(
        &observed_value,
        &restored_value
    ));
}

#[tokio::test]
async fn builtin_function_transition_allowlist_loads_and_reuses_genuine_package() {
    use slug_identity_v2::PackagePath;

    use crate::PackageTargetKind;
    use crate::RepositoryPackageLoadKey;
    use crate::RepositoryPackageLoadObservationKey;

    let dice = builtin_graph_dice();
    let module = builtin_graph_module();
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let trace = Arc::new(RouteTrace::default());
    let mut tx = transaction(
        &dice,
        &module,
        EXTENSION_A,
        true,
        Some(trace.clone() as Arc<dyn ActivationTracker>),
    )
    .await;
    let route_key = HostRootRepositoryLoadRouteKey::new(
        workspace,
        ApparentRepoName::new("bazel_tools").unwrap(),
    )
    .unwrap();
    let SourcePreparationOutcome::Complete(route) = tx.compute(&route_key).await.unwrap() else {
        panic!("builtin route must complete without materialization Need")
    };
    let route = route.as_ref().as_ref().unwrap();
    assert_eq!(route.apparent_repo().as_str(), "bazel_tools");
    assert_eq!(route.canonical_repo().as_str(), "bazel_tools");
    let HostRepositorySourceRoute::Canonical(input) = route.source() else {
        panic!("builtin package must retain canonical owner")
    };
    assert_eq!(
        input.view().route().view().kind(),
        HostCanonicalRepositoryRouteKind::Builtin
    );
    let package_path =
        PackagePath::parse("tools/allowlists/function_transition_allowlist").unwrap();
    let legacy_key = RepositoryPackageLoadKey::new_canonical(input.clone(), package_path.clone());
    let observed_key =
        RepositoryPackageLoadObservationKey::new_canonical(input.clone(), package_path);
    let SourcePreparationOutcome::Complete(legacy) = tx.compute(&legacy_key).await.unwrap() else {
        panic!("genuine legacy builtin package must complete without Need")
    };
    let SourcePreparationOutcome::Complete(Ok(observed)) = tx.compute(&observed_key).await.unwrap()
    else {
        panic!("genuine observed builtin package must complete without Need")
    };
    assert_eq!(observed.result(), &legacy);
    assert!(observed.observations().observations().is_empty());
    let package = legacy.as_ref().as_ref().unwrap();
    assert!(package.direct_load_roots.is_empty() && package.reachable_loads.is_empty());
    let target = package
        .targets
        .iter()
        .find(|target| target.name == "function_transition_allowlist")
        .unwrap();
    let PackageTargetKind::PackageGroup { contents, includes } = &target.kind else {
        panic!("original package_group must be produced, not a synthesized rule")
    };
    assert!(contents.positive_all() && !contents.has_private());
    assert!(contents.exact_positive().is_empty() && contents.subtree_positive().is_empty());
    assert!(contents.exact_negative().is_empty() && contents.subtree_negative().is_empty());
    assert!(includes.is_empty());
    assert!(trace.contains(
        "builtin-bazel-tools-source-file:tools/allowlists/function_transition_allowlist/BUILD"
    ));
    let source_key = HostRepositorySourceObservationEpochKey::new_canonical(
        input.clone(),
        host_repository_relative_path(PathBuf::from(
            "tools/allowlists/function_transition_allowlist/BUILD",
        ))
        .unwrap(),
    );
    let SourcePreparationOutcome::Complete(Ok(source)) = tx.compute(&source_key).await.unwrap()
    else {
        panic!("genuine embedded source must complete without Need")
    };
    let HostRepositorySourceObservationView::Builtin(file) =
        source.result().as_ref().as_ref().unwrap().view()
    else {
        panic!("allowlist source must use builtin catalog")
    };
    assert_eq!(
        file.bytes(),
        include_bytes!(
            "../../slug_bzlmod_v2/builtin/bazel_tools/tools/allowlists/function_transition_allowlist/BUILD"
        )
    );
    assert_eq!(
        trace.dependencies(&source_key.to_string()),
        ["builtin-bazel-tools-source-file:tools/allowlists/function_transition_allowlist/BUILD"]
    );
    let SourcePreparationOutcome::Complete(warm_legacy) = tx.compute(&legacy_key).await.unwrap()
    else {
        panic!("warm legacy package must complete")
    };
    assert!(Arc::ptr_eq(&legacy, &warm_legacy));
    let SourcePreparationOutcome::Complete(Ok(warm)) = tx.compute(&observed_key).await.unwrap()
    else {
        panic!("warm observed package must complete")
    };
    assert!(Arc::ptr_eq(observed.result(), warm.result()));
    let SourcePreparationOutcome::Complete(Ok(warm_source)) =
        tx.compute(&source_key).await.unwrap()
    else {
        panic!("warm source must complete")
    };
    assert!(Arc::ptr_eq(source.result(), warm_source.result()));
    assert!(!trace.contains_fragment("repository-materialization-result:@@bazel_tools"));
    assert!(!trace.contains_fragment("host-repository-source-file:@@bazel_tools:"));
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

// Private test bridges, not semantic constructors or public API.
pub(crate) fn diagnostic_projection(message: Arc<str>) -> HostRootRepositoryLoadRouteError {
    HostRootRepositoryLoadRouteError {
        apparent_repo: ApparentRepoName::new("repo").unwrap(),
        kind: HostRootRepositoryLoadRouteErrorKind::Projection(message),
    }
}
pub(crate) fn diagnostic_load(
    error: HostCanonicalRepositoryLoadRouteError,
) -> HostRootRepositoryLoadRouteError {
    HostRootRepositoryLoadRouteError {
        apparent_repo: ApparentRepoName::new("repo").unwrap(),
        kind: HostRootRepositoryLoadRouteErrorKind::LoadRoute(error),
    }
}

#[tokio::test]
async fn root_load_diagnostic_views_borrow_original_siblings() {
    use HostRootRepositoryLoadRouteErrorKind as E;

    use super::RootLoadDiagnostic as V;
    use crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteErrorKind as L;
    use crate::canonical_repository_mapping::HostCanonicalRepositoryApparentMappingErrorKind as M;
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
    let SourcePreparationOutcome::Complete(original) = tx
        .compute(
            &RootRepositoryRouteKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                ApparentRepoName::new("missing").unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap()
    else {
        panic!("retained root error")
    };
    let root = HostRootRepositoryLoadRouteError {
        apparent_repo: ApparentRepoName::new("repo").unwrap(),
        kind: E::Root(original.as_ref().as_ref().unwrap_err().clone()),
    };
    let (E::Root(retained), V::Root(view)) = (&root.kind, root.diagnostic_view()) else {
        panic!("Root")
    };
    assert!(std::ptr::eq(retained, view));
    assert_eq!(
        crate::registration_diagnostic::tests::render_root_load(&root),
        "RootLoad: Unknown @missing"
    );
    let mapping = HostRootRepositoryLoadRouteError {
        apparent_repo: ApparentRepoName::new("repo").unwrap(),
        kind: E::Mapping(HostCanonicalRepositoryApparentMappingError {
            context_repo: CanonicalRepoName::new("ctx+").unwrap(),
            apparent_repo: ApparentRepoName::new("child").unwrap(),
            kind: M::RootApparent,
        }),
    };
    let (E::Mapping(retained), V::Mapping(view)) = (&mapping.kind, mapping.diagnostic_view())
    else {
        panic!("Mapping")
    };
    assert!(std::ptr::eq(retained, view));
    let load = diagnostic_load(HostCanonicalRepositoryLoadRouteError {
        canonical_repo: CanonicalRepoName::new("ctx+").unwrap(),
        kind: L::RouteCompute("CAUSE".into()),
    });
    let (E::LoadRoute(retained), V::LoadRoute(view)) = (&load.kind, load.diagnostic_view()) else {
        panic!("LoadRoute")
    };
    assert!(std::ptr::eq(retained, view));
    let message: Arc<str> = Arc::from("CAUSE");
    let weak = Arc::downgrade(&message);
    for kind in [
        E::RootCompute(message.clone()),
        E::MappingCompute(message.clone()),
        E::LoadRouteCompute(message.clone()),
        E::Projection(message.clone()),
    ] {
        let owner = HostRootRepositoryLoadRouteError {
            apparent_repo: ApparentRepoName::new("repo").unwrap(),
            kind,
        };
        let before = Arc::strong_count(&message);
        let (view, name) = match owner.diagnostic_view() {
            V::RootCompute(s) => (s, "RootCompute"),
            V::MappingCompute(s) => (s, "MappingCompute"),
            V::LoadRouteCompute(s) => (s, "LoadRouteCompute"),
            V::Projection(s) => (s, "Projection"),
            _ => panic!("message sibling"),
        };
        assert_eq!(view.as_ptr(), message.as_ptr());
        assert_eq!(
            crate::registration_diagnostic::tests::render_root_load(&owner),
            format!("RootLoad: {name}: CAUSE")
        );
        assert_eq!(Arc::strong_count(&message), before);
    }
    drop(message);
    assert!(weak.upgrade().is_none());
}

#[test]
fn root_load_diagnostic_walks_nested_routes_and_bounds_messages() {
    use crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteErrorKind as L;
    use crate::canonical_repository_mapping::HostCanonicalRepositoryApparentMappingErrorKind as M;
    use crate::canonical_repository_route::HostCanonicalRepositoryRouteError;
    use crate::canonical_repository_route::HostCanonicalRepositoryRouteErrorKind as R;
    use crate::registration_diagnostic::tests::render_root_load;
    let repo = CanonicalRepoName::new("ctx+").unwrap();
    let route = || HostCanonicalRepositoryRouteError {
        canonical_repo: repo.clone(),
        kind: R::SelectedCompute("CAUSE".into()),
    };
    let load = diagnostic_load(HostCanonicalRepositoryLoadRouteError {
        canonical_repo: repo.clone(),
        kind: L::Route(route()),
    });
    assert_eq!(
        render_root_load(&load),
        "RootLoad: CanonicalRoute ctx+: Route ctx+: SelectedCompute: CAUSE"
    );
    let mapping = HostRootRepositoryLoadRouteError {
        apparent_repo: ApparentRepoName::new("repo").unwrap(),
        kind: HostRootRepositoryLoadRouteErrorKind::Mapping(
            HostCanonicalRepositoryApparentMappingError {
                context_repo: repo.clone(),
                apparent_repo: ApparentRepoName::new("child").unwrap(),
                kind: M::Route(route()),
            },
        ),
    };
    assert_eq!(
        render_root_load(&mapping),
        "RootLoad: ApparentMapping ctx+ @child: Route ctx+: SelectedCompute: CAUSE"
    );
    assert_eq!(
        render_root_load(&diagnostic_projection("line\n\u{e9}\t".into())),
        "RootLoad: Projection: line\\n\\u{e9}\\t"
    );
    let owner = diagnostic_projection(format!("{}TAIL_POISON", "\u{e9}".repeat(3072)).into());
    let text = render_root_load(&owner);
    assert!(
        text.is_ascii()
            && text.len() <= 3072
            && text.ends_with("[diagnostic incomplete: output limit]")
            && !text.contains("TAIL_POISON")
    );
}

const COMMAND_LOCAL_MODULE: &str =
    "module(name='bazel_tools')\nbazel_dep(name='local',version='1',repo_name='local_alias')\n";

async fn command_local_transaction(
    dice: &Arc<Dice>,
    root_module: &str,
    command_root: Option<&str>,
) -> (dice::DiceTransaction, PathObservationEpoch) {
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    let command = command_root.map(|root| format!("local={root}"));
    let policy = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        std::path::Path::new(WORKSPACE),
        command.as_deref(),
    )
    .unwrap();
    let mut tx = crate::canonical_repository_route_tests::tests::transaction_with_policy(
        dice,
        root_module,
        EXTENSION_A,
        true,
        None,
        policy,
    )
    .await;
    let old = tx.compute(&PathObservationEpochKey).await.unwrap();
    let root = command_root
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{WORKSPACE}/local"));
    let demand = |suffix: &str, operation| {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(format!("{root}{suffix}")).unwrap(),
            operation,
        )
    };
    let body_demand = demand("/MODULE.bazel", PathObservationOperation::FileBytes);
    let replaced = [
        body_demand.clone(),
        demand("", PathObservationOperation::Lstat),
        demand("/MODULE.bazel", PathObservationOperation::Lstat),
        demand("/REPO.bazel", PathObservationOperation::Lstat),
        demand("/.bazelignore", PathObservationOperation::Lstat),
    ];
    let mut rows = old
        .observations()
        .iter()
        .filter(|(key, _)| !replaced.contains(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    for (suffix, kind) in [
        ("", PathNodeKind::Directory),
        ("/MODULE.bazel", PathNodeKind::RegularFile),
    ] {
        rows.push((
            demand(suffix, PathObservationOperation::Lstat),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(kind, 36, 1, 1, 1, 0o755),
            ))),
        ));
    }
    for suffix in ["/REPO.bazel", "/.bazelignore"] {
        rows.push((
            demand(suffix, PathObservationOperation::Lstat),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing)),
        ));
    }
    rows.push((
        body_demand,
        Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Present(Arc::from(
                b"module(name='local',version='2')\n".as_slice(),
            )),
        )),
    ));
    let epoch = PathObservationEpoch::from_shared(rows).unwrap();
    let mut updater = tx.into_updater();
    updater
        .changed_to([(PathObservationEpochKey, epoch.clone())])
        .unwrap();
    let mut tx = updater.commit().await;
    let key = HostCanonicalRepositoryLoadRouteKey::new(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        CanonicalRepoName::new("local+").unwrap(),
    );
    let SourcePreparationOutcome::Need(need) = tx.compute(&key).await.unwrap() else {
        panic!("normal Local requires materialization")
    };
    let request = need
        .repository_materializations()
        .values()
        .next()
        .expect("original materialization request")
        .clone();
    assert!(
        matches!(&request.kind, slug_bzlmod_v2::RepositoryMaterializationKind::Local { logical_root } if logical_root.as_path() == std::path::Path::new(&root))
    );
    let mut updater = tx.into_updater();
    updater
        .changed_to([(
            RepositoryMaterializationResultEpochKey {
                workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            },
            RepositoryMaterializationResultEpoch::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
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
    (updater.commit().await, epoch)
}

fn command_local_input(
    value: &<HostRootRepositoryLoadRouteObservationKey as Key>::Value,
) -> &HostCanonicalRepositorySourceInput {
    let observed = observed_route(value);
    let HostRepositorySourceRoute::Canonical(input) =
        observed.result().as_ref().as_ref().unwrap().source()
    else {
        panic!("original canonical input")
    };
    input
}

#[tokio::test]
async fn root_build_command_local_preserves_selected_input_mapping_and_epoch() {
    use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionKey;
    use slug_bzlmod_v2::HostCanonicalSelectedModuleIdentity;
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let root = format!("{WORKSPACE}/local");
    let (mut tx, epoch) = command_local_transaction(&dice, COMMAND_LOCAL_MODULE, Some(&root)).await;
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let apparent = ApparentRepoName::new("local_alias").unwrap();
    let target = CanonicalRepoName::new("local+").unwrap();
    let canonical = tx
        .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
            workspace.clone(),
            target.clone(),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(canonical)) = canonical else {
        panic!("canonical producer")
    };
    let key = HostRootRepositoryLoadRouteObservationKey::for_root_build(
        workspace.clone(),
        apparent.clone(),
    )
    .unwrap();
    let value = tx.compute(&key).await.unwrap();
    let input = command_local_input(&value);
    assert!(Arc::ptr_eq(
        input.view().route(),
        canonical.result().as_ref().as_ref().unwrap().route()
    ));
    assert_eq!(
        input.view().route().bzl_repository_mapping(),
        canonical
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .route()
            .bzl_repository_mapping()
    );
    let mapping = input.view().route().bzl_repository_mapping();
    assert!(!mapping.is_empty());
    assert!(
        mapping
            .iter()
            .any(|(name, target)| name.as_str() == "local" && target.as_str() == "local+")
    );
    let view = input.view().route().view();
    assert_eq!(
        view.kind(),
        HostCanonicalRepositoryRouteKind::SelectedNonregistry
    );
    assert_eq!(
        view.local_path_policy(),
        Some(HostRepositoryLocalPathPolicy::CommandAbsolute)
    );
    assert_eq!(view.associated_module_metadata(), Some(("local", "2")));
    let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
    else {
        panic!("Local request")
    };
    assert_eq!(request.repo_spec, *view.repo_spec().unwrap());
    assert!(
        matches!(&request.kind, RepositoryMaterializationKind::Local { logical_root } if logical_root.as_path() == std::path::Path::new(&root))
    );
    let SourcePreparationOutcome::Complete(definition) = tx
        .compute(&HostCanonicalSelectedModuleDefinitionKey::new(
            workspace.clone(),
            target,
        ))
        .await
        .unwrap()
    else {
        panic!("selected owner")
    };
    assert!(matches!(
        definition.as_ref().as_ref().unwrap().view().identity(),
        HostCanonicalSelectedModuleIdentity::Module {
            name: "local",
            normalized_version: ""
        }
    ));
    for (demand, original) in canonical.observations().observations() {
        assert!(Arc::ptr_eq(
            original,
            observed_route(&value).observations().get(demand).unwrap()
        ));
    }
    for (demand, observed) in observed_route(&value).observations().observations() {
        assert!(Arc::ptr_eq(observed, epoch.get(demand).unwrap()));
    }
    let ordinary = tx
        .compute(&HostRootRepositoryLoadRouteKey::new(workspace, apparent).unwrap())
        .await
        .unwrap();
    assert!(
        matches!(ordinary, SourcePreparationOutcome::Complete(value) if value.as_ref().is_err())
    );
}

#[tokio::test]
async fn root_build_command_local_admission_is_exact_and_fail_closed() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let root = format!("{WORKSPACE}/local");
    let (mut tx, _) = command_local_transaction(&dice, COMMAND_LOCAL_MODULE, Some(&root)).await;
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let apparent = ApparentRepoName::new("local_alias").unwrap();
    let target = CanonicalRepoName::new("local+").unwrap();
    let key = HostRootRepositoryLoadRouteObservationKey::for_root_build(
        workspace.clone(),
        apparent.clone(),
    )
    .unwrap();
    let value = tx.compute(&key).await.unwrap();
    let input = command_local_input(&value);
    assert!(command_local_input_admitted(&key.0, &target, input));
    assert!(!command_local_input_admitted(
        &HostRootRepositoryLoadRouteKey::new(workspace.clone(), apparent.clone()).unwrap(),
        &target,
        input
    ));
    assert!(!command_local_input_admitted(
        &HostRootRepositoryLoadRouteKey::for_root_build(
            NormalizedAbsolutePath::new("/other").unwrap(),
            apparent
        )
        .unwrap(),
        &target,
        input
    ));
    assert!(!command_local_input_admitted(
        &key.0,
        &CanonicalRepoName::new("other+").unwrap(),
        input
    ));
    let module_local = "module(name='bazel_tools')\nbazel_dep(name='local',version='1',repo_name='local_alias')\nlocal_path_override(module_name='local',path='local')\n";
    let other_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let (mut other, _) = command_local_transaction(&other_dice, module_local, None).await;
    let result = other
        .compute(&HostCanonicalRepositoryLoadRouteKey::new(
            workspace.clone(),
            target.clone(),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(result) = result else {
        panic!("MODULE Local producer")
    };
    assert!(!command_local_input_admitted(
        &key.0,
        &target,
        result.as_ref().as_ref().unwrap().input()
    ));
    let no_command = registry_dice(PARENT_MODULE, LEAF_MAPPING_A, SOURCE_A);
    let mut registry = transaction(&no_command, ROOT_MODULE, EXTENSION_A, true, None).await;
    let target = CanonicalRepoName::new("parent+").unwrap();
    let result = registry
        .compute(&HostCanonicalRepositoryLoadRouteKey::new(
            workspace,
            target.clone(),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(result) = result else {
        panic!("registry producer")
    };
    let input = result.as_ref().as_ref().unwrap().input();
    assert_eq!(
        input.view().route().view().kind(),
        HostCanonicalRepositoryRouteKind::SelectedRegistry
    );
    assert!(!command_local_input_admitted(&key.0, &target, input));
}

#[tokio::test]
async fn root_build_command_local_warm_change_restore_and_need_remain_owned() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let a = format!("{WORKSPACE}/local");
    let b = format!("{WORKSPACE}/local-b");
    let key = HostRootRepositoryLoadRouteObservationKey::for_root_build(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        ApparentRepoName::new("local_alias").unwrap(),
    )
    .unwrap();
    let (mut tx, epoch) = command_local_transaction(&dice, COMMAND_LOCAL_MODULE, Some(&a)).await;
    let first = tx.compute(&key).await.unwrap();
    let warm = tx.compute(&key).await.unwrap();
    assert!(Arc::ptr_eq(
        observed_route(&first).result(),
        observed_route(&warm).result()
    ));
    let retained = command_local_input(&first).clone();
    let weak = Arc::downgrade(retained.view().route());
    let before = Arc::strong_count(retained.view().route());
    assert!(command_local_input_admitted(
        &key.0,
        &CanonicalRepoName::new("local+").unwrap(),
        &retained
    ));
    assert_eq!(Arc::strong_count(retained.view().route()), before);
    drop(tx);
    let (mut changed_tx, _) =
        command_local_transaction(&dice, COMMAND_LOCAL_MODULE, Some(&b)).await;
    let changed = changed_tx.compute(&key).await.unwrap();
    assert_ne!(command_local_input(&changed), &retained);
    assert!(
        matches!(retained.view().disposition(), HostRepositorySourceInputDispositionView::Request(request) if matches!(&request.kind, RepositoryMaterializationKind::Local { logical_root } if logical_root.as_path() == std::path::Path::new(&a)))
    );
    drop(changed_tx);
    let (mut restored_tx, _) =
        command_local_transaction(&dice, COMMAND_LOCAL_MODULE, Some(&a)).await;
    let restored = restored_tx.compute(&key).await.unwrap();
    assert_eq!(command_local_input(&restored), &retained);
    let mut updater = restored_tx.into_updater();
    updater
        .changed_to([(PathObservationEpochKey, PathObservationEpoch::empty())])
        .unwrap();
    let mut pending = updater.commit().await;
    assert!(matches!(
        pending.compute(&key).await.unwrap(),
        SourcePreparationOutcome::Need(_)
    ));
    let mut updater = pending.into_updater();
    updater
        .changed_to([(PathObservationEpochKey, epoch)])
        .unwrap();
    let mut recovered = updater.commit().await;
    assert_eq!(
        command_local_input(&recovered.compute(&key).await.unwrap()),
        &retained
    );
    drop(recovered);
    drop(first);
    drop(warm);
    drop(changed);
    drop(restored);
    drop(dice);
    assert!(weak.upgrade().is_some());
    drop(retained);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while weak.upgrade().is_some() {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("original route retires after DICE history and last handle drop");
}
