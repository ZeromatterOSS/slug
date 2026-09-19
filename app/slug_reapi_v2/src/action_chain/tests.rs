use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use prost::Message;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_core_v2::runtime::AcceptedCommand;
use slug_core_v2::runtime::ActionChainResult;
use slug_core_v2::runtime::BuildCommandError;
use slug_core_v2::runtime::BzlmodCommandPolicyKey;
use slug_core_v2::runtime::BzlmodEnvironmentPolicyKey;
use slug_core_v2::runtime::LockfileMode;
use slug_core_v2::runtime::ProcessHostOwner;
use slug_core_v2::runtime::TargetPattern;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::WorkspaceRuntime;

use super::*;

#[path = "../../../slug_core_v2/src/runtime/source_staging/test_workspace.rs"]
mod fixture;

pub(super) const DEFS: &str = r#"def _impl(ctx):
    seed = ctx.actions.declare_file('seed')
    file = ctx.actions.declare_file('file')
    tree = ctx.actions.declare_directory('tree')
    empty = ctx.actions.declare_directory('emptytree')
    done = ctx.actions.declare_file('done')
    result_tree = ctx.actions.declare_directory('result_tree')
    ctx.actions.write(seed, 'seed')
    ctx.actions.run(executable=ctx.attr.tool[DefaultInfo].files.to_list()[0], inputs=[seed, ctx.attr.input[DefaultInfo].files.to_list()[0]], outputs=[file, tree, empty], arguments=['produce'])
    args = ctx.actions.args()
    args.add('consume')
    args.use_param_file('@%s', use_always = True)
    args.set_param_file_format('multiline')
    ctx.actions.run(executable=ctx.attr.tool[DefaultInfo].files.to_list()[0], inputs=[file, tree, empty, ctx.attr.input[DefaultInfo].files.to_list()[0]], outputs=[done, result_tree], arguments=[args])
    return [DefaultInfo(files=depset([done]))]
stage = rule(implementation=_impl, attrs={'input':attr.label(allow_single_file=True), 'tool':attr.label(allow_single_file=True)})
"#;

pub(super) const TOOL: &str = r#"#!/bin/sh
set -eu
if [ "$1" = produce ]; then
  [ "$(/bin/cat seed)" = seed ]
  [ -d tree ] && [ -d emptytree ]
  /bin/mkdir -p tree/nested tree/empty
  /bin/cat input > tree/nested/value
  /bin/cat input > file
  /bin/chmod 0644 tree/nested/value file
else
  read step < "${1#@}"
  [ "$step" = consume ]
  [ -x file ] && [ -x tree/nested/value ]
  [ -d emptytree ] && [ -d result_tree ] && [ ! -e tree/empty ]
  /bin/cat file tree/nested/value input > done
fi
"#;

pub(super) struct Workspace {
    pub(super) root: PathBuf,
    pub(super) runtime: WorkspaceRuntime,
    pub(super) owner: ConfiguredTargetKey,
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl Workspace {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/wp737/wire-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        fixture::write(&root);
        std::fs::write(root.join("defs.bzl"), DEFS).unwrap();
        std::fs::create_dir_all(root.join("tools")).unwrap();
        std::fs::write(root.join("tools/tool"), TOOL).unwrap();
        let build = std::fs::read_to_string(root.join("BUILD.bazel"))
            .unwrap()
            .replace("'tool'", "'tools/tool'");
        std::fs::write(root.join("BUILD.bazel"), build).unwrap();
        let runtime = WorkspaceRuntime::new(&root, ProcessHostOwner::native()).unwrap();
        let mut owner = None;
        drop(
            runtime
                .build_command_with_repository_environment(
                    &[TargetPattern::parse("//:one").unwrap()],
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[format!("file://{}/empty-registry", root.display())],
                    Default::default(),
                    Default::default(),
                )
                .unwrap()
                .project(|value| {
                    owner = value
                        .as_ref()
                        .as_ref()
                        .unwrap()
                        .analyses()
                        .find_map(|node| {
                            node.configured_target_key()
                                .filter(|key| key.label().target().as_str() == "one")
                                .cloned()
                        });
                    TerminalOutput::new(0, String::new(), String::new())
                }),
        );
        Self {
            root,
            runtime,
            owner: owner.unwrap(),
        }
    }

    fn prepare(&self) -> Arc<PreparedActionChainInputs> {
        let accepted = self
            .runtime
            .prepare_action_chain_inputs_with_repository_environment(
                &[TargetPattern::parse("//:one").unwrap()],
                self.owner.clone(),
                2,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!("file://{}/empty-registry", self.root.display())],
                Default::default(),
                Default::default(),
            )
            .unwrap();
        let mut prepared = None;
        drop(accepted.project(|value| {
            prepared = Some(value.clone());
            TerminalOutput::new(0, String::new(), String::new())
        }));
        prepared.unwrap()
    }

    fn run<T: ActionChainTransport>(
        &self,
        transport: &T,
    ) -> Result<AcceptedCommand<ActionChainResult<T::Output>>, BuildCommandError> {
        self.runtime
            .execute_action_chain_with_repository_environment(
                &[TargetPattern::parse("//:one").unwrap()],
                self.owner.clone(),
                2,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!("file://{}/empty-registry", self.root.display())],
                Default::default(),
                Default::default(),
                transport,
            )
    }

    fn assert_no_outputs(&self) {
        for output in [
            "seed",
            "file",
            "tree",
            "emptytree",
            "done",
            "result_tree",
            "done-0.params",
        ] {
            assert!(
                !self.root.join(output).exists(),
                "unexpected local {output}"
            );
        }
    }
}

#[test]
fn whole_chain_preflight_rejects_downstream_policy_and_empty_tree_param_collision() {
    let workspace = Workspace::new();
    let transport = ActionChainReapiTransport::new(
        RemoteConfig::from_args(&["--remote_executor=grpc://127.0.0.1:1"]).unwrap(),
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert_eq!(
        Template::prepare(&workspace.prepare(), &Default::default())
            .unwrap()
            .len(),
        3
    );
    for (defs, expected) in [
        (
            DEFS.replace(
                "outputs=[done, result_tree],",
                "outputs=[done, result_tree], execution_requirements={'no-remote':'1'},",
            ),
            "execution requirements",
        ),
        (
            DEFS.replace("use_always = True", "use_always = False"),
            "conditional",
        ),
        (
            DEFS.replace(
                "declare_directory('emptytree')",
                "declare_directory('done-0.params')",
            ),
            "namespace conflict",
        ),
    ] {
        std::fs::write(workspace.root.join("defs.bzl"), defs).unwrap();
        let error = match runtime.block_on(transport.start(workspace.prepare())) {
            Err(error) => error,
            Ok(_) => panic!("invalid downstream action opened a session"),
        };
        assert!(matches!(error, RemoteExecutionError::Command(_)), "{error}");
        assert!(
            error.to_string().to_lowercase().contains(expected),
            "{error}"
        );
    }
    std::fs::write(workspace.root.join("defs.bzl"), DEFS).unwrap();
    assert_eq!(
        Template::prepare(&workspace.prepare(), &Default::default())
            .unwrap()
            .len(),
        3
    );
    workspace.assert_no_outputs();
}

#[test]
fn empty_tree_namespace_rejects_descendants_and_outputs_but_allows_siblings() {
    let workspace = Workspace::new();
    let mut templates = Template::prepare(&workspace.prepare(), &Default::default()).unwrap();
    let Template::Spawn {
        mut command,
        expanded,
        ..
    } = templates.pop().unwrap()
    else {
        panic!("selected consumer must be a Spawn");
    };
    command.output_directories.clear();
    for (sources, output, conflict) in [
        (&[][..], "tree/child", true),
        (&[][..], "tree", true),
        (&["tree/file"][..], "done", true),
        (&[][..], "tree.other", false),
        (&["tree.other"][..], "tree/child", true),
    ] {
        // An empty tree has a namespace even before producer contents exist.
        let mut inputs = vec![Binding::Generated {
            producer: 0,
            path: "tree".to_owned(),
            output: ActionOutput::new("tree", ActionOutputKind::Directory),
        }];
        inputs.extend(sources.iter().map(|path| Binding::Source {
            path: (*path).to_owned(),
            digest: ReapiDigest::of_bytes(b""),
            index: 0,
        }));
        command.output_files = vec![output.to_owned()];
        let result = validate_namespaces(&inputs, &expanded, &command);
        assert_eq!(result.is_err(), conflict, "{sources:?} -> {output}");
    }
}

fn decoded_directory(tree: &ReapiInputTree, digest: &ReapiDigest) -> crate::proto::Directory {
    let blob = tree
        .directory_blobs()
        .iter()
        .find(|blob| blob.digest() == digest)
        .unwrap();
    crate::proto::Directory::decode(blob.data()).unwrap()
}

fn check_projection(session: &ActionChainReapiSession, index: usize) {
    if index == 0 {
        return;
    }
    let bound = session.templates[index].bind(&session.results).unwrap();
    for blob in bound.tree.directory_blobs() {
        let directory = crate::proto::Directory::decode(blob.data()).unwrap();
        assert!(directory.files.iter().all(|file| file.is_executable));
    }
    if index == 1 {
        assert_eq!(bound.tree.directories(), ["emptytree", "tree"]);
    } else {
        assert_eq!(
            bound.tree.directories(),
            ["emptytree", "result_tree", "tree"]
        );
        let root = decoded_directory(&bound.tree, bound.tree.root_digest());
        let node = root
            .directories
            .iter()
            .find(|node| node.name == "tree")
            .unwrap();
        let digest = crate::executor::digest_from_proto(node.digest.as_ref().unwrap()).unwrap();
        let tree = decoded_directory(&bound.tree, &digest);
        assert_eq!(
            tree.directories
                .iter()
                .map(|node| node.name.as_str())
                .collect::<Vec<_>>(),
            ["nested"]
        );
        assert_eq!(bound.tree.inline_blobs().len(), 1);
        assert_eq!(bound.tree.inline_blobs()[0].data(), b"consume\n");
        let source = bound
            .sources
            .keys()
            .find(|digest| bound.generated.contains(*digest));
        assert!(
            source.is_some(),
            "generated and source provenance must share content"
        );
    }
}

#[derive(Clone, Copy)]
enum CasMutation {
    Remove,
    Corrupt,
}

struct CheckedTransport {
    inner: ActionChainReapiTransport,
    mutation: Option<CasMutation>,
    executions: AtomicUsize,
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
        self.inner.start(inputs).await
    }
    async fn stage(
        &self,
        session: &mut Self::Session,
        index: usize,
    ) -> Result<Self::Staged, Self::Error> {
        check_projection(session, index);
        self.inner.stage(session, index).await
    }
    async fn execute(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: Self::Staged,
    ) -> Result<(), Self::Error> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        self.inner.execute(session, index, staged).await?;
        if index == 1 {
            if let Some(mutation) = self.mutation {
                let digest = session.results[1].remote().unwrap().result.output_files()[0].digest();
                let root = PathBuf::from(std::env::var("SLUG_V2_NATIVELINK_TEST_ROOT").unwrap());
                // Only the supervised fixture CAS is mutated, never workspace outputs.
                // NativeLink filesystem_store::DIGEST_FOLDER is "d".
                for store in ["fast-content", "slow-content"] {
                    let path = root.join("cas").join(store).join("d").join(format!(
                        "{}-{}",
                        digest.hash(),
                        digest.size_bytes()
                    ));
                    assert!(
                        path.is_file(),
                        "verified generated blob absent at {}",
                        path.display()
                    );
                    match mutation {
                        CasMutation::Remove => std::fs::remove_file(path).unwrap(),
                        CasMutation::Corrupt => {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                let mut permissions = path.metadata().unwrap().permissions();
                                permissions.set_mode(permissions.mode() | 0o200);
                                std::fs::set_permissions(&path, permissions).unwrap();
                            }
                            std::fs::write(path, vec![b'!'; digest.size_bytes() as usize]).unwrap()
                        }
                    }
                }
                let requested = [digest.clone()].into_iter().collect();
                match mutation {
                    CasMutation::Remove => {
                        // A failed read clears one stale filesystem index. Two
                        // bounded reads cover the fast and slow stores.
                        for _ in 0..2 {
                            session
                                .cache
                                .read_blob_verified(digest, |_| Ok(()))
                                .await
                                .expect_err("removed generated blob must fail verification");
                        }
                        assert_eq!(
                            session.cache.find_missing(&requested).await.unwrap(),
                            requested,
                            "consumer staging must exercise the missing-generated guard"
                        );
                    }
                    CasMutation::Corrupt => {
                        assert!(
                            session
                                .cache
                                .find_missing(&requested)
                                .await
                                .unwrap()
                                .is_empty(),
                            "corrupt content must still be advertised present"
                        );
                    }
                }
            }
        }
        Ok(())
    }
    async fn finish(&self, session: Self::Session) -> Result<Self::Output, Self::Error> {
        self.inner.finish(session).await
    }
}

fn transport(mutation: Option<CasMutation>) -> CheckedTransport {
    let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap();
    CheckedTransport {
        inner: ActionChainReapiTransport::new(
            RemoteConfig::from_args(&[&format!("--remote_executor={endpoint}")]).unwrap(),
        )
        .unwrap(),
        mutation,
        executions: AtomicUsize::new(0),
    }
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink"]
fn nativelink_chain_generated_file_tree_cache_change_restore() {
    let workspace = Workspace::new();
    let transport = transport(None);
    let mut original = None;
    for (run, (bytes, hit)) in [
        (b"aaa".as_slice(), false),
        (b"aaa", true),
        (b"bbb", false),
        (b"aaa", true),
    ]
    .into_iter()
    .enumerate()
    {
        std::fs::write(workspace.root.join("input"), bytes).unwrap();
        drop(workspace.run(&transport).unwrap().project(|accepted| {
            let results = accepted
                .output()
                .results()
                .iter()
                .map(|step| step.remote().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(results.len(), 3);
            for (index, result) in results.iter().enumerate() {
                let hit = if index == 0 { run != 0 } else { hit };
                assert_eq!(result.evidence.ac_hits, u64::from(hit));
                assert_eq!(result.evidence.ac_misses, u64::from(!hit));
                assert!(result.output_blobs.is_empty());
                assert!(result.evidence.materialized_outputs.is_empty());
                if hit {
                    assert!(result.evidence.uploaded_digests.is_empty());
                }
            }
            assert_eq!(results[0].result.output_files()[0].path(), "seed");
            assert_eq!(
                results[0].result.output_files()[0].digest(),
                &ReapiDigest::of_bytes(b"seed")
            );
            let producer = &results[1].result;
            assert_eq!(producer.output_files()[0].path(), "file");
            assert_eq!(
                producer.output_files()[0].digest(),
                &ReapiDigest::of_bytes(bytes)
            );
            assert!(!producer.output_files()[0].is_executable());
            assert_eq!(producer.output_directories().len(), 2);
            let tree = producer
                .output_directories()
                .iter()
                .find(|tree| tree.path() == "tree")
                .unwrap();
            assert_eq!(tree.directories(), ["", "empty", "nested"]);
            assert_eq!(tree.files().len(), 1);
            assert_eq!(tree.files()[0].path(), "nested/value");
            assert_eq!(tree.files()[0].digest(), &ReapiDigest::of_bytes(bytes));
            assert!(!tree.files()[0].is_executable());
            let empty = producer
                .output_directories()
                .iter()
                .find(|tree| tree.path() == "emptytree")
                .unwrap();
            assert_eq!(empty.directories(), [""]);
            assert!(empty.files().is_empty());
            let selected = accepted.output().selected().unwrap().remote().unwrap();
            assert_eq!(selected.result.output_files()[0].path(), "done");
            assert_eq!(
                selected.result.output_files()[0].digest(),
                &ReapiDigest::of_bytes(&bytes.repeat(3))
            );
            assert_eq!(
                selected.result.output_directories()[0].path(),
                "result_tree"
            );
            assert_eq!(selected.result.output_directories()[0].directories(), [""]);
            assert!(selected.result.output_directories()[0].files().is_empty());
            let digests = results
                .iter()
                .map(|result| result.action_digest.clone())
                .collect::<Vec<_>>();
            if let Some(original) = &original {
                assert_eq!(original == &digests, bytes == b"aaa");
            } else {
                original = Some(digests);
            }
            TerminalOutput::new(0, String::new(), String::new())
        }));
        workspace.assert_no_outputs();
    }
    assert_eq!(transport.executions.load(Ordering::SeqCst), 12);
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and fixture CAS paths"]
fn nativelink_chain_generated_cas_eviction_and_corruption_block_consumer() {
    let workspace = Workspace::new();
    for (mutation, bytes) in [
        (CasMutation::Remove, b"evicted".as_slice()),
        (CasMutation::Corrupt, b"corrupt"),
    ] {
        std::fs::write(workspace.root.join("input"), bytes).unwrap();
        let transport = transport(Some(mutation));
        let error = workspace.run(&transport).unwrap_err().to_string();
        let expected = match mutation {
            CasMutation::Remove => "REAPI CAS is missing input",
            CasMutation::Corrupt => "download digest mismatch:",
        };
        assert!(error.contains(expected), "{error}");
        assert_eq!(
            transport.executions.load(Ordering::SeqCst),
            2,
            "consumer Execute must not start"
        );
        workspace.assert_no_outputs();
    }
    // Distinct public tool sources ensure these cannot reuse a valid producer AC result.
    for (exit, expected) in [(7, "exited 7"), (0, "output")] {
        let workspace = Workspace::new();
        std::fs::write(workspace.root.join("input"), b"producer-rejection").unwrap();
        std::fs::write(
            workspace.root.join("tools/tool"),
            TOOL.replace(
                "if [ \"$1\" = produce ]; then",
                &format!("if [ \"$1\" = produce ]; then\n  exit {exit}"),
            ),
        )
        .unwrap();
        let transport = transport(None);
        let error = workspace.run(&transport).unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
        assert_eq!(transport.executions.load(Ordering::SeqCst), 2);
        workspace.assert_no_outputs();
    }
}
