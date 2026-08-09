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
    /// The one admitted root build setting transition. This remains a raw
    /// Unicode value at the command boundary; configuration owns its typed
    /// representation and semantic identity.
    pub root_string_setting: Option<String>,
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
        let root_string_setting = build_flag_admission(&parsed.flags)?;
        Ok(Self {
            targets: parse_target_patterns(CommandKind::Build, &parsed.positionals)?,
            flags: parsed.flags,
            root_string_setting,
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

/// Reject configuration-affecting flags until their typed structural
/// representation is admitted. This runs before the CLI selects one-shot or
/// daemon mode, so neither path can silently discard a configuration input.
fn build_flag_admission(flags: &[ParsedFlag]) -> Result<Option<String>, CommandParseError> {
    let mut root_string_setting = None;
    for flag in flags {
        match flag.name.as_str() {
            "//:setting" => {
                let value =
                    flag.value
                        .clone()
                        .ok_or_else(|| CommandParseError::InvalidFlagValue {
                            flag: flag.raw.clone(),
                            message: "expected --//:setting=<Unicode>".to_owned(),
                        })?;
                root_string_setting = Some(value);
            }
            "config" => {
                return Err(unsupported_build_flag(flag));
            }
            name if admitted_build_flag(name) => {}
            _ => return Err(unsupported_build_flag(flag)),
        }
    }
    Ok(root_string_setting)
}

fn unsupported_build_flag(flag: &ParsedFlag) -> CommandParseError {
    CommandParseError::InvalidFlagValue {
        flag: flag.raw.clone(),
        message: "not supported by build".to_owned(),
    }
}

fn admitted_build_flag(name: &str) -> bool {
    matches!(
        name,
        // Bzlmod request inputs.
        "allow_yanked_versions"
            | "ignore_dev_dependency"
            | "noignore_dev_dependency"
            | "lockfile_mode"
            | "registry"
            // UI and output-base controls.
            | "color"
            | "show_progress"
            | "noshow_progress"
            | "keep_going"
            | "output_base"
            // BEP controls.
            | "build_event_json_file"
            | "build_event_text_file"
            | "bes_backend"
            | "bes_results_url"
            // Remote execution/cache controls.
            | "remote_cache"
            | "remote_executor"
            | "remote_header"
            | "remote_instance_name"
            | "remote_timeout"
            | "remote_retries"
            | "remote_default_exec_properties"
    )
}
