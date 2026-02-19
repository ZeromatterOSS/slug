/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashMap;
use std::sync::Arc;

use dupe::Dupe;
use either::Either;
use host_sharing::WeightClass;
use host_sharing::WeightPercentage;
use kuro_artifact::artifact::artifact_type::Artifact;
use kuro_artifact::artifact::artifact_type::ArtifactErrors;
use kuro_artifact::artifact::artifact_type::DeclaredArtifact;
use kuro_artifact::artifact::artifact_type::OutputArtifact;
use kuro_build_api::artifact_groups::ArtifactGroup;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkInputArtifactLike;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_output_artifact::StarlarkOutputArtifact;
use kuro_build_api::interpreter::rule_defs::artifact_tagging::ArtifactTag;
use kuro_build_api::interpreter::rule_defs::cmd_args::CommandLineArgLike;
use kuro_build_api::interpreter::rule_defs::cmd_args::CommandLineArtifactVisitor;
use kuro_build_api::interpreter::rule_defs::cmd_args::SimpleCommandLineArtifactVisitor;
use kuro_build_api::interpreter::rule_defs::cmd_args::StarlarkCmdArgs;
use kuro_build_api::interpreter::rule_defs::cmd_args::StarlarkCommandLineValueUnpack;
use kuro_build_api::interpreter::rule_defs::cmd_args::value_as::ValueAsCommandLineLike;
use kuro_build_api::interpreter::rule_defs::command_executor_config::parse_custom_re_image;
use kuro_build_api::interpreter::rule_defs::command_executor_config::parse_meta_internal_extra_params;
use kuro_build_api::interpreter::rule_defs::context::AnalysisActions;
use kuro_build_api::interpreter::rule_defs::provider::builtin::run_info::RunInfo;
use kuro_build_api::interpreter::rule_defs::provider::builtin::worker_run_info::WorkerRunInfo;
use kuro_core::category::CategoryRef;
use kuro_core::deferred::base_deferred_key::BaseDeferredKey;
use kuro_core::execution_types::executor_config::MetaInternalExtraParams;
use kuro_core::execution_types::executor_config::ReGangWorker;
use kuro_core::execution_types::executor_config::RemoteExecutorDependency;
use kuro_error::BuckErrorContext;
use kuro_error::conversion::from_any_with_tag;
use kuro_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use starlark::collections::SmallSet;
use starlark::environment::MethodsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::StringValue;
use starlark::values::UnpackAndDiscard;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::ValueOf;
use starlark::values::ValueTyped;
use starlark::values::dict::DictRef;
use starlark::values::dict::UnpackDictEntries;
use starlark::values::list::UnpackList;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::typing::StarlarkCallable;
use starlark_map::small_map;
use starlark_map::small_map::SmallMap;

use crate::actions::impls::run::MetadataParameter;
use crate::actions::impls::run::StarlarkRunActionValues;
use crate::actions::impls::run::UnregisteredRunAction;
use crate::actions::impls::run::dep_files::RunActionDepFiles;
use crate::actions::impls::run::new_executor_preference;

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
pub(crate) enum RunActionError {
    #[error("expected at least one output artifact, did not get any")]
    NoOutputsSpecified,
    #[error("`outputs` parameter must be a list or iterable, got `{0}`")]
    OutputsNotIterable(String),
    #[error(
        "`outputs` contains invalid value `{0}` - expected artifact from declare_file() or .as_output()"
    )]
    InvalidOutputType(String),
    #[error("`weight` must be a positive integer, got `{0}`")]
    InvalidWeight(i32),
    #[error("`weight` and `weight_percentage` cannot both be passed")]
    DuplicateWeightsSpecified,
    #[error("`dep_files` value with key `{}` has an invalid count of associated outputs. Expected 1, got {}.", .key, .count)]
    InvalidDepFileOutputs { key: String, count: usize },
    #[error("`dep_files` with keys `{}` and `{}` are using the same tag", .first, .second)]
    ConflictingDepFiles { first: String, second: String },
    #[error("Dep-files input `{}` is tagged with multiple tags relevant for dep-files: `{}` and `{}`", .input, .tags[0], .tags[1])]
    ConflictingDepFileInputTags {
        input: ArtifactGroup,
        tags: Vec<String>,
    },
    #[error(
        "missing `metadata_path` parameter which is required when `metadata_env_var` parameter is present"
    )]
    MetadataPathMissing,
    #[error(
        "missing `metadata_env_var` parameter which is required when `metadata_path` parameter is present"
    )]
    MetadataEnvVarMissing,
    #[error(
        "Recursion limit exceeded when visiting artifacts: do you have a cycle in your inputs or outputs?"
    )]
    ArtifactVisitRecursionLimitExceeded,
    #[error(
        "`{}` was marked to be materialized on failure but is not declared as an output of the action.", .path
    )]
    FailedActionArtifactNotDeclared { path: String },
    #[error(
        "Action is marked with `incremental_remote_outputs` but output `{}` is content-based, which is not allowed.", .path
    )]
    IncrementalRemoteOutputsWithContentBasedOutputs { path: String },
    #[error(
        "Action is marked with `incremental_remote_outputs` but not `no_outputs_cleanup`, which is not allowed."
    )]
    IncrementalRemoteOutputsWithoutNoOutputsCleanup,
    #[error(
        "Action is marked with `expect_eligible_for_dedupe` but output `{}` is not content-based", .path
    )]
    ExpectEligibleForDedupeWithNonContentBasedOutput { path: String },
    #[error(
        "Action is marked with `expect_eligible_for_dedupe` but input `{}` is not eligible for dedupe", .input
    )]
    ExpectEligibleForDedupeWithIneligibleInput { input: ArtifactGroup },
}

#[starlark_module]
pub(crate) fn analysis_actions_methods_run(methods: &mut MethodsBuilder) {
    /// Run a command to produce one or more artifacts.
    ///
    /// * `arguments`: must be of type `cmd_args`, or a type convertible to such (such as a list of
    ///   strings and artifacts). See below for detailed description of artifact arguments.
    /// * `env`: environment variables to set when the command is executed.
    /// * `category`: category and identifier - when used together, identify the action in Kuro's
    ///   event stream, and must be unique for a given target
    /// * `weight`: used to note how heavy the command is and will typically be set to a higher
    ///   value to indicate that less such commands should be run in parallel (if running locally)
    /// * `no_outputs_cleanup`: if this flag is set then Kuro won't clean the outputs of a previous
    ///   build that might be present on a disk; in which case, command from arguments should be
    ///   responsible for the cleanup (that is useful, for example, when an action is supporting
    ///   incremental mode and its outputs are based on result from a previous build)
    /// * `metadata_env_var` and `meadata_path` should be used together: both set or both unset
    ///     * `metadata_path`: defines a path relative to the result directory for a file with
    ///       action metadata, which will be created right before the command will be run.
    ///     * Metadata contains the path relative to the Kuro project root and hash digest for
    ///       every action input (this excludes symlinks as they could be resolved by a user script
    ///       if needed). The resolved path relative to the Kuro project for the metadata file will
    ///       be passed to command from arguments, via the environment variable, with its name set
    ///       by `metadata_env_var`
    ///     * Both `metadata_env_var` and `metadata_path` are useful when making actions behave in
    ///       an incremental manner (for details, see [Incremental
    ///       Actions](https://kuro.build/docs/rule_authors/incremental_actions/))
    /// * `dep_files`: a dictionary mapping labels to `ArtifactTag` instances for tracking actual
    ///   dependencies via dependency files (depfiles). This enables precise incremental builds by
    ///   allowing the build tool to report which inputs it actually used.
    ///     * Each entry maps a string label (e.g., `"headers"`) to an `ArtifactTag` created via
    ///       `ctx.actions.artifact_tag()`
    ///     * The tag should be used to mark both the potential inputs (via `tag.tag_artifacts()`)
    ///       and the depfile output that will list the actual inputs used
    ///     * After execution, Kuro reads the depfile and only tracks changes to inputs listed in it,
    ///       rather than all tagged inputs
    ///     * Depfiles must use Makefile syntax: `output: input1 input2 input3`
    ///     * For complete documentation and examples, see [`ctx.actions.artifact_tag()`](../AnalysisActions#analysisactionsartifact_tag)
    /// * `allow_offline_output_cache`: enables caching of this action's outputs for offline builds (default: `false`)
    ///     * When `true`, action outputs are cached during trace builds (via `kuro debug trace-io`)
    ///       and restored during offline builds without re-executing the action
    ///     * Intended for actions that read from the network (e.g., downloads, remote artifact fetches)
    ///       which cannot execute in offline build environments where network access is restricted
    ///     * During trace builds: outputs are copied to `buck-out/offline-cache/` after successful execution
    ///     * During offline builds: if all outputs exist in offline cache, they are restored without
    ///       running the action; otherwise the action executes normally (graceful fallback)
    ///     * Requires `kuro.use_network_action_output_cache=true` config to take effect
    ///     * Example use case: caching network downloads in containerized offline build environments
    /// * The `prefer_local`, `prefer_remote` and `local_only` options allow selecting where the
    /// action should run if the executor selected for this target is a hybrid executor.
    ///     * All those options disable concurrent execution: the action will run on the preferred
    ///     platform first (concurrent execution only happens with a "full" hybrid executor).
    ///     * Execution may be retried on the "non-preferred" platform if it fails due to a
    ///     transient error, except for `local_only`, which does not allow this.
    ///     * If the executor selected is a remote-only executor and you use `local_only`, that's an
    ///     error. The other options will not raise errors.
    ///     * Setting more than one of those options is an error.
    ///     * Those flags behave the same way as the equivalent `--prefer-remote`, `--prefer-local`
    ///     and `--local-only` CLI flags. The CLI flags take precedence.
    ///     * The `force_full_hybrid_if_capable` option overrides the `use_limited_hybrid` hybrid.
    ///     The options listed above take precedence if set.
    /// * `remote_execution_dependencies`: list of dependencies which is passed to Remote Execution.
    ///   Each dependency is dictionary with the following keys:
    ///     * `smc_tier`: name of the SMC tier to call by RE Scheduler.
    ///     * `id`: name of the dependency.
    /// * `remote_execution_dynamic_image`: a custom Tupperware image which is passed to Remote Execution.
    ///   It takes a dictionary with the following keys:
    ///     * `identifier`: name of the SMC tier to call by RE Scheduler.
    ///         * `name`: name of the image.
    ///         * `uuid`: uuid of the image.
    ///     * `drop_host_mount_globs`: list of strings containing file
    ///     globs. Any mounts globs specified will not be bind mounted
    ///     from the host.
    ///  * `meta_internal_extra_params`: a dictionary to pass extra parameters to RE, can add more keys in the future:
    ///     * `remote_execution_policy`: refer to TExecutionPolicy.
    ///  * `error_handler`: an optional function that analyzes action failures and produces structured error information.
    ///     * Type signature: `def error_handler(ctx: ActionErrorCtx) -> list[ActionSubError]`
    ///     * The function receives an [`ActionErrorCtx`](../ActionErrorCtx) parameter and should return a list of [`ActionSubError`](../ActionSubError) objects
    ///     * Error handlers enable better error diagnostics and language-specific error categorization
    ///  * `outputs_for_error_handler`: Output files to be provided to the action error handler and read by
    /// [error handler](https://kuro.build/docs/api/build/ActionErrorCtx/#actionerrorctxoutput_artifacts) in the event of a failure..
    ///     * The output must also be declared as an output of the action
    ///     * The output artifact must be created if the action fails
    ///     * Nothing will be provided if left empty (Which is the default)
    ///
    /// When actions execute, they'll do so from the root of the repository. As they execute,
    /// actions have exclusive access to their output directory.
    ///
    /// Actions also get exclusive access to a "scratch" path that is exposed via the environment
    /// variable `BUCK_SCRATCH_PATH`. This path is expressed as a path relative to the working
    /// directory (i.e. relative to the project). This path is guaranteed to exist when the action
    /// executes.
    ///
    /// When actions run locally, the scratch path is also used as the `TMPDIR`.
    ///
    /// ### Input and output artifacts
    ///
    /// Run action consumes arbitrary number of input artifacts
    /// and produces at least one output artifact.
    ///
    /// Both input and output artifacts can be passed in:
    /// - positional `arguments` parameters
    /// - `env` dict
    ///
    /// Input artifacts must be already bound prior to this call,
    /// meaning these artifacts must be either:
    /// - source artifacts
    /// - coming from dependencies
    /// - declared locally and bound to another action (passed to `.as_output()`)
    ///   *before* this `run()` call
    /// - or created already bound with some simple action like `write()`
    ///
    /// Output artifacts must be declared locally (within the same analysis),
    /// and must not be already bound. Output artifacts become "bound" after this call.
    fn run<'v>(
        this: &AnalysisActions<'v>,
        // Accept both positional (Buck2) and named (Bazel) argument passing
        arguments: StarlarkCommandLineValueUnpack<'v>,
        #[starlark(require = named, default = NoneOr::None)] category: NoneOr<StringValue<'v>>,
        // Bazel-compatible parameters
        #[starlark(require = named, default = NoneOr::None)] mnemonic: NoneOr<StringValue<'v>>,
        #[starlark(require = named, default = NoneOr::None)] executable: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] inputs: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] outputs: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] progress_message: NoneOr<
            StringValue<'v>,
        >,
        #[starlark(require = named, default = NoneOr::None)] resource_set: NoneOr<Value<'v>>,
        #[starlark(require = named, default = false)] use_default_shell_env: bool,
        #[starlark(require = named, default = NoneOr::None)] execution_requirements: NoneOr<
            Value<'v>,
        >,
        #[starlark(require = named, default = NoneOr::None)] toolchain: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] exec_group: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] tools: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] input_manifests: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] unused_inputs_list: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] shadowed_action: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] identifier: NoneOr<StringValue<'v>>,
        #[starlark(require = named)] env: Option<
            ValueOf<'v, UnpackDictEntries<UnpackAndDiscard<&'v str>, ValueAsCommandLineLike<'v>>>,
        >,
        #[starlark(require = named, default = false)] local_only: bool,
        #[starlark(require = named, default = false)] prefer_local: bool,
        #[starlark(require = named, default = false)] prefer_remote: bool,
        #[starlark(require = named, default = true)] low_pass_filter: bool,
        #[starlark(require = named, default = false)] always_print_stderr: bool,
        #[starlark(require = named)] weight: Option<i32>,
        #[starlark(require = named)] weight_percentage: Option<i32>,
        #[starlark(require = named)] dep_files: Option<SmallMap<&'v str, &'v ArtifactTag>>,
        #[starlark(require = named)] metadata_env_var: Option<String>,
        #[starlark(require = named)] metadata_path: Option<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        incremental_metadata_ignore_tags: UnpackListOrTuple<&'v ArtifactTag>,
        // TODO(scottcao): Refactor `no_outputs_cleanup` to `outputs_cleanup`
        #[starlark(require = named, default = false)] no_outputs_cleanup: bool,
        #[starlark(require = named, default = false)] incremental_remote_outputs: bool,
        #[starlark(require = named, default = NoneOr::None)] allow_cache_upload: NoneOr<bool>,
        #[starlark(require = named, default = false)] allow_dep_file_cache_upload: bool,
        #[starlark(require = named, default = false)] allow_offline_output_cache: bool,
        #[starlark(require = named, default = false)] force_full_hybrid_if_capable: bool,
        #[starlark(require = named)] exe: Option<
            Either<ValueOf<'v, &'v WorkerRunInfo<'v>>, ValueOf<'v, &'v RunInfo<'v>>>,
        >,
        #[starlark(require = named, default = false)] unique_input_inodes: bool,
        #[starlark(require = named, default = NoneOr::None)] error_handler: NoneOr<
            StarlarkCallable<'v>,
        >,
        eval: &mut Evaluator<'v, '_, '_>,
        #[starlark(require = named, default=UnpackList::default())]
        remote_execution_dependencies: UnpackList<SmallMap<&'v str, &'v str>>,
        #[starlark(require = named, default=UnpackList::default())] re_gang_workers: UnpackList<
            SmallMap<&'v str, &'v str>,
        >,
        #[starlark(default = NoneType, require = named)] remote_execution_dynamic_image: Value<'v>,
        #[starlark(require = named, default = NoneOr::None)] meta_internal_extra_params: NoneOr<
            DictRef<'v>,
        >,
        // Note: Intentionally don't support frozen output artifacts
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        outputs_for_error_handler: UnpackListOrTuple<
            ValueTyped<'v, StarlarkOutputArtifact<'v>>,
        >,
        #[starlark(require = named, default = NoneOr::None)] expect_eligible_for_dedupe: NoneOr<
            bool,
        >,
    ) -> starlark::Result<NoneType> {
        if incremental_remote_outputs && !no_outputs_cleanup {
            // Precaution to make sure content-based paths are not involved.
            return Err(kuro_error::Error::from(
                RunActionError::IncrementalRemoteOutputsWithoutNoOutputsCleanup,
            )
            .into());
        }

        struct RunCommandArtifactVisitor<'v> {
            inner: SimpleCommandLineArtifactVisitor<'v>,
            tagged_outputs: HashMap<ArtifactTag, Vec<OutputArtifact<'v>>>,
            depth: u64,
            dep_file_artifact_tags: Option<SmallSet<&'v ArtifactTag>>,
            inputs_with_multiple_tags_for_dep_files: Vec<(ArtifactGroup, Vec<ArtifactTag>)>,
        }

        impl<'v> RunCommandArtifactVisitor<'v> {
            fn new(dep_files: &Option<SmallMap<&'v str, &'v ArtifactTag>>) -> Self {
                let dep_file_artifact_tags = if let Some(dep_files) = dep_files {
                    let mut tags = SmallSet::with_capacity(dep_files.len());
                    for (_key, tag) in dep_files {
                        tags.insert(tag.dupe());
                    }
                    Some(tags)
                } else {
                    None
                };
                Self {
                    inner: SimpleCommandLineArtifactVisitor::new(),
                    tagged_outputs: HashMap::new(),
                    depth: 0,
                    dep_file_artifact_tags,
                    inputs_with_multiple_tags_for_dep_files: Vec::new(),
                }
            }
        }

        impl<'v> CommandLineArtifactVisitor<'v> for RunCommandArtifactVisitor<'v> {
            fn visit_input(&mut self, input: ArtifactGroup, tags: Vec<&ArtifactTag>) {
                if let Some(ref dep_file_artifact_tags) = self.dep_file_artifact_tags {
                    let dep_file_tags: Vec<&ArtifactTag> = tags
                        .iter()
                        .filter_map(|t| {
                            if dep_file_artifact_tags.contains(*t) {
                                Some(*t)
                            } else {
                                None
                            }
                        })
                        .collect();
                    if dep_file_tags.len() > 1 {
                        self.inputs_with_multiple_tags_for_dep_files.push((
                            input.dupe(),
                            dep_file_tags.into_iter().map(|t| t.dupe()).collect(),
                        ));
                    }
                }
                self.inner.visit_input(input, tags);
            }

            fn visit_declared_output(
                &mut self,
                artifact: OutputArtifact<'v>,
                tags: Vec<&ArtifactTag>,
            ) {
                for tag in tags.iter() {
                    self.tagged_outputs
                        .entry((*tag).dupe())
                        .or_default()
                        .push(artifact.dupe());
                }

                self.inner.visit_declared_output(artifact, tags);
            }

            fn visit_frozen_output(&mut self, artifact: Artifact, tags: Vec<&ArtifactTag>) {
                self.inner.visit_frozen_output(artifact, tags)
            }

            /// Bazel compatibility: handle unbound declared artifacts in command line args.
            ///
            /// In Bazel, the same artifact can appear in both `arguments` and `outputs`:
            ///   ctx.actions.run(arguments=[output], outputs=[output], ...)
            /// The default implementation calls `ensure_bound()` which fails for unbound artifacts.
            /// When an artifact is not yet bound, treat it as a declared output rather than an input.
            fn visit_declared_artifact(
                &mut self,
                declared_artifact: DeclaredArtifact<'v>,
                tags: Vec<&ArtifactTag>,
            ) -> kuro_error::Result<()> {
                if declared_artifact.is_bound() {
                    // Bound artifact: treat as input (normal Buck2 behavior)
                    self.visit_input(
                        ArtifactGroup::Artifact(declared_artifact.ensure_bound()?.into_artifact()),
                        tags,
                    );
                } else {
                    // Unbound artifact: treat as declared output (Bazel compatibility)
                    let output: OutputArtifact<'v> = declared_artifact.into();
                    self.visit_declared_output(output, tags);
                }
                Ok(())
            }

            fn push_frame(&mut self) -> kuro_error::Result<()> {
                self.depth += 1;
                if self.depth > 1000 {
                    return Err(RunActionError::ArtifactVisitRecursionLimitExceeded.into());
                }
                Ok(())
            }

            fn pop_frame(&mut self) {
                self.depth = self.depth.saturating_sub(1);
            }
        }

        let executor_preference = new_executor_preference(local_only, prefer_local, prefer_remote)?;

        let mut artifact_visitor = RunCommandArtifactVisitor::new(&dep_files);

        let mut starlark_args = StarlarkCmdArgs::try_from_value_typed(arguments)?;
        starlark_args.visit_artifacts(&mut artifact_visitor)?;

        // TODO(nga): we should not accept output artifacts in worker.
        let (starlark_exe, starlark_worker, starlark_remote_worker) = match exe {
            Some(Either::Left(worker_run)) => {
                let worker = worker_run.typed.worker();
                let remote_worker = worker_run.typed.remote_worker();
                let worker_exe = worker_run.typed.exe();
                worker_exe.as_ref().visit_artifacts(&mut artifact_visitor)?;
                let starlark_exe = StarlarkCmdArgs::try_from_value(worker_exe.to_value())?;
                starlark_exe.visit_artifacts(&mut artifact_visitor)?;
                (starlark_exe, worker, remote_worker)
            }
            Some(Either::Right(exe)) => {
                let starlark_exe = StarlarkCmdArgs::try_from_value(*exe)?;
                starlark_exe.visit_artifacts(&mut artifact_visitor)?;
                (starlark_exe, None, None)
            }
            None => {
                // Bazel compatibility: If no Buck2-style `exe` is provided,
                // check for Bazel-style `executable` parameter.
                // In Bazel, `executable` is prepended to `arguments` to form the command.
                match executable.into_option() {
                    Some(exec_val) => {
                        let starlark_exe = StarlarkCmdArgs::try_from_value(exec_val)?;
                        starlark_exe.visit_artifacts(&mut artifact_visitor)?;
                        (starlark_exe, None, None)
                    }
                    None => (StarlarkCmdArgs::default(), None, None),
                }
            }
        };

        let weight = match (weight, weight_percentage) {
            (None, None) => WeightClass::Permits(1),
            (Some(v), None) => {
                if v < 1 {
                    return Err(kuro_error::Error::from(RunActionError::InvalidWeight(v)).into());
                } else {
                    WeightClass::Permits(v as usize)
                }
            }
            (None, Some(v)) => WeightClass::Percentage(
                WeightPercentage::try_new(v)
                    .map_err(|e| from_any_with_tag(e, kuro_error::ErrorTag::Tier0))
                    .buck_error_context("Invalid `weight_percentage`")?,
            ),
            (Some(..), Some(..)) => {
                return Err(
                    kuro_error::Error::from(RunActionError::DuplicateWeightsSpecified).into(),
                );
            }
        };

        let starlark_env = match &env {
            None => None,
            Some(env) => {
                for (_k, v) in &env.typed.entries {
                    v.0.visit_artifacts(&mut artifact_visitor)?;
                }
                Some(env.as_unchecked().cast())
            }
        };

        let RunCommandArtifactVisitor {
            inner: mut artifacts,
            tagged_outputs,
            inputs_with_multiple_tags_for_dep_files,
            ..
        } = artifact_visitor;

        if let Some(frozen) = { artifacts.frozen_outputs }.pop() {
            return Err(kuro_error::Error::from(ArtifactErrors::DuplicateBind(frozen)).into());
        }

        let mut dep_files_configuration = RunActionDepFiles::new();

        if let Some(dep_files) = dep_files {
            for (key, tag) in dep_files {
                let tagged = tagged_outputs.get(tag);
                let count = tagged.map_or(0, |t| t.len());

                if count != 1 {
                    return Err(
                        kuro_error::Error::from(RunActionError::InvalidDepFileOutputs {
                            key: (*key).to_owned(),
                            count,
                        })
                        .into(),
                    );
                }

                match dep_files_configuration.labels.entry(tag.dupe()) {
                    small_map::Entry::Vacant(v) => {
                        v.insert(Arc::from(key));
                    }
                    small_map::Entry::Occupied(o) => {
                        return Err(
                            kuro_error::Error::from(RunActionError::ConflictingDepFiles {
                                first: (**o.get()).to_owned(),
                                second: (*key).to_owned(),
                            })
                            .into(),
                        );
                    }
                }
            }
        }

        if let Some((input, conflicting_tags)) = inputs_with_multiple_tags_for_dep_files.first() {
            return Err(
                kuro_error::Error::from(RunActionError::ConflictingDepFileInputTags {
                    input: input.dupe(),
                    tags: conflicting_tags
                        .iter()
                        .map(|t| (**dep_files_configuration.labels.get(t).unwrap()).to_owned())
                        .collect(),
                })
                .into(),
            );
        }

        let metadata_param = match (metadata_env_var, metadata_path) {
            (Some(env_var), Some(path)) => {
                let path: ForwardRelativePathBuf = path.try_into()?;
                this.state()?.claim_output_path(eval, &path)?;
                kuro_error::Ok(Some(MetadataParameter {
                    env_var,
                    path,
                    ignore_tags: incremental_metadata_ignore_tags
                        .into_iter()
                        .map(|x| x.dupe())
                        .collect(),
                }))
            }
            (Some(_), None) => Err(RunActionError::MetadataPathMissing.into()),
            (None, Some(_)) => Err(RunActionError::MetadataEnvVarMissing.into()),
            (None, None) => Ok(None),
        }?;

        // Bazel compatibility: process the `outputs` parameter if provided.
        // In Bazel, outputs are passed as a list of File objects to actions.run():
        //   ctx.actions.run(outputs = [out1, out2], ...)
        // These File objects come from ctx.actions.declare_file() calls.
        if let NoneOr::Other(bazel_outputs) = outputs {
            // Try to iterate over the outputs list
            let iter = bazel_outputs.iterate(eval.heap()).map_err(|_| {
                kuro_error::Error::from(RunActionError::OutputsNotIterable(bazel_outputs.to_repr()))
            })?;

            for output_val in iter {
                // StarlarkDeclaredArtifact: Created by ctx.actions.declare_file("name")
                // This is the standard Bazel way to declare output files.
                if let Some(declared) = output_val.downcast_ref::<StarlarkDeclaredArtifact>() {
                    artifacts
                        .declared_outputs
                        .insert(declared.output_artifact());
                }
                // StarlarkOutputArtifact: Created by artifact.as_output()
                // This is the Buck2 way, also supported for compatibility.
                else if let Some(output_artifact) =
                    output_val.downcast_ref::<StarlarkOutputArtifact>()
                {
                    artifacts
                        .declared_outputs
                        .insert(output_artifact.artifact());
                }
                // Unknown type - provide a clear error message
                else {
                    return Err(kuro_error::Error::from(RunActionError::InvalidOutputType(
                        output_val.to_repr(),
                    ))
                    .into());
                }
            }
        }
        if artifacts.declared_outputs.is_empty() {
            return Err(kuro_error::Error::from(RunActionError::NoOutputsSpecified).into());
        }
        let heap = eval.heap();

        for o in outputs_for_error_handler.items.iter() {
            let to_materialize = o.artifact();
            if !artifacts.declared_outputs.contains(&to_materialize) {
                return Err(kuro_error::Error::from(
                    RunActionError::FailedActionArtifactNotDeclared {
                        path: o.to_string(),
                    },
                )
                .into());
            }
        }

        // Use category if provided, otherwise fall back to mnemonic (Bazel compatibility)
        // Convert Bazel mnemonic (PascalCase) to Kuro category (snake_case)
        let effective_category = match (category.into_option(), mnemonic.into_option()) {
            (Some(c), _) => c,
            (None, Some(m)) => {
                // Convert PascalCase mnemonic to snake_case category
                let mnemonic_str = m.as_str();
                let snake_case: String = mnemonic_str
                    .chars()
                    .enumerate()
                    .flat_map(|(i, c)| {
                        if c.is_uppercase() && i > 0 {
                            vec!['_', c.to_ascii_lowercase()]
                        } else {
                            vec![c.to_ascii_lowercase()]
                        }
                    })
                    .collect();
                heap.alloc_str(&snake_case)
            }
            (None, None) => heap.alloc_str("action"), // Default fallback
        };

        // Bazel compatibility: process the `inputs` parameter if provided.
        // In Bazel, inputs specify the dependencies of an action:
        //   ctx.actions.run(inputs = depset([artifact1, artifact2]), ...)
        // These input artifacts must be added to establish proper action dependencies.

        // Debug logging for inputs processing
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "[actions.run] inputs parameter: {:?}, category: {}",
                match &inputs {
                    NoneOr::None => "None".to_string(),
                    NoneOr::Other(v) => format!("Some({})", v),
                },
                effective_category
            );
        }

        // Collect bazel inputs to pass to StarlarkRunActionValues for dependency tracking
        let mut collected_bazel_inputs: Vec<Value<'v>> = vec![];

        if let NoneOr::Other(bazel_inputs) = inputs {
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/cc_common_compile.log")
            {
                let _ = writeln!(
                    f,
                    "  inputs type: {}, value: {}",
                    bazel_inputs.get_type(),
                    bazel_inputs
                );
            }

            // Try to get the to_list method (for depset) or iterate directly (for list)
            let items_to_process: Vec<Value<'v>> =
                if let Ok(Some(to_list)) = bazel_inputs.get_attr("to_list", eval.heap()) {
                    // It's a depset - call to_list() to get the items
                    if let Ok(list_val) = eval.eval_function(to_list, &[], &[]) {
                        if let Ok(iter) = list_val.iterate(eval.heap()) {
                            iter.collect()
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                } else if let Ok(iter) = bazel_inputs.iterate(eval.heap()) {
                    // It's already iterable (list, tuple, etc.)
                    iter.collect()
                } else {
                    // Single value - wrap in vec
                    vec![bazel_inputs]
                };

            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/cc_common_compile.log")
            {
                let _ = writeln!(f, "  items_to_process count: {}", items_to_process.len());
            }

            // Collect the bazel inputs for passing to StarlarkRunActionValues
            // This allows visit_artifacts to include them as dependencies
            collected_bazel_inputs = items_to_process.clone();

            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/cc_common_compile.log")
            {
                let _ = writeln!(
                    f,
                    "  collected_bazel_inputs count: {}",
                    collected_bazel_inputs.len()
                );
                for (i, v) in collected_bazel_inputs.iter().enumerate() {
                    let _ = writeln!(f, "    [{}] type={}, value={}", i, v.get_type(), v);
                }
            }

            // Create a cmd_args with the input artifacts as hidden dependencies
            // This ensures they are tracked as dependencies without appearing on the command line
            if !items_to_process.is_empty() {
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/cc_common_compile.log")
                {
                    let _ = writeln!(f, "  attempting to add hidden inputs to cmd_args");
                }

                // Get the cmd_args function from the module
                let cmd_args_fn_opt = eval.module().get("cmd_args");
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/cc_common_compile.log")
                {
                    let _ = writeln!(
                        f,
                        "  cmd_args function lookup: {:?}",
                        cmd_args_fn_opt.map(|v| v.to_string())
                    );
                }

                if let Some(cmd_args_fn) = cmd_args_fn_opt {
                    // Create a list of the input artifacts
                    let input_list = heap.alloc(items_to_process.clone());

                    // Call cmd_args(hidden=inputs) to create a cmd_args with hidden inputs
                    if let Ok(hidden_cmd) =
                        eval.eval_function(cmd_args_fn, &[], &[("hidden", input_list)])
                    {
                        if let Ok(mut f) = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open("/tmp/cc_common_compile.log")
                        {
                            let _ = writeln!(f, "  created hidden cmd_args: {}", hidden_cmd);
                        }

                        // Add this hidden cmd_args to the main args so it's included in visit_artifacts
                        // First allocate starlark_args to get a Value, then call .add() on it
                        let args_value = heap.alloc_complex(std::mem::take(&mut starlark_args));
                        if let Ok(Some(add_method)) = args_value.get_attr("add", heap) {
                            let add_result = eval.eval_function(add_method, &[hidden_cmd], &[]);
                            if let Ok(mut f) = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open("/tmp/cc_common_compile.log")
                            {
                                let _ = writeln!(
                                    f,
                                    "  add result: {:?}",
                                    add_result.as_ref().map(|v| v.to_string())
                                );
                            }
                            // Update starlark_args with the result if successful
                            if let Ok(new_args) = add_result {
                                if let Ok(typed_args) = StarlarkCmdArgs::try_from_value(new_args) {
                                    starlark_args = typed_args;
                                }
                            }
                        } else {
                            // Restore starlark_args if we couldn't call add
                            if let Ok(restored) = StarlarkCmdArgs::try_from_value(args_value) {
                                starlark_args = restored;
                            }
                        }
                    }
                }
            }

            for input_val in items_to_process {
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/cc_common_compile.log")
                {
                    let _ = writeln!(
                        f,
                        "    processing input: type={}, value={}",
                        input_val.get_type(),
                        input_val
                    );
                }

                // Try to extract an artifact from the input value and add it as an input dependency
                // StarlarkArtifact: A frozen/bound artifact (source or build artifact)
                if let Some(artifact) = input_val.downcast_ref::<StarlarkArtifact>() {
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("/tmp/cc_common_compile.log")
                    {
                        let _ = writeln!(f, "      -> StarlarkArtifact: added as input");
                    }
                    artifacts
                        .inputs
                        .insert(ArtifactGroup::Artifact(artifact.artifact().dupe()));
                }
                // StarlarkDeclaredArtifact: An artifact declared in the current analysis
                // When used as an input, this creates a dependency on the action that produces it
                else if let Some(declared) = input_val.downcast_ref::<StarlarkDeclaredArtifact>()
                {
                    // Use get_artifact_group() to get the artifact and establish dependency
                    if let Ok(artifact_group) = declared.get_artifact_group() {
                        if let Ok(mut f) = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open("/tmp/cc_common_compile.log")
                        {
                            let _ =
                                writeln!(f, "      -> StarlarkDeclaredArtifact: added as input");
                        }
                        artifacts.inputs.insert(artifact_group);
                    }
                }
                // Note: StarlarkOutputArtifact is typically not expected in `inputs` parameter,
                // as it represents outputs of an action. If needed, it can be handled by getting
                // the underlying artifact via the Starlark as_input() method.
                else {
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("/tmp/cc_common_compile.log")
                    {
                        let _ = writeln!(f, "      -> unrecognized type, ignored");
                    }
                }
            }
        }

        // Bazel compatibility: process the `tools` parameter if provided.
        // Tools are extra executable binaries needed by the action (e.g., protoc plugins).
        // They must be added as bazel_inputs so visit_artifacts tracks them as dependencies.
        if let NoneOr::Other(bazel_tools) = tools {
            let tool_items: Vec<Value<'v>> =
                if let Ok(Some(to_list)) = bazel_tools.get_attr("to_list", eval.heap()) {
                    if let Ok(list_val) = eval.eval_function(to_list, &[], &[]) {
                        if let Ok(iter) = list_val.iterate(eval.heap()) {
                            iter.collect()
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                } else if let Ok(iter) = bazel_tools.iterate(eval.heap()) {
                    iter.collect()
                } else {
                    vec![bazel_tools]
                };

            collected_bazel_inputs.extend(tool_items);
        }

        // Ignore other Bazel-specific parameters that are not yet implemented.
        let _ = (
            progress_message,
            resource_set,
            use_default_shell_env,
            execution_requirements,
            toolchain,
            exec_group,
            input_manifests,
            unused_inputs_list,
            shadowed_action,
        );

        let starlark_values = heap.alloc_complex(StarlarkRunActionValues {
            exe: heap.alloc_typed(starlark_exe),
            args: heap.alloc_typed(starlark_args),
            env: starlark_env,
            worker: starlark_worker,
            remote_worker: starlark_remote_worker,
            category: {
                CategoryRef::new(effective_category.as_str())?;
                effective_category
            },
            identifier: identifier.into_option(),
            outputs_for_error_handler: outputs_for_error_handler.items,
            // Bazel compatibility: track extra input artifacts from the `inputs` parameter
            bazel_inputs: collected_bazel_inputs,
        });

        let re_dependencies = remote_execution_dependencies
            .into_iter()
            .map(RemoteExecutorDependency::parse)
            .collect::<kuro_error::Result<Vec<RemoteExecutorDependency>>>()?;

        let re_gang_workers = re_gang_workers
            .into_iter()
            .map(ReGangWorker::parse)
            .collect::<kuro_error::Result<Vec<ReGangWorker>>>()?;

        let re_custom_image = parse_custom_re_image(
            "remote_execution_dynamic_image",
            remote_execution_dynamic_image,
        )?;

        let extra_params =
            parse_meta_internal_extra_params(meta_internal_extra_params.into_option())?;

        if incremental_remote_outputs {
            for o in artifacts.declared_outputs.iter() {
                if o.has_content_based_path() {
                    return Err(kuro_error::Error::from(
                        RunActionError::IncrementalRemoteOutputsWithContentBasedOutputs {
                            path: o.get_path().to_string(),
                        },
                    )
                    .into());
                }
            }
        }

        let action = UnregisteredRunAction {
            executor_preference,
            always_print_stderr,
            weight,
            low_pass_filter,
            dep_files: dep_files_configuration,
            metadata_param,
            no_outputs_cleanup,
            incremental_remote_outputs,
            allow_cache_upload: allow_cache_upload.into_option(),
            allow_dep_file_cache_upload,
            allow_offline_output_cache,
            force_full_hybrid_if_capable,
            unique_input_inodes,
            remote_execution_dependencies: re_dependencies,
            re_gang_workers,
            remote_execution_custom_image: re_custom_image,
            meta_internal_extra_params: extra_params,
            expected_eligible_for_dedupe: expect_eligible_for_dedupe.into_option(),
        };

        if expect_eligible_for_dedupe.into_option().unwrap_or(false) {
            for o in artifacts.declared_outputs.iter() {
                if !o.has_content_based_path() {
                    return Err(kuro_error::Error::from(
                        RunActionError::ExpectEligibleForDedupeWithNonContentBasedOutput {
                            path: o.get_path().to_string(),
                        },
                    )
                    .into());
                }
            }
            let deferred_holder_key = &this.state()?.analysis_value_storage.self_key;
            let target_platform = if let BaseDeferredKey::TargetLabel(configured_label) =
                deferred_holder_key.owner()
            {
                Some(configured_label.cfg())
            } else {
                None
            };

            for i in artifacts.inputs.iter() {
                if !i.is_eligible_for_dedupe(target_platform) {
                    return Err(kuro_error::Error::from(
                        RunActionError::ExpectEligibleForDedupeWithIneligibleInput {
                            input: i.dupe(),
                        },
                    )
                    .into());
                }
            }
        }

        this.state()?.register_action(
            artifacts.declared_outputs,
            action,
            Some(starlark_values),
            error_handler.into_option(),
        )?;
        Ok(NoneType)
    }

    /// Run a shell command to produce one or more artifacts (Bazel-compatible).
    ///
    /// This is the Bazel equivalent of `ctx.actions.run()` but takes a shell command
    /// string (executed via `bash -c`) or a list of strings (executed directly).
    ///
    /// Parameters:
    /// - `outputs`: Required list of output files this action will produce
    /// - `command`: Required shell command (string for `bash -c`, or list of strings)
    /// - `inputs`: Optional input files (list or depset)
    /// - `tools`: Optional tool dependencies (list or depset)
    /// - `arguments`: Optional additional arguments appended to the command (string case)
    /// - `env`: Optional dict of environment variables
    /// - `mnemonic`: Optional short action description used as category
    fn run_shell<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = named)] outputs: Value<'v>,
        #[starlark(require = named)] command: Value<'v>,
        #[starlark(require = named, default = NoneOr::None)] inputs: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] tools: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] arguments: NoneOr<Value<'v>>,
        #[starlark(require = named)] env: Option<
            ValueOf<'v, UnpackDictEntries<UnpackAndDiscard<&'v str>, ValueAsCommandLineLike<'v>>>,
        >,
        #[starlark(require = named, default = NoneOr::None)] mnemonic: NoneOr<StringValue<'v>>,
        #[starlark(require = named, default = NoneOr::None)] progress_message: NoneOr<Value<'v>>,
        #[starlark(require = named, default = false)] use_default_shell_env: bool,
        #[starlark(require = named, default = NoneOr::None)] execution_requirements: NoneOr<
            Value<'v>,
        >,
        #[starlark(require = named, default = NoneOr::None)] input_manifests: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] exec_group: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] shadowed_action: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] resource_set: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] toolchain: NoneOr<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = (
            progress_message,
            use_default_shell_env,
            execution_requirements,
            input_manifests,
            exec_group,
            shadowed_action,
            resource_set,
            toolchain,
        );
        let heap = eval.heap();

        // Build the exe and args command lines:
        // - String command: exe = ["/bin/bash", "-c"], args = [cmd_str, ...extra_arguments]
        // - List command: exe = the list, args = extra_arguments
        let (starlark_exe, starlark_args) = if let Some(cmd_str) = command.unpack_str() {
            let exe_list = heap.alloc(vec![
                heap.alloc_str("/bin/bash").to_value(),
                heap.alloc_str("-c").to_value(),
            ]);
            let starlark_exe = StarlarkCmdArgs::try_from_value(exe_list)
                .buck_error_context("run_shell: building bash exe")?;

            // In Bazel, run_shell with a string command and `arguments` passes them as
            // $1, $2, ... to the shell script. The bash -c invocation is:
            //   bash -c 'script' $0_placeholder arg1 arg2 ...
            // where $0_placeholder (the "script name") occupies $0, and the actual
            // arguments start at $1. We use an empty string as the $0 placeholder.
            let mut args_items = vec![
                heap.alloc_str(cmd_str).to_value(),
                heap.alloc_str("").to_value(), // placeholder for $0 (bash script name)
            ];
            if let NoneOr::Other(extra_args) = arguments {
                if let Ok(iter) = extra_args.iterate(heap) {
                    args_items.extend(iter);
                }
            }
            let args_list = heap.alloc(args_items);
            let starlark_args = StarlarkCmdArgs::try_from_value(args_list)
                .buck_error_context("run_shell: building shell args")?;
            (starlark_exe, starlark_args)
        } else {
            // List or cmd_args: use directly as the exe command
            let starlark_exe = StarlarkCmdArgs::try_from_value(command)
                .buck_error_context("run_shell: building command from list")?;
            let starlark_args = if let NoneOr::Other(extra_args) = arguments {
                StarlarkCmdArgs::try_from_value(extra_args)
                    .buck_error_context("run_shell: building extra arguments")?
            } else {
                StarlarkCmdArgs::default()
            };
            (starlark_exe, starlark_args)
        };

        let mut artifact_visitor = SimpleCommandLineArtifactVisitor::new();
        starlark_exe.visit_artifacts(&mut artifact_visitor)?;
        starlark_args.visit_artifacts(&mut artifact_visitor)?;

        // Process declared outputs
        let outputs_iter = outputs.iterate(heap).map_err(|_| {
            kuro_error::Error::from(RunActionError::OutputsNotIterable(outputs.to_repr()))
        })?;
        for output_val in outputs_iter {
            if let Some(declared) = output_val.downcast_ref::<StarlarkDeclaredArtifact>() {
                artifact_visitor
                    .declared_outputs
                    .insert(declared.output_artifact());
            } else if let Some(output_artifact) =
                output_val.downcast_ref::<StarlarkOutputArtifact>()
            {
                artifact_visitor
                    .declared_outputs
                    .insert(output_artifact.artifact());
            } else {
                return Err(kuro_error::Error::from(RunActionError::InvalidOutputType(
                    output_val.to_repr(),
                ))
                .into());
            }
        }
        if artifact_visitor.declared_outputs.is_empty() {
            return Err(kuro_error::Error::from(RunActionError::NoOutputsSpecified).into());
        }

        // Collect bazel_inputs for dependency tracking (inputs and tools)
        let mut bazel_inputs: Vec<Value<'v>> = vec![];

        if let NoneOr::Other(inputs_val) = inputs {
            let items = collect_items_from_value(inputs_val, eval)?;
            for input_val in items {
                bazel_inputs.push(input_val);
                if let Some(artifact) = input_val.downcast_ref::<StarlarkArtifact>() {
                    artifact_visitor
                        .inputs
                        .insert(ArtifactGroup::Artifact(artifact.artifact().dupe()));
                } else if let Some(declared) = input_val.downcast_ref::<StarlarkDeclaredArtifact>()
                {
                    if let Ok(group) = declared.get_artifact_group() {
                        artifact_visitor.inputs.insert(group);
                    }
                }
            }
        }

        if let NoneOr::Other(tools_val) = tools {
            let items = collect_items_from_value(tools_val, eval)?;
            for tool_val in items {
                bazel_inputs.push(tool_val);
                if let Some(artifact) = tool_val.downcast_ref::<StarlarkArtifact>() {
                    artifact_visitor
                        .inputs
                        .insert(ArtifactGroup::Artifact(artifact.artifact().dupe()));
                }
            }
        }

        // Determine category from mnemonic (convert PascalCase to snake_case)
        let effective_category = match mnemonic.into_option() {
            Some(m) => {
                let snake_case: String = m
                    .as_str()
                    .chars()
                    .enumerate()
                    .flat_map(|(i, c)| {
                        if c.is_uppercase() && i > 0 {
                            vec!['_', c.to_ascii_lowercase()]
                        } else {
                            vec![c.to_ascii_lowercase()]
                        }
                    })
                    .collect();
                heap.alloc_str(&snake_case)
            }
            None => heap.alloc_str("run_shell"),
        };
        CategoryRef::new(effective_category.as_str())?;

        // Process environment variables
        let starlark_env = match &env {
            None => None,
            Some(env_val) => {
                for (_k, v) in &env_val.typed.entries {
                    v.0.visit_artifacts(&mut artifact_visitor)?;
                }
                Some(env_val.as_unchecked().cast())
            }
        };

        let starlark_values = heap.alloc_complex(StarlarkRunActionValues {
            exe: heap.alloc_typed(starlark_exe),
            args: heap.alloc_typed(starlark_args),
            env: starlark_env,
            worker: None,
            remote_worker: None,
            category: effective_category,
            identifier: None,
            outputs_for_error_handler: vec![],
            bazel_inputs,
        });

        let action = UnregisteredRunAction {
            executor_preference: new_executor_preference(false, false, false)?,
            always_print_stderr: false,
            weight: WeightClass::Permits(1),
            low_pass_filter: true,
            dep_files: RunActionDepFiles::new(),
            metadata_param: None,
            no_outputs_cleanup: false,
            incremental_remote_outputs: false,
            allow_cache_upload: None,
            allow_dep_file_cache_upload: false,
            allow_offline_output_cache: false,
            force_full_hybrid_if_capable: false,
            unique_input_inodes: false,
            remote_execution_dependencies: vec![],
            re_gang_workers: vec![],
            remote_execution_custom_image: None,
            meta_internal_extra_params: MetaInternalExtraParams::default(),
            expected_eligible_for_dedupe: None,
        };

        this.state()?.register_action(
            artifact_visitor.declared_outputs,
            action,
            Some(starlark_values),
            None,
        )?;
        Ok(NoneType)
    }
}

/// Collect items from a depset, list, or single value into a Vec<Value<'v>>.
fn collect_items_from_value<'v>(
    val: Value<'v>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> kuro_error::Result<Vec<Value<'v>>> {
    // Check if it's a depset (has to_list method)
    if let Ok(Some(to_list)) = val.get_attr("to_list", eval.heap()) {
        if let Ok(list_val) = eval.eval_function(to_list, &[], &[]) {
            if let Ok(iter) = list_val.iterate(eval.heap()) {
                return Ok(iter.collect());
            }
        }
    }
    // Try to iterate (list, tuple, other iterables)
    if let Ok(iter) = val.iterate(eval.heap()) {
        return Ok(iter.collect());
    }
    // Single value
    Ok(vec![val])
}
