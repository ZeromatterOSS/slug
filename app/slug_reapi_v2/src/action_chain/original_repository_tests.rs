//! Genuine original repository Execute -> sealed physical sources -> NativeLink
//! and durable runfiles. No source metadata, native root or result constructors.
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use slug_core_v2::runtime::ActionChainOutputTransport;
use slug_core_v2::runtime::BzlmodCommandPolicyKey;
use slug_core_v2::runtime::BzlmodEnvironmentPolicyKey;
use slug_core_v2::runtime::LockfileMode;
use slug_core_v2::runtime::PlannedActionOutputStaging;
use slug_core_v2::runtime::ProcessHostOwner;
use slug_core_v2::runtime::TargetPattern;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::WorkspaceRuntime;
use slug_workspace_v2::PathObservationInstanceId;

use super::*;

#[path = "runfiles_fixture.rs"]
mod authored;

const REPOSITORY: &str = r#"def _impl(ctx):
    ctx.file('seed', ctx.attr.content, executable=False)
    result = ctx.execute(['/usr/bin/python3', '-c', '''import pathlib, os
body = pathlib.Path('seed').read_bytes()
pathlib.Path('data').write_bytes(body)
pathlib.Path('executable_data').write_bytes(body)
pathlib.Path('tool').write_text('#!/bin/sh\\nset -eu\\n/bin/cat input > "$1"\\n/bin/cat input > "$2"\\n')
os.chmod('data', 0o444)
os.chmod('executable_data', 0o555)
os.chmod('tool', 0o555)
pathlib.Path('executed').write_text('actual process')
'''])
    if result.return_code != 0 or ctx.read('executed') != 'actual process':
        fail('original process did not complete')
    ctx.file('BUILD.bazel', "exports_files(['data', 'executable_data', 'tool'])\n", executable=False)
repo = repository_rule(implementation=_impl, attrs={'content': attr.string()})
"#;

const CONSUMER: &str = r#"
def _consume(ctx):
    out = ctx.actions.declare_file('consumer.out')
    tool = ctx.attr.tool[DefaultInfo]
    ctx.actions.run(executable=ctx.attr.runner[DefaultInfo].files.to_list()[0], tools=[tool.files_to_run], outputs=[out], arguments=[tool.files.to_list()[0].path, out.path], env={'ORIGINAL_REAPI_VARIANT': 'base'})
    return [DefaultInfo(files=depset([out]))]
consume = rule(implementation=_consume, attrs={'tool': attr.label(), 'runner': attr.label(allow_single_file=True)})
"#;

struct Fixture {
    root: PathBuf,
    runtime: Option<WorkspaceRuntime>,
    resources: slug_core_v2::runtime::RepositoryWorkspaceIntegrationTestResources,
    module: String,
    definitions: String,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/wp425/original-reapi-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let module = authored::write(&root);
        fs::write(root.join("repo.bzl"), REPOSITORY).unwrap();
        let definitions = format!(
            "{}\n{CONSUMER}",
            fs::read_to_string(root.join("defs.bzl")).unwrap()
        );
        fs::write(root.join("defs.bzl"), &definitions).unwrap();
        let build = fs::read_to_string(root.join("BUILD.bazel"))
            .unwrap()
            .replace("'binary')", "'binary', 'consume')")
            .replace("tool='tools/tool'", "tool='@generated//:tool'");
        fs::write(
            root.join("BUILD.bazel"),
            format!("{build}\nconsume(name='consume', tool=':one', runner='tools/consumer')\n"),
        )
        .unwrap();
        fs::write(
            root.join("tools/consumer"),
            "#!/bin/sh\nset -eu\n\"./$1\" > \"$2\"\n",
        )
        .unwrap();
        let mut runtime = WorkspaceRuntime::new(&root, ProcessHostOwner::native()).unwrap();
        let resources = runtime
            .enable_general_repository_workspaces_for_integration_test()
            .unwrap();
        Self {
            root,
            runtime: Some(runtime),
            resources,
            module,
            definitions,
        }
    }
    fn content(&self, bytes: &str) {
        authored::content(&self.root, &self.module, bytes);
    }
    fn consumer_variant(&self, changed: bool) {
        fs::write(
            self.root.join("defs.bzl"),
            if changed {
                self.definitions.replace(
                    "'ORIGINAL_REAPI_VARIANT': 'base'",
                    "'ORIGINAL_REAPI_VARIANT': 'changed'",
                )
            } else {
                self.definitions.clone()
            },
        )
        .unwrap();
    }
    fn publish(
        &self,
        transport: &CheckedTransport,
    ) -> Result<
        slug_core_v2::runtime::AcceptedCommand<
            Result<
                slug_core_v2::runtime::RequestedActionResult<ActionChainRemoteResult>,
                slug_core_v2::runtime::BuildCommandError,
            >,
        >,
        slug_core_v2::runtime::BuildCommandError,
    > {
        self.runtime
            .as_ref()
            .unwrap()
            .execute_and_publish_requested_actions_with_repository_environment(
                &[
                    TargetPattern::parse("//:one").unwrap(),
                    TargetPattern::parse("//:consume").unwrap(),
                ],
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!("file://{}/empty-registry", self.root.display())],
                Default::default(),
                Default::default(),
                transport,
            )
    }
    fn retired(&mut self, transport: &CheckedTransport) {
        drop(self.runtime.take());
        assert!(
            self.resources.snapshot().backing_bytes > 0,
            "retained selected data must survive runtime retirement"
        );
        transport.held.lock().unwrap().clear();
        let snapshot = self.resources.snapshot();
        assert_eq!(snapshot.native_fds, 0);
        assert_eq!(snapshot.backing_bytes, 0);
        assert_eq!(
            snapshot.retained_bytes,
            self.resources.baseline().retained_bytes
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fn writable(path: &Path) {
            let Ok(metadata) = fs::symlink_metadata(path) else {
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
            for child in fs::read_dir(path).unwrap() {
                writable(&child.unwrap().path());
            }
        }
        writable(&self.root);
        fs::remove_dir_all(&self.root).unwrap();
    }
}

struct CheckedTransport {
    inner: ActionChainReapiTransport,
    held: Mutex<Vec<NativeActionChainInputs>>,
    generations: Mutex<Vec<(PathBuf, PathObservationInstanceId)>>,
    physical_tool_uploads: AtomicUsize,
}
impl CheckedTransport {
    fn new() -> Self {
        let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap();
        Self {
            inner: ActionChainReapiTransport::new(
                RemoteConfig::from_args(&[&format!("--remote_executor={endpoint}")]).unwrap(),
            )
            .unwrap(),
            held: Mutex::new(Vec::new()),
            generations: Mutex::new(Vec::new()),
            physical_tool_uploads: AtomicUsize::new(0),
        }
    }
}
impl ActionChainTransport for CheckedTransport {
    type Session = ActionChainReapiSession;
    type Staged = StagedChainAction;
    type Output = ActionChainRemoteResult;
    type Error = RemoteExecutionError;
    async fn start(
        &self,
        inputs: NativeActionChainInputs,
        args: &dyn slug_build_api_v2::ArgsDirectoryContext,
    ) -> Result<Self::Session, Self::Error> {
        let mut physical = 0;
        let mut generation = None;
        for (index, source) in inputs.sources().enumerate() {
            let Some(anchor) = source.anchor() else {
                continue;
            };
            physical += 1;
            assert!(
                fs::symlink_metadata(anchor.root().as_path()).is_err(),
                "selected physical display must remain absent on Host"
            );
            let mut bytes = Vec::new();
            inputs
                .open_source(index)
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            let actual = ReapiDigest::of_bytes(&bytes);
            assert_eq!(actual.size_bytes(), source.digest().size_bytes());
            assert_eq!(
                actual.hash(),
                source
                    .digest()
                    .sha256()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            );
            let current = (
                anchor.root().as_path().to_owned(),
                match source.namespace() {
                    slug_workspace_v2::PathObservationNamespace::Materialization(instance) => {
                        instance
                    }
                    _ => panic!("physical source lost materialization namespace"),
                },
            );
            if let Some(prior) = &generation {
                assert_eq!(prior, &current);
            } else {
                generation = Some(current);
            }
        }
        assert_eq!(
            physical, 3,
            "actual General tool plus both mode-distinct data files"
        );
        let tool = inputs
            .sources()
            .find(|source| source.anchor().is_some() && source.label().target().as_str() == "tool")
            .expect("actual physical source executable");
        assert!(
            inputs
                .sources()
                .filter(|source| source.anchor().is_none())
                .all(|source| source.digest() != tool.digest()),
            "cold-upload discriminator requires a unique physical tool digest without a Host twin"
        );
        self.generations.lock().unwrap().push(generation.unwrap());
        self.held.lock().unwrap().push(inputs.clone());
        self.inner.start(inputs, args).await
    }
    async fn stage(
        &self,
        session: &mut Self::Session,
        index: usize,
    ) -> Result<Self::Staged, Self::Error> {
        let staged = self.inner.stage(session, index).await?;
        if let StagedPayload::Remote { uploaded, .. } = &staged.payload {
            for source in session.inputs.sources().filter(|source| {
                source.anchor().is_some() && source.label().target().as_str() == "tool"
            }) {
                let digest = ReapiDigest::new(
                    source
                        .digest()
                        .sha256()
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>(),
                    source.digest().size_bytes(),
                )
                .unwrap();
                if uploaded.contains(&digest) {
                    self.physical_tool_uploads.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        Ok(staged)
    }
    async fn execute(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: Self::Staged,
    ) -> Result<(), Self::Error> {
        self.inner.execute(session, index, staged).await
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

fn published_root(
    accepted: &slug_core_v2::runtime::RequestedActionResult<ActionChainRemoteResult>,
) -> PathBuf {
    accepted
        .published_outputs()
        .iter()
        .find(|group| {
            group
                .outputs()
                .outputs()
                .iter()
                .any(|out| out.root_relative_path() == "binary")
        })
        .unwrap()
        .outputs()
        .root()
        .to_owned()
}

#[test]
#[ignore = "requires actual compiled CLI and supervised fresh verifying NativeLink"]
fn nativelink_original_repository_execute_physical_sources_cache_and_runfiles_survive_shutdown() {
    let mut fixture = Fixture::new();
    let transport = CheckedTransport::new();
    let mut original = None;
    let mut root = None;
    for (iteration, bytes) in ["aaa", "aaa", "bbb", "aaa"].into_iter().enumerate() {
        if iteration >= 2 {
            fixture.content(bytes);
        }
        drop(fixture.publish(&transport).unwrap().project(|accepted| {
            let accepted = accepted.as_ref().unwrap();
            let remotes = accepted
                .output()
                .unwrap()
                .results()
                .iter()
                .filter_map(ActionChainStepResult::remote)
                .collect::<Vec<_>>();
            assert_eq!(remotes.len(), 3);
            if iteration == 0 {
                assert!(
                    transport.physical_tool_uploads.load(Ordering::Relaxed) > 0,
                    "cold CAS must actually upload the unique physical source executable digest"
                );
            }
            assert_eq!(
                remotes.iter().map(|r| r.evidence.ac_hits).sum::<u64>(),
                [0, 3, 1, 3][iteration]
            );
            let digests = remotes
                .iter()
                .map(|r| r.action_digest.clone())
                .collect::<Vec<_>>();
            if let Some(original) = &original {
                assert_eq!(original == &digests, bytes == "aaa");
            } else {
                original = Some(digests);
            }
            if iteration == 1 || iteration == 3 {
                assert!(
                    remotes
                        .iter()
                        .all(|r| r.evidence.uploaded_digests.is_empty())
                );
            }
            let path = published_root(accepted);
            authored::check_tree(&fixture.root, &path, bytes.as_bytes());
            assert_eq!(
                fs::read(path.join("consumer.out")).unwrap(),
                bytes.as_bytes().repeat(3)
            );
            root = Some(path);
            TerminalOutput::new(0, String::new(), String::new())
        }));
    }
    let generations = transport.generations.lock().unwrap();
    assert_eq!(generations.len(), 4);
    assert!(
        generations
            .iter()
            .all(|(path, _)| path == &generations[0].0)
    );
    for (index, (_, instance)) in generations.iter().enumerate() {
        assert!(
            !generations[..index]
                .iter()
                .any(|(_, prior)| prior == instance)
        );
    }
    drop(generations);
    fixture.retired(&transport);
    authored::check_tree(&fixture.root, &root.unwrap(), b"aaa");
}

#[test]
#[ignore = "requires actual compiled CLI and supervised fresh verifying NativeLink fixture CAS"]
fn nativelink_original_repository_corrupt_cas_refuses_publication_and_restores() {
    let mut fixture = Fixture::new();
    let transport = CheckedTransport::new();
    let mut root = None;
    drop(fixture.publish(&transport).unwrap().project(|accepted| {
        let accepted = accepted.as_ref().unwrap();
        assert_eq!(
            accepted
                .output()
                .unwrap()
                .results()
                .iter()
                .filter_map(ActionChainStepResult::remote)
                .map(|r| r.evidence.ac_hits)
                .sum::<u64>(),
            0
        );
        root = Some(published_root(accepted));
        TerminalOutput::new(0, String::new(), String::new())
    }));
    let root = root.unwrap();
    authored::check_tree(&fixture.root, &root, b"aaa");
    let visible = [
        "binary",
        "hidden.txt",
        "consumer.out",
        "binary.runfiles_manifest",
        "binary.repo_mapping",
    ]
    .map(|name| (root.join(name), fs::read(root.join(name)).unwrap()));
    let digest = ReapiDigest::of_bytes(b"aaa");
    let copies = crate::cas_fixture::completed_blob_paths(&digest);
    for path in &copies {
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(fs::metadata(path).unwrap().permissions().mode() | 0o200),
        )
        .unwrap();
        fs::write(path, b"!!!").unwrap();
    }
    fixture.consumer_variant(true);
    let error = fixture
        .publish(&transport)
        .err()
        .expect("corrupt completed source CAS cannot publish")
        .to_string();
    assert!(error.contains("digest mismatch"), "{error}");
    for (path, body) in &visible {
        assert_eq!(
            &fs::read(path).unwrap(),
            body,
            "prior publication changed at {}",
            path.display()
        );
    }
    for path in &copies {
        fs::write(path, b"aaa").unwrap();
    }
    fixture.consumer_variant(false);
    drop(fixture.publish(&transport).unwrap().project(|accepted| {
        let accepted = accepted.as_ref().unwrap();
        assert_eq!(
            accepted
                .output()
                .unwrap()
                .results()
                .iter()
                .filter_map(ActionChainStepResult::remote)
                .map(|r| r.evidence.ac_hits)
                .sum::<u64>(),
            3
        );
        authored::check_tree(&fixture.root, &root, b"aaa");
        TerminalOutput::new(0, String::new(), String::new())
    }));
    fixture.retired(&transport);
    authored::check_tree(&fixture.root, &root, b"aaa");
}
