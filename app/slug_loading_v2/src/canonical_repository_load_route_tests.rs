/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::BTreeMap;
    use std::collections::hash_map::DefaultHasher;
    use std::fmt;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    use allocative::Allocative;
    use async_trait::async_trait;
    use compact_str::CompactString;
    use dice::ActivationData;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DiceComputations;
    use dice::DynKey;
    use dice::Key;
    use dice::UserComputationData;
    use dice_futures::cancellation::CancellationContext;
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
    use slug_bzlmod_v2::RepositoryHostInputTransaction;
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_bzlmod_v2::RepositoryPackageSource;
    use slug_bzlmod_v2::RepositoryPackageSourceAddress;
    use slug_bzlmod_v2::RepositoryPackageSourceKey;
    use slug_bzlmod_v2::RepositoryPackageSourceObservationKey;
    use slug_bzlmod_v2::RepositoryPlatform;
    use slug_bzlmod_v2::RepositoryPlatformKey;
    use slug_bzlmod_v2::RootPackageBzlTarget;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_bzlmod_v2::RootRepositoryBzlLoadRoute;
    use slug_bzlmod_v2::RootRepositoryRouteKey;
    use slug_bzlmod_v2::SourcePreparationNeeds;
    use slug_bzlmod_v2::SourcePreparationOutcome;
    use slug_bzlmod_v2::host_canonical_repository_source_input;
    use slug_bzlmod_v2::host_repository_relative_path;
    use slug_bzlmod_v2::host_repository_source_input;
    use slug_bzlmod_v2::inject_root_package_policy_inputs;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalLabel;
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
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::super::HostCanonicalRepositoryRouteObservationKey;
    use super::super::HostSelectedRepositoryFileEffectObservationKey;
    use crate::ObservedRepositoryPackageLoad;
    use crate::PackageTargetKind;
    use crate::RepositoryPackageLoadKey;
    use crate::RepositoryPackageLoadObservationKey;
    use crate::bzl_module::BzlLoadManifest;
    use crate::bzl_module::BzlModuleIdentity;
    use crate::bzl_module::ExternalBzlCycleIdentity;
    use crate::bzl_module::ExternalBzlModuleError;
    use crate::bzl_module::ExternalBzlModuleEvalKey;
    use crate::bzl_module::ExternalBzlModuleObservationKey;
    use crate::bzl_module::ObservedExternalBzlModule;
    use crate::bzl_module::RepositoryBzlLabel;
    use crate::canonical_repository_load_route::*;
    use crate::canonical_repository_route_tests::tests::EXTENSION_A;
    use crate::canonical_repository_route_tests::tests::MODULE;
    use crate::canonical_repository_route_tests::tests::WORKSPACE;
    use crate::canonical_repository_route_tests::tests::builtin_graph_dice;
    use crate::canonical_repository_route_tests::tests::builtin_graph_module;
    use crate::canonical_repository_route_tests::tests::names;
    use crate::canonical_repository_route_tests::tests::transaction;
    use crate::canonical_repository_route_tests::tests::validated;
    use crate::cycle_detector::bzl_load_cycle_detector;
    use crate::external_subtree_package_set::ExternalSubtreePackageSetKey;
    use crate::external_subtree_package_set::ExternalSubtreePackageSetObservationKey;
    use crate::external_subtree_package_set::ObservedExternalSubtreePackageSet;
    use crate::glob::GlobPattern;
    use crate::host_glob::HostGlobBoundaryScope;
    use crate::host_glob::HostGlobLoadingOperation;
    use crate::host_glob::HostGlobLoadingRequest;
    use crate::host_glob::HostGlobRequestInputError;
    use crate::host_glob::HostGlobRequestOutcome;
    use crate::host_glob::compute_host_glob_request;
    use crate::package::loading_globals;
    use crate::provider::BzlEvaluationContext;

    pub(crate) const ROOT_MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='parent', version='1', repo_name='parent_alias')\n";
    pub(crate) const PARENT_MODULE: &[u8] =
        b"module(name='parent', version='1')\nbazel_dep(name='leaf', version='1', repo_name='leaf_from_parent')\n";
    const GENERATED_PARENT_MODULE: &[u8] = b"module(name='parent', version='1')\ne=use_extension('//:compatibility.bzl','compatibility')\nuse_repo(e, compatibility_repo='compatibility_repo')\n";
    const GENERATED_PARENT_EXTENSION: &[u8] = br#"
repo=repository_rule(implementation=lambda ctx: None)
def impl(ctx): repo(name='compatibility_repo')
compatibility=module_extension(implementation=impl)
"#;
    pub(crate) const LEAF_MAPPING_A: &[u8] = b"module(name='leaf', version='1')\nbazel_dep(name='mapped', version='1', repo_name='alias_a')\n";
    const LEAF_MAPPING_B: &[u8] = b"module(name='leaf', version='1')\nbazel_dep(name='mapped', version='1', repo_name='alias_b')\n";
    const MAPPED_MODULE: &[u8] = b"module(name='mapped', version='1')\n";
    pub(crate) const SOURCE_A: &[u8] =
        br#"{"url":"https://origin.invalid/leaf-a.tgz","integrity":"sha256-a"}"#;
    const SOURCE_B: &[u8] =
        br#"{"url":"https://origin.invalid/leaf-b.tgz","integrity":"sha256-b"}"#;
    const OTHER_SOURCE: &[u8] =
        br#"{"url":"https://origin.invalid/source.tgz","integrity":"sha256-source"}"#;
    const OBSERVED_PARENT_SOURCE: &str = "observed-host-repository-source-observation:@@parent+";
    const OBSERVED_MISSING_SOURCE: &str = "observed-host-repository-source-observation:@@missing";

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

        fn clear(&self) {
            self.0.lock().unwrap().clear();
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
    struct CatalogGlobProbeKey {
        workspace: NormalizedAbsolutePath,
        input: HostCanonicalRepositorySourceInput,
        package: PackagePath,
        request: HostGlobLoadingRequest,
        observed: bool,
    }

    impl fmt::Display for CatalogGlobProbeKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "catalog-glob-probe:{}//{}", self.workspace, self.package)
        }
    }

    #[async_trait]
    impl Key for CatalogGlobProbeKey {
        type Value = Result<HostGlobRequestOutcome, HostGlobRequestInputError>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _: &CancellationContext,
        ) -> Self::Value {
            compute_host_glob_request(
                ctx,
                self.workspace.dupe(),
                HostGlobBoundaryScope::BuiltinCatalog(HostRepositorySourceRoute::Canonical(
                    self.input.clone(),
                )),
                self.workspace.dupe(),
                self.package.clone(),
                self.request.dupe(),
                self.observed,
            )
            .await
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            match (x, y) {
                (Ok(x), Ok(y)) => x.complete_eq(y),
                (Err(x), Err(y)) => x == y,
                _ => false,
            }
        }

        fn validity(value: &Self::Value) -> bool {
            match value {
                Ok(value) => value.is_complete(),
                Err(_) => true,
            }
        }
    }

    fn assert_no_activation(tracker: &DependencyTrace, prefix: &str) {
        assert!(!tracker.all_keys().iter().any(|key| key.starts_with(prefix)));
    }

    fn registry_io(
        parent_module: &'static [u8],
        leaf_module: &'static [u8],
        leaf_source: &'static [u8],
    ) -> StaticRegistryIo {
        let mut files = BTreeMap::from([(
            "https://registry.invalid/bazel_registry.json".to_owned(),
            Arc::from(&b"{}"[..]),
        )]);
        for (name, module, source) in [
            ("parent", parent_module, OTHER_SOURCE),
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

    pub(crate) fn registry_dice(
        parent_module: &'static [u8],
        leaf_module: &'static [u8],
        leaf_source: &'static [u8],
    ) -> Arc<Dice> {
        let mut builder = Dice::builder();
        slug_bzlmod_v2::install_registry_io(
            &mut builder,
            Arc::new(registry_io(parent_module, leaf_module, leaf_source)),
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

    fn assert_a_b_a<T: Eq + Hash + std::fmt::Debug>(a: &T, b: &T, restored: &T) {
        assert_ne!(a, b);
        assert_ne!(hash(a), hash(b));
        assert_eq!(a, restored);
        assert_eq!(hash(a), hash(restored));
    }

    fn assert_allocative<T: allocative::Allocative>() {}

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
        let dice = registry_dice(PARENT_MODULE, leaf_module, leaf_source);
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
        let dice = registry_dice(PARENT_MODULE, leaf_module, leaf_source);
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

    async fn requests_transaction_with_observations(
        dice: &Arc<Dice>,
        requests: impl IntoIterator<
            Item = (
                Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>,
                RepositoryMaterializationSuccess,
            ),
        >,
        tracker: Arc<DependencyTrace>,
        observations: PathObservationEpoch,
    ) -> dice::DiceTransaction {
        let entries = requests
            .into_iter()
            .map(|(request, result)| RepositoryMaterializationEpochEntry {
                request,
                result: RepositoryMaterializationResult::Success(result),
            })
            .collect::<Vec<_>>();
        let workspace = entries[0].request.id.workspace.clone();
        let epoch = RepositoryMaterializationResultEpoch::new(workspace.clone(), entries).unwrap();
        let mut user_data = UserComputationData {
            cycle_detector: Some(bzl_load_cycle_detector()),
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        user_data
            .data
            .set(RepositoryHostInputTransaction::default());
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(
                RepositoryPlatformKey::new(workspace.clone()),
                RepositoryPlatform::new("linux", "x86_64"),
            )])
            .unwrap();
        inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                workspace.clone(),
                Arc::from([workspace.clone()]),
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey { workspace },
                epoch,
            )])
            .unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, observations)])
            .unwrap();
        updater.commit().await
    }

    async fn request_transaction_with_observations(
        dice: &Arc<Dice>,
        request: Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>,
        tracker: Arc<DependencyTrace>,
        result: RepositoryMaterializationSuccess,
        observations: PathObservationEpoch,
    ) -> dice::DiceTransaction {
        requests_transaction_with_observations(dice, [(request, result)], tracker, observations)
            .await
    }

    fn canonical_bzl_key(
        input: HostCanonicalRepositorySourceInput,
        target: &str,
    ) -> ExternalBzlModuleObservationKey {
        ExternalBzlModuleObservationKey::new_canonical(
            input,
            RepositoryBzlLabel::new(
                PackagePath::root(),
                RootPackageBzlTarget::parse(target).unwrap(),
            )
            .unwrap(),
        )
    }

    fn bzl_source_epoch(
        namespace: PathObservationNamespace,
        root: &str,
        files: &[(&str, Option<&'static [u8]>)],
    ) -> PathObservationEpoch {
        let demand = |value: &str, operation| {
            PathObservationDemand::new(
                namespace,
                NormalizedAbsolutePath::new(value).unwrap(),
                operation,
            )
        };
        let present = |kind, stamp| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, stamp, stamp, stamp, stamp, 0o755,
            )))
        };
        let mut observations = vec![
            (
                demand("/", PathObservationOperation::Lstat),
                present(PathNodeKind::Directory, 1),
            ),
            (
                demand(root, PathObservationOperation::Lstat),
                present(PathNodeKind::Directory, 2),
            ),
        ];
        for (index, (relative, bytes)) in files.iter().enumerate() {
            let path = format!("{root}/{relative}");
            observations.push((
                demand(&path, PathObservationOperation::Lstat),
                bytes.map_or(
                    PathObservationResult::Lstat(PathOperationResult::Missing),
                    |_| present(PathNodeKind::RegularFile, index as i64 + 3),
                ),
            ));
            if let Some(bytes) = bytes {
                observations.push((
                    demand(&path, PathObservationOperation::FileBytes),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        *bytes,
                    ))),
                ));
            }
        }
        PathObservationEpoch::new(observations).unwrap()
    }

    fn builtin_rules_cc_source_epoch(
        instance: PathObservationInstanceId,
        utility: &'static [u8],
    ) -> PathObservationEpoch {
        let namespace = PathObservationNamespace::Materialization(instance);
        let demand = |path: &str| {
            PathObservationDemand::new(
                namespace,
                NormalizedAbsolutePath::new(path).unwrap(),
                PathObservationOperation::Lstat,
            )
        };
        let directories = PathObservationEpoch::from_shared(
            [
                "/registry-rules-cc/cc",
                "/registry-rules-cc/cc/toolchains",
                "/registry-rules-cc/cc/private",
                "/registry-rules-cc/cc/private/toolchain",
            ]
            .into_iter()
            .enumerate()
            .map(|(index, path)| {
                (
                    demand(path),
                    Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                        PathLstat::new(PathNodeKind::Directory, index as i64 + 20, 1, 1, 1, 0o755),
                    ))),
                )
            }),
        )
        .unwrap();
        merge_observations(
            &directories,
            &bzl_source_epoch(
                namespace,
                "/registry-rules-cc",
                &[
                    (
                        "cc/toolchains/toolchain_config_utils.bzl",
                        Some(utility),
                    ),
                    (
                        "cc/private/toolchain/escape.bzl",
                        Some(
                            b"CONTEXT = native.bazel_version\ndef escape_string(value): return str(value).replace('%', '%%')\n",
                        ),
                    ),
                ],
            ),
        )
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
            package_source_result.address(),
            &RepositoryPackageSourceAddress::Host(
                NormalizedAbsolutePath::new("/registry-leaf/BUILD.bazel").unwrap()
            )
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
        let package_load_key =
            RepositoryPackageLoadObservationKey::new_canonical(input.clone(), PackagePath::root());
        let package_load = tx.compute(&package_load_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(package_load)) = package_load else {
            panic!("alias-free canonical package load must complete")
        };
        let loaded = package_load.result().as_ref().as_ref().unwrap();
        assert_eq!(
            loaded
                .targets
                .iter()
                .map(|target| target.name.as_str())
                .collect::<Vec<_>>(),
            ["leaf.txt"]
        );
        assert_eq!(
            loaded.build_file,
            PathBuf::from("<output_base>/external/leaf+/BUILD.bazel")
        );
        assert_eq!(package_load.observations(), &expected_observations);
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

    fn external_label(package: &str, target: &str) -> RepositoryBzlLabel {
        RepositoryBzlLabel::new(
            if package.is_empty() {
                PackagePath::root()
            } else {
                PackagePath::parse(package).unwrap()
            },
            RootPackageBzlTarget::parse(target).unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn root_request_external_bzl_preserves_exact_source_children() {
        let dice = registry_dice(PARENT_MODULE, LEAF_MAPPING_A, SOURCE_A);
        let mut root_tx = transaction(&dice, ROOT_MODULE, EXTENSION_A, true, None).await;
        let route = admitted_root_route(&mut root_tx, "parent_alias").await;
        let input = host_repository_source_input(route.source_capability()).unwrap();
        let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
        else {
            panic!("selected root route must retain a source request")
        };
        let tracker = Arc::new(DependencyTrace::default());
        let instance = PathObservationInstanceId::new(76);
        let mut tx = requests_transaction_with_observations(
            &dice,
            [(
                request.clone(),
                RepositoryMaterializationSuccess::Immutable {
                    source_identity: Arc::from("root-request-bzl-source"),
                    generation_root: PathBuf::from("/registry-parent"),
                    observation_instance: instance,
                },
            )],
            tracker.clone(),
            bzl_source_epoch(
                PathObservationNamespace::Materialization(instance),
                "/registry-parent",
                &[("defs.bzl", Some(b"VALUE = 1\n"))],
            ),
        )
        .await;
        let label = external_label("", "defs.bzl");
        let legacy_key = ExternalBzlModuleEvalKey::new(route.clone(), label.clone());
        let observed_key = ExternalBzlModuleObservationKey::new(route, label);

        let SourcePreparationOutcome::Complete(legacy) = tx.compute(&legacy_key).await.unwrap()
        else {
            panic!("root request legacy module must complete")
        };
        let legacy = legacy.as_ref().as_ref().unwrap();
        assert_eq!(
            legacy.manifest.root.workspace_path,
            PathBuf::from("/registry-parent/defs.bzl")
        );
        let SourcePreparationOutcome::Complete(Ok(observed)) =
            tx.compute(&observed_key).await.unwrap()
        else {
            panic!("root request observed module must complete")
        };
        let observed_module = observed.result().as_ref().as_ref().unwrap();
        assert_eq!(observed_module.manifest.root, legacy.manifest.root);
        assert!(!observed.observations().observations().is_empty());

        let source_key = "host-repository-source-file:@@parent+:defs.bzl";
        let observed_source_key = "observed-host-repository-source-file:@@parent+:defs.bzl";
        assert_eq!(
            tracker.dependencies(&legacy_key.to_string()),
            [source_key.to_owned()]
        );
        assert_eq!(
            tracker.dependencies(&observed_key.to_string()),
            [observed_source_key.to_owned()]
        );
        assert_eq!(
            tracker.dependencies(source_key),
            [
                "host-repository-path:@@parent+:defs.bzl".to_owned(),
                "repository-materialization-result:@@parent+".to_owned(),
                "path-observation:Materialization(PathObservationInstanceId { value: 76 }):\"/registry-parent/defs.bzl\":FileBytes".to_owned(),
            ]
        );
        assert_eq!(
            tracker.dependencies(observed_source_key),
            [
                "observed-host-repository-path:@@parent+:defs.bzl".to_owned(),
                "repository-materialization-result:@@parent+".to_owned(),
                "path-observation:Materialization(PathObservationInstanceId { value: 76 }):\"/registry-parent/defs.bzl\":FileBytes".to_owned(),
            ]
        );
        assert_no_activation(&tracker, "host-repository-source-observation:@@parent+");
        assert_no_activation(&tracker, OBSERVED_PARENT_SOURCE);
        assert_no_activation(&tracker, "host-canonical-repository-load-route:");
        assert_no_activation(&tracker, "observed-host-canonical-repository-load-route:");
    }

    #[tokio::test]
    async fn selected_child_load_stays_root_and_missing_mapping_activates_no_canonical_route() {
        let dice = registry_dice(PARENT_MODULE, LEAF_MAPPING_A, SOURCE_A);
        let mut root_tx = transaction(&dice, ROOT_MODULE, EXTENSION_A, true, None).await;
        let route = admitted_root_route(&mut root_tx, "parent_alias").await;
        let parent_input = host_repository_source_input(route.source_capability()).unwrap();
        let HostRepositorySourceInputDispositionView::Request(parent_request) =
            parent_input.view().disposition()
        else {
            panic!("selected parent must retain a source request")
        };
        let RootRepositoryBzlLoadRoute::Root(leaf_route) = route
            .selected_bzl_load_route(&ApparentRepoName::new("leaf_from_parent").unwrap())
            .unwrap()
        else {
            panic!("selected dependency must retain the direct Root route")
        };
        let leaf_input = host_repository_source_input(leaf_route.source_capability()).unwrap();
        let HostRepositorySourceInputDispositionView::Request(leaf_request) =
            leaf_input.view().disposition()
        else {
            panic!("selected child must retain a source request")
        };
        let host = root_tx.compute(&PathObservationEpochKey).await.unwrap();
        let parent_instance = PathObservationInstanceId::new(77);
        let leaf_instance = PathObservationInstanceId::new(78);
        let parent = bzl_source_epoch(
            PathObservationNamespace::Materialization(parent_instance),
            "/registry-parent",
            &[
                (
                    "selected.bzl",
                    Some(b"load('@leaf_from_parent//:child.bzl', 'VALUE')\nRESULT = VALUE\n"),
                ),
                (
                    "missing.bzl",
                    Some(b"load('@missing//:child.bzl', 'VALUE')\n"),
                ),
            ],
        );
        let leaf = bzl_source_epoch(
            PathObservationNamespace::Materialization(leaf_instance),
            "/registry-leaf",
            &[("child.bzl", Some(b"VALUE = 2\n"))],
        );
        let observations = merge_observations(&merge_observations(&host, &parent), &leaf);
        let tracker = Arc::new(DependencyTrace::default());
        let immutable = |source, root, instance| RepositoryMaterializationSuccess::Immutable {
            source_identity: Arc::from(source),
            generation_root: PathBuf::from(root),
            observation_instance: instance,
        };
        let mut tx = requests_transaction_with_observations(
            &dice,
            [
                (
                    parent_request.clone(),
                    immutable("selected-parent", "/registry-parent", parent_instance),
                ),
                (
                    leaf_request.clone(),
                    immutable("selected-child", "/registry-leaf", leaf_instance),
                ),
            ],
            tracker.clone(),
            observations,
        )
        .await;

        let missing_key =
            ExternalBzlModuleObservationKey::new(route.clone(), external_label("", "missing.bzl"));
        let SourcePreparationOutcome::Complete(Ok(missing)) =
            tx.compute(&missing_key).await.unwrap()
        else {
            panic!("missing mapping must complete with a semantic error")
        };
        assert!(matches!(
            missing.result().as_ref().as_ref().unwrap_err(),
            ExternalBzlModuleError::LoadLabel { .. }
        ));
        let selected_key =
            ExternalBzlModuleObservationKey::new(route, external_label("", "selected.bzl"));
        let SourcePreparationOutcome::Complete(Ok(selected)) =
            tx.compute(&selected_key).await.unwrap()
        else {
            panic!("selected dependency load must complete")
        };
        let selected = selected.result().as_ref().as_ref().unwrap();
        assert_eq!(
            selected.manifest.direct_children[0].label,
            CanonicalLabel::parse("@@leaf+//:child.bzl").unwrap()
        );
        let child_key = "observed-external-bzl-module:@@leaf+//:child.bzl";
        assert!(
            tracker
                .dependencies(&selected_key.to_string())
                .iter()
                .any(|dependency| dependency == child_key)
        );
        assert!(
            tracker
                .all_keys()
                .contains(&"observed-host-repository-source-file:@@leaf+:child.bzl".to_owned())
        );
        assert_no_activation(&tracker, OBSERVED_MISSING_SOURCE);
        assert_no_activation(&tracker, "host-canonical-repository-load-route:");
        assert_no_activation(&tracker, "observed-host-canonical-repository-load-route:");
    }

    #[tokio::test]
    async fn selected_parent_routes_generated_load_through_canonical_owner() {
        let dice = registry_dice(GENERATED_PARENT_MODULE, LEAF_MAPPING_A, SOURCE_A);
        let mut root_tx = transaction(&dice, ROOT_MODULE, EXTENSION_A, true, None).await;
        let route = admitted_root_route(&mut root_tx, "parent_alias").await;
        let RootRepositoryBzlLoadRoute::Canonical(child_repo) = route
            .selected_bzl_load_route(&ApparentRepoName::new("compatibility_repo").unwrap())
            .unwrap()
        else {
            panic!("generated import must defer to the canonical owner")
        };
        let parent_input = host_repository_source_input(route.source_capability()).unwrap();
        let HostRepositorySourceInputDispositionView::Request(parent_request) =
            parent_input.view().disposition()
        else {
            panic!("selected parent must retain a source request")
        };
        let host = root_tx.compute(&PathObservationEpochKey).await.unwrap();
        let parent_instance = PathObservationInstanceId::new(79);
        let parent = bzl_source_epoch(
            PathObservationNamespace::Materialization(parent_instance),
            "/generated-parent",
            &[
                ("compatibility.bzl", Some(GENERATED_PARENT_EXTENSION)),
                (
                    "entry.bzl",
                    Some(b"load('@compatibility_repo//:symbols.bzl', 'VALUE')\nRESULT = VALUE\n"),
                ),
            ],
        );
        let tracker = Arc::new(DependencyTrace::default());
        let success = |source, root, instance| RepositoryMaterializationSuccess::Immutable {
            source_identity: Arc::from(source),
            generation_root: PathBuf::from(root),
            observation_instance: instance,
        };
        let mut stage = requests_transaction_with_observations(
            &dice,
            [(
                parent_request.clone(),
                success("generated-parent", "/generated-parent", parent_instance),
            )],
            tracker.clone(),
            merge_observations(&host, &parent),
        )
        .await;
        let child = stage
            .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                child_repo.clone(),
            ))
            .await
            .unwrap();
        let child_input = load_route(&child).input().clone();
        let HostRepositorySourceInputDispositionView::Request(child_request) =
            child_input.view().disposition()
        else {
            panic!("generated child must retain its effect request")
        };
        let child_instance = PathObservationInstanceId::new(80);
        let child_source = bzl_source_epoch(
            PathObservationNamespace::Materialization(child_instance),
            "/generated-compatibility",
            &[("symbols.bzl", Some(b"VALUE = 'generated'\n"))],
        );
        let mut tx = requests_transaction_with_observations(
            &dice,
            [
                (
                    parent_request.clone(),
                    success("generated-parent", "/generated-parent", parent_instance),
                ),
                (
                    child_request.clone(),
                    success(
                        "generated-compatibility",
                        "/generated-compatibility",
                        child_instance,
                    ),
                ),
            ],
            tracker.clone(),
            merge_observations(&merge_observations(&host, &parent), &child_source),
        )
        .await;
        let label = external_label("", "entry.bzl");
        let legacy_key = ExternalBzlModuleEvalKey::new(route.clone(), label.clone());
        let observed_key = ExternalBzlModuleObservationKey::new(route, label);
        let SourcePreparationOutcome::Complete(legacy) = tx.compute(&legacy_key).await.unwrap()
        else {
            panic!("generated child legacy load must complete")
        };
        let legacy = legacy.as_ref().as_ref().unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) =
            tx.compute(&observed_key).await.unwrap()
        else {
            panic!("generated child observed load must complete")
        };
        let observed_module = observed.result().as_ref().as_ref().unwrap();
        let result = observed_module.module.get("RESULT").unwrap();
        assert_eq!(result.value().to_string(), "\"generated\"");
        assert_eq!(
            observed_module.manifest.direct_children[0].label,
            CanonicalLabel::parse(&format!("{child_repo}//:symbols.bzl")).unwrap()
        );
        assert_eq!(
            legacy.manifest.direct_children[0].label,
            observed_module.manifest.direct_children[0].label
        );
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let legacy_route =
            HostCanonicalRepositoryLoadRouteKey::new(workspace.clone(), child_repo.clone())
                .to_string();
        let observed_route =
            HostCanonicalRepositoryLoadRouteObservationKey::new(workspace, child_repo.clone())
                .to_string();
        let legacy_dependencies = tracker.dependencies(&legacy_key.to_string());
        assert!(
            legacy_dependencies.contains(&legacy_route),
            "legacy dependencies: {legacy_dependencies:?}"
        );
        let observed_dependencies = tracker.dependencies(&observed_key.to_string());
        assert!(
            observed_dependencies.contains(&observed_route),
            "observed dependencies: {observed_dependencies:?}"
        );
        let canonical_source =
            format!("observed-host-repository-source-observation:{child_repo}:symbols.bzl");
        assert!(tracker.all_keys().contains(&canonical_source));
        let root_source = format!("observed-host-repository-source-file:{child_repo}");
        assert_no_activation(&tracker, &root_source);
        assert_no_activation(
            &tracker,
            "host-selected-extension-definition-load-requests:",
        );
        assert_no_activation(
            &tracker,
            "observed-host-selected-extension-definition-load-requests:",
        );
    }

    #[tokio::test]
    async fn root_builtin_external_bzl_promotes_one_public_load_without_changing_its_key() {
        let dice = builtin_graph_dice();
        let module = builtin_graph_module();
        let mut seed = transaction(&dice, &module, EXTENSION_A, true, None).await;
        let route = admitted_root_route(&mut seed, "bazel_tools").await;
        let rules_cc = CanonicalRepoName::new("rules_cc+").unwrap();
        let SourcePreparationOutcome::Complete(rules_route) = seed
            .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                rules_cc.clone(),
            ))
            .await
            .unwrap()
        else {
            panic!("rules_cc route must complete")
        };
        let rules_input = rules_route.as_ref().as_ref().unwrap().input().clone();
        let HostRepositorySourceInputDispositionView::Request(request) =
            rules_input.view().disposition()
        else {
            panic!("rules_cc must retain its source request")
        };
        let label = external_label("tools/cpp", "lib_cc_configure.bzl");
        let legacy_key = ExternalBzlModuleEvalKey::new_bzlmod(route.clone(), label.clone());
        let observed_key = ExternalBzlModuleObservationKey::new_bzlmod(route, label);
        let pending = seed.compute(&observed_key).await.unwrap();
        let SourcePreparationOutcome::Need(need) = pending else {
            panic!("built-in load must request the selected rules_cc source")
        };
        assert_eq!(
            need.repository_materializations().get(&request.id),
            Some(request)
        );
        let host = seed.compute(&PathObservationEpochKey).await.unwrap();
        let instance = PathObservationInstanceId::new(87);
        let utility = b"load('//cc/private/toolchain:escape.bzl', _escape_string = 'escape_string')\nescape_string = _escape_string\n";
        let sources = builtin_rules_cc_source_epoch(instance, utility);
        let materialization = || RepositoryMaterializationSuccess::Immutable {
            source_identity: Arc::from("builtin-public-load-rules-cc"),
            generation_root: PathBuf::from("/registry-rules-cc"),
            observation_instance: instance,
        };
        let mut platform_pending = request_transaction_with_observations(
            &dice,
            request.clone(),
            Arc::new(DependencyTrace::default()),
            materialization(),
            merge_observations(&host, &sources),
        )
        .await;
        let pending = platform_pending.compute(&observed_key).await.unwrap();
        let SourcePreparationOutcome::Need(need) = pending else {
            panic!("built-in mapping must request the unresolved platform source")
        };
        let platform_request = need
            .repository_materializations()
            .iter()
            .find(|(id, _)| id.canonical_repo.as_str() == "platforms+")
            .map(|(_, request)| request.clone())
            .unwrap_or_else(|| panic!("built-in mapping needs: {need:?}"));
        let tracker = Arc::new(DependencyTrace::default());
        let mut tx = requests_transaction_with_observations(
            &dice,
            [
                (
                    platform_request.clone(),
                    RepositoryMaterializationSuccess::Local,
                ),
                (
                    request.clone(),
                    RepositoryMaterializationSuccess::Immutable {
                        source_identity: Arc::from("builtin-public-load-rules-cc"),
                        generation_root: PathBuf::from("/registry-rules-cc"),
                        observation_instance: instance,
                    },
                ),
            ],
            tracker.clone(),
            merge_observations(&host, &sources),
        )
        .await;

        let legacy_outcome = tx.compute(&legacy_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy) = legacy_outcome else {
            panic!("root built-in legacy module must complete: {legacy_outcome:?}")
        };
        let legacy = legacy.as_ref().as_ref().unwrap();
        let observed_value = tx.compute(&observed_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = &observed_value else {
            panic!("root built-in observed module must complete")
        };
        let observed_module = observed.result().as_ref().as_ref().unwrap();
        assert_eq!(observed_module.manifest, legacy.manifest);
        assert!(
            observed_module
                .manifest
                .root
                .repository_mapping
                .iter()
                .any(|(apparent, canonical)| apparent.as_str() == "rules_cc"
                    && canonical == &rules_cc)
        );
        assert_eq!(
            observed_module.manifest.direct_children[0]
                .label
                .to_string(),
            "@@rules_cc+//cc/toolchains:toolchain_config_utils.bzl"
        );
        assert_eq!(
            observed_module.manifest.reachable[2].label.to_string(),
            "@@rules_cc+//cc/private/toolchain:escape.bzl"
        );

        let target = external_label("cc/toolchains", "toolchain_config_utils.bzl");
        let child = external_label("cc/private/toolchain", "escape.bzl");
        let legacy_target = tx
            .compute(&ExternalBzlModuleEvalKey::new_canonical_bzlmod(
                rules_input.clone(),
                target.clone(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(legacy_target) = legacy_target else {
            panic!("legacy target must complete")
        };
        let legacy_target = legacy_target.as_ref().as_ref().unwrap();
        let observed_target = tx
            .compute(&ExternalBzlModuleObservationKey::new_canonical_bzlmod(
                rules_input.clone(),
                target,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed_target)) = observed_target else {
            panic!("observed target must complete")
        };
        let observed_child = tx
            .compute(&ExternalBzlModuleObservationKey::new_canonical_bzlmod(
                rules_input.clone(),
                child,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed_child)) = observed_child else {
            panic!("observed recursive child must complete")
        };
        let observed_child = observed_child.result().as_ref().as_ref().unwrap();
        assert_eq!(
            observed_child
                .module
                .get("CONTEXT")
                .unwrap()
                .value()
                .unpack_str(),
            Some("9.2.0")
        );
        assert!(
            legacy
                .module
                .get("escape_string")
                .unwrap()
                .value()
                .ptr_eq(legacy_target.module.get("escape_string").unwrap().value())
        );
        assert!(
            observed_module
                .module
                .get("escape_string")
                .unwrap()
                .value()
                .ptr_eq(observed_child.module.get("escape_string").unwrap().value())
        );

        let builtin_route = tx
            .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::new("bazel_tools").unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(builtin_route)) = builtin_route else {
            panic!("observed built-in route must complete")
        };
        let selected_route = tx
            .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                rules_cc,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(selected_route)) = selected_route else {
            panic!("observed selected route must complete")
        };
        let expected = merge_observations(
            builtin_route.observations(),
            &merge_observations(
                selected_route.observations(),
                observed_target.observations(),
            ),
        );
        assert_eq!(observed.observations(), &expected);
        let warm = tx.compute(&observed_key).await.unwrap();
        assert!(ExternalBzlModuleObservationKey::equality(
            &observed_value,
            &warm
        ));
        assert!(ExternalBzlModuleObservationKey::validity(&warm));

        let changed_sources = builtin_rules_cc_source_epoch(
            instance,
            b"load('//cc/private/toolchain:escape.bzl', _escape_string = 'escape_string')\nescape_string = _escape_string\nVARIANT = 'changed'\n",
        );
        let mut changed = requests_transaction_with_observations(
            &dice,
            [
                (
                    platform_request.clone(),
                    RepositoryMaterializationSuccess::Local,
                ),
                (request.clone(), materialization()),
            ],
            Arc::new(DependencyTrace::default()),
            merge_observations(&host, &changed_sources),
        )
        .await;
        let changed = changed.compute(&observed_key).await.unwrap();
        assert!(!ExternalBzlModuleObservationKey::equality(
            &observed_value,
            &changed
        ));
        let mut source_restored = requests_transaction_with_observations(
            &dice,
            [
                (
                    platform_request.clone(),
                    RepositoryMaterializationSuccess::Local,
                ),
                (request.clone(), materialization()),
            ],
            Arc::new(DependencyTrace::default()),
            merge_observations(&host, &sources),
        )
        .await;
        let source_restored = source_restored.compute(&observed_key).await.unwrap();
        assert!(ExternalBzlModuleObservationKey::equality(
            &observed_value,
            &source_restored
        ));

        assert!(tracker.all_keys().iter().all(|key| {
            !(key.starts_with("host-repository-source-file:") && key.contains("bazel_tools"))
                && !(key.starts_with("observed-host-repository-source-file:")
                    && key.contains("bazel_tools"))
                && !key.contains("repository-materialization-result:@@bazel_tools")
        }));
    }

    #[tokio::test]
    async fn root_builtin_external_bzl_keeps_excluded_shapes_and_missing_mapping_fail_closed() {
        let tracker = Arc::new(DependencyTrace::default());
        let dice = builtin_graph_dice();
        let module = builtin_graph_module();
        let mut tx = transaction(&dice, &module, EXTENSION_A, true, Some(tracker.clone())).await;
        let route = admitted_root_route(&mut tx, "bazel_tools").await;
        let multi = ExternalBzlModuleObservationKey::new_bzlmod(
            route.clone(),
            external_label("tools/cpp", "cc_configure.bzl"),
        );
        let SourcePreparationOutcome::Complete(Ok(multi)) = tx.compute(&multi).await.unwrap()
        else {
            panic!("excluded multi-load module must complete with its typed boundary")
        };
        assert!(matches!(
            multi.result().as_ref().as_ref().unwrap_err(),
            ExternalBzlModuleError::LoadLabel { source, .. }
                if source.to_string() == "@@bazel_tools//tools/cpp:cc_configure.bzl"
        ));
        let missing = ExternalBzlModuleObservationKey::new_bzlmod(
            route,
            external_label("tools/cpp", "missing-before-mapping.bzl"),
        );
        let SourcePreparationOutcome::Complete(Ok(missing)) = tx.compute(&missing).await.unwrap()
        else {
            panic!("missing built-in source must complete before mapping")
        };
        assert!(
            matches!(
                missing.result().as_ref().as_ref().unwrap_err(),
                ExternalBzlModuleError::SourceObservation { .. }
            ),
            "missing source outcome: {missing:?}"
        );
        assert_no_activation(&tracker, "host-canonical-repository-load-route:");
        assert_no_activation(&tracker, "observed-host-canonical-repository-load-route:");
    }

    #[tokio::test]
    async fn builtin_route_source_and_listing_use_only_catalog_drivers() {
        let tracker = Arc::new(DependencyTrace::default());
        let dice = builtin_graph_dice();
        let module = builtin_graph_module();
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut tx = transaction(&dice, &module, EXTENSION_A, true, Some(tracker.clone())).await;
        let canonical = CanonicalRepoName::new("bazel_tools").unwrap();
        let load_key = HostCanonicalRepositoryLoadRouteObservationKey::new(workspace, canonical);
        let load = tx.compute(&load_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(load)) = load else {
            panic!("built-in load route must complete: {load:?}")
        };
        let route = load.result().as_ref().as_ref().unwrap();
        assert_eq!(
            route.route().view().kind(),
            HostCanonicalRepositoryRouteKind::Builtin
        );
        assert!(!load.observations().observations().is_empty());

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
        let tools_key = RepositoryPackageLoadObservationKey::new_canonical(
            route.input().clone(),
            PackagePath::parse("tools").unwrap(),
        );
        let tools = tx.compute(&tools_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(tools)) = tools else {
            panic!("built-in tools package must complete: {tools:?}")
        };
        let tools = tools.result().as_ref().as_ref().unwrap();
        assert!(matches!(
            &tools
                .targets
                .iter()
                .find(|target| target.name == "host_platform")
                .unwrap()
                .kind,
            PackageTargetKind::Alias { actual }
                if actual == &CanonicalLabel::parse("@@platforms//host:host").unwrap()
        ));
        assert_eq!(
            tools.direct_load_roots[0].label,
            CanonicalLabel::parse("@@bazel_tools//tools:build_defs.bzl").unwrap()
        );
        assert!(tools.reachable_loads.iter().any(|load| {
            load.label == CanonicalLabel::parse("@@platforms//host:constraints.bzl").unwrap()
        }));

        let source_route = HostRepositorySourceRoute::Canonical(route.input().clone());
        let host_scope = HostGlobBoundaryScope::External(source_route.clone());
        let catalog_scope = HostGlobBoundaryScope::BuiltinCatalog(source_route.clone());
        assert_ne!(host_scope, catalog_scope);
        assert_eq!(
            catalog_scope,
            HostGlobBoundaryScope::BuiltinCatalog(source_route)
        );

        let launcher_probe = CatalogGlobProbeKey {
            workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            input: route.input().clone(),
            package: PackagePath::parse("src/tools/launcher").unwrap(),
            request: HostGlobLoadingRequest::new(
                GlobPattern::include("**").unwrap(),
                HostGlobLoadingOperation::Files,
            ),
            observed: true,
        };
        tracker.clear();
        let launcher = tx.compute(&launcher_probe).await.unwrap().unwrap();
        let SourcePreparationOutcome::Complete(Ok((launcher, observations))) = launcher else {
            panic!("built-in launcher glob must complete: {launcher:?}")
        };
        assert!(observations.observations().is_empty());
        let launcher = launcher.as_ref().as_ref().unwrap();
        let launcher_paths = launcher
            .paths()
            .iter()
            .map(|path| path.to_vec())
            .collect::<Vec<_>>();
        assert_eq!(
            launcher_paths.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [
                b"BUILD".as_slice(),
                b"bash_launcher.cc".as_slice(),
                b"bash_launcher.h".as_slice(),
                b"dummy.cc".as_slice(),
                b"java_launcher.cc".as_slice(),
                b"java_launcher.h".as_slice(),
                b"launcher.cc".as_slice(),
                b"launcher.h".as_slice(),
                b"launcher_main.cc".as_slice(),
                b"launcher_maker.cc".as_slice(),
                b"launcher_maker_test.bzl".as_slice(),
                b"launcher_maker_test.cc".as_slice(),
                b"python_launcher.cc".as_slice(),
                b"python_launcher.h".as_slice(),
                b"win_manifest.xml".as_slice(),
                b"win_resources.rc".as_slice(),
                b"win_rules.bzl".as_slice(),
            ]
        );
        let launcher_keys = tracker.all_keys();
        assert!(launcher_keys.iter().any(|key| {
            key.starts_with("observed-host-external-package-boundary:")
                && key.ends_with("//src/tools/launcher/util")
        }));
        assert!(launcher_keys.iter().any(|key| {
            key.starts_with("observed-host-repository-directory-listing:")
                && key.ends_with("//src/tools/launcher")
        }));
        assert!(
            launcher_keys
                .iter()
                .any(|key| { key == "builtin-bazel-tools-directory-listing:src/tools/launcher" })
        );
        assert!(launcher_keys.iter().all(|key| {
            !key.starts_with("path-directory-listing:")
                && !key.starts_with("observed-path-directory-listing:")
                && !key.contains("repository-materialization-result:@@bazel_tools")
        }));
        tracker.clear();
        let warm = tx.compute(&launcher_probe).await.unwrap().unwrap();
        let SourcePreparationOutcome::Complete(Ok((warm, observations))) = warm else {
            panic!("warm built-in launcher glob must complete: {warm:?}")
        };
        assert!(observations.observations().is_empty());
        assert_eq!(
            warm.as_ref()
                .as_ref()
                .unwrap()
                .paths()
                .iter()
                .map(|path| path.to_vec())
                .collect::<Vec<_>>(),
            launcher_paths
        );
        assert_no_activation(&tracker, "observed-host-repository-directory-listing:");
        assert_no_activation(&tracker, "observed-host-external-package-boundary:");

        tracker.clear();
        for (glob_pattern, expected) in [
            (
                "**",
                vec![
                    b"BUILD".to_vec(),
                    b"win_res.bzl".to_vec(),
                    b"winsdk_configure.bzl".to_vec(),
                    b"winsdk_toolchain.bzl".to_vec(),
                ],
            ),
            (
                "*.bzl",
                vec![
                    b"win_res.bzl".to_vec(),
                    b"winsdk_configure.bzl".to_vec(),
                    b"winsdk_toolchain.bzl".to_vec(),
                ],
            ),
        ] {
            let probe = CatalogGlobProbeKey {
                workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                input: route.input().clone(),
                package: PackagePath::parse("tools/res").unwrap(),
                request: HostGlobLoadingRequest::new(
                    GlobPattern::include(glob_pattern).unwrap(),
                    HostGlobLoadingOperation::Files,
                ),
                observed: true,
            };
            let outcome = tx.compute(&probe).await.unwrap().unwrap();
            let SourcePreparationOutcome::Complete(Ok((matches, observations))) = outcome else {
                panic!("built-in tools/res {glob_pattern} glob must complete: {outcome:?}")
            };
            assert!(observations.observations().is_empty());
            assert_eq!(
                matches
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .paths()
                    .iter()
                    .map(|path| path.to_vec())
                    .collect::<Vec<_>>(),
                expected
            );
            let keys = tracker.all_keys();
            assert!(keys.iter().any(|key| {
                key.starts_with("observed-host-repository-directory-listing:")
                    && key.ends_with("//tools/res")
            }));
            assert!(keys.iter().all(|key| {
                !key.starts_with("path-directory-listing:")
                    && !key.starts_with("observed-path-directory-listing:")
                    && !key.contains("repository-materialization-result:@@bazel_tools")
            }));
        }

        tracker.clear();
        let res_key = RepositoryPackageLoadObservationKey::new_canonical(
            route.input().clone(),
            PackagePath::parse("tools/res").unwrap(),
        );
        let res = tx.compute(&res_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(res)) = res else {
            panic!("built-in tools/res package must complete: {res:?}")
        };
        assert!(res.observations().observations().is_empty());
        let error = res.result().as_ref().as_ref().unwrap_err().to_string();
        assert!(error.contains("@@bazel_tools//tools/res:empty"));
        assert!(error.contains("visibility is not explicitly public"));
        assert!(!error.contains("BUILD globs are deferred"));
        let res_keys = tracker.all_keys();
        assert!(res_keys.iter().all(|key| {
            !key.starts_with("path-directory-listing:")
                && !key.starts_with("observed-path-directory-listing:")
                && !key.contains("repository-materialization-result:@@bazel_tools")
        }));

        let package_key = RepositoryPackageLoadObservationKey::new_canonical(
            route.input().clone(),
            PackagePath::parse("src/conditions").unwrap(),
        );
        let package = tx.compute(&package_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(package)) = package else {
            panic!("built-in canonical package load must complete")
        };
        let error = package.result().as_ref().as_ref().unwrap_err().to_string();
        assert!(error.contains("@@bazel_tools//src/conditions:BUILD"));
        assert!(!error.contains("<output_base>"));
        assert!(!error.contains("builtin/bazel_tools"));
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
    async fn complete_canonical_mapping_resolves_and_rejects_missing_apparent_repo() {
        let (_, input) = selected_input(LEAF_MAPPING_A, SOURCE_A).await;
        assert!(matches!(
            crate::bzl_module::resolve_canonical_external_bzl_load_label(
                &input,
                &PackagePath::root(),
                "@missing//:child.bzl"
            ),
            Err(crate::bzl_module::ExternalLoadLabelError::Repository { .. })
        ));
        let route = input.view().route();
        let mapping = route.bzl_repository_mapping();
        assert!(mapping.iter().any(|(apparent, canonical)| {
            apparent.as_str() == "alias_a" && canonical.as_str() == "mapped+"
        }));
        let owner = BzlModuleIdentity {
            label: CanonicalLabel::parse("@@leaf+//:defs.bzl").unwrap(),
            workspace_path: PathBuf::from("@@leaf+//:defs.bzl"),
            repository_mapping: mapping,
        };
        let context = BzlEvaluationContext::from_manifest(&BzlLoadManifest {
            root: owner.clone(),
            direct_children: Arc::from([]),
            reachable: Arc::from([owner]),
            fingerprint: [0; 32],
        });
        let ast = AstModule::parse(
            "@@leaf+//:defs.bzl",
            "mapped = Label('@alias_a//:target')\n".to_owned(),
            &Dialect::Bazel,
        )
        .unwrap();
        assert!(ast.loads().is_empty());
        let module = Module::new();
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        evaluator.eval_module(ast, &loading_globals()).unwrap();
        drop(evaluator);
        let module = module.freeze().unwrap();
        assert_eq!(
            module.get("mapped").unwrap().value().to_string(),
            "@@mapped+//:target"
        );
    }

    struct MappedChildFixture {
        transaction: dice::DiceTransaction,
        tracker: Arc<DependencyTrace>,
        leaf_input: HostCanonicalRepositorySourceInput,
        mapped_input: HostCanonicalRepositorySourceInput,
    }

    fn mapped_child_source_observations() -> PathObservationEpoch {
        let leaf = PathObservationNamespace::Materialization(PathObservationInstanceId::new(71));
        let mapped = PathObservationNamespace::Materialization(PathObservationInstanceId::new(72));
        let demand = |namespace, value: &str, operation| {
            PathObservationDemand::new(
                namespace,
                NormalizedAbsolutePath::new(value).unwrap(),
                operation,
            )
        };
        let present = |namespace, value: &str, kind, stamp| {
            (
                demand(namespace, value, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    kind, stamp, stamp, stamp, stamp, 0o755,
                ))),
            )
        };
        let missing = |namespace, value: &str| {
            (
                demand(namespace, value, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            )
        };
        let bytes = |namespace, value: &str, source: &'static [u8]| {
            (
                demand(namespace, value, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(source))),
            )
        };
        PathObservationEpoch::new([
            present(leaf, "/", PathNodeKind::Directory, 71),
            present(leaf, "/registry-leaf", PathNodeKind::Directory, 71),
            present(
                leaf,
                "/registry-leaf/BUILD.bazel",
                PathNodeKind::RegularFile,
                71,
            ),
            bytes(
                leaf,
                "/registry-leaf/BUILD.bazel",
                b"load(':entry.bzl', 'mapped_rule')\nmapped_rule(name = 'leaf', visibility = ['//visibility:public'])\n",
            ),
            present(
                leaf,
                "/registry-leaf/entry.bzl",
                PathNodeKind::RegularFile,
                71,
            ),
            bytes(
                leaf,
                "/registry-leaf/entry.bzl",
                b"load('@alias_a//:defs.bzl', _mapped_rule = 'mapped_rule')\nmapped_rule = _mapped_rule\n",
            ),
            missing(leaf, "/registry-leaf/REPO.bazel"),
            missing(leaf, "/registry-leaf/.bazelignore"),
            present(mapped, "/", PathNodeKind::Directory, 72),
            present(mapped, "/registry-mapped", PathNodeKind::Directory, 72),
            present(
                mapped,
                "/registry-mapped/defs.bzl",
                PathNodeKind::RegularFile,
                72,
            ),
            bytes(
                mapped,
                "/registry-mapped/defs.bzl",
                b"def _impl(ctx):\n    return None\nmapped_rule = rule(implementation = _impl)\n",
            ),
        ])
        .unwrap()
    }

    fn mapped_child_materializations(
        leaf: Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>,
        mapped: Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>,
    ) -> RepositoryMaterializationResultEpoch {
        let success = |source, root, instance| {
            RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
                source_identity: Arc::from(source),
                generation_root: PathBuf::from(root),
                observation_instance: PathObservationInstanceId::new(instance),
            })
        };
        RepositoryMaterializationResultEpoch::new(
            leaf.id.workspace.clone(),
            [
                RepositoryMaterializationEpochEntry {
                    request: leaf,
                    result: success("leaf-load-source", "/registry-leaf", 71),
                },
                RepositoryMaterializationEpochEntry {
                    request: mapped,
                    result: success("mapped-load-source", "/registry-mapped", 72),
                },
            ],
        )
        .unwrap()
    }

    async fn mapped_child_fixture() -> MappedChildFixture {
        let (dice, leaf_input) = selected_input(LEAF_MAPPING_A, SOURCE_A).await;
        let mut route_tx = transaction(&dice, ROOT_MODULE, EXTENSION_A, true, None).await;
        let mapped_route = route_tx
            .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::new("mapped+").unwrap(),
            ))
            .await
            .unwrap();
        let mapped_input = load_route(&mapped_route).input().clone();
        let host = route_tx.compute(&PathObservationEpochKey).await.unwrap();
        let request = |input: &HostCanonicalRepositorySourceInput| match input.view().disposition()
        {
            HostRepositorySourceInputDispositionView::Request(request) => request.clone(),
            _ => panic!("selected route must retain a materialization request"),
        };
        let materialized = mapped_child_source_observations();
        let observations = PathObservationEpoch::from_shared(
            host.observations()
                .iter()
                .chain(materialized.observations())
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap();
        let tracker = Arc::new(DependencyTrace::default());
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(bzl_load_cycle_detector()),
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                },
                mapped_child_materializations(request(&leaf_input), request(&mapped_input)),
            )])
            .unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, observations)])
            .unwrap();
        MappedChildFixture {
            transaction: updater.commit().await,
            tracker,
            leaf_input,
            mapped_input,
        }
    }

    async fn assert_mapped_child_observation_epoch(
        fixture: &mut MappedChildFixture,
        observed: &ObservedRepositoryPackageLoad,
    ) {
        let package_source = fixture
            .transaction
            .compute(
                &RepositoryPackageSourceObservationKey::new_canonical(
                    fixture.leaf_input.clone(),
                    PackageIdentifier::new(
                        CanonicalRepoName::new("leaf+").unwrap(),
                        PackagePath::root(),
                    ),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(package_source)) = package_source else {
            panic!("canonical package source must complete")
        };
        let same_repo = fixture
            .transaction
            .compute(&HostRepositorySourceObservationEpochKey::new_canonical(
                fixture.leaf_input.clone(),
                host_repository_relative_path(PathBuf::from("entry.bzl")).unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(same_repo)) = same_repo else {
            panic!("same-repository child source must complete")
        };
        let child_route = fixture
            .transaction
            .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::new("mapped+").unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(child_route)) = child_route else {
            panic!("mapped child route must complete")
        };
        let child_source = fixture
            .transaction
            .compute(&HostRepositorySourceObservationEpochKey::new_canonical(
                fixture.mapped_input.clone(),
                host_repository_relative_path(PathBuf::from("defs.bzl")).unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(child_source)) = child_source else {
            panic!("mapped child source must complete")
        };
        let expected = PathObservationEpoch::from_shared(
            package_source
                .observations()
                .observations()
                .iter()
                .chain(same_repo.observations().observations().iter())
                .chain(child_route.observations().observations().iter())
                .chain(child_source.observations().observations().iter())
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap();
        assert_eq!(observed.observations(), &expected);
    }

    #[tokio::test]
    async fn canonical_package_load_resolves_mapped_child_route_before_source() {
        let mut fixture = mapped_child_fixture().await;
        let key = RepositoryPackageLoadObservationKey::new_canonical(
            fixture.leaf_input.clone(),
            PackagePath::root(),
        );
        let outcome = fixture.transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("canonical mapped-child package load must complete: {outcome:?}")
        };
        let package = observed.result().as_ref().as_ref().unwrap();
        assert_eq!(package.targets.len(), 1);
        assert_eq!(package.targets[0].name.as_str(), "leaf");
        assert_eq!(package.direct_load_roots.len(), 1);
        assert_eq!(
            package.direct_load_roots[0].label,
            CanonicalLabel::parse("@@leaf+//:entry.bzl").unwrap()
        );
        assert_eq!(
            package.direct_load_roots[0].workspace_path,
            PathBuf::from("@@leaf+//:entry.bzl")
        );
        assert_eq!(
            package.reachable_loads[1].label,
            CanonicalLabel::parse("@@mapped+//:defs.bzl").unwrap()
        );
        assert_eq!(
            package.reachable_loads[1].workspace_path,
            PathBuf::from("@@mapped+//:defs.bzl")
        );
        assert_eq!(
            package.build_file,
            PathBuf::from("<output_base>/external/leaf+/BUILD.bazel")
        );
        assert_mapped_child_observation_epoch(&mut fixture, &observed).await;
        let keys = fixture.tracker.all_keys();
        let route_index = keys
            .iter()
            .position(|key| {
                key.contains("observed-host-canonical-repository-load-route")
                    && key.contains("mapped+")
            })
            .unwrap();
        let source_index = keys
            .iter()
            .position(|key| {
                key.contains("observed-host-repository-source") && key.contains("defs.bzl")
            })
            .unwrap();
        assert!(route_index < source_index);
    }

    #[tokio::test]
    async fn canonical_external_bzl_failures_cycles_and_source_names_are_carrier_exact() {
        let (dice, input) = selected_input(LEAF_MAPPING_A, SOURCE_A).await;
        let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
        else {
            panic!("selected route must retain a materialization request")
        };
        let instance = PathObservationInstanceId::new(73);
        let namespace = PathObservationNamespace::Materialization(instance);
        let tracker = Arc::new(DependencyTrace::default());
        let mut tx = request_transaction_with_observations(
            &dice,
            request.clone(),
            tracker,
            RepositoryMaterializationSuccess::Immutable {
                source_identity: Arc::from("canonical-bzl-error-source"),
                generation_root: PathBuf::from("/registry-errors"),
                observation_instance: instance,
            },
            bzl_source_epoch(
                namespace,
                "/registry-errors",
                &[
                    ("missing.bzl", None),
                    ("parse.bzl", Some(b"VALUE =\n")),
                    ("eval.bzl", Some(b"fail('boom')\n")),
                    ("one.bzl", Some(b"load(':two.bzl', 'TWO')\nONE = TWO\n")),
                    ("two.bzl", Some(b"load(':one.bzl', 'ONE')\nTWO = ONE\n")),
                ],
            ),
        )
        .await;

        let missing = tx
            .compute(&canonical_bzl_key(input.clone(), "missing.bzl"))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(missing)) = missing else {
            panic!("canonical missing source must be a semantic terminal")
        };
        assert!(matches!(
            missing.result().as_ref().as_ref().unwrap_err(),
            ExternalBzlModuleError::Absent { label }
                if label.to_string() == "@@leaf+//:missing.bzl"
        ));

        let parse = tx
            .compute(&canonical_bzl_key(input.clone(), "parse.bzl"))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(parse)) = parse else {
            panic!("canonical parse error must complete")
        };
        let parse = parse.result().as_ref().as_ref().unwrap_err();
        assert!(matches!(
            parse,
            ExternalBzlModuleError::Parse { label, message }
                if label.to_string() == "@@leaf+//:parse.bzl"
                    && message.contains("@@leaf+//:parse.bzl")
                    && !message.contains("/registry-errors")
        ));

        let evaluation = tx
            .compute(&canonical_bzl_key(input.clone(), "eval.bzl"))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(evaluation)) = evaluation else {
            panic!("canonical evaluation error must complete")
        };
        let evaluation = evaluation.result().as_ref().as_ref().unwrap_err();
        assert!(matches!(
            evaluation,
            ExternalBzlModuleError::Evaluation { label, message }
                if label.to_string() == "@@leaf+//:eval.bzl"
                    && message.contains("@@leaf+//:eval.bzl")
                    && !message.contains("/registry-errors")
        ));

        let cycle = tx
            .compute(&canonical_bzl_key(input, "one.bzl"))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(cycle)) = cycle else {
            panic!("canonical cycle must complete")
        };
        let cycle = cycle
            .result()
            .as_ref()
            .as_ref()
            .unwrap_err()
            .cycle()
            .unwrap();
        assert_eq!(
            cycle
                .keys
                .iter()
                .map(ExternalBzlCycleIdentity::canonical_label)
                .map(|label| label.to_string())
                .collect::<Vec<_>>(),
            ["@@leaf+//:one.bzl", "@@leaf+//:two.bzl"]
        );
        assert!(cycle.keys.iter().all(|key| {
            let label = key.canonical_label().to_string();
            !label.contains("/registry-errors") && !label.contains("<output_base>")
        }));
    }

    fn assert_generalized_loading_key_identity(
        first: HostCanonicalRepositorySourceInput,
        spec_changed: HostCanonicalRepositorySourceInput,
        mapping_changed: HostCanonicalRepositorySourceInput,
        restored: HostCanonicalRepositorySourceInput,
    ) {
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
        let spec_package = RepositoryPackageSourceKey::new_canonical(
            spec_changed.clone(),
            package_identifier.clone(),
        )
        .unwrap();
        let mapping_package = RepositoryPackageSourceKey::new_canonical(
            mapping_changed.clone(),
            package_identifier.clone(),
        )
        .unwrap();
        let restored_package =
            RepositoryPackageSourceKey::new_canonical(restored.clone(), package_identifier.clone())
                .unwrap();
        assert_eq!(first_package, restored_package);
        assert_eq!(hash(&first_package), hash(&restored_package));
        assert_ne!(first_package, spec_package);
        assert_ne!(hash(&first_package), hash(&spec_package));
        assert_ne!(first_package, mapping_package);
        assert_ne!(hash(&first_package), hash(&mapping_package));
        assert_loading_driver_identity(first, spec_changed, mapping_changed, restored);
    }

    fn assert_loading_driver_identity(
        first: HostCanonicalRepositorySourceInput,
        spec_changed: HostCanonicalRepositorySourceInput,
        mapping_changed: HostCanonicalRepositorySourceInput,
        restored: HostCanonicalRepositorySourceInput,
    ) {
        let package_load =
            |input, package| RepositoryPackageLoadObservationKey::new_canonical(input, package);
        assert_a_b_a(
            &package_load(first.clone(), PackagePath::root()),
            &package_load(spec_changed.clone(), PackagePath::root()),
            &package_load(restored.clone(), PackagePath::root()),
        );
        assert_ne!(
            package_load(first.clone(), PackagePath::root()),
            package_load(first.clone(), PackagePath::parse("other").unwrap())
        );
        let bzl = |input, target| canonical_bzl_key(input, target);
        let first_bzl = bzl(first.clone(), "defs.bzl");
        assert_a_b_a(
            &first_bzl,
            &bzl(mapping_changed.clone(), "defs.bzl"),
            &bzl(restored.clone(), "defs.bzl"),
        );
        assert_ne!(
            first_bzl,
            bzl(first.clone(), "other.bzl"),
            "external Bzl key retains its label"
        );
        assert_a_b_a(
            &first_bzl.cycle_identity(),
            &bzl(spec_changed.clone(), "defs.bzl").cycle_identity(),
            &bzl(restored.clone(), "defs.bzl").cycle_identity(),
        );
        let subtree =
            |input, package| ExternalSubtreePackageSetObservationKey::new_canonical(input, package);
        assert_a_b_a(
            &subtree(first.clone(), PackagePath::root()),
            &subtree(mapping_changed.clone(), PackagePath::root()),
            &subtree(restored.clone(), PackagePath::root()),
        );
        assert_ne!(
            subtree(first.clone(), PackagePath::root()),
            subtree(first.clone(), PackagePath::parse("other").unwrap())
        );
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
        assert_generalized_loading_key_identity(first, spec_changed, mapping_changed, restored);
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

    const ROUTE_FAILURE_MODULE: &str = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, broken='broken')\n";
    const ROUTE_FAILURE_EXTENSION: &str = r#"
repo=repository_rule(implementation=lambda ctx: fail('effect failed'))
def impl(ctx):
    repo(name='broken')
ext=module_extension(implementation=impl)
"#;

    async fn assert_route_need_suppresses_effect() {
        let tracker = Arc::new(DependencyTrace::default());
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut present = transaction(
            &dice,
            ROUTE_FAILURE_MODULE,
            ROUTE_FAILURE_EXTENSION,
            true,
            None,
        )
        .await;
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
    }

    async fn assert_generated_error_preserves_route_effect_prefix() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(DependencyTrace::default());
        let mut tx = transaction(
            &dice,
            ROUTE_FAILURE_MODULE,
            ROUTE_FAILURE_EXTENSION,
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
    async fn route_need_suppresses_effect_and_generated_error_preserves_route_effect_prefix() {
        assert_route_need_suppresses_effect().await;
        assert_generated_error_preserves_route_effect_prefix().await;
    }

    const RECURSIVE_FAILURE_MODULE: &str = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, parent='parent', broken='broken')\n";
    const RECURSIVE_FAILURE_EXTENSION: &str = r#"
parent_repo=repository_rule(implementation=lambda ctx: None)
broken_repo=repository_rule(implementation=lambda ctx: fail('effect failed'))
def impl(ctx):
    parent_repo(name='parent')
    broken_repo(name='broken')
ext=module_extension(implementation=impl)
"#;

    struct RecursiveRouteFailureFixture {
        dice: Arc<Dice>,
        parent_input: HostCanonicalRepositorySourceInput,
        parent_request: Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>,
        child_repo: CanonicalRepoName,
        host_observations: PathObservationEpoch,
    }

    async fn recursive_route_failure_fixture() -> RecursiveRouteFailureFixture {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut tx = transaction(
            &dice,
            RECURSIVE_FAILURE_MODULE,
            RECURSIVE_FAILURE_EXTENSION,
            true,
            None,
        )
        .await;
        let generated = names(&validated(&mut tx).await);
        let apparent = ApparentRepoName::new("broken").unwrap();
        let mut selected = None;
        for canonical in generated {
            let outcome = tx
                .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                    canonical,
                ))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(result) = outcome else {
                continue;
            };
            let Ok(route) = result.as_ref() else {
                continue;
            };
            if let Some(child_repo) = route.input().view().route().mapping_target(&apparent) {
                selected = Some((route.input().clone(), child_repo.clone()));
            }
        }
        let (parent_input, child_repo) = selected.expect("parent route maps the broken sibling");
        let HostRepositorySourceInputDispositionView::Request(parent_request) =
            parent_input.view().disposition()
        else {
            panic!("generated parent retains its materialization request")
        };
        let parent_request = parent_request.clone();
        let host_observations = tx.compute(&PathObservationEpochKey).await.unwrap();
        RecursiveRouteFailureFixture {
            dice,
            parent_input,
            parent_request,
            child_repo,
            host_observations,
        }
    }

    fn recursive_parent_source_observations() -> PathObservationEpoch {
        bzl_source_epoch(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(74)),
            "/generated-parent",
            &[("entry.bzl", Some(b"load('@broken//:defs.bzl', 'VALUE')\n"))],
        )
    }

    fn merge_observations(
        left: &PathObservationEpoch,
        right: &PathObservationEpoch,
    ) -> PathObservationEpoch {
        PathObservationEpoch::from_shared(
            left.observations()
                .iter()
                .chain(right.observations().iter())
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap()
    }

    async fn recursive_failure_transaction(
        fixture: &RecursiveRouteFailureFixture,
        host_observations: Option<&PathObservationEpoch>,
        tracker: Arc<DependencyTrace>,
    ) -> dice::DiceTransaction {
        let source = recursive_parent_source_observations();
        let observations = host_observations
            .map(|host| merge_observations(host, &source))
            .unwrap_or(source);
        request_transaction_with_observations(
            &fixture.dice,
            fixture.parent_request.clone(),
            tracker,
            RepositoryMaterializationSuccess::Immutable {
                source_identity: Arc::from("recursive-parent-source"),
                generation_root: PathBuf::from("/generated-parent"),
                observation_instance: PathObservationInstanceId::new(74),
            },
            observations,
        )
        .await
    }

    fn assert_no_recursive_child_source(tracker: &DependencyTrace) {
        assert!(!tracker.all_keys().iter().any(|key| {
            key.contains("observed-host-repository-source") && key.contains("defs.bzl")
        }));
    }

    async fn assert_recursive_route_error(
        fixture: &RecursiveRouteFailureFixture,
        tx: &mut dice::DiceTransaction,
        tracker: &DependencyTrace,
        effect_error: bool,
    ) {
        let parent = tx
            .compute(&canonical_bzl_key(
                fixture.parent_input.clone(),
                "entry.bzl",
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(parent)) = parent else {
            panic!("recursive route error must complete: {parent:?}")
        };
        let ExternalBzlModuleError::Route {
            source,
            load,
            message,
        } = parent.result().as_ref().as_ref().unwrap_err()
        else {
            panic!("recursive failure must be reported at the load route")
        };
        assert_eq!(
            source.to_string(),
            format!(
                "{}//:entry.bzl",
                fixture.parent_input.view().route().view().canonical_repo()
            )
        );
        assert_eq!(load.as_ref(), "@broken//:defs.bzl");
        assert_eq!(message.contains("effect failed"), effect_error);
        let child = tx
            .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                fixture.child_repo.clone(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(child)) = child else {
            panic!("child route error must retain an observed carrier")
        };
        assert_eq!(
            child
                .result()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .is_effect_error(),
            effect_error
        );
        let source = tx
            .compute(&HostRepositorySourceObservationEpochKey::new_canonical(
                fixture.parent_input.clone(),
                host_repository_relative_path(PathBuf::from("entry.bzl")).unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(source)) = source else {
            panic!("parent source must complete")
        };
        let expected = merge_observations(source.observations(), child.observations());
        assert_eq!(parent.observations(), &expected);
        assert_no_recursive_child_source(tracker);
    }

    #[tokio::test]
    async fn recursive_mapped_child_need_and_route_errors_stop_before_child_source() {
        let fixture = recursive_route_failure_fixture().await;
        let tracker = Arc::new(DependencyTrace::default());
        let mut need = recursive_failure_transaction(&fixture, None, tracker.clone()).await;
        assert!(matches!(
            need.compute(&canonical_bzl_key(
                fixture.parent_input.clone(),
                "entry.bzl"
            ))
            .await
            .unwrap(),
            SourcePreparationOutcome::Need(_)
        ));
        assert!(tracker.all_keys().iter().any(|key| {
            key.contains("observed-host-canonical-repository-load-route")
                && key.contains(fixture.child_repo.as_str())
        }));
        assert_no_recursive_child_source(&tracker);

        let tracker = Arc::new(DependencyTrace::default());
        let mut effect = recursive_failure_transaction(
            &fixture,
            Some(&fixture.host_observations),
            tracker.clone(),
        )
        .await;
        assert_recursive_route_error(&fixture, &mut effect, &tracker, true).await;

        let mut invalid = transaction(
            &fixture.dice,
            "module(",
            RECURSIVE_FAILURE_EXTENSION,
            true,
            None,
        )
        .await;
        let invalid_host = invalid.compute(&PathObservationEpochKey).await.unwrap();
        let tracker = Arc::new(DependencyTrace::default());
        let mut semantic =
            recursive_failure_transaction(&fixture, Some(&invalid_host), tracker.clone()).await;
        assert_recursive_route_error(&fixture, &mut semantic, &tracker, false).await;
    }

    #[tokio::test]
    async fn constructor_fail_closed_and_hash_table_covers_keys_dispositions_and_effect_plan() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let dice = builtin_graph_dice();
        let module = builtin_graph_module();
        let mut tx = transaction(&dice, &module, EXTENSION_A, true, None).await;
        let builtin = tx
            .compute(&super::super::HostCanonicalRepositoryRouteKey::new(
                workspace.clone(),
                CanonicalRepoName::new("bazel_tools").unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(builtin) = builtin else {
            panic!("built-in route must complete")
        };
        let builtin = Arc::new(builtin.as_ref().as_ref().unwrap().clone());
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

    #[tokio::test]
    async fn cancellation_publishes_no_canonical_package_load_and_recovery_completes() {
        let (dice, input) = selected_input(LEAF_MAPPING_A, SOURCE_A).await;
        let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
        else {
            panic!("selected registry input must retain a request")
        };
        let tracker = Arc::new(DependencyTrace::default());
        let instance = PathObservationInstanceId::new(74);
        let materialization = || RepositoryMaterializationSuccess::Immutable {
            source_identity: Arc::from("canonical-package-cancellation"),
            generation_root: PathBuf::from("/registry-package-cancellation"),
            observation_instance: instance,
        };
        let key =
            RepositoryPackageLoadObservationKey::new_canonical(input.clone(), PackagePath::root());
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
            "/registry-package-cancellation",
            None,
        )
        .await;
        let outcome = recovered.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(package)) = outcome else {
            panic!("recovered canonical package load must complete")
        };
        assert!(package.result().is_ok());
        assert!(
            tracker
                .all_keys()
                .iter()
                .any(|name| name == &key.to_string())
        );
    }

    #[tokio::test]
    async fn cancellation_publishes_no_canonical_recursive_bzl_and_recovery_completes() {
        let (dice, input) = selected_input(LEAF_MAPPING_A, SOURCE_A).await;
        let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
        else {
            panic!("selected registry input must retain a request")
        };
        let tracker = Arc::new(DependencyTrace::default());
        let instance = PathObservationInstanceId::new(75);
        let materialization = || RepositoryMaterializationSuccess::Immutable {
            source_identity: Arc::from("canonical-bzl-cancellation"),
            generation_root: PathBuf::from("/registry-bzl-cancellation"),
            observation_instance: instance,
        };
        let key = canonical_bzl_key(input.clone(), "entry.bzl");
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

        let namespace = PathObservationNamespace::Materialization(instance);
        let mut recovered = request_transaction_with_observations(
            &dice,
            request.clone(),
            tracker.clone(),
            materialization(),
            bzl_source_epoch(
                namespace,
                "/registry-bzl-cancellation",
                &[("entry.bzl", Some(b"VALUE = 1\n"))],
            ),
        )
        .await;
        let outcome = recovered.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(module)) = outcome else {
            panic!("recovered canonical external Bzl must complete")
        };
        let module = module.result().as_ref().as_ref().unwrap();
        assert_eq!(
            module.manifest.root.workspace_path,
            PathBuf::from("@@leaf+//:entry.bzl")
        );
        assert!(
            tracker
                .all_keys()
                .iter()
                .any(|name| name == &key.to_string())
        );
    }

    #[test]
    fn keys_are_complete_only_and_retained_shapes_stay_compact_and_apparent_free() {
        assert_allocative::<RepositoryPackageSourceAddress>();
        assert_allocative::<RepositoryPackageSource>();
        assert_allocative::<ExternalBzlModuleEvalKey>();
        assert_allocative::<ExternalBzlModuleObservationKey>();
        assert_allocative::<ExternalBzlCycleIdentity>();
        assert_allocative::<ObservedExternalBzlModule>();
        assert_allocative::<RepositoryPackageLoadKey>();
        assert_allocative::<RepositoryPackageLoadObservationKey>();
        assert_allocative::<ObservedRepositoryPackageLoad>();
        assert_allocative::<ExternalSubtreePackageSetKey>();
        assert_allocative::<ExternalSubtreePackageSetObservationKey>();
        assert_allocative::<ObservedExternalSubtreePackageSet>();
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
        assert!(std::mem::size_of::<RepositoryPackageSourceAddress>() <= 32);
        assert!(std::mem::size_of::<RepositoryPackageSource>() <= 80);
        assert!(std::mem::size_of::<ObservedHostExternalPackageBoundary>() <= 48);
        assert!(std::mem::size_of::<ObservedRepositoryPackageSource>() <= 48);
        assert!(std::mem::size_of::<ExternalBzlModuleEvalKey>() <= 320);
        assert!(std::mem::size_of::<ExternalBzlModuleObservationKey>() <= 320);
        assert!(std::mem::size_of::<ExternalBzlCycleIdentity>() <= 320);
        assert!(std::mem::size_of::<ObservedExternalBzlModule>() <= 48);
        assert!(std::mem::size_of::<RepositoryPackageLoadKey>() <= 320);
        assert!(std::mem::size_of::<RepositoryPackageLoadObservationKey>() <= 320);
        assert!(std::mem::size_of::<ObservedRepositoryPackageLoad>() <= 48);
        assert!(std::mem::size_of::<ExternalSubtreePackageSetKey>() <= 320);
        assert!(std::mem::size_of::<ExternalSubtreePackageSetObservationKey>() <= 320);
        assert!(std::mem::size_of::<ObservedExternalSubtreePackageSet>() <= 48);
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
