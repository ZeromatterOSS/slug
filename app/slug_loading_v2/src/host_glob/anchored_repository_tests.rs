//! Actual authored General replay followed by tracked repository globbing.
//! Epoch rows below are pure DICE inputs; they mint no native root or IO grant.
use slug_bzlmod_v2::*;
use slug_identity_v2::ApparentRepoName;

use super::*;

const DISPLAY: &str = "/__slug_repositories__/glob-original";
const ORIGINAL: &str = r#"
def write(ctx):
    ctx.file('entry', 'first body', executable=False)
    ctx.file('other', 'second body', executable=False)
repo=repository_rule(implementation=write)
def impl(ctx):
    repo(name='first')
    repo(name='second')
ext=module_extension(implementation=impl)
"#;

fn limits() -> RepositoryWorkspaceRetentionLimits {
    RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024).unwrap()
}

async fn workspace_update(
    tx: ::dice::DiceTransaction,
    epoch: RepositoryWorkspaceEpoch,
) -> ::dice::DiceTransaction {
    let workspace = path(crate::canonical_repository_route_tests::tests::WORKSPACE);
    let mut update = tx.into_updater();
    update
        .changed_to([(
            RepositoryWorkspaceExecutionPolicyKey::new(workspace.dupe()),
            RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        )])
        .unwrap();
    update
        .changed_to([(RepositoryWorkspaceEpochKey::new(workspace), epoch)])
        .unwrap();
    update.commit().await
}

async fn original(
    tx: ::dice::DiceTransaction,
    lease: u64,
    instance: u64,
) -> (
    ::dice::DiceTransaction,
    Arc<HostRepositorySourceRoute>,
    RepositoryWorkspaceRealization,
) {
    let workspace = path(crate::canonical_repository_route_tests::tests::WORKSPACE);
    let mut tx = workspace_update(
        tx,
        RepositoryWorkspaceEpoch::fresh(workspace.dupe(), lease, limits()).unwrap(),
    )
    .await;
    let selected = tx
        .compute(&HostSelectedExtensionDemandKey::new(
            workspace.dupe(),
            slug_identity_v2::CanonicalRepoName::new("+ext+first").unwrap(),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(selected) = selected else {
        panic!("authored extension must select")
    };
    let owner = selected.as_ref().as_ref().unwrap().owner().clone();
    let effect =
        crate::HostSelectedRepositoryFileEffectObservationKey::new(workspace.dupe(), owner, 0);
    let mut epoch = RepositoryWorkspaceEpoch::fresh(workspace.dupe(), lease, limits()).unwrap();
    let mut completed = 0;
    let realization = loop {
        match tx.compute(&effect).await.unwrap() {
            SourcePreparationOutcome::Need(needs) if needs.workspace_reservations().len() == 1 => {
                let demand = needs.workspace_reservations().values().next().unwrap();
                let reservation = Arc::new(
                    RepositoryWorkspaceReservation::new(
                        workspace.dupe(),
                        demand.key().owner().clone(),
                        demand.key().repository().clone(),
                        demand.key().ordinal(),
                        demand.repo_spec().clone(),
                        path(DISPLAY),
                        PathObservationInstanceId::new(instance),
                        lease,
                        limits(),
                    )
                    .unwrap(),
                );
                epoch = epoch.reserve(reservation, 0o755).unwrap();
            }
            SourcePreparationOutcome::Need(needs) => {
                assert_eq!(
                    needs.workspace_operations().len(),
                    1,
                    "only actual authored File may suspend"
                );
                let request = needs
                    .workspace_operations()
                    .values()
                    .next()
                    .unwrap()
                    .request();
                assert_eq!(request.index(), completed);
                let RepositoryWorkspaceOperation::File { path, body, mode } = request.operation()
                else {
                    panic!("authored File")
                };
                assert_eq!(path.as_str(), ["entry", "other"][completed]);
                assert!(body.equals_slice(
                    [b"first body".as_slice(), b"second body".as_slice()][completed]
                ));
                let mut entries = request.predecessor().entries().to_vec();
                entries.push(
                    RepositoryWorkspaceEntry::new(
                        path.clone(),
                        *mode,
                        RepositoryWorkspaceNode::File(body.clone()),
                    )
                    .unwrap(),
                );
                entries.sort_by(|a, b| a.path().cmp(b.path()));
                epoch = epoch
                    .complete(
                        request.as_ref().clone(),
                        RepositoryWorkspaceResult::Unit,
                        request.predecessor().mode(),
                        entries,
                    )
                    .unwrap();
                completed += 1;
                assert!(completed <= 2);
            }
            SourcePreparationOutcome::Complete(observed) => {
                let observed = observed.unwrap();
                let result = observed.result().as_ref().as_ref().unwrap();
                break result.workspace_realization().unwrap().clone();
            }
        }
        tx = workspace_update(tx, epoch.clone()).await;
    };
    assert_eq!(completed, 2);
    assert_eq!(realization.transcript().records().len(), 2);
    let key = crate::HostRootRepositoryLoadRouteKey::new(
        workspace,
        ApparentRepoName::new("first").unwrap(),
    )
    .unwrap();
    let SourcePreparationOutcome::Complete(route) = tx.compute(&key).await.unwrap() else {
        panic!("actual General route")
    };
    let route = Arc::new(route.as_ref().as_ref().unwrap().source().clone());
    assert_eq!(route_realization(&route), &realization);
    (tx, route, realization)
}

fn route_request(route: &HostRepositorySourceRoute) -> &Arc<RepositoryMaterializationRequest> {
    let HostRepositorySourceRoute::Canonical(input) = route else {
        panic!("canonical authored route")
    };
    let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
    else {
        panic!("General request")
    };
    request
}
fn route_realization(route: &HostRepositorySourceRoute) -> &RepositoryWorkspaceRealization {
    let RepositoryMaterializationKind::GeneralWorkspace(realization) = &route_request(route).kind
    else {
        panic!("General producer required")
    };
    realization
}

async fn fixture() -> ::dice::DiceTransaction {
    use crate::canonical_repository_route_tests::tests as f;
    let dice = f::generated_graph_dice();
    let tx = f::transaction(&dice, f::CONSUMER_MODULE, ORIGINAL, true, None).await;
    // Same existing closed command inputs as the authored General replay tests.
    let workspace = path(f::WORKSPACE);
    let mut data = UserComputationData {
        cycle_detector: Some(crate::bzl_load_cycle_detector()),
        ..Default::default()
    };
    data.data.set(slug_events_v2::CaptureEvaluationEvents);
    data.data.set(RepositoryHostInputTransaction::default());
    data.data.set(SelectedExtensionDownloadTransaction::empty(
        workspace.dupe(),
    ));
    data.data.set(RepositoryExecuteEpoch::revoked());
    let mut update = tx.into_updater();
    update
        .changed_to([(
            RepositoryExecuteEpochKey::new(workspace.dupe()),
            RepositoryExecuteEpoch::revoked(),
        )])
        .unwrap();
    update
        .changed_to([(
            RepositoryEnvironmentSnapshotKey::new(workspace),
            RepositoryEnvironmentSnapshot::from_canonical(Vec::<RepositoryEnvironmentEntry>::new())
                .unwrap(),
        )])
        .unwrap();
    update.commit_with_data(data).await
}

fn row(namespace: PathObservationNamespace, p: &str, result: PathObservationResult) -> ScriptEntry {
    let operation = match &result {
        PathObservationResult::Lstat(_) => PathObservationOperation::Lstat,
        PathObservationResult::ReadLink(_) => PathObservationOperation::ReadLink,
        PathObservationResult::DirectoryEntries(_) => PathObservationOperation::DirectoryEntries,
        _ => panic!("fixture metadata only"),
    };
    (
        PathObservationDemand::new(namespace, path(p), operation),
        result,
    )
}
fn stat(namespace: PathObservationNamespace, p: &str, kind: PathNodeKind) -> ScriptEntry {
    row(
        namespace,
        p,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(kind))),
    )
}
fn listing(namespace: PathObservationNamespace, p: &str, names: &[&str]) -> ScriptEntry {
    row(
        namespace,
        p,
        PathObservationResult::DirectoryEntries(PathOperationResult::Present(
            PathDirectoryEntries::new(names.iter().map(|name| {
                PathDirectoryEntry::new(
                    PathDirectoryName::new(OsString::from(name)).unwrap(),
                    PathDirectoryEntryKind::File,
                )
            })),
        )),
    )
}
fn owned_rows(instance: u64) -> Vec<ScriptEntry> {
    let namespace =
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance));
    vec![
        stat(namespace, DISPLAY, PathNodeKind::Directory),
        stat(
            namespace,
            &format!("{DISPLAY}/entry"),
            PathNodeKind::RegularFile,
        ),
        stat(
            namespace,
            &format!("{DISPLAY}/other"),
            PathNodeKind::RegularFile,
        ),
        listing(namespace, DISPLAY, &["other", "entry"]),
    ]
}
async fn publish(
    tx: ::dice::DiceTransaction,
    route: &HostRepositorySourceRoute,
    base: &PathObservationEpoch,
    rows: &PathObservationEpoch,
) -> (::dice::DiceTransaction, PathObservationEpoch) {
    let realization = route_realization(route);
    let reservation = realization.reservation();
    let epoch = base.try_union(rows).unwrap();
    let mut update = tx.into_updater();
    update
        .changed_to([(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    update
        .changed_to([(
            RepositoryMaterializationResultEpochKey {
                workspace: reservation.workspace().dupe(),
            },
            RepositoryMaterializationResultEpoch::new(
                reservation.workspace().dupe(),
                [RepositoryMaterializationEpochEntry {
                    request: route_request(route).clone(),
                    result: RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Immutable {
                            path_policy: RepositoryMaterializationPathPolicy::GenerationRoot,
                            source_identity: realization.source_identity().clone(),
                            generation_root: reservation.display_root().as_path().to_owned(),
                            observation_instance: reservation.instance(),
                        },
                    ),
                }],
            )
            .unwrap(),
        )])
        .unwrap();
    (update.commit().await, epoch)
}
fn assert_arcs(observations: &PathObservationEpoch, expected: &PathObservationEpoch) {
    assert!(!observations.observations().is_empty());
    for (demand, result) in observations.observations() {
        assert!(Arc::ptr_eq(
            result,
            expected.get(demand).expect("actual supplied fact")
        ));
    }
}
async fn resolved(
    tx: &mut ::dice::DiceTransaction,
    route: &HostRepositorySourceRoute,
    relative: &str,
    epoch: &PathObservationEpoch,
) -> ResolvedPath {
    let key = if relative.is_empty() {
        HostRepositoryPathObservationKey::for_repository_root(route.clone())
    } else {
        HostRepositoryPathObservationKey::from_source_route(route.clone(), PathBuf::from(relative))
    };
    let SourcePreparationOutcome::Complete(observed) = tx.compute(&key).await.unwrap() else {
        panic!("source anchor must complete BEFORE glob")
    };
    let observed = observed.unwrap();
    assert_arcs(observed.observations(), epoch);
    let value = observed.result().as_ref().as_ref().unwrap().resolved();
    assert_eq!(
        value.anchor(),
        Some(
            &route_realization(route)
                .final_state()
                .resolution_anchor()
                .unwrap()
        )
    );
    value.dupe()
}
fn glob(
    route: Arc<HostRepositorySourceRoute>,
    directory: &str,
    logical: &str,
) -> HostGlobSegmentCandidatesObservationKey {
    HostGlobSegmentCandidatesObservationKey(HostGlobSegmentCandidatesKey::from_repository(
        route,
        Arc::new(PathBuf::from(directory)),
        path(logical),
        pattern("*"),
    ))
}
fn assert_glob(
    outcome: &ObservedHostGlobSegmentOutcome,
    epoch: &PathObservationEpoch,
    logical_namespace: PathObservationNamespace,
    expected: &[&[u8]],
) {
    if let SourcePreparationOutcome::Need(needs) = outcome {
        // Baseline diagnosis is relational only. No private source/runtime path.
        let demands = needs
            .path_observations()
            .expect("baseline anchor-loss path Need")
            .demands();
        assert!(!demands.is_empty());
        assert!(demands.iter().all(
            |d| d.namespace() == logical_namespace && !d.path().as_path().starts_with(DISPLAY)
        ));
    }
    let value = observed_value(outcome); // Primary RED only here, after actual anchor/certificate proof.
    assert_eq!(
        value
            .result
            .as_ref()
            .as_ref()
            .unwrap()
            .candidates()
            .iter()
            .map(|c| c.component.as_ref())
            .collect::<Vec<_>>(),
        expected
    );
    assert_arcs(&value.observations, epoch);
}

#[tokio::test]
async fn general_repository_wildcard_preserves_actual_observed_anchor_and_fact_owners() {
    let tx = fixture().await;
    let (mut tx, route, _) = original(tx, 71, 701).await;
    let base = tx.compute(&PathObservationEpochKey).await.unwrap();
    let rows = PathObservationEpoch::new(owned_rows(701)).unwrap();
    let (mut tx, epoch) = publish(tx, &route, &base, &rows).await;
    let value = resolved(&mut tx, &route, "", &epoch).await;
    assert_eq!(
        value.namespace(),
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(701))
    );
    assert_eq!(value.requested_path(), &path(DISPLAY));
    assert!(
        epoch
            .observations()
            .keys()
            .filter(|d| d.namespace() == value.namespace())
            .all(|d| d.path().as_path().starts_with(DISPLAY))
    );
    let key = glob(route.into(), "", DISPLAY);
    let outcome = tx.compute(&key).await.unwrap();
    assert_glob(&outcome, &epoch, value.namespace(), &[b"entry", b"other"]);
    assert!(HostGlobSegmentCandidatesObservationKey::validity(&outcome));
    let warm = tx.compute(&key).await.unwrap();
    assert!(HostGlobSegmentCandidatesObservationKey::equality(
        &outcome, &warm
    ));
    assert_glob(&warm, &epoch, value.namespace(), &[b"entry", b"other"]);
}

#[tokio::test]
async fn general_repository_same_display_cross_instance_a_b_a_preserves_selected_anchor() {
    let mut tx = fixture().await;
    let base = tx.compute(&PathObservationEpochKey).await.unwrap();
    // Preserve the authoritative fact allocations, including the A restoration.
    let a = PathObservationEpoch::new(owned_rows(701)).unwrap();
    let b = PathObservationEpoch::new(owned_rows(702)).unwrap();
    let mut previous = None;
    let mut first = None;
    for (lease, instance) in [(71, 701), (72, 702), (71, 701)] {
        let (next, route, _) = original(tx, lease, instance).await;
        let (next, epoch) =
            publish(next, &route, &base, if instance == 701 { &a } else { &b }).await;
        tx = next;
        let value = resolved(&mut tx, &route, "", &epoch).await;
        assert_eq!(value.anchor().unwrap().root(), &path(DISPLAY));
        assert_eq!(
            value.namespace(),
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance))
        );
        let key = glob(route.into(), "", DISPLAY);
        if let Some(previous) = &previous {
            assert_ne!(&key, previous);
        }
        if instance == 701 {
            if let Some(first) = &first {
                assert_eq!(&key, first)
            } else {
                first = Some(key.clone())
            }
        }
        let outcome = tx.compute(&key).await.unwrap();
        assert_glob(&outcome, &epoch, value.namespace(), &[b"entry", b"other"]);
        previous = Some(key);
    }
}

// The source-control continuation uses only closed public protocol operations;
// its original two File records still came from the actual observed callable.
// Native admission/sealing is tested separately by the Core fixture.
async fn source_route(
    tx: &mut ::dice::DiceTransaction,
    original: &HostRepositorySourceRoute,
    instance: u64,
) -> Arc<HostRepositorySourceRoute> {
    let realization = route_realization(original);
    let key = crate::HostCanonicalRepositoryRouteObservationKey::new(
        original.workspace().dupe(),
        slug_identity_v2::CanonicalRepoName::new("bazel_tools").unwrap(),
    );
    let SourcePreparationOutcome::Complete(Ok(observed)) = tx.compute(&key).await.unwrap() else {
        panic!("actual observed builtin canonical route")
    };
    let epoch = tx.compute(&PathObservationEpochKey).await.unwrap();
    assert_arcs(observed.observations(), &epoch);
    let route = observed.result().as_ref().as_ref().unwrap();
    assert_eq!(
        route.view().kind(),
        HostCanonicalRepositoryRouteKind::Builtin
    );
    assert!(route.view().builtin_identity().is_some());
    let builtin = host_canonical_repository_source_input(Arc::new(route.clone()), None).unwrap();
    let request = Arc::new(
        RepositorySourceSnapshotRequest::from_canonical(
            &builtin,
            realization.reservation().lease(),
        )
        .unwrap(),
    );
    let reservation = Arc::new(
        RepositorySourceSnapshotReservation::new(
            request,
            path("/__slug_repositories__/glob-source"),
            PathObservationInstanceId::new(instance),
        )
        .unwrap(),
    );
    let state = Arc::new(
        RepositorySourceSnapshotState::new(
            reservation,
            vec![
                RepositorySourceSnapshotEntry::new(
                    "".into(),
                    0o755,
                    RepositorySourceSnapshotNode::Directory,
                )
                .unwrap(),
                RepositorySourceSnapshotEntry::new(
                    "dir".into(),
                    0o755,
                    RepositorySourceSnapshotNode::Directory,
                )
                .unwrap(),
                RepositorySourceSnapshotEntry::new(
                    "dir/leaf".into(),
                    0o644,
                    RepositorySourceSnapshotNode::File(FileContentDigest::new([9; 32], 4).unwrap()),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let source = RepositoryWorkspaceReadView::from_source(state).unwrap();
    let transcript = realization.transcript();
    let target = "/__slug_repositories__/glob-source/dir";
    let request = transcript
        .request(RepositoryWorkspaceOperation::Symlink {
            path: "linked".into(),
            target: path(target),
            source: Some(source),
        })
        .unwrap();
    let mut entries = transcript.current().entries().to_vec();
    entries.push(
        RepositoryWorkspaceEntry::new(
            "linked".into(),
            0o777,
            RepositoryWorkspaceNode::Link(target.into()),
        )
        .unwrap(),
    );
    entries.sort_by(|a, b| a.path().cmp(b.path()));
    let transcript = transcript
        .complete(
            request,
            RepositoryWorkspaceResult::Unit,
            transcript.current().mode(),
            entries,
        )
        .unwrap();
    assert_eq!(transcript.records().len(), 3);
    let next = transcript.realize(3).unwrap();
    let HostRepositorySourceRoute::Canonical(input) = original else {
        panic!("canonical original")
    };
    Arc::new(HostRepositorySourceRoute::canonical(
        host_canonical_repository_source_input_with_workspace_realization(
            input.view().route().clone(),
            next,
        )
        .unwrap(),
    ))
}

#[tokio::test]
async fn general_repository_source_symlink_listing_keeps_logical_anchor_and_terminal_namespace() {
    let tx = fixture().await;
    let (mut tx, original, _) = original(tx, 71, 701).await;
    let base = tx.compute(&PathObservationEpochKey).await.unwrap();
    let ns = PathObservationNamespace::Materialization(PathObservationInstanceId::new(701));
    let mut logical_rows = owned_rows(701);
    logical_rows.extend([
        stat(ns, &format!("{DISPLAY}/linked"), PathNodeKind::Symlink),
        row(
            ns,
            &format!("{DISPLAY}/linked"),
            PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(PathBuf::from(
                "/__slug_repositories__/glob-source/dir",
            )))),
        ),
    ]);
    let logical_facts = PathObservationEpoch::new(logical_rows).unwrap();
    let source_facts = |instance| {
        let source_ns =
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance));
        logical_facts
            .try_union(
                &PathObservationEpoch::new([
                    stat(
                        source_ns,
                        "/__slug_repositories__/glob-source",
                        PathNodeKind::Directory,
                    ),
                    stat(
                        source_ns,
                        "/__slug_repositories__/glob-source/dir",
                        PathNodeKind::Directory,
                    ),
                    stat(
                        source_ns,
                        "/__slug_repositories__/glob-source/dir/leaf",
                        PathNodeKind::RegularFile,
                    ),
                    listing(
                        source_ns,
                        "/__slug_repositories__/glob-source/dir",
                        &["leaf"],
                    ),
                ])
                .unwrap(),
            )
            .unwrap()
    };
    let a = source_facts(801);
    let b = source_facts(802);
    let mut first = None;
    let mut previous = None;
    for instance in [801, 802, 801] {
        let route = source_route(&mut tx, &original, instance).await;
        let ns = PathObservationNamespace::Materialization(PathObservationInstanceId::new(701));
        let source_ns =
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance));
        let (next, epoch) = publish(tx, &route, &base, if instance == 801 { &a } else { &b }).await;
        tx = next;
        let value = resolved(&mut tx, &route, "linked", &epoch).await;
        let logical = value.anchor().unwrap();
        assert_eq!(logical.namespace(), ns);
        assert_eq!(logical.root(), &path(DISPLAY));
        assert_eq!(logical.source_anchors().len(), 1);
        assert_eq!(logical.source_anchors()[0].namespace(), source_ns);
        assert_eq!(value.content_origin().namespace(), source_ns);
        assert_ne!(value.content_origin().anchor(), value.anchor());
        assert_eq!(value.requested_path(), &path(&format!("{DISPLAY}/linked")));
        assert_eq!(
            value.real_path(),
            &path("/__slug_repositories__/glob-source/dir")
        );
        let key = glob(route, "linked", &format!("{DISPLAY}/linked"));
        if let Some(previous) = &previous {
            assert_ne!(&key, previous)
        }
        if instance == 801 {
            if let Some(first) = &first {
                assert_eq!(&key, first)
            } else {
                first = Some(key.clone())
            }
        }
        let outcome = tx.compute(&key).await.unwrap();
        assert_glob(&outcome, &epoch, value.namespace(), &[b"leaf"]);
        let facts = &observed_value(&outcome).observations;
        assert!(
            facts
                .observations()
                .keys()
                .any(|d| d.namespace() == source_ns
                    && d.operation() == PathObservationOperation::DirectoryEntries)
        );
        assert!(facts.observations().keys().all(|d| {
            let root = if d.namespace() == ns {
                DISPLAY
            } else {
                assert_eq!(d.namespace(), source_ns);
                "/__slug_repositories__/glob-source"
            };
            d.path().as_path().starts_with(root)
        }));
        previous = Some(key);
    }
}

#[tokio::test]
async fn general_repository_legacy_wildcard_uses_the_same_bounded_anchor() {
    let tx = fixture().await;
    let (mut tx, route, _) = original(tx, 71, 701).await;
    let base = tx.compute(&PathObservationEpochKey).await.unwrap();
    let rows = PathObservationEpoch::new(owned_rows(701)).unwrap();
    let (mut tx, epoch) = publish(tx, &route, &base, &rows).await;
    let value = resolved(&mut tx, &route, "", &epoch).await;
    assert_eq!(value.anchor().unwrap().root(), &path(DISPLAY));
    let observed_key = glob(route.into(), "", DISPLAY);
    let key = &observed_key.0;
    let outcome = tx.compute(key).await.unwrap();
    let SourcePreparationOutcome::Complete(result) = &outcome else {
        panic!("Legacy repository wildcard must retain the bounded anchor")
    };
    assert_eq!(
        result
            .as_ref()
            .as_ref()
            .unwrap()
            .candidates()
            .iter()
            .map(|candidate| (candidate.component.as_ref(), candidate.kind))
            .collect::<Vec<_>>(),
        vec![
            (
                b"entry".as_slice(),
                HostGlobSegmentCandidateKind::NonDirectory
            ),
            (
                b"other".as_slice(),
                HostGlobSegmentCandidateKind::NonDirectory
            ),
        ]
    );
    assert!(HostGlobSegmentCandidatesKey::validity(&outcome));
    let warm = tx.compute(key).await.unwrap();
    assert!(HostGlobSegmentCandidatesKey::equality(&outcome, &warm));
    let observed = tx.compute(&observed_key).await.unwrap();
    assert_glob(&observed, &epoch, value.namespace(), &[b"entry", b"other"]);
    assert_eq!(observed_value(&observed).result.as_ref(), result.as_ref());
}
