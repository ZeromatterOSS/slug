// Real checkpoint/Label producers; only raw source observations are injected.
use slug_bzlmod_v2::GuestLabelDataPolicy as DataTestPolicy;
use slug_bzlmod_v2::RepositoryMaterialization as DataTestMaterialization;

use super::selected_label_data as data_test_api;

struct DataTestFixture {
    dice: Arc<Dice>,
    tx: Option<dice::DiceTransaction>,
    key: data_test_api::SelectedDataLabelSourcesKey,
    r: runtime_test_bzl::SelectedScratchResources,
    tracker: Arc<RepositoryRuleActivationTracker>,
    runtime: runtime_test_bzl::RuntimeSourceDeclarationPolicy,
    requests: runtime_test_api::ObservedRuntimeSourceRequests,
    epoch: PathObservationEpoch,
    guest: GuestTestPolicy,
    known_paths: Vec<PathObservationDemand>,
}
impl DataTestFixture {
    async fn new(derive: &str, relative: &str, guest_prefix: &str) -> Self {
        Self::label_case("//pkg:data", derive, relative, guest_prefix, false).await
    }
    async fn label_case(
        label: &str,
        derive: &str,
        relative: &str,
        guest_prefix: &str,
        label_generator: bool,
    ) -> Self {
        let module = SCRATCH_TEST_MODULE.replace(
            "e.from_cargo(value='final')",
            &format!("e.from_cargo(value='final')\ne.path(file='{label}')"),
        );
        let extra = format!(
            "    data=ctx.path(ctx.modules[0].tags.path[0].file)\n{derive}    ctx.file('config',str(data),executable=False)\n"
        );
        let source = tool_test_source(true, true, &extra)
            .replace(
                "from_cargo=tag_class",
                "path_tag=tag_class(attrs={'file':attr.label()})\nfrom_cargo=tag_class",
            )
            .replace(
                "'from_cargo':from_cargo",
                "'from_cargo':from_cargo,'path':path_tag",
            );
        let source = if label_generator {
            source.replace("ctx.execute([p,'query'])", "ctx.execute([data,'query'])")
        } else {
            source
        };
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = builder.build(DetectCycles::Enabled);
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (tx, owner) =
            selected_factor_transaction(&dice, &module, &source, tracker.clone(), true).await;
        let r = scratch_test_resources();
        // Runtime fixture installation supplies current userdata, but deliberately
        // preserves existing platform/environment raw cells. Establish the same
        // genuine Host input baseline as tool_test_fixture before any checkpoint.
        let tx = scratch_test_inputs(
            tx,
            tracker.clone(),
            &r,
            RepositoryEnvironmentSnapshot::empty(),
            None,
        )
        .await;
        let tools = super::selected_tools::SelectedExtensionToolSourceRequestsKey::new(
            super::selected_scratch::HostSelectedExtensionScratchPrefixKey::new(
                NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
                owner,
                runtime_test_bzl::SelectedScratchPolicy::plain(),
            ),
            tool_test_policy(),
        );
        let key = data_test_api::SelectedDataLabelSourcesKey::new(
            runtime_test_api::RuntimeSourceProfileKey::new(tools),
        );
        let runtime = runtime_test_policy(
            &r,
            &[runtime_test_relative(relative, guest_prefix)],
            Default::default(),
        );
        let guest = guest_test_policy()
            .with_label_data(DataTestPolicy::IssuedFileLeaves { max_witnesses: 64 })
            .unwrap();
        let tx = runtime_test_install_with_guest(
            tx,
            tracker.clone(),
            &r,
            Some(runtime.dupe()),
            Some(runtime_test_authorization(&runtime, &r)),
            None,
            None,
            Some((Some(guest), Some(guest_test_tx(guest, &r)))),
        )
        .await;
        let mut known_paths = Vec::new();
        let mut tx = data_test_observations(
            tx,
            &selected_path_package_epoch(&module, &source, false, true),
            &mut known_paths,
        )
        .await;
        let requests = runtime_test_requests(
            crate::compute_runtime_source_requests(&mut tx, key.profile_key().requests_key())
                .await
                .unwrap(),
        );
        let baseline = selected_path_package_epoch(&module, &source, false, true);
        let epoch = runtime_test_union(
            &baseline,
            &runtime_test_image_epoch(
                requests.requests().next().unwrap().image_key(),
                &r,
                1,
                Some((1, 0o644)),
            ),
        );
        let tx = runtime_test_install_with_guest(
            tx,
            tracker.clone(),
            &r,
            Some(runtime.dupe()),
            Some(runtime_test_authorization(&runtime, &r)),
            Some(data_test_ready_tools(&requests, &r)),
            None,
            Some((Some(guest), Some(guest_test_tx(guest, &r)))),
        )
        .await;
        let tx = data_test_observations(tx, &epoch, &mut known_paths).await;
        Self {
            dice,
            tx: Some(tx),
            key,
            r,
            tracker,
            runtime,
            requests,
            epoch,
            guest,
            known_paths,
        }
    }
    async fn install(&mut self, guest: GuestTestPolicy, current: Option<GuestTestTransaction>) {
        self.guest = guest;
        self.tx = Some(
            runtime_test_install_with_guest(
                self.tx.take().unwrap(),
                self.tracker.clone(),
                &self.r,
                Some(self.runtime.dupe()),
                Some(runtime_test_authorization(&self.runtime, &self.r)),
                Some(data_test_ready_tools(&self.requests, &self.r)),
                None,
                Some((Some(guest), current)),
            )
            .await,
        );
        self.tx = Some(
            data_test_observations(self.tx.take().unwrap(), &self.epoch, &mut self.known_paths)
                .await,
        );
    }
    async fn plan(&mut self) -> data_test_api::ObservedSelectedDataLabelRequests {
        match crate::compute_selected_data_label_requests(
            self.tx.as_mut().unwrap(),
            &self.key.requests_key(),
        )
        .await
        .unwrap()
        {
            SourcePreparationOutcome::Complete(Ok(v)) => v,
            v => panic!("{v:?}"),
        }
    }
    async fn sources(&mut self) -> <data_test_api::SelectedDataLabelSourcesKey as Key>::Value {
        crate::compute_selected_data_label_sources(self.tx.as_mut().unwrap(), &self.key)
            .await
            .unwrap()
    }
    async fn ready(&mut self) -> data_test_api::ObservedSelectedDataLabelSources {
        match self.sources().await {
            SourcePreparationOutcome::Complete(Ok(v)) => v,
            v => panic!("{v:?}"),
        }
    }
    async fn facts(&mut self) -> guest_test_api::ObservedGuestNamespaceFacts {
        let key =
            guest_test_api::SelectedGuestNamespaceFactsKey::new(self.key.profile_key().clone());
        guest_test_facts(
            crate::compute_selected_guest_namespace_facts(self.tx.as_mut().unwrap(), &key)
                .await
                .unwrap(),
        )
    }
}
// New fixture updates use the landed per-demand transport, not cumulative raw
// epochs. Keep the union of issued cells to revoke removed materialization scopes.
async fn data_test_observations(
    tx: dice::DiceTransaction,
    epoch: &PathObservationEpoch,
    known: &mut Vec<PathObservationDemand>,
) -> dice::DiceTransaction {
    for (demand, _) in epoch.observations() {
        if !known.contains(demand) {
            known.push(demand.dupe());
        }
    }
    let mut updater = tx.into_updater();
    slug_workspace_v2::inject_path_observation_inputs(&mut updater, epoch, known.clone()).unwrap();
    updater.commit().await
}
fn data_test_ready_tools(
    requests: &runtime_test_api::ObservedRuntimeSourceRequests,
    r: &runtime_test_bzl::SelectedScratchResources,
) -> (
    SelectedScratchElfMetadataTransaction,
    Option<(
        SelectedScratchElfMetadataEpochKey,
        SelectedScratchElfMetadataEpoch,
    )>,
) {
    if requests
        .tool_requests()
        .requests()
        .any(|r| r.scratch_request().is_some())
    {
        runtime_test_ready_scratch(requests, r)
    } else {
        (
            SelectedScratchElfMetadataTransaction::new(
                requests.tool_requests().selector(),
                r,
                None,
                &[],
            )
            .unwrap(),
            None,
        )
    }
}
fn data_test_leaf_epoch(
    key: &runtime_test_ws::ObservedInputImageKey,
    r: &runtime_test_bzl::SelectedScratchResources,
    kind: PathNodeKind,
    digest: u8,
    freshness: i64,
) -> PathObservationEpoch {
    use runtime_test_ws::ImageObservationData as D;
    use runtime_test_ws::ImageObservationOperation as O;
    let anchor = runtime_test_ws::ImageChargedPath::pin_existing(
        key.anchor_root().dupe(),
        r.descriptor_budget(),
    )
    .unwrap();
    let mut cursor = anchor.dupe();
    let mut rows = Vec::new();
    let mut add = |path: runtime_test_ws::ImageChargedPath, op, data| {
        let request = runtime_test_ws::ImageObservationRequest::new(anchor.dupe(), op);
        let demand =
            PathObservationDemand::input_image(key.namespace(), path.dupe(), request.dupe())
                .unwrap();
        let result = runtime_test_ws::ImageObservationResult::new(
            path,
            request,
            data,
            r.descriptor_budget().try_reserve(512).unwrap(),
        )
        .unwrap();
        rows.push((demand, PathObservationResult::InputImage(result)));
    };
    for component in key
        .source_root()
        .as_path()
        .strip_prefix(anchor.as_path())
        .unwrap()
        .components()
    {
        add(
            cursor.dupe(),
            O::Lstat,
            D::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::Directory,
                0,
                1,
                1,
                1,
                0o755,
            ))),
        );
        cursor = cursor
            .try_join_component(
                &runtime_test_ws::PathDirectoryName::new(component.as_os_str()).unwrap(),
            )
            .unwrap();
    }
    add(
        cursor.dupe(),
        O::Lstat,
        D::Lstat(PathOperationResult::Present(PathLstat::new(
            kind,
            if kind == PathNodeKind::Directory {
                0
            } else {
                1
            },
            freshness,
            freshness,
            1,
            if kind == PathNodeKind::Directory {
                0o755
            } else {
                0o644
            },
        ))),
    );
    if kind == PathNodeKind::RegularFile {
        add(
            cursor,
            O::BoundedDigest {
                max_bytes: key.policy().limits().max_file_bytes,
            },
            D::Digest(PathOperationResult::Present(
                runtime_test_ws::FileContentDigest::new([digest; 32], 1).unwrap(),
            )),
        );
    }
    PathObservationEpoch::new(rows).unwrap()
}

#[tokio::test]
async fn issued_data_labels_normal_ordered_plan_need_leaf_and_sparse_view() {
    let mut f = DataTestFixture::new("", "runtime", "/rt").await;
    let plan = f.plan().await;
    assert_eq!(plan.requests().len(), 1);
    let request = plan.requests().next().unwrap();
    assert_eq!(request.occurrence_index(), 0);
    assert!(request.leaf_image_key().is_regular_file_root());
    let prefix = plan.runtime_requests().tool_requests().prefix();
    assert_eq!(prefix.label_sources().len(), 1);
    assert!(prefix.prefix().process().arguments().iter().all(|a| {
        a.spelling()
            != request
                .selection()
                .selected_file()
                .as_path()
                .to_str()
                .unwrap()
    }));
    let need = f.sources().await;
    assert!(!data_test_api::SelectedDataLabelSourcesKey::validity(&need));
    assert!(
        matches!(need,SourcePreparationOutcome::Need(ref n) if n.path_observations().is_some())
    );
    let path = request
        .selection()
        .selected_file()
        .as_path()
        .to_str()
        .unwrap()
        .to_owned();
    f.epoch = runtime_test_union(
        &f.epoch,
        &data_test_leaf_epoch(
            request.leaf_image_key(),
            &f.r,
            PathNodeKind::RegularFile,
            7,
            1,
        ),
    );
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    let ready = f.ready().await;
    assert!(matches!(
        ready.sources().next().unwrap().binding(),
        data_test_api::SelectedDataLabelBinding::Leaf { .. }
    ));
    let facts = f.facts().await;
    assert_eq!(facts.data_sources(), Some(&ready));
    use guest_test_api::GuestLookupError as E;
    use guest_test_api::GuestNamespaceOperation as Op;
    assert!(matches!(
        facts.lookup(&path, Op::ReadRegular).unwrap().binding(),
        guest_test_api::GuestNamespaceBinding::DataLeaf { occurrence: 0 }
    ));
    let parent = path.rsplit_once('/').unwrap().0;
    assert_eq!(
        facts
            .lookup(REPO_RULE_WORKSPACE, Op::ListDirectory)
            .unwrap_err(),
        E::Unowned,
        "data ancestor shared with runtime image is sparse"
    );
    assert_eq!(
        facts.lookup(parent, Op::ListDirectory).unwrap_err(),
        E::Unowned
    );
    assert_eq!(
        facts
            .lookup(&format!("{parent}/unrelated"), Op::Stat)
            .unwrap_err(),
        E::Unowned
    );
    assert_eq!(facts.lookup(&path, Op::Access(2)).unwrap_err(), E::ReadOnly);
    scratch_provenance_no_publication(
        &f.tracker,
        f.key
            .profile_key()
            .requests_key()
            .tool_requests_key()
            .checkpoint(),
    );
}

#[tokio::test]
async fn issued_data_labels_all_derived_occurrences_limit_and_uncovered_directory_refusal() {
    let mut f = DataTestFixture::new(
        "    parent=data.dirname\n    again=parent.get_child('data')\n",
        "runtime",
        "/rt",
    )
    .await;
    let plan = f.plan().await;
    assert_eq!(plan.requests().len(), 3);
    let events: Vec<_> = plan.requests().map(|r| r.event_index()).collect();
    assert!(events.windows(2).all(|w| w[0] < w[1]));
    let mut epoch = f.epoch.dupe();
    for request in plan.requests() {
        epoch = runtime_test_union(
            &epoch,
            &data_test_leaf_epoch(
                request.leaf_image_key(),
                &f.r,
                if request.occurrence_index() == 1 {
                    PathNodeKind::Directory
                } else {
                    PathNodeKind::RegularFile
                },
                7,
                1,
            ),
        );
    }
    f.epoch = epoch;
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    let value = f.sources().await;
    assert!(
        matches!(
            value,
            SourcePreparationOutcome::Complete(Err(data_test_api::SelectedDataLabelError::Image(
                runtime_test_ws::InputImageError::WrongKind
            )))
        ),
        "{value:?}"
    );
    let limited = f
        .guest
        .with_label_data(DataTestPolicy::IssuedFileLeaves { max_witnesses: 2 })
        .unwrap();
    f.install(limited, Some(guest_test_tx(limited, &f.r))).await;
    let value =
        crate::compute_selected_data_label_requests(f.tx.as_mut().unwrap(), &f.key.requests_key())
            .await
            .unwrap();
    assert!(matches!(
        value,
        SourcePreparationOutcome::Complete(Err(
            data_test_api::SelectedDataLabelError::WitnessLimit
        ))
    ));
    let disabled = f.guest.with_label_data(DataTestPolicy::Disabled).unwrap();
    f.install(disabled, Some(guest_test_tx(disabled, &f.r)))
        .await;
    let held = f
        .r
        .descriptor_budget()
        .try_reserve(f.r.descriptor_budget().limit_bytes() - f.r.descriptor_budget().used_bytes())
        .unwrap();
    let value =
        crate::compute_selected_data_label_requests(f.tx.as_mut().unwrap(), &f.key.requests_key())
            .await
            .unwrap();
    assert!(matches!(
        value,
        SourcePreparationOutcome::Complete(Err(data_test_api::SelectedDataLabelError::Disabled))
    ));
    drop(held);
    let restored = f
        .guest
        .with_label_data(DataTestPolicy::IssuedFileLeaves { max_witnesses: 64 })
        .unwrap();
    f.install(restored, Some(guest_test_tx(restored, &f.r)))
        .await;
    assert_eq!(
        f.plan().await,
        plan,
        "ordered derivation plan survives keyed-policy A/B/A"
    );
}

#[tokio::test]
async fn issued_data_labels_reuse_runtime_regular_and_original_directory_without_leaf_needs() {
    let mut f = DataTestFixture::new("", "pkg", "/rt").await;
    let ready = f.ready().await;
    let source = ready.sources().next().unwrap();
    assert!(matches!(
        source.binding(),
        data_test_api::SelectedDataLabelBinding::RuntimeMember {
            declaration: 0,
            entry: 1
        }
    ));
    assert!(source.leaf_image().is_none());
    let path = ready
        .requests()
        .requests()
        .next()
        .unwrap()
        .selection()
        .selected_file()
        .as_path()
        .to_str()
        .unwrap();
    let facts = f.facts().await;
    assert!(matches!(
        facts
            .lookup(path, guest_test_api::GuestNamespaceOperation::ReadRegular)
            .unwrap()
            .binding(),
        guest_test_api::GuestNamespaceBinding::RuntimeImage {
            declaration: 0,
            entry: 1
        }
    ));
    let original = format!("{REPO_RULE_WORKSPACE}/pkg");
    let mut covered = DataTestFixture::new(
        "    parent=data.dirname\n    again=parent.get_child('data')\n",
        "pkg",
        &original,
    )
    .await;
    let ready = covered.ready().await;
    assert_eq!(ready.sources().len(), 3);
    assert!(ready.sources().all(|s| s.leaf_image().is_none()));
    assert!(matches!(
        ready.sources().nth(1).unwrap().binding(),
        data_test_api::SelectedDataLabelBinding::RuntimeMember { entry: 0, .. }
    ));
    let facts = covered.facts().await;
    assert!(
        facts
            .lookup(
                &original,
                guest_test_api::GuestNamespaceOperation::ListDirectory
            )
            .is_ok()
    );
    assert_eq!(
        facts
            .lookup(
                &format!("{original}/absent"),
                guest_test_api::GuestNamespaceOperation::Stat
            )
            .unwrap_err(),
        guest_test_api::GuestLookupError::Missing
    );
}

#[tokio::test]
async fn issued_data_labels_full_leaf_frontier_a_b_a_and_warm_authorization() {
    let mut f = DataTestFixture::new("", "runtime", "/rt").await;
    let plan = f.plan().await;
    let request = plan.requests().next().unwrap();
    let base = f.epoch.dupe();
    f.epoch = runtime_test_union(
        &base,
        &data_test_leaf_epoch(
            request.leaf_image_key(),
            &f.r,
            PathNodeKind::RegularFile,
            7,
            1,
        ),
    );
    let original = f.epoch.dupe();
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    let a = f.ready().await;
    f.epoch = runtime_test_union(
        &base,
        &data_test_leaf_epoch(
            request.leaf_image_key(),
            &f.r,
            PathNodeKind::RegularFile,
            7,
            2,
        ),
    );
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    let fresh = f.ready().await;
    assert_ne!(a, fresh);
    assert_eq!(
        a.sources().next().unwrap().leaf_image().unwrap().content(),
        fresh
            .sources()
            .next()
            .unwrap()
            .leaf_image()
            .unwrap()
            .content()
    );
    f.epoch = runtime_test_union(
        &base,
        &data_test_leaf_epoch(
            request.leaf_image_key(),
            &f.r,
            PathNodeKind::RegularFile,
            8,
            1,
        ),
    );
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    let b = f.ready().await;
    assert_ne!(a, b);
    f.epoch = original;
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    assert_eq!(f.ready().await, a);
    f.install(f.guest, None).await;
    assert_eq!(
        f.tx.as_mut()
            .unwrap()
            .compute(&f.key)
            .await
            .unwrap()
            .complete_eq(&SourcePreparationOutcome::Complete(Ok(a.dupe()))),
        true,
        "cached semantic value remains available"
    );
    assert!(
        crate::compute_selected_data_label_sources(f.tx.as_mut().unwrap(), &f.key)
            .await
            .unwrap_err()
            .is_transient()
    );
    let foreign = scratch_test_resources();
    let wrong = GuestTestTransaction::new(
        NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
        Some(f.guest),
        foreign.descriptor_budget().dupe(),
    )
    .unwrap();
    f.install(f.guest, Some(wrong)).await;
    assert!(
        crate::compute_selected_data_label_sources(f.tx.as_mut().unwrap(), &f.key)
            .await
            .unwrap_err()
            .is_transient()
    );
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    assert_eq!(f.ready().await, a);
}

#[tokio::test]
async fn issued_data_labels_witness_occurrence_caps_and_history_pressure_repair() {
    let mut f = DataTestFixture::new(
        "    duplicate=ctx.path(ctx.modules[0].tags.path[0].file)\n",
        "runtime",
        "/rt",
    )
    .await;
    let plan = f.plan().await;
    assert_eq!(plan.requests().len(), 2);
    let request = plan.requests().next().unwrap();
    f.epoch = runtime_test_union(
        &f.epoch,
        &data_test_leaf_epoch(
            request.leaf_image_key(),
            &f.r,
            PathNodeKind::RegularFile,
            7,
            1,
        ),
    );
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    let facts = f.facts().await;
    let path = request
        .selection()
        .selected_file()
        .as_path()
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        facts
            .nodes()
            .iter()
            .filter(|n| n.path() == path.as_str())
            .count(),
        1
    );
    assert_eq!(
        facts.data_sources().unwrap().sources().len(),
        2,
        "dedup doesn't erase ordered provenance"
    );
    let mut single = DataTestFixture::new("", "runtime", "/rt").await;
    let single_plan = single.plan().await;
    let r = single_plan.requests().next().unwrap();
    single.epoch = runtime_test_union(
        &single.epoch,
        &data_test_leaf_epoch(
            r.leaf_image_key(),
            &single.r,
            PathNodeKind::RegularFile,
            7,
            1,
        ),
    );
    single
        .install(single.guest, Some(guest_test_tx(single.guest, &single.r)))
        .await;
    // Count actual config bytes as well as generator/runtime; each witness adds one byte.
    let baseline_bytes = plan
        .runtime_requests()
        .tool_requests()
        .prefix()
        .prefix()
        .entries()
        .iter()
        .filter_map(|e| e.blob())
        .map(|b| b.bytes().len() as u64)
        .sum::<u64>()
        + 1;
    let mut p = f.guest;
    let limits = GuestTestLimits {
        max_total_file_bytes: baseline_bytes + 1,
        ..p.limits()
    };
    p = GuestTestPolicy::new(
        p.owner_uid(),
        p.owner_gid(),
        p.caller_uid(),
        p.caller_gid(),
        p.device(),
        p.epoch_seconds(),
        p.epoch_nanoseconds(),
        p.scaffold_mode(),
        limits,
    )
    .unwrap()
    .with_label_data(p.label_data())
    .unwrap();
    single.install(p, Some(guest_test_tx(p, &single.r))).await;
    assert!(single.facts().await.data_sources().is_some());
    f.install(p, Some(guest_test_tx(p, &f.r))).await;
    let key = guest_test_api::SelectedGuestNamespaceFactsKey::new(f.key.profile_key().clone());
    let value = crate::compute_selected_guest_namespace_facts(f.tx.as_mut().unwrap(), &key)
        .await
        .unwrap();
    assert!(
        matches!(
            value,
            SourcePreparationOutcome::Complete(Err(
                guest_test_api::GuestNamespaceFactsError::Limit
            ))
        ),
        "{value:?}"
    );
    let budget = f.r.descriptor_budget().dupe();
    let payload = f.r.payload_budget().dupe();
    let retained = facts.dupe();
    drop(facts);
    drop(plan);
    drop(single_plan);
    drop(single);
    let held = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    assert_eq!(
        retained
            .lookup(&path, guest_test_api::GuestNamespaceOperation::ReadRegular)
            .unwrap_err(),
        guest_test_api::GuestLookupError::ObservationRejected
    );
    drop(held);
    assert!(
        retained
            .lookup(&path, guest_test_api::GuestNamespaceOperation::ReadRegular)
            .is_ok()
    );
    drop(retained);
    drop(f.tx.take());
    f.dice.wait_for_idle().await;
    let engine = f.dice.dupe();
    drop(f);
    assert!(
        budget.used_bytes() > 0,
        "engine history retains original descriptor owners"
    );
    drop(engine);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while budget.used_bytes() != 0 || payload.used_bytes() != 0 {
            tokio::task::yield_now().await
        }
    })
    .await
    .unwrap();
    assert_eq!(budget.used_bytes(), 0);
    assert_eq!(payload.used_bytes(), 0);
}

#[tokio::test]
async fn issued_data_labels_materialized_request_namespace_and_policy_a_b_a() {
    let mut f = DataTestFixture::label_case("@dep_alias//:data", "", "runtime", "/rt", false).await;
    let plan = f.plan().await;
    let row = plan.requests().next().unwrap();
    let (request, materialization) = row.selection().repository().unwrap();
    let request = request.dupe();
    assert!(
        matches!(materialization, DataTestMaterialization::Immutable { generation_root, observation_instance, .. }
        if generation_root == &PathBuf::from("/registry-dep") && *observation_instance == PathObservationInstanceId::new(77))
    );
    assert_eq!(
        row.leaf_image_key().namespace(),
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(77))
    );
    assert_eq!(
        row.leaf_image_key().source_root().as_path(),
        std::path::Path::new("/registry-dep/data")
    );
    let witness = &plan
        .runtime_requests()
        .tool_requests()
        .prefix()
        .label_sources()[0];
    assert_eq!(row.event_index(), witness.event_index());
    assert_eq!(
        row.selection(),
        witness.selection(),
        "complete normal origin, route and request lineage survives into Data Requests"
    );
    assert!(matches!(
        f.sources().await,
        SourcePreparationOutcome::Need(_)
    ));
    f.epoch = runtime_test_union(
        &f.epoch,
        &data_test_leaf_epoch(row.leaf_image_key(), &f.r, PathNodeKind::RegularFile, 7, 1),
    );
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    let a = f.ready().await;
    let original_epoch = f.epoch.dupe();
    let original_requests = f.requests.dupe();
    let enabled = f.guest;
    let disabled = enabled.with_label_data(DataTestPolicy::Disabled).unwrap();
    f.install(disabled, Some(guest_test_tx(disabled, &f.r)))
        .await;
    assert!(matches!(
        f.sources().await,
        SourcePreparationOutcome::Complete(Err(data_test_api::SelectedDataLabelError::Disabled))
    ));
    assert!(f.facts().await.data_sources().is_none());
    f.install(enabled, Some(guest_test_tx(enabled, &f.r))).await;
    assert_eq!(f.ready().await, a);
    for instance in [78, 77] {
        let from = PathObservationNamespace::Materialization(PathObservationInstanceId::new(77));
        let to =
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance));
        f.epoch = PathObservationEpoch::from_shared(original_epoch.observations().iter().map(
            |(d, result)| {
                let namespace = if d.namespace() == from {
                    to.dupe()
                } else {
                    d.namespace()
                };
                let demand = match d.operation() {
                    PathObservationOperation::InputImage(request) => {
                        PathObservationDemand::input_image(
                            namespace,
                            d.image_path().unwrap().dupe(),
                            request,
                        )
                        .unwrap()
                    }
                    op => PathObservationDemand::new(namespace, d.path().dupe(), op),
                };
                (demand, result.dupe())
            },
        ))
        .unwrap();
        let mut updater = f.tx.take().unwrap().into_updater();
        updater
            .changed_to([(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
                },
                RepositoryMaterializationResultEpoch::new(
                    NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
                    [RepositoryMaterializationEpochEntry {
                        request: request.dupe(),
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Immutable {
                                source_identity: Arc::from("sha256-test"),
                                generation_root: PathBuf::from("/registry-dep"),
                                observation_instance: PathObservationInstanceId::new(instance),
                            },
                        ),
                    }],
                )
                .unwrap(),
            )])
            .unwrap();
        f.tx = Some(updater.commit().await);
        f.install(enabled, Some(guest_test_tx(enabled, &f.r))).await;
        f.requests = runtime_test_requests(
            crate::compute_runtime_source_requests(
                f.tx.as_mut().unwrap(),
                f.key.profile_key().requests_key(),
            )
            .await
            .unwrap(),
        );
        f.install(enabled, Some(guest_test_tx(enabled, &f.r))).await;
        let current = f.ready().await;
        assert_eq!(
            current
                .requests()
                .requests()
                .next()
                .unwrap()
                .selection()
                .repository()
                .unwrap()
                .0,
            &request
        );
        assert_eq!(
            current
                .sources()
                .next()
                .unwrap()
                .leaf_image()
                .unwrap()
                .namespace(),
            to
        );
        if instance == 78 {
            assert_ne!(
                current, a,
                "same path and digest cannot erase the new normal materialization namespace"
            );
            assert_ne!(f.requests, original_requests);
        } else {
            assert_eq!(current, a);
        }
    }
}

// A real Label image and normal metadata producer; trusted raw facts describe the
// same sectionless 64-byte ELF used by the scratch fixture. No complete image,
// tool source or data source is injected.
fn data_test_tool_epoch(
    selected: &slug_bzlmod_v2::ObservedSelectedLabelSource,
    r: &runtime_test_bzl::SelectedScratchResources,
) -> PathObservationEpoch {
    use runtime_test_ws::ImageObservationData as D;
    use runtime_test_ws::ImageObservationOperation as O;
    let mut elf = [0u8; 64];
    elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    elf[16] = 2;
    elf[18] = 62;
    elf[20] = 1;
    elf[32] = 64;
    elf[52] = 64;
    elf[54] = 56;
    let digest = runtime_test_ws::FileContentDigest::new(Sha256::digest(elf).into(), 64).unwrap();
    let image_rows = runtime_test_image_epoch(selected.image_key(), r, 1, Some((1, 0o755)));
    let mut rows = image_rows
        .observations()
        .iter()
        .map(|(demand, result)| {
            let PathObservationResult::InputImage(observed) = result.as_ref() else {
                panic!("scoped image rows only")
            };
            let data = if demand.path() == selected.selected_file().normalized_ref() {
                match observed.data() {
                    D::Lstat(_) => D::Lstat(PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::RegularFile,
                        64,
                        1,
                        1,
                        1,
                        0o755,
                    ))),
                    D::Digest(_) => D::Digest(PathOperationResult::Present(digest)),
                    _ => panic!("unexpected file operation"),
                }
            } else {
                observed.data().clone()
            };
            (
                demand.dupe(),
                Arc::new(PathObservationResult::InputImage(
                    runtime_test_ws::ImageObservationResult::new(
                        observed.issued_path().dupe(),
                        observed.request().dupe(),
                        data,
                        r.descriptor_budget().try_reserve(512).unwrap(),
                    )
                    .unwrap(),
                )),
            )
        })
        .collect::<Vec<_>>();
    let binding = runtime_test_ws::ElfSourceDigestBinding::from_digest(digest);
    let policy = tool_test_policy().elf_policy();
    let verified = VerifiedElfMetadata::new(
        binding,
        policy,
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
        }),
        r.descriptor_budget().try_reserve(0).unwrap(),
    )
    .unwrap();
    let request = runtime_test_ws::ImageObservationRequest::new(
        runtime_test_ws::ImageChargedPath::pin_existing(
            selected.image_key().anchor_root().dupe(),
            r.descriptor_budget(),
        )
        .unwrap(),
        O::BoundedElfMetadata {
            expected_digest: binding,
            policy,
        },
    );
    let path = selected.selected_file().dupe();
    let demand = PathObservationDemand::input_image(
        selected.image_key().namespace(),
        path.dupe(),
        request.dupe(),
    )
    .unwrap();
    let result = runtime_test_ws::ImageObservationResult::new(
        path,
        request,
        D::ElfMetadata(PathOperationResult::Present(verified)),
        r.descriptor_budget().try_reserve(512).unwrap(),
    )
    .unwrap();
    rows.push((demand, Arc::new(PathObservationResult::InputImage(result))));
    PathObservationEpoch::from_shared(rows).unwrap()
}

#[tokio::test]
async fn issued_data_labels_tool_parent_reuse_retains_complete_membership() {
    let mut f = DataTestFixture::label_case(
        "//pkg:data",
        "    parent=data.dirname\n    again=parent.get_child('data')\n",
        "runtime",
        "/rt",
        true,
    )
    .await;
    let plan = f.plan().await;
    assert_eq!(plan.requests().len(), 3);
    let first = plan.requests().next().unwrap();
    let need = f.sources().await;
    assert!(
        matches!(need, SourcePreparationOutcome::Need(_)),
        "{need:?}"
    );
    f.epoch = runtime_test_union(&f.epoch, &data_test_tool_epoch(first.selection(), &f.r));
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    let ready = f.ready().await;
    assert!(ready.sources().all(|s| matches!(
        s.binding(),
        data_test_api::SelectedDataLabelBinding::ToolMember { role: 0, .. }
    )));
    assert!(matches!(
        ready.sources().nth(1).unwrap().binding(),
        data_test_api::SelectedDataLabelBinding::ToolMember { role: 0, entry: 0 }
    ));
    let facts = f.facts().await;
    let parent = first
        .selection()
        .image_key()
        .source_root()
        .as_path()
        .to_str()
        .unwrap();
    let node = facts
        .lookup(
            parent,
            guest_test_api::GuestNamespaceOperation::ListDirectory,
        )
        .unwrap();
    assert!(matches!(
        node.binding(),
        guest_test_api::GuestNamespaceBinding::ToolImage { role: 0, entry: 0 }
    ));
    assert_eq!(
        facts
            .lookup(
                &format!("{parent}/absent"),
                guest_test_api::GuestNamespaceOperation::Stat
            )
            .unwrap_err(),
        guest_test_api::GuestLookupError::Missing
    );
    assert_eq!(facts.data_sources(), Some(&ready));
}

#[tokio::test]
async fn issued_data_labels_foreign_namespace_same_guest_claim_is_not_reused() {
    let mut f =
        DataTestFixture::label_case("@dep_alias//:data", "", "runtime", "/registry-dep", false)
            .await;
    let plan = f.plan().await;
    let row = plan.requests().next().unwrap();
    // Both regular entries have the same size/digest/mode and guest spelling;
    // the runtime image is Host while the Label is materialization instance77.
    f.epoch = runtime_test_union(
        &f.epoch,
        &data_test_leaf_epoch(row.leaf_image_key(), &f.r, PathNodeKind::RegularFile, 1, 1),
    );
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    let ready = f.ready().await;
    let source = ready.sources().next().unwrap();
    assert!(matches!(
        source.binding(),
        data_test_api::SelectedDataLabelBinding::Leaf { .. }
    ));
    let image = source.leaf_image().unwrap();
    assert_ne!(
        image.namespace(),
        ready.profile().images().next().unwrap().namespace()
    );
    assert_eq!(
        image.entries()[0].file_digest(),
        ready.profile().images().next().unwrap().entries()[1].file_digest()
    );
    let key = guest_test_api::SelectedGuestNamespaceFactsKey::new(f.key.profile_key().clone());
    let value = crate::compute_selected_guest_namespace_facts(f.tx.as_mut().unwrap(), &key)
        .await
        .unwrap();
    assert!(
        matches!(
            value,
            SourcePreparationOutcome::Complete(Err(
                guest_test_api::GuestNamespaceFactsError::Collision
            ))
        ),
        "{value:?}"
    );
}

#[tokio::test]
async fn issued_data_labels_current_services_guard_cold_and_warm_without_raw_changes() {
    let mut f = DataTestFixture::new("", "runtime", "/rt").await;
    let plan = f.plan().await;
    f.epoch = runtime_test_union(
        &f.epoch,
        &data_test_leaf_epoch(
            plan.requests().next().unwrap().leaf_image_key(),
            &f.r,
            PathNodeKind::RegularFile,
            7,
            1,
        ),
    );
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    let a = f.ready().await;
    let foreign = scratch_test_resources();
    let own_scratch = data_test_ready_tools(&f.requests, &f.r).0;
    // Construct the foreign scratch transaction using a genuine independent plan:
    // a full request from this workspace may not be rebound to another service.
    let other = DataTestFixture::new("", "runtime", "/rt").await;
    assert_eq!(
        f.requests.tool_requests().selector(),
        other.requests.tool_requests().selector()
    );
    let foreign_scratch = data_test_ready_tools(&other.requests, &other.r).0;
    for case in 0..6 {
        let mut data = dice::UserComputationData {
            cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
            activation_tracker: Some(f.tracker.clone()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        data.data.set(f.r.descriptor_budget().dupe());
        if case != 4 {
            data.data.set(f.r.dupe());
        }
        data.data.set(if case == 5 {
            GuestTestTransaction::new(
                NormalizedAbsolutePath::new("/wrong-workspace").unwrap(),
                Some(f.guest),
                f.r.descriptor_budget().dupe(),
            )
            .unwrap()
        } else {
            guest_test_tx(f.guest, &f.r)
        });
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
        match case {
            0 => {}
            1 => data
                .data
                .set(runtime_test_authorization(&other.runtime, &other.r)),
            _ => data.data.set(runtime_test_authorization(&f.runtime, &f.r)),
        }
        match case {
            2 => {}
            3 => data.data.set(foreign_scratch.dupe()),
            _ => data.data.set(own_scratch.dupe()),
        }
        // No injection: all source cells, policies and cached normal values are
        // unchanged. Current services alone are insufficient on a stale cache hit.
        f.tx = Some(
            f.tx.take()
                .unwrap()
                .into_updater()
                .commit_with_data(data)
                .await,
        );
        assert!(
            f.tx.as_mut()
                .unwrap()
                .compute(&f.key)
                .await
                .unwrap()
                .complete_eq(&SourcePreparationOutcome::Complete(Ok(a.dupe())))
        );
        assert!(
            crate::compute_selected_data_label_sources(f.tx.as_mut().unwrap(), &f.key)
                .await
                .unwrap_err()
                .is_transient(),
            "warm case {case}"
        );
        let mut updater = f.tx.take().unwrap().into_updater();
        updater.changed([f.key.clone()]).unwrap();
        updater.changed([f.key.requests_key()]).unwrap();
        f.tx = Some(updater.commit().await);
        let cold = crate::compute_selected_data_label_sources(f.tx.as_mut().unwrap(), &f.key)
            .await
            .unwrap();
        assert!(
            matches!(&cold, SourcePreparationOutcome::Complete(Err(e)) if e.is_transient()),
            "cold case {case}: {cold:?}"
        );
        assert!(!data_test_api::SelectedDataLabelSourcesKey::validity(&cold));
        assert!(!data_test_api::SelectedDataLabelSourcesKey::equality(
            &cold, &cold
        ));
        f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
        assert_eq!(f.ready().await, a, "repair case {case}");
    }
    assert!(!a.same_resources(&foreign));
}

#[tokio::test]
async fn issued_data_labels_many_duplicates_keep_first_binding_and_count_all_parents() {
    let derive = "    duplicate=ctx.path(ctx.modules[0].tags.path[0].file)\n".repeat(31);
    let mut f = DataTestFixture::new(&derive, "runtime", "/rt").await;
    let plan = f.plan().await;
    assert_eq!(plan.requests().len(), 32);
    let first = plan.requests().next().unwrap();
    for (index, (row, witness)) in plan
        .requests()
        .zip(
            plan.runtime_requests()
                .tool_requests()
                .prefix()
                .label_sources(),
        )
        .enumerate()
    {
        assert_eq!(row.occurrence_index(), index);
        assert_eq!(row.event_index(), witness.event_index());
        assert_eq!(row.selection(), witness.selection());
        assert_eq!(row.selection(), first.selection());
    }
    f.epoch = runtime_test_union(
        &f.epoch,
        &data_test_leaf_epoch(
            first.leaf_image_key(),
            &f.r,
            PathNodeKind::RegularFile,
            7,
            1,
        ),
    );
    f.install(f.guest, Some(guest_test_tx(f.guest, &f.r))).await;
    let a = f.facts().await;
    let path = first
        .selection()
        .selected_file()
        .as_path()
        .to_str()
        .unwrap();
    assert!(matches!(
        a.lookup(path, guest_test_api::GuestNamespaceOperation::ReadRegular)
            .unwrap()
            .binding(),
        guest_test_api::GuestNamespaceBinding::DataLeaf { occurrence: 0 }
    ));
    assert_eq!(a.nodes().iter().filter(|n| n.path() == path).count(), 1);
    assert_eq!(a.data_sources().unwrap().sources().len(), 32);
    let mut entries = 0usize;
    let mut descriptors = 0usize;
    let mut count = |path: &std::path::Path| {
        entries += 1;
        descriptors += path.as_os_str().len();
    };
    let prefix = plan.runtime_requests().tool_requests().prefix().prefix();
    for entry in prefix.entries() {
        let root = std::path::Path::new(prefix.policy().guest_root());
        if entry.path().relative().is_empty() {
            count(root);
        } else {
            count(&root.join(entry.path().relative()));
        }
    }
    for parent in std::path::Path::new(prefix.policy().guest_root())
        .ancestors()
        .skip(1)
    {
        count(parent);
    }
    for (request, image) in a.profile().requests().requests().zip(a.profile().images()) {
        for entry in image.entries() {
            let suffix = entry
                .path()
                .as_path()
                .strip_prefix(image.source_root().as_path())
                .unwrap();
            if suffix.as_os_str().is_empty() {
                count(request.guest_prefix().as_path());
            } else {
                count(&request.guest_prefix().as_path().join(suffix));
            }
        }
        for parent in request.guest_prefix().as_path().ancestors().skip(1) {
            count(parent);
        }
    }
    for request in plan.requests() {
        for path in request.selection().selected_file().as_path().ancestors() {
            count(path);
        }
    }
    let make = |limits| {
        GuestTestPolicy::new(1000, 100, 1000, 100, 9, 123, 456, 0o755, limits)
            .unwrap()
            .with_label_data(DataTestPolicy::IssuedFileLeaves { max_witnesses: 32 })
            .unwrap()
    };
    let limits = GuestTestLimits {
        max_entries: entries,
        max_descriptor_bytes: descriptors,
        ..f.guest.limits()
    };
    let exact = make(limits);
    f.install(exact, Some(guest_test_tx(exact, &f.r))).await;
    assert_eq!(f.facts().await.nodes(), a.nodes());
    for limits in [
        GuestTestLimits {
            max_entries: entries - 1,
            ..limits
        },
        GuestTestLimits {
            max_descriptor_bytes: descriptors - 1,
            ..limits
        },
    ] {
        let limited = make(limits);
        f.install(limited, Some(guest_test_tx(limited, &f.r))).await;
        let key = guest_test_api::SelectedGuestNamespaceFactsKey::new(f.key.profile_key().clone());
        let value = crate::compute_selected_guest_namespace_facts(f.tx.as_mut().unwrap(), &key)
            .await
            .unwrap();
        assert!(
            matches!(
                &value,
                SourcePreparationOutcome::Complete(Err(
                    guest_test_api::GuestNamespaceFactsError::Limit
                ))
            ),
            "{value:?}"
        );
        assert!(guest_test_api::SelectedGuestNamespaceFactsKey::validity(
            &value
        ));
    }
    let limited = exact
        .with_label_data(DataTestPolicy::IssuedFileLeaves { max_witnesses: 31 })
        .unwrap();
    f.install(limited, Some(guest_test_tx(limited, &f.r))).await;
    let value =
        crate::compute_selected_data_label_requests(f.tx.as_mut().unwrap(), &f.key.requests_key())
            .await
            .unwrap();
    assert!(matches!(
        value,
        SourcePreparationOutcome::Complete(Err(
            data_test_api::SelectedDataLabelError::WitnessLimit
        ))
    ));
    f.install(exact, Some(guest_test_tx(exact, &f.r))).await;
    assert_eq!(f.facts().await.nodes(), a.nodes());
}
