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

fn complete_symlink(
    epoch: &RepositoryWorkspaceEpoch,
    need: &slug_bzlmod_v2::RepositoryWorkspaceOperationDemand,
) -> RepositoryWorkspaceEpoch {
    let request = need.request();
    let RepositoryWorkspaceOperation::Symlink { path, target, .. } = request.operation() else {
        panic!("original invocation must demand a typed symlink")
    };
    let mut entries = request.predecessor().entries().to_vec();
    entries.push(
        RepositoryWorkspaceEntry::new(
            path.clone(),
            0o777,
            RepositoryWorkspaceNode::Link(target.as_path().to_str().unwrap().into()),
        )
        .unwrap(),
    );
    entries.sort_unstable_by(|a, b| a.path().cmp(b.path()));
    epoch
        .complete(
            request.as_ref().clone(),
            RepositoryWorkspaceResult::Unit,
            request.predecessor().mode(),
            entries,
        )
        .unwrap()
}

#[tokio::test]
async fn general_symlink_converts_own_string_path_and_label_before_exact_prefix_replay() {
    let (mut epoch, demand) = super::workspace_tests::binding(7).await;
    let snapshots = Arc::new(
        RepositorySourceSnapshotEpoch::fresh(demand.key().workspace().clone(), 7).unwrap(),
    );
    let source = r#"def implementation(ctx):
    if ctx.symlink('missing',Label('@@repo//:one')) != None: fail('symlink result')
    ctx.symlink(ctx.path('other'),ctx.path('two'))
    ctx.symlink(Label('@@repo//:third'),'three')
"#;
    for target_tail in ["missing", "other", "third"] {
        let Err(RepositoryRuleInvocationError::WorkspaceNeed(need)) = attempt(
            source,
            &epoch,
            &demand,
            snapshots.clone(),
            &SmallMap::new(),
            &SmallMap::new(),
        )
        .result
        else {
            panic!("symlink Need")
        };
        let RepositoryWorkspaceOperation::Symlink { target, source, .. } =
            need.request().operation()
        else {
            panic!("typed link")
        };
        assert!(source.is_none());
        assert!(need.declared_sources().is_empty());
        assert_eq!(
            target.as_path(),
            std::path::Path::new("/__slug_repositories__/ordinary").join(target_tail)
        );
        epoch = complete_symlink(&epoch, &need);
    }
    let completed = attempt(
        source,
        &epoch,
        &demand,
        snapshots.clone(),
        &SmallMap::new(),
        &SmallMap::new(),
    )
    .result
    .unwrap();
    assert_eq!(
        completed
            .workspace_realization
            .unwrap()
            .transcript()
            .records()
            .len(),
        3
    );
    assert!(
        attempt(
            source,
            &epoch,
            &demand,
            snapshots.clone(),
            &SmallMap::new(),
            &SmallMap::new()
        )
        .result
        .is_ok()
    );
    let changed = source.replace("'missing'", "'different'");
    assert_eq!(
        attempt(
            &changed,
            &epoch,
            &demand,
            snapshots,
            &SmallMap::new(),
            &SmallMap::new()
        )
        .result
        .err(),
        Some(RepositoryRuleInvocationError::WorkspaceReplay)
    );
}

#[tokio::test]
async fn general_source_symlink_preserves_projection_and_admits_directory_and_dangling_without_body_need()
 {
    let (mut epoch, demand) = super::workspace_tests::binding(7).await;
    let (snapshots, paths, _bodies) = builtin(7).await;
    let source = r#"def implementation(ctx):
    ctx.symlink(Label('@@bazel_tools//:MODULE.bazel'),'module')
    p=ctx.path(Label('@@bazel_tools//:MODULE.bazel'))
    ctx.symlink(p,'alias')
    ctx.symlink(str(p).replace('/MODULE.bazel',''),'directory')
    ctx.symlink(str(p).replace('/MODULE.bazel','/missing'),'dangling')
    if not ctx.path('directory').get_child('MODULE.bazel').exists: fail('directory target')
    if ctx.path('dangling').exists: fail('dangling target')
"#;
    let mut links = 0;
    loop {
        match attempt(
            source,
            &epoch,
            &demand,
            snapshots.clone(),
            &paths,
            &SmallMap::new(),
        )
        .result
        {
            Ok(completed) => {
                let realization = completed.workspace_realization.unwrap();
                assert_eq!(realization.final_state().source_bindings().len(), 1);
                assert_eq!(realization.transcript().retention().body_bytes(), 0);
                break;
            }
            Err(RepositoryRuleInvocationError::WorkspaceNeed(need)) => {
                match need.request().operation() {
                    RepositoryWorkspaceOperation::Symlink {
                        source: Some(view), ..
                    } => {
                        links += 1;
                        assert!(view.workspace_state().is_none());
                        assert!(view.repo_spec().is_none());
                        assert_eq!(need.declared_sources().len(), 1);
                        assert!(Arc::ptr_eq(
                            view.source_state().unwrap(),
                            &snapshots.states().unwrap()[0]
                        ));
                        epoch = complete_symlink(&epoch, &need);
                    }
                    RepositoryWorkspaceOperation::Exists(path) => {
                        let expected = need.request().predecessor().exists(path).unwrap();
                        epoch = epoch
                            .complete(
                                need.request().as_ref().clone(),
                                RepositoryWorkspaceResult::Exists(expected),
                                need.request().predecessor().mode(),
                                need.request().predecessor().entries().to_vec(),
                            )
                            .unwrap();
                    }
                    _ => panic!("only symlink/exists operations, no body acquisition"),
                }
            }
            Err(error) => panic!("unexpected original source link failure: {error:?}"),
        }
    }
    assert_eq!(links, 4);
    assert!(
        attempt(source, &epoch, &demand, snapshots, &paths, &SmallMap::new())
            .result
            .is_ok()
    );
}

#[tokio::test]
async fn general_source_string_symlink_cannot_use_available_but_undeclared_or_future_source() {
    let (epoch, demand) = super::workspace_tests::binding(7).await;
    let (snapshots, paths, _bodies) = builtin(7).await;
    for source in [
        "def implementation(ctx):\n    ctx.symlink('/__slug_repositories__/builtin/MODULE.bazel','link')\n",
        "def implementation(ctx):\n    ctx.symlink('/__slug_repositories__/builtin/MODULE.bazel','link')\n    ctx.path(Label('@@bazel_tools//:MODULE.bazel'))\n",
        "def implementation(ctx):\n    p=ctx.path(Label('@@bazel_tools//:MODULE.bazel'))\n    ctx.symlink('../builtin/MODULE.bazel','link')\n",
        "def implementation(ctx):\n    p=ctx.path(Label('@@bazel_tools//:MODULE.bazel'))\n    ctx.symlink('/__slug_repositories__/builtin/../builtin/MODULE.bazel','link')\n",
    ] {
        assert_eq!(
            attempt(
                source,
                &epoch,
                &demand,
                snapshots.clone(),
                &paths,
                &SmallMap::new()
            )
            .result
            .err(),
            Some(RepositoryRuleInvocationError::WorkspaceUnsupported)
        );
    }
}

#[tokio::test]
async fn general_symlink_target_conversion_precedes_destination_and_stale_source_refuses() {
    let (epoch, demand) = super::workspace_tests::binding(7).await;
    let (snapshots, paths, _bodies) = builtin(7).await;
    let address = RepositoryLabelPathAddress::from_label(
        &CanonicalLabel::parse("@@bazel_tools//:MODULE.bazel").unwrap(),
    );
    assert_eq!(attempt("def implementation(ctx):\n    ctx.symlink(Label('@@bazel_tools//:MODULE.bazel'),False)\n",&epoch,&demand,snapshots.clone(),&SmallMap::new(),&SmallMap::new()).result.err(),Some(RepositoryRuleInvocationError::LabelPathNeed(address)));
    assert_eq!(attempt("def implementation(ctx):\n    ctx.symlink(False,Label('@@bazel_tools//:MODULE.bazel'))\n",&epoch,&demand,snapshots.clone(),&SmallMap::new(),&SmallMap::new()).result.err(),Some(RepositoryRuleInvocationError::SymlinkArgument));
    assert_eq!(attempt("def implementation(ctx):\n    ctx.symlink(Label('@@bazel_tools//:MODULE.bazel'),False)\n",&epoch,&demand,snapshots,&paths,&SmallMap::new()).result.err(),Some(RepositoryRuleInvocationError::PathArgument));
    let (foreign, _, _) = builtin(8).await;
    assert_eq!(attempt("def implementation(ctx):\n    ctx.symlink(Label('@@bazel_tools//:MODULE.bazel'),'link')\n",&epoch,&demand,foreign,&paths,&SmallMap::new()).result.err(),Some(RepositoryRuleInvocationError::WorkspaceReplay));
}

#[tokio::test]
async fn general_source_alias_read_template_and_delete_use_logical_workspace_operations() {
    let (mut epoch, demand) = super::workspace_tests::binding(7).await;
    let (snapshots, paths, bodies) = builtin(7).await;
    let body = bodies.values().next().unwrap().clone();
    let source = r#"def implementation(ctx):
    ctx.symlink(Label('@@bazel_tools//:MODULE.bazel'),'alias')
    if ctx.read(ctx.path('alias')) != ctx.read(Label('@@bazel_tools//:MODULE.bazel')): fail('alias read')
    ctx.template('copy',ctx.path('alias'),{},False)
    if not ctx.delete('alias'): fail('unlink')
    if ctx.path('alias').exists: fail('link survived')
"#;
    loop {
        match attempt(source, &epoch, &demand, snapshots.clone(), &paths, &bodies).result {
            Ok(completed) => {
                let realization = completed.workspace_realization.unwrap();
                assert!(realization.final_state().source_bindings().is_empty());
                assert!(realization.final_state().entry("alias").is_none());
                assert!(
                    matches!(realization.final_state().entry("copy").unwrap().node(),RepositoryWorkspaceNode::File(actual) if actual==&body)
                );
                let mut history_sources = 0;
                realization.visit_read_views(&mut |_| history_sources += 1);
                assert!(history_sources > 0);
                break;
            }
            Err(RepositoryRuleInvocationError::WorkspaceNeed(need)) => {
                if matches!(
                    need.request().operation(),
                    RepositoryWorkspaceOperation::Symlink { .. }
                ) {
                    epoch = complete_symlink(&epoch, &need);
                    continue;
                }
                let mut entries = need.request().predecessor().entries().to_vec();
                let result = match need.request().operation() {
                    RepositoryWorkspaceOperation::Read(path) => {
                        assert_eq!(path.as_str(), "alias");
                        RepositoryWorkspaceResult::Read(body.clone())
                    }
                    RepositoryWorkspaceOperation::Template {
                        path,
                        body: rendered,
                        mode,
                        ..
                    } => {
                        assert_eq!(rendered, &body);
                        entries.push(
                            RepositoryWorkspaceEntry::new(
                                path.clone(),
                                *mode,
                                RepositoryWorkspaceNode::File(rendered.clone()),
                            )
                            .unwrap(),
                        );
                        RepositoryWorkspaceResult::Unit
                    }
                    RepositoryWorkspaceOperation::Delete(path) => {
                        assert_eq!(path.as_str(), "alias");
                        entries.retain(|entry| entry.path() != path.as_str());
                        RepositoryWorkspaceResult::Delete(true)
                    }
                    RepositoryWorkspaceOperation::Exists(path) => {
                        assert_eq!(path.as_str(), "alias");
                        RepositoryWorkspaceResult::Exists(false)
                    }
                    _ => panic!("unexpected alias operation"),
                };
                entries.sort_unstable_by(|a, b| a.path().cmp(b.path()));
                epoch = epoch
                    .complete(
                        need.request().as_ref().clone(),
                        result,
                        need.request().predecessor().mode(),
                        entries,
                    )
                    .unwrap();
            }
            Err(error) => panic!("unexpected alias failure: {error:?}"),
        }
    }
}
