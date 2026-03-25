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
    (
        "execution_root",
        "Absolute path to the execution root (buck-out/v2)",
    ),
    ("output_path", "Absolute path to the output directory"),
    (
        "bazel-bin",
        "Absolute path to the bazel-bin convenience directory",
    ),
    (
        "bazel-genfiles",
        "Absolute path to the bazel-genfiles directory (same as bazel-bin)",
    ),
    ("bazel-testlogs", "Absolute path to the test logs directory"),
    ("server_pid", "PID of the running kuro daemon"),
    ("server_log", "Path to the daemon log file"),
    ("release", "Version info for this Kuro release"),
    ("build-language", "Starlark build language info"),
    (
        "starlark-semantics",
        "Starlark dialect semantics (Bazel 9.0 compatible)",
    ),
    ("command_log", "Path to the most recent command log"),
    (
        "character-encoding",
        "Character encoding used for source files",
    ),
    (
        "used-heap-size-after-gc",
        "Approximate heap memory used by the server",
    ),
    ("package_path", "Colon-separated package search path"),
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

        let resolve_key = |key: &str| -> kuro_error::Result<String> {
            match key {
                "workspace" => Ok(project_root.to_string_lossy().into_owned()),
                "output_base" => Ok(daemon_dir.to_string_lossy().into_owned()),
                "execution_root" => Ok(project_root
                    .join("buck-out")
                    .join("v2")
                    .to_string_lossy()
                    .into_owned()),
                "output_path" => Ok(project_root.join("buck-out").to_string_lossy().into_owned()),
                "bazel-bin" => Ok(project_root
                    .join("bazel-bin")
                    .to_string_lossy()
                    .into_owned()),
                "bazel-genfiles" => Ok(project_root
                    .join("bazel-bin")
                    .to_string_lossy()
                    .into_owned()),
                "bazel-testlogs" => Ok(project_root
                    .join("bazel-testlogs")
                    .to_string_lossy()
                    .into_owned()),
                "server_pid" => {
                    // Try to read the daemon PID from the buckd info file
                    let pid_file = daemon_dir.as_path().join("buckd.pid");
                    match std::fs::read_to_string(&pid_file) {
                        Ok(pid) => Ok(pid.trim().to_owned()),
                        Err(_) => Ok("(not running)".to_owned()),
                    }
                }
                "server_log" => Ok(daemon_dir
                    .as_path()
                    .join("buckd.log")
                    .to_string_lossy()
                    .into_owned()),
                "release" => {
                    let ver = env!("CARGO_PKG_VERSION");
                    Ok(format!("release {ver}"))
                }
                "build-language" => Ok("Starlark".to_owned()),
                "starlark-semantics" => Ok("Bazel 9.0 compatible Starlark".to_owned()),
                "command_log" => Ok(daemon_dir
                    .as_path()
                    .join("buckd.log")
                    .to_string_lossy()
                    .into_owned()),
                "character-encoding" => Ok("UTF-8".to_owned()),
                "used-heap-size-after-gc" => Ok("0".to_owned()),
                "package_path" => Ok("%workspace%".to_owned()),
                other => Err(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Unknown info key: '{}'. Known keys: {}",
                    other,
                    INFO_KEYS
                        .iter()
                        .map(|(k, _)| *k)
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        };

        if self.keys.is_empty() {
            // Print all keys with "key: value" format
            for (key, _) in INFO_KEYS {
                let value = resolve_key(key)?;
                kuro_client_ctx::println!("{}: {}", key, value)?;
            }
        } else if self.keys.len() == 1 {
            // Single key: print just the value (Bazel compatibility)
            let value = resolve_key(&self.keys[0])?;
            kuro_client_ctx::println!("{}", value)?;
        } else {
            // Multiple keys: print "key: value" format
            for key in &self.keys {
                let value = resolve_key(key)?;
                kuro_client_ctx::println!("{}: {}", key, value)?;
            }
        }
        Ok(())
    }

    pub fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }
}
