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

use crate::label::ApparentLabel;
use crate::package::PackagePath;
use crate::repo::ApparentRepoName;

/// Package-wildcard spelling retained until loading resolves a possible
/// same-named explicit target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPatternWildcard {
    All,
    Star,
    AllTargets,
}

impl TargetPatternWildcard {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Star => "*",
            Self::AllTargets => "all-targets",
        }
    }

    pub fn rules_only(self) -> bool {
        matches!(self, Self::All)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetPattern {
    Single(ApparentLabel),
    PackageWildcard {
        repo: ApparentRepoName,
        package: PackagePath,
        wildcard: TargetPatternWildcard,
    },
    Recursive {
        repo: ApparentRepoName,
        package: PackagePath,
        wildcard: Option<TargetPatternWildcard>,
    },
}

impl TargetPattern {
    pub fn parse(value: &str) -> Result<Self, String> {
        let (repo, rest) = split_repo(value)?;
        if let Some((package, wildcard)) = recursive_pattern(rest) {
            return Ok(Self::Recursive {
                repo,
                package: PackagePath::parse(package)?,
                wildcard,
            });
        }
        let package_part = rest.split_once(':').map_or(rest, |(package, _)| package);
        if package_part.split('/').any(|component| component == "...") {
            return Err(format!(
                "invalid target pattern {value}: '...' can only be used with wildcard targets"
            ));
        }
        for (suffix, wildcard) in wildcard_suffixes() {
            if let Some(package) = rest.strip_suffix(suffix) {
                return Ok(Self::PackageWildcard {
                    repo,
                    package: PackagePath::parse(package)?,
                    wildcard: *wildcard,
                });
            }
        }
        ApparentLabel::parse(value).map(Self::Single)
    }
}

impl fmt::Display for TargetPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(label) => write!(f, "{label}"),
            Self::PackageWildcard {
                repo,
                package,
                wildcard,
            } => write_repo_package(f, repo, package, ":", wildcard.as_str()),
            Self::Recursive {
                repo,
                package,
                wildcard,
            } => {
                write_recursive(f, repo, package)?;
                if let Some(wildcard) = wildcard {
                    write!(f, ":{}", wildcard.as_str())?;
                }
                Ok(())
            }
        }
    }
}

fn recursive_pattern(value: &str) -> Option<(&str, Option<TargetPatternWildcard>)> {
    for (suffix, wildcard) in wildcard_suffixes() {
        if let Some(package) = value.strip_suffix(suffix).and_then(recursive_package) {
            return Some((package, Some(*wildcard)));
        }
    }
    recursive_package(value).map(|package| (package, None))
}

fn wildcard_suffixes() -> &'static [(&'static str, TargetPatternWildcard)] {
    &[
        (":all", TargetPatternWildcard::All),
        (":*", TargetPatternWildcard::Star),
        (":all-targets", TargetPatternWildcard::AllTargets),
    ]
}

fn recursive_package(value: &str) -> Option<&str> {
    (value == "...")
        .then_some("")
        .or_else(|| value.strip_suffix("/..."))
}

fn split_repo(value: &str) -> Result<(ApparentRepoName, &str), String> {
    if let Some(rest) = value.strip_prefix('@') {
        if rest.starts_with('@') {
            return Err(format!(
                "target pattern uses apparent repo spelling, not @@: {value}"
            ));
        }
        let Some((repo, package)) = rest.split_once("//") else {
            return Err(format!("target pattern must contain //: {value}"));
        };
        let repo = if repo.is_empty() {
            ApparentRepoName::root()
        } else {
            ApparentRepoName::new(repo)?
        };
        return Ok((repo, package));
    }
    let Some(rest) = value.strip_prefix("//") else {
        return Err(format!(
            "target pattern must start with // or @repo//: {value}"
        ));
    };
    Ok((ApparentRepoName::root(), rest))
}

fn write_repo_package(
    f: &mut fmt::Formatter<'_>,
    repo: &ApparentRepoName,
    package: &PackagePath,
    separator: &str,
    suffix: &str,
) -> fmt::Result {
    if repo.is_root() {
        write!(f, "//{}{separator}{suffix}", package)
    } else {
        write!(f, "{}//{}{separator}{suffix}", repo, package)
    }
}

fn write_recursive(
    f: &mut fmt::Formatter<'_>,
    repo: &ApparentRepoName,
    package: &PackagePath,
) -> fmt::Result {
    let separator = if package.as_str().is_empty() {
        "..."
    } else {
        "/..."
    };
    write_repo_package(f, repo, package, separator, "")
}
