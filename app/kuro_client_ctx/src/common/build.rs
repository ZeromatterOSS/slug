/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use clap::ArgGroup;
use clap::builder::FalseyValueParser;
use kuro_cli_proto::common_build_options::ExecutionStrategy;
use kuro_core::kuro_env_name;
use kuro_error::conversion::clap::buck_error_clap_parser;
use tracing::warn;

use crate::common::PrintOutputsFormat;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct BuildReportOption {
    /// Fill out the failures in build report as it was done by default in buck1.
    fill_out_failures: bool,

    /// Include package relative paths in the output.
    include_package_project_relative_paths: bool,

    /// Include artifact hash information in the output.
    include_artifact_hash_information: bool,
}

fn parse_build_report_option(s: &str) -> kuro_error::Result<BuildReportOption> {
    let mut fill_out_failures = false;
    let mut include_package_project_relative_paths = false;
    let mut include_artifact_hash_information = false;

    if s.to_lowercase() == "fill-out-failures" {
        fill_out_failures = true;
    } else if s.to_lowercase() == "package-project-relative-paths" {
        include_package_project_relative_paths = true;
    } else if s.to_lowercase() == "include-artifact-hash-information" {
        include_artifact_hash_information = true;
    } else {
        warn!(
            "Incorrect syntax for build report option. Got: `{}` but expected one of `fill-out-failures, package-project-relative-paths`",
            s.to_owned()
        )
    }
    Ok(BuildReportOption {
        fill_out_failures,
        include_package_project_relative_paths,
        include_artifact_hash_information,
    })
}

/// Defines common options for build-like commands (build, test, install).
#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize)]
pub struct CommonBuildOptions {
    /// Print a build report
    ///
    /// `--build-report=-` will print the build report to stdout
    /// `--build-report=<filepath>` will write the build report to the file
    #[clap(long = "build-report", value_name = "PATH")]
    build_report: Option<String>,

    /// Comma separated list of validation names to run that are marked optional.
    ///
    /// By default, validations marked as optional are skipped. This option overrides the behaviour and executes those validations.
    #[clap(long, value_name = "VALIDATION_NAMES", value_delimiter = ',')]
    enable_optional_validations: Vec<String>,

    /// Comma separated list of build report options.
    ///
    /// The following options are supported:
    ///
    /// `fill-out-failures`:
    /// fill out failures the same way Buck1 would.
    ///
    /// `package-project-relative-paths`:
    /// emit the project-relative path of packages for the targets that were built.
    #[clap(
        long = "build-report-options",
        requires = "build_report",
        value_delimiter = ',',
        value_parser = buck_error_clap_parser(parse_build_report_option),
    )]
    build_report_options: Vec<BuildReportOption>,

    /// Stream intermediary build reports to a file in json lines format.
    ///
    /// Each output materialization will trigger a new build report which
    /// will be written to the file as a single line json.
    #[clap(long = "streaming-build-report", value_name = "PATH")]
    streaming_build_report: Option<String>,

    /// Number of threads to use during execution (default is # cores)
    // TODO(cjhopman): This only limits the threads used for action execution and it doesn't work correctly with concurrent commands.
    #[clap(
        short = 'j',
        long = "num-threads",
        alias = "jobs",  // Bazel compatibility: --jobs=N
        value_name = "THREADS"
    )]
    pub num_threads: Option<u32>,

    /// Enable only local execution. Will reject actions that cannot execute locally.
    #[clap(long, group = "build_strategy", env = kuro_env_name!("BUCK_OFFLINE_BUILD"), value_parser = FalseyValueParser::new())]
    local_only: bool,

    /// Enable only remote execution. Will reject actions that cannot execute remotely.
    #[clap(long, group = "build_strategy")]
    remote_only: bool,

    /// Enable hybrid execution. Will prefer executing actions that can execute locally on the
    /// local host.
    #[clap(long, group = "build_strategy")]
    prefer_local: bool,

    /// Enable hybrid execution. Will prefer executing actions that can execute remotely on RE and will avoid racing local and remote execution.
    #[clap(long, group = "build_strategy", env = kuro_env_name!("BUCK_PREFER_REMOTE"), value_parser = FalseyValueParser::new())]
    prefer_remote: bool,

    /// Experimental: Disable all execution.
    #[clap(long, group = "build_strategy")]
    unstable_no_execution: bool,

    /// Do not perform remote cache queries or cache writes. If remote execution is enabled, the RE
    /// service might still deduplicate actions, so for e.g. benchmarking, using a random isolation
    /// dir is preferred.
    #[clap(long, env = kuro_env_name!("BUCK_OFFLINE_BUILD"), value_parser = FalseyValueParser::new())]
    no_remote_cache: bool,

    /// Could be used to enable the action cache writes on the RE worker when no_remote_cache is specified
    #[clap(long, requires = "no_remote_cache")]
    write_to_cache_anyway: bool,

    /// Process dep files when they are generated (i.e. after running a command that produces dep
    /// files), rather than when they are used (i.e. before re-running a command that previously
    /// produced dep files). Use this when debugging commands that produce dep files. Note that
    /// commands that previously produced dep files will not re-run: only dep files produced during
    /// this command will be eagerly loaded.
    #[clap(long)]
    eager_dep_files: bool,

    /// Uploads every action to the RE service, regardless of whether the action needs to execute on RE.
    ///
    /// This is useful when debugging builds and trying to inspect actions which executed remotely.
    /// It's possible that the action result is cached but the action itself has expired. In this case,
    /// downloading the action itself would fail. Enabling this option would unconditionally upload
    /// all actions, thus you will not hit any expiration issues.
    #[clap(long)]
    upload_all_actions: bool,

    /// If Buck hits an error, do as little work as possible before exiting.
    ///
    /// To illustrate the effect of this flag, consider an invocation of `build :foo :bar`. The
    /// default behavior of buck is to do enough work to get a result for the builds of each of
    /// `:foo` and `:bar`, and no more. This means that buck will continue to complete the build of
    /// `:bar` after the build of `:foo` has failed; however, once one dependency of `:foo` has
    /// failed, other dependencies will be cancelled unless they are needed by `:bar`.
    ///
    /// This flag changes the behavior of buck to not wait on `:bar` to complete once `:foo` has
    /// failed. Generally, this flag only has an effect on builds that specify multiple targets.
    ///
    /// `--keep-going` changes the behavior of buck to not only wait on `:bar` once one dependency
    /// of `:foo` has failed, but to additionally attempt to build other dependencies of `:foo` if
    /// possible.
    // Alias fail_fast for Bazel compatibility (Bazel uses underscores; both forms are accepted).
    #[clap(long, alias = "fail_fast", group = "fail-when")]
    fail_fast: bool,

    /// If Buck hits an error, continue doing as much work as possible before exiting.
    ///
    /// See `--fail-fast` for more details.
    // Alias keep_going for Bazel compatibility (Bazel uses underscores; both forms are accepted).
    #[clap(long, alias = "keep_going", group = "fail-when")]
    keep_going: bool,

    /// If target is missing, then skip building instead of throwing error.
    #[clap(long)]
    skip_missing_targets: bool,

    /// If target is incompatible with the specified configuration, skip building instead of throwing error.
    /// This does not apply to targets specified with glob patterns `/...` or `:`
    /// which are skipped unconditionally.
    #[clap(long)]
    skip_incompatible_targets: bool,

    /// Materializes inputs for failed actions which ran on RE
    #[clap(long)]
    materialize_failed_inputs: bool,

    /// Materializes outputs (if present) for failed actions which ran on RE
    #[clap(long)]
    materialize_failed_outputs: bool,

    /// Enable filesystem sandboxing for local actions.
    /// When enabled, actions run in an isolated mount namespace where the filesystem
    /// is read-only except for declared output directories. Overrides [sandbox] enabled
    /// in .buckconfig. Currently only supported on Linux (no-op on other platforms).
    #[clap(long, overrides_with = "nosandbox")]
    sandbox: bool,

    /// Disable filesystem sandboxing (overrides --sandbox and [sandbox] enabled in .buckconfig).
    #[clap(long, overrides_with = "sandbox")]
    nosandbox: bool,

    // ---- Bazel compatibility flags (accepted, some are no-ops) ----
    /// Print the full build command when an action fails (Bazel compatibility).
    ///
    /// Accepted for compatibility with Bazel's --verbose_failures flag.
    /// Kuro already prints failure details; this flag is accepted but currently
    /// does not change the output format.
    #[clap(long = "verbose-failures", alias = "verbose_failures", hide = true)]
    pub verbose_failures: bool,

    /// Set user-defined build variables (Bazel compatibility).
    ///
    /// Values are in KEY=VALUE format. Accepted for compatibility with Bazel's
    /// --define flag. Can be used with config_setting(define_values = {...}).
    /// Currently accepted but not yet wired to select() evaluation.
    #[clap(long = "define", hide = true, value_name = "KEY=VALUE")]
    pub define: Vec<String>,

    /// Pass environment variable to build actions (Bazel compatibility).
    ///
    /// Values are in NAME or NAME=VALUE format. Accepted for compatibility with
    /// Bazel's --action_env flag. Currently accepted but not propagated to actions.
    #[clap(
        long = "action-env",
        alias = "action_env",
        hide = true,
        value_name = "NAME[=VALUE]"
    )]
    pub action_env: Vec<String>,

    /// Pass environment variable to host actions (Bazel compatibility).
    ///
    /// Accepted for compatibility with Bazel's --host_action_env flag.
    #[clap(
        long = "host-action-env",
        alias = "host_action_env",
        hide = true,
        value_name = "NAME[=VALUE]"
    )]
    pub host_action_env: Vec<String>,

    /// Enable build stamping (Bazel compatibility).
    ///
    /// When enabled, Bazel embeds build information (timestamp, version, etc.)
    /// via ctx.info_file and ctx.version_file. Accepted for compatibility.
    #[clap(long = "stamp", hide = true, overrides_with = "nostamp")]
    pub stamp: bool,

    /// Disable build stamping (Bazel compatibility). This is the default.
    ///
    /// Accepted for compatibility with Bazel's --nostamp flag.
    #[clap(long = "nostamp", hide = true, overrides_with = "stamp")]
    pub nostamp: bool,

    /// Number of jobs for remote execution (Bazel compatibility).
    ///
    /// This is distinct from `--jobs`/`--num-threads`. In Bazel, `--jobs`
    /// controls local parallelism while this controls RE concurrency.
    /// Accepted for compatibility.
    #[clap(
        long = "remote-num-threads",
        alias = "remote_num_threads",
        hide = true,
        value_name = "N"
    )]
    pub remote_num_threads: Option<u32>,

    /// Use aspect-like flag (Bazel compatibility).
    ///
    /// Accepted for compatibility with Bazel's --aspects flag. In Bazel, this
    /// applies aspects to all targets. Currently accepted but not implemented.
    #[clap(long = "aspects", hide = true, value_name = "ASPECT_LABEL")]
    pub aspects: Vec<String>,

    /// Set toolchain type for execution (Bazel compatibility).
    ///
    /// Accepted for compatibility with Bazel's --extra_toolchains flag.
    #[clap(
        long = "extra-toolchains",
        alias = "extra_toolchains",
        hide = true,
        value_name = "TOOLCHAIN_LABEL"
    )]
    pub extra_toolchains: Vec<String>,

    /// Set execution platforms (Bazel compatibility).
    ///
    /// Accepted for compatibility with Bazel's --extra_execution_platforms flag.
    #[clap(
        long = "extra-execution-platforms",
        alias = "extra_execution_platforms",
        hide = true,
        value_name = "PLATFORM_LABEL"
    )]
    pub extra_execution_platforms: Vec<String>,
}

impl CommonBuildOptions {
    fn build_report(&self) -> (bool, String) {
        match &self.build_report {
            None => (false, "".to_owned()),
            Some(path) if path != "-" => (true, path.to_owned()),
            _ => (true, "".to_owned()),
        }
    }

    pub fn to_proto(&self) -> kuro_cli_proto::CommonBuildOptions {
        let (unstable_print_build_report, unstable_build_report_filename) = self.build_report();
        let unstable_streaming_build_report_filename =
            self.streaming_build_report.clone().unwrap_or_default();
        let unstable_include_failures_build_report = self
            .build_report_options
            .iter()
            .any(|option| option.fill_out_failures);
        let unstable_include_package_project_relative_paths = self
            .build_report_options
            .iter()
            .any(|option| option.include_package_project_relative_paths);
        let unstable_include_artifact_hash_information = self
            .build_report_options
            .iter()
            .any(|option| option.include_artifact_hash_information);
        let concurrency = self
            .num_threads
            .map(|num| kuro_cli_proto::Concurrency { concurrency: num });
        let enable_optional_validations = self
            .enable_optional_validations
            .iter()
            .map(|s| s.to_owned())
            .collect();

        kuro_cli_proto::CommonBuildOptions {
            concurrency,
            execution_strategy: if self.local_only {
                ExecutionStrategy::LocalOnly as i32
            } else if self.remote_only {
                ExecutionStrategy::RemoteOnly as i32
            } else if self.prefer_local {
                ExecutionStrategy::HybridPreferLocal as i32
            } else if self.prefer_remote {
                ExecutionStrategy::HybridPreferRemote as i32
            } else if self.unstable_no_execution {
                ExecutionStrategy::NoExecution as i32
            } else {
                ExecutionStrategy::Default as i32
            },
            unstable_print_build_report,
            unstable_build_report_filename,
            eager_dep_files: self.eager_dep_files,
            upload_all_actions: self.upload_all_actions,
            skip_cache_read: self.no_remote_cache,
            skip_cache_write: self.no_remote_cache && !self.write_to_cache_anyway,
            fail_fast: self.fail_fast,
            keep_going: self.keep_going,
            skip_missing_targets: self.skip_missing_targets,
            skip_incompatible_targets: self.skip_incompatible_targets,
            materialize_failed_inputs: self.materialize_failed_inputs,
            enable_optional_validations,
            materialize_failed_outputs: self.materialize_failed_outputs,
            unstable_include_failures_build_report,
            unstable_include_package_project_relative_paths,
            unstable_include_artifact_hash_information,
            unstable_streaming_build_report_filename,
            // If --sandbox or --nosandbox was given, send explicit override.
            // Otherwise None means "use server config ([sandbox] enabled)".
            sandbox_enabled: if self.sandbox {
                Some(true)
            } else if self.nosandbox {
                Some(false)
            } else {
                None
            },
        }
    }
}

/// Show-output options shared by `build` and `targets`.
#[derive(Debug, clap::Parser)]
#[clap(group(
    // Make mutually exclusive. A command may have at most one of the flags in
    // the following group.
    ArgGroup::new("output_args").args(&[
        "show_output",
        "show_full_output",
        "show_simple_output",
        "show_full_simple_output",
        "show_json_output",
        "show_full_json_output",
    ])
))]
pub struct CommonOutputOptions {
    /// Print the path to the output for each of the rules relative to the project root
    #[clap(long)]
    pub show_output: bool,

    /// Print the absolute path to the output for each of the rules
    #[clap(long)]
    pub show_full_output: bool,

    /// Print only the path to the output for each of the rules relative to the project root
    #[clap(long)]
    pub show_simple_output: bool,

    /// Print only the absolute path to the output for each of the rules
    #[clap(long)]
    pub show_full_simple_output: bool,

    /// Print the output paths relative to the project root, in JSON format
    #[clap(long)]
    pub show_json_output: bool,

    /// Print the output absolute paths, in JSON format
    #[clap(long)]
    pub show_full_json_output: bool,
}

impl CommonOutputOptions {
    pub fn format(&self) -> Option<PrintOutputsFormat> {
        if self.show_output || self.show_full_output {
            Some(PrintOutputsFormat::Plain)
        } else if self.show_simple_output || self.show_full_simple_output {
            Some(PrintOutputsFormat::Simple)
        } else if self.show_json_output || self.show_full_json_output {
            Some(PrintOutputsFormat::Json)
        } else {
            None
        }
    }

    pub fn is_full(&self) -> bool {
        self.show_full_output || self.show_full_simple_output || self.show_full_json_output
    }
}
