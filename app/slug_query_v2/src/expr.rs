/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-shaped query syntax values and loading-query function registry.
//!
//! Parsing is the owned lowering of the borrowed-span/nom parser in
//! `parser.rs`, adapted from Buck2's `buck2_query_parser`.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct Spanned<T> {
    pub span: SourceSpan,
    pub value: T,
}

impl<T> Spanned<T> {
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            span: self.span,
            value: map(self.value),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct QueryExpression {
    pub span: SourceSpan,
    pub kind: QueryExpressionKind,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum QueryExpressionKind {
    TargetLiteral(CompactString),
    Integer(u64),
    Function {
        name: Spanned<CompactString>,
        args: Arc<[QueryExpression]>,
    },
    Let {
        name: Spanned<CompactString>,
        value: Box<QueryExpression>,
        body: Box<QueryExpression>,
    },
    Set(Arc<[Spanned<CompactString>]>),
    /// Buck2's non-recursive `BinaryOpSequence`; evaluation folds it from the
    /// left, preserving Bazel's equal-precedence semantics.
    BinaryOpSequence {
        left: Box<QueryExpression>,
        operations: Arc<[(BinaryOperator, QueryExpression)]>,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum BinaryOperator {
    Union,
    Except,
    Intersect,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct QueryParseError {
    pub message: CompactString,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum QueryArgumentKind {
    Expression,
    Word,
    Integer,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum QueryFunctionStatus {
    Implemented,
    Deferred,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct QueryFunctionSpec {
    pub name: &'static str,
    pub mandatory_arguments: usize,
    pub argument_kinds: &'static [QueryArgumentKind],
    pub status: QueryFunctionStatus,
}

const EXPR: QueryArgumentKind = QueryArgumentKind::Expression;
const WORD: QueryArgumentKind = QueryArgumentKind::Word;
const INT: QueryArgumentKind = QueryArgumentKind::Integer;

// Bazel 9.2 QueryEnvironment.DEFAULT_QUERY_FUNCTIONS, in source order.
const FUNCTIONS: &[QueryFunctionSpec] = &[
    QueryFunctionSpec {
        name: "allpaths",
        mandatory_arguments: 2,
        argument_kinds: &[EXPR, EXPR],
        status: QueryFunctionStatus::Implemented,
    },
    deferred("attr", 3, &[WORD, WORD, EXPR]),
    QueryFunctionSpec {
        name: "buildfiles",
        mandatory_arguments: 1,
        argument_kinds: &[EXPR],
        status: QueryFunctionStatus::Implemented,
    },
    QueryFunctionSpec {
        name: "deps",
        mandatory_arguments: 1,
        argument_kinds: &[EXPR, INT],
        status: QueryFunctionStatus::Implemented,
    },
    QueryFunctionSpec {
        name: "executables",
        mandatory_arguments: 1,
        argument_kinds: &[EXPR],
        status: QueryFunctionStatus::Implemented,
    },
    deferred("filter", 2, &[WORD, EXPR]),
    deferred("kind", 2, &[WORD, EXPR]),
    QueryFunctionSpec {
        name: "labels",
        mandatory_arguments: 2,
        argument_kinds: &[WORD, EXPR],
        status: QueryFunctionStatus::Implemented,
    },
    QueryFunctionSpec {
        name: "loadfiles",
        mandatory_arguments: 1,
        argument_kinds: &[EXPR],
        status: QueryFunctionStatus::Implemented,
    },
    QueryFunctionSpec {
        name: "rdeps",
        mandatory_arguments: 2,
        argument_kinds: &[EXPR, EXPR, INT],
        status: QueryFunctionStatus::Implemented,
    },
    QueryFunctionSpec {
        name: "same_pkg_direct_rdeps",
        mandatory_arguments: 1,
        argument_kinds: &[EXPR],
        status: QueryFunctionStatus::Implemented,
    },
    QueryFunctionSpec {
        name: "siblings",
        mandatory_arguments: 1,
        argument_kinds: &[EXPR],
        status: QueryFunctionStatus::Implemented,
    },
    QueryFunctionSpec {
        name: "some",
        mandatory_arguments: 1,
        argument_kinds: &[EXPR, INT],
        status: QueryFunctionStatus::Implemented,
    },
    QueryFunctionSpec {
        name: "somepath",
        mandatory_arguments: 2,
        argument_kinds: &[EXPR, EXPR],
        status: QueryFunctionStatus::Implemented,
    },
    QueryFunctionSpec {
        name: "tests",
        mandatory_arguments: 1,
        argument_kinds: &[EXPR],
        status: QueryFunctionStatus::Implemented,
    },
    deferred("visible", 2, &[EXPR, EXPR]),
];

const fn deferred(
    name: &'static str,
    mandatory_arguments: usize,
    argument_kinds: &'static [QueryArgumentKind],
) -> QueryFunctionSpec {
    QueryFunctionSpec {
        name,
        mandatory_arguments,
        argument_kinds,
        status: QueryFunctionStatus::Deferred,
    }
}

pub fn loading_query_functions() -> &'static [QueryFunctionSpec] {
    FUNCTIONS
}

pub fn loading_query_function(name: &str) -> Option<&'static QueryFunctionSpec> {
    FUNCTIONS.iter().find(|function| function.name == name)
}

pub fn parse_query_expression(source: &str) -> Result<QueryExpression, QueryParseError> {
    crate::parser::parse(source)
}

pub fn validate_loading_query(expression: &QueryExpression) -> Result<(), QueryParseError> {
    validate_expression(expression)
}

impl QueryExpression {
    pub fn parse(source: &str) -> Result<Self, QueryParseError> {
        parse_query_expression(source)
    }

    /// Bazel's QueryCommand AUTO-order exception applies only when the parsed
    /// root expression is directly `somepath`. Parentheses lower away in the
    /// parser, while binary operations and `let` retain wrapper nodes.
    pub fn is_top_level_somepath(&self) -> bool {
        matches!(
            &self.kind,
            QueryExpressionKind::Function { name, .. } if name.value == "somepath"
        )
    }

    pub(crate) fn java_integer_literal(&self) -> Result<i32, CompactString> {
        let raw = match &self.kind {
            QueryExpressionKind::Integer(value) => CompactString::from(value.to_string()),
            QueryExpressionKind::TargetLiteral(value) => value.clone(),
            _ => CompactString::from(self.to_string()),
        };
        raw.parse::<i32>().map_err(|_| raw)
    }
}

fn validate_expression(expression: &QueryExpression) -> Result<(), QueryParseError> {
    match &expression.kind {
        QueryExpressionKind::TargetLiteral(_)
        | QueryExpressionKind::Integer(_)
        | QueryExpressionKind::Set(_) => Ok(()),
        QueryExpressionKind::Let { value, body, .. } => {
            validate_expression(value)?;
            validate_expression(body)
        }
        QueryExpressionKind::BinaryOpSequence { left, operations } => {
            validate_expression(left)?;
            for (_, right) in operations.iter() {
                validate_expression(right)?;
            }
            Ok(())
        }
        QueryExpressionKind::Function { name, args } => {
            let Some(spec) = loading_query_function(&name.value) else {
                let expected = FUNCTIONS
                    .iter()
                    .map(|spec| spec.name)
                    .collect::<Vec<_>>()
                    .join("', '");
                return Err(QueryParseError::new(
                    format!(
                        "unknown function '{}'; expected one of ['{}']",
                        name.value, expected
                    ),
                    name.span,
                ));
            };
            if args.len() < spec.mandatory_arguments {
                return Err(QueryParseError::new(
                    format!("too few arguments to function '{}'", spec.name),
                    expression.span,
                ));
            }
            if args.len() > spec.argument_kinds.len() {
                return Err(QueryParseError::new(
                    format!("too many arguments to function '{}'", spec.name),
                    expression.span,
                ));
            }
            for (index, (argument, expected)) in args.iter().zip(spec.argument_kinds).enumerate() {
                let valid = match expected {
                    // Bazel treats an integer token in an expression position
                    // as a target literal (for example `1` becomes `//:1`).
                    QueryArgumentKind::Expression => true,
                    QueryArgumentKind::Word => {
                        matches!(argument.kind, QueryExpressionKind::TargetLiteral(_))
                    }
                    QueryArgumentKind::Integer => match argument.java_integer_literal() {
                        Ok(_) => true,
                        Err(raw) => {
                            return Err(QueryParseError::new(
                                format!("expected an integer literal: '{raw}'"),
                                argument.span,
                            ));
                        }
                    },
                };
                if !valid {
                    return Err(QueryParseError::new(
                        format!(
                            "argument {} to function '{}' must be {}",
                            index + 1,
                            spec.name,
                            expected
                        ),
                        argument.span,
                    ));
                }
                validate_expression(argument)?;
            }
            if spec.status == QueryFunctionStatus::Deferred {
                return Err(QueryParseError::new(
                    format!(
                        "query function '{}' is recognized by Bazel 9.2 but not implemented in this loading-query slice",
                        spec.name
                    ),
                    name.span,
                ));
            }
            Ok(())
        }
    }
}

impl QueryParseError {
    pub(crate) fn new(message: impl Into<CompactString>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for QueryParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at bytes {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for QueryParseError {}

impl fmt::Display for QueryArgumentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Expression => "an expression",
            Self::Word => "a word",
            Self::Integer => "an integer",
        })
    }
}

impl fmt::Display for BinaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Union => "union",
            Self::Except => "except",
            Self::Intersect => "intersect",
        })
    }
}

impl fmt::Display for QueryExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            QueryExpressionKind::TargetLiteral(value) => f.write_str(value),
            QueryExpressionKind::Integer(value) => write!(f, "{value}"),
            QueryExpressionKind::Function { name, args } => {
                write!(f, "{}(", name.value)?;
                for (index, argument) in args.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{argument}")?;
                }
                f.write_str(")")
            }
            QueryExpressionKind::Let { name, value, body } => {
                write!(f, "let {} = {value} in {body}", name.value)
            }
            QueryExpressionKind::Set(values) => {
                f.write_str("set(")?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        f.write_str(" ")?;
                    }
                    f.write_str(&value.value)?;
                }
                f.write_str(")")
            }
            QueryExpressionKind::BinaryOpSequence { left, operations } => {
                write!(f, "{left}")?;
                for (operator, right) in operations.iter() {
                    write!(f, " {operator} {right}")?;
                }
                Ok(())
            }
        }
    }
}
