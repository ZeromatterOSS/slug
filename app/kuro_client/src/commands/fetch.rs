/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! `kuro fetch` - Bazel-compatible fetch command.
//!
//! In Bazel, `fetch` downloads external dependencies. In Kuro, external
//! dependencies are resolved lazily during build, so this command performs
//! a configuration-only analysis pass to trigger any needed downloads.

use kuro_client_ctx::client_ctx::ClientCommandContext;
use kuro_client_ctx::common::BuckArgMatches;
use kuro_common::argv::Argv;
use kuro_common::argv::SanitizedArgv;

#[derive(Debug, clap::Parser)]
#[clap(
    about = "Fetch external repositories (Bazel `bazel fetch` compatibility)",
    long_about = "Fetch external repositories needed by the targets.\n\
In Kuro, external dependencies are resolved lazily, so this command\n\
is a no-op that exists for Bazel script compatibility.\n\
Use `kuro build` to trigger actual dependency resolution."
)]
pub struct FetchCommand {
    /// Target patterns to fetch dependencies for.
    #[clap(value_name = "TARGET")]
    targets: Vec<String>,

    /// Fetch all external repositories (equivalent to `bazel fetch //...`).
    #[clap(long)]
    all: bool,
}

impl FetchCommand {
    pub fn exec(
        self,
        _matches: BuckArgMatches<'_>,
        _ctx: ClientCommandContext<'_>,
    ) -> kuro_error::Result<()> {
        // Kuro resolves external deps lazily during build.
        // This command exists for Bazel script compatibility.
        kuro_client_ctx::eprintln!(
            "INFO: External dependencies are resolved lazily in Kuro. No fetch needed."
        )?;
        Ok(())
    }

    pub fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }
}
