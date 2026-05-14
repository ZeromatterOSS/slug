/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashSet;
use std::fmt::Debug;
use std::ops::ControlFlow;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use derivative::Derivative;
use derive_more::Display;
use dice::DiceComputations;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use either::Either;
use fxhash::FxHashMap;
use indexmap::IndexMap;
use indexmap::IndexSet;
use indexmap::indexmap;
use itertools::Itertools;
use remote_execution::TActionResult2;
use slug_artifact::artifact::artifact_type::Artifact;
use slug_artifact::artifact::build_artifact::BuildArtifact;
use slug_build_signals::env::WaitingData;
use slug_common::dice::data::HasIoProvider;
use slug_common::events::HasEvents;
use slug_common::http::HasHttpClient;
use slug_common::io::IoProvider;
use slug_common::liveliness_observer::NoopLivelinessObserver;
use slug_core::content_hash::ContentBasedPathHash;
use slug_core::execution_types::executor_config::CommandExecutorConfig;
use slug_core::fs::artifact_path_resolver::ArtifactFs;
use slug_core::fs::buck_out_path::BuildArtifactPath;
use slug_data::SchedulingMode;
use slug_error::BuckErrorContext;
use slug_error::internal_error;
use slug_events::dispatch::EventDispatcher;
use slug_execute::artifact::fs::ExecutorFs;
use slug_execute::artifact_value::ArtifactValue;
use slug_execute::digest_config::DigestConfig;
use slug_execute::digest_config::HasDigestConfig;
use slug_execute::execute::action_digest_and_blobs::ActionDigestAndBlobs;
use slug_execute::execute::blocking::BlockingExecutor;
use slug_execute::execute::blocking::HasBlockingExecutor;
use slug_execute::execute::cache_uploader::CacheUploadInfo;
use slug_execute::execute::cache_uploader::CacheUploadResult;
use slug_execute::execute::cache_uploader::IntoRemoteDepFile;
use slug_execute::execute::claim::MutexClaimManager;
use slug_execute::execute::clean_output_paths::CleanOutputPaths;
use slug_execute::execute::command_executor::ActionExecutionTimingData;
use slug_execute::execute::command_executor::CommandExecutor;
use slug_execute::execute::dep_file_digest::DepFileDigest;
use slug_execute::execute::kind::CommandExecutionKind;
use slug_execute::execute::manager::CommandExecutionManager;
use slug_execute::execute::prepared::PreparedAction;
use slug_execute::execute::prepared::PreparedCommand;
use slug_execute::execute::request::CommandExecutionRequest;
use slug_execute::execute::request::ExecutorPreference;
use slug_execute::execute::request::OutputType;
use slug_execute::execute::result::CommandExecutionReport;
use slug_execute::execute::result::CommandExecutionResult;
use slug_execute::execute::result::CommandExecutionStatus;
use slug_execute::materialize::materializer::HasMaterializer;
use slug_execute::materialize::materializer::Materializer;
use slug_execute::output_size::OutputCountAndBytes;
use slug_execute::output_size::OutputSize;
use slug_execute::path::artifact_path::ArtifactPath;
use slug_execute::re::manager::UnconfiguredRemoteExecutionClient;
use slug_execute::re::output_trees_download_config::OutputTreesDownloadConfig;
use slug_file_watcher::mergebase::GetMergebase;
use slug_file_watcher::mergebase::Mergebase;
use slug_http::HttpClient;

use crate::actions::ActionExecutionCtx;
use crate::actions::RegisteredAction;
use crate::actions::artifact::get_artifact_fs::GetArtifactFs;
use crate::actions::execute::action_execution_target::ActionExecutionTarget;
use crate::actions::execute::dice_data::CommandExecutorResponse;
use crate::actions::execute::dice_data::DiceHasCommandExecutor;
use crate::actions::execute::dice_data::GetInvalidationTrackingConfig;
use crate::actions::execute::dice_data::GetReClient;
use crate::actions::execute::error::ExecuteError;
use crate::actions::impls::run_action_knobs::HasRunActionKnobs;
use crate::actions::impls::run_action_knobs::RunActionKnobs;
use crate::artifact_groups::ArtifactGroup;
use crate::artifact_groups::ArtifactGroupValues;

/// This is the result of the action as exposed to other things in the dice computation.
#[derive(Clone, Dupe, Debug, PartialEq, Eq, Allocative)]
pub struct ActionOutputs(Arc<ActionOutputsData>);

impl OutputSize for ActionOutputs {
    fn calc_output_count_and_bytes(&self) -> OutputCountAndBytes {
        let mut total_count = 0;
        let mut total_bytes = 0;
        for v in self.values() {
            let count_and_bytes = v.calc_output_count_and_bytes();
            total_count += count_and_bytes.count;
            total_bytes += count_and_bytes.bytes;
        }
        OutputCountAndBytes {
            count: total_count,
            bytes: total_bytes,
        }
    }
}

#[derive(Derivative, Debug, Allocative)]
#[derivative(PartialEq, Eq)]
struct ActionOutputsData {
    outputs: IndexMap<BuildArtifactPath, ArtifactValue>,
}

/// Metadata associated with the execution of this action.
#[derive(Debug)]
pub struct ActionExecutionMetadata {
    pub execution_kind: ActionExecutionKind,
    pub timing: ActionExecutionTimingData,
    pub input_files_bytes: Option<u64>,
    pub waiting_data: WaitingData,
}

/// The *way* that a particular action was executed.
#[derive(Debug, Display, Clone)]
pub enum ActionExecutionKind {
    #[display("command({})", kind)]
    Command {
        kind: Box<CommandExecutionKind>,
        prefers_local: bool,
        requires_local: bool,
        allows_cache_upload: bool,
        did_cache_upload: bool,
        allows_dep_file_cache_upload: bool,
        did_dep_file_cache_upload: bool,
        eligible_for_full_hybrid: bool,
        dep_file_key: Option<DepFileDigest>,
        scheduling_mode: Option<SchedulingMode>,
        incremental_kind: slug_data::IncrementalKind,
    },
    /// This action is simple and executed inline within slug (e.g. write, symlink_dir)
    #[display("simple")]
    Simple,
    /// This action logically executed, but didn't do all the work.
    #[display("deferred")]
    Deferred,
    /// This action was served by the local dep file cache and not executed.
    #[display("local_dep_files")]
    LocalDepFile,

    /// This action was served by the local action cache and not executed.
    #[display("local_action_cache")]
    LocalActionCache,
}

pub struct CommandExecutionRef<'a> {
    pub kind: &'a CommandExecutionKind,
    pub prefers_local: bool,
    pub requires_local: bool,
    pub allows_cache_upload: bool,
    pub did_cache_upload: bool,
    pub allows_dep_file_cache_upload: bool,
    pub did_dep_file_cache_upload: bool,
    pub eligible_for_full_hybrid: bool,
    pub scheduling_mode: Option<SchedulingMode>,
    pub dep_file_key: &'a Option<DepFileDigest>,
    pub incremental_kind: slug_data::IncrementalKind,
}

impl ActionExecutionKind {
    pub fn as_enum(&self) -> slug_data::ActionExecutionKind {
        match self {
            ActionExecutionKind::Command { kind, .. } => kind.as_enum(),
            ActionExecutionKind::Simple => slug_data::ActionExecutionKind::Simple,
            ActionExecutionKind::Deferred => slug_data::ActionExecutionKind::Deferred,
            ActionExecutionKind::LocalDepFile => slug_data::ActionExecutionKind::LocalDepFile,
            ActionExecutionKind::LocalActionCache => {
                slug_data::ActionExecutionKind::LocalActionCache
            }
        }
    }

    pub fn command(&self) -> Option<CommandExecutionRef<'_>> {
        match self {
            Self::Command {
                kind,
                prefers_local,
                requires_local,
                allows_cache_upload,
                did_cache_upload,
                allows_dep_file_cache_upload,
                did_dep_file_cache_upload,
                dep_file_key,
                eligible_for_full_hybrid,
                scheduling_mode,
                incremental_kind,
                ..
            } => Some(CommandExecutionRef {
                kind,
                prefers_local: *prefers_local,
                requires_local: *requires_local,
                allows_cache_upload: *allows_cache_upload,
                did_cache_upload: *did_cache_upload,
                allows_dep_file_cache_upload: *allows_dep_file_cache_upload,
                did_dep_file_cache_upload: *did_dep_file_cache_upload,
                dep_file_key,
                eligible_for_full_hybrid: *eligible_for_full_hybrid,
                scheduling_mode: *scheduling_mode.dupe(),
                incremental_kind: *incremental_kind,
            }),
            Self::Simple | Self::Deferred | Self::LocalDepFile | Self::LocalActionCache => None,
        }
    }
}

impl ActionOutputs {
    pub fn new(outputs: IndexMap<BuildArtifactPath, ArtifactValue>) -> Self {
        Self(Arc::new(ActionOutputsData { outputs }))
    }

    pub fn from_single(artifact: BuildArtifactPath, value: ArtifactValue) -> Self {
        Self::new(indexmap! {artifact => value})
    }

    pub fn get(&self, artifact: &BuildArtifactPath) -> Option<&ArtifactValue> {
        self.0.outputs.get(artifact)
    }

    pub fn get_from_artifact_path(&self, path: &ArtifactPath) -> Option<&ArtifactValue> {
        match path.base_path.as_ref() {
            Either::Left(base) => self.get(base),
            Either::Right(_) => None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&BuildArtifactPath, &ArtifactValue)> {
        self.0.outputs.iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &ArtifactValue> {
        self.0.outputs.values()
    }
}

#[async_trait]
pub trait HasActionExecutor {
    async fn get_action_executor(
        &mut self,
        config: &CommandExecutorConfig,
    ) -> slug_error::Result<Arc<BuckActionExecutor>>;
}

#[async_trait]
impl HasActionExecutor for DiceComputations<'_> {
    async fn get_action_executor(
        &mut self,
        executor_config: &CommandExecutorConfig,
    ) -> slug_error::Result<Arc<BuckActionExecutor>> {
        let artifact_fs = self.get_artifact_fs().await?;
        let digest_config = self.global_data().get_digest_config();

        let CommandExecutorResponse {
            executor,
            platform,
            action_cache_checker,
            remote_dep_file_cache_checker,
            cache_uploader,
            output_trees_download_config,
        } = self.get_command_executor_from_dice(executor_config).await?;
        let blocking_executor = self.get_blocking_executor();
        let materializer = self.per_transaction_data().get_materializer();
        let events = self.per_transaction_data().get_dispatcher().dupe();
        let re_client = self.per_transaction_data().get_re_client();
        let run_action_knobs = self.per_transaction_data().get_run_action_knobs().dupe();
        let io_provider = self.global_data().get_io_provider();
        let http_client = self.per_transaction_data().get_http_client();
        let mergebase = self.per_transaction_data().get_mergebase();
        let invalidation_tracking_enabled = self.get_invalidation_tracking_config().enabled;

        Ok(Arc::new(BuckActionExecutor::new(
            CommandExecutor::new(
                executor,
                action_cache_checker,
                remote_dep_file_cache_checker,
                cache_uploader,
                artifact_fs,
                executor_config.options,
                platform,
            ),
            blocking_executor,
            materializer,
            events,
            re_client,
            digest_config,
            run_action_knobs,
            io_provider,
            http_client,
            mergebase,
            invalidation_tracking_enabled,
            output_trees_download_config,
        )))
    }
}

pub struct BuckActionExecutor {
    command_executor: CommandExecutor,
    blocking_executor: Arc<dyn BlockingExecutor>,
    materializer: Arc<dyn Materializer>,
    events: EventDispatcher,
    re_client: UnconfiguredRemoteExecutionClient,
    digest_config: DigestConfig,
    run_action_knobs: RunActionKnobs,
    io_provider: Arc<dyn IoProvider>,
    http_client: HttpClient,
    mergebase: Mergebase,
    invalidation_tracking_enabled: bool,
    output_trees_download_config: OutputTreesDownloadConfig,
}

impl BuckActionExecutor {
    pub fn new(
        command_executor: CommandExecutor,
        blocking_executor: Arc<dyn BlockingExecutor>,
        materializer: Arc<dyn Materializer>,
        events: EventDispatcher,
        re_client: UnconfiguredRemoteExecutionClient,
        digest_config: DigestConfig,
        run_action_knobs: RunActionKnobs,
        io_provider: Arc<dyn IoProvider>,
        http_client: HttpClient,
        mergebase: Mergebase,
        invalidation_tracking_enabled: bool,
        output_trees_download_config: OutputTreesDownloadConfig,
    ) -> Self {
        BuckActionExecutor {
            command_executor,
            blocking_executor,
            materializer,
            events,
            re_client,
            digest_config,
            run_action_knobs,
            io_provider,
            http_client,
            mergebase,
            invalidation_tracking_enabled,
            output_trees_download_config,
        }
    }
}

struct BuckActionExecutionContext<'a> {
    executor: &'a BuckActionExecutor,
    action: &'a RegisteredAction,
    inputs: IndexMap<ArtifactGroup, ArtifactGroupValues>,
    outputs: &'a [BuildArtifact],
    command_reports: &'a mut Vec<CommandExecutionReport>,
    cancellations: &'a CancellationContext,
}

#[async_trait]
impl ActionExecutionCtx for BuckActionExecutionContext<'_> {
    fn target(&self) -> ActionExecutionTarget<'_> {
        ActionExecutionTarget::new(self.action)
    }

    fn fs(&self) -> &ArtifactFs {
        self.executor.command_executor.fs()
    }

    fn executor_fs(&self) -> ExecutorFs<'_> {
        self.executor.command_executor.executor_fs()
    }

    fn materializer(&self) -> &dyn Materializer {
        self.executor.materializer.as_ref()
    }

    fn events(&self) -> &EventDispatcher {
        &self.executor.events
    }

    fn command_execution_manager(&self, waiting_data: WaitingData) -> CommandExecutionManager {
        CommandExecutionManager::new(
            Box::new(MutexClaimManager::new()),
            self.executor.events.dupe(),
            NoopLivelinessObserver::create(),
            waiting_data,
        )
    }

    fn artifact_values(&self, artifact: &ArtifactGroup) -> &ArtifactGroupValues {
        self.inputs.get(artifact).unwrap_or_else(|| panic!("Internal error: action {} tried to grab the artifact {} even though it was not an input.", self.action.owner(), artifact))
    }

    fn artifact_path_mapping(
        &self,
        filter: Option<IndexSet<ArtifactGroup>>,
    ) -> FxHashMap<&Artifact, ContentBasedPathHash> {
        self.inputs
            .iter()
            .filter(|(ag, _)| {
                if !ag.uses_content_based_path() {
                    return false;
                }

                match filter {
                    Some(ref filter) => filter.contains(*ag),
                    None => true,
                }
            })
            .flat_map(|(_, v)| v.iter())
            .map(|(a, v)| (a, v.content_based_path_hash()))
            .collect()
    }

    fn blocking_executor(&self) -> &dyn BlockingExecutor {
        self.executor.blocking_executor.as_ref()
    }

    fn re_client(&self) -> UnconfiguredRemoteExecutionClient {
        self.executor.re_client.dupe()
    }

    fn re_platform(&self) -> &remote_execution::Platform {
        self.executor.command_executor.re_platform()
    }

    fn digest_config(&self) -> DigestConfig {
        self.executor.digest_config
    }

    fn run_action_knobs(&self) -> &RunActionKnobs {
        &self.executor.run_action_knobs
    }

    fn cancellation_context(&self) -> &CancellationContext {
        self.cancellations
    }

    fn mergebase(&self) -> &Mergebase {
        &self.executor.mergebase
    }

    fn prepare_action(
        &mut self,
        request: &CommandExecutionRequest,
        re_outputs_required: bool,
    ) -> slug_error::Result<PreparedAction> {
        self.executor.command_executor.prepare_action(
            request,
            self.digest_config(),
            re_outputs_required,
        )
    }

    async fn action_cache(
        &mut self,
        manager: CommandExecutionManager,
        request: &CommandExecutionRequest,
        prepared_action: &PreparedAction,
    ) -> ControlFlow<CommandExecutionResult, CommandExecutionManager> {
        let action = self.target();
        self.executor
            .command_executor
            .action_cache(
                manager,
                &PreparedCommand {
                    target: &action as _,
                    request,
                    prepared_action,
                    digest_config: self.digest_config(),
                },
                self.cancellations,
            )
            .await
    }

    async fn remote_dep_file_cache(
        &mut self,
        manager: CommandExecutionManager,
        request: &CommandExecutionRequest,
        prepared_action: &PreparedAction,
    ) -> ControlFlow<CommandExecutionResult, CommandExecutionManager> {
        let action = self.target();
        self.executor
            .command_executor
            .remote_dep_file_cache(
                manager,
                &PreparedCommand {
                    target: &action as _,
                    request,
                    prepared_action,
                    digest_config: self.digest_config(),
                },
                self.cancellations,
            )
            .await
    }

    fn unpack_command_execution_result(
        &mut self,
        executor_preference: ExecutorPreference,
        result: CommandExecutionResult,
        allows_cache_upload: bool,
        allows_dep_file_cache_upload: bool,
        input_files_bytes: Option<u64>,
        incremental_kind: slug_data::IncrementalKind,
    ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
        let CommandExecutionResult {
            outputs,
            report,
            rejected_execution,
            did_cache_upload,
            did_dep_file_cache_upload,
            dep_file_key,
            eligible_for_full_hybrid,
            scheduling_mode,
            waiting_data,
            ..
        } = result;

        // TODO(T156483516): We should also validate that the outputs match the expected outputs
        let action_outputs = ActionOutputs::new(
            outputs
                .into_iter()
                .filter_map(|(output, value)| Some((output.into_build_artifact()?.0, value)))
                .collect(),
        );

        // TODO (@torozco): The execution kind should be made to come via the command reports too.
        let res = match &report.status {
            CommandExecutionStatus::Success { execution_kind } => {
                let result = (
                    action_outputs,
                    ActionExecutionMetadata {
                        execution_kind: ActionExecutionKind::Command {
                            kind: Box::new(execution_kind.clone()),
                            prefers_local: executor_preference.prefers_local(),
                            requires_local: executor_preference.requires_local(),
                            allows_cache_upload,
                            did_cache_upload,
                            allows_dep_file_cache_upload,
                            did_dep_file_cache_upload,
                            dep_file_key,
                            eligible_for_full_hybrid,
                            scheduling_mode,
                            incremental_kind,
                        },
                        timing: report.timing.into(),
                        input_files_bytes,
                        waiting_data,
                    },
                );
                Ok(result)
            }
            CommandExecutionStatus::Error { error, .. } => {
                Err(ExecuteError::CommandExecutionError {
                    action_outputs,
                    error: Some(error.clone()),
                })
            }
            _ => Err(ExecuteError::CommandExecutionError {
                action_outputs,
                error: None,
            }),
        };
        self.command_reports.extend(rejected_execution);
        self.command_reports.push(report);
        res
    }

    async fn exec_cmd(
        &mut self,
        manager: CommandExecutionManager,
        request: &CommandExecutionRequest,
        prepared_action: &PreparedAction,
    ) -> CommandExecutionResult {
        let action = self.target();
        self.executor
            .command_executor
            .exec_cmd(
                manager,
                &PreparedCommand {
                    target: &action as _,
                    request,
                    prepared_action,
                    digest_config: self.digest_config(),
                },
                self.cancellations,
            )
            .await
    }

    async fn cache_upload(
        &mut self,
        action_digest_and_blobs: &ActionDigestAndBlobs,
        execution_result: &CommandExecutionResult,
        re_result: Option<TActionResult2>,
        dep_file_bundle: Option<&mut dyn IntoRemoteDepFile>,
    ) -> slug_error::Result<CacheUploadResult> {
        let action = self.target();
        Ok(self
            .executor
            .command_executor
            .cache_upload(
                &CacheUploadInfo {
                    target: &action as _,
                    digest_config: self.digest_config(),
                    mergebase: self.mergebase().0.as_ref(),
                    re_platform: self.re_platform(),
                },
                execution_result,
                re_result,
                dep_file_bundle,
                action_digest_and_blobs,
            )
            .await?)
    }

    async fn cleanup_outputs(&mut self) -> slug_error::Result<()> {
        // Delete all outputs before we start, so things will be clean.
        let output_paths = self
            .outputs
            .iter()
            .map(|o| {
                if o.get_path().is_content_based_path() {
                    internal_error!("Cleanup outputs is not supported for content-based paths!");
                }
                self.fs().resolve_build(o.get_path(), None)
            })
            .collect::<slug_error::Result<Vec<_>>>()?;

        // Invalidate all the output paths this action might provide. Note that this is a bit
        // approximative: we might have previous instances of this action that declared
        // different outputs with a different materialization method that will become invalid
        // now. However, nothing should reference those stale outputs, so while this does not
        // do a good job of cleaning up garbage, it prevents using invalid artifacts.
        self.executor
            .materializer
            .invalidate_many(output_paths.clone())
            .await
            .buck_error_context("Failed to invalidate output directory")?;

        self.executor
            .blocking_executor
            .execute_io(
                Box::new(CleanOutputPaths {
                    paths: output_paths,
                }),
                self.cancellations,
            )
            .await
            .buck_error_context("Failed to cleanup output directory")?;

        Ok(())
    }

    fn io_provider(&self) -> Arc<dyn IoProvider> {
        self.executor.io_provider.dupe()
    }

    fn http_client(&self) -> HttpClient {
        self.executor.http_client.dupe()
    }

    fn output_trees_download_config(&self) -> &OutputTreesDownloadConfig {
        &self.executor.output_trees_download_config
    }
}

impl BuckActionExecutor {
    pub(crate) async fn execute(
        &self,
        waiting_data: WaitingData,
        inputs: IndexMap<ArtifactGroup, ArtifactGroupValues>,
        action: &RegisteredAction,
        cancellations: &CancellationContext,
    ) -> (
        Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError>,
        Vec<CommandExecutionReport>,
    ) {
        let mut command_reports = Vec::new();

        let res = async {
            let outputs = action.outputs();

            let mut ctx = BuckActionExecutionContext {
                executor: self,
                action,
                inputs,
                outputs: outputs.as_ref(),
                command_reports: &mut command_reports,
                cancellations,
            };

            let (result, metadata) = action.execute(&mut ctx, waiting_data).await?;

            // Check that all the outputs are the right output_type
            for x in outputs.iter() {
                let declared = x.output_type();
                // FIXME: One day we should treat FileOrDirectory as a File, and soft_error if it is a directory
                if declared != OutputType::FileOrDirectory {
                    if let Some(t) = result.0.outputs.get(x.get_path()) {
                        let real = if t.is_dir() {
                            OutputType::Directory
                        } else {
                            OutputType::File
                        };
                        if real != declared {
                            return Err(ExecuteError::WrongOutputType {
                                path: self.command_executor.fs().resolve_build(
                                    x.get_path(),
                                    Some(&t.content_based_path_hash()),
                                )?,
                                declared,
                                real,
                            });
                        }
                    }
                }
            }

            fn check_all_requested_outputs_returned_without_extra<'a>(
                outputs: &[BuildArtifact],
                result_outputs: impl IntoIterator<Item = &'a BuildArtifactPath>,
            ) -> bool {
                // Ignore ordering as outputs in original action might be ordered differently from
                // output paths in action result (they are sorted there).
                let result_output_paths: HashSet<&BuildArtifactPath> =
                    result_outputs.into_iter().collect();
                let mut outputs_count = 0;
                for output in outputs.iter() {
                    outputs_count += 1;
                    let output_path = output.get_path();
                    if !result_output_paths.contains(output_path) {
                        return false;
                    }
                }
                outputs_count == result_output_paths.len()
            }

            // TODO (T122966509): Check projections here as well
            if !check_all_requested_outputs_returned_without_extra(
                &outputs,
                result.0.outputs.keys(),
            ) {
                let declared = outputs
                    .iter()
                    .filter(|x| !result.0.outputs.contains_key(x.get_path()))
                    .map(|x| {
                        self.command_executor.fs().resolve_build(
                            x.get_path(),
                            Some(&ContentBasedPathHash::for_output_artifact()),
                        )
                    })
                    .collect::<slug_error::Result<_>>()?;
                let real = result
                    .0
                    .outputs
                    .keys()
                    .filter(|x| {
                        // This is error message, linear search is fine.
                        !outputs.iter().map(|b| b.get_path()).contains(x)
                    })
                    .map(|x| {
                        self.command_executor
                            .fs()
                            .resolve_build(x, Some(&ContentBasedPathHash::for_output_artifact()))
                    })
                    .collect::<slug_error::Result<Vec<_>>>()?;
                if real.is_empty() {
                    Err(ExecuteError::MissingOutputs { declared })
                } else {
                    Err(ExecuteError::MismatchedOutputs { declared, real })
                }
            } else {
                Ok((result, metadata))
            }
        }
        .await;

        (res, command_reports)
    }

    pub fn invalidation_tracking_enabled(&self) -> bool {
        self.invalidation_tracking_enabled
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use allocative::Allocative;
    use async_trait::async_trait;
    use dice_futures::cancellation::CancellationContext;
    use dupe::Dupe;
    use indexmap::indexset;
    use slug_artifact::actions::key::ActionIndex;
    use slug_artifact::actions::key::ActionKey;
    use slug_artifact::artifact::artifact_type::Artifact;
    use slug_artifact::artifact::artifact_type::testing::BuildArtifactTestingExt;
    use slug_artifact::artifact::build_artifact::BuildArtifact;
    use slug_artifact::artifact::source_artifact::SourceArtifact;
    use slug_build_signals::env::WaitingData;
    use slug_common::cas_digest::CasDigestConfig;
    use slug_common::io::fs::FsIoProvider;
    use slug_core::category::CategoryRef;
    use slug_core::cells::CellResolver;
    use slug_core::cells::cell_root_path::CellRootPathBuf;
    use slug_core::cells::name::CellName;
    use slug_core::configuration::data::ConfigurationData;
    use slug_core::deferred::base_deferred_key::BaseDeferredKey;
    use slug_core::deferred::key::DeferredHolderKey;
    use slug_core::execution_types::executor_config::CommandExecutorConfig;
    use slug_core::execution_types::executor_config::CommandGenerationOptions;
    use slug_core::execution_types::executor_config::PathSeparatorKind;
    use slug_core::fs::artifact_path_resolver::ArtifactFs;
    use slug_core::fs::buck_out_path::BuckOutPathResolver;
    use slug_core::fs::project::ProjectRootTemp;
    use slug_core::fs::project_rel_path::ProjectRelativePath;
    use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
    use slug_core::package::source_path::SourcePath;
    use slug_core::target::label::label::TargetLabel;
    use slug_events::dispatch::EventDispatcher;
    use slug_events::dispatch::with_dispatcher_async;
    use slug_execute::artifact_value::ArtifactValue;
    use slug_execute::digest_config::DigestConfig;
    use slug_execute::execute::blocking::testing::DummyBlockingExecutor;
    use slug_execute::execute::cache_uploader::NoOpCacheUploader;
    use slug_execute::execute::clean_output_paths::cleanup_path;
    use slug_execute::execute::command_executor::ActionExecutionTimingData;
    use slug_execute::execute::command_executor::CommandExecutor;
    use slug_execute::execute::prepared::NoOpCommandOptionalExecutor;
    use slug_execute::execute::request::CommandExecutionInput;
    use slug_execute::execute::request::CommandExecutionOutput;
    use slug_execute::execute::request::CommandExecutionPaths;
    use slug_execute::execute::request::CommandExecutionRequest;
    use slug_execute::execute::request::OutputType;
    use slug_execute::execute::testing_dry_run::DryRunExecutor;
    use slug_execute::materialize::nodisk::NoDiskMaterializer;
    use slug_execute::re::manager::UnconfiguredRemoteExecutionClient;
    use slug_execute::re::output_trees_download_config::OutputTreesDownloadConfig;
    use slug_fs::fs_util;
    use slug_http::HttpClientBuilder;
    use sorted_vector_map::SortedVectorMap;

    use crate::actions::Action;
    use crate::actions::ActionExecutionCtx;
    use crate::actions::ExecuteError;
    use crate::actions::RegisteredAction;
    use crate::actions::box_slice_set::BoxSliceSet;
    use crate::actions::execute::action_executor::ActionExecutionKind;
    use crate::actions::execute::action_executor::ActionExecutionMetadata;
    use crate::actions::execute::action_executor::ActionOutputs;
    use crate::actions::execute::action_executor::BuckActionExecutor;
    use crate::artifact_groups::ArtifactGroup;
    use crate::artifact_groups::ArtifactGroupValues;

    #[tokio::test]
    async fn can_execute_some_action() {
        slug_certs::certs::maybe_setup_cryptography();
        let cells = CellResolver::testing_with_name_and_path(
            CellName::testing_new("cell"),
            CellRootPathBuf::new(ProjectRelativePathBuf::unchecked_new("cell_path".into())),
        );

        let temp_fs = ProjectRootTemp::new().unwrap();

        let project_fs = temp_fs.path().dupe();
        let artifact_fs = ArtifactFs::new(
            cells,
            BuckOutPathResolver::new(ProjectRelativePathBuf::unchecked_new(
                "cell/buck-out/v2".into(),
            )),
            project_fs.dupe(),
        );

        let tracker = Arc::new(Mutex::new(Vec::new()));

        let executor = BuckActionExecutor::new(
            CommandExecutor::new(
                Arc::new(DryRunExecutor::new(tracker, artifact_fs.clone())),
                Arc::new(NoOpCommandOptionalExecutor {}),
                Arc::new(NoOpCommandOptionalExecutor {}),
                Arc::new(NoOpCacheUploader {}),
                artifact_fs,
                CommandGenerationOptions {
                    path_separator: PathSeparatorKind::Unix,
                    output_paths_behavior: Default::default(),
                    use_bazel_protocol_remote_persistent_workers: false,
                },
                Default::default(),
            ),
            Arc::new(DummyBlockingExecutor {
                fs: project_fs.dupe(),
            }),
            Arc::new(NoDiskMaterializer),
            EventDispatcher::null(),
            UnconfiguredRemoteExecutionClient::testing_new_dummy(),
            DigestConfig::testing_default(),
            Default::default(),
            Arc::new(FsIoProvider::new(
                project_fs,
                CasDigestConfig::testing_default(),
            )),
            HttpClientBuilder::https_with_system_roots()
                .await
                .unwrap()
                .build(),
            Default::default(),
            true,
            OutputTreesDownloadConfig::new(None, true),
        );

        #[derive(Debug, Allocative)]
        struct TestingAction {
            inputs: BoxSliceSet<ArtifactGroup>,
            outputs: BoxSliceSet<BuildArtifact>,
            ran: AtomicBool,
        }

        #[async_trait]
        impl Action for TestingAction {
            fn kind(&self) -> slug_data::ActionKind {
                slug_data::ActionKind::NotSet
            }

            fn inputs(&self) -> slug_error::Result<Cow<'_, [ArtifactGroup]>> {
                Ok(Cow::Borrowed(self.inputs.as_slice()))
            }

            fn outputs(&self) -> Cow<'_, [BuildArtifact]> {
                Cow::Borrowed(self.outputs.as_slice())
            }

            fn first_output(&self) -> &BuildArtifact {
                &self.outputs.as_slice()[0]
            }

            fn category(&self) -> CategoryRef<'_> {
                CategoryRef::new("testing").unwrap()
            }

            fn identifier(&self) -> Option<&str> {
                None
            }

            async fn execute(
                &self,
                ctx: &mut dyn ActionExecutionCtx,
                waiting_data: WaitingData,
            ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
                self.ran.store(true, Ordering::SeqCst);

                let req = CommandExecutionRequest::new(
                    vec![],
                    vec!["foo".to_owned(), "bar".to_owned(), "cmd".to_owned()],
                    CommandExecutionPaths::new(
                        self.inputs
                            .iter()
                            .map(|x| {
                                CommandExecutionInput::Artifact(Box::new(
                                    ArtifactGroupValues::from_artifact(
                                        x.unpack_artifact().unwrap().dupe(),
                                        ArtifactValue::file(ctx.digest_config().empty_file()),
                                    ),
                                ))
                            })
                            .collect(),
                        self.outputs
                            .iter()
                            .map(|b| CommandExecutionOutput::BuildArtifact {
                                path: b.get_path().dupe(),
                                output_type: OutputType::FileOrDirectory,
                                supports_incremental_remote: false,
                            })
                            .collect(),
                        ctx.fs(),
                        ctx.digest_config(),
                        None,
                    )?,
                    SortedVectorMap::new(),
                );

                // on fake executor, this does nothing
                let prepared_action = ctx.prepare_action(&req, true)?;
                let manager = ctx.command_execution_manager(waiting_data);
                let res = ctx.exec_cmd(manager, &req, &prepared_action).await;

                // Must write out the things we promised to do
                for x in &self.outputs {
                    let dest = x.get_path();
                    let dest_path = ctx.fs().resolve_build(dest, None)?;
                    ctx.fs().fs().write_file(&dest_path, "", false)?
                }

                ctx.unpack_command_execution_result(
                    req.executor_preference,
                    res,
                    false,
                    false,
                    None,
                    slug_data::IncrementalKind::NonIncremental,
                )?;
                let outputs = self
                    .outputs
                    .iter()
                    .map(|o| {
                        (
                            o.get_path().dupe(),
                            ArtifactValue::file(ctx.digest_config().empty_file()),
                        )
                    })
                    .collect();
                Ok((
                    ActionOutputs::new(outputs),
                    ActionExecutionMetadata {
                        execution_kind: ActionExecutionKind::Simple,
                        timing: ActionExecutionTimingData::default(),
                        input_files_bytes: None,
                        waiting_data: WaitingData::new(),
                    },
                ))
            }
        }

        let inputs = indexset![ArtifactGroup::Artifact(Artifact::from(
            SourceArtifact::new(SourcePath::testing_new("cell//pkg", "source"))
        ))];
        let label =
            TargetLabel::testing_parse("cell//pkg:foo").configure(ConfigurationData::testing_new());
        let outputs = indexset![BuildArtifact::testing_new(
            label.dupe(),
            "output",
            ActionIndex::new(0),
        )];

        let action = RegisteredAction::new(
            ActionKey::new(
                DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(label.dupe())),
                ActionIndex::new(0),
            ),
            Box::new(TestingAction {
                inputs: BoxSliceSet::from(inputs),
                outputs: BoxSliceSet::from(outputs.clone()),
                ran: Default::default(),
            }),
            CommandExecutorConfig::testing_local(),
        );
        let res = with_dispatcher_async(
            EventDispatcher::null(),
            executor.execute(
                WaitingData::new(),
                Default::default(),
                &action,
                CancellationContext::testing(),
            ),
        )
        .await
        .0
        .unwrap();
        let outputs = outputs
            .iter()
            .map(|o| {
                (
                    o.get_path().dupe(),
                    ArtifactValue::file(executor.digest_config.empty_file()),
                )
            })
            .collect();
        assert_eq!(res.0, ActionOutputs::new(outputs));
    }

    #[test]
    fn test_cleanup_path_missing() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let fs = fs.path();
        fs_util::create_dir_all(fs.resolve(ProjectRelativePath::unchecked_new("foo/bar/qux")))?;
        cleanup_path(fs, ProjectRelativePath::unchecked_new("foo/bar/qux/xx"))?;
        assert!(
            fs.resolve(ProjectRelativePath::unchecked_new("foo/bar/qux"))
                .exists()
        );
        Ok(())
    }

    #[test]
    fn test_cleanup_path_present() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let fs = fs.path();
        fs_util::create_dir_all(fs.resolve(ProjectRelativePath::unchecked_new("foo/bar/qux")))?;
        cleanup_path(fs, ProjectRelativePath::unchecked_new("foo/bar/qux"))?;
        assert!(
            !fs.resolve(ProjectRelativePath::unchecked_new("foo/bar/qux"))
                .exists()
        );
        assert!(
            fs.resolve(ProjectRelativePath::unchecked_new("foo/bar"))
                .exists()
        );
        Ok(())
    }

    #[test]
    fn test_cleanup_path_overlap() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let fs = fs.path();
        fs.write_file(ProjectRelativePath::unchecked_new("foo/bar"), "xx", false)?;
        cleanup_path(fs, ProjectRelativePath::unchecked_new("foo/bar/qux"))?;
        assert!(
            !fs.resolve(ProjectRelativePath::unchecked_new("foo/bar"))
                .exists()
        );
        assert!(
            fs.resolve(ProjectRelativePath::unchecked_new("foo"))
                .exists()
        );
        Ok(())
    }

    #[test]
    fn test_cleanup_path_overlap_deep() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let fs = fs.path();
        fs.write_file(ProjectRelativePath::unchecked_new("foo/bar"), "xx", false)?;
        cleanup_path(
            fs,
            ProjectRelativePath::unchecked_new("foo/bar/qux/1/2/3/4"),
        )?;
        assert!(
            !fs.resolve(ProjectRelativePath::unchecked_new("foo/bar"))
                .exists()
        );
        assert!(
            fs.resolve(ProjectRelativePath::unchecked_new("foo"))
                .exists()
        );
        Ok(())
    }
}
