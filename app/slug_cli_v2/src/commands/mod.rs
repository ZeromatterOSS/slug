/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod aquery;
pub mod build;
pub mod cquery;
pub mod help;
pub mod query;
mod repository_environment;
pub mod run;
pub mod test;
pub mod version;

use std::path::Path;
use std::path::PathBuf;

use slug_commands_v2::CommandKind;
use slug_commands_v2::CommandParseError;
use slug_commands_v2::CommandPlaceholderError;
use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::ProcessHostOwner;
use slug_core_v2::runtime::WorkspaceRuntime;

#[derive(Clone, Copy)]
pub(crate) enum RuntimeStartup {
    Generic,
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    Installed,
}

impl RuntimeStartup {
    pub(crate) fn for_installed_binary() -> Self {
        #[cfg(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ))]
        {
            Self::Installed
        }
        #[cfg(not(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        )))]
        {
            Self::Generic
        }
    }

    fn new_runtime(self, workspace: &Path) -> Result<WorkspaceRuntime, String> {
        match self {
            Self::Generic => WorkspaceRuntime::new(workspace, ProcessHostOwner::native())
                .map_err(|error| error.to_string()),
            #[cfg(all(
                target_os = "linux",
                target_arch = "x86_64",
                target_pointer_width = "64"
            ))]
            Self::Installed => WorkspaceRuntime::new_with_installed_repository_launcher(workspace)
                .map_err(|error| format!("{error:#}")),
        }
    }

    fn daemon_executable(self) -> std::io::Result<PathBuf> {
        match self {
            Self::Generic => std::env::current_exe(),
            #[cfg(all(
                target_os = "linux",
                target_arch = "x86_64",
                target_pointer_width = "64"
            ))]
            Self::Installed => Ok(PathBuf::from("/proc/self/exe")),
        }
    }

    fn daemon_socket(self, output_base: &Path) -> PathBuf {
        // Keep an existing daemon's authority tied to its startup policy.
        match self {
            Self::Generic => slug_server_v2::socket_path(output_base),
            #[cfg(all(
                target_os = "linux",
                target_arch = "x86_64",
                target_pointer_width = "64"
            ))]
            Self::Installed => output_base.join("slugd-installed.sock"),
        }
    }

    fn daemon_pid(self, output_base: &Path) -> PathBuf {
        match self {
            Self::Generic => slug_server_v2::pid_path(output_base),
            #[cfg(all(
                target_os = "linux",
                target_arch = "x86_64",
                target_pointer_width = "64"
            ))]
            Self::Installed => output_base.join("slugd-installed.pid"),
        }
    }

    fn serve(self, socket: &Path, workspace: &Path) -> Result<(), String> {
        match self {
            Self::Generic => {
                slug_server_v2::serve(socket, workspace).map_err(|error| error.to_string())
            }
            #[cfg(all(
                target_os = "linux",
                target_arch = "x86_64",
                target_pointer_width = "64"
            ))]
            Self::Installed => {
                let runtime = self.new_runtime(workspace)?;
                slug_server_v2::serve_with_runtime(socket, runtime)
                    .map_err(|error| error.to_string())
            }
        }
    }
}

#[cfg(all(
    test,
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
#[test]
fn daemon_endpoints_are_partitioned_by_startup_policy() {
    let base = Path::new("/output");
    assert_eq!(
        RuntimeStartup::Generic.daemon_socket(base),
        slug_server_v2::socket_path(base)
    );
    assert_eq!(
        RuntimeStartup::Generic.daemon_pid(base),
        slug_server_v2::pid_path(base)
    );
    assert_ne!(
        RuntimeStartup::Installed.daemon_socket(base),
        RuntimeStartup::Generic.daemon_socket(base)
    );
    assert_ne!(
        RuntimeStartup::Installed.daemon_pid(base),
        RuntimeStartup::Generic.daemon_pid(base)
    );
}

/// Forward accepted event/terminal bytes without text conversion. A failed
/// partial write is an output failure; callers must not launch a run payload.
pub(crate) fn write_stderr(bytes: &[u8]) -> std::io::Result<()> {
    write_stderr_to(&mut std::io::stderr().lock(), bytes)
}
fn write_stderr_to(output: &mut impl std::io::Write, bytes: &[u8]) -> std::io::Result<()> {
    output.write_all(bytes)
}

pub fn dispatch<I, S>(args: I) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    dispatch_with_startup(args, RuntimeStartup::Generic)
}

pub(crate) fn dispatch_with_startup<I, S>(args: I, startup: RuntimeStartup) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let _program = args.next();
    let all_args: Vec<String> = args.collect();
    // Collect startup flags (--output_base and its value) that appear before
    // the subcommand, then dispatch the remaining args.
    let mut startup_output_base: Option<String> = None;
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;
    while i < all_args.len() {
        let arg = &all_args[i];
        if let Some(value) = arg.strip_prefix("--output_base=") {
            startup_output_base = Some(value.to_string());
            i += 1;
            continue;
        }
        if arg == "--output_base" {
            if i + 1 < all_args.len() {
                startup_output_base = Some(all_args[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        rest.push(arg.clone());
        i += 1;
    }
    if rest.first().is_some_and(|f| f == "--serve") {
        return serve_daemon(&rest[1..], startup);
    }
    // Extract the command (first non-flag in rest).
    let command = rest.iter().position(|a| !a.starts_with('-'));
    let Some(command_idx) = command else {
        help::print_help();
        return 0;
    };
    let command = rest[command_idx].clone();
    rest.drain(..=command_idx);
    // Inject --output_base back into the build args if it was a startup flag.
    if let Some(ob) = &startup_output_base {
        rest.insert(0, format!("--output_base={ob}"));
    }
    match command.as_str() {
        "help" | "--help" | "-h" => {
            help::print_help();
            0
        }
        "version" | "--version" => {
            version::print_version();
            0
        }
        "build" => build::run_with_startup(rest, startup),
        "query" => query::run_with_startup(rest, startup),
        "cquery" => cquery::run_with_startup(rest, startup),
        "aquery" => aquery::run_with_startup(rest, startup),
        "test" => test::run(rest),
        "run" => run::run_with_startup(rest, startup),
        other => {
            eprintln!(
                "{{\"error\":\"unknown_command\",\"command\":\"{}\",\"known_commands\":[\"version\",\"help\",\"build\",\"query\",\"cquery\",\"aquery\",\"test\",\"run\"]}}",
                json_escape(other)
            );
            2
        }
    }
}

fn emit_result(
    command: CommandKind,
    argv: Vec<String>,
    result: Result<CommandPlaceholderError, CommandParseError>,
) -> i32 {
    match result {
        Ok(error) => planned(error, argv),
        Err(error) => parse_error(command, error),
    }
}

fn planned(error: CommandPlaceholderError, argv: Vec<String>) -> i32 {
    let argv = argv
        .iter()
        .map(|arg| slug_commands_v2::redact_repository_environment_arg(arg))
        .map(|arg| format!("\"{}\"", json_escape(arg)))
        .collect::<Vec<_>>()
        .join(",");
    eprintln!(
        "{{\"error\":\"planned_placeholder\",\"command\":\"{}\",\"argv\":[{}],\"owner_stage\":\"{}\",\"reason\":\"{}\",\"runtime_mode\":\"one-shot\"}}",
        error.command,
        argv,
        json_escape(error.owner_stage),
        json_escape(error.reason),
    );
    2
}

fn parse_error(command: CommandKind, error: CommandParseError) -> i32 {
    eprintln!(
        "{{\"error\":\"command_parse_error\",\"command\":\"{}\",\"message\":\"{}\",\"runtime_mode\":\"one-shot\"}}",
        command,
        json_escape(&error.to_string()),
    );
    2
}

/// Enter daemon server mode. Parses `--socket` and `--workspace` from the
/// args, then blocks serving build requests on the Unix socket.
fn serve_daemon(args: &[String], startup: RuntimeStartup) -> i32 {
    let mut socket = None;
    let mut workspace = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix("--socket=") {
            socket = Some(value.to_string());
        } else if arg == "--socket" {
            socket = iter.next().cloned();
        } else if let Some(value) = arg.strip_prefix("--workspace=") {
            workspace = Some(value.to_string());
        } else if arg == "--workspace" {
            workspace = iter.next().cloned();
        }
    }
    let (Some(socket), Some(workspace)) = (socket, workspace) else {
        eprintln!(
            "{{\"error\":\"daemon_args\",\"message\":\"--socket and --workspace are required for --serve mode\"}}"
        );
        return 2;
    };
    match startup.serve(Path::new(&socket), Path::new(&workspace)) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!(
                "{{\"error\":\"daemon_serve_error\",\"message\":\"{}\"}}",
                json_escape(&error)
            );
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_stderr_retries_partial_writes_and_propagates_delivery_failure() {
        struct Partial {
            bytes: Vec<u8>,
            fail_after: Option<usize>,
        }
        impl std::io::Write for Partial {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self.fail_after == Some(self.bytes.len()) {
                    return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
                }
                let count = bytes.len().min(1);
                self.bytes.extend_from_slice(&bytes[..count]);
                Ok(count)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut complete = Partial {
            bytes: Vec::new(),
            fail_after: None,
        };
        write_stderr_to(&mut complete, &[0xff, 0, 0x80]).unwrap();
        assert_eq!(complete.bytes, [0xff, 0, 0x80]);
        let mut failed = Partial {
            bytes: Vec::new(),
            fail_after: Some(1),
        };
        assert_eq!(
            write_stderr_to(&mut failed, &[0xff, 0]).unwrap_err().kind(),
            std::io::ErrorKind::BrokenPipe
        );
        assert_eq!(failed.bytes, [0xff]);
    }

    #[test]
    fn unknown_command_is_structured() {
        let code = dispatch(["slug", "unknown"]);
        assert_eq!(code, 2);
    }
}
