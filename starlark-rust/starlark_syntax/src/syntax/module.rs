/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::collections::HashMap;
use std::fmt::Write;
use std::fs;
use std::mem;
use std::path::Path;

use derivative::Derivative;
use dupe::Dupe;
use lalrpop_util as lu;

use crate::codemap::CodeMap;
use crate::codemap::FileSpan;
use crate::codemap::Pos;
use crate::codemap::Span;
use crate::codemap::Spanned;
use crate::dialect::StringEncoding;
use crate::eval_exception::EvalException;
use crate::lexer::Lexer;
use crate::lexer::LexerStringEncoding;
use crate::lexer::Token;
use crate::syntax::AstLoad;
use crate::syntax::Dialect;
use crate::syntax::DialectTypes;
use crate::syntax::ast::ArgumentP;
use crate::syntax::ast::AstExpr;
use crate::syntax::ast::AstStmt;
use crate::syntax::ast::CallArgsP;
use crate::syntax::ast::ExprP;
use crate::syntax::ast::IdentP;
use crate::syntax::ast::LoadArgP;
use crate::syntax::ast::Stmt;
use crate::syntax::grammar::StarlarkParser;
use crate::syntax::lint_suppressions::LintSuppressions;
use crate::syntax::lint_suppressions::LintSuppressionsBuilder;
use crate::syntax::state::ParserState;
use crate::syntax::validate::validate_module;

// The grammar can build deep trees without nested delimiters. Keep a ceiling on
// the entire stream that can reach recursive semantic actions, visitors and Drop.
const MAX_BOUNDED_GRAMMAR_TOKENS: usize = 256;
const MAX_BOUNDED_SOURCE_BYTES: usize = 64 * 1024;
const MAX_BOUNDED_INTEGER_DIGITS: usize = 128;
const MAX_BOUNDED_FILENAME_BYTES: usize = 4 * 1024;

/// Limits for parsing an untyped Starlark module without f-strings.
#[derive(Clone, Copy, Debug)]
pub struct ParseLimits {
    source_bytes: usize,
    grammar_tokens: usize,
    integer_digits: usize,
}

impl ParseLimits {
    /// Set limits for source bytes, grammar tokens, and digits per integer.
    ///
    /// Values are capped at 64 KiB, 256 grammar tokens, and 128 digits.
    /// Grammar tokens include layout and EOF newline tokens, but exclude comments.
    pub const fn new(source_bytes: usize, grammar_tokens: usize, integer_digits: usize) -> Self {
        Self {
            source_bytes,
            grammar_tokens,
            integer_digits,
        }
    }
}

/// A resource refusal during opt-in bounded parsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseLimit {
    /// The source exceeded the pre-lexer byte limit.
    #[error("Starlark parser source byte limit exceeded")]
    SourceBytes,
    /// The diagnostic filename exceeded the pre-lexer byte limit.
    #[error("Starlark parser filename byte limit exceeded")]
    FilenameBytes,
    /// A token exceeded the admitted grammar stream.
    #[error("Starlark parser grammar token limit exceeded")]
    GrammarTokens,
    /// An integer literal exceeded the limit before radix conversion.
    #[error("Starlark parser integer digit limit exceeded")]
    IntegerDigits,
}

fn one_of(expected: &[String]) -> String {
    let mut result = String::new();
    for (i, e) in expected.iter().enumerate() {
        let sep = match i {
            0 => "one of",
            _ if i < expected.len() - 1 => ",",
            // Last expected message to be written
            _ => " or",
        };
        write!(result, "{sep} {e}").unwrap();
    }
    result
}

/// Convert the error to a codemap diagnostic.
///
/// To build this diagnostic, the method needs the file span corresponding
/// to the parsed file.
fn parse_error_add_span(
    err: lu::ParseError<usize, Token, EvalException>,
    pos: usize,
    codemap: &CodeMap,
) -> crate::Error {
    let (message, span) = match err {
        lu::ParseError::InvalidToken { location } => (
            "Parse error: invalid token".to_owned(),
            Span::new(Pos::new(location as u32), Pos::new(location as u32)),
        ),
        lu::ParseError::UnrecognizedToken {
            token: (x, t, y),
            expected,
        } => (
            format!(
                "Parse error: unexpected {} here, expected {}",
                t,
                one_of(&expected)
            ),
            Span::new(Pos::new(x as u32), Pos::new(y as u32)),
        ),
        lu::ParseError::UnrecognizedEOF { .. } => (
            "Parse error: unexpected end of file".to_owned(),
            Span::new(Pos::new(pos as u32), Pos::new(pos as u32)),
        ),
        lu::ParseError::ExtraToken { token: (x, t, y) } => (
            format!("Parse error: extraneous token {t}"),
            Span::new(Pos::new(x as u32), Pos::new(y as u32)),
        ),
        lu::ParseError::User { error } => return error.into_error(),
    };

    crate::Error::new_spanned(
        crate::ErrorKind::Parser(anyhow::anyhow!(message)),
        span,
        codemap,
    )
}

/// A representation of a Starlark module abstract syntax tree.
///
/// Created with either [`parse`](AstModule::parse) or [`parse_file`](AstModule::parse_file),
/// and evaluated with `Evaluator::eval_module`.
///
/// The internal details (statements/expressions) are deliberately omitted, as they change
/// more regularly. A few methods to obtain information about the AST are provided.
#[derive(Derivative)]
#[derivative(Debug, Clone)]
pub struct AstModule {
    #[derivative(Debug = "ignore")]
    pub(crate) codemap: CodeMap,
    pub(crate) statement: AstStmt,
    pub(crate) dialect: Dialect,
    /// Opt-in typecheck.
    /// Specified with `@starlark-rust: typecheck`.
    pub(crate) typecheck: bool,
    /// Lint issues suppressed in this module using inline comments of shape
    /// # starlark-lint-disable <ISSUE_NAME>, <ISSUE_NAME>, ...
    lint_suppressions: LintSuppressions,
}

/// This trait is not exported as public API of starlark.
pub trait AstModuleFields: Sized {
    fn codemap(&self) -> &CodeMap;

    fn statement(&self) -> &AstStmt;

    fn dialect(&self) -> &Dialect;

    fn into_parts(self) -> (CodeMap, AstStmt, Dialect, bool);
}

impl AstModuleFields for AstModule {
    fn codemap(&self) -> &CodeMap {
        &self.codemap
    }

    fn statement(&self) -> &AstStmt {
        &self.statement
    }

    fn dialect(&self) -> &Dialect {
        &self.dialect
    }

    fn into_parts(self) -> (CodeMap, AstStmt, Dialect, bool) {
        (self.codemap, self.statement, self.dialect, self.typecheck)
    }
}

impl AstModule {
    fn create(
        codemap: CodeMap,
        statement: AstStmt,
        dialect: &Dialect,
        typecheck: bool,
        lint_suppressions: LintSuppressions,
    ) -> crate::Result<AstModule> {
        let mut errors = Vec::new();
        validate_module(
            &statement,
            &mut ParserState {
                codemap: &codemap,
                dialect,
                errors: &mut errors,
            },
        );
        // We need the first error, so we don't use `.pop()`.
        if let Some(err) = errors.into_iter().next() {
            return Err(err.into_error());
        }
        Ok(AstModule {
            codemap,
            statement,
            dialect: dialect.clone(),
            typecheck,
            lint_suppressions,
        })
    }

    /// Parse a file stored on disk. For details see [`parse`](AstModule::parse).
    pub fn parse_file(path: &Path, dialect: &Dialect) -> crate::Result<Self> {
        let content = fs::read_to_string(path).map_err(anyhow::Error::new)?;
        Self::parse(&path.to_string_lossy(), content, dialect)
    }

    /// Parse a Starlark module to produce an [`AstModule`], or an error if there are syntax errors.
    /// The `filename` is for error messages only, and does not have to be a valid file.
    /// The [`Dialect`] selects which Starlark constructs are valid.
    ///
    /// The returned error may contain diagnostic information. For example:
    ///
    /// ```
    /// use starlark_syntax::codemap::FileSpan;
    /// use starlark_syntax::syntax::AstModule;
    /// use starlark_syntax::syntax::Dialect;
    ///
    /// let err: starlark_syntax::Error =
    ///     AstModule::parse("filename", "\n(unmatched".to_owned(), &Dialect::Standard).unwrap_err();
    /// let span: &FileSpan = err.span().unwrap();
    /// assert_eq!(span.to_string(), "filename:2:11");
    /// ```
    pub fn parse(filename: &str, content: String, dialect: &Dialect) -> crate::Result<Self> {
        Self::parse_unicode(filename, content, dialect, None)
    }

    /// Parse using resource limits before grammar reduction and validation.
    ///
    /// The filename is limited to 4 KiB. This entry point requires an untyped
    /// dialect with f-strings disabled and uses the Unicode source encoding.
    pub fn parse_with_limits(
        filename: &str,
        content: String,
        dialect: &Dialect,
        limits: ParseLimits,
    ) -> crate::Result<Self> {
        if filename.len() > MAX_BOUNDED_FILENAME_BYTES {
            return Err(crate::Error::new_other(ParseLimit::FilenameBytes));
        }
        if content.len() > limits.source_bytes.min(MAX_BOUNDED_SOURCE_BYTES) {
            return Err(crate::Error::new_other(ParseLimit::SourceBytes));
        }
        if dialect.enable_types != DialectTypes::Disable || dialect.enable_f_strings {
            return Err(crate::Error::new_other(anyhow::anyhow!(
                "Bounded Starlark parsing requires an untyped dialect without f-strings"
            )));
        }
        Self::parse_unicode(filename, content, dialect, Some(limits))
    }

    fn parse_unicode(
        filename: &str,
        content: String,
        dialect: &Dialect,
        limits: Option<ParseLimits>,
    ) -> crate::Result<Self> {
        let typecheck = content.contains("@starlark-rust: typecheck");
        let codemap = CodeMap::new(filename.to_owned(), content);
        let mut lexer = Lexer::new(codemap.source(), dialect, codemap.dupe());
        if let Some(limits) = limits {
            lexer.limit_integer_digits(limits.integer_digits.min(MAX_BOUNDED_INTEGER_DIGITS));
        }
        let mut remaining =
            limits.map(|limits| limits.grammar_tokens.min(MAX_BOUNDED_GRAMMAR_TOKENS));
        // Store lint suppressions found during parsing
        let mut lint_suppressions_builder = LintSuppressionsBuilder::new();
        // Keep track of block of comments, used for accumulating lint suppressions
        let mut in_comment_block = false;
        let mut errors = Vec::new();
        match StarlarkParser::new().parse(
            &mut ParserState {
                codemap: &codemap,
                dialect,
                errors: &mut errors,
            },
            lexer.filter_map(|token| match token {
                // Filter out comment tokens and accumulate lint suppressions
                Ok((start, Token::Comment(comment), end)) => {
                    lint_suppressions_builder.parse_comment(&codemap, &comment, start, end);
                    in_comment_block = true;
                    None
                }
                token => {
                    if in_comment_block {
                        lint_suppressions_builder.end_of_comment_block(&codemap);
                        in_comment_block = false;
                    }
                    if let (Some(remaining), Ok((start, token, end))) = (&mut remaining, &token) {
                        let span = Span::new(Pos::new(*start as u32), Pos::new(*end as u32));
                        if *remaining == 0 {
                            return Some(Err(EvalException::new_anyhow(
                                ParseLimit::GrammarTokens.into(),
                                span,
                                &codemap,
                            )));
                        }
                        *remaining -= 1;
                        if matches!(token, Token::FString(_)) {
                            return Some(Err(EvalException::parser_error(
                                "Your Starlark dialect must enable f-strings to use them",
                                span,
                                &codemap,
                            )));
                        }
                    }
                    Some(token)
                }
            }),
        ) {
            Ok(v) => {
                if let Some(err) = errors.into_iter().next() {
                    return Err(err.into_error());
                }
                Ok(AstModule::create(
                    codemap,
                    v,
                    dialect,
                    typecheck,
                    lint_suppressions_builder.build(),
                )?)
            }
            Err(p) => Err(parse_error_add_span(p, codemap.source().len(), &codemap)),
        }
    }

    /// Parse a module with an explicit representation for ordinary string literals.
    ///
    /// [`StringEncoding::BazelInternal`] is a dormant compatibility seam for callers
    /// that need Bazel's byte carrier. The default [`parse`](AstModule::parse) behavior
    /// remains Unicode.
    pub fn parse_with_string_encoding(
        filename: &str,
        content: String,
        dialect: &Dialect,
        string_encoding: StringEncoding,
    ) -> crate::Result<Self> {
        let typecheck = content.contains("@starlark-rust: typecheck");
        let (codemap, lexer_string_encoding) = match string_encoding {
            StringEncoding::Unicode => (
                CodeMap::new(filename.to_owned(), content),
                LexerStringEncoding::Unicode,
            ),
            StringEncoding::BazelInternal => (
                CodeMap::new_with_byte_column_reporting(filename.to_owned(), content),
                LexerStringEncoding::BazelInternal,
            ),
        };
        let lexer = Lexer::new_with_string_encoding(
            codemap.source(),
            dialect,
            codemap.dupe(),
            lexer_string_encoding,
        );
        let mut lint_suppressions_builder = LintSuppressionsBuilder::new();
        let mut in_comment_block = false;
        let mut errors = Vec::new();
        match StarlarkParser::new().parse(
            &mut ParserState {
                codemap: &codemap,
                dialect,
                errors: &mut errors,
            },
            lexer.filter(|token| match token {
                Ok((start, Token::Comment(comment), end)) => {
                    lint_suppressions_builder.parse_comment(&codemap, comment, *start, *end);
                    in_comment_block = true;
                    false
                }
                _ => {
                    if in_comment_block {
                        lint_suppressions_builder.end_of_comment_block(&codemap);
                        in_comment_block = false;
                    }
                    true
                }
            }),
        ) {
            Ok(v) => {
                if let Some(err) = errors.into_iter().next() {
                    return Err(err.into_error());
                }
                Ok(AstModule::create(
                    codemap,
                    v,
                    dialect,
                    typecheck,
                    lint_suppressions_builder.build(),
                )?)
            }
            Err(p) => Err(parse_error_add_span(p, codemap.source().len(), &codemap)),
        }
    }

    /// Return the file names of all the `load` statements in the module.
    /// If the [`Dialect`] had [`enable_load`](Dialect::enable_load) set to [`false`] this will be an empty list.
    pub fn loads(&self) -> Vec<AstLoad<'_>> {
        // We know that `load` statements must be at the top-level, so no need to descend inside `if`, `for`, `def` etc.
        // There is a suggestion that `load` statements should be at the top of a file, but we tolerate that not being true.
        fn f<'a>(ast: &'a AstStmt, codemap: &CodeMap, vec: &mut Vec<AstLoad<'a>>) {
            match &ast.node {
                Stmt::Load(load) => vec.push(AstLoad {
                    span: FileSpan {
                        file: codemap.dupe(),
                        span: load.module.span,
                    },
                    module_id: &load.module.node,
                    symbols: load
                        .args
                        .iter()
                        .map(|LoadArgP { local, their, .. }| {
                            (local.node.ident.as_str(), their.node.as_str())
                        })
                        .collect(),
                }),
                Stmt::Statements(stmts) => {
                    for s in stmts {
                        f(s, codemap, vec);
                    }
                }
                _ => {}
            }
        }

        let mut loads = Vec::new();
        f(&self.statement, &self.codemap, &mut loads);
        loads
    }

    /// Look up a [`Span`] contained in this module to a [`FileSpan`].
    pub fn file_span(&self, x: Span) -> FileSpan {
        self.codemap.file_span(x)
    }

    /// Get back the AST statement for the module
    pub fn statement(&self) -> &AstStmt {
        &self.statement
    }

    /// Locations where statements occur.
    pub fn stmt_locations(&self) -> Vec<FileSpan> {
        fn go(x: &AstStmt, codemap: &CodeMap, res: &mut Vec<FileSpan>) {
            match &**x {
                Stmt::Statements(_) => {} // These are not interesting statements that come up
                _ => res.push(FileSpan {
                    span: x.span,
                    file: codemap.dupe(),
                }),
            }
            x.visit_stmt(|x| go(x, codemap, res))
        }

        let mut res = Vec::new();
        go(&self.statement, &self.codemap, &mut res);
        res
    }

    /// Function to help people who want to write deeper AST transformations in Starlark.
    /// Likely to break type checking and LSP support to some extent.
    ///
    /// Replacement must be a map from operator name (e.g. `+` or `==`) to a function name
    /// (e.g. `my_plus` or `my_equals`).
    pub fn replace_binary_operators(&mut self, replace: &HashMap<String, String>) {
        fn f(x: &mut AstExpr, replace: &HashMap<String, String>) {
            let mut temp = ExprP::Tuple(vec![]);
            mem::swap(&mut x.node, &mut temp);
            let mut res = match temp {
                ExprP::Op(lhs, op, rhs) => match replace.get(op.to_string().trim()) {
                    Some(func) => ExprP::Call(
                        Box::new(Spanned {
                            span: x.span,
                            node: ExprP::Identifier(Spanned {
                                span: x.span,
                                node: IdentP {
                                    ident: func.clone(),
                                    payload: (),
                                },
                            }),
                        }),
                        CallArgsP {
                            lparen: x.span.end_span(),
                            args: vec![
                                Spanned {
                                    span: lhs.span,
                                    node: ArgumentP::Positional(*lhs),
                                },
                                Spanned {
                                    span: rhs.span,
                                    node: ArgumentP::Positional(*rhs),
                                },
                            ],
                        },
                    ),
                    None => ExprP::Op(lhs, op, rhs),
                },
                _ => temp,
            };
            mem::swap(&mut x.node, &mut res);
            x.visit_expr_mut(|x| f(x, replace));
        }

        self.statement.visit_expr_mut(|x| f(x, replace));
    }

    /// Check if a given Lint short_name and span is suppressed in this module
    pub fn is_suppressed(&self, issue_short_name: &str, issue_span: Span) -> bool {
        self.lint_suppressions
            .is_suppressed(issue_short_name, issue_span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slice_vec_ext::SliceExt;
    use crate::syntax::grammar_tests;

    fn assert_parse_limit(result: crate::Result<AstModule>, expected: ParseLimit) {
        let error = result.unwrap_err();
        match error.kind() {
            crate::ErrorKind::Other(error) => {
                assert_eq!(error.downcast_ref::<ParseLimit>(), Some(&expected));
            }
            _ => panic!("expected {expected:?}, got {error}"),
        }
    }

    #[test]
    fn bounded_source_and_grammar_tokens_are_exact() {
        let source = "# a comment\n1";
        AstModule::parse_with_limits(
            "format.star",
            source.to_owned(),
            &Dialect::Standard,
            ParseLimits::new(source.len(), 2, 1),
        )
        .unwrap();
        assert_parse_limit(
            AstModule::parse_with_limits(
                "format.star",
                source.to_owned(),
                &Dialect::Standard,
                ParseLimits::new(source.len() - 1, 2, 1),
            ),
            ParseLimit::SourceBytes,
        );
        assert_parse_limit(
            AstModule::parse_with_limits(
                "format.star",
                source.to_owned(),
                &Dialect::Standard,
                ParseLimits::new(source.len(), 1, 1),
            ),
            ParseLimit::GrammarTokens,
        );
    }

    #[test]
    fn bounded_integer_admission_precedes_radix_conversion() {
        for source in ["9999", "0xaaaa", "0o7777", "0b1111"] {
            AstModule::parse_with_limits(
                "format.star",
                source.to_owned(),
                &Dialect::Standard,
                ParseLimits::new(32, 8, 4),
            )
            .unwrap();
            assert_parse_limit(
                AstModule::parse_with_limits(
                    "format.star",
                    source.to_owned(),
                    &Dialect::Standard,
                    ParseLimits::new(32, 8, 3),
                ),
                ParseLimit::IntegerDigits,
            );
        }
        let source = "9".repeat(MAX_BOUNDED_INTEGER_DIGITS + 1);
        AstModule::parse("ordinary.star", source.clone(), &Dialect::Standard).unwrap();
        assert_parse_limit(
            AstModule::parse_with_limits(
                "format.star",
                source,
                &Dialect::Standard,
                ParseLimits::new(256, 8, usize::MAX),
            ),
            ParseLimit::IntegerDigits,
        );
    }

    #[test]
    fn bounded_parser_handles_near_limit_escaped_newline_trivia() {
        use crate::lexer::TokenInt;

        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                let trivia = " \\\n  \\\r\n";
                let repetitions = (MAX_BOUNDED_SOURCE_BYTES - "1+2".len()) / trivia.len();
                let source = format!("1+{}2", trivia.repeat(repetitions));
                let source_len = source.len();
                assert!(MAX_BOUNDED_SOURCE_BYTES - source_len < trivia.len());

                let codemap = CodeMap::new("format.star".to_owned(), source.clone());
                let mut lexer = Lexer::new(&source, &Dialect::Standard, codemap);
                assert_eq!(
                    lexer.next().unwrap().unwrap(),
                    (0, Token::Int(TokenInt::I32(1)), 1)
                );
                assert_eq!(lexer.next().unwrap().unwrap(), (1, Token::Plus, 2));
                assert_eq!(
                    lexer.next().unwrap().unwrap(),
                    (source_len - 1, Token::Int(TokenInt::I32(2)), source_len)
                );
                assert_eq!(
                    lexer.next().unwrap().unwrap(),
                    (source_len, Token::Newline, source_len)
                );
                assert!(lexer.next().is_none());

                AstModule::parse_with_limits(
                    "format.star",
                    source,
                    &Dialect::Standard,
                    ParseLimits::new(source_len, 4, 1),
                )
                .unwrap();
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn bounded_grammar_limits_deep_tree_shapes_before_validation() {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                let cap = MAX_BOUNDED_GRAMMAR_TOKENS;
                let limits = ParseLimits::new(4096, usize::MAX, 4);
                for source in [
                    format!("{}True", "not ".repeat(cap - 2)),
                    format!("{}1", "1+".repeat((cap - 2) / 2)),
                    format!("x{}", ".x".repeat((cap - 2) / 2)),
                ] {
                    AstModule::parse_with_limits("format.star", source, &Dialect::Standard, limits)
                        .unwrap();
                }
                assert_parse_limit(
                    AstModule::parse_with_limits(
                        "format.star",
                        format!("{}True", "not ".repeat(cap - 1)),
                        &Dialect::Standard,
                        limits,
                    ),
                    ParseLimit::GrammarTokens,
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn bounded_parser_requires_an_untyped_dialect_without_f_strings() {
        let dialect = Dialect {
            enable_f_strings: true,
            ..Dialect::Standard
        };
        let limits = ParseLimits::new(64, 16, 4);
        assert!(
            AstModule::parse_with_limits("format.star", "1".to_owned(), &dialect, limits)
                .unwrap_err()
                .to_string()
                .contains("requires an untyped dialect without f-strings")
        );
        assert!(
            AstModule::parse_with_limits(
                "format.star",
                "f\"{identifier}\"".to_owned(),
                &Dialect::Standard,
                limits,
            )
            .unwrap_err()
            .to_string()
            .contains("must enable f-strings")
        );
    }

    #[test]
    fn test_locations() {
        fn get(code: &str) -> String {
            grammar_tests::parse_ast(code)
                .stmt_locations()
                .map(|x| x.resolve_span().to_string())
                .join(" ")
        }

        assert_eq!(&get("foo"), "1:1-4");
        assert_eq!(&get("foo\ndef x():\n   pass"), "1:1-4 2:1-3:8 3:4-8");
    }
}
