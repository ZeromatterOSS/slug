/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use compact_str::CompactString;
    use dice::ActivationData;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::Key;
    use dice::UserComputationData;
    use dupe::Dupe;
    use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
    use slug_bzlmod_v2::HostCanonicalRepositoryRouteKind;
    use slug_bzlmod_v2::HostCanonicalRepositorySourceInput;
    use slug_bzlmod_v2::HostExternalPackageBoundaryKind;
    use slug_bzlmod_v2::HostExternalPackageBoundaryObservationKey;
    use slug_bzlmod_v2::HostRepositoryDirectoryListingObservationKey;
    use slug_bzlmod_v2::HostRepositorySourceFileObservationKey;
    use slug_bzlmod_v2::HostRepositorySourceFileValue;
    use slug_bzlmod_v2::HostRepositorySourceInputDispositionView;
    use slug_bzlmod_v2::HostRepositorySourceObservationEpochKey;
    use slug_bzlmod_v2::HostRepositorySourceObservationInput;
    use slug_bzlmod_v2::HostRepositorySourceObservationView;
    use slug_bzlmod_v2::HostRepositorySourceRoute;
    use slug_bzlmod_v2::HostRootRepositoryMappingKey;
    use slug_bzlmod_v2::ObservedHostExternalPackageBoundary;
    use slug_bzlmod_v2::ObservedHostRepositorySourceObservation;
    use slug_bzlmod_v2::ObservedRepositoryPackageSource;
    use slug_bzlmod_v2::RegistryFileUrl;
    use slug_bzlmod_v2::RegistryIo;
    use slug_bzlmod_v2::RegistryIoOutcome;
    use slug_bzlmod_v2::RegistryTransportError;
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_bzlmod_v2::RepositoryPackageSourceKey;
    use slug_bzlmod_v2::RepositoryPackageSourceObservationKey;
    use slug_bzlmod_v2::RootRepositoryRouteKey;
    use slug_bzlmod_v2::SourcePreparationNeeds;
    use slug_bzlmod_v2::SourcePreparationOutcome;
    use slug_bzlmod_v2::host_canonical_repository_source_input;
    use slug_bzlmod_v2::host_repository_relative_path;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalRepoName;
    use slug_identity_v2::PackageIdentifier;
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
    use slug_workspace_v2::PathObservationInstanceId;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;

    use super::super::HostCanonicalRepositoryRouteObservationKey;
    use super::super::HostSelectedRepositoryFileEffectObservationKey;
    use crate::canonical_repository_load_route::*;
    use crate::canonical_repository_route_tests::tests::EXTENSION_A;
    use crate::canonical_repository_route_tests::tests::MODULE;
    use crate::canonical_repository_route_tests::tests::WORKSPACE;
    use crate::canonical_repository_route_tests::tests::names;
    use crate::canonical_repository_route_tests::tests::transaction;
    use crate::canonical_repository_route_tests::tests::validated;

    const ROOT_MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='parent', version='1', repo_name='parent_alias')\n";
    const PARENT_MODULE: &[u8] =
        b"module(name='parent', version='1')\nbazel_dep(name='leaf', version='1', repo_name='leaf_from_parent')\n";
    const LEAF_MAPPING_A: &[u8] = b"module(name='leaf', version='1')\nbazel_dep(name='mapped', version='1', repo_name='alias_a')\n";
    const LEAF_MAPPING_B: &[u8] = b"module(name='leaf', version='1')\nbazel_dep(name='mapped', version='1', repo_name='alias_b')\n";
    const MAPPED_MODULE: &[u8] = b"module(name='mapped', version='1')\n";
    const SOURCE_A: &[u8] =
        br#"{"url":"https://origin.invalid/leaf-a.tgz","integrity":"sha256-a"}"#;
    const SOURCE_B: &[u8] =
        br#"{"url":"https://origin.invalid/leaf-b.tgz","integrity":"sha256-b"}"#;
    const OTHER_SOURCE: &[u8] =
        br#"{"url":"https://origin.invalid/source.tgz","integrity":"sha256-source"}"#;

    struct StaticRegistryIo(BTreeMap<String, Arc<[u8]>>);

    #[async_trait]
    impl RegistryIo for StaticRegistryIo {
        async fn read_exact(
            &self,
            url: &RegistryFileUrl,
        ) -> Result<RegistryIoOutcome, RegistryTransportError> {
            Ok(self
                .0
                .get(url.as_str())
                .cloned()
                .map_or(RegistryIoOutcome::NotFound, RegistryIoOutcome::Found))
        }
    }

    #[derive(Default)]
    struct DependencyTrace(Mutex<Vec<(String, Vec<String>)>>);

    impl ActivationTracker for DependencyTrace {
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

    impl DependencyTrace {
        fn dependencies(&self, key: &str) -> Vec<String> {
            self.0
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find_map(|(candidate, dependencies)| {
                    (candidate == key).then(|| dependencies.clone())
                })
                .unwrap_or_default()
        }

        fn all_keys(&self) -> Vec<String> {
            self.0
                .lock()
                .unwrap()
                .iter()
                .map(|(key, _)| key.clone())
                .collect()
        }
    }

    fn assert_no_activation(tracker: &DependencyTrace, prefix: &str) {
        assert!(!tracker.all_keys().iter().any(|key| key.starts_with(prefix)));
    }

    fn registry_io(leaf_module: &'static [u8], leaf_source: &'static [u8]) -> StaticRegistryIo {
        let mut files = BTreeMap::from([(
            "https://registry.invalid/bazel_registry.json".to_owned(),
            Arc::from(&b"{}"[..]),
        )]);
        for (name, module, source) in [
            ("parent", PARENT_MODULE, OTHER_SOURCE),
            ("leaf", leaf_module, leaf_source),
            ("mapped", MAPPED_MODULE, OTHER_SOURCE),
        ] {
            files.insert(
                format!("https://registry.invalid/modules/{name}/1/MODULE.bazel"),
                Arc::from(module),
            );
            files.insert(
                format!("https://registry.invalid/modules/{name}/1/source.json"),
                Arc::from(source),
            );
        }
        StaticRegistryIo(files)
    }

    fn registry_dice(leaf_module: &'static [u8], leaf_source: &'static [u8]) -> Arc<Dice> {
        let mut builder = Dice::builder();
        slug_bzlmod_v2::install_registry_io(
            &mut builder,
            Arc::new(registry_io(leaf_module, leaf_source)),
        );
        builder.build(DetectCycles::Enabled)
    }

    fn load_route(
        value: &HostCanonicalRepositoryLoadRouteOutcome,
    ) -> &HostCanonicalRepositoryLoadRoute {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("load route must complete")
        };
        value.as_ref().as_ref().unwrap()
    }

    fn hash<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    async fn selected_input(
        leaf_module: &'static [u8],
        leaf_source: &'static [u8],
    ) -> (Arc<Dice>, HostCanonicalRepositorySourceInput) {
        selected_input_for(leaf_module, leaf_source, "leaf+").await
    }

    async fn selected_load_outcome(
        leaf_module: &'static [u8],
        leaf_source: &'static [u8],
    ) -> HostCanonicalRepositoryLoadRouteOutcome {
        let dice = registry_dice(leaf_module, leaf_source);
        transaction(&dice, ROOT_MODULE, EXTENSION_A, true, None)
            .await
            .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::new("leaf+").unwrap(),
            ))
            .await
            .unwrap()
    }

    async fn selected_input_for(
        leaf_module: &'static [u8],
        leaf_source: &'static [u8],
        canonical_repo: &str,
    ) -> (Arc<Dice>, HostCanonicalRepositorySourceInput) {
        let dice = registry_dice(leaf_module, leaf_source);
        let mut tx = transaction(&dice, ROOT_MODULE, EXTENSION_A, true, None).await;
        let outcome = tx
            .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::new(canonical_repo).unwrap(),
            ))
            .await
            .unwrap();
        (dice, load_route(&outcome).input().clone())
    }

    async fn generated_input(
        extension: &str,
        tracker: Option<Arc<dyn ActivationTracker>>,
    ) -> (
        Arc<Dice>,
        CanonicalRepoName,
        HostCanonicalRepositorySourceInput,
    ) {
        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, made='made')\n";
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut tx = transaction(&dice, module, extension, true, tracker).await;
        let canonical = names(&validated(&mut tx).await).remove(0);
        let outcome = tx
            .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                canonical.clone(),
            ))
            .await
            .unwrap();
        let input = load_route(&outcome).input().clone();
        let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
        else {
            panic!("generated route must retain its materialization request")
        };
        let slug_bzlmod_v2::RepositoryMaterializationKind::GeneratedFileEffects(_) = &request.kind
        else {
            panic!("generated route must retain its effect plan")
        };
        (dice, canonical, input)
    }

    fn source_epoch(
        namespace: PathObservationNamespace,
        root: &str,
        module: Option<&'static [u8]>,
    ) -> PathObservationEpoch {
        let path = |value: &str, operation| {
            PathObservationDemand::new(
                namespace,
                NormalizedAbsolutePath::new(value).unwrap(),
                operation,
            )
        };
        let lstat = |kind, stamp| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, stamp, 1, 1, 1, 0o755,
            )))
        };
        let mut observations = vec![
            (
                path("/", PathObservationOperation::Lstat),
                lstat(PathNodeKind::Directory, 1),
            ),
            (
                path(root, PathObservationOperation::Lstat),
                lstat(PathNodeKind::Directory, 2),
            ),
            (
                path(
                    &format!("{root}/BUILD.bazel"),
                    PathObservationOperation::Lstat,
                ),
                lstat(PathNodeKind::RegularFile, 3),
            ),
            (
                path(
                    &format!("{root}/BUILD.bazel"),
                    PathObservationOperation::FileBytes,
                ),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    &b"exports_files(['leaf.txt'])\n"[..],
                ))),
            ),
            (
                path(root, PathObservationOperation::DirectoryEntries),
                PathObservationResult::DirectoryEntries(PathOperationResult::Present(
                    PathDirectoryEntries::new([
                        PathDirectoryEntry::new(
                            PathDirectoryName::new("BUILD.bazel").unwrap(),
                            PathDirectoryEntryKind::File,
                        ),
                        PathDirectoryEntry::new(
                            PathDirectoryName::new("leaf.txt").unwrap(),
                            PathDirectoryEntryKind::File,
                        ),
                    ]),
                )),
            ),
            (
                path(
                    &format!("{root}/REPO.bazel"),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ),
            (
                path(
                    &format!("{root}/.bazelignore"),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ),
        ];
        if let Some(parent) = PathBuf::from(root)
            .parent()
            .filter(|path| *path != std::path::Path::new("/"))
        {
            observations.push((
                path(parent.to_str().unwrap(), PathObservationOperation::Lstat),
                lstat(PathNodeKind::Directory, 5),
            ));
        }
        if let Some(module) = module {
            observations.extend([
                (
                    path(
                        &format!("{root}/MODULE.bazel"),
                        PathObservationOperation::Lstat,
                    ),
                    lstat(PathNodeKind::RegularFile, 4),
                ),
                (
                    path(
                        &format!("{root}/MODULE.bazel"),
                        PathObservationOperation::FileBytes,
                    ),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        module,
                    ))),
                ),
            ]);
        }
        PathObservationEpoch::new(observations).unwrap()
    }

    async fn materialized_transaction(
        dice: &Arc<Dice>,
        input: &HostCanonicalRepositorySourceInput,
        tracker: Arc<DependencyTrace>,
        result: RepositoryMaterializationSuccess,
        namespace: PathObservationNamespace,
        root: &str,
        module: Option<&'static [u8]>,
    ) -> dice::DiceTransaction {
        let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
        else {
            panic!("selected registry input must retain a request")
        };
        request_transaction(
            dice,
            request.clone(),
            tracker,
            result,
            namespace,
            root,
            module,
        )
        .await
    }

    async fn request_transaction(
        dice: &Arc<Dice>,
        request: Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>,
        tracker: Arc<DependencyTrace>,
        result: RepositoryMaterializationSuccess,
        namespace: PathObservationNamespace,
        root: &str,
        module: Option<&'static [u8]>,
    ) -> dice::DiceTransaction {
        request_transaction_with_observations(
            dice,
            request,
            tracker,
            result,
            source_epoch(namespace, root, module),
        )
        .await
    }

    async fn request_transaction_with_observations(
        dice: &Arc<Dice>,
        request: Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>,
        tracker: Arc<DependencyTrace>,
        result: RepositoryMaterializationSuccess,
        observations: PathObservationEpoch,
    ) -> dice::DiceTransaction {
        let epoch = RepositoryMaterializationResultEpoch::new(
            request.id.workspace.clone(),
            [RepositoryMaterializationEpochEntry {
                request: request.clone(),
                result: RepositoryMaterializationResult::Success(result),
            }],
        )
        .unwrap();
        let mut updater = dice.updater_with_data(UserComputationData {
            activation_tracker: Some(tracker),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: request.id.workspace.clone(),
                },
                epoch,
            )])
            .unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, observations)])
            .unwrap();
        updater.commit().await
    }

    async fn prove_root_adapter_parity(
        tx: &mut dice::DiceTransaction,
        input: &HostCanonicalRepositorySourceInput,
        route: slug_bzlmod_v2::RootRepositoryRoute,
    ) {
        let relative = host_repository_relative_path(PathBuf::from("BUILD.bazel")).unwrap();
        let canonical_source = tx
            .compute(&HostRepositorySourceObservationEpochKey::new_canonical(
                input.clone(),
                relative.clone(),
            ))
            .await
            .unwrap();
        let ordinary_source = tx
            .compute(&HostRepositorySourceFileObservationKey::new(
                route.clone(),
                relative.as_path().to_owned(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(canonical_source)) = canonical_source else {
            panic!("canonical source must complete")
        };
        let SourcePreparationOutcome::Complete(Ok(ordinary_source)) = ordinary_source else {
            panic!("ordinary source must complete")
        };
        let canonical_value = canonical_source.result().as_ref().as_ref().unwrap();
        let HostRepositorySourceObservationView::Request(canonical_value) = canonical_value.view()
        else {
            panic!("non-builtin canonical source must retain a request value")
        };
        assert_eq!(
            canonical_value,
            ordinary_source.result().as_ref().as_ref().unwrap()
        );
        assert_eq!(
            canonical_source.observations(),
            ordinary_source.observations()
        );

        let canonical_listing = tx
            .compute(
                &HostRepositoryDirectoryListingObservationKey::new_canonical(
                    input.clone(),
                    PackagePath::root(),
                ),
            )
            .await
            .unwrap();
        let ordinary_listing = tx
            .compute(&HostRepositoryDirectoryListingObservationKey::new(
                route,
                PackagePath::root(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(canonical_listing)) = canonical_listing else {
            panic!("canonical listing must complete")
        };
        let SourcePreparationOutcome::Complete(Ok(ordinary_listing)) = ordinary_listing else {
            panic!("ordinary listing must complete")
        };
        assert_eq!(canonical_listing.result(), ordinary_listing.result());
        assert_eq!(
            canonical_listing.observations(),
            ordinary_listing.observations()
        );
    }

    async fn prove_alias_free_canonical_package_policy(
        tx: &mut dice::DiceTransaction,
        input: &HostCanonicalRepositorySourceInput,
        tracker: &DependencyTrace,
        source_key: &HostRepositorySourceObservationEpochKey,
        source: &ObservedHostRepositorySourceObservation,
    ) {
        let source_bytes = match source.result().as_ref().as_ref().unwrap().view() {
            HostRepositorySourceObservationView::Request(
                HostRepositorySourceFileValue::Present { bytes, .. },
            ) => bytes.dupe(),
            _ => panic!("registry BUILD source must be a present request result"),
        };
        assert_eq!(source_bytes.as_ref(), b"exports_files(['leaf.txt'])\n");
        let package = PackagePath::root();
        let boundary_key = HostExternalPackageBoundaryObservationKey::new_canonical(
            input.clone(),
            package.clone(),
        );
        let package_source_key = RepositoryPackageSourceObservationKey::new_canonical(
            input.clone(),
            PackageIdentifier::new(CanonicalRepoName::new("leaf+").unwrap(), package),
        )
        .unwrap();
        let boundary = tx.compute(&boundary_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(boundary)) = boundary else {
            panic!("alias-free canonical package boundary must complete")
        };
        let boundary_result = boundary.result().as_ref().unwrap();
        assert_eq!(
            boundary_result.kind(),
            HostExternalPackageBoundaryKind::Package
        );
        assert_eq!(
            boundary_result.selected_build_file_name(),
            Some("BUILD.bazel")
        );
        let package_source = tx.compute(&package_source_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(package_source)) = package_source else {
            panic!("alias-free canonical package source must complete")
        };
        let package_source_result = package_source.result().as_ref().as_ref().unwrap();
        assert_eq!(package_source_result.build_file_name(), "BUILD.bazel");
        assert_eq!(
            package_source_result.logical_path().as_path(),
            std::path::Path::new("/registry-leaf/BUILD.bazel")
        );
        assert!(Arc::ptr_eq(package_source_result.bytes(), &source_bytes));
        let expected_observations = PathObservationEpoch::from_shared(
            boundary
                .observations()
                .observations()
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .chain(
                    source
                        .observations()
                        .observations()
                        .iter()
                        .map(|(demand, result)| (demand.dupe(), result.dupe())),
                ),
        )
        .unwrap();
        assert_eq!(package_source.observations(), &expected_observations);
        let boundary_dependencies = tracker.dependencies(&boundary_key.to_string());
        assert_eq!(boundary_dependencies.len(), 1);
        assert!(
            boundary_dependencies[0].starts_with("observed-external-repository-package-lookup:")
        );
        let lookup_dependencies = tracker.dependencies(&boundary_dependencies[0]);
        assert_eq!(lookup_dependencies.len(), 3);
        assert!(lookup_dependencies[0].starts_with("canonical-deleted-packages:"));
        assert!(lookup_dependencies[1].starts_with("observed-host-route-repository-ignore:"));
        assert!(lookup_dependencies[2].starts_with("observed-host-repository-path:"));
        assert_eq!(
            tracker.dependencies(&package_source_key.to_string()),
            [boundary_dependencies[0].clone(), source_key.to_string()]
        );
        assert!(tracker.all_keys().iter().all(|key| {
            !key.contains("HostCanonicalRepositorySourceFile")
                && !key.contains("HostCanonicalRepositoryDirectoryListing")
                && !key.contains("root-repository-route")
                && !key.contains("leaf_from_parent")
        }));
    }

    async fn admitted_root_route(
        tx: &mut dice::DiceTransaction,
        apparent: &str,
    ) -> slug_bzlmod_v2::RootRepositoryRoute {
        let outcome = tx
            .compute(
                &RootRepositoryRouteKey::for_root_build(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                    ApparentRepoName::new(apparent).unwrap(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("root alias must be admitted before source epoch replacement")
        };
        value
            .as_ref()
            .as_ref()
            .unwrap_or_else(|error| panic!("root alias {apparent} rejected: {error:?}"))
            .clone()
    }

    #[tokio::test]
    async fn builtin_route_source_and_listing_use_only_catalog_drivers() {
        let tracker = Arc::new(DependencyTrace::default());
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut tx = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await;
        let workspace = NormalizedAbsolutePath::new("/canonical-builtin").unwrap();
        let canonical = CanonicalRepoName::new("bazel_tools").unwrap();
        let load_key = HostCanonicalRepositoryLoadRouteObservationKey::new(workspace, canonical);
        let load = tx.compute(&load_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(load)) = load else {
            panic!("built-in load route must complete")
        };
        let route = load.result().as_ref().as_ref().unwrap();
        assert_eq!(
            route.route().view().kind(),
            HostCanonicalRepositoryRouteKind::Builtin
        );
        assert!(load.observations().observations().is_empty());

        let source_key = HostRepositorySourceObservationEpochKey::new_canonical(
            route.input().clone(),
            host_repository_relative_path(PathBuf::from("MODULE.bazel")).unwrap(),
        );
        let listing_key = HostRepositoryDirectoryListingObservationKey::new_canonical(
            route.input().clone(),
            PackagePath::root(),
        );
        let source = tx.compute(&source_key).await.unwrap();
        let listing = tx.compute(&listing_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(source)) = source else {
            panic!("built-in canonical source must complete")
        };
        assert!(source.result().as_ref().is_ok());
        assert!(source.observations().observations().is_empty());
        let SourcePreparationOutcome::Complete(Ok(listing)) = listing else {
            panic!("built-in canonical listing must complete")
        };
        assert!(matches!(
            listing.result().as_ref(),
            Ok(PathDirectoryListing::Present(_))
        ));
        assert!(listing.observations().observations().is_empty());
        assert_eq!(
            tracker.dependencies(&source_key.to_string()),
            ["builtin-bazel-tools-source-file:MODULE.bazel"]
        );
        assert_eq!(
            tracker.dependencies(&listing_key.to_string()),
            ["builtin-bazel-tools-directory-listing:"]
        );
    }

    #[tokio::test]
    async fn local_and_root_adapter_share_materialization_path_and_listing_owners() {
        const LOCAL_MODULE: &str = "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostCanonicalRepositoryLoadRouteKey::new(
            workspace.clone(),
            CanonicalRepoName::new("local+").unwrap(),
        );
        let mut pending = transaction(&dice, LOCAL_MODULE, EXTENSION_A, true, None).await;
        let outcome = pending.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Need(need) = outcome else {
            panic!("local selected route must request its source")
        };
        let request = need
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .clone();
        let materialization = RepositoryMaterializationResultEpoch::new(
            workspace.clone(),
            [RepositoryMaterializationEpochEntry {
                request: request.clone(),
                result: RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Local,
                ),
            }],
        )
        .unwrap();
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.clone(),
                },
                materialization,
            )])
            .unwrap();
        let mut routed = updater.commit().await;
        let input = load_route(&routed.compute(&key).await.unwrap())
            .input()
            .clone();
        let root_route = admitted_root_route(&mut routed, "local_alias").await;
        let tracker = Arc::new(DependencyTrace::default());
        let root = format!("{WORKSPACE}/local");
        let mut tx = request_transaction(
            &dice,
            request,
            tracker.clone(),
            RepositoryMaterializationSuccess::Local,
            PathObservationNamespace::Host,
            &root,
            Some(b"module(name='local')\n"),
        )
        .await;
        assert_eq!(
            input.view().route().view().kind(),
            HostCanonicalRepositoryRouteKind::SelectedNonregistry
        );
        prove_root_adapter_parity(&mut tx, &input, root_route).await;
        let source_key = HostRepositorySourceObservationEpochKey::new_canonical(
            input.clone(),
            host_repository_relative_path(PathBuf::from("BUILD.bazel")).unwrap(),
        );
        let listing_key = HostRepositoryDirectoryListingObservationKey::new_canonical(
            input.clone(),
            PackagePath::root(),
        );
        let source_deps = tracker.dependencies(&source_key.to_string());
        let listing_deps = tracker.dependencies(&listing_key.to_string());
        assert!(
            source_deps
                .iter()
                .any(|dep| dep == "repository-materialization-result:@@local+")
        );
        assert!(
            source_deps
                .iter()
                .any(|dep| dep.starts_with("observed-resolved-path:Host:"))
        );
        assert!(
            listing_deps
                .iter()
                .any(|dep| dep.starts_with("observed-path-directory-listing:Host:"))
        );
        assert!(
            !tracker
                .all_keys()
                .iter()
                .any(|key| key.starts_with("host-selected-repository-file-effect:"))
        );
    }

    #[tokio::test]
    async fn admitted_registry_alias_matches_root_adapter_and_deepest_owners() {
        let (dice, input) = selected_input_for(LEAF_MAPPING_A, SOURCE_A, "parent+").await;
        let mut root_tx = transaction(&dice, ROOT_MODULE, EXTENSION_A, true, None).await;
        let root_route = admitted_root_route(&mut root_tx, "parent_alias").await;
        let tracker = Arc::new(DependencyTrace::default());
        let instance = PathObservationInstanceId::new(45);
        let mut tx = materialized_transaction(
            &dice,
            &input,
            tracker.clone(),
            RepositoryMaterializationSuccess::Immutable {
                source_identity: Arc::from("parent-source"),
                generation_root: PathBuf::from("/registry-parent"),
                observation_instance: instance,
            },
            PathObservationNamespace::Materialization(instance),
            "/registry-parent",
            None,
        )
        .await;
        prove_root_adapter_parity(&mut tx, &input, root_route).await;
        let source_key = HostRepositorySourceObservationEpochKey::new_canonical(
            input.clone(),
            host_repository_relative_path(PathBuf::from("BUILD.bazel")).unwrap(),
        );
        let listing_key =
            HostRepositoryDirectoryListingObservationKey::new_canonical(input, PackagePath::root());
        assert!(
            tracker
                .dependencies(&source_key.to_string())
                .iter()
                .any(|dep| dep == "repository-materialization-result:@@parent+")
        );
        assert!(
            tracker
                .dependencies(&listing_key.to_string())
                .iter()
                .any(|dep| dep.starts_with("observed-path-directory-listing:"))
        );
    }

    #[tokio::test]
    async fn generated_route_activates_effect_then_shares_canonical_source_owners() {
        const EXTENSION: &str = r#"
def materialize(ctx):
    ctx.file('BUILD.bazel', "exports_files(['leaf.txt'])\n")
    ctx.file('leaf.txt', 'generated-a\n')
repo=repository_rule(implementation=materialize)
def impl(ctx):
    repo(name='made')
ext=module_extension(implementation=impl)
"#;
        let tracker = Arc::new(DependencyTrace::default());
        let (dice, canonical, input) = generated_input(
            EXTENSION,
            Some(tracker.clone() as Arc<dyn ActivationTracker>),
        )
        .await;
        assert_eq!(
            input.view().route().view().kind(),
            HostCanonicalRepositoryRouteKind::Generated
        );
        let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
        else {
            panic!("generated route must retain a request")
        };
        assert!(matches!(
            request.kind,
            slug_bzlmod_v2::RepositoryMaterializationKind::GeneratedFileEffects(_)
        ));
        assert!(
            tracker
                .all_keys()
                .iter()
                .any(|key| key.starts_with("host-selected-repository-file-effect:"))
        );
        let instance = PathObservationInstanceId::new(46);
        let mut tx = materialized_transaction(
            &dice,
            &input,
            tracker.clone(),
            RepositoryMaterializationSuccess::Immutable {
                source_identity: Arc::from("generated-source"),
                generation_root: PathBuf::from("/generated-made"),
                observation_instance: instance,
            },
            PathObservationNamespace::Materialization(instance),
            "/generated-made",
            None,
        )
        .await;
        let source_key = HostRepositorySourceObservationEpochKey::new_canonical(
            input.clone(),
            host_repository_relative_path(PathBuf::from("BUILD.bazel")).unwrap(),
        );
        let listing_key = HostRepositoryDirectoryListingObservationKey::new_canonical(
            input.clone(),
            PackagePath::root(),
        );
        let source = tx.compute(&source_key).await.unwrap();
        let listing = tx.compute(&listing_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(source)) = source else {
            panic!("generated canonical source must complete")
        };
        let observed = source.result().as_ref().as_ref().unwrap();
        assert!(matches!(
            observed.view(),
            HostRepositorySourceObservationView::Request(
                HostRepositorySourceFileValue::Present { bytes, .. }
            ) if bytes.as_ref() == b"exports_files(['leaf.txt'])\n"
        ));
        let SourcePreparationOutcome::Complete(Ok(listing)) = listing else {
            panic!("generated canonical listing must complete")
        };
        assert!(matches!(
            listing.result().as_ref(),
            Ok(PathDirectoryListing::Present(_))
        ));
        assert!(
            tracker
                .dependencies(&source_key.to_string())
                .iter()
                .any(|dep| dep == &format!("repository-materialization-result:{canonical}"))
        );
        assert!(
            tracker
                .dependencies(&listing_key.to_string())
                .iter()
                .any(|dep| dep.starts_with("observed-path-directory-listing:"))
        );
        assert!(
            tracker
                .all_keys()
                .iter()
                .any(|key| key == &source_key.to_string())
        );

        for forbidden in [
            "host-external-package-boundary:",
            "external-subtree-package-set:",
            "root-subtree-package-set:",
            "repository-package-load:",
            "host-selected-registration-patterns:",
        ] {
            assert!(
                !tracker
                    .all_keys()
                    .iter()
                    .any(|key| key.starts_with(forbidden))
            );
        }
    }

    #[tokio::test]
    async fn transitive_registry_repo_is_canonical_only_and_reads_shared_source_owners() {
        let (dice, input) = selected_input(LEAF_MAPPING_A, SOURCE_A).await;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut root_tx = transaction(&dice, ROOT_MODULE, EXTENSION_A, true, None).await;
        let root_mapping = root_tx
            .compute(&HostRootRepositoryMappingKey::new(workspace.clone()))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(root_mapping) = root_mapping else {
            panic!("root mapping must complete")
        };
        let root_mapping = root_mapping.as_ref().as_ref().unwrap().view().unwrap();
        assert!(
            root_mapping
                .mapping()
                .all(|(apparent, _)| apparent.as_str() != "leaf_from_parent")
        );
        let root_route = root_tx
            .compute(
                &RootRepositoryRouteKey::new(
                    workspace,
                    ApparentRepoName::new("leaf_from_parent").unwrap(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert!(matches!(
            root_route,
            SourcePreparationOutcome::Complete(value) if value.as_ref().is_err()
        ));
        assert_eq!(
            input.view().route().view().canonical_repo().as_str(),
            "leaf+"
        );
        assert_eq!(
            input.view().route().view().kind(),
            HostCanonicalRepositoryRouteKind::SelectedRegistry
        );
        let tracker = Arc::new(DependencyTrace::default());
        let instance = PathObservationInstanceId::new(44);
        let mut tx = materialized_transaction(
            &dice,
            &input,
            tracker.clone(),
            RepositoryMaterializationSuccess::Immutable {
                source_identity: Arc::from("leaf-source-a"),
                generation_root: PathBuf::from("/registry-leaf"),
                observation_instance: instance,
            },
            PathObservationNamespace::Materialization(instance),
            "/registry-leaf",
            None,
        )
        .await;
        let source_key = HostRepositorySourceObservationEpochKey::new_canonical(
            input.clone(),
            host_repository_relative_path(PathBuf::from("BUILD.bazel")).unwrap(),
        );
        let listing_key = HostRepositoryDirectoryListingObservationKey::new_canonical(
            input.clone(),
            PackagePath::root(),
        );
        let source = tx.compute(&source_key).await.unwrap();
        let listing = tx.compute(&listing_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(source)) = source else {
            panic!("alias-free canonical source must complete")
        };
        prove_alias_free_canonical_package_policy(
            &mut tx,
            &input,
            tracker.as_ref(),
            &source_key,
            &source,
        )
        .await;
        let SourcePreparationOutcome::Complete(Ok(listing)) = listing else {
            panic!("alias-free canonical listing must complete")
        };
        let Ok(PathDirectoryListing::Present(entries)) = listing.result().as_ref() else {
            panic!("registry listing must be present")
        };
        assert_eq!(
            entries
                .entries()
                .iter()
                .map(|entry| entry.name().as_os_str())
                .collect::<Vec<_>>(),
            ["BUILD.bazel", "leaf.txt"]
        );
        let source_deps = tracker.dependencies(&source_key.to_string());
        assert!(
            source_deps
                .iter()
                .any(|dep| dep == "repository-materialization-result:@@leaf+")
        );
        assert!(
            source_deps
                .iter()
                .any(|dep| dep.starts_with("observed-resolved-path:"))
        );
        let listing_deps = tracker.dependencies(&listing_key.to_string());
        assert!(
            listing_deps
                .iter()
                .any(|dep| dep == "repository-materialization-result:@@leaf+")
        );
        assert!(
            listing_deps
                .iter()
                .any(|dep| dep.starts_with("observed-path-directory-listing:"))
        );
        assert!(
            tracker
                .all_keys()
                .iter()
                .any(|key| key == &source_key.to_string())
        );
    }

    #[tokio::test]
    async fn selected_spec_and_mapping_change_independently_with_a_b_a_restoration() {
        let first_outcome = selected_load_outcome(LEAF_MAPPING_A, SOURCE_A).await;
        let spec_outcome = selected_load_outcome(LEAF_MAPPING_A, SOURCE_B).await;
        let mapping_outcome = selected_load_outcome(LEAF_MAPPING_B, SOURCE_A).await;
        let restored_outcome = selected_load_outcome(LEAF_MAPPING_A, SOURCE_A).await;
        assert!(HostCanonicalRepositoryLoadRouteKey::equality(
            &first_outcome,
            &restored_outcome
        ));
        assert!(!HostCanonicalRepositoryLoadRouteKey::equality(
            &first_outcome,
            &spec_outcome
        ));
        assert!(!HostCanonicalRepositoryLoadRouteKey::equality(
            &first_outcome,
            &mapping_outcome
        ));
        let first = load_route(&first_outcome).input().clone();
        let spec_changed = load_route(&spec_outcome).input().clone();
        let mapping_changed = load_route(&mapping_outcome).input().clone();
        let restored = load_route(&restored_outcome).input().clone();
        for input in [&first, &spec_changed, &mapping_changed, &restored] {
            assert_eq!(
                input.view().route().view().canonical_repo().as_str(),
                "leaf+"
            );
        }
        assert_eq!(first, restored);
        assert_eq!(hash(&first), hash(&restored));
        assert_ne!(first, spec_changed);
        assert_ne!(hash(&first), hash(&spec_changed));
        assert_ne!(first, mapping_changed);
        assert_ne!(hash(&first), hash(&mapping_changed));
        let relative = host_repository_relative_path(PathBuf::from("BUILD.bazel")).unwrap();
        let first_key =
            HostRepositorySourceObservationEpochKey::new_canonical(first.clone(), relative.clone());
        let spec_key = HostRepositorySourceObservationEpochKey::new_canonical(
            spec_changed.clone(),
            relative.clone(),
        );
        let mapping_key = HostRepositorySourceObservationEpochKey::new_canonical(
            mapping_changed.clone(),
            relative.clone(),
        );
        let restored_key =
            HostRepositorySourceObservationEpochKey::new_canonical(restored.clone(), relative);
        assert_eq!(first_key, restored_key);
        assert_eq!(hash(&first_key), hash(&restored_key));
        assert_ne!(first_key, spec_key);
        assert_ne!(hash(&first_key), hash(&spec_key));
        assert_ne!(first_key, mapping_key);
        assert_ne!(hash(&first_key), hash(&mapping_key));
        let package = PackagePath::root();
        let package_identifier =
            PackageIdentifier::new(CanonicalRepoName::new("leaf+").unwrap(), package.clone());
        let first_boundary = HostExternalPackageBoundaryObservationKey::new_canonical(
            first.clone(),
            package.clone(),
        );
        let spec_boundary = HostExternalPackageBoundaryObservationKey::new_canonical(
            spec_changed.clone(),
            package.clone(),
        );
        let mapping_boundary = HostExternalPackageBoundaryObservationKey::new_canonical(
            mapping_changed.clone(),
            package.clone(),
        );
        let restored_boundary =
            HostExternalPackageBoundaryObservationKey::new_canonical(restored.clone(), package);
        assert_eq!(first_boundary, restored_boundary);
        assert_eq!(hash(&first_boundary), hash(&restored_boundary));
        assert_ne!(first_boundary, spec_boundary);
        assert_ne!(hash(&first_boundary), hash(&spec_boundary));
        assert_ne!(first_boundary, mapping_boundary);
        assert_ne!(hash(&first_boundary), hash(&mapping_boundary));
        let first_package =
            RepositoryPackageSourceKey::new_canonical(first.clone(), package_identifier.clone())
                .unwrap();
        let spec_package =
            RepositoryPackageSourceKey::new_canonical(spec_changed, package_identifier.clone())
                .unwrap();
        let mapping_package = RepositoryPackageSourceKey::new_canonical(
            mapping_changed.clone(),
            package_identifier.clone(),
        )
        .unwrap();
        let restored_package =
            RepositoryPackageSourceKey::new_canonical(restored, package_identifier).unwrap();
        assert_eq!(first_package, restored_package);
        assert_eq!(hash(&first_package), hash(&restored_package));
        assert_ne!(first_package, spec_package);
        assert_ne!(hash(&first_package), hash(&spec_package));
        assert_ne!(first_package, mapping_package);
        assert_ne!(hash(&first_package), hash(&mapping_package));
        assert_eq!(
            first
                .view()
                .route()
                .mapping_target(&ApparentRepoName::new("alias_a").unwrap())
                .unwrap()
                .as_str(),
            "mapped+"
        );
        assert_eq!(
            mapping_changed
                .view()
                .route()
                .mapping_target(&ApparentRepoName::new("alias_b").unwrap())
                .unwrap()
                .as_str(),
            "mapped+"
        );
    }

    #[tokio::test]
    async fn selected_route_terminal_stops_before_effect_and_projection() {
        let tracker = Arc::new(DependencyTrace::default());
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let module = "module(name='root')\nbazel_dep(name='missing', version='1')\n";
        let mut tx = transaction(
            &dice,
            module,
            EXTENSION_A,
            true,
            Some(tracker.clone() as Arc<dyn ActivationTracker>),
        )
        .await;
        let key = HostCanonicalRepositoryLoadRouteObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            CanonicalRepoName::root(),
        );
        let outcome = tx.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(value)) = outcome else {
            panic!("selected route failure must remain a semantic load terminal")
        };
        assert!(value.result().as_ref().is_err());
        assert!(!tracker.all_keys().iter().any(|name| {
            name.starts_with("observed-host-selected-repository-file-effect:")
                || name.starts_with("host-canonical-repository-source-file:")
        }));
    }

    #[tokio::test]
    async fn route_need_suppresses_effect_and_generated_error_preserves_route_effect_prefix() {
        const MODULE: &str = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, broken='broken')\n";
        const EXTENSION: &str = r#"
repo=repository_rule(implementation=lambda ctx: fail('effect failed'))
def impl(ctx):
    repo(name='broken')
ext=module_extension(implementation=impl)
"#;
        let tracker = Arc::new(DependencyTrace::default());
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut present = transaction(&dice, MODULE, EXTENSION, true, None).await;
        let canonical = names(&validated(&mut present).await).remove(0);
        let mut updater = dice.updater_with_data(UserComputationData {
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::empty(),
            )])
            .unwrap();
        let mut missing = updater.commit().await;
        let missing_key = HostCanonicalRepositoryLoadRouteObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            canonical,
        );
        assert!(matches!(
            missing.compute(&missing_key).await.unwrap(),
            SourcePreparationOutcome::Need(_)
        ));
        assert_no_activation(&tracker, "observed-host-selected-repository-file-effect:");

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(DependencyTrace::default());
        let mut tx = transaction(
            &dice,
            MODULE,
            EXTENSION,
            true,
            Some(tracker.clone() as Arc<dyn ActivationTracker>),
        )
        .await;
        let mut generated = names(&validated(&mut tx).await);
        let canonical = generated.remove(0);
        let load_key = HostCanonicalRepositoryLoadRouteObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            canonical.clone(),
        );
        let load = tx.compute(&load_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(load)) = load else {
            panic!("generated effect failure must remain a semantic load result")
        };
        assert!(
            load.result()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .is_effect_error()
        );
        assert!(
            tracker
                .all_keys()
                .iter()
                .any(|key| { key.starts_with("observed-host-selected-repository-file-effect:") })
        );
        assert_no_activation(&tracker, "host-canonical-repository-source-file:");
        let route = tx
            .compute(&HostCanonicalRepositoryRouteObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                canonical,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(route)) = route else {
            panic!("generated route predecessor must complete")
        };
        let route_value = route.result().as_ref().as_ref().unwrap();
        let seed = route_value.view().generated_effect_seed().unwrap();
        let effect = tx
            .compute(&HostSelectedRepositoryFileEffectObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                seed.owner().clone(),
                seed.ordinal(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(effect)) = effect else {
            panic!("semantic effect failure must retain its observed carrier")
        };
        assert!(effect.result().as_ref().is_err());
        let expected = PathObservationEpoch::from_shared(
            route
                .observations()
                .observations()
                .iter()
                .chain(effect.observations().observations().iter())
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap();
        assert_eq!(
            load.observations()
                .observations()
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .collect::<Vec<_>>(),
            expected
                .observations()
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn constructor_fail_closed_and_hash_table_covers_keys_dispositions_and_effect_plan() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let builtin = Arc::new(slug_bzlmod_v2::HostCanonicalRepositoryRoute::builtin(
            workspace.clone(),
        ));
        let builtin_input = host_canonical_repository_source_input(builtin.clone(), None).unwrap();
        let empty_plan = GeneratedRepositoryFileEffectPlan::build(std::iter::empty::<(
            CompactString,
            Arc<[u8]>,
            bool,
        )>())
        .unwrap();
        assert!(host_canonical_repository_source_input(builtin.clone(), Some(empty_plan)).is_err());

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let root = tx
            .compute(&super::super::HostCanonicalRepositoryRouteKey::new(
                workspace.clone(),
                CanonicalRepoName::root(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(root) = root else {
            panic!("root route must complete")
        };
        assert!(
            host_canonical_repository_source_input(
                Arc::new(root.as_ref().as_ref().unwrap().clone()),
                None,
            )
            .is_err()
        );

        let (_, selected) = selected_input(LEAF_MAPPING_A, SOURCE_A).await;
        assert!(
            host_canonical_repository_source_input(
                selected.view().route().clone(),
                Some(
                    GeneratedRepositoryFileEffectPlan::build(std::iter::empty::<(
                        CompactString,
                        Arc<[u8]>,
                        bool
                    )>(),)
                    .unwrap()
                ),
            )
            .is_err()
        );

        const EXTENSION: &str = r#"
repo=repository_rule(implementation=lambda ctx: ctx.file('value', 'a'))
def impl(ctx): repo(name='made')
ext=module_extension(implementation=impl)
"#;
        let (_, _, generated) = generated_input(EXTENSION, None).await;
        assert!(
            host_canonical_repository_source_input(generated.view().route().clone(), None,)
                .is_err()
        );
        let plan = |bytes: &'static [u8]| {
            GeneratedRepositoryFileEffectPlan::build([(
                CompactString::new("value"),
                Arc::from(bytes),
                false,
            )])
            .unwrap()
        };
        let generated_a = host_canonical_repository_source_input(
            generated.view().route().clone(),
            Some(plan(b"a")),
        )
        .unwrap();
        let generated_b = host_canonical_repository_source_input(
            generated.view().route().clone(),
            Some(plan(b"b")),
        )
        .unwrap();
        let generated_restored = host_canonical_repository_source_input(
            generated.view().route().clone(),
            Some(plan(b"a")),
        )
        .unwrap();
        assert_ne!(generated_a, generated_b);
        assert_ne!(hash(&generated_a), hash(&generated_b));
        assert_eq!(generated_a, generated_restored);
        assert_eq!(hash(&generated_a), hash(&generated_restored));
        assert_ne!(generated_a, selected);
        assert_ne!(hash(&generated_a), hash(&selected));
        assert_ne!(builtin_input, selected);
        assert_ne!(hash(&builtin_input), hash(&selected));
        assert_ne!(builtin_input, generated_a);
        assert_ne!(hash(&builtin_input), hash(&generated_a));

        let key = |workspace, canonical| {
            HostCanonicalRepositoryLoadRouteKey::new(
                NormalizedAbsolutePath::new(workspace).unwrap(),
                CanonicalRepoName::new(canonical).unwrap(),
            )
        };
        let first = key("/identity-a", "same+");
        let workspace_changed = key("/identity-b", "same+");
        let canonical_changed = key("/identity-a", "other+");
        assert_ne!(first, workspace_changed);
        assert_ne!(hash(&first), hash(&workspace_changed));
        assert_ne!(first, canonical_changed);
        assert_ne!(hash(&first), hash(&canonical_changed));
        assert_eq!(first, key("/identity-a", "same+"));
        assert_eq!(hash(&first), hash(&key("/identity-a", "same+")));
    }

    #[tokio::test]
    async fn cancellation_publishes_no_load_route_and_recovery_completes() {
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, MODULE, EXTENSION_A, true, None).await;
        let canonical = names(&validated(&mut prep_tx).await).remove(0);
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(DependencyTrace::default());
        let key = HostCanonicalRepositoryLoadRouteObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            canonical,
        );
        let mut cancelled = transaction(
            &dice,
            MODULE,
            EXTENSION_A,
            true,
            Some(tracker.clone() as Arc<dyn ActivationTracker>),
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
        assert!(
            !tracker
                .all_keys()
                .iter()
                .any(|name| name == &key.to_string())
        );

        let mut recovered = transaction(
            &dice,
            MODULE,
            EXTENSION_A,
            true,
            Some(tracker.clone() as Arc<dyn ActivationTracker>),
        )
        .await;
        let outcome = recovered.compute(&key).await.unwrap();
        assert!(matches!(outcome, SourcePreparationOutcome::Complete(Ok(_))));
        assert!(
            tracker
                .all_keys()
                .iter()
                .any(|name| name == &key.to_string())
        );
    }

    #[tokio::test]
    async fn cancellation_publishes_no_canonical_package_source_and_recovery_completes() {
        let (dice, input) = selected_input(LEAF_MAPPING_A, SOURCE_A).await;
        let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
        else {
            panic!("selected registry input must retain a request")
        };
        let tracker = Arc::new(DependencyTrace::default());
        let instance = PathObservationInstanceId::new(46);
        let key = RepositoryPackageSourceObservationKey::new_canonical(
            input.clone(),
            PackageIdentifier::new(
                CanonicalRepoName::new("leaf+").unwrap(),
                PackagePath::root(),
            ),
        )
        .unwrap();
        let materialization = || RepositoryMaterializationSuccess::Immutable {
            source_identity: Arc::from("leaf-source-cancellation"),
            generation_root: PathBuf::from("/registry-leaf-cancellation"),
            observation_instance: instance,
        };
        let mut cancelled = request_transaction_with_observations(
            &dice,
            request.clone(),
            tracker.clone(),
            materialization(),
            PathObservationEpoch::empty(),
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
        assert!(
            !tracker
                .all_keys()
                .iter()
                .any(|name| name == &key.to_string())
        );

        let mut recovered = request_transaction(
            &dice,
            request.clone(),
            tracker.clone(),
            materialization(),
            PathObservationNamespace::Materialization(instance),
            "/registry-leaf-cancellation",
            None,
        )
        .await;
        let outcome = recovered.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(source)) = outcome else {
            panic!("recovered canonical package-source chain must complete")
        };
        assert!(source.result().is_ok());
        assert!(
            tracker
                .all_keys()
                .iter()
                .any(|name| name == &key.to_string())
        );
        let dependencies = tracker.dependencies(&key.to_string());
        assert_eq!(dependencies.len(), 2);
        assert!(dependencies[0].starts_with("observed-external-repository-package-lookup:"));
        assert!(dependencies[1].starts_with("observed-host-repository-source-observation:"));
    }

    #[test]
    fn keys_are_complete_only_and_retained_shapes_stay_compact_and_apparent_free() {
        assert!(!HostCanonicalRepositoryLoadRouteKey::validity(
            &SourcePreparationOutcome::Need(synthetic_need())
        ));
        assert!(!HostRepositorySourceObservationEpochKey::validity(
            &SourcePreparationOutcome::Need(synthetic_need())
        ));
        assert!(!HostRepositoryDirectoryListingObservationKey::validity(
            &SourcePreparationOutcome::Need(synthetic_need())
        ));
        assert!(std::mem::size_of::<HostCanonicalRepositorySourceInput>() <= 128);
        assert!(
            std::mem::size_of::<HostRepositorySourceRoute>() <= 256,
            "source route size: {}",
            std::mem::size_of::<HostRepositorySourceRoute>()
        );
        assert!(std::mem::size_of::<HostCanonicalRepositoryLoadRoute>() <= 128);
        assert!(std::mem::size_of::<HostRepositorySourceObservationInput>() <= 192);
        assert!(std::mem::size_of::<HostRepositorySourceObservationEpochKey>() <= 224);
        assert!(std::mem::size_of::<ObservedHostRepositorySourceObservation>() <= 48);
        assert!(std::mem::size_of::<HostExternalPackageBoundaryObservationKey>() <= 320);
        assert!(std::mem::size_of::<RepositoryPackageSourceKey>() <= 320);
        assert!(std::mem::size_of::<ObservedHostExternalPackageBoundary>() <= 48);
        assert!(std::mem::size_of::<ObservedRepositoryPackageSource>() <= 48);
        let source = include_str!(
            "../../slug_bzlmod_v2/src/source_preparation/canonical_repository_source.rs"
        );
        let route = include_str!("canonical_repository_load_route.rs");
        assert!(source.contains("route: Arc<HostCanonicalRepositoryRoute>"));
        assert!(source.contains("disposition: HostRepositoryMaterializationDisposition"));
        for forbidden in [
            "apparent_repo:",
            "mapping:",
            "physical_root",
            "source_bytes",
            "HostCanonicalRepositorySourceFileKey",
            "HostCanonicalRepositoryDirectoryListingKey",
        ] {
            assert!(!source.contains(forbidden));
        }
        assert!(source.contains("pub enum HostRepositorySourceRoute"));
        assert!(source.contains("Root(RootRepositoryRoute)"));
        assert!(source.contains("Canonical(HostCanonicalRepositorySourceInput)"));
        assert!(source.contains("HostRepositorySourceObservationKey::new("));
        assert!(source.contains("HostRepositorySourceObservationKey::new_canonical("));
        assert!(route.contains("compute_route_predecessor(ctx, key, mode)"));
        assert!(route.contains("compute_generated_effect("));
        for forbidden in ["RootRepositoryRouteKey", "Mutex", "RwLock", "OnceLock"] {
            assert!(!route.contains(forbidden));
        }
    }

    #[test]
    fn synthetic_need_values_are_transient_and_never_equal() {
        let need = SourcePreparationOutcome::<
            Arc<Result<HostCanonicalRepositoryLoadRoute, HostCanonicalRepositoryLoadRouteError>>,
        >::Need(synthetic_need());
        assert!(!HostCanonicalRepositoryLoadRouteKey::validity(&need));
        assert!(!HostCanonicalRepositoryLoadRouteKey::equality(&need, &need));
    }

    fn synthetic_need() -> SourcePreparationNeeds {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/need").unwrap(),
            PathObservationOperation::Lstat,
        )))
    }
}
