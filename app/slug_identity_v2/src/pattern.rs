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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetPattern {
    Single(ApparentLabel),
    PackageAll {
        repo: ApparentRepoName,
        package: PackagePath,
    },
    Recursive {
        repo: ApparentRepoName,
        package: PackagePath,
    },
}

impl TargetPattern {
    pub fn parse(value: &str) -> Result<Self, String> {
        let (repo, rest) = split_repo(value)?;
        let package_part = rest;
        if let Some(package) = package_part.strip_suffix("/...") {
            return Ok(Self::Recursive {
                repo,
                package: PackagePath::parse(package)?,
            });
        }
        if let Some(package) = package_part.strip_suffix(":all") {
            return Ok(Self::PackageAll {
                repo,
                package: PackagePath::parse(package)?,
            });
        }
        ApparentLabel::parse(value).map(Self::Single)
    }
}

impl fmt::Display for TargetPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(label) => write!(f, "{label}"),
            Self::PackageAll { repo, package } => write_repo_package(f, repo, package, ":all"),
            Self::Recursive { repo, package } => write_repo_package(f, repo, package, "/..."),
        }
    }
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
    suffix: &str,
) -> fmt::Result {
    if repo.is_root() {
        write!(f, "//{}{suffix}", package)
    } else {
        write!(f, "{}//{}{suffix}", repo, package)
    }
}
