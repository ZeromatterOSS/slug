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
    let Some(command) = args.next() else {
        help::print_help();
        return 0;
    };
    let rest = args.collect::<Vec<_>>();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_command_is_structured() {
        let code = dispatch(["slug", "unknown"]);
        assert_eq!(code, 2);
    }
}
