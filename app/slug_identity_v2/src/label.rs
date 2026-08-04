/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cmp::Ordering;
use std::fmt;

use allocative::Allocative;

use crate::package::PackageIdentifier;
use crate::package::PackagePath;
use crate::package::TargetName;
use crate::repo::ApparentRepoName;
use crate::repo::CanonicalRepoName;
use crate::repo_mapping::RepositoryMapping;
use crate::repo_mapping::RepositoryMappingId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub struct ApparentLabel {
    repo: ApparentRepoName,
    package: PackagePath,
    target: TargetName,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
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

    pub fn rebind_provisional_root_repository(
        &self,
        destination: &CanonicalRepoName,
    ) -> Result<Self, String> {
        if !self.package.repo().is_root() {
            return Err("canonical label source repository must be provisional root".to_owned());
        }
        if destination.is_root() {
            return Err("canonical label destination repository must be nonroot".to_owned());
        }
        Ok(Self {
            package: PackageIdentifier::new(destination.clone(), self.package.package().clone()),
            target: self.target.clone(),
            mapping_id: None,
        })
    }

    /// Bazel's natural `Label` order compares canonical repository, package,
    /// then target identity. Repository-mapping provenance is not part of a
    /// resolved Bazel label and is intentionally ignored here.
    pub fn bazel_natural_cmp(&self, other: &Self) -> Ordering {
        self.package
            .repo()
            .as_str()
            .cmp(other.package.repo().as_str())
            .then_with(|| {
                self.package
                    .package()
                    .as_str()
                    .cmp(other.package.package().as_str())
            })
            .then_with(|| self.target.as_str().cmp(other.target.as_str()))
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

#[cfg(test)]
mod tests {
    use crate::ApparentLabel;
    use crate::CanonicalLabel;
    use crate::CanonicalRepoName;
    use crate::RepositoryMapping;
    use crate::RepositoryMappingId;
    use crate::serialization::StableSerialize;

    #[test]
    fn rebind_provisional_root_repository_is_typed_and_clears_mapping_provenance() {
        let mapping = RepositoryMapping::new(RepositoryMappingId::new("root-map").unwrap());
        let mapped_root = ApparentLabel::parse("//pkg/sub:target")
            .unwrap()
            .resolve(&mapping);
        assert_eq!(mapped_root.mapping_id(), Some(mapping.id()));

        let rebound = mapped_root
            .rebind_provisional_root_repository(&CanonicalRepoName::new("dep+").unwrap())
            .unwrap();
        assert_eq!(
            rebound,
            CanonicalLabel::parse("@@dep+//pkg/sub:target").unwrap()
        );
        assert_eq!(rebound.package().package(), mapped_root.package().package());
        assert_eq!(rebound.target(), mapped_root.target());
        assert_eq!(rebound.mapping_id(), None);
        assert_eq!(rebound.stable_serialize(), "@@dep+//pkg/sub:target");

        let nonroot = CanonicalLabel::parse("@@other+//pkg:target").unwrap();
        assert_eq!(
            nonroot
                .rebind_provisional_root_repository(&CanonicalRepoName::root())
                .unwrap_err(),
            "canonical label source repository must be provisional root"
        );
        assert_eq!(
            mapped_root
                .rebind_provisional_root_repository(&CanonicalRepoName::root())
                .unwrap_err(),
            "canonical label destination repository must be nonroot"
        );
    }
}
