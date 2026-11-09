//! DICE policy/freshness fences around the actual original-evaluation route.
use slug_bzlmod_v2::RepositoryWorkspaceEntry;
use slug_bzlmod_v2::RepositoryWorkspaceEpoch;
use slug_bzlmod_v2::RepositoryWorkspaceEpochKey;
use slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicy;
use slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicyKey;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceReservation;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits;

use super::*;
#[path = "dependency_cutoff_tests.rs"]
mod dependency_cutoff_tests;
#[path = "general_workspace_tests/observation_union_tests.rs"]
mod observation_union_tests;
async fn update(
    transaction: dice::DiceTransaction,
    policy: RepositoryWorkspaceExecutionPolicy,
    epoch: RepositoryWorkspaceEpoch,
) -> dice::DiceTransaction {
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            RepositoryWorkspaceExecutionPolicyKey::new(workspace.clone()),
            policy,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            RepositoryWorkspaceEpochKey::new(workspace.clone()),
            epoch,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace.clone(),
            },
            RepositoryMaterializationResultEpoch::new(workspace, []).unwrap(),
        )])
        .unwrap();
    updater.commit().await
}
fn empty(lease: u64) -> RepositoryWorkspaceEpoch {
    RepositoryWorkspaceEpoch::fresh(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        lease,
        RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024).unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn original_general_policy_invalidates_disabled_prepared_result_and_fresh_zero_execute_commands()
 {
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut transaction = transaction_untracked(&dice, MODULE, EXTENSION, true).await;
    let owner = owner(&mut transaction).await;
    let key = HostSelectedRepositoryFileEffectKey::new(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        owner,
        0,
    );
    let SourcePreparationOutcome::Complete(legacy) = transaction.compute(&key).await.unwrap()
    else {
        panic!("Disabled original must complete")
    };
    assert_eq!(legacy.as_ref().as_ref().unwrap().plan().effects().len(), 2);
    let mut epoch = empty(71);
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        epoch.clone(),
    )
    .await;
    let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
        panic!("Enabled must not reuse old authored plan")
    };
    let demand = needs.workspace_reservations().values().next().unwrap();
    assert!(needs.repository_executes().is_empty());
    let reservation = Arc::new(
        RepositoryWorkspaceReservation::new(
            demand.key().workspace().clone(),
            demand.key().owner().clone(),
            demand.key().repository().clone(),
            demand.key().ordinal(),
            demand.repo_spec().clone(),
            NormalizedAbsolutePath::new("/__slug_repositories__/original-policy").unwrap(),
            PathObservationInstanceId::new(81),
            71,
            RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024)
                .unwrap(),
        )
        .unwrap(),
    );
    epoch = epoch.reserve(reservation, 0o755).unwrap();
    for index in 0..2 {
        transaction = update(
            transaction,
            RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
            epoch.clone(),
        )
        .await;
        let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
            panic!("new original File must suspend")
        };
        assert!(
            needs.repository_executes().is_empty()
                && needs.repository_filesystem_operations().is_empty()
        );
        let demand = needs.workspace_operations().values().next().unwrap();
        let request = demand.request();
        assert_eq!(request.index(), index);
        let RepositoryWorkspaceOperation::File { path, body, mode } = request.operation() else {
            panic!("actual original File operation required")
        };
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
    }
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        epoch,
    )
    .await;
    let SourcePreparationOutcome::Complete(general) = transaction.compute(&key).await.unwrap()
    else {
        panic!("exact observed prefix must complete")
    };
    let general = general.as_ref().as_ref().unwrap();
    assert_eq!(
        general
            .workspace_realization()
            .unwrap()
            .transcript()
            .records()
            .len(),
        2
    );
    assert!(general.plan().effects().is_empty());
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        empty(72),
    )
    .await;
    assert!(
        matches!(transaction.compute(&key).await.unwrap(),SourcePreparationOutcome::Need(needs) if needs.workspace_reservations().len()==1)
    );
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        RepositoryWorkspaceEpoch::revoked(),
    )
    .await;
    assert!(
        matches!(transaction.compute(&key).await.unwrap(),SourcePreparationOutcome::Complete(result) if result.is_err())
    );
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::Disabled,
        RepositoryWorkspaceEpoch::revoked(),
    )
    .await;
    let SourcePreparationOutcome::Complete(restored) = transaction.compute(&key).await.unwrap()
    else {
        panic!("Disabled semantic policy should restore old path")
    };
    assert_eq!(
        restored.as_ref().as_ref().unwrap().plan().effects().len(),
        2
    );
}

#[tokio::test]
async fn disabled_original_execute_ignores_completed_legacy_epoch_and_policy_changes_invalidate() {
    const SOURCE: &str = r#"def unrelated_original(ctx):
    ctx.file('tools/cpp/empty.cc','int main() {}',executable=True)
    completed=ctx.execute([ctx.which('gcc'),'-v'])
    if completed.stdout != 'legacy bytes': fail('completed legacy result differs')
    ctx.file('forbidden-replay-continuation','legacy replay',executable=False)
repo=repository_rule(implementation=unrelated_original)
def impl(ctx):
    repo(name='first')
ext=module_extension(implementation=impl)
"#;
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut transaction = transaction_untracked(&dice, MODULE, SOURCE, true).await;
    let selected_owner = owner(&mut transaction).await;
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let SourcePreparationOutcome::Complete(certificate) = transaction
        .compute(&HostSelectedExtensionOwnerCertificateKey::new(
            workspace.clone(),
            selected_owner.clone(),
        ))
        .await
        .unwrap()
    else {
        panic!("actual selected owner certificate")
    };
    let canonical_name = certificate
        .as_ref()
        .as_ref()
        .unwrap()
        .repository(0)
        .unwrap()
        .spec_parts()
        .0
        .clone();
    let snapshot = environment_snapshot(&[("PATH", "/usr/bin")]);
    let frontier = environment_frontier(&["PATH"]);
    let mut prefix = GeneratedRepositoryFileEffectPlan::builder();
    prefix
        .push(
            "tools/cpp/empty.cc".into(),
            Arc::from(&b"int main() {}"[..]),
            true,
        )
        .unwrap();
    // All fields match the actual original invocation's former legacy request:
    // canonical certificate name, owner/ordinal/index, Which path, default
    // options/environment and the exact ordered executable source-file prefix.
    let request = Arc::new(RepositoryExecuteRequest::new(
        slug_bzlmod_v2::RepositoryExecuteCallId::new(selected_owner.clone(), 0, 0),
        canonical_name.as_str().into(),
        Arc::from([
            slug_bzlmod_v2::RepositoryExecuteArg::Which(
                HostRepositoryLabelPathValue::new(
                    NormalizedAbsolutePath::new("/usr/bin/gcc").unwrap(),
                    PathObservationNamespace::Host,
                )
                .unwrap(),
            ),
            slug_bzlmod_v2::RepositoryExecuteArg::String("-v".into()),
        ]),
        snapshot.clone(),
        prefix.finish(),
    ));
    let completed = RepositoryExecuteTranscript::empty()
        .append(RepositoryExecuteRecord::new(
            request,
            RepositoryExecuteResult::new(0, Arc::from(&b"legacy bytes"[..]), Arc::from([])),
        ))
        .unwrap();
    let legacy = RepositoryExecuteEpoch::active(900, completed);
    transaction = with_execute_epoch(
        transaction,
        workspace.clone(),
        snapshot.clone(),
        frontier.clone(),
        legacy.clone(),
    )
    .await;
    let key = HostSelectedRepositoryFileEffectKey::new(workspace.clone(), selected_owner, 0);
    // Prepare ordinary Which metadata only. No process Need or result is
    // allowed: the otherwise replayable completed record cannot authorize it.
    for _ in 0..8 {
        let outcome = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Need(need) = outcome else {
            break;
        };
        assert!(need.repository_executes().is_empty());
        let demands = need
            .path_observations()
            .expect("only ordinary Which path facts");
        let current = transaction.compute(&PathObservationEpochKey).await.unwrap();
        let mut rows = current
            .observations()
            .iter()
            .map(|(d, r)| (d.dupe(), r.dupe()))
            .collect::<Vec<_>>();
        for demand in demands.demands() {
            assert_eq!(demand.operation(), PathObservationOperation::Lstat);
            let name = demand.path().as_path();
            let kind = if name == Path::new("/usr/bin/gcc") {
                PathNodeKind::RegularFile
            } else {
                assert!(
                    ["/", "/usr", "/usr/bin"]
                        .iter()
                        .any(|path| name == Path::new(path))
                );
                PathNodeKind::Directory
            };
            rows.retain(|(old, _)| old != demand);
            rows.push((
                demand.dupe(),
                Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                    PathLstat::new(kind, 1, 2, 3, 4, 0o755),
                ))),
            ));
        }
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::from_shared(rows).unwrap(),
            )])
            .unwrap();
        transaction = with_execute_epoch(
            updater.commit().await,
            workspace.clone(),
            snapshot.clone(),
            frontier.clone(),
            legacy.clone(),
        )
        .await;
    }
    let check_disabled = |outcome: SourcePreparationOutcome<EffectResult>| {
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("Disabled Execute must not create a process Need")
        };
        assert!(
            matches!(value.as_ref(),Err(HostSelectedRepositoryFileEffectError::Invocation {message,..})
            if message.as_str()=="repository_ctx.execute requires the general repository workspace capability; it is disabled")
        );
    };
    check_disabled(transaction.compute(&key).await.unwrap());
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        empty(901),
    )
    .await;
    assert!(
        matches!(transaction.compute(&key).await.unwrap(),SourcePreparationOutcome::Need(need)
        if need.workspace_reservations().len()==1 && need.repository_executes().is_empty())
    );
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::Disabled,
        RepositoryWorkspaceEpoch::revoked(),
    )
    .await;
    check_disabled(transaction.compute(&key).await.unwrap());
}

#[tokio::test]
async fn general_template_enumerated_environment_invalidates_same_completed_epoch_a_b_a() {
    const SOURCE: &str = r#"
def ordinary_render(ctx):
    ctx.file('source', 'value=%{x}\n', executable=False)
    declared = ctx.getenv('DECLARED')
    env = ctx.os.environ
    ctx.template('out', ctx.path('source'),
                 {'%{x}': declared + '|' + env['EXTRA'] + '|' + env['HOMEBREW_RUBY_PATH']},
                 executable=False)
repo = repository_rule(implementation=ordinary_render, environ=['DECLARED'])
def impl(ctx):
    repo(name='first')
ext = module_extension(implementation=impl)
"#;
    // This update changes only the full-snapshot DICE cell. In particular it
    // never reinjects policy, declared-name cells, or a workspace epoch/fence.
    async fn environment_only(
        transaction: dice::DiceTransaction,
        workspace: &NormalizedAbsolutePath,
        snapshot: RepositoryEnvironmentSnapshot,
    ) -> dice::DiceTransaction {
        let mut data = UserComputationData {
            cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        data.data.set(RepositoryHostInputTransaction::new(
            snapshot.clone(),
            environment_frontier(&["DECLARED"]),
        ));
        data.data.set(SelectedExtensionDownloadTransaction::empty(
            workspace.clone(),
        ));
        data.data.set(RepositoryExecuteEpoch::revoked());
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                RepositoryEnvironmentSnapshotKey::new(workspace.clone()),
                snapshot,
            )])
            .unwrap();
        updater.commit_with_data(data).await
    }
    async fn complete_prefix(
        mut transaction: dice::DiceTransaction,
        key: &HostSelectedRepositoryFileEffectKey,
        mut epoch: RepositoryWorkspaceEpoch,
    ) -> (
        dice::DiceTransaction,
        RepositoryWorkspaceEpoch,
        EffectResult,
    ) {
        let mut operations = 0;
        loop {
            transaction = update(
                transaction,
                RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
                epoch.clone(),
            )
            .await;
            match transaction.compute(key).await.unwrap() {
                SourcePreparationOutcome::Complete(result) => {
                    assert_eq!(operations, 3, "File, Read, Template must all replay");
                    assert!(result.is_ok(), "original General invocation: {result:?}");
                    return (transaction, epoch, result);
                }
                SourcePreparationOutcome::Need(needs) => {
                    assert!(operations < 3);
                    assert_eq!(needs.workspace_operations().len(), 1);
                    let request = needs
                        .workspace_operations()
                        .values()
                        .next()
                        .unwrap()
                        .request();
                    assert_eq!(request.index(), operations);
                    let mut entries = request.predecessor().entries().to_vec();
                    let result = match request.operation() {
                        RepositoryWorkspaceOperation::File { path, body, mode }
                        | RepositoryWorkspaceOperation::Template {
                            path, body, mode, ..
                        } => {
                            entries.push(
                                RepositoryWorkspaceEntry::new(
                                    path.clone(),
                                    *mode,
                                    RepositoryWorkspaceNode::File(body.clone()),
                                )
                                .unwrap(),
                            );
                            RepositoryWorkspaceResult::Unit
                        }
                        RepositoryWorkspaceOperation::Read(path) => {
                            let RepositoryWorkspaceNode::File(body) =
                                request.predecessor().entry(path).unwrap().node()
                            else {
                                panic!("template must read its own source file")
                            };
                            RepositoryWorkspaceResult::Read(body.clone())
                        }
                        operation => panic!("unexpected original operation: {operation:?}"),
                    };
                    entries.sort_by(|a, b| a.path().cmp(b.path()));
                    epoch = epoch
                        .complete(
                            request.as_ref().clone(),
                            result,
                            request.predecessor().mode(),
                            entries,
                        )
                        .unwrap();
                    operations += 1;
                }
            }
        }
    }
    fn output(result: &EffectResult) -> (Vec<u8>, Arc<str>) {
        let realization = result
            .as_ref()
            .as_ref()
            .unwrap()
            .workspace_realization()
            .unwrap();
        let RepositoryWorkspaceNode::File(body) =
            realization.final_state().entry("out").unwrap().node()
        else {
            panic!("rendered output must be a file")
        };
        let mut bytes = vec![0; body.len()];
        assert!(body.copy_into(0, &mut bytes));
        (bytes, realization.source_identity().clone())
    }
    fn replay_refused(outcome: SourcePreparationOutcome<EffectResult>) {
        assert!(matches!(outcome, SourcePreparationOutcome::Complete(result)
            if matches!(result.as_ref(), Err(HostSelectedRepositoryFileEffectError::Invocation { message, .. })
                if message.as_str() == "repository workspace transcript differs from this invocation")));
    }
    let snapshot = |extra| {
        environment_snapshot(&[
            ("DECLARED", "stable"),
            ("EXTRA", extra),
            ("HOMEBREW_RUBY_PATH", "/constant/ruby"),
        ])
    };
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let base = base_transaction_untracked(&dice, MODULE, SOURCE, true).await;
    let mut transaction = with_host_inputs_for(
        base,
        workspace.clone(),
        None,
        RepositoryPlatform::new("linux", "x86_64"),
        snapshot("A"),
        environment_frontier(&["DECLARED"]),
    )
    .await;
    let selected_owner = owner(&mut transaction).await;
    let key = HostSelectedRepositoryFileEffectKey::new(workspace.clone(), selected_owner, 0);
    let fresh = empty(911);
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        fresh.clone(),
    )
    .await;
    let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
        panic!("actual original must first request a reservation")
    };
    let demand = needs.workspace_reservations().values().next().unwrap();
    let reservation = Arc::new(
        RepositoryWorkspaceReservation::new(
            workspace.clone(),
            demand.key().owner().clone(),
            demand.key().repository().clone(),
            demand.key().ordinal(),
            demand.repo_spec().clone(),
            NormalizedAbsolutePath::new("/__slug_repositories__/enumerated-template").unwrap(),
            PathObservationInstanceId::new(912),
            911,
            RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024)
                .unwrap(),
        )
        .unwrap(),
    );
    let reserved = fresh.reserve(reservation, 0o755).unwrap();
    let (mut transaction, epoch_a, a) = complete_prefix(transaction, &key, reserved.clone()).await;
    assert_eq!(output(&a).0, b"value=stable|A|/constant/ruby\n");
    let SourcePreparationOutcome::Complete(warm) = transaction.compute(&key).await.unwrap() else {
        panic!("same snapshot and completed epoch must stay complete")
    };
    assert_eq!(output(&warm), output(&a));

    transaction = environment_only(transaction, &workspace, snapshot("B")).await;
    assert_eq!(
        transaction
            .compute(&RepositoryWorkspaceEpochKey::new(workspace.clone()))
            .await
            .unwrap(),
        epoch_a
    );
    replay_refused(transaction.compute(&key).await.unwrap());
    let (mut transaction, epoch_b, b) = complete_prefix(transaction, &key, reserved).await;
    assert_eq!(output(&b).0, b"value=stable|B|/constant/ruby\n");
    assert_ne!(output(&a).1, output(&b).1);

    transaction = environment_only(transaction, &workspace, snapshot("A")).await;
    assert_eq!(
        transaction
            .compute(&RepositoryWorkspaceEpochKey::new(workspace.clone()))
            .await
            .unwrap(),
        epoch_b
    );
    replay_refused(transaction.compute(&key).await.unwrap());
    transaction = update(
        transaction,
        RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1,
        epoch_a,
    )
    .await;
    let SourcePreparationOutcome::Complete(restored) = transaction.compute(&key).await.unwrap()
    else {
        panic!("restoring the exact A epoch must complete again")
    };
    assert_eq!(output(&restored), output(&a));
}
