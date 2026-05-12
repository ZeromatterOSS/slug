/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use clap::Parser;
use slug_client_ctx::client_ctx::ClientCommandContext;
use slug_client_ctx::common::BuckArgMatches;
use slug_client_ctx::events_ctx::EventsCtx;
use slug_client_ctx::exit_result::ExitResult;
use slug_error::ErrorTag;
use tokio::runtime::Runtime;

#[derive(Debug, Parser)]
#[clap(about = "run the internal test runner")]
pub(crate) struct InternalTestRunnerCommand {
    #[cfg(unix)]
    #[clap(flatten)]
    unix_runner: slug_test_runner::unix::SlugTestRunnerUnix,

    #[cfg(not(unix))]
    #[clap(flatten)]
    tcp_runner: slug_test_runner::tcp::SlugTestRunnerTcp,
}

impl InternalTestRunnerCommand {
    pub(crate) fn exec(
        self,
        _matches: BuckArgMatches<'_>,
        _ctx: ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        events_ctx.log_invocation_record = false;

        // Internal test runner should only be used in the open source version of Slug.
        if slug_core::is_open_source()
            || std::env::var("BUCK2_ALLOW_INTERNAL_TEST_RUNNER_DO_NOT_USE").is_ok()
        {
            let runtime = Runtime::new().expect("Failed to create Tokio runtime");
            runtime
                .block_on(async move {
                    #[cfg(unix)]
                    {
                        self.unix_runner.run().await
                    }
                    #[cfg(not(unix))]
                    {
                        self.tcp_runner.run().await
                    }
                })
                .into()
        } else {
            slug_error::slug_error!(
                ErrorTag::Input,
                "Cannot use internal test runner. Config value must be provided for test.v2_test_executor."
            ).into()
        }
    }
}
