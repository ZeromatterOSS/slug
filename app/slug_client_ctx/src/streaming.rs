/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use async_trait::async_trait;
use dupe::Dupe;
use slug_common::argv::Argv;
use slug_common::argv::SanitizedArgv;
use slug_common::init::DEFAULT_RETAINED_EVENT_LOGS;
use slug_common::init::ReConfigSnapshot;
use slug_common::invocation_paths::InvocationPaths;
use slug_error::ExitCode;
use slug_event_observer::span_tracker::EventTimestamp;

use crate::client_ctx::BuckSubcommand;
use crate::client_ctx::ClientCommandContext;
use crate::common::BuckArgMatches;
use crate::common::CommonBuildConfigurationOptions;
use crate::common::CommonEventLogOptions;
use crate::common::CommonStarlarkOptions;
use crate::common::ui::CommonConsoleOptions;
use crate::common::ui::get_console_with_root;
use crate::daemon::client::BuckdClientConnector;
use crate::daemon::client::connect::BuckdConnectConstraints;
use crate::daemon::client::connect::DaemonConstraintsRequest;
use crate::daemon::client::connect::DesiredTraceIoState;
use crate::daemon::client::connect::connect_buckd;
use crate::events_ctx::EventsCtx;
use crate::exit_result::ExitResult;
use crate::path_arg::PathArg;
use crate::signal_handler::with_simple_sigint_handler;
use crate::subscribers::bep_bes_sink::BesSubscriber;
use crate::subscribers::bep_file_sink::BepFileSubscriber;
use crate::subscribers::build_graph_stats::BuildGraphStats;
use crate::subscribers::build_id_writer::BuildIdWriter;
use crate::subscribers::event_log::EventLog;
use crate::subscribers::health_check_subscriber::HealthCheckSubscriber;
use crate::subscribers::re_log::ReLog;
use crate::subscribers::subscriber::EventSubscriber;
use crate::subscribers::superconsole::timekeeper::RealtimeClock;
use crate::subscribers::superconsole::timekeeper::Timekeeper;

const HEALTH_CHECK_CHANNEL_SIZE: usize = 100;

fn update_events_ctx<T: StreamingCommand>(
    cmd: &T,
    matches: BuckArgMatches<'_>,
    ctx: &ClientCommandContext,
    events_ctx: &mut EventsCtx,
) {
    let console_opts = cmd.console_opts();
    let event_log_opts = cmd.event_log_opts();
    let mut subscribers = vec![];
    let expect_spans = cmd.should_expect_spans();

    let paths = ctx.paths().ok();

    // Need this to get information from one subscriber (event_log)
    // and log it in another (invocation_recorder)
    let log_size_counter_bytes = Some(Arc::new(AtomicU64::new(0)));

    let enable_health_checks = ctx
        .immediate_config
        .daemon_startup_config()
        .map(|daemon_startup_config| {
            daemon_startup_config
                .health_check_config
                .enable_health_checks
        })
        .unwrap_or(false);

    let (
        health_check_tags_receiver,
        health_check_display_reports_receiver,
        health_check_subscriber,
    ) = if enable_health_checks {
        let (tag_tx, tag_rx) = tokio::sync::mpsc::channel(HEALTH_CHECK_CHANNEL_SIZE);
        let (report_tx, report_rx) = tokio::sync::mpsc::channel(HEALTH_CHECK_CHANNEL_SIZE);
        let subscriber = HealthCheckSubscriber::new(tag_tx, report_tx, paths);
        (Some(tag_rx), Some(report_rx), Some(subscriber))
    } else {
        (None, None, None)
    };

    subscribers.push(get_console_with_root(
        ctx.trace_id.dupe(),
        console_opts.console_type,
        ctx.verbosity,
        expect_spans,
        Timekeeper::new(
            Box::new(RealtimeClock),
            EventTimestamp(ctx.start_time.into()),
        ),
        T::COMMAND_NAME,
        console_opts.superconsole_config(),
        health_check_display_reports_receiver,
    ));

    if let Some(paths) = paths {
        let re_log_subscriber = ReLog::new(paths.isolation.clone());
        subscribers.push(Box::new(re_log_subscriber));

        if !event_log_opts.no_event_log {
            let event_log_subscriber =
                get_event_log_subscriber(cmd, ctx, log_size_counter_bytes.clone(), paths);
            subscribers.push(event_log_subscriber);
        }
    }
    if let Some(build_id_writer) = get_build_id_writer(cmd.event_log_opts(), ctx) {
        subscribers.push(build_id_writer)
    }
    if let Some(bep_subscriber) = get_bep_file_subscriber(cmd, ctx) {
        subscribers.push(bep_subscriber);
    }
    if let Some(bes_subscriber) = get_bes_subscriber(cmd, ctx) {
        subscribers.push(bes_subscriber);
    }
    if let Some(build_graph_stats) = get_build_graph_stats(cmd, ctx) {
        subscribers.push(build_graph_stats)
    }
    let representative_config_flags = if ctx.paths().is_ok() {
        matches.get_representative_config_flags()
    } else {
        Vec::new()
    };

    if let Some(recorder) = events_ctx.recorder.as_mut() {
        recorder.update_for_command(
            ctx,
            cmd.event_log_opts(),
            cmd.sanitize_argv(ctx.argv.clone()).argv,
            Some(cmd.build_config_opts()),
            representative_config_flags,
            log_size_counter_bytes,
            health_check_tags_receiver,
            paths,
        );
    }

    if let Some(subscriber) = health_check_subscriber {
        subscribers.push(subscriber);
    }

    subscribers.extend(cmd.extra_subscribers());
    events_ctx.subscribers = subscribers;
}

/// Trait to generalize the behavior of executable slug commands that rely on a server.
/// This trait is most helpful when the command wants a superconsole, to stream events, etc.
/// However, this is the most robustly tested of our code paths, and there is little cost to defaulting to it.
/// As a result, prefer to default to streaming mode unless there is a compelling reason not to
/// (e.g `status`)
#[async_trait(?Send)]
pub trait StreamingCommand: Sized + Send + Sync {
    const COMMAND_NAME: &'static str;

    /// Run the command.
    async fn exec_impl(
        self,
        buckd: &mut BuckdClientConnector,
        matches: BuckArgMatches<'_>,
        ctx: &mut ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult;

    /// Should we only connect to existing servers (`true`), or spawn a new server if required (`false`).
    /// Defaults to `false`.
    fn existing_only() -> bool {
        false
    }

    fn trace_io(&self) -> DesiredTraceIoState {
        DesiredTraceIoState::Existing
    }

    fn console_opts(&self) -> &CommonConsoleOptions;

    fn event_log_opts(&self) -> &CommonEventLogOptions;

    fn build_config_opts(&self) -> &CommonBuildConfigurationOptions;

    fn starlark_opts(&self) -> &CommonStarlarkOptions;

    fn extra_subscribers(&self) -> Vec<Box<dyn EventSubscriber>> {
        vec![]
    }

    fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }

    /// Some commands should always be displaying
    /// at least 1 ongoing process in the terminal, aka "spans".
    /// In the simple console, we want to display a "Waiting for daemon..." message
    /// exclusively for these commands whenever there are long periods without spans.
    fn should_expect_spans(&self) -> bool {
        true
    }

    /// Currently only for BxlCommand.
    fn user_event_log(&self) -> &Option<PathArg> {
        &None
    }
}

impl<T: StreamingCommand> BuckSubcommand for T {
    const COMMAND_NAME: &'static str = T::COMMAND_NAME;

    /// Actual call that runs a `StreamingCommand`.
    /// Handles the business of setting up a server connection for streaming.
    async fn exec_impl(
        self,
        matches: BuckArgMatches<'_>,
        mut ctx: ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let work = async {
            let mut constraints = if T::existing_only() {
                BuckdConnectConstraints::ExistingOnly
            } else {
                let mut req =
                    DaemonConstraintsRequest::new(ctx.immediate_config, T::trace_io(&self))?;
                ctx.restarter.apply_to_constraints(&mut req);
                // Layer Bazel-shape RE flags (`--remote_executor`,
                // `--remote_header`, …) onto the daemon-startup snapshot
                // before the constraint check. Any change here forces a
                // `ConstraintMismatchStartupConfig` daemon restart, so the
                // newly-spawned daemon picks up the user's updated RE
                // backend instead of silently keeping its prior one.
                if let Some(cli_re) = self.build_config_opts().cli_re_config_snapshot() {
                    req.daemon_startup_config.re_config =
                        merge_re_config(&req.daemon_startup_config.re_config, &cli_re);
                }
                // `--digest_function` overrides the daemon's startup digest
                // algorithm. Layered here (rather than during config parse)
                // so the constraint check sees the change and restarts a
                // daemon running with the wrong digest.
                if let Some(digest) = self
                    .build_config_opts()
                    .digest_function
                    .as_deref()
                    .filter(|s| !s.is_empty())
                {
                    req.daemon_startup_config.digest_algorithms = Some(digest.to_owned());
                }
                if let Some(watcher) = self
                    .build_config_opts()
                    .slug_file_watcher
                    .as_deref()
                    .filter(|s| !s.is_empty())
                {
                    req.daemon_startup_config.file_watcher = Some(watcher.to_owned());
                }
                BuckdConnectConstraints::Constraints(req)
            };
            let buckd = match ctx.start_in_process_daemon.take() {
                None => connect_buckd(constraints, events_ctx, ctx.paths()?).await,
                Some(start_in_process_daemon) => {
                    // Start in-process daemon, wait until it is ready to accept connections.
                    start_in_process_daemon()?;

                    // Do not attempt to spawn a daemon if connect failed.
                    // Connect should not fail.
                    constraints = BuckdConnectConstraints::ExistingOnly;

                    connect_buckd(constraints, events_ctx, ctx.paths()?).await
                }
            };

            let mut buckd = match buckd {
                Ok(buckd) => buckd,
                Err(e) => {
                    return ExitResult::err_with_exit_code(e, ExitCode::ConnectError);
                }
            };

            let command_result = self
                .exec_impl(&mut buckd, matches, &mut ctx, events_ctx)
                .await;

            ctx.restarter.observe(&buckd, events_ctx);

            command_result
        };

        // FIXME: move this into client_ctx
        with_simple_sigint_handler(work)
            .await
            .unwrap_or_else(ExitResult::signal_interrupt)
    }

    fn update_events_ctx(
        &self,
        matches: BuckArgMatches<'_>,
        ctx: &ClientCommandContext,
        events_ctx: &mut EventsCtx,
    ) {
        update_events_ctx(self, matches, ctx, events_ctx);
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        self.event_log_opts()
    }

    fn logging_name(&self) -> &'static str {
        Self::COMMAND_NAME
    }
}

/// Given the command arguments, conditionally create an event log.
fn get_event_log_subscriber<T: StreamingCommand>(
    cmd: &T,
    ctx: &ClientCommandContext,
    log_size_counter_bytes: Option<Arc<AtomicU64>>,
    paths: &InvocationPaths,
) -> Box<dyn EventSubscriber> {
    let event_log_opts = cmd.event_log_opts();
    let sanitized_argv = cmd.sanitize_argv(ctx.argv.clone());
    let user_event_log = cmd.user_event_log();

    let logdir = paths.log_dir();
    let log = EventLog::new(
        logdir,
        ctx.working_dir.clone(),
        event_log_opts
            .event_log
            .as_ref()
            .map(|p| p.resolve(&ctx.working_dir)),
        user_event_log.as_ref().map(|p| p.resolve(&ctx.working_dir)),
        sanitized_argv,
        T::COMMAND_NAME.to_owned(),
        ctx.start_time,
        log_size_counter_bytes,
        ctx.immediate_config
            .daemon_startup_config()
            .map(|daemon_startup_config| daemon_startup_config.retained_event_logs)
            .unwrap_or(DEFAULT_RETAINED_EVENT_LOGS),
    );
    Box::new(log)
}

fn get_build_id_writer(
    opts: &CommonEventLogOptions,
    ctx: &ClientCommandContext,
) -> Option<Box<dyn EventSubscriber>> {
    if let Some(file_loc) = opts.write_build_id.as_ref() {
        Some(Box::new(BuildIdWriter::new(
            file_loc.resolve(&ctx.working_dir),
        )))
    } else {
        None
    }
}

fn get_build_graph_stats<T: StreamingCommand>(
    cmd: &T,
    ctx: &ClientCommandContext,
) -> Option<Box<dyn EventSubscriber>> {
    if should_handle_build_graph_stats(cmd) {
        Some(Box::new(BuildGraphStats::new(
            ctx.fbinit(),
            ctx.trace_id.dupe(),
        )))
    } else {
        None
    }
}

/// Build the BEP file-output subscriber from CLI flags
/// (`--build_event_binary_file`, `--build_event_text_file`).
///
/// Returns `None` when neither flag is set, so the subscriber stack pays zero
/// cost for invocations that don't ask for BEP output.
fn get_bep_file_subscriber<T: StreamingCommand>(
    cmd: &T,
    ctx: &ClientCommandContext,
) -> Option<Box<dyn EventSubscriber>> {
    let opts = cmd.build_config_opts();
    let binary_path = opts
        .build_event_binary_file
        .as_ref()
        .map(|p| resolve_relative_path(p, ctx));
    let text_path = opts
        .build_event_text_file
        .as_ref()
        .map(|p| resolve_relative_path(p, ctx));

    let build_event_ctx = bep_build_event_context::<T>(cmd, ctx);

    match BepFileSubscriber::maybe_new(binary_path, text_path, build_event_ctx) {
        Ok(Some(sub)) => Some(Box::new(sub)),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("BEP file subscriber disabled: {e}");
            None
        }
    }
}

/// Build a `BuildEventContext` from invocation-level state. Shared between
/// the file and BES subscribers.
fn bep_build_event_context<T: StreamingCommand>(
    cmd: &T,
    ctx: &ClientCommandContext,
) -> slug_build_event_stream::translate::BuildEventContext {
    let workspace_directory = ctx
        .paths()
        .ok()
        .map(|p| p.project_root().root().to_string())
        .unwrap_or_default();
    // Best-effort root cell name from the workspace directory's basename.
    // Slug's `TargetPattern.value` records patterns as `<cell>//pkg:name`;
    // Bazel's BEP emits root-cell patterns without the prefix. The
    // translator strips this prefix from PatternExpanded ids so
    // BuildBuddy's pattern-extraction code finds something to display.
    // If the actual cell name in `MODULE.bazel` differs from the workspace
    // directory name, the prefix won't strip and BB will see slug's
    // internal form — visually wrong but not broken. Revisit when we
    // have a real cell-resolver hookup on the client side.
    let root_cell_name = std::path::Path::new(&workspace_directory)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_owned();
    // Use the slug crate version (`0.1.0` etc.) so the BuildBuddy
    // invocation header shows something other than `vunknown`. If the
    // revision is also baked in (release builds set this via
    // `slug_build_info::revision()`) append it for traceability.
    let slug_version = match slug_build_info::revision() {
        Some(rev) => format!("slug {} ({})", env!("CARGO_PKG_VERSION"), rev),
        None => format!("slug {}", env!("CARGO_PKG_VERSION")),
    };
    slug_build_event_stream::translate::BuildEventContext {
        invocation_id: ctx.trace_id.to_string(),
        build_tool_version: slug_version,
        root_cell_name,
        workspace_directory,
        working_directory: ctx.working_dir.path().to_string(),
        user: std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_default(),
        host: hostname::get()
            .ok()
            .and_then(|s| s.into_string().ok())
            .unwrap_or_default(),
        command: T::COMMAND_NAME.to_owned(),
        cli_args: cmd.sanitize_argv(ctx.argv.clone()).argv,
        server_pid: 0,
    }
}

fn resolve_relative_path(path: &str, ctx: &ClientCommandContext) -> std::path::PathBuf {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        ctx.working_dir.path().as_path().join(p)
    }
}

/// Build the BES gRPC subscriber. Connection is deferred to the first
/// event so we can return synchronously from here; a failing connection
/// logs a warning and becomes a no-op.
fn get_bes_subscriber<T: StreamingCommand>(
    cmd: &T,
    ctx: &ClientCommandContext,
) -> Option<Box<dyn EventSubscriber>> {
    let opts = cmd.build_config_opts();
    if opts.bes_backend.is_none() {
        return None;
    }

    // Announce the results URL once, up front, so CI scrapers can find it in
    // both success and failure paths.
    BesSubscriber::log_results_url(opts.bes_results_url.as_deref(), &ctx.trace_id.to_string());

    let build_event_ctx = bep_build_event_context::<T>(cmd, ctx);
    match BesSubscriber::maybe_new(opts, build_event_ctx) {
        Ok(Some(sub)) => Some(Box::new(sub)),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("BES subscriber disabled: {e}");
            None
        }
    }
}

/// Layer the CLI-derived RE snapshot onto the buckconfig-derived one.
/// Set fields on the CLI side win; the buckconfig values stand in for
/// anything the CLI didn't override. Used at constraint-check time so
/// `--remote_executor` etc. force a daemon restart with the user's
/// requested RE backend.
fn merge_re_config(base: &ReConfigSnapshot, cli: &ReConfigSnapshot) -> ReConfigSnapshot {
    ReConfigSnapshot {
        address: cli.address.clone().or_else(|| base.address.clone()),
        cas_address: cli.cas_address.clone().or_else(|| base.cas_address.clone()),
        engine_address: cli
            .engine_address
            .clone()
            .or_else(|| base.engine_address.clone()),
        action_cache_address: cli
            .action_cache_address
            .clone()
            .or_else(|| base.action_cache_address.clone()),
        tls: cli.tls.or(base.tls),
        tls_client_cert: cli
            .tls_client_cert
            .clone()
            .or_else(|| base.tls_client_cert.clone()),
        http_headers: if cli.http_headers.is_empty() {
            base.http_headers.clone()
        } else {
            cli.http_headers.clone()
        },
        instance_name: cli
            .instance_name
            .clone()
            .or_else(|| base.instance_name.clone()),
        default_exec_properties: if cli.default_exec_properties.is_empty() {
            base.default_exec_properties.clone()
        } else {
            cli.default_exec_properties.clone()
        },
    }
}

fn should_handle_build_graph_stats<T: StreamingCommand>(cmd: &T) -> bool {
    // Currently, we only care about graph size info in BuildResponse which build command produces
    cmd.build_config_opts()
        .config_values
        .contains(&"buck2.log_configured_graph_size=true".to_owned())
        && cmd.logging_name() == "build"
}
