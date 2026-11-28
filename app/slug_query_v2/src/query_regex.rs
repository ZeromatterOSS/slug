/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Resource and syntax policy for the admitted Rust-native query regex dialect.

use regex::Regex;
use regex::RegexBuilder;
use regex_automata::MatchKind;
use regex_automata::meta::Regex as MetaRegex;
use regex_automata::util::syntax;
use regex_syntax::hir::Hir;
use regex_syntax::hir::Look;

pub(crate) const SLUG_REGEX_PATTERN_LIMIT: usize = 4_096;
const SLUG_REGEX_PROGRAM_LIMIT: usize = 1_048_576;
pub(crate) const SLUG_REGEX_NEST_LIMIT: u32 = 128;

const PATTERN_LIMIT: &str = "Slug regex resource limit exceeded: pattern is longer than 4096 bytes";
const PROGRAM_LIMIT: &str =
    "Slug regex resource limit exceeded: compiled program is larger than 1048576 bytes";
const INVALID_SYNTAX: &str = "invalid Slug regex: unsupported or malformed syntax";

pub(crate) fn compile_search_regex(pattern: &str) -> Result<Regex, &'static str> {
    check_pattern_length(pattern)?;

    let mut builder = RegexBuilder::new(pattern);
    builder
        .size_limit(SLUG_REGEX_PROGRAM_LIMIT)
        .dfa_size_limit(SLUG_REGEX_PROGRAM_LIMIT)
        .nest_limit(SLUG_REGEX_NEST_LIMIT)
        .case_insensitive(false)
        .multi_line(false)
        .dot_matches_new_line(false)
        .crlf(false)
        .line_terminator(b'\n')
        .swap_greed(false)
        .ignore_whitespace(false)
        .unicode(true)
        .octal(false);
    builder.build().map_err(|error| match error {
        regex::Error::CompiledTooBig(_) => PROGRAM_LIMIT,
        _ => INVALID_SYNTAX,
    })
}

pub(crate) fn compile_full_regex(pattern: &str) -> Result<MetaRegex, &'static str> {
    check_pattern_length(pattern)?;

    // Parse the original text with the same string-regex defaults as above.
    // Adding anchors after parsing keeps inline flags and EOF comments local
    // to the operand and leaves the original pattern and nesting budgets exact.
    let syntax = syntax::Config::new()
        .nest_limit(SLUG_REGEX_NEST_LIMIT)
        .utf8(true)
        .line_terminator(b'\n');
    let body = syntax::parse_with(pattern, &syntax).map_err(|_| INVALID_SYNTAX)?;
    let full = Hir::concat(vec![Hir::look(Look::Start), body, Hir::look(Look::End)]);
    MetaRegex::builder()
        .configure(
            MetaRegex::config()
                .match_kind(MatchKind::LeftmostFirst)
                .utf8_empty(true)
                .line_terminator(b'\n')
                .nfa_size_limit(Some(SLUG_REGEX_PROGRAM_LIMIT))
                .hybrid_cache_capacity(SLUG_REGEX_PROGRAM_LIMIT),
        )
        .build_from_hir(&full)
        .map_err(|error| {
            if error.size_limit().is_some() {
                PROGRAM_LIMIT
            } else {
                INVALID_SYNTAX
            }
        })
}

pub(crate) fn check_pattern_length(pattern: &str) -> Result<(), &'static str> {
    if pattern.len() > SLUG_REGEX_PATTERN_LIMIT {
        Err(PATTERN_LIMIT)
    } else {
        Ok(())
    }
}
