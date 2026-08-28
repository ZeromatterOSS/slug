/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Loading-owned Bazel visibility values.
//!
//! Package-group labels and includes deliberately remain unresolved here.
//! Cross-package lookup, wrong-kind handling, and cycle suppression belong to
//! the request-local query accessor.

use std::sync::Arc;

use allocative::Allocative;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use starlark_map::small_set::SmallSet;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RuleVisibility {
    Public,
    Private,
    Restricted(Arc<RestrictedVisibility>),
}

impl Default for RuleVisibility {
    fn default() -> Self {
        Self::Private
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RestrictedVisibility {
    declared_labels: Arc<[CanonicalLabel]>,
    package_groups: Arc<[CanonicalLabel]>,
    direct_packages: Arc<PackageGroupContents>,
}

impl RestrictedVisibility {
    pub fn declared_labels(&self) -> &[CanonicalLabel] {
        &self.declared_labels
    }

    pub fn package_groups(&self) -> &[CanonicalLabel] {
        &self.package_groups
    }

    pub fn direct_packages(&self) -> &PackageGroupContents {
        &self.direct_packages
    }
}

impl RuleVisibility {
    pub fn is_public(&self) -> bool {
        matches!(self, Self::Public)
    }

    pub fn is_private(&self) -> bool {
        matches!(self, Self::Private)
    }

    /// Labels retained by Bazel's stored raw NODEP visibility attribute.
    ///
    /// Public/private are replaced by an empty stored list during loading.
    pub fn raw_declared_labels(&self) -> &[CanonicalLabel] {
        match self {
            Self::Restricted(value) => value.declared_labels(),
            Self::Public | Self::Private => &[],
        }
    }

    /// Loadable top-level package-group labels only.
    pub fn dependency_labels(&self) -> &[CanonicalLabel] {
        match self {
            Self::Restricted(value) => value.package_groups(),
            Self::Public | Self::Private => &[],
        }
    }

    pub fn direct_packages(&self) -> Option<&PackageGroupContents> {
        match self {
            Self::Restricted(value) => Some(value.direct_packages()),
            Self::Public | Self::Private => None,
        }
    }

    pub fn in_repository_context(&self, repo: &CanonicalRepoName) -> anyhow::Result<Self> {
        let Self::Restricted(value) = self else {
            return Ok(self.clone());
        };
        let project = |label: &CanonicalLabel| {
            if label.package().repo() == repo {
                Ok(label.clone())
            } else {
                label
                    .rebind_provisional_root_repository(repo)
                    .map_err(anyhow::Error::msg)
            }
        };
        Ok(Self::Restricted(Arc::new(RestrictedVisibility {
            declared_labels: value
                .declared_labels
                .iter()
                .map(project)
                .collect::<anyhow::Result<Vec<_>>>()?
                .into(),
            package_groups: value
                .package_groups
                .iter()
                .map(project)
                .collect::<anyhow::Result<Vec<_>>>()?
                .into(),
            direct_packages: Arc::new(value.direct_packages.in_repository_context(repo)?),
        })))
    }

    pub(crate) fn from_declared_labels(
        labels: impl IntoIterator<Item = CanonicalLabel>,
    ) -> anyhow::Result<Self> {
        let labels = labels.into_iter().collect::<Vec<_>>();
        let mut has_public = false;
        let mut private_count = 0;
        for label in &labels {
            if is_special(label, "public") {
                has_public = true;
            } else if is_special(label, "private") {
                private_count += 1;
            } else if label.package().repo().is_root()
                && label.package().package().as_str() == "visibility"
                && direct_kind(label).is_none()
            {
                anyhow::bail!(
                    "Invalid visibility label '{}'; did you mean //visibility:public or //visibility:private?",
                    apparent_label(label)
                );
            }
        }
        if has_public {
            return Ok(Self::Public);
        }
        if private_count == labels.len() {
            return Ok(Self::Private);
        }

        let declared_labels = labels
            .into_iter()
            .filter(|label| !is_special(label, "private"))
            .collect::<Vec<_>>();
        let mut direct = MutablePackageGroupContents::default();
        let mut package_groups = Vec::new();
        for label in &declared_labels {
            match direct_kind(label) {
                Some(PackageSpecKind::Exact) => {
                    direct.exact_positive.insert(label.package().clone());
                }
                Some(PackageSpecKind::Subtree) => {
                    direct.subtree_positive.push(label.package().clone());
                }
                None => package_groups.push(label.clone()),
            }
        }
        Ok(Self::Restricted(Arc::new(RestrictedVisibility {
            declared_labels: declared_labels.into(),
            package_groups: package_groups.into(),
            direct_packages: Arc::new(direct.freeze()),
        })))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum VisibilitySource {
    Declared(RuleVisibility),
    PackageDefault,
    GeneratingRule,
    AlwaysPublic,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct PackageGroupContents {
    exact_positive: SmallSet<PackageIdentifier>,
    subtree_positive: Arc<[PackageIdentifier]>,
    positive_all: bool,
    exact_negative: SmallSet<PackageIdentifier>,
    subtree_negative: Arc<[PackageIdentifier]>,
    has_private: bool,
}

impl Default for PackageGroupContents {
    fn default() -> Self {
        Self {
            exact_positive: SmallSet::new(),
            subtree_positive: Arc::from([]),
            positive_all: false,
            exact_negative: SmallSet::new(),
            subtree_negative: Arc::from([]),
            has_private: false,
        }
    }
}

impl PackageGroupContents {
    pub fn exact_positive(&self) -> &SmallSet<PackageIdentifier> {
        &self.exact_positive
    }

    pub fn subtree_positive(&self) -> &[PackageIdentifier] {
        &self.subtree_positive
    }

    pub fn positive_all(&self) -> bool {
        self.positive_all
    }

    pub fn exact_negative(&self) -> &SmallSet<PackageIdentifier> {
        &self.exact_negative
    }

    pub fn subtree_negative(&self) -> &[PackageIdentifier] {
        &self.subtree_negative
    }

    pub fn has_private(&self) -> bool {
        self.has_private
    }

    pub fn contains_package(&self, package: &PackageIdentifier) -> bool {
        if self.exact_negative.contains(package)
            || self
                .subtree_negative
                .iter()
                .any(|prefix| package_is_beneath(package, prefix))
        {
            return false;
        }
        self.positive_all
            || self.exact_positive.contains(package)
            || self
                .subtree_positive
                .iter()
                .any(|prefix| package_is_beneath(package, prefix))
    }

    pub(crate) fn from_package_specs(specs: &[String]) -> anyhow::Result<Self> {
        let mut value = MutablePackageGroupContents::default();
        for spec in specs {
            value.add(spec)?;
        }
        Ok(value.freeze())
    }

    pub fn in_repository_context(&self, repo: &CanonicalRepoName) -> anyhow::Result<Self> {
        if repo.is_root() {
            anyhow::bail!("package specification destination repository must be nonroot");
        }
        let project = |package: &PackageIdentifier| {
            if package.repo() == repo {
                return Ok(package.clone());
            }
            if package.repo().is_root() {
                return Ok(PackageIdentifier::new(
                    repo.clone(),
                    package.package().clone(),
                ));
            }
            anyhow::bail!("package specification is not in repository {repo}: {package}");
        };
        Ok(Self {
            exact_positive: self
                .exact_positive
                .iter()
                .map(project)
                .collect::<anyhow::Result<SmallSet<_>>>()?,
            subtree_positive: self
                .subtree_positive
                .iter()
                .map(project)
                .collect::<anyhow::Result<Vec<_>>>()?
                .into(),
            positive_all: self.positive_all,
            exact_negative: self
                .exact_negative
                .iter()
                .map(project)
                .collect::<anyhow::Result<SmallSet<_>>>()?,
            subtree_negative: self
                .subtree_negative
                .iter()
                .map(project)
                .collect::<anyhow::Result<Vec<_>>>()?
                .into(),
            has_private: self.has_private,
        })
    }
}

#[derive(Default)]
struct MutablePackageGroupContents {
    exact_positive: SmallSet<PackageIdentifier>,
    subtree_positive: Vec<PackageIdentifier>,
    positive_all: bool,
    exact_negative: SmallSet<PackageIdentifier>,
    subtree_negative: Vec<PackageIdentifier>,
    has_private: bool,
}

impl MutablePackageGroupContents {
    fn add(&mut self, raw: &str) -> anyhow::Result<()> {
        let (negative, spec) = match raw.strip_prefix('-') {
            Some(spec) => (true, spec),
            None => (false, raw),
        };
        if negative && matches!(spec, "public" | "private") {
            anyhow::bail!("Cannot negate \"{spec}\" package specification");
        }
        if spec == "public" {
            self.positive_all = true;
            return Ok(());
        }
        if spec == "private" {
            self.has_private = true;
            return Ok(());
        }
        if !spec.starts_with("//") {
            if spec.starts_with('@') {
                anyhow::bail!("external repository package specifications are deferred: '{raw}'");
            }
            anyhow::bail!(
                "invalid package name '{spec}': must start with '//', '@', or be 'public' or 'private'"
            );
        }

        let (kind, package) = if spec == "//..." {
            (PackageSpecKind::Subtree, "//")
        } else if let Some(package) = spec.strip_suffix("/...") {
            (PackageSpecKind::Subtree, package)
        } else if let Some((package, target)) = spec.rsplit_once(':') {
            match target {
                "__pkg__" => (PackageSpecKind::Exact, package),
                "__subpackages__" => (PackageSpecKind::Subtree, package),
                _ => anyhow::bail!("invalid package name '{spec}'"),
            }
        } else {
            (PackageSpecKind::Exact, spec)
        };
        let package = package
            .strip_prefix("//")
            .expect("validated package specification");
        let label = CanonicalLabel::parse(&format!("@@//{package}:__pkg__"))
            .map_err(|error| anyhow::anyhow!("invalid package name '{spec}': {error}"))?;
        let package = label.package().clone();
        match (negative, kind) {
            (false, PackageSpecKind::Exact) => {
                self.exact_positive.insert(package);
            }
            (false, PackageSpecKind::Subtree) => self.subtree_positive.push(package),
            (true, PackageSpecKind::Exact) => {
                self.exact_negative.insert(package);
            }
            (true, PackageSpecKind::Subtree) => self.subtree_negative.push(package),
        }
        Ok(())
    }

    fn freeze(self) -> PackageGroupContents {
        PackageGroupContents {
            exact_positive: self.exact_positive,
            subtree_positive: self.subtree_positive.into(),
            positive_all: self.positive_all,
            exact_negative: self.exact_negative,
            subtree_negative: self.subtree_negative.into(),
            has_private: self.has_private,
        }
    }
}

#[derive(Clone, Copy)]
enum PackageSpecKind {
    Exact,
    Subtree,
}

fn is_special(label: &CanonicalLabel, target: &str) -> bool {
    label.package().repo().is_root()
        && label.package().package().as_str() == "visibility"
        && label.target().as_str() == target
}

fn direct_kind(label: &CanonicalLabel) -> Option<PackageSpecKind> {
    match label.target().as_str() {
        "__pkg__" => Some(PackageSpecKind::Exact),
        "__subpackages__" => Some(PackageSpecKind::Subtree),
        _ => None,
    }
}

fn apparent_label(label: &CanonicalLabel) -> String {
    let package = label.package().package().as_str();
    format!("//{package}:{}", label.target())
}

#[cfg(test)]
mod tests {
    use slug_identity_v2::PackagePath;

    use super::*;

    fn package(repo: &str, path: &str) -> PackageIdentifier {
        PackageIdentifier::new(
            CanonicalRepoName::new(repo).unwrap(),
            PackagePath::parse(path).unwrap(),
        )
    }

    #[test]
    fn repository_context_preserves_visibility_order_duplicates_and_rejects_invalid_directions() {
        let repo = CanonicalRepoName::new("dep+").unwrap();
        let group = CanonicalLabel::parse("@@//owner:friends").unwrap();
        let visibility = RuleVisibility::from_declared_labels([group.clone(), group]).unwrap();
        let projected = visibility.in_repository_context(&repo).unwrap();
        assert_eq!(
            projected.dependency_labels(),
            &[
                CanonicalLabel::parse("@@dep+//owner:friends").unwrap(),
                CanonicalLabel::parse("@@dep+//owner:friends").unwrap(),
            ]
        );
        assert_eq!(
            projected.raw_declared_labels(),
            projected.dependency_labels()
        );
        assert_eq!(projected.in_repository_context(&repo).unwrap(), projected);
        assert_eq!(
            RuleVisibility::Public.in_repository_context(&repo).unwrap(),
            RuleVisibility::Public
        );
        assert_eq!(
            RuleVisibility::Private
                .in_repository_context(&repo)
                .unwrap(),
            RuleVisibility::Private
        );

        let foreign = RuleVisibility::from_declared_labels([CanonicalLabel::parse(
            "@@other+//owner:friends",
        )
        .unwrap()])
        .unwrap();
        assert!(foreign.in_repository_context(&repo).is_err());
        assert!(
            visibility
                .in_repository_context(&CanonicalRepoName::root())
                .is_err()
        );
    }

    #[test]
    fn repository_context_preserves_every_package_group_contents_class() {
        let repo = CanonicalRepoName::new("dep+").unwrap();
        let contents = PackageGroupContents::from_package_specs(&[
            "//exact".to_owned(),
            "//tree/...".to_owned(),
            "-//tree/exact-blocked".to_owned(),
            "-//tree/blocked/...".to_owned(),
        ])
        .unwrap()
        .in_repository_context(&repo)
        .unwrap();
        assert!(contents.contains_package(&package("dep+", "exact")));
        assert!(contents.contains_package(&package("dep+", "tree/child")));
        assert!(!contents.contains_package(&package("dep+", "tree/exact-blocked")));
        assert!(!contents.contains_package(&package("dep+", "tree/blocked/child")));
        assert!(!contents.contains_package(&package("other+", "tree/child")));
        assert_eq!(contents.exact_positive().len(), 1);
        assert_eq!(contents.exact_negative().len(), 1);
        assert_eq!(contents.subtree_positive().len(), 1);
        assert_eq!(contents.subtree_negative().len(), 1);
        assert_eq!(contents.in_repository_context(&repo).unwrap(), contents);

        let public = PackageGroupContents::from_package_specs(&["public".to_owned()])
            .unwrap()
            .in_repository_context(&repo)
            .unwrap();
        assert!(public.positive_all());
        assert!(public.contains_package(&package("other+", "anywhere")));
        let private = PackageGroupContents::from_package_specs(&["private".to_owned()])
            .unwrap()
            .in_repository_context(&repo)
            .unwrap();
        assert!(private.has_private());
        assert!(!private.contains_package(&package("dep+", "anywhere")));

        let foreign = PackageGroupContents {
            exact_positive: [package("other+", "foreign")].into_iter().collect(),
            ..PackageGroupContents::default()
        };
        assert!(foreign.in_repository_context(&repo).is_err());
        assert!(
            PackageGroupContents::default()
                .in_repository_context(&CanonicalRepoName::root())
                .is_err()
        );
    }
}

fn package_is_beneath(package: &PackageIdentifier, prefix: &PackageIdentifier) -> bool {
    if package.repo() != prefix.repo() {
        return false;
    }
    let package = package.package().as_str();
    let prefix = prefix.package().as_str();
    prefix.is_empty()
        || package == prefix
        || package
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('/'))
}
