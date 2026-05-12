/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_client_ctx::client_ctx::BuckSubcommand;
use slug_client_ctx::client_ctx::ClientCommandContext;
use slug_client_ctx::common::BuckArgMatches;
use slug_client_ctx::common::CommonEventLogOptions;
use slug_client_ctx::events_ctx::EventsCtx;
use slug_client_ctx::exit_result::ExitResult;
use slug_wrapper_common::is_slug::WhoIsAsking;

#[derive(Debug, clap::Parser)]
#[clap(about = "Kill all slug processes on the machine")]
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
        slug_wrapper_common::killall(WhoIsAsking::Slug, |s| {
            let _ignored = slug_client_ctx::eprintln!("{}", s);
        })
        .then_some(())
        .ok_or(slug_error::slug_error!(
            slug_error::ErrorTag::KillAll,
            "Killall command failed"
        ))
        .into()
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        &self.event_log_opts
    }
}
