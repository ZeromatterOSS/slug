/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! `kuro version` - Bazel-compatible version command.
//!
//! Prints build version information, mirroring Bazel's `bazel version` command.
//! Scripts that check `bazel version` will work with Kuro.

use kuro_common::argv::Argv;
use kuro_common::argv::SanitizedArgv;

#[derive(Debug, clap::Parser)]
#[clap(
    about = "Print version information (Bazel `bazel version` compatibility)",
    long_about = "Print version information about this Kuro build.\n\
Mimics Bazel's `bazel version` command for script compatibility."
)]
pub struct VersionCommand;

impl VersionCommand {
    pub fn exec(self) -> kuro_error::Result<()> {
        let version = env!("CARGO_PKG_VERSION");
        kuro_client_ctx::println!("Build label: {}", version)?;
        kuro_client_ctx::println!("Build target: kuro")?;
        kuro_client_ctx::println!("Build time: unknown")?;
        kuro_client_ctx::println!("Build timestamp: 0")?;
        kuro_client_ctx::println!("Build timestamp as int: 0")?;
        Ok(())
    }

    pub fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }
}
