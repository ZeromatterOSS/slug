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
use slug_client_ctx::common::CommonCommandOptions;
use slug_client_ctx::common::target_cfg::TargetCfgUnusedOptions;

use crate::AuditSubcommand;

#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize)]
#[clap(
    name = "audit-cell",
    about = "Query information about the [cells] list in .buckconfig."
)]
pub struct AuditCellCommand {
    #[clap(long = "json", help = "Output in JSON format")]
    pub json: bool,

    #[clap(
        long = "paths-only",
        help = "Don't include the cell name in the output"
    )]
    pub paths_only: bool,

    #[clap(
        long = "aliases",
        help = "If enabled and no explicit aliases are passed, will query for all aliases in the working directory cell."
    )]
    pub aliases: bool,

    #[clap(
        name = "CELL_ALIASES",
        help = "Cell aliases to query. These aliases will be resolved in the working directory cell.",
        allow_hyphen_values = true
    )]
    pub aliases_to_resolve: Vec<String>,

    /// Command doesn't need these flags, but they are used in mode files, so we need to keep them.
    #[clap(flatten)]
    _target_cfg: TargetCfgUnusedOptions,

    #[clap(flatten)]
    common_opts: CommonCommandOptions,
}

impl AuditCellCommand {
    pub fn normalize_bazel_flag_positionals(&mut self) {
        let mut aliases = Vec::with_capacity(self.aliases_to_resolve.len());
        for arg in std::mem::take(&mut self.aliases_to_resolve) {
            if let Some(mode) = arg
                .strip_prefix("--lockfile_mode=")
                .or_else(|| arg.strip_prefix("--lockfile-mode="))
            {
                if self.common_opts.config_opts.lockfile_mode.is_none() {
                    self.common_opts.config_opts.lockfile_mode = Some(mode.to_owned());
                }
            } else {
                aliases.push(arg);
            }
        }
        self.aliases_to_resolve = aliases;
    }
}

#[async_trait]
impl AuditSubcommand for AuditCellCommand {
    fn common_opts(&self) -> &CommonCommandOptions {
        &self.common_opts
    }
}
