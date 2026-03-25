/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! This modules contains common options that are shared between different commands.
//! They are shared by composition together with flattening of the options.
//!
//! For example, to adopt config options, add the following field to the
//! command definition:
//!
//! ```ignore
//! #[derive(Debug, clap::Parser)]
//! struct MyCommand {
//!    #[clap(flatten)]
//!    config_opts: CommonConfigOptions,
//!    ...
//! }
//! ```

pub mod build;
pub mod profiling;
pub mod target_cfg;
pub mod timeout;
pub mod ui;

use std::path::Path;

use dupe::Dupe;
use gazebo::prelude::*;
use kuro_cli_proto::ConfigOverride;
use kuro_cli_proto::RepresentativeConfigFlag;
use kuro_cli_proto::config_override::ConfigType;
use kuro_cli_proto::representative_config_flag::Source as RepresentativeConfigFlagSource;
use kuro_common::argv::ArgFileKind;
use kuro_common::argv::ArgFilePath;
use kuro_common::argv::ExpandedArgSource;
use kuro_common::argv::ExpandedArgv;
use kuro_common::argv::FlagfileArgSource;
use kuro_fs::paths::abs_path::AbsPath;
use kuro_fs::working_dir::AbsWorkingDir;

use crate::common::profiling::BuckProfileMode;
use crate::common::ui::CommonConsoleOptions;
use crate::immediate_config::ImmediateConfigContext;
use crate::path_arg::PathArg;

pub const EVENT_LOG: &str = "event-log";
pub const NO_EVENT_LOG: &str = "no-event-log";

#[derive(Debug, kuro_error::Error)]
#[error("indices len is not equal to collection len for flag `{flag_name}`")]
#[kuro(tag = kuro_error::ErrorTag::InternalError)]
struct IndicesLengthMismatchError {
    flag_name: String,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Dupe,
    Copy,
    clap::ValueEnum
)]
#[clap(rename_all = "lower")]
pub enum HostPlatformOverride {
    Default,
    Linux,
    MacOs,
    Windows,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Dupe,
    Copy,
    clap::ValueEnum,
    Default
)]
#[clap(rename_all = "lower")]
pub enum PreemptibleWhen {
    /// (default) When another command starts that cannot run in parallel with this one, block that command.
    #[default]
    Never, // Read; "If I am Never, then never preempt me" (the default)
    /// When another command starts, interrupt this command, *even if they could run in
    /// parallel*. There is no good reason to use this other than that it provides slightly nicer
    /// superconsole output.
    Always,
    /// When another command starts that cannot run in parallel with this one,
    /// interrupt this command.
    OnDifferentState, // Read; "if a command comes in, preempt me on different state"
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Dupe,
    Copy,
    clap::ValueEnum,
    Default
)]
#[clap(rename_all = "lower")]
pub enum ExitWhen {
    /// (default) Execute this command normally.
    #[default]
    Never,
    /// Fail this command if another command is already running with a different state.
    DifferentState,
    /// Fail this command if another command is already running (regardless of daemon state).
    NotIdle,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Dupe,
    Copy,
    clap::ValueEnum
)]
#[clap(rename_all = "lower")]
pub enum HostArchOverride {
    Default,
    AArch64,
    X86_64,
}

/// Defines options related to commands that involves a streaming daemon command.
#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize, Default)]
#[clap(next_help_heading = "Event Log Options")]
pub struct CommonEventLogOptions {
    /// Write events to this log file
    #[clap(value_name = "PATH", long = EVENT_LOG)]
    pub event_log: Option<PathArg>,

    /// Do not write any event logs. Overrides --event-log. Used from `replay` to avoid recursive logging
    #[clap(long = NO_EVENT_LOG, hide = true)]
    pub no_event_log: bool,

    /// Write command invocation id into this file.
    #[clap(long, value_name = "PATH")]
    pub(crate) write_build_id: Option<PathArg>,

    /// Write the invocation record (as JSON) to this path. No guarantees whatsoever are made
    /// regarding the stability of the format.
    #[clap(long, value_name = "PATH")]
    pub(crate) unstable_write_invocation_record: Option<PathArg>,

    /// Write the command report to this path. A command report is always
    /// written to `buck-out/v2/<uuid>/command_report` even without this flag.
    #[clap(long, value_name = "PATH")]
    pub(crate) command_report_path: Option<PathArg>,
}

impl CommonEventLogOptions {
    pub fn default_ref() -> &'static Self {
        static DEFAULT: CommonEventLogOptions = CommonEventLogOptions {
            event_log: None,
            no_event_log: false,
            write_build_id: None,
            command_report_path: None,
            unstable_write_invocation_record: None,
        };
        &DEFAULT
    }

    pub fn no_event_log_ref() -> &'static Self {
        static NO_EVENT_LOG: CommonEventLogOptions = CommonEventLogOptions {
            event_log: None,
            no_event_log: true,
            write_build_id: None,
            command_report_path: None,
            unstable_write_invocation_record: None,
        };
        &NO_EVENT_LOG
    }
}

/// Defines options for config and configuration related things. Any command that involves the build
/// graph should include these options.
#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize, Default)]
#[clap(next_help_heading = "Buckconfig Options")]
pub struct CommonBuildConfigurationOptions {
    #[clap(
        value_name = "SECTION.OPTION=VALUE",
        long = "config",
        short = 'c',
        help = "List of config options",
        // Needs to be explicitly set, otherwise will treat `-c a b c` -> [a, b, c]
        // rather than [a] and other positional arguments `b c`.
        num_args = 1
    )]
    pub config_values: Vec<String>,

    #[clap(
        value_name = "PATH",
        long = "config-file",
        help = "List of config file paths",
        num_args = 1
    )]
    pub config_files: Vec<String>,

    #[clap(long, ignore_case = true, value_name = "HOST", value_enum)]
    pub fake_host: Option<HostPlatformOverride>,

    #[clap(long, ignore_case = true, value_name = "ARCH", value_enum)]
    pub fake_arch: Option<HostArchOverride>,

    /// Value must be formatted as: version-build (e.g., 14.3.0-14C18 or 14.1-14B47b)
    #[clap(long, value_name = "VERSION-BUILD")]
    pub fake_xcode_version: Option<String>,

    /// Re-uses any `--config` values (inline or via modefiles) if there's
    /// a previous command, otherwise the flag is ignored.
    ///
    /// If there is a previous command and `--reuse-current-config` is set,
    /// then the old config is used, ignoring any overrides.
    ///
    /// If there is no previous command but the flag was set, then the flag is ignored,
    /// the command behaves as if the flag was not set at all.
    #[clap(long)]
    pub reuse_current_config: bool,

    /// Used for exiting a concurrent command when a different state is detected.
    #[clap(long, hide = true)]
    pub exit_when_different_state: bool,

    /// Used to configure when this command could be preempted by another command for the same isolation dir.
    ///
    /// Normally, when you run two commands - from different terminals, say - kuro will attempt
    /// to run them in parallel. However, if the two commands are based on different state, that
    /// is they either have different configs or different filesystem states, kuro cannot run them
    /// in parallel. The default behavior in this case is to block the second command until the
    /// first completes.
    #[clap(long, ignore_case = true, value_enum)]
    pub preemptible: Option<PreemptibleWhen>,
    /// Whether to proceed with or fail this invocation based on the daemon state.
    #[clap(long, ignore_case = true, value_enum)]
    pub exit_when: Option<ExitWhen>,

    /// Compilation mode (Bazel compatibility).
    ///
    /// Controls optimization level for the build. Accepted values:
    /// - `fastbuild` (default): No optimizations, enables assertions
    /// - `dbg`: Full debug info, no optimization
    /// - `opt`: Maximum optimization, no debug info
    ///
    /// Note: Equivalent to Bazel's --compilation_mode flag. Currently accepted
    /// for compatibility but does not yet affect C++ compiler flags.
    #[clap(
        long = "compilation_mode",
        alias = "compilation-mode",
        hide = true,
        value_name = "MODE",
        ignore_case = true
    )]
    pub compilation_mode: Option<String>,

    /// Define a Bazel-compatible variable accessible via `ctx.var`.
    ///
    /// Format: --define KEY=VALUE (e.g., --define FOO=bar).
    /// Multiple --define flags can be specified.
    #[clap(long = "define", hide = true, value_name = "KEY=VALUE", num_args = 1)]
    pub define: Vec<String>,

    /// Pass environment variable to build actions (Bazel compatibility).
    ///
    /// Values are in NAME or NAME=VALUE format. NAME without =VALUE inherits
    /// from the host environment.
    #[clap(
        long = "action-env",
        alias = "action_env",
        hide = true,
        value_name = "NAME[=VALUE]",
        num_args = 1
    )]
    pub action_env: Vec<String>,

    /// C/C++ compilation flags (Bazel compatibility).
    ///
    /// Values are passed to ctx.fragments.cpp.copts and added to all C/C++ compile actions.
    #[clap(long = "copt", hide = true, value_name = "FLAG", num_args = 1)]
    pub copts: Vec<String>,

    /// C++-specific compilation flags (Bazel compatibility).
    ///
    /// Values are passed to ctx.fragments.cpp.cxxopts and added to C++ compile actions.
    #[clap(long = "cxxopt", hide = true, value_name = "FLAG", num_args = 1)]
    pub cxxopts: Vec<String>,

    /// C-specific compilation flags (Bazel compatibility).
    ///
    /// Values are passed to ctx.fragments.cpp.conlyopts and added to C compile actions.
    #[clap(long = "conlyopt", hide = true, value_name = "FLAG", num_args = 1)]
    pub conlyopts: Vec<String>,

    /// Linker flags (Bazel compatibility).
    ///
    /// Values are passed to ctx.fragments.cpp.linkopts and added to all link actions.
    #[clap(long = "linkopt", hide = true, value_name = "FLAG", num_args = 1)]
    pub linkopts: Vec<String>,

    /// Strip mode for binaries: "always", "sometimes", or "never" (Bazel compatibility).
    #[clap(long = "strip", hide = true, value_name = "always|sometimes|never")]
    pub strip_mode: Option<String>,

    /// Enable or disable build features (Bazel compatibility).
    ///
    /// Use --features=FEATURE to enable, --features=-FEATURE to disable.
    #[clap(long = "features", hide = true, value_name = "FEATURE", num_args = 1)]
    pub global_features: Vec<String>,

    /// Pass environment variable to test actions (Bazel compatibility).
    ///
    /// Values are in NAME or NAME=VALUE format. NAME without =VALUE inherits
    /// from the host environment.
    #[clap(
        long = "test-env",
        alias = "test_env",
        hide = true,
        value_name = "NAME[=VALUE]",
        num_args = 1
    )]
    pub test_env: Vec<String>,

    /// Enable code coverage collection (Bazel compatibility).
    #[clap(
        long = "collect-code-coverage",
        alias = "collect_code_coverage",
        hide = true
    )]
    pub collect_code_coverage: bool,

    /// Disable code coverage collection (Bazel compatibility).
    #[clap(
        long = "nocollect-code-coverage",
        alias = "nocollect_code_coverage",
        hide = true
    )]
    pub nocollect_code_coverage: bool,

    // ---- Bazel compatibility flags (accepted, some are no-ops) ----
    /// Enable ANSI color output (Bazel compatibility).
    ///
    /// Accepted for compatibility with Bazel's --color flag.
    /// Use `--console` to control Kuro's output style.
    #[clap(long = "color", hide = true, value_name = "yes|no|auto")]
    pub color: Option<String>,

    /// Show progress (Bazel compatibility).
    ///
    /// Accepted for compatibility with Bazel's --show_progress flag.
    #[clap(long = "show-progress", alias = "show_progress", hide = true)]
    pub show_progress: bool,

    /// Set per-action execution strategy (Bazel compatibility).
    ///
    /// Format: --strategy=MNEMONIC=STRATEGY (e.g., CppCompile=remote).
    /// Accepted for compatibility with Bazel's --strategy flag.
    #[clap(long = "strategy", hide = true, value_name = "MNEMONIC=STRATEGY")]
    pub strategy: Vec<String>,

    /// Set execution strategy for genrule actions (Bazel compatibility).
    ///
    /// Accepted for compatibility with Bazel's --genrule_strategy flag.
    #[clap(
        long = "genrule-strategy",
        alias = "genrule_strategy",
        hide = true,
        value_name = "STRATEGY"
    )]
    pub genrule_strategy: Vec<String>,

    /// Remote execution options (Bazel compatibility).
    ///
    /// Accepted for compatibility with Bazel's --remote_* flags.
    /// These are currently accepted but not applied.
    #[clap(
        long = "remote-executor",
        alias = "remote_executor",
        hide = true,
        value_name = "HOST:PORT"
    )]
    pub remote_executor: Option<String>,

    #[clap(
        long = "remote-cache",
        alias = "remote_cache",
        hide = true,
        value_name = "URL"
    )]
    pub remote_cache: Option<String>,

    /// Maximum number of remote execution retries (Bazel compatibility).
    #[clap(
        long = "remote-retries",
        alias = "remote_retries",
        hide = true,
        value_name = "N"
    )]
    pub remote_retries: Option<u32>,

    /// Spawn strategy (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "spawn-strategy",
        alias = "spawn_strategy",
        hide = true,
        value_name = "STRATEGY"
    )]
    pub spawn_strategy: Option<String>,

    /// Dynamic local strategy (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "dynamic-local-strategy",
        alias = "dynamic_local_strategy",
        hide = true,
        value_name = "MNEMONIC=STRATEGY",
        num_args = 1
    )]
    pub dynamic_local_strategy: Vec<String>,

    /// Dynamic remote strategy (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "dynamic-remote-strategy",
        alias = "dynamic_remote_strategy",
        hide = true,
        value_name = "MNEMONIC=STRATEGY",
        num_args = 1
    )]
    pub dynamic_remote_strategy: Vec<String>,

    /// Disk cache directory (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "disk-cache",
        alias = "disk_cache",
        hide = true,
        value_name = "PATH"
    )]
    pub disk_cache: Option<String>,

    /// Repository cache directory (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "repository-cache",
        alias = "repository_cache",
        hide = true,
        value_name = "PATH"
    )]
    pub repository_cache: Option<String>,

    /// Symlink prefix for output directories (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "symlink-prefix",
        alias = "symlink_prefix",
        hide = true,
        value_name = "PREFIX"
    )]
    pub symlink_prefix: Option<String>,

    /// Remote timeout (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "remote-timeout",
        alias = "remote_timeout",
        hide = true,
        value_name = "SECONDS"
    )]
    pub remote_timeout: Option<String>,

    /// Loading phase threads (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "loading-phase-threads",
        alias = "loading_phase_threads",
        hide = true,
        value_name = "N"
    )]
    pub loading_phase_threads: Option<String>,

    /// Build event text file (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "build-event-text-file",
        alias = "build_event_text_file",
        hide = true,
        value_name = "PATH"
    )]
    pub build_event_text_file: Option<String>,

    /// Build event binary file (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "build-event-binary-file",
        alias = "build_event_binary_file",
        hide = true,
        value_name = "PATH"
    )]
    pub build_event_binary_file: Option<String>,

    /// --repo_env NAME=VALUE pairs (Bazel compatibility).
    /// Sets environment variables for repository rules.
    #[clap(
        long = "repo-env",
        alias = "repo_env",
        hide = true,
        value_name = "NAME=VALUE",
        num_args = 1
    )]
    pub repo_env: Vec<String>,

    /// Remote upload local results (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "remote-upload-local-results",
        alias = "remote_upload_local_results",
        hide = true
    )]
    pub remote_upload_local_results: bool,

    /// Remote accept cached results (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "remote-accept-cached",
        alias = "remote_accept_cached",
        hide = true
    )]
    pub remote_accept_cached: bool,

    /// Announce RC (Bazel compatibility, accepted but ignored).
    #[clap(long = "announce-rc", alias = "announce_rc", hide = true)]
    pub announce_rc: bool,

    /// Tool tag for build metrics (Bazel compatibility, accepted but ignored).
    #[clap(long = "tool-tag", alias = "tool_tag", hide = true, value_name = "TAG")]
    pub tool_tag: Option<String>,

    /// Host C compiler flags (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "host-copt",
        alias = "host_copt",
        hide = true,
        value_name = "FLAG",
        num_args = 1
    )]
    pub host_copt: Vec<String>,

    /// Host C++ compiler flags (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "host-cxxopt",
        alias = "host_cxxopt",
        hide = true,
        value_name = "FLAG",
        num_args = 1
    )]
    pub host_cxxopt: Vec<String>,

    /// Host linker flags (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "host-linkopt",
        alias = "host_linkopt",
        hide = true,
        value_name = "FLAG",
        num_args = 1
    )]
    pub host_linkopt: Vec<String>,

    /// Force position-independent code (Bazel compatibility, accepted but ignored).
    #[clap(long = "force-pic", alias = "force_pic", hide = true)]
    pub force_pic: bool,

    /// Per-file compiler options (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "per-file-copt",
        alias = "per_file_copt",
        hide = true,
        value_name = "REGEX=OPTS",
        num_args = 1
    )]
    pub per_file_copt: Vec<String>,

    /// Local CPU resources (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "local-cpu-resources",
        alias = "local_cpu_resources",
        hide = true,
        value_name = "N"
    )]
    pub local_cpu_resources: Option<String>,

    /// Local RAM resources in MB (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "local-ram-resources",
        alias = "local_ram_resources",
        hide = true,
        value_name = "MB"
    )]
    pub local_ram_resources: Option<String>,

    /// Show subcommands (Bazel compatibility, accepted but ignored).
    #[clap(long = "subcommands", hide = true)]
    pub subcommands: bool,

    /// Sandbox debug mode (Bazel compatibility, accepted but ignored).
    #[clap(long = "sandbox-debug", alias = "sandbox_debug", hide = true)]
    pub sandbox_debug: bool,

    /// Host platform (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "host-platform",
        alias = "host_platform",
        hide = true,
        value_name = "LABEL"
    )]
    pub host_platform: Option<String>,

    /// Host compilation mode (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "host-compilation-mode",
        alias = "host_compilation_mode",
        hide = true,
        value_name = "MODE"
    )]
    pub host_compilation_mode: Option<String>,

    /// Build tag filter (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "build-tag-filter",
        alias = "build_tag_filter",
        hide = true,
        value_name = "TAGS"
    )]
    pub build_tag_filter: Option<String>,

    /// Test tag filter (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "test-tag-filter",
        alias = "test_tag_filter",
        hide = true,
        value_name = "TAGS"
    )]
    pub test_tag_filter: Option<String>,

    /// Run under (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "run-under",
        alias = "run_under",
        hide = true,
        value_name = "COMMAND"
    )]
    pub run_under: Option<String>,

    /// Host features (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "host-features",
        alias = "host_features",
        hide = true,
        value_name = "FEATURE",
        num_args = 1
    )]
    pub host_features: Vec<String>,

    /// Register toolchains for toolchain resolution (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "register-toolchains",
        alias = "register_toolchains",
        hide = true
    )]
    pub register_toolchains: bool,

    /// Whether to check direct dependencies (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "check-direct-dependencies",
        alias = "check_direct_dependencies",
        hide = true,
        value_name = "MODE"
    )]
    pub check_direct_dependencies: Option<String>,

    /// Modify execution info (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "modify-execution-info",
        alias = "modify_execution_info",
        hide = true,
        value_name = "SPEC"
    )]
    pub modify_execution_info: Option<String>,

    /// Output filter regex (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "output-filter",
        alias = "output_filter",
        hide = true,
        value_name = "REGEX"
    )]
    pub output_filter: Option<String>,

    /// Build runfiles (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "build-runfile-links",
        alias = "build_runfile_links",
        hide = true
    )]
    pub build_runfile_links: bool,

    /// Don't build runfiles (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "nobuild-runfile-links",
        alias = "nobuild_runfile_links",
        hide = true
    )]
    pub nobuild_runfile_links: bool,

    /// Enable bzlmod (Bazel compatibility, accepted but ignored - always on).
    #[clap(long = "enable-bzlmod", alias = "enable_bzlmod", hide = true)]
    pub enable_bzlmod: bool,

    /// Allow yanked versions (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "allow-yanked-versions",
        alias = "allow_yanked_versions",
        hide = true,
        value_name = "MODULE",
        num_args = 1
    )]
    pub allow_yanked_versions: Vec<String>,

    /// Output groups to build (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "output-groups",
        alias = "output_groups",
        hide = true,
        value_name = "GROUPS"
    )]
    pub output_groups: Option<String>,

    /// Java runtime version (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "java-runtime-version",
        alias = "java_runtime_version",
        hide = true,
        value_name = "VERSION"
    )]
    pub java_runtime_version: Option<String>,

    /// Java language version (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "java-language-version",
        alias = "java_language_version",
        hide = true,
        value_name = "VERSION"
    )]
    pub java_language_version: Option<String>,

    /// Tool Java runtime version (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "tool-java-runtime-version",
        alias = "tool_java_runtime_version",
        hide = true,
        value_name = "VERSION"
    )]
    pub tool_java_runtime_version: Option<String>,

    /// Tool Java language version (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "tool-java-language-version",
        alias = "tool_java_language_version",
        hide = true,
        value_name = "VERSION"
    )]
    pub tool_java_language_version: Option<String>,

    /// Enable platform-specific config (Bazel compatibility).
    /// Auto-activates build:<os> config in .bazelrc based on host OS.
    #[clap(
        long = "enable-platform-specific-config",
        alias = "enable_platform_specific_config",
        hide = true
    )]
    pub enable_platform_specific_config: bool,

    /// Show progress rate limit (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "show-progress-rate-limit",
        alias = "show_progress_rate_limit",
        hide = true,
        value_name = "SECONDS"
    )]
    pub show_progress_rate_limit: Option<f64>,

    /// Convenience symlinks mode (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "experimental-convenience-symlinks",
        alias = "experimental_convenience_symlinks",
        hide = true,
        value_name = "MODE"
    )]
    pub convenience_symlinks: Option<String>,

    /// Heap dump on OOM (Bazel JVM compatibility, accepted but ignored).
    #[clap(long = "heap-dump-on-oom", alias = "heap_dump_on_oom", hide = true)]
    pub heap_dump_on_oom: bool,

    /// Curses output mode (Bazel compatibility, accepted but ignored).
    #[clap(long = "curses", hide = true, value_name = "MODE")]
    pub curses: Option<String>,

    /// Flaky test retry attempts (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "flaky-test-attempts",
        alias = "flaky_test_attempts",
        hide = true,
        value_name = "N"
    )]
    pub flaky_test_attempts: Option<String>,

    /// gRPC keepalive time (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "grpc-keepalive-time",
        alias = "grpc_keepalive_time",
        hide = true,
        value_name = "DURATION"
    )]
    pub grpc_keepalive_time: Option<String>,

    /// Lockfile mode (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "lockfile-mode",
        alias = "lockfile_mode",
        hide = true,
        value_name = "MODE"
    )]
    pub lockfile_mode: Option<String>,

    /// Module mirrors for BCR (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "module-mirrors",
        alias = "module_mirrors",
        hide = true,
        value_name = "URL"
    )]
    pub module_mirrors: Option<String>,

    /// Remote download outputs mode (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "remote-download-outputs",
        alias = "remote_download_outputs",
        hide = true,
        value_name = "MODE"
    )]
    pub remote_download_outputs: Option<String>,

    /// Remote local fallback (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "remote-local-fallback",
        alias = "remote_local_fallback",
        hide = true
    )]
    pub remote_local_fallback: bool,

    /// Reuse sandbox directories (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "reuse-sandbox-directories",
        alias = "reuse_sandbox_directories",
        hide = true
    )]
    pub reuse_sandbox_directories: bool,

    /// Sandbox default allow network (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "sandbox-default-allow-network",
        alias = "sandbox_default_allow_network",
        hide = true
    )]
    pub sandbox_default_allow_network: bool,

    /// No sandbox default allow network (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "nosandbox-default-allow-network",
        alias = "nosandbox_default_allow_network",
        hide = true
    )]
    pub nosandbox_default_allow_network: bool,

    /// Show result count (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "show-result",
        alias = "show_result",
        hide = true,
        value_name = "N"
    )]
    pub show_result: Option<i32>,

    /// Show timestamps (Bazel compatibility, accepted but ignored).
    #[clap(long = "show-timestamps", alias = "show_timestamps", hide = true)]
    pub show_timestamps: bool,

    /// Terminal columns (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "terminal-columns",
        alias = "terminal_columns",
        hide = true,
        value_name = "N"
    )]
    pub terminal_columns: Option<i32>,

    /// Test strategy (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "test-strategy",
        alias = "test_strategy",
        hide = true,
        value_name = "STRATEGY"
    )]
    pub test_strategy: Option<String>,

    /// Test summary mode (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "test-summary",
        alias = "test_summary",
        hide = true,
        value_name = "MODE"
    )]
    pub test_summary: Option<String>,

    /// Test timeout (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "test-timeout",
        alias = "test_timeout",
        hide = true,
        value_name = "SECONDS"
    )]
    pub test_timeout: Option<String>,

    /// Cache test results (Bazel compatibility, accepted but ignored).
    #[clap(long = "cache-test-results", alias = "cache_test_results", hide = true)]
    pub cache_test_results: bool,

    /// No cache test results (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "nocache-test-results",
        alias = "nocache_test_results",
        hide = true
    )]
    pub nocache_test_results: bool,

    /// Workspace status command (Bazel compatibility, accepted but ignored).
    #[clap(
        long = "workspace-status-command",
        alias = "workspace_status_command",
        hide = true,
        value_name = "CMD"
    )]
    pub workspace_status_command: Option<String>,
}

impl CommonBuildConfigurationOptions {
    /// Produces a single, ordered list of config overrides. A `ConfigOverride`
    /// represents either a file, passed via `--config-file`, or a config value,
    /// passed via `-c`/`--config`. The relative order of those are important,
    /// hence they're merged into a single list.
    pub fn config_overrides(
        &self,
        matches: BuckArgMatches<'_>,
        immediate_ctx: &ImmediateConfigContext<'_>,
        cwd: &AbsWorkingDir,
    ) -> kuro_error::Result<Vec<ConfigOverride>> {
        fn with_indices<'a, T>(
            collection: &'a [T],
            name: &str,
            matches: BuckArgMatches<'a>,
        ) -> kuro_error::Result<impl Iterator<Item = (usize, &'a T)> + use<'a, T>> {
            // Skip calling indices_of when collection is empty: the argument may not
            // be registered in the current command (e.g., `complete`), and clap 4
            // panics with "is not an id of an argument" for unregistered argument ids.
            let indices = if collection.is_empty() {
                None
            } else {
                matches.inner.indices_of(name)
            };
            let indices = indices.unwrap_or_default();
            if indices.len() != collection.len() {
                return Err(kuro_error::Error::from(IndicesLengthMismatchError {
                    flag_name: name.to_owned(),
                }));
            }
            Ok(indices.into_iter().zip(collection))
        }

        let config_values_args = with_indices(&self.config_values, "config_values", matches)?
            .map(|(index, config_value)| {
                let (cell, raw_arg) = match config_value.split_once("//") {
                    Some((cell, val)) if !cell.contains('=') => {
                        let cell = immediate_ctx
                            .resolve_alias_to_path_in_cwd(cell)?
                            .to_string();
                        (Some(cell), val)
                    }
                    _ => (None, config_value.as_str()),
                };

                kuro_error::Ok((
                    index,
                    ConfigOverride {
                        cell,
                        config_override: raw_arg.to_owned(),
                        config_type: ConfigType::Value as i32,
                    },
                ))
            })
            .collect::<kuro_error::Result<Vec<_>>>()?;

        let config_file_args = with_indices(&self.config_files, "config_files", matches)?
            .map(|(index, file)| {
                let (cell, path) = match file.split_once("//") {
                    Some((cell, val)) => {
                        // This should also reject =?
                        let cell = immediate_ctx
                            .resolve_alias_to_path_in_cwd(cell)?
                            .to_string();
                        (Some(cell), val.to_owned())
                    }
                    None => {
                        let abs_path = match AbsPath::new(file) {
                            Ok(p) => p.to_owned(),
                            Err(_) => cwd.resolve(Path::new(file)),
                        };
                        (None, abs_path.to_string())
                    }
                };
                Ok((
                    index,
                    ConfigOverride {
                        cell,
                        config_override: path,
                        config_type: ConfigType::File as i32,
                    },
                ))
            })
            .collect::<kuro_error::Result<Vec<_>>>()?;

        let mut ordered_merged_configs: Vec<(usize, ConfigOverride)> = config_file_args;
        ordered_merged_configs.extend(config_values_args);
        ordered_merged_configs.sort_by(|(lhs_index, _), (rhs_index, _)| lhs_index.cmp(rhs_index));

        Ok(ordered_merged_configs.into_map(|(_, config_arg)| config_arg))
    }

    pub fn host_platform_override(&self) -> HostPlatformOverride {
        match &self.fake_host {
            Some(v) => *v,
            None => HostPlatformOverride::Default,
        }
    }
    pub fn host_arch_override(&self) -> HostArchOverride {
        match &self.fake_arch {
            Some(v) => *v,
            None => HostArchOverride::Default,
        }
    }
    pub fn host_xcode_version_override(&self) -> Option<String> {
        self.fake_xcode_version.to_owned()
    }

    pub fn default_ref() -> &'static Self {
        static DEFAULT: CommonBuildConfigurationOptions = CommonBuildConfigurationOptions {
            config_values: vec![],
            config_files: vec![],
            fake_host: None,
            fake_arch: None,
            fake_xcode_version: None,
            reuse_current_config: false,
            exit_when_different_state: false,
            preemptible: Some(PreemptibleWhen::Never),
            exit_when: None,
            compilation_mode: None,
            define: vec![],
            action_env: vec![],
            copts: vec![],
            cxxopts: vec![],
            conlyopts: vec![],
            linkopts: vec![],
            strip_mode: None,
            global_features: vec![],
            test_env: vec![],
            collect_code_coverage: false,
            nocollect_code_coverage: false,
            color: None,
            show_progress: false,
            strategy: vec![],
            genrule_strategy: vec![],
            remote_executor: None,
            remote_cache: None,
            remote_retries: None,
            spawn_strategy: None,
            dynamic_local_strategy: vec![],
            dynamic_remote_strategy: vec![],
            disk_cache: None,
            repository_cache: None,
            symlink_prefix: None,
            remote_timeout: None,
            loading_phase_threads: None,
            build_event_text_file: None,
            build_event_binary_file: None,
            repo_env: vec![],
            remote_upload_local_results: false,
            remote_accept_cached: false,
            announce_rc: false,
            tool_tag: None,
            host_copt: vec![],
            host_cxxopt: vec![],
            host_linkopt: vec![],
            force_pic: false,
            per_file_copt: vec![],
            local_cpu_resources: None,
            local_ram_resources: None,
            subcommands: false,
            sandbox_debug: false,
            host_platform: None,
            host_compilation_mode: None,
            build_tag_filter: None,
            test_tag_filter: None,
            run_under: None,
            host_features: vec![],
            register_toolchains: false,
            check_direct_dependencies: None,
            modify_execution_info: None,
            output_filter: None,

            build_runfile_links: false,
            nobuild_runfile_links: false,
            enable_bzlmod: false,
            allow_yanked_versions: vec![],
            output_groups: None,
            java_runtime_version: None,
            java_language_version: None,
            tool_java_runtime_version: None,
            tool_java_language_version: None,
            enable_platform_specific_config: false,
            show_progress_rate_limit: None,
            convenience_symlinks: None,
            heap_dump_on_oom: false,
            curses: None,
            flaky_test_attempts: None,
            grpc_keepalive_time: None,
            lockfile_mode: None,
            module_mirrors: None,
            remote_download_outputs: None,
            remote_local_fallback: false,
            reuse_sandbox_directories: false,
            sandbox_default_allow_network: false,
            nosandbox_default_allow_network: false,
            show_result: None,
            show_timestamps: false,
            terminal_columns: None,
            test_strategy: None,
            test_summary: None,
            test_timeout: None,
            cache_test_results: false,
            nocache_test_results: false,
            workspace_status_command: None,
        };
        &DEFAULT
    }

    pub fn reuse_current_config_and_preemptible_ref() -> &'static Self {
        static OPTS: CommonBuildConfigurationOptions = CommonBuildConfigurationOptions {
            config_values: vec![],
            config_files: vec![],
            fake_host: None,
            fake_arch: None,
            fake_xcode_version: None,
            reuse_current_config: true,
            exit_when_different_state: false,
            preemptible: Some(PreemptibleWhen::OnDifferentState),
            exit_when: None,
            compilation_mode: None,
            define: vec![],
            action_env: vec![],
            copts: vec![],
            cxxopts: vec![],
            conlyopts: vec![],
            linkopts: vec![],
            strip_mode: None,
            global_features: vec![],
            test_env: vec![],
            collect_code_coverage: false,
            nocollect_code_coverage: false,
            color: None,
            show_progress: false,
            strategy: vec![],
            genrule_strategy: vec![],
            remote_executor: None,
            remote_cache: None,
            remote_retries: None,
            spawn_strategy: None,
            dynamic_local_strategy: vec![],
            dynamic_remote_strategy: vec![],
            disk_cache: None,
            repository_cache: None,
            symlink_prefix: None,
            remote_timeout: None,
            loading_phase_threads: None,
            build_event_text_file: None,
            build_event_binary_file: None,
            repo_env: vec![],
            remote_upload_local_results: false,
            remote_accept_cached: false,
            announce_rc: false,
            tool_tag: None,
            host_copt: vec![],
            host_cxxopt: vec![],
            host_linkopt: vec![],
            force_pic: false,
            per_file_copt: vec![],
            local_cpu_resources: None,
            local_ram_resources: None,
            subcommands: false,
            sandbox_debug: false,
            host_platform: None,
            host_compilation_mode: None,
            build_tag_filter: None,
            test_tag_filter: None,
            run_under: None,
            host_features: vec![],
            register_toolchains: false,
            check_direct_dependencies: None,
            modify_execution_info: None,
            output_filter: None,

            build_runfile_links: false,
            nobuild_runfile_links: false,
            enable_bzlmod: false,
            allow_yanked_versions: vec![],
            output_groups: None,
            java_runtime_version: None,
            java_language_version: None,
            tool_java_runtime_version: None,
            tool_java_language_version: None,
            enable_platform_specific_config: false,
            show_progress_rate_limit: None,
            convenience_symlinks: None,
            heap_dump_on_oom: false,
            curses: None,
            flaky_test_attempts: None,
            grpc_keepalive_time: None,
            lockfile_mode: None,
            module_mirrors: None,
            remote_download_outputs: None,
            remote_local_fallback: false,
            reuse_sandbox_directories: false,
            sandbox_default_allow_network: false,
            nosandbox_default_allow_network: false,
            show_result: None,
            show_timestamps: false,
            terminal_columns: None,
            test_strategy: None,
            test_summary: None,
            test_timeout: None,
            cache_test_results: false,
            nocache_test_results: false,
            workspace_status_command: None,
        };
        &OPTS
    }
}

#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize, Default)]
#[clap(next_help_heading = "Starlark Options")]
pub struct CommonStarlarkOptions {
    /// Disable runtime type checking in Starlark interpreter.
    ///
    /// This option is not stable, and can be used only locally
    /// to diagnose evaluation performance problems.
    #[clap(long)]
    pub disable_starlark_types: bool,

    /// Typecheck bzl and bxl files during evaluation.
    #[clap(long, hide = true)]
    pub unstable_typecheck: bool,

    /// Record or show target call stacks.
    ///
    /// Starlark call stacks will be included in duplicate targets error.
    ///
    /// If a command outputs targets (like `targets` command),
    /// starlark call stacks will be printed after the targets.
    #[clap(long = "stack")]
    pub target_call_stacks: bool,

    /// If there are targets with duplicate names in `BUCK` file,
    /// skip all the duplicates but the first one.
    /// This is a hack for TD. Do not use this option.
    #[clap(long, hide = true)]
    pub(crate) skip_targets_with_duplicate_names: bool,

    /// Enables profiling for all evaluations whose evaluation identifier matches one of the provided patterns.
    ///
    /// Some examples identifiers:
    ///    analysis/cell//kuro/app/kuro_action_impl:kuro_action_impl (cfg:linux-x86_64#27ac5723e0c99706)
    ///    load/cell//build_defs/json.bzl
    ///    load/prelude//playground/test.bxl
    ///    load/cell//build_defs/json.bzl@other_cell
    ///    load_buildfile/fbcode//third-party-buck/platform010/build/ncurses
    ///    load_packagefile/fbcode//cli/rust/cli_delegate
    ///    anon_analysis/anon//:_anon_link_rule (anon: 766183dc9b6f680a) (fbcode//kuro/platform/execution:linux-x86_64#08961b14cfb182aa)
    ///    bxl/prelude//playground/test.bxl:playground
    ///
    /// You can pass `--profile-patterns=.*` to enable no-op profiling for everything (additionally pass `--profile-patterns-mode=none` to
    /// use no-op profiling to just get a list of all the identifiers).
    ///
    /// The profile results will be written to individual .profile files in `<ROOT_OUTPUT>/<data+time>-<uuid>/` where ROOT_OUTPUT comes from
    /// the --profile-patterns-output flag. In that directory there will also be a file listing all the identifiers that were profiled.
    ///
    /// Enabling/disabling profiling of an evaluation will invalidate the results of that evaluation and it will be recomputed. In some
    /// cases, this will cause other work to also need to be redone (for example, invalidating the result of loading PACKAGE files
    /// causes all consumers to be recomputed). But if you keep profiling options consistent between commands, only the work that is
    /// otherwise invalidated will be redone (and only for those would profiling results be created).
    ///
    /// You must also pass --profile-patterns-mode and --profile-patterns-output.
    #[clap(
        long,
        requires = "profile_patterns_output",
        requires = "profile_patterns_mode"
    )]
    pub(crate) profile_patterns: Option<Vec<String>>,

    #[clap(long, value_name = "PATH")]
    profile_patterns_output: Option<PathArg>,

    /// Profile mode.
    ///
    /// Memory profiling modes have suffixes either `-allocated` or `-retained`.
    ///
    /// `-retained` means memory kept in frozen starlark heaps after analysis completes.
    /// `-retained` does not work when profiling loading,
    /// because no memory is retained after loading and frozen heap is not even created.
    /// This is probably what you want when profiling analysis.
    ///
    /// `-allocated` means allocated memory, including memory which is later garbage collected.
    #[clap(long, value_enum)]
    profile_patterns_mode: Option<BuckProfileMode>,
}

impl CommonStarlarkOptions {
    pub fn default_ref() -> &'static Self {
        static DEFAULT: CommonStarlarkOptions = CommonStarlarkOptions {
            disable_starlark_types: false,
            unstable_typecheck: false,
            target_call_stacks: false,
            skip_targets_with_duplicate_names: false,
            profile_patterns: None,
            profile_patterns_output: None,
            profile_patterns_mode: None,
        };
        &DEFAULT
    }

    pub(crate) fn profile_pattern_opts(
        &self,
        working_dir: &AbsWorkingDir,
    ) -> Option<kuro_cli_proto::client_context::ProfilePatternOptions> {
        self.profile_patterns.as_ref().map(|v| {
            kuro_cli_proto::client_context::ProfilePatternOptions {
                profile_patterns: v.clone(),
                profile_mode: self.profile_patterns_mode.as_ref().unwrap().to_proto() as i32,
                profile_output: self
                    .profile_patterns_output
                    .as_ref()
                    .unwrap()
                    .resolve(working_dir)
                    .to_string(),
            }
        })
    }
}

/// Common options for commands like `build` or `query`.
/// Not all the commands have all the options.
#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize, Default)]
pub struct CommonCommandOptions {
    /// Buckconfig and similar options.
    #[clap(flatten)]
    pub config_opts: CommonBuildConfigurationOptions,

    /// Starlark options.
    #[clap(flatten)]
    pub starlark_opts: CommonStarlarkOptions,

    /// UI options.
    #[clap(flatten)]
    pub console_opts: CommonConsoleOptions,

    /// Event-log options.
    #[clap(flatten)]
    pub event_log_opts: CommonEventLogOptions,
}

#[derive(Debug, PartialEq)]
pub enum PrintOutputsFormat {
    Plain,
    Simple,
    Json,
}

#[derive(Clone, Copy)]
pub struct BuckArgMatches<'a> {
    inner: &'a clap::ArgMatches,
    expanded_argv: &'a ExpandedArgv,
}

impl<'a> BuckArgMatches<'a> {
    pub fn from_clap(inner: &'a clap::ArgMatches, expanded_argv: &'a ExpandedArgv) -> Self {
        Self {
            inner,
            expanded_argv,
        }
    }

    pub fn unwrap_subcommand(&self) -> Self {
        match self.inner.subcommand().map(|s| s.1) {
            Some(submatches) => Self {
                inner: submatches,
                expanded_argv: self.expanded_argv,
            },
            None => panic!("Parsed a subcommand but couldn't extract subcommand argument matches"),
        }
    }

    /// A subset of the expanded argv containing config flags. When a config flag is from an argfile in the project,
    /// it will be represented with the argfile rather than the raw config flag. This gives a compact, stable, and
    /// recognizable form of the flags.
    pub fn get_representative_config_flags(&self) -> Vec<String> {
        self.get_representative_config_flags_by_source()
            .map(|flags| match &flags.source {
                Some(RepresentativeConfigFlagSource::ConfigFlag(v)) => format!("-c {v}"),
                Some(RepresentativeConfigFlagSource::ConfigFile(v)) => {
                    format!("--config-file {v}")
                }
                Some(RepresentativeConfigFlagSource::ModeFile(v)) => v.clone(),
                Some(RepresentativeConfigFlagSource::Modifier(v)) => format!("-m {v}"),
                Some(RepresentativeConfigFlagSource::TargetPlatforms(v)) => {
                    format!("--target-platforms {v}")
                }
                None => unreachable!("impossible flag"),
            })
    }

    pub fn get_representative_config_flags_by_source(&self) -> Vec<RepresentativeConfigFlag> {
        fn get_flagfile_for_logging(flagfile: &FlagfileArgSource) -> Option<&FlagfileArgSource> {
            if let Some(parent) = &flagfile.parent {
                if let Some(v) = get_flagfile_for_logging(parent) {
                    return Some(v);
                }
            }
            match &flagfile.kind {
                ArgFileKind::Path(ArgFilePath::External(_))
                | ArgFileKind::PythonExecutable(ArgFilePath::External(_), _)
                | ArgFileKind::Stdin => None,
                _ => Some(flagfile),
            }
        }
        // FIXME: Ideally we'd be able to recover this from the clap ArgMatches, but that only
        // tracks clap's index concept which doesn't map directly to argv index.
        enum State {
            None,
            Matched(&'static str),
            Finished,
        }
        let mut state = State::None;
        let config_args = self
            .expanded_argv
            .iter()
            .filter_map(move |(value, source)| {
                match state {
                    State::None => match value {
                        "-c" => {
                            state = State::Matched("-c");
                            None
                        }
                        "--config" => {
                            state = State::Matched("-c");
                            None
                        }
                        "--config-file" => {
                            state = State::Matched("--config-file");
                            None
                        }
                        "-m" => {
                            state = State::Matched("-m");
                            None
                        }
                        v if v.starts_with("-m") => Some(RepresentativeConfigFlagSource::Modifier(
                            v.split_at("-m".len()).1.trim().to_owned(),
                        )),
                        "--modifier" => {
                            state = State::Matched("-m");
                            None
                        }
                        "--target-platforms" | "--platforms" => {
                            state = State::Matched("--target-platforms");
                            None
                        }
                        v if v.starts_with("--config=") || v.starts_with("-c=") => {
                            Some(RepresentativeConfigFlagSource::ConfigFlag(
                                v.split_once("=").unwrap().1.to_owned(),
                            ))
                        }
                        v if v.starts_with("-c") => {
                            Some(RepresentativeConfigFlagSource::ConfigFlag(
                                v.split_at("-c".len()).1.trim().to_owned(),
                            ))
                        }

                        v if v.starts_with("--config-file=") => {
                            Some(RepresentativeConfigFlagSource::ConfigFile(
                                v.split_at("--config-file=".len()).1.to_owned(),
                            ))
                        }
                        v if v.starts_with("--modifier=") || v.starts_with("-m=") => {
                            Some(RepresentativeConfigFlagSource::Modifier(
                                v.split_once("=").unwrap().1.to_owned(),
                            ))
                        }
                        v if v.starts_with("--target-platforms=")
                            || v.starts_with("--platforms=") =>
                        {
                            Some(RepresentativeConfigFlagSource::TargetPlatforms(
                                v.split_once("=").unwrap().1.to_owned(),
                            ))
                        }
                        // The `--` separator indicates the end of Buck flags and the start of args for the target itself.
                        "--" => {
                            state = State::Finished;
                            None
                        }
                        _ => None,
                    },
                    State::Matched(flag) => {
                        state = State::None;
                        match flag {
                            "-c" => {
                                Some(RepresentativeConfigFlagSource::ConfigFlag(value.to_owned()))
                            }
                            "--config-file" => {
                                Some(RepresentativeConfigFlagSource::ConfigFile(value.to_owned()))
                            }
                            "-m" => {
                                Some(RepresentativeConfigFlagSource::Modifier(value.to_owned()))
                            }
                            "--target-platforms" => Some(
                                RepresentativeConfigFlagSource::TargetPlatforms(value.to_owned()),
                            ),
                            _ => unreachable!("impossible flag"),
                        }
                    }
                    State::Finished => None,
                }
                .map(|flag_value| (flag_value, source))
            });

        let mut args: Vec<RepresentativeConfigFlag> = Vec::new();
        let mut last_flagfile = None;

        for (flag_value, source) in config_args {
            let flagfile = match source {
                ExpandedArgSource::Inline => None,
                ExpandedArgSource::Flagfile(file) => get_flagfile_for_logging(&file),
            };

            match flagfile {
                Some(flagfile) => {
                    if Some(flagfile) != last_flagfile {
                        args.push(RepresentativeConfigFlag {
                            source: Some(RepresentativeConfigFlagSource::ModeFile(
                                flagfile.kind.to_string(),
                            )),
                        });
                    }
                }
                None => {
                    args.push(RepresentativeConfigFlag {
                        source: Some(flag_value),
                    });
                }
            }
            last_flagfile = flagfile;
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use kuro_common::argv::ExpandedArgvBuilder;
    use kuro_core::cells::cell_path::CellPath;
    use kuro_core::fs::project::ProjectRootTemp;
    use kuro_fs::paths::forward_rel_path::ForwardRelativePathBuf;

    use super::*;

    #[test]
    fn test_get_representative_config_flags() -> kuro_error::Result<()> {
        let mut argv = ExpandedArgvBuilder::new();

        argv.push("-c".to_owned());
        argv.push("section.option=value".to_owned());

        argv.push("-c section1.option=value".to_owned());
        argv.push("-csection2.option=value".to_owned());

        argv.push("--other-flag".to_owned());
        argv.push("value".to_owned());
        argv.push("--other-flag2".to_owned());
        argv.push("value".to_owned());

        argv.push("--config".to_owned());
        argv.push("section.option2=value".to_owned());

        argv.push("--config=section.option3=value".to_owned());
        argv.push("-c=section.option4=value".to_owned());

        argv.push("--config-file=//1.bcfg".to_owned());
        argv.push("--config-file".to_owned());
        argv.push("//2.bcfg".to_owned());

        argv.push("-m".to_owned());
        argv.push("//bar:baz".to_owned());
        argv.push("-m //bar1:baz".to_owned());
        argv.push("-m//bar2:baz".to_owned());
        argv.push("--modifier=//foo:bar".to_owned());
        argv.push("--modifier".to_owned());
        argv.push("//bar:foo".to_owned());

        argv.push("--target-platforms=ovr_config//platforms/linux:some_linux_platform".to_owned());

        let argv = argv.build();

        let clap = clap::ArgMatches::default(); // we don't actually inspect this right now so just use an empty one.
        let matches = BuckArgMatches::from_clap(&clap, &argv);

        let flags = matches.get_representative_config_flags();

        assert_eq!(
            flags,
            vec![
                "-c section.option=value",
                "-c section1.option=value",
                "-c section2.option=value",
                "-c section.option2=value",
                "-c section.option3=value",
                "-c section.option4=value",
                "--config-file //1.bcfg",
                "--config-file //2.bcfg",
                "-m //bar:baz",
                "-m //bar1:baz",
                "-m //bar2:baz",
                "-m //foo:bar",
                "-m //bar:foo",
                "--target-platforms ovr_config//platforms/linux:some_linux_platform"
            ]
        );

        Ok(())
    }

    #[test]
    fn test_get_representative_config_flags_for_flagfiles() -> kuro_error::Result<()> {
        let project_argfile = |path: &str| ArgFilePath::Project(CellPath::testing_new(path));

        let external_root = ProjectRootTemp::new().unwrap();
        let external_root = external_root.path();
        let external_argfile = |path: &str| {
            ArgFilePath::External(
                external_root
                    .root()
                    .join(ForwardRelativePathBuf::new(path.to_owned()).unwrap()),
            )
        };

        let mut argv = ExpandedArgvBuilder::new();
        argv.push("-m".to_owned());
        argv.push("//bar:baz".to_owned());

        argv.argfile_scope(ArgFileKind::Path(project_argfile("root//mode/1")), |argv| {
            argv.push("-c=a.b=c".to_owned());
            argv.push("-c=a.b2=c".to_owned());
            argv.push("-c=a.b3=c".to_owned());
            argv.push("--modifier=//foo:bar".to_owned());
            argv.push("--modifier".to_owned());
            argv.push("//bar:foo".to_owned());
        });

        argv.argfile_scope(ArgFileKind::Path(external_argfile("mode/1")), |argv| {
            argv.argfile_scope(ArgFileKind::Path(external_argfile("mode/2")), |argv| {
                argv.argfile_scope(ArgFileKind::Path(project_argfile("root//mode/2")), |argv| {
                    argv.push("-c=a.b4=c".to_owned());
                });

                argv.push("-c=a.b5=c".to_owned());
            });
            argv.push("-c=a.b6=c".to_owned());
        });

        // Ignored because other-flag is not a config flag
        argv.argfile_scope(ArgFileKind::Path(project_argfile("root//mode/3")), |argv| {
            argv.push("--other-flag".to_owned());
        });

        let argv = argv.build();

        let clap = clap::ArgMatches::default(); // we don't actually inspect this right now so just use an empty one.
        let matches = BuckArgMatches::from_clap(&clap, &argv);

        let flags = matches.get_representative_config_flags();

        assert_eq!(
            flags,
            vec![
                "-m //bar:baz",
                "@root//mode/1",
                "@root//mode/2",
                "-c a.b5=c",
                "-c a.b6=c"
            ]
        );
        Ok(())
    }

    #[test]
    fn test_get_representative_config_flags_stops_at_double_dash() -> kuro_error::Result<()> {
        let mut argv = ExpandedArgvBuilder::new();

        argv.push("-c".to_owned());
        argv.push("section.option=value".to_owned());

        argv.push("--config".to_owned());
        argv.push("section.option2=value".to_owned());

        // Add the -- separator
        argv.push("--".to_owned());

        // These should be ignored after --
        argv.push("-c".to_owned());
        argv.push("section.ignored=value".to_owned());
        argv.push("--config".to_owned());
        argv.push("section.ignored2=value".to_owned());

        let argv = argv.build();

        let clap = clap::ArgMatches::default();
        let matches = BuckArgMatches::from_clap(&clap, &argv);

        let flags = matches.get_representative_config_flags();

        // Should only include flags before --, not after
        assert_eq!(
            flags,
            vec!["-c section.option=value", "-c section.option2=value",]
        );

        Ok(())
    }
}
