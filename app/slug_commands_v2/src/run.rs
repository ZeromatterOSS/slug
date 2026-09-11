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
use crate::common::RepositoryEnvironmentOverride;
use crate::common::bzlmod_command_policy;
use crate::common::bzlmod_lockfile_mode;
use crate::common::bzlmod_registry_urls;
use crate::common::parse_single_target;
use crate::common::repository_environment_overrides;
use crate::common::split_args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    pub target: TargetPattern,
    pub program_args: Vec<String>,
    pub flags: Vec<ParsedFlag>,
    pub bzlmod_policy: BzlmodCommandPolicyKey,
    pub lockfile_mode: LockfileMode,
    pub registry_urls: Vec<String>,
    pub repository_environment_overrides: Vec<RepositoryEnvironmentOverride>,
}

impl RunRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
        let parsed = split_args(args);
        let target = parse_single_target(CommandKind::Run, parsed.positionals.first())?;
        let mut program_args = parsed.positionals.into_iter().skip(1).collect::<Vec<_>>();
        program_args.extend(parsed.passthrough);
        let bzlmod_policy = bzlmod_command_policy(&parsed.flags)?;
        let lockfile_mode = bzlmod_lockfile_mode(&parsed.flags)?;
        let registry_urls = bzlmod_registry_urls(&parsed.flags)?;
        let repository_environment_overrides = repository_environment_overrides(&parsed.flags)?;
        Ok(Self {
            target,
            program_args,
            flags: parsed.flags,
            bzlmod_policy,
            lockfile_mode,
            registry_urls,
            repository_environment_overrides,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_urls_retain_order_and_stop_at_program_arguments() {
        let request = RunRequest::parse(&[
            "--registry=https://a.example/",
            "--registry=https://b.example/",
            "//pkg:bin",
            "--",
            "--registry=file://program-argument",
            "tail",
        ])
        .unwrap();
        assert_eq!(
            request.registry_urls,
            ["https://a.example/", "https://b.example/"]
        );
        assert_eq!(
            request.program_args,
            ["--registry=file://program-argument", "tail"]
        );
    }

    #[test]
    fn registry_url_requires_a_nonempty_equality_value() {
        for flag in ["--registry", "--registry="] {
            assert_eq!(
                RunRequest::parse(&[flag, "//pkg:bin"]),
                Err(CommandParseError::InvalidFlagValue {
                    flag: flag.to_owned(),
                    message: "expected a non-empty registry URL".to_owned(),
                })
            );
        }
    }
}
