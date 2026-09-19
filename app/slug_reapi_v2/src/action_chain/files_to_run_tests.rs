//! Real requested consumers use tool runfiles without publishing tool intermediates.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use slug_core_v2::runtime::AcceptedCommand;
use slug_core_v2::runtime::ActionChainOutputTransport;
use slug_core_v2::runtime::BuildCommandError;
use slug_core_v2::runtime::BzlmodCommandPolicyKey;
use slug_core_v2::runtime::BzlmodEnvironmentPolicyKey;
use slug_core_v2::runtime::LockfileMode;
use slug_core_v2::runtime::PlannedActionOutputStaging;
use slug_core_v2::runtime::ProcessHostOwner;
use slug_core_v2::runtime::RequestedActionResult;
use slug_core_v2::runtime::TargetPattern;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::WorkspaceRuntime;

use super::*;

#[path = "files_to_run_fixture.rs"]
mod authored;

struct Fixture {
    root: PathBuf,
    runtime: WorkspaceRuntime,
    module: String,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/wp751/wire-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let module = authored::write(&root);
        let runtime = WorkspaceRuntime::new(&root, ProcessHostOwner::native()).unwrap();
        Self {
            root,
            runtime,
            module,
        }
    }
    fn content(&self, bytes: &str) {
        authored::content(&self.root, &self.module, bytes);
    }
    fn run<T: ActionChainOutputTransport>(
        &self,
        transport: &T,
        publish: bool,
    ) -> Result<
        AcceptedCommand<Result<RequestedActionResult<T::Output>, BuildCommandError>>,
        BuildCommandError,
    > {
        let targets = ["//:implicit", "//:explicit", "//:authored"]
            .map(|value| TargetPattern::parse(value).unwrap());
        let registry = [format!("file://{}/empty-registry", self.root.display())];
        let command = BzlmodCommandPolicyKey::from_flags(None, false).unwrap();
        let environment =
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap();
        if publish {
            self.runtime
                .execute_and_publish_requested_actions_with_repository_environment(
                    &targets,
                    command,
                    environment,
                    LockfileMode::Update,
                    &registry,
                    Default::default(),
                    Default::default(),
                    transport,
                )
        } else {
            self.runtime
                .execute_requested_actions_with_repository_environment(
                    &targets,
                    command,
                    environment,
                    LockfileMode::Update,
                    &registry,
                    Default::default(),
                    Default::default(),
                    transport,
                )
        }
    }
    fn assert_outputs(&self, inputs: &PreparedActionChainInputs, published: bool, bytes: &[u8]) {
        for node in inputs
            .evaluation()
            .unwrap()
            .analyses()
            .filter(|node| !node.actions().is_empty())
        {
            let root = slug_core_v2::runtime::configured_output_root(
                &self.root,
                node.configured_target_key()
                    .unwrap()
                    .configuration()
                    .slug_configuration()
                    .unwrap(),
            );
            for action in node.actions() {
                for output in action.outputs() {
                    let path = root.join(output.path());
                    if published
                        && ["implicit.out", "explicit.out", "authored.out"].contains(&output.path())
                    {
                        assert_eq!(
                            fs::read(&path).unwrap(),
                            bytes.repeat(4),
                            "{}",
                            path.display()
                        );
                    } else {
                        assert!(
                            fs::symlink_metadata(&path).is_err(),
                            "unrequested output installed: {}",
                            path.display()
                        );
                    }
                }
            }
        }
        assert!(
            !self.root.join("bazel-out/.slug-runfiles-sources").exists(),
            "tool-only sources need no durable publication backing"
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fn writable(path: &Path) {
            let Ok(metadata) = path.symlink_metadata() else {
                return;
            };
            if !metadata.is_dir() {
                return;
            }
            fs::set_permissions(
                path,
                fs::Permissions::from_mode(metadata.permissions().mode() | 0o700),
            )
            .unwrap();
            for entry in fs::read_dir(path).unwrap() {
                writable(&entry.unwrap().path());
            }
        }
        writable(&self.root);
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[derive(Clone, Copy)]
enum Mutation {
    Remove,
    Corrupt,
}

struct CheckedTransport {
    inner: ActionChainReapiTransport,
    mutation: Option<Mutation>,
    consumers: AtomicUsize,
    checked: AtomicUsize,
    producers: AtomicUsize,
    inputs: std::sync::Mutex<Option<Arc<PreparedActionChainInputs>>>,
}
impl CheckedTransport {
    fn new(mutation: Option<Mutation>) -> Self {
        let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap();
        Self {
            inner: ActionChainReapiTransport::new(
                RemoteConfig::from_args(&[&format!("--remote_executor={endpoint}")]).unwrap(),
            )
            .unwrap(),
            mutation,
            consumers: AtomicUsize::new(0),
            checked: AtomicUsize::new(0),
            producers: AtomicUsize::new(0),
            inputs: std::sync::Mutex::new(None),
        }
    }
}
fn is_consumer(session: &ActionChainReapiSession, index: usize) -> bool {
    session.inputs.plan().unwrap().actions()[index]
        .action()
        .outputs()
        .iter()
        .any(|output| ["implicit.out", "explicit.out", "authored.out"].contains(&output.path()))
}
impl ActionChainTransport for CheckedTransport {
    type Session = ActionChainReapiSession;
    type Staged = StagedChainAction;
    type Output = ActionChainRemoteResult;
    type Error = RemoteExecutionError;
    async fn start(
        &self,
        inputs: Arc<PreparedActionChainInputs>,
    ) -> Result<Self::Session, Self::Error> {
        *self.inputs.lock().unwrap() = Some(inputs.clone());
        self.inner.start(inputs).await
    }
    async fn stage(
        &self,
        session: &mut Self::Session,
        index: usize,
    ) -> Result<Self::Staged, Self::Error> {
        if is_consumer(session, index) {
            let bound = session.templates[index].bind(&session.results).unwrap();
            let entries = bound.tree.entries();
            let generated = entries
                .iter()
                .find(|entry| entry.path().ends_with(".runfiles/_main/gen_file"))
                .unwrap();
            let source = entries
                .iter()
                .find(|entry| entry.path().ends_with(".runfiles/_main/input"))
                .unwrap();
            let materialized = entries
                .iter()
                .find(|entry| entry.path().ends_with(".runfiles/+repo+generated/data"))
                .unwrap();
            assert_eq!(generated.digest(), source.digest());
            assert_eq!(generated.digest(), materialized.digest());
            assert!(bound.generated.contains(generated.digest()));
            assert!(
                bound.sources.contains_key(source.digest()),
                "equal source bytes must remain a separate provenance candidate"
            );
            assert!(entries.iter().all(|entry| entry.is_executable()));
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.path().ends_with(".runfiles/_repo_mapping"))
            );
            let authored = generated
                .path()
                .starts_with("tools/authored_tool.runfiles/");
            assert_eq!(
                entries
                    .iter()
                    .any(|entry| entry.path().ends_with(".runfiles/MANIFEST")),
                authored
            );
            assert!(
                bound
                    .tree
                    .directories()
                    .iter()
                    .any(|path| path.ends_with(".runfiles/_main/empty_tree"))
            );
            assert!(
                !bound
                    .tree
                    .directories()
                    .iter()
                    .any(|path| path.ends_with("omitted_empty"))
            );
            self.checked.fetch_add(1, Ordering::SeqCst);
        }
        self.inner.stage(session, index).await
    }
    async fn execute(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: Self::Staged,
    ) -> Result<(), Self::Error> {
        if is_consumer(session, index) {
            self.consumers.fetch_add(1, Ordering::SeqCst);
        }
        self.inner.execute(session, index, staged).await?;
        let generated = session.results[index].remote().and_then(|remote| {
            remote
                .result
                .output_files()
                .iter()
                .find(|file| file.path() == "gen_file")
        });
        if let Some(file) = generated {
            self.producers.fetch_add(1, Ordering::SeqCst);
            if let Some(mutation) = self.mutation {
                let digest = file.digest();
                let root = PathBuf::from(std::env::var("SLUG_V2_NATIVELINK_TEST_ROOT").unwrap());
                for store in ["fast-content", "slow-content"] {
                    let path = root.join("cas").join(store).join("d").join(format!(
                        "{}-{}",
                        digest.hash(),
                        digest.size_bytes()
                    ));
                    assert!(
                        path.is_file(),
                        "generated CAS blob missing before mutation: {}",
                        path.display()
                    );
                    match mutation {
                        Mutation::Remove => fs::remove_file(path).unwrap(),
                        Mutation::Corrupt => {
                            fs::set_permissions(
                                &path,
                                fs::Permissions::from_mode(
                                    path.metadata().unwrap().permissions().mode() | 0o200,
                                ),
                            )
                            .unwrap();
                            fs::write(path, vec![b'!'; digest.size_bytes() as usize]).unwrap();
                        }
                    }
                }
                let requested = [digest.clone()].into_iter().collect();
                match mutation {
                    Mutation::Remove => {
                        for _ in 0..2 {
                            session
                                .cache
                                .read_blob_verified(digest, |_| Ok(()))
                                .await
                                .expect_err("removed generated CAS blob must fail");
                        }
                        assert_eq!(
                            session.cache.find_missing(&requested).await.unwrap(),
                            requested
                        );
                    }
                    Mutation::Corrupt => assert!(
                        session
                            .cache
                            .find_missing(&requested)
                            .await
                            .unwrap()
                            .is_empty()
                    ),
                }
            }
        }
        Ok(())
    }
    async fn finish(&self, session: Self::Session) -> Result<Self::Output, Self::Error> {
        self.inner.finish(session).await
    }
}
impl ActionChainOutputTransport for CheckedTransport {
    async fn stage_outputs(
        &self,
        session: &mut Self::Session,
        stages: &[PlannedActionOutputStaging],
    ) -> Result<(), Self::Error> {
        self.inner.stage_outputs(session, stages).await
    }
}

#[test]
fn files_to_run_output_namespace_conflict_rejects_before_connection() {
    let fixture = Fixture::new();
    let prepare = || {
        let accepted = fixture
            .runtime
            .prepare_requested_action_inputs_with_repository_environment(
                &[TargetPattern::parse("//:implicit").unwrap()],
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!("file://{}/empty-registry", fixture.root.display())],
                Default::default(),
                Default::default(),
            )
            .unwrap();
        let mut inputs = None;
        drop(accepted.project(|value| {
            inputs = Some(value.clone());
            TerminalOutput::new(0, String::new(), String::new())
        }));
        inputs.unwrap()
    };
    let valid = prepare();
    Template::prepare(&valid, &Default::default()).unwrap();
    fixture.assert_outputs(&valid, false, b"aaa");
    let defs = fs::read_to_string(fixture.root.join("defs.bzl")).unwrap();
    let needle = "ctx.actions.declare_file(ctx.label.name + '.out')";
    assert!(defs.contains(needle));
    fs::write(
        fixture.root.join("defs.bzl"),
        defs.replace(
            needle,
            "ctx.actions.declare_file('tools/plain_tool.runfiles/consumer.out')",
        ),
    )
    .unwrap();
    // The exec-configured tool and target-configured output are distinct declared
    // artifacts, but collide in this consumer's remote input/output namespace.
    let inputs = prepare();
    let transport = ActionChainReapiTransport::new(
        RemoteConfig::from_args(&["--remote_executor=grpc://127.0.0.1:1"]).unwrap(),
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let error = match runtime.block_on(transport.start(inputs.clone())) {
        Err(error) => error,
        Ok(_) => panic!("conflicting consumer opened a remote session"),
    };
    assert!(matches!(error, RemoteExecutionError::Command(_)), "{error}");
    assert!(error.to_string().contains("namespace conflict"), "{error}");
    fixture.assert_outputs(&inputs, false, b"aaa");
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink"]
fn nativelink_files_to_run_consumers_cache_restore_without_tool_publication() {
    let fixture = Fixture::new();
    let transport = CheckedTransport::new(None);
    let mut original = None;
    for (iteration, bytes) in ["aaa", "aaa", "bbb", "aaa"].into_iter().enumerate() {
        if iteration >= 2 {
            fixture.content(bytes);
        }
        drop(fixture.run(&transport, true).unwrap().project(|accepted| {
            let accepted = accepted.as_ref().unwrap();
            let output = accepted.output().unwrap();
            let remotes = output
                .results()
                .iter()
                .filter_map(ActionChainStepResult::remote)
                .collect::<Vec<_>>();
            assert_eq!(
                remotes.len(),
                6,
                "two tool writes, shared backing, three consumers"
            );
            let digests = remotes
                .iter()
                .map(|result| result.action_digest.clone())
                .collect::<Vec<_>>();
            if let Some(original) = &original {
                assert_eq!(original == &digests, bytes == "aaa");
            } else {
                original = Some(digests);
            }
            let hits: u64 = remotes.iter().map(|result| result.evidence.ac_hits).sum();
            assert_eq!(hits, [0, 6, 2, 6][iteration], "iteration {iteration}");
            for result in remotes {
                assert!(result.output_blobs.is_empty());
                if iteration == 1 || iteration == 3 {
                    assert!(result.evidence.uploaded_digests.is_empty());
                }
            }
            assert_eq!(
                accepted
                    .published_outputs()
                    .iter()
                    .map(|group| group.outputs().outputs().len())
                    .sum::<usize>(),
                3
            );
            fixture.assert_outputs(accepted.inputs(), true, bytes.as_bytes());
            TerminalOutput::new(0, String::new(), String::new())
        }));
    }
    assert_eq!(transport.producers.load(Ordering::SeqCst), 4);
    assert_eq!(transport.consumers.load(Ordering::SeqCst), 12);
    assert_eq!(transport.checked.load(Ordering::SeqCst), 12);
    // The same real consumers also work through execution-only requested API.
    let execution_only = Fixture::new();
    drop(
        execution_only
            .run(&transport, false)
            .unwrap()
            .project(|accepted| {
                let accepted = accepted.as_ref().unwrap();
                assert!(accepted.published_outputs().is_empty());
                fixture_output_digests(accepted.output().unwrap(), b"aaa");
                execution_only.assert_outputs(accepted.inputs(), false, b"aaa");
                TerminalOutput::new(0, String::new(), String::new())
            }),
    );
}

fn fixture_output_digests(output: &ActionChainRemoteResult, bytes: &[u8]) {
    let mut count = 0;
    for file in output
        .results()
        .iter()
        .filter_map(ActionChainStepResult::remote)
        .flat_map(|remote| remote.result.output_files())
    {
        if ["implicit.out", "explicit.out", "authored.out"].contains(&file.path()) {
            assert_eq!(file.digest(), &ReapiDigest::of_bytes(&bytes.repeat(4)));
            count += 1;
        }
    }
    assert_eq!(count, 3);
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and fixture CAS paths"]
fn nativelink_files_to_run_generated_cas_failure_is_not_repaired_from_equal_sources() {
    for (mutation, bytes, expected) in [
        (Mutation::Remove, "evicted", "REAPI CAS is missing input"),
        (Mutation::Corrupt, "corrupt", "download digest mismatch:"),
    ] {
        let fixture = Fixture::new();
        fixture.content(bytes);
        let transport = CheckedTransport::new(Some(mutation));
        let error = fixture.run(&transport, false).unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
        assert_eq!(transport.producers.load(Ordering::SeqCst), 1);
        assert_eq!(transport.checked.load(Ordering::SeqCst), 1);
        assert_eq!(
            transport.consumers.load(Ordering::SeqCst),
            0,
            "consumer Execute must not start"
        );
        // No publication API was invoked and no tool filesystem projection is permitted.
        fixture.assert_outputs(
            transport.inputs.lock().unwrap().as_ref().unwrap(),
            false,
            bytes.as_bytes(),
        );
    }
}
