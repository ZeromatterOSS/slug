use std::fs;
use std::io::Read;
use std::io::Write;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use super::*;
#[path = "../source_staging/test_workspace.rs"]
mod fixture;

const DEFS: &str = r#"def _impl(ctx):
    seed = ctx.actions.declare_file('seed')
    file = ctx.actions.declare_file('file')
    tree = ctx.actions.declare_directory('tree')
    left = ctx.actions.declare_file('left')
    right = ctx.actions.declare_file('right')
    done = ctx.actions.declare_file('done')
    ctx.actions.write(seed, 'seed')
    ctx.actions.run(executable=ctx.attr.tool[DefaultInfo].files.to_list()[0], inputs=[seed, ctx.attr.input[DefaultInfo].files.to_list()[0]], outputs=[file, tree])
    ctx.actions.run(executable=ctx.attr.tool[DefaultInfo].files.to_list()[0], inputs=[file], outputs=[left])
    ctx.actions.run(executable=ctx.attr.tool[DefaultInfo].files.to_list()[0], inputs=[tree], outputs=[right])
    ctx.actions.run(executable=ctx.attr.tool[DefaultInfo].files.to_list()[0], inputs=[left, right], outputs=[done])
    return [DefaultInfo(files=depset([done]))]
stage = rule(implementation=_impl, attrs={'input':attr.label(allow_single_file=True), 'tool':attr.label(allow_single_file=True)})
"#;

struct Workspace {
    root: tempfile::TempDir,
    runtime: WorkspaceRuntime,
    owner: ConfiguredTargetKey,
}
impl Workspace {
    fn new() -> Self {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp737/fixtures");
        fs::create_dir_all(&parent).unwrap();
        let root = tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap();
        fixture::write(root.path());
        fs::write(root.path().join("defs.bzl"), DEFS).unwrap();
        let runtime =
            WorkspaceRuntime::new(root.path(), crate::runtime::ProcessHostOwner::native()).unwrap();
        let accepted = runtime
            .build_command_with_repository_environment(
                &[TargetPattern::parse("//:one").unwrap()],
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!("file://{}/empty-registry", root.path().display())],
                Default::default(),
                Default::default(),
            )
            .unwrap();
        let owner = accepted
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap()
            .analyses()
            .find_map(|node| {
                node.configured_target_key()
                    .filter(|key| key.label().target().as_str() == "one")
            })
            .unwrap()
            .clone();
        Self {
            root,
            runtime,
            owner,
        }
    }
    fn run(
        &self,
        action: usize,
        transport: &Fake,
    ) -> Result<AcceptedCommand<ActionChainResult<Output>>, BuildCommandError> {
        self.runtime
            .execute_action_chain_with_repository_environment(
                &[TargetPattern::parse("//:one").unwrap()],
                self.owner.clone(),
                action,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!(
                    "file://{}/empty-registry",
                    self.root.path().display()
                )],
                Default::default(),
                Default::default(),
                transport,
            )
    }
    fn publish(
        &self,
        transport: &Fake,
    ) -> Result<AcceptedCommand<ActionChainResult<Output>>, BuildCommandError> {
        self.runtime
            .execute_and_publish_action_chain_with_repository_environment(
                &[TargetPattern::parse("//:one").unwrap()],
                self.owner.clone(),
                4,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!(
                    "file://{}/empty-registry",
                    self.root.path().display()
                )],
                Default::default(),
                Default::default(),
                transport,
            )
    }
    fn output_path(&self) -> PathBuf {
        crate::runtime::configured_output_root(
            self.root.path(),
            self.owner.configuration().slug_configuration().unwrap(),
        )
        .join("done")
    }
    fn assert_stage_cleanup(&self) {
        let path = self.output_path();
        let configuration = path.parent().unwrap().parent().unwrap();
        assert!(
            fs::read_dir(configuration)
                .unwrap()
                .all(|entry| entry.unwrap().file_name() == "bin")
        );
        assert!(fs::read_dir(path.parent().unwrap()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".slug-output-stage-")
        }));
    }
    fn snapshot(&self) -> AcceptedNativeDemandSnapshot {
        self.runtime
            .native_demand_sessions
            .state
            .lock()
            .unwrap()
            .accepted
            .clone()
    }
    fn assert_restored(&self, before: &AcceptedNativeDemandSnapshot) {
        let after = self.snapshot();
        assert_eq!(after.inputs, before.inputs);
        assert_eq!(after.repository_results, before.repository_results);
        assert_eq!(after.path_observations, before.path_observations);
        assert_eq!(after.selected, before.selected);
        assert!(!self.root.path().join("done").exists());
    }
}

#[derive(Clone, Copy)]
enum Behavior {
    Normal,
    StageSource,
    StageModule,
    RetrySource,
    StageError,
    ExecuteError,
    StagePanic,
    OutputSource,
    OutputError,
    OutputPanic,
    OutputCommitFailure,
}
struct Fake {
    root: PathBuf,
    behavior: Behavior,
    starts: AtomicUsize,
    stages: AtomicUsize,
    executions: AtomicUsize,
    output_stages: AtomicUsize,
    session_drops: Arc<AtomicUsize>,
    output_drops: Arc<AtomicUsize>,
}
impl Fake {
    fn new(root: &Path, behavior: Behavior) -> Self {
        Self {
            root: root.to_owned(),
            behavior,
            starts: AtomicUsize::new(0),
            stages: AtomicUsize::new(0),
            executions: AtomicUsize::new(0),
            output_stages: AtomicUsize::new(0),
            session_drops: Arc::new(AtomicUsize::new(0)),
            output_drops: Arc::new(AtomicUsize::new(0)),
        }
    }
}
struct Session {
    inputs: Arc<PreparedActionChainInputs>,
    attempt: usize,
    executed: Vec<usize>,
    bytes: Vec<u8>,
    drops: Arc<AtomicUsize>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
struct Output {
    bytes: Vec<u8>,
    executed: Vec<usize>,
    drops: Arc<AtomicUsize>,
}
impl Drop for Output {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl ActionChainTransport for Fake {
    type Session = Session;
    type Staged = usize;
    type Output = Output;
    type Error = std::io::Error;

    async fn start(&self, inputs: Arc<PreparedActionChainInputs>) -> Result<Session, Self::Error> {
        let attempt = self.starts.fetch_add(1, Ordering::SeqCst);
        if matches!(
            self.behavior,
            Behavior::RetrySource | Behavior::OutputSource
        ) && attempt > 0
        {
            assert_eq!(self.session_drops.load(Ordering::SeqCst), attempt);
            assert_eq!(self.output_drops.load(Ordering::SeqCst), attempt);
        }
        if matches!(self.behavior, Behavior::OutputSource) && attempt > 0 {
            let plan = inputs.plan().unwrap();
            let action = plan.actions().last().unwrap().action();
            let root = crate::runtime::configured_output_root(
                &self.root,
                action
                    .context()
                    .owner()
                    .configuration()
                    .slug_configuration()
                    .unwrap(),
            );
            assert_eq!(
                fs::read(root.join(action.outputs()[0].path())).unwrap(),
                b"old"
            );
        }
        let mut bytes = Vec::new();
        if let Some(index) = inputs
            .sources()
            .position(|source| source.label().target().as_str() == "input")
        {
            inputs.open_source(index)?.read_to_end(&mut bytes)?;
        }
        Ok(Session {
            inputs,
            attempt,
            executed: Vec::new(),
            bytes,
            drops: self.session_drops.clone(),
        })
    }
    async fn stage(&self, session: &mut Session, index: usize) -> Result<usize, Self::Error> {
        self.stages.fetch_add(1, Ordering::SeqCst);
        let plan = session.inputs.plan().unwrap();
        assert_eq!(session.executed.len(), index);
        for input in plan.actions()[index].inputs() {
            if let Some(producer) = input.producer() {
                assert!(producer < index);
                assert!(session.executed.contains(&producer));
            }
        }
        match self.behavior {
            Behavior::StageSource if index == 4 => fs::write(self.root.join("input"), b"bbb")?,
            Behavior::StageModule if index == 4 => {
                let path = self.root.join("MODULE.bazel");
                fs::write(&path, fs::read_to_string(&path)? + "# downstream stage\n")?;
            }
            Behavior::StageError if index == 1 => {
                return Err(std::io::Error::other("producer stage failure"));
            }
            Behavior::StagePanic if index == 1 => panic!("producer stage panic"),
            _ => {}
        }
        Ok(index)
    }
    async fn execute(
        &self,
        session: &mut Session,
        index: usize,
        staged: usize,
    ) -> Result<(), Self::Error> {
        assert_eq!(staged, index);
        self.executions.fetch_add(1, Ordering::SeqCst);
        match self.behavior {
            Behavior::ExecuteError if index == 1 => {
                return Err(std::io::Error::other("producer execute failure"));
            }
            Behavior::RetrySource if index == 4 && session.attempt == 0 => {
                fs::write(self.root.join("input"), b"bbb")?
            }
            _ => {}
        }
        session.executed.push(index);
        Ok(())
    }
    async fn finish(&self, mut session: Session) -> Result<Output, Self::Error> {
        assert_eq!(
            session.executed.len(),
            session.inputs.plan().unwrap().actions().len()
        );
        if matches!(self.behavior, Behavior::OutputCommitFailure) {
            let plan = session.inputs.plan().unwrap();
            let action = plan.actions().last().unwrap().action();
            let root = crate::runtime::configured_output_root(
                &self.root,
                action
                    .context()
                    .owner()
                    .configuration()
                    .slug_configuration()
                    .unwrap(),
            );
            // Sealing has completed; simulate a final namespace change before
            // the synchronized publication callback. Its preflight must reject.
            fs::create_dir(root.join(action.outputs()[0].path()))?;
        }
        Ok(Output {
            bytes: std::mem::take(&mut session.bytes),
            executed: std::mem::take(&mut session.executed),
            drops: self.output_drops.clone(),
        })
    }
}

impl ActionChainOutputTransport for Fake {
    async fn stage_outputs(
        &self,
        session: &mut Session,
        staging: &ActionOutputStaging,
    ) -> Result<(), Self::Error> {
        self.output_stages.fetch_add(1, Ordering::SeqCst);
        assert_eq!(staging.outputs().len(), 1);
        let mut file = staging.create_file(0, "")?;
        file.write_all(&session.bytes)?;
        drop(file);
        match self.behavior {
            Behavior::OutputSource if session.attempt == 0 => {
                fs::write(self.root.join("input"), b"bbb")?
            }
            Behavior::OutputError => return Err(std::io::Error::other("output transfer failure")),
            Behavior::OutputPanic => panic!("output transfer panic"),
            _ => {}
        }
        Ok(())
    }
}

#[test]
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn output_download_source_change_retries_before_publication() {
    let workspace = Workspace::new();
    let path = workspace.output_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"old").unwrap();
    let transport = Fake::new(workspace.root.path(), Behavior::OutputSource);
    let accepted = workspace.publish(&transport).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"bbb");
    let published = accepted.terminal_for_test().published_outputs().unwrap();
    assert_eq!(published.root(), path.parent().unwrap());
    assert_eq!(published.outputs().len(), 1);
    assert_eq!(transport.starts.load(Ordering::SeqCst), 2);
    assert_eq!(transport.output_stages.load(Ordering::SeqCst), 2);
    assert_eq!(transport.session_drops.load(Ordering::SeqCst), 2);
    assert_eq!(transport.output_drops.load(Ordering::SeqCst), 1);
    workspace.assert_stage_cleanup();
    drop(accepted);
    assert_eq!(transport.output_drops.load(Ordering::SeqCst), 2);
}

#[test]
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn output_transfer_failure_and_unwind_preserve_old_outputs_and_cleanup() {
    let workspace = Workspace::new();
    let path = workspace.output_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"old").unwrap();
    let before = workspace.snapshot();
    for behavior in [Behavior::OutputError, Behavior::OutputPanic] {
        let transport = Fake::new(workspace.root.path(), behavior);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            workspace.publish(&transport)
        }));
        if matches!(behavior, Behavior::OutputPanic) {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(transport.session_drops.load(Ordering::SeqCst), 1);
        assert_eq!(transport.output_drops.load(Ordering::SeqCst), 0);
        workspace.assert_restored(&before);
        workspace.assert_stage_cleanup();
    }
    let transport = Fake::new(workspace.root.path(), Behavior::Normal);
    drop(workspace.publish(&transport).unwrap());
    assert_eq!(fs::read(path).unwrap(), b"aaa");
}

#[test]
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn post_publication_bookkeeping_errors_report_outputs_changed_without_acceptance() {
    for fail_close in [false, true] {
        let workspace = Workspace::new();
        if fail_close {
            workspace
                .runtime
                .native_demand_sessions
                .force_next_close_failure();
        } else {
            workspace
                .runtime
                .native_demand_sessions
                .force_next_replace_accepted_failure();
        }
        let transport = Fake::new(workspace.root.path(), Behavior::Normal);
        let error = workspace.publish(&transport).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("after action outputs were published"),
            "{error}"
        );
        assert_eq!(fs::read(workspace.output_path()).unwrap(), b"aaa");
        workspace.assert_stage_cleanup();
        assert_eq!(transport.output_drops.load(Ordering::SeqCst), 1);
        assert!(
            workspace
                .publish(&transport)
                .unwrap_err()
                .to_string()
                .contains("already active")
        );
    }
}

#[test]
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn publication_failure_preserves_partial_output_context_when_abort_also_fails() {
    let workspace = Workspace::new();
    workspace
        .runtime
        .native_demand_sessions
        .force_next_restoration_failure();
    let transport = Fake::new(workspace.root.path(), Behavior::OutputCommitFailure);
    let error = workspace.publish(&transport).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("publishing action outputs"), "{message}");
    assert!(
        message.contains("output destination changed during staging"),
        "{message}"
    );
    assert!(message.contains("native abort also failed"), "{message}");
    assert!(message.contains("command restoration failed"), "{message}");
    assert!(message.contains("outputs may have changed"), "{message}");
    workspace.assert_stage_cleanup();
    assert_eq!(transport.output_drops.load(Ordering::SeqCst), 1);
    assert!(
        workspace
            .publish(&transport)
            .unwrap_err()
            .to_string()
            .contains("already active")
    );
}

#[test]
fn diamond_executes_each_producer_once_and_file_write_can_be_selected() {
    let workspace = Workspace::new();
    for action in [4, 0] {
        let transport = Fake::new(workspace.root.path(), Behavior::Normal);
        let accepted = workspace.run(action, &transport).unwrap();
        let result = accepted.terminal_for_test();
        assert_eq!(result.output().executed, (0..=action).collect::<Vec<_>>());
        assert_eq!(
            result.inputs().sources().len(),
            if action == 0 { 0 } else { 2 }
        );
        assert_eq!(transport.starts.load(Ordering::SeqCst), 1);
        assert_eq!(transport.executions.load(Ordering::SeqCst), action + 1);
        assert_eq!(transport.session_drops.load(Ordering::SeqCst), 1);
        drop(accepted);
        assert_eq!(transport.output_drops.load(Ordering::SeqCst), 1);
    }
    assert!(!workspace.root.path().join("done").exists());
}

#[test]
fn downstream_staging_checks_producer_sources_and_entire_build_frontier() {
    let workspace = Workspace::new();
    let normal = Fake::new(workspace.root.path(), Behavior::Normal);
    drop(workspace.run(4, &normal).unwrap());
    let before = workspace.snapshot();
    let module = fs::read(workspace.root.path().join("MODULE.bazel")).unwrap();
    for behavior in [Behavior::StageSource, Behavior::StageModule] {
        let transport = Fake::new(workspace.root.path(), behavior);
        let error = workspace.run(4, &transport).unwrap_err();
        assert!(
            error.to_string().contains("changed before execution"),
            "{error}"
        );
        assert_eq!(transport.executions.load(Ordering::SeqCst), 4);
        assert_eq!(transport.session_drops.load(Ordering::SeqCst), 1);
        assert_eq!(transport.output_drops.load(Ordering::SeqCst), 0);
        workspace.assert_restored(&before);
        fs::write(workspace.root.path().join("input"), b"aaa").unwrap();
        fs::write(workspace.root.path().join("MODULE.bazel"), &module).unwrap();
    }
    drop(workspace.run(4, &normal).unwrap());
}

#[test]
fn producer_failures_and_unwind_discard_session_before_fresh_attempt() {
    let workspace = Workspace::new();
    let normal = Fake::new(workspace.root.path(), Behavior::Normal);
    drop(workspace.run(4, &normal).unwrap());
    let before = workspace.snapshot();
    for behavior in [
        Behavior::StageError,
        Behavior::ExecuteError,
        Behavior::StagePanic,
    ] {
        let transport = Fake::new(workspace.root.path(), behavior);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            workspace.run(4, &transport)
        }));
        if matches!(behavior, Behavior::StagePanic) {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(transport.starts.load(Ordering::SeqCst), 1);
        assert_eq!(
            transport.executions.load(Ordering::SeqCst),
            if matches!(behavior, Behavior::ExecuteError) {
                2
            } else {
                1
            }
        );
        assert_eq!(transport.session_drops.load(Ordering::SeqCst), 1);
        assert_eq!(transport.output_drops.load(Ordering::SeqCst), 0);
        workspace.assert_restored(&before);
    }
    drop(workspace.run(4, &normal).unwrap());
}

#[test]
fn final_source_change_retries_whole_chain_with_fresh_session() {
    let workspace = Workspace::new();
    let transport = Fake::new(workspace.root.path(), Behavior::RetrySource);
    let accepted = workspace.run(4, &transport).unwrap();
    assert_eq!(accepted.terminal_for_test().output().bytes, b"bbb");
    assert_eq!(
        accepted.terminal_for_test().output().executed,
        [0, 1, 2, 3, 4]
    );
    assert_eq!(transport.starts.load(Ordering::SeqCst), 2);
    assert_eq!(transport.executions.load(Ordering::SeqCst), 10);
    assert_eq!(transport.session_drops.load(Ordering::SeqCst), 2);
    assert_eq!(transport.output_drops.load(Ordering::SeqCst), 1);
    drop(accepted);
    assert_eq!(transport.output_drops.load(Ordering::SeqCst), 2);
}

#[path = "requested_tests.rs"]
mod requested_tests;
