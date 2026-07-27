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
use slug_query_v2::QueryOrder;
use slug_query_v2::QueryPolicy;

use crate::common::CommandKind;
use crate::common::CommandParseError;
use crate::common::ParsedFlag;
use crate::common::QueryOutputFormat;
use crate::common::bzlmod_command_policy;
use crate::common::bzlmod_lockfile_mode;
use crate::common::bzlmod_registry_urls;
use crate::common::output_format;
use crate::common::parse_bool_flag;
use crate::common::parse_query_expression_for;
use crate::common::split_args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRequest {
    pub expression: String,
    pub output: QueryOutputFormat,
    pub order: QueryOrder,
    pub graph_factored: bool,
    pub policy: QueryPolicy,
    pub flags: Vec<ParsedFlag>,
    pub bzlmod_policy: BzlmodCommandPolicyKey,
    pub lockfile_mode: LockfileMode,
    pub registry_urls: Vec<String>,
}

impl QueryRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
        parse_query_like(CommandKind::Query, args)
    }
}

pub(crate) fn parse_query_like(
    command: CommandKind,
    args: &[impl AsRef<str>],
) -> Result<QueryRequest, CommandParseError> {
    let parsed = split_args(args);
    let expression = parse_query_expression_for(command, &parsed.positionals)?;
    let output = output_format(&parsed.flags);
    let (order, graph_factored, policy, bzlmod_policy, lockfile_mode, registry_urls) =
        if command == CommandKind::Query {
            let (order, graph_factored, policy) = validate_query_flags(&parsed.flags)?;
            (
                order,
                graph_factored,
                policy,
                bzlmod_command_policy(&[])?,
                bzlmod_lockfile_mode(&[])?,
                bzlmod_registry_urls(&parsed.flags)?,
            )
        } else {
            (
                QueryOrder::Auto,
                true,
                QueryPolicy::default(),
                bzlmod_command_policy(&parsed.flags)?,
                bzlmod_lockfile_mode(&parsed.flags)?,
                bzlmod_registry_urls(&parsed.flags)?,
            )
        };
    Ok(QueryRequest {
        expression,
        output,
        order,
        graph_factored,
        policy,
        flags: parsed.flags,
        bzlmod_policy,
        lockfile_mode,
        registry_urls,
    })
}

fn validate_query_flags(
    flags: &[ParsedFlag],
) -> Result<(QueryOrder, bool, QueryPolicy), CommandParseError> {
    let mut order = QueryOrder::Auto;
    let mut graph_factored = true;
    let mut policy = QueryPolicy::default();
    for flag in flags {
        match flag.name.as_str() {
            "output" => {
                let value =
                    required_query_flag_value(flag, "label, graph, label_kind, or package")?;
                if value == "text" {
                    return Err(CommandParseError::InvalidFlagValue {
                        flag: flag.raw.clone(),
                        message: "Invalid output format 'text'. Valid values are: label, label_kind, build, minrank, maxrank, package, location, graph, xml, proto, streamed_jsonproto, streamed_proto".to_owned(),
                    });
                }
                if value != "label"
                    && value != "graph"
                    && value != "label_kind"
                    && value != "package"
                {
                    return Err(CommandParseError::InvalidFlagValue {
                        flag: flag.raw.clone(),
                        message: format!(
                            "output format '{value}' is recognized but deferred; only label, graph, label_kind, and package are implemented"
                        ),
                    });
                }
            }
            "order_output" => {
                let value = required_query_flag_value(flag, "auto or full")?;
                order = QueryOrder::parse(value).map_err(|error| {
                    CommandParseError::InvalidFlagValue {
                        flag: flag.raw.clone(),
                        message: error.to_string(),
                    }
                })?;
            }
            "output_base" => {
                required_query_flag_value(flag, "a non-empty path")?;
            }
            "registry" => {
                required_query_flag_value(flag, "a non-empty registry URL")?;
            }
            "graph:factored" => {
                graph_factored = parse_bool_flag(flag, false)?;
            }
            "nograph:factored" => {
                graph_factored = parse_bool_flag(flag, true)?;
            }
            "graph:node_limit" => {
                let value = required_query_flag_value(flag, "the default value 512")?;
                let limit =
                    value
                        .parse::<i32>()
                        .map_err(|_| CommandParseError::InvalidFlagValue {
                            flag: flag.raw.clone(),
                            message: "expected an integer".to_owned(),
                        })?;
                if limit != 512 {
                    return Err(CommandParseError::InvalidFlagValue {
                        flag: flag.raw.clone(),
                        message: "--graph:node_limit other than 512 is deferred".to_owned(),
                    });
                }
            }
            "strict_test_suite" => {
                policy.strict_test_suite = parse_bool_flag(flag, false)?;
            }
            "nostrict_test_suite" => {
                policy.strict_test_suite = parse_bool_flag(flag, true)?;
            }
            _ => {
                return Err(CommandParseError::InvalidFlagValue {
                    flag: flag.raw.clone(),
                    message: "flag is not supported by loading query".to_owned(),
                });
            }
        }
    }
    Ok((order, graph_factored, policy))
}

fn required_query_flag_value<'a>(
    flag: &'a ParsedFlag,
    expected: &str,
) -> Result<&'a str, CommandParseError> {
    flag.value
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CommandParseError::InvalidFlagValue {
            flag: flag.raw.clone(),
            message: format!("expected {expected}"),
        })
}
