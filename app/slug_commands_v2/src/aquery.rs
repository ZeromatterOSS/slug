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
use slug_query_v2::QueryExpression;
use slug_query_v2::aquery_literal;

use crate::common::CommandKind;
use crate::common::CommandParseError;
use crate::common::ParsedFlag;
use crate::common::bzlmod_command_policy;
use crate::common::bzlmod_lockfile_mode;
use crate::common::bzlmod_registry_urls;
use crate::common::parse_query_expression_for;
use crate::common::split_args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AqueryRequest {
    pub expression: String,
    pub target: TargetPattern,
    pub output_base: Option<String>,
    pub bzlmod_policy: BzlmodCommandPolicyKey,
    pub lockfile_mode: LockfileMode,
    pub registry_urls: Vec<String>,
}

impl AqueryRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
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
        let literal = aquery_literal(&parsed_expression).map_err(|error| {
            CommandParseError::InvalidQueryExpression {
                message: error.to_string(),
            }
        })?;
        let target = TargetPattern::parse(literal).map_err(|message| {
            CommandParseError::InvalidTargetPattern {
                value: literal.to_owned(),
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

        let mut output_seen = false;
        let mut output_base = None;
        for flag in &parsed.flags {
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
                "allow_yanked_versions"
                | "ignore_dev_dependency"
                | "noignore_dev_dependency"
                | "lockfile_mode"
                | "registry" => {}
                _ => {
                    return Err(unsupported(&format!(
                        "{} is not supported by this aquery",
                        flag.raw
                    )));
                }
            }
        }

        Ok(Self {
            expression,
            target,
            output_base,
            bzlmod_policy: bzlmod_command_policy(&parsed.flags)?,
            lockfile_mode: bzlmod_lockfile_mode(&parsed.flags)?,
            registry_urls: bzlmod_registry_urls(&parsed.flags)?,
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
