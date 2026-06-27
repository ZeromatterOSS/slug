/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_identity_v2::TargetPattern;

use crate::common::CommandKind;
use crate::common::CommandParseError;
use crate::common::CommandPlaceholderError;
use crate::common::ParsedFlag;
use crate::common::parse_target_patterns;
use crate::common::split_args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRequest {
    pub targets: Vec<TargetPattern>,
    pub flags: Vec<ParsedFlag>,
}

impl TestRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
        let parsed = split_args(args);
        Ok(Self {
            targets: parse_target_patterns(CommandKind::Test, &parsed.positionals)?,
            flags: parsed.flags,
        })
    }

    pub const fn placeholder_error(&self) -> CommandPlaceholderError {
        CommandPlaceholderError::planned(
            CommandKind::Test,
            "Stage 7/8",
            "test execution, logs, and BEP result semantics are not wired to the command runner yet",
        )
    }
}
