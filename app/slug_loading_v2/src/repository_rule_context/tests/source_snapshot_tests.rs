//! Original evaluation consumes source snapshots before prepared external bytes.
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositorySourceSnapshotEntry;
use slug_bzlmod_v2::RepositorySourceSnapshotEpoch;
use slug_bzlmod_v2::RepositorySourceSnapshotNode;
use slug_bzlmod_v2::RepositorySourceSnapshotRequest;
use slug_bzlmod_v2::RepositorySourceSnapshotReservation;
use slug_bzlmod_v2::RepositorySourceSnapshotState;
use slug_bzlmod_v2::RepositoryWorkspaceEntry;
use slug_bzlmod_v2::RepositoryWorkspaceEpoch;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceReservationDemand;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::PathObservationInstanceId;

use super::*;

fn attempt(
    source: &str,
    epoch: &RepositoryWorkspaceEpoch,
    demand: &RepositoryWorkspaceReservationDemand,
    snapshots: Arc<RepositorySourceSnapshotEpoch>,
    paths: &PreparedRepositoryLabelPaths,
    sources: &SmallMap<RepositoryLabelPathAddress, Arc<[u8]>>,
) -> RepositoryRuleInvocationAttempt {
    let manifest = invocation_manifest();
    let module = freeze_bzl(&manifest, source, None);
    let implementation = unsafe {
        module
            .get("implementation")
            .unwrap()
            .unchecked_frozen_value()
    };
    let input = RepositoryRuleInvocationInput::new(
        "repo".into(),
        None,
        Arc::new(SmallMap::new()),
        Arc::from([]),
    )
    .unwrap()
    .with_workspace(
        epoch.clone(),
        demand.clone(),
        RepositoryMaterializationResultEpoch::new(demand.key().workspace().clone(), []).unwrap(),
    )
    .unwrap()
    .with_source_snapshots(snapshots);
    invoke_repository_rule_with_host_paths(
        implementation,
        &manifest,
        paths,
        sources,
        &SmallMap::new(),
        &SmallMap::new(),
        input,
        RepositoryPlatform::new("linux", "x86_64"),
        RepositoryEnvironmentSnapshot::from_canonical(Arc::from([])).unwrap(),
        None,
    )
}

async fn builtin(
    lease: u64,
) -> (
    Arc<RepositorySourceSnapshotEpoch>,
    PreparedRepositoryLabelPaths,
    SmallMap<RepositoryLabelPathAddress, Arc<[u8]>>,
) {
    use crate::canonical_repository_route_tests::tests as fixture;
    let dice = fixture::builtin_graph_dice();
    let mut transaction = fixture::transaction(
        &dice,
        &fixture::builtin_graph_module(),
        fixture::EXTENSION_A,
        true,
        None,
    )
    .await;
    let result = transaction
        .compute(
            &slug_bzlmod_v2::HostBuiltinBazelToolsRepositoryMappingKey::new(
                NormalizedAbsolutePath::new(fixture::WORKSPACE).unwrap(),
            ),
        )
        .await
        .unwrap();
    let slug_bzlmod_v2::SourcePreparationOutcome::Complete(mapping) = result else {
        panic!("genuine builtin mapping must complete")
    };
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let route = Arc::new(slug_bzlmod_v2::HostCanonicalRepositoryRoute::builtin(
        workspace.clone(),
        mapping.as_ref().as_ref().unwrap().clone(),
    ));
    let input = slug_bzlmod_v2::host_canonical_repository_source_input(route, None).unwrap();
    let request = Arc::new(RepositorySourceSnapshotRequest::from_canonical(&input, lease).unwrap());
    assert!(request.repo_spec().is_none());
    let reservation = Arc::new(
        RepositorySourceSnapshotReservation::new(
            request,
            NormalizedAbsolutePath::new("/__slug_repositories__/builtin").unwrap(),
            PathObservationInstanceId::new(100),
        )
        .unwrap(),
    );
    let mut entries = vec![
        RepositorySourceSnapshotEntry::new(
            "".into(),
            0o755,
            RepositorySourceSnapshotNode::Directory,
        )
        .unwrap(),
    ];
    slug_bzlmod_v2::BuiltinBazelToolsSnapshot::CURRENT
        .visit_validated_catalog(
            slug_bzlmod_v2::BuiltinBazelToolsSnapshot::CURRENT.route_identity(),
            |path, bytes, executable, digest| {
                let mut parent = path;
                while let Some((directory, _)) = parent.rsplit_once('/') {
                    if !entries.iter().any(|e| e.path() == directory) {
                        entries.push(
                            RepositorySourceSnapshotEntry::new(
                                directory.into(),
                                0o755,
                                RepositorySourceSnapshotNode::Directory,
                            )
                            .unwrap(),
                        );
                    }
                    parent = directory;
                }
                entries.push(
                    RepositorySourceSnapshotEntry::new(
                        path.into(),
                        if executable { 0o755 } else { 0o644 },
                        RepositorySourceSnapshotNode::File(
                            FileContentDigest::new(digest, bytes.len() as u64).unwrap(),
                        ),
                    )
                    .unwrap(),
                );
                Ok::<_, slug_bzlmod_v2::BuiltinBazelToolsSourceFileError>(())
            },
        )
        .unwrap();
    entries.sort_unstable_by(|a, b| a.path().cmp(b.path()));
    let state = Arc::new(RepositorySourceSnapshotState::new(reservation.clone(), entries).unwrap());
    let epoch = Arc::new(
        RepositorySourceSnapshotEpoch::fresh(workspace, lease)
            .unwrap()
            .with_snapshot(state)
            .unwrap(),
    );
    let address = RepositoryLabelPathAddress::from_label(
        &CanonicalLabel::parse("@@bazel_tools//:MODULE.bazel").unwrap(),
    );
    let path = HostRepositoryLabelPathValue::new(
        NormalizedAbsolutePath::new(reservation.display_root().as_path().join("MODULE.bazel"))
            .unwrap(),
        PathObservationNamespace::Materialization(reservation.instance()),
    )
    .unwrap();
    let body = transaction
        .compute(&slug_bzlmod_v2::BuiltinBazelToolsSourceFileKey::new(
            slug_bzlmod_v2::BuiltinBazelToolsSnapshot::CURRENT,
            "MODULE.bazel",
        ))
        .await
        .unwrap();
    (
        epoch,
        SmallMap::from_iter([(address.clone(), path)]),
        SmallMap::from_iter([(address, body.as_ref().as_ref().unwrap().bytes_arc().clone())]),
    )
}

#[tokio::test]
async fn direct_external_read_and_template_demand_label_path_before_source_bytes() {
    let (epoch, demand) = super::workspace_tests::binding(7).await;
    let snapshots = Arc::new(
        RepositorySourceSnapshotEpoch::fresh(demand.key().workspace().clone(), 7).unwrap(),
    );
    let address = RepositoryLabelPathAddress::from_label(
        &CanonicalLabel::parse("@@bazel_tools//:MODULE.bazel").unwrap(),
    );
    let prepared = SmallMap::from_iter([(address.clone(), Arc::<[u8]>::from(&b"prepared"[..]))]);
    for expression in [
        "ctx.read(Label('@@bazel_tools//:MODULE.bazel'))",
        "ctx.template('copy',Label('@@bazel_tools//:MODULE.bazel'))",
    ] {
        let source = format!("def implementation(ctx):\n    {expression}\n");
        assert!(
            matches!(attempt(&source,&epoch,&demand,snapshots.clone(),&SmallMap::new(),&prepared).result,
            Err(RepositoryRuleInvocationError::LabelPathNeed(actual)) if actual==address)
        );
    }
}

#[tokio::test]
async fn builtin_prepared_read_template_and_execute_require_matching_source_snapshot() {
    let (mut epoch, demand) = super::workspace_tests::binding(7).await;
    let (snapshots, paths, sources) = builtin(7).await;
    let address = RepositoryLabelPathAddress::from_label(
        &CanonicalLabel::parse("@@bazel_tools//:MODULE.bazel").unwrap(),
    );
    let source = "def implementation(ctx):\n    if not ctx.read(Label('@@bazel_tools//:MODULE.bazel')): fail('empty')\n    ctx.template('copy',Label('@@bazel_tools//:MODULE.bazel'),{},False)\n    ctx.execute(['arbitrary'])\n";
    assert!(
        matches!(attempt(source,&epoch,&demand,snapshots.clone(),&paths,&SmallMap::new()).result,
        Err(RepositoryRuleInvocationError::RepositorySourceNeed(actual)) if actual==address)
    );
    for stale in [
        Arc::new(RepositorySourceSnapshotEpoch::revoked()),
        Arc::new(
            RepositorySourceSnapshotEpoch::fresh(demand.key().workspace().clone(), 8).unwrap(),
        ),
    ] {
        assert_eq!(
            attempt(
                "def implementation(ctx):\n    ctx.path(Label('@@bazel_tools//:MODULE.bazel'))\n",
                &epoch,
                &demand,
                stale.clone(),
                &paths,
                &sources
            )
            .result
            .err(),
            Some(RepositoryRuleInvocationError::WorkspaceReplay)
        );
        assert_eq!(
            attempt(source, &epoch, &demand, stale, &paths, &sources)
                .result
                .err(),
            Some(RepositoryRuleInvocationError::WorkspaceReplay)
        );
    }
    let first = attempt(source, &epoch, &demand, snapshots.clone(), &paths, &sources);
    let Err(RepositoryRuleInvocationError::WorkspaceNeed(template)) = first.result else {
        panic!("template requires actual workspace completion")
    };
    let RepositoryWorkspaceOperation::Template {
        path,
        body,
        source_body,
        ..
    } = template.request().operation()
    else {
        panic!("template operation")
    };
    assert_eq!(path.as_str(), "copy");
    assert_eq!(body.as_ref(), sources.get(&address).unwrap().as_ref());
    assert_eq!(source_body, body);
    let mut entries = template.request().predecessor().entries().to_vec();
    entries.push(
        RepositoryWorkspaceEntry::new(
            path.clone(),
            0o644,
            RepositoryWorkspaceNode::File(body.clone()),
        )
        .unwrap(),
    );
    entries.sort_unstable_by(|a, b| a.path().cmp(b.path()));
    epoch = epoch
        .complete(
            template.request().as_ref().clone(),
            RepositoryWorkspaceResult::Unit,
            template.request().predecessor().mode(),
            entries,
        )
        .unwrap();
    let Err(RepositoryRuleInvocationError::WorkspaceNeed(execute)) =
        attempt(source, &epoch, &demand, snapshots, &paths, &sources).result
    else {
        panic!("execute requires observation")
    };
    let RepositoryWorkspaceOperation::Execute(execute) = execute.request().operation() else {
        panic!("execute operation")
    };
    assert_eq!(execute.read_views().views().len(), 1);
    let view = &execute.read_views().views()[0];
    assert!(view.workspace_state().is_none());
    assert!(view.repo_spec().is_none());
    assert_eq!(
        view.source_state().unwrap().reservation().request().kind(),
        slug_bzlmod_v2::RepositorySourceSnapshotKind::Builtin
    );
}

#[tokio::test]
async fn invalid_execute_options_precede_unresolved_label_and_source_paths_never_fall_back_to_host()
{
    let (epoch, demand) = super::workspace_tests::binding(7).await;
    let snapshots = Arc::new(
        RepositorySourceSnapshotEpoch::fresh(demand.key().workspace().clone(), 7).unwrap(),
    );
    for expression in [
        "ctx.execute([Label('@@bazel_tools//:MODULE.bazel')],True)",
        "ctx.execute([Label('@@bazel_tools//:MODULE.bazel')],600,{False:'x'})",
        "ctx.execute([Label('@@bazel_tools//:MODULE.bazel')],600,{'A':False})",
    ] {
        assert_eq!(
            attempt(
                &format!("def implementation(ctx):\n    {expression}\n"),
                &epoch,
                &demand,
                snapshots.clone(),
                &SmallMap::new(),
                &SmallMap::new()
            )
            .result
            .err(),
            Some(RepositoryRuleInvocationError::ExecuteOptions)
        );
    }
    let (snapshots, paths, sources) = builtin(7).await;
    let source = "def implementation(ctx):\n    ctx.read(str(ctx.path(Label('@@bazel_tools//:MODULE.bazel'))))\n";
    assert_eq!(
        attempt(source, &epoch, &demand, snapshots.clone(), &paths, &sources)
            .result
            .err(),
        Some(RepositoryRuleInvocationError::WorkspaceUnsupported)
    );
    let source = "def implementation(ctx):\n    ctx.path(Label('@@bazel_tools//:MODULE.bazel')).get_child('child')\n";
    assert_eq!(
        attempt(source, &epoch, &demand, snapshots, &paths, &sources)
            .result
            .err(),
        Some(RepositoryRuleInvocationError::WorkspaceUnsupported)
    );
}
