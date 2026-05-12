/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible `.bazelignore` parsing.
//!
//! `.bazelignore` lists ignored paths one per line, project-relative. `#`
//! introduces a comment to end-of-line; blank lines are skipped. The slug
//! ignore engine consumes a comma-separated spec, so we translate.

/// Convert a `.bazelignore` file content to the comma-separated ignore-spec
/// string consumed by `IgnoreSet::from_ignore_spec`.
pub fn parse_bazelignore(content: &str) -> String {
    content
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple() {
        assert_eq!(parse_bazelignore("foo\nbar\n"), "foo,bar");
    }

    #[test]
    fn skips_comments_and_blanks() {
        let input = "# header\nfoo\n\n# block\nbar  # trailing\n";
        assert_eq!(parse_bazelignore(input), "foo,bar");
    }

    #[test]
    fn empty_file_yields_empty_spec() {
        assert_eq!(parse_bazelignore(""), "");
        assert_eq!(parse_bazelignore("# only comment\n\n"), "");
    }
}
