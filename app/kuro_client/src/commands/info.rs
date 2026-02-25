/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! `kuro info` - Bazel-compatible info command.
//!
//! Prints build workspace information, mirroring Bazel's `bazel info` command.
//! Useful for scripts that need to locate output directories.

use kuro_client_ctx::client_ctx::ClientCommandContext;
use kuro_client_ctx::common::BuckArgMatches;
use kuro_common::argv::Argv;
use kuro_common::argv::SanitizedArgv;

/// Known info keys supported by `kuro info`.
const INFO_KEYS: &[(&str, &str)] = &[
    ("workspace", "Absolute path to the workspace root"),
    ("output_base", "Absolute path to the base output directory"),
    ("execution_root", "Absolute path to the execution root (buck-out/v2)"),
    ("bazel-bin", "Absolute path to the bazel-bin convenience directory"),
    ("bazel-testlogs", "Absolute path to the test logs directory"),
    ("release", "Version info for this Kuro release"),
];

#[derive(Debug, clap::Parser)]
#[clap(
    about = "Print workspace information (Bazel `bazel info` compatibility)",
    long_about = "Print one or more build workspace information values.\n\
Mimics Bazel's `bazel info` command for script compatibility.\n\
\n\
Usage:\n  kuro info            # print all keys\n  kuro info workspace  # print just workspace path\n\
\n\
Supported keys: workspace, output_base, execution_root, bazel-bin, bazel-testlogs, release"
)]
pub struct InfoCommand {
    /// Info key(s) to query. If none, all keys are printed.
    #[clap(value_name = "KEY")]
    keys: Vec<String>,
}

impl InfoCommand {
    pub fn exec(
        self,
        _matches: BuckArgMatches<'_>,
        ctx: ClientCommandContext<'_>,
    ) -> kuro_error::Result<()> {
        let project_root = ctx.paths()?.project_root().root().as_path().to_path_buf();
        let daemon_dir = ctx.paths()?.daemon_dir()?.path;

        let print_key = |key: &str| -> kuro_error::Result<()> {
            let value = match key {
                "workspace" => project_root.to_string_lossy().into_owned(),
                "output_base" => daemon_dir.to_string_lossy().into_owned(),
                "execution_root" => project_root
                    .join("buck-out")
                    .join("v2")
                    .to_string_lossy()
                    .into_owned(),
                "bazel-bin" => project_root.join("bazel-bin").to_string_lossy().into_owned(),
                "bazel-testlogs" => project_root
                    .join("bazel-testlogs")
                    .to_string_lossy()
                    .into_owned(),
                "release" => {
                    let ver = env!("CARGO_PKG_VERSION");
                    format!("release {ver}")
                }
                other => {
                    return Err(kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "Unknown info key: '{}'. Known keys: {}",
                        other,
                        INFO_KEYS
                            .iter()
                            .map(|(k, _)| *k)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            };
            kuro_client_ctx::println!("{}: {}", key, value)?;
            Ok(())
        };

        if self.keys.is_empty() {
            for (key, _) in INFO_KEYS {
                print_key(key)?;
            }
        } else {
            for key in &self.keys {
                print_key(key)?;
            }
        }
        Ok(())
    }

    pub fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }
}
