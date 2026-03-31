/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::ffi::OsStr;
use std::ffi::OsString;
use std::ops::ControlFlow;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::From;
use dice_futures::cancellation::CancellationContext;
use dice_futures::cancellation::CancellationObserver;
use dupe::Dupe;
use futures::future;
use futures::future::Either;
use futures::future::FutureExt;
use futures::future::Shared;
use futures::future::join_all;
use futures::stream::StreamExt;
use gazebo::prelude::*;
use host_sharing::HostSharingBroker;
use host_sharing::HostSharingRequirements;
use host_sharing::host_sharing::HostSharingGuard;
use indexmap::IndexMap;
use kuro_build_signals::env::WaitingCategory;
use kuro_common::file_ops::metadata::FileDigestConfig;
use kuro_common::liveliness_observer::LivelinessObserver;
use kuro_common::liveliness_observer::LivelinessObserverExt;
use kuro_common::liveliness_observer::NoopLivelinessObserver;
use kuro_common::local_resource_state::LocalResourceHolder;
use kuro_core::content_hash::ContentBasedPathHash;
use kuro_core::fs::artifact_path_resolver::ArtifactFs;
use kuro_core::fs::buck_out_path::BuildArtifactPath;
use kuro_core::fs::project_rel_path::ProjectRelativePath;
use kuro_core::fs::project_rel_path::ProjectRelativePathBuf;
use kuro_core::soft_error;
use kuro_core::tag_error;
use kuro_core::tag_result;
use kuro_error::BuckErrorContext;
use kuro_error::kuro_error;
use kuro_events::daemon_id::DaemonId;
use kuro_events::dispatch::EventDispatcher;
use kuro_events::dispatch::get_dispatcher_opt;
use kuro_execute::artifact_utils::ArtifactValueBuilder;
use kuro_execute::artifact_value::ArtifactValue;
use kuro_execute::digest_config::DigestConfig;
use kuro_execute::directory::extract_artifact_value;
use kuro_execute::directory::insert_entry;
use kuro_execute::entry::HashingInfo;
use kuro_execute::entry::build_entry_from_disk;
use kuro_execute::execute::action_digest::ActionDigest;
use kuro_execute::execute::blocking::BlockingExecutor;
use kuro_execute::execute::clean_output_paths::CleanOutputPaths;
use kuro_execute::execute::environment_inheritance::EnvironmentInheritance;
use kuro_execute::execute::executor_stage_async;
use kuro_execute::execute::inputs_directory::inputs_directory;
use kuro_execute::execute::kind::CommandExecutionKind;
use kuro_execute::execute::manager::CommandExecutionManager;
use kuro_execute::execute::manager::CommandExecutionManagerExt;
use kuro_execute::execute::manager::CommandExecutionManagerWithClaim;
use kuro_execute::execute::output::CommandStdStreams;
use kuro_execute::execute::prepared::PreparedCommand;
use kuro_execute::execute::prepared::PreparedCommandExecutor;
use kuro_execute::execute::request::CommandExecutionInput;
use kuro_execute::execute::request::CommandExecutionOutput;
use kuro_execute::execute::request::CommandExecutionOutputRef;
use kuro_execute::execute::request::CommandExecutionRequest;
use kuro_execute::execute::request::ExecutorPreference;
use kuro_execute::execute::request::ParamFileFormat;
use kuro_execute::execute::result::CommandExecutionMetadata;
use kuro_execute::execute::result::CommandExecutionResult;
use kuro_execute::knobs::ExecutorGlobalKnobs;
use kuro_execute::materialize::materializer::CopiedArtifact;
use kuro_execute::materialize::materializer::DeclareArtifactPayload;
use kuro_execute::materialize::materializer::MaterializationError;
use kuro_execute::materialize::materializer::Materializer;
use kuro_execute::output_size::OutputSize;
use kuro_execute_local::CommandResult;
use kuro_execute_local::DefaultKillProcess;
use kuro_execute_local::GatherOutputStatus;
use kuro_execute_local::decode_command_event_stream;
use kuro_execute_local::maybe_absolutize_exe;
use kuro_execute_local::spawn_command_and_stream_events;
use kuro_execute_local::status_decoder::DefaultStatusDecoder;
use kuro_fs::async_fs_util;
use kuro_fs::fs_util;
use kuro_fs::paths::abs_norm_path::AbsNormPathBuf;
use kuro_fs::paths::abs_path::AbsPath;
use kuro_resource_control::ActionFreezeEvent;
use kuro_resource_control::ActionFreezeEventReceiver;
use kuro_resource_control::CommandType;
use kuro_resource_control::action_cgroups::ActionCgroupSession;
use kuro_resource_control::memory_tracker::MemoryTrackerHandle;
use kuro_resource_control::path::CgroupPathBuf;
use kuro_sandbox::SandboxSpec;
use kuro_util::process::background_command;
use kuro_util::time_span::TimeSpan;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::info;

use crate::executors::worker::WorkerHandle;
use crate::executors::worker::WorkerPool;
use crate::incremental_actions_helper::get_incremental_path_map;
use crate::incremental_actions_helper::save_content_based_incremental_state;
use crate::sqlite::incremental_state_db::IncrementalDbState;

#[derive(Debug, kuro_error::Error)]
#[kuro(input)]
enum LocalExecutionError {
    #[error("Args list was empty")]
    NoArgs,

    #[error("Trying to execute a remote-only action on a local executor")]
    RemoteOnlyAction,
}

#[derive(Clone, Dupe, Allocative)]
pub enum ForkserverAccess {
    None,
    #[cfg(unix)]
    Client(kuro_forkserver::client::ForkserverClient),
}

#[derive(Clone)]
pub struct LocalExecutor {
    artifact_fs: ArtifactFs,
    materializer: Arc<dyn Materializer>,
    incremental_db_state: Arc<IncrementalDbState>,
    blocking_executor: Arc<dyn BlockingExecutor>,
    pub(crate) host_sharing_broker: Arc<HostSharingBroker>,
    root: AbsNormPathBuf,
    forkserver: ForkserverAccess,
    #[allow(unused)]
    knobs: ExecutorGlobalKnobs,
    worker_pool: Option<Arc<WorkerPool>>,
    memory_tracker: Option<MemoryTrackerHandle>,
    daemon_id: DaemonId,
}

impl LocalExecutor {
    pub fn new(
        artifact_fs: ArtifactFs,
        materializer: Arc<dyn Materializer>,
        incremental_db_state: Arc<IncrementalDbState>,
        blocking_executor: Arc<dyn BlockingExecutor>,
        host_sharing_broker: Arc<HostSharingBroker>,
        root: AbsNormPathBuf,
        forkserver: ForkserverAccess,
        knobs: ExecutorGlobalKnobs,
        worker_pool: Option<Arc<WorkerPool>>,
        memory_tracker: Option<MemoryTrackerHandle>,
        daemon_id: DaemonId,
    ) -> Self {
        Self {
            artifact_fs,
            materializer,
            incremental_db_state,
            blocking_executor,
            host_sharing_broker,
            root,
            forkserver,
            knobs,
            worker_pool,
            memory_tracker,
            daemon_id,
        }
    }

    // Compiler gets confused (on the not(unix) branch only, weirdly) if you use an async fn.
    #[allow(clippy::manual_async_fn)]
    fn exec<'a>(
        &'a self,
        exe: &'a str,
        args: impl IntoIterator<Item = impl AsRef<OsStr> + Send> + Send + 'a,
        env: impl IntoIterator<Item = (impl AsRef<OsStr> + Send, impl AsRef<OsStr> + Send)> + Send + 'a,
        working_directory: &'a ProjectRelativePath,
        timeout: Option<Duration>,
        env_inheritance: Option<&'a EnvironmentInheritance>,
        liveliness_observer: impl LivelinessObserver + 'static,
        disable_miniperf: bool,
        cgroup: Option<CgroupPathBuf>,
        freeze_rx: impl ActionFreezeEventReceiver,
        sandbox: Option<SandboxSpec>,
    ) -> impl futures::future::Future<Output = kuro_error::Result<CommandResult>> + Send + 'a {
        async move {
            let working_directory = self.root.join_cow(working_directory);

            // When sandbox is active, bypass the forkserver and use direct process spawning.
            // The sandbox relies on pre_exec hooks (Linux namespaces) which are only available
            // in the direct spawn path, not via the forkserver's gRPC protocol.
            let effective_forkserver = if sandbox.is_some() {
                #[cfg(unix)]
                {
                    tracing::debug!("Sandbox enabled: bypassing forkserver to apply sandbox");
                }
                &ForkserverAccess::None
            } else {
                &self.forkserver
            };

            match effective_forkserver {
                #[cfg(unix)]
                ForkserverAccess::Client(forkserver) => {
                    unix::exec_via_forkserver(
                        forkserver,
                        exe,
                        args,
                        env,
                        &working_directory,
                        timeout,
                        env_inheritance,
                        liveliness_observer,
                        self.knobs.enable_miniperf && !disable_miniperf,
                        cgroup,
                        freeze_rx,
                    )
                    .await
                }
                ForkserverAccess::None => {
                    let _disable_miniperf = disable_miniperf;
                    let exe = maybe_absolutize_exe(exe, &working_directory)?;
                    let mut cmd = background_command(exe.as_ref());
                    cmd.current_dir(working_directory.as_path());
                    cmd.args(args);
                    apply_local_execution_environment(
                        &mut cmd,
                        &working_directory,
                        env,
                        env_inheritance,
                    );

                    // Apply filesystem sandbox if requested.
                    if let Some(sandbox_spec) = sandbox {
                        kuro_sandbox::apply_sandbox(&mut cmd, sandbox_spec);
                    }

                    let alive = liveliness_observer
                        .while_alive()
                        .map(|()| Ok(GatherOutputStatus::Cancelled));

                    let stream = spawn_command_and_stream_events(
                        cmd,
                        timeout,
                        alive,
                        DefaultStatusDecoder,
                        DefaultKillProcess::default(),
                        None,
                        true,
                        cgroup,
                        freeze_rx,
                    )
                    .await?;
                    decode_command_event_stream(stream).await
                }
                .with_buck_error_context(|| format!("Failed to gather output from command: {exe}")),
            }
        }
    }

    async fn exec_once(
        &self,
        action_digest: &ActionDigest,
        request: &CommandExecutionRequest,
        manager: CommandExecutionManagerWithClaim,
        cancellations: &CancellationContext,
        liveliness_observer: impl LivelinessObserver + 'static,
        scratch_path: &ScratchPath,
        args: &[String],
        worker: Option<&WorkerHandle>,
        env: &[(&str, StrOrOsStr<'_>)],
        cgroup: Option<CgroupPathBuf>,
        freeze_rx: impl ActionFreezeEventReceiver,
        sandbox: Option<SandboxSpec>,
    ) -> Result<
        (
            TimeSpan,
            SystemTime,
            CommandResult,
            CommandExecutionManagerWithClaim,
        ),
        CommandExecutionResult,
    > {
        if let Err(e) = executor_stage_async(
            kuro_data::LocalStage {
                stage: Some(kuro_data::LocalPrepareOutputDirs {}.into()),
            },
            async {
                tokio::try_join!(
                    create_output_dirs(
                        &self.artifact_fs,
                        request,
                        self.materializer.dupe(),
                        self.blocking_executor.dupe(),
                        cancellations,
                    ),
                    prep_scratch_path(&scratch_path, &self.artifact_fs),
                )
                .buck_error_context("Error creating output directories")?;

                kuro_error::Ok(())
            },
        )
        .boxed()
        .await
        {
            return Err(manager.error("prepare_output_dirs_failed", e));
        };

        let (time_span, start_time, res) = executor_stage_async(
            {
                let env = env
                    .iter()
                    .copied()
                    .map(|(k, v)| kuro_data::EnvironmentEntry {
                        key: k.to_owned(),
                        value: v.into_string_lossy(),
                    })
                    .collect();

                let stage = match worker {
                    None => kuro_data::LocalExecute {
                        command: Some(kuro_data::LocalCommand {
                            action_digest: action_digest.to_string(),
                            argv: args.to_vec(),
                            env,
                        }),
                    }
                    .into(),
                    Some(_) => kuro_data::WorkerExecute {
                        command: Some(kuro_data::WorkerCommand {
                            action_digest: action_digest.to_string(),
                            argv: request.args().to_vec(),
                            env,
                            fallback_exe: request.exe().to_vec(),
                        }),
                    }
                    .into(),
                };
                kuro_data::LocalStage { stage: Some(stage) }
            },
            async move {
                let execution_start = TimeSpan::start_now();
                let start_time = SystemTime::now();

                let env = env.iter().map(|(k, v)| (k, v.into_os_str()));
                let r = if let Some(worker) = worker {
                    let env: Vec<(OsString, OsString)> = env
                        .into_iter()
                        .map(|(k, v)| (OsString::from(k), v.to_owned()))
                        .collect();
                    Ok(worker
                        .exec_cmd(request.args(), env, request.timeout())
                        .await)
                } else {
                    self.exec(
                        &args[0],
                        &args[1..],
                        env,
                        request.working_directory(),
                        request.timeout(),
                        request.local_environment_inheritance(),
                        liveliness_observer,
                        request.disable_miniperf(),
                        cgroup,
                        freeze_rx,
                        sandbox,
                    )
                    .await
                };

                let time_span = execution_start.end_now();

                (time_span, start_time, r)
            },
        )
        .boxed()
        .await;

        match res {
            Ok(res) => Ok((time_span, start_time, res, manager)),
            Err(e) => Err(manager.error("exec_failed", e)),
        }
    }

    async fn exec_with_resource_control(
        &self,
        action_digest: &ActionDigest,
        request: &CommandExecutionRequest,
        mut manager: CommandExecutionManagerWithClaim,
        cancellations: &CancellationContext,
        liveliness_observer: impl LivelinessObserver + 'static,
        scratch_path: &ScratchPath,
        args: &[String],
        worker: Option<&WorkerHandle>,
        env: &[(&str, StrOrOsStr<'_>)],
        sandbox: Option<SandboxSpec>,
    ) -> Result<
        (
            TimeSpan,
            SystemTime,
            CommandResult,
            CommandExecutionManagerWithClaim,
        ),
        CommandExecutionResult,
    > {
        let (cgroup_session, mut start_future) = if worker.is_some() {
            (None, None)
        } else {
            let command_type = if request.is_test() {
                CommandType::Test
            } else {
                CommandType::Build
            };
            let disable_kill_and_retry_suspend = !request.outputs_cleanup;
            match ActionCgroupSession::maybe_create(
                &self.memory_tracker,
                command_type,
                Some(action_digest.to_string()),
                disable_kill_and_retry_suspend,
            )
            .await
            {
                Ok(Some((session, start_future))) => (Some(session), Some(start_future)),
                Ok(None) => (None, None),
                Err(e) => return Err(manager.error("initializing_resource_control", e)),
            }
        };

        let liveliness_observer: Arc<dyn LivelinessObserver> = Arc::new(liveliness_observer);

        let mut res = loop {
            let (kill_future, freeze_rx) = if let Some(start_future) = start_future {
                start_future.0.await.ok().unzip()
            } else {
                (None, None)
            };
            let freeze_rx = match freeze_rx {
                Some(x) => Either::Left(UnboundedReceiverStream::new(x)),
                None => Either::Right(futures::stream::pending::<ActionFreezeEvent>()),
            };

            let retry_future = Arc::new(std::sync::Mutex::new(None));

            let kill_observer = if let Some(kill_future) = kill_future {
                let kill_awaiter = kuro_util::async_move_clone!(retry_future, {
                    if let Ok(r) = kill_future.0.await {
                        *retry_future.lock().unwrap() = Some(r);
                    } else {
                        // If the other end hung up for some reason, we definitely do not want to
                        // treat that as indicating a kill, so never return from this future
                        std::future::pending().await
                    }
                });

                struct FutureLivelinessObserver<F: Future<Output = ()> + Send + Sync>(Shared<F>);

                #[async_trait::async_trait]
                impl<F: Future<Output = ()> + Send + Sync> LivelinessObserver for FutureLivelinessObserver<F> {
                    async fn while_alive(&self) {
                        self.0.clone().await
                    }
                }

                Arc::new(FutureLivelinessObserver(kill_awaiter.shared()))
                    as Arc<dyn LivelinessObserver>
            } else {
                Arc::new(NoopLivelinessObserver) as Arc<dyn LivelinessObserver>
            };

            let liveliness_observer = liveliness_observer.dupe().and(kill_observer);
            let res = self
                .exec_once(
                    action_digest,
                    request,
                    manager,
                    cancellations,
                    liveliness_observer,
                    scratch_path,
                    args,
                    worker,
                    env,
                    cgroup_session.as_ref().map(|s| s.path.clone()),
                    freeze_rx,
                    sandbox.clone(),
                )
                .await;

            let res = match res {
                Ok((time_span, start_time, status, res_manager)) => {
                    if matches!(status.status, GatherOutputStatus::Cancelled) {
                        let f = retry_future.lock().unwrap().take();
                        if let Some(retry_future) = f {
                            start_future = Some(retry_future);
                            manager = res_manager;
                            continue;
                        }
                    }
                    Ok((time_span, start_time, status, res_manager))
                }
                Err(e) => Err(e),
            };

            break res;
        };

        if let Some(mut cgroup_session) = cgroup_session {
            let cgroup_res = cgroup_session.action_finished().await;
            if let Ok(res) = &mut res {
                res.2.cgroup_result = Some(cgroup_res);
            }
        }

        res
    }

    async fn exec_request(
        &self,
        action_digest: &ActionDigest,
        request: &CommandExecutionRequest,
        mut manager: CommandExecutionManager,
        cancellation: CancellationObserver,
        cancellations: &CancellationContext,
        digest_config: DigestConfig,
        local_resource_holders: &[LocalResourceHolder],
    ) -> CommandExecutionResult {
        let args = &request.all_args_vec();
        if args.is_empty() {
            return manager.error("no_args", LocalExecutionError::NoArgs);
        }
        manager.start_waiting_category(WaitingCategory::MaterializingInputs);
        let executor_stage_result = executor_stage_async(
            kuro_data::LocalStage {
                stage: Some(kuro_data::LocalMaterializeInputs {}.into()),
            },
            async {
                let start = Instant::now();

                let (r1, r2) = future::join(
                    async {
                        materialize_inputs(
                            &self.artifact_fs,
                            self.materializer.as_ref(),
                            request,
                            digest_config,
                        )
                        .await
                    },
                    async {
                        if !request.outputs_cleanup {
                            // When user requests to not perform a cleanup for a specific action
                            // output from previous run of that action could actually be used as the
                            // input during current run (e.g. extra output which is an incremental state describing the actual output).
                            materialize_build_outputs(
                                &self.artifact_fs,
                                &self.incremental_db_state,
                                self.materializer.as_ref(),
                                request,
                            )
                            .await?;

                            // TODO(minglunli): There might be a dedup opportunity here to save some copying/materialization
                            // if the paths already exist on disk, should explore that
                            self.prepare_content_based_incremental_actions(request, cancellations)
                                .await?;

                            kuro_error::Ok(())
                        } else {
                            Ok(())
                        }
                    },
                )
                .await;

                let scratch_path = r1?.scratch;
                r2?;

                kuro_error::Ok((scratch_path, Instant::now() - start))
            },
        )
        .boxed()
        .await;

        let (scratch_path, input_materialization_duration) = match executor_stage_result {
            Ok((scratch_path, input_materialization_duration)) => {
                (scratch_path, input_materialization_duration)
            }
            Err(e) => return manager.error("materialize_inputs_failed", e),
        };

        manager.start_waiting_category(WaitingCategory::Unknown);

        // TODO: Release here.
        let manager = manager.claim().boxed().await;

        info!(
            "Local execution command line:\n```\n$ {}\n```",
            args.join(" "),
        );

        let dispatcher = match get_dispatcher_opt() {
            Some(dispatcher) => dispatcher,
            None => {
                return manager.error(
                    "no_dispatcher",
                    kuro_error!(
                        kuro_error::ErrorTag::DispatcherUnavailable,
                        "No dispatcher available"
                    ),
                );
            }
        };
        let build_id: &str = &dispatcher.trace_id().to_string();

        let mut env = vec![];

        let scratch_path_abs;
        let test_tmpdir_fallback;

        if let Some(scratch_path) = &scratch_path.0 {
            // For the $TMPDIR - important it is absolute
            scratch_path_abs = self.artifact_fs.fs().resolve(scratch_path);

            if cfg!(windows) {
                const MAX_PATH: usize = 260;
                if scratch_path_abs.as_os_str().len() > MAX_PATH {
                    return manager.error(
                        "scratch_dir_too_long",
                        kuro_error!(
                            kuro_error::ErrorTag::Environment,
                            "Scratch directory path is longer than MAX_PATH: {}",
                            scratch_path_abs
                        ),
                    );
                }
                env.push(("TEMP", StrOrOsStr::OsStr(scratch_path_abs.as_os_str())));
                env.push(("TMP", StrOrOsStr::OsStr(scratch_path_abs.as_os_str())));
            } else {
                env.push(("TMPDIR", StrOrOsStr::OsStr(scratch_path_abs.as_os_str())));
            }

            // Bazel-compatible: TEST_TMPDIR is set for all test actions
            if request.is_test() {
                env.push((
                    "TEST_TMPDIR",
                    StrOrOsStr::OsStr(scratch_path_abs.as_os_str()),
                ));
            }
        } else if request.is_test() {
            // For test commands without a scratch path, use system temp dir for TEST_TMPDIR
            test_tmpdir_fallback = std::env::temp_dir();
            env.push((
                "TEST_TMPDIR",
                StrOrOsStr::OsStr(test_tmpdir_fallback.as_os_str()),
            ));
        }
        env.extend(
            request
                .env()
                .iter()
                .map(|(k, v)| (k.as_str(), StrOrOsStr::from(v.as_str()))),
        );

        env.extend(local_resource_holders.iter().flat_map(|h| {
            h.as_ref().0.iter().map(|env_var| {
                (
                    env_var.key.as_str(),
                    StrOrOsStr::from(env_var.value.as_str()),
                )
            })
        }));
        let daemon_id = self.daemon_id.to_string();
        env.push(("BUCK2_DAEMON_UUID", StrOrOsStr::from(&*daemon_id)));
        env.push(("BUCK_BUILD_ID", StrOrOsStr::from(build_id)));

        // Bazel-compatible test environment variables
        if request.is_test() {
            env.push(("BAZEL_TEST", StrOrOsStr::from("1")));
            env.push(("TZ", StrOrOsStr::from("UTC")));
        }

        let liveliness_observer = manager.inner.liveliness_observer.dupe().and(cancellation);

        let (worker, manager) = self
            .initialize_worker(request, manager, dispatcher.dupe())
            .boxed()
            .await?;

        let execution_kind = match worker {
            None => CommandExecutionKind::Local {
                digest: action_digest.dupe(),
                command: args.to_vec(),
                env: request.env().clone(),
            },
            Some(_) => CommandExecutionKind::LocalWorker {
                digest: action_digest.dupe(),
                command: request.args().to_vec(),
                env: request.env().clone(),
                fallback_exe: request.exe().to_vec(),
            },
        };

        // Build sandbox spec if sandboxing is enabled.
        // Collect the output directories that need to be writable in the sandbox.
        let sandbox = if self.knobs.sandbox_enabled {
            let output_dirs: Vec<_> = request
                .outputs()
                .filter_map(|output| {
                    output
                        .resolve(
                            &self.artifact_fs,
                            Some(&ContentBasedPathHash::for_output_artifact()),
                        )
                        .ok()
                        .and_then(|resolved| {
                            resolved
                                .path_to_create()
                                .map(|p| self.artifact_fs.fs().resolve(p).as_path().to_owned())
                        })
                })
                .collect();

            // Collect declared inputs from buck-out for input isolation.
            // This restricts actions to only see declared build artifacts in buck-out,
            // catching undeclared dependency reads (e.g., accidentally reading a sibling
            // action's output that wasn't declared as a dep).
            //
            // Source files (in the project root) remain accessible via the real filesystem.
            // Only files under buck-out are subject to input isolation.
            let buck_out_root_abs = self
                .artifact_fs
                .fs()
                .resolve(self.artifact_fs.buck_out_path_resolver().root())
                .as_path()
                .to_owned();

            let mut input_files = Vec::new();
            for input in request.inputs() {
                if let CommandExecutionInput::Artifact(group) = input {
                    for (artifact, _value) in group.iter() {
                        if let Ok(rel_path) =
                            artifact.resolve_configuration_hash_path(&self.artifact_fs)
                        {
                            let abs = self
                                .artifact_fs
                                .fs()
                                .resolve(&rel_path)
                                .as_path()
                                .to_owned();
                            if abs.starts_with(&buck_out_root_abs) {
                                input_files.push(abs);
                            }
                        }
                    }
                }
            }

            Some(SandboxSpec {
                output_dirs,
                input_files,
                buck_out_root: Some(buck_out_root_abs),
            })
        } else {
            None
        };

        // Optionally write args to a param file and replace command line.
        // This supports Bazel's args.use_param_file() for very long argument lists.
        let param_args_replaced: Option<Vec<String>> = if let Some(pf) = request.param_file() {
            let exe_len = request.exe().len();
            let param_args = &args[exe_len..];
            let needs_param_file = pf.use_always || {
                let total: usize = param_args.iter().map(|s| s.len() + 1).sum();
                total > 32768
            };
            if needs_param_file {
                let param_dir = scratch_path
                    .0
                    .as_ref()
                    .map(|sp| self.artifact_fs.fs().resolve(sp).as_path().to_owned())
                    .unwrap_or_else(std::env::temp_dir);
                let param_path = param_dir.join("kuro-params");

                let content = match pf.format {
                    ParamFileFormat::Shell => param_args
                        .iter()
                        .map(|s| {
                            // Simple shell quoting: wrap in single quotes, escape internal '
                            let mut q = String::from("'");
                            q.push_str(&s.replace('\'', "'\\''"));
                            q.push('\'');
                            q
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    ParamFileFormat::Multiline | ParamFileFormat::FlagPerLine => {
                        param_args.join("\n")
                    }
                };

                match std::fs::write(&param_path, content) {
                    Ok(()) => {
                        let param_file_arg = pf
                            .param_file_arg
                            .replace("%s", &param_path.to_string_lossy());
                        let mut new_args: Vec<String> = args[..exe_len].to_vec();
                        new_args.push(param_file_arg);
                        Some(new_args)
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to write param file {}: {}; using full args",
                            param_path.display(),
                            e
                        );
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        // Bazel compatibility: handle inline param file args for use_param_file.
        // When a Bazel Args object has use_param_file() but kuro expands them inline
        // (because the param file config is on a nested Args, not the top-level),
        // the args appear as positional params. Detect this pattern and write them
        // to a temp file with the correct --cargo_manifest_args=@<file> argument.
        let param_args_replaced = param_args_replaced.or_else(|| {
            // Look for inline cargo_runfiles args: after all --key=value named args,
            // there may be positional args: runfiles_dir, retain_list, mapping1=dest1, ...
            // These should go into a param file for --cargo_manifest_args=@<file>.
            let exe_len = request.exe().len();
            let remaining = &args[exe_len..];
            // Find the cargo_runfiles directory arg specifically.
            // It's a positional arg (no --) that ends with .cargo_runfiles.
            // This ONLY matches the cargo_build_script runner pattern.
            let cargo_runfiles_pos = remaining
                .iter()
                .position(|a| !a.starts_with("--") && a.ends_with(".cargo_runfiles"));
            if let Some(pos_idx) = cargo_runfiles_pos {
                let positional = &remaining[pos_idx..];
                // Heuristic: positional args for cargo_runfiles have at least 3 entries
                // (dir, retain_list, mapping1=dest1) and the 3rd+ contain "="
                if positional.len() >= 3 && positional.iter().skip(2).any(|a| a.contains('=')) {
                    // Use a temp file outside the build output tree. The scratch_path
                    // is inside the action's output dir which gets cleaned by
                    // create_output_dirs before the command runs.
                    let param_dir = std::env::temp_dir().join("kuro-param-files");
                    let _ = std::fs::create_dir_all(&param_dir);
                    // Use a unique temp file per invocation
                    let param_path = {
                        use std::sync::atomic::AtomicU64;
                        use std::sync::atomic::Ordering;
                        static COUNTER: AtomicU64 = AtomicU64::new(0);
                        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
                        param_dir.join(format!("cargo-manifest-{}", id))
                    };
                    let content = positional.join("\n");
                    if let Ok(()) = std::fs::write(&param_path, &content) {
                        let param_file_arg =
                            format!("--cargo_manifest_args=@{}", param_path.to_string_lossy());
                        let mut new_args: Vec<String> = args[..exe_len].to_vec();
                        new_args.extend_from_slice(&remaining[..pos_idx]);
                        new_args.push(param_file_arg);
                        return Some(new_args);
                    }
                }
            }
            None
        });
        let effective_args: &[String] = param_args_replaced.as_deref().unwrap_or(args);

        let (time_span, start_time, res, manager) = match self
            .exec_with_resource_control(
                action_digest,
                request,
                manager,
                cancellations,
                liveliness_observer,
                &scratch_path,
                effective_args,
                worker.as_deref(),
                &env,
                sandbox,
            )
            .await
        {
            Ok(x) => x,
            Err(e) => return e,
        };

        let CommandResult {
            status,
            stdout,
            stderr,
            cgroup_result,
        } = res;

        let std_streams = CommandStdStreams::Local { stdout, stderr };

        let mut timing = Box::new(CommandExecutionMetadata {
            time_span,
            execution_time: time_span.duration(),
            start_time,
            execution_stats: None, // We fill this in later if available.
            input_materialization_duration,
            hashing_duration: Duration::ZERO, // We fill hashing info in later if available.
            hashed_artifacts_count: 0,
            queue_duration: None,
            suspend_duration: None,
            suspend_count: None,
        });

        let result = match status {
            GatherOutputStatus::Finished {
                exit_code,
                execution_stats,
            } => {
                let (outputs, hashing_time) = match self
                    .calculate_and_declare_output_values(request, digest_config)
                    .boxed()
                    .await
                {
                    Ok((output_values, hashing_time)) => (output_values, hashing_time),
                    Err(e) => {
                        return manager.error("calculate_output_values_failed", e);
                    }
                };

                let mut execution_stats =
                    execution_stats.map(|s| kuro_data::CommandExecutionStats {
                        cpu_instructions_user: s.cpu_instructions_user,
                        cpu_instructions_kernel: s.cpu_instructions_kernel,
                        userspace_events: s.userspace_events,
                        kernel_events: s.kernel_events,
                        memory_peak: None,
                    });

                if let Some(memory_peak) =
                    cgroup_result.as_ref().and_then(|r| r.memory_peak.as_ref())
                {
                    execution_stats.get_or_insert_default().memory_peak = Some(*memory_peak);
                }

                timing.execution_stats = execution_stats;
                if let Some(cgroup_result) = cgroup_result {
                    if let Some(e) = cgroup_result.error {
                        let _unused = soft_error!("action_cgroup_error", e);
                    }
                    timing.suspend_duration = cgroup_result.suspend_duration;
                    timing.suspend_count = Some(cgroup_result.suspend_count);
                }

                timing.hashing_duration = hashing_time.hashing_duration;
                timing.hashed_artifacts_count = hashing_time.hashed_artifacts_count;

                if exit_code == 0 {
                    manager.success(execution_kind, outputs, std_streams, *timing)
                } else {
                    let manager = check_inputs(
                        manager,
                        &self.artifact_fs,
                        self.blocking_executor.as_ref(),
                        request,
                    )
                    .boxed()
                    .await?;

                    manager.failure(
                        execution_kind,
                        outputs,
                        std_streams,
                        Some(exit_code),
                        *timing,
                        None,
                    )
                }
            }
            GatherOutputStatus::SpawnFailed(reason) => {
                let manager = check_inputs(
                    manager,
                    &self.artifact_fs,
                    self.blocking_executor.as_ref(),
                    request,
                )
                .boxed()
                .await?;

                // We are lying about the std streams here because we don't have a good mechanism
                // to report that the command does not exist, and because that's exactly what RE
                // also does when this happens.
                if matches!(execution_kind, CommandExecutionKind::Local { .. }) {
                    manager.failure(
                        execution_kind,
                        Default::default(),
                        CommandStdStreams::Local {
                            stdout: Default::default(),
                            stderr: format!("Spawning executable `{}` failed: {}", args[0], reason)
                                .into_bytes(),
                        },
                        None,
                        *timing,
                        None,
                    )
                } else {
                    // Workers executing tests often employ a health check to avoid producing
                    // invalid test results. Differentiating a worker spawn failure from a normal
                    // spawn or execution failure allows the test runner to handle this case
                    // accordingly.
                    manager.worker_failure(
                        execution_kind,
                        // Could probably use a better error message.
                        format!("Spawning executable `{}` failed: {}", args[0], reason),
                        *timing,
                    )
                }
            }
            GatherOutputStatus::TimedOut(duration) => {
                let (outputs, hashing_time) = match self
                    .calculate_and_declare_output_values(request, digest_config)
                    .boxed()
                    .await
                {
                    Ok((output_values, hashing_time)) => (output_values, hashing_time),
                    Err(e) => {
                        return manager.error("calculate_output_values_failed", e);
                    }
                };

                timing.hashing_duration = hashing_time.hashing_duration;
                timing.hashed_artifacts_count = hashing_time.hashed_artifacts_count;

                manager.timeout(
                    execution_kind,
                    outputs,
                    duration,
                    std_streams,
                    *timing,
                    None,
                )
            }
            GatherOutputStatus::Cancelled => manager.cancel_claim(execution_kind, *timing),
        };

        if let Some(run_action_key) = request.run_action_key()
            && !request.outputs_cleanup
        {
            save_content_based_incremental_state(
                run_action_key.clone(),
                &self.incremental_db_state,
                &self.artifact_fs,
                &result,
            );
        }

        result
    }

    async fn calculate_and_declare_output_values(
        &self,
        request: &CommandExecutionRequest,
        digest_config: DigestConfig,
    ) -> kuro_error::Result<(IndexMap<CommandExecutionOutput, ArtifactValue>, HashingInfo)> {
        let mut builder = inputs_directory(request.inputs(), digest_config, &self.artifact_fs)?;

        // Read outputs from disk and add them to the builder
        let mut entries = Vec::new();
        let mut total_hashing_time = Duration::ZERO;
        let mut total_hashed_outputs = 0;
        for output in request.outputs() {
            let path = output
                .resolve(
                    &self.artifact_fs,
                    Some(&ContentBasedPathHash::for_output_artifact()),
                )?
                .into_path();
            let abspath = self.root.join(&path);
            let (entry, hashing_info) = build_entry_from_disk(
                abspath,
                FileDigestConfig::build(digest_config.cas_digest_config()),
                self.blocking_executor.as_ref(),
                self.artifact_fs.fs().root(),
            )
            .await
            .with_buck_error_context(|| format!("collecting output {path:?}"))?;
            total_hashing_time += hashing_info.hashing_duration;
            total_hashed_outputs += hashing_info.hashed_artifacts_count;
            if let Some(entry) = entry {
                insert_entry(&mut builder, path.clone(), entry)?;
                entries.push((output.cloned(), path));
            }
        }

        let mut to_declare = vec![];
        let mut mapped_outputs = IndexMap::with_capacity(entries.len());
        let mut configuration_path_to_content_based_path_symlinks = vec![];
        let mut output_path_to_content_based_path_copies = vec![];

        for (output, output_path) in entries {
            let value = extract_artifact_value(&builder, &output_path, digest_config)?;
            if let Some(value) = value {
                match output {
                    CommandExecutionOutput::BuildArtifact {
                        supports_incremental_remote,
                        ..
                    } => {
                        // For content-based paths, things are a bit complicated here, because (a) the action
                        // wrote outputs at "placeholder" paths, not the final content-based paths (because
                        // they are not know until the output is produced), and (b) other actions can declare
                        // outputs at the same content-based path. Note that only remote actions can do that
                        // concurrently (with this local action), as we prevent any local actions with any of
                        // the same placeholder output paths from running at the same time.
                        // We do the following:
                        // (1) We create a symlink from the configuration-based path to the content-based path
                        //     (for any users/tooling that only has access to the configuration-based path)
                        // (2) Declare an existing artifact at the "placeholder" output path that the action wrote to.
                        // (3) Then we declare a copy from the "placeholder" output path to the content-based path.
                        // (4) Finally, we ensure everything is materialized.
                        // (5) Note that we don't need to invalidate the "placeholder" output path, as that is
                        //     the responsibility of any action that subsequently uses it.
                        if output.as_ref().has_content_based_path() {
                            let hashed_path = output
                                .as_ref()
                                .resolve(&self.artifact_fs, Some(&value.content_based_path_hash()))?
                                .into_path();

                            let configuration_hash_path = output
                                .as_ref()
                                .resolve_configuration_hash_path(&self.artifact_fs)?
                                .into_path();
                            let mut builder =
                                ArtifactValueBuilder::new(self.artifact_fs.fs(), digest_config);
                            builder.add_symlinked(
                                &value,
                                hashed_path.clone(),
                                &configuration_hash_path,
                            )?;
                            let symlink_value = builder.build(&configuration_hash_path)?;
                            configuration_path_to_content_based_path_symlinks
                                .push((configuration_hash_path, symlink_value));

                            to_declare.push(DeclareArtifactPayload {
                                path: output_path.clone(),
                                artifact: value.dupe(),
                                persist_full_directory_structure: supports_incremental_remote,
                            });
                            output_path_to_content_based_path_copies.push((
                                hashed_path.clone(),
                                value.dupe(),
                                vec![CopiedArtifact {
                                    src: output_path.clone(),
                                    dest: hashed_path,
                                    dest_entry: value.entry().dupe().map_dir(|d| d.as_immutable()),
                                    executable_bit_override: None,
                                }],
                            ));
                        } else {
                            to_declare.push(DeclareArtifactPayload {
                                path: output_path,
                                artifact: value.dupe(),
                                persist_full_directory_structure: supports_incremental_remote,
                            });
                        }
                    }
                    CommandExecutionOutput::TestPath { .. } => {
                        // Don't declare those as we don't currently have any form of GC so this
                        // would take up space for nothing, and most importantly, we will never
                        // need them to be in materializer state for e.g. matching as nothing
                        // should depend on them.
                    }
                }

                mapped_outputs.insert(output, value);
            }
        }

        let configuration_paths = configuration_path_to_content_based_path_symlinks
            .iter()
            .map(|(p, _)| p.clone())
            .collect();
        // Collect stats before to_declare is consumed so we can emit materialization events.
        let matl_stats: Vec<(ProjectRelativePathBuf, u64, u64)> = to_declare
            .iter()
            .map(|p| {
                let counts = p.artifact.calc_output_count_and_bytes();
                (p.path.clone(), counts.count, counts.bytes)
            })
            .collect();
        self.materializer.declare_existing(to_declare).await?;
        // Emit MaterializationStart/End events for locally-produced artifacts so that
        // `kuro log what-materialized` reports them.
        if let Some(dispatcher) = get_dispatcher_opt() {
            for (path, file_count, total_bytes) in matl_stats {
                let path_str = path.as_str().to_owned();
                dispatcher.span(
                    kuro_data::MaterializationStart {
                        action_digest: None,
                    },
                    || {
                        (
                            (),
                            kuro_data::MaterializationEnd {
                                action_digest: None,
                                file_count,
                                total_bytes,
                                path: path_str,
                                success: true,
                                error: None,
                                method: Some(kuro_data::MaterializationMethod::Write as i32),
                            },
                        )
                    },
                );
            }
        }
        kuro_util::future::try_join_all(output_path_to_content_based_path_copies.into_iter().map(
            |(path, value, copied_artifacts)| {
                self.materializer
                    .declare_copy(path, value, copied_artifacts)
            },
        ))
        .await?;
        kuro_util::future::try_join_all(
            configuration_path_to_content_based_path_symlinks
                .into_iter()
                .map(|(path, value)| self.materializer.declare_copy(path, value, vec![])),
        )
        .await?;

        self.materializer
            .ensure_materialized(configuration_paths)
            .await?;

        Ok((
            mapped_outputs,
            HashingInfo {
                hashing_duration: total_hashing_time,
                hashed_artifacts_count: total_hashed_outputs,
            },
        ))
    }

    async fn acquire_worker_permit(
        &self,
        request: &CommandExecutionRequest,
    ) -> Option<HostSharingGuard> {
        if let (Some(worker_spec), Some(worker_pool)) = (request.worker(), self.worker_pool.dupe())
        {
            if let Some(broker) = &worker_pool.get_worker_broker(worker_spec) {
                Some(
                    executor_stage_async(
                        kuro_data::LocalStage {
                            stage: Some(kuro_data::WorkerQueued {}.into()),
                        },
                        broker.acquire(&HostSharingRequirements::default()),
                    )
                    .await,
                )
            } else {
                None
            }
        } else {
            None
        }
    }

    #[cfg(not(unix))]
    async fn initialize_worker(
        &self,
        _request: &CommandExecutionRequest,
        manager: CommandExecutionManagerWithClaim,
        _dispatcher: EventDispatcher,
    ) -> ControlFlow<
        CommandExecutionResult,
        (Option<Arc<WorkerHandle>>, CommandExecutionManagerWithClaim),
    > {
        ControlFlow::Continue((None, manager))
    }

    #[cfg(unix)]
    async fn initialize_worker(
        &self,
        request: &CommandExecutionRequest,
        manager: CommandExecutionManagerWithClaim,
        dispatcher: EventDispatcher,
    ) -> ControlFlow<
        CommandExecutionResult,
        (Option<Arc<WorkerHandle>>, CommandExecutionManagerWithClaim),
    > {
        if let (Some(worker_spec), Some(worker_pool), ForkserverAccess::Client(_)) =
            (request.worker(), self.worker_pool.dupe(), &self.forkserver)
        {
            let env = worker_spec
                .env
                .iter()
                .map(|(k, v)| (OsString::from(k), OsString::from(v)));
            let (new_worker, worker_fut) = worker_pool.get_or_create_worker(
                worker_spec,
                env,
                &self.root,
                self.forkserver.dupe(),
                dispatcher,
            );

            if let Some(Ok(worker)) = worker_fut.peek() {
                return ControlFlow::Continue((Some(worker.clone()), manager));
            }

            // Might make more sense for the stage to always be `WorkerWait` and for `WorkerInit` to be a separate, top level event
            let stage = if new_worker {
                kuro_data::LocalStage {
                    stage: Some(
                        kuro_data::WorkerInit {
                            command: Some(kuro_data::WorkerInitCommand {
                                argv: worker_spec.exe.clone(),
                                env: worker_spec
                                    .env
                                    .iter()
                                    .map(|(k, v)| kuro_data::EnvironmentEntry {
                                        key: k.to_owned(),
                                        value: v.to_owned(),
                                    })
                                    .collect(),
                            }),
                        }
                        .into(),
                    ),
                }
            } else {
                kuro_data::LocalStage {
                    stage: Some(kuro_data::WorkerWait {}.into()),
                }
            };

            match executor_stage_async(stage, worker_fut).await {
                Ok(worker) => ControlFlow::Continue((Some(worker), manager)),
                Err(e) => {
                    let res = {
                        let manager = check_inputs(
                            manager,
                            &self.artifact_fs,
                            self.blocking_executor.as_ref(),
                            request,
                        )
                        .await?;

                        e.to_command_execution_result(request, manager)
                    };
                    ControlFlow::Break(res)
                }
            }
        } else {
            ControlFlow::Continue((None, manager))
        }
    }

    async fn prepare_content_based_incremental_actions(
        &self,
        request: &CommandExecutionRequest,
        cancellations: &CancellationContext,
    ) -> kuro_error::Result<()> {
        let declared_content_based_outputs: Vec<BuildArtifactPath> = request
            .outputs()
            .filter_map(|output| match output {
                CommandExecutionOutputRef::BuildArtifact { path, .. }
                    if path.is_content_based_path() =>
                {
                    Some(path.clone())
                }
                _ => None,
            })
            .collect();

        let outputs_to_delete = declared_content_based_outputs
            .iter()
            .map(|path| {
                self.artifact_fs
                    .resolve_build(&path, Some(&ContentBasedPathHash::OutputArtifact))
            })
            .collect::<kuro_error::Result<Vec<_>>>()?;

        self.materializer
            .invalidate_many(outputs_to_delete.clone())
            .await?;

        // Need to clean the placeholder paths before execution as there could be stale outputs that can cause unexpected behavior
        self.blocking_executor
            .execute_io(
                Box::new(CleanOutputPaths {
                    paths: outputs_to_delete,
                }),
                cancellations,
            )
            .await
            .buck_error_context("Failed to cleanup output directory")?;

        if let Some(state) =
            get_incremental_path_map(&self.incremental_db_state, &request.run_action_key())
        {
            let mut copy_futs = Vec::new();

            for output in declared_content_based_outputs {
                let p = output.path().to_buf();

                if let Some(content_path) = state.get(&p) {
                    copy_futs.push(async move {
                        self.blocking_executor
                            .execute_io_inline(|| {
                                self.artifact_fs.fs().copy(
                                    content_path.clone(),
                                    self.artifact_fs.resolve_build(
                                        &output,
                                        Some(&ContentBasedPathHash::OutputArtifact),
                                    )?,
                                )
                            })
                            .await
                    })
                }
            }

            // The materialization we do for incremental action outputs is best-effort. The copy
            // will fail if the materialization failed, and that's okay.
            join_all(copy_futs).await;
        }

        Ok(())
    }
}

#[async_trait]
impl PreparedCommandExecutor for LocalExecutor {
    async fn exec_cmd(
        &self,
        command: &PreparedCommand<'_, '_>,
        manager: CommandExecutionManager,
        cancellations: &CancellationContext,
    ) -> CommandExecutionResult {
        let mut manager = manager.with_execution_kind(CommandExecutionKind::Local {
            digest: command.prepared_action.digest(),
            command: command.request.all_args_vec(),
            env: command.request.env().clone(),
        });
        if command.request.executor_preference().requires_remote() {
            return manager.error("local_prepare", LocalExecutionError::RemoteOnlyAction);
        }

        let PreparedCommand {
            request,
            target: _,
            prepared_action,
            digest_config,
        } = command;

        manager.start_waiting_category(WaitingCategory::LocalQueued);
        let local_resource_holders = executor_stage_async(
            kuro_data::LocalStage {
                stage: Some(kuro_data::AcquireLocalResource {}.into()),
            },
            async move {
                let mut holders = vec![];
                // Acquire resources in a sorted way to avoid deadlock.
                // It might happen if 2 tests both requiring resources A and B are run simultaneously and there is only 1 instance of resource per type.
                // If tests are not acquiring them in a sorted way the following situation might happen:
                // Test 1 acquires resource B and test 2 acquires resource A.
                // Now test 1 is waiting on resource B and test 2 is waiting on resource A.
                for r in request.required_local_resources() {
                    holders.push(r.acquire_resource().await);
                }
                holders
            },
        )
        .await;

        let _worker_permit = self.acquire_worker_permit(request).await;

        let _permit = executor_stage_async(
            kuro_data::LocalStage {
                stage: Some(kuro_data::LocalQueued {}.into()),
            },
            self.host_sharing_broker
                .acquire(request.host_sharing_requirements()),
        )
        .await;
        manager.start_waiting_category(WaitingCategory::Unknown);

        // If we start running something, we don't want this task to get dropped, because if we do
        // we might interfere with e.g. clean up.
        cancellations
            .with_structured_cancellation(|cancellation| {
                Self::exec_request(
                    self,
                    &prepared_action.action_and_blobs.action,
                    request,
                    manager,
                    cancellation,
                    cancellations,
                    *digest_config,
                    &local_resource_holders,
                )
            })
            .await
    }

    fn is_local_execution_possible(&self, _executor_preference: ExecutorPreference) -> bool {
        true
    }
}

/// Either a str or a OsStr, so that we can turn it back into a String without having to check for
/// valid utf-8, while using the same struct.
#[derive(Copy, Clone, Dupe, From)]
enum StrOrOsStr<'a> {
    Str(&'a str),
    OsStr(&'a OsStr),
}

impl<'a> StrOrOsStr<'a> {
    fn into_string_lossy(self) -> String {
        match self {
            Self::Str(s) => s.to_owned(),
            Self::OsStr(s) => s.to_string_lossy().into_owned(),
        }
    }

    fn into_os_str(self) -> &'a OsStr {
        match self {
            Self::Str(s) => OsStr::new(s),
            Self::OsStr(s) => s,
        }
    }
}

pub struct MaterializedInputPaths {
    pub scratch: ScratchPath,
    pub paths: Vec<ProjectRelativePathBuf>,
}

/// Materialize all inputs artifact for CommandExecutionRequest so the command can be executed
/// locally.
///
/// This also discovers the scratch directory if any was passed, but does not yet do anything with
/// it - call `prep_scratch_path`.
pub async fn materialize_inputs(
    artifact_fs: &ArtifactFs,
    materializer: &dyn Materializer,
    request: &CommandExecutionRequest,
    digest_config: DigestConfig,
) -> kuro_error::Result<MaterializedInputPaths> {
    let mut paths = vec![];
    let mut scratch = ScratchPath(None);
    let mut configuration_path_to_content_based_path_symlinks = vec![];

    for input in request.inputs().iter().chain(
        request
            .worker()
            .as_ref()
            .map(|w| w.inputs())
            .unwrap_or_default(),
    ) {
        match input {
            CommandExecutionInput::Artifact(group) => {
                for (artifact, artifact_value) in group.iter() {
                    if artifact.requires_materialization(artifact_fs) {
                        let configuration_hash_path =
                            artifact.resolve_configuration_hash_path(artifact_fs)?;

                        if artifact.has_content_based_path() {
                            let content_based_path = artifact.resolve_path(
                                artifact_fs,
                                Some(&artifact_value.content_based_path_hash()),
                            )?;

                            // TODO(ianc) We want to also create symlinks here for projected artifacts.
                            if artifact.is_projected() {
                                paths.push(content_based_path);
                            } else {
                                let mut builder =
                                    ArtifactValueBuilder::new(artifact_fs.fs(), digest_config);
                                builder.add_symlinked(
                                    artifact_value,
                                    content_based_path,
                                    &configuration_hash_path,
                                )?;
                                let symlink_value = builder.build(&configuration_hash_path)?;
                                configuration_path_to_content_based_path_symlinks
                                    .push((configuration_hash_path.clone(), symlink_value));
                                paths.push(configuration_hash_path);
                            }
                        } else {
                            paths.push(configuration_hash_path);
                        }
                    }
                }
            }
            CommandExecutionInput::ActionMetadata(metadata) => {
                let path = artifact_fs
                    .buck_out_path_resolver()
                    .resolve_gen(&metadata.path, Some(&metadata.content_hash))?;
                paths.push(path);
            }
            CommandExecutionInput::ScratchPath(path) => {
                let path = artifact_fs.buck_out_path_resolver().resolve_scratch(path)?;

                if scratch.0.is_some() {
                    return Err(kuro_error::internal_error!(
                        "Multiple scratch paths for one action"
                    ));
                }
                scratch.0 = Some(path);
            }
            CommandExecutionInput::IncrementalRemoteOutput(..) => {
                // Ignore, should be already materialized
            }
        }
    }

    kuro_util::future::try_join_all(
        configuration_path_to_content_based_path_symlinks
            .into_iter()
            .map(|(path, value)| materializer.declare_copy(path, value, vec![])),
    )
    .await?;

    let mut stream = materializer.materialize_many(paths.clone()).await?;
    while let Some(res) = stream.next().await {
        match res {
            Ok(()) => {}
            Err(MaterializationError::NotFound { source }) => {
                let corrupted = source.info.origin.guaranteed_by_action_cache();

                return Err(tag_error!(
                    "cas_missing_fatal",
                    MaterializationError::NotFound { source }.into(),
                    quiet: true,
                    task: false,
                    daemon_in_memory_state_is_corrupted: true,
                    action_cache_is_corrupted: corrupted
                )
                .into());
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    Ok(MaterializedInputPaths { scratch, paths })
}

/// A scratch path discovered during `materialize_inputs`.
pub struct ScratchPath(Option<ProjectRelativePathBuf>);

pub async fn prep_scratch_path(
    scratch_path: &ScratchPath,
    artifact_fs: &ArtifactFs,
) -> kuro_error::Result<()> {
    let Some(path) = scratch_path.0.as_ref() else {
        return Ok(());
    };
    CleanOutputPaths::clean(std::iter::once(path.as_ref()), artifact_fs.fs())?;
    async_fs_util::create_dir_all(artifact_fs.fs().resolve(path)).await
}

async fn check_inputs(
    manager: CommandExecutionManagerWithClaim,
    artifact_fs: &ArtifactFs,
    blocking_executor: &dyn BlockingExecutor,
    request: &CommandExecutionRequest,
) -> ControlFlow<CommandExecutionResult, CommandExecutionManagerWithClaim> {
    let res = blocking_executor
        .execute_io_inline(|| {
            for input in request.inputs() {
                match input {
                    CommandExecutionInput::Artifact(group) => {
                        for (artifact, artifact_value) in group.iter() {
                            if artifact.requires_materialization(artifact_fs) {
                                let path = artifact.resolve_path(artifact_fs,
                                    if artifact.has_content_based_path() {
                                        Some(artifact_value.content_based_path_hash())
                                    } else {
                                        None
                                    }.as_ref())?;
                                let abs_path = artifact_fs.fs().resolve(&path);

                                // We ignore the result here because while we want to tag it, we'd
                                // prefer to just show the normal error to the user, so we don't
                                // want to propagate it.
                                let _ignored = tag_result!(
                                    "missing_local_inputs",
                                    fs_util::symlink_metadata(&abs_path).buck_error_context("Missing input").map_err(|e| e.into()),
                                    quiet: true,
                                    task: false,
                                    daemon_materializer_state_is_corrupted: true
                                );
                            }
                        }
                    }
                    CommandExecutionInput::ActionMetadata(..) => {
                        // Ignore those here.
                    }
                    CommandExecutionInput::ScratchPath(..) => {
                        // Nothing to look at
                    }
                    CommandExecutionInput::IncrementalRemoteOutput(..) => {
                        // Ignore
                    }
                }
            }

            Ok(())
        })
        .await;

    match res {
        Ok(()) => ControlFlow::Continue(manager),
        Err(err) => ControlFlow::Break(manager.error("local_check_inputs", err)),
    }
}

/// Materialize all output artifact for CommandExecutionRequest.
///
/// Note that the outputs could be from the previous run of the same command if cleanup on the action was not performed.
/// The above is useful when executing incremental actions first remotely and then locally.
/// In that case output from remote execution which is incremental state should be materialized prior to local execution.
/// Such incremental state in fact serves as the input while being output as well.
async fn materialize_build_outputs(
    artifact_fs: &ArtifactFs,
    incremental_db_state: &Arc<IncrementalDbState>,
    materializer: &dyn Materializer,
    request: &CommandExecutionRequest,
) -> kuro_error::Result<Vec<ProjectRelativePathBuf>> {
    let mut paths = vec![];
    let path_map = get_incremental_path_map(incremental_db_state, request.run_action_key());
    for output in request.outputs() {
        match output {
            CommandExecutionOutputRef::BuildArtifact { path, .. } => {
                if path.is_content_based_path() {
                    if let Some(ref state) = path_map {
                        let p = path.path().to_buf();
                        if let Some(content_path) = state.get(&p) {
                            paths.push(content_path.clone());
                        }
                    }
                } else {
                    paths.push(artifact_fs.resolve_build(path, None)?);
                }
            }
            CommandExecutionOutputRef::TestPath { .. } => {}
        }
    }

    materializer.ensure_materialized(paths.clone()).await?;

    Ok(paths)
}

/// Create any output dirs requested by the command. Note that this makes no effort to delete
/// the output paths first. Eventually it should, but right now this happens earlier. This
/// would be a separate refactor.
pub async fn create_output_dirs(
    artifact_fs: &ArtifactFs,
    request: &CommandExecutionRequest,
    materializer: Arc<dyn Materializer>,
    blocking_executor: Arc<dyn BlockingExecutor>,
    cancellations: &CancellationContext,
) -> kuro_error::Result<()> {
    let outputs: Vec<_> = request
        .outputs()
        .map(|output| {
            output.resolve(
                artifact_fs,
                Some(&ContentBasedPathHash::for_output_artifact()),
            )
        })
        .collect::<kuro_error::Result<Vec<_>>>()?;

    // Invalidate all the output paths this action might provide. Note that this is a bit
    // approximative: we might have previous instances of this action that declared
    // different outputs with a different materialization method that will become invalid
    // now. However, nothing should reference those stale outputs, so while this does not
    // do a good job of cleaning up garbage, it prevents using invalid artifacts.
    let output_paths = outputs.map(|output| output.path.to_owned());
    materializer.invalidate_many(output_paths.clone()).await?;

    if request.outputs_cleanup {
        // TODO(scottcao): Move this deletion logic into materializer itself.
        blocking_executor
            .execute_io(
                Box::new(CleanOutputPaths {
                    paths: output_paths,
                }),
                cancellations,
            )
            .await
            .buck_error_context("Failed to cleanup output directory")?;
    }

    let project_fs = artifact_fs.fs();
    for output in outputs {
        if let Some(path) = output.path_to_create() {
            fs_util::create_dir_all(project_fs.resolve(path))?;
        }
    }

    Ok(())
}

pub fn apply_local_execution_environment(
    builder: &mut impl EnvironmentBuilder,
    working_directory: &AbsPath,
    env: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
    env_inheritance: Option<&EnvironmentInheritance>,
) {
    if let Some(env_inheritance) = env_inheritance {
        if env_inheritance.clear() {
            builder.clear();
        }

        for key in env_inheritance.exclusions() {
            builder.remove(key);
        }

        for (key, val) in env_inheritance.values() {
            builder.set(key, val);
        }
    }
    for (key, val) in env {
        builder.set(key, val);
    }
    builder.set("PWD", working_directory.as_path());
}

pub trait EnvironmentBuilder {
    fn clear(&mut self);

    fn set<K, V>(&mut self, key: K, val: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>;

    fn remove<K>(&mut self, key: K)
    where
        K: AsRef<OsStr>;
}

impl EnvironmentBuilder for Command {
    fn clear(&mut self) {
        Command::env_clear(self);
    }

    fn set<K, V>(&mut self, key: K, val: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        Command::env(self, key, val);
    }

    fn remove<K>(&mut self, key: K)
    where
        K: AsRef<OsStr>,
    {
        Command::env_remove(self, key);
    }
}

#[cfg(unix)]
mod unix {
    use std::os::unix::ffi::OsStrExt;

    use super::*;

    pub async fn exec_via_forkserver(
        forkserver: &kuro_forkserver::client::ForkserverClient,
        exe: impl AsRef<OsStr>,
        args: impl IntoIterator<Item = impl AsRef<OsStr>>,
        env: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
        working_directory: &AbsPath,
        command_timeout: Option<Duration>,
        env_inheritance: Option<&EnvironmentInheritance>,
        liveliness_observer: impl LivelinessObserver + 'static,
        enable_miniperf: bool,
        cgroup_path: Option<CgroupPathBuf>,
        freeze_rx: impl ActionFreezeEventReceiver,
    ) -> kuro_error::Result<CommandResult> {
        let exe = exe.as_ref();

        let mut req = kuro_forkserver_proto::CommandRequest {
            exe: exe.as_bytes().to_vec(),
            argv: args
                .into_iter()
                .map(|s| s.as_ref().as_bytes().to_vec())
                .collect(),
            cwd: Some(kuro_forkserver_proto::WorkingDirectory {
                path: working_directory.as_path().as_os_str().as_bytes().to_vec(),
            }),
            env: vec![],
            timeout: command_timeout.try_map(|d| d.try_into())?,
            enable_miniperf,
            std_redirects: None,
            graceful_shutdown_timeout_s: None,
            command_cgroup: cgroup_path.map(|p| p.to_string()),
        };
        apply_local_execution_environment(&mut req, working_directory, env, env_inheritance);
        forkserver
            .execute(
                req,
                async move { liveliness_observer.while_alive().await },
                freeze_rx,
            )
            .await
    }

    trait CommandRequestExt {
        fn push_env_directive<D>(&mut self, directive: D)
        where
            D: Into<kuro_forkserver_proto::env_directive::Data>;
    }

    impl CommandRequestExt for kuro_forkserver_proto::CommandRequest {
        fn push_env_directive<D>(&mut self, directive: D)
        where
            D: Into<kuro_forkserver_proto::env_directive::Data>,
        {
            self.env.push(kuro_forkserver_proto::EnvDirective {
                data: Some(directive.into()),
            });
        }
    }

    impl EnvironmentBuilder for kuro_forkserver_proto::CommandRequest {
        fn clear(&mut self) {
            self.push_env_directive(kuro_forkserver_proto::EnvClear {});
        }

        fn set<K, V>(&mut self, key: K, val: V)
        where
            K: AsRef<OsStr>,
            V: AsRef<OsStr>,
        {
            self.push_env_directive(kuro_forkserver_proto::EnvSet {
                key: key.as_ref().as_bytes().to_vec(),
                value: val.as_ref().as_bytes().to_vec(),
            })
        }

        fn remove<K>(&mut self, key: K)
        where
            K: AsRef<OsStr>,
        {
            self.push_env_directive(kuro_forkserver_proto::EnvRemove {
                key: key.as_ref().as_bytes().to_vec(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::str;

    use assert_matches::assert_matches;
    use host_sharing::HostSharingStrategy;
    use kuro_common::liveliness_observer::NoopLivelinessObserver;
    use kuro_core::cells::CellResolver;
    use kuro_core::cells::cell_root_path::CellRootPathBuf;
    use kuro_core::cells::name::CellName;
    use kuro_core::fs::buck_out_path::BuckOutPathResolver;
    use kuro_core::fs::project::ProjectRoot;
    use kuro_core::fs::project::ProjectRootTemp;
    use kuro_execute::execute::blocking::testing::DummyBlockingExecutor;
    use kuro_execute::materialize::nodisk::NoDiskMaterializer;

    use super::*;

    fn artifact_fs(project_fs: ProjectRoot) -> ArtifactFs {
        ArtifactFs::new(
            CellResolver::testing_with_name_and_path(
                CellName::testing_new("cell"),
                CellRootPathBuf::new(ProjectRelativePathBuf::unchecked_new("cell_path".into())),
            ),
            BuckOutPathResolver::new(ProjectRelativePathBuf::unchecked_new("buck_out/v2".into())),
            project_fs,
        )
    }

    fn test_executor() -> kuro_error::Result<(LocalExecutor, AbsNormPathBuf, ProjectRootTemp)> {
        let temp = ProjectRootTemp::new().unwrap();
        let project_fs = temp.path();
        let artifact_fs = artifact_fs(project_fs.dupe());

        let executor = LocalExecutor::new(
            artifact_fs,
            Arc::new(NoDiskMaterializer),
            Arc::new(IncrementalDbState::db_disabled()),
            Arc::new(DummyBlockingExecutor {
                fs: project_fs.dupe(),
            }),
            Arc::new(HostSharingBroker::new(
                HostSharingStrategy::SmallerTasksFirst,
                1,
            )),
            temp.path().root().to_buf(),
            ForkserverAccess::None,
            ExecutorGlobalKnobs::default(),
            None,
            None,
            DaemonId::new(),
        );

        Ok((executor, temp.path().root().to_buf(), temp))
    }

    #[tokio::test]
    async fn test_exec_cmd_environment() -> kuro_error::Result<()> {
        let (executor, root, _tmpdir) = test_executor()?;

        let interpreter = if cfg!(windows) { "powershell" } else { "sh" };
        let CommandResult { status, stdout, .. } = executor
            .exec(
                interpreter,
                ["-c", "echo $PWD; pwd"],
                &HashMap::<String, String>::default(),
                ProjectRelativePath::empty(),
                None,
                None,
                NoopLivelinessObserver::create(),
                false,
                None,
                futures::stream::pending(),
            )
            .await?;
        assert_matches!(status, GatherOutputStatus::Finished { exit_code, .. } if exit_code == 0);

        let stdout = std::str::from_utf8(&stdout).buck_error_context("Invalid stdout")?;

        if cfg!(windows) {
            let lines: Vec<&str> = stdout.split("\r\n").collect();
            let expected_path = format!("{root}");

            assert_eq!(lines[3], expected_path);
            assert_eq!(lines[4], expected_path);
        } else {
            assert_eq!(stdout, format!("{root}\n{root}\n"));
        }

        Ok(())
    }

    #[cfg(fbcode_build)]
    #[tokio::test]
    async fn test_exec_cmd_timeout() -> kuro_error::Result<()> {
        let (executor, _, _tmpdir) = test_executor()?;

        let interpreter = if cfg!(windows) { "powershell" } else { "sh" };
        let CommandResult { status, .. } = executor
            .exec(
                interpreter,
                ["-c", "sleep 2s"],
                &HashMap::<String, String>::default(),
                ProjectRelativePath::empty(),
                Some(Duration::from_secs(1)),
                None,
                NoopLivelinessObserver::create(),
                false,
                None,
                futures::stream::pending(),
            )
            .await?;
        assert_matches!(status, GatherOutputStatus::TimedOut ( duration ) if duration == Duration::from_secs(1));

        Ok(())
    }

    #[cfg(unix)] // TODO: something similar on Windows: T123279320
    #[tokio::test]
    async fn test_exec_cmd_environment_filtering() -> kuro_error::Result<()> {
        use kuro_execute::execute::environment_inheritance::EnvironmentInheritance;

        let (executor, _root, _tmpdir) = test_executor()?;

        let CommandResult { status, stdout, .. } = executor
            .exec(
                "sh",
                ["-c", "echo $USER"],
                &HashMap::<String, String>::default(),
                ProjectRelativePath::empty(),
                None,
                Some(&EnvironmentInheritance::empty()),
                NoopLivelinessObserver::create(),
                false,
                None,
                futures::stream::pending(),
            )
            .await?;
        assert_matches!(status, GatherOutputStatus::Finished { exit_code, .. } if exit_code == 0);
        assert_eq!(stdout, b"\n");

        Ok(())
    }
}
