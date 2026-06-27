/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryExpression {
    TargetPattern(String),
    Function(QueryFunction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryFunction {
    pub name: QueryFunctionName,
    pub args: Vec<QueryArg>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryFunctionName {
    Deps,
    Rdeps,
    Kind,
    Attr,
    Filter,
    Buildfiles,
    Tests,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryArg {
    Expr(Box<QueryExpression>),
    StringLiteral(String),
    Word(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryParseError {
    EmptyExpression,
    ExpectedExpression { offset: usize },
    ExpectedCommaOrCloseParen { offset: usize },
    ExpectedCloseParen { function: String, offset: usize },
    UnsupportedFunction { name: String, offset: usize },
    UnterminatedString { offset: usize },
    TrailingInput { offset: usize },
}

pub fn parse_query_expression(source: &str) -> Result<QueryExpression, QueryParseError> {
    Parser::new(source).parse()
}

impl QueryExpression {
    pub fn parse(source: &str) -> Result<Self, QueryParseError> {
        parse_query_expression(source)
    }
}

impl QueryFunctionName {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "deps" => Some(Self::Deps),
            "rdeps" => Some(Self::Rdeps),
            "kind" => Some(Self::Kind),
            "attr" => Some(Self::Attr),
            "filter" => Some(Self::Filter),
            "buildfiles" => Some(Self::Buildfiles),
            "tests" => Some(Self::Tests),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deps => "deps",
            Self::Rdeps => "rdeps",
            Self::Kind => "kind",
            Self::Attr => "attr",
            Self::Filter => "filter",
            Self::Buildfiles => "buildfiles",
            Self::Tests => "tests",
        }
    }
}

impl fmt::Display for QueryFunctionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for QueryExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetPattern(pattern) => f.write_str(pattern),
            Self::Function(function) => write!(f, "{function}"),
        }
    }
}

impl fmt::Display for QueryFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.name)?;
        for (index, arg) in self.args.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{arg}")?;
        }
        f.write_str(")")
    }
}

impl fmt::Display for QueryArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expr(expr) => write!(f, "{expr}"),
            Self::StringLiteral(value) => write!(f, "\"{}\"", escape_string(value)),
            Self::Word(value) => f.write_str(value),
        }
    }
}

impl fmt::Display for QueryParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyExpression => f.write_str("query expression is empty"),
            Self::ExpectedExpression { offset } => {
                write!(f, "expected query expression at byte {offset}")
            }
            Self::ExpectedCommaOrCloseParen { offset } => {
                write!(f, "expected ',' or ')' at byte {offset}")
            }
            Self::ExpectedCloseParen { function, offset } => {
                write!(f, "expected ')' for {function} at byte {offset}")
            }
            Self::UnsupportedFunction { name, offset } => {
                write!(f, "unsupported query function {name:?} at byte {offset}")
            }
            Self::UnterminatedString { offset } => {
                write!(f, "unterminated query string starting at byte {offset}")
            }
            Self::TrailingInput { offset } => write!(f, "trailing input at byte {offset}"),
        }
    }
}

impl std::error::Error for QueryParseError {}

struct Parser<'a> {
    source: &'a str,
    offset: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, offset: 0 }
    }

    fn parse(mut self) -> Result<QueryExpression, QueryParseError> {
        self.skip_ws();
        if self.is_eof() {
            return Err(QueryParseError::EmptyExpression);
        }
        let arg = self.parse_arg(true)?;
        self.skip_ws();
        if !self.is_eof() {
            return Err(QueryParseError::TrailingInput {
                offset: self.offset,
            });
        }
        match arg {
            QueryArg::Expr(expr) => Ok(*expr),
            QueryArg::StringLiteral(_) | QueryArg::Word(_) => {
                Err(QueryParseError::ExpectedExpression { offset: 0 })
            }
        }
    }

    fn parse_arg(&mut self, top_level: bool) -> Result<QueryArg, QueryParseError> {
        self.skip_ws();
        if self.is_eof() || self.peek() == Some(')') || self.peek() == Some(',') {
            return Err(QueryParseError::ExpectedExpression {
                offset: self.offset,
            });
        }
        if let Some(quote @ ('"' | '\'')) = self.peek() {
            return self.parse_string(quote).map(QueryArg::StringLiteral);
        }

        let atom_start = self.offset;
        let atom = self.parse_atom()?;
        self.skip_ws();
        if self.peek() == Some('(') {
            self.offset += 1;
            let Some(function_name) = QueryFunctionName::parse(&atom) else {
                return Err(QueryParseError::UnsupportedFunction {
                    name: atom,
                    offset: atom_start,
                });
            };
            return Ok(QueryArg::Expr(Box::new(QueryExpression::Function(
                self.parse_function(atom_start, atom, function_name)?,
            ))));
        }

        if top_level || looks_like_target_pattern(&atom) {
            Ok(QueryArg::Expr(Box::new(QueryExpression::TargetPattern(
                atom,
            ))))
        } else {
            Ok(QueryArg::Word(atom))
        }
    }

    fn parse_function(
        &mut self,
        function_offset: usize,
        raw_name: String,
        name: QueryFunctionName,
    ) -> Result<QueryFunction, QueryParseError> {
        let mut args = Vec::new();
        self.skip_ws();
        if self.peek() == Some(')') {
            self.offset += 1;
            return Ok(QueryFunction { name, args });
        }

        loop {
            args.push(self.parse_arg(false)?);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.offset += 1;
                    self.skip_ws();
                }
                Some(')') => {
                    self.offset += 1;
                    return Ok(QueryFunction { name, args });
                }
                None => {
                    return Err(QueryParseError::ExpectedCloseParen {
                        function: raw_name,
                        offset: function_offset,
                    });
                }
                _ => {
                    return Err(QueryParseError::ExpectedCommaOrCloseParen {
                        offset: self.offset,
                    });
                }
            }
        }
    }

    fn parse_string(&mut self, quote: char) -> Result<String, QueryParseError> {
        let start = self.offset;
        self.offset += quote.len_utf8();
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            self.offset += ch.len_utf8();
            if ch == quote {
                return Ok(value);
            }
            if ch == '\\' {
                let Some(escaped) = self.peek() else {
                    return Err(QueryParseError::UnterminatedString { offset: start });
                };
                self.offset += escaped.len_utf8();
                value.push(match escaped {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    other => other,
                });
            } else {
                value.push(ch);
            }
        }
        Err(QueryParseError::UnterminatedString { offset: start })
    }

    fn parse_atom(&mut self) -> Result<String, QueryParseError> {
        let start = self.offset;
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() || ch == ',' || ch == ')' || ch == '(' {
                break;
            }
            self.offset += ch.len_utf8();
        }
        if self.offset == start {
            Err(QueryParseError::ExpectedExpression { offset: start })
        } else {
            Ok(self.source[start..self.offset].to_owned())
        }
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if !ch.is_whitespace() {
                break;
            }
            self.offset += ch.len_utf8();
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn is_eof(&self) -> bool {
        self.offset >= self.source.len()
    }
}

fn looks_like_target_pattern(atom: &str) -> bool {
    atom.starts_with("//")
        || atom.starts_with('@')
        || atom.starts_with(':')
        || atom.contains(':')
        || atom.ends_with("/...")
        || atom.ends_with(":all")
}

fn escape_string(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch => escaped.push(ch),
        }
    }
    escaped
}
