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
pub mod run;
pub mod test;
pub mod version;

use slug_commands_v2::CommandKind;
use slug_commands_v2::CommandParseError;
use slug_commands_v2::CommandPlaceholderError;
use slug_core_v2::error::json_escape;

pub fn dispatch<I, S>(args: I) -> i32
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
        return serve_daemon(&rest[1..]);
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
        "build" => build::run(rest),
        "query" => query::run(rest),
        "cquery" => cquery::run(rest),
        "aquery" => aquery::run(rest),
        "test" => test::run(rest),
        "run" => run::run(rest),
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
fn serve_daemon(args: &[String]) -> i32 {
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
    match slug_server_v2::serve(&socket, &workspace) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!(
                "{{\"error\":\"daemon_serve_error\",\"message\":\"{}\"}}",
                json_escape(&error.to_string())
            );
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_command_is_structured() {
        let code = dispatch(["slug", "unknown"]);
        assert_eq!(code, 2);
    }
}
