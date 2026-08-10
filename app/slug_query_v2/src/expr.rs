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
use starlark_map::small_set::SmallSet;

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

/// The one configured `deps()` form whose traversal semantics are admitted.
#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct CqueryDepsSpec {
    target: CompactString,
    depth: Option<i32>,
}

impl CqueryDepsSpec {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub const fn depth(&self) -> Option<i32> {
        self.depth
    }
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
    QueryFunctionSpec {
        name: "attr",
        mandatory_arguments: 3,
        argument_kinds: &[WORD, WORD, EXPR],
        status: QueryFunctionStatus::Implemented,
    },
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
    QueryFunctionSpec {
        name: "filter",
        mandatory_arguments: 2,
        argument_kinds: &[WORD, EXPR],
        status: QueryFunctionStatus::Implemented,
    },
    QueryFunctionSpec {
        name: "kind",
        mandatory_arguments: 2,
        argument_kinds: &[WORD, EXPR],
        status: QueryFunctionStatus::Implemented,
    },
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
    QueryFunctionSpec {
        name: "visible",
        mandatory_arguments: 2,
        argument_kinds: &[EXPR, EXPR],
        status: QueryFunctionStatus::Implemented,
    },
];

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

/// Validates the deliberately small configured-query subset.
pub fn validate_cquery_query(expression: &QueryExpression) -> Result<(), QueryParseError> {
    if matches!(
        &expression.kind,
        QueryExpressionKind::Function { name, .. } if name.value == "deps"
    ) {
        return parse_cquery_deps_spec(expression).map(|_| ());
    }
    let mut bindings = SmallSet::new();
    validate_cquery_query_inner(expression, &mut bindings)
}

fn validate_cquery_query_inner(
    expression: &QueryExpression,
    bindings: &mut SmallSet<CompactString>,
) -> Result<(), QueryParseError> {
    match &expression.kind {
        QueryExpressionKind::TargetLiteral(literal) => match literal.strip_prefix('$') {
            Some(name) if bindings.contains(name) => Ok(()),
            Some(_) => Err(QueryParseError::new(
                format!("undefined query variable '{literal}'"),
                expression.span,
            )),
            None => Ok(()),
        },
        QueryExpressionKind::Set(literals) => {
            for literal in literals.iter() {
                if literal.value.starts_with('$') {
                    return Err(QueryParseError::new(
                        "set() accepts only concrete target literals",
                        literal.span,
                    ));
                }
            }
            Ok(())
        }
        QueryExpressionKind::Let { name, value, body } => {
            validate_cquery_query_inner(value, bindings)?;
            let shadows_existing = bindings.contains(name.value.as_str());
            bindings.insert(name.value.clone());
            let result = validate_cquery_query_inner(body, bindings);
            if !shadows_existing {
                bindings.shift_remove(name.value.as_str());
            }
            result
        }
        QueryExpressionKind::BinaryOpSequence { left, operations } => {
            validate_cquery_query_inner(left, bindings)?;
            for (_, right) in operations.iter() {
                validate_cquery_query_inner(right, bindings)?;
            }
            Ok(())
        }
        QueryExpressionKind::Integer(_) => Err(QueryParseError::new(
            "integer literals are not supported by this cquery",
            expression.span,
        )),
        QueryExpressionKind::Function { name, args } if name.value == "filter" => {
            let spec = cquery_function("filter");
            validate_function_arguments(expression, args, spec)?;
            validate_cquery_query_inner(&args[1], bindings)
        }
        QueryExpressionKind::Function { name, args } if name.value == "some" => {
            let spec = cquery_function("some");
            validate_function_arguments(expression, args, spec)?;
            // The optional count is an integer parameter, never a target
            // expression. It has already been validated above.
            validate_cquery_query_inner(&args[0], bindings)
        }
        QueryExpressionKind::Function { name, args } if name.value == "siblings" => {
            let spec = cquery_function("siblings");
            validate_function_arguments(expression, args, spec)?;
            validate_cquery_query_inner(&args[0], bindings)
        }
        QueryExpressionKind::Function { name, args } if name.value == "visible" => {
            let spec = cquery_function("visible");
            validate_function_arguments(expression, args, spec)?;
            validate_cquery_query_inner(&args[0], bindings)?;
            validate_cquery_query_inner(&args[1], bindings)
        }
        QueryExpressionKind::Function { name, args }
            if matches!(name.value.as_str(), "buildfiles" | "loadfiles") =>
        {
            let spec = cquery_function(name.value.as_str());
            validate_function_arguments(expression, args, spec)?;
            validate_cquery_query_inner(&args[0], bindings)
        }
        QueryExpressionKind::Function { name, args } if name.value == "executables" => {
            let spec = cquery_function("executables");
            validate_function_arguments(expression, args, spec)?;
            validate_cquery_query_inner(&args[0], bindings)
        }
        QueryExpressionKind::Function { name, args } if name.value == "kind" => {
            let spec = cquery_function("kind");
            validate_function_arguments(expression, args, spec)?;
            validate_cquery_query_inner(&args[1], bindings)
        }
        QueryExpressionKind::Function { name, .. } if name.value == "deps" => {
            Err(QueryParseError::new(
                "deps() is supported only as a top-level cquery expression",
                name.span,
            ))
        }
        QueryExpressionKind::Function { name, .. } => Err(QueryParseError::new(
            format!(
                "query function '{}' is not supported by this cquery",
                name.value
            ),
            name.span,
        )),
    }
}

/// Returns concrete literals in lexical resolution order. Variables are
/// intentionally absent: they are resolved by the shared evaluator.
pub fn cquery_literals(expression: &QueryExpression) -> Vec<&str> {
    let mut literals = Vec::new();
    collect_cquery_literals(expression, &mut literals);
    literals
}

fn collect_cquery_literals<'a>(expression: &'a QueryExpression, literals: &mut Vec<&'a str>) {
    match &expression.kind {
        QueryExpressionKind::TargetLiteral(literal) if !literal.starts_with('$') => {
            literals.push(literal)
        }
        QueryExpressionKind::Set(values) => {
            literals.extend(values.iter().map(|value| value.value.as_str()));
        }
        QueryExpressionKind::Let { value, body, .. } => {
            collect_cquery_literals(value, literals);
            collect_cquery_literals(body, literals);
        }
        QueryExpressionKind::BinaryOpSequence { left, operations } => {
            collect_cquery_literals(left, literals);
            for (_, right) in operations.iter() {
                collect_cquery_literals(right, literals);
            }
        }
        QueryExpressionKind::Function { name, args } if name.value == "filter" => {
            collect_cquery_literals(&args[1], literals);
        }
        QueryExpressionKind::Function { name, args } if name.value == "some" => {
            // A selection count is not a target literal and must not create a
            // configured root before the expression is evaluated.
            collect_cquery_literals(&args[0], literals);
        }
        QueryExpressionKind::Function { name, args } if name.value == "siblings" => {
            collect_cquery_literals(&args[0], literals);
        }
        QueryExpressionKind::Function { name, args } if name.value == "visible" => {
            collect_cquery_literals(&args[0], literals);
            collect_cquery_literals(&args[1], literals);
        }
        QueryExpressionKind::Function { name, args }
            if matches!(name.value.as_str(), "buildfiles" | "loadfiles") =>
        {
            collect_cquery_literals(&args[0], literals);
        }
        QueryExpressionKind::Function { name, args } if name.value == "executables" => {
            collect_cquery_literals(&args[0], literals);
        }
        QueryExpressionKind::Function { name, args } if name.value == "kind" => {
            collect_cquery_literals(&args[1], literals);
        }
        QueryExpressionKind::Function { name, args } if name.value == "deps" => {
            if let Some(operand) = args.first() {
                collect_cquery_literals(operand, literals);
            }
        }
        QueryExpressionKind::TargetLiteral(_)
        | QueryExpressionKind::Integer(_)
        | QueryExpressionKind::Function { .. } => {}
    }
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

    /// Returns the admitted top-level configured `deps()` invocation after it
    /// has passed [`validate_cquery_query`].
    pub fn cquery_deps_spec(&self) -> Option<CqueryDepsSpec> {
        parse_cquery_deps_spec(self).ok()
    }
}

fn parse_cquery_deps_spec(expression: &QueryExpression) -> Result<CqueryDepsSpec, QueryParseError> {
    let QueryExpressionKind::Function { name, args } = &expression.kind else {
        return Err(QueryParseError::new(
            "deps() is supported only as a top-level cquery expression",
            expression.span,
        ));
    };
    if name.value != "deps" {
        return Err(QueryParseError::new(
            "deps() is supported only as a top-level cquery expression",
            name.span,
        ));
    }
    validate_function_arguments(expression, args, cquery_function("deps"))?;
    let target = match args.first().map(|argument| &argument.kind) {
        Some(QueryExpressionKind::TargetLiteral(target)) if !target.starts_with('$') => {
            target.clone()
        }
        Some(_) => {
            return Err(QueryParseError::new(
                "cquery deps() requires one concrete target literal",
                args[0].span,
            ));
        }
        None => unreachable!("the deps function arity was validated"),
    };
    let depth = match args.get(1) {
        Some(argument) => {
            let depth = argument.java_integer_literal().map_err(|raw| {
                QueryParseError::new(
                    format!("expected an integer literal: '{raw}'"),
                    argument.span,
                )
            })?;
            if depth < 0 {
                return Err(QueryParseError::new(
                    "cquery deps() depth must be nonnegative",
                    argument.span,
                ));
            }
            Some(depth)
        }
        None => None,
    };
    Ok(CqueryDepsSpec { target, depth })
}

fn cquery_function(name: &str) -> &'static QueryFunctionSpec {
    loading_query_function(name).expect("cquery function is in the static Bazel registry")
}

fn validate_function_arguments(
    expression: &QueryExpression,
    args: &[QueryExpression],
    spec: &QueryFunctionSpec,
) -> Result<(), QueryParseError> {
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
            // Bazel treats an integer token in an expression position as a
            // target literal (for example `1` becomes `//:1`).
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
    }
    Ok(())
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
            validate_function_arguments(expression, args, spec)?;
            for argument in args.iter() {
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
