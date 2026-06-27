/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedCommand {
    Build,
    Query,
    Test,
    Run,
}

impl PlannedCommand {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Query => "query",
            Self::Test => "test",
            Self::Run => "run",
        }
    }
}

impl fmt::Display for PlannedCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceholderCommandError {
    pub command: PlannedCommand,
    pub argv: Vec<String>,
    pub reason: &'static str,
}

impl PlaceholderCommandError {
    pub fn not_implemented(command: PlannedCommand, argv: Vec<String>) -> Self {
        Self {
            command,
            argv,
            reason: "not_yet_implemented",
        }
    }

    pub fn to_json_line(&self) -> String {
        let argv = self
            .argv
            .iter()
            .map(|arg| format!("\"{}\"", json_escape(arg)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"error\":\"{}\",\"command\":\"{}\",\"argv\":[{}],\"runtime_mode\":\"one-shot\"}}",
            self.reason, self.command, argv,
        )
    }
}

impl fmt::Display for PlaceholderCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_json_line())
    }
}

pub fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_error_preserves_argv() {
        let error = PlaceholderCommandError::not_implemented(
            PlannedCommand::Build,
            vec!["//:x".to_owned(), "--unknown_flag".to_owned()],
        );
        let rendered = error.to_json_line();
        assert!(rendered.contains("\"command\":\"build\""));
        assert!(rendered.contains("--unknown_flag"));
    }
}
