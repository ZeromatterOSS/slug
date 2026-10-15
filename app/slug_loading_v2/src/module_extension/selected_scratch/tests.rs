// Included in the existing selected owner tests: reuse its genuine definition/usage fixture.
async fn scratch_test_inputs(
    transaction: dice::DiceTransaction,
    tracker: Arc<RepositoryRuleActivationTracker>,
    resources: &slug_bzlmod_v2::SelectedScratchResources,
    snapshot: RepositoryEnvironmentSnapshot,
    deadline: Option<std::time::Instant>,
) -> dice::DiceTransaction {
    let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
    let mut data = dice::UserComputationData {
        cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
        activation_tracker: Some(tracker),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    data.data.set(resources.descriptor_budget().clone());
    data.data.set(resources.clone());
    data.data.set(RepositoryHostInputTransaction::new(
        snapshot.clone(),
        RepositoryEnvironmentNameFrontier::from_unsorted([
            CompactString::new("FACTOR"),
            CompactString::new("MISSING"),
        ]),
    ));
    data.data.set(SelectedExtensionDownloadTransaction::empty(
        workspace.clone(),
    ));
    if let Some(deadline) = deadline {
        data.data
            .set(super::selected_scratch::SelectedScratchPreparationDeadline::new(deadline));
    }
    let mut updater = transaction.into_updater();
    updater
        .changed_to(["FACTOR", "MISSING"].map(|name| {
            (
                RepositoryEnvironmentCellKey::new(workspace.clone(), name),
                slug_bzlmod_v2::RepositoryEnvironmentCell::observed(snapshot.get(name).cloned()),
            )
        }))
        .unwrap();
    updater
        .changed_to([(
            RepositoryPlatformKey::new(workspace.clone()),
            RepositoryPlatform::new("linux", "x86_64"),
        )])
        .unwrap();
    updater
        .changed_to([(
            slug_bzlmod_v2::RepositoryEnvironmentSnapshotKey::new(workspace),
            snapshot,
        )])
        .unwrap();
    updater.commit_with_data(data).await
}
fn scratch_test_resources() -> slug_bzlmod_v2::SelectedScratchResources {
    slug_bzlmod_v2::SelectedScratchResources::new(
        slug_bzlmod_v2::SelectedScratchPayloadBudget::new(256 << 20),
        slug_workspace_v2::ImageDescriptorBudget::new(64 << 20),
    )
}
const SCRATCH_TEST_MODULE: &str = "module(name='consumer')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.from_cargo(value='final')\nuse_repo(e, proof='proof')\n";
const SCRATCH_TEST_EXTENSION: &str = r#"
from_cargo = tag_class(attrs={'value':attr.string()})
def impl(ctx):
    print('private attempt must not publish')
    p = ctx.path('crate').get_child('config.json')
    ctx.file(p, 'earlier', executable=False)
    ctx.file(p, ctx.modules[0].tags.from_cargo[0].value, executable=True)
    ctx.execute(['generator', 'query', p], environment={'EMPTY':'', 'REMOVE':None})
    fail('post-process statement must not execute')
ext = module_extension(implementation=impl, tag_classes={'from_cargo':from_cargo}, environ=['FACTOR','MISSING','FACTOR'])
"#;
fn scratch_test_value(
    outcome: &<super::selected_scratch::HostSelectedExtensionScratchPrefixKey as Key>::Value,
) -> &super::selected_scratch::ObservedSelectedExtensionScratchPrefix {
    let SourcePreparationOutcome::Complete(Ok(value)) = outcome else {
        panic!("normal checkpoint did not complete: {outcome:?}")
    };
    value
}
#[tokio::test]
async fn selected_scratch_normal_first_boundary_order_modes_and_raw_environment_a_b_a() {
    use super::selected_scratch::HostSelectedExtensionScratchPrefixKey;
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(RepositoryRuleActivationTracker::default());
    let (base, owner) = selected_factor_transaction(
        &dice,
        SCRATCH_TEST_MODULE,
        SCRATCH_TEST_EXTENSION,
        tracker.clone(),
        true,
    )
    .await;
    let resources = scratch_test_resources();
    let key = HostSelectedExtensionScratchPrefixKey::new(
        NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
        owner,
        slug_bzlmod_v2::SelectedScratchPolicy::plain(),
    );
    let mut tx = scratch_test_inputs(
        base,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        None,
    )
    .await;
    let a = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    let value = scratch_test_value(&a);
    assert_eq!(
        value.prefix().process().arguments()[2].spelling(),
        "/__slug_module/crate/config.json"
    );
    let file = value
        .prefix()
        .entries()
        .iter()
        .find(|entry| entry.path().relative() == "crate/config.json")
        .unwrap();
    assert_eq!(file.blob().unwrap().bytes(), b"final");
    assert_eq!(file.mode(), 0o755);
    assert_eq!(
        value.prefix().occurrence_bytes(),
        b"earlier".len() + b"final".len()
    );
    assert!(value.matches_key(&key));
    assert!(value.same_resources(&resources));
    let changed = RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
        RepositoryEnvironmentEntry::new("UNRELATED", "secret-pin"),
    ]))
    .unwrap();
    tx = scratch_test_inputs(tx, tracker.clone(), &resources, changed, None).await;
    let b = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    assert!(!HostSelectedExtensionScratchPrefixKey::equality(&a, &b));
    assert!(!format!("{:?}", scratch_test_value(&b)).contains("secret-pin"));
    tx = scratch_test_inputs(
        tx,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        None,
    )
    .await;
    let restored = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    assert!(HostSelectedExtensionScratchPrefixKey::equality(
        &a, &restored
    ));
    // No extension-implementation print batch or repository receipt is stored.
    assert!(!tracker.take().iter().any(|activation| matches!(&activation.batch, Some(batch) if batch.events().iter().any(|event| matches!(event, EvaluationEvent::StarlarkPrint {text,..} if text.as_str().contains("private attempt"))))));
}
#[tokio::test]
async fn selected_scratch_private_methods_preserve_ordinary_absence_and_sticky_refusal() {
    use super::selected_scratch::HostSelectedExtensionScratchPrefixKey;
    for extension in [
        "def impl(ctx):\n    if hasattr(ctx,'file'):\n        ctx.execute(['generator'])\n    return ctx.extension_metadata(reproducible=True)\next=module_extension(implementation=impl,environ=['FACTOR','MISSING','FACTOR'])\n",
        "def impl(ctx):\n    p=ctx.path('one')\n    getattr(p,'unknown','fallback')\n    ctx.execute(['generator'])\next=module_extension(implementation=impl,environ=['FACTOR','MISSING','FACTOR'])\n",
        "def impl(ctx):\n    p=ctx.path('one')\n    ctx.file(p,'private bytes')\n    ctx.read(p)\n    ctx.execute(['generator'])\next=module_extension(implementation=impl,environ=['FACTOR','MISSING','FACTOR'])\n",
    ] {
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let module = SCRATCH_TEST_MODULE.replace("e.from_cargo(value='final')\n", "");
        let (base, owner) =
            selected_factor_transaction(&dice, &module, extension, tracker.clone(), true).await;
        let resources = scratch_test_resources();
        let mut tx = scratch_test_inputs(
            base,
            tracker,
            &resources,
            RepositoryEnvironmentSnapshot::empty(),
            None,
        )
        .await;
        let key = HostSelectedExtensionScratchPrefixKey::new(
            NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
            owner.clone(),
            slug_bzlmod_v2::SelectedScratchPolicy::plain(),
        );
        let outcome = crate::compute_selected_scratch_prefix(&mut tx, &key)
            .await
            .unwrap();
        if extension.contains("hasattr") {
            scratch_test_value(&outcome);
            let pure = tx
                .compute(&HostSelectedExtensionOwnerPureObservationKey::new(
                    key.workspace().clone(),
                    owner,
                ))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(Ok(pure)) = pure else {
                panic!("ordinary owner failed")
            };
            assert!(
                pure.result().as_ref().is_ok(),
                "ordinary methods widened: {pure:?}"
            );
        } else {
            assert!(
                matches!(outcome, SourcePreparationOutcome::Complete(Err(ref e)) if !e.is_transient())
            );
        }
    }
}
#[tokio::test]
async fn selected_scratch_warm_foreign_resources_and_expired_prepare_are_operational() {
    use super::selected_scratch::HostSelectedExtensionScratchPrefixKey;
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(RepositoryRuleActivationTracker::default());
    let (base, owner) = selected_factor_transaction(
        &dice,
        SCRATCH_TEST_MODULE,
        SCRATCH_TEST_EXTENSION,
        tracker.clone(),
        true,
    )
    .await;
    let resources = scratch_test_resources();
    let key = HostSelectedExtensionScratchPrefixKey::new(
        NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
        owner,
        slug_bzlmod_v2::SelectedScratchPolicy::plain(),
    );
    let mut tx = scratch_test_inputs(
        base,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        Some(std::time::Instant::now() - std::time::Duration::from_secs(1)),
    )
    .await;
    let expired = tx.compute(&key).await.unwrap();
    assert!(!HostSelectedExtensionScratchPrefixKey::validity(&expired));
    tx = scratch_test_inputs(
        tx,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        None,
    )
    .await;
    scratch_test_value(
        &crate::compute_selected_scratch_prefix(&mut tx, &key)
            .await
            .unwrap(),
    );
    let foreign = scratch_test_resources();
    tx = scratch_test_inputs(
        tx,
        tracker.clone(),
        &foreign,
        RepositoryEnvironmentSnapshot::empty(),
        None,
    )
    .await;
    assert!(
        crate::compute_selected_scratch_prefix(&mut tx, &key)
            .await
            .unwrap_err()
            .is_transient()
    );
}
