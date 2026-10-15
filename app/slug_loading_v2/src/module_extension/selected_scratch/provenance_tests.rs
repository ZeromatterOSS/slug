// Included in module_extension::tests; all prefixes are computed by the real Loading key.
use std::path::Path;

async fn scratch_provenance_inputs(
    transaction: dice::DiceTransaction,
    tracker: Arc<RepositoryRuleActivationTracker>,
    resources: &slug_bzlmod_v2::SelectedScratchResources,
    snapshot: RepositoryEnvironmentSnapshot,
    observe_dynamic: bool,
    verified: Option<Arc<SelectedExtensionDownloadVerified>>,
) -> dice::DiceTransaction {
    let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
    let names = RepositoryEnvironmentNameFrontier::from_unsorted(
        ["FACTOR", "MISSING"]
            .into_iter()
            .chain(observe_dynamic.then_some("DYNAMIC"))
            .map(CompactString::new),
    );
    let download = match &verified {
        Some(value) => SelectedExtensionDownloadTransaction::new(
            workspace.dupe(),
            [value.request().owner().clone()],
            [value.clone()],
        )
        .unwrap(),
        None => SelectedExtensionDownloadTransaction::empty(workspace.dupe()),
    };
    let mut data = UserComputationData {
        cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
        activation_tracker: Some(tracker),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    data.data.set(resources.descriptor_budget().clone());
    data.data.set(resources.clone());
    data.data.set(RepositoryHostInputTransaction::new(
        snapshot.clone(),
        names.clone(),
    ));
    data.data.set(download);
    let mut updater = transaction.into_updater();
    updater
        .changed_to([(
            RepositoryPlatformKey::new(workspace.dupe()),
            RepositoryPlatform::new("linux", "x86_64"),
        )])
        .unwrap();
    updater
        .changed_to([(
            slug_bzlmod_v2::RepositoryEnvironmentSnapshotKey::new(workspace.dupe()),
            snapshot.clone(),
        )])
        .unwrap();
    updater
        .changed_to(
            names
                .iter()
                .map(|name| {
                    (
                        RepositoryEnvironmentCellKey::new(workspace.dupe(), name.clone()),
                        RepositoryEnvironmentCell::observed(snapshot.get(name).cloned()),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
    if let Some(value) = verified {
        updater
            .changed_to([(
                SelectedExtensionDownloadEpochKey::new(workspace, value.request().owner().clone()),
                SelectedExtensionDownloadEpoch::Ready(value),
            )])
            .unwrap();
    }
    updater.commit_with_data(data).await
}

fn scratch_provenance_no_publication(
    tracker: &RepositoryRuleActivationTracker,
    key: &super::selected_scratch::HostSelectedExtensionScratchPrefixKey,
) {
    let pure = HostSelectedExtensionOwnerPureObservationKey::new(
        key.workspace().dupe(),
        key.owner().clone(),
    )
    .to_string();
    let legacy =
        HostSelectedExtensionOwnerPureKey::new(key.workspace().dupe(), key.owner().clone())
            .to_string();
    let rows = tracker.take();
    assert!(
        rows.iter().all(|row| row.key != pure && row.key != legacy),
        "checkpoint must not compute the outer PureOwner"
    );
    assert!(
        rows.iter()
            .flat_map(|row| row.batch.as_ref())
            .flat_map(EventBatch::events)
            .all(
                |event| !matches!(event, EvaluationEvent::StarlarkPrint { text, .. }
            if text.as_str().contains("scratch-provenance-attempt"))
            ),
        "private attempts must not publish extension prints"
    );
}

const SCRATCH_PROVENANCE_LABEL_MODULE: &str = "module(name='consumer')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.path(file='//pkg:missing')\nuse_repo(e, proof='proof')\n";
const SCRATCH_PROVENANCE_LABEL_EXTENSION: &str = r#"
path_tag=tag_class(attrs={'file':attr.label()})
def impl(ctx):
    print('scratch-provenance-attempt')
    origin=ctx.path(ctx.modules[0].tags.path[0].file)
    parent=origin.dirname
    tool=parent.get_child('bin/tool')
    ctx.execute([tool,str(tool),'--origin='+str(origin)])
    fail('after-boundary')
ext=module_extension(implementation=impl,tag_classes={'path':path_tag},environ=['FACTOR','MISSING','FACTOR'])
"#;

async fn scratch_provenance_label_case() -> (
    Arc<Dice>,
    dice::DiceTransaction,
    super::selected_scratch::HostSelectedExtensionScratchPrefixKey,
    slug_bzlmod_v2::SelectedScratchResources,
    Arc<RepositoryRuleActivationTracker>,
) {
    use super::selected_scratch::HostSelectedExtensionScratchPrefixKey;
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
    let dice = builder.build(DetectCycles::Enabled);
    let tracker = Arc::new(RepositoryRuleActivationTracker::default());
    let (base, owner) = selected_factor_transaction(
        &dice,
        SCRATCH_PROVENANCE_LABEL_MODULE,
        SCRATCH_PROVENANCE_LABEL_EXTENSION,
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
    let tx = scratch_provenance_inputs(
        base,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        false,
        None,
    )
    .await;
    (dice, tx, key, resources, tracker)
}

#[tokio::test]
async fn selected_scratch_label_provenance_need_replay_and_typed_arguments() {
    use slug_bzlmod_v2::SelectedLabelSourceTransform;
    use slug_bzlmod_v2::SelectedScratchOperation;

    use super::selected_scratch::HostSelectedExtensionScratchPrefixKey;
    let (_dice, mut tx, key, resources, tracker) = scratch_provenance_label_case().await;
    tracker.take();
    let cold = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    assert!(
        matches!(&cold, SourcePreparationOutcome::Need(need) if need.path_observations().is_some()),
        "{cold:?}"
    );
    assert!(!HostSelectedExtensionScratchPrefixKey::validity(&cold));
    assert_eq!(resources.payload_budget().used_bytes(), 0);
    scratch_provenance_no_publication(&tracker, &key);
    tx = selected_watch_observations(
        tx,
        selected_path_package_epoch(
            SCRATCH_PROVENANCE_LABEL_MODULE,
            SCRATCH_PROVENANCE_LABEL_EXTENSION,
            false,
            true,
        ),
    )
    .await;
    let completed = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    let value = scratch_test_value(&completed);
    let labels = value.label_sources();
    assert_eq!(labels.len(), 3);
    assert_eq!(labels[0].selection().recipe().transforms().len(), 0);
    assert!(matches!(
        labels[1].selection().recipe().transforms(),
        [SelectedLabelSourceTransform::Parent]
    ));
    assert!(matches!(
        labels[2].selection().recipe().transforms(),
        [
            SelectedLabelSourceTransform::Parent,
            SelectedLabelSourceTransform::Child(_)
        ]
    ));
    assert_eq!(
        labels[0].selection().recipe().label(),
        labels[2].selection().recipe().label()
    );
    assert_eq!(
        labels[2].selection().selected_file().as_path(),
        Path::new(&format!("{REPO_RULE_WORKSPACE}/pkg/bin/tool"))
    );
    assert_eq!(
        labels[2].selection().image_key().source_root().as_path(),
        Path::new(&format!("{REPO_RULE_WORKSPACE}/pkg"))
    );
    let arguments = value.prefix().process().arguments();
    assert_eq!(arguments[0].spelling(), arguments[1].spelling());
    assert_eq!(
        arguments[0].external_path().unwrap().event_index(),
        labels[2].event_index()
    );
    assert!(arguments[1].external_path().is_none());
    assert_eq!(
        arguments[2].spelling(),
        format!("--origin={REPO_RULE_WORKSPACE}/pkg/missing")
    );
    for label in labels {
        assert!(
            matches!(&value.prefix().operations()[label.event_index()], SelectedScratchOperation::ExternalPath { path } if path.event_index() == label.event_index())
        );
        assert!(label.selection().same_budget(resources.descriptor_budget()));
    }
    assert!(
        value
            .observations()
            .observations()
            .iter()
            .all(|(demand, _)| !demand.path().as_path().ends_with("pkg/missing")),
        "Label authority must not fabricate target byte observations"
    );
    scratch_provenance_no_publication(&tracker, &key);
    let warm = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    assert!(HostSelectedExtensionScratchPrefixKey::equality(
        &completed, &warm
    ));
}

const SCRATCH_PROVENANCE_DOWNLOAD_MODULE: &str = "module(name='consumer')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\nuse_repo(e, proof='proof')\n";
const SCRATCH_PROVENANCE_DOWNLOAD_BYTES: &[u8] = b"normal selected downloaded generator";
fn scratch_provenance_download_extension() -> String {
    let checksum = format!("{:x}", Sha256::digest(SCRATCH_PROVENANCE_DOWNLOAD_BYTES));
    format!(
        r#"
def impl(ctx):
    print('scratch-provenance-attempt')
    value=ctx.getenv('DYNAMIC','fallback')
    p=ctx.path('cargo-bazel')
    ctx.file(p,'discarded',executable=False)
    ctx.download(url='file:///scratch-provenance-generator',output=p,sha256='{checksum}',executable=True)
    ctx.download(url='file:///scratch-provenance-generator',output=p,sha256='{checksum}',executable=True)
    ctx.execute([p,'query',value])
ext=module_extension(implementation=impl,environ=['FACTOR','MISSING','FACTOR'])
"#
    )
}

#[tokio::test]
async fn selected_scratch_getenv_download_need_replay_retains_exact_verified_lineage() {
    use super::selected_scratch::HostSelectedExtensionScratchPrefixKey;
    let extension = scratch_provenance_download_extension();
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
    let dice = builder.build(DetectCycles::Enabled);
    let tracker = Arc::new(RepositoryRuleActivationTracker::default());
    let (base, owner) = selected_factor_transaction(
        &dice,
        SCRATCH_PROVENANCE_DOWNLOAD_MODULE,
        &extension,
        tracker.clone(),
        true,
    )
    .await;
    let resources = scratch_test_resources();
    let key = HostSelectedExtensionScratchPrefixKey::new(
        NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
        owner.clone(),
        slug_bzlmod_v2::SelectedScratchPolicy::plain(),
    );
    let mut tx = scratch_provenance_inputs(
        base,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        false,
        None,
    )
    .await;
    tracker.take();
    let environment_need = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    let SourcePreparationOutcome::Need(need) = &environment_need else {
        panic!("{environment_need:?}")
    };
    assert_eq!(
        need.repository_environment()
            .unwrap()
            .names()
            .iter()
            .map(CompactString::as_str)
            .collect::<Vec<_>>(),
        ["DYNAMIC"]
    );
    assert_eq!(resources.payload_budget().used_bytes(), 0);
    scratch_provenance_no_publication(&tracker, &key);
    tx = scratch_provenance_inputs(
        tx,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        true,
        None,
    )
    .await;
    let download_need = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    let SourcePreparationOutcome::Need(need) = &download_need else {
        panic!("{download_need:?}")
    };
    assert_eq!(need.selected_extension_downloads().len(), 1);
    let request = need
        .selected_extension_downloads()
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    assert_eq!(request.owner(), &owner);
    assert_eq!(request.workspace(), key.workspace());
    assert_eq!(request.url(), "file:///scratch-provenance-generator");
    assert_eq!(request.output().as_path(), Path::new("cargo-bazel"));
    assert!(request.executable());
    assert_eq!(
        resources.payload_budget().used_bytes(),
        0,
        "Need must discard overwritten provisional bytes"
    );
    scratch_provenance_no_publication(&tracker, &key);
    let verified = Arc::new(
        SelectedExtensionDownloadVerified::new(
            request.clone(),
            Arc::from(SCRATCH_PROVENANCE_DOWNLOAD_BYTES),
        )
        .unwrap(),
    );
    tx = scratch_provenance_inputs(
        tx,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        true,
        Some(verified.clone()),
    )
    .await;
    let outcome = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    let value = scratch_test_value(&outcome);
    assert_eq!(value.download_requests(), &[request]);
    let blob = value
        .prefix()
        .entries()
        .iter()
        .find(|entry| entry.path().relative() == "cargo-bazel")
        .unwrap()
        .blob()
        .unwrap();
    assert!(Arc::ptr_eq(blob.download().unwrap(), &verified));
    assert_eq!(blob.bytes().as_ptr(), verified.bytes().as_ptr());
    assert_eq!(
        value.prefix().process().arguments()[2].spelling(),
        "fallback"
    );
    assert_eq!(
        value.prefix().occurrence_bytes(),
        b"discarded".len() + 2 * SCRATCH_PROVENANCE_DOWNLOAD_BYTES.len()
    );
    assert_eq!(
        resources.payload_budget().used_bytes(),
        b"discarded".len() + SCRATCH_PROVENANCE_DOWNLOAD_BYTES.len()
    );
    scratch_provenance_no_publication(&tracker, &key);
}

#[tokio::test]
async fn selected_scratch_download_prefix_history_holds_payload_until_engine_drop() {
    use super::selected_scratch::HostSelectedExtensionScratchPrefixKey;
    let extension = scratch_provenance_download_extension();
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
    let dice = builder.build(DetectCycles::Enabled);
    let tracker = Arc::new(RepositoryRuleActivationTracker::default());
    let (base, owner) = selected_factor_transaction(
        &dice,
        SCRATCH_PROVENANCE_DOWNLOAD_MODULE,
        &extension,
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
    let mut tx = scratch_provenance_inputs(
        base,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        true,
        None,
    )
    .await;
    let cold = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    let SourcePreparationOutcome::Need(need) = cold else {
        panic!("expected normal download Need")
    };
    let verified = Arc::new(
        SelectedExtensionDownloadVerified::new(
            need.selected_extension_downloads()
                .iter()
                .next()
                .unwrap()
                .1
                .clone(),
            Arc::from(SCRATCH_PROVENANCE_DOWNLOAD_BYTES),
        )
        .unwrap(),
    );
    tx = scratch_provenance_inputs(
        tx,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        true,
        Some(verified.clone()),
    )
    .await;
    let a = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    scratch_test_value(&a);
    let old_transaction = tx.dupe();
    let per_prefix = b"discarded".len() + SCRATCH_PROVENANCE_DOWNLOAD_BYTES.len();
    assert_eq!(resources.payload_budget().used_bytes(), per_prefix);
    let changed =
        RepositoryEnvironmentSnapshot::from_canonical(vec![RepositoryEnvironmentEntry::new(
            "UNRELATED",
            "version-b",
        )])
        .unwrap();
    tx = scratch_provenance_inputs(
        tx,
        tracker.clone(),
        &resources,
        changed,
        true,
        Some(verified.clone()),
    )
    .await;
    let b = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    scratch_test_value(&b);
    assert!(!HostSelectedExtensionScratchPrefixKey::equality(&a, &b));
    assert_eq!(resources.payload_budget().used_bytes(), 2 * per_prefix);
    drop(a);
    drop(b);
    drop(old_transaction);
    drop(tx);
    dice.wait_for_idle().await;
    assert!(
        resources.payload_budget().used_bytes() >= per_prefix,
        "draining live requests must not pretend historical values retired"
    );
    drop(dice);
    // Cancellation drain is distinct from the dedicated actor dropping its
    // historical graph after the last engine sender closes.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while resources.payload_budget().used_bytes() != 0
            || resources.descriptor_budget().used_bytes() != 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual cached prefix allocations must retire after engine shutdown");
    assert_eq!(resources.payload_budget().used_bytes(), 0);
    assert_eq!(resources.descriptor_budget().used_bytes(), 0);
    assert_eq!(
        verified.bytes().as_ref(),
        SCRATCH_PROVENANCE_DOWNLOAD_BYTES,
        "baseline verified storage remains outside the process-payload credit"
    );
}

#[tokio::test]
async fn selected_scratch_label_budget_pressure_reopens_same_version_after_release() {
    use super::selected_scratch::HostSelectedExtensionScratchPrefixKey;
    let (dice, tx, key, resources, tracker) = scratch_provenance_label_case().await;
    let mut tx = selected_watch_observations(
        tx,
        selected_path_package_epoch(
            SCRATCH_PROVENANCE_LABEL_MODULE,
            SCRATCH_PROVENANCE_LABEL_EXTENSION,
            false,
            true,
        ),
    )
    .await;
    let budget = resources.descriptor_budget();
    let pressure = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    tracker.take();
    let failed = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    assert!(
        matches!(&failed, SourcePreparationOutcome::Complete(Err(e)) if e.is_transient()),
        "{failed:?}"
    );
    assert!(!HostSelectedExtensionScratchPrefixKey::validity(&failed));
    assert!(!HostSelectedExtensionScratchPrefixKey::equality(
        &failed, &failed
    ));
    assert_eq!(resources.payload_budget().used_bytes(), 0);
    scratch_provenance_no_publication(&tracker, &key);
    drop(pressure);
    let updater = tx.into_updater();
    dice.wait_for_idle().await;
    let mut tx = updater.commit().await;
    let repaired = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    assert_eq!(scratch_test_value(&repaired).label_sources().len(), 3);
    assert!(HostSelectedExtensionScratchPrefixKey::validity(&repaired));
    scratch_provenance_no_publication(&tracker, &key);
}

#[tokio::test]
async fn selected_scratch_real_frontier_merge_precharges_and_retires_at_last_drop() {
    let (_dice, tx, key, resources, _tracker) = scratch_provenance_label_case().await;
    let mut tx = selected_watch_observations(
        tx,
        selected_path_package_epoch(
            SCRATCH_PROVENANCE_LABEL_MODULE,
            SCRATCH_PROVENANCE_LABEL_EXTENSION,
            false,
            true,
        ),
    )
    .await;
    let outcome = crate::compute_selected_scratch_prefix(&mut tx, &key)
        .await
        .unwrap();
    let value = scratch_test_value(&outcome);
    let incoming = value.label_sources()[2].selection().observations();
    let budget = resources.descriptor_budget();
    let baseline = budget.used_bytes();
    let mut credit = budget.try_reserve(0).unwrap();
    let pressure = budget.try_reserve(budget.limit_bytes() - baseline).unwrap();
    let before = budget.used_bytes();
    let failed = super::selected_scratch::merge_checkpoint_frontier(
        &mut credit,
        value.observations(),
        incoming,
    );
    assert!(failed.unwrap_err().is_transient());
    assert_eq!(credit.charged_bytes(), 0);
    assert_eq!(budget.used_bytes(), before);
    drop(pressure);
    let merged = super::selected_scratch::merge_checkpoint_frontier(
        &mut credit,
        value.observations(),
        incoming,
    )
    .unwrap();
    assert!(credit.charged_bytes() > 0);
    for (demand, result) in incoming.observations() {
        assert!(
            merged
                .observations()
                .iter()
                .any(|(other, actual)| other == demand && actual == result)
        );
    }
    let block = credit.freeze(merged).unwrap();
    let clone = block.dupe();
    let retained = budget.used_bytes();
    assert!(retained > baseline);
    drop(block);
    assert_eq!(budget.used_bytes(), retained);
    drop(clone);
    assert_eq!(budget.used_bytes(), baseline);
}
