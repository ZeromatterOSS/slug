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

use crate::cas_fixture;

fn source_defs() -> String {
    format!(
        "def _resources(os, count):\n    fail('real-source-resource-must-not-execute')\n{}",
        fixture::DEFS.replace(
            "arguments = [args]",
            "arguments = [args, out.path], resource_set = _resources"
        )
    )
}

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
        std::fs::write(root.join("defs.bzl"), source_defs()).unwrap();
        std::fs::create_dir_all(root.join("tools")).unwrap();
        std::fs::write(
            root.join("tools/tool"),
            b"#!/bin/sh\n/bin/cat input > \"$2\"\n",
        )
        .unwrap();
        let build = std::fs::read_to_string(root.join("BUILD.bazel"))
            .unwrap()
            .replace("'tool'", "'tools/tool'");
        std::fs::write(root.join("BUILD.bazel"), build).unwrap();
        if !package.is_empty() {
            std::fs::create_dir_all(root.join(package)).unwrap();
            std::fs::write(root.join(package).join("defs.bzl"), source_defs()).unwrap();
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
    ) -> Result<
        slug_core_v2::runtime::NativeSourceActionInputs,
        slug_core_v2::runtime::BuildCommandError,
    > {
        let accepted = self
            .runtime
            .prepare_native_source_action_inputs_with_repository_environment(
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
fn different_original_resource_callbacks_preserve_identical_reapi_command_and_action_bytes() {
    let workspace = Workspace::new();
    let first = workspace.prepare().unwrap();
    let original = first.spawn().resource_set().unwrap();
    assert!(original.published_callable().original_slot().is_some());
    let first_plan = SourceSpawnReapiPlan::from_native(
        first.clone(),
        &Default::default(),
        &slug_build_api_v2::UnsupportedTreeArgsDirectoryContext,
    )
    .unwrap();
    let source = std::fs::read_to_string(workspace.root.join("defs.bzl")).unwrap();
    assert!(source.contains("real-source-resource-must-not-execute"));
    std::fs::write(
        workspace.root.join("defs.bzl"),
        source.replace(
            "real-source-resource-must-not-execute",
            "different-original-resource-failure",
        ),
    )
    .unwrap();
    let second = workspace.prepare().unwrap();
    let second_plan = SourceSpawnReapiPlan::from_native(
        second.clone(),
        &Default::default(),
        &slug_build_api_v2::UnsupportedTreeArgsDirectoryContext,
    )
    .unwrap();
    assert_ne!(first.configured_action(), second.configured_action());
    assert_ne!(original, second.spawn().resource_set().unwrap());
    assert_eq!(
        first_plan.command().serialized(),
        second_plan.command().serialized()
    );
    assert_eq!(
        first_plan.identity().action_bytes(),
        second_plan.identity().action_bytes()
    );
    std::fs::write(workspace.root.join("defs.bzl"), source).unwrap();
    assert_eq!(
        first.configured_action(),
        workspace.prepare().unwrap().configured_action()
    );
}

#[test]
fn source_counter_preserves_original_and_refuses_callback_free_prepared_action() {
    let workspace = Workspace::new();
    let original = workspace.prepare().unwrap();
    let (wrapped, counter) = original.with_resource_set_invocation_counter().unwrap();
    assert_eq!(wrapped.metadata(), original.metadata());
    assert!(
        wrapped
            .spawn()
            .resource_set()
            .unwrap()
            .published_callable()
            .same_original(
                original
                    .spawn()
                    .resource_set()
                    .unwrap()
                    .published_callable()
            )
    );
    let error = wrapped
        .spawn()
        .resource_set()
        .unwrap()
        .evaluate(
            slug_build_api_v2::actions::ResourceSetInputs {
                os: slug_build_api_v2::actions::ResourceSetOs::Linux,
                parent_input_count: 2,
            },
            &resource_support::Context::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("real-source-resource-must-not-execute"),
        "{error}"
    );
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    counter.store(0, Ordering::SeqCst);
    let plan = SourceSpawnReapiPlan::from_native(
        wrapped,
        &Default::default(),
        &slug_build_api_v2::UnsupportedTreeArgsDirectoryContext,
    )
    .unwrap();
    assert!(!plan.command().serialized().is_empty());
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    std::fs::write(
        workspace.root.join("defs.bzl"),
        source_defs().replace(", resource_set = _resources", ""),
    )
    .unwrap();
    assert!(
        workspace
            .prepare()
            .unwrap()
            .with_resource_set_invocation_counter()
            .is_err()
    );
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink"]
fn nativelink_native_source_execute_cache_change_and_incomplete_cas() {
    let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap();
    let config = RemoteConfig::from_args(&[&format!("--remote_executor={endpoint}")]).unwrap();
    let transport = CountingSourceTransport {
        inner: SourceReapiTransport::new(config).unwrap(),
        counters: std::sync::Mutex::new(Vec::new()),
    };
    let workspace = Workspace::new();
    let prepared = workspace.prepare().unwrap();
    assert!(prepared.spawn().resource_set().is_some());
    drop(prepared);
    let mut original_digest = None;
    for (bytes, hit) in [(b"aaa".as_slice(), false), (b"aaa", true), (b"bbb", false)] {
        std::fs::write(workspace.root.join("input"), bytes).unwrap();
        drop(workspace.run(&transport).unwrap().project(|accepted| {
            let output = accepted.output();
            assert_eq!(
                output
                    .output_blobs
                    .get(&crate::test_roots::owner_path(
                        &workspace.owner,
                        "shared.out"
                    ))
                    .unwrap(),
                bytes
            );
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
        transport.assert_zero();
        assert!(!workspace.root.join("shared.out").exists());
    }
    assert_eq!(transport.counters.lock().unwrap().len(), 3);
    std::fs::write(workspace.root.join("input"), b"incomplete-required-source").unwrap();
    let incomplete = PartialAndCorruptCas {
        transport,
        executions: AtomicUsize::new(0),
    };
    let error = workspace.run(&incomplete).unwrap_err();
    assert!(
        error.to_string().contains("download digest mismatch:"),
        "{error}"
    );
    assert_eq!(incomplete.executions.load(Ordering::SeqCst), 0);
    incomplete.transport.assert_zero();
    assert!(!workspace.root.join("shared.out").exists());
}

// The actual Core-selected DTO is instrumented at the real transport boundary.
// This fixed wrapper never supplies a replacement callback or staging capability.
struct CountingSourceTransport {
    inner: SourceReapiTransport,
    counters: std::sync::Mutex<Vec<Arc<AtomicUsize>>>,
}
impl CountingSourceTransport {
    fn assert_zero(&self) {
        for counter in self.counters.lock().unwrap().iter() {
            assert_eq!(
                counter.load(Ordering::SeqCst),
                0,
                "remote source invoked numeric resource callback"
            );
        }
    }
}
impl SourceActionTransport for CountingSourceTransport {
    type Staged = StagedSourceAction;
    type Output = RemoteExecutionResult;
    type Error = RemoteExecutionError;
    async fn stage(
        &self,
        inputs: slug_core_v2::runtime::NativeSourceActionInputs,
        args_context: &dyn slug_build_api_v2::ArgsDirectoryContext,
    ) -> Result<Self::Staged, Self::Error> {
        let (wrapped, counter) = inputs.with_resource_set_invocation_counter().unwrap();
        assert_eq!(wrapped.metadata(), inputs.metadata());
        // Positive control on the SAME DTO passed below, completed before reset.
        let error = wrapped
            .spawn()
            .resource_set()
            .unwrap()
            .evaluate(
                slug_build_api_v2::actions::ResourceSetInputs {
                    os: slug_build_api_v2::actions::ResourceSetOs::Linux,
                    parent_input_count: 2,
                },
                &resource_support::Context::default(),
            )
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("real-source-resource-must-not-execute"),
            "{error}"
        );
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        counter.store(0, Ordering::SeqCst);
        self.counters.lock().unwrap().push(counter);
        self.assert_zero();
        let result = self.inner.stage(wrapped, args_context).await;
        self.assert_zero();
        result
    }
    async fn execute(&self, staged: Self::Staged) -> Result<Self::Output, Self::Error> {
        self.assert_zero();
        let result = self.inner.execute(staged).await;
        self.assert_zero();
        result
    }
}

struct PartialAndCorruptCas {
    transport: CountingSourceTransport,
    executions: AtomicUsize,
}
impl SourceActionTransport for PartialAndCorruptCas {
    type Staged = StagedSourceAction;
    type Output = RemoteExecutionResult;
    type Error = RemoteExecutionError;
    async fn stage(
        &self,
        inputs: slug_core_v2::runtime::NativeSourceActionInputs,
        args_context: &dyn slug_build_api_v2::ArgsDirectoryContext,
    ) -> Result<Self::Staged, Self::Error> {
        use crate::proto::google::bytestream;
        let plan =
            SourceSpawnReapiPlan::from_native(inputs.clone(), &Default::default(), args_context)
                .unwrap();
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
            tonic_endpoint(self.transport.inner.config.executor.as_deref().unwrap()).unwrap(),
        )
        .unwrap()
        .connect()
        .await
        .unwrap();
        let cache =
            CacheClient::new(channel.clone(), String::new(), TransferPolicy::default()).unwrap();
        let requested = [digest.clone()].into_iter().collect();
        assert_eq!(cache.find_missing(&requested).await.unwrap(), requested);
        let resource = format!("uploads/wp734-incomplete/blobs/{digest}");
        let partial = b"wrong partial";
        assert_eq!(partial.len(), 13);
        let mut status_client =
            bytestream::byte_stream_client::ByteStreamClient::new(channel.clone());
        let (send, receive) = tokio::sync::mpsc::channel(1);
        send.send(bytestream::WriteRequest {
            resource_name: resource.clone(),
            write_offset: 0,
            finish_write: false,
            data: partial.to_vec(),
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
        eprintln!("resource CAS fixture: before partial QWS");
        // Progress belongs to this upload resource, not FindMissingBlobs.
        // A missing digest may legitimately be repaired from the real source.
        tokio::time::timeout(std::time::Duration::from_millis(500), async {
            let mut previous_committed = 0;
            loop {
                match status_client
                    .query_write_status(bytestream::QueryWriteStatusRequest {
                        resource_name: resource.clone(),
                    })
                    .await
                {
                    Ok(response) => {
                        let status = response.into_inner();
                        assert!(
                            (0..=partial.len() as i64).contains(&status.committed_size),
                            "partial upload reported invalid progress: {status:?}"
                        );
                        assert!(!status.complete, "finish_write was never sent");
                        assert!(
                            status.committed_size >= previous_committed,
                            "partial upload progress regressed: {previous_committed} -> {}",
                            status.committed_size
                        );
                        previous_committed = status.committed_size;
                        if status.committed_size == partial.len() as i64 {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                    Err(status) if status.code() == tonic::Code::NotFound => {
                        tokio::task::yield_now().await;
                    }
                    Err(status) => panic!("partial upload status refused: {status}"),
                }
            }
        })
        .await
        .expect("server must report the actual partial upload within 500ms");
        eprintln!("resource CAS fixture: after actual QWS13/incomplete");
        assert!(!writer.0.is_finished(), "raw upload remains in flight");
        assert!(!send.is_closed(), "partial request stream remains open");
        eprintln!("resource CAS fixture: before active verified read observation");
        let active_read = cache.read_blob_verified(&digest, |_| Ok(()));
        tokio::pin!(active_read);
        // Keep the same future if pending; a deadline is not a read rejection.
        let observed_read =
            match tokio::time::timeout(std::time::Duration::from_millis(50), active_read.as_mut())
                .await
            {
                Ok(result) => {
                    assert!(result.is_err(), "unfinished wrong source must not verify");
                    Some(result)
                }
                Err(_) => None,
            };
        eprintln!("resource CAS fixture: after active verified read observation");
        self.transport.assert_zero();
        assert_eq!(self.executions.load(Ordering::SeqCst), 0);
        // One deadline covers the actual join, old read, retirement and fresh
        // typed missing proof. An old transport error alone proves no integrity.
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            eprintln!("resource CAS fixture: before writer abort/join");
            drop(send);
            writer.0.abort();
            assert!((&mut writer.0).await.unwrap_err().is_cancelled());
            eprintln!("resource CAS fixture: after writer abort/join");
            let original_result = match observed_read {
                Some(result) => result,
                None => active_read.as_mut().await,
            };
            let original_error =
                original_result.expect_err("original incomplete-source read must actually fail");
            match original_error {
                slug_reapi_cache_v2::CacheError::Protocol(message) => {
                    let actual_cas_error = message
                        .strip_prefix("CAS status ")
                        .and_then(|status| status.split_once(": "))
                        .and_then(|(code, _)| code.parse::<i32>().ok())
                        .is_some_and(|code| code != 0);
                    assert!(
                        actual_cas_error || message.starts_with("download digest mismatch:"),
                        "{message}"
                    );
                }
                slug_reapi_cache_v2::CacheError::Transport(status) => {
                    assert_eq!(status.code(), tonic::Code::NotFound, "{status}");
                }
                error => panic!("original read had no actual CAS error evidence: {error}"),
            }
            eprintln!("resource CAS fixture: after actual original read error");
            eprintln!("resource CAS fixture: before QWS/CAS cleanup");
            loop {
                match status_client
                    .query_write_status(bytestream::QueryWriteStatusRequest {
                        resource_name: resource.clone(),
                    })
                    .await
                {
                    Err(status) if status.code() == tonic::Code::NotFound => break,
                    Ok(response) => {
                        let status = response.into_inner();
                        assert!(!status.complete);
                        if status.committed_size == 0 {
                            break;
                        }
                        assert_eq!(status.committed_size, partial.len() as i64);
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    }
                    Err(status) => panic!("partial upload cleanup status refused: {status}"),
                }
            }
            assert_eq!(cache.find_missing(&requested).await.unwrap(), requested);
            eprintln!("resource CAS fixture: before fresh typed missing read");
            let missing = cache
                .read_blob_verified(&digest, |_| Ok(()))
                .await
                .expect_err("retired incomplete source must actually be missing");
            match missing {
                slug_reapi_cache_v2::CacheError::Protocol(message) => {
                    assert!(message.starts_with("CAS status 5:"), "{message}");
                }
                slug_reapi_cache_v2::CacheError::Transport(status) => {
                    assert_eq!(status.code(), tonic::Code::NotFound, "{status}");
                }
                error => panic!("fresh read did not establish actual CAS missing: {error}"),
            }
            eprintln!("resource CAS fixture: after fresh typed missing read");
        })
        .await
        .expect("aborted upload must actually retire within the existing 2s bound");
        eprintln!("resource CAS fixture: after actual QWS/CAS cleanup");
        self.transport.assert_zero();
        assert_eq!(self.executions.load(Ordering::SeqCst), 0);

        // Publish the SAME genuine source plan normally before adversarial fault.
        eprintln!("resource CAS fixture: before genuine full plan upload");
        plan.inputs().upload_missing(&cache).await.unwrap();
        eprintln!("resource CAS fixture: after genuine full plan upload");
        assert!(cache.find_missing(&requested).await.unwrap().is_empty());
        cache.read_blob_verified(&digest, |_| Ok(())).await.unwrap();
        let paths = cas_fixture::completed_blob_paths(&digest);
        let corrupt = vec![b'!'; digest.size_bytes() as usize];
        assert_ne!(ReapiDigest::of_bytes(&corrupt), digest);
        // Both stores and every generation were checked BEFORE either is changed.
        for path in paths {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut permissions = path.metadata().unwrap().permissions();
                permissions.set_mode(permissions.mode() | 0o200);
                std::fs::set_permissions(&path, permissions).unwrap();
            }
            std::fs::write(path, &corrupt).unwrap();
        }
        assert!(
            cache.find_missing(&requested).await.unwrap().is_empty(),
            "completed corrupt source must still be advertised present"
        );
        eprintln!("resource CAS fixture: before corrupt actual source stage");
        let error = match self.transport.stage(inputs, args_context).await {
            Err(error) => error,
            Ok(_) => panic!("corrupt advertised source must never certify staging"),
        };
        eprintln!("resource CAS fixture: after corrupt actual source stage: {error}");
        assert!(
            error.to_string().contains("download digest mismatch:"),
            "{error}"
        );
        self.transport.assert_zero();
        assert_eq!(self.executions.load(Ordering::SeqCst), 0);
        Err(error) // The actual transport error, never a fabricated source refusal.
    }
    async fn execute(&self, staged: Self::Staged) -> Result<Self::Output, Self::Error> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        self.transport.execute(staged).await
    }
}

fn tree_defs() -> String {
    source_defs().replace("out = ctx.actions.declare_file('shared.out')", "out = ctx.actions.declare_directory('tree', sibling=None)\n    repeated = ctx.actions.declare_directory(filename='tree')\n    if not out.is_directory or out.is_source or out != repeated or ctx.attr.input[DefaultInfo].files.to_list()[0].is_directory:\n        fail('directory artifact metadata')\n    plain = ctx.actions.declare_file('a.out')")
        .replace("outputs = [out]", "outputs = [out, plain]")
        .replace("arguments = [args, out.path]", "arguments = [args, out.path, plain.path]")
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
    let plan = SourceSpawnReapiPlan::from_native(
        prepared,
        &Default::default(),
        &slug_build_api_v2::UnsupportedTreeArgsDirectoryContext,
    )
    .unwrap();
    assert_eq!(
        plan.command().output_directories,
        [crate::test_roots::owner_path(&workspace.owner, "pkg/tree")]
    );
    assert_eq!(
        plan.command().output_files,
        [crate::test_roots::owner_path(&workspace.owner, "pkg/a.out")]
    );
    let command = crate::proto::Command::decode(plan.command().serialized().as_slice()).unwrap();
    assert_eq!(
        command.output_paths,
        ["pkg/a.out", "pkg/tree"].map(|path| crate::test_roots::owner_path(&workspace.owner, path))
    );
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
        SourceSpawnReapiPlan::from_native(
            workspace.prepare().unwrap(),
            &Default::default(),
            &slug_build_api_v2::UnsupportedTreeArgsDirectoryContext
        )
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
    std::fs::write(workspace.root.join("tools/tool"), b"#!/bin/sh\n/bin/mkdir -p \"$2/nested\" \"$2/empty\"\n/bin/cat input > \"$2/nested/value\"\n/bin/chmod 0644 \"$2/nested/value\"\n/bin/cat input > \"$3\"\n").unwrap();
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
            assert_eq!(
                directory.path(),
                crate::test_roots::owner_path(&workspace.owner, "tree")
            );
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
            assert_eq!(
                output.output_blobs[&crate::test_roots::owner_path(&workspace.owner, "a.out")],
                bytes
            );
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
        let [file] = last.result.output_files() else {
            panic!("one regular output required for missing-descendant fixture")
        };
        let [directory] = last.result.output_directories() else {
            panic!("one tree output required for missing-descendant fixture")
        };
        let poisoned = crate::proto::ActionResult {
            output_files: vec![crate::proto::OutputFile {
                path: file.path().into(),
                digest: Some(digest_to_proto(file.digest())),
                ..Default::default()
            }],
            output_directories: vec![crate::proto::OutputDirectory {
                path: directory.path().into(),
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

#[path = "../../../slug_core_v2/src/runtime/source_staging/resource_set_test_support.rs"]
mod resource_support;
