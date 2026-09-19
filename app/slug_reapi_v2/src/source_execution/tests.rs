use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use slug_analysis_v2::ConfiguredTargetKey;
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

#[test]
fn unsupported_source_transport_policy_fails_before_connecting() {
    let base = RemoteConfig::from_args(&["--remote_executor=grpc://127.0.0.1:1"]).unwrap();
    assert!(SourceReapiTransport::new(base.clone()).is_ok());
    for variant in 0..5 {
        let mut config = base.clone();
        match variant {
            0 => {
                config
                    .headers
                    .insert("authorization".into(), "unused".into());
            }
            1 => config.retry_attempts = Some(0),
            2 => config.timeout_seconds = Some(1),
            3 => config.cache = Some("grpc://127.0.0.1:2".into()),
            _ => config.executor = None,
        }
        assert!(SourceReapiTransport::new(config).is_err());
    }
}

struct Workspace {
    root: std::path::PathBuf,
    runtime: WorkspaceRuntime,
    owner: ConfiguredTargetKey,
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl Workspace {
    fn new() -> Self {
        Self::in_package("")
    }
    fn in_package(package: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/wp735/wire-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        fixture::write(&root);
        std::fs::create_dir_all(root.join("tools")).unwrap();
        std::fs::write(
            root.join("tools/tool"),
            b"#!/bin/sh\n/bin/cat input > shared.out\n",
        )
        .unwrap();
        let build = std::fs::read_to_string(root.join("BUILD.bazel"))
            .unwrap()
            .replace("'tool'", "'tools/tool'");
        std::fs::write(root.join("BUILD.bazel"), build).unwrap();
        if !package.is_empty() {
            std::fs::create_dir_all(root.join(package)).unwrap();
            std::fs::write(root.join(package).join("defs.bzl"), fixture::DEFS).unwrap();
            std::fs::write(root.join(package).join("BUILD.bazel"),
                "load(':defs.bzl', 'stage')\nstage(name='one', input='//:input', tool='//:tools/tool')\n").unwrap();
        }
        let runtime = WorkspaceRuntime::new(&root, ProcessHostOwner::native()).unwrap();
        let mut owner = None;
        drop(
            runtime
                .build_command_with_repository_environment(
                    &[TargetPattern::parse(&format!("//{package}:one")).unwrap()],
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
    fn prepare(
        &self,
    ) -> Result<Arc<PreparedSourceActionInputs>, slug_core_v2::runtime::BuildCommandError> {
        let accepted = self
            .runtime
            .prepare_source_action_inputs_with_repository_environment(
                &[TargetPattern::parse(&format!(
                    "//{}:one",
                    self.owner.label().package().package().as_str()
                ))
                .unwrap()],
                self.owner.clone(),
                0,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!("file://{}/empty-registry", self.root.display())],
                Default::default(),
                Default::default(),
            )?;
        let mut prepared = None;
        drop(accepted.project(|value| {
            prepared = Some(value.clone());
            TerminalOutput::new(0, String::new(), String::new())
        }));
        Ok(prepared.unwrap())
    }
    fn run<T: SourceActionTransport>(
        &self,
        transport: &T,
    ) -> Result<
        slug_core_v2::runtime::AcceptedCommand<
            slug_core_v2::runtime::SourceActionResult<T::Output>,
        >,
        slug_core_v2::runtime::BuildCommandError,
    > {
        self.runtime
            .execute_source_action_with_repository_environment(
                &[TargetPattern::parse(&format!(
                    "//{}:one",
                    self.owner.label().package().package().as_str()
                ))
                .unwrap()],
                self.owner.clone(),
                0,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!("file://{}/empty-registry", self.root.display())],
                Default::default(),
                Default::default(),
                transport,
            )
    }
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink"]
fn nativelink_native_source_execute_cache_change_and_incomplete_cas() {
    let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap();
    let config = RemoteConfig::from_args(&[&format!("--remote_executor={endpoint}")]).unwrap();
    let transport = SourceReapiTransport::new(config).unwrap();
    let workspace = Workspace::new();
    let mut original_digest = None;
    for (bytes, hit) in [(b"aaa".as_slice(), false), (b"aaa", true), (b"bbb", false)] {
        std::fs::write(workspace.root.join("input"), bytes).unwrap();
        drop(workspace.run(&transport).unwrap().project(|accepted| {
            let output = accepted.output();
            assert_eq!(output.output_blobs.get("shared.out").unwrap(), bytes);
            assert_eq!(
                output.result.output_files()[0].digest(),
                &ReapiBlob::from_bytes(bytes.to_vec()).digest().clone()
            );
            assert_eq!(output.evidence.ac_hits, u64::from(hit));
            assert_eq!(output.evidence.ac_misses, u64::from(!hit));
            assert!(output.evidence.materialized_outputs.is_empty());
            if let Some(previous) = &original_digest {
                assert_eq!(&output.action_digest == previous, bytes == b"aaa");
            } else {
                original_digest = Some(output.action_digest.clone());
            }
            if hit {
                assert!(output.evidence.uploaded_digests.is_empty());
            }
            TerminalOutput::new(0, String::new(), String::new())
        }));
        assert!(!workspace.root.join("shared.out").exists());
    }
    std::fs::write(workspace.root.join("input"), b"incomplete-required-source").unwrap();
    let incomplete = IncompleteCas {
        transport,
        executions: AtomicUsize::new(0),
    };
    let error = workspace.run(&incomplete).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("incomplete CAS prevented staging"),
        "{error}"
    );
    assert_eq!(incomplete.executions.load(Ordering::SeqCst), 0);
    assert!(!workspace.root.join("shared.out").exists());
}

struct IncompleteCas {
    transport: SourceReapiTransport,
    executions: AtomicUsize,
}
impl SourceActionTransport for IncompleteCas {
    type Staged = StagedSourceAction;
    type Output = RemoteExecutionResult;
    type Error = RemoteExecutionError;
    async fn stage(
        &self,
        inputs: Arc<PreparedSourceActionInputs>,
    ) -> Result<Self::Staged, Self::Error> {
        use crate::proto::google::bytestream;
        let plan =
            SourceSpawnReapiPlan::from_prepared(inputs.clone(), &Default::default()).unwrap();
        let digest = plan
            .inputs()
            .input_tree()
            .entries()
            .iter()
            .find(|entry| entry.path() == "input")
            .unwrap()
            .digest()
            .clone();
        let channel = tonic::transport::Endpoint::from_shared(
            tonic_endpoint(self.transport.config.executor.as_deref().unwrap()).unwrap(),
        )
        .unwrap()
        .connect()
        .await
        .unwrap();
        let cache =
            CacheClient::new(channel.clone(), String::new(), TransferPolicy::default()).unwrap();
        let requested = [digest.clone()].into_iter().collect();
        assert_eq!(cache.find_missing(&requested).await.unwrap(), requested);
        let (send, receive) = tokio::sync::mpsc::channel(1);
        send.send(bytestream::WriteRequest {
            resource_name: format!("uploads/wp734-incomplete/blobs/{digest}"),
            write_offset: 0,
            finish_write: false,
            data: b"wrong partial".to_vec(),
        })
        .await
        .unwrap();
        let writer = tokio::spawn(async move {
            bytestream::byte_stream_client::ByteStreamClient::new(channel)
                .write(tonic::codegen::tokio_stream::wrappers::ReceiverStream::new(
                    receive,
                ))
                .await
        });
        // Abort-on-drop also covers an assertion unwind before the normal join.
        struct PendingWriter(
            tokio::task::JoinHandle<
                Result<tonic::Response<bytestream::WriteResponse>, tonic::Status>,
            >,
        );
        impl Drop for PendingWriter {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let mut writer = PendingWriter(writer);
        tokio::time::timeout(std::time::Duration::from_millis(500), async {
            while !cache.find_missing(&requested).await.unwrap().is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("in-flight source must be advertised present");
        let staging = self.transport.stage(inputs);
        tokio::pin!(staging);
        let early = tokio::time::timeout(std::time::Duration::from_millis(50), &mut staging).await;
        assert!(
            !matches!(early, Ok(Ok(_))),
            "in-flight CAS must never certify staging"
        );
        assert!(
            !writer.0.is_finished(),
            "raw upload remains in flight during the check"
        );
        drop(send);
        writer.0.abort();
        assert!((&mut writer.0).await.unwrap_err().is_cancelled());
        // NativeLink retains disconnected writes for 1s and sweeps every .5s.
        // Require the adapter's actual failure, never treat timeout as rejection.
        let terminal = match early {
            Ok(result) => result,
            Err(_) => tokio::time::timeout(std::time::Duration::from_secs(2), &mut staging)
                .await
                .expect("adapter must reject after incomplete upload cleanup"),
        };
        let error = match terminal {
            Err(error) => error,
            Ok(_) => panic!("incomplete CAS must fail verification"),
        };
        Err(RemoteExecutionError::Protocol(format!(
            "incomplete CAS prevented staging: {error}"
        )))
    }
    async fn execute(&self, staged: Self::Staged) -> Result<Self::Output, Self::Error> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        self.transport.execute(staged).await
    }
}

fn tree_defs() -> String {
    fixture::DEFS.replace("out = ctx.actions.declare_file('shared.out')", "out = ctx.actions.declare_directory('tree', sibling=None)\n    repeated = ctx.actions.declare_directory(filename='tree')\n    if not out.is_directory or out.is_source or out != repeated or ctx.attr.input[DefaultInfo].files.to_list()[0].is_directory:\n        fail('directory artifact metadata')\n    plain = ctx.actions.declare_file('a.out')")
        .replace("outputs = [out]", "outputs = [out, plain]")
}

#[test]
fn native_directory_declarations_and_command_paths_follow_retained_kinds() {
    use prost::Message;
    let workspace = Workspace::in_package("pkg");
    let defs = tree_defs();
    std::fs::write(workspace.root.join("pkg/defs.bzl"), &defs).unwrap();
    let prepared = workspace.prepare().unwrap();
    let owner = prepared.configured_action().context().owner();
    assert_eq!(owner, &workspace.owner);
    let plan = SourceSpawnReapiPlan::from_prepared(prepared, &Default::default()).unwrap();
    assert_eq!(plan.command().output_directories, ["pkg/tree"]);
    assert_eq!(plan.command().output_files, ["pkg/a.out"]);
    let command = crate::proto::Command::decode(plan.command().serialized().as_slice()).unwrap();
    assert_eq!(command.output_paths, ["pkg/a.out", "pkg/tree"]);
    for (changed, expected) in [
        (defs.replace("repeated = ctx.actions.declare_directory(filename='tree')", "repeated = ctx.actions.declare_file('tree')"), "different type"),
        (defs.replace("out = ctx.actions.declare_directory", "reserved = ctx.actions.declare_file('tree')\n    out = ctx.actions.declare_directory"), "different type"),
        (defs.replace("stage = rule(implementation = _impl, attrs", "stage = rule(implementation = _impl, outputs={'reserved':'tree'}, attrs"), "different type"),
        (defs.replace("'tree', sibling=None", "'../tree', sibling=None"), "invalid action output"),
        (defs.replace("'tree', sibling=None", "'tree', sibling=ctx.attr.input[DefaultInfo].files.to_list()[0]"), "sibling is not admitted"),
        (defs.replace("DefaultInfo(files = depset([out]))", "DefaultInfo(files = depset([out]), executable = out)"), "executable must be a declared file"),
    ] {
        std::fs::write(workspace.root.join("pkg/defs.bzl"), changed).unwrap();
        let error = workspace.prepare().unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
    }
    std::fs::write(workspace.root.join("pkg/defs.bzl"), defs).unwrap();
    assert_eq!(
        SourceSpawnReapiPlan::from_prepared(workspace.prepare().unwrap(), &Default::default())
            .unwrap()
            .identity(),
        plan.identity()
    );
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink"]
fn nativelink_source_output_tree_cold_cache_restore_and_missing_descendant() {
    use prost::Message;

    use crate::command::digest_to_proto;
    let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap();
    let config = RemoteConfig::from_args(&[&format!("--remote_executor={endpoint}")]).unwrap();
    let transport = SourceReapiTransport::new(config).unwrap();
    let workspace = Workspace::new();
    std::fs::write(workspace.root.join("defs.bzl"), tree_defs()).unwrap();
    std::fs::write(workspace.root.join("tools/tool"), b"#!/bin/sh\n/bin/mkdir -p tree/nested tree/empty\n/bin/cat input > tree/nested/value\n/bin/chmod 0644 tree/nested/value\n/bin/cat input > a.out\n").unwrap();
    let mut original = None;
    let mut last = None;
    for (bytes, hit) in [
        (b"aaa".as_slice(), false),
        (b"aaa", true),
        (b"bbb", false),
        (b"aaa", true),
    ] {
        std::fs::write(workspace.root.join("input"), bytes).unwrap();
        drop(workspace.run(&transport).unwrap().project(|accepted| {
            let output = accepted.output();
            let [directory] = output.result.output_directories() else {
                panic!("one tree expected")
            };
            assert_eq!(directory.path(), "tree");
            assert_eq!(directory.directories(), ["", "empty", "nested"]);
            let [file] = directory.files() else {
                panic!("one descendant expected")
            };
            assert_eq!(file.path(), "nested/value");
            assert_eq!(file.digest(), &ReapiDigest::of_bytes(bytes));
            assert!(!file.is_executable());
            assert_eq!(
                output.output_blobs.len(),
                1,
                "tree contents are verified but not retained"
            );
            assert_eq!(output.output_blobs["a.out"], bytes);
            assert_eq!(output.evidence.ac_hits, u64::from(hit));
            assert_eq!(output.evidence.ac_misses, u64::from(!hit));
            if let Some(original) = &original {
                assert_eq!(original == &output.action_digest, bytes == b"aaa");
            } else {
                original = Some(output.action_digest.clone());
            }
            let local = workspace.root.join("not-published");
            assert!(
                crate::materialize_outputs(&local, output)
                    .unwrap_err()
                    .to_string()
                    .contains("tree materialization")
            );
            assert!(
                !local.exists(),
                "mixed output rejection precedes even regular file writes"
            );
            last = Some(output.clone());
            TerminalOutput::new(0, String::new(), String::new())
        }));
    }
    let last = last.unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let channel = tonic::transport::Endpoint::from_shared(tonic_endpoint(&endpoint).unwrap())
            .unwrap()
            .connect()
            .await
            .unwrap();
        let cache =
            CacheClient::new(channel.clone(), String::new(), TransferPolicy::default()).unwrap();
        let missing = ReapiDigest::of_bytes(b"never uploaded descendant");
        assert!(
            !cache
                .find_missing(&[missing.clone()].into_iter().collect())
                .await
                .unwrap()
                .is_empty()
        );
        let tree = crate::proto::Tree {
            root: Some(crate::proto::Directory {
                files: vec![crate::proto::FileNode {
                    name: "missing".into(),
                    digest: Some(digest_to_proto(&missing)),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            children: vec![],
        };
        let blob = ReapiBlob::from_bytes(tree.encode_to_vec());
        cache.upload_missing(&[blob.clone()]).await.unwrap();
        let poisoned = crate::proto::ActionResult {
            output_files: vec![crate::proto::OutputFile {
                path: "a.out".into(),
                digest: Some(digest_to_proto(last.result.output_files()[0].digest())),
                ..Default::default()
            }],
            output_directories: vec![crate::proto::OutputDirectory {
                path: "tree".into(),
                tree_digest: Some(digest_to_proto(blob.digest())),
                ..Default::default()
            }],
            ..Default::default()
        };
        crate::proto::action_cache_client::ActionCacheClient::new(channel)
            .update_action_result(crate::proto::UpdateActionResultRequest {
                action_digest: Some(digest_to_proto(&last.action_digest)),
                action_result: Some(poisoned),
                ..Default::default()
            })
            .await
            .unwrap();
    });
    let error = workspace.run(&transport).unwrap_err().to_string();
    assert!(
        error.contains("NOT_FOUND") || error.contains("NotFound") || error.contains("not found"),
        "{error}"
    );
    assert!(!workspace.root.join("tree").exists());
    assert!(!workspace.root.join("a.out").exists());
}
