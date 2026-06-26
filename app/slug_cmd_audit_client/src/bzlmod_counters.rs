/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 */

use async_trait::async_trait;
use slug_client_ctx::common::CommonCommandOptions;
use slug_client_ctx::common::target_cfg::TargetCfgUnusedOptions;

use crate::AuditSubcommand;

#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize)]
#[clap(
    name = "audit-bzlmod-counters",
    about = "Print Plan 61 bzlmod observability counters as JSON."
)]
pub struct AuditBzlmodCountersCommand {
    /// Only include events whose detail starts with this prefix.
    #[clap(long = "detail-prefix")]
    pub detail_prefix: Option<String>,

    /// Command doesn't need these flags, but they are used in mode files, so we need to keep them.
    #[clap(flatten)]
    _target_cfg: TargetCfgUnusedOptions,

    #[clap(flatten)]
    common_opts: CommonCommandOptions,
}

#[async_trait]
impl AuditSubcommand for AuditBzlmodCountersCommand {
    fn common_opts(&self) -> &CommonCommandOptions {
        &self.common_opts
    }
}
