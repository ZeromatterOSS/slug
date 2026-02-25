/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![feature(error_generic_member_access)]
#![feature(used_with_arg)]

use std::thread;

use clap::CommandFactory;
use clap::FromArgMatches;
use dupe::Dupe;
use kuro_client::commands::build::BuildCommand;
use kuro_client::commands::bxl::BxlCommand;
use kuro_client::commands::clean::CleanCommand;
use kuro_client::commands::ctargets::ConfiguredTargetsCommand;
use kuro_client::commands::expand_external_cell::ExpandExternalCellsCommand;
use kuro_client::commands::explain::ExplainCommand;
use kuro_client::commands::help_env::HelpEnvCommand;
use kuro_client::commands::init::InitCommand;
use kuro_client::commands::install::InstallCommand;
use kuro_client::commands::kill::KillCommand;
use kuro_client::commands::killall::KillallCommand;
use kuro_client::commands::lsp::LspCommand;
use kuro_client::commands::profile::ProfileCommand;
use kuro_client::commands::query::aquery::AqueryCommand;
use kuro_client::commands::query::cquery::CqueryCommand;
use kuro_client::commands::query::uquery::UqueryCommand;
use kuro_client::commands::info::InfoCommand;
use kuro_client::commands::root::RootCommand;
use kuro_client::commands::run::RunCommand;
use kuro_client::commands::server::ServerCommand;
use kuro_client::commands::status::StatusCommand;
use kuro_client::commands::subscribe::SubscribeCommand;
use kuro_client::commands::targets::TargetsCommand;
use kuro_client::commands::test::TestCommand;
use kuro_client_ctx::argfiles::expand_argv;
use kuro_client_ctx::bazelrc::inject_bazelrc_args;
use kuro_client_ctx::client_ctx::BuckSubcommand;
use kuro_client_ctx::client_ctx::ClientCommandContext;
use kuro_client_ctx::client_metadata::ClientMetadata;
use kuro_client_ctx::client_metadata::parse_client_metadata;
use kuro_client_ctx::common::BuckArgMatches;
use kuro_client_ctx::exit_result::ExitResult;
use kuro_client_ctx::immediate_config::ImmediateConfigContext;
use kuro_client_ctx::version::BuckVersion;
use kuro_cmd_audit_client::AuditCommand;
use kuro_cmd_debug_client::DebugCommand;
use kuro_cmd_log_client::LogCommand;
use kuro_cmd_rage_client::rage::RageCommand;
use kuro_cmd_starlark_client::StarlarkCommand;
use kuro_common::argv::Argv;
use kuro_common::invocation_paths_result::InvocationPathsResult;
use kuro_common::invocation_roots::get_invocation_paths_result;
use kuro_core::kuro_env;
use kuro_data::ErrorReport;
use kuro_error::BuckErrorContext;
use kuro_error::ErrorTag;
use kuro_error::ExitCode;
use kuro_error::conversion::clap::buck_error_clap_parser;
use kuro_error::kuro_error;
use kuro_event_observer::verbosity::Verbosity;
use kuro_fs::paths::file_name::FileNameBuf;
use kuro_util::threads::thread_spawn_scoped;

use crate::check_user_allowed::check_user_allowed;
use crate::process_context::ProcessContext;

mod check_user_allowed;
mod cli_style;
pub(crate) mod commands;
pub mod panic;
pub mod process_context;

fn parse_isolation_dir(s: &str) -> kuro_error::Result<FileNameBuf> {
    FileNameBuf::try_from(s.to_owned()).buck_error_context("isolation dir must be a directory name")
}

/// Options of `kuro` command, before subcommand.
#[derive(Clone, Debug, clap::Parser)]
#[clap(next_help_heading = "Universal Options")]
struct BeforeSubcommandOptions {
    /// The name of the directory that Kuro creates within buck-out for writing outputs and daemon
    /// information. If one is not provided, Kuro creates a directory with the default name.
    ///
    /// Instances of Kuro share a daemon if and only if their isolation directory is identical.
    /// The isolation directory also influences the output paths provided by Kuro,
    /// and as a result using a non-default isolation dir will cause cache misses (and slower builds).
    #[clap(
        value_parser = buck_error_clap_parser(parse_isolation_dir),
        env("BUCK_ISOLATION_DIR"),
        long,
        global = true,
        default_value="v2"
    )]
    isolation_dir: FileNameBuf,

    /// How verbose buck should be while logging.
    ///
    /// Values:
    /// 0 = Quiet, errors only;
    /// 1 = Show status. Default;
    /// 2 = more info about errors;
    /// 3 = more info about everything;
    /// 4 = more info about everything + stderr;
    ///
    /// It can be combined with specific log items (stderr, full_failed_command, commands, actions,
    /// status, stats, success) to fine-tune the verbosity of the log. Example usage "-v=1,stderr"
    #[clap(
        short = 'v',
        long = "verbose",
        default_value = "1",
        global = true,
        value_parser= buck_error_clap_parser(Verbosity::try_from_cli)
    )]
    verbosity: Verbosity,

    /// The oncall executing this command
    #[clap(long, global = true)]
    oncall: Option<String>,

    /// Metadata key-value pairs to inject into Kuro's logging. Client metadata must be of the
    /// form `key=value`, where `key` is a snake_case identifier, and will be sent to backend
    /// datasets.
    #[clap(long, global = true, value_parser = buck_error_clap_parser(parse_client_metadata))]
    client_metadata: Vec<ClientMetadata>,

    /// Do not launch a daemon process, run buck server in client process.
    ///
    /// Note even when running in no-buckd mode, it still writes state files.
    /// In particular, this command effectively kills buckd process
    /// running with the same isolation directory.
    ///
    /// This is an unsupported option used only for development work.
    #[clap(env("BUCK2_NO_BUCKD"), long, global(true), hide(true))]
    // Env var is BUCK2_NO_BUCKD instead of NO_BUCKD env var from buck1 because no buckd
    // is not supported for production work for kuro and lots of places already set
    // NO_BUCKD=1 for buck1.
    no_buckd: bool,

    /// Print buck wrapper help.
    #[clap(skip)] // @oss-enable
    // @oss-disable: #[clap(long)]
    help_wrapper: bool,

    /// Disable loading of .bazelrc files (Bazel compatibility).
    ///
    /// When set, kuro will not load ~/.bazelrc or <workspace>/.bazelrc.
    /// Equivalent to Bazel's --nobazelrc startup option.
    #[clap(long, global(true), hide(true))]
    nobazelrc: bool,

    /// Path to a .bazelrc file to load instead of the default locations,
    /// or "none" to disable .bazelrc loading entirely (Bazel compatibility).
    ///
    /// Equivalent to Bazel's --bazelrc startup option.
    #[clap(long, global(true), hide(true), value_name = "PATH")]
    bazelrc: Option<String>,
}

#[rustfmt::skip] // Formatting in internal and in OSS versions disagree after oss markers applied.
fn help() -> &'static str {
    concat!(
        "A build system\n",
        "\n",
        "Documentation: https://kuro.build/docs/\n", // @oss-enable
        // @oss-disable: "Documentation: https://internalfb.com/intern/staticdocs/kuro/docs/\n",
    )
}

#[derive(Debug, clap::Parser)]
#[clap(
    name = "kuro",
    about(Some(help())),
    version(BuckVersion::get_version()),
    styles = cli_style::get_styles(),
)]
pub(crate) struct Opt {
    #[clap(subcommand)]
    cmd: CommandKind,
    #[clap(flatten)]
    common_opts: BeforeSubcommandOptions,
}

impl Opt {
    pub(crate) fn exec(
        self,
        process: ProcessContext<'_>,
        immediate_config: &ImmediateConfigContext,
        matches: BuckArgMatches<'_>,
        argv: Argv,
    ) -> ExitResult {
        let subcommand_matches = matches.unwrap_subcommand();

        self.cmd.exec(
            process,
            immediate_config,
            subcommand_matches,
            argv,
            self.common_opts,
        )
    }
}

pub fn exec(process: ProcessContext<'_>) -> ExitResult {
    let cwd = process.shared.working_dir.clone();
    let mut immediate_config = ImmediateConfigContext::new(&cwd);
    let arg0_override = kuro_env!("BUCK2_ARG0")?;

    // Find the project root early so we can load .bazelrc from the workspace.
    // This mirrors Bazel's behavior: .bazelrc files are read before argument expansion.
    let project_root = kuro_common::invocation_roots::find_invocation_roots(&cwd)
        .ok()
        .map(|roots| roots.project_root.root().as_path().to_path_buf());

    // Inject .bazelrc flags before argfile expansion. The bazelrc flags are
    // inserted after the subcommand, so explicit command-line flags take precedence.
    // Pass through `--nobazelrc` / `--bazelrc=none` by scanning raw args early.
    let raw_args = inject_bazelrc_args(
        process.shared.args.to_vec(),
        project_root.as_deref(),
    );

    let expanded_args = expand_argv(
        arg0_override,
        raw_args,
        &mut immediate_config,
        &cwd,
    )
    .buck_error_context("Error expanding argsfiles")?;

    let argv = Argv {
        argv: process.shared.args.to_vec(),
        expanded_argv: expanded_args,
    };

    let clap = Opt::command();
    let matches = match clap.try_get_matches_from(argv.expanded_argv.args()) {
        Ok(matches) => matches,
        Err(e) => {
            // Print colorized output, ExitResult::report will not colorize
            e.print()?;
            return if e.exit_code() == 0 {
                ExitResult::success()
            } else {
                let e = kuro_error::Error::from(e).tag([ErrorTag::ClapMatch]);
                ExitResult::status_with_emitted_errors(
                    ExitCode::UserError,
                    vec![ErrorReport::from(&e)],
                )
            };
        }
    };
    let mut opt = ParsedArgv::parse(argv, matches)?;

    let client_metadata = ClientMetadata::from_env()?;
    if !client_metadata.is_empty() {
        // insert the `client_metadata` at the beginning of the list, so that the client id metadata from the env var could be overridden by the cli arg
        opt.opt
            .common_opts
            .client_metadata
            .splice(0..0, client_metadata);
    }

    // If --client-metadata=? was not set and from_env did not find "id", then
    // if we are running in a terminal, we add id=terminal-fallback to
    // opt.opt.common_opts.client_metadata to transmit to scuba that the client
    // is an end user: https://fburl.com/scuba/kuro_builds/n4klo51d
    let has_client_id = opt
        .opt
        .common_opts
        .client_metadata
        .iter()
        .any(|m| m.key == "id");

    if !has_client_id {
        use std::io::IsTerminal;
        let client_id = if std::io::stdin().is_terminal() {
            Some("terminal-fallback")
        } else {
            // Check if running from VSCode
            let is_vscode = std::env::var("VSCODE_PID")
                .ok()
                .is_some_and(|v| !v.is_empty())
                || std::env::var("TERM_PROGRAM").ok().as_deref() == Some("vscode");
            if is_vscode {
                Some("vscode-fallback")
            } else {
                None
            }
        };

        if let Some(val) = client_id {
            opt.opt.common_opts.client_metadata.push(ClientMetadata {
                key: "id".to_owned(),
                value: val.to_owned(),
            });
        }
    }
    opt.exec(process, &immediate_config)
}

struct ParsedArgv {
    opt: Opt,
    argv: Argv,
    matches: clap::ArgMatches,
}

impl ParsedArgv {
    fn parse(argv: Argv, matches: clap::ArgMatches) -> kuro_error::Result<Self> {
        let opt: Opt = Opt::from_arg_matches(&matches)?;

        if opt.common_opts.help_wrapper {
            return Err(kuro_error!(
                kuro_error::ErrorTag::Tier0,
                "`--help-wrapper` should have been handled by the wrapper"
            ));
        }

        match &opt.cmd {
            #[cfg(not(client_only))]
            CommandKind::Daemon(..) | CommandKind::Forkserver(..) => {}
            CommandKind::Clean(..) => {}
            _ => {
                check_user_allowed()?;
            }
        }

        Ok(ParsedArgv { opt, argv, matches })
    }

    fn exec(
        self,
        process: ProcessContext<'_>,
        immediate_config: &ImmediateConfigContext,
    ) -> ExitResult {
        let expanded_args = self.argv.expanded_argv.clone();
        self.opt.exec(
            process,
            &immediate_config,
            BuckArgMatches::from_clap(&self.matches, &expanded_args),
            self.argv,
        )
    }
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum CommandKind {
    #[cfg(not(client_only))]
    #[clap(hide = true)]
    Daemon(kuro_daemon::daemon::DaemonCommand),
    #[cfg(not(client_only))]
    #[clap(hide = true)]
    Forkserver(crate::commands::forkserver::ForkserverCommand),
    #[cfg(not(client_only))]
    #[clap(hide = true)]
    InternalTestRunner(crate::commands::internal_test_runner::InternalTestRunnerCommand),
    #[clap(subcommand)]
    Audit(AuditCommand),
    Aquery(AqueryCommand),
    Build(BuildCommand),
    Bxl(BxlCommand),
    // TODO(nga): implement `kuro help-buckconfig` too
    //   https://www.internalfb.com/tasks/?t=183528129
    HelpEnv(HelpEnvCommand),
    Test(TestCommand),
    Cquery(CqueryCommand),
    Init(InitCommand),
    Explain(ExplainCommand),
    ExpandExternalCell(ExpandExternalCellsCommand),
    Install(InstallCommand),
    Kill(KillCommand),
    Killall(KillallCommand),
    Root(RootCommand),
    /// Print workspace build information (Bazel `bazel info` compatibility)
    Info(InfoCommand),
    /// Alias for `uquery`.
    Query(UqueryCommand),
    Run(RunCommand),
    Server(ServerCommand),
    Status(StatusCommand),
    #[clap(subcommand)]
    Starlark(StarlarkCommand),
    /// Alias for `utargets`.
    Targets(TargetsCommand),
    Utargets(TargetsCommand),
    Ctargets(ConfiguredTargetsCommand),
    Uquery(UqueryCommand),
    #[clap(subcommand, hide = true)]
    Debug(DebugCommand),
    #[clap(hide = true)]
    Complete(kuro_cmd_completion_client::complete::CompleteCommand),
    Completion(kuro_cmd_completion_client::completion::CompletionCommand),
    Docs(kuro_cmd_docs_client::DocsCommand),
    #[clap(subcommand)]
    Profile(ProfileCommand),
    #[clap(hide(true))] // @oss-enable
    Rage(RageCommand),
    Clean(CleanCommand),
    #[clap(subcommand)]
    Log(LogCommand),
    Lsp(LspCommand),
    Subscribe(SubscribeCommand),
}

impl CommandKind {
    pub(crate) fn exec(
        self,
        process: ProcessContext<'_>,
        immediate_config: &ImmediateConfigContext,
        matches: BuckArgMatches<'_>,
        argv: Argv,
        common_opts: BeforeSubcommandOptions,
    ) -> ExitResult {
        let paths_result = get_invocation_paths_result(
            &process.shared.working_dir,
            common_opts.isolation_dir.clone(),
        );

        // Handle the daemon command earlier: it wants to fork, but the things we do below might
        // want to create threads.
        #[cfg(not(client_only))]
        if let CommandKind::Daemon(cmd) = self {
            process.events_ctx.log_invocation_record = false;
            return cmd
                .exec(
                    process.shared.log_reload_handle.dupe(),
                    paths_result.get_result()?,
                    false,
                    || {},
                )
                .into();
        }
        thread::scope(|scope| {
            // Spawn a thread to have stack size independent on linker/environment.
            match thread_spawn_scoped("kuro-main", scope, move || {
                self.exec_no_daemon(
                    common_opts,
                    process,
                    immediate_config,
                    matches,
                    argv,
                    paths_result,
                )
            }) {
                Ok(t) => match t.join() {
                    Ok(res) => res,
                    Err(_) => ExitResult::bail("Main thread panicked"),
                },
                Err(e) => ExitResult::bail(format_args!("Failed to start main thread: {e}")),
            }
        })
    }

    fn exec_no_daemon(
        self,
        common_opts: BeforeSubcommandOptions,
        process: ProcessContext<'_>,
        immediate_config: &ImmediateConfigContext,
        matches: BuckArgMatches<'_>,
        argv: Argv,
        paths: InvocationPathsResult,
    ) -> ExitResult {
        if common_opts.no_buckd {
            // `no_buckd` can't work in a client-only binary
            if let Some(res) = ExitResult::retry_command_with_full_binary()? {
                return res;
            }
        }

        let fb = kuro_common::fbinit::get_or_init_fbcode_globals();

        let ProcessContext {
            trace_id,
            events_ctx,
            shared,
            runtime,
            start_time,
        } = process;

        let runtime = runtime.get_or_init()?;

        let start_in_process_daemon = if common_opts.no_buckd {
            #[cfg(not(client_only))]
            let v = kuro_daemon::no_buckd::start_in_process_daemon(
                immediate_config.daemon_startup_config()?,
                paths.clone().get_result()?,
                &runtime,
            )?;
            #[cfg(client_only)]
            let v = unreachable!(); // case covered above
            #[allow(dead_code)]
            v
        } else {
            None
        };

        let command_ctx = ClientCommandContext::new(
            fb,
            immediate_config,
            paths,
            shared.working_dir.clone(),
            common_opts.verbosity,
            start_time,
            start_in_process_daemon,
            argv,
            trace_id.dupe(),
            &mut shared.stdin,
            &mut shared.restarter,
            &runtime,
            common_opts.oncall,
            common_opts.client_metadata,
            common_opts.isolation_dir,
        );
        if let Some(recorder) = events_ctx.recorder.as_mut() {
            recorder.update_for_client_ctx(&command_ctx, self.command_name());
        }

        match self {
            #[cfg(not(client_only))]
            CommandKind::Daemon(..) => unreachable!("Checked earlier"),
            #[cfg(not(client_only))]
            CommandKind::Forkserver(cmd) => cmd.exec(
                matches,
                command_ctx,
                events_ctx,
                shared.log_reload_handle.dupe(),
            ),
            #[cfg(not(client_only))]
            CommandKind::InternalTestRunner(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Aquery(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Build(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Bxl(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Test(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Cquery(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::HelpEnv(cmd) => cmd.exec(matches, command_ctx),
            CommandKind::Kill(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Killall(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Clean(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Root(cmd) => cmd.exec(matches, command_ctx).into(),
            CommandKind::Info(cmd) => cmd.exec(matches, command_ctx).into(),
            CommandKind::Query(cmd) => {
                kuro_client_ctx::eprintln!(
                    "WARNING: \"kuro query\" is an alias for \"kuro uquery\". Consider using \"kuro cquery\" or \"kuro uquery\" explicitly."
                )?;
                command_ctx.exec(cmd, matches, events_ctx)
            }
            CommandKind::Server(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Status(cmd) => cmd.exec(matches, command_ctx).into(),
            CommandKind::Targets(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Utargets(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Ctargets(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Audit(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Starlark(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Run(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Uquery(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Debug(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Complete(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Completion(cmd) => cmd.exec(Opt::command(), matches, command_ctx),
            CommandKind::Docs(cmd) => cmd.exec(Opt::command(), matches, command_ctx, events_ctx),
            CommandKind::Profile(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Rage(cmd) => cmd.exec(matches, command_ctx),
            CommandKind::Init(cmd) => cmd.exec(matches, command_ctx),
            CommandKind::Explain(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Install(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Log(cmd) => cmd.exec(matches, command_ctx, events_ctx),
            CommandKind::Lsp(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::Subscribe(cmd) => command_ctx.exec(cmd, matches, events_ctx),
            CommandKind::ExpandExternalCell(cmd) => command_ctx.exec(cmd, matches, events_ctx),
        }
    }

    fn command_name(&self) -> &'static str {
        match self {
            #[cfg(not(client_only))]
            CommandKind::Daemon(_) => "daemon",
            #[cfg(not(client_only))]
            CommandKind::Forkserver(_) => "forkserver",
            #[cfg(not(client_only))]
            CommandKind::InternalTestRunner(_) => "internal-test-runner",
            CommandKind::Aquery(cmd) => cmd.logging_name(),
            CommandKind::Build(cmd) => cmd.logging_name(),
            CommandKind::Bxl(cmd) => cmd.logging_name(),
            CommandKind::Test(cmd) => cmd.logging_name(),
            CommandKind::Cquery(cmd) => cmd.logging_name(),
            CommandKind::HelpEnv(_) => "help-env",
            CommandKind::Kill(cmd) => cmd.logging_name(),
            CommandKind::Killall(cmd) => cmd.logging_name(),
            CommandKind::Clean(cmd) => cmd.command_name(),
            CommandKind::Root(_) => "root",
            CommandKind::Info(_) => "info",
            CommandKind::Query(cmd) => cmd.logging_name(),
            CommandKind::Server(cmd) => cmd.logging_name(),
            CommandKind::Status(_) => "status",
            CommandKind::Targets(cmd) => cmd.logging_name(),
            CommandKind::Utargets(cmd) => cmd.logging_name(),
            CommandKind::Ctargets(cmd) => cmd.logging_name(),
            CommandKind::Audit(cmd) => cmd.logging_name(),
            CommandKind::Starlark(cmd) => cmd.command_name(),
            CommandKind::Run(cmd) => cmd.logging_name(),
            CommandKind::Uquery(cmd) => cmd.logging_name(),
            CommandKind::Debug(_) => "debug",
            CommandKind::Complete(_) => "complete",
            CommandKind::Completion(_) => "completion",
            CommandKind::Docs(_) => "docs",
            CommandKind::Profile(_) => "profile",
            CommandKind::Rage(_) => "rage",
            CommandKind::Init(_) => "init",
            CommandKind::Explain(cmd) => cmd.logging_name(),
            CommandKind::Install(cmd) => cmd.logging_name(),
            CommandKind::Log(cmd) => cmd.command_name(),
            CommandKind::Lsp(cmd) => cmd.logging_name(),
            CommandKind::Subscribe(cmd) => cmd.logging_name(),
            CommandKind::ExpandExternalCell(cmd) => cmd.logging_name(),
        }
    }
}
