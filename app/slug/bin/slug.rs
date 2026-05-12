/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![feature(used_with_arg)]

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use dupe::Dupe;
use slug::exec;
use slug::panic;
use slug::process_context::ClientRuntime;
use slug::process_context::ProcessContext;
use slug::process_context::SharedProcessContext;
use slug_build_info::BUCK2_BUILD_INFO;
use slug_build_info::SlugBuildInfo;
use slug_client_ctx::events_ctx::EventsCtx;
use slug_client_ctx::exit_result::ExitResult;
use slug_client_ctx::restarter::Restarter;
use slug_client_ctx::stdin::Stdin;
use slug_client_ctx::stdio;
use slug_client_ctx::subscribers::recorder::InvocationRecorder;
use slug_core::slug_env;
use slug_core::logging::LogConfigurationReloadHandle;
use slug_core::logging::init_tracing_for_writer;
use slug_core::logging::log_file::TracingLogFile;
use slug_fs::working_dir::AbsWorkingDir;
use slug_wrapper_common::invocation_id::TraceId;

// fbcode likes to set its own allocator in fbcode.default_allocator
// So when we set our own allocator, buck build slug or slug build slug often breaks.
// Making jemalloc the default only when we do a cargo build.
#[global_allocator]
#[cfg(all(any(target_os = "linux", target_os = "macos"), not(buck_build)))]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
#[global_allocator]
#[cfg(target_os = "windows")]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn init_logging() -> slug_error::Result<Arc<dyn LogConfigurationReloadHandle>> {
    static ENV_TRACING_LOG_FILE_PATH: &str = "BUCK_LOG_TO_FILE_PATH";

    let handle = match std::env::var_os(ENV_TRACING_LOG_FILE_PATH) {
        Some(path) => {
            let path = PathBuf::from(path);
            // we set the writer to stderr first until later, when we have the logdir, set the
            // tracing log sink to that file

            fs::create_dir_all(&path)?;
            let tracing_log = path.join("tracing_log");
            let file = TracingLogFile::new(tracing_log)?;
            init_tracing_for_writer(file)
        }
        _ => init_tracing_for_writer(io::stderr),
    }?;

    #[cfg(fbcode_build)]
    {
        use slug_event_log::should_upload_log;
        use slug_events::sink::remote;

        if !should_upload_log()? {
            remote::disable();
        }
    }

    Ok(handle)
}

// When using a cargo build, some essential services (e.g. RE, scribe)
// fall back to slow paths that give terrible performance.
// Therefore, if we are using cargo, warn strongly.
fn check_cargo() {
    if !cfg!(fbcode_build) && !slug_core::is_open_source() {
        eprintln!("=====================================================================");
        eprintln!("WARNING: You are using Buck v2 compiled with `cargo`, not `buck`.");
        eprintln!("         Some operations may go slower and logging may be impaired.");
        eprintln!("=====================================================================");
        eprintln!();
    }
}

fn print_retry() -> slug_error::Result<()> {
    slug_client_ctx::eprintln!("============================================================")?;
    slug_client_ctx::eprintln!("|| Slug has detected that it needs to restart to proceed ||")?;
    slug_client_ctx::eprintln!("|| Your command will now restart.                         ||")?;
    slug_client_ctx::eprintln!("============================================================")?;
    slug_client_ctx::eprintln!()?;
    Ok(())
}

fn exec_with_logging(
    trace_id: TraceId,
    start_time: SystemTime,
    restarted_trace_id: Option<TraceId>,
    shared: slug_error::Result<SharedProcessContext>,
    runtime: &mut ClientRuntime,
) -> (Option<SharedProcessContext>, ExitResult) {
    let args = std::env::args().collect::<Vec<String>>();
    let recorder = InvocationRecorder::new(trace_id.dupe(), restarted_trace_id, start_time, args);
    let mut events_ctx = EventsCtx::new(Some(recorder), vec![]);
    let (shared, res) = match shared {
        Ok(mut shared) => {
            let res = exec(ProcessContext {
                trace_id: trace_id.dupe(),
                events_ctx: &mut events_ctx,
                shared: &mut shared,
                runtime,
                start_time,
            });
            (Some(shared), res)
        }
        Err(e) => (None, e.into()),
    };
    let res = match runtime.get_or_init() {
        Ok(runtime) => events_ctx.finalize_events(trace_id, res, &runtime),
        Err(e) => e.into(),
    };
    (shared, res)
}

// As this main() is used as the entry point for the `buck daemon` command,
// it must be single-threaded. Commands that want to be multi-threaded/async
// will start up their own tokio runtime.
fn main() -> ! {
    slug_core::client_only::CLIENT_ONLY_VAL.init(cfg!(client_only));
    #[cfg(not(client_only))]
    {
        slug_analysis::init_late_bindings();
        slug_anon_target::init_late_bindings();
        slug_action_impl::init_late_bindings();
        slug_cmd_audit_server::init_late_bindings();
        slug_build_api::init_late_bindings();
        slug_cmd_docs_server::init_late_bindings();
        slug_external_cells::init_late_bindings();
        slug_transition::init_late_bindings();
        slug_build_signals_impl::init_late_bindings();
        slug_bxl::init_late_bindings();
        slug_cfg_constructor::init_late_bindings();
        slug_configured::init_late_bindings();
        slug_query_impls::init_late_bindings();
        slug_interpreter_for_build::init_late_bindings();
        slug_server_commands::init_late_bindings();
        slug_cmd_targets_server::init_late_bindings();
        slug_cmd_query_server::init_late_bindings();
        slug_cmd_starlark_server::init_late_bindings();
        slug_test::init_late_bindings();
        slug_validation::init_late_bindings();
        slug_events::init_late_bindings();
    }
    BUCK2_BUILD_INFO.init(SlugBuildInfo {
        revision: std::option_env!("BUCK2_SET_EXPLICIT_VERSION"),
        win_internal_version: std::option_env!("BUCK2_WIN_INTERNAL_VERSION"),
        release_timestamp: std::option_env!("BUCK2_RELEASE_TIMESTAMP"),
    });

    // Set up crypto impl once per process
    slug_certs::certs::setup_cryptography_or_fail();

    fn init_shared_context() -> slug_error::Result<SharedProcessContext> {
        panic::initialize()?;
        check_cargo();

        // Log the start timestamp
        tracing::debug!("Client initialized logging");

        Ok(SharedProcessContext {
            log_reload_handle: init_logging()?,
            stdin: Stdin::new()?,
            working_dir: AbsWorkingDir::current_dir()?,
            args: std::env::args().collect::<Vec<String>>(),
            restarter: Restarter::new(),
            force_want_restart: slug_env!("FORCE_WANT_RESTART", bool)?,
        })
    }

    fn main_with_result() -> ExitResult {
        let start_time = SystemTime::now();
        let first_trace_id = TraceId::from_env_or_new()?;
        let mut runtime = ClientRuntime::new();
        let shared = init_shared_context();
        let (shared, res) = exec_with_logging(
            first_trace_id.dupe(),
            start_time,
            None,
            shared,
            &mut runtime,
        );

        if let Some(shared) = shared {
            maybe_restart(first_trace_id, res, shared, &mut runtime)
        } else {
            res
        }
    }

    fn maybe_restart(
        first_trace_id: TraceId,
        initial_result: ExitResult,
        shared: SharedProcessContext,
        runtime: &mut ClientRuntime,
    ) -> ExitResult {
        let force_want_restart = shared.force_want_restart;
        let restart = |res| {
            let restart_start_time = SystemTime::now();

            if !force_want_restart && !shared.restarter.should_restart() {
                tracing::debug!("No restart was requested");
                return res;
            }

            if stdio::has_written_to_stdout() {
                tracing::debug!("Cannot restart: wrote to stdout");
                return res;
            }

            if print_retry().is_err() {
                tracing::debug!("Cannot restart: warning message cannot be printed");
                return res;
            }

            let (_, res) = exec_with_logging(
                TraceId::new(),
                restart_start_time,
                Some(first_trace_id),
                Ok(shared),
                runtime,
            );
            res
        };

        if force_want_restart {
            restart(initial_result)
        } else {
            initial_result.or_else(restart)
        }
    }

    main_with_result().report()
}
