/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::sync::RwLock;

use allocative::Allocative;
use dupe::Dupe;
use gazebo::prelude::SliceExt;
use kuro_core::pattern::pattern::ParsedPattern;
use kuro_core::pattern::pattern_type::TargetPatternExtra;
use kuro_core::target::label::label::TargetLabel;
use kuro_util::arc_str::ThinArcSlice;
use pagable::Pagable;

use crate::attrs::attr_type::any_matches::AnyMatches;

// ============================================================================
// Package Group Registry
// ============================================================================

/// A package specification from a package_group's `packages` attribute.
#[derive(Debug, Clone)]
pub enum PackageSpec {
    /// Match a single package exactly: `"//foo/bar"`
    Exact(String),
    /// Match a package and all subpackages: `"//foo/bar/..."`
    Recursive(String),
    /// Match all packages: `"//..."`
    AllPackages,
    /// Negate a spec: `"-//foo/bar"`
    Negated(Box<PackageSpec>),
    /// Public - visible to all
    Public,
    /// Private - visible to none
    Private,
}

impl PackageSpec {
    /// Parse a package specification string.
    pub fn parse(spec: &str) -> PackageSpec {
        if spec == "public" || spec == "//visibility:public" {
            return PackageSpec::Public;
        }
        if spec == "private" || spec == "//visibility:private" {
            return PackageSpec::Private;
        }
        if spec.starts_with('-') {
            return PackageSpec::Negated(Box::new(PackageSpec::parse(&spec[1..])));
        }
        if spec == "//..." {
            return PackageSpec::AllPackages;
        }
        if spec.ends_with("/...") {
            // "//foo/bar/..." -> prefix is "foo/bar"
            let prefix = spec.trim_start_matches("//").trim_end_matches("/...");
            return PackageSpec::Recursive(prefix.to_owned());
        }
        // "//foo/bar" -> exact match on "foo/bar"
        let pkg = spec.trim_start_matches("//");
        PackageSpec::Exact(pkg.to_owned())
    }

    /// Check if a package path matches this spec.
    fn matches_package_path(&self, pkg_path: &str) -> bool {
        match self {
            PackageSpec::Exact(p) => pkg_path == p,
            PackageSpec::Recursive(prefix) => {
                pkg_path == prefix.as_str()
                    || pkg_path.starts_with(&format!("{}/", prefix))
            }
            PackageSpec::AllPackages => true,
            PackageSpec::Public => true,
            PackageSpec::Private => false,
            PackageSpec::Negated(inner) => !inner.matches_package_path(pkg_path),
        }
    }
}

/// Registered package group data.
#[derive(Debug, Clone)]
pub struct PackageGroupData {
    pub packages: Vec<PackageSpec>,
    pub includes: Vec<String>,
}

static PACKAGE_GROUP_REGISTRY: RwLock<Option<HashMap<String, PackageGroupData>>> =
    RwLock::new(None);

/// Register a package_group with its packages and includes.
pub fn register_package_group(label: &str, packages: Vec<String>, includes: Vec<String>) {
    let specs: Vec<PackageSpec> = packages.iter().map(|s| PackageSpec::parse(s)).collect();
    let data = PackageGroupData {
        packages: specs,
        includes,
    };
    let mut registry = PACKAGE_GROUP_REGISTRY.write().unwrap();
    registry
        .get_or_insert_with(HashMap::new)
        .insert(label.to_owned(), data);
}

/// Check if a target's package matches a registered package_group.
/// Returns None if the label is not a registered package_group.
pub fn check_package_group(group_label: &str, target: &TargetLabel) -> Option<bool> {
    let registry = PACKAGE_GROUP_REGISTRY.read().unwrap();
    let registry = registry.as_ref()?;
    let data = registry.get(group_label)?;

    let pkg_path = target.pkg().cell_relative_path().as_str();

    // Check direct package specs
    let mut matched = false;
    for spec in &data.packages {
        match spec {
            PackageSpec::Negated(_) => {
                if !spec.matches_package_path(pkg_path) {
                    return Some(false);
                }
            }
            _ => {
                if spec.matches_package_path(pkg_path) {
                    matched = true;
                }
            }
        }
    }
    if matched {
        return Some(true);
    }

    // Check included package groups (transitively)
    for include_label in &data.includes {
        if let Some(true) = check_package_group(include_label, target) {
            return Some(true);
        }
    }

    Some(false)
}

#[derive(Debug, kuro_error::Error)]
pub enum VisibilityError {
    #[error(
        "`{0}` is not visible to `{1}` (run `kuro uquery --output-attribute visibility {0}` to check the visibility)"
    )]
    #[kuro(input, tag = Visibility)]
    NotVisibleTo(TargetLabel, TargetLabel),
}

#[derive(
    Debug,
    Eq,
    PartialEq,
    Hash,
    Clone,
    Allocative,
    derive_more::Display,
    Pagable
)]
pub struct VisibilityPattern(pub ParsedPattern<TargetPatternExtra>);

impl VisibilityPattern {
    pub const PUBLIC: &'static str = "PUBLIC";

    pub fn testing_new(pattern: &str) -> VisibilityPattern {
        VisibilityPattern(ParsedPattern::testing_parse(pattern))
    }
}

#[derive(derive_more::Display)]
#[display("\"{}\"", _0)]
struct VisibilityPatternQuoted<'a>(&'a VisibilityPattern);

#[derive(Debug, Eq, PartialEq, Hash, Clone, Dupe, Allocative, Pagable)]
pub enum VisibilityPatternList {
    Public,
    List(ThinArcSlice<VisibilityPattern>),
}

impl VisibilityPatternList {
    fn is_empty(&self) -> bool {
        match self {
            VisibilityPatternList::Public => false,
            VisibilityPatternList::List(patterns) => patterns.is_empty(),
        }
    }

    fn to_json(&self) -> serde_json::Value {
        let list = match self {
            VisibilityPatternList::Public => vec![serde_json::Value::String(
                VisibilityPattern::PUBLIC.to_owned(),
            )],
            VisibilityPatternList::List(patterns) => {
                patterns.map(|p| serde_json::Value::String(p.to_string()))
            }
        };
        serde_json::Value::Array(list)
    }

    fn extend_with(&self, other: &VisibilityPatternList) -> VisibilityPatternList {
        match (self, other) {
            (VisibilityPatternList::Public, _) | (_, VisibilityPatternList::Public) => {
                VisibilityPatternList::Public
            }
            (VisibilityPatternList::List(this), VisibilityPatternList::List(other)) => {
                VisibilityPatternList::List(this.iter().chain(other).cloned().collect())
            }
        }
    }

    fn testing_parse(patterns: &[&str]) -> VisibilityPatternList {
        if patterns.contains(&VisibilityPattern::PUBLIC) {
            VisibilityPatternList::Public
        } else {
            VisibilityPatternList::List(
                patterns
                    .iter()
                    .map(|p| VisibilityPattern::testing_new(p))
                    .collect(),
            )
        }
    }

    pub fn matches_target(&self, target: &TargetLabel) -> bool {
        match self {
            VisibilityPatternList::Public => true,
            VisibilityPatternList::List(patterns) => {
                for pattern in patterns {
                    // First try standard pattern matching (//pkg:__pkg__, //pkg/..., etc.)
                    if pattern.0.matches(target) {
                        return true;
                    }
                    // If the pattern is a Target (e.g. //some:package_group), check if
                    // it's a registered package_group and resolve it
                    if let ParsedPattern::Target(pkg, name, TargetPatternExtra) = &pattern.0 {
                        let group_label = format!(
                            "{}//{}:{}",
                            pkg.cell_name(),
                            pkg.cell_relative_path(),
                            name
                        );
                        // Also try without cell name for root cell
                        let group_label_short = format!(
                            "//{}:{}",
                            pkg.cell_relative_path(),
                            name
                        );
                        if let Some(true) = check_package_group(&group_label, target)
                            .or_else(|| check_package_group(&group_label_short, target))
                        {
                            return true;
                        }
                    }
                }
                false
            }
        }
    }
}

impl Display for VisibilityPatternList {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            VisibilityPatternList::Public => write!(f, "[\"{}\"]", VisibilityPattern::PUBLIC),
            VisibilityPatternList::List(patterns) => display_container::fmt_container(
                f,
                "[",
                "]",
                patterns.iter().map(VisibilityPatternQuoted),
            ),
        }
    }
}

impl AnyMatches for VisibilityPatternList {
    fn any_matches(
        &self,
        filter: &dyn Fn(&str) -> kuro_error::Result<bool>,
    ) -> kuro_error::Result<bool> {
        match self {
            VisibilityPatternList::Public => filter(VisibilityPattern::PUBLIC),
            VisibilityPatternList::List(patterns) => {
                for p in patterns {
                    if filter(&p.to_string())? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}

/// Represents the visibility spec of a target. Note that targets in the same package will ignore the
/// visibility spec of each other.
#[derive(Debug, Eq, PartialEq, Hash, Clone, Dupe, Allocative, Pagable)]
pub struct VisibilitySpecification(pub VisibilityPatternList);

impl Default for VisibilitySpecification {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Dupe, Allocative, Pagable)]
pub struct WithinViewSpecification(pub VisibilityPatternList);

impl Default for WithinViewSpecification {
    fn default() -> Self {
        Self::PUBLIC
    }
}

impl VisibilitySpecification {
    pub const DEFAULT: VisibilitySpecification =
        VisibilitySpecification(VisibilityPatternList::List(ThinArcSlice::empty()));

    pub(crate) fn to_json(&self) -> serde_json::Value {
        self.0.to_json()
    }

    pub fn extend_with(&self, other: &VisibilitySpecification) -> VisibilitySpecification {
        VisibilitySpecification(self.0.extend_with(&other.0))
    }

    pub fn testing_parse(patterns: &[&str]) -> VisibilitySpecification {
        VisibilitySpecification(VisibilityPatternList::testing_parse(patterns))
    }
}

impl Display for VisibilitySpecification {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl WithinViewSpecification {
    pub const PUBLIC: WithinViewSpecification =
        WithinViewSpecification(VisibilityPatternList::Public);

    pub fn extend_with(&self, other: &WithinViewSpecification) -> WithinViewSpecification {
        WithinViewSpecification(self.0.extend_with(&other.0))
    }

    pub fn to_json(&self) -> serde_json::Value {
        self.0.to_json()
    }
}

impl Display for WithinViewSpecification {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl AnyMatches for VisibilitySpecification {
    fn any_matches(
        &self,
        filter: &dyn Fn(&str) -> kuro_error::Result<bool>,
    ) -> kuro_error::Result<bool> {
        self.0.any_matches(filter)
    }
}

impl AnyMatches for WithinViewSpecification {
    fn any_matches(
        &self,
        filter: &dyn Fn(&str) -> kuro_error::Result<bool>,
    ) -> kuro_error::Result<bool> {
        self.0.any_matches(filter)
    }
}

pub struct VisibilityWithinViewBuilder {
    cap: usize,
    seen_public: bool,
    patterns: Option<Vec<VisibilityPattern>>,
}

impl VisibilityWithinViewBuilder {
    pub fn with_capacity(cap: usize) -> VisibilityWithinViewBuilder {
        VisibilityWithinViewBuilder {
            cap,
            seen_public: false,
            patterns: None,
        }
    }

    pub fn add_public(&mut self) {
        self.seen_public = true;
    }

    pub fn add(&mut self, pattern: VisibilityPattern) {
        if !self.seen_public {
            self.patterns
                .get_or_insert_with(|| Vec::with_capacity(self.cap))
                .push(pattern);
        }
    }

    fn build_list(self) -> VisibilityPatternList {
        if self.seen_public {
            VisibilityPatternList::Public
        } else {
            VisibilityPatternList::List(ThinArcSlice::from_iter(self.patterns.unwrap_or_default()))
        }
    }

    pub fn build_visibility(self) -> VisibilitySpecification {
        VisibilitySpecification(self.build_list())
    }

    pub fn build_within_view(self) -> WithinViewSpecification {
        let list = self.build_list();
        if list.is_empty() {
            WithinViewSpecification::PUBLIC
        } else {
            WithinViewSpecification(list)
        }
    }
}
