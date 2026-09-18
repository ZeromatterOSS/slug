use std::fs;
use std::io::Read;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use super::*;
#[path = "../source_staging/test_workspace.rs"]
mod fixture;

struct Workspace {
    root: tempfile::TempDir,
    runtime: WorkspaceRuntime,
    owner: ConfiguredTargetKey,
}

impl Workspace {
    fn new() -> Self {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp734/fixtures");
        fs::create_dir_all(&parent).unwrap();
        let root = tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap();
        fixture::write(root.path());
        fs::write(
            root.path().join("defs.bzl"),
            fixture::DEFS.replace("    out =", "    print('execution-event')\n    out ="),
        )
        .unwrap();
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
        transport: &Fake,
    ) -> Result<AcceptedCommand<SourceActionResult<Output>>, BuildCommandError> {
        self.runtime
            .execute_source_action_with_repository_environment(
                &[TargetPattern::parse("//:one").unwrap()],
                self.owner.clone(),
                0,
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
        assert!(!self.root.path().join("shared.out").exists());
    }
}

#[derive(Clone, Copy)]
enum Behavior {
    Normal,
    StageSource,
    StageModule,
    ExecuteSource,
    StageError,
    ExecuteError,
    StagePanic,
    ExecutePanic,
}
struct Fake {
    root: PathBuf,
    behavior: Behavior,
    stages: AtomicUsize,
    executions: AtomicUsize,
    drops: Arc<AtomicUsize>,
}
impl Fake {
    fn new(root: &Path, behavior: Behavior) -> Self {
        Self {
            root: root.to_owned(),
            behavior,
            stages: AtomicUsize::new(0),
            executions: AtomicUsize::new(0),
            drops: Arc::new(AtomicUsize::new(0)),
        }
    }
}
struct Output {
    bytes: Vec<u8>,
    drops: Arc<AtomicUsize>,
}
impl Drop for Output {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl SourceActionTransport for Fake {
    type Staged = Vec<u8>;
    type Output = Output;
    type Error = std::io::Error;

    async fn stage(&self, inputs: Arc<PreparedSourceActionInputs>) -> Result<Vec<u8>, Self::Error> {
        let attempt = self.stages.fetch_add(1, Ordering::SeqCst);
        if matches!(self.behavior, Behavior::ExecuteSource) && attempt > 0 {
            assert_eq!(
                self.drops.load(Ordering::SeqCst),
                attempt,
                "retry dropped old provisional output"
            );
        }
        let index = inputs
            .sources()
            .position(|source| source.label().target().as_str() == "input")
            .unwrap();
        let mut bytes = Vec::new();
        inputs.open_source(index)?.read_to_end(&mut bytes)?;
        match self.behavior {
            Behavior::StageSource => fs::write(self.root.join("input"), b"bbb")?,
            Behavior::StageModule => {
                let path = self.root.join("MODULE.bazel");
                fs::write(
                    &path,
                    fs::read_to_string(&path)? + "# changed during staging\n",
                )?;
            }
            Behavior::StageError => return Err(std::io::Error::other("stage failure")),
            Behavior::StagePanic => panic!("stage panic"),
            _ => {}
        }
        Ok(bytes)
    }

    async fn execute(&self, bytes: Vec<u8>) -> Result<Output, Self::Error> {
        let attempt = self.executions.fetch_add(1, Ordering::SeqCst);
        match self.behavior {
            Behavior::ExecuteSource if attempt == 0 => fs::write(self.root.join("input"), b"bbb")?,
            Behavior::ExecuteError => return Err(std::io::Error::other("execute failure")),
            Behavior::ExecutePanic => panic!("execute panic"),
            _ => {}
        }
        Ok(Output {
            bytes,
            drops: self.drops.clone(),
        })
    }
}

#[test]
fn accepted_source_execution_and_retry_publish_one_current_result_and_event_stream() {
    let workspace = Workspace::new();
    for (index, behavior) in [Behavior::Normal, Behavior::ExecuteSource]
        .into_iter()
        .enumerate()
    {
        // Native events are incremental: the initial owner-selection build has
        // already published its print. Introduce one new analysis event per
        // command, and require exactly one publication even across a retry.
        fs::write(
            workspace.root.path().join("defs.bzl"),
            fixture::DEFS.replace(
                "    out =",
                &format!("    print('execution-event-{index}')\n    out ="),
            ),
        )
        .unwrap();
        let transport = Fake::new(workspace.root.path(), behavior);
        let accepted = workspace.run(&transport).unwrap();
        let expected = if matches!(behavior, Behavior::Normal) {
            b"aaa"
        } else {
            b"bbb"
        };
        assert_eq!(accepted.terminal_for_test().output().bytes, expected);
        assert_eq!(accepted.terminal_for_test().inputs().sources().len(), 2);
        let published = accepted
            .project(|_| crate::runtime::TerminalOutput::new(0, String::new(), String::new()))
            .publish();
        let (published, _, _, stderr) = published.into_parts();
        assert_eq!(stderr.matches("execution-event").count(), 1);
        let attempts = if matches!(behavior, Behavior::Normal) {
            1
        } else {
            2
        };
        assert_eq!(transport.stages.load(Ordering::SeqCst), attempts);
        assert_eq!(transport.executions.load(Ordering::SeqCst), attempts);
        drop(published);
        assert_eq!(transport.drops.load(Ordering::SeqCst), attempts);
        assert!(!workspace.root.path().join("shared.out").exists());
    }
}

#[test]
fn staging_source_and_build_frontier_changes_prevent_execute_and_restore_acceptance() {
    let workspace = Workspace::new();
    let initial = Fake::new(workspace.root.path(), Behavior::Normal);
    drop(workspace.run(&initial).unwrap());
    let before = workspace.snapshot();
    for behavior in [Behavior::StageSource, Behavior::StageModule] {
        let transport = Fake::new(workspace.root.path(), behavior);
        let error = workspace.run(&transport).unwrap_err();
        assert!(
            error.to_string().contains("changed before execution"),
            "{error}"
        );
        assert_eq!(transport.executions.load(Ordering::SeqCst), 0);
        workspace.assert_restored(&before);
        fs::write(workspace.root.path().join("input"), b"aaa").unwrap();
    }
    drop(workspace.run(&initial).unwrap());
}

#[test]
fn source_transport_errors_and_unwind_restore_prior_snapshot_and_reopen() {
    let workspace = Workspace::new();
    let normal = Fake::new(workspace.root.path(), Behavior::Normal);
    drop(workspace.run(&normal).unwrap());
    let before = workspace.snapshot();
    for behavior in [
        Behavior::StageError,
        Behavior::ExecuteError,
        Behavior::StagePanic,
        Behavior::ExecutePanic,
    ] {
        let transport = Fake::new(workspace.root.path(), behavior);
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| workspace.run(&transport)));
        if matches!(behavior, Behavior::StagePanic | Behavior::ExecutePanic) {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        workspace.assert_restored(&before);
        assert_eq!(
            transport.executions.load(Ordering::SeqCst),
            usize::from(matches!(
                behavior,
                Behavior::ExecuteError | Behavior::ExecutePanic
            ))
        );
    }
    drop(workspace.run(&normal).unwrap());
}

#[test]
fn closure_owned_representative_check_rejects_shared_duplicate() {
    let workspace = Workspace::new();
    // Spawn sharing is currently rejected by closure admission. Exercise the
    // same Core coordinate guard using the admitted scalar FileWrite family,
    // without constructing or exposing a forged prepared Spawn.
    fs::write(workspace.root.path().join("defs.bzl"),
        "def _impl(ctx):\n    out = ctx.actions.declare_file('shared.out')\n    ctx.actions.write(out, 'same')\n    return [DefaultInfo(files=depset([out]))]\nstage=rule(implementation=_impl, attrs={'input':attr.label(allow_single_file=True), 'tool':attr.label(allow_single_file=True)})\n").unwrap();
    let accepted = workspace
        .runtime
        .build_command_with_repository_environment(
            &[
                TargetPattern::parse("//:one").unwrap(),
                TargetPattern::parse("//:two").unwrap(),
            ],
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[format!(
                "file://{}/empty-registry",
                workspace.root.path().display()
            )],
            Default::default(),
            Default::default(),
        )
        .unwrap();
    let evaluation = accepted.terminal_for_test().as_ref().as_ref().unwrap();
    let mut admitted = 0;
    let mut rejected = 0;
    for (owner, node) in evaluation.action_closure.owners().iter().enumerate() {
        for action in 0..node.actions().len() {
            match source_staging::require_execution_representative(evaluation, owner, action) {
                Ok(()) => admitted += 1,
                Err(error) => {
                    assert!(
                        error
                            .to_string()
                            .contains("not an execution representative")
                    );
                    rejected += 1;
                }
            }
        }
    }
    assert_eq!((admitted, rejected), (1, 1));
}
