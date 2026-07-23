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

/// One observed BUILD-file glob call.
#[derive(Debug, Clone, Dupe, PartialEq, Eq, Allocative)]
pub struct GlobSpec {
    pub includes: Arc<[CompactString]>,
    pub excludes: Arc<[CompactString]>,
    pub exclude_directories: bool,
    pub allow_empty: bool,
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
            .map(|pattern| validate_pattern(pattern.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        let excludes = excludes
            .into_iter()
            .map(|pattern| validate_pattern(pattern.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            includes: includes.into(),
            excludes: excludes.into(),
            exclude_directories,
            allow_empty,
        })
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
    if spec.includes.is_empty() {
        return if spec.allow_empty {
            Ok(Vec::new())
        } else {
            Err(GlobError::AllExcluded)
        };
    }

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
            if pattern_matches(pattern, candidate) {
                include_matched[index] = true;
                included = true;
            }
        }
        if included
            && !spec
                .excludes
                .iter()
                .any(|pattern| pattern_matches(pattern, candidate))
        {
            matches.push(candidate.to_string());
        }
    }

    if !spec.allow_empty {
        if let Some((index, _)) = include_matched
            .iter()
            .enumerate()
            .find(|(_, matched)| !**matched)
        {
            return Err(GlobError::EmptyPattern {
                pattern: spec.includes[index].to_string(),
            });
        }
        if matches.is_empty() {
            return Err(GlobError::AllExcluded);
        }
    }

    matches.sort_unstable();
    matches.dedup();
    Ok(matches)
}

fn validate_pattern(pattern: &str) -> Result<CompactString, GlobError> {
    let reject = |message| {
        Err(GlobError::InvalidPattern {
            pattern: pattern.to_owned(),
            message,
        })
    };
    if pattern.is_empty() {
        return reject("empty patterns are not supported");
    }
    if pattern.starts_with('/') {
        return reject("absolute patterns are not supported");
    }
    if pattern.ends_with('/') {
        return reject("trailing separators are not supported");
    }
    if pattern.contains("//") {
        return reject("doubled separators are not supported");
    }
    if pattern.contains('\\') {
        return reject("backslashes and escapes are not supported");
    }
    if pattern.contains("**") {
        return reject("recursive ** patterns are not supported");
    }
    if pattern.contains('?') {
        return reject("? wildcards are not supported");
    }
    if pattern.contains('[') || pattern.contains(']') {
        return reject("character classes are not supported");
    }
    if pattern.contains('{') || pattern.contains('}') {
        return reject("brace patterns are not supported");
    }
    if pattern
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        return reject("dot and uplevel segments are not supported");
    }
    Ok(pattern.into())
}

fn pattern_matches(pattern: &str, candidate: &str) -> bool {
    let mut pattern_segments = pattern.split('/');
    let mut candidate_segments = candidate.split('/');
    loop {
        match (pattern_segments.next(), candidate_segments.next()) {
            (Some(pattern), Some(candidate)) if segment_matches(pattern, candidate) => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

fn segment_matches(pattern: &str, candidate: &str) -> bool {
    let pattern = pattern.as_bytes();
    let candidate = candidate.as_bytes();
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
