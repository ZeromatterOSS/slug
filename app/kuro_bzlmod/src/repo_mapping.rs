/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

//! Bazel-style repository mapping for bzlmod labels.
//!
//! Bazel parses label strings in a package/module context containing a
//! `RepositoryMapping`; the resulting `Label` stores the canonical repository
//! name. This module is Kuro's bzlmod-level equivalent, with explicit typed
//! canonical labels at the mapping boundary.

use std::collections::HashMap;

use crate::extension_execution_dice::extract_extension_name;
use crate::extension_execution_dice::extract_owning_module;
use crate::extensions::canonical_extension_id;
use crate::types::ExtensionUsage;
use crate::types::ParsedModuleFile;

/// Repository mapping scoped to a single MODULE.bazel file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BzlmodRepoMapping {
    entries: HashMap<String, CanonicalRepoName>,
}

/// Canonical bzlmod repository name, without a leading `@`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalRepoName(String);

impl CanonicalRepoName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for CanonicalRepoName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for CanonicalRepoName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for CanonicalRepoName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// Canonical bzlmod label in single-`@` storage form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalLabel {
    repo: CanonicalRepoName,
    package_and_target: String,
}

impl CanonicalLabel {
    pub fn new(repo: CanonicalRepoName, package_and_target: impl Into<String>) -> Self {
        Self {
            repo,
            package_and_target: package_and_target.into(),
        }
    }

    pub fn repo(&self) -> &CanonicalRepoName {
        &self.repo
    }

    pub fn package_and_target(&self) -> &str {
        &self.package_and_target
    }

    pub fn parse_storage_string(label: &str) -> Option<Self> {
        let parsed = ParsedLabel::parse(label)?;
        Some(parsed.to_canonical_label(CanonicalRepoName::from(parsed.repo)))
    }

    pub fn to_storage_string(&self) -> String {
        format!("@{}//{}", self.repo, self.package_and_target)
    }

    pub fn into_storage_string(self) -> String {
        format!("@{}//{}", self.repo, self.package_and_target)
    }
}

impl std::fmt::Display for CanonicalLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}//{}", self.repo, self.package_and_target)
    }
}

impl BzlmodRepoMapping {
    /// Build the full repository mapping visible from a parsed MODULE.bazel.
    ///
    /// This mirrors Bazel's bzlmod mapping composition:
    /// - module `bazel_dep()` apparent names;
    /// - repositories imported with `use_repo()`;
    /// - `override_repo()` entries overriding generated extension repos.
    pub fn for_module(parsed: &ParsedModuleFile, root_module_name: &str) -> Self {
        let mut entries = HashMap::new();
        let module_name = parsed_module_name(parsed, root_module_name);

        for dep in &parsed.module.bazel_deps {
            entries.insert(
                dep.apparent_name().to_owned(),
                CanonicalRepoName::new(dep.name.clone()),
            );
        }

        for usage in &parsed.extension_usages {
            let ext_id = canonical_extension_id(
                &usage.extension_bzl_file,
                &usage.extension_name,
                module_name,
            );
            let ext_name = extract_extension_name(&ext_id);
            let owner_module = extract_owning_module(&ext_id, root_module_name);

            for import in &usage.imports {
                for repo_name in &import.repos {
                    entries.insert(
                        repo_name.clone(),
                        canonical_repo_for_extension_import(
                            usage,
                            &owner_module,
                            &ext_name,
                            repo_name,
                        )
                        .canonical_name,
                    );
                }
                for (apparent_name, actual_name) in &import.repo_mapping {
                    entries.insert(
                        apparent_name.clone(),
                        canonical_repo_for_extension_import(
                            usage,
                            &owner_module,
                            &ext_name,
                            actual_name,
                        )
                        .canonical_name,
                    );
                }
            }
        }

        Self { entries }
    }

    /// Resolve an apparent repository name to a canonical repository name.
    pub fn canonical_repo_name(&self, apparent: &str) -> CanonicalRepoName {
        self.entries
            .get(apparent)
            .cloned()
            .unwrap_or_else(|| CanonicalRepoName::new(apparent))
    }

    /// Convert a label string to a canonical label in this repository-mapping context.
    ///
    /// Already-canonical labels (`@@repo//...`) and Kuro extension canonical
    /// names (`@module+extension+repo//...`) are returned in single-`@` storage
    /// form without applying the apparent-name mapping again.
    pub fn canonicalize_label(&self, label: &str) -> Option<CanonicalLabel> {
        let parsed = ParsedLabel::parse(label)?;

        if parsed.canonical || parsed.repo.contains('+') {
            return Some(parsed.to_canonical_label(CanonicalRepoName::from(parsed.repo)));
        }

        let canonical_repo = self.canonical_repo_name(parsed.repo);
        Some(parsed.to_canonical_label(canonical_repo))
    }

    /// Canonicalize a label for legacy storage paths that still use raw strings.
    pub fn canonicalize_label_to_storage_string(&self, label: &str) -> String {
        self.canonicalize_label(label)
            .map(CanonicalLabel::into_storage_string)
            .unwrap_or_else(|| label.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionImportCanonicalization {
    pub canonical_name: CanonicalRepoName,
    pub is_override: bool,
}

/// Canonical repository name for one repo imported from a module extension.
pub fn canonical_repo_for_extension_import(
    usage: &ExtensionUsage,
    owner_module: &str,
    ext_name: &str,
    internal_name: &str,
) -> ExtensionImportCanonicalization {
    if let Some(dep_repo) = usage
        .repo_overrides
        .iter()
        .find_map(|(repo_in_extension, dep_repo)| {
            (repo_in_extension == internal_name).then_some(dep_repo.as_str())
        })
    {
        return ExtensionImportCanonicalization {
            canonical_name: CanonicalRepoName::new(dep_repo),
            is_override: true,
        };
    }

    ExtensionImportCanonicalization {
        canonical_name: CanonicalRepoName::new(format!(
            "{}+{}+{}",
            owner_module, ext_name, internal_name
        )),
        is_override: false,
    }
}

fn parsed_module_name<'a>(parsed: &'a ParsedModuleFile, root_module_name: &'a str) -> &'a str {
    if parsed.module.name.is_empty() {
        root_module_name
    } else {
        &parsed.module.name
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedLabel<'a> {
    canonical: bool,
    repo: &'a str,
    rest: &'a str,
}

impl<'a> ParsedLabel<'a> {
    fn parse(label: &'a str) -> Option<Self> {
        let (canonical, stripped) = if let Some(rest) = label.strip_prefix("@@") {
            (true, rest)
        } else if let Some(rest) = label.strip_prefix('@') {
            (false, rest)
        } else {
            return None;
        };
        let (repo, rest) = stripped.split_once("//")?;
        Some(Self {
            canonical,
            repo,
            rest,
        })
    }

    fn to_canonical_label(self, repo: CanonicalRepoName) -> CanonicalLabel {
        CanonicalLabel::new(repo, self.rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BazelDep;
    use crate::types::Module;
    use crate::types::UseRepo;
    use crate::version::Version;

    fn parsed_module(name: &str) -> ParsedModuleFile {
        ParsedModuleFile {
            module: Module::new(name.to_owned(), Version::empty()),
            has_module_directive: true,
            extension_usages: Vec::new(),
            repo_rule_invocations: Vec::new(),
            registered_toolchains: Vec::new(),
            registered_execution_platforms: Vec::new(),
        }
    }

    #[test]
    fn canonicalizes_module_scoped_use_repo_labels() {
        let mut module = parsed_module("bazel_lib");
        let mut usage = ExtensionUsage::new(
            "@bazel_lib//lib:extensions.bzl".to_owned(),
            "toolchains".to_owned(),
        );
        usage
            .imports
            .push(UseRepo::new().add_repo("coreutils_toolchains".to_owned()));
        module.extension_usages.push(usage);

        let mapping = BzlmodRepoMapping::for_module(&module, "zeromatter");

        assert_eq!(
            mapping
                .canonicalize_label("@coreutils_toolchains//:all")
                .unwrap()
                .to_storage_string(),
            "@bazel_lib+toolchains+coreutils_toolchains//:all"
        );
    }

    #[test]
    fn canonicalizes_keyword_use_repo_and_override_repo() {
        let mut module = parsed_module("rules_owner");
        let mut usage =
            ExtensionUsage::new("@rules_owner//:extensions.bzl".to_owned(), "ext".to_owned());
        usage.imports.push(
            UseRepo::new().add_mapping("public_name".to_owned(), "generated_name".to_owned()),
        );
        usage
            .repo_overrides
            .push(("generated_name".to_owned(), "actual_dep".to_owned()));
        module.extension_usages.push(usage);

        let mapping = BzlmodRepoMapping::for_module(&module, "root");

        assert_eq!(
            mapping
                .canonicalize_label("@public_name//pkg:target")
                .unwrap()
                .to_storage_string(),
            "@actual_dep//pkg:target"
        );
    }

    #[test]
    fn canonicalizes_bazel_dep_repo_name() {
        let mut module = parsed_module("owner");
        let mut dep = BazelDep::new("rules_cc".to_owned(), Version::empty());
        dep.repo_name = Some("cc_rules".to_owned());
        module.module.bazel_deps.push(dep);

        let mapping = BzlmodRepoMapping::for_module(&module, "root");

        assert_eq!(
            mapping
                .canonicalize_label("@cc_rules//cc:toolchain")
                .unwrap()
                .to_storage_string(),
            "@rules_cc//cc:toolchain"
        );
    }

    #[test]
    fn canonical_labels_are_not_remapped() {
        let mut module = parsed_module("owner");
        let mut dep = BazelDep::new("rules_cc".to_owned(), Version::empty());
        dep.repo_name = Some("cc_rules".to_owned());
        module.module.bazel_deps.push(dep);

        let mapping = BzlmodRepoMapping::for_module(&module, "root");

        assert_eq!(
            mapping
                .canonicalize_label("@@cc_rules//cc:toolchain")
                .unwrap()
                .to_storage_string(),
            "@cc_rules//cc:toolchain"
        );
    }

    #[test]
    fn canonical_label_exposes_typed_repo_name() {
        let mut module = parsed_module("owner");
        let mut dep = BazelDep::new("rules_cc".to_owned(), Version::empty());
        dep.repo_name = Some("cc_rules".to_owned());
        module.module.bazel_deps.push(dep);

        let mapping = BzlmodRepoMapping::for_module(&module, "root");
        let label = mapping
            .canonicalize_label("@cc_rules//cc:toolchain")
            .unwrap();

        assert_eq!(label.repo().as_str(), "rules_cc");
        assert_eq!(label.package_and_target(), "cc:toolchain");
    }
}
