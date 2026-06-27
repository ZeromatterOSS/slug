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

use crate::package::PackageIdentifier;
use crate::package::PackagePath;
use crate::package::TargetName;
use crate::repo::ApparentRepoName;
use crate::repo::CanonicalRepoName;
use crate::repo_mapping::RepositoryMapping;
use crate::repo_mapping::RepositoryMappingId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApparentLabel {
    repo: ApparentRepoName,
    package: PackagePath,
    target: TargetName,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalLabel {
    package: PackageIdentifier,
    target: TargetName,
    mapping_id: Option<RepositoryMappingId>,
}

impl ApparentLabel {
    pub fn parse(value: &str) -> Result<Self, String> {
        let (repo, rest) = split_apparent_repo(value)?;
        let (package, target) = split_package_and_target(rest)?;
        Ok(Self {
            repo,
            package,
            target,
        })
    }

    pub fn resolve(&self, mapping: &RepositoryMapping) -> CanonicalLabel {
        CanonicalLabel {
            package: PackageIdentifier::new(mapping.resolve(&self.repo), self.package.clone()),
            target: self.target.clone(),
            mapping_id: Some(mapping.id().clone()),
        }
    }

    pub fn repo(&self) -> &ApparentRepoName {
        &self.repo
    }

    pub fn package(&self) -> &PackagePath {
        &self.package
    }

    pub fn target(&self) -> &TargetName {
        &self.target
    }
}

impl CanonicalLabel {
    pub fn parse(value: &str) -> Result<Self, String> {
        let (repo, rest) = split_canonical_repo(value)?;
        let (package, target) = split_package_and_target(rest)?;
        Ok(Self {
            package: PackageIdentifier::new(repo, package),
            target,
            mapping_id: None,
        })
    }

    pub fn package(&self) -> &PackageIdentifier {
        &self.package
    }

    pub fn target(&self) -> &TargetName {
        &self.target
    }

    pub fn mapping_id(&self) -> Option<&RepositoryMappingId> {
        self.mapping_id.as_ref()
    }
}

impl fmt::Display for ApparentLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repo = if self.repo.is_root() {
            String::new()
        } else {
            self.repo.to_string()
        };
        write!(f, "{}//{}:{}", repo, self.package, self.target)
    }
}

impl fmt::Display for CanonicalLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.package, self.target)
    }
}

fn split_apparent_repo(value: &str) -> Result<(ApparentRepoName, &str), String> {
    if let Some(rest) = value.strip_prefix('@') {
        if rest.starts_with('@') {
            return Err(format!("canonical label is not an apparent label: {value}"));
        }
        let Some((repo, package)) = rest.split_once("//") else {
            return Err(format!("apparent label must contain //: {value}"));
        };
        let repo = if repo.is_empty() {
            ApparentRepoName::root()
        } else {
            ApparentRepoName::new(repo)?
        };
        return Ok((repo, package));
    }
    let Some(rest) = value.strip_prefix("//") else {
        return Err(format!("label must start with // or @repo//: {value}"));
    };
    Ok((ApparentRepoName::root(), rest))
}

fn split_canonical_repo(value: &str) -> Result<(CanonicalRepoName, &str), String> {
    let Some(rest) = value.strip_prefix("@@") else {
        return Err(format!("canonical label must start with @@: {value}"));
    };
    let Some((repo, package)) = rest.split_once("//") else {
        return Err(format!("canonical label must contain //: {value}"));
    };
    let repo = if repo.is_empty() {
        CanonicalRepoName::root()
    } else {
        CanonicalRepoName::new(repo)?
    };
    Ok((repo, package))
}

fn split_package_and_target(value: &str) -> Result<(PackagePath, TargetName), String> {
    let (package, target) = match value.split_once(':') {
        Some((package, target)) => (PackagePath::parse(package)?, TargetName::parse(target)?),
        None => {
            let package = PackagePath::parse(value)?;
            let target = package.default_target_name()?;
            (package, target)
        }
    };
    Ok((package, target))
}
