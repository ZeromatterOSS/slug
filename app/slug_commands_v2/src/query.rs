/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_query_v2::QueryExpression;

use crate::common::CommandKind;
use crate::common::CommandParseError;
use crate::common::CommandPlaceholderError;
use crate::common::ParsedFlag;
use crate::common::QueryOutputFormat;
use crate::common::output_format;
use crate::common::parse_query_expression_for;
use crate::common::split_args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRequest {
    pub expression: QueryExpression,
    pub output: QueryOutputFormat,
    pub flags: Vec<ParsedFlag>,
}

impl QueryRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
        parse_query_like(CommandKind::Query, args)
    }

    pub const fn placeholder_error(&self) -> CommandPlaceholderError {
        CommandPlaceholderError::planned(
            CommandKind::Query,
            "Stage 4/8",
            "package graph query evaluation is not wired to the command runner yet",
        )
    }
}

pub(crate) fn parse_query_like(
    command: CommandKind,
    args: &[impl AsRef<str>],
) -> Result<QueryRequest, CommandParseError> {
    let parsed = split_args(args);
    let expression = parse_query_expression_for(command, &parsed.positionals)?;
    let output = output_format(&parsed.flags);
    Ok(QueryRequest {
        expression,
        output,
        flags: parsed.flags,
    })
}
