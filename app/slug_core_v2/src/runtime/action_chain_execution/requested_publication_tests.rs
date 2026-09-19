//! Selected subset publication through one requested native attempt.
use slug_build_api_v2::ActionOutputKind;

use super::*;

#[derive(Clone, Copy)]
enum PublishBehavior {
    Normal,
    LateError,
    LatePanic,
    ChangeSource,
}

struct Publisher {
    inner: RequestedFake,
    behavior: PublishBehavior,
    transfers: AtomicUsize,
    batches: Mutex<Vec<Vec<(usize, Vec<String>)>>>,
}
impl Publisher {
    fn new(workspace: &Workspace, behavior: PublishBehavior) -> Self {
        Self {
            inner: RequestedFake::new(workspace, RequestedBehavior::Normal),
            behavior,
            transfers: AtomicUsize::new(0),
            batches: Mutex::new(Vec::new()),
        }
    }
}
impl ActionChainTransport for Publisher {
    type Session = RequestedSession;
    type Staged = usize;
    type Output = RequestedOutput;
    type Error = std::io::Error;
    async fn start(
        &self,
        inputs: Arc<PreparedActionChainInputs>,
    ) -> Result<Self::Session, Self::Error> {
        self.inner.start(inputs).await
    }
    async fn stage(&self, session: &mut Self::Session, index: usize) -> Result<usize, Self::Error> {
        self.inner.stage(session, index).await
    }
    async fn execute(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: usize,
    ) -> Result<(), Self::Error> {
        self.inner.execute(session, index, staged).await
    }
    async fn finish(&self, session: Self::Session) -> Result<Self::Output, Self::Error> {
        self.inner.finish(session).await
    }
}
impl ActionChainOutputTransport for Publisher {
    async fn stage_outputs(
        &self,
        session: &mut Self::Session,
        groups: &[PlannedActionOutputStaging],
    ) -> Result<(), Self::Error> {
        self.transfers.fetch_add(1, Ordering::SeqCst);
        let plan = session.inner.inputs.plan().unwrap();
        assert_eq!(session.inner.executed.len(), plan.actions().len());
        self.batches.lock().unwrap().push(
            groups
                .iter()
                .map(|group| {
                    (
                        group.action_index(),
                        group
                            .staging()
                            .outputs()
                            .iter()
                            .map(|output| output.path().to_owned())
                            .collect(),
                    )
                })
                .collect(),
        );
        for (group_index, group) in groups.iter().enumerate() {
            let action = plan.actions()[group.action_index()].action();
            for (index, output) in group.staging().outputs().iter().enumerate() {
                assert!(action.outputs().contains(output));
                match output.kind() {
                    ActionOutputKind::File => group
                        .staging()
                        .create_file(index, "")?
                        .write_all(&session.standalone)?,
                    ActionOutputKind::Directory => {
                        group.staging().create_directory(index, "")?;
                        group.staging().create_directory(index, "empty")?;
                        group
                            .staging()
                            .create_file(index, "leaf")?
                            .write_all(&session.standalone)?;
                    }
                    kind => panic!("unexpected publication kind {kind:?}"),
                }
            }
            if group_index + 1 == groups.len() {
                match self.behavior {
                    PublishBehavior::LateError => {
                        return Err(std::io::Error::other("late requested transfer failed"));
                    }
                    PublishBehavior::LatePanic => panic!("late requested transfer panic"),
                    PublishBehavior::ChangeSource if session.inner.attempt == 0 => {
                        fs::write(self.inner.inner.root.join("standalone"), b"bbb")?
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

fn subset_workspace() -> Workspace {
    let workspace = workspace();
    fs::write(
        workspace.root.path().join("defs.bzl"),
        REQUESTED_DEFS
            .replace("SELECTED_TERMINALS", "left, right")
            .replace("default=depset([file, tree])", "default=depset([tree])"),
    )
    .unwrap();
    workspace
}

fn publish(
    workspace: &Workspace,
    targets: &[&str],
    transport: &Publisher,
) -> Result<AcceptedCommand<RequestedActionResult<RequestedOutput>>, BuildCommandError> {
    workspace
        .runtime
        .execute_and_publish_requested_actions_with_repository_environment(
            &targets
                .iter()
                .map(|target| TargetPattern::parse(target).unwrap())
                .collect::<Vec<_>>(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[format!(
                "file://{}/empty-registry",
                workspace.root.path().display()
            )],
            Default::default(),
            Default::default(),
            transport,
        )
        .map(|accepted| accepted.map_terminal(Result::unwrap))
}

fn output_root(workspace: &Workspace) -> PathBuf {
    workspace.output_path().parent().unwrap().to_owned()
}

fn write_old_outputs(workspace: &Workspace) {
    let root = output_root(workspace);
    fs::create_dir_all(root.join("tree")).unwrap();
    for path in ["left", "right", "tree/leaf", "tree/stale"] {
        fs::write(root.join(path), b"old").unwrap();
    }
}

fn assert_outputs(workspace: &Workspace, bytes: &[u8], old_tree: bool) {
    let root = output_root(workspace);
    for path in ["left", "right", "tree/leaf"] {
        assert_eq!(fs::read(root.join(path)).unwrap(), bytes, "{path}");
    }
    assert_eq!(root.join("tree/stale").exists(), old_tree);
    assert_eq!(root.join("tree/empty").is_dir(), !old_tree);
    assert!(
        !root.join("file").exists(),
        "unselected cooutput stays absent"
    );
    assert!(!root.join("seed").exists(), "intermediate stays absent");
    assert!(
        !root.join("unused").exists(),
        "unreachable action stays absent"
    );
    assert!(fs::read_dir(root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".slug-output-stage-")
    }));
}

#[test]
fn requested_publication_preserves_strict_subsets_and_producer_bindings() {
    let workspace = subset_workspace();
    write_old_outputs(&workspace);
    let transport = Publisher::new(&workspace, PublishBehavior::Normal);
    let accepted = publish(
        &workspace,
        &["//:one", "//:one_alias", "//:left", "//:one"],
        &transport,
    )
    .unwrap();
    let result = accepted.terminal_for_test();
    let plan = result.inputs().plan().unwrap();
    assert_eq!(
        plan.requested().unwrap().artifact_producers(),
        &[None, Some(2), Some(3), Some(1)]
    );
    assert_eq!(
        result
            .published_outputs()
            .iter()
            .map(|group| group.action_index())
            .collect::<Vec<_>>(),
        [2, 3, 1]
    );
    for (group, path) in result
        .published_outputs()
        .iter()
        .zip(["left", "right", "tree"])
    {
        assert_eq!(group.outputs().root(), output_root(&workspace));
        assert_eq!(group.outputs().outputs().len(), 1);
        assert_eq!(group.outputs().outputs()[0].path(), path);
    }
    assert_eq!(
        transport.batches.lock().unwrap().as_slice(),
        &[vec![
            (2, vec!["left".to_owned()]),
            (3, vec!["right".to_owned()]),
            (1, vec!["tree".to_owned()])
        ]]
    );
    assert_eq!(transport.transfers.load(Ordering::SeqCst), 1);
    assert_eq!(result.output().unwrap().inner.executed, [0, 1, 2, 3]);
    assert_outputs(&workspace, b"aaa", false);
    assert_eq!(
        fs::read(workspace.root.path().join("standalone")).unwrap(),
        b"aaa"
    );
}

#[test]
fn requested_publication_coalesces_equivalent_file_write_owners_and_aliases() {
    let workspace = workspace();
    let defs = fs::read_to_string(workspace.root.path().join("defs.bzl")).unwrap();
    fs::write(workspace.root.path().join("defs.bzl"), defs + r#"
def _shared(ctx):
    out = ctx.actions.declare_file('shared')
    ctx.actions.write(out, 'same')
    return [DefaultInfo(files=depset([out])), OutputGroupInfo(_validation=depset([ctx.attr.standalone[DefaultInfo].files.to_list()[0]]))]
shared = rule(implementation=_shared, attrs={'standalone':attr.label(allow_files=True)})
"#).unwrap();
    let build = fs::read_to_string(workspace.root.path().join("BUILD.bazel")).unwrap();
    fs::write(workspace.root.path().join("BUILD.bazel"), build + "\nload(':defs.bzl', 'shared')\nshared(name='shared_a', standalone='standalone')\nshared(name='shared_b', standalone='standalone')\nalias(name='shared_alias', actual='shared_b')\n").unwrap();
    let transport = Publisher::new(&workspace, PublishBehavior::Normal);
    let accepted = publish(
        &workspace,
        &["//:shared_a", "//:shared_b", "//:shared_alias"],
        &transport,
    )
    .unwrap();
    let result = accepted.terminal_for_test();
    let plan = result.inputs().plan().unwrap();
    let requested = plan.requested().unwrap();
    assert_eq!(requested.selection().targets().len(), 3);
    assert_eq!(requested.selection().artifacts().len(), 3);
    assert_eq!(requested.artifact_producers(), &[None, Some(0), Some(0)]);
    assert_ne!(
        requested.selection().artifacts()[1],
        requested.selection().artifacts()[2]
    );
    assert_eq!(result.output().unwrap().inner.executed, [0]);
    assert_eq!(result.published_outputs().len(), 1);
    assert_eq!(result.published_outputs()[0].action_index(), 0);
    assert_eq!(
        transport.batches.lock().unwrap().as_slice(),
        &[vec![(0, vec!["shared".to_owned()])]]
    );
    assert_eq!(
        fs::read(output_root(&workspace).join("shared")).unwrap(),
        b"aaa"
    );
}

#[test]
fn requested_late_transfer_failure_and_unwind_preserve_all_old_outputs() {
    let workspace = subset_workspace();
    write_old_outputs(&workspace);
    let normal = RequestedFake::new(&workspace, RequestedBehavior::Normal);
    drop(run(&workspace, &["//:one"], &normal).unwrap());
    let before = workspace.snapshot();
    for behavior in [PublishBehavior::LateError, PublishBehavior::LatePanic] {
        let transport = Publisher::new(&workspace, behavior);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            publish(&workspace, &["//:one"], &transport)
        }));
        if matches!(behavior, PublishBehavior::LatePanic) {
            assert!(result.is_err());
        } else {
            assert!(
                result
                    .unwrap()
                    .unwrap_err()
                    .to_string()
                    .contains("late requested transfer failed")
            );
        }
        assert_eq!(transport.transfers.load(Ordering::SeqCst), 1);
        assert_eq!(
            transport.inner.inner.session_drops.load(Ordering::SeqCst),
            1
        );
        assert_eq!(transport.inner.finishes.load(Ordering::SeqCst), 0);
        workspace.assert_restored(&before);
        assert_outputs(&workspace, b"old", true);
    }
}

#[test]
fn requested_transfer_source_change_retries_whole_batch_before_publication() {
    let workspace = subset_workspace();
    write_old_outputs(&workspace);
    let transport = Publisher::new(&workspace, PublishBehavior::ChangeSource);
    let accepted = publish(&workspace, &["//:one"], &transport).unwrap();
    assert_eq!(transport.inner.inner.starts.load(Ordering::SeqCst), 2);
    assert_eq!(transport.transfers.load(Ordering::SeqCst), 2);
    assert_eq!(transport.inner.inner.executions.load(Ordering::SeqCst), 8);
    assert_eq!(
        transport.inner.inner.session_drops.load(Ordering::SeqCst),
        2
    );
    assert_eq!(transport.inner.inner.output_drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        accepted.terminal_for_test().output().unwrap().standalone,
        b"bbb"
    );
    assert_outputs(&workspace, b"bbb", false);
}

#[test]
fn requested_zero_action_publication_skips_transport_and_staging() {
    let workspace = workspace();
    for targets in [
        vec![],
        vec!["//:empty"],
        vec!["//:standalone"],
        vec!["//:source_only"],
    ] {
        let transport = Publisher::new(&workspace, PublishBehavior::Normal);
        let accepted = publish(&workspace, &targets, &transport).unwrap();
        let result = accepted.terminal_for_test();
        assert!(result.output().is_none());
        assert!(result.published_outputs().is_empty());
        assert_eq!(transport.inner.inner.starts.load(Ordering::SeqCst), 0);
        assert_eq!(transport.inner.finishes.load(Ordering::SeqCst), 0);
        assert_eq!(transport.transfers.load(Ordering::SeqCst), 0);
        assert!(transport.batches.lock().unwrap().is_empty());
    }
}

#[test]
fn requested_post_publication_failure_reports_changed_outputs_without_acceptance() {
    let workspace = subset_workspace();
    write_old_outputs(&workspace);
    workspace
        .runtime
        .native_demand_sessions
        .force_next_replace_accepted_failure();
    let transport = Publisher::new(&workspace, PublishBehavior::Normal);
    let error = publish(&workspace, &["//:one"], &transport).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("after action outputs were published"),
        "{error}"
    );
    assert_eq!(transport.inner.inner.output_drops.load(Ordering::SeqCst), 1);
    assert_outputs(&workspace, b"aaa", false);
}

#[test]
fn requested_publication_routes_same_path_through_target_and_exec_producers() {
    let workspace = workspace();
    let defs = fs::read_to_string(workspace.root.path().join("defs.bzl")).unwrap();
    fs::write(workspace.root.path().join("defs.bzl"), defs + r#"
def _exec_output(ctx):
    out = ctx.actions.declare_file('configured')
    ctx.actions.write(out, 'exec')
    return [DefaultInfo(files=depset([out]))]
exec_output = rule(implementation=_exec_output)
def _configured(ctx):
    out = ctx.actions.declare_file('configured')
    ctx.actions.write(out, 'target')
    return [DefaultInfo(files=depset([out], transitive=[ctx.attr.dep[DefaultInfo].files])), OutputGroupInfo(_validation=depset([ctx.attr.standalone[DefaultInfo].files.to_list()[0]]))]
configured = rule(implementation=_configured, attrs={'dep':attr.label(cfg='exec'), 'standalone':attr.label(allow_files=True)})
"#).unwrap();
    let build = fs::read_to_string(workspace.root.path().join("BUILD.bazel")).unwrap();
    fs::write(workspace.root.path().join("BUILD.bazel"), build + "\nload(':defs.bzl', 'exec_output', 'configured')\nexec_output(name='exec_output')\nconfigured(name='configured', dep='exec_output', standalone='standalone')\n").unwrap();
    let transport = Publisher::new(&workspace, PublishBehavior::Normal);
    let accepted = publish(&workspace, &["//:configured"], &transport).unwrap();
    let result = accepted.terminal_for_test();
    let plan = result.inputs().plan().unwrap();
    let requested = plan.requested().unwrap();
    assert_eq!(plan.actions().len(), 2);
    assert_eq!(result.published_outputs().len(), 2);
    assert_ne!(
        result.published_outputs()[0].outputs().root(),
        result.published_outputs()[1].outputs().root()
    );
    for (artifact, index) in requested
        .selection()
        .artifacts()
        .iter()
        .zip(requested.artifact_producers())
    {
        let Some(index) = index else { continue };
        let producer = plan.actions()[*index].action();
        let group = result
            .published_outputs()
            .iter()
            .find(|group| group.action_index() == *index)
            .unwrap();
        let root = crate::runtime::configured_output_root(
            workspace.root.path(),
            producer
                .context()
                .owner()
                .configuration()
                .slug_configuration()
                .unwrap(),
        );
        assert_eq!(group.outputs().root(), root);
        assert_eq!(artifact.path(), "configured");
        assert_eq!(group.outputs().outputs()[0].path(), "configured");
        assert_eq!(fs::read(root.join("configured")).unwrap(), b"aaa");
    }
    let modes = plan
        .actions()
        .iter()
        .map(|step| {
            step.action().context().owner().configuration().kind()
                == slug_analysis_v2::ConfigurationKind::Exec
        })
        .collect::<Vec<_>>();
    assert!(modes.contains(&true));
    assert!(modes.contains(&false));
}
