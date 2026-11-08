//! Actual original evaluation against immutable accepted-prefix observations.
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryWorkspaceEntry;
use slug_bzlmod_v2::RepositoryWorkspaceEpoch;
use slug_bzlmod_v2::RepositoryWorkspaceMode;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceReservation;
use slug_bzlmod_v2::RepositoryWorkspaceReservationDemand;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits;
use slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey;

use super::*;

pub(super) async fn binding(
    lease: u64,
) -> (
    RepositoryWorkspaceEpoch,
    RepositoryWorkspaceReservationDemand,
) {
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let owner = crate::module_extension_repository_file_effect::workspace_owner_for_test().await;
    let repository = slug_identity_v2::CanonicalRepoName::new("repo").unwrap();
    let spec = Arc::new(slug_bzlmod_v2::RepoSpec {
        rule_id: slug_bzlmod_v2::RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@//:repository_rule.bzl").unwrap(),
            rule_name: "ordinary".into(),
        },
        attributes: Arc::new(SmallMap::new()),
    });
    let limits =
        RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024).unwrap();
    let key = Arc::new(
        RepositoryWorkspaceStableKey::new(workspace.clone(), owner.clone(), repository.clone(), 0)
            .unwrap(),
    );
    let reservation = Arc::new(
        RepositoryWorkspaceReservation::new(
            workspace.clone(),
            owner,
            repository,
            0,
            spec.clone(),
            NormalizedAbsolutePath::new("/__slug_repositories__/ordinary").unwrap(),
            slug_workspace_v2::PathObservationInstanceId::new(lease),
            lease,
            limits,
        )
        .unwrap(),
    );
    let demand = RepositoryWorkspaceReservationDemand::new(key, spec).unwrap();
    (
        RepositoryWorkspaceEpoch::fresh(workspace, lease, limits)
            .unwrap()
            .reserve(reservation, 0o755)
            .unwrap(),
        demand,
    )
}
fn attempt(
    source: &str,
    epoch: &RepositoryWorkspaceEpoch,
    demand: &RepositoryWorkspaceReservationDemand,
) -> RepositoryRuleInvocationAttempt {
    attempt_selected(
        source,
        epoch,
        demand,
        &SmallMap::new(),
        RepositoryMaterializationResultEpoch::new(demand.key().workspace().clone(), []).unwrap(),
    )
}
fn attempt_selected(
    source: &str,
    epoch: &RepositoryWorkspaceEpoch,
    demand: &RepositoryWorkspaceReservationDemand,
    paths: &PreparedRepositoryLabelPaths,
    selected: RepositoryMaterializationResultEpoch,
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
    .with_workspace(epoch.clone(), demand.clone(), selected)
    .unwrap();
    invoke_repository_rule_with_host_paths(
        implementation,
        &manifest,
        paths,
        &SmallMap::new(),
        &SmallMap::new(),
        &SmallMap::new(),
        input,
        RepositoryPlatform::new("linux", "x86_64"),
        RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
            RepositoryEnvironmentEntry::new("DROP", "old"),
        ]))
        .unwrap(),
        None,
    )
}
fn entry(path: &str, mode: u16, node: RepositoryWorkspaceNode) -> RepositoryWorkspaceEntry {
    RepositoryWorkspaceEntry::new(path.into(), mode, node).unwrap()
}
#[tokio::test]
async fn general_original_replays_nonzero_state_reads_template_delete_and_ordered_output() {
    let source = r#"
def implementation(ctx):
    print("before")
    root = ctx.path("")
    if str(root) != "/__slug_repositories__/ordinary": fail("reserved root")
    ctx.file(root.get_child("seed"), "seed", False)
    result = ctx.execute(["arbitrary", "literal", ctx.path("seed")], 600, {"EMPTY":"", "DROP":None}, False, "work")
    if type(result) != "exec_result" or result.return_code != 7 or result.stdout != "\xffout": fail("result")
    print("after")
    if not ctx.path("process").exists: fail("process write missing")
    if ctx.read(Label("@@repo//:process")) != "\xffvalue": fail("process read")
    ctx.template(Label("@@repo//:copy"),ctx.path("process"),{"value":"rendered"},False)
    if ctx.read("copy") != "\xffrendered": fail("template did not read current state")
    if not ctx.delete(ctx.path("seed")): fail("delete")
    if ctx.path("seed").exists: fail("deleted file remains")
    again = ctx.execute(["different",ctx.path("copy")])
    if again.return_code != 0: fail("second result")
"#;
    let (mut epoch, demand) = binding(7).await;
    let mut executes = 0;
    let complete = loop {
        let current = attempt(source, &epoch, &demand);
        match current.result {
            Ok(invocation) => break (invocation, current.general_events.unwrap()),
            Err(RepositoryRuleInvocationError::WorkspaceNeed(need)) => {
                let request = need.request();
                let mut rows = request.predecessor().entries().to_vec();
                let mut mode = request.predecessor().mode();
                let result = match request.operation() {
                    RepositoryWorkspaceOperation::File { path, body, mode }
                    | RepositoryWorkspaceOperation::Template {
                        path, body, mode, ..
                    } => {
                        rows.push(entry(
                            path,
                            *mode,
                            RepositoryWorkspaceNode::File(body.clone()),
                        ));
                        RepositoryWorkspaceResult::Unit
                    }
                    RepositoryWorkspaceOperation::Read(path) => {
                        let RepositoryWorkspaceNode::File(body) =
                            request.predecessor().entry(path).unwrap().node()
                        else {
                            panic!("file expected")
                        };
                        RepositoryWorkspaceResult::Read(body.clone())
                    }
                    RepositoryWorkspaceOperation::Exists(path) => {
                        RepositoryWorkspaceResult::Exists(
                            request.predecessor().entry(path).is_some(),
                        )
                    }
                    RepositoryWorkspaceOperation::Delete(path) => {
                        let before = rows.len();
                        rows.retain(|entry| entry.path() != path.as_str());
                        RepositoryWorkspaceResult::Delete(rows.len() != before)
                    }
                    RepositoryWorkspaceOperation::Execute(operation) => {
                        mode = RepositoryWorkspaceMode::Process;
                        if executes == 0 {
                            assert_eq!(operation.timeout_seconds(), 600);
                            assert_eq!(
                                operation.working_directory().path_str(),
                                "/__slug_repositories__/ordinary/work"
                            );
                            assert!(matches!(
                                operation.overrides().get("DROP"),
                                Some(slug_bzlmod_v2::RepositoryWorkspaceEnvironmentValue::Remove)
                            ));
                            rows.push(entry(
                                "process",
                                0o644,
                                RepositoryWorkspaceNode::File(Arc::from(&b"\xffvalue"[..])),
                            ));
                            rows.push(entry("work", 0o755, RepositoryWorkspaceNode::Directory));
                        } else {
                            assert!(request.predecessor().entry("seed").is_none());
                            assert!(request.predecessor().entry("copy").is_some());
                        }
                        executes += 1;
                        RepositoryWorkspaceResult::Execute(
                            slug_bzlmod_v2::RepositoryExecuteResult::new(
                                if executes == 1 { 7 } else { 0 },
                                Arc::from(&b"\xffout"[..]),
                                Arc::from(&b"err"[..]),
                            ),
                        )
                    }
                    _ => panic!("unexpected operation"),
                };
                rows.sort_by(|a, b| a.path().cmp(b.path()));
                epoch = epoch
                    .complete(request.as_ref().clone(), result, mode, rows)
                    .unwrap();
            }
            Err(error) => panic!("unexpected original failure: {error:?}"),
        }
    };
    assert_eq!(executes, 2);
    let realized = complete.0.workspace_realization.unwrap();
    assert_eq!(realized.transcript().records().len(), 10);
    assert_eq!(complete.1.events().len(), 3);
    assert!(
        matches!(&complete.1.events()[0],slug_events_v2::EvaluationEvent::StarlarkPrint { text,.. } if text=="before")
    );
    assert!(
        matches!(&complete.1.events()[1],slug_events_v2::EvaluationEvent::RepositoryExecuteOutput { stdout,stderr,.. } if stdout.as_ref()==b"\xffout" && stderr.as_ref()==b"err")
    );
    assert!(
        matches!(&complete.1.events()[2],slug_events_v2::EvaluationEvent::StarlarkPrint { text,.. } if text=="after")
    );
    // Full replay never creates a new operation, and a branch cannot ignore a tail.
    assert!(attempt(source, &epoch, &demand).result.is_ok());
    assert!(matches!(
        attempt("def implementation(ctx):\n    pass\n", &epoch, &demand).result,
        Err(RepositoryRuleInvocationError::Workspace(
            slug_bzlmod_v2::RepositoryWorkspaceError::UnusedRecords
        ))
    ));
    let (fresh, fresh_demand) = binding(8).await;
    assert!(matches!(
        attempt(source, &fresh, &fresh_demand).result,
        Err(RepositoryRuleInvocationError::WorkspaceNeed(_))
    ));
}
#[tokio::test]
async fn general_execute_validates_argv_before_timeout_and_all_environment_keys_before_values() {
    let (epoch, demand) = binding(7).await;
    for (expression, expected) in [
        (
            "ctx.execute([False], True, {False:False})",
            RepositoryRuleInvocationError::ExecuteArgument,
        ),
        (
            "ctx.execute(['tool'], True, {False:False})",
            RepositoryRuleInvocationError::ExecuteOptions,
        ),
        (
            "ctx.execute(['tool'], 600, {'A':False, False:'x'})",
            RepositoryRuleInvocationError::ExecuteOptions,
        ),
    ] {
        let source = format!("def implementation(ctx):\n    {expression}\n");
        assert_eq!(
            attempt(&source, &epoch, &demand).result.err(),
            Some(expected)
        );
    }
    let revoked = RepositoryWorkspaceEpoch::revoked();
    assert!(matches!(
        RepositoryRuleInvocationInput::new(
            "repo".into(),
            None,
            Arc::new(SmallMap::new()),
            Arc::from([])
        )
        .unwrap()
        .with_workspace(
            revoked,
            demand,
            RepositoryMaterializationResultEpoch::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                []
            )
            .unwrap()
        ),
        Err(RepositoryRuleInvocationError::Workspace(
            slug_bzlmod_v2::RepositoryWorkspaceError::Revoked
        ))
    ));
}

#[tokio::test]
async fn general_prefix_declarations_ignore_later_prepared_paths_and_refuse_unbridged_sources() {
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationKind;
    use slug_bzlmod_v2::RepositoryMaterializationRequest;
    use slug_bzlmod_v2::RepositoryMaterializationRequestId;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_bzlmod_v2::RepositoryWorkspaceTranscript;
    let (epoch, demand) = binding(7).await;
    let selected = epoch.transcripts().unwrap()[0].reservation().clone();
    let source = Arc::new(
        RepositoryWorkspaceReservation::new(
            selected.workspace().clone(),
            selected.owner().clone(),
            slug_identity_v2::CanonicalRepoName::new("dep").unwrap(),
            1,
            selected.expected_repo_spec().clone(),
            NormalizedAbsolutePath::new("/__slug_repositories__/dependency").unwrap(),
            slug_workspace_v2::PathObservationInstanceId::new(8),
            selected.lease(),
            selected.limits(),
        )
        .unwrap(),
    );
    let transcript = RepositoryWorkspaceTranscript::new(source.clone(), 0o755).unwrap();
    let realization = transcript.realize(0).unwrap();
    let source_request = Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: selected.workspace().clone(),
            canonical_repo: source.repository().clone(),
        },
        repo_spec: source.expected_repo_spec().as_ref().clone(),
        kind: RepositoryMaterializationKind::GeneralWorkspace(realization.clone()),
    });
    let result =
        RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            path_policy: slug_bzlmod_v2::RepositoryMaterializationPathPolicy::GenerationRoot,
            source_identity: realization.source_identity().clone(),
            generation_root: source.display_root().as_path().to_path_buf(),
            observation_instance: source.instance(),
        });
    let selected_epoch = RepositoryMaterializationResultEpoch::new(
        selected.workspace().clone(),
        [RepositoryMaterializationEpochEntry {
            request: source_request.clone(),
            result: result.clone(),
        }],
    )
    .unwrap();
    let label = CanonicalLabel::parse("@@dep//:data").unwrap();
    let paths = SmallMap::from_iter([(
        RepositoryLabelPathAddress::from_label(&label),
        HostRepositoryLabelPathValue::new(
            NormalizedAbsolutePath::new(source.display_root().as_path().join("data")).unwrap(),
            PathObservationNamespace::Materialization(source.instance()),
        )
        .unwrap(),
    )]);
    let original = "def implementation(ctx):\n    ctx.execute(['one'])\n    unused = ctx.path(Label('@@dep//:data'))\n    ctx.execute(['two'])\n";
    let first = attempt_selected(original, &epoch, &demand, &paths, selected_epoch.clone());
    let Err(RepositoryRuleInvocationError::WorkspaceNeed(first)) = first.result else {
        panic!("first execute needs observation")
    };
    let RepositoryWorkspaceOperation::Execute(command) = first.request().operation() else {
        panic!("execute")
    };
    assert!(command.read_views().views().is_empty());
    assert!(first.selected_entries().is_empty());
    let epoch = epoch
        .complete(
            first.request().as_ref().clone(),
            RepositoryWorkspaceResult::Execute(slug_bzlmod_v2::RepositoryExecuteResult::new(
                0,
                Arc::from([]),
                Arc::from([]),
            )),
            RepositoryWorkspaceMode::Process,
            first.request().predecessor().entries().to_vec(),
        )
        .unwrap();
    let second = attempt_selected(original, &epoch, &demand, &paths, selected_epoch);
    let Err(RepositoryRuleInvocationError::WorkspaceNeed(second)) = second.result else {
        panic!("later declaration needs second execute")
    };
    assert_eq!(second.request().index(), 1);
    let RepositoryWorkspaceOperation::Execute(command) = second.request().operation() else {
        panic!("execute")
    };
    assert_eq!(command.read_views().views().len(), 1);
    assert_eq!(second.selected_entries().len(), 1);
    assert_eq!(second.selected_entries()[0].request, source_request);
    let legacy = RepositoryMaterializationRequest {
        id: source_request.id.clone(),
        repo_spec: source_request.repo_spec.clone(),
        kind: RepositoryMaterializationKind::GeneratedFileEffects(
            GeneratedRepositoryFileEffectPlan::builder().finish(),
        ),
    };
    let legacy_epoch = RepositoryMaterializationResultEpoch::new(
        selected.workspace().clone(),
        [RepositoryMaterializationEpochEntry {
            request: Arc::new(legacy),
            result: RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    path_policy: slug_bzlmod_v2::RepositoryMaterializationPathPolicy::LegacyUnbounded,
                    source_identity: "legacy".into(),
                    generation_root: source.display_root().as_path().to_path_buf(),
                    observation_instance: source.instance(),
                },
            ),
        }],
    )
    .unwrap();
    assert!(matches!(
        attempt_selected(original, &epoch, &demand, &paths, legacy_epoch).result,
        Err(RepositoryRuleInvocationError::WorkspaceUnsupported)
    ));
}

#[tokio::test]
async fn general_replay_preserves_output_before_failure_and_never_host_opens_foreign_display() {
    let (epoch, demand) = binding(7).await;
    let original = "def implementation(ctx):\n    print('before')\n    ctx.execute(['arbitrary'],quiet=False)\n    fail('after-child')\n";
    let Err(RepositoryRuleInvocationError::WorkspaceNeed(need)) =
        attempt(original, &epoch, &demand).result
    else {
        panic!("new execute must suspend")
    };
    let epoch = epoch
        .complete(
            need.request().as_ref().clone(),
            RepositoryWorkspaceResult::Execute(slug_bzlmod_v2::RepositoryExecuteResult::new(
                0,
                Arc::from(&b"\xff"[..]),
                Arc::from(&b"\0err"[..]),
            )),
            RepositoryWorkspaceMode::Process,
            need.request().predecessor().entries().to_vec(),
        )
        .unwrap();
    let failure = attempt(original, &epoch, &demand);
    assert!(matches!(
        failure.result,
        Err(RepositoryRuleInvocationError::Evaluation(_))
    ));
    let events = failure.general_events.unwrap();
    assert_eq!(events.events().len(), 2);
    assert!(
        matches!(&events.events()[1],slug_events_v2::EvaluationEvent::RepositoryExecuteOutput { stdout,stderr,.. } if stdout.as_ref()==b"\xff" && stderr.as_ref()==b"\0err")
    );
    assert_eq!(
        events,
        attempt(original, &epoch, &demand).general_events.unwrap()
    );
    let (fresh, demand) = binding(8).await;
    let Err(RepositoryRuleInvocationError::WorkspaceNeed(next)) =
        attempt(original, &fresh, &demand).result
    else {
        panic!("fresh command must sample again")
    };
    let fresh_completed = fresh
        .complete(
            next.request().as_ref().clone(),
            RepositoryWorkspaceResult::Execute(slug_bzlmod_v2::RepositoryExecuteResult::new(
                0,
                Arc::from(&b"\xff"[..]),
                Arc::from(&b"\0err"[..]),
            )),
            RepositoryWorkspaceMode::Process,
            next.request().predecessor().entries().to_vec(),
        )
        .unwrap();
    let fresh_events = attempt(original, &fresh_completed, &demand)
        .general_events
        .unwrap();
    assert_ne!(events, fresh_events);
    assert_eq!(events.events()[0], fresh_events.events()[0]);
    for operation in [
        "ctx.path('/__slug_repositories__/foreign/data')",
        "ctx.read('/__slug_repositories__/foreign/data')",
    ] {
        let original = format!("def implementation(ctx):\n    {operation}\n");
        assert!(matches!(
            attempt(&original, &fresh, &demand).result,
            Err(RepositoryRuleInvocationError::WorkspaceUnsupported)
        ));
    }
}

#[tokio::test]
async fn general_own_labels_resolve_before_terminal_materialization_in_every_argv_position() {
    let (epoch, demand) = binding(7).await;
    let original = r#"
def implementation(ctx):
    own = Label("@@repo//pkg:data")
    if str(ctx.path(own)) != "/__slug_repositories__/ordinary/pkg/data": fail("own label root")
    ctx.file(own,"data",False)
    ctx.execute([own,ctx.path(own),"literal",own])
"#;
    let Err(RepositoryRuleInvocationError::WorkspaceNeed(first)) =
        attempt(original, &epoch, &demand).result
    else {
        panic!("own Label must produce File Need without source self-cycle")
    };
    let RepositoryWorkspaceOperation::File { path, body, mode } = first.request().operation()
    else {
        panic!("File")
    };
    assert_eq!(path, "pkg/data");
    let mut entries = first.request().predecessor().entries().to_vec();
    entries.push(entry("pkg", 0o755, RepositoryWorkspaceNode::Directory));
    entries.push(entry(
        path,
        *mode,
        RepositoryWorkspaceNode::File(body.clone()),
    ));
    let epoch = epoch
        .complete(
            first.request().as_ref().clone(),
            RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            entries,
        )
        .unwrap();
    let Err(RepositoryRuleInvocationError::WorkspaceNeed(second)) =
        attempt(original, &epoch, &demand).result
    else {
        panic!("own argv Labels must not request own terminal source")
    };
    let RepositoryWorkspaceOperation::Execute(command) = second.request().operation() else {
        panic!("Execute")
    };
    assert!(
        matches!(&command.argv()[0],slug_bzlmod_v2::RepositoryWorkspaceArgument::Label {address,resolved} if address.repo()==first.request().reservation().repository() && resolved.path_str()=="/__slug_repositories__/ordinary/pkg/data")
    );
    assert!(matches!(
        &command.argv()[1],
        slug_bzlmod_v2::RepositoryWorkspaceArgument::Path(_)
    ));
    assert!(matches!(
        &command.argv()[3],
        slug_bzlmod_v2::RepositoryWorkspaceArgument::Label { .. }
    ));
    assert!(command.read_views().views().is_empty() && second.selected_entries().is_empty());
}

#[tokio::test]
async fn general_normalized_foreign_displays_refuse_before_host_lookup_or_need() {
    let (epoch, demand) = binding(7).await;
    for expression in [
        "ctx.path('/tmp/../__slug_repositories__/v1/foreign/file')",
        "ctx.read('/tmp/../__slug_repositories__/v1/foreign/file')",
    ] {
        let source = format!("def implementation(ctx):\n    {expression}\n");
        assert!(matches!(
            attempt(&source, &epoch, &demand).result,
            Err(RepositoryRuleInvocationError::WorkspaceUnsupported)
        ));
    }
    let source = "def implementation(ctx):\n    ctx.path('/tmp').get_child('..','__slug_repositories__','v1','foreign','file')\n";
    let (_module, implementation) = implementation(source);
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
    .unwrap();
    let host = SmallMap::from_iter([(NormalizedAbsolutePath::new("/tmp").unwrap(), true)]);
    let child = invoke_repository_rule_with_host_paths(
        implementation,
        &invocation_manifest(),
        &SmallMap::new(),
        &SmallMap::new(),
        &SmallMap::new(),
        &host,
        input,
        RepositoryPlatform::new("linux", "x86_64"),
        RepositoryEnvironmentSnapshot::empty(),
        None,
    );
    assert!(matches!(
        child.result,
        Err(RepositoryRuleInvocationError::WorkspaceUnsupported)
    ));
    assert!(attempt("def implementation(ctx):\n    if ctx.path('/tmp/../__slug_repositories__/ordinary/file') != ctx.path('file'): fail('own normalized path')\n",&epoch,&demand).result.is_ok());
}

#[tokio::test]
async fn general_dirname_refuses_above_anchor_and_preserves_owned_parent() {
    let (epoch, demand) = binding(7).await;
    assert!(matches!(
        attempt(
            "def implementation(ctx):\n    ctx.path('.').dirname\n",
            &epoch,
            &demand
        )
        .result,
        Err(RepositoryRuleInvocationError::WorkspaceUnsupported)
    ));
    assert!(attempt("def implementation(ctx):\n    if ctx.path('child').dirname != ctx.path('.'): fail('owned dirname')\n",&epoch,&demand).result.is_ok());
}
