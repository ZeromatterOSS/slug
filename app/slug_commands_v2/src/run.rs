/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_identity_v2::TargetPattern;

use crate::common::CommandKind;
use crate::common::CommandParseError;
use crate::common::CommandPlaceholderError;
use crate::common::ParsedFlag;
use crate::common::bzlmod_command_policy;
use crate::common::bzlmod_lockfile_mode;
use crate::common::parse_single_target;
use crate::common::split_args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    pub target: TargetPattern,
    pub program_args: Vec<String>,
    pub flags: Vec<ParsedFlag>,
    pub bzlmod_policy: BzlmodCommandPolicyKey,
    pub lockfile_mode: LockfileMode,
}

impl RunRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
        let parsed = split_args(args);
        let target = parse_single_target(CommandKind::Run, parsed.positionals.first())?;
        let mut program_args = parsed.positionals.into_iter().skip(1).collect::<Vec<_>>();
        program_args.extend(parsed.passthrough);
        let bzlmod_policy = bzlmod_command_policy(&parsed.flags)?;
        let lockfile_mode = bzlmod_lockfile_mode(&parsed.flags)?;
        Ok(Self {
            target,
            program_args,
            flags: parsed.flags,
            bzlmod_policy,
            lockfile_mode,
        })
    }

    pub const fn placeholder_error(&self) -> CommandPlaceholderError {
        CommandPlaceholderError::planned(
            CommandKind::Run,
            "Stage 7/8",
            "runfiles materialization and executable handoff are not wired to the command runner yet",
        )
    }
}
