/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Borrowed-span nom parser adapted from Buck2
//! `app/buck2_query_parser/src/{lib,span,spanned}.rs`.
//!
//! Local deltas are Bazel's `let NAME = EXPR in EXPR`, Bazel diagnostic
//! wording, and lowering fragments into compact owned runtime values.

use std::ops::Deref;
use std::str::CharIndices;
use std::str::Chars;

use compact_str::CompactString;
use nom::Compare;
use nom::CompareResult;
use nom::IResult;
use nom::Input;
use nom::Needed;
use nom::Offset;
use nom::Parser as _;
use nom::branch::alt;
use nom::bytes::complete::is_a;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_till;
use nom::character::complete::alpha1;
use nom::character::complete::alphanumeric1;
use nom::character::complete::char;
use nom::character::complete::digit1;
use nom::character::complete::multispace0;
use nom::character::complete::multispace1;
use nom::combinator::all_consuming;
use nom::combinator::cut;
use nom::combinator::recognize;
use nom::multi::many0;
use nom::multi::many1;
use nom::multi::separated_list0;
use nom::sequence::delimited;
use nom::sequence::pair;
use nom::sequence::preceded;
use nom::sequence::terminated;

use crate::BinaryOperator;
use crate::QueryExpression;
use crate::QueryExpressionKind;
use crate::QueryParseError;
use crate::SourceSpan;
use crate::Spanned;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Span<'a> {
    offset: usize,
    fragment: &'a str,
}

impl<'a> Span<'a> {
    fn new(fragment: &'a str) -> Self {
        Self {
            offset: 0,
            fragment,
        }
    }

    fn fragment(self) -> &'a str {
        self.fragment
    }
}

impl Deref for Span<'_> {
    type Target = str;
    fn deref(&self) -> &str {
        self.fragment
    }
}

impl<'a> Input for Span<'a> {
    type Item = char;
    type Iter = Chars<'a>;
    type IterIndices = CharIndices<'a>;
    fn input_len(&self) -> usize {
        self.fragment.len()
    }
    fn take(&self, index: usize) -> Self {
        Self {
            offset: self.offset,
            fragment: &self.fragment[..index],
        }
    }
    fn take_from(&self, index: usize) -> Self {
        Self {
            offset: self.offset + index,
            fragment: &self.fragment[index..],
        }
    }
    fn take_split(&self, index: usize) -> (Self, Self) {
        (self.take_from(index), self.take(index))
    }
    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        self.fragment.find(predicate)
    }
    fn iter_elements(&self) -> Self::Iter {
        self.fragment.chars()
    }
    fn iter_indices(&self) -> Self::IterIndices {
        self.fragment.char_indices()
    }
    fn slice_index(&self, count: usize) -> Result<usize, Needed> {
        Input::slice_index(&self.fragment, count)
    }
}

impl Offset for Span<'_> {
    fn offset(&self, second: &Self) -> usize {
        second.offset - self.offset
    }
}

impl<'a> Compare<&'a str> for Span<'a> {
    fn compare(&self, value: &'a str) -> CompareResult {
        self.fragment.compare(value)
    }
    fn compare_no_case(&self, value: &'a str) -> CompareResult {
        self.fragment.compare_no_case(value)
    }
}

type Parsed<'a, T> = IResult<Span<'a>, T, ()>;

fn spanned<'a, T>(
    mut parser: impl FnMut(Span<'a>) -> Parsed<'a, T>,
) -> impl FnMut(Span<'a>) -> Parsed<'a, Spanned<T>> {
    move |original| {
        let start = original.offset;
        let (remaining, value) = parser(original)?;
        Ok((
            remaining,
            Spanned {
                span: SourceSpan {
                    start,
                    end: remaining.offset,
                },
                value,
            },
        ))
    }
}

pub(crate) fn parse(source: &str) -> Result<QueryExpression, QueryParseError> {
    match all_consuming(expression).parse(Span::new(source)) {
        Ok((_, expression)) => Ok(expression),
        Err(_) => {
            let end = SourceSpan {
                start: source.len(),
                end: source.len(),
            };
            let premature = source.trim_end().ends_with('(')
                || source.matches('(').count() > source.matches(')').count()
                || source.trim_end().ends_with(',')
                || source.trim_end().ends_with('=');
            let message = if premature {
                CompactString::new("premature end of input")
            } else if let Some(tokens) = unquoted_bare_negative_tokens(source) {
                CompactString::from(format!("syntax error at '{tokens}'"))
            } else {
                CompactString::new("syntax error in query expression")
            };
            Err(QueryParseError::new(message, end))
        }
    }
}

fn unquoted_bare_negative_tokens(source: &str) -> Option<String> {
    fn next_char(source: &str, index: usize) -> Option<(char, usize)> {
        source[index..]
            .chars()
            .next()
            .map(|ch| (ch, index + ch.len_utf8()))
    }

    fn scan_word(source: &str, start: usize) -> usize {
        let (first, mut index) = next_char(source, start).expect("word starts before EOF");
        let starts_with_double_at =
            first == '@' && next_char(source, index).is_some_and(|(ch, _)| ch == '@');
        while let Some((ch, next)) = next_char(source, index) {
            let continues = ch.is_ascii_alphanumeric()
                || "*/@.-_:$~[]".contains(ch)
                || !ch.is_ascii()
                || (ch == '+' && starts_with_double_at);
            if !continues {
                break;
            }
            index = next;
        }
        index
    }

    fn scan_token(source: &str, mut index: usize) -> Option<(String, usize)> {
        while let Some((ch, next)) = next_char(source, index) {
            if !ch.is_ascii_whitespace() {
                break;
            }
            index = next;
        }
        let (first, next) = next_char(source, index)?;
        if "(),+-=^".contains(first) {
            return Some((first.to_string(), next));
        }
        if first == '\'' || first == '"' {
            let value_start = next;
            let mut end = next;
            while let Some((ch, following)) = next_char(source, end) {
                if ch == first {
                    return Some((source[value_start..end].to_owned(), following));
                }
                end = following;
            }
            return Some((source[value_start..].to_owned(), source.len()));
        }
        let end = scan_word(source, index);
        Some((source[index..end].to_owned(), end))
    }

    let mut quote = None;
    let mut index = 0;
    while let Some((ch, next)) = next_char(source, index) {
        match ch {
            '\'' | '"' => {
                quote = if quote == Some(ch) {
                    None
                } else if quote.is_none() {
                    Some(ch)
                } else {
                    quote
                };
                index = next;
            }
            '(' | ',' if quote.is_none() => {
                index = next;
                while let Some((ch, following)) = next_char(source, index) {
                    if !ch.is_ascii_whitespace() {
                        break;
                    }
                    index = following;
                }
                let Some(('-', after_minus)) = next_char(source, index) else {
                    continue;
                };
                if !next_char(source, after_minus).is_some_and(|(ch, _)| ch.is_ascii_digit()) {
                    continue;
                }
                let mut tokens = Vec::with_capacity(3);
                let mut cursor = index;
                while tokens.len() < 3 {
                    let Some((token, next)) = scan_token(source, cursor) else {
                        break;
                    };
                    tokens.push(token);
                    cursor = next;
                }
                return Some(tokens.join(" "));
            }
            _ => index = next,
        }
    }
    None
}

fn expression(input: Span<'_>) -> Parsed<'_, QueryExpression> {
    delimited(
        multispace0,
        spanned(|input| {
            let (input, left) = single_expression(input)?;
            if let Ok((input, operations)) = trailing_infix(input) {
                Ok((
                    input,
                    QueryExpressionKind::BinaryOpSequence {
                        left: Box::new(left),
                        operations: operations.into(),
                    },
                ))
            } else {
                Ok((input, left.kind))
            }
        })
        .map(|spanned| QueryExpression {
            span: spanned.span,
            kind: spanned.value,
        }),
        multispace0,
    )
    .parse(input)
}

fn single_expression(input: Span<'_>) -> Parsed<'_, QueryExpression> {
    alt((
        preceded(char('('), cut(terminated(expression, char(')')))),
        let_expression,
        set_expression,
        function_expression,
        integer_expression,
        word_expression,
    ))
    .parse(input)
}

fn trailing_infix(input: Span<'_>) -> Parsed<'_, Vec<(BinaryOperator, QueryExpression)>> {
    many1(pair(binary_operator, cut(single_expression))).parse(input)
}

fn binary_operator(input: Span<'_>) -> Parsed<'_, BinaryOperator> {
    fn keyword(
        word: &'static str,
        operator: BinaryOperator,
    ) -> impl FnMut(Span<'_>) -> Parsed<'_, BinaryOperator> {
        move |input| {
            let (input, _) = delimited(multispace1, tag(word), multispace1).parse(input)?;
            Ok((input, operator))
        }
    }
    fn symbol(
        symbol: &'static str,
        operator: BinaryOperator,
    ) -> impl FnMut(Span<'_>) -> Parsed<'_, BinaryOperator> {
        move |input| {
            let (input, _) = delimited(multispace0, tag(symbol), multispace0).parse(input)?;
            Ok((input, operator))
        }
    }
    alt((
        symbol("-", BinaryOperator::Except),
        keyword("except", BinaryOperator::Except),
        symbol("^", BinaryOperator::Intersect),
        keyword("intersect", BinaryOperator::Intersect),
        symbol("+", BinaryOperator::Union),
        keyword("union", BinaryOperator::Union),
    ))
    .parse(input)
}

fn word_expression(input: Span<'_>) -> Parsed<'_, QueryExpression> {
    spanned(word)
        .map(|value| QueryExpression {
            span: value.span,
            kind: QueryExpressionKind::TargetLiteral(CompactString::new(value.value.fragment())),
        })
        .parse(input)
}

fn integer_expression(input: Span<'_>) -> Parsed<'_, QueryExpression> {
    spanned(|input| {
        let (remaining, value) = digit1(input)?;
        if remaining
            .fragment()
            .chars()
            .next()
            .is_some_and(|ch| ch.is_alphanumeric() || "*/@.-_:$#%".contains(ch))
        {
            return Err(nom::Err::Error(()));
        }
        let parsed = value
            .fragment()
            .parse::<u64>()
            .map_err(|_| nom::Err::Failure(()))?;
        Ok((remaining, parsed))
    })
    .map(|value| QueryExpression {
        span: value.span,
        kind: QueryExpressionKind::Integer(value.value),
    })
    .parse(input)
}

fn let_expression(input: Span<'_>) -> Parsed<'_, QueryExpression> {
    spanned(|input| {
        let (input, _) = terminated(tag("let"), multispace1).parse(input)?;
        let (input, name) = spanned(function_name).parse(input)?;
        let (input, _) = delimited(multispace0, char('='), multispace0).parse(input)?;
        let (input, value) = expression(input)?;
        // `expression` consumes trailing whitespace, so only require the
        // keyword and whitespace following it here.
        let (input, _) = terminated(tag("in"), multispace1).parse(input)?;
        let (input, body) = expression(input)?;
        Ok((
            input,
            QueryExpressionKind::Let {
                name: name.map(|span| CompactString::new(span.fragment())),
                value: Box::new(value),
                body: Box::new(body),
            },
        ))
    })
    .map(|value| QueryExpression {
        span: value.span,
        kind: value.value,
    })
    .parse(input)
}

fn set_expression(input: Span<'_>) -> Parsed<'_, QueryExpression> {
    spanned(|input| {
        let (input, _) = tag("set(").parse(input)?;
        let (input, values) = cut(delimited(
            multispace0,
            separated_list0(multispace1, spanned(word)),
            terminated(multispace0, char(')')),
        ))
        .parse(input)?;
        Ok((
            input,
            QueryExpressionKind::Set(
                values
                    .into_iter()
                    .map(|value| value.map(|span| CompactString::new(span.fragment())))
                    .collect::<Vec<_>>()
                    .into(),
            ),
        ))
    })
    .map(|value| QueryExpression {
        span: value.span,
        kind: value.value,
    })
    .parse(input)
}

fn function_expression(input: Span<'_>) -> Parsed<'_, QueryExpression> {
    spanned(|input| {
        let (input, name) = spanned(function_name).parse(input)?;
        let (input, _) = char('(').parse(input)?;
        let (input, args) = cut(terminated(
            separated_list0(terminated(char(','), multispace0), expression),
            char(')'),
        ))
        .parse(input)?;
        Ok((
            input,
            QueryExpressionKind::Function {
                name: name.map(|span| CompactString::new(span.fragment())),
                args: args.into(),
            },
        ))
    })
    .map(|value| QueryExpression {
        span: value.span,
        kind: value.value,
    })
    .parse(input)
}

fn function_name(input: Span<'_>) -> Parsed<'_, Span<'_>> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))
    .parse(input)
}

fn word(input: Span<'_>) -> Parsed<'_, Span<'_>> {
    fn quoted(quote: char) -> impl FnMut(Span<'_>) -> Parsed<'_, Span<'_>> {
        move |input| {
            preceded(
                char(quote),
                cut(terminated(take_till(move |ch| ch == quote), char(quote))),
            )
            .parse(input)
        }
    }
    fn unquoted(input: Span<'_>) -> Parsed<'_, Span<'_>> {
        if input
            .fragment()
            .strip_prefix('-')
            .and_then(|rest| rest.chars().next())
            .is_some_and(|ch| ch.is_ascii_digit())
        {
            return Err(nom::Err::Error(()));
        }
        recognize(many1(alt((alphanumeric1, is_a("*/@.-_:$#%"))))).parse(input)
    }
    alt((quoted('\''), quoted('"'), unquoted)).parse(input)
}
