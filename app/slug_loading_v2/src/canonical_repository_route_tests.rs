#[cfg(test)]
pub(super) mod tests {
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Arc;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use compact_str::CompactString;
    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::Key;
    use dice::RichActivation;
    use dice::UserComputationData;
    use dupe::Dupe;
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::HostBuiltinBazelToolsRepositoryMappingKey;
    use slug_bzlmod_v2::HostBuiltinBazelToolsRepositoryMappingObservationKey;
    use slug_bzlmod_v2::HostCanonicalRepositoryRoute;
    use slug_bzlmod_v2::HostCanonicalRepositoryRouteKind;
    use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionErrorDisposition;
    use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionKey;
    use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionObservationKey;
    use slug_bzlmod_v2::HostRepositoryLocalPathPolicy;
    use slug_bzlmod_v2::HostRepositorySourceFileKey;
    use slug_bzlmod_v2::HostRootRepositoryMappingKey;
    use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
    use slug_bzlmod_v2::HostSelectedExtensionDemandObservationKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::OverrideAttributeValue;
    use slug_bzlmod_v2::RegistryFileKey;
    use slug_bzlmod_v2::RegistryFileUrl;
    use slug_bzlmod_v2::RegistryIo;
    use slug_bzlmod_v2::RegistryIoOutcome;
    use slug_bzlmod_v2::RegistryRequestGeneration;
    use slug_bzlmod_v2::RegistryTransportError;
    use slug_bzlmod_v2::RegistryUrls;
    use slug_bzlmod_v2::RepoRuleId;
    use slug_bzlmod_v2::RepoSpec;
    use slug_bzlmod_v2::RepositoryHostInputTransaction;
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationKey;
    use slug_bzlmod_v2::RepositoryMaterializationKind;
    use slug_bzlmod_v2::RepositoryMaterializationRequest;
    use slug_bzlmod_v2::RepositoryMaterializationRequestId;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_bzlmod_v2::RepositoryPackageSourceKey;
    use slug_bzlmod_v2::RepositoryPlatform;
    use slug_bzlmod_v2::RepositoryPlatformKey;
    use slug_bzlmod_v2::RepositorySourceFileKey;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_bzlmod_v2::RootRepositoryRouteKey;
    use slug_bzlmod_v2::SourcePreparationOutcome;
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_events_v2::EvaluationEvent;
    use slug_events_v2::EventBatch;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::ObservedPathFrontierError;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use starlark_map::small_map::SmallMap;
    use starlark_map::sorted_map::SortedMap;

    use crate::HostValidatedGeneratedRepositorySpecsOutcome;
    use crate::HostValidatedModuleExtensionRepositoriesKey;
    use crate::HostValidatedModuleExtensionRepositoriesObservationKey;
    use crate::canonical_repository_mapping::*;
    use crate::canonical_repository_route::*;
    use crate::generated_repository_definition::*;

    pub(crate) const WORKSPACE: &str = "/generated-repository-definition";
    pub(crate) const MODULE: &str = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, first='first', second='second')\n";
    pub(crate) fn builtin_graph_module() -> String {
        let mut module = include_str!("../../slug_bzlmod_v2/builtin/bazel_tools/MODULE.bazel")
            .replacen(
                "module(name = \"bazel_tools\")",
                "module(name = \"root\")",
                1,
            )
            .replace(", repo_name = None", "");
        module.push_str(
            "\n# builtin_graph\nlocal_path_override(module_name='platforms', path='platforms')\n",
        );
        module
    }
    pub(crate) const EXTENSION_A: &str = r#"
repo=repository_rule(implementation=lambda ctx: None, attrs={'value':attr.string(), 'target':attr.label()})
def impl(ctx):
    repo(name='first', value='one', target=':local')
    repo(name='second', value='two', target='@first//:item')
ext=module_extension(implementation=impl)
"#;
    const EXTENSION_B: &str = r#"
other=repository_rule(implementation=lambda ctx: None, attrs={'value':attr.string(), 'target':attr.label()})
def impl(ctx):
    other(name='first', value='one', target=':local')
    other(name='second', value='two', target='@first//:item')
ext=module_extension(implementation=impl)
"#;

    struct StaticRegistryIo(BTreeMap<String, Arc<[u8]>>);

    struct BuiltinGraphRegistryIo;

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

    #[async_trait]
    impl RegistryIo for BuiltinGraphRegistryIo {
        async fn read_exact(
            &self,
            url: &RegistryFileUrl,
        ) -> Result<RegistryIoOutcome, RegistryTransportError> {
            let parts = url.as_str().split('/').collect::<Vec<_>>();
            let bytes = match parts.as_slice() {
                [.., "modules", name, version, "MODULE.bazel"] => Some(
                    format!("module(name='{name}', version='{version}')\n")
                        .into_bytes()
                        .into(),
                ),
                [.., "modules", name, _, "source.json"] => Some(
                    format!(
                        "{{\"url\":\"https://origin.invalid/{name}.tgz\",\"integrity\":\"sha256-a\"}}"
                    )
                    .into_bytes()
                    .into(),
                ),
                [.., "bazel_registry.json"] => Some(Arc::from(b"{}".as_slice())),
                _ => None,
            };
            Ok(bytes.map_or(RegistryIoOutcome::NotFound, RegistryIoOutcome::Found))
        }
    }

    pub(crate) fn builtin_graph_dice() -> Arc<Dice> {
        let mut builder = Dice::builder();
        slug_bzlmod_v2::install_registry_io(&mut builder, Arc::new(BuiltinGraphRegistryIo));
        builder.build(DetectCycles::Enabled)
    }

    #[derive(Default)]
    struct LookupTracker {
        canonical: Mutex<Vec<(ActivationKind, bool)>>,
        selected: Mutex<Vec<(ActivationKind, bool)>>,
        lookup: Mutex<Vec<(ActivationKind, bool)>>,
        apparent: Mutex<Vec<(ActivationKind, bool)>>,
        root_mapping: Mutex<Vec<(ActivationKind, bool)>>,
        forbidden: Mutex<Vec<&'static str>>,
        activations: Mutex<Vec<(String, ActivationKind, Option<EventBatch>)>>,
        dependencies: Mutex<Vec<(String, Vec<String>)>>,
    }

    impl ActivationTracker for LookupTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            dependencies: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            self.dependencies.lock().unwrap().push((
                key.to_string(),
                dependencies.map(ToString::to_string).collect(),
            ));
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            let batch = activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe);
            self.activations
                .lock()
                .unwrap()
                .push((key.to_string(), activation.kind(), batch));
            if key
                .downcast_ref::<HostCanonicalRepositoryRouteKey>()
                .is_some()
            {
                self.canonical
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key
                .downcast_ref::<HostCanonicalSelectedModuleDefinitionKey>()
                .is_some()
            {
                self.selected
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key
                .downcast_ref::<HostGeneratedRepositoryDefinitionKey>()
                .is_some()
            {
                self.lookup
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key
                .downcast_ref::<HostCanonicalRepositoryApparentMappingKey>()
                .is_some()
            {
                self.apparent
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key.downcast_ref::<HostRootRepositoryMappingKey>().is_some() {
                self.root_mapping
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key.downcast_ref::<RootRepositoryRouteKey>().is_some() {
                self.forbidden.lock().unwrap().push("root-route");
            } else if key.downcast_ref::<RegistryFileKey>().is_some() {
                self.forbidden.lock().unwrap().push("registry");
            } else if key.downcast_ref::<RepositoryMaterializationKey>().is_some() {
                self.forbidden.lock().unwrap().push("materialization");
            } else if key
                .downcast_ref::<crate::HostSelectedRepositoryFileEffectKey>()
                .is_some()
            {
                self.forbidden.lock().unwrap().push("effect");
            } else if key.downcast_ref::<RepositoryPackageSourceKey>().is_some()
                || key.downcast_ref::<RepositorySourceFileKey>().is_some()
                || key.downcast_ref::<HostRepositorySourceFileKey>().is_some()
            {
                self.forbidden.lock().unwrap().push("source");
            } else if key.downcast_ref::<PathObservationEpochKey>().is_some() {
                self.forbidden.lock().unwrap().push("filesystem");
            }
        }
    }

    pub(crate) async fn transaction(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        extension_present: bool,
        tracker: Option<Arc<dyn ActivationTracker>>,
    ) -> dice::DiceTransaction {
        transaction_with_policy(
            dice,
            module,
            extension,
            extension_present,
            tracker,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        )
        .await
    }

    fn generated_definition_observation_epoch(
        module: &str,
        extension: &str,
        extension_present: bool,
    ) -> PathObservationEpoch {
        let demand = |path: &str, operation| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        };
        let lstat = |kind, stamp, mode| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, stamp, 1, 1, 1, mode,
            )))
        };
        let path = |name: &str| format!("{WORKSPACE}/{name}");
        let mut observations = Vec::new();
        for (stamp, directory) in [(1, "/"), (2, WORKSPACE)] {
            observations.push((
                demand(directory, PathObservationOperation::Lstat),
                lstat(PathNodeKind::Directory, stamp, 0o755),
            ));
        }
        for name in ["REPO.bazel", ".bazelignore", "BUILD", "MODULE.bazel.lock"] {
            observations.push((
                demand(&path(name), PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ));
        }
        for (name, kind, stamp, mode) in [
            ("MODULE.bazel", PathNodeKind::RegularFile, 9, 0o644),
            ("BUILD.bazel", PathNodeKind::RegularFile, 10, 0o644),
            ("local", PathNodeKind::Directory, 12, 0o755),
            ("local/MODULE.bazel", PathNodeKind::RegularFile, 13, 0o644),
        ] {
            observations.push((
                demand(&path(name), PathObservationOperation::Lstat),
                lstat(kind, stamp, mode),
            ));
        }
        observations.push((
            demand(&path("MODULE.bazel"), PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                module.as_bytes(),
            ))),
        ));
        if module.contains("# builtin_graph") {
            for (name, kind, stamp) in [
                ("platforms", PathNodeKind::Directory, 20),
                ("platforms/MODULE.bazel", PathNodeKind::RegularFile, 21),
                ("platforms/host", PathNodeKind::Directory, 22),
                (
                    "platforms/host/constraints.bzl",
                    PathNodeKind::RegularFile,
                    23,
                ),
                ("platforms/host/BUILD.bazel", PathNodeKind::RegularFile, 24),
            ] {
                observations.push((
                    demand(&path(name), PathObservationOperation::Lstat),
                    lstat(
                        kind,
                        stamp,
                        if kind == PathNodeKind::Directory {
                            0o755
                        } else {
                            0o644
                        },
                    ),
                ));
            }
            for name in [
                "platforms/REPO.bazel",
                "platforms/.bazelignore",
                "platforms/BUILD",
            ] {
                observations.push((
                    demand(&path(name), PathObservationOperation::Lstat),
                    PathObservationResult::Lstat(PathOperationResult::Missing),
                ));
            }
            for (name, bytes) in [
                (
                    "platforms/MODULE.bazel",
                    b"module(name='platforms', version='1.0.0')\n".as_slice(),
                ),
                (
                    "platforms/host/constraints.bzl",
                    b"HOST_CONSTRAINTS = ['@platforms//cpu:x86_64']\n".as_slice(),
                ),
                (
                    "platforms/host/BUILD.bazel",
                    b"exports_files(['constraints.bzl'])\n".as_slice(),
                ),
            ] {
                observations.push((
                    demand(&path(name), PathObservationOperation::FileBytes),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        bytes,
                    ))),
                ));
            }
        }
        observations.push((
            demand(
                &path("local/MODULE.bazel"),
                PathObservationOperation::FileBytes,
            ),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                &b"module(name='local')\n"[..],
            ))),
        ));
        let extension_lstat = if extension_present {
            PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                11,
                1,
                1,
                1,
                0o644,
            ))
        } else {
            PathOperationResult::Missing
        };
        observations.push((
            demand(&path("ext.bzl"), PathObservationOperation::Lstat),
            PathObservationResult::Lstat(extension_lstat),
        ));
        let extension_bytes = if extension_present {
            PathOperationResult::Present(Arc::from(extension.as_bytes()))
        } else {
            PathOperationResult::Missing
        };
        observations.push((
            demand(&path("ext.bzl"), PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(extension_bytes),
        ));
        PathObservationEpoch::new(observations).unwrap()
    }

    async fn transaction_with_policy(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        extension_present: bool,
        tracker: Option<Arc<dyn ActivationTracker>>,
        command_policy: BzlmodCommandPolicyKey,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut user_data = UserComputationData {
            cycle_detector: Some(crate::bzl_load_cycle_detector()),
            activation_tracker: tracker,
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        user_data
            .data
            .set(RepositoryHostInputTransaction::default());
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(
                RepositoryPlatformKey::new(workspace.dupe()),
                RepositoryPlatform::new("linux", "x86_64"),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(SortedMap::from_iter([
                        (
                            workspace.as_path().join("MODULE.bazel"),
                            WorkspaceFileValue::Present(Arc::new(module.to_owned())),
                        ),
                        (
                            workspace.as_path().join("ext.bzl"),
                            if extension_present {
                                WorkspaceFileValue::Present(Arc::new(extension.to_owned()))
                            } else {
                                WorkspaceFileValue::Absent
                            },
                        ),
                        (
                            workspace.as_path().join("BUILD.bazel"),
                            WorkspaceFileValue::Present(Arc::new(String::new())),
                        ),
                    ])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel.lock"),
                        WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        slug_bzlmod_v2::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            command_policy,
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        slug_bzlmod_v2::inject_registry_request_inputs(
            &mut updater,
            workspace.as_path(),
            RegistryUrls::new(["https://registry.invalid"]),
            RegistryRequestGeneration(1),
        )
        .unwrap();
        slug_bzlmod_v2::inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                workspace.dupe(),
                Arc::from([workspace.dupe()]),
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        let materializations = module
            .contains("# builtin_graph")
            .then(|| {
                ["platforms+", "platforms"].map(|canonical_repo| {
                    RepositoryMaterializationEpochEntry {
                        request: Arc::new(RepositoryMaterializationRequest {
                            id: RepositoryMaterializationRequestId {
                                workspace: workspace.dupe(),
                                canonical_repo: CanonicalRepoName::new(canonical_repo).unwrap(),
                            },
                            repo_spec: RepoSpec {
                                rule_id: RepoRuleId {
                                    bzl_file: CanonicalLabel::parse(
                                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                                    )
                                    .unwrap(),
                                    rule_name: "local_repository".into(),
                                },
                                attributes: Arc::new(SmallMap::from_iter([(
                                    CompactString::new("path"),
                                    OverrideAttributeValue::String("platforms".into()),
                                )])),
                            },
                            kind: RepositoryMaterializationKind::Local {
                                logical_root: NormalizedAbsolutePath::new(format!(
                                    "{WORKSPACE}/platforms"
                                ))
                                .unwrap(),
                            },
                        }),
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Local,
                        ),
                    }
                })
            })
            .into_iter()
            .flatten();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(workspace.dupe(), materializations)
                    .unwrap(),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                generated_definition_observation_epoch(module, extension, extension_present),
            )])
            .unwrap();
        updater.commit().await
    }

    pub(crate) async fn validated(
        transaction: &mut dice::DiceTransaction,
    ) -> HostValidatedGeneratedRepositorySpecsOutcome {
        transaction
            .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    pub(crate) fn names(
        value: &HostValidatedGeneratedRepositorySpecsOutcome,
    ) -> Vec<CanonicalRepoName> {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("validation must complete")
        };
        value
            .as_ref()
            .as_ref()
            .unwrap()
            .iter()
            .map(|(name, _, _, _)| name.clone())
            .collect()
    }

    async fn lookup(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        requested: Option<&CanonicalRepoName>,
    ) -> HostGeneratedRepositoryDefinitionOutcome {
        let mut tx = transaction(dice, module, extension, true, None).await;
        let mut generated = names(&validated(&mut tx).await);
        let name = requested.cloned().unwrap_or_else(|| generated.remove(0));
        tx.compute(&HostGeneratedRepositoryDefinitionKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            name,
        ))
        .await
        .unwrap()
    }

    async fn observed_lookup(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        extension_present: bool,
        requested: CanonicalRepoName,
        tracker: Option<Arc<LookupTracker>>,
    ) -> <HostGeneratedRepositoryDefinitionObservationKey as Key>::Value {
        transaction(
            dice,
            module,
            extension,
            extension_present,
            tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
        )
        .await
        .compute(&HostGeneratedRepositoryDefinitionObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            requested,
        ))
        .await
        .unwrap()
    }

    fn observed_carrier(
        value: &<HostGeneratedRepositoryDefinitionObservationKey as Key>::Value,
    ) -> &ObservedHostGeneratedRepositoryDefinition {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("expected observed generated definition carrier: {value:?}"),
        }
    }

    async fn observed_canonical_lookup(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        extension_present: bool,
        requested: CanonicalRepoName,
        tracker: Option<Arc<LookupTracker>>,
    ) -> <HostCanonicalRepositoryRouteObservationKey as Key>::Value {
        transaction(
            dice,
            module,
            extension,
            extension_present,
            tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
        )
        .await
        .compute(&HostCanonicalRepositoryRouteObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            requested,
        ))
        .await
        .unwrap()
    }

    fn observed_canonical_carrier(
        value: &<HostCanonicalRepositoryRouteObservationKey as Key>::Value,
    ) -> &ObservedHostCanonicalRepositoryRoute {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("expected observed canonical definition carrier: {value:?}"),
        }
    }

    fn observed_apparent_mapping_carrier(
        value: &<HostCanonicalRepositoryApparentMappingObservationKey as Key>::Value,
    ) -> &ObservedHostCanonicalRepositoryApparentMapping {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("expected observed apparent mapping carrier: {value:?}"),
        }
    }

    fn assert_apparent_epoch_current(epoch: &PathObservationEpoch, global: &PathObservationEpoch) {
        for (demand, result) in epoch.observations() {
            assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
        }
    }

    fn activation_dependencies(tracker: &LookupTracker, key: &str) -> Vec<String> {
        tracker
            .dependencies
            .lock()
            .unwrap()
            .iter()
            .find(|(name, _)| name == key)
            .unwrap_or_else(|| panic!("missing dependency row for {key}"))
            .1
            .clone()
    }

    fn starlark_print_owners(tracker: &LookupTracker) -> Vec<(String, String)> {
        tracker
            .activations
            .lock()
            .unwrap()
            .iter()
            .flat_map(|(name, _, batch)| {
                batch
                    .iter()
                    .flat_map(EventBatch::events)
                    .filter_map(|event| match event {
                        EvaluationEvent::StarlarkPrint { text, .. } => {
                            Some((name.clone(), text.to_string()))
                        }
                        _ => None,
                    })
            })
            .collect()
    }

    fn assert_activation_families_absent(tracker: &LookupTracker, families: &[&str]) {
        let activations = tracker.activations.lock().unwrap();
        let dependencies = tracker.dependencies.lock().unwrap();
        for family in families {
            assert!(
                activations
                    .iter()
                    .all(|(name, _, _)| !name.starts_with(family))
            );
            assert!(dependencies.iter().all(|(name, children)| {
                !name.starts_with(family) && children.iter().all(|child| !child.starts_with(family))
            }));
        }
    }

    fn snapshot(value: &HostGeneratedRepositoryDefinitionOutcome) -> Vec<String> {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("lookup must complete")
        };
        let route = value.as_ref().as_ref().unwrap();
        let view = route.view();
        let mapping = ["bazel_tools", "first", "second"].map(|name| {
            route
                .mapping_target(&ApparentRepoName::new(name).unwrap())
                .map(ToString::to_string)
        });
        vec![
            view.canonical_repo().as_str().to_owned(),
            view.internal_name().unwrap().to_owned(),
            view.repo_spec().unwrap().rule_id.rule_name.to_string(),
            format!("{:?}", view.repo_spec().unwrap().attributes),
            view.mapping_context().as_str().to_owned(),
            format!("{mapping:?}"),
        ]
    }

    fn mapping(
        value: &HostCanonicalRepositoryApparentMappingOutcome,
    ) -> &HostCanonicalRepositoryApparentMapping {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("mapping must complete")
        };
        value.as_ref().as_ref().unwrap()
    }

    fn route(value: &HostCanonicalRepositoryRouteOutcome) -> &HostCanonicalRepositoryRoute {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("route must complete")
        };
        value.as_ref().as_ref().unwrap()
    }

    fn route_hash(value: &HostCanonicalRepositoryRouteOutcome) -> u64 {
        let mut hasher = DefaultHasher::new();
        route(value).hash(&mut hasher);
        hasher.finish()
    }

    fn generated_route_hash(value: &HostGeneratedRepositoryDefinitionOutcome) -> u64 {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("generated route must complete")
        };
        let mut hasher = DefaultHasher::new();
        value.as_ref().as_ref().unwrap().hash(&mut hasher);
        hasher.finish()
    }

    fn target(value: &HostCanonicalRepositoryApparentMappingOutcome) -> CanonicalRepoName {
        mapping(value).resolved_target().unwrap().clone()
    }

    async fn canonical_lookup(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        canonical_repo: CanonicalRepoName,
    ) -> HostCanonicalRepositoryRouteOutcome {
        transaction(dice, module, extension, true, None)
            .await
            .compute(&HostCanonicalRepositoryRouteKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                canonical_repo,
            ))
            .await
            .unwrap()
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum SyntheticDomainOutcome {
        Need,
        Success,
        Missing,
        Terminal,
    }

    fn synthetic_composition(
        selected: SyntheticDomainOutcome,
        generated: SyntheticDomainOutcome,
        generated_calls: &std::cell::Cell<usize>,
    ) -> SyntheticDomainOutcome {
        match selected {
            SyntheticDomainOutcome::Success
            | SyntheticDomainOutcome::Terminal
            | SyntheticDomainOutcome::Need => selected,
            SyntheticDomainOutcome::Missing => {
                generated_calls.set(generated_calls.get() + 1);
                generated
            }
        }
    }

    #[test]
    fn composition_branch_matrix_is_missing_only_and_selected_first() {
        use SyntheticDomainOutcome as O;

        for selected in [O::Success, O::Terminal, O::Need] {
            let calls = std::cell::Cell::new(0);
            assert_eq!(
                synthetic_composition(selected, O::Success, &calls),
                selected
            );
            assert_eq!(calls.get(), 0, "same-canonical generated candidate ran");
        }
        for (generated, expected) in [
            (O::Success, O::Success),
            (O::Terminal, O::Terminal),
            (O::Missing, O::Missing),
            (O::Need, O::Need),
        ] {
            let calls = std::cell::Cell::new(0);
            assert_eq!(
                synthetic_composition(O::Missing, generated, &calls),
                expected
            );
            assert_eq!(calls.get(), 1);
        }
    }

    #[tokio::test]
    async fn builtin_route_uses_complete_selected_mapping_owner() {
        let tracker = Arc::new(LookupTracker::default());
        let dice = builtin_graph_dice();
        let module = builtin_graph_module();
        let mut tx = transaction(&dice, &module, EXTENSION_A, true, Some(tracker.clone())).await;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let canonical = CanonicalRepoName::new("bazel_tools").unwrap();
        let legacy_key = HostCanonicalRepositoryRouteKey::new(workspace.clone(), canonical.clone());
        let legacy = tx.compute(&legacy_key).await.unwrap();
        let legacy_route = route(&legacy);
        assert_eq!(
            legacy_route.view().kind(),
            HostCanonicalRepositoryRouteKind::Builtin
        );
        assert_eq!(legacy_route.view().canonical_repo(), &canonical);
        assert!(legacy_route.view().builtin_identity().is_some());
        assert!(legacy_route.view().repo_spec().is_none());
        assert!(legacy_route.view().generated_effect_seed().is_none());
        assert_eq!(
            legacy_route
                .mapping_target(&ApparentRepoName::new("platforms").unwrap())
                .unwrap()
                .as_str(),
            "platforms"
        );
        assert!(
            legacy_route
                .mapping_target(&ApparentRepoName::new("buildozer_binary").unwrap())
                .is_some()
        );
        assert!(
            legacy_route
                .mapping_target(&ApparentRepoName::new("anything").unwrap())
                .is_none()
        );

        let observed_key = HostCanonicalRepositoryRouteObservationKey::new(workspace, canonical);
        let observed = tx.compute(&observed_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = &observed else {
            panic!("builtin observed route must complete directly")
        };
        assert_eq!(observed.result().as_ref().as_ref().unwrap(), legacy_route);
        assert!(!observed.observations().observations().is_empty());
        assert_eq!(
            activation_dependencies(&tracker, &legacy_key.to_string()),
            [HostBuiltinBazelToolsRepositoryMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            )
            .to_string()]
        );
        assert_eq!(
            activation_dependencies(&tracker, &observed_key.to_string()),
            [HostBuiltinBazelToolsRepositoryMappingObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            )
            .to_string()]
        );
        assert!(tracker.selected.lock().unwrap().is_empty());
        assert!(tracker.lookup.lock().unwrap().is_empty());
        assert_activation_families_absent(
            &tracker,
            &[
                "host-canonical-selected-module-definition:",
                "host-generated-repository-definition:",
                "repository-package-source:",
            ],
        );
    }

    async fn selected_registry_route(source: &'static [u8]) -> HostCanonicalRepositoryRouteOutcome {
        const MODULE_URL: &str = "https://registry.invalid/modules/dep/1/MODULE.bazel";
        const SOURCE_URL: &str = "https://registry.invalid/modules/dep/1/source.json";
        let io = StaticRegistryIo(BTreeMap::from([
            (
                "https://registry.invalid/bazel_registry.json".to_owned(),
                Arc::from(&b"{}"[..]),
            ),
            (
                MODULE_URL.to_owned(),
                Arc::from(&b"module(name='dep', version='1')\n"[..]),
            ),
            (SOURCE_URL.to_owned(), Arc::from(source)),
        ]));
        let mut builder = Dice::builder();
        slug_bzlmod_v2::install_registry_io(&mut builder, Arc::new(io));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let module = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\n";
        transaction(&dice, module, EXTENSION_A, true, None)
            .await
            .compute(&HostCanonicalRepositoryRouteKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::new("dep+").unwrap(),
            ))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn selected_registry_route_retains_source_identity_and_hashes_structurally() {
        const SOURCE_A: &[u8] = br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#;
        const SOURCE_B: &[u8] = br#"{"url":"https://origin.test/b.tgz","integrity":"sha256-b"}"#;
        let first = selected_registry_route(SOURCE_A).await;
        let warm = selected_registry_route(SOURCE_A).await;
        let changed = selected_registry_route(SOURCE_B).await;
        let view = route(&first).view();
        assert_eq!(
            view.kind(),
            HostCanonicalRepositoryRouteKind::SelectedRegistry
        );
        assert_eq!(view.canonical_repo().as_str(), "dep+");
        assert_eq!(view.mapping_context().as_str(), "dep+");
        assert_eq!(
            view.local_path_policy(),
            Some(HostRepositoryLocalPathPolicy::LocalUnsupported)
        );
        assert_eq!(
            view.repo_spec().unwrap().rule_id.rule_name.as_str(),
            "http_archive"
        );
        assert_eq!(
            route(&first)
                .mapping_target(&ApparentRepoName::new("dep").unwrap())
                .unwrap()
                .as_str(),
            "dep+"
        );
        assert!(HostCanonicalRepositoryRouteKey::equality(&first, &warm));
        assert_eq!(route_hash(&first), route_hash(&warm));
        assert!(!HostCanonicalRepositoryRouteKey::equality(&first, &changed));
        assert_ne!(route_hash(&first), route_hash(&changed));
    }

    #[test]
    fn complete_scan_rejects_missing_and_duplicate() {
        use std::cell::Cell;

        let requested = CanonicalRepoName::new("wanted").unwrap();
        let other = CanonicalRepoName::new("other").unwrap();
        assert_eq!(
            find_unique_ordinal(&requested, [].iter()),
            Err(UniqueOrdinalError::Missing)
        );
        assert_eq!(
            find_unique_ordinal(&requested, [&other, &requested].into_iter()),
            Ok(1)
        );
        let consumed = Cell::new(0);
        let names = [&requested, &other, &requested, &other];
        assert_eq!(
            find_unique_ordinal(
                &requested,
                names
                    .into_iter()
                    .inspect(|_| consumed.set(consumed.get() + 1)),
            ),
            Err(UniqueOrdinalError::Duplicate {
                first: 0,
                conflicting: 2
            })
        );
        assert_eq!(consumed.get(), names.len());
    }
    #[test]
    fn mapping_context_mismatch_precedes_target_lookup() {
        let requested = CanonicalRepoName::new("requested").unwrap();
        let other = CanonicalRepoName::new("other").unwrap();
        let target_checks = Cell::new(0);
        let has_target = || {
            target_checks.set(target_checks.get() + 1);
            false
        };
        assert_eq!(
            mapping_lookup_status(&requested, &other, &requested, has_target),
            MappingLookupStatus::ContextMismatch
        );
        assert_eq!(target_checks.get(), 0);
        assert_eq!(
            mapping_lookup_status(&requested, &requested, &other, has_target),
            MappingLookupStatus::ContextMismatch
        );
        assert_eq!(target_checks.get(), 0);
        assert_eq!(
            mapping_lookup_status(&requested, &requested, &requested, has_target),
            MappingLookupStatus::Missing
        );
        assert_eq!(target_checks.get(), 1);
        assert_eq!(
            mapping_lookup_status(&requested, &requested, &requested, || true),
            MappingLookupStatus::Found
        );
    }
    #[tokio::test]
    async fn canonical_route_selects_before_missing_only_generated_fallback() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let generated = names(&validated(&mut tx).await);
        tracker.canonical.lock().unwrap().clear();
        tracker.selected.lock().unwrap().clear();
        tracker.lookup.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = |canonical_repo| {
            HostCanonicalRepositoryRouteKey::new(workspace.clone(), canonical_repo)
        };
        let root_key = key(CanonicalRepoName::root());
        let root = tx.compute(&root_key).await.unwrap();
        let SourcePreparationOutcome::Complete(root_value) = &root else {
            panic!("root definition must complete")
        };
        let root_definition = root_value.as_ref().as_ref().unwrap();
        let root_view = root_definition.view();
        assert_eq!(root_view.kind(), HostCanonicalRepositoryRouteKind::Root);
        assert_eq!(root_view.canonical_repo(), &CanonicalRepoName::root());
        assert!(root_view.repo_spec().is_none());
        assert!(tracker.lookup.lock().unwrap().is_empty());
        let mut selected_error_tx = transaction(
            &dice,
            "module(name='root')\nbazel_dep(name='missing', version='1')\n",
            EXTENSION_A,
            true,
            Some(tracker.clone()),
        )
        .await;
        let selected_error = selected_error_tx
            .compute(&key(CanonicalRepoName::root()))
            .await
            .unwrap();
        assert!(matches!(
            selected_error,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryRouteError {
                        kind: HostCanonicalRepositoryRouteErrorKind::Selected(error),
                        ..
                    }) if error.disposition()
                        == HostCanonicalSelectedModuleDefinitionErrorDisposition::Terminal
                )
        ));
        assert!(tracker.lookup.lock().unwrap().is_empty());
        let selected_mapping_terminal = selected_error_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                CanonicalRepoName::new("missing+").unwrap(),
                ApparentRepoName::new("bazel_tools").unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(value) = selected_mapping_terminal else {
            panic!("selected terminal must complete")
        };
        assert!(matches!(
            value.as_ref(),
            Err(HostCanonicalRepositoryApparentMappingError {
                kind: HostCanonicalRepositoryApparentMappingErrorKind::Route(_),
                ..
            })
        ));
        assert!(tracker.lookup.lock().unwrap().is_empty());
        tracker.forbidden.lock().unwrap().clear();
        let generated_key = key(generated[0].clone());
        let generated_value = tx.compute(&generated_key).await.unwrap();
        let SourcePreparationOutcome::Complete(value) = &generated_value else {
            panic!("generated definition must complete")
        };
        let generated_definition = value.as_ref().as_ref().unwrap();
        let generated_view = generated_definition.view();
        let kind = generated_view.kind();
        assert_eq!(kind, HostCanonicalRepositoryRouteKind::Generated);
        assert_eq!(generated_view.canonical_repo(), &generated[0]);
        assert_eq!(generated_view.internal_name(), Some("first"));
        let rule_name = &generated_view.repo_spec().unwrap().rule_id.rule_name;
        assert_eq!(rule_name, "repo");
        assert_eq!(generated_view.mapping_context(), &generated[0]);
        let generated_seed = generated_view.generated_effect_seed().unwrap();
        assert_eq!(generated_seed.ordinal(), 0);
        let generated_spec = generated_view.repo_spec().unwrap().clone();
        let rejected = |canonical_repo, mapping_context| {
            HostCanonicalRepositoryRoute::generated(
                workspace.clone(),
                canonical_repo,
                generated_seed.owner().clone(),
                generated_seed.ordinal(),
                "first",
                generated_spec.clone(),
                mapping_context,
                SmallMap::new(),
            )
            .is_none()
        };
        let root_canonical = CanonicalRepoName::root();
        assert!(rejected(root_canonical.clone(), root_canonical));
        let builtin = CanonicalRepoName::new("bazel_tools").unwrap();
        assert!(rejected(builtin.clone(), builtin));
        let other = CanonicalRepoName::new("other+").unwrap();
        assert!(rejected(generated[0].clone(), other));
        let generated_terminal = transaction(&dice, MODULE, EXTENSION_A, false, None)
            .await
            .compute(&generated_key)
            .await
            .unwrap();
        assert!(matches!(
            &generated_terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryRouteError {
                        canonical_repo,
                        kind: HostCanonicalRepositoryRouteErrorKind::Generated {
                            selected_missing,
                            error,
                        },
                    }) if canonical_repo == &generated[0]
                        && selected_missing.disposition()
                            == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing
                        && matches!(
                            error.kind,
                            HostGeneratedRepositoryDefinitionErrorKind::Loading(_)
                        )
                )
        ));
        assert!(!HostCanonicalRepositoryRouteKey::equality(
            &generated_value,
            &generated_terminal,
        ));
        let missing_key = key(CanonicalRepoName::new("missing").unwrap());
        let missing = tx.compute(&missing_key).await.unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryRouteError {
                        canonical_repo,
                        kind: HostCanonicalRepositoryRouteErrorKind::Missing {
                            selected_missing,
                            generated_missing,
                        },
                    }) if canonical_repo.as_str() == "missing"
                        && selected_missing.disposition()
                            == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing
                        && matches!(
                            generated_missing.kind,
                            HostGeneratedRepositoryDefinitionErrorKind::Missing { .. }
                        )
                )
        ));
        let warm = tx.compute(&root_key).await.unwrap();
        assert!(HostCanonicalRepositoryRouteKey::equality(&root, &warm));
        assert_eq!(route_hash(&root), route_hash(&warm));
        assert_eq!(
            *tracker.canonical.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert_eq!(tracker.lookup.lock().unwrap().len(), 2);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let generated_b = canonical_lookup(&dice, MODULE, EXTENSION_B, generated[0].clone()).await;
        assert!(!HostCanonicalRepositoryRouteKey::equality(
            &generated_value,
            &generated_b,
        ));
        assert_ne!(route_hash(&generated_value), route_hash(&generated_b));
        assert!(HostCanonicalRepositoryRouteKey::equality(
            &generated_value,
            &canonical_lookup(&dice, MODULE, EXTENSION_A, generated[0].clone()).await,
        ));
        let selected_b = canonical_lookup(
            &dice,
            &MODULE.replace(
                "module(name='bazel_tools')",
                "module(name='bazel_tools', repo_name='changed')",
            ),
            EXTENSION_A,
            CanonicalRepoName::root(),
        )
        .await;
        assert!(!HostCanonicalRepositoryRouteKey::equality(
            &root,
            &selected_b
        ));
        assert_ne!(route_hash(&root), route_hash(&selected_b));
        assert!(HostCanonicalRepositoryRouteKey::equality(
            &root,
            &canonical_lookup(&dice, MODULE, EXTENSION_A, CanonicalRepoName::root(),).await,
        ));
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(crate::bzl_load_cycle_detector()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let need = updater
            .commit()
            .await
            .compute(&generated_key)
            .await
            .unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostCanonicalRepositoryRouteKey::validity(&need));
        assert!(!HostCanonicalRepositoryRouteKey::equality(&need, &need));
    }
    #[tokio::test]
    async fn canonical_route_borrows_nonregistry_selected_view() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let local_module = "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let local_key = HostCanonicalRepositoryRouteKey::new(
            workspace.clone(),
            CanonicalRepoName::new("local+").unwrap(),
        );
        let local_mapping_key = HostCanonicalRepositoryApparentMappingKey::new(
            workspace.clone(),
            CanonicalRepoName::new("local+").unwrap(),
            ApparentRepoName::new("bazel_tools").unwrap(),
        );
        let local_need = transaction(
            &dice,
            local_module,
            EXTENSION_A,
            true,
            Some(tracker.clone()),
        )
        .await
        .compute(&local_mapping_key)
        .await
        .unwrap();
        assert!(!HostCanonicalRepositoryApparentMappingKey::validity(
            &local_need
        ));
        assert!(tracker.lookup.lock().unwrap().is_empty());
        let SourcePreparationOutcome::Need(need) = local_need else {
            panic!("local definition must first request materialization")
        };
        let request = need
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .clone();
        let mut updater = dice.updater_with_data(UserComputationData {
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.clone(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace,
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
        let mut local_tx = updater.commit().await;
        let local_outcome = local_tx.compute(&local_key).await.unwrap();
        let SourcePreparationOutcome::Complete(local) = &local_outcome else {
            panic!("local definition must complete: {local_outcome:?}")
        };
        let local = local.as_ref().as_ref().unwrap();
        let local_view = local.view();
        assert_eq!(
            local_view.kind(),
            HostCanonicalRepositoryRouteKind::SelectedNonregistry
        );
        assert_eq!(local_view.canonical_repo().as_str(), "local+");
        assert_eq!(local_view.mapping_context().as_str(), "local+");
        assert_eq!(
            local_view.local_path_policy(),
            Some(HostRepositoryLocalPathPolicy::WorkspaceRelative)
        );
        assert_eq!(
            local_view.repo_spec().unwrap().rule_id.rule_name,
            "local_repository"
        );
        let local_warm = local_tx.compute(&local_key).await.unwrap();
        assert!(HostCanonicalRepositoryRouteKey::equality(
            &local_outcome,
            &local_warm
        ));
        assert_eq!(route_hash(&local_outcome), route_hash(&local_warm));
        tracker.canonical.lock().unwrap().clear();
        tracker.lookup.lock().unwrap().clear();
        tracker.apparent.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let mapping_value = local_tx.compute(&local_mapping_key).await.unwrap();
        let selected_target = local
            .mapping_target(&ApparentRepoName::new("bazel_tools").unwrap())
            .unwrap();
        assert_eq!(target(&mapping_value), selected_target.clone());
        let borrowed_target = mapping(&mapping_value).resolved_target().unwrap();
        assert!(std::ptr::eq(selected_target, borrowed_target));
        assert_eq!(
            *tracker.canonical.lock().unwrap(),
            [(ActivationKind::Reused, false)]
        );
        assert!(tracker.lookup.lock().unwrap().is_empty());
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [(ActivationKind::Evaluated, false)]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        tracker.canonical.lock().unwrap().clear();
        tracker.root_mapping.lock().unwrap().clear();
        let root_local = local_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("local_alias").unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(target(&root_local).as_str(), "local+");
        assert!(tracker.canonical.lock().unwrap().is_empty());
        assert_eq!(
            *tracker.root_mapping.lock().unwrap(),
            [(ActivationKind::Evaluated, false)]
        );
    }

    #[tokio::test]
    async fn real_lookup_borrows_exact_definition_and_restores() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let validation = validated(&mut tx).await;
        let generated = names(&validation);
        assert_eq!(generated.len(), 2);
        tracker.forbidden.lock().unwrap().clear();

        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let first_key =
            HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), generated[0].clone());
        let second_key =
            HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), generated[1].clone());
        let first = tx.compute(&first_key).await.unwrap();
        let second = tx.compute(&second_key).await.unwrap();
        let warm = tx.compute(&first_key).await.unwrap();
        assert!(HostGeneratedRepositoryDefinitionKey::validity(&first));
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &first, &warm
        ));

        let SourcePreparationOutcome::Complete(first_value) = &first else {
            panic!("lookup must complete")
        };
        let SourcePreparationOutcome::Complete(second_value) = &second else {
            panic!("lookup must complete")
        };
        let first_route = first_value.as_ref().as_ref().unwrap();
        let second_route = second_value.as_ref().as_ref().unwrap();
        let first_view = first_route.view();
        let second_view = second_route.view();
        assert_eq!(first_view.canonical_repo(), &generated[0]);
        assert_eq!(first_view.internal_name(), Some("first"));
        assert_eq!(
            first_view.repo_spec().unwrap().rule_id.rule_name.as_str(),
            "repo"
        );
        assert_eq!(second_view.internal_name(), Some("second"));
        assert!(matches!(
            second_view.repo_spec().unwrap().attributes.get("value"),
            Some(slug_bzlmod_v2::OverrideAttributeValue::String(value)) if value == "two"
        ));
        assert_eq!(first_view.mapping_context(), &generated[0]);
        assert_eq!(second_view.mapping_context(), &generated[1]);
        let first_seed = first_view.generated_effect_seed().unwrap();
        let second_seed = second_view.generated_effect_seed().unwrap();
        assert_eq!(first_seed.ordinal(), 0);
        assert_eq!(second_seed.ordinal(), 1);
        assert_eq!(first_seed.owner(), second_seed.owner());

        let missing_key = HostGeneratedRepositoryDefinitionKey::new(
            workspace.clone(),
            CanonicalRepoName::new("missing").unwrap(),
        );
        let missing = tx.compute(&missing_key).await.unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostGeneratedRepositoryDefinitionError {
                        kind: HostGeneratedRepositoryDefinitionErrorKind::Missing { .. },
                        ..
                    })
                )
        ));

        let baseline = snapshot(&first);
        for (case, (module, extension, changed_fields)) in [
            (MODULE.to_owned(), EXTENSION_B.to_owned(), &[2][..]),
            (
                MODULE.replace("first='first'", "first='renamed'"),
                EXTENSION_A.replacen("name='first'", "name='renamed'", 1),
                &[0, 1, 4, 5],
            ),
            (MODULE.to_owned(), EXTENSION_A.replace("value", "renamed_value"), &[3][..]),
            (MODULE.to_owned(), EXTENSION_A.replace("value='one'", "value='changed'"), &[3][..]),
            (
                MODULE.to_owned(),
                EXTENSION_A.replace("value='one', target=':local'", "target=':local', value='one'"),
                &[][..],
            ),
            (MODULE.to_owned(), EXTENSION_A.replace("target=':local'", "target=':changed'"), &[3][..]),
            (
                MODULE.to_owned(),
                EXTENSION_A.replace(
                    "repo(name='first', value='one', target=':local')\n    repo(name='second', value='two', target='@first//:item')",
                    "repo(name='second', value='two', target='@first//:item')\n    repo(name='first', value='one', target=':local')",
                ),
                &[0, 1, 3, 4],
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let b = lookup(&dice, &module, &extension, None).await;
            if case == 4 {
                assert!(HostGeneratedRepositoryDefinitionKey::equality(&first, &b));
                assert_eq!(generated_route_hash(&first), generated_route_hash(&b));
            } else {
                assert!(!HostGeneratedRepositoryDefinitionKey::equality(&first, &b));
            }
            let changed = snapshot(&b);
            assert!(
                changed_fields.iter().all(|index| baseline[*index] != changed[*index]),
                "case {case}: {baseline:?} == {changed:?}"
            );
            let a2 = lookup(&dice, MODULE, EXTENSION_A, None).await;
            assert!(HostGeneratedRepositoryDefinitionKey::equality(&first, &a2));
        }

        let inject_a = format!(
            "{MODULE}inject_repo(e, injected='bazel_tools')\ninject_repo(e, other='bazel_tools')\n"
        );
        let inject_b = format!(
            "{MODULE}inject_repo(e, other='bazel_tools')\ninject_repo(e, injected='bazel_tools')\n"
        );
        let mapping_a = lookup(&dice, &inject_a, EXTENSION_A, None).await;
        let mapping_b = lookup(&dice, &inject_b, EXTENSION_A, None).await;
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &mapping_a, &mapping_b,
        ));
        assert_eq!(
            generated_route_hash(&mapping_a),
            generated_route_hash(&mapping_b)
        );
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &mapping_a,
            &lookup(&dice, &inject_a, EXTENSION_A, None).await,
        ));
        let overridden = lookup(
            &dice,
            &format!("{MODULE}override_repo(e, first='bazel_tools')\n"),
            EXTENSION_A,
            None,
        )
        .await;
        assert_ne!(baseline[5], snapshot(&overridden)[5]);
        assert!(!HostGeneratedRepositoryDefinitionKey::equality(
            &first,
            &overridden,
        ));
        assert_ne!(
            generated_route_hash(&first),
            generated_route_hash(&overridden)
        );
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &first,
            &lookup(&dice, MODULE, EXTENSION_A, None).await,
        ));

        let multi_extension = EXTENSION_A.replace(
            "ext=module_extension(implementation=impl)",
            "first=module_extension(implementation=impl)\nsecond=module_extension(implementation=impl)",
        );
        let request_a = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\n";
        let request_b = "module(name='bazel_tools')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\n";
        let order_a = lookup(&dice, request_a, &multi_extension, None).await;
        let fixed = CanonicalRepoName::new(&snapshot(&order_a)[0]).unwrap();
        let order_b = lookup(&dice, request_b, &multi_extension, Some(&fixed)).await;
        assert_eq!(&snapshot(&order_a)[..2], &snapshot(&order_b)[..2]);
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &order_a, &order_b
        ));
        assert_eq!(
            generated_route_hash(&order_a),
            generated_route_hash(&order_b)
        );
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &order_a,
            &lookup(&dice, request_a, &multi_extension, Some(&fixed)).await,
        ));
        assert_eq!(
            *tracker.lookup.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
                (ActivationKind::Evaluated, false),
            ]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn real_apparent_mapping_borrows_effective_target_and_restores() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let generated = names(&validated(&mut tx).await);
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let definition_key =
            HostCanonicalRepositoryRouteKey::new(workspace.clone(), generated[0].clone());
        tx.compute(&definition_key).await.unwrap();
        tracker.canonical.lock().unwrap().clear();
        tracker.lookup.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let key = |context: CanonicalRepoName, apparent: &str| {
            HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                context,
                ApparentRepoName::new(apparent).unwrap(),
            )
        };
        let self_key = key(generated[0].clone(), "first");
        let sibling_key = key(generated[0].clone(), "second");
        let host_key = key(generated[0].clone(), "bazel_tools");
        let self_mapping = tx.compute(&self_key).await.unwrap();
        let sibling_mapping = tx.compute(&sibling_key).await.unwrap();
        let host_mapping = tx.compute(&host_key).await.unwrap();
        let warm = tx.compute(&self_key).await.unwrap();
        assert_eq!(target(&self_mapping), generated[0]);
        assert_eq!(target(&sibling_mapping), generated[1]);
        assert_eq!(target(&host_mapping), CanonicalRepoName::root());
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &sibling_mapping,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &warm,
        ));
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert_eq!(
            *tracker.canonical.lock().unwrap(),
            [
                (ActivationKind::Reused, false),
                (ActivationKind::Reused, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert!(tracker.lookup.lock().unwrap().is_empty());
        assert!(tracker.root_mapping.lock().unwrap().is_empty());
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let missing = tx
            .compute(&key(generated[0].clone(), "missing"))
            .await
            .unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Missing { .. },
                        ..
                    })
                )
        ));
        let predecessor_activations = tracker.canonical.lock().unwrap().len();
        let root_apparent = tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                generated[0].clone(),
                ApparentRepoName::root(),
            ))
            .await
            .unwrap();
        let root_context = tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("first").unwrap(),
            ))
            .await
            .unwrap();
        let root_self = tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                CanonicalRepoName::root(),
                ApparentRepoName::root(),
            ))
            .await
            .unwrap();
        assert_eq!(
            tracker.canonical.lock().unwrap().len(),
            predecessor_activations
        );
        assert!(matches!(
            root_apparent,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::RootApparent,
                        ..
                    })
                )
        ));
        assert_eq!(target(&root_context), generated[0]);
        assert_eq!(target(&root_self), CanonicalRepoName::root());
        let ApparentMappingPredecessor::Root(root_predecessor) =
            &mapping(&root_context).predecessor
        else {
            panic!("root lookup must retain root mapping predecessor")
        };
        let published_target = root_predecessor
            .view()
            .unwrap()
            .mapping()
            .find_map(|(name, target)| (name.as_str() == "first").then_some(target))
            .unwrap();
        assert!(std::ptr::eq(
            published_target,
            mapping(&root_context).resolved_target().unwrap(),
        ));
        tx.compute(&HostRootRepositoryMappingKey::new(workspace.clone()))
            .await
            .unwrap();
        tracker.root_mapping.lock().unwrap().clear();
        tracker.canonical.lock().unwrap().clear();
        tracker.apparent.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let root_builtin_key = HostCanonicalRepositoryApparentMappingKey::new(
            workspace.clone(),
            CanonicalRepoName::root(),
            ApparentRepoName::new("bazel_tools").unwrap(),
        );
        let root_builtin = tx.compute(&root_builtin_key).await.unwrap();
        assert_eq!(target(&root_builtin), CanonicalRepoName::root());
        assert_eq!(
            *tracker.root_mapping.lock().unwrap(),
            [(ActivationKind::Reused, false)]
        );
        assert!(tracker.canonical.lock().unwrap().is_empty());
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [(ActivationKind::Evaluated, false)]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let root_builtin_warm = tx.compute(&root_builtin_key).await.unwrap();
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_builtin,
            &root_builtin_warm,
        ));
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert_eq!(tracker.root_mapping.lock().unwrap().len(), 1);
        assert!(tracker.canonical.lock().unwrap().is_empty());
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        async fn resolve(
            dice: &Arc<Dice>,
            module: &str,
            apparent: &str,
        ) -> HostCanonicalRepositoryApparentMappingOutcome {
            let mut tx = transaction(dice, module, EXTENSION_A, true, None).await;
            let context = names(&validated(&mut tx).await).remove(0);
            tx.compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                context,
                ApparentRepoName::new(apparent).unwrap(),
            ))
            .await
            .unwrap()
        }
        async fn resolve_root(
            dice: &Arc<Dice>,
            module: &str,
            apparent: &str,
        ) -> HostCanonicalRepositoryApparentMappingOutcome {
            transaction(dice, module, EXTENSION_A, true, None)
                .await
                .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                    CanonicalRepoName::root(),
                    ApparentRepoName::new(apparent).unwrap(),
                ))
                .await
                .unwrap()
        }
        let override_module = format!("{MODULE}override_repo(e, first='bazel_tools')\n");
        let overridden = resolve(&dice, &override_module, "first").await;
        assert_eq!(target(&overridden), CanonicalRepoName::root());
        let SourcePreparationOutcome::Complete(overridden_value) = &overridden else {
            panic!("override mapping must complete")
        };
        let ApparentMappingPredecessor::Canonical(overridden_predecessor) =
            &overridden_value.as_ref().as_ref().unwrap().predecessor
        else {
            panic!("overridden mapping must retain canonical predecessor")
        };
        let overridden_view = overridden_predecessor.view();
        assert_eq!(
            overridden_view.kind(),
            HostCanonicalRepositoryRouteKind::Generated
        );
        assert_eq!(
            overridden_view
                .repo_spec()
                .unwrap()
                .rule_id
                .rule_name
                .as_str(),
            "repo"
        );
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &overridden,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &resolve(&dice, MODULE, "first").await,
        ));
        let root_a = resolve_root(&dice, MODULE, "first").await;
        let root_b = resolve_root(&dice, &override_module, "first").await;
        assert_eq!(target(&root_a), generated[0]);
        assert_eq!(target(&root_b), CanonicalRepoName::root());
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a, &root_b,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let order_module = "module(name='bazel_tools')\n\
            e=use_extension('//:ext.bzl','ext')\n\
            use_repo(e, second='second', first='first')\n";
        let root_order = resolve_root(&dice, order_module, "first").await;
        assert_eq!(target(&root_order), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &root_order,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let named_root_module = MODULE.replacen(
            "module(name='bazel_tools')",
            "module(name='bazel_tools', repo_name='root_self')",
            1,
        );
        let named_root = resolve_root(&dice, &named_root_module, "first").await;
        assert_eq!(target(&named_root), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &named_root,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let alternate_extension = EXTENSION_A.replace(
            "ext=module_extension(implementation=impl)",
            "ext=module_extension(implementation=impl)\nother=module_extension(implementation=impl)",
        );
        let alternate_module = "module(name='bazel_tools')\n\
            e=use_extension('//:ext.bzl','other')\n\
            use_repo(e, first='first', second='second')\n";
        let alternate_root = transaction(&dice, alternate_module, &alternate_extension, true, None)
            .await
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("first").unwrap(),
            ))
            .await
            .unwrap();
        assert_ne!(target(&alternate_root), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &alternate_root,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let mut invalid_tx = transaction(
            &dice,
            "this is not valid Starlark\n",
            EXTENSION_A,
            true,
            None,
        )
        .await;
        let direct_error = invalid_tx
            .compute(&HostRootRepositoryMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(direct_error) = direct_error else {
            panic!("invalid root mapping must complete")
        };
        let direct_error = direct_error.as_ref().as_ref().unwrap_err().clone();
        let root_terminal = invalid_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("first").unwrap(),
            ))
            .await
            .unwrap();
        assert!(matches!(
            &root_terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        context_repo,
                        apparent_repo,
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::RootMapping(error),
                        ..
                    }) if context_repo.is_root()
                        && apparent_repo.as_str() == "first"
                        && error == &direct_error
                )
        ));
        let root_injected = resolve_root(
            &dice,
            &format!("{MODULE}inject_repo(e, injected='bazel_tools')\n"),
            "first",
        )
        .await;
        assert_eq!(target(&root_injected), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &root_injected,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &self_mapping,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let root_missing = resolve_root(&dice, MODULE, "missing").await;
        assert!(matches!(
            root_missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Missing {
                            predecessor: ApparentMappingPredecessor::Root(_),
                        },
                        ..
                    })
                )
        ));
        let injected = resolve(
            &dice,
            &format!("{MODULE}inject_repo(e, injected='bazel_tools')\n"),
            "injected",
        )
        .await;
        assert_eq!(target(&injected), CanonicalRepoName::root());
        let injected_context = {
            let SourcePreparationOutcome::Complete(value) = &injected else {
                panic!("injected mapping must complete")
            };
            let ApparentMappingPredecessor::Canonical(predecessor) =
                &value.as_ref().as_ref().unwrap().predecessor
            else {
                panic!("injected mapping must retain canonical predecessor")
            };
            predecessor.view().canonical_repo().clone()
        };
        let invalid_override_module = format!("{MODULE}override_repo(e, injected='bazel_tools')\n");
        let mut invalid_override_tx =
            transaction(&dice, &invalid_override_module, EXTENSION_A, true, None).await;
        let invalid_override = invalid_override_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                injected_context,
                ApparentRepoName::new("injected").unwrap(),
            ))
            .await
            .unwrap();
        assert!(matches!(
            &invalid_override,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Route(_),
                        ..
                    })
                )
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &injected,
            &invalid_override,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &injected,
            &resolve(
                &dice,
                &format!("{MODULE}inject_repo(e, injected='bazel_tools')\n"),
                "injected",
            )
            .await,
        ));
        let inject_a = format!(
            "{MODULE}inject_repo(e, injected='bazel_tools')\ninject_repo(e, other='bazel_tools')\n"
        );
        let inject_b = format!(
            "{MODULE}inject_repo(e, other='bazel_tools')\ninject_repo(e, injected='bazel_tools')\n"
        );
        let order_a = resolve(&dice, &inject_a, "injected").await;
        let order_b = resolve(&dice, &inject_b, "injected").await;
        assert_eq!(target(&order_a), target(&order_b));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &order_a, &order_b,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &order_a,
            &resolve(&dice, &inject_a, "injected").await,
        ));
        let multi_extension = EXTENSION_A.replace(
            "ext=module_extension(implementation=impl)",
            "first=module_extension(implementation=impl)\nsecond=module_extension(implementation=impl)",
        );
        let multi_module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first_a='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, first_b='first')\n";
        let mut multi_tx = transaction(&dice, multi_module, &multi_extension, true, None).await;
        let contexts = names(&validated(&mut multi_tx).await);
        let first_context = HostCanonicalRepositoryApparentMappingKey::new(
            workspace.clone(),
            contexts[0].clone(),
            ApparentRepoName::new("first").unwrap(),
        );
        let second_context = HostCanonicalRepositoryApparentMappingKey::new(
            workspace,
            contexts[2].clone(),
            ApparentRepoName::new("first").unwrap(),
        );
        let isolated_a = multi_tx.compute(&first_context).await.unwrap();
        let isolated_b = multi_tx.compute(&second_context).await.unwrap();
        assert_eq!(target(&isolated_a), contexts[0]);
        assert_eq!(target(&isolated_b), contexts[2]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &isolated_a,
            &isolated_b,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &isolated_a,
            &multi_tx.compute(&first_context).await.unwrap(),
        ));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_generated_definition_identity_scan_and_terminal_algebra() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let requested = CanonicalRepoName::new("generated").unwrap();
        let key = HostGeneratedRepositoryDefinitionObservationKey::new(NormalizedAbsolutePath::new("/workspace").unwrap(), requested.clone());
        let same = HostGeneratedRepositoryDefinitionObservationKey::new(NormalizedAbsolutePath::new("/workspace").unwrap(), requested.clone());
        let other = HostGeneratedRepositoryDefinitionObservationKey::new(NormalizedAbsolutePath::new("/workspace").unwrap(), CanonicalRepoName::new("other").unwrap());
        let hash = |key: &HostGeneratedRepositoryDefinitionObservationKey| {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(key.to_string(), "observed-host-generated-repository-definition:\"/workspace\":@@generated");
        assert_eq!(key, same);
        assert_ne!(key, other);
        assert_eq!(hash(&key), hash(&same));
        assert_ne!(hash(&key), hash(&other));

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut initial = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let generated = names(&validated(&mut initial).await);
        let success = observed_lookup(&dice, MODULE, EXTENSION_A, true, generated[0].clone(), None).await;
        let carrier = observed_carrier(&success);
        assert!(carrier.result().is_ok());
        assert!(!carrier.observations().observations().is_empty());
        assert!(HostGeneratedRepositoryDefinitionObservationKey::validity(&success));
        assert!(HostGeneratedRepositoryDefinitionObservationKey::equality(&success, &success));

        let missing_name = CanonicalRepoName::new("missing").unwrap();
        let missing = observed_lookup(&dice, MODULE, EXTENSION_A, true, missing_name.clone(), None).await;
        assert!(matches!(
            observed_carrier(&missing).result().as_ref(),
            Err(HostGeneratedRepositoryDefinitionError {
                requested,
                kind: HostGeneratedRepositoryDefinitionErrorKind::Missing {},
            }) if requested == &missing_name
        ));
        let loading = observed_lookup(&dice, MODULE, EXTENSION_A, false, generated[0].clone(), None).await;
        assert!(matches!(
            observed_carrier(&loading).result().as_ref(),
            Err(HostGeneratedRepositoryDefinitionError {
                kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(_),
                ..
            })
        ));

        let duplicate = complete_generated_driver(
            Err(HostGeneratedRepositoryDefinitionError {
                requested: generated[0].clone(),
                kind: HostGeneratedRepositoryDefinitionErrorKind::Duplicate {
                    first: 0,
                    conflicting: 2,
                },
            }),
            carrier.observations().clone(),
        );
        assert!(matches!(
            duplicate,
            SourcePreparationOutcome::Complete(Ok((value, observations)))
                if matches!(value.as_ref(), Err(HostGeneratedRepositoryDefinitionError {
                    kind: HostGeneratedRepositoryDefinitionErrorKind::Duplicate { first: 0, conflicting: 2, .. },
                    ..
                })) && observations == *carrier.observations()
        ));
        let compute = complete_generated_driver(
            Err(HostGeneratedRepositoryDefinitionError {
                requested: generated[0].clone(),
                kind: HostGeneratedRepositoryDefinitionErrorKind::LoadingCompute("failed".into()),
            }),
            PathObservationEpoch::empty(),
        );
        assert!(matches!(compute, SourcePreparationOutcome::Complete(Ok((value, observations)))
            if matches!(value.as_ref(), Err(HostGeneratedRepositoryDefinitionError {
                kind: HostGeneratedRepositoryDefinitionErrorKind::LoadingCompute(message), ..
            }) if message.as_ref() == "failed") && observations.observations().is_empty()));

        let consumed = Cell::new(0);
        let unrelated = CanonicalRepoName::new("unrelated").unwrap();
        let scan = [&generated[0], &unrelated, &generated[0], &unrelated];
        assert_eq!(
            find_unique_ordinal(&generated[0], scan.into_iter().inspect(|_| consumed.set(consumed.get() + 1)),),
            Err(UniqueOrdinalError::Duplicate { first: 0, conflicting: 2 })
        );
        assert_eq!(consumed.get(), scan.len());

        let tracker = Arc::new(LookupTracker::default());
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(crate::bzl_load_cycle_detector()),
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        });
        updater.changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())]).unwrap();
        let observed_key = HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), generated[0].clone());
        let need = updater.commit().await.compute(&observed_key).await.unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostGeneratedRepositoryDefinitionObservationKey::validity(&need));
        assert!(!HostGeneratedRepositoryDefinitionObservationKey::equality(&need, &need));
        assert_eq!(
            tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &observed_key.to_string()).unwrap().1,
            [HostSelectedExtensionDemandObservationKey::new(workspace, generated[0].clone()).to_string()]
        );

        let source = include_str!("generated_repository_definition.rs");
        let producer = &source[source.find("type GeneratedRepositoryDefinitionResult").unwrap()..];
        assert_eq!(producer.matches("HostSelectedExtensionDemandObservationKey::new").count(), 1);
        assert_eq!(producer.matches("HostSelectedExtensionOwnerCertificateObservationKey::new").count(), 1);
        assert!(producer.find("HostSelectedExtensionDemandObservationKey::new").unwrap() < producer.find("HostSelectedExtensionOwnerCertificateObservationKey::new").unwrap());
        assert!(!producer.contains("HostCanonicalSelectedModuleDefinitionKey"));
        assert!(producer.contains("merge_generated_observations"));
        assert!(!producer.contains("store_evaluation_data"));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_generated_definition_real_order_events_and_parity() {
        let extension = r#"print('load')
repo=repository_rule(implementation=lambda ctx: None)
def first_impl(ctx):
    print('invoke-first')
    repo(name='first')
def second_impl(ctx):
    print('invoke-second')
    repo(name='second')
first=module_extension(implementation=first_impl)
second=module_extension(implementation=second_impl)
"#;
        let module =
            "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\n";
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, module, extension, true, None).await;
        let requested = names(&validated(&mut prep_tx).await).remove(0);
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, module, extension, true, Some(tracker.clone())).await;
        let observed_key = HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), requested.clone());
        let observed = tx.compute(&observed_key).await.unwrap();
        let carrier = observed_carrier(&observed);
        let legacy_key = HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), requested.clone());
        let dependencies = tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &observed_key.to_string()).unwrap().1.clone();
        assert_eq!(dependencies.len(), 2);
        assert_eq!(dependencies[0], HostSelectedExtensionDemandObservationKey::new(workspace.clone(), requested.clone()).to_string());
        assert!(dependencies[1].starts_with("observed-host-selected-extension-owner-certificate:"));
        assert!(tracker.selected.lock().unwrap().is_empty());
        let activations = tracker.activations.lock().unwrap();
        let parent = activations.iter().find(|(name, _, _)| name == &observed_key.to_string()).unwrap();
        assert_eq!(parent.1, ActivationKind::Evaluated);
        assert!(parent.2.is_none());
        let prints = activations
            .iter()
            .filter_map(|(_, _, batch)| batch.as_ref())
            .flat_map(EventBatch::events)
            .filter_map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(prints, ["load", "invoke-first"], "activations: {activations:#?}");
        assert!(
            activations
                .iter()
                .filter(|(name, _, _)| {
                    name.contains("instantiated-module-extension-repositories:") || name.contains("validated-module-extension-repositories:") || name == &observed_key.to_string()
                })
                .all(|(_, _, batch)| batch.is_none())
        );
        drop(activations);

        let legacy = tx.compute(&legacy_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("legacy generated definition must complete")
        };
        assert_eq!(legacy.as_ref(), carrier.result().as_ref());
        let snapshot = snapshot(&SourcePreparationOutcome::Complete(legacy.clone()));
        assert_eq!(snapshot[0], requested.as_str());
        assert_eq!(&snapshot[1..3], ["first", "repo"]);
        assert_eq!(snapshot[4], requested.as_str());

        tracker.activations.lock().unwrap().clear();
        let warm = tx.compute(&observed_key).await.unwrap();
        assert!(HostGeneratedRepositoryDefinitionObservationKey::equality(&observed, &warm));
        assert!(Arc::ptr_eq(carrier.result(), observed_carrier(&warm).result()));
        assert!(
            tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .any(|(name, kind, batch)| { name == &observed_key.to_string() && *kind == ActivationKind::Reused && batch.is_none() })
        );
        assert!(tracker.activations.lock().unwrap().iter().all(|(_, _, batch)| batch.is_none()));

        for (present, requested_case, expected) in [(false, requested.clone(), "Loading"), (true, CanonicalRepoName::new("missing").unwrap(), "Missing")] {
            let case_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let observed = observed_lookup(&case_dice, module, extension, present, requested_case.clone(), None).await;
            let mut legacy_tx = transaction(&case_dice, module, extension, present, None).await;
            let legacy = legacy_tx
                .compute(&HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), requested_case))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(legacy) = legacy else {
                panic!("legacy terminal must complete")
            };
            assert_eq!(legacy.as_ref(), observed_carrier(&observed).result().as_ref());
            assert!(format!("{:?}", observed_carrier(&observed).result()).contains(expected));
        }

        let terminal_tracker = Arc::new(LookupTracker::default());
        let _ = observed_lookup(
            &Dice::builder().build(DetectCycles::Enabled),
            module,
            extension,
            false,
            requested,
            Some(terminal_tracker.clone()),
        )
        .await;
        assert!(
            terminal_tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .filter_map(|(_, _, batch)| batch.as_ref())
                .flat_map(EventBatch::events)
                .all(|event| !matches!(event, EvaluationEvent::StarlarkPrint { .. }))
        );
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_generated_definition_lifecycle_cancellation_and_nonactivation() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, MODULE, EXTENSION_A, true, None).await;
        let requested = names(&validated(&mut prep_tx).await).remove(0);
        let order = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, second='second', first='first')\n";
        let mapping = format!("{MODULE}override_repo(e, first='bazel_tools')\n");
        let same_semantic = format!("{MODULE}\n");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), requested.clone());
        let mut held = Vec::new();
        for (module, extension) in [
            (MODULE, EXTENSION_A),
            (MODULE, EXTENSION_B),
            (MODULE, EXTENSION_A),
            (order, EXTENSION_A),
            (MODULE, EXTENSION_A),
            (mapping.as_str(), EXTENSION_A),
            (MODULE, EXTENSION_A),
            (same_semantic.as_str(), EXTENSION_A),
        ] {
            let mut tx = transaction(&dice, module, extension, true, None).await;
            let global = tx.compute(&PathObservationEpochKey).await.unwrap();
            let carrier = observed_carrier(&tx.compute(&key).await.unwrap()).clone();
            let child = tx.compute(&HostValidatedModuleExtensionRepositoriesObservationKey::new(workspace.clone())).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child else {
                panic!("observed validation child must complete")
            };
            assert_eq!(carrier.observations(), child.observations());
            for (demand, result) in carrier.observations().observations() {
                assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
            }
            held.push(carrier);
        }
        assert_ne!(held[0].result(), held[1].result());
        assert_eq!(held[0].result(), held[2].result());
        assert_eq!(held[2].result(), held[3].result());
        assert_eq!(held[2].result(), held[4].result());
        assert_ne!(held[4].result(), held[5].result());
        assert_eq!(held[4].result(), held[6].result());
        assert_eq!(held[0].result(), held[7].result());
        assert_ne!(held[0].observations(), held[7].observations());

        let warm_tracker = Arc::new(LookupTracker::default());
        let mut warm_tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(warm_tracker.clone())).await;
        let first = observed_carrier(&warm_tx.compute(&key).await.unwrap()).clone();
        warm_tracker.activations.lock().unwrap().clear();
        let reused = observed_carrier(&warm_tx.compute(&key).await.unwrap()).clone();
        assert!(Arc::ptr_eq(first.result(), reused.result()));
        assert!(
            warm_tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .any(|(name, kind, batch)| { name == &key.to_string() && *kind == ActivationKind::Reused && batch.is_none() })
        );

        let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let cancel_tracker = Arc::new(LookupTracker::default());
        let mut cancelled = transaction(&cancel_dice, MODULE, EXTENSION_A, true, Some(cancel_tracker.clone())).await;
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(cancel_tracker.activations.lock().unwrap().iter().all(|(name, _, _)| name != &key.to_string()));
        assert!(cancel_tracker.dependencies.lock().unwrap().iter().all(|(name, _)| name != &key.to_string()));

        let mut recovery = transaction(&cancel_dice, MODULE, EXTENSION_A, true, Some(cancel_tracker.clone())).await;
        let global = recovery.compute(&PathObservationEpochKey).await.unwrap();
        let recovered = observed_carrier(&recovery.compute(&key).await.unwrap()).clone();
        assert_eq!(recovered.result(), held[0].result());
        for (demand, result) in recovered.observations().observations() {
            assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
        }
        let activations = cancel_tracker.activations.lock().unwrap();
        let dependencies = cancel_tracker.dependencies.lock().unwrap();
        let forbidden_captures = cancel_tracker.forbidden.lock().unwrap();
        assert!(!forbidden_captures.is_empty());
        assert!(forbidden_captures.iter().all(|capture| *capture == "filesystem"));
        let legacy = HostGeneratedRepositoryDefinitionKey::new(workspace, requested).to_string();
        assert!(activations.iter().all(|(name, _, _)| name != &legacy));
        assert!(dependencies.iter().all(|(name, children)| name != &legacy && children.iter().all(|child| child != &legacy)));
        for forbidden in [
            "host-canonical-selected-module-definition:",
            "host-canonical-repository-route:",
            "host-canonical-repository-apparent-mapping:",
            "host-root-repository-mapping:",
            "host-root-apparent-repository-definition:",
            "HostRootApparentRepositoryRouteKey",
            "HostRootApparentRepositorySourceInputKey",
            "HostRootApparentRepositorySourceObservationKey",
            "HostRootApparentRepositorySourcePathInputKey",
            "root-repository-route:",
            "repository-package-source:",
            "repository-source-file:",
            "host-repository-source-file:",
            "build-command-root:",
        ] {
            assert!(activations.iter().all(|(name, _, _)| !name.contains(forbidden)));
            assert!(
                dependencies
                    .iter()
                    .all(|(name, children)| !name.contains(forbidden) && children.iter().all(|child| !child.contains(forbidden)))
            );
        }
        let source = include_str!("generated_repository_definition.rs");
        let producer = &source[source.find("type GeneratedRepositoryDefinitionResult").unwrap()..];
        assert!(!producer.contains("RootModuleBootstrap"));
        assert!(!producer.contains("HostCanonicalRepositoryRouteKey::new"));
        assert!(!producer.contains("HostCanonicalRepositoryApparentMappingKey::new"));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_route_identity_staging_and_terminal_algebra() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let requested = CanonicalRepoName::new("requested").unwrap();
        let key = HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), requested.clone());
        let same = HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), requested.clone());
        let other = HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), CanonicalRepoName::new("other").unwrap());
        let hash = |key: &HostCanonicalRepositoryRouteObservationKey| { let mut hasher = DefaultHasher::new(); key.hash(&mut hasher); hasher.finish() };
        assert_eq!(key.to_string(), format!("observed-host-canonical-repository-route:{workspace}:{requested}"));
        assert_eq!(key, same); assert_ne!(key, other); assert_eq!(hash(&key), hash(&same)); assert_ne!(hash(&key), hash(&other));

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let root = observed_canonical_lookup(&dice, MODULE, EXTENSION_A, true, CanonicalRepoName::root(), None).await;
        let root_carrier = observed_canonical_carrier(&root);
        assert!(matches!(
            root_carrier.result().as_ref(),
            Ok(route) if route.view().kind() == HostCanonicalRepositoryRouteKind::Root
        ));
        assert!(!root_carrier.observations().observations().is_empty());
        assert!(HostCanonicalRepositoryRouteObservationKey::validity(&root));
        assert!(HostCanonicalRepositoryRouteObservationKey::equality(&root, &root));

        let mut prep = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let generated_name = names(&validated(&mut prep).await).remove(0);
        let generated = observed_canonical_lookup(&dice, MODULE, EXTENSION_A, true, generated_name.clone(), None).await;
        assert!(matches!(
            observed_canonical_carrier(&generated).result().as_ref(),
            Ok(route) if route.view().kind() == HostCanonicalRepositoryRouteKind::Generated
        ));
        let missing_name = CanonicalRepoName::new("missing").unwrap();
        let missing = observed_canonical_lookup(&dice, MODULE, EXTENSION_A, true, missing_name.clone(), None).await;
        assert!(matches!(observed_canonical_carrier(&missing).result().as_ref(), Err(HostCanonicalRepositoryRouteError { canonical_repo, kind: HostCanonicalRepositoryRouteErrorKind::Missing { selected_missing, generated_missing } }) if canonical_repo == &missing_name && selected_missing.disposition() == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing && matches!(generated_missing.kind, HostGeneratedRepositoryDefinitionErrorKind::Missing { .. })));
        let loading = observed_canonical_lookup(&dice, MODULE, EXTENSION_A, false, generated_name, None).await;
        assert!(matches!(observed_canonical_carrier(&loading).result().as_ref(), Err(HostCanonicalRepositoryRouteError { kind: HostCanonicalRepositoryRouteErrorKind::Generated { selected_missing, error: HostGeneratedRepositoryDefinitionError { kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(_), .. } }, .. }) if selected_missing.disposition() == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing));
        let selected_terminal = observed_canonical_lookup(&dice, "module(name='root')\nbazel_dep(name='missing', version='1')\n", EXTENSION_A, true, CanonicalRepoName::root(), None).await;
        assert!(matches!(observed_canonical_carrier(&selected_terminal).result().as_ref(), Err(HostCanonicalRepositoryRouteError { kind: HostCanonicalRepositoryRouteErrorKind::Selected(error), .. }) if error.disposition() == HostCanonicalSelectedModuleDefinitionErrorDisposition::Terminal));

        let mut selected_tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let selected_key = HostCanonicalSelectedModuleDefinitionObservationKey::new(workspace.clone(), missing_name.clone());
        let selected_value = selected_tx.compute(&selected_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(selected_carrier)) = &selected_value else { panic!("selected Missing carrier must complete") };
        let selected_missing = selected_carrier.result().as_ref().as_ref().unwrap_err().clone();
        let selected_epoch = selected_carrier.observations().clone();
        assert_eq!(selected_missing.disposition(), HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing);

        let selected_need_tracker = Arc::new(LookupTracker::default());
        let mut updater = dice.updater_with_data(UserComputationData { cycle_detector: Some(crate::bzl_load_cycle_detector()), activation_tracker: Some(selected_need_tracker.clone()), ..Default::default() });
        updater.changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())]).unwrap();
        let selected_need = updater.commit().await.compute(&key).await.unwrap();
        assert!(matches!(selected_need, SourcePreparationOutcome::Need(_)));
        assert_eq!(selected_need_tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &key.to_string()).unwrap().1, [HostCanonicalSelectedModuleDefinitionObservationKey::new(workspace.clone(), requested.clone()).to_string()]);

        let generated_need_tracker = Arc::new(LookupTracker::default());
        let mut updater = dice.updater_with_data(UserComputationData { cycle_detector: Some(crate::bzl_load_cycle_detector()), activation_tracker: Some(generated_need_tracker.clone()), ..Default::default() });
        updater.changed_to(vec![(PathObservationEpochKey, selected_epoch.clone())]).unwrap();
        let generated_need_key = HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), missing_name.clone());
        let generated_need = updater.commit().await.compute(&generated_need_key).await.unwrap();
        assert!(matches!(generated_need, SourcePreparationOutcome::Complete(_)));
        assert!(HostCanonicalRepositoryRouteObservationKey::validity(&generated_need));
        assert!(HostCanonicalRepositoryRouteObservationKey::equality(&generated_need, &generated_need));
        assert_eq!(generated_need_tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &generated_need_key.to_string()).unwrap().1, [selected_key.to_string(), HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), missing_name.clone()).to_string()]);

        let selected_compute = complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: requested.clone(), kind: HostCanonicalRepositoryRouteErrorKind::SelectedCompute("selected-dice".into()) }), PathObservationEpoch::empty());
        assert!(matches!(selected_compute, SourcePreparationOutcome::Complete(Ok((result, observations))) if matches!(result.as_ref(), Err(HostCanonicalRepositoryRouteError { canonical_repo, kind: HostCanonicalRepositoryRouteErrorKind::SelectedCompute(message) }) if canonical_repo.as_str() == requested.as_str() && message.as_ref() == "selected-dice") && observations.observations().is_empty()));
        let generated_compute = complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: missing_name.clone(), kind: HostCanonicalRepositoryRouteErrorKind::GeneratedCompute { selected_missing: selected_missing.clone(), message: "generated-dice".into() } }), selected_epoch.clone());
        assert!(matches!(generated_compute, SourcePreparationOutcome::Complete(Ok((result, observations))) if matches!(result.as_ref(), Err(HostCanonicalRepositoryRouteError { canonical_repo, kind: HostCanonicalRepositoryRouteErrorKind::GeneratedCompute { selected_missing: prefix, message } }) if canonical_repo.as_str() == missing_name.as_str() && prefix == &selected_missing && message.as_ref() == "generated-dice") && observations == selected_epoch));

        let demand = PathObservationDemand::new(PathObservationNamespace::Host, NormalizedAbsolutePath::new("/merge").unwrap(), PathObservationOperation::Lstat);
        let first = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let left = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let equal = PathObservationEpoch::from_shared([(demand.dupe(), Arc::new(first.as_ref().clone()))]).unwrap();
        let merged = merge_route_observations(&left, &equal).unwrap();
        assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first));
        let conflict = PathObservationEpoch::from_shared([(demand.dupe(), Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(PathNodeKind::RegularFile, 1, 1, 1, 1, 0o644))))) ]).unwrap();
        assert!(matches!(merge_route_observations(&left, &conflict), Err(ObservedPathFrontierError::Epoch(slug_workspace_v2::PathObservationEpochError::ConflictingDemand(found))) if found == demand));

        let generated_source = include_str!("generated_repository_definition.rs");
        let generated_producer = &generated_source
            [generated_source.find("type GeneratedRepositoryDefinitionResult").unwrap()..];
        let source = include_str!("canonical_repository_route.rs");
        let producer = &source[source.find("type CanonicalRepositoryRouteResult").unwrap()..];
        assert_eq!(producer.matches("HostCanonicalSelectedModuleDefinitionObservationKey::new").count(), 1);
        assert_eq!(producer.matches("HostGeneratedRepositoryDefinitionObservationKey::new").count(), 1);
        assert!(producer.find("HostCanonicalSelectedModuleDefinitionObservationKey::new").unwrap() < producer.find("HostGeneratedRepositoryDefinitionObservationKey::new").unwrap());
        assert!(producer.contains("CanonicalRepositoryRouteObservationError::Selected(error)"));
        assert!(producer.contains("CanonicalRepositoryRouteObservationError::Generated { selected_missing, error }"));
        assert!(producer.contains("CanonicalRepositoryRouteObservationError::Merge { selected_missing, error }"));
        assert!(producer.contains("Err(error) => return complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::SelectedCompute"));
        assert!(producer.contains("Err(error) => return complete_route_driver(Err(HostCanonicalRepositoryRouteError { canonical_repo: key.canonical_repo.clone(), kind: HostCanonicalRepositoryRouteErrorKind::GeneratedCompute { selected_missing"));
        assert!(producer.contains("return SourcePreparationOutcome::Complete(Err(CanonicalRepositoryRouteObservationError::Selected(error)))"));
        assert!(producer.contains("return SourcePreparationOutcome::Complete(Err(CanonicalRepositoryRouteObservationError::Generated { selected_missing, error }))"));
        assert_eq!(producer.matches("HostCanonicalRepositoryRouteObservationError(error)").count(), 1);
        assert!(!producer.contains("store_evaluation_data")); assert!(!producer.contains("union_")); assert!(!producer.contains("HostCanonicalRepositoryApparentMappingKey::new")); assert!(!producer.contains("HostSelectedRepositoryFileEffectKey"));
        let selected_source = include_str!("../../slug_bzlmod_v2/src/selected_repo_spec.rs");
        let selected_proof = &selected_source[selected_source.find("observed_canonical_selected_definition_identity_scan_and_terminal_algebra").unwrap()..selected_source.find("observed_canonical_selected_definition_real_order_events_and_parity").unwrap()];
        assert!(selected_proof.contains("PathObservationEpochError::OperationMismatch"));
        assert!(selected_proof.contains("RepoSpecChild::Outer(HostSelectedModuleRoutesObservationError::Graph"));
        assert!(selected_proof.contains("observed route outer must remain carrierless"));
        let pure_source = include_str!("module_extension.rs");
        let pure_proof = &pure_source[pure_source.find("observed_pure_identity_finisher_and_prefix_algebra").unwrap()..];
        assert!(pure_proof.contains("PathObservationEpochError::OperationMismatch"));
        assert!(pure_proof.contains("assert_observed_pure_outer_stages(&prepared, lower_error, merge_error)"));
        let instantiation_source = include_str!("module_extension_repository_instantiation.rs");
        let validation_source = include_str!("module_extension_repository_validation.rs");
        assert!(instantiation_source.contains("Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(InstantiatedModuleExtensionRepositoriesObservationError::Pure(error)))"));
        assert!(validation_source.contains("Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(ValidatedModuleExtensionRepositoriesObservationError::Instantiation(error)))"));
        assert!(generated_producer.contains("Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(HostGeneratedRepositoryDefinitionObservationError::Validation { demand, error }))"));
        assert!(generated_producer.contains("Err(error) => return SourcePreparationOutcome::Complete(Err(HostGeneratedRepositoryDefinitionObservationError::Merge { demand, error }))"));
        assert!(!generated_producer.contains("HostGeneratedRepositoryDefinitionObservationError::Validation(error)"));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_route_real_order_events_and_parity() {
        let extension = r#"print('load')
repo=repository_rule(implementation=lambda ctx: None)
def first_impl(ctx):
    print('invoke-first')
    repo(name='first')
def second_impl(ctx):
    print('invoke-second')
    repo(name='second')
first=module_extension(implementation=first_impl)
second=module_extension(implementation=second_impl)
"#;
        let module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\n";
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, module, extension, true, None).await;
        let generated = names(&validated(&mut prep_tx).await).remove(0);
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let selected_failure = "module(name='root')\nbazel_dep(name='missing', version='1')\n";
        let empty: &[&str] = &[];
        let generated_prints = ["load", "invoke-first"];
        for (case, case_module, case_extension, present, requested, generated_stage, expected_prints) in [
            ("selected-success", MODULE, EXTENSION_A, true, CanonicalRepoName::root(), false, empty),
            ("selected-failure", selected_failure, EXTENSION_A, true, CanonicalRepoName::root(), false, empty),
            ("generated-success", module, extension, true, generated.clone(), true, generated_prints.as_slice()),
            ("generated-failure", module, extension, false, generated.clone(), true, empty),
            ("generated-missing", module, extension, true, CanonicalRepoName::new("missing").unwrap(), true, empty),
        ] {
            let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let tracker = Arc::new(LookupTracker::default());
            let mut tx = transaction(&dice, case_module, case_extension, present, Some(tracker.clone())).await;
            let key = HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), requested.clone());
            let observed = tx.compute(&key).await.unwrap();
            let carrier = observed_canonical_carrier(&observed);
            match case {
                "selected-success" => assert!(matches!(carrier.result().as_ref(), Ok(route) if route.view().kind() == HostCanonicalRepositoryRouteKind::Root)),
                "selected-failure" => assert!(matches!(carrier.result().as_ref(), Err(HostCanonicalRepositoryRouteError { kind: HostCanonicalRepositoryRouteErrorKind::Selected(_), .. }))),
                "generated-success" => assert!(matches!(carrier.result().as_ref(), Ok(route) if route.view().kind() == HostCanonicalRepositoryRouteKind::Generated)),
                "generated-failure" => assert!(matches!(carrier.result().as_ref(), Err(HostCanonicalRepositoryRouteError { kind: HostCanonicalRepositoryRouteErrorKind::Generated { .. }, .. }))),
                "generated-missing" => assert!(matches!(carrier.result().as_ref(), Err(HostCanonicalRepositoryRouteError { kind: HostCanonicalRepositoryRouteErrorKind::Missing { selected_missing, generated_missing }, .. }) if selected_missing.disposition() == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing && matches!(generated_missing.kind, HostGeneratedRepositoryDefinitionErrorKind::Missing { .. }))),
                _ => unreachable!(),
            }
            let mut expected_children = vec![HostCanonicalSelectedModuleDefinitionObservationKey::new(workspace.clone(), requested.clone()).to_string()];
            if generated_stage { expected_children.push(HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), requested.clone()).to_string()); }
            assert_eq!(tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &key.to_string()).unwrap().1, expected_children);
            let activations = tracker.activations.lock().unwrap();
            let parent = activations.iter().find(|(name, _, _)| name == &key.to_string()).unwrap();
            assert_eq!(parent.1, ActivationKind::Evaluated); assert!(parent.2.is_none());
            for child in &expected_children { assert!(activations.iter().any(|(name, _, batch)| name == child && batch.is_none()), "{case}: {child}"); }
            let prints = activations.iter().filter_map(|(_, _, batch)| batch.as_ref()).flat_map(EventBatch::events).filter_map(|event| match event { EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()), _ => None }).collect::<Vec<_>>();
            assert_eq!(prints, expected_prints, "{case}: {activations:#?}");
            drop(activations);
            let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let mut legacy_tx = transaction(&legacy_dice, case_module, case_extension, present, None).await;
            let SourcePreparationOutcome::Complete(legacy) = legacy_tx.compute(&HostCanonicalRepositoryRouteKey::new(workspace.clone(), requested)).await.unwrap() else { panic!("{case}: legacy must complete") };
            assert_eq!(legacy.as_ref(), carrier.result().as_ref(), "{case}");
            tracker.activations.lock().unwrap().clear();
            let warm = tx.compute(&key).await.unwrap();
            assert!(HostCanonicalRepositoryRouteObservationKey::equality(&observed, &warm));
            assert!(Arc::ptr_eq(carrier.result(), observed_canonical_carrier(&warm).result()));
            let warm_activations = tracker.activations.lock().unwrap();
            assert!(warm_activations.iter().any(|(name, kind, batch)| name == &key.to_string() && *kind == ActivationKind::Reused && batch.is_none()));
            assert!(warm_activations.iter().all(|(_, _, batch)| batch.is_none()));
            drop(warm_activations);
            for forbidden in ["host-canonical-selected-module-definition:", "host-generated-repository-definition:", "host-canonical-repository-route:", "host-canonical-repository-apparent-mapping:", "host-root-repository-mapping:", "host-root-apparent-repository-definition:", "root-repository-route:", "repository-package-source:", "repository-source-file:", "host-repository-source-file:", "build-command-root:"] {
                assert!(tracker.activations.lock().unwrap().iter().all(|(name, _, _)| !name.starts_with(forbidden)), "{case}: {forbidden}");
                assert!(tracker.dependencies.lock().unwrap().iter().all(|(name, children)| !name.starts_with(forbidden) && children.iter().all(|child| !child.starts_with(forbidden))), "{case}: {forbidden}");
            }
        }
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_route_lifecycle_cancellation_and_nonactivation() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, MODULE, EXTENSION_A, true, None).await;
        let generated = names(&validated(&mut prep_tx).await).remove(0);
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let selected_tracker = Arc::new(LookupTracker::default());
        let selected_parent_key = HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), CanonicalRepoName::root());
        let selected_child_key = HostCanonicalSelectedModuleDefinitionObservationKey::new(workspace.clone(), CanonicalRepoName::root());
        let selected_b = MODULE.replacen("bazel_tools", "root", 1);
        let mut selected_held = Vec::new();
        for module in [MODULE, selected_b.as_str(), MODULE] {
            let mut tx = transaction(&dice, module, EXTENSION_A, true, Some(selected_tracker.clone())).await;
            let global = tx.compute(&PathObservationEpochKey).await.unwrap();
            let parent = observed_canonical_carrier(&tx.compute(&selected_parent_key).await.unwrap()).clone();
            let child_value = tx.compute(&selected_child_key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child_value else { panic!("selected child must complete") };
            assert_eq!(parent.observations(), child.observations());
            for carrier_epoch in [parent.observations(), child.observations()] {
                for (demand, result) in carrier_epoch.observations() { assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref()); }
            }
            selected_held.push((parent, child));
        }
        assert_ne!(selected_held[0].0.result(), selected_held[1].0.result());
        assert_eq!(selected_held[0].0.result(), selected_held[2].0.result());
        assert_ne!(selected_held[0].1.result(), selected_held[1].1.result());
        assert_eq!(selected_held[0].1.result(), selected_held[2].1.result());

        let generated_tracker = Arc::new(LookupTracker::default());
        let generated_parent_key = HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), generated.clone());
        let generated_child_key = HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), generated.clone());
        let same_semantic = format!("{MODULE}\n");
        let mut generated_held = Vec::new();
        for (module, extension) in [(MODULE, EXTENSION_A), (MODULE, EXTENSION_B), (MODULE, EXTENSION_A), (same_semantic.as_str(), EXTENSION_A)] {
            let mut tx = transaction(&dice, module, extension, true, Some(generated_tracker.clone())).await;
            let global = tx.compute(&PathObservationEpochKey).await.unwrap();
            let parent = observed_canonical_carrier(&tx.compute(&generated_parent_key).await.unwrap()).clone();
            let child_value = tx.compute(&generated_child_key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child_value else { panic!("generated child must complete") };
            for carrier_epoch in [parent.observations(), child.observations()] {
                for (demand, result) in carrier_epoch.observations() { assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref()); }
            }
            generated_held.push((parent, child));
        }
        assert_ne!(generated_held[0].0.result(), generated_held[1].0.result());
        assert_eq!(generated_held[0].0.result(), generated_held[2].0.result());
        assert_ne!(generated_held[0].1.result(), generated_held[1].1.result());
        assert_eq!(generated_held[0].1.result(), generated_held[2].1.result());
        assert_eq!(generated_held[0].0.result(), generated_held[3].0.result());
        assert_ne!(generated_held[0].0.observations(), generated_held[3].0.observations());
        assert_eq!(generated_held[0].1.result(), generated_held[3].1.result());
        assert_ne!(generated_held[0].1.observations(), generated_held[3].1.observations());

        let selected_failure = "module(name='root')\nbazel_dep(name='missing', version='1')\n";
        let mut trackers = vec![selected_tracker, generated_tracker];
        for (case, module, requested, generated_stage) in [
            ("selected-terminal", selected_failure, CanonicalRepoName::root(), false),
            ("missing-fallback", MODULE, CanonicalRepoName::new("missing").unwrap(), true),
        ] {
            let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let tracker = Arc::new(LookupTracker::default());
            let mut cancelled = transaction(&cancel_dice, module, EXTENSION_A, true, Some(tracker.clone())).await;
            let key = HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), requested.clone());
            let mut future = Box::pin(cancelled.compute(&key));
            std::future::poll_fn(|context| { assert!(std::future::Future::poll(future.as_mut(), context).is_pending()); std::task::Poll::Ready(()) }).await;
            drop(future); drop(cancelled);
            assert!(tracker.activations.lock().unwrap().iter().all(|(name, _, _)| name != &key.to_string()));
            assert!(tracker.dependencies.lock().unwrap().iter().all(|(name, _)| name != &key.to_string()));

            let mut recovery = transaction(&cancel_dice, module, EXTENSION_A, true, Some(tracker.clone())).await;
            let global = recovery.compute(&PathObservationEpochKey).await.unwrap();
            let recovered_value = recovery.compute(&key).await.unwrap();
            let recovered = observed_canonical_carrier(&recovered_value);
            match case {
                "selected-terminal" => assert!(matches!(recovered.result().as_ref(), Err(HostCanonicalRepositoryRouteError { kind: HostCanonicalRepositoryRouteErrorKind::Selected(_), .. }))),
                "missing-fallback" => assert!(matches!(recovered.result().as_ref(), Err(HostCanonicalRepositoryRouteError { kind: HostCanonicalRepositoryRouteErrorKind::Missing { .. }, .. }))),
                _ => unreachable!(),
            }
            for (demand, result) in recovered.observations().observations() { assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref()); }
            let clean_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let clean = observed_canonical_lookup(&clean_dice, module, EXTENSION_A, true, requested.clone(), None).await;
            assert_eq!(recovered.result(), observed_canonical_carrier(&clean).result(), "{case}");
            let mut expected_children = vec![HostCanonicalSelectedModuleDefinitionObservationKey::new(workspace.clone(), requested.clone()).to_string()];
            if generated_stage { expected_children.push(HostGeneratedRepositoryDefinitionObservationKey::new(workspace.clone(), requested).to_string()); }
            assert_eq!(tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &key.to_string()).unwrap().1, expected_children);
            let activations = tracker.activations.lock().unwrap();
            assert!(activations.iter().any(|(name, kind, batch)| name == &key.to_string() && *kind == ActivationKind::Evaluated && batch.is_none()));
            for child in &expected_children { assert!(activations.iter().any(|(name, _, batch)| name == child && batch.is_none()), "{case}: {child}"); }
            drop(activations);
            trackers.push(tracker);
        }

        for tracker in trackers {
            let activations = tracker.activations.lock().unwrap();
            let dependencies = tracker.dependencies.lock().unwrap();
            for family in ["host-canonical-selected-module-definition:", "host-generated-repository-definition:", "host-canonical-repository-route:"] {
                assert!(activations.iter().all(|(name, _, _)| !name.starts_with(family)));
                assert!(dependencies.iter().all(|(name, children)| !name.starts_with(family) && children.iter().all(|child| !child.starts_with(family))));
            }
            for upper in ["host-canonical-repository-apparent-mapping:", "host-root-repository-mapping:", "host-root-apparent-repository-definition:", "HostRootApparentRepositoryRouteKey", "HostRootApparentRepositorySourceInputKey", "HostRootApparentRepositorySourceObservationKey", "HostRootApparentRepositorySourcePathInputKey", "root-repository-route:", "repository-package-source:", "repository-source-file:", "host-repository-source-file:", "build-command-root:"] {
                assert!(activations.iter().all(|(name, _, _)| !name.contains(upper)));
                assert!(dependencies.iter().all(|(name, children)| !name.contains(upper) && children.iter().all(|child| !child.contains(upper))));
            }
        }
        let source = include_str!("canonical_repository_route.rs");
        let producer = &source[source.find("type CanonicalRepositoryRouteResult").unwrap()..];
        assert!(!producer.contains("HostCanonicalRepositoryRouteKey::new"));
        assert!(!producer.contains("HostCanonicalRepositoryApparentMappingKey::new"));
        assert!(!producer.contains("HostRootRepositoryMappingKey"));
        assert!(!producer.contains("RootModuleBootstrap"));
        assert!(!producer.contains("CaptureEvaluationEvents"));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_apparent_mapping_identity_branch_and_terminal_algebra() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let root = CanonicalRepoName::root();
        let first = ApparentRepoName::new("first").unwrap();
        let key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), root.clone(), first.clone());
        let same = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), root.clone(), first.clone());
        let other = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), root.clone(), ApparentRepoName::new("second").unwrap());
        let hash = |value: &HostCanonicalRepositoryApparentMappingObservationKey| { let mut state = DefaultHasher::new(); value.hash(&mut state); state.finish() };
        assert_eq!(key, same); assert_ne!(key, other); assert_eq!(hash(&key), hash(&same)); assert_ne!(hash(&key), hash(&other));
        assert_eq!(key.to_string(), "observed-host-canonical-repository-apparent-mapping:\"/generated-repository-definition\":@@:@first");

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let generated = names(&validated(&mut tx).await).remove(0);
        tracker.dependencies.lock().unwrap().clear();
        let root_value = tx.compute(&key).await.unwrap();
        let root_carrier = observed_apparent_mapping_carrier(&root_value);
        let root_child_key = HostRootRepositoryMappingObservationKey::new(workspace.clone());
        let root_child_value = tx.compute(&root_child_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(root_child)) = root_child_value else { panic!("root child must complete") };
        assert_eq!(root_carrier.observations(), root_child.observations());
        assert_eq!(activation_dependencies(&tracker, &key.to_string()), [root_child_key.to_string()]);
        assert!(matches!(root_carrier.result().as_ref(), Ok(HostCanonicalRepositoryApparentMapping { predecessor: ApparentMappingPredecessor::Root(_), .. })));

        let nonroot_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), generated.clone(), first.clone());
        let nonroot_value = tx.compute(&nonroot_key).await.unwrap();
        let nonroot_carrier = observed_apparent_mapping_carrier(&nonroot_value);
        let definition_child_key = HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), generated.clone());
        let definition_child_value = tx.compute(&definition_child_key).await.unwrap();
        let definition_child = observed_canonical_carrier(&definition_child_value);
        assert_eq!(nonroot_carrier.observations(), definition_child.observations());
        assert_eq!(activation_dependencies(&tracker, &nonroot_key.to_string()), [definition_child_key.to_string()]);
        assert!(matches!(nonroot_carrier.result().as_ref(), Ok(HostCanonicalRepositoryApparentMapping { predecessor: ApparentMappingPredecessor::Canonical(_), .. })));
        assert!(HostCanonicalRepositoryApparentMappingObservationKey::validity(&nonroot_value));
        assert!(HostCanonicalRepositoryApparentMappingObservationKey::equality(&nonroot_value, &nonroot_value));

        let epoch_only = SourcePreparationOutcome::Complete(Ok(ObservedHostCanonicalRepositoryApparentMapping { result: nonroot_carrier.result().clone(), observations: PathObservationEpoch::empty() }));
        assert!(!HostCanonicalRepositoryApparentMappingObservationKey::equality(&nonroot_value, &epoch_only));
        let root_apparent_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), generated.clone(), ApparentRepoName::root());
        let root_apparent = tx.compute(&root_apparent_key).await.unwrap();
        assert!(matches!(observed_apparent_mapping_carrier(&root_apparent).result().as_ref(), Err(HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::RootApparent, .. })));
        assert!(observed_apparent_mapping_carrier(&root_apparent).observations().observations().is_empty());
        assert!(activation_dependencies(&tracker, &root_apparent_key.to_string()).is_empty());

        let need_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let _ = transaction(&need_dice, MODULE, EXTENSION_A, true, None).await;
        let mut updater = need_dice.updater();
        updater.changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())]).unwrap();
        let need = updater.commit().await.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Need(needs) = &need else { panic!("observed parent must need") };
        assert!(!HostCanonicalRepositoryApparentMappingObservationKey::validity(&need));
        assert!(!HostCanonicalRepositoryApparentMappingObservationKey::equality(&need, &need));
        assert!(matches!(finish_mapping(&key.0, CanonicalRepositoryApparentMappingChildOutcome::Need(needs.clone())), SourcePreparationOutcome::Need(_)));

        for kind in [HostCanonicalRepositoryApparentMappingErrorKind::RootMappingCompute("root-dice".into()), HostCanonicalRepositoryApparentMappingErrorKind::RouteCompute("definition-dice".into())] {
            let complete = finish_mapping(&key.0, CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Err(kind), observations: PathObservationEpoch::empty() });
            assert!(matches!(complete, SourcePreparationOutcome::Complete(Ok((result, observations))) if result.is_err() && observations.observations().is_empty()));
        }
        let mismatch_key = HostCanonicalRepositoryApparentMappingKey::new(workspace.clone(), root.clone(), first.clone());
        let predecessor = definition_child.result().as_ref().as_ref().unwrap().clone();
        let mismatch = finish_mapping(&mismatch_key, CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Ok(ApparentMappingPredecessor::Canonical(predecessor.clone())), observations: definition_child.observations().clone() });
        assert!(matches!(mismatch, SourcePreparationOutcome::Complete(Ok((result, observations))) if matches!(result.as_ref(), Err(error @ HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::ContextMismatch { predecessor: ApparentMappingPredecessor::Canonical(_) }, .. }) if error.disposition() == HostCanonicalRepositoryApparentMappingErrorDisposition::ContextMismatch) && observations == *definition_child.observations()));
        let missing_key = HostCanonicalRepositoryApparentMappingKey::new(workspace.clone(), generated.clone(), ApparentRepoName::new("missing").unwrap());
        let missing = finish_mapping(&missing_key, CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Ok(ApparentMappingPredecessor::Canonical(predecessor.clone())), observations: definition_child.observations().clone() });
        assert!(matches!(missing, SourcePreparationOutcome::Complete(Ok((result, observations))) if matches!(result.as_ref(), Err(error @ HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::Missing { predecessor: ApparentMappingPredecessor::Canonical(_) }, .. }) if error.disposition() == HostCanonicalRepositoryApparentMappingErrorDisposition::Missing) && observations == *definition_child.observations()));
        let success = finish_mapping(&nonroot_key.0, CanonicalRepositoryApparentMappingChildOutcome::Complete { result: Ok(ApparentMappingPredecessor::Canonical(predecessor)), observations: definition_child.observations().clone() });
        assert!(matches!(success, SourcePreparationOutcome::Complete(Ok((result, observations))) if result.as_ref().as_ref().unwrap().resolved_target() == nonroot_carrier.result().as_ref().as_ref().unwrap().resolved_target() && observations == *definition_child.observations()));

        let bad_root_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut bad_root_tx = transaction(&bad_root_dice, "this is not valid Starlark\n", EXTENSION_A, true, None).await;
        let bad_root = bad_root_tx.compute(&key).await.unwrap();
        assert!(matches!(observed_apparent_mapping_carrier(&bad_root).result().as_ref(), Err(error @ HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::RootMapping(_), .. }) if error.disposition() == HostCanonicalRepositoryApparentMappingErrorDisposition::Other));
        let bad_definition_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut bad_definition_tx = transaction(&bad_definition_dice, MODULE, EXTENSION_A, false, None).await;
        let bad_definition = bad_definition_tx.compute(&nonroot_key).await.unwrap();
        assert!(matches!(observed_apparent_mapping_carrier(&bad_definition).result().as_ref(), Err(HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::Route(_), .. })));

        let missing_definition = tx.compute(&HostCanonicalRepositoryRouteKey::new(workspace.clone(), CanonicalRepoName::new("missing").unwrap())).await.unwrap();
        let SourcePreparationOutcome::Complete(missing_definition) = missing_definition else { panic!("missing definition must complete") };
        let HostCanonicalRepositoryRouteErrorKind::Missing { selected_missing, .. } = &missing_definition.as_ref().as_ref().unwrap_err().kind else { panic!("missing definition terminal expected") };
        let conflict = merge_route_observations(&generated_definition_observation_epoch(MODULE, EXTENSION_A, true), &generated_definition_observation_epoch(&format!("{MODULE}\n"), EXTENSION_A, true)).unwrap_err();
        let outer = finish_mapping(&nonroot_key.0, CanonicalRepositoryApparentMappingChildOutcome::Outer(CanonicalRepositoryApparentMappingObservationError::Route(HostCanonicalRepositoryRouteObservationError(CanonicalRepositoryRouteObservationError::Merge { selected_missing: selected_missing.clone(), error: conflict }))));
        let outer_value: <HostCanonicalRepositoryApparentMappingObservationKey as Key>::Value = match outer { SourcePreparationOutcome::Complete(Err(error @ CanonicalRepositoryApparentMappingObservationError::Route(_))) => SourcePreparationOutcome::Complete(Err(HostCanonicalRepositoryApparentMappingObservationError(error))), _ => panic!("definition outer expected") };
        assert!(HostCanonicalRepositoryApparentMappingObservationKey::validity(&outer_value));
        assert!(HostCanonicalRepositoryApparentMappingObservationKey::equality(&outer_value, &outer_value));

        let source = include_str!("canonical_repository_mapping.rs");
        let producer = &source[source.find("type CanonicalRepositoryApparentMappingResult").unwrap()..];
        assert_eq!(producer.matches("HostRootRepositoryMappingObservationKey::new").count(), 1);
        assert_eq!(producer.matches("HostCanonicalRepositoryRouteObservationKey::new").count(), 1);
        assert_eq!(producer.matches("CanonicalRepositoryApparentMappingObservationError::RootMapping(error)").count(), 2);
        assert_eq!(producer.matches("CanonicalRepositoryApparentMappingObservationError::Route(error)").count(), 2);
        assert_eq!(producer.matches("HostCanonicalRepositoryApparentMappingObservationError(error)").count(), 1);
        assert_eq!(producer.matches("HostCanonicalRepositoryRouteObservationError(error)").count(), 0);
        for forbidden in ["merge_route_observations", "union_", "store_evaluation_data", "HostRootApparentRepositoryDefinitionKey", "HostSelectedRepositoryFileEffectKey"] { assert!(!producer.contains(forbidden)); }
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_apparent_mapping_real_branches_events_and_parity() {
        let extension = r#"print('load')
repo=repository_rule(implementation=lambda ctx: None)
def first_impl(ctx):
    print('invoke-first')
    repo(name='first')
def second_impl(ctx):
    print('invoke-second')
    repo(name='second')
first=module_extension(implementation=first_impl)
second=module_extension(implementation=second_impl)
"#;
        let module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\n";
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, module, extension, true, None).await;
        let generated = names(&validated(&mut prep_tx).await);
        for (case, case_module, present, context, expected) in [
            ("root-success", module, true, CanonicalRepoName::root(), "success"),
            ("root-error", "this is not valid Starlark\n", true, CanonicalRepoName::root(), "root-error"),
            ("nonroot-success", module, true, generated[0].clone(), "success"),
            ("nonroot-error", module, false, generated[0].clone(), "definition-error"),
        ] {
            let observed_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let observed_tracker = Arc::new(LookupTracker::default());
            let mut observed_tx = transaction(&observed_dice, case_module, extension, present, Some(observed_tracker.clone())).await;
            let observed_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), context.clone(), ApparentRepoName::new("first").unwrap());
            let observed_value = observed_tx.compute(&observed_key).await.unwrap();
            let carrier = observed_apparent_mapping_carrier(&observed_value);
            let (observed_child, legacy_child) = if context.is_root() {
                (HostRootRepositoryMappingObservationKey::new(workspace.clone()).to_string(), HostRootRepositoryMappingKey::new(workspace.clone()).to_string())
            } else {
                (HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), context.clone()).to_string(), HostCanonicalRepositoryRouteKey::new(workspace.clone(), context.clone()).to_string())
            };
            assert_eq!(activation_dependencies(&observed_tracker, &observed_key.to_string()), [observed_child.clone()], "{case}");
            assert!(observed_tracker.dependencies.lock().unwrap().iter().find(|(name, _)| name == &observed_key.to_string()).unwrap().1.iter().all(|child| child == &observed_child));
            let observed_parent = observed_tracker.activations.lock().unwrap().iter().find(|(name, _, _)| name == &observed_key.to_string()).cloned().unwrap();
            assert_eq!(observed_parent.1, ActivationKind::Evaluated); assert!(observed_parent.2.is_none());
            let observed_prints = starlark_print_owners(&observed_tracker);

            let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let legacy_tracker = Arc::new(LookupTracker::default());
            let mut legacy_tx = transaction(&legacy_dice, case_module, extension, present, Some(legacy_tracker.clone())).await;
            let legacy_key = HostCanonicalRepositoryApparentMappingKey::new(workspace.clone(), context.clone(), ApparentRepoName::new("first").unwrap());
            let legacy_value = legacy_tx.compute(&legacy_key).await.unwrap();
            assert_eq!(activation_dependencies(&legacy_tracker, &legacy_key.to_string()), [legacy_child], "{case}");
            let SourcePreparationOutcome::Complete(legacy_result) = &legacy_value else { panic!("{case}: legacy must complete") };
            assert_eq!(legacy_result.as_ref(), carrier.result().as_ref(), "{case}");
            let child_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let child_tracker = Arc::new(LookupTracker::default());
            let mut child_tx = transaction(&child_dice, case_module, extension, present, Some(child_tracker.clone())).await;
            if context.is_root() {
                let _ = child_tx.compute(&HostRootRepositoryMappingObservationKey::new(workspace.clone())).await.unwrap();
            } else {
                let _ = child_tx.compute(&HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), context.clone())).await.unwrap();
            }
            assert_eq!(observed_prints, starlark_print_owners(&child_tracker), "{case}: lower event owner/payload");
            match expected {
                "success" => {
                    let value = carrier.result().as_ref().as_ref().unwrap();
                    assert_eq!(value.resolved_target(), Some(&generated[0]), "{case}");
                    let borrowed = match &value.predecessor {
                        ApparentMappingPredecessor::Root(predecessor) => predecessor.view().unwrap().mapping().find_map(|(name, target)| (name.as_str() == "first").then_some(target)).unwrap(),
                        ApparentMappingPredecessor::Canonical(predecessor) => predecessor.mapping_target(&ApparentRepoName::new("first").unwrap()).unwrap(),
                    };
                    assert!(std::ptr::eq(borrowed, value.resolved_target().unwrap()), "{case}");
                }
                "root-error" => assert!(matches!(carrier.result().as_ref(), Err(HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::RootMapping(_), .. }))),
                "definition-error" => assert!(matches!(carrier.result().as_ref(), Err(HostCanonicalRepositoryApparentMappingError { kind: HostCanonicalRepositoryApparentMappingErrorKind::Route(_), .. }))),
                _ => unreachable!(),
            }
            observed_tracker.activations.lock().unwrap().clear();
            let warm = observed_tx.compute(&observed_key).await.unwrap();
            assert!(HostCanonicalRepositoryApparentMappingObservationKey::equality(&observed_value, &warm));
            assert!(Arc::ptr_eq(carrier.result(), observed_apparent_mapping_carrier(&warm).result()));
            let warm_rows = observed_tracker.activations.lock().unwrap();
            assert!(warm_rows.iter().any(|(name, kind, batch)| name == &observed_key.to_string() && *kind == ActivationKind::Reused && batch.is_none()), "{case}");
            assert!(warm_rows.iter().all(|(_, _, batch)| batch.is_none()), "{case}");
            drop(warm_rows);
            assert!(starlark_print_owners(&observed_tracker).is_empty(), "{case}: warm replay");
            let unchosen = if context.is_root() { "observed-host-canonical-repository-route:" } else { "observed-host-root-repository-mapping:" };
            assert_activation_families_absent(&observed_tracker, &[unchosen, "host-root-apparent-repository-definition:", "build-command-root:"]);
        }
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_canonical_repository_apparent_mapping_lifecycle_cancellation_and_nonactivation() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let prep = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prep_tx = transaction(&prep, MODULE, EXTENSION_A, true, None).await;
        let generated = names(&validated(&mut prep_tx).await).remove(0);
        let root_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), CanonicalRepoName::root(), ApparentRepoName::new("first").unwrap());
        let nonroot_key = HostCanonicalRepositoryApparentMappingObservationKey::new(workspace.clone(), generated.clone(), ApparentRepoName::new("first").unwrap());
        let root_b = format!("{MODULE}override_repo(e, first='bazel_tools')\n");
        let same_semantic = format!("{MODULE}\n");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));

        let mut root_held = Vec::new();
        for module in [MODULE, root_b.as_str(), MODULE, same_semantic.as_str()] {
            let mut tx = transaction(&dice, module, EXTENSION_A, true, None).await;
            let global = tx.compute(&PathObservationEpochKey).await.unwrap();
            let parent = observed_apparent_mapping_carrier(&tx.compute(&root_key).await.unwrap()).clone();
            let child_value = tx.compute(&HostRootRepositoryMappingObservationKey::new(workspace.clone())).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child_value else { panic!("root child must complete") };
            assert_eq!(parent.observations(), child.observations());
            assert_apparent_epoch_current(parent.observations(), &global);
            assert_apparent_epoch_current(child.observations(), &global);
            root_held.push((parent, child));
        }
        assert_ne!(root_held[0].0.result(), root_held[1].0.result()); assert_eq!(root_held[0].0.result(), root_held[2].0.result());
        assert_ne!(root_held[0].1.result(), root_held[1].1.result()); assert_eq!(root_held[0].1.result(), root_held[2].1.result());
        assert_eq!(root_held[0].0.result(), root_held[3].0.result()); assert_ne!(root_held[0].0.observations(), root_held[3].0.observations());
        assert_eq!(root_held[0].1.result(), root_held[3].1.result()); assert_ne!(root_held[0].1.observations(), root_held[3].1.observations());

        let mut nonroot_held = Vec::new();
        for (module, extension) in [(MODULE, EXTENSION_A), (MODULE, EXTENSION_B), (MODULE, EXTENSION_A), (same_semantic.as_str(), EXTENSION_A)] {
            let mut tx = transaction(&dice, module, extension, true, None).await;
            let global = tx.compute(&PathObservationEpochKey).await.unwrap();
            let parent = observed_apparent_mapping_carrier(&tx.compute(&nonroot_key).await.unwrap()).clone();
            let child_value = tx.compute(&HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), generated.clone())).await.unwrap();
            let child = observed_canonical_carrier(&child_value).clone();
            assert_eq!(parent.observations(), child.observations());
            assert_apparent_epoch_current(parent.observations(), &global);
            assert_apparent_epoch_current(child.observations(), &global);
            nonroot_held.push((parent, child));
        }
        assert_ne!(nonroot_held[0].0.result(), nonroot_held[1].0.result()); assert_eq!(nonroot_held[0].0.result(), nonroot_held[2].0.result());
        assert_ne!(nonroot_held[0].1.result(), nonroot_held[1].1.result()); assert_eq!(nonroot_held[0].1.result(), nonroot_held[2].1.result());
        assert_eq!(nonroot_held[0].0.result(), nonroot_held[3].0.result()); assert_ne!(nonroot_held[0].0.observations(), nonroot_held[3].0.observations());
        assert_eq!(nonroot_held[0].1.result(), nonroot_held[3].1.result()); assert_ne!(nonroot_held[0].1.observations(), nonroot_held[3].1.observations());

        for key in [&root_key, &nonroot_key] {
            let tracker = Arc::new(LookupTracker::default());
            let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
            let first = observed_apparent_mapping_carrier(&tx.compute(key).await.unwrap()).clone();
            tracker.activations.lock().unwrap().clear();
            let reused = observed_apparent_mapping_carrier(&tx.compute(key).await.unwrap()).clone();
            assert!(Arc::ptr_eq(first.result(), reused.result()));
            assert!(tracker.activations.lock().unwrap().iter().any(|(name, kind, batch)| name == &key.to_string() && *kind == ActivationKind::Reused && batch.is_none()));
        }

        let mut lifecycle_trackers = Vec::new();
        for (case, key, child) in [
            ("root", root_key.clone(), HostRootRepositoryMappingObservationKey::new(workspace.clone()).to_string()),
            ("nonroot", nonroot_key.clone(), HostCanonicalRepositoryRouteObservationKey::new(workspace.clone(), generated.clone()).to_string()),
        ] {
            let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let tracker = Arc::new(LookupTracker::default());
            let mut cancelled = transaction(&cancel_dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
            let mut future = Box::pin(cancelled.compute(&key));
            std::future::poll_fn(|context| { assert!(std::future::Future::poll(future.as_mut(), context).is_pending()); std::task::Poll::Ready(()) }).await;
            drop(future); drop(cancelled);
            assert!(tracker.activations.lock().unwrap().iter().all(|(name, _, _)| name != &key.to_string()), "{case}");
            assert!(tracker.dependencies.lock().unwrap().iter().all(|(name, _)| name != &key.to_string()), "{case}");

            let mut recovery = transaction(&cancel_dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
            let global = recovery.compute(&PathObservationEpochKey).await.unwrap();
            let recovered = observed_apparent_mapping_carrier(&recovery.compute(&key).await.unwrap()).clone();
            let expected = if case == "root" { root_held[0].0.result() } else { nonroot_held[0].0.result() };
            assert_eq!(recovered.result(), expected, "{case}");
            assert_apparent_epoch_current(recovered.observations(), &global);
            assert_eq!(activation_dependencies(&tracker, &key.to_string()), [child]);
            assert!(tracker.activations.lock().unwrap().iter().any(|(name, kind, batch)| name == &key.to_string() && *kind == ActivationKind::Evaluated && batch.is_none()));
            lifecycle_trackers.push((case, tracker));
        }

        let inactive = ["host-canonical-repository-apparent-mapping:", "host-root-apparent-repository-definition:", "HostRootApparentRepositoryRouteKey", "HostRootApparentRepositorySourceInputKey", "HostRootApparentRepositorySourceObservationKey", "HostRootApparentRepositorySourcePathInputKey", "root-repository-route:", "repository-package-source:", "repository-source-file:", "host-repository-source-file:", "repository-materialization:", "build-command-root:"];
        for (case, tracker) in lifecycle_trackers {
            assert_activation_families_absent(&tracker, &inactive);
            let unchosen = if case == "root" { "observed-host-canonical-repository-route:" } else { "observed-host-root-repository-mapping:" };
            assert_activation_families_absent(&tracker, &[unchosen]);
        }
        let source = include_str!("canonical_repository_mapping.rs");
        let producer = &source[source.find("type CanonicalRepositoryApparentMappingResult").unwrap()..];
        for forbidden in ["HostRootApparentRepositoryDefinitionKey", "HostRootApparentRepositoryRouteKey", "HostRootApparentRepositorySourceInputKey", "HostRootApparentRepositorySourceObservationKey", "HostRootApparentRepositorySourcePathInputKey", "RepositoryMaterializationKey", "BuildCommandRootKey", "RootModuleBootstrap", "store_evaluation_data", "Mutex", "spawn"] { assert!(!producer.contains(forbidden)); }
    }

    #[tokio::test]
    async fn predecessor_need_and_error_precede_lookup() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut initial = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let generated = names(&validated(&mut initial).await);
        let key = HostGeneratedRepositoryDefinitionKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            generated[0].clone(),
        );
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(crate::bzl_load_cycle_detector()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let mut need_tx = updater.commit().await;
        let need = need_tx.compute(&key).await.unwrap();
        assert!(!HostGeneratedRepositoryDefinitionKey::validity(&need));
        assert!(!HostGeneratedRepositoryDefinitionKey::equality(
            &need, &need
        ));
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        let mapping_key = HostCanonicalRepositoryApparentMappingKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            generated[0].clone(),
            ApparentRepoName::new("first").unwrap(),
        );
        let mapping_need = need_tx.compute(&mapping_key).await.unwrap();
        assert!(!HostCanonicalRepositoryApparentMappingKey::validity(
            &mapping_need
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &mapping_need,
            &mapping_need,
        ));
        let root_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let _root_tx = transaction(
            &root_dice,
            "module(name='bazel_tools')\n\
             local_path_override(module_name='local', path='local')\n\
             bazel_dep(name='local', version='1', repo_name='local_alias')\n",
            EXTENSION_A,
            true,
            None,
        )
        .await;
        let mut root_updater = root_dice.updater();
        root_updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let root_mapping_need = root_updater
            .commit()
            .await
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::root(),
            ))
            .await
            .unwrap();
        assert!(matches!(
            root_mapping_need,
            SourcePreparationOutcome::Need(_)
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::validity(
            &root_mapping_need,
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_mapping_need,
            &root_mapping_need,
        ));

        let mut missing_source = transaction(&dice, MODULE, EXTENSION_A, false, None).await;
        let terminal = missing_source.compute(&key).await.unwrap();
        assert!(HostGeneratedRepositoryDefinitionKey::validity(&terminal));
        assert!(matches!(
            terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostGeneratedRepositoryDefinitionError {
                        kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(_),
                        ..
                    })
                )
        ));
        let mapping_terminal = missing_source.compute(&mapping_key).await.unwrap();
        assert!(matches!(
            mapping_terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Route(_),
                        ..
                    })
                )
        ));
    }

    /// Test-only sibling proof: core can construct a public
    /// `RootRepositoryRoute` from the accepted Generated route-view shape and
    /// drive the existing routed package owners without any production caller
    /// or compute edge.
    #[test]
    fn generated_route_capability_is_sibling_usable() {
        use compact_str::CompactString;
        use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
        use slug_bzlmod_v2::HostRepositoryLocalPathPolicy;
        use slug_bzlmod_v2::OverrideAttributeValue;
        use slug_bzlmod_v2::RepoRuleId;
        use slug_bzlmod_v2::RepoSpec;
        use slug_bzlmod_v2::RootRepositoryRoute;
        use slug_identity_v2::CanonicalLabel;
        use slug_identity_v2::PackageIdentifier;
        use slug_identity_v2::PackagePath;
        use starlark_map::small_map::SmallMap;

        fn local_repository_spec(path: &str) -> RepoSpec {
            RepoSpec {
                rule_id: RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                    )
                    .unwrap(),
                    rule_name: CompactString::new("local_repository"),
                },
                attributes: Arc::new(SmallMap::from_iter([(
                    CompactString::new("path"),
                    OverrideAttributeValue::String(path.into()),
                )])),
            }
        }

        fn empty_plan() -> GeneratedRepositoryFileEffectPlan {
            GeneratedRepositoryFileEffectPlan::build(std::iter::empty::<(
                CompactString,
                Arc<[u8]>,
                bool,
            )>())
            .unwrap()
        }

        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = ApparentRepoName::new("rust_toolchains").unwrap();
        let canonical = CanonicalRepoName::new("rules_rust++rust+rust_toolchains").unwrap();

        // Rejection polarity: wrong policy and root apparent name.
        assert!(
            RootRepositoryRoute::for_generated_repo_spec(
                workspace.dupe(),
                apparent.clone(),
                canonical.clone(),
                local_repository_spec("dep"),
                HostRepositoryLocalPathPolicy::WorkspaceRelative,
                empty_plan(),
            )
            .is_none()
        );
        assert!(
            RootRepositoryRoute::for_generated_repo_spec(
                workspace.dupe(),
                ApparentRepoName::root(),
                canonical.clone(),
                local_repository_spec("dep"),
                HostRepositoryLocalPathPolicy::LocalUnsupported,
                empty_plan(),
            )
            .is_none()
        );
        let route = RootRepositoryRoute::for_generated_repo_spec(
            workspace.dupe(),
            apparent.clone(),
            canonical.clone(),
            local_repository_spec("dep"),
            HostRepositoryLocalPathPolicy::LocalUnsupported,
            empty_plan(),
        )
        .expect("generated view shape constructs a route");

        // Identity and capability projection.
        assert_eq!(route.workspace(), &workspace);
        assert_eq!(route.apparent_repo(), &apparent);
        assert_eq!(route.canonical_repo(), &canonical);
        assert!(!route.is_builtin_bazel_tools());
        let capability = route.source_capability();
        let _ = format!("{capability:?}");

        // The existing routed package owners accept the generated route.
        let package = PackageIdentifier::new(canonical.clone(), PackagePath::parse("pkg").unwrap());
        assert!(RepositoryPackageSourceKey::new(route.clone(), package).is_some());

        // Nonexecuted exact type-check across routed key surfaces.
        fn drives_routed_owners(_: &HostRepositorySourceFileKey, _: &RepositoryPackageSourceKey) {}
        fn hash_and_display(value: &RootRepositoryRoute) -> u64 {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        }
        let _ = |route: &RootRepositoryRoute| {
            drives_routed_owners(
                &HostRepositorySourceFileKey::new(route.clone(), "REPO.bazel".into()),
                &RepositoryPackageSourceKey::new(
                    route.clone(),
                    PackageIdentifier::new(
                        route.canonical_repo().clone(),
                        PackagePath::parse("pkg").unwrap(),
                    ),
                )
                .unwrap(),
            )
        };
        let hash_a = hash_and_display(&route);
        let hash_b = hash_and_display(&route);
        assert_eq!(hash_a, hash_b);
    }
}
