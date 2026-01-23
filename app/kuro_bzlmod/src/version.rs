/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible version parsing and comparison.
//!
//! Bazel uses a relaxed SemVer format:
//!
//! ```text
//! RELEASE[-PRERELEASE][+BUILD]
//!
//! Examples:
//!   1.0.0
//!   2.3.1-alpha
//!   20210324.2        (date-based, like Abseil)
//!   1.0               (fewer than 3 segments OK)
//!   1.0.0-rc1+build5  (build metadata ignored)
//! ```
//!
//! # Comparison Rules
//!
//! 1. Empty version compares **higher than everything** (used for non-registry overrides)
//! 2. Release segments compared left-to-right:
//!    - Numeric identifiers compared as numbers
//!    - Non-numeric compared lexicographically
//!    - Numeric < non-numeric
//! 3. A version with prerelease is **lower** than the same release without
//! 4. Prerelease segments compared like release segments

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use allocative::Allocative;

/// A version identifier segment.
///
/// Can be either a number (compared numerically) or a string (compared lexicographically).
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum Identifier {
    /// Numeric identifier (e.g., "1", "42", "20210324").
    Numeric(u64),
    /// String identifier (e.g., "alpha", "rc1").
    String(String),
}

impl Identifier {
    fn parse(s: &str) -> Self {
        if let Ok(n) = s.parse::<u64>() {
            Identifier::Numeric(n)
        } else {
            Identifier::String(s.to_owned())
        }
    }
}

impl Ord for Identifier {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Identifier::Numeric(a), Identifier::Numeric(b)) => a.cmp(b),
            (Identifier::String(a), Identifier::String(b)) => a.cmp(b),
            // Numeric < String (per Bazel spec)
            (Identifier::Numeric(_), Identifier::String(_)) => Ordering::Less,
            (Identifier::String(_), Identifier::Numeric(_)) => Ordering::Greater,
        }
    }
}

impl PartialOrd for Identifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Identifier::Numeric(n) => write!(f, "{}", n),
            Identifier::String(s) => write!(f, "{}", s),
        }
    }
}

/// A Bazel-compatible version.
///
/// Versions follow a relaxed SemVer format with flexible segment counts
/// and optional prerelease suffixes.
#[derive(Debug, Clone, PartialEq, Eq, Default, Allocative)]
pub struct Version {
    /// Release segments (e.g., [1, 0, 0] for "1.0.0").
    release: Vec<Identifier>,

    /// Prerelease segments (e.g., ["alpha", 1] for "-alpha.1").
    /// Empty if no prerelease.
    prerelease: Vec<Identifier>,

    /// Whether this is an empty version (compares higher than everything).
    is_empty: bool,

    /// Original string representation for display.
    original: String,
}

impl Version {
    /// Creates an empty version that compares higher than all other versions.
    /// Used for non-registry overrides (local_path_override, git_override, etc.).
    pub fn empty() -> Self {
        Self {
            release: Vec::new(),
            prerelease: Vec::new(),
            is_empty: true,
            original: String::new(),
        }
    }

    /// Parses a version string.
    ///
    /// # Format
    ///
    /// ```text
    /// RELEASE[-PRERELEASE][+BUILD]
    /// ```
    ///
    /// - RELEASE: dot-separated identifiers (e.g., "1.0.0", "20210324.2")
    /// - PRERELEASE: optional, after "-" (e.g., "-alpha", "-rc1")
    /// - BUILD: optional, after "+" (ignored in comparisons)
    ///
    /// # Examples
    ///
    /// ```
    /// use kuro_bzlmod::Version;
    ///
    /// let v1 = Version::parse("1.0.0").unwrap();
    /// let v2 = Version::parse("2.3.1-alpha").unwrap();
    /// let v3 = Version::parse("1.0.0-rc1+build5").unwrap();
    /// ```
    pub fn parse(s: &str) -> kuro_error::Result<Self> {
        if s.is_empty() {
            return Ok(Self::empty());
        }

        let original = s.to_owned();

        // Strip build metadata (everything after "+")
        let s = s.split('+').next().unwrap_or(s);

        // Split release and prerelease
        let (release_str, prerelease_str) = match s.find('-') {
            Some(idx) => (&s[..idx], Some(&s[idx + 1..])),
            None => (s, None),
        };

        // Parse release segments
        let release: Vec<Identifier> = release_str
            .split('.')
            .filter(|s| !s.is_empty())
            .map(Identifier::parse)
            .collect();

        if release.is_empty() {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Invalid version: no release segments in '{}'",
                original
            ));
        }

        // Parse prerelease segments
        let prerelease: Vec<Identifier> = match prerelease_str {
            Some(pre) => pre
                .split('.')
                .filter(|s| !s.is_empty())
                .map(Identifier::parse)
                .collect(),
            None => Vec::new(),
        };

        Ok(Self {
            release,
            prerelease,
            is_empty: false,
            original,
        })
    }

    /// Returns true if this is an empty version.
    pub fn is_empty(&self) -> bool {
        self.is_empty
    }

    /// Returns true if this version has a prerelease suffix.
    pub fn is_prerelease(&self) -> bool {
        !self.prerelease.is_empty()
    }

    /// Returns the original version string.
    pub fn as_str(&self) -> &str {
        &self.original
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // Empty versions compare higher than everything
        match (self.is_empty, other.is_empty) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Greater,
            (false, true) => return Ordering::Less,
            (false, false) => {}
        }

        // Compare release segments
        let max_len = self.release.len().max(other.release.len());
        for i in 0..max_len {
            let a = self.release.get(i);
            let b = other.release.get(i);

            match (a, b) {
                (Some(a), Some(b)) => {
                    let cmp = a.cmp(b);
                    if cmp != Ordering::Equal {
                        return cmp;
                    }
                }
                // Missing segments are treated as less than present segments
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (None, None) => unreachable!(),
            }
        }

        // Release parts are equal, compare prerelease
        // Non-prerelease > prerelease (1.0.0 > 1.0.0-alpha)
        match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                // Compare prerelease segments
                let max_len = self.prerelease.len().max(other.prerelease.len());
                for i in 0..max_len {
                    let a = self.prerelease.get(i);
                    let b = other.prerelease.get(i);

                    match (a, b) {
                        (Some(a), Some(b)) => {
                            let cmp = a.cmp(b);
                            if cmp != Ordering::Equal {
                                return cmp;
                            }
                        }
                        (None, Some(_)) => return Ordering::Less,
                        (Some(_), None) => return Ordering::Greater,
                        (None, None) => unreachable!(),
                    }
                }
                Ordering::Equal
            }
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty {
            write!(f, "<empty>")
        } else {
            write!(f, "{}", self.original)
        }
    }
}

impl FromStr for Version {
    type Err = kuro_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Version::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let v = Version::parse("1.0.0").unwrap();
        assert!(!v.is_empty());
        assert!(!v.is_prerelease());
        assert_eq!(v.as_str(), "1.0.0");
    }

    #[test]
    fn test_parse_prerelease() {
        let v = Version::parse("1.0.0-alpha").unwrap();
        assert!(v.is_prerelease());
        assert_eq!(v.as_str(), "1.0.0-alpha");
    }

    #[test]
    fn test_parse_build_metadata_ignored() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("1.0.0+build123").unwrap();
        assert_eq!(v1.cmp(&v2), Ordering::Equal);
    }

    #[test]
    fn test_comparison_basic() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("2.0.0").unwrap();
        assert!(v1 < v2);
    }

    #[test]
    fn test_comparison_prerelease() {
        let v1 = Version::parse("1.0.0-alpha").unwrap();
        let v2 = Version::parse("1.0.0-beta").unwrap();
        let v3 = Version::parse("1.0.0").unwrap();

        assert!(v1 < v2); // alpha < beta
        assert!(v2 < v3); // prerelease < release
    }

    #[test]
    fn test_comparison_empty() {
        let empty = Version::empty();
        let v1 = Version::parse("999.999.999").unwrap();
        assert!(empty > v1); // empty > everything
    }

    #[test]
    fn test_comparison_numeric_vs_string() {
        let v1 = Version::parse("1.0.0-1").unwrap();
        let v2 = Version::parse("1.0.0-alpha").unwrap();
        assert!(v1 < v2); // numeric < string
    }

    #[test]
    fn test_parse_date_based() {
        let v = Version::parse("20210324.2").unwrap();
        assert!(!v.is_empty());
    }

    #[test]
    fn test_parse_short_version() {
        let v = Version::parse("1.0").unwrap();
        assert!(!v.is_empty());
    }
}
