/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_client_ctx::client_ctx::BuckSubcommand;
use kuro_client_ctx::client_ctx::ClientCommandContext;
use kuro_client_ctx::common::BuckArgMatches;
use kuro_client_ctx::common::CommonEventLogOptions;
use kuro_client_ctx::events_ctx::EventsCtx;
use kuro_client_ctx::exit_result::ExitResult;
use kuro_wrapper_common::is_kuro::WhoIsAsking;

#[derive(Debug, clap::Parser)]
#[clap(about = "Kill all kuro processes on the machine")]
pub struct KillallCommand {
    #[clap(flatten)]
    pub(crate) event_log_opts: CommonEventLogOptions,
}

impl BuckSubcommand for KillallCommand {
    const COMMAND_NAME: &'static str = "killall";

    async fn exec_impl(
        self,
        _matches: BuckArgMatches<'_>,
        _ctx: ClientCommandContext<'_>,
        _events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        kuro_wrapper_common::killall(WhoIsAsking::Kuro, |s| {
            let _ignored = kuro_client_ctx::eprintln!("{}", s);
        })
        .then_some(())
        .ok_or(kuro_error::kuro_error!(
            kuro_error::ErrorTag::KillAll,
            "Killall command failed"
        ))
        .into()
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        &self.event_log_opts
    }
}
