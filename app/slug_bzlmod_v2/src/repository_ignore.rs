/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the later Host repository-ignore packet.

use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use slug_identity_v2::PackagePath;

/// Compact repository-relative ignore entries.
///
/// Literal prefixes precede patterns during matching. Pattern spellings are
/// retained in request order while their segment matchers are precompiled.
#[derive(Debug, Clone, Allocative, Dupe)]
pub(crate) struct RepositoryIgnoreMatcher {
    literal_prefixes: Arc<[PackagePath]>,
    patterns: Arc<[CompiledPattern]>,
}

impl RepositoryIgnoreMatcher {
    pub(crate) fn new(
        literal_prefixes: impl IntoIterator<Item = PackagePath>,
        patterns: impl IntoIterator<Item = CompactString>,
    ) -> Self {
        let mut literal_prefixes = literal_prefixes.into_iter().collect::<Vec<_>>();
        literal_prefixes.sort();
        literal_prefixes.dedup();
        let patterns = patterns
            .into_iter()
            .map(CompiledPattern::new)
            .collect::<Vec<_>>();
        Self {
            literal_prefixes: literal_prefixes.into(),
            patterns: patterns.into(),
        }
    }

    pub(crate) fn matching_entry<'a>(&'a self, directory: &PackagePath) -> Option<&'a str> {
        for prefix in self.literal_prefixes.iter() {
            if is_component_prefix(prefix.as_str(), directory.as_str()) {
                return Some(prefix.as_str());
            }
        }

        let path_segments = if directory.as_str().is_empty() {
            Vec::new()
        } else {
            directory.as_str().split('/').collect::<Vec<_>>()
        };
        self.patterns
            .iter()
            .find(|pattern| pattern.matches_prefix(&path_segments))
            .map(|pattern| pattern.original.as_str())
    }
}

impl PartialEq for RepositoryIgnoreMatcher {
    fn eq(&self, other: &Self) -> bool {
        self.literal_prefixes == other.literal_prefixes
            && self.patterns.len() == other.patterns.len()
            && self
                .patterns
                .iter()
                .zip(other.patterns.iter())
                .all(|(left, right)| left.original == right.original)
    }
}

impl Eq for RepositoryIgnoreMatcher {}

fn is_component_prefix(prefix: &str, directory: &str) -> bool {
    if prefix.is_empty() || prefix == directory {
        return true;
    }
    directory
        .strip_prefix(prefix)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

#[derive(Debug, Clone, Allocative)]
struct CompiledPattern {
    original: CompactString,
    segments: Arc<[CompiledPatternSegment]>,
}

impl CompiledPattern {
    fn new(original: CompactString) -> Self {
        let segments = original
            .split('/')
            .map(CompiledPatternSegment::new)
            .collect::<Vec<_>>()
            .into();
        Self { original, segments }
    }

    fn matches_prefix(&self, path: &[&str]) -> bool {
        // With PREFIX semantics, exhausting the pattern succeeds regardless
        // of how many path segments remain.
        let mut suffix = vec![true; path.len() + 1];
        for segment in self.segments.iter().rev() {
            let mut current = vec![false; path.len() + 1];
            match segment {
                CompiledPatternSegment::Recursive => {
                    current[path.len()] = suffix[path.len()];
                    for path_index in (0..path.len()).rev() {
                        current[path_index] = suffix[path_index] || current[path_index + 1];
                    }
                }
                CompiledPatternSegment::Ordinary(segment) => {
                    for path_index in (0..path.len()).rev() {
                        current[path_index] =
                            segment.matches(path[path_index]) && suffix[path_index + 1];
                    }
                }
            }
            suffix = current;
        }
        suffix[0]
    }
}

#[derive(Debug, Clone, Allocative)]
enum CompiledPatternSegment {
    Recursive,
    Ordinary(CompiledOrdinarySegment),
}

impl CompiledPatternSegment {
    fn new(raw: &str) -> Self {
        if raw == "**" {
            return Self::Recursive;
        }
        Self::Ordinary(CompiledOrdinarySegment::new(raw))
    }
}

#[derive(Debug, Clone, Allocative)]
struct CompiledOrdinarySegment {
    explicitly_matches_leading_dot: bool,
    matcher: SegmentMatcher,
}

impl CompiledOrdinarySegment {
    fn new(raw: &str) -> Self {
        let matcher = if raw.is_empty() {
            SegmentMatcher::Never
        } else if raw == "*" {
            SegmentMatcher::Any
        } else if raw.starts_with('*') && raw.rfind('*') == Some(0) {
            SegmentMatcher::SuffixLiteral(CompactString::new(&raw[1..]))
        } else {
            let last_index = raw.len() - 1;
            if raw.ends_with('*') && raw.find('*') == Some(last_index) {
                SegmentMatcher::PrefixLiteral(CompactString::new(&raw[..last_index]))
            } else {
                SegmentMatcher::Generic(
                    raw.chars()
                        .filter_map(|character| match character {
                            '(' | ')' => None,
                            '*' => Some(SegmentAtom::AnyMany),
                            '?' => Some(SegmentAtom::AnyOne),
                            literal => Some(SegmentAtom::Literal(literal.into())),
                        })
                        .collect::<Vec<_>>()
                        .into(),
                )
            }
        };
        Self {
            explicitly_matches_leading_dot: raw.starts_with('.'),
            matcher,
        }
    }

    fn matches(&self, candidate: &str) -> bool {
        if candidate.is_empty() {
            return false;
        }
        match &self.matcher {
            SegmentMatcher::Never => false,
            // Bazel's exact `*` fast path precedes its leading-dot guard.
            SegmentMatcher::Any => true,
            matcher => {
                if candidate.starts_with('.') && !self.explicitly_matches_leading_dot {
                    return false;
                }
                match matcher {
                    SegmentMatcher::SuffixLiteral(suffix) => candidate.ends_with(suffix.as_str()),
                    SegmentMatcher::PrefixLiteral(prefix) => candidate.starts_with(prefix.as_str()),
                    SegmentMatcher::Generic(atoms) => generic_segment_matches(atoms, candidate),
                    SegmentMatcher::Never | SegmentMatcher::Any => unreachable!(),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Allocative)]
enum SegmentMatcher {
    Never,
    Any,
    SuffixLiteral(CompactString),
    PrefixLiteral(CompactString),
    Generic(Arc<[SegmentAtom]>),
}

#[derive(Debug, Clone, Copy, Allocative)]
enum SegmentAtom {
    AnyMany,
    AnyOne,
    Literal(u32),
}

fn generic_segment_matches(atoms: &[SegmentAtom], candidate: &str) -> bool {
    let candidate = candidate.chars().collect::<Vec<_>>();
    let mut matched = vec![false; candidate.len() + 1];
    matched[0] = true;

    for atom in atoms {
        let mut next = vec![false; candidate.len() + 1];
        match atom {
            SegmentAtom::AnyMany => {
                next[0] = matched[0];
                for index in 1..=candidate.len() {
                    next[index] = matched[index]
                        || (next[index - 1] && java_regex_dot_matches(candidate[index - 1]));
                }
            }
            SegmentAtom::AnyOne => {
                for index in 0..candidate.len() {
                    next[index + 1] = matched[index] && java_regex_dot_matches(candidate[index]);
                }
            }
            SegmentAtom::Literal(literal) => {
                for index in 0..candidate.len() {
                    next[index + 1] = matched[index] && u32::from(candidate[index]) == *literal;
                }
            }
        }
        matched = next;
    }
    matched[candidate.len()]
}

fn java_regex_dot_matches(character: char) -> bool {
    !matches!(
        character,
        '\n' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}'
    )
}

#[cfg(test)]
mod tests {
    use compact_str::CompactString;
    use slug_identity_v2::PackagePath;

    use super::RepositoryIgnoreMatcher;

    fn path(value: &str) -> PackagePath {
        PackagePath::parse(value).unwrap()
    }

    fn matcher(prefixes: &[&str], patterns: &[&str]) -> RepositoryIgnoreMatcher {
        RepositoryIgnoreMatcher::new(
            prefixes.iter().map(|prefix| path(prefix)),
            patterns.iter().map(|pattern| CompactString::new(pattern)),
        )
    }

    fn assert_pattern(pattern: &str, directory: &str, expected: bool) {
        let matcher = matcher(&[], &[pattern]);
        assert_eq!(
            matcher.matching_entry(&path(directory)),
            expected.then_some(pattern),
            "pattern={pattern:?}, directory={directory:?}"
        );
    }

    #[test]
    fn literal_prefixes_are_sorted_deduplicated_and_component_aware() {
        let repository_ignore = matcher(&["foo/bar", "foo", "bar", "foo"], &["pattern/**"]);
        assert_eq!(
            repository_ignore
                .literal_prefixes
                .iter()
                .map(PackagePath::as_str)
                .collect::<Vec<_>>(),
            ["bar", "foo", "foo/bar"]
        );
        assert_eq!(repository_ignore.matching_entry(&path("foo")), Some("foo"));
        assert_eq!(
            repository_ignore.matching_entry(&path("foo/bar/baz")),
            Some("foo")
        );
        assert_eq!(repository_ignore.matching_entry(&path("fooz")), None);
        assert_eq!(
            repository_ignore.matching_entry(&path("pattern/child")),
            Some("pattern/**")
        );

        let root = matcher(&["child", ""], &["**"]);
        assert_eq!(root.matching_entry(&path("")), Some(""));
        assert_eq!(root.matching_entry(&path("child/grandchild")), Some(""));
    }

    #[test]
    fn matches_bazel_prefix_recursive_and_mixed_wildcard_table() {
        for (pattern, directory, expected) in [
            ("foo/bar", "foo/bar", true),
            ("foo/bar", "foo/bar/child", true),
            ("foo/bar", "foo", false),
            ("foo/bar", "foo/barn", false),
            ("**", "", true),
            ("**", ".hidden/child", true),
            ("**/sub", "sub", true),
            ("**/sub", "a/b/sub/child", true),
            ("**/sub", "a/b", false),
            ("foo/**/bar", "foo/bar", true),
            ("foo/**/bar", "foo/a/b/bar/child", true),
            ("foo/**", "foo", true),
            ("foo/**", "foo/a/b", true),
            ("bar/*/one?ub", "bar/x/onesub", true),
            ("bar/*/one?ub", "bar/x/oneXXub", false),
            ("*/sub", ".hidden/sub", true),
            ("*dden/sub", ".hidden/sub", false),
            (".hi*/*/sub", ".hidden/x/sub", true),
            ("?/sub", ".hidden/sub", false),
        ] {
            assert_pattern(pattern, directory, expected);
        }
    }

    #[test]
    fn matches_bazel_escaping_and_optimization_sensitive_parentheses() {
        for (pattern, directory, expected) in [
            ("*?", "value?", true),
            ("*?", "valuex", false),
            ("?*", "?value", true),
            ("?*", "xvalue", false),
            ("*(bar)", "x(bar)", true),
            ("*(bar)", "xbar", false),
            ("(bar)*", "(bar)x", true),
            ("(bar)*", "barx", false),
            ("foo(bar)", "foobar", true),
            ("foo(bar)", "foo(bar)", false),
            ("*foo(bar)*", "xfoobary", true),
            ("*foo(bar)*", "xfoo(bar)y", false),
            ("foo*(bar)", "fooxbar", true),
            ("foo*(bar)", "foox(bar)", false),
            ("(.hidden)", ".hidden", false),
            (".(hidden)", ".hidden", true),
            (r"^$|+{}[]\.", r"^$|+{}[]\.", true),
            ("file.txt", "fileXtxt", false),
        ] {
            assert_pattern(pattern, directory, expected);
        }
    }

    #[test]
    fn generic_regex_wildcards_reject_java_line_terminators_but_fast_paths_do_not() {
        for separator in ['\n', '\r', '\u{0085}', '\u{2028}', '\u{2029}'] {
            let middle = format!("a{separator}b");
            assert_pattern("a?b", &middle, false);
            assert_pattern("a*b", &middle, false);

            let separator = separator.to_string();
            assert_pattern("*", &separator, true);
            assert_pattern("**", &separator, true);
            assert_pattern("*b", &format!("{separator}b"), true);
            assert_pattern("a*", &format!("a{separator}"), true);
        }
    }

    #[test]
    fn raw_patterns_are_retained_in_order_without_validation() {
        let originals = [
            "",
            "/absolute",
            "trailing/",
            "a//b",
            ".",
            "..",
            "a**b",
            "**foo",
            "a**b",
        ];
        let repository_ignore = matcher(&[], &originals);
        assert_eq!(
            repository_ignore
                .patterns
                .iter()
                .map(|pattern| pattern.original.as_str())
                .collect::<Vec<_>>(),
            originals
        );
        assert_eq!(repository_ignore.matching_entry(&path("")), None);
        assert_eq!(repository_ignore.matching_entry(&path("absolute")), None);
        assert_eq!(repository_ignore.matching_entry(&path("trailing")), None);
        assert_eq!(
            repository_ignore.matching_entry(&path("axxb")),
            Some("a**b")
        );

        let leading_embedded_recursive = matcher(&[], &["**foo"]);
        assert_eq!(
            leading_embedded_recursive.matching_entry(&path("xxfoo")),
            Some("**foo")
        );
    }

    #[test]
    fn matching_entry_prefers_literals_then_original_pattern_order() {
        let pattern_first = matcher(&[], &["**", "foo"]);
        assert_eq!(pattern_first.matching_entry(&path("foo")), Some("**"));

        let exact_first = matcher(&[], &["foo", "**"]);
        assert_eq!(exact_first.matching_entry(&path("foo")), Some("foo"));

        let literal_first = matcher(&["foo"], &["**"]);
        assert_eq!(
            literal_first.matching_entry(&path("foo/child")),
            Some("foo")
        );
    }

    #[test]
    fn semantic_equality_uses_normalized_prefixes_and_ordered_original_patterns() {
        let left = matcher(&["z", "a", "z"], &["**/one", "two"]);
        let same = matcher(&["a", "z"], &["**/one", "two"]);
        let reordered_patterns = matcher(&["a", "z"], &["two", "**/one"]);
        let duplicate_pattern = matcher(&["a", "z"], &["**/one", "two", "two"]);
        let different_prefix = matcher(&["a"], &["**/one", "two"]);

        assert_eq!(left, same);
        assert_ne!(left, reordered_patterns);
        assert_ne!(left, duplicate_pattern);
        assert_ne!(left, different_prefix);
    }
}
