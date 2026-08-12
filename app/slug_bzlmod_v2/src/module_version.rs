/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory.
 */

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;

#[derive(Debug, Clone, Allocative)]
pub(crate) struct BazelModuleVersion {
    pub(crate) canonical: CompactString,
    release: ReleaseIdentifiers,
    prerelease: Option<Arc<[VersionIdentifier]>>,
}

#[derive(Debug, Clone, Allocative)]
enum ReleaseIdentifiers {
    EmptySentinel,
    Identifiers(Arc<[VersionIdentifier]>),
}

#[derive(Debug, Clone, Allocative)]
struct VersionIdentifier {
    spelling: CompactString,
    numeric: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BazelModuleVersionParseError {
    InvalidBuildSuffix,
    InvalidVersion,
    NumericOverflow,
}

impl BazelModuleVersionParseError {
    pub(crate) fn lockfile_message(self) -> &'static str {
        match self {
            Self::InvalidBuildSuffix => "invalid Bazel module version build suffix",
            Self::InvalidVersion => "invalid Bazel module version",
            Self::NumericOverflow => "numeric version identifier exceeds unsigned 64-bit range",
        }
    }
}

impl fmt::Display for BazelModuleVersionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.lockfile_message())
    }
}

impl Error for BazelModuleVersionParseError {}

impl BazelModuleVersion {
    pub(crate) fn empty() -> Self {
        Self {
            canonical: CompactString::new(""),
            release: ReleaseIdentifiers::EmptySentinel,
            prerelease: None,
        }
    }

    pub(crate) fn parse(spelling: &str) -> Result<Self, BazelModuleVersionParseError> {
        if spelling.is_empty() {
            return Ok(Self::empty());
        }
        let mut build_split = spelling.split('+');
        let without_build = build_split.next().unwrap_or(spelling);
        let build = build_split.next();
        if build_split.next().is_some()
            || build.is_some_and(|part| !valid_identifier_part(part, true))
        {
            return Err(BazelModuleVersionParseError::InvalidBuildSuffix);
        }
        let (release, prerelease) = match without_build.split_once('-') {
            Some((release, prerelease)) => (release, Some(prerelease)),
            None => (without_build, None),
        };
        let release = parse_identifiers(release, false)?;
        let prerelease = prerelease
            .map(|value| parse_identifiers(value, true))
            .transpose()?;
        Ok(Self {
            canonical: without_build.into(),
            release: ReleaseIdentifiers::Identifiers(release),
            prerelease,
        })
    }

    pub(crate) fn normalized(&self) -> &str {
        self.canonical.as_str()
    }

    pub(crate) fn is_empty(&self) -> bool {
        matches!(self.release, ReleaseIdentifiers::EmptySentinel)
    }
}

impl PartialEq for BazelModuleVersion {
    fn eq(&self, other: &Self) -> bool {
        self.canonical == other.canonical
    }
}

impl Eq for BazelModuleVersion {}

impl Hash for BazelModuleVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.canonical.hash(state);
    }
}

impl PartialOrd for BazelModuleVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BazelModuleVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.release, &other.release) {
            (ReleaseIdentifiers::EmptySentinel, ReleaseIdentifiers::EmptySentinel) => {
                Ordering::Equal
            }
            (ReleaseIdentifiers::EmptySentinel, _) => Ordering::Greater,
            (_, ReleaseIdentifiers::EmptySentinel) => Ordering::Less,
            (ReleaseIdentifiers::Identifiers(left), ReleaseIdentifiers::Identifiers(right)) => {
                compare_identifier_lists(left, right).then_with(|| {
                    match (&self.prerelease, &other.prerelease) {
                        (Some(left), Some(right)) => compare_identifier_lists(left, right),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => Ordering::Equal,
                    }
                })
            }
        }
    }
}

fn parse_identifiers(
    part: &str,
    allow_hyphen: bool,
) -> Result<Arc<[VersionIdentifier]>, BazelModuleVersionParseError> {
    if !valid_identifier_part(part, allow_hyphen) {
        return Err(BazelModuleVersionParseError::InvalidVersion);
    }
    part.split('.')
        .map(|spelling| {
            let numeric = if spelling.bytes().all(|byte| byte.is_ascii_digit()) {
                Some(
                    spelling
                        .parse::<u64>()
                        .map_err(|_| BazelModuleVersionParseError::NumericOverflow)?,
                )
            } else {
                None
            };
            Ok(VersionIdentifier {
                spelling: spelling.into(),
                numeric,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Arc::from)
}

fn valid_identifier_part(part: &str, allow_hyphen: bool) -> bool {
    !part.is_empty()
        && part.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || (allow_hyphen && byte == b'-'))
        })
}

fn compare_identifier_lists(left: &[VersionIdentifier], right: &[VersionIdentifier]) -> Ordering {
    let mut left = left.iter();
    let mut right = right.iter();
    loop {
        match (left.next(), right.next()) {
            (Some(left), Some(right)) => {
                let ordering = match (left.numeric, right.numeric) {
                    (Some(left_number), Some(right_number)) => left_number
                        .cmp(&right_number)
                        .then_with(|| left.spelling.cmp(&right.spelling)),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => left.spelling.cmp(&right.spelling),
                };
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => return Ordering::Equal,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;

    use super::BazelModuleVersion;

    fn version(spelling: &str) -> BazelModuleVersion {
        BazelModuleVersion::parse(spelling).unwrap()
    }

    #[test]
    fn grammar_and_normalization_match_bazel_9_2() {
        for spelling in [
            "",
            "1",
            "1.alpha.2",
            "1-a-b.2",
            "1+build-1.2",
            "1-a+build-1.2",
            "18446744073709551615",
        ] {
            BazelModuleVersion::parse(spelling).unwrap();
        }
        for spelling in [
            "_",
            "é",
            "1..2",
            "1-",
            "1+",
            "1+a+b",
            "18446744073709551616",
        ] {
            assert!(BazelModuleVersion::parse(spelling).is_err(), "{spelling}");
        }
        assert_eq!(version("1+build-1.2").normalized(), "1");
        assert_eq!(version("1-a+build-1.2").normalized(), "1-a");
    }

    #[test]
    fn equality_hash_and_order_match_bazel_9_2() {
        let left = version("1+a");
        let right = version("1+b");
        assert_eq!(left, right);
        let mut left_hash = DefaultHasher::new();
        left.hash(&mut left_hash);
        let mut right_hash = DefaultHasher::new();
        right.hash(&mut right_hash);
        assert_eq!(left_hash.finish(), right_hash.finish());

        for (lower, higher) in [
            ("1-01", "1-1"),
            ("1", "1.0"),
            ("1-a", "1"),
            ("2", "10"),
            ("1", "alpha"),
            ("1", ""),
        ] {
            assert!(version(lower) < version(higher), "{lower} < {higher}");
        }
    }

    #[test]
    fn comparison_equal_is_exactly_normalized_equality() {
        let spellings = ["", "0", "00", "1", "1+a", "1+b", "1-0", "1-a", "1.0"];
        for left in spellings {
            for right in spellings {
                let left = version(left);
                let right = version(right);
                assert_eq!(left.cmp(&right) == Ordering::Equal, left == right);
            }
        }
    }
}
