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
use crate::repo_mapping::OptionMappingLookup;
use crate::repo_mapping::RepositoryMapping;
use crate::repo_mapping::RepositoryMappingId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
enum OptionRepository {
    Visible(CanonicalRepoName),
    NonVisible {
        requested: String,
        owner: CanonicalRepoName,
        did_you_mean_suffix: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub struct ResolvedOptionLabel {
    repository: OptionRepository,
    package: PackagePath,
    target: TargetName,
}

#[derive(Clone, Copy)]
pub enum OptionLabelContext<'a> {
    FirstRoundCanonical,
    MainRepository {
        mapping: &'a RepositoryMapping,
    },
    Package {
        base_package: &'a PackageIdentifier,
        mapping: &'a RepositoryMapping,
    },
}

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

impl<'a> OptionLabelContext<'a> {
    pub fn parse(self, value: &str) -> Result<ResolvedOptionLabel, String> {
        ResolvedOptionLabel::parse(value, self)
    }
}

impl ResolvedOptionLabel {
    pub fn canonical(&self) -> Option<CanonicalLabel> {
        let OptionRepository::Visible(repository) = &self.repository else {
            return None;
        };
        Some(CanonicalLabel {
            package: PackageIdentifier::new(repository.clone(), self.package.clone()),
            target: self.target.clone(),
            mapping_id: None,
        })
    }

    pub fn from_canonical(label: &CanonicalLabel) -> Self {
        Self {
            repository: OptionRepository::Visible(label.package.repo().clone()),
            package: label.package.package().clone(),
            target: label.target.clone(),
        }
    }

    pub fn parse(value: &str, context: OptionLabelContext<'_>) -> Result<Self, String> {
        let rewritten;
        let value = match context {
            OptionLabelContext::FirstRoundCanonical | OptionLabelContext::MainRepository { .. }
                if !value.starts_with('/') && !value.starts_with('@') =>
            {
                rewritten = format!("//{value}");
                rewritten.as_str()
            }
            _ => value,
        };
        let (spelling, package, target, relative) = parse_label_spelling(value)?;
        if package
            .as_str()
            .split('/')
            .any(|component| component == "...")
        {
            return Err("package name cannot contain '...'".to_owned());
        }
        let (repository, package) = match context {
            OptionLabelContext::FirstRoundCanonical => (spelling.into_visible()?, package),
            OptionLabelContext::MainRepository { mapping } => (
                spelling.resolve(mapping, &CanonicalRepoName::root()),
                package,
            ),
            OptionLabelContext::Package {
                base_package,
                mapping,
            } => {
                let repository = match spelling {
                    LabelRepoSpelling::None => {
                        if matches!(package.as_str(), "conditions" | "visibility") {
                            OptionRepository::Visible(CanonicalRepoName::root())
                        } else {
                            OptionRepository::Visible(base_package.repo().clone())
                        }
                    }
                    spelling => spelling.resolve(mapping, base_package.repo()),
                };
                (
                    repository,
                    if relative {
                        base_package.package().clone()
                    } else {
                        package
                    },
                )
            }
        };
        Ok(Self {
            repository,
            package,
            target,
        })
    }

    pub fn unambiguous_form(&self) -> String {
        match &self.repository {
            OptionRepository::Visible(repository) if repository.is_root() => {
                format!("@@//{}:{}", self.package, self.target)
            }
            _ => self.to_string(),
        }
    }

    pub fn bazel_natural_cmp(&self, other: &Self) -> Ordering {
        java_utf16_cmp(self.repository_name(), other.repository_name())
            .then_with(|| java_utf16_cmp(self.package.as_str(), other.package.as_str()))
            .then_with(|| java_utf16_cmp(self.target.as_str(), other.target.as_str()))
    }

    fn repository_name(&self) -> &str {
        match &self.repository {
            OptionRepository::Visible(repository) => repository.as_str(),
            OptionRepository::NonVisible { requested, .. } => requested,
        }
    }
}

impl fmt::Display for ResolvedOptionLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repository {
            OptionRepository::Visible(repository) if repository.is_root() => {
                write!(f, "//{}:{}", self.package, self.target)
            }
            OptionRepository::Visible(repository) => {
                write!(
                    f,
                    "@@{}//{}:{}",
                    repository.as_str(),
                    self.package,
                    self.target
                )
            }
            OptionRepository::NonVisible {
                requested,
                owner,
                did_you_mean_suffix,
            } => write!(
                f,
                "@@[unknown repo '{}' requested from @@{}{}]//{}:{}",
                requested,
                owner.as_str(),
                did_you_mean_suffix,
                self.package,
                self.target
            ),
        }
    }
}

enum LabelRepoSpelling<'a> {
    None,
    Apparent(&'a str),
    Canonical(CanonicalRepoName),
}

impl LabelRepoSpelling<'_> {
    fn into_visible(self) -> Result<OptionRepository, String> {
        match self {
            Self::None => Ok(OptionRepository::Visible(CanonicalRepoName::root())),
            Self::Apparent(apparent) => Ok(OptionRepository::Visible(
                CanonicalRepoName::new_for_bazel_package_identifier(apparent)?,
            )),
            Self::Canonical(canonical) => Ok(OptionRepository::Visible(canonical)),
        }
    }

    fn resolve(self, mapping: &RepositoryMapping, owner: &CanonicalRepoName) -> OptionRepository {
        match self {
            Self::None => OptionRepository::Visible(CanonicalRepoName::root()),
            Self::Canonical(canonical) => OptionRepository::Visible(canonical),
            Self::Apparent(requested) => match mapping.option_lookup(&requested) {
                OptionMappingLookup::Visible(canonical) => OptionRepository::Visible(canonical),
                OptionMappingLookup::NonVisible {
                    did_you_mean_suffix,
                } => OptionRepository::NonVisible {
                    requested: requested.to_owned(),
                    owner: owner.clone(),
                    did_you_mean_suffix,
                },
            },
        }
    }
}

fn parse_label_spelling(
    value: &str,
) -> Result<(LabelRepoSpelling<'_>, PackagePath, TargetName, bool), String> {
    if value.starts_with('/') && !value.starts_with("//") {
        return Err("absolute label must begin with '@' or '//'".to_owned());
    }
    if let Some(rest) = value.strip_prefix("@@") {
        return parse_absolute_label(rest, true);
    }
    if let Some(rest) = value.strip_prefix('@') {
        return parse_absolute_label(rest, false);
    }
    if let Some(rest) = value.strip_prefix("//") {
        let (package, target) = split_option_package_and_target(rest)?;
        return Ok((LabelRepoSpelling::None, package, target, false));
    }
    let target = value.strip_prefix(':').unwrap_or(value);
    if !value.starts_with(':') && (value == "..." || value.ends_with("/...")) {
        return Err("package name cannot contain '...'".to_owned());
    }
    if value.contains(':') && !value.starts_with(':') {
        return Err("absolute label must begin with '@' or '//'".to_owned());
    }
    Ok((
        LabelRepoSpelling::None,
        PackagePath::root(),
        TargetName::parse(target)?,
        true,
    ))
}

fn parse_absolute_label(
    rest: &str,
    canonical: bool,
) -> Result<(LabelRepoSpelling<'_>, PackagePath, TargetName, bool), String> {
    let (repository, value) = if let Some((repository, value)) = rest.split_once("//") {
        (repository, value)
    } else {
        if rest.is_empty() {
            return Err("empty target name".to_owned());
        }
        let repository = option_repo_spelling(rest, canonical)?;
        return Ok((
            repository,
            PackagePath::root(),
            TargetName::parse(rest)?,
            false,
        ));
    };
    let repository = option_repo_spelling(repository, canonical)?;
    let (package, target) = split_option_package_and_target(value)?;
    Ok((repository, package, target, false))
}

fn option_repo_spelling(value: &str, canonical: bool) -> Result<LabelRepoSpelling<'_>, String> {
    if canonical {
        return CanonicalRepoName::new_for_bazel_package_identifier(value)
            .map(LabelRepoSpelling::Canonical);
    }
    validate_bazel_label_repo_name(value)?;
    Ok(LabelRepoSpelling::Apparent(value))
}

fn validate_bazel_label_repo_name(value: &str) -> Result<(), String> {
    if matches!(value, "." | "..") {
        return Err(format!(
            "invalid repository name {value:?}: repo names are not allowed to be {value:?}"
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'+'))
    {
        return Err(format!(
            "invalid repository name {value:?}: repo names may contain only A-Z, a-z, 0-9, '-', '_', '.' and '+'"
        ));
    }
    Ok(())
}

fn split_option_package_and_target(value: &str) -> Result<(PackagePath, TargetName), String> {
    let (package, target) = match value.split_once(':') {
        Some((package, target)) => (
            parse_option_package_path(package)?,
            TargetName::parse(target)?,
        ),
        None => {
            let package = parse_option_package_path(value)?;
            let target = package.default_target_name()?;
            (package, target)
        }
    };
    Ok((package, target))
}

fn parse_option_package_path(value: &str) -> Result<PackagePath, String> {
    if value.split('/').any(|component| component == "...") {
        return Err("package name cannot contain '...'".to_owned());
    }
    if !value.is_empty()
        && (!value
            .bytes()
            .all(|byte| (b' '..=b'~').contains(&byte) && !matches!(byte, b':' | b'\\'))
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains("//")
            || value
                .split('/')
                .any(|component| component.bytes().all(|byte| byte == b'.')))
    {
        return Err("invalid Bazel option package path".to_owned());
    }
    PackagePath::parse(value)
}

fn java_utf16_cmp(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
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

    /// Parses a Bazel Label string using a caller-owned package and mapping.
    pub fn parse_with_package_context(
        value: &str,
        base_package: &PackageIdentifier,
        mut resolve_apparent: impl FnMut(&str) -> Result<CanonicalRepoName, String>,
    ) -> Result<Self, String> {
        let (spelling, package, target, relative) = parse_label_spelling(value)?;
        let repository = match spelling {
            LabelRepoSpelling::None => {
                if !relative && matches!(package.as_str(), "conditions" | "visibility") {
                    CanonicalRepoName::root()
                } else {
                    base_package.repo().clone()
                }
            }
            LabelRepoSpelling::Apparent(requested) => resolve_apparent(requested)?,
            LabelRepoSpelling::Canonical(repository) => repository,
        };
        Ok(Self {
            package: PackageIdentifier::new(
                repository,
                if relative {
                    base_package.package().clone()
                } else {
                    package
                },
            ),
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

    pub fn with_target(&self, target: TargetName) -> Self {
        Self {
            package: self.package.clone(),
            target,
            mapping_id: self.mapping_id.clone(),
        }
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
    use crate::TargetName;
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
    #[test]
    fn option_label_canonical_projection_preserves_nonvisible_state() {
        let canonical = CanonicalLabel::parse("@@dep+//pkg:item").unwrap();
        let option = crate::ResolvedOptionLabel::from_canonical(&canonical);
        assert_eq!(option.canonical(), Some(canonical));
        let mapping = RepositoryMapping::new(RepositoryMappingId::new("owner-map").unwrap());
        let nonvisible = crate::OptionLabelContext::MainRepository { mapping: &mapping }
            .parse("@missing//pkg:item")
            .unwrap();
        assert!(nonvisible.canonical().is_none());
        assert!(nonvisible.to_string().contains("unknown repo 'missing'"));
    }
    #[test]
    fn replacing_target_preserves_package_and_mapping_provenance() {
        let mapping = RepositoryMapping::new(RepositoryMappingId::new("root-map").unwrap());
        let original = ApparentLabel::parse("//pkg:generated.out")
            .unwrap()
            .resolve(&mapping);
        let producer = original.with_target(TargetName::parse("producer").unwrap());

        assert_eq!(producer.to_string(), "@@//pkg:producer");
        assert_eq!(producer.mapping_id(), Some(mapping.id()));
    }
}
