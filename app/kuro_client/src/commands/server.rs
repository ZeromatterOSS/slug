/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use async_trait::async_trait;
use kuro_client_ctx::client_ctx::ClientCommandContext;
use kuro_client_ctx::common::BuckArgMatches;
use kuro_client_ctx::common::CommonBuildConfigurationOptions;
use kuro_client_ctx::common::CommonEventLogOptions;
use kuro_client_ctx::common::CommonStarlarkOptions;
use kuro_client_ctx::common::ui::CommonConsoleOptions;
use kuro_client_ctx::daemon::client::BuckdClientConnector;
use kuro_client_ctx::events_ctx::EventsCtx;
use kuro_client_ctx::exit_result::ExitResult;
use kuro_client_ctx::streaming::StreamingCommand;

#[derive(Debug, clap::Parser)]
#[clap(
    about = "Start, query, and control the http server",
    long_about = "Start, query, and control the kuro server, a long-lived process, spanning kuro command line invocations.
Using this command can ensure the daemon is running.

To stop a specific server, use `kuro kill` and add `--isolation-dir` for a specific instance.
To stop all instances, use `kuro killall`."
)]
pub struct ServerCommand {}

#[async_trait(?Send)]
impl StreamingCommand for ServerCommand {
    const COMMAND_NAME: &'static str = "server";

    async fn exec_impl(
        self,
        buckd: &mut BuckdClientConnector,
        _matches: BuckArgMatches<'_>,
        _ctx: &mut ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let status = buckd
            .with_flushing()
            .status(events_ctx, false, false)
            .await?;
        kuro_client_ctx::println!("buckd.endpoint={}", status.process_info.unwrap().endpoint)?;
        ExitResult::success()
    }

    fn console_opts(&self) -> &CommonConsoleOptions {
        CommonConsoleOptions::simple_ref()
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        CommonEventLogOptions::default_ref()
    }

    fn build_config_opts(&self) -> &CommonBuildConfigurationOptions {
        CommonBuildConfigurationOptions::default_ref()
    }

    fn starlark_opts(&self) -> &CommonStarlarkOptions {
        CommonStarlarkOptions::default_ref()
    }
}
