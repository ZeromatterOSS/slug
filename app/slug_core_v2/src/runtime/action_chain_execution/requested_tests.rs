//! Requested forests reuse the selected-chain native attempt and abort owner.
use super::*;

const REQUESTED_DEFS: &str = r#"
def _impl(ctx):
    seed = ctx.actions.declare_file('seed')
    file = ctx.actions.declare_file('file')
    tree = ctx.actions.declare_directory('tree')
    left = ctx.outputs.left
    right = ctx.actions.declare_file('right')
    unused = ctx.actions.declare_file('unused')
    tool = ctx.attr.tool[DefaultInfo].files.to_list()[0]
    ctx.actions.write(seed, 'seed')
    ctx.actions.run(executable=tool, inputs=[seed, ctx.attr.input[DefaultInfo].files.to_list()[0]], outputs=[file, tree])
    ctx.actions.run(executable=tool, inputs=[file], outputs=[left])
    ctx.actions.run(executable=tool, inputs=[tree], outputs=[right])
    ctx.actions.write(unused, 'unselected')
    return [DefaultInfo(files=depset([SELECTED_TERMINALS])), OutputGroupInfo(default=depset([file, tree]), _validation=depset([ctx.attr.standalone[DefaultInfo].files.to_list()[0]]))]
stage = rule(implementation=_impl, attrs={'left':attr.output(mandatory=True), 'input':attr.label(allow_files=True), 'tool':attr.label(allow_files=True), 'standalone':attr.label(allow_files=True)})
def _source(ctx): return [DefaultInfo(files=ctx.attr.source[DefaultInfo].files)]
source_only = rule(implementation=_source, attrs={'source':attr.label(allow_files=True)})
def _empty(ctx): return []
empty = rule(implementation=_empty)
"#;

fn workspace() -> Workspace {
    requested_workspace(Workspace::new())
}

fn requested_workspace(workspace: Workspace) -> Workspace {
    fs::write(workspace.root.path().join("standalone"), b"aaa").unwrap();
    fs::write(
        workspace.root.path().join("defs.bzl"),
        REQUESTED_DEFS.replace("SELECTED_TERMINALS", "left, right"),
    )
    .unwrap();
    fs::write(
        workspace.root.path().join("BUILD.bazel"),
        r#"
load(':defs.bzl', 'stage', 'source_only', 'empty')
platform(name='platform')
exports_files(['input', 'tool', 'standalone'])
stage(name='one', left='left', input='input', tool='tool', standalone='standalone')
alias(name='one_alias', actual='one')
source_only(name='source_only', source='standalone')
empty(name='empty')
"#,
    )
    .unwrap();
    workspace
}

fn run(
    workspace: &Workspace,
    targets: &[&str],
    transport: &RequestedFake,
) -> Result<AcceptedCommand<RequestedActionResult<RequestedOutput>>, BuildCommandError> {
    workspace
        .runtime
        .execute_requested_actions_with_repository_environment(
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

#[derive(Clone, Copy)]
enum RequestedBehavior {
    Normal,
    StageStandalone,
    #[cfg(target_os = "linux")]
    StageUnrelatedWorkspaceSibling,
    RetryStandalone,
    LateError,
    LatePanic,
}

struct RequestedFake {
    inner: Fake,
    behavior: RequestedBehavior,
    finishes: AtomicUsize,
}
impl RequestedFake {
    fn new(workspace: &Workspace, behavior: RequestedBehavior) -> Self {
        Self {
            inner: Fake::new(workspace.root.path(), Behavior::Normal),
            behavior,
            finishes: AtomicUsize::new(0),
        }
    }
}
struct RequestedSession {
    inner: Session,
    standalone: Vec<u8>,
}
struct RequestedOutput {
    inner: Output,
    standalone: Vec<u8>,
}
impl ActionChainTransport for RequestedFake {
    type Session = RequestedSession;
    type Staged = usize;
    type Output = RequestedOutput;
    type Error = std::io::Error;

    async fn start(
        &self,
        inputs: crate::runtime::NativeActionChainInputs,
        args_context: &dyn slug_build_api_v2::ArgsDirectoryContext,
    ) -> Result<Self::Session, Self::Error> {
        assert!(inputs.plan().unwrap().requested().is_some());
        if self.inner.starts.load(Ordering::SeqCst) > 0 {
            assert_eq!(self.inner.session_drops.load(Ordering::SeqCst), 1);
            assert_eq!(self.inner.output_drops.load(Ordering::SeqCst), 1);
        }
        let mut standalone = Vec::new();
        let index = inputs
            .sources()
            .position(|source| source.label().target().as_str() == "standalone")
            .unwrap();
        inputs.open_source(index)?.read_to_end(&mut standalone)?;
        Ok(RequestedSession {
            inner: self.inner.start(inputs, args_context).await?,
            standalone,
        })
    }

    async fn stage(&self, session: &mut Self::Session, index: usize) -> Result<usize, Self::Error> {
        let staged = self.inner.stage(&mut session.inner, index).await?;
        if index + 1 == session.inner.inputs.plan().unwrap().actions().len() {
            match self.behavior {
                RequestedBehavior::StageStandalone => {
                    fs::write(self.inner.root.join("standalone"), b"bbb")?
                }
                #[cfg(target_os = "linux")]
                RequestedBehavior::StageUnrelatedWorkspaceSibling => {
                    change_workspace_parent(&self.inner.root, &session.inner.inputs)?;
                }
                RequestedBehavior::LatePanic => panic!("requested final stage panic"),
                _ => {}
            }
        }
        Ok(staged)
    }

    async fn execute(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: usize,
    ) -> Result<(), Self::Error> {
        self.inner
            .execute(&mut session.inner, index, staged)
            .await?;
        if index + 1 == session.inner.inputs.plan().unwrap().actions().len() {
            match self.behavior {
                RequestedBehavior::RetryStandalone if session.inner.attempt == 0 => {
                    fs::write(self.inner.root.join("standalone"), b"bbb")?
                }
                RequestedBehavior::LateError => {
                    return Err(std::io::Error::other("requested final execute failure"));
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn finish(&self, session: Self::Session) -> Result<Self::Output, Self::Error> {
        self.finishes.fetch_add(1, Ordering::SeqCst);
        Ok(RequestedOutput {
            inner: self.inner.finish(session.inner).await?,
            standalone: session.standalone,
        })
    }
}

#[cfg(target_os = "linux")]
fn change_workspace_parent(
    workspace: &Path,
    inputs: &crate::runtime::NativeActionChainInputs,
) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    use std::time::Duration;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use slug_workspace_v2::DirectoryTraversalObservation;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;

    let parent = workspace.parent().unwrap();
    // Every fixture input is inside the workspace. In particular, the parent
    // has a traversal observation and no independent directory listing.
    let mut parent_facts = inputs
        .observations()
        .observations()
        .iter()
        .filter(|(demand, _)| {
            demand.namespace() == PathObservationNamespace::Host
                && demand.path().as_path() == parent
        });
    let (_, observation) = parent_facts.next().expect("selected Host parent traversal");
    assert!(matches!(
        observation.as_ref(),
        PathObservationResult::DirectoryTraversal(PathOperationResult::Present(
            DirectoryTraversalObservation::Directory(_)
        ))
    ));
    assert!(parent_facts.next().is_none());
    let input = workspace.join("input");
    let standalone = workspace.join("standalone");
    let metadata = || {
        [parent, workspace, input.as_path(), standalone.as_path()]
            .map(|path| fs::symlink_metadata(path).unwrap())
    };
    let before = metadata();
    assert!(before[0].is_dir());
    // Get past the old ctime even on a filesystem with one-second timestamps.
    let ctime = UNIX_EPOCH
        + Duration::new(
            before[0].ctime().try_into().unwrap(),
            before[0].ctime_nsec().try_into().unwrap(),
        );
    let precision = Duration::from_secs(1);
    if let Ok(delay) = (ctime + precision).duration_since(SystemTime::now()) {
        assert!(delay <= precision);
        std::thread::sleep(delay);
    }
    let sibling = parent.join("unrelated-sibling");
    fs::create_dir(&sibling)?;
    fs::remove_dir(&sibling)?;
    let after = metadata();
    let identity = |m: &fs::Metadata| (m.dev(), m.ino(), m.mode(), m.uid(), m.gid());
    for (before, after) in before.iter().zip(&after) {
        assert_eq!(identity(before), identity(after));
    }
    assert_eq!(before[0].len(), after[0].len());
    assert!(before[0].modified()? < after[0].modified()?);
    assert!(
        (before[0].ctime(), before[0].ctime_nsec()) < (after[0].ctime(), after[0].ctime_nsec())
    );
    assert_eq!(fs::read(input)?, b"aaa");
    assert_eq!(fs::read(standalone)?, b"aaa");
    Ok(())
}

#[test]
fn requested_forest_executes_shared_producers_once_and_restores_group_bindings() {
    let workspace = workspace();
    let transport = RequestedFake::new(&workspace, RequestedBehavior::Normal);
    let targets = ["//:one", "//:one_alias", "//:left", "//:one"];
    let accepted = run(&workspace, &targets, &transport).unwrap();
    let result = accepted.terminal_for_test();
    assert!(result.published_outputs().is_empty());
    let plan = result.inputs().plan().unwrap();
    assert!(plan.selected_action().is_none());
    let requested = plan.requested().unwrap();
    assert_eq!(
        requested
            .selection()
            .targets()
            .iter()
            .map(|target| target.pattern())
            .collect::<Vec<_>>(),
        targets
    );
    assert_eq!(
        requested.artifact_producers(),
        &[None, Some(2), Some(3), Some(1), Some(1)]
    );
    assert_eq!(
        requested
            .selection()
            .artifacts()
            .iter()
            .map(|artifact| crate::runtime::configured_output::test_artifact_coordinate(artifact))
            .collect::<Vec<_>>(),
        ["standalone", "left", "right", "file", "tree"]
    );
    assert_eq!(result.output().unwrap().inner.executed, [0, 1, 2, 3]);
    assert_eq!(transport.inner.starts.load(Ordering::SeqCst), 1);
    assert_eq!(transport.inner.executions.load(Ordering::SeqCst), 4);
    assert_eq!(transport.finishes.load(Ordering::SeqCst), 1);
    assert_eq!(result.inputs().sources().len(), 3);
    assert!(
        !plan
            .actions()
            .iter()
            .flat_map(|step| step.inputs())
            .any(|input| input.artifact().path() == "standalone")
    );
    for target in requested.selection().targets() {
        let default = target
            .groups()
            .iter()
            .find(|group| group.name() == "default")
            .unwrap();
        assert_eq!(
            default.artifact_indices(),
            if target.pattern() == "//:left" {
                &[1][..]
            } else {
                &[1, 2, 3, 4][..]
            }
        );
    }
    drop(accepted);
    assert_eq!(transport.inner.output_drops.load(Ordering::SeqCst), 1);

    let mut snapshots = Vec::new();
    for selected in ["left", "right", "left"] {
        fs::write(
            workspace.root.path().join("defs.bzl"),
            REQUESTED_DEFS.replace("SELECTED_TERMINALS", selected),
        )
        .unwrap();
        let transport = RequestedFake::new(&workspace, RequestedBehavior::Normal);
        let accepted = run(&workspace, &["//:one"], &transport).unwrap();
        let result = accepted.terminal_for_test();
        let plan = result.inputs().plan().unwrap();
        let requested = plan.requested().unwrap();
        assert_eq!(result.output().unwrap().inner.executed, [0, 1, 2]);
        assert_eq!(
            plan.actions().last().unwrap().action().outputs()[0].root_relative_path(),
            selected
        );
        snapshots.push((
            requested.selection().artifacts().to_vec(),
            requested.artifact_producers().to_vec(),
            plan.actions()
                .iter()
                .map(|step| step.action().clone())
                .collect::<Vec<_>>(),
        ));
    }
    assert_ne!(snapshots[0], snapshots[1]);
    assert_eq!(snapshots[0], snapshots[2]);
    assert!(!workspace.output_path().exists());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires Linux unique statx mount IDs and effective descriptor search"]
fn requested_build_accepts_unrelated_workspace_parent_timestamp_change() {
    let parent = Workspace::fixture_dir();
    let root = tempfile::tempdir_in(parent.path()).unwrap();
    fs::File::open(parent.path())
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH)
        .unwrap();
    let workspace = requested_workspace(Workspace::with_root(root));
    let normal = RequestedFake::new(&workspace, RequestedBehavior::Normal);
    drop(run(&workspace, &["//:one"], &normal).unwrap());

    let transport = RequestedFake::new(
        &workspace,
        RequestedBehavior::StageUnrelatedWorkspaceSibling,
    );
    let accepted = run(&workspace, &["//:one"], &transport)
        .expect("expected final acceptance after the workspace parent timestamp change");
    let result = accepted.terminal_for_test();
    let output = result.output().unwrap();
    assert_eq!(output.inner.executed, [0, 1, 2, 3]);
    assert_eq!(output.inner.bytes, b"aaa");
    assert_eq!(output.standalone, b"aaa");
    assert!(result.published_outputs().is_empty());
    assert_eq!(transport.inner.starts.load(Ordering::SeqCst), 1);
    assert_eq!(transport.inner.stages.load(Ordering::SeqCst), 4);
    assert_eq!(transport.inner.executions.load(Ordering::SeqCst), 4);
    assert_eq!(transport.finishes.load(Ordering::SeqCst), 1);
    assert_eq!(transport.inner.session_drops.load(Ordering::SeqCst), 1);
}

#[test]
fn standalone_selected_source_is_checked_before_execute_and_retries_after_execute() {
    let workspace = workspace();
    let normal = RequestedFake::new(&workspace, RequestedBehavior::Normal);
    drop(run(&workspace, &["//:one"], &normal).unwrap());
    let before = workspace.snapshot();
    let transport = RequestedFake::new(&workspace, RequestedBehavior::StageStandalone);
    let error = run(&workspace, &["//:one"], &transport).unwrap_err();
    assert!(
        error.to_string().contains("changed before execution"),
        "{error}"
    );
    assert_eq!(transport.inner.executions.load(Ordering::SeqCst), 3);
    assert_eq!(transport.inner.session_drops.load(Ordering::SeqCst), 1);
    assert_eq!(transport.finishes.load(Ordering::SeqCst), 0);
    workspace.assert_restored(&before);
    fs::write(workspace.root.path().join("standalone"), b"aaa").unwrap();

    let transport = RequestedFake::new(&workspace, RequestedBehavior::RetryStandalone);
    let accepted = run(&workspace, &["//:one"], &transport).unwrap();
    assert_eq!(
        accepted.terminal_for_test().output().unwrap().standalone,
        b"bbb"
    );
    assert_eq!(transport.inner.starts.load(Ordering::SeqCst), 2);
    assert_eq!(transport.inner.executions.load(Ordering::SeqCst), 8);
    assert_eq!(transport.inner.session_drops.load(Ordering::SeqCst), 2);
    assert_eq!(transport.finishes.load(Ordering::SeqCst), 2);
    assert_eq!(transport.inner.output_drops.load(Ordering::SeqCst), 1);
    drop(accepted);
    assert_eq!(transport.inner.output_drops.load(Ordering::SeqCst), 2);
}

#[test]
fn requested_zero_action_roots_skip_every_transport_callback() {
    let workspace = workspace();
    for targets in [
        vec![],
        vec!["//:empty"],
        vec!["//:standalone"],
        vec!["//:source_only"],
        vec!["//:empty", "//:source_only", "//:standalone"],
    ] {
        let transport = RequestedFake::new(&workspace, RequestedBehavior::Normal);
        let accepted = run(&workspace, &targets, &transport).unwrap();
        let result = accepted.terminal_for_test();
        assert!(result.output().is_none());
        let plan = result.inputs().plan().unwrap();
        assert!(plan.actions().is_empty());
        assert_eq!(
            plan.requested().unwrap().selection().targets().len(),
            targets.len()
        );
        assert_eq!(
            result.inputs().sources().len(),
            usize::from(targets.iter().any(|target| *target != "//:empty"))
        );
        assert!(!result.inputs().observations().observations().is_empty());
        assert_eq!(transport.inner.starts.load(Ordering::SeqCst), 0);
        assert_eq!(transport.inner.stages.load(Ordering::SeqCst), 0);
        assert_eq!(transport.inner.executions.load(Ordering::SeqCst), 0);
        assert_eq!(transport.finishes.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn requested_late_failures_and_unwind_discard_results_and_restore_accepted_state() {
    let workspace = workspace();
    let normal = RequestedFake::new(&workspace, RequestedBehavior::Normal);
    drop(run(&workspace, &["//:one"], &normal).unwrap());
    let before = workspace.snapshot();
    for behavior in [RequestedBehavior::LateError, RequestedBehavior::LatePanic] {
        let transport = RequestedFake::new(&workspace, behavior);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(&workspace, &["//:one"], &transport)
        }));
        if matches!(behavior, RequestedBehavior::LatePanic) {
            assert!(outcome.is_err());
        } else {
            assert!(
                outcome
                    .unwrap()
                    .unwrap_err()
                    .to_string()
                    .contains("requested final execute failure")
            );
        }
        assert_eq!(transport.inner.starts.load(Ordering::SeqCst), 1);
        assert_eq!(transport.inner.session_drops.load(Ordering::SeqCst), 1);
        assert_eq!(transport.inner.output_drops.load(Ordering::SeqCst), 0);
        assert_eq!(transport.finishes.load(Ordering::SeqCst), 0);
        assert_eq!(
            transport.inner.executions.load(Ordering::SeqCst),
            if matches!(behavior, RequestedBehavior::LatePanic) {
                3
            } else {
                4
            }
        );
        workspace.assert_restored(&before);
    }
    let normal = RequestedFake::new(&workspace, RequestedBehavior::Normal);
    assert!(
        run(&workspace, &["//:one"], &normal)
            .unwrap()
            .terminal_for_test()
            .output()
            .is_some()
    );
}

#[path = "requested_publication_tests.rs"]
#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod requested_publication_tests;
