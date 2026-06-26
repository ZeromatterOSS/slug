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
use std::path::Path;
use std::path::PathBuf;
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
use slug_build_signals::env::WaitingCategory;
use slug_common::file_ops::metadata::FileDigestConfig;
use slug_common::liveliness_observer::LivelinessObserver;
use slug_common::liveliness_observer::LivelinessObserverExt;
use slug_common::liveliness_observer::NoopLivelinessObserver;
use slug_common::local_resource_state::LocalResourceHolder;
use slug_core::content_hash::ContentBasedPathHash;
use slug_core::fs::artifact_path_resolver::ArtifactFs;
use slug_core::fs::buck_out_path::BuildArtifactPath;
use slug_core::fs::project::ProjectRoot;
use slug_core::fs::project_rel_path::ProjectRelativePath;
use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
use slug_core::soft_error;
use slug_core::tag_error;
use slug_core::tag_result;
use slug_error::BuckErrorContext;
use slug_error::slug_error;
use slug_events::daemon_id::DaemonId;
use slug_events::dispatch::EventDispatcher;
use slug_events::dispatch::get_dispatcher_opt;
use slug_execute::artifact_utils::ArtifactValueBuilder;
use slug_execute::artifact_value::ArtifactValue;
use slug_execute::digest_config::DigestConfig;
use slug_execute::directory::extract_artifact_value;
use slug_execute::directory::insert_entry;
use slug_execute::entry::HashingInfo;
use slug_execute::entry::build_entry_from_disk;
use slug_execute::execute::action_digest::ActionDigest;
use slug_execute::execute::blocking::BlockingExecutor;
use slug_execute::execute::clean_output_paths::CleanOutputPaths;
use slug_execute::execute::environment_inheritance::EnvironmentInheritance;
use slug_execute::execute::executor_stage_async;
use slug_execute::execute::inputs_directory::inputs_directory;
use slug_execute::execute::kind::CommandExecutionKind;
use slug_execute::execute::manager::CommandExecutionManager;
use slug_execute::execute::manager::CommandExecutionManagerExt;
use slug_execute::execute::manager::CommandExecutionManagerWithClaim;
use slug_execute::execute::output::CommandStdStreams;
use slug_execute::execute::prepared::PreparedCommand;
use slug_execute::execute::prepared::PreparedCommandExecutor;
use slug_execute::execute::request::CommandExecutionInput;
use slug_execute::execute::request::CommandExecutionOutput;
use slug_execute::execute::request::CommandExecutionOutputRef;
use slug_execute::execute::request::CommandExecutionRequest;
use slug_execute::execute::request::ExecutorPreference;
use slug_execute::execute::request::ParamFileFormat;
use slug_execute::execute::result::CommandExecutionMetadata;
use slug_execute::execute::result::CommandExecutionResult;
use slug_execute::knobs::ExecutorGlobalKnobs;
use slug_execute::materialize::materializer::CopiedArtifact;
use slug_execute::materialize::materializer::DeclareArtifactPayload;
use slug_execute::materialize::materializer::MaterializationError;
use slug_execute::materialize::materializer::Materializer;
use slug_execute::output_size::OutputSize;
use slug_execute_local::CommandResult;
use slug_execute_local::DefaultKillProcess;
use slug_execute_local::GatherOutputStatus;
use slug_execute_local::decode_command_event_stream;
use slug_execute_local::maybe_absolutize_exe;
use slug_execute_local::spawn_command_and_stream_events;
use slug_execute_local::status_decoder::DefaultStatusDecoder;
use slug_fs::async_fs_util;
use slug_fs::fs_util;
use slug_fs::paths::abs_norm_path::AbsNormPathBuf;
use slug_fs::paths::abs_path::AbsPath;
use slug_resource_control::ActionFreezeEvent;
use slug_resource_control::ActionFreezeEventReceiver;
use slug_resource_control::CommandType;
use slug_resource_control::action_cgroups::ActionCgroupSession;
use slug_resource_control::memory_tracker::MemoryTrackerHandle;
use slug_resource_control::path::CgroupPathBuf;
use slug_sandbox::SandboxSpec;
use slug_util::process::background_command;
use slug_util::time_span::TimeSpan;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::info;

use crate::executors::worker::WorkerHandle;
use crate::executors::worker::WorkerPool;
use crate::incremental_actions_helper::get_incremental_path_map;
use crate::incremental_actions_helper::save_content_based_incremental_state;
use crate::sqlite::incremental_state_db::IncrementalDbState;

#[derive(Debug, slug_error::Error)]
#[slug(input)]
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
    Client(slug_forkserver::client::ForkserverClient),
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
        action_execroot: Option<&'a slug_fs::paths::abs_norm_path::AbsNormPath>,
    ) -> impl futures::future::Future<Output = slug_error::Result<CommandResult>> + Send + 'a {
        async move {
            // Plan 44 Phase 2.6: cwd is the per-action execroot built
            // by `action_execroot::ensure_execroot`. It contains only
            // the symlinks the action's declared inputs require, so
            // `read_dir(cwd)` returns exactly the prefixes the action
            // needs — matching Bazel's exec_root invariant.
            //
            // Phase 2.5 fallback: when the caller didn't supply a
            // per-action execroot (e.g. a code path that hasn't been
            // updated, or non-action shell-out tests), fall back to
            // the shared synthesized execroot from
            // `slug_core::cells::execroot_path`. Last resort: the
            // project root.
            let working_directory = match action_execroot {
                Some(execroot) => std::borrow::Cow::Owned(execroot.join(working_directory)),
                None => match slug_core::cells::execroot_path(self.root.as_path())
                    .filter(|p| p.is_dir())
                    .and_then(|p| slug_fs::paths::abs_norm_path::AbsNormPathBuf::new(p).ok())
                {
                    Some(execroot) => std::borrow::Cow::Owned(execroot.join(working_directory)),
                    None => self.root.join_cow(working_directory),
                },
            };

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
                    let spawn_cwd = windows_spawn_path(working_directory.as_path());
                    let spawn_exe = windows_spawn_path(exe.as_ref());
                    let spawn_args = windows_spawn_args(args);
                    let mut cmd = background_command(spawn_exe.as_os_str());
                    cmd.current_dir(&spawn_cwd);
                    cmd.args(&spawn_args);
                    apply_local_execution_environment(
                        &mut cmd,
                        &working_directory,
                        env,
                        env_inheritance,
                    );
                    #[cfg(windows)]
                    cmd.env("PWD", &spawn_cwd);

                    // Apply filesystem sandbox if requested.
                    if let Some(sandbox_spec) = sandbox {
                        slug_sandbox::apply_sandbox(&mut cmd, sandbox_spec);
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
            slug_data::LocalStage {
                stage: Some(slug_data::LocalPrepareOutputDirs {}.into()),
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

                slug_error::Ok(())
            },
        )
        .boxed()
        .await
        {
            return Err(manager.error("prepare_output_dirs_failed", e));
        };

        // Materialize per-Args paramfile slots into the freshly prepared
        // scratch dir, then splice the slot's `param_file_arg` (with `%s` →
        // path) over the slot range in `args`. Iterate slots in descending
        // `start` order so earlier indices remain valid after splicing.
        let scratch_dir = scratch_path
            .0
            .as_ref()
            .map(|sp| self.artifact_fs.fs().resolve(sp).as_path().to_owned())
            .unwrap_or_else(std::env::temp_dir);
        // Plan 44 Phase 2.6: per-action execroot keyed by the action's
        // declared input prefix set. Compute it before final argv preparation
        // so rustc invocations can remap the physical digest execroot out of
        // metadata while still executing from the narrowed filesystem view.
        let execroot_plan =
            crate::executors::action_execroot::collect_execroot_plan(request, &self.artifact_fs);
        let action_execroot =
            crate::executors::action_execroot::ensure_execroot(&self.root, &execroot_plan);
        let param_args_owned: Option<Vec<String>> = if request.param_files().is_empty() {
            None
        } else {
            let exe_len = request.exe().len();
            let mut new_args: Vec<String> = args.to_vec();
            let mut any_failed = false;
            let mut slots: Vec<&slug_execute::execute::request::ParamFileSlot> =
                request.param_files().iter().collect();
            slots.sort_by(|a, b| b.start.cmp(&a.start));
            for (slot_idx, slot) in slots.iter().enumerate() {
                let arg_start = exe_len + slot.start;
                let arg_end = exe_len + slot.end;
                if arg_end > new_args.len() || arg_start > arg_end {
                    tracing::warn!(
                        "param_file slot out of range: start={} end={} args_len={}",
                        slot.start,
                        slot.end,
                        new_args.len()
                    );
                    any_failed = true;
                    break;
                }
                let slot_args = &new_args[arg_start..arg_end];
                let needs_materialize = slot.use_always || {
                    let total: usize = slot_args.iter().map(|s| s.len() + 1).sum();
                    total > 32768
                };
                if !needs_materialize {
                    continue;
                }
                let param_path = scratch_dir.join(format!("slug-params-{}", slot_idx));
                let mut slot_args_for_file = slot_args.to_vec();
                if let Err(e) =
                    rewrite_rustc_llvm_linker_for_compiler_rt(&mut slot_args_for_file, &scratch_dir)
                {
                    tracing::warn!("Failed to prepare rustc linker wrapper: {e}");
                    any_failed = true;
                    break;
                }
                if should_add_rustc_paramfile_execroot_remap(&new_args) {
                    add_rustc_flags_execroot_remap(
                        &mut slot_args_for_file,
                        action_execroot
                            .as_ref()
                            .map(|execroot| execroot.as_abs_norm_path()),
                    );
                }
                if cfg!(windows) {
                    add_windows_msvc_rust_lld_crt_args(&mut slot_args_for_file);
                }
                let content = match slot.format {
                    ParamFileFormat::Shell => slot_args_for_file
                        .iter()
                        .map(|s| {
                            let mut q = String::from("'");
                            q.push_str(&s.replace('\'', "'\\''"));
                            q.push('\'');
                            q
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    ParamFileFormat::Multiline | ParamFileFormat::FlagPerLine => {
                        slot_args_for_file.join("\n")
                    }
                };
                if let Err(e) = std::fs::write(&param_path, content) {
                    tracing::warn!(
                        "Failed to write param file {}: {}; using inline args",
                        param_path.display(),
                        e
                    );
                    any_failed = true;
                    break;
                }
                let replacement = slot
                    .param_file_arg
                    .replace("%s", &param_path.to_string_lossy());
                new_args.splice(arg_start..arg_end, std::iter::once(replacement));
            }
            if any_failed { None } else { Some(new_args) }
        };
        let mut args_owned = param_args_owned;
        match rewrite_inline_rustc_llvm_linker_for_compiler_rt(
            args_owned.as_deref().unwrap_or(args),
            &scratch_dir,
        ) {
            Ok(Some(rewritten_args)) => args_owned = Some(rewritten_args),
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("Failed to prepare inline rustc linker wrapper: {e}");
            }
        }
        if let Some(execroot) = action_execroot
            .as_ref()
            .map(|execroot| execroot.as_abs_norm_path())
        {
            let mut rewritten_args = args_owned.as_deref().unwrap_or(args).to_vec();
            if rewrite_process_wrapper_execroot_substitutions(
                &mut rewritten_args,
                execroot,
                self.root.as_path(),
            ) {
                args_owned = Some(rewritten_args);
                tracing::debug!("Rewrote process_wrapper execroot substitutions");
            }
        }
        if let Some(args) = args_owned.as_mut() {
            if add_rustc_execroot_remap(
                args,
                action_execroot
                    .as_ref()
                    .map(|execroot| execroot.as_abs_norm_path()),
            ) {
                tracing::debug!("Added rustc remap for physical action execroot");
            }
        } else if args_owned.is_none() {
            let mut rewritten_args = args.to_vec();
            if add_rustc_execroot_remap(
                &mut rewritten_args,
                action_execroot
                    .as_ref()
                    .map(|execroot| execroot.as_abs_norm_path()),
            ) {
                args_owned = Some(rewritten_args);
                tracing::debug!("Added rustc remap for physical action execroot");
            }
        }
        if cfg!(windows) {
            let mut rewritten_args = args_owned.as_deref().unwrap_or(args).to_vec();
            if rewrite_windows_cargo_build_script_runner_args(
                &mut rewritten_args,
                action_execroot
                    .as_ref()
                    .map(|execroot| execroot.as_abs_norm_path()),
            ) {
                args_owned = Some(rewritten_args);
            }
        }
        if cfg!(windows) {
            let mut rewritten_args = args_owned.as_deref().unwrap_or(args).to_vec();
            if rewrite_windows_process_wrapper_child_tool_path(
                &mut rewritten_args,
                action_execroot
                    .as_ref()
                    .map(|execroot| execroot.as_abs_norm_path()),
                self.root.as_path(),
            ) {
                args_owned = Some(rewritten_args);
            }
        }
        if cfg!(windows) {
            let mut rewritten_args = args_owned.as_deref().unwrap_or(args).to_vec();
            if add_windows_msvc_rust_lld_crt_link_args(&mut rewritten_args) {
                args_owned = Some(rewritten_args);
            }
        }
        if cfg!(windows) {
            let mut rewritten_args = args_owned.as_deref().unwrap_or(args).to_vec();
            if parametrize_windows_process_wrapper_rustc_tail(&mut rewritten_args, &scratch_dir) {
                args_owned = Some(rewritten_args);
            }
        }
        let args: &[String] = args_owned.as_deref().unwrap_or(args);

        let (time_span, start_time, res) = executor_stage_async(
            {
                let env = env
                    .iter()
                    .copied()
                    .map(|(k, v)| slug_data::EnvironmentEntry {
                        key: k.to_owned(),
                        value: v.into_string_lossy(),
                    })
                    .collect();

                let stage = match worker {
                    None => slug_data::LocalExecute {
                        command: Some(slug_data::LocalCommand {
                            action_digest: action_digest.to_string(),
                            argv: args.to_vec(),
                            env,
                        }),
                    }
                    .into(),
                    Some(_) => slug_data::WorkerExecute {
                        command: Some(slug_data::WorkerCommand {
                            action_digest: action_digest.to_string(),
                            argv: request.args().to_vec(),
                            env,
                            fallback_exe: request.exe().to_vec(),
                        }),
                    }
                    .into(),
                };
                slug_data::LocalStage { stage: Some(stage) }
            },
            async move {
                let execution_start = TimeSpan::start_now();
                let start_time = SystemTime::now();

                let mut env: Vec<(OsString, OsString)> = env
                    .iter()
                    .map(|(k, v)| (OsString::from(k), v.into_os_str().to_owned()))
                    .collect();
                let mut args_for_exec = args.to_vec();
                let args = if rewrite_windows_cargo_manifest_dir_env(
                    &mut args_for_exec,
                    &mut env,
                    action_execroot
                        .as_ref()
                        .map(|execroot| execroot.as_abs_norm_path()),
                ) {
                    args_for_exec.as_slice()
                } else {
                    args
                };
                let r = if let Some(worker) = worker {
                    Ok(worker
                        .exec_cmd(request.args(), env, request.timeout())
                        .await)
                } else {
                    let env = env.iter().map(|(k, v)| (k.as_os_str(), v.as_os_str()));
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
                        action_execroot
                            .as_ref()
                            .map(|execroot| execroot.as_abs_norm_path()),
                    )
                    .await
                };

                let r = match (
                    r,
                    action_execroot
                        .as_ref()
                        .map(|execroot| execroot.as_abs_norm_path()),
                ) {
                    (Ok(res), Some(execroot)) => sync_outputs_from_action_execroot(
                        execroot.as_path(),
                        self.root.as_path(),
                        request,
                        &self.artifact_fs,
                    )
                    .map(|()| res),
                    (other, _) => other,
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
                let kill_awaiter = slug_util::async_move_clone!(retry_future, {
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
            slug_data::LocalStage {
                stage: Some(slug_data::LocalMaterializeInputs {}.into()),
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

                            slug_error::Ok(())
                        } else {
                            Ok(())
                        }
                    },
                )
                .await;

                let scratch_path = r1?.scratch;
                r2?;

                slug_error::Ok((scratch_path, Instant::now() - start))
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
                    slug_error!(
                        slug_error::ErrorTag::DispatcherUnavailable,
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
                        slug_error!(
                            slug_error::ErrorTag::Environment,
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

        // Param-file slot materialization happens inside `exec_once` after
        // `prep_scratch_path` cleans+recreates the scratch dir; otherwise our
        // freshly written paramfiles would be deleted before the action runs.
        let effective_args: &[String] = args;

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
                    execution_stats.map(|s| slug_data::CommandExecutionStats {
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

        if !request.outputs_cleanup {
            if let Some(run_action_key) = request.run_action_key() {
                save_content_based_incremental_state(
                    run_action_key.clone(),
                    &self.incremental_db_state,
                    &self.artifact_fs,
                    &result,
                );
            }
        }

        result
    }

    async fn calculate_and_declare_output_values(
        &self,
        request: &CommandExecutionRequest,
        digest_config: DigestConfig,
    ) -> slug_error::Result<(IndexMap<CommandExecutionOutput, ArtifactValue>, HashingInfo)> {
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
            let permission_path = abspath.clone();
            self.blocking_executor
                .execute_io_inline(move || normalize_local_output_permissions(&permission_path))
                .await
                .with_buck_error_context(|| {
                    format!("normalizing output permissions for {path:?}")
                })?;
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
                            if let Some((configuration_hash_path, symlink_value)) =
                                content_based_configuration_symlink(
                                    self.artifact_fs.fs(),
                                    digest_config,
                                    &value,
                                    &configuration_hash_path,
                                    hashed_path.clone(),
                                )?
                            {
                                configuration_path_to_content_based_path_symlinks
                                    .push((configuration_hash_path, symlink_value));
                            }

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
        // `slug log what-materialized` reports them.
        if let Some(dispatcher) = get_dispatcher_opt() {
            for (path, file_count, total_bytes) in matl_stats {
                let path_str = path.as_str().to_owned();
                dispatcher.span(
                    slug_data::MaterializationStart {
                        action_digest: None,
                    },
                    || {
                        (
                            (),
                            slug_data::MaterializationEnd {
                                action_digest: None,
                                file_count,
                                total_bytes,
                                path: path_str,
                                success: true,
                                error: None,
                                method: Some(slug_data::MaterializationMethod::Write as i32),
                            },
                        )
                    },
                );
            }
        }
        slug_util::future::try_join_all(output_path_to_content_based_path_copies.into_iter().map(
            |(path, value, copied_artifacts)| {
                self.materializer
                    .declare_copy(path, value, copied_artifacts)
            },
        ))
        .await?;
        slug_util::future::try_join_all(
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
                        slug_data::LocalStage {
                            stage: Some(slug_data::WorkerQueued {}.into()),
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
                slug_data::LocalStage {
                    stage: Some(
                        slug_data::WorkerInit {
                            command: Some(slug_data::WorkerInitCommand {
                                argv: worker_spec.exe.clone(),
                                env: worker_spec
                                    .env
                                    .iter()
                                    .map(|(k, v)| slug_data::EnvironmentEntry {
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
                slug_data::LocalStage {
                    stage: Some(slug_data::WorkerWait {}.into()),
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
    ) -> slug_error::Result<()> {
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
            .collect::<slug_error::Result<Vec<_>>>()?;

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

#[cfg(unix)]
fn normalize_local_output_permissions(path: &AbsNormPathBuf) -> slug_error::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let Some(metadata) = fs_util::symlink_metadata_if_exists(path)? else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() {
        return Ok(());
    }

    if metadata.is_dir() {
        for entry in fs_util::read_dir(path)? {
            let entry = entry?;
            normalize_local_output_permissions(&entry.path())?;
        }
    }

    let mut permissions = metadata.permissions();
    permissions.set_mode((permissions.mode() & !0o777) | 0o555);
    fs_util::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn normalize_local_output_permissions(_path: &AbsNormPathBuf) -> slug_error::Result<()> {
    Ok(())
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
            slug_data::LocalStage {
                stage: Some(slug_data::AcquireLocalResource {}.into()),
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
            slug_data::LocalStage {
                stage: Some(slug_data::LocalQueued {}.into()),
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
) -> slug_error::Result<MaterializedInputPaths> {
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
                            } else if let Some((configuration_hash_path, symlink_value)) =
                                content_based_configuration_symlink(
                                    artifact_fs.fs(),
                                    digest_config,
                                    artifact_value,
                                    &configuration_hash_path,
                                    content_based_path,
                                )?
                            {
                                configuration_path_to_content_based_path_symlinks
                                    .push((configuration_hash_path.clone(), symlink_value));
                                paths.push(configuration_hash_path);
                            } else {
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
                if let Some(data) = &metadata.data {
                    let abs_path = artifact_fs.fs().resolve(&path);
                    if let Some(parent) = abs_path.parent() {
                        async_fs_util::create_dir_all(parent).await?;
                    }
                    async_fs_util::write(&abs_path, &data.0).await?;
                } else {
                    paths.push(path);
                }
            }
            CommandExecutionInput::ScratchPath(path) => {
                let path = artifact_fs.buck_out_path_resolver().resolve_scratch(path)?;

                if scratch.0.is_some() {
                    return Err(slug_error::internal_error!(
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

    slug_util::future::try_join_all(
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

fn rewrite_rustc_llvm_linker_for_compiler_rt(
    args: &mut [String],
    scratch_dir: &Path,
) -> slug_error::Result<bool> {
    #[cfg(not(unix))]
    {
        let _ = (args, scratch_dir);
        return Ok(false);
    }

    #[cfg(unix)]
    {
        let Some(linker_idx) = args
            .iter()
            .position(|arg| arg.starts_with("--codegen=linker="))
        else {
            return Ok(false);
        };
        let linker = args[linker_idx]
            .trim_start_matches("--codegen=linker=")
            .to_owned();
        if !should_filter_rustc_implicit_gcc_s(args, &linker) {
            return Ok(false);
        }

        let wrapper = scratch_dir.join("slug-rustc-clangxx-filter-gcc-s");
        write_rustc_linker_filter_wrapper(&wrapper, &linker)?;
        args[linker_idx] = format!("--codegen=linker={}", wrapper.to_string_lossy());
        Ok(true)
    }
}

fn rewrite_inline_rustc_llvm_linker_for_compiler_rt(
    args: &[String],
    scratch_dir: &Path,
) -> slug_error::Result<Option<Vec<String>>> {
    let mut rewritten_args = args.to_vec();
    if rewrite_rustc_llvm_linker_for_compiler_rt(&mut rewritten_args, scratch_dir)? {
        Ok(Some(rewritten_args))
    } else {
        Ok(None)
    }
}

fn add_rustc_execroot_remap(
    args: &mut Vec<String>,
    action_execroot: Option<&slug_fs::paths::abs_norm_path::AbsNormPath>,
) -> bool {
    let Some(execroot) = action_execroot else {
        return false;
    };
    if !is_process_wrapper_rustc_invocation(args) {
        return false;
    }
    add_rustc_flags_execroot_remap(args, Some(execroot))
}

fn rewrite_process_wrapper_execroot_substitutions(
    args: &mut [String],
    action_execroot: &slug_fs::paths::abs_norm_path::AbsNormPath,
    output_base: &Path,
) -> bool {
    if !is_process_wrapper_invocation(args) {
        return false;
    }

    let substitutions =
        process_wrapper_execroot_substitution_values(action_execroot.as_path(), output_base);
    let mut changed = false;
    let mut idx = 0;
    while idx < args.len() {
        if args[idx] == "--subst" {
            if let Some(arg) = args.get_mut(idx + 1) {
                changed |= rewrite_process_wrapper_subst_value(arg, &substitutions);
            }
            idx += 2;
            continue;
        }
        if let Some(subst) = args[idx].strip_prefix("--subst=") {
            let mut rewritten = subst.to_owned();
            if rewrite_process_wrapper_subst_value(&mut rewritten, &substitutions) {
                args[idx] = format!("--subst={rewritten}");
                changed = true;
            }
        }
        idx += 1;
    }
    changed
}

struct ProcessWrapperSubstitutions {
    pwd: String,
    execroot: String,
    output_base: String,
}

fn process_wrapper_execroot_substitution_values(
    action_execroot: &Path,
    output_base: &Path,
) -> ProcessWrapperSubstitutions {
    #[cfg(unix)]
    {
        let stable_execroot = slug_core::cells::execroot_path(output_base)
            .unwrap_or_else(|| action_execroot.to_path_buf());
        let action_execroot = action_execroot.to_string_lossy().into_owned();
        ProcessWrapperSubstitutions {
            pwd: action_execroot.clone(),
            execroot: stable_execroot.to_string_lossy().into_owned(),
            output_base: action_execroot,
        }
    }

    #[cfg(not(unix))]
    {
        ProcessWrapperSubstitutions {
            pwd: action_execroot.to_string_lossy().into_owned(),
            execroot: action_execroot.to_string_lossy().into_owned(),
            output_base: output_base.to_string_lossy().into_owned(),
        }
    }
}

fn rewrite_process_wrapper_subst_value(
    arg: &mut String,
    substitutions: &ProcessWrapperSubstitutions,
) -> bool {
    let Some((key, value)) = arg.split_once('=') else {
        return false;
    };
    let Some(replacement) = (match (key, value) {
        ("pwd", "${pwd}") => Some(substitutions.pwd.as_str()),
        ("exec_root", "${exec_root}") => Some(substitutions.execroot.as_str()),
        ("output_base", "${output_base}") => Some(substitutions.output_base.as_str()),
        _ => None,
    }) else {
        return false;
    };
    *arg = format!("{key}={replacement}");
    true
}

fn add_rustc_flags_execroot_remap(
    args: &mut Vec<String>,
    action_execroot: Option<&slug_fs::paths::abs_norm_path::AbsNormPath>,
) -> bool {
    let Some(execroot) = action_execroot else {
        return false;
    };
    if args.iter().any(|arg| {
        arg == "--remap-path-prefix=${pwd}=." || arg == "--remap-path-prefix=${exec_root}=."
    }) {
        return false;
    }
    let remap = format!(
        "--remap-path-prefix={}={}",
        rustc_execroot_remap_prefix(execroot),
        "."
    );
    if args.iter().any(|arg| arg == &remap) {
        return false;
    }
    let insert_at = args
        .iter()
        .position(|arg| arg.starts_with("--remap-path-prefix="))
        .unwrap_or(args.len());
    args.insert(insert_at, remap);
    true
}

fn rustc_execroot_remap_prefix(execroot: &slug_fs::paths::abs_norm_path::AbsNormPath) -> String {
    execroot.as_path().to_string_lossy().into_owned()
}

fn should_add_rustc_paramfile_execroot_remap(args: &[String]) -> bool {
    is_process_wrapper_rustc_invocation(args)
}

fn windows_spawn_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        windows_short_path(path).unwrap_or_else(|| path.to_path_buf())
    }

    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

fn windows_spawn_args<'a>(args: impl IntoIterator<Item = impl AsRef<OsStr> + 'a>) -> Vec<OsString> {
    args.into_iter()
        .map(|arg| windows_spawn_arg(arg.as_ref()))
        .collect()
}

fn windows_spawn_arg(arg: &OsStr) -> OsString {
    #[cfg(windows)]
    {
        windows_short_path_arg(arg).unwrap_or_else(|| arg.to_os_string())
    }

    #[cfg(not(windows))]
    {
        arg.to_os_string()
    }
}

#[cfg(windows)]
fn windows_short_path(path: &Path) -> Option<PathBuf> {
    unsafe extern "system" {
        fn GetShortPathNameW(
            lpszLongPath: *const u16,
            lpszShortPath: *mut u16,
            cchBuffer: u32,
        ) -> u32;
    }

    fn get_short_path(path: &Path) -> Option<PathBuf> {
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::ffi::OsStringExt;

        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let len = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        let written = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), len) };
        if written == 0 || written >= len {
            return None;
        }
        Some(PathBuf::from(OsString::from_wide(&buf[..written as usize])))
    }

    get_short_path(path).or_else(|| {
        let text = path.as_os_str().to_string_lossy();
        if text.starts_with(r"\\?\") || !path.is_absolute() {
            return None;
        }
        let prefixed = PathBuf::from(format!(r"\\?\{text}"));
        let short = get_short_path(&prefixed)?;
        let short_text = short.as_os_str().to_string_lossy();
        short_text
            .strip_prefix(r"\\?\")
            .map(PathBuf::from)
            .or(Some(short))
    })
}

#[cfg(windows)]
fn windows_short_path_arg(arg: &OsStr) -> Option<OsString> {
    let path = Path::new(arg);
    if path.is_absolute() {
        return windows_short_path(path).map(OsString::from);
    }

    let text = arg.to_string_lossy();
    for prefix in [
        "/LIBPATH:",
        "-LIBPATH:",
        "/OUT:",
        "-OUT:",
        "/IMPLIB:",
        "-IMPLIB:",
        "/PDB:",
        "-PDB:",
    ] {
        let Some(rest) = text.strip_prefix(prefix) else {
            continue;
        };
        let rest_path = Path::new(rest);
        if !rest_path.is_absolute() {
            continue;
        }
        let short = windows_short_path(rest_path)?;
        return Some(OsString::from(format!(
            "{prefix}{}",
            short.to_string_lossy()
        )));
    }

    None
}

fn parametrize_windows_process_wrapper_rustc_tail(
    args: &mut Vec<String>,
    scratch_dir: &Path,
) -> bool {
    #[cfg(not(windows))]
    {
        let _ = (args, scratch_dir);
        false
    }

    #[cfg(windows)]
    {
        if !is_process_wrapper_rustc_invocation(args) {
            return false;
        }
        if windows_command_line_len(args) <= 30_000 {
            return false;
        }
        let Some(separator) = args.iter().position(|arg| arg == "--") else {
            return false;
        };
        let tail_start = separator + 2;
        if tail_start >= args.len() {
            return false;
        }

        let param_path = scratch_dir.join("slug-process-wrapper-rustc-tail.params");
        let content = args[tail_start..]
            .iter()
            .map(|arg| encode_process_wrapper_paramfile_arg(arg))
            .collect::<Vec<_>>()
            .join("\n");
        if let Err(e) = std::fs::write(&param_path, content) {
            tracing::warn!(
                ?e,
                path = %param_path.display(),
                "failed to write process_wrapper rustc tail paramfile"
            );
            return false;
        }
        args.splice(
            tail_start..,
            std::iter::once(format!("@{}", param_path.to_string_lossy())),
        );
        true
    }
}

fn add_windows_msvc_rust_lld_crt_link_args(args: &mut Vec<String>) -> bool {
    #[cfg(not(windows))]
    {
        let _ = args;
        false
    }

    #[cfg(windows)]
    {
        if !is_process_wrapper_rustc_invocation(args) {
            return false;
        }
        let Some(separator) = args.iter().position(|arg| arg == "--") else {
            return false;
        };
        let Some(missing) = missing_windows_msvc_rust_lld_crt_args(&args[(separator + 2)..]) else {
            return false;
        };
        args.extend(missing);
        true
    }
}

fn add_windows_msvc_rust_lld_crt_args(args: &mut Vec<String>) -> bool {
    #[cfg(not(windows))]
    {
        let _ = args;
        false
    }

    #[cfg(windows)]
    {
        let Some(missing) = missing_windows_msvc_rust_lld_crt_args(args) else {
            return false;
        };
        args.extend(missing);
        true
    }
}

#[cfg(windows)]
fn missing_windows_msvc_rust_lld_crt_args(args: &[String]) -> Option<Vec<String>> {
    if !args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "-Clinker=rust-lld"
                | "-Clinker=rust-lld.exe"
                | "--codegen=linker=rust-lld"
                | "--codegen=linker=rust-lld.exe"
        )
    }) {
        return None;
    }
    if !args
        .iter()
        .any(|arg| arg.contains("windows-msvc") || arg.contains("\\msvc") || arg.contains("/msvc"))
    {
        return None;
    }

    let missing: Vec<String> = ["/nodefaultlib:libucrt", "ucrt.lib", "oldnames.lib"]
        .into_iter()
        .map(|lib| format!("--codegen=link-arg={lib}"))
        .filter(|link_arg| !args.iter().any(|arg| arg == link_arg))
        .collect();
    if missing.is_empty() {
        None
    } else {
        Some(missing)
    }
}

#[cfg(not(windows))]
fn missing_windows_msvc_rust_lld_crt_args(args: &[String]) -> Option<Vec<String>> {
    let _ = args;
    None
}

fn rewrite_windows_process_wrapper_child_tool_path(
    args: &mut [String],
    action_execroot: Option<&slug_fs::paths::abs_norm_path::AbsNormPath>,
    project_root: &Path,
) -> bool {
    #[cfg(not(windows))]
    {
        let _ = (args, action_execroot, project_root);
        false
    }

    #[cfg(windows)]
    {
        if !is_process_wrapper_rustc_invocation(args) {
            return false;
        }
        let Some(separator) = args.iter().position(|arg| arg == "--") else {
            return false;
        };
        let Some(tool) = args.get_mut(separator + 1) else {
            return false;
        };
        let tool_path = Path::new(tool.as_str());
        if tool_path.is_absolute() {
            return false;
        }

        let mut candidates = Vec::new();
        if let Some(execroot) = action_execroot {
            let absolute = execroot.as_path().join(tool_path);
            if absolute.exists() {
                candidates.push(absolute);
            }
        }
        let project_absolute = project_root.join(tool_path);
        if project_absolute.exists() {
            candidates.push(project_absolute);
        }

        let Some(replacement) = candidates
            .into_iter()
            .map(|path| windows_short_path(&path).unwrap_or(path))
            .min_by_key(|path| path.as_os_str().len())
        else {
            return false;
        };
        *tool = replacement.to_string_lossy().into_owned();
        true
    }
}

#[cfg(windows)]
fn windows_command_line_len(args: &[String]) -> usize {
    // Conservative estimate for CreateProcessW's command-line string:
    // arguments plus separating spaces and quotes for args that need them.
    args.iter()
        .map(|arg| {
            let quote_overhead = if arg.contains([' ', '\t', '"']) { 2 } else { 0 };
            arg.len() + quote_overhead + 1
        })
        .sum()
}

#[cfg(windows)]
fn encode_process_wrapper_paramfile_arg(arg: &str) -> String {
    let trailing_backslashes = arg.chars().rev().take_while(|c| *c == '\\').count();
    if trailing_backslashes == 0 {
        return arg.to_owned();
    }
    let split_at = arg.len() - trailing_backslashes;
    let mut encoded = String::with_capacity(arg.len() + trailing_backslashes);
    encoded.push_str(&arg[..split_at]);
    for _ in 0..(trailing_backslashes * 2) {
        encoded.push('\\');
    }
    encoded
}

fn rewrite_windows_cargo_manifest_dir_env(
    args: &mut [String],
    env: &mut [(OsString, OsString)],
    action_execroot: Option<&slug_fs::paths::abs_norm_path::AbsNormPath>,
) -> bool {
    #[cfg(not(windows))]
    {
        let _ = (args, env, action_execroot);
        false
    }

    #[cfg(windows)]
    {
        use std::hash::Hash;
        use std::hash::Hasher;

        let Some(execroot) = action_execroot else {
            return false;
        };
        if !is_cargo_build_script_runner_invocation(args) {
            return false;
        }

        let mut changed = rewrite_windows_cargo_tool_env_paths(env, execroot.as_path());

        let Some(manifest_index) = env
            .iter()
            .rposition(|(key, _)| key.as_os_str() == OsStr::new("CARGO_MANIFEST_DIR"))
        else {
            return changed;
        };

        let manifest_string = env[manifest_index].1.to_string_lossy();
        let manifest_abs = execroot.as_path().join(manifest_string.as_ref());
        if manifest_abs.as_os_str().len() <= 240 {
            return changed;
        }
        if let Some(repo) = cargo_runfiles_repo_name(manifest_string.as_ref()) {
            let manifest_rel = PathBuf::from("external").join(repo);
            let manifest_abs = execroot.as_path().join(&manifest_rel);
            if manifest_abs.is_dir() && manifest_abs.as_os_str().len() <= 240 {
                apply_windows_cargo_manifest_dir_rewrite(args, env, manifest_rel.into_os_string());
                return true;
            }
        }
        let manifest_sources = cargo_manifest_alias_source_candidates(
            manifest_string.as_ref(),
            execroot.as_path(),
            slug_core::cells::get_dynamic_project_root().as_deref(),
        );

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        manifest_string.hash(&mut hasher);
        let alias_rel = PathBuf::from(".slug-cargo-manifest")
            .join(format!("{:016x}", hasher.finish())[..8].to_owned());
        let alias_abs = execroot.as_path().join(&alias_rel);
        let Some(parent) = alias_abs.parent() else {
            return false;
        };
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::debug!(
                ?e,
                path = %parent.display(),
                "failed to create short cargo manifest alias parent"
            );
            return false;
        }
        match alias_abs.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                let _ = std::fs::remove_file(&alias_abs);
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return false,
            Err(_) => {}
        }
        if cargo_manifest_alias_needs_population(&alias_abs) {
            if let Err(e) = copy_first_manifest_alias_source(&manifest_sources, &alias_abs) {
                tracing::warn!(
                    ?e,
                    alias = %alias_abs.display(),
                    target = %manifest_abs.display(),
                    "failed to prepopulate short cargo manifest alias; using empty alias"
                );
                if let Err(e) = std::fs::create_dir_all(&alias_abs) {
                    tracing::debug!(
                        ?e,
                        alias = %alias_abs.display(),
                        "failed to create empty short cargo manifest alias"
                    );
                    return changed;
                }
            }
        }

        apply_windows_cargo_manifest_dir_rewrite(args, env, alias_rel.into_os_string());
        changed = true;
        changed
    }
}

#[cfg(windows)]
fn rewrite_windows_cargo_tool_env_paths(env: &mut [(OsString, OsString)], execroot: &Path) -> bool {
    let mut changed = false;
    for key in ["CARGO", "RUSTC", "RUSTDOC"] {
        for (_, value) in env
            .iter_mut()
            .filter(|(env_key, _)| env_key.as_os_str() == OsStr::new(key))
        {
            let value_path = Path::new(value);
            let absolute = if value_path.is_absolute() {
                value_path.to_path_buf()
            } else {
                execroot.join(value_path)
            };
            if !absolute.exists() {
                continue;
            }
            let replacement = windows_short_path(&absolute).unwrap_or(absolute);
            if replacement.as_os_str() == value.as_os_str() {
                continue;
            }
            *value = replacement.into_os_string();
            changed = true;
        }
    }
    changed
}

#[cfg(windows)]
fn apply_windows_cargo_manifest_dir_rewrite(
    args: &mut [String],
    env: &mut [(OsString, OsString)],
    manifest_rel: OsString,
) {
    for (key, value) in env.iter_mut() {
        if key.as_os_str() == OsStr::new("CARGO_MANIFEST_DIR") {
            *value = manifest_rel.clone();
        } else if key.as_os_str() == OsStr::new("RULES_RUST_SYMLINK_EXEC_ROOT") {
            *value = OsString::from("0");
        }
    }
    if let Some(rundir_arg) = args.iter_mut().find(|arg| arg.as_str() == "--rundir=") {
        *rundir_arg = format!("--rundir={}", Path::new(&manifest_rel).to_string_lossy());
    }
}

fn rewrite_windows_cargo_build_script_runner_args(
    args: &mut [String],
    action_execroot: Option<&slug_fs::paths::abs_norm_path::AbsNormPath>,
) -> bool {
    #[cfg(not(windows))]
    {
        let _ = (args, action_execroot);
        false
    }

    #[cfg(windows)]
    {
        use std::hash::Hash;
        use std::hash::Hasher;

        let Some(execroot) = action_execroot else {
            return false;
        };
        if !is_cargo_build_script_runner_invocation(args) {
            return false;
        }

        let Some(script_arg) = args.iter_mut().find(|arg| arg.starts_with("--script=")) else {
            return false;
        };
        let script = script_arg.trim_start_matches("--script=");
        let script = script.to_owned();
        let script_abs = execroot.as_path().join(&script);
        if script_abs.as_os_str().len() <= 240 {
            return false;
        }

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        script.hash(&mut hasher);
        let alias_rel =
            PathBuf::from(".slug-cargo-script").join(format!("{:016x}.exe", hasher.finish()));
        let alias_abs = execroot.as_path().join(&alias_rel);
        let Some(parent) = alias_abs.parent() else {
            return false;
        };
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::debug!(
                ?e,
                path = %parent.display(),
                "failed to create short cargo build-script alias parent"
            );
            return false;
        }
        if !alias_abs.exists() {
            if let Err(e) = std::fs::copy(&script_abs, &alias_abs) {
                tracing::debug!(
                    ?e,
                    alias = %alias_abs.display(),
                    target = %script_abs.display(),
                    "failed to create short cargo build-script alias"
                );
                return false;
            }
        }

        *script_arg = format!("--script={}", alias_rel.to_string_lossy());
        rewrite_windows_cargo_manifest_args_sources(
            args,
            execroot.as_path(),
            &[(PathBuf::from(script), alias_rel)],
        );
        true
    }
}

#[cfg(windows)]
fn rewrite_windows_cargo_manifest_args_sources(
    args: &mut [String],
    execroot: &Path,
    replacements: &[(PathBuf, PathBuf)],
) -> bool {
    rewrite_windows_cargo_manifest_args_file(args, Some(execroot), |lines| {
        use std::hash::Hash;
        use std::hash::Hasher;

        let mut changed = false;
        for line in lines.iter_mut().skip(2) {
            let quoted = line.starts_with('\'') && line.ends_with('\'') && line.len() >= 2;
            let body = if quoted {
                &line[1..line.len() - 1]
            } else {
                line.as_str()
            };
            let Some((src, dest)) = body.split_once('=') else {
                continue;
            };
            let replacement = replacements
                .iter()
                .find(|(original, _)| Path::new(src) == original.as_path())
                .map(|(_, replacement)| replacement.clone())
                .or_else(|| {
                    let src_path = Path::new(src);
                    if src_path.is_absolute() {
                        return None;
                    }
                    let src_abs = execroot.join(src_path);
                    if src_abs.as_os_str().len() <= 240 || !src_abs.is_file() {
                        return None;
                    }
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    src.hash(&mut hasher);
                    let alias_rel = PathBuf::from(".slug-cargo-runfile-src")
                        .join(format!("{:016x}", hasher.finish()))
                        .join(src_path.file_name()?);
                    let alias_abs = execroot.join(&alias_rel);
                    if let Some(parent) = alias_abs.parent() {
                        if std::fs::create_dir_all(parent).is_err() {
                            return None;
                        }
                    }
                    if !alias_abs.exists() && std::fs::copy(&src_abs, &alias_abs).is_err() {
                        return None;
                    }
                    Some(alias_rel)
                });
            let Some(replacement) = replacement else {
                continue;
            };
            let rewritten = format!("{}={dest}", replacement.to_string_lossy());
            *line = if quoted {
                format!("'{rewritten}'")
            } else {
                rewritten
            };
            changed = true;
        }
        changed
    })
}

#[cfg(windows)]
fn rewrite_windows_cargo_manifest_args_file<F>(
    args: &mut [String],
    execroot: Option<&Path>,
    rewrite: F,
) -> bool
where
    F: FnOnce(&mut Vec<String>) -> bool,
{
    let Some(arg) = args
        .iter()
        .find(|arg| arg.starts_with("--cargo_manifest_args=@"))
    else {
        return false;
    };
    let param_path = arg.trim_start_matches("--cargo_manifest_args=@");
    let param_path = Path::new(param_path);
    let param_path = if param_path.is_absolute() {
        param_path.to_path_buf()
    } else if let Some(execroot) = execroot {
        execroot.join(param_path)
    } else {
        param_path.to_path_buf()
    };
    let Ok(content) = std::fs::read_to_string(&param_path) else {
        return false;
    };
    let mut lines: Vec<String> = content.lines().map(str::to_owned).collect();
    if !rewrite(&mut lines) {
        return false;
    }
    match std::fs::write(&param_path, lines.join("\n")) {
        Ok(()) => true,
        Err(e) => {
            tracing::debug!(
                ?e,
                path = %param_path.display(),
                "failed to rewrite cargo manifest args file"
            );
            false
        }
    }
}

#[cfg(windows)]
fn cargo_manifest_alias_source_candidates(
    manifest: &str,
    execroot: &Path,
    project_root: Option<&Path>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    let Some(project_root) = project_root else {
        push_unique_path(&mut candidates, execroot.join(manifest));
        return candidates;
    };

    if let Some((repo, repo_relative_path)) = cargo_runfiles_repo_and_path(manifest) {
        if repo == "_main" {
            if !repo_relative_path.as_os_str().is_empty() {
                push_unique_path(&mut candidates, project_root.join(&repo_relative_path));
            }
            push_unique_path(&mut candidates, project_root.to_path_buf());
        } else {
            push_unique_path(&mut candidates, execroot.join("external").join(&repo));
            push_unique_path(&mut candidates, project_root.join("external").join(&repo));
            push_unique_path(
                &mut candidates,
                project_root.join("bazel-external").join(&repo),
            );
        }
    } else if let Some(repo) = manifest_repo_path_component(manifest, "external") {
        push_unique_path(&mut candidates, execroot.join("external").join(repo));
        push_unique_path(&mut candidates, project_root.join("external").join(repo));
    } else if let Some(repo) = manifest_repo_path_component(manifest, "bazel-external") {
        push_unique_path(
            &mut candidates,
            project_root.join("bazel-external").join(repo),
        );
    }
    push_unique_path(&mut candidates, execroot.join(manifest));

    candidates
}

#[cfg(windows)]
fn cargo_runfiles_repo_name(manifest: &str) -> Option<String> {
    cargo_runfiles_repo_and_path(manifest).map(|(repo, _)| repo)
}

#[cfg(windows)]
fn cargo_runfiles_repo_and_path(manifest: &str) -> Option<(String, PathBuf)> {
    let normalized = manifest.replace('\\', "/");
    let marker = ".cargo_runfiles/";
    let repo_start = normalized.rfind(marker)? + marker.len();
    let repo = normalized[repo_start..].split('/').next()?;
    if repo.is_empty() {
        return None;
    }
    let rest_start = repo_start + repo.len();
    let rest = normalized[rest_start..].trim_start_matches('/');
    if rest.is_empty() {
        Some((repo.to_owned(), PathBuf::new()))
    } else {
        Some((repo.to_owned(), PathBuf::from(rest)))
    }
}

#[cfg(windows)]
fn manifest_repo_path_component<'a>(manifest: &'a str, prefix: &str) -> Option<&'a str> {
    let mut components = manifest.split(['/', '\\']);
    if components.next()? != prefix {
        return None;
    }
    let repo = components.next()?;
    if repo.is_empty() { None } else { Some(repo) }
}

#[cfg(windows)]
fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

#[cfg(windows)]
fn cargo_manifest_alias_needs_population(alias: &Path) -> bool {
    match std::fs::read_dir(alias) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => true,
    }
}

#[cfg(windows)]
fn copy_first_manifest_alias_source(sources: &[PathBuf], alias: &Path) -> std::io::Result<()> {
    let mut last_error = None;
    for source in sources {
        if !source.exists() {
            continue;
        }
        match copy_dir_all_for_manifest_alias(source, alias) {
            Ok(()) => return Ok(()),
            Err(e) => {
                tracing::debug!(
                    ?e,
                    alias = %alias.display(),
                    source = %source.display(),
                    "failed to prepopulate short cargo manifest alias from candidate"
                );
                last_error = Some(e);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no cargo manifest alias source exists",
        )
    }))
}

#[cfg(windows)]
fn copy_dir_all_for_manifest_alias(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all_for_manifest_alias(&entry.path(), &dst_path)?;
        } else if ty.is_symlink() {
            let target = std::fs::read_link(entry.path())?;
            #[cfg(windows)]
            {
                if entry.path().is_dir() {
                    std::os::windows::fs::symlink_dir(target, dst_path)?;
                } else {
                    std::os::windows::fs::symlink_file(target, dst_path)?;
                }
            }
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

fn is_cargo_build_script_runner_invocation(args: &[String]) -> bool {
    args.first()
        .and_then(|arg| path_file_name(arg))
        .is_some_and(|name| name == "runner" || name == "runner.exe")
        && args.iter().any(|arg| arg.starts_with("--script="))
}

fn is_process_wrapper_invocation(args: &[String]) -> bool {
    args.first()
        .and_then(|arg| path_file_name(arg))
        .is_some_and(|name| name == "process_wrapper" || name == "process_wrapper.exe")
}

fn is_process_wrapper_rustc_invocation(args: &[String]) -> bool {
    let Some(separator) = args.iter().position(|arg| arg == "--") else {
        return false;
    };
    args.get(separator + 1)
        .is_some_and(|tool| path_file_name(tool).is_some_and(is_rustc_file_name))
}

fn path_file_name(path: &str) -> Option<&str> {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
}

fn is_rustc_file_name(name: &str) -> bool {
    name == "rustc" || name == "rustc.exe"
}

#[cfg(unix)]
fn should_filter_rustc_implicit_gcc_s(args: &[String], linker: &str) -> bool {
    let linker_is_clangxx = linker.ends_with("/clang++") || linker == "clang++";
    linker_is_clangxx
        && args
            .iter()
            .any(|arg| arg == "--codegen=link-arg=-rtlib=compiler-rt")
        && args
            .iter()
            .any(|arg| arg == "--codegen=link-arg=-nostdlib++")
        && args
            .iter()
            .any(|arg| arg == "--codegen=link-arg=--unwindlib=none")
}

#[cfg(unix)]
fn write_rustc_linker_filter_wrapper(wrapper: &Path, linker: &str) -> slug_error::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let quoted_linker = shell_single_quote(linker);
    let script = format!(
        "#!/usr/bin/env bash\n\
         set -euo pipefail\n\
         args=()\n\
         for arg in \"$@\"; do\n\
           if [ \"$arg\" = \"-lgcc_s\" ]; then\n\
             continue\n\
           fi\n\
           args+=(\"$arg\")\n\
         done\n\
         exec {quoted_linker} \"${{args[@]}}\"\n"
    );
    std::fs::write(wrapper, script)?;
    let mut permissions = std::fs::metadata(wrapper)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(wrapper, permissions)?;
    Ok(())
}

#[cfg(unix)]
fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

/// A scratch path discovered during `materialize_inputs`.
pub struct ScratchPath(Option<ProjectRelativePathBuf>);

pub async fn prep_scratch_path(
    scratch_path: &ScratchPath,
    artifact_fs: &ArtifactFs,
) -> slug_error::Result<()> {
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
) -> slug_error::Result<Vec<ProjectRelativePathBuf>> {
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

fn content_based_configuration_symlink(
    project_fs: &ProjectRoot,
    digest_config: DigestConfig,
    artifact_value: &ArtifactValue,
    configuration_hash_path: &ProjectRelativePathBuf,
    content_based_path: ProjectRelativePathBuf,
) -> slug_error::Result<Option<(ProjectRelativePathBuf, ArtifactValue)>> {
    if content_based_path == *configuration_hash_path {
        return Ok(None);
    }

    let mut builder = ArtifactValueBuilder::new(project_fs, digest_config);
    builder.add_symlinked(
        artifact_value,
        content_based_path,
        configuration_hash_path.as_ref(),
    )?;
    let symlink_value = builder.build(configuration_hash_path.as_ref())?;
    Ok(Some((configuration_hash_path.clone(), symlink_value)))
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
) -> slug_error::Result<()> {
    let outputs: Vec<_> = request
        .outputs()
        .map(|output| {
            output.resolve(
                artifact_fs,
                Some(&ContentBasedPathHash::for_output_artifact()),
            )
        })
        .collect::<slug_error::Result<Vec<_>>>()?;

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

fn sync_outputs_from_action_execroot(
    execroot: &Path,
    project_root: &Path,
    request: &CommandExecutionRequest,
    artifact_fs: &ArtifactFs,
) -> slug_error::Result<()> {
    for output in request.outputs() {
        let path = output
            .resolve(
                artifact_fs,
                Some(&ContentBasedPathHash::for_output_artifact()),
            )?
            .into_path();
        sync_output_from_action_execroot_path(execroot, project_root, &path)?;
    }
    Ok(())
}

fn sync_output_from_action_execroot_path(
    execroot: &Path,
    project_root: &Path,
    path: &ProjectRelativePath,
) -> slug_error::Result<()> {
    let src = execroot.join(path.as_str());
    if src.symlink_metadata().is_err() {
        return Ok(());
    }
    let dst = project_root.join(path.as_str());
    if symlink_points_to_path(&src, &dst)? {
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .with_buck_error_context(|| format!("creating parent for staged output {path}"))?;
    }
    remove_existing_path(&dst)
        .with_buck_error_context(|| format!("removing previous output {path}"))?;
    std::fs::rename(&src, &dst)
        .with_buck_error_context(|| format!("moving staged output {path} into buck-out"))?;
    Ok(())
}

fn symlink_points_to_path(link: &Path, path: &Path) -> slug_error::Result<bool> {
    let metadata = link.symlink_metadata();
    let Ok(metadata) = metadata else {
        return Ok(false);
    };
    if !metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let target = std::fs::read_link(link)
        .with_buck_error_context(|| format!("reading symlink {}", link.display()))?;
    let target = if target.is_absolute() {
        target
    } else {
        link.parent().unwrap_or_else(|| Path::new("")).join(target)
    };
    Ok(target == path)
}

fn remove_existing_path(path: &Path) -> std::io::Result<()> {
    match path.symlink_metadata() {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            std::fs::remove_dir_all(path)
        }
        Ok(_) => std::fs::remove_file(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
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
        forkserver: &slug_forkserver::client::ForkserverClient,
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
    ) -> slug_error::Result<CommandResult> {
        let exe = exe.as_ref();

        let mut req = slug_forkserver_proto::CommandRequest {
            exe: exe.as_bytes().to_vec(),
            argv: args
                .into_iter()
                .map(|s| s.as_ref().as_bytes().to_vec())
                .collect(),
            cwd: Some(slug_forkserver_proto::WorkingDirectory {
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
            D: Into<slug_forkserver_proto::env_directive::Data>;
    }

    impl CommandRequestExt for slug_forkserver_proto::CommandRequest {
        fn push_env_directive<D>(&mut self, directive: D)
        where
            D: Into<slug_forkserver_proto::env_directive::Data>,
        {
            self.env.push(slug_forkserver_proto::EnvDirective {
                data: Some(directive.into()),
            });
        }
    }

    impl EnvironmentBuilder for slug_forkserver_proto::CommandRequest {
        fn clear(&mut self) {
            self.push_env_directive(slug_forkserver_proto::EnvClear {});
        }

        fn set<K, V>(&mut self, key: K, val: V)
        where
            K: AsRef<OsStr>,
            V: AsRef<OsStr>,
        {
            self.push_env_directive(slug_forkserver_proto::EnvSet {
                key: key.as_ref().as_bytes().to_vec(),
                value: val.as_ref().as_bytes().to_vec(),
            })
        }

        fn remove<K>(&mut self, key: K)
        where
            K: AsRef<OsStr>,
        {
            self.push_env_directive(slug_forkserver_proto::EnvRemove {
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
    use slug_common::file_ops::metadata::FileMetadata;
    use slug_common::liveliness_observer::NoopLivelinessObserver;
    use slug_core::cells::CellResolver;
    use slug_core::cells::cell_root_path::CellRootPathBuf;
    use slug_core::cells::name::CellName;
    use slug_core::fs::buck_out_path::BuckOutPathResolver;
    use slug_core::fs::project::ProjectRoot;
    use slug_core::fs::project::ProjectRootTemp;
    use slug_execute::execute::blocking::testing::DummyBlockingExecutor;
    use slug_execute::materialize::nodisk::NoDiskMaterializer;

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

    fn test_executor() -> slug_error::Result<(LocalExecutor, AbsNormPathBuf, ProjectRootTemp)> {
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

    #[test]
    fn content_based_configuration_symlink_skips_identity_alias() -> slug_error::Result<()> {
        let temp = ProjectRootTemp::new().unwrap();
        let digest_config = DigestConfig::testing_default();
        let path = ProjectRelativePathBuf::unchecked_new(
            "buck-out/plan61/gen/repo/hash/external/repo/bin/rustc".to_owned(),
        );
        let value = ArtifactValue::file(FileMetadata::empty(digest_config.cas_digest_config()));

        assert!(
            content_based_configuration_symlink(
                temp.path(),
                digest_config,
                &value,
                &path,
                path.clone(),
            )?
            .is_none()
        );

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn sync_outputs_skips_execroot_alias_to_project_output() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let project_root = temp.path().join("project");
        let execroot = temp.path().join("execroot");
        let output = ProjectRelativePathBuf::unchecked_new(
            "buck-out/plan61/gen/repo/hash/external/repo/bin/rustc".to_owned(),
        );
        let dst = project_root.join(output.as_str());
        let src = execroot.join(output.as_str());
        let real_rustc = project_root.join("external/repo/bin/rustc");
        std::fs::create_dir_all(real_rustc.parent().unwrap())?;
        std::fs::write(&real_rustc, b"rustc")?;
        std::fs::create_dir_all(dst.parent().unwrap())?;
        std::os::unix::fs::symlink(&real_rustc, &dst)?;
        std::fs::create_dir_all(src.parent().unwrap())?;
        std::os::unix::fs::symlink(&dst, &src)?;

        sync_output_from_action_execroot_path(&execroot, &project_root, &output)?;

        assert_eq!(std::fs::read_link(&dst)?, real_rustc);
        assert_eq!(std::fs::read_link(&src)?, dst);

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_rustc_compiler_rt_linker_filter_rewrites_clangxx() -> slug_error::Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir()?;
        let fake_linker = temp.path().join("clang++");
        let fake_linker_args = temp.path().join("fake-linker-args");
        std::fs::write(
            &fake_linker,
            format!(
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > {}\n",
                shell_single_quote(&fake_linker_args.to_string_lossy())
            ),
        )?;
        let mut permissions = std::fs::metadata(&fake_linker)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_linker, permissions)?;

        let mut args = vec![
            "lib.rs".to_owned(),
            format!("--codegen=linker={}", fake_linker.display()),
            "--codegen=link-arg=-rtlib=compiler-rt".to_owned(),
            "--codegen=link-arg=-nostdlib++".to_owned(),
            "--codegen=link-arg=--unwindlib=none".to_owned(),
        ];

        rewrite_rustc_llvm_linker_for_compiler_rt(&mut args, temp.path())?;

        let linker = args
            .iter()
            .find_map(|arg| arg.strip_prefix("--codegen=linker="))
            .expect("rewritten linker");
        assert!(linker.ends_with("slug-rustc-clangxx-filter-gcc-s"));
        let script = std::fs::read_to_string(linker)?;
        assert!(script.contains(r#"[ "$arg" = "-lgcc_s" ]"#));
        assert!(script.contains(fake_linker.to_string_lossy().as_ref()));

        let status = std::process::Command::new(linker)
            .args(["-lc++", "-lgcc_s", "-lm"])
            .status()?;
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(fake_linker_args)?, "-lc++\n-lm\n");

        Ok(())
    }

    #[test]
    fn test_rustc_compiler_rt_linker_filter_requires_compiler_rt_shape() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let mut args = vec![
            "lib.rs".to_owned(),
            "--codegen=linker=external/llvm++http_archive+llvm-toolchain-minimal/bin/clang++"
                .to_owned(),
            "--codegen=link-arg=-rtlib=libgcc".to_owned(),
        ];

        rewrite_rustc_llvm_linker_for_compiler_rt(&mut args, temp.path())?;

        assert_eq!(
            args[1],
            "--codegen=linker=external/llvm++http_archive+llvm-toolchain-minimal/bin/clang++"
        );
        assert!(!temp.path().join("slug-rustc-clangxx-filter-gcc-s").exists());

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_rustc_compiler_rt_linker_filter_rewrites_inline_argv() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let args = vec![
            "rustc".to_owned(),
            "build.rs".to_owned(),
            "--codegen=linker=clang++".to_owned(),
            "--codegen=link-arg=-rtlib=compiler-rt".to_owned(),
            "--codegen=link-arg=-nostdlib++".to_owned(),
            "--codegen=link-arg=--unwindlib=none".to_owned(),
        ];

        let rewritten = rewrite_inline_rustc_llvm_linker_for_compiler_rt(&args, temp.path())?
            .expect("inline rustc argv should be rewritten");

        assert_eq!(args[2], "--codegen=linker=clang++");
        assert_ne!(rewritten[2], args[2]);
        assert!(rewritten[2].ends_with("slug-rustc-clangxx-filter-gcc-s"));
        assert!(temp.path().join("slug-rustc-clangxx-filter-gcc-s").exists());

        Ok(())
    }

    #[test]
    fn test_rustc_execroot_remap_precedes_generic_pwd_remap() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let execroot = temp.path().join("execroot").join("7476792f9006565e");
        std::fs::create_dir_all(&execroot)?;
        let execroot = AbsNormPathBuf::new(execroot)?;
        let mut args = vec![
            "external/rules_rust/util/process_wrapper/process_wrapper".to_owned(),
            "--subst".to_owned(),
            "pwd=${pwd}".to_owned(),
            "--".to_owned(),
            "external/rust_linux_x86_64/bin/rustc".to_owned(),
            "external/rules_rs++crate+crates__anyhow-1.0.102/src/lib.rs".to_owned(),
            "--remap-path-prefix=${pwd}=.".to_owned(),
        ];

        assert!(
            !add_rustc_execroot_remap(&mut args, Some(&execroot)),
            "generic process_wrapper pwd remap is rewritten to a stable cwd alias"
        );
        assert!(!args.iter().any(|arg| arg.contains("7476792f9006565e")));

        Ok(())
    }

    #[test]
    fn test_process_wrapper_execroot_substitutions_use_stable_cwd_and_output_base()
    -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let output_base = AbsNormPathBuf::new(temp.path().to_path_buf())?;
        let execroot = output_base
            .as_path()
            .join("execroot")
            .join("7476792f9006565e");
        std::fs::create_dir_all(&execroot)?;
        let execroot = AbsNormPathBuf::new(execroot)?;
        let mut args = vec![
            "external/rules_rust/util/process_wrapper/process_wrapper".to_owned(),
            "--subst".to_owned(),
            "pwd=${pwd}".to_owned(),
            "--subst".to_owned(),
            "exec_root=${exec_root}".to_owned(),
            "--subst".to_owned(),
            "output_base=${output_base}".to_owned(),
            "--".to_owned(),
            "external/rust_linux_x86_64/bin/rustc".to_owned(),
            "src/lib.rs".to_owned(),
        ];

        assert!(rewrite_process_wrapper_execroot_substitutions(
            &mut args,
            &execroot,
            &output_base,
        ));

        #[cfg(unix)]
        {
            let stable_execroot = slug_core::cells::execroot_path(output_base.as_path())
                .expect("test output base has a basename");
            assert!(args.contains(&format!("pwd={}", execroot.as_path().to_string_lossy())));
            assert!(args.contains(&format!("exec_root={}", stable_execroot.to_string_lossy())));
            assert!(args.contains(&format!(
                "output_base={}",
                execroot.as_path().to_string_lossy()
            )));
        }
        #[cfg(not(unix))]
        {
            assert!(args.contains(&format!("pwd={}", execroot.as_path().to_string_lossy())));
            assert!(args.contains(&format!(
                "exec_root={}",
                execroot.as_path().to_string_lossy()
            )));
            assert!(args.contains(&format!(
                "output_base={}",
                output_base.as_path().to_string_lossy()
            )));
        }
        assert!(!args.contains(&"pwd=${pwd}".to_owned()));
        assert!(!args.contains(&"exec_root=${exec_root}".to_owned()));
        assert!(!args.contains(&"output_base=${output_base}".to_owned()));

        Ok(())
    }

    #[test]
    fn test_process_wrapper_execroot_substitutions_support_equals_flag() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let output_base = AbsNormPathBuf::new(temp.path().to_path_buf())?;
        let execroot = output_base
            .as_path()
            .join("execroot")
            .join("7476792f9006565e");
        std::fs::create_dir_all(&execroot)?;
        let execroot = AbsNormPathBuf::new(execroot)?;
        let mut args = vec![
            "external/rules_rust/util/process_wrapper/process_wrapper".to_owned(),
            "--subst=exec_root=${exec_root}".to_owned(),
            "--".to_owned(),
            "external/rust_linux_x86_64/bin/rustc".to_owned(),
        ];

        assert!(rewrite_process_wrapper_execroot_substitutions(
            &mut args,
            &execroot,
            &output_base,
        ));

        #[cfg(unix)]
        {
            let stable_execroot = slug_core::cells::execroot_path(output_base.as_path())
                .expect("test output base has a basename");
            assert_eq!(
                args[1],
                format!("--subst=exec_root={}", stable_execroot.to_string_lossy())
            );
        }
        #[cfg(not(unix))]
        assert_eq!(
            args[1],
            format!("--subst=exec_root={}", execroot.as_path().to_string_lossy())
        );

        Ok(())
    }

    #[test]
    fn test_rustc_flags_execroot_remap_precedes_paramfile_remap() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let execroot = temp.path().join("execroot").join("7476792f9006565e");
        std::fs::create_dir_all(&execroot)?;
        let execroot = AbsNormPathBuf::new(execroot)?;
        let mut args = vec![
            "external/rules_rs++crate+crates__anyhow-1.0.102/src/lib.rs".to_owned(),
            "--remap-path-prefix=${pwd}=.".to_owned(),
        ];

        assert!(!add_rustc_flags_execroot_remap(&mut args, Some(&execroot)));
        assert_eq!(args.len(), 2);

        Ok(())
    }

    #[test]
    fn test_rustc_flags_execroot_remap_uses_action_execroot() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let execroot = temp.path().join("execroot").join("7476792f9006565e");
        std::fs::create_dir_all(&execroot)?;
        let execroot = AbsNormPathBuf::new(execroot)?;
        let mut args =
            vec!["external/rules_rs++crate+crates__anyhow-1.0.102/src/lib.rs".to_owned()];

        assert!(add_rustc_flags_execroot_remap(&mut args, Some(&execroot)));

        let expected = format!(
            "--remap-path-prefix={}={}",
            execroot.as_path().to_string_lossy(),
            "."
        );
        assert!(args.contains(&expected));

        Ok(())
    }

    #[test]
    fn test_rustc_paramfile_remap_ignores_cargo_manifest_paramfile() {
        let args = vec![
            "external/rules_rust/cargo/private/cargo_build_script_runner/runner".to_owned(),
            "--script=buck-out/gen/external/crate/_bs-.exe".to_owned(),
            "--cargo_manifest_args=@buck-out/tmp/slug-params-0".to_owned(),
        ];

        assert!(!should_add_rustc_paramfile_execroot_remap(&args));
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_process_wrapper_rustc_tail_paramfile_shortens_spawn() -> slug_error::Result<()>
    {
        let temp = tempfile::tempdir()?;
        let mut args = vec![
            "external/rules_rust/util/process_wrapper/process_wrapper.exe".to_owned(),
            "--subst".to_owned(),
            "pwd=${pwd}".to_owned(),
            "--".to_owned(),
            "external/rustc/bin/rustc.exe".to_owned(),
            "src/lib.rs".to_owned(),
        ];
        for idx in 0..900 {
            args.push(format!(
                "-Ldependency=buck-out/p44/gen/rules_rs++crate+crates__dep-{idx}/416f1912d74383a3/external/rules_rs++crate+crates__dep-{idx}"
            ));
        }

        assert!(windows_command_line_len(&args) > 30_000);
        assert!(parametrize_windows_process_wrapper_rustc_tail(
            &mut args,
            temp.path()
        ));

        assert_eq!(args.len(), 6);
        assert_eq!(args[4], "external/rustc/bin/rustc.exe");
        assert!(args[5].starts_with('@'));
        let param_path = args[5].trim_start_matches('@');
        let content = std::fs::read_to_string(param_path)?;
        assert!(content.contains("src/lib.rs"));
        assert!(content.contains("-Ldependency=buck-out/p44/gen/rules_rs++crate+crates__dep-899"));

        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_msvc_rust_lld_gets_dynamic_crt_link_args() {
        let mut args = vec![
            "external/rules_rust/util/process_wrapper/process_wrapper.exe".to_owned(),
            "--".to_owned(),
            "external/rustc/bin/rustc.exe".to_owned(),
            "src/lib.rs".to_owned(),
            "--sysroot=buck-out/gen/toolchains/windows_x86_64_rust_toolchain".to_owned(),
            "-Clinker=rust-lld.exe".to_owned(),
            "-Lnative=buck-out/gen/lib/rustlib/x86_64-pc-windows-msvc/lib".to_owned(),
        ];

        assert!(add_windows_msvc_rust_lld_crt_link_args(&mut args));
        assert!(args.contains(&"--codegen=link-arg=/nodefaultlib:libucrt".to_owned()));
        assert!(args.contains(&"--codegen=link-arg=ucrt.lib".to_owned()));
        assert!(args.contains(&"--codegen=link-arg=oldnames.lib".to_owned()));
        assert!(!args.contains(&"--codegen=link-arg=vcruntime.lib".to_owned()));
        assert!(!args.contains(&"--codegen=link-arg=msvcrt.lib".to_owned()));
        let len = args.len();
        assert!(!add_windows_msvc_rust_lld_crt_link_args(&mut args));
        assert_eq!(len, args.len());
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_msvc_rust_lld_paramfile_gets_dynamic_crt_link_args() {
        let mut args = vec![
            "--target=x86_64-pc-windows-msvc".to_owned(),
            "-Lbuck-out/gen/toolchains/windows_x86_64_rust_toolchain/lib/rustlib/x86_64-pc-windows-msvc/lib".to_owned(),
            "-Clinker=rust-lld.exe".to_owned(),
        ];

        assert!(add_windows_msvc_rust_lld_crt_args(&mut args));
        assert!(args.contains(&"--codegen=link-arg=/nodefaultlib:libucrt".to_owned()));
        assert!(args.contains(&"--codegen=link-arg=ucrt.lib".to_owned()));
        assert!(args.contains(&"--codegen=link-arg=oldnames.lib".to_owned()));
        assert!(!args.contains(&"--codegen=link-arg=vcruntime.lib".to_owned()));
        assert!(!args.contains(&"--codegen=link-arg=msvcrt.lib".to_owned()));
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_process_wrapper_child_tool_path_uses_execroot_absolute_path()
    -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let execroot = AbsNormPathBuf::new(temp.path().join("execroot"))?;
        let tool_rel = PathBuf::from("buck-out")
            .join("iso")
            .join("gen")
            .join("rules_rs++toolchains+default_rust_toolchains")
            .join("cfg")
            .join("external")
            .join("rules_rs++toolchains+default_rust_toolchains")
            .join("windows_x86_64_rust_toolchain")
            .join("bin")
            .join("rustc.exe");
        let tool_abs = execroot.as_path().join(&tool_rel);
        std::fs::create_dir_all(tool_abs.parent().unwrap())?;
        std::fs::write(&tool_abs, b"")?;

        let mut args = vec![
            "external/rules_rust/util/process_wrapper/process_wrapper.exe".to_owned(),
            "--".to_owned(),
            tool_rel.to_string_lossy().into_owned(),
            "src/lib.rs".to_owned(),
        ];

        assert!(rewrite_windows_process_wrapper_child_tool_path(
            &mut args,
            Some(&execroot),
            execroot.as_path()
        ));
        assert!(Path::new(&args[2]).is_absolute());
        assert!(Path::new(&args[2]).exists());
        assert!(args[2].ends_with("rustc.exe"));

        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_process_wrapper_paramfile_arg_doubles_trailing_backslashes() {
        assert_eq!(
            encode_process_wrapper_paramfile_arg("C:\\path\\"),
            "C:\\path\\\\"
        );
        assert_eq!(
            encode_process_wrapper_paramfile_arg("C:\\path\\\\"),
            "C:\\path\\\\\\\\"
        );
    }

    #[test]
    fn test_windows_cargo_manifest_dir_rewrite_ignores_non_runner() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let execroot = AbsNormPathBuf::new(temp.path().join("execroot"))?;
        std::fs::create_dir_all(&execroot)?;
        let mut env = vec![(
            OsString::from("CARGO_MANIFEST_DIR"),
            OsString::from("buck-out/very/long/generated/cargo_runfiles/crate"),
        )];
        let mut args = vec!["external/tools/not-runner.exe".to_owned()];

        assert!(!rewrite_windows_cargo_manifest_dir_env(
            &mut args,
            &mut env,
            Some(&execroot)
        ));
        assert_eq!(
            env[0].1,
            OsString::from("buck-out/very/long/generated/cargo_runfiles/crate")
        );

        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_cargo_manifest_dir_rewrite_shortens_runner_env() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let execroot = AbsNormPathBuf::new(temp.path().join("execroot"))?;
        std::fs::create_dir_all(&execroot)?;
        let manifest = format!(
            "buck-out/{}/gen/crate/_bs.cargo_runfiles/crate",
            "x".repeat(185),
        );
        std::fs::create_dir_all(execroot.as_path().join(&manifest))?;
        assert!(execroot.as_path().join(&manifest).as_os_str().len() > 240);
        let mut env = vec![
            (
                OsString::from("CARGO_MANIFEST_DIR"),
                OsString::from("stale"),
            ),
            (
                OsString::from("CARGO_MANIFEST_DIR"),
                OsString::from(&manifest),
            ),
            (
                OsString::from("RULES_RUST_SYMLINK_EXEC_ROOT"),
                OsString::from("1"),
            ),
        ];
        let mut args = vec![
            "buck-out/gen/rules_rust/cargo/private/cargo_build_script_runner/runner.exe".to_owned(),
            "--script=buck-out/gen/external/crate/_bs-.exe".to_owned(),
            "--rundir=".to_owned(),
        ];

        assert!(rewrite_windows_cargo_manifest_dir_env(
            &mut args,
            &mut env,
            Some(&execroot)
        ));
        assert!(
            env[1]
                .1
                .to_string_lossy()
                .starts_with(".slug-cargo-manifest")
        );
        assert_eq!(env[0].1, env[1].1);
        assert!(execroot.as_path().join(Path::new(&env[1].1)).is_dir());
        assert_eq!(
            args[2],
            format!("--rundir={}", Path::new(&env[1].1).to_string_lossy())
        );
        assert_eq!(env[2].1, OsString::from("0"));

        let first_alias = env[1].1.clone();
        args[2] = "--rundir=".to_owned();
        env[2].1 = OsString::from("1");
        env[0].1 = OsString::from("stale");
        env[1].1 = OsString::from(&manifest);
        assert!(rewrite_windows_cargo_manifest_dir_env(
            &mut args,
            &mut env,
            Some(&execroot)
        ));
        assert_eq!(env[0].1, first_alias);
        assert_eq!(env[1].1, first_alias);
        assert_eq!(
            args[2],
            format!("--rundir={}", Path::new(&env[1].1).to_string_lossy())
        );
        assert_eq!(env[2].1, OsString::from("0"));

        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_cargo_manifest_alias_sources_include_external_repo_fallback()
    -> slug_error::Result<()> {
        let execroot = PathBuf::from("C:\\s\\execroot");
        let project_root = PathBuf::from("C:\\dev\\workspace");
        let manifest = "buck-out/p44/gen/rules_rs++crate+crates__windows_x86_64_msvc-0.52.6/416f1912d74383a3/external/rules_rs++crate+crates__windows_x86_64_msvc-0.52.6/_bs_x86_64-pc-windows-msvc.cargo_runfiles/rules_rs++crate+crates__windows_x86_64_msvc-0.52.6";

        let candidates =
            cargo_manifest_alias_source_candidates(manifest, &execroot, Some(&project_root));

        assert_eq!(
            candidates[0],
            execroot
                .join("external")
                .join("rules_rs++crate+crates__windows_x86_64_msvc-0.52.6")
        );
        assert!(
            candidates.contains(
                &project_root
                    .join("external")
                    .join("rules_rs++crate+crates__windows_x86_64_msvc-0.52.6")
            )
        );
        assert!(
            candidates.contains(
                &project_root
                    .join("bazel-external")
                    .join("rules_rs++crate+crates__windows_x86_64_msvc-0.52.6")
            )
        );
        assert_eq!(candidates.last(), Some(&execroot.join(manifest)));

        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_cargo_manifest_alias_sources_include_main_workspace_package()
    -> slug_error::Result<()> {
        let execroot = PathBuf::from("C:\\s\\execroot");
        let project_root = PathBuf::from("C:\\dev\\workspace");
        let manifest = "buck-out/iso/gen/reactor/a08fa8f28613e62e/zerobuf_generated/component_animation_types/build_script.cargo_runfiles/_main/zerobuf_generated/component_animation_types";

        let candidates =
            cargo_manifest_alias_source_candidates(manifest, &execroot, Some(&project_root));

        assert_eq!(
            candidates[0],
            project_root.join("zerobuf_generated/component_animation_types")
        );
        assert!(candidates.contains(&project_root));
        assert_eq!(candidates.last(), Some(&execroot.join(manifest)));

        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_cargo_manifest_dir_rewrite_uses_short_external_repo() -> slug_error::Result<()>
    {
        let temp = tempfile::tempdir()?;
        let execroot = AbsNormPathBuf::new(temp.path().join("execroot"))?;
        let repo = "rules_rs++crate+crates__windows_x86_64_msvc-0.52.6";
        std::fs::create_dir_all(execroot.as_path().join("external").join(repo).join("lib"))?;
        std::fs::write(
            execroot
                .as_path()
                .join("external")
                .join(repo)
                .join("lib")
                .join("windows.0.52.0.lib"),
            b"native lib",
        )?;
        let manifest = format!(
            "buck-out/{}/gen/{repo}/_bs_x86_64-pc-windows-msvc.cargo_runfiles/{repo}",
            "x".repeat(185),
        );
        assert!(execroot.as_path().join(&manifest).as_os_str().len() > 240);
        let mut env = vec![
            (
                OsString::from("CARGO_MANIFEST_DIR"),
                OsString::from(&manifest),
            ),
            (
                OsString::from("RULES_RUST_SYMLINK_EXEC_ROOT"),
                OsString::from("1"),
            ),
        ];
        let mut args = vec![
            "buck-out/gen/rules_rust/cargo/private/cargo_build_script_runner/runner.exe".to_owned(),
            "--script=buck-out/gen/external/crate/_bs-.exe".to_owned(),
            "--rundir=".to_owned(),
        ];

        assert!(rewrite_windows_cargo_manifest_dir_env(
            &mut args,
            &mut env,
            Some(&execroot)
        ));
        assert_eq!(
            env[0].1,
            OsString::from(PathBuf::from("external").join(repo))
        );
        assert_eq!(env[1].1, OsString::from("0"));
        assert_eq!(args[2], format!("--rundir=external\\{repo}"));

        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_cargo_manifest_alias_copies_first_existing_fallback() -> slug_error::Result<()>
    {
        let temp = tempfile::tempdir()?;
        let missing = temp.path().join("missing");
        let source = temp.path().join("external").join("crate");
        let alias = temp.path().join("alias");
        std::fs::create_dir_all(source.join("lib"))?;
        std::fs::write(source.join("lib").join("windows.0.52.0.lib"), b"native lib")?;

        copy_first_manifest_alias_source(&[missing, source], &alias)?;

        assert_eq!(
            std::fs::read(alias.join("lib").join("windows.0.52.0.lib"))?,
            b"native lib"
        );

        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_cargo_tool_env_paths_use_existing_absolute_paths() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let execroot = AbsNormPathBuf::new(temp.path().join("execroot"))?;
        let rustc_rel = PathBuf::from("buck-out")
            .join("toolchains")
            .join("rust")
            .join("bin")
            .join("rustc.exe");
        let rustc_abs = execroot.as_path().join(&rustc_rel);
        std::fs::create_dir_all(rustc_abs.parent().unwrap())?;
        std::fs::write(&rustc_abs, b"fake rustc")?;
        let mut env = vec![
            (
                OsString::from("RUSTC"),
                OsString::from(rustc_rel.as_os_str()),
            ),
            (
                OsString::from("UNRELATED"),
                OsString::from("buck-out/toolchains/rust/bin/rustc.exe"),
            ),
        ];

        assert!(rewrite_windows_cargo_tool_env_paths(
            &mut env,
            execroot.as_path()
        ));
        assert!(Path::new(&env[0].1).is_absolute());
        assert_eq!(
            env[1].1,
            OsString::from("buck-out/toolchains/rust/bin/rustc.exe")
        );

        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_cargo_runner_args_rewrite_shortens_script_path() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let execroot = AbsNormPathBuf::new(temp.path().join("execroot"))?;
        std::fs::create_dir_all(&execroot)?;
        let script = format!("buck-out/{}/gen/crate/_bs-.exe", "x".repeat(230));
        let script_abs = execroot.as_path().join(&script);
        std::fs::create_dir_all(script_abs.parent().unwrap())?;
        std::fs::write(&script_abs, b"fake exe")?;
        assert!(script_abs.as_os_str().len() > 200);
        let mut args = vec![
            "buck-out/gen/rules_rust/cargo/private/cargo_build_script_runner/runner.exe".to_owned(),
            format!("--script={script}"),
        ];

        assert!(rewrite_windows_cargo_build_script_runner_args(
            &mut args,
            Some(&execroot)
        ));
        assert!(args[1].starts_with("--script=.slug-cargo-script"));
        let alias = args[1].trim_start_matches("--script=");
        assert!(execroot.as_path().join(alias).is_file());

        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_cargo_runner_args_rewrite_updates_manifest_runfile_source()
    -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let execroot = AbsNormPathBuf::new(temp.path().join("execroot"))?;
        std::fs::create_dir_all(&execroot)?;
        let script = format!("buck-out/{}/gen/crate/_bs-.exe", "x".repeat(230));
        let companion = format!("buck-out/{}/gen/crate/_bs_.exe", "x".repeat(230));
        let script_abs = execroot.as_path().join(&script);
        let companion_abs = execroot.as_path().join(&companion);
        std::fs::create_dir_all(script_abs.parent().unwrap())?;
        std::fs::write(&script_abs, b"fake exe")?;
        std::fs::write(&companion_abs, b"fake companion exe")?;
        let param = execroot.as_path().join("cargo-manifest.params");
        std::fs::write(
            &param,
            format!(
                "buck-out/long/gen/crate/_bs.cargo_runfiles\n.lib,.so\n{script}=crate/_bs-.exe\n{companion}=crate/_bs_.exe\nexternal/crate/Cargo.toml=crate/Cargo.toml"
            ),
        )?;
        let mut args = vec![
            "buck-out/gen/rules_rust/cargo/private/cargo_build_script_runner/runner.exe".to_owned(),
            format!("--script={script}"),
            format!("--cargo_manifest_args=@{}", param.display()),
        ];

        assert!(rewrite_windows_cargo_build_script_runner_args(
            &mut args,
            Some(&execroot)
        ));
        let rewritten = std::fs::read_to_string(&param)?;
        assert!(rewritten.contains(".slug-cargo-script"));
        assert!(rewritten.contains(".slug-cargo-runfile-src"));
        assert!(!rewritten.contains(&script));
        assert!(!rewritten.contains(&companion));

        Ok(())
    }

    #[test]
    fn test_rustc_execroot_remap_ignores_non_rustc_process_wrapper() -> slug_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let execroot = temp.path().join("execroot").join("7476792f9006565e");
        std::fs::create_dir_all(&execroot)?;
        let execroot = AbsNormPathBuf::new(execroot)?;
        let mut args = vec![
            "external/rules_rust/util/process_wrapper/process_wrapper".to_owned(),
            "--subst".to_owned(),
            "pwd=${pwd}".to_owned(),
            "--".to_owned(),
            "external/tools/not-rustc".to_owned(),
        ];

        assert!(!add_rustc_execroot_remap(&mut args, Some(&execroot)));
        assert_eq!(args.len(), 5);

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_normalize_local_output_permissions_sets_bazel_output_mode() -> slug_error::Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir()?;
        let dir = AbsNormPathBuf::new(temp.path().join("out"))?;
        let file = AbsNormPathBuf::new(temp.path().join("out/data.txt"))?;
        let executable = AbsNormPathBuf::new(temp.path().join("out/tool"))?;
        fs_util::create_dir_all(&dir)?;
        fs_util::write(&file, b"data")?;
        fs_util::write(&executable, b"tool")?;
        fs_util::set_executable(&file, false)?;
        fs_util::set_executable(&executable, true)?;

        normalize_local_output_permissions(&dir)?;

        let dir_mode = fs_util::metadata(&dir)?.permissions().mode() & 0o777;
        let file_mode = fs_util::metadata(&file)?.permissions().mode() & 0o777;
        let executable_mode = fs_util::metadata(&executable)?.permissions().mode() & 0o777;

        assert_eq!(dir_mode, 0o555);
        assert_eq!(file_mode, 0o555);
        assert_eq!(executable_mode, 0o555);

        Ok(())
    }

    #[tokio::test]
    async fn test_exec_cmd_environment() -> slug_error::Result<()> {
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
                None,
                None,
            )
            .await?;
        assert_matches!(status, GatherOutputStatus::Finished { exit_code, .. } if exit_code == 0);

        let stdout = std::str::from_utf8(&stdout).buck_error_context("Invalid stdout")?;

        // Plan 44 Phase 2.5: cwd is the synthesized execroot
        // (`<root>/execroot/<basename>/`), so PWD/pwd reflect that path
        // when the helper succeeds. The helper may decline to set up a
        // layout (e.g. test_executor's tmpdir lacks a basename) — fall
        // back to the project root.
        let expected_path = slug_core::cells::execroot_path(root.as_path())
            .filter(|p| p.is_dir())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned());

        if cfg!(windows) {
            let lines: Vec<&str> = stdout.split("\r\n").collect();
            assert_eq!(lines[3], expected_path);
            assert_eq!(lines[4], expected_path);
        } else {
            assert_eq!(stdout, format!("{expected_path}\n{expected_path}\n"));
        }

        Ok(())
    }

    #[cfg(fbcode_build)]
    #[tokio::test]
    async fn test_exec_cmd_timeout() -> slug_error::Result<()> {
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
                None,
                None,
            )
            .await?;
        assert_matches!(status, GatherOutputStatus::TimedOut ( duration ) if duration == Duration::from_secs(1));

        Ok(())
    }

    #[cfg(unix)] // TODO: something similar on Windows: T123279320
    #[tokio::test]
    async fn test_exec_cmd_environment_filtering() -> slug_error::Result<()> {
        use slug_execute::execute::environment_inheritance::EnvironmentInheritance;

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
                None,
                None,
            )
            .await?;
        assert_matches!(status, GatherOutputStatus::Finished { exit_code, .. } if exit_code == 0);
        assert_eq!(stdout, b"\n");

        Ok(())
    }
}
