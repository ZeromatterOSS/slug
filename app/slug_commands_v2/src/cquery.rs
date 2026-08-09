/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_identity_v2::TargetPattern;
use slug_query_v2::QueryExpression;
use slug_query_v2::function_free_literals;
use slug_query_v2::validate_function_free_query;

use crate::common::CommandKind;
use crate::common::CommandParseError;
use crate::common::bzlmod_command_policy;
use crate::common::bzlmod_lockfile_mode;
use crate::common::bzlmod_registry_urls;
use crate::common::split_args;

const LABEL_EXPRESSION: &str = "str(target.label)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CqueryOutputMode {
    Label,
    StarlarkLabel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CqueryRequest {
    pub expression: String,
    pub output_mode: CqueryOutputMode,
    pub output_base: Option<String>,
    /// The one admitted root build setting transition. Configuration owns its
    /// typed representation and semantic identity.
    pub root_string_setting: Option<String>,
    pub bzlmod_policy: BzlmodCommandPolicyKey,
    pub lockfile_mode: LockfileMode,
    pub registry_urls: Vec<String>,
}

impl CqueryRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
        let parsed = split_args(args);
        if !parsed.passthrough.is_empty() {
            return Err(unsupported("-- is not supported by this cquery"));
        }
        if parsed.positionals.len() != 1 {
            return Err(if parsed.positionals.is_empty() {
                CommandParseError::MissingTarget {
                    command: CommandKind::Cquery,
                }
            } else {
                unsupported("cquery accepts exactly one literal target label")
            });
        }
        let cquery_expression = parsed.positionals[0].clone();
        let parsed_expression = QueryExpression::parse(&cquery_expression)
            .map_err(|error| unsupported(&error.to_string()))?;
        validate_function_free_query(&parsed_expression)
            .map_err(|error| unsupported(&error.to_string()))?;
        for literal in function_free_literals(&parsed_expression) {
            let target = TargetPattern::parse(literal).map_err(|message| {
                CommandParseError::InvalidTargetPattern {
                    value: literal.to_owned(),
                    message,
                }
            })?;
            let TargetPattern::Single(label) = target else {
                return Err(unsupported(
                    "target patterns are not supported by this cquery",
                ));
            };
            if !label.repo().is_root() {
                return Err(unsupported(
                    "external repository labels are not supported by this cquery",
                ));
            }
        }

        let mut output = None;
        let mut starlark_expression = None;
        let mut output_base = None;
        let mut root_string_setting = None;
        for flag in &parsed.flags {
            match flag.name.as_str() {
                "output" => set_once(&mut output, flag, "--output")?,
                "starlark:expr" => set_once(&mut starlark_expression, flag, "--starlark:expr")?,
                "output_base" => set_once(&mut output_base, flag, "--output_base")?,
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
                "allow_yanked_versions"
                | "ignore_dev_dependency"
                | "noignore_dev_dependency"
                | "lockfile_mode"
                | "registry" => {}
                _ => {
                    return Err(unsupported(&format!(
                        "{} is not supported by this cquery",
                        flag.raw
                    )));
                }
            }
        }
        let output_mode = match (output.as_deref(), starlark_expression.as_deref()) {
            (None | Some("label"), None) => CqueryOutputMode::Label,
            (Some("starlark"), Some(LABEL_EXPRESSION))
                if parsed_expression.single_literal().is_some() =>
            {
                CqueryOutputMode::StarlarkLabel
            }
            (Some("starlark"), Some(LABEL_EXPRESSION)) => {
                return Err(unsupported(
                    "set expressions are not supported by --output=starlark",
                ));
            }
            _ => {
                return Err(unsupported(
                    "expected default output, --output=label, or --output=starlark \
                     --starlark:expr=str(target.label)",
                ));
            }
        };
        let bzlmod_policy = bzlmod_command_policy(&parsed.flags)?;
        let lockfile_mode = bzlmod_lockfile_mode(&parsed.flags)?;
        let registry_urls = bzlmod_registry_urls(&parsed.flags)?;
        Ok(Self {
            expression: cquery_expression,
            output_mode,
            output_base,
            root_string_setting,
            bzlmod_policy,
            lockfile_mode,
            registry_urls,
        })
    }
}

fn set_once(
    slot: &mut Option<String>,
    flag: &crate::common::ParsedFlag,
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
        flag: "cquery".to_owned(),
        message: message.to_owned(),
    }
}
