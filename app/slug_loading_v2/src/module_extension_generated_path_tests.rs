mod generated_path_tests {
    use std::time::Duration;

    use super::*;

    const EXTENSION: &str = r#"
def write(ctx):
    ctx.file('BUILD.bazel', "exports_files(['marker'])\n")
    ctx.file('marker', 'complete-producer')
    ctx.file('bin/tool', 'tool', executable=True)
repo = repository_rule(implementation=write)
path_tag = tag_class(attrs={'file': attr.label()})
def paths(ctx):
    print('consumer-before-path')
    repo(name='prefix')
    for mod in ctx.modules:
        for tag in mod.tags.path:
            path = ctx.path(tag.file)
            str(path.dirname.get_child('bin'))
def impl(ctx):
    paths(ctx)
    repo(name='proof')
def other_impl(ctx):
    paths(ctx)
    repo(name='other')
def third_impl(ctx):
    paths(ctx)
    repo(name='third')
ext = module_extension(implementation=impl, tag_classes={'path':path_tag}, environ=['FACTOR','MISSING','FACTOR'])
other = module_extension(implementation=other_impl, tag_classes={'path':path_tag}, environ=['FACTOR','MISSING','FACTOR'])
third = module_extension(implementation=third_impl, tag_classes={'path':path_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;

    fn graph() -> Arc<Dice> {
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        builder.build(DetectCycles::Enabled)
    }

    async fn case(
        dice: &Arc<Dice>,
        tracker: Arc<RepositoryRuleActivationTracker>,
        links: &[(&str, &str)],
    ) -> (
        dice::DiceTransaction,
        HostSelectedExtensionOwnerPureObservationKey,
    ) {
        let base = "module(name='consumer')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\nuse_repo(e, proof='proof')\nf=use_extension('@dep_alias//:ext.bzl','other')\nuse_repo(f, other='other')\ng=use_extension('@dep_alias//:ext.bzl','third')\nuse_repo(g, third='third')\n";
        let (mut bootstrap, _) = selected_path_case(dice, base, EXTENSION, tracker.clone()).await;
        let mapping = bootstrap
            .compute(&HostRootRepositoryMappingKey::new(
                NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(mapping) = mapping else {
            panic!("fixture mapping must complete")
        };
        let mapping = mapping.as_ref().as_ref().unwrap().view().unwrap();
        let mut module = base.to_owned();
        for (consumer, producer) in links {
            let canonical = mapping
                .mapping()
                .find(|(name, _)| name.as_str() == *producer)
                .unwrap()
                .1;
            module.push_str(&format!("{consumer}.path(file='{canonical}//:marker')\n"));
        }
        drop(bootstrap);
        selected_path_case(dice, &module, EXTENSION, tracker).await
    }

    async fn outcome(
        transaction: &mut dice::DiceTransaction,
        key: &HostSelectedExtensionOwnerPureObservationKey,
    ) -> <HostSelectedExtensionOwnerPureObservationKey as Key>::Value {
        tokio::time::timeout(Duration::from_secs(2), transaction.compute(key))
            .await
            .expect("generated owner wait must terminate")
            .unwrap()
    }

    fn no_consumer_publication(tracker: &RepositoryRuleActivationTracker, key: &impl ToString) {
        let rows = tracker.take();
        assert!(
            rows.iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
        assert!(
            rows.iter()
                .filter(|row| row.key == key.to_string())
                .flat_map(|row| row.batch.as_ref())
                .flat_map(EventBatch::events)
                .all(|event| !format!("{event:?}").contains("consumer-before-path"))
        );
    }

    #[tokio::test]
    async fn independent_generated_owner_returns_completed_producer_materialization_need() {
        let dice = graph();
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (mut tx, key) = case(&dice, tracker.clone(), &[("e", "other")]).await;
        tracker.take();
        let terminal = outcome(&mut tx, &key).await;
        let SourcePreparationOutcome::Need(need) = terminal else {
            panic!(
                "independent generated owner must request its actual materialization: {terminal:?}"
            )
        };
        assert_eq!(need.repository_materializations().len(), 1, "{need:?}");
        let request = need.repository_materializations().values().next().unwrap();
        let slug_bzlmod_v2::RepositoryMaterializationKind::GeneratedFileEffects(plan) =
            &request.kind
        else {
            panic!("Need must retain the completed producer's file effects: {request:?}")
        };
        assert!(
            plan.effects()
                .iter()
                .any(|effect| effect.path() == "BUILD.bazel"
                    && effect.content() == b"exports_files(['marker'])\n")
        );
        assert!(plan.effects().iter().any(|effect| effect.path() == "marker" && effect.content() == b"complete-producer"));
        assert!(
            plan.effects()
                .iter()
                .any(|effect| effect.path() == "bin/tool" && effect.executable())
        );
        no_consumer_publication(&tracker, &key);
        drop(tx);
        dice.wait_for_idle().await;
    }

    #[tokio::test]
    async fn reciprocal_and_three_owner_cycles_terminate_without_consumer_publication() {
        for links in [
            vec![("e", "other"), ("f", "proof")],
            vec![("e", "other"), ("f", "third"), ("g", "proof")],
        ] {
            let dice = graph();
            let tracker = Arc::new(RepositoryRuleActivationTracker::default());
            let (mut tx, key) = case(&dice, tracker.clone(), &links).await;
            tracker.take();
            let terminal = outcome(&mut tx, &key).await;
            let SourcePreparationOutcome::Complete(Ok(observed)) = terminal else {
                panic!("cycle must be a source-owned semantic terminal: {terminal:?}")
            };
            let error = observed.result().as_ref().as_ref().unwrap_err();
            assert!(
                format!("{error:?}")
                    .contains("cycle detected in generated repository dependencies"),
                "{error:?}"
            );
            no_consumer_publication(&tracker, &key);
            drop(tx);
            dice.wait_for_idle().await;
        }
    }

    #[tokio::test]
    async fn warm_cycle_insertion_and_removal_restores_valid_owner_result() {
        let dice = graph();
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (mut first_tx, key) = case(&dice, tracker.clone(), &[]).await;
        let first = outcome(&mut first_tx, &key).await;
        let SourcePreparationOutcome::Complete(Ok(first)) = first else {
            panic!("acyclic owner must complete")
        };
        assert!(first.result().as_ref().is_ok());
        let (mut cyclic_tx, cyclic_key) =
            case(&dice, tracker.clone(), &[("e", "other"), ("f", "proof")]).await;
        tracker.take();
        let cycle = outcome(&mut cyclic_tx, &cyclic_key).await;
        let SourcePreparationOutcome::Complete(Ok(cycle)) = cycle else {
            panic!("warm inserted cycle must terminate")
        };
        assert!(
            format!("{:?}", cycle.result().as_ref().as_ref().unwrap_err())
                .contains("cycle detected in generated repository dependencies")
        );
        no_consumer_publication(&tracker, &cyclic_key);
        let (mut restored_tx, restored_key) = case(&dice, tracker.clone(), &[]).await;
        let restored = outcome(&mut restored_tx, &restored_key).await;
        let SourcePreparationOutcome::Complete(Ok(restored)) = restored else {
            panic!("removed cycle must restore")
        };
        assert_eq!(restored.result(), first.result());
        drop((first_tx, cyclic_tx, restored_tx));
        dice.wait_for_idle().await;
    }
}
