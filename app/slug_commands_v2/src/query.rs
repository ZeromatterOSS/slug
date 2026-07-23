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

use crate::common::CommandKind;
use crate::common::CommandParseError;
use crate::common::ParsedFlag;
use crate::common::QueryOutputFormat;
use crate::common::bzlmod_command_policy;
use crate::common::bzlmod_lockfile_mode;
use crate::common::output_format;
use crate::common::parse_query_expression_for;
use crate::common::split_args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRequest {
    pub expression: String,
    pub output: QueryOutputFormat,
    pub order: QueryOrder,
    pub flags: Vec<ParsedFlag>,
    pub bzlmod_policy: BzlmodCommandPolicyKey,
    pub lockfile_mode: LockfileMode,
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
    let (order, bzlmod_policy, lockfile_mode) = if command == CommandKind::Query {
        let order = validate_query_flags(&parsed.flags)?;
        (
            order,
            bzlmod_command_policy(&[])?,
            bzlmod_lockfile_mode(&[])?,
        )
    } else {
        (
            QueryOrder::Auto,
            bzlmod_command_policy(&parsed.flags)?,
            bzlmod_lockfile_mode(&parsed.flags)?,
        )
    };
    Ok(QueryRequest {
        expression,
        output,
        order,
        flags: parsed.flags,
        bzlmod_policy,
        lockfile_mode,
    })
}

fn validate_query_flags(flags: &[ParsedFlag]) -> Result<QueryOrder, CommandParseError> {
    let mut order = QueryOrder::Auto;
    for flag in flags {
        match flag.name.as_str() {
            "output" => {
                let value = required_query_flag_value(flag, "text")?;
                if value != "text" {
                    return Err(CommandParseError::InvalidFlagValue {
                        flag: flag.raw.clone(),
                        message: format!(
                            "output format '{value}' is recognized but deferred; only text is implemented"
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
            _ => {
                return Err(CommandParseError::InvalidFlagValue {
                    flag: flag.raw.clone(),
                    message: "flag is not supported by loading query".to_owned(),
                });
            }
        }
    }
    Ok(order)
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
