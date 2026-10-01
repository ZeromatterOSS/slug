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
        tracker.take();
        tracker.take_dependencies();
        // A separate ordinary request retains the defining .bzl waiter until its
        // own mixed notification records poison, even if the owner finishes first.
        let mut bzl_tx = tx.dupe();
        let (owner, bzl) = tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(tx.compute(&key), bzl_tx.compute(&bzl_key))
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
        drop((tx, bzl_tx));
        dice.wait_for_idle().await;
        let dependencies = tracker.take_dependencies();
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
