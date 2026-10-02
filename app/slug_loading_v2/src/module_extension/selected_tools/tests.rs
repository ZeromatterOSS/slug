// Loading consumes trusted raw facts; Core tests prove their real byte observer.
use slug_bzlmod_v2::SelectedScratchElfMetadataEpoch;
use slug_bzlmod_v2::SelectedScratchElfMetadataEpochKey;
use slug_bzlmod_v2::SelectedScratchElfMetadataNeeds;
use slug_bzlmod_v2::SelectedScratchElfMetadataResult;
use slug_bzlmod_v2::SelectedScratchElfMetadataTransaction;
use slug_bzlmod_v2::SelectedToolOperand;
use slug_bzlmod_v2::SelectedToolRole;
use slug_bzlmod_v2::SelectedToolSourcesPolicy;
use slug_workspace_v2::ElfHeaderFacts;
use slug_workspace_v2::ElfMetadataFacts;
use slug_workspace_v2::ElfMetadataOutcome;
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::VerifiedElfMetadata;

fn tool_test_policy() -> SelectedToolSourcesPolicy {
    SelectedToolSourcesPolicy::new(
        &[SelectedToolOperand {
            role: SelectedToolRole::Generator,
            argument_index: 0,
        }],
        ElfMetadataPolicy::default(),
    )
    .unwrap()
}
fn tool_test_source(typed: bool, executable: bool, extra: &str) -> String {
    // Sectionless ELF64, no interpreter or program headers. All bytes are ASCII.
    let mut bytes = [0u8; 64];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16] = 2;
    bytes[18] = 62;
    bytes[20] = 1;
    bytes[32] = 64;
    bytes[52] = 64;
    bytes[54] = 56;
    let literal = bytes
        .iter()
        .map(|b| format!("\\{b:03o}"))
        .collect::<String>();
    let argument = if typed { "p" } else { "str(p)" };
    format!(
        "from_cargo=tag_class(attrs={{'value':attr.string()}})\ndef impl(ctx):\n    p=ctx.path('generator')\n    ctx.file(p,'earlier',executable=True)\n    ctx.file(p,'{literal}',executable={})\n{extra}    ctx.execute([{argument},'query'])\n    fail('must not execute after checkpoint')\next=module_extension(implementation=impl, tag_classes={{'from_cargo':from_cargo}}, environ=['FACTOR','MISSING','FACTOR'])\n",
        if executable { "True" } else { "False" }
    )
}
async fn tool_test_fixture(
    source: &str,
) -> (
    Arc<Dice>,
    dice::DiceTransaction,
    super::selected_tools::SelectedExtensionToolSourcesKey,
    slug_bzlmod_v2::SelectedScratchResources,
    Arc<RepositoryRuleActivationTracker>,
) {
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
    let dice = builder.build(DetectCycles::Enabled);
    let tracker = Arc::new(RepositoryRuleActivationTracker::default());
    let (base, owner) =
        selected_factor_transaction(&dice, SCRATCH_TEST_MODULE, source, tracker.clone(), true)
            .await;
    let resources = scratch_test_resources();
    let tx = scratch_test_inputs(
        base,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        None,
    )
    .await;
    let key = super::selected_tools::SelectedExtensionToolSourcesKey::new(
        super::selected_scratch::HostSelectedExtensionScratchPrefixKey::new(
            NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
            owner,
            slug_bzlmod_v2::SelectedScratchPolicy::plain(),
        ),
        tool_test_policy(),
    );
    (dice, tx, key, resources, tracker)
}
async fn tool_test_install(
    tx: dice::DiceTransaction,
    tracker: Arc<RepositoryRuleActivationTracker>,
    resources: &slug_bzlmod_v2::SelectedScratchResources,
    authorization: SelectedScratchElfMetadataTransaction,
    raw: Option<(
        SelectedScratchElfMetadataEpochKey,
        SelectedScratchElfMetadataEpoch,
    )>,
) -> dice::DiceTransaction {
    let mut data = dice::UserComputationData {
        cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
        activation_tracker: Some(tracker),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    data.data.set(resources.descriptor_budget().clone());
    data.data.set(resources.clone());
    data.data.set(RepositoryHostInputTransaction::new(
        RepositoryEnvironmentSnapshot::empty(),
        RepositoryEnvironmentNameFrontier::from_unsorted([
            CompactString::new("FACTOR"),
            CompactString::new("MISSING"),
        ]),
    ));
    data.data.set(SelectedExtensionDownloadTransaction::empty(
        NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
    ));
    data.data.set(authorization);
    let mut updater = tx.into_updater();
    if let Some(raw) = raw {
        updater.changed_to([raw]).unwrap();
    }
    updater.commit_with_data(data).await
}
fn tool_test_fact(
    request: &slug_bzlmod_v2::SelectedScratchElfMetadataRequest,
    r: &slug_bzlmod_v2::SelectedScratchResources,
    rejected: bool,
) -> SelectedScratchElfMetadataResult {
    let outcome = if rejected {
        ElfMetadataOutcome::Rejected(slug_workspace_v2::ElfMetadataRejection::Malformed(
            slug_workspace_v2::ElfMalformedReason::Header,
        ))
    } else {
        ElfMetadataOutcome::Parsed(ElfMetadataFacts {
            header: ElfHeaderFacts {
                object_type: 2,
                machine: 62,
                os_abi: 0,
                abi_version: 0,
                flags: 0,
                entry: 0,
            },
            program_headers: Vec::new(),
            interpreter: None,
            dynamic_entries: Vec::new(),
            string_refs: Vec::new(),
        })
    };
    SelectedScratchElfMetadataResult::new(
        request,
        VerifiedElfMetadata::new(
            request.expected_digest(),
            request.policy(),
            outcome,
            r.descriptor_budget().try_reserve(0).unwrap(),
        )
        .unwrap(),
        r,
    )
    .unwrap()
}

#[tokio::test]
async fn selected_tools_normal_requests_need_ready_and_warm_full_authorization() {
    let (_dice, mut tx, key, resources, tracker) =
        tool_test_fixture(&tool_test_source(true, true, "")).await;
    let plan = match crate::compute_selected_tool_source_requests(&mut tx, key.requests_key())
        .await
        .unwrap()
    {
        SourcePreparationOutcome::Complete(Ok(v)) => v,
        other => panic!("normal plan: {other:?}"),
    };
    let request = plan
        .requests()
        .next()
        .unwrap()
        .scratch_request()
        .unwrap()
        .clone();
    assert_eq!(
        request.member().guest_spelling(),
        "/__slug_module/generator"
    );
    assert_eq!(request.blob().bytes().len(), 64);
    let allowed = SelectedScratchElfMetadataNeeds::singleton(&request, &resources).unwrap();
    let empty = SelectedScratchElfMetadataTransaction::new(
        plan.selector(),
        &resources,
        Some(&allowed),
        &[],
    )
    .unwrap();
    let cell = SelectedScratchElfMetadataEpochKey::new(&request);
    tx = tool_test_install(
        tx,
        tracker.clone(),
        &resources,
        empty,
        Some((
            cell.clone(),
            SelectedScratchElfMetadataEpoch::AuthorizedEmpty,
        )),
    )
    .await;
    let need = crate::compute_selected_tool_sources(&mut tx, &key)
        .await
        .unwrap();
    let SourcePreparationOutcome::Need(need) = need else {
        panic!("missing proof must Need")
    };
    assert_eq!(
        need.scratch_elf_metadata_requests()
            .unwrap()
            .requests()
            .next()
            .unwrap(),
        &request
    );
    // Raw absence is valid supplied input so restoration can inject it. The
    // normal certificate error remains transient and cannot become reusable.
    let authorization = SelectedScratchElfMetadataTransaction::new(
        plan.selector(),
        &resources,
        Some(&allowed),
        &[],
    )
    .unwrap();
    tx = tool_test_install(
        tx,
        tracker.clone(),
        &resources,
        authorization,
        Some((cell.clone(), SelectedScratchElfMetadataEpoch::Unauthorized)),
    )
    .await;
    let revoked = crate::compute_selected_tool_sources(&mut tx, &key)
        .await
        .unwrap();
    assert!(
        matches!(revoked, SourcePreparationOutcome::Complete(Err(ref error)) if error.is_transient())
    );
    assert!(!<super::selected_tools::SelectedExtensionToolSourcesKey as Key>::validity(&revoked));
    let result = tool_test_fact(&request, &resources, false);
    let ready = SelectedScratchElfMetadataTransaction::new(
        plan.selector(),
        &resources,
        Some(&allowed),
        &[result.clone()],
    )
    .unwrap();
    tx = tool_test_install(
        tx,
        tracker.clone(),
        &resources,
        ready,
        Some((cell, SelectedScratchElfMetadataEpoch::Ready(result.clone()))),
    )
    .await;
    let completed = crate::compute_selected_tool_sources(&mut tx, &key)
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(value)) = &completed else {
        panic!("trusted fact must complete: {completed:?}")
    };
    assert_eq!(value.prefix(), plan.prefix());
    assert_eq!(
        value.sources().next().unwrap().scratch_result(),
        Some(&result)
    );
    // SAME raw epoch and semantic version; userdata-only removal of exact allowed
    // requests must not let a warm completed DICE value escape the helper.
    let unauthorized =
        SelectedScratchElfMetadataTransaction::new(plan.selector(), &resources, None, &[]).unwrap();
    tx = tool_test_install(tx, tracker, &resources, unauthorized, None).await;
    let error = crate::compute_selected_tool_sources(&mut tx, &key)
        .await
        .unwrap_err();
    assert!(error.is_transient());
}

#[tokio::test]
async fn selected_tools_literal_nonexec_and_absent_index_are_stable_refusals() {
    for (typed, executable) in [(false, true), (true, false)] {
        let (_dice, mut tx, key, _, _) =
            tool_test_fixture(&tool_test_source(typed, executable, "")).await;
        let outcome = crate::compute_selected_tool_source_requests(&mut tx, key.requests_key())
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Err(error)) = &outcome else {
            panic!("must refuse")
        };
        assert!(!error.is_transient());
        assert!(
            <super::selected_tools::SelectedExtensionToolSourceRequestsKey as Key>::validity(
                &outcome
            )
        );
    }
    let (_dice, mut tx, key, _, _) = tool_test_fixture(&tool_test_source(true, true, "")).await;
    let policy = SelectedToolSourcesPolicy::new(
        &[
            SelectedToolOperand {
                role: SelectedToolRole::Generator,
                argument_index: 0,
            },
            SelectedToolOperand {
                role: SelectedToolRole::Cargo,
                argument_index: 9,
            },
        ],
        ElfMetadataPolicy::default(),
    )
    .unwrap();
    let wrong = super::selected_tools::SelectedExtensionToolSourceRequestsKey::new(
        key.checkpoint().clone(),
        policy,
    );
    let outcome = crate::compute_selected_tool_source_requests(&mut tx, &wrong)
        .await
        .unwrap();
    assert!(matches!(outcome, SourcePreparationOutcome::Complete(Err(ref e)) if !e.is_transient()));
}

#[tokio::test]
async fn selected_tools_foreign_service_and_missing_authorization_are_transient() {
    let (_dice, mut tx, key, resources, tracker) =
        tool_test_fixture(&tool_test_source(true, true, "")).await;
    let plan = crate::compute_selected_tool_source_requests(&mut tx, key.requests_key())
        .await
        .unwrap();
    assert!(matches!(plan, SourcePreparationOutcome::Complete(Ok(_))));
    let absent = crate::compute_selected_tool_sources(&mut tx, &key)
        .await
        .unwrap();
    assert!(matches!(absent, SourcePreparationOutcome::Complete(Err(ref e)) if e.is_transient()));
    assert!(!<super::selected_tools::SelectedExtensionToolSourcesKey as Key>::validity(&absent));
    let foreign = scratch_test_resources();
    tx = scratch_test_inputs(
        tx,
        tracker,
        &foreign,
        RepositoryEnvironmentSnapshot::empty(),
        None,
    )
    .await;
    assert!(
        crate::compute_selected_tool_source_requests(&mut tx, key.requests_key())
            .await
            .unwrap_err()
            .is_transient()
    );
    assert!(resources.payload_budget().used_bytes() > 0);
}

#[tokio::test]
async fn selected_tools_full_prefix_change_same_digest_requests_current_fact() {
    let source = tool_test_source(true, true, "");
    let (_dice, mut tx, key, resources, tracker) = tool_test_fixture(&source).await;
    let a = match crate::compute_selected_tool_source_requests(&mut tx, key.requests_key())
        .await
        .unwrap()
    {
        SourcePreparationOutcome::Complete(Ok(v)) => v,
        _ => panic!(),
    };
    let old = a
        .requests()
        .next()
        .unwrap()
        .scratch_request()
        .unwrap()
        .clone();
    let old_result = tool_test_fact(&old, &resources, false);
    let changed = tool_test_source(
        true,
        true,
        "    ctx.file('config', 'new', executable=False)\n",
    );
    let epoch = selected_factor_epoch(SCRATCH_TEST_MODULE, &changed);
    let mut updater = tx.into_updater();
    updater.changed_to(path_observation_shards(&epoch)).unwrap();
    updater
        .changed_to([(PathObservationEpochKey, epoch)])
        .unwrap();
    tx = updater.commit().await;
    tx = scratch_test_inputs(
        tx,
        tracker.clone(),
        &resources,
        RepositoryEnvironmentSnapshot::empty(),
        None,
    )
    .await;
    let b = match crate::compute_selected_tool_source_requests(&mut tx, key.requests_key())
        .await
        .unwrap()
    {
        SourcePreparationOutcome::Complete(Ok(v)) => v,
        other => panic!("{other:?}"),
    };
    let current = b
        .requests()
        .next()
        .unwrap()
        .scratch_request()
        .unwrap()
        .clone();
    assert_eq!(old.expected_digest(), current.expected_digest());
    assert_ne!(old, current);
    let allowed = SelectedScratchElfMetadataNeeds::singleton(&current, &resources).unwrap();
    let authorized =
        SelectedScratchElfMetadataTransaction::new(b.selector(), &resources, Some(&allowed), &[])
            .unwrap();
    tx = tool_test_install(
        tx,
        tracker,
        &resources,
        authorized,
        Some((
            SelectedScratchElfMetadataEpochKey::new(&current),
            SelectedScratchElfMetadataEpoch::Ready(old_result),
        )),
    )
    .await;
    let need = crate::compute_selected_tool_sources(&mut tx, &key)
        .await
        .unwrap();
    let SourcePreparationOutcome::Need(need) = need else {
        panic!("old Ready cannot complete")
    };
    assert_eq!(
        need.scratch_elf_metadata_requests()
            .unwrap()
            .requests()
            .next()
            .unwrap(),
        &current
    );
}

#[tokio::test]
async fn selected_tools_authenticated_rejected_elf_never_promotes() {
    let (_dice, mut tx, key, resources, tracker) =
        tool_test_fixture(&tool_test_source(true, true, "")).await;
    let plan = match crate::compute_selected_tool_source_requests(&mut tx, key.requests_key())
        .await
        .unwrap()
    {
        SourcePreparationOutcome::Complete(Ok(v)) => v,
        _ => panic!(),
    };
    let request = plan
        .requests()
        .next()
        .unwrap()
        .scratch_request()
        .unwrap()
        .clone();
    let result = tool_test_fact(&request, &resources, true);
    let allowed = SelectedScratchElfMetadataNeeds::singleton(&request, &resources).unwrap();
    let auth = SelectedScratchElfMetadataTransaction::new(
        plan.selector(),
        &resources,
        Some(&allowed),
        &[result.clone()],
    )
    .unwrap();
    tx = tool_test_install(
        tx,
        tracker,
        &resources,
        auth,
        Some((
            SelectedScratchElfMetadataEpochKey::new(&request),
            SelectedScratchElfMetadataEpoch::Ready(result),
        )),
    )
    .await;
    let outcome = crate::compute_selected_tool_sources(&mut tx, &key)
        .await
        .unwrap();
    assert!(matches!(outcome, SourcePreparationOutcome::Complete(Err(ref e)) if !e.is_transient()));
    assert!(<super::selected_tools::SelectedExtensionToolSourcesKey as Key>::validity(&outcome));
}
