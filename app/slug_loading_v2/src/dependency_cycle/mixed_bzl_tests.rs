mod mixed_bzl_tests {
    use std::time::Duration;

    use super::*;

    const DEP_MODULE: &str = "module(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, proof='proof')\n";

    struct MixedRegistryIo;

    #[async_trait]
    impl RegistryIo for MixedRegistryIo {
        async fn read_exact(
            &self,
            url: &RegistryFileUrl,
        ) -> Result<RegistryIoOutcome, RegistryTransportError> {
            if url.as_str() == "https://registry.invalid/modules/dep/1/MODULE.bazel" {
                Ok(RegistryIoOutcome::Found(Arc::from(DEP_MODULE.as_bytes())))
            } else {
                SelectedFactorRegistryIo.read_exact(url).await
            }
        }
    }

    #[tokio::test]
    async fn generated_path_and_producer_defining_bzl_cycle_has_fresh_bzl_listener() {
        generated_definition_cycle(0).await;
    }

    #[tokio::test]
    async fn selected_tools_generated_definition_cycle_terminates_without_certificate() {
        generated_definition_cycle(1).await;
    }

    #[tokio::test]
    async fn runtime_profile_generated_definition_cycle_terminates_without_certificate() {
        generated_definition_cycle(2).await;
    }

    #[tokio::test]
    async fn guest_namespace_generated_definition_cycle_terminates_without_view() {
        generated_definition_cycle(3).await;
    }

    #[tokio::test]
    async fn issued_data_label_generated_definition_cycle_terminates_without_sources() {
        generated_definition_cycle(4).await;
    }

    async fn generated_definition_cycle(consumer: u8) {
        let with_tools = consumer != 0;
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\nuse_repo(e, proof='proof')\nf=use_extension('@dep_alias//:b.bzl','other')\nuse_repo(f, other='other')\n";
        const SAFE: &str = "repo=repository_rule(lambda ctx:None)\ndef impl(ctx):\n    repo(name='proof')\next=module_extension(implementation=impl,environ=['FACTOR','MISSING','FACTOR'])\n";
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(MixedRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (mut tx, key) = selected_path_case(&dice, MODULE, SAFE, tracker.clone()).await;
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let SourcePreparationOutcome::Complete(mapping) = tx
            .compute(&HostRootRepositoryMappingKey::new(workspace.dupe()))
            .await
            .unwrap()
        else {
            panic!("fixture mapping must complete")
        };
        let mapping = mapping.as_ref().as_ref().unwrap().view().unwrap();
        let canonical = |name: &str| {
            mapping
                .mapping()
                .find(|(apparent, _)| apparent.as_str() == name)
                .unwrap()
                .1
                .clone()
        };
        let a = canonical("proof");
        let b = canonical("other");
        let SourcePreparationOutcome::Complete(demand) = tx
            .compute(&HostSelectedExtensionDemandKey::new(
                workspace.dupe(),
                b.clone(),
            ))
            .await
            .unwrap()
        else {
            panic!("producer metadata must complete")
        };
        let SourcePreparationOutcome::Complete(inputs) = tx
            .compute(&HostSelectedExtensionOwnerInputsKey::new(
                workspace.dupe(),
                demand.as_ref().as_ref().unwrap().owner().clone(),
            ))
            .await
            .unwrap()
        else {
            panic!("producer defining source must complete")
        };
        let inputs = inputs.as_ref().as_ref().unwrap();
        let route = RootRepositoryRoute::for_selected_extension_definition(
            workspace.dupe(),
            inputs.request().source(),
        )
        .unwrap();
        let label = RepositoryBzlLabel::new(
            inputs.request().parts().0.package().package().clone(),
            RootPackageBzlTarget::parse(inputs.request().parts().0.target().as_str()).unwrap(),
        )
        .unwrap();
        let bzl_key = ExternalBzlModuleObservationKey::new_bzlmod(route, label);
        let extension = format!(
            "repo=repository_rule(lambda ctx:None)\ndef impl(ctx):\n    print('consumer-before-mixed-cycle')\n    repo(name='prefix')\n    ctx.path(Label('{b}//:marker'))\n    repo(name='proof')\next=module_extension(implementation=impl,environ=['FACTOR','MISSING','FACTOR'])\n"
        );
        // The defining selected module imports A itself. Its apparent load is
        // resolved through that module's actual mapping to the canonical route.
        let defining_source = "load('@proof//:helper.bzl','symbol')\nrepo=repository_rule(lambda ctx:None)\ndef impl(ctx):\n    repo(name='other')\nother=module_extension(implementation=impl,environ=['FACTOR','MISSING','FACTOR'])\n";
        let namespace =
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(77));
        let b_path = NormalizedAbsolutePath::new("/registry-dep/b.bzl").unwrap();
        let module_path = NormalizedAbsolutePath::new("/registry-dep/MODULE.bazel").unwrap();
        let epoch = selected_factor_epoch(MODULE, &extension);
        let epoch = PathObservationEpoch::from_shared(
            epoch
                .observations()
                .iter()
                .filter(|(demand, _)| {
                    !(demand.namespace() == namespace
                        && demand.path() == &module_path
                        && demand.operation() == PathObservationOperation::FileBytes)
                })
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .chain([
                    (
                        PathObservationDemand::new(
                            namespace.clone(),
                            module_path.dupe(),
                            PathObservationOperation::FileBytes,
                        ),
                        Arc::new(PathObservationResult::FileBytes(
                            PathOperationResult::Present(Arc::from(DEP_MODULE.as_bytes())),
                        )),
                    ),
                    (
                        PathObservationDemand::new(
                            namespace.clone(),
                            b_path.dupe(),
                            PathObservationOperation::Lstat,
                        ),
                        Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                            PathLstat::new(PathNodeKind::RegularFile, 85, 1, 1, 1, 0o755),
                        ))),
                    ),
                    (
                        PathObservationDemand::new(
                            namespace.clone(),
                            b_path.dupe(),
                            PathObservationOperation::FileBytes,
                        ),
                        Arc::new(PathObservationResult::FileBytes(
                            PathOperationResult::Present(Arc::from(defining_source.as_bytes())),
                        )),
                    ),
                ]),
        )
        .unwrap();
        tx = selected_watch_observations(tx, epoch).await;
        let resources = scratch_test_resources();
        let tool_key = super::super::selected_tools::SelectedExtensionToolSourcesKey::new(
            super::super::selected_scratch::HostSelectedExtensionScratchPrefixKey::new(
                workspace.dupe(),
                key.0.owner.clone(),
                slug_bzlmod_v2::SelectedScratchPolicy::plain(),
            ),
            tool_test_policy(),
        );
        if with_tools {
            tx = scratch_test_inputs(
                tx,
                tracker.clone(),
                &resources,
                RepositoryEnvironmentSnapshot::empty(),
                None,
            )
            .await;
        }
        let runtime_key = super::super::runtime_sources::RuntimeSourceProfileKey::new(
            tool_key.requests_key().clone(),
        );
        if consumer >= 2 {
            let policy = runtime_test_policy(
                &resources,
                &[runtime_test_relative("runtime", "/runtime")],
                Default::default(),
            );
            let guest = if consumer >= 3 {
                let guest = slug_bzlmod_v2::GuestNamespacePolicy::new(
                    1000,
                    1000,
                    1000,
                    1000,
                    7,
                    0,
                    0,
                    0o755,
                    Default::default(),
                )
                .unwrap();
                let guest = if consumer == 4 {
                    guest
                        .with_label_data(slug_bzlmod_v2::GuestLabelDataPolicy::IssuedFileLeaves {
                            max_witnesses: 64,
                        })
                        .unwrap()
                } else {
                    guest
                };
                let current = slug_bzlmod_v2::GuestNamespaceTransaction::new(
                    workspace.dupe(),
                    Some(guest),
                    resources.descriptor_budget().dupe(),
                )
                .unwrap();
                Some((Some(guest), Some(current)))
            } else {
                None
            };
            tx = runtime_test_install_with_guest(
                tx,
                tracker.clone(),
                &resources,
                Some(policy.dupe()),
                Some(runtime_test_authorization(&policy, &resources)),
                None,
                None,
                guest,
            )
            .await;
        }
        tracker.take();
        tracker.take_dependencies();
        // A separate ordinary request retains the defining .bzl waiter until its
        // own mixed notification records poison, even if the owner finishes first.
        let mut bzl_tx = tx.dupe();
        let mut tool_tx = tx.dupe();
        let mut runtime_tx = tx.dupe();
        let mut guest_tx = tx.dupe();
        let guest_key =
            super::super::guest_namespace::SelectedGuestNamespaceFactsKey::new(runtime_key.clone());
        let mut data_tx = tx.dupe();
        let data_key = super::super::selected_label_data::SelectedDataLabelSourcesKey::new(
            runtime_key.clone(),
        );
        let (owner, bzl, tools, runtime, guest, data) =
            tokio::time::timeout(Duration::from_secs(2), async {
                tokio::join!(
                    tx.compute(&key),
                    bzl_tx.compute(&bzl_key),
                    async {
                        if consumer == 1 {
                            Some(
                                crate::compute_selected_tool_sources(&mut tool_tx, &tool_key)
                                    .await
                                    .unwrap(),
                            )
                        } else {
                            None
                        }
                    },
                    async {
                        if consumer == 2 {
                            Some(
                                crate::compute_runtime_source_profile(
                                    &mut runtime_tx,
                                    &runtime_key,
                                )
                                .await
                                .unwrap(),
                            )
                        } else {
                            None
                        }
                    },
                    async {
                        if consumer == 3 {
                            Some(
                                crate::compute_selected_guest_namespace_facts(
                                    &mut guest_tx,
                                    &guest_key,
                                )
                                .await
                                .unwrap(),
                            )
                        } else {
                            None
                        }
                    },
                    async {
                        if consumer == 4 {
                            Some(
                                crate::compute_selected_data_label_sources(&mut data_tx, &data_key)
                                    .await
                                    .unwrap(),
                            )
                        } else {
                            None
                        }
                    }
                )
            })
            .await
            .expect("mixed generated-source cycle must terminate");
        let SourcePreparationOutcome::Complete(Ok(owner)) = owner.unwrap() else {
            panic!("owner cycle must be a semantic terminal")
        };
        let mut error = owner.result().as_ref().as_ref().unwrap_err();
        while let HostSelectedExtensionOwnerPureError::Factored { error: inner, .. } = error {
            error = inner;
        }
        let HostSelectedExtensionOwnerPureError::AfterInputs {
            current_calls,
            message,
            ..
        } = error
        else {
            panic!("owner must retain its pre-path diagnostic prefix: {error:?}")
        };
        assert_eq!(
            current_calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>(),
            ["prefix"],
            "owner cycle diagnostic: {message}"
        );
        assert!(
            message.contains("cycle detected in generated repository dependencies"),
            "unexpected owner diagnostic: {message}"
        );
        let SourcePreparationOutcome::Complete(Ok(bzl)) = bzl.unwrap() else {
            panic!("defining bzl must observe its own cycle terminal")
        };
        let Err(crate::bzl_module::ExternalBzlModuleError::DependencyCycle(cycle)) =
            bzl.result().as_ref()
        else {
            panic!(
                "fresh defining bzl must consume its own mixed notification: {:?}",
                bzl.result()
            )
        };
        let natural_route = crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteObservationKey::new(workspace, a).to_string();
        assert!(
            cycle.to_string().contains(&natural_route),
            "cycle {cycle} omitted natural route {natural_route}"
        );
        assert!(
            bzl.observations()
                .observations()
                .keys()
                .any(|demand| demand.namespace() == namespace && demand.path() == &b_path),
            "defining bzl observations: {:?}",
            bzl.observations()
        );
        if let Some(tools) = tools {
            assert!(
                matches!(
                    tools,
                    SourcePreparationOutcome::Complete(Err(
                        super::super::selected_tools::SelectedToolSourcesError::Prefix(_)
                    ))
                ),
                "mixed cycle must refuse the actual normal certificate: {tools:?}"
            );
        }
        if let Some(runtime) = runtime {
            assert!(
                matches!(
                    runtime,
                    SourcePreparationOutcome::Complete(Err(
                        super::super::runtime_sources::RuntimeSourcesError::Tool(
                            super::super::selected_tools::SelectedToolSourcesError::Prefix(_)
                        )
                    ))
                ),
                "mixed cycle must refuse the new normal runtime profile: {runtime:?}"
            );
        }
        if let Some(guest) = guest {
            assert!(
                matches!(
                    guest,
                    SourcePreparationOutcome::Complete(Err(
                        super::super::guest_namespace::GuestNamespaceFactsError::Runtime(
                            super::super::runtime_sources::RuntimeSourcesError::Tool(
                                super::super::selected_tools::SelectedToolSourcesError::Prefix(_)
                            )
                        )
                    ))
                ),
                "mixed cycle must refuse actual normal namespace: {guest:?}"
            );
        }
        if let Some(data) = data {
            assert!(
                matches!(
                    data,
                    SourcePreparationOutcome::Complete(Err(
                        super::super::selected_label_data::SelectedDataLabelError::Runtime(
                            super::super::runtime_sources::RuntimeSourcesError::Tool(
                                super::super::selected_tools::SelectedToolSourcesError::Prefix(_)
                            )
                        )
                    ))
                ),
                "data consumer must consume typed mixed error: {data:?}"
            );
        }
        drop((tx, bzl_tx, tool_tx, runtime_tx, guest_tx, data_tx));
        dice.wait_for_idle().await;
        let dependencies = tracker.take_dependencies();
        if consumer == 1 {
            let (_, tool_dependencies) = dependencies
                .iter()
                .find(|(name, _)| name == &tool_key.to_string())
                .expect("new Sources consumer must activate genuinely");
            assert!(
                tool_dependencies.contains(&tool_key.requests_key().to_string()),
                "{tool_dependencies:?}"
            );
            let (_, request_dependencies) = dependencies
                .iter()
                .find(|(name, _)| name == &tool_key.requests_key().to_string())
                .expect("new Requests consumer must activate genuinely");
            assert!(
                request_dependencies.contains(&tool_key.checkpoint().to_string()),
                "{request_dependencies:?}"
            );
        }
        if consumer == 3 {
            let (_, dependencies) = dependencies
                .iter()
                .find(|(name, _)| name == &guest_key.to_string())
                .expect("Namespace consumer must genuinely activate");
            assert!(
                dependencies.contains(&runtime_key.to_string()),
                "{dependencies:?}"
            );
        }
        if consumer == 2 || consumer == 3 {
            let (_, profile_dependencies) = dependencies
                .iter()
                .find(|(name, _)| name == &runtime_key.to_string())
                .expect("runtime Profile consumer must activate genuinely");
            assert!(
                profile_dependencies.contains(&runtime_key.requests_key().to_string()),
                "{profile_dependencies:?}"
            );
            let (_, runtime_dependencies) = dependencies
                .iter()
                .find(|(name, _)| name == &runtime_key.requests_key().to_string())
                .expect("runtime Requests consumer must activate genuinely");
            assert!(
                runtime_dependencies.contains(&tool_key.requests_key().to_string()),
                "{runtime_dependencies:?}"
            );
            let (_, tool_dependencies) = dependencies
                .iter()
                .find(|(name, _)| name == &tool_key.requests_key().to_string())
                .expect("normal ToolRequests dependency must activate genuinely");
            assert!(
                tool_dependencies.contains(&tool_key.checkpoint().to_string()),
                "{tool_dependencies:?}"
            );
        }
        if consumer == 4 {
            let (_, source_deps) = dependencies
                .iter()
                .find(|(name, _)| name == &data_key.to_string())
                .expect("DataSources must genuinely activate");
            assert!(source_deps.contains(&data_key.requests_key().to_string()));
            let (_, request_deps) = dependencies
                .iter()
                .find(|(name, _)| name == &data_key.requests_key().to_string())
                .expect("DataRequests must genuinely activate");
            assert!(request_deps.contains(&runtime_key.requests_key().to_string()));
        }
        let (_, fresh_bzl_dependencies) = dependencies
            .iter()
            .find(|(name, _)| name == &bzl_key.to_string())
            .expect("defining bzl must activate freshly");
        assert!(
            fresh_bzl_dependencies
                .iter()
                .any(|name| name == "bzl-load-cycle-poison"),
            "fresh defining bzl dependencies: {fresh_bzl_dependencies:?}"
        );
        // The raced route need not activate: its exact key is in the cycle above,
        // while this fresh listener's activation records the poison dependency.
        let rows = tracker.take();
        assert!(
            rows.iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none()),
            "owner published a partial batch: {rows:?}"
        );
        assert!(
            rows.iter()
                .flat_map(|row| row.batch.as_ref())
                .flat_map(EventBatch::events)
                .all(|event| !format!("{event:?}").contains("consumer-before-mixed-cycle")),
            "failed consumer published its diagnostic print: {rows:?}"
        );
    }
}
