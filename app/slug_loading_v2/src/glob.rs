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
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

/// Immutable package-relative input for synchronous BUILD-file glob calls.
#[derive(Debug, Clone, Dupe, PartialEq, Eq, Allocative)]
pub struct PackageListing {
    regular_files: Arc<[CompactString]>,
    directories: Arc<[CompactString]>,
    watched_directories: Arc<[CompactString]>,
    subpackages: Arc<[CompactString]>,
}

impl PackageListing {
    pub fn new(
        regular_files: Vec<CompactString>,
        directories: Vec<CompactString>,
        watched_directories: Vec<CompactString>,
        subpackages: Vec<CompactString>,
    ) -> Self {
        Self {
            regular_files: sorted_shared(regular_files),
            directories: sorted_shared(directories),
            watched_directories: sorted_shared(watched_directories),
            subpackages: sorted_shared(subpackages),
        }
    }

    pub fn regular_files(&self) -> &[CompactString] {
        &self.regular_files
    }

    pub fn directories(&self) -> &[CompactString] {
        &self.directories
    }

    pub fn watched_directories(&self) -> &[CompactString] {
        &self.watched_directories
    }

    pub fn subpackages(&self) -> &[CompactString] {
        &self.subpackages
    }
}

fn sorted_shared(mut paths: Vec<CompactString>) -> Arc<[CompactString]> {
    paths.sort_unstable();
    paths.dedup();
    paths.into()
}

#[derive(Debug, Clone, Copy, Dupe, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum GlobSegmentPatternKind {
    Literal,
    Wildcard,
}

#[derive(Debug, Clone, Dupe, PartialEq, Eq, Hash, Allocative)]
enum GlobPatternFragment {
    Segment {
        start: usize,
        end: usize,
        kind: GlobSegmentPatternKind,
    },
    RecursiveWildcard,
}

#[derive(Debug, Clone, Dupe, PartialEq, Eq, Hash, Allocative)]
struct GlobPatternData {
    raw: Arc<str>,
    fragments: Arc<[GlobPatternFragment]>,
}

/// One checked immutable pattern shared by flat and observed Host globbing.
#[derive(Debug, Clone, Dupe, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct GlobPattern(Arc<GlobPatternData>);

#[derive(Debug, Clone, Dupe, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct GlobPatternError {
    pattern: Arc<str>,
    message: &'static str,
}

impl GlobPatternError {
    fn public(&self) -> GlobError {
        GlobError::InvalidPattern {
            pattern: self.pattern.to_string(),
            message: self.message,
        }
    }
}

impl GlobPattern {
    pub(crate) fn include(pattern: impl AsRef<str>) -> Result<Self, GlobPatternError> {
        Self::parse(pattern.as_ref(), false)
    }

    fn complex_exclude(pattern: &str) -> Result<Self, GlobPatternError> {
        Self::parse(pattern, true)
    }

    fn parse(pattern: &str, allow_question: bool) -> Result<Self, GlobPatternError> {
        let raw: Arc<str> = Arc::from(pattern);
        let reject = |message| GlobPatternError {
            pattern: raw.dupe(),
            message,
        };
        // GlobValue rejects '?' before UnixGlob performs structural validation.
        if !allow_question && pattern.contains('?') {
            return Err(reject("? wildcards are not allowed in include patterns"));
        }
        if pattern.is_empty() {
            return Err(reject("empty pattern"));
        }
        if pattern.starts_with('/') {
            return Err(reject("pattern cannot be absolute"));
        }
        if pattern.contains('\0') {
            return Err(reject("NUL path bytes are unsupported"));
        }

        let mut fragments = Vec::new();
        let mut start = 0;
        for segment in pattern.split('/') {
            let end = start + segment.len();
            if segment.is_empty() {
                return Err(reject("empty segment not permitted"));
            }
            if segment == "." {
                return Err(reject("segment '.' not permitted"));
            }
            if segment == ".." {
                return Err(reject("segment '..' not permitted"));
            }
            if contains_adjacent_stars(segment.as_bytes()) && segment != "**" {
                return Err(reject("recursive wildcard must be its own segment"));
            }
            if segment == "**" {
                fragments.push(GlobPatternFragment::RecursiveWildcard);
            } else {
                fragments.push(GlobPatternFragment::Segment {
                    start,
                    end,
                    kind: if segment.contains('*') || segment.contains('?') {
                        GlobSegmentPatternKind::Wildcard
                    } else {
                        GlobSegmentPatternKind::Literal
                    },
                });
            }
            start = end + 1;
        }
        Ok(Self(Arc::new(GlobPatternData {
            raw,
            fragments: fragments.into(),
        })))
    }

    pub(crate) fn raw(&self) -> &str {
        &self.0.raw
    }

    pub(crate) fn len(&self) -> usize {
        self.0.fragments.len()
    }

    pub(crate) fn is_recursive(&self, index: usize) -> bool {
        matches!(
            self.0.fragments[index],
            GlobPatternFragment::RecursiveWildcard
        )
    }

    pub(crate) fn segment(&self, index: usize) -> Option<GlobSegmentPattern> {
        matches!(
            self.0.fragments.get(index),
            Some(GlobPatternFragment::Segment { .. })
        )
        .then(|| GlobSegmentPattern {
            pattern: self.dupe(),
            fragment_index: index,
        })
    }

    pub(crate) fn recursive_count(&self) -> usize {
        self.0
            .fragments
            .iter()
            .filter(|fragment| matches!(fragment, GlobPatternFragment::RecursiveWildcard))
            .count()
    }

    pub(crate) fn matches_bytes(&self, candidate: &[u8]) -> bool {
        let candidate_segments = if candidate.is_empty() {
            Vec::new()
        } else {
            candidate.split(|byte| *byte == b'/').collect::<Vec<_>>()
        };
        let mut current = vec![false; candidate_segments.len() + 1];
        let mut next = vec![false; candidate_segments.len() + 1];
        current[0] = true;
        for (fragment_index, fragment) in self.0.fragments.iter().enumerate() {
            next.fill(false);
            match fragment {
                GlobPatternFragment::RecursiveWildcard => {
                    next[0] = current[0];
                    for index in 1..next.len() {
                        next[index] = current[index] || next[index - 1];
                    }
                }
                GlobPatternFragment::Segment { .. } => {
                    let segment = GlobSegmentPattern {
                        pattern: self.dupe(),
                        fragment_index,
                    };
                    for (index, candidate) in candidate_segments.iter().enumerate() {
                        if current[index] && glob_segment_matches(&segment, candidate) {
                            next[index + 1] = true;
                        }
                    }
                }
            }
            std::mem::swap(&mut current, &mut next);
        }
        current[candidate_segments.len()]
    }
}

/// A shallow segment view. Equality deliberately ignores surrounding fragments.
#[derive(Debug, Clone, Dupe, Allocative)]
pub(crate) struct GlobSegmentPattern {
    pattern: GlobPattern,
    fragment_index: usize,
}

impl GlobSegmentPattern {
    pub(crate) fn bytes(&self) -> &[u8] {
        self.text().as_bytes()
    }

    fn text(&self) -> &str {
        let GlobPatternFragment::Segment { start, end, .. } =
            &self.pattern.0.fragments[self.fragment_index]
        else {
            unreachable!("a segment view always references a segment")
        };
        &self.pattern.0.raw[*start..*end]
    }

    pub(crate) fn kind(&self) -> GlobSegmentPatternKind {
        let GlobPatternFragment::Segment { kind, .. } =
            self.pattern.0.fragments[self.fragment_index]
        else {
            unreachable!("a segment view always references a segment")
        };
        kind
    }
}

impl PartialEq for GlobSegmentPattern {
    fn eq(&self, other: &Self) -> bool {
        self.kind() == other.kind() && self.bytes() == other.bytes()
    }
}

impl Eq for GlobSegmentPattern {}

impl Hash for GlobSegmentPattern {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind().hash(state);
        self.bytes().hash(state);
    }
}

#[derive(Debug, Clone, Dupe, PartialEq, Eq, Allocative)]
enum GlobExcludePattern {
    Literal(Arc<str>),
    HeadTail {
        raw: Arc<str>,
        head_end: usize,
        tail_start: usize,
    },
    Pattern(GlobPattern),
    Invalid(GlobPatternError),
}

impl GlobExcludePattern {
    fn new(pattern: &str) -> Self {
        if is_wildcard_free(pattern) {
            return Self::Literal(Arc::from(pattern));
        }
        if let Some(position) = pattern.find("**/*") {
            let tail_start = position + 4;
            if is_wildcard_free(&pattern[..position]) && is_wildcard_free(&pattern[tail_start..]) {
                return Self::HeadTail {
                    raw: Arc::from(pattern),
                    head_end: position,
                    tail_start,
                };
            }
        }
        match GlobPattern::complex_exclude(pattern) {
            Ok(pattern) => Self::Pattern(pattern),
            Err(error) => Self::Invalid(error),
        }
    }

    fn matches(&self, candidate: &[u8]) -> bool {
        match self {
            Self::Literal(pattern) => pattern.as_bytes() == candidate,
            Self::HeadTail {
                raw,
                head_end,
                tail_start,
            } => {
                candidate.starts_with(raw[..*head_end].as_bytes())
                    && candidate.ends_with(raw[*tail_start..].as_bytes())
            }
            Self::Pattern(pattern) => pattern.matches_bytes(candidate),
            Self::Invalid(_) => false,
        }
    }
}

/// One observed BUILD-file glob call.
#[derive(Debug, Clone, Dupe, PartialEq, Eq, Allocative)]
pub struct GlobSpec {
    includes: Arc<[GlobPattern]>,
    excludes: Arc<[GlobExcludePattern]>,
    pub(crate) exclude_directories: bool,
    pub(crate) allow_empty: bool,
}

impl GlobSpec {
    pub fn new(
        includes: impl IntoIterator<Item = impl AsRef<str>>,
        excludes: impl IntoIterator<Item = impl AsRef<str>>,
        exclude_directories: bool,
        allow_empty: bool,
    ) -> Result<Self, GlobError> {
        let includes = includes
            .into_iter()
            .map(|pattern| GlobPattern::include(pattern.as_ref()).map_err(|error| error.public()))
            .collect::<Result<Vec<_>, _>>()?;
        let excludes = excludes
            .into_iter()
            .map(|pattern| GlobExcludePattern::new(pattern.as_ref()))
            .collect::<Vec<_>>();
        Ok(Self {
            includes: includes.into(),
            excludes: excludes.into(),
            exclude_directories,
            allow_empty,
        })
    }

    pub(crate) fn includes(&self) -> &[GlobPattern] {
        &self.includes
    }

    pub(crate) fn check_include_matches(&self, matched: &[bool]) -> Result<(), GlobError> {
        if !self.allow_empty
            && let Some((index, _)) = matched.iter().enumerate().find(|(_, value)| !**value)
        {
            return Err(GlobError::EmptyPattern {
                pattern: self.includes[index].raw().to_owned(),
            });
        }
        Ok(())
    }

    pub(crate) fn validate_excludes(&self) -> Result<(), GlobError> {
        for exclude in self.excludes.iter() {
            if let GlobExcludePattern::Invalid(error) = exclude {
                return Err(error.public());
            }
        }
        Ok(())
    }

    pub(crate) fn is_excluded(&self, candidate: &[u8]) -> bool {
        self.excludes
            .iter()
            .any(|pattern| pattern.matches(candidate))
    }

    pub(crate) fn check_final_matches(&self, empty: bool) -> Result<(), GlobError> {
        if !self.allow_empty && empty {
            Err(GlobError::AllExcluded)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobError {
    InvalidPattern {
        pattern: String,
        message: &'static str,
    },
    EmptyPattern {
        pattern: String,
    },
    AllExcluded,
}

impl fmt::Display for GlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPattern { pattern, message } => {
                write!(f, "invalid glob pattern {pattern:?}: {message}")
            }
            Self::EmptyPattern { pattern } => write!(
                f,
                "glob pattern '{pattern}' didn't match anything, but allow_empty is set to False"
            ),
            Self::AllExcluded => f.write_str(
                "all files in the glob have been excluded, but allow_empty is set to False",
            ),
        }
    }
}

impl std::error::Error for GlobError {}

/// Resolve a reviewed M1 glob pattern subset over a prepared package listing.
pub fn expand_glob(listing: &PackageListing, spec: &GlobSpec) -> Result<Vec<String>, GlobError> {
    let mut include_matched = vec![false; spec.includes.len()];
    let mut matches = Vec::new();
    let candidates = listing.regular_files.iter().chain(
        (!spec.exclude_directories)
            .then_some(listing.directories.iter())
            .into_iter()
            .flatten(),
    );

    for candidate in candidates {
        let mut included = false;
        for (index, pattern) in spec.includes.iter().enumerate() {
            if pattern.matches_bytes(candidate.as_bytes()) {
                include_matched[index] = true;
                included = true;
            }
        }
        if included {
            matches.push(candidate.to_string());
        }
    }

    spec.check_include_matches(&include_matched)?;
    spec.validate_excludes()?;
    matches.retain(|candidate| !spec.is_excluded(candidate.as_bytes()));
    spec.check_final_matches(matches.is_empty())?;
    for path in &mut matches {
        if path.starts_with('@') {
            path.insert(0, ':');
        }
    }
    matches.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    matches.dedup();
    Ok(matches)
}

fn contains_adjacent_stars(bytes: &[u8]) -> bool {
    bytes.windows(2).any(|pair| pair == b"**")
}

fn is_wildcard_free(pattern: &str) -> bool {
    !pattern.contains('*') && !pattern.contains('?')
}

pub(crate) fn glob_segment_matches(pattern: &GlobSegmentPattern, candidate: &[u8]) -> bool {
    let raw = pattern.bytes();
    if raw.is_empty() || candidate.is_empty() {
        return false;
    }
    if pattern.kind() == GlobSegmentPatternKind::Literal {
        return raw == candidate;
    }
    if raw == b"*" {
        return true;
    }
    if candidate[0] == b'.' && raw[0] != b'.' {
        return false;
    }
    if raw.contains(&b'?') {
        let Ok(candidate) = std::str::from_utf8(candidate) else {
            return false;
        };
        return unicode_segment_matches(pattern.text(), candidate);
    }
    let ignored_parentheses = (raw.contains(&b'(') || raw.contains(&b')')).then(|| {
        raw.iter()
            .copied()
            .filter(|byte| !matches!(byte, b'(' | b')'))
            .collect::<Vec<_>>()
    });
    star_segment_matches(ignored_parentheses.as_deref().unwrap_or(raw), candidate)
}

fn star_segment_matches(pattern: &[u8], candidate: &[u8]) -> bool {
    let (mut pattern_index, mut candidate_index) = (0, 0);
    let (mut last_star, mut star_candidate) = (None, 0);

    while candidate_index < candidate.len() {
        if pattern_index < pattern.len()
            && pattern[pattern_index] != b'*'
            && pattern[pattern_index] == candidate[candidate_index]
        {
            pattern_index += 1;
            candidate_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            last_star = Some(pattern_index);
            pattern_index += 1;
            star_candidate = candidate_index;
        } else if let Some(star) = last_star {
            star_candidate += 1;
            candidate_index = star_candidate;
            pattern_index = star + 1;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

fn unicode_segment_matches(pattern: &str, candidate: &str) -> bool {
    let pattern = pattern
        .chars()
        .filter(|value| !matches!(value, '(' | ')'))
        .collect::<Vec<_>>();
    let candidate = candidate.chars().collect::<Vec<_>>();
    let (mut pattern_index, mut candidate_index) = (0, 0);
    let (mut last_star, mut star_candidate) = (None, 0);

    while candidate_index < candidate.len() {
        if pattern_index < pattern.len()
            && pattern[pattern_index] != '*'
            && (pattern[pattern_index] == '?'
                || pattern[pattern_index] == candidate[candidate_index])
        {
            pattern_index += 1;
            candidate_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == '*' {
            last_star = Some(pattern_index);
            pattern_index += 1;
            star_candidate = candidate_index;
        } else if let Some(star) = last_star {
            star_candidate += 1;
            candidate_index = star_candidate;
            pattern_index = star + 1;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == '*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}
