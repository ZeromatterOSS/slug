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
use crate::common::bzlmod_registry_urls;
use crate::common::parse_target_patterns;
use crate::common::split_args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRequest {
    pub targets: Vec<TargetPattern>,
    pub flags: Vec<ParsedFlag>,
    pub bzlmod_policy: BzlmodCommandPolicyKey,
    pub lockfile_mode: LockfileMode,
    pub registry_urls: Vec<String>,
}

impl BuildRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
        let parsed = split_args(args);
        let bzlmod_policy = bzlmod_command_policy(&parsed.flags)?;
        let lockfile_mode = bzlmod_lockfile_mode(&parsed.flags)?;
        let registry_urls = bzlmod_registry_urls(&parsed.flags)?;
        Ok(Self {
            targets: parse_target_patterns(CommandKind::Build, &parsed.positionals)?,
            flags: parsed.flags,
            bzlmod_policy,
            lockfile_mode,
            registry_urls,
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
