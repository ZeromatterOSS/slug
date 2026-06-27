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

use slug_core_v2::PlannedCommand;
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

fn planned(command: PlannedCommand, argv: Vec<String>) -> i32 {
    let error = slug_core_v2::PlaceholderCommandError::not_implemented(command, argv);
    eprintln!("{error}");
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
