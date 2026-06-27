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
pub struct BuildRequest {
    pub targets: Vec<TargetPattern>,
    pub flags: Vec<ParsedFlag>,
}

impl BuildRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
        let parsed = split_args(args);
        Ok(Self {
            targets: parse_target_patterns(CommandKind::Build, &parsed.positionals)?,
            flags: parsed.flags,
        })
    }

    pub const fn placeholder_error(&self) -> CommandPlaceholderError {
        CommandPlaceholderError::planned(
            CommandKind::Build,
            "Stage 6/7",
            "configured-target analysis and REAPI execution are not wired to the command runner yet",
        )
    }
}
