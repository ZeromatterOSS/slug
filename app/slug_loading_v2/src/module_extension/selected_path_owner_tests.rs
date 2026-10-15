#[tokio::test]
async fn budget_exhaustion_is_not_cached_after_reopening_same_owner() {
    use slug_bzlmod_v2::install_registry_io;
    use slug_workspace_v2::ImageDescriptorBudget;

    const MODULE: &str = "module(name='consumer')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.path(file='//:missing')\nuse_repo(e, proof='proof')\n";
    const EXTENSION: &str = r#"
repo=repository_rule(lambda ctx: None)
path_tag=tag_class(attrs={'file':attr.label()})
def impl(ctx):
    print('before-source')
    for mod in ctx.modules:
        for tag in mod.tags.path:
            str(ctx.path(tag.file).dirname.get_child('bin/tool'))
    repo(name='proof')
ext=module_extension(implementation=impl, tag_classes={'path':path_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;
    let mut builder = dice::Dice::builder();
    install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
    let dice = Arc::new(builder.build(dice::DetectCycles::Enabled));
    let tracker = Arc::new(RepositoryRuleActivationTracker::default());
    let (mut tx, key) = selected_path_case(&dice, MODULE, EXTENSION, tracker.clone()).await;
    let budget = tx
        .per_transaction_data()
        .data
        .get::<ImageDescriptorBudget>()
        .unwrap()
        .dupe();
    let blocker = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    tracker.take();
    let failed = tx.compute(&key).await.unwrap();
    let SourcePreparationOutcome::Complete(Ok(observed)) = &failed else {
        panic!("expected typed owner error: {failed:?}");
    };
    assert!(
        observed
            .result()
            .as_ref()
            .as_ref()
            .unwrap_err()
            .is_transient()
    );
    assert!(!HostSelectedExtensionOwnerPureObservationKey::validity(
        &failed
    ));
    assert!(!HostSelectedExtensionOwnerPureObservationKey::equality(
        &failed, &failed
    ));
    let legacy = tx.compute(&key.0).await.unwrap();
    assert!(!HostSelectedExtensionOwnerPureKey::validity(&legacy));
    let rows = tracker.take();
    assert!(
        rows.iter()
            .filter(|row| row.key == key.to_string())
            .all(|row| row.batch.is_none())
    );
    drop(blocker);
    let updater = tx.into_updater();
    dice.wait_for_idle().await;
    let mut reopened = updater.commit().await;
    let repaired = reopened.compute(&key).await.unwrap();
    let SourcePreparationOutcome::Complete(Ok(observed)) = &repaired else {
        panic!("owner repair did not complete: {repaired:?}");
    };
    assert!(observed.result().as_ref().is_ok(), "{repaired:?}");
    assert!(HostSelectedExtensionOwnerPureObservationKey::validity(
        &repaired
    ));
    assert!(
        tracker
            .take()
            .iter()
            .any(|row| row.key == key.to_string() && row.batch.is_some())
    );
    drop(reopened);
    dice.wait_for_idle().await;
}
