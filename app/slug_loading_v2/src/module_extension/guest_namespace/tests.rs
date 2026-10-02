// Trusted raw observations exercise normal producers; native tests own real IO.
use slug_bzlmod_v2::GuestNamespaceLimits as GuestTestLimits;
use slug_bzlmod_v2::GuestNamespacePolicy as GuestTestPolicy;
use slug_bzlmod_v2::GuestNamespaceTransaction as GuestTestTransaction;

use super::guest_namespace as guest_test_api;

fn guest_test_policy() -> GuestTestPolicy {
    GuestTestPolicy::new(1000, 100, 1000, 100, 9, 123, 456, 0o755, Default::default()).unwrap()
}
fn guest_test_tx(
    p: GuestTestPolicy,
    r: &runtime_test_bzl::SelectedScratchResources,
) -> GuestTestTransaction {
    GuestTestTransaction::new(
        NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
        Some(p),
        r.descriptor_budget().dupe(),
    )
    .unwrap()
}
fn guest_test_facts(
    value: <guest_test_api::SelectedGuestNamespaceFactsKey as Key>::Value,
) -> guest_test_api::ObservedGuestNamespaceFacts {
    match value {
        SourcePreparationOutcome::Complete(Ok(v)) => v,
        other => panic!("{other:?}"),
    }
}
struct GuestTestFixture {
    dice: Arc<Dice>,
    tx: Option<dice::DiceTransaction>,
    key: guest_test_api::SelectedGuestNamespaceFactsKey,
    r: runtime_test_bzl::SelectedScratchResources,
    tracker: Arc<RepositoryRuleActivationTracker>,
    policy: runtime_test_bzl::RuntimeSourceDeclarationPolicy,
    requests: runtime_test_api::ObservedRuntimeSourceRequests,
    baseline: PathObservationEpoch,
    epoch: PathObservationEpoch,
}
impl GuestTestFixture {
    async fn new(extra: &str, file: Option<(u8, u32)>) -> Self {
        let (dice, tx, tool, r, tracker) =
            tool_test_fixture(&tool_test_source(true, true, extra)).await;
        let key = guest_test_api::SelectedGuestNamespaceFactsKey::new(
            runtime_test_api::RuntimeSourceProfileKey::new(tool.requests_key().clone()),
        );
        let policy = runtime_test_policy(
            &r,
            &[runtime_test_relative("runtime", "/rt")],
            Default::default(),
        );
        let mut tx = runtime_test_install(
            tx,
            tracker.clone(),
            &r,
            Some(policy.dupe()),
            Some(runtime_test_authorization(&policy, &r)),
            None,
            None,
        )
        .await;
        let requests = runtime_test_requests(
            crate::compute_runtime_source_requests(&mut tx, key.profile_key().requests_key())
                .await
                .unwrap(),
        );
        let baseline = tx.compute(&PathObservationEpochKey).await.unwrap();
        let epoch = runtime_test_union(
            &baseline,
            &runtime_test_image_epoch(requests.requests().next().unwrap().image_key(), &r, 1, file),
        );
        Self {
            dice,
            tx: Some(tx),
            key,
            r,
            tracker,
            policy,
            requests,
            baseline,
            epoch,
        }
    }
    async fn install(
        &mut self,
        raw: Option<GuestTestPolicy>,
        current: Option<GuestTestTransaction>,
    ) {
        self.tx = Some(
            runtime_test_install_with_guest(
                self.tx.take().unwrap(),
                self.tracker.clone(),
                &self.r,
                Some(self.policy.dupe()),
                Some(runtime_test_authorization(&self.policy, &self.r)),
                Some(runtime_test_ready_scratch(&self.requests, &self.r)),
                Some(self.epoch.dupe()),
                Some((raw, current)),
            )
            .await,
        );
    }
    async fn compute(&mut self) -> <guest_test_api::SelectedGuestNamespaceFactsKey as Key>::Value {
        crate::compute_selected_guest_namespace_facts(self.tx.as_mut().unwrap(), &self.key)
            .await
            .unwrap()
    }
    async fn ready(&mut self, p: GuestTestPolicy) -> guest_test_api::ObservedGuestNamespaceFacts {
        self.install(Some(p), Some(guest_test_tx(p, &self.r))).await;
        guest_test_facts(self.compute().await)
    }
    fn image(&self) -> &runtime_test_ws::ObservedInputImageKey {
        self.requests.requests().next().unwrap().image_key()
    }
    fn replace_image(&mut self, freshness: i64, file: Option<(u8, u32)>) {
        self.epoch = runtime_test_union(
            &self.baseline,
            &runtime_test_image_epoch(self.image(), &self.r, freshness, file),
        );
    }
    async fn change_declaration(&mut self, relative: &str, guest: &str) {
        self.policy = runtime_test_policy(
            &self.r,
            &[runtime_test_relative(relative, guest)],
            Default::default(),
        );
        let mut tx = runtime_test_install(
            self.tx.take().unwrap(),
            self.tracker.clone(),
            &self.r,
            Some(self.policy.dupe()),
            Some(runtime_test_authorization(&self.policy, &self.r)),
            None,
            None,
        )
        .await;
        self.requests = runtime_test_requests(
            crate::compute_runtime_source_requests(&mut tx, self.key.profile_key().requests_key())
                .await
                .unwrap(),
        );
        self.tx = Some(tx);
        self.replace_image(1, Some((1, 0o644)));
    }
    fn change_parent_freshness(&mut self, freshness: i64) {
        let mut rows = Vec::new();
        for (demand, value) in self.epoch.observations() {
            let replacement = if demand.path() == self.image().source_root() {
                if let (
                    PathObservationOperation::InputImage(request),
                    PathObservationResult::InputImage(image),
                ) = (demand.operation(), value.as_ref())
                {
                    if let runtime_test_ws::ImageObservationData::Lstat(
                        PathOperationResult::Present(stat),
                    ) = image.data()
                    {
                        Some(Arc::new(PathObservationResult::InputImage(
                            runtime_test_ws::ImageObservationResult::new(
                                demand.image_path().unwrap().dupe(),
                                request,
                                runtime_test_ws::ImageObservationData::Lstat(
                                    PathOperationResult::Present(PathLstat::new(
                                        stat.kind(),
                                        stat.size(),
                                        freshness,
                                        freshness,
                                        stat.node_id(),
                                        stat.permissions(),
                                    )),
                                ),
                                self.r.descriptor_budget().try_reserve(512).unwrap(),
                            )
                            .unwrap(),
                        )))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            rows.push((demand.dupe(), replacement.unwrap_or_else(|| value.dupe())));
        }
        self.epoch = PathObservationEpoch::from_shared(rows).unwrap();
    }
}

#[tokio::test]
async fn guest_namespace_missing_policy_precedes_profile_and_pressure() {
    let (dice, tx, tool, r, tracker) = tool_test_fixture(&tool_test_source(true, true, "")).await;
    let key = guest_test_api::SelectedGuestNamespaceFactsKey::new(
        runtime_test_api::RuntimeSourceProfileKey::new(tool.requests_key().clone()),
    );
    let mut tx = runtime_test_install_with_guest(
        tx,
        tracker.clone(),
        &r,
        None,
        None,
        None,
        None,
        Some((None, None)),
    )
    .await;
    tracker.take_dependencies();
    let held = r
        .descriptor_budget()
        .try_reserve(r.descriptor_budget().limit_bytes() - r.descriptor_budget().used_bytes())
        .unwrap();
    let value = crate::compute_selected_guest_namespace_facts(&mut tx, &key)
        .await
        .unwrap();
    assert!(value.complete_eq(&SourcePreparationOutcome::Complete(Err(
        guest_test_api::GuestNamespaceFactsError::MissingDeclaration
    ))));
    assert!(<guest_test_api::SelectedGuestNamespaceFactsKey as Key>::validity(&value));
    drop(held);
    drop(tx);
    dice.wait_for_idle().await;
    assert!(
        !tracker
            .take_dependencies()
            .iter()
            .any(|(name, _)| name == &key.profile_key().to_string()),
        "absence must not start profile"
    );
}

#[tokio::test]
async fn guest_namespace_normal_needs_complete_metadata_and_child_lists() {
    let mut f = GuestTestFixture::new(
        "    ctx.file('dir/note','abc',executable=False)\n",
        Some((1, 0o644)),
    )
    .await;
    let p = guest_test_policy();
    let scratch = runtime_test_ready_scratch(&f.requests, &f.r);
    let request = f
        .requests
        .tool_requests()
        .requests()
        .next()
        .unwrap()
        .scratch_request()
        .unwrap();
    let allowed = SelectedScratchElfMetadataNeeds::singleton(request, &f.r).unwrap();
    let empty = SelectedScratchElfMetadataTransaction::new(
        f.requests.tool_requests().selector(),
        &f.r,
        Some(&allowed),
        &[],
    )
    .unwrap();
    f.tx = Some(
        runtime_test_install_with_guest(
            f.tx.take().unwrap(),
            f.tracker.clone(),
            &f.r,
            Some(f.policy.dupe()),
            Some(runtime_test_authorization(&f.policy, &f.r)),
            Some((
                empty,
                Some((
                    scratch.1.unwrap().0,
                    SelectedScratchElfMetadataEpoch::AuthorizedEmpty,
                )),
            )),
            None,
            Some((Some(p), Some(guest_test_tx(p, &f.r)))),
        )
        .await,
    );
    assert!(
        matches!(f.compute().await,SourcePreparationOutcome::Need(ref need) if need.scratch_elf_metadata_requests().is_some())
    );
    f.tx = Some(
        runtime_test_install_with_guest(
            f.tx.take().unwrap(),
            f.tracker.clone(),
            &f.r,
            Some(f.policy.dupe()),
            Some(runtime_test_authorization(&f.policy, &f.r)),
            Some(runtime_test_ready_scratch(&f.requests, &f.r)),
            None,
            Some((Some(p), Some(guest_test_tx(p, &f.r)))),
        )
        .await,
    );
    assert!(
        matches!(f.compute().await,SourcePreparationOutcome::Need(ref need) if need.path_observations().is_some())
    );
    let facts = f.ready(p).await;
    use guest_test_api::GuestNamespaceNodeKind as Kind;
    use guest_test_api::GuestNamespaceOperation as Op;
    for (i, n) in facts.nodes().iter().enumerate() {
        let m = n.metadata();
        assert_eq!(n.index(), i);
        assert_eq!(m.inode, i as u64 + 1);
        assert_eq!(m.device, 9);
        assert_eq!(
            (m.uid, m.gid, m.epoch_seconds, m.epoch_nanoseconds),
            (1000, 100, 123, 456)
        );
        assert_eq!(m.block_size, 4096);
        assert_eq!(m.blocks, m.size.div_ceil(512));
        assert_eq!(m.mode & 0o777, n.mode());
        if n.kind() == Kind::Directory {
            assert_eq!(m.size, 4096);
            assert_eq!(
                m.nlink,
                2 + facts
                    .children(i)
                    .iter()
                    .filter(|index| facts.node(**index).unwrap().kind() == Kind::Directory)
                    .count() as u64
            );
        } else {
            assert_eq!(m.nlink, 1);
            assert_eq!(
                m.size,
                match n.kind() {
                    Kind::RegularFile(d) => d.size_bytes(),
                    _ => unreachable!(),
                }
            );
        }
    }
    let root = facts.lookup("/", Op::ListDirectory).unwrap();
    assert_eq!(
        facts
            .children(root.index())
            .iter()
            .map(|i| facts.node(*i).unwrap().path())
            .collect::<Vec<_>>(),
        ["/__slug_module", "/rt"]
    );
    let dir = facts
        .lookup("/__slug_module/dir", Op::ListDirectory)
        .unwrap();
    assert_eq!(facts.children(dir.index()).len(), 1);
    assert_eq!(
        facts
            .lookup("/__slug_module/dir/note", Op::ReadRegular)
            .unwrap()
            .metadata()
            .size,
        3
    );
    assert!(
        facts
            .lookup("/__slug_module/generator", Op::Stat)
            .unwrap()
            .read_only()
    );
    assert!(
        !facts
            .lookup("/__slug_module/dir/note", Op::Stat)
            .unwrap()
            .read_only()
    );
}

#[tokio::test]
async fn guest_namespace_sequential_paths_permissions_and_readonly_outcomes() {
    let mut f = GuestTestFixture::new(
        "    ctx.file('dir/note','abc',executable=False)\n",
        Some((1, 0o644)),
    )
    .await;
    let facts = f.ready(guest_test_policy()).await;
    use guest_test_api::GuestLookupError as E;
    use guest_test_api::GuestNamespaceOperation as Op;
    for path in [
        "/__slug_module/dir/note/.",
        "/__slug_module/dir/note/..",
        "/__slug_module/dir/note/",
        "/__slug_module/dir/note//",
    ] {
        assert_eq!(
            facts.lookup(path, Op::Stat).unwrap_err(),
            E::NotDirectory,
            "{path}"
        );
    }
    assert_eq!(
        facts
            .lookup("//__slug_module///dir/./../generator", Op::Stat)
            .unwrap()
            .path(),
        "/__slug_module/generator"
    );
    assert_eq!(facts.lookup("/../../.", Op::Stat).unwrap().path(), "/");
    assert_eq!(
        facts
            .lookup("/__slug_module/generator", Op::Access(2))
            .unwrap_err(),
        E::ReadOnly
    );
    assert!(
        facts
            .lookup("/__slug_module/dir/note", Op::Access(2))
            .is_ok()
    );
    assert_eq!(
        facts.lookup("/rt", Op::ReadRegular).unwrap_err(),
        E::IsDirectory
    );
    assert_eq!(
        facts.lookup("/rt/data", Op::ListDirectory).unwrap_err(),
        E::NotDirectory
    );
    assert_eq!(
        facts.lookup("/rt/data", Op::ReadLink).unwrap_err(),
        E::NotSymlink
    );
    // Owner match wins, then primary group, then other; no group/root bypass.
    for (uid, gid, mode, expected) in [
        (1000, 999, 0o600, true),
        (999, 100, 0o600, false),
        (999, 999, 0o600, false),
        (999, 100, 0o640, true),
        (999, 999, 0o640, false),
        (999, 999, 0o004, true),
        (1000, 999, 0o004, false),
    ] {
        let p = GuestTestPolicy::new(1000, 100, uid, gid, 9, 123, 456, 0o755, Default::default())
            .unwrap();
        f.replace_image(1, Some((1, mode)));
        let changed = f.ready(p).await;
        assert_eq!(
            changed.lookup("/rt/data", Op::ReadRegular).is_ok(),
            expected
        );
        if !expected {
            assert_eq!(
                changed.lookup("/rt/data", Op::Access(2)).unwrap_err(),
                E::PermissionDenied
            );
        }
    }
    let deny =
        GuestTestPolicy::new(1000, 100, 1000, 100, 9, 123, 456, 0, Default::default()).unwrap();
    let inaccessible = f.ready(deny).await;
    assert_eq!(
        inaccessible.lookup("/absent", Op::Stat).unwrap_err(),
        E::PermissionDenied,
        "search denial precedes unknown name"
    );
    assert_eq!(
        inaccessible.lookup("/", Op::Stat).unwrap().path(),
        "/",
        "stat does not require final-directory R"
    );
}

#[tokio::test]
async fn guest_namespace_owned_missing_scaffold_unowned_and_callback_order() {
    let mut f = GuestTestFixture::new("", Some((1, 0o644))).await;
    let facts = f.ready(guest_test_policy()).await;
    use guest_test_api::GuestDirectoryCheck as C;
    use guest_test_api::GuestLookupError as E;
    use guest_test_api::GuestNamespaceOperation as Op;
    let mut calls = Vec::new();
    let mut callback = |node: &guest_test_api::GuestNamespaceNode, check| {
        calls.push((node.path().to_owned(), check));
        true
    };
    assert_eq!(
        facts
            .lookup_with("/rt/missing", Op::Stat, &mut callback)
            .unwrap_err(),
        E::Missing
    );
    assert_eq!(
        calls,
        [
            ("/".to_owned(), C::Traverse),
            ("/".to_owned(), C::Membership),
            ("/rt".to_owned(), C::Traverse),
            ("/rt".to_owned(), C::Membership)
        ]
    );
    assert_eq!(facts.lookup("/ambient", Op::Stat).unwrap_err(), E::Unowned);
    assert_eq!(
        facts
            .lookup_with("/rt/missing", Op::Stat, &mut |_, kind| kind
                != C::Membership)
            .unwrap_err(),
        E::ObservationRejected
    );
    for (path, op, expected) in [
        ("relative", Op::Stat, E::InvalidPath),
        ("/bad\0name", Op::Stat, E::InvalidPath),
        ("/rt", Op::Access(8), E::Unsupported),
        ("/rt", Op::Unsupported, E::Unsupported),
    ] {
        let mut called = false;
        assert_eq!(
            facts
                .lookup_with(path, op, &mut |_, _| {
                    called = true;
                    true
                })
                .unwrap_err(),
            expected
        );
        assert!(
            !called,
            "invalid operation/path cannot touch native callback"
        );
    }
    let long = format!("/{}", "a".repeat(4096));
    let mut called = false;
    assert_eq!(
        facts
            .lookup_with(&long, Op::Stat, &mut |_, _| {
                called = true;
                true
            })
            .unwrap_err(),
        E::PathLimit
    );
    assert!(!called);

    // A non-directory traversal component also needs its real metadata check
    // before file/.. can return a certified ENOTDIR.
    let mut calls = Vec::new();
    assert_eq!(
        facts
            .lookup_with("/rt/data/..", Op::Stat, &mut |node, kind| {
                calls.push((node.path().to_owned(), kind));
                true
            })
            .unwrap_err(),
        E::NotDirectory
    );
    assert_eq!(
        calls,
        [
            ("/".to_owned(), C::Traverse),
            ("/".to_owned(), C::Membership),
            ("/rt".to_owned(), C::Traverse),
            ("/rt".to_owned(), C::Membership),
            ("/rt/data".to_owned(), C::Metadata),
        ]
    );
    assert_eq!(
        facts
            .lookup_with("/rt/data/..", Op::Stat, &mut |_, kind| kind != C::Metadata)
            .unwrap_err(),
        E::ObservationRejected
    );

    // Final regular metadata is checked before a modeled permission error.
    // A rejected native observation cannot be disguised as certified EACCES.
    f.replace_image(1, Some((1, 0)));
    let denied_file = f.ready(guest_test_policy()).await;
    let mut calls = Vec::new();
    assert_eq!(
        denied_file
            .lookup_with("/rt/data", Op::ReadRegular, &mut |node, kind| {
                calls.push((node.path().to_owned(), kind));
                true
            })
            .unwrap_err(),
        E::PermissionDenied
    );
    assert_eq!(
        calls,
        [
            ("/".to_owned(), C::Traverse),
            ("/".to_owned(), C::Membership),
            ("/rt".to_owned(), C::Traverse),
            ("/rt".to_owned(), C::Membership),
            ("/rt/data".to_owned(), C::Metadata),
        ]
    );
    assert_eq!(
        denied_file
            .lookup_with("/rt/data", Op::ReadRegular, &mut |_, kind| kind
                != C::Metadata)
            .unwrap_err(),
        E::ObservationRejected
    );

    let inaccessible = f
        .ready(
            GuestTestPolicy::new(1000, 100, 1000, 100, 9, 123, 456, 0, Default::default()).unwrap(),
        )
        .await;
    let mut calls = Vec::new();
    assert_eq!(
        inaccessible
            .lookup_with("/", Op::ListDirectory, &mut |node, kind| {
                calls.push((node.path().to_owned(), kind));
                true
            })
            .unwrap_err(),
        E::PermissionDenied
    );
    assert_eq!(
        calls,
        [("/".to_owned(), C::Metadata)],
        "denied listing validates metadata without enumerating"
    );
    assert_eq!(
        inaccessible
            .lookup_with("/", Op::ListDirectory, &mut |_, _| false)
            .unwrap_err(),
        E::ObservationRejected
    );

    let mut calls = Vec::new();
    facts
        .lookup_with("/", Op::ListDirectory, &mut |_, kind| {
            calls.push(kind);
            true
        })
        .unwrap();
    assert_eq!(calls, [C::Metadata, C::Listing]);
}

#[tokio::test]
async fn guest_namespace_policy_content_mode_frontier_and_provenance_a_b_a() {
    let mut f = GuestTestFixture::new("", Some((1, 0o644))).await;
    let p = guest_test_policy();
    let a = f.ready(p).await;
    f.replace_image(1, Some((2, 0o600)));
    let b = f.ready(p).await;
    assert_ne!(a, b);
    f.replace_image(2, Some((2, 0o600)));
    let fresh = f.ready(p).await;
    assert_ne!(b, fresh);
    assert_eq!(
        b.profile().images().next().unwrap().content(),
        fresh.profile().images().next().unwrap().content()
    );
    f.replace_image(1, Some((1, 0o644)));
    assert_eq!(a, f.ready(p).await);
    let changed = GuestTestPolicy::new(
        1000,
        100,
        2000,
        200,
        10,
        124,
        457,
        0o750,
        Default::default(),
    )
    .unwrap();
    let credential = f.ready(changed).await;
    assert_ne!(a, credential);
    assert_eq!(credential.policy(), changed);
    assert_eq!(credential.nodes()[0].mode(), 0o750);
    assert_eq!(a, f.ready(p).await);
    // Same bytes and guest spelling with a different normal source tree remain
    // different full certificates; do not collapse provenance to digest alone.
    f.policy = runtime_test_policy(
        &f.r,
        &[runtime_test_relative("other-runtime", "/rt")],
        Default::default(),
    );
    let mut tx = runtime_test_install_with_guest(
        f.tx.take().unwrap(),
        f.tracker.clone(),
        &f.r,
        Some(f.policy.dupe()),
        Some(runtime_test_authorization(&f.policy, &f.r)),
        None,
        None,
        Some((Some(p), Some(guest_test_tx(p, &f.r)))),
    )
    .await;
    f.requests = runtime_test_requests(
        crate::compute_runtime_source_requests(&mut tx, f.key.profile_key().requests_key())
            .await
            .unwrap(),
    );
    f.tx = Some(tx);
    f.replace_image(1, Some((1, 0o644)));
    let origin = f.ready(p).await;
    assert_ne!(a, origin);
    assert_eq!(
        a.lookup("/rt/data", guest_test_api::GuestNamespaceOperation::Stat)
            .unwrap()
            .kind(),
        origin
            .lookup("/rt/data", guest_test_api::GuestNamespaceOperation::Stat)
            .unwrap()
            .kind()
    );
    f.change_declaration("runtime", "/rt").await;
    assert_eq!(a, f.ready(p).await);
    f.replace_image(1, None);
    let removed = f.ready(p).await;
    assert_eq!(
        removed
            .lookup("/rt/data", guest_test_api::GuestNamespaceOperation::Stat)
            .unwrap_err(),
        guest_test_api::GuestLookupError::Missing
    );
    f.replace_image(1, Some((1, 0o644)));
    assert_eq!(a, f.ready(p).await);
    f.change_declaration("runtime", "/rt-b").await;
    let spelling = f.ready(p).await;
    assert!(
        spelling
            .lookup("/rt-b/data", guest_test_api::GuestNamespaceOperation::Stat)
            .is_ok()
    );
    assert_eq!(
        spelling
            .lookup("/rt/data", guest_test_api::GuestNamespaceOperation::Stat)
            .unwrap_err(),
        guest_test_api::GuestLookupError::Unowned
    );
    f.change_declaration("runtime", "/rt").await;
    assert_eq!(a, f.ready(p).await);
    f.change_parent_freshness(2);
    let parent = f.ready(p).await;
    assert_ne!(
        a, parent,
        "parent-only observation freshness participates in full Eq"
    );
    assert_eq!(
        a.profile().images().next().unwrap().content(),
        parent.profile().images().next().unwrap().content()
    );
    f.change_parent_freshness(1);
    assert_eq!(a, f.ready(p).await);
}

#[tokio::test]
async fn guest_namespace_warm_full_authorization_requires_current_workspace_and_service() {
    let mut f = GuestTestFixture::new("", Some((1, 0o644))).await;
    let p = guest_test_policy();
    let a = f.ready(p).await;
    for current in [
        None,
        Some(
            GuestTestTransaction::new(
                NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
                Some(p),
                scratch_test_resources().descriptor_budget().dupe(),
            )
            .unwrap(),
        ),
        Some(
            GuestTestTransaction::new(
                NormalizedAbsolutePath::new("/foreign-workspace").unwrap(),
                Some(p),
                f.r.descriptor_budget().dupe(),
            )
            .unwrap(),
        ),
        Some(guest_test_tx(
            GuestTestPolicy::new(1000, 100, 2000, 100, 9, 123, 456, 0o755, Default::default())
                .unwrap(),
            &f.r,
        )),
    ] {
        f.install(Some(p), current).await;
        // Raw policy/ELF/image values are EXACTLY unchanged, so normal DICE may
        // reuse the old complete facts. The guarded warm helper must refuse.
        let cached = guest_test_facts(f.tx.as_mut().unwrap().compute(&f.key).await.unwrap());
        assert_eq!(a, cached);
        assert!(
            crate::compute_selected_guest_namespace_facts(f.tx.as_mut().unwrap(), &f.key)
                .await
                .unwrap_err()
                .is_transient()
        );
    }
    assert_eq!(a, f.ready(p).await);

    let foreign = GuestTestFixture::new("", Some((1, 0o644))).await;
    assert_eq!(f.policy, foreign.policy);
    assert_eq!(
        f.requests.tool_requests().selector(),
        foreign.requests.tool_requests().selector(),
        "independent normal requests have equal semantic selectors"
    );
    let foreign_scratch = runtime_test_ready_scratch(&foreign.requests, &foreign.r).0;
    let own_scratch = runtime_test_ready_scratch(&f.requests, &f.r).0;
    for case in 0..4 {
        let mut data = dice::UserComputationData {
            cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
            activation_tracker: Some(f.tracker.clone()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        data.data.set(f.r.descriptor_budget().dupe());
        data.data.set(f.r.dupe());
        data.data.set(guest_test_tx(p, &f.r));
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
            0 => {} // Missing runtime current transaction.
            1 => data
                .data
                .set(runtime_test_authorization(&foreign.policy, &foreign.r)),
            _ => data.data.set(runtime_test_authorization(&f.policy, &f.r)),
        }
        match case {
            2 => {} // Missing scratch current transaction.
            3 => data.data.set(foreign_scratch.dupe()),
            _ => data.data.set(own_scratch.dupe()),
        }
        // No changed_to calls: replace ONLY userdata; all raw policies, image
        // observations and Ready cells remain the same cached dependencies.
        f.tx = Some(
            f.tx.take()
                .unwrap()
                .into_updater()
                .commit_with_data(data)
                .await,
        );
        let cached = guest_test_facts(f.tx.as_mut().unwrap().compute(&f.key).await.unwrap());
        assert_eq!(a, cached, "case {case}: warm complete facts are unchanged");
        assert!(
            crate::compute_selected_guest_namespace_facts(f.tx.as_mut().unwrap(), &f.key)
                .await
                .unwrap_err()
                .is_transient(),
            "case {case}: cached facts cannot authorize lower foreign/missing transactions"
        );
    }
    assert_eq!(a, f.ready(p).await);
}

#[tokio::test]
async fn guest_namespace_special_modes_refuse_before_projection() {
    let mut f = GuestTestFixture::new("", Some((1, 0o644))).await;
    let p = guest_test_policy();
    let a = f.ready(p).await;
    for mode in [0o1644, 0o2644, 0o4644] {
        f.replace_image(1, Some((1, mode)));
        f.install(Some(p), Some(guest_test_tx(p, &f.r))).await;
        let outcome = f.compute().await;
        assert!(outcome.complete_eq(&SourcePreparationOutcome::Complete(Err(
            guest_test_api::GuestNamespaceFactsError::UnsupportedSource
        ))));
        assert!(<guest_test_api::SelectedGuestNamespaceFactsKey as Key>::validity(&outcome));
    }
    f.replace_image(1, Some((1, 0o644)));
    assert_eq!(a, f.ready(p).await);
}

#[tokio::test]
async fn guest_namespace_combined_scaffold_entry_path_and_byte_exact_limits() {
    let mut f = GuestTestFixture::new("", Some((1, 0o644))).await;
    let p = guest_test_policy();
    let a = f.ready(p).await;
    // Scratch root+generator, runtime root+data, and root scaffolds generated
    // independently for BOTH scopes. Occurrences count before root dedup.
    assert_eq!(a.nodes().len(), 5);
    let bytes = a
        .nodes()
        .iter()
        .map(|n| match n.kind() {
            guest_test_api::GuestNamespaceNodeKind::RegularFile(d) => d.size_bytes(),
            _ => 0,
        })
        .sum::<u64>();
    assert_eq!(bytes, 65);
    let descriptors = a.nodes().iter().map(|n| n.path().len()).sum::<usize>() + 1; // second scaffold '/' occurrence
    let exact = GuestTestLimits {
        max_entries: 6,
        max_depth: 2,
        max_path_bytes: "/__slug_module/generator".len(),
        max_descriptor_bytes: descriptors,
        max_total_file_bytes: 65,
    };
    let make =
        |limits| GuestTestPolicy::new(1000, 100, 1000, 100, 9, 123, 456, 0o755, limits).unwrap();
    assert_eq!(f.ready(make(exact)).await.nodes(), a.nodes());
    for limits in [
        GuestTestLimits {
            max_entries: 5,
            ..exact
        },
        GuestTestLimits {
            max_depth: 1,
            ..exact
        },
        GuestTestLimits {
            max_path_bytes: exact.max_path_bytes - 1,
            ..exact
        },
        GuestTestLimits {
            max_descriptor_bytes: descriptors - 1,
            ..exact
        },
        GuestTestLimits {
            max_total_file_bytes: 64,
            ..exact
        },
    ] {
        let limited = make(limits);
        f.install(Some(limited), Some(guest_test_tx(limited, &f.r)))
            .await;
        let value = f.compute().await;
        assert!(
            value.complete_eq(&SourcePreparationOutcome::Complete(Err(
                guest_test_api::GuestNamespaceFactsError::Limit
            ))),
            "{limits:?}: {value:?}"
        );
        assert!(<guest_test_api::SelectedGuestNamespaceFactsKey as Key>::validity(&value));
    }
}

#[tokio::test]
async fn guest_namespace_normal_pressure_repair_history_and_cloned_last_drop() {
    let mut f = GuestTestFixture::new("", Some((1, 0o644))).await;
    let p = guest_test_policy();
    f.install(Some(p), Some(guest_test_tx(p, &f.r))).await;
    runtime_test_profile(
        crate::compute_runtime_source_profile(f.tx.as_mut().unwrap(), f.key.profile_key())
            .await
            .unwrap(),
    );
    let held = f
        .r
        .descriptor_budget()
        .try_reserve(f.r.descriptor_budget().limit_bytes() - f.r.descriptor_budget().used_bytes())
        .unwrap();
    let failed = f.compute().await;
    assert!(matches!(failed,SourcePreparationOutcome::Complete(Err(ref e)) if e.is_transient()));
    assert!(!<guest_test_api::SelectedGuestNamespaceFactsKey as Key>::validity(&failed));
    drop(held);
    let updater = f.tx.take().unwrap().into_updater();
    f.dice.wait_for_idle().await;
    f.tx = Some(updater.commit().await);
    let a = guest_test_facts(f.compute().await);
    let lookup_pressure = f
        .r
        .descriptor_budget()
        .try_reserve(f.r.descriptor_budget().limit_bytes() - f.r.descriptor_budget().used_bytes())
        .unwrap();
    let used = f.r.descriptor_budget().used_bytes();
    let mut called = false;
    assert_eq!(
        a.lookup_with(
            "/rt/data",
            guest_test_api::GuestNamespaceOperation::Stat,
            &mut |_, _| {
                called = true;
                true
            }
        )
        .unwrap_err(),
        guest_test_api::GuestLookupError::ObservationRejected
    );
    assert!(
        !called,
        "resolver credit must precede stack scratch and native callbacks"
    );
    assert_eq!(f.r.descriptor_budget().used_bytes(), used);
    drop(lookup_pressure);
    let baseline = f.r.descriptor_budget().used_bytes();
    assert!(
        a.lookup_with(
            "/rt/data",
            guest_test_api::GuestNamespaceOperation::Stat,
            &mut |_, _| {
                assert_eq!(f.r.descriptor_budget().used_bytes(), baseline + 4352);
                true
            }
        )
        .is_ok()
    );
    assert_eq!(
        f.r.descriptor_budget().used_bytes(),
        baseline,
        "successful lookup releases scratch while facts stay alive"
    );
    assert_eq!(
        a.lookup_with(
            "/rt/data",
            guest_test_api::GuestNamespaceOperation::Stat,
            &mut |_, kind| {
                assert_eq!(f.r.descriptor_budget().used_bytes(), baseline + 4352);
                kind != guest_test_api::GuestDirectoryCheck::Metadata
            }
        )
        .unwrap_err(),
        guest_test_api::GuestLookupError::ObservationRejected
    );
    assert_eq!(
        f.r.descriptor_budget().used_bytes(),
        baseline,
        "callback rejection releases scratch while facts stay alive"
    );
    let changed = GuestTestPolicy::new(
        1000,
        100,
        1000,
        100,
        10,
        123,
        456,
        0o755,
        Default::default(),
    )
    .unwrap();
    let b = f.ready(changed).await;
    assert_ne!(a, b);
    let keep = a.dupe();
    drop(a);
    drop(b);
    drop(f.tx.take());
    f.dice.wait_for_idle().await;
    assert!(
        f.r.descriptor_budget().used_bytes() > 0,
        "idle is not historical retirement"
    );
    let resources = f.r.dupe();
    drop(f);
    // The actual certificate clone pins its producer/profile payload and
    // descriptor credits even after every engine sender has been dropped.
    assert!(resources.descriptor_budget().used_bytes() > 0);
    assert!(resources.payload_budget().used_bytes() > 0);
    drop(keep);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while resources.descriptor_budget().used_bytes() != 0
            || resources.payload_budget().used_bytes() != 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual engine-history and namespace-clone lastDrop must retire credits");
    assert_eq!(resources.descriptor_budget().used_bytes(), 0);
    assert_eq!(resources.payload_budget().used_bytes(), 0);
}
