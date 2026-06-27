/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use crate::common::CommandKind;
use crate::common::CommandParseError;
use crate::common::CommandPlaceholderError;
use crate::query::QueryRequest;
use crate::query::parse_query_like;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CqueryRequest {
    pub query: QueryRequest,
}

impl CqueryRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
        Ok(Self {
            query: parse_query_like(CommandKind::Cquery, args)?,
        })
    }

    pub const fn placeholder_error(&self) -> CommandPlaceholderError {
        CommandPlaceholderError::planned(
            CommandKind::Cquery,
            "Stage 6/8",
            "configured target provider output is not wired to the command runner yet",
        )
    }
}
