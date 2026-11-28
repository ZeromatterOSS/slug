/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::path::Path;

use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_identity_v2::TargetPattern;
use slug_query_v2::AqueryMnemonicFilter;
use slug_query_v2::AqueryScope;
use slug_query_v2::QueryExpression;
use slug_query_v2::aquery_expression_spec;

use crate::common::CommandKind;
use crate::common::CommandParseError;
use crate::common::ParsedFlag;
use crate::common::RepositoryEnvironmentOverride;
use crate::common::archive_distdir_value;
use crate::common::bzlmod_command_policy;
use crate::common::bzlmod_command_policy_for_workspace;
use crate::common::bzlmod_lockfile_mode;
use crate::common::bzlmod_registry_urls;
use crate::common::parse_query_expression_for;
use crate::common::repository_environment_overrides;
use crate::common::split_args;
use crate::common::starlark_configuration_occurrence;
use crate::common::validate_bare_boolean_flag;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AqueryRequest {
    pub expression: String,
    pub target: TargetPattern,
    pub scope: AqueryScope,
    pub mnemonic_filter: Option<AqueryMnemonicFilter>,
    pub output_base: Option<String>,
    pub configuration_overlay: CommandConfigurationOverlay,
    pub distdirs: Vec<String>,
    pub bzlmod_policy: BzlmodCommandPolicyKey,
    pub lockfile_mode: LockfileMode,
    pub registry_urls: Vec<String>,
    pub repository_environment_overrides: Vec<RepositoryEnvironmentOverride>,
}

impl AqueryRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
        Self::parse_impl(args, None)
    }

    pub fn parse_at_workspace(
        args: &[impl AsRef<str>],
        workspace: &Path,
    ) -> Result<Self, CommandParseError> {
        Self::parse_impl(args, Some(workspace))
    }

    fn parse_impl(
        args: &[impl AsRef<str>],
        workspace: Option<&Path>,
    ) -> Result<Self, CommandParseError> {
        let parsed = split_args(args);
        if !parsed.passthrough.is_empty() {
            return Err(unsupported("-- is not supported by this aquery"));
        }
        let expression = parse_query_expression_for(CommandKind::Aquery, &parsed.positionals)?;
        let parsed_expression = QueryExpression::parse(&expression).map_err(|error| {
            CommandParseError::InvalidQueryExpression {
                message: error.to_string(),
            }
        })?;
        let spec = aquery_expression_spec(&parsed_expression).map_err(|error| {
            CommandParseError::InvalidQueryExpression {
                message: error.to_string(),
            }
        })?;
        let target = TargetPattern::parse(spec.target()).map_err(|message| {
            CommandParseError::InvalidTargetPattern {
                value: spec.target().to_owned(),
                message,
            }
        })?;
        if !matches!(
            &target,
            TargetPattern::Single(label) if label.repo().is_root()
        ) {
            return Err(unsupported(
                "aquery accepts only a main-repository literal target label",
            ));
        }
        let mnemonic_filter = spec.compile_mnemonic_filter().map_err(|error| {
            CommandParseError::InvalidQueryExpression {
                message: error.to_string(),
            }
        })?;

        let mut output_seen = false;
        let mut output_base = None;
        let mut configuration = Vec::new();
        let mut distdirs = Vec::new();
        for flag in &parsed.flags {
            if let Some(occurrence) = starlark_configuration_occurrence(flag)? {
                configuration.push(occurrence);
                continue;
            }
            match flag.name.as_str() {
                "output" => {
                    if output_seen {
                        return Err(unsupported("--output may be specified only once"));
                    }
                    output_seen = true;
                    if flag.value.as_deref() != Some("text") {
                        return Err(CommandParseError::InvalidFlagValue {
                            flag: flag.raw.clone(),
                            message: "only --output=text is supported by this aquery".to_owned(),
                        });
                    }
                }
                "output_base" => set_once(&mut output_base, flag, "--output_base")?,
                "noshow_progress" => validate_bare_boolean_flag(flag)?,
                "distdir" => distdirs.push(archive_distdir_value(flag)?),
                "allow_yanked_versions"
                | "ignore_dev_dependency"
                | "noignore_dev_dependency"
                | "lockfile_mode"
                | "override_module"
                | "registry"
                | "repo_env" => {}
                _ => {
                    return Err(unsupported(&format!(
                        "{} is not supported by this aquery",
                        flag.raw
                    )));
                }
            }
        }

        let repository_environment_overrides = repository_environment_overrides(&parsed.flags)?;
        Ok(Self {
            expression,
            target,
            scope: spec.scope(),
            mnemonic_filter,
            output_base,
            configuration_overlay: configuration.into(),
            distdirs,
            bzlmod_policy: match workspace {
                Some(workspace) => bzlmod_command_policy_for_workspace(&parsed.flags, workspace)?,
                None => bzlmod_command_policy(&parsed.flags)?,
            },
            lockfile_mode: bzlmod_lockfile_mode(&parsed.flags)?,
            registry_urls: bzlmod_registry_urls(&parsed.flags)?,
            repository_environment_overrides,
        })
    }
}

fn set_once(
    slot: &mut Option<String>,
    flag: &ParsedFlag,
    name: &str,
) -> Result<(), CommandParseError> {
    let value = flag
        .value
        .clone()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CommandParseError::InvalidFlagValue {
            flag: flag.raw.clone(),
            message: format!("{name} requires a non-empty value"),
        })?;
    if slot.replace(value).is_some() {
        return Err(unsupported(&format!("{name} may be specified only once")));
    }
    Ok(())
}

fn unsupported(message: &str) -> CommandParseError {
    CommandParseError::InvalidFlagValue {
        flag: "aquery".to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use slug_configuration_v2::CommandConfigurationOccurrence;

    use super::*;

    #[test]
    fn pinned_rust_aquery_flags_retain_setting_and_ordered_distdirs() {
        let mut flags = vec![
            "mnemonic(\"(Rustc|SymlinkTree|RunfilesTree)\", //pkg:probe)",
            "--output=text",
            "--@rules_rust//rust/toolchain/channel=nightly",
            "--lockfile_mode=off",
            "--distdir=relative",
            "--distdir=/absolute",
            "--distdir=relative",
        ];
        let expected = AqueryRequest::parse(&flags).unwrap();
        flags.push("--noshow_progress");
        let parsed = AqueryRequest::parse(&flags).unwrap();
        assert_eq!(parsed, expected);
        assert_eq!(parsed.scope, AqueryScope::Literal);
        assert_eq!(parsed.distdirs, ["relative", "/absolute", "relative"]);
        assert!(parsed.configuration_overlay.iter().eq([
            &CommandConfigurationOccurrence::starlark(
                "@rules_rust//rust/toolchain/channel",
                Some("nightly"),
                false,
            ),
        ]));
        let filter = parsed.mnemonic_filter.unwrap();
        assert!(filter.is_match("Rustc"));
        assert!(!filter.is_match("RustcMetadata"));

        for flag in [
            "--noshow_progress=false",
            "--distdir=",
            "--distdir",
            "--compilation_mode=opt",
        ] {
            assert!(
                matches!(
                    AqueryRequest::parse(&["//pkg:probe", flag]),
                    Err(CommandParseError::InvalidFlagValue { .. })
                ),
                "{flag}"
            );
        }
    }
}
