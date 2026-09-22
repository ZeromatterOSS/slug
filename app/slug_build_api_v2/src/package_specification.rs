/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Shared package-group membership and the native provider's retained closure.
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use starlark_map::small_set::SmallSet;

use crate::Depset;
use crate::DepsetOrder;

/// Loading-owned parsing feeds this owned construction input. No text parser or
/// repository discovery is part of the retained membership model.
#[derive(Debug, Default)]
pub struct PackageGroupContentsInput {
    pub exact_positive: SmallSet<PackageIdentifier>,
    pub subtree_positive: Arc<[PackageIdentifier]>,
    pub positive_all: bool,
    pub exact_negative: SmallSet<PackageIdentifier>,
    pub subtree_negative: Arc<[PackageIdentifier]>,
    pub has_private: bool,
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
    pub fn new(input: PackageGroupContentsInput) -> Self {
        Self {
            exact_positive: input.exact_positive,
            subtree_positive: input.subtree_positive,
            positive_all: input.positive_all,
            exact_negative: input.exact_negative,
            subtree_negative: input.subtree_negative,
            has_private: input.has_private,
        }
    }

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

    pub fn in_repository_context(&self, repo: &CanonicalRepoName) -> Result<Self, String> {
        if repo.is_root() {
            return Err("package specification destination repository must be nonroot".to_owned());
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
            Err(format!(
                "package specification is not in repository {repo}: {package}"
            ))
        };
        Ok(Self {
            exact_positive: self
                .exact_positive
                .iter()
                .map(project)
                .collect::<Result<SmallSet<_>, String>>()?,
            subtree_positive: self
                .subtree_positive
                .iter()
                .map(project)
                .collect::<Result<Vec<_>, String>>()?
                .into(),
            positive_all: self.positive_all,
            exact_negative: self
                .exact_negative
                .iter()
                .map(project)
                .collect::<Result<SmallSet<_>, String>>()?,
            subtree_negative: self
                .subtree_negative
                .iter()
                .map(project)
                .collect::<Result<Vec<_>, String>>()?
                .into(),
            has_private: self.has_private,
        })
    }
}

// Exact sets compare without order; subtree lists preserve native ordered
// payload equality. These hashes accelerate lookup, never replace equality.
impl Hash for PackageGroupContents {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_set(&self.exact_positive, state);
        self.subtree_positive.hash(state);
        self.positive_all.hash(state);
        hash_set(&self.exact_negative, state);
        self.subtree_negative.hash(state);
        self.has_private.hash(state);
    }
}

fn hash_set<H: Hasher>(values: &SmallSet<PackageIdentifier>, state: &mut H) {
    let mut sum = 0u64;
    let mut xor = 0u64;
    for value in values {
        let hash = fxhash::hash64(value);
        sum = sum.wrapping_add(hash);
        xor ^= hash.rotate_left(17);
    }
    values.len().hash(state);
    sum.hash(state);
    xor.hash(state);
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

/// Complete semantic payload; its Eq is for publication, not native Starlark
/// equality. NativeInfo has only a callable contains field, so its observable
/// equality/hash deliberately omit this hidden graph.
#[derive(Debug, Clone, Dupe, PartialEq, Eq, Allocative)]
pub struct PackageSpecificationInfo(Depset<Arc<PackageGroupContents>>);

impl PackageSpecificationInfo {
    pub fn new(
        contents: Arc<PackageGroupContents>,
        includes: impl IntoIterator<Item = Self>,
    ) -> Result<Self, String> {
        Depset::new(
            DepsetOrder::Default,
            vec![contents],
            includes.into_iter().map(|info| info.0).collect(),
        )
        .map(Self)
        .map_err(|error| error.to_string())
    }

    pub fn groups(&self) -> &Depset<Arc<PackageGroupContents>> {
        &self.0
    }

    pub fn contains_package(&self, package: &PackageIdentifier) -> bool {
        // Short-circuit using the existing DAG traversal, not a flattened copy
        // or a second membership engine. Negatives remain local to each group.
        self.0
            .visit(|contents| {
                if contents.contains_package(package) {
                    Err(())
                } else {
                    Ok(())
                }
            })
            .is_err()
    }
}

#[cfg(test)]
mod tests;
