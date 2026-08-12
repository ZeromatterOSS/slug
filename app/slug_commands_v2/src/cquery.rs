/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

use std::path::Path;

use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_identity_v2::TargetPattern;
use slug_query_v2::QueryExpression;
use slug_query_v2::cquery_literals;
use slug_query_v2::validate_cquery_query;

use crate::common::CommandKind;
use crate::common::CommandParseError;
use crate::common::bzlmod_command_policy;
use crate::common::bzlmod_command_policy_for_workspace;
use crate::common::bzlmod_lockfile_mode;
use crate::common::bzlmod_registry_urls;
use crate::common::parse_bool_flag;
use crate::common::split_args;

const LABEL_EXPRESSION: &str = "str(target.label)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CqueryOutputMode {
    Label,
    LabelKind,
    StarlarkLabel,
    Graph,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CqueryRequest {
    pub expression: String,
    pub output_mode: CqueryOutputMode,
    pub include_implicit: bool,
    pub include_tool: bool,
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
        validate_cquery_query(&parsed_expression)
            .map_err(|error| unsupported(&error.to_string()))?;
        for literal in cquery_literals(&parsed_expression)
            .into_iter()
            .chain(parsed_expression.cquery_rdeps_seed())
        {
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
        let mut include_implicit = true;
        let mut include_tool = true;
        let mut saw_noimplicit_deps = false;
        let mut graph_unfactored = false;
        let mut saw_graph_unfactored = false;
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
                "implicit_deps" => include_implicit = parse_bool_flag(flag, false)?,
                "noimplicit_deps" => {
                    include_implicit = parse_bool_flag(flag, true)?;
                    saw_noimplicit_deps |= !include_implicit;
                }
                "tool_deps" => include_tool = parse_bool_flag(flag, false)?,
                "notool_deps" => include_tool = parse_bool_flag(flag, true)?,
                "nograph:factored" => {
                    graph_unfactored = !parse_bool_flag(flag, true)?;
                    saw_graph_unfactored = true;
                }
                "allow_yanked_versions"
                | "ignore_dev_dependency"
                | "noignore_dev_dependency"
                | "lockfile_mode"
                | "override_module"
                | "registry" => {}
                _ => {
                    return Err(unsupported(&format!(
                        "{} is not supported by this cquery",
                        flag.raw
                    )));
                }
            }
        }
        if parsed_expression.cquery_preactivation_deps_spec().is_some() && include_implicit {
            return Err(unsupported(
                "deps() requires --noimplicit_deps in this cquery",
            ));
        }
        let output_mode = match (output.as_deref(), starlark_expression.as_deref()) {
            (None | Some("label"), None) => CqueryOutputMode::Label,
            (Some("label_kind"), None) => CqueryOutputMode::LabelKind,
            (Some("starlark"), Some(LABEL_EXPRESSION)) => CqueryOutputMode::StarlarkLabel,
            (Some("graph"), None) => {
                let Some(_) = parsed_expression.cquery_preactivation_deps_spec() else {
                    return Err(unsupported(
                        "--output=graph requires a top-level deps() cquery expression",
                    ));
                };
                if !saw_noimplicit_deps || include_implicit {
                    return Err(unsupported("--output=graph requires --noimplicit_deps"));
                }
                if !saw_graph_unfactored || !graph_unfactored {
                    return Err(unsupported("--output=graph requires --nograph:factored"));
                }
                CqueryOutputMode::Graph
            }
            _ => {
                return Err(unsupported(
                    "expected default output, --output=label, --output=label_kind, or --output=starlark \
                     --starlark:expr=str(target.label)",
                ));
            }
        };
        if output_mode != CqueryOutputMode::Graph && saw_graph_unfactored {
            return Err(unsupported(
                "--nograph:factored is supported only with --output=graph",
            ));
        }
        let bzlmod_policy = match workspace {
            Some(workspace) => bzlmod_command_policy_for_workspace(&parsed.flags, workspace)?,
            None => bzlmod_command_policy(&parsed.flags)?,
        };
        let lockfile_mode = bzlmod_lockfile_mode(&parsed.flags)?;
        let registry_urls = bzlmod_registry_urls(&parsed.flags)?;
        Ok(Self {
            expression: cquery_expression,
            output_mode,
            include_implicit,
            include_tool,
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
