/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_client_ctx::client_ctx::ClientCommandContext;
use slug_client_ctx::common::BuckArgMatches;
use slug_client_ctx::events_ctx::EventsCtx;
use slug_client_ctx::exit_result::ExitResult;

mod action_divergence;
mod diff_options;
mod external_config_diff;
mod summary_diff;

#[derive(Debug, clap::Subcommand)]
#[clap(about = "Subcommands for diff'ing two slug commands")]
pub enum DiffCommand {
    ActionDivergence(action_divergence::ActionDivergenceCommand),
    ExternalConfigs(external_config_diff::ExternalConfigDiffCommand),
    Summary(summary_diff::SummaryDiffCommand),
}

impl DiffCommand {
    pub fn exec(
        self,
        matches: BuckArgMatches<'_>,
        ctx: ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        match self {
            Self::ExternalConfigs(cmd) => ctx.exec(cmd, matches, events_ctx),
            Self::ActionDivergence(cmd) => ctx.exec(cmd, matches, events_ctx),
            Self::Summary(cmd) => ctx.exec(cmd, matches, events_ctx),
        }
    }
}
