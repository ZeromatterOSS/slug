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
//! name. This module is Slug's bzlmod-level equivalent, with explicit typed
//! canonical labels at the mapping boundary.

use std::collections::BTreeMap;
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

/// Canonical bzlmod label.
///
/// Bazel distinguishes unambiguous canonical label syntax (`@@repo//pkg:target`)
/// from apparent label syntax (`@repo//pkg:target`). Keep that distinction in
/// the API so callsites must choose whether they need Bazel canonical form or
/// the legacy single-`@` storage form still used by some Slug paths.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalLabel {
    repo: CanonicalRepoName,
    package_and_target: String,
    package: String,
    target: String,
}

impl CanonicalLabel {
    pub fn new(repo: CanonicalRepoName, package_and_target: impl Into<String>) -> Self {
        let package_and_target = package_and_target.into();
        let (package, target) = split_package_and_target(&package_and_target)
            .unwrap_or(("", package_and_target.as_str()));
        let package = package.to_owned();
        let target = target.to_owned();
        Self {
            repo,
            package_and_target,
            package,
            target,
        }
    }

    pub fn repo(&self) -> &CanonicalRepoName {
        &self.repo
    }

    pub fn package(&self) -> &str {
        &self.package
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn package_and_target(&self) -> &str {
        &self.package_and_target
    }

    pub fn parse_storage_string(label: &str) -> Option<Self> {
        let parsed = ParsedAbsoluteLabel::parse(label)?;
        Some(parsed.to_canonical_label(CanonicalRepoName::from(parsed.repo)))
    }

    /// Render in Bazel's unambiguous canonical label form.
    pub fn to_unambiguous_string(&self) -> String {
        format!("@@{}//{}:{}", self.repo, self.package, self.target)
    }

    /// Render in the legacy Slug storage form.
    ///
    /// Prefer `to_unambiguous_string()` for new code unless the callsite is
    /// explicitly reading or writing legacy single-`@` data.
    pub fn to_legacy_storage_string(&self) -> String {
        format!("@{}//{}", self.repo, self.package_and_target)
    }

    pub fn into_legacy_storage_string(self) -> String {
        self.to_legacy_storage_string()
    }

    pub fn to_storage_string(&self) -> String {
        self.to_legacy_storage_string()
    }

    pub fn into_storage_string(self) -> String {
        self.into_legacy_storage_string()
    }
}

impl std::fmt::Display for CanonicalLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_unambiguous_string())
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
        let use_usage_overrides = parsed.module.name.is_empty()
            || module_name == root_module_name
            || module_name == "_main";

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
                        canonical_repo_for_extension_import_with_usage_overrides(
                            usage,
                            &owner_module,
                            &ext_name,
                            repo_name,
                            use_usage_overrides,
                        )
                        .canonical_name,
                    );
                }
                for (apparent_name, actual_name) in &import.repo_mapping {
                    entries.insert(
                        apparent_name.clone(),
                        canonical_repo_for_extension_import_with_usage_overrides(
                            usage,
                            &owner_module,
                            &ext_name,
                            actual_name,
                            use_usage_overrides,
                        )
                        .canonical_name,
                    );
                }
            }

            if use_usage_overrides {
                for (repo_name, actual_name) in &usage.repo_overrides {
                    let generated_canonical =
                        format!("{}+{}+{}", owner_module, ext_name, repo_name);
                    entries.insert(
                        generated_canonical,
                        CanonicalRepoName::new(actual_name.clone()),
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

    pub fn entries_as_strings(&self) -> BTreeMap<String, String> {
        self.entries
            .iter()
            .map(|(apparent, canonical)| (apparent.clone(), canonical.as_str().to_owned()))
            .collect()
    }

    /// Convert a label string to a canonical label in this repository-mapping context.
    ///
    /// Already-canonical labels (`@@repo//...`) and Slug extension canonical
    /// names (`@module+extension+repo//...`) are returned in single-`@` storage
    /// form without applying the apparent-name mapping again.
    pub fn canonicalize_label(&self, label: &str) -> Option<CanonicalLabel> {
        canonicalize_label_with_package_context(label, "", "", Some(self))
    }

    /// Canonicalize a label for legacy storage paths that still use raw strings.
    pub fn canonicalize_label_to_storage_string(&self, label: &str) -> String {
        self.canonicalize_label(label)
            .map(CanonicalLabel::into_storage_string)
            .unwrap_or_else(|| label.to_owned())
    }
}

/// Canonicalize a label string in a Bazel package context.
///
/// This mirrors the bzlmod-relevant part of Bazel's
/// `Label.parseWithPackageContext`: `@@repo` is already canonical, `@repo`
/// is mapped through the provided repository mapping, `//pkg` stays in the
/// current repository, and `:target` stays in the current package.
pub fn canonicalize_label_with_package_context(
    label: &str,
    current_repo: impl Into<CanonicalRepoName>,
    current_package: &str,
    repo_mapping: Option<&BzlmodRepoMapping>,
) -> Option<CanonicalLabel> {
    canonicalize_label_with_package_context_and_repo_resolver(
        label,
        current_repo,
        current_package,
        repo_mapping,
        |_| None,
    )
}

/// Canonicalize a label string in a Bazel package context, using a caller
/// supplied apparent-repository resolver when a full `BzlmodRepoMapping` is not
/// available at the callsite.
pub fn canonicalize_label_with_package_context_and_repo_resolver(
    label: &str,
    current_repo: impl Into<CanonicalRepoName>,
    current_package: &str,
    repo_mapping: Option<&BzlmodRepoMapping>,
    mut apparent_repo_resolver: impl FnMut(&str) -> Option<CanonicalRepoName>,
) -> Option<CanonicalLabel> {
    let current_repo = current_repo.into();
    let parsed = ParsedPackageContextLabel::parse(label, current_package)?;
    let canonical_repo = match parsed.repo {
        ParsedRepo::Current => current_repo,
        ParsedRepo::Canonical(repo) => CanonicalRepoName::new(repo),
        ParsedRepo::Apparent(repo) => {
            if let Some(mapping) = repo_mapping {
                mapping.canonical_repo_name(repo)
            } else if repo.contains('+') {
                CanonicalRepoName::new(repo)
            } else if let Some(repo) = apparent_repo_resolver(repo) {
                repo
            } else {
                CanonicalRepoName::new(repo)
            }
        }
    };
    Some(CanonicalLabel::new(
        canonical_repo,
        format!("{}:{}", parsed.package, parsed.target),
    ))
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
    canonical_repo_for_extension_import_with_usage_overrides(
        usage,
        owner_module,
        ext_name,
        internal_name,
        true,
    )
}

/// Canonical repository name for one repo imported from a module extension.
pub fn canonical_repo_for_extension_import_with_usage_overrides(
    usage: &ExtensionUsage,
    owner_module: &str,
    ext_name: &str,
    internal_name: &str,
    use_usage_overrides: bool,
) -> ExtensionImportCanonicalization {
    if use_usage_overrides {
        if let Some(dep_repo) =
            usage
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
    }

    ExtensionImportCanonicalization {
        canonical_name: CanonicalRepoName::new(format!(
            "{}+{}+{}",
            owner_module, ext_name, internal_name
        )),
        is_override: false,
    }
}

/// Add Bazel-shaped repo mappings for repositories generated by one module
/// extension.
///
/// Bazel computes the mapping for every repo generated by a given extension as
/// the owning module's full mapping, then the generated repos from the same
/// extension, then root-module `override_repo()` rows. Later entries win.
/// Returns `false` when the owning module mapping is unavailable, in which
/// case callers should conservatively treat replay as a miss.
pub fn add_extension_generated_repo_mappings(
    snapshot: &mut crate::RepoMappingSnapshot,
    extension_id: &str,
    root_module_name: &str,
    generated_repos: impl IntoIterator<Item = (String, String)>,
    repo_overrides: Option<&BTreeMap<String, String>>,
) -> bool {
    let owner_module = extract_owning_module(extension_id, root_module_name);
    let Some(owner_mapping) = owner_module_mapping(snapshot, &owner_module).cloned() else {
        return false;
    };

    let generated_repos: Vec<(String, String)> = generated_repos.into_iter().collect();
    if generated_repos.is_empty() {
        return true;
    }

    let mut entries = owner_mapping;
    for (internal_name, canonical_name) in &generated_repos {
        entries.insert(internal_name.clone(), canonical_name.clone());
    }
    if let Some(repo_overrides) = repo_overrides {
        for (internal_name, canonical_name) in repo_overrides {
            entries.insert(internal_name.clone(), canonical_name.clone());
        }
    }

    for (_, canonical_name) in generated_repos {
        snapshot.insert(canonical_name, entries.clone());
    }

    true
}

fn owner_module_mapping<'a>(
    snapshot: &'a crate::RepoMappingSnapshot,
    owner_module: &str,
) -> Option<&'a BTreeMap<String, String>> {
    if owner_module == "_main" {
        return snapshot.get("");
    }

    snapshot.get(owner_module).or_else(|| {
        owner_module
            .strip_suffix('+')
            .and_then(|name| snapshot.get(name))
    })
}

fn parsed_module_name<'a>(parsed: &'a ParsedModuleFile, root_module_name: &'a str) -> &'a str {
    if parsed.module.name.is_empty() {
        root_module_name
    } else {
        &parsed.module.name
    }
}

fn split_package_and_target(package_and_target: &str) -> Option<(&str, &str)> {
    if let Some((package, target)) = package_and_target.split_once(':') {
        return Some((package, target));
    }
    let target = package_and_target
        .rsplit('/')
        .next()
        .unwrap_or(package_and_target);
    Some((package_and_target, target))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedAbsoluteLabel<'a> {
    canonical: bool,
    repo: &'a str,
    rest: &'a str,
}

impl<'a> ParsedAbsoluteLabel<'a> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsedRepo<'a> {
    Current,
    Canonical(&'a str),
    Apparent(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedPackageContextLabel<'a> {
    repo: ParsedRepo<'a>,
    package: &'a str,
    target: &'a str,
}

impl<'a> ParsedPackageContextLabel<'a> {
    fn parse(label: &'a str, current_package: &'a str) -> Option<Self> {
        if let Some(rest) = label.strip_prefix("@@") {
            let Some((repo, rest)) = parse_repo_and_rest_or_shorthand(rest) else {
                return Some(Self {
                    repo: ParsedRepo::Canonical(rest),
                    package: "",
                    target: rest,
                });
            };
            let (package, target) = parse_package_and_target(rest)?;
            return Some(Self {
                repo: ParsedRepo::Canonical(repo),
                package,
                target,
            });
        }

        if let Some(rest) = label.strip_prefix('@') {
            let Some((repo, rest)) = parse_repo_and_rest_or_shorthand(rest) else {
                return Some(Self {
                    repo: ParsedRepo::Apparent(rest),
                    package: "",
                    target: rest,
                });
            };
            let (package, target) = parse_package_and_target(rest)?;
            return Some(Self {
                repo: ParsedRepo::Apparent(repo),
                package,
                target,
            });
        }

        if let Some(rest) = label.strip_prefix("//") {
            let (package, target) = parse_package_and_target(rest)?;
            return Some(Self {
                repo: ParsedRepo::Current,
                package,
                target,
            });
        }

        if let Some(target) = label.strip_prefix(':') {
            return Some(Self {
                repo: ParsedRepo::Current,
                package: current_package,
                target,
            });
        }

        if let Some((repo, rest)) = label.split_once("//") {
            if !repo.is_empty() {
                let (package, target) = parse_package_and_target(rest)?;
                return Some(Self {
                    repo: ParsedRepo::Apparent(repo),
                    package,
                    target,
                });
            }
        }

        None
    }
}

fn parse_repo_and_rest_or_shorthand(label_without_at: &str) -> Option<(&str, &str)> {
    label_without_at.split_once("//")
}

fn parse_package_and_target(rest: &str) -> Option<(&str, &str)> {
    if let Some((package, target)) = rest.split_once(':') {
        if target.is_empty() {
            return None;
        }
        return Some((package, target));
    }
    if rest.is_empty() {
        return None;
    }
    let target = rest.rsplit('/').next().unwrap_or(rest);
    Some((rest, target))
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
            "@bazel_lib++toolchains+coreutils_toolchains//:all"
        );
    }

    #[test]
    fn canonicalizes_keyword_use_repo_and_override_repo() {
        let mut module = parsed_module("root");
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
    fn non_root_override_repo_is_ignored_for_module_mapping() {
        let mut module = parsed_module("rules_owner");
        let mut usage = ExtensionUsage::new("//:extensions.bzl".to_owned(), "ext".to_owned());
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
            "@rules_owner++ext+generated_name//pkg:target"
        );
        assert_eq!(
            mapping
                .canonicalize_label("@rules_owner++ext+generated_name//pkg:target")
                .unwrap()
                .to_storage_string(),
            "@rules_owner++ext+generated_name//pkg:target"
        );
    }

    #[test]
    fn canonicalizes_override_generated_repo_name_to_selected_dep() {
        let mut module = parsed_module("root");
        let mut usage = ExtensionUsage::new(
            "@rules_rs//rs:extensions.bzl".to_owned(),
            "rules_rust".to_owned(),
        );
        usage
            .repo_overrides
            .push(("rules_rust".to_owned(), "rules_rust".to_owned()));
        module.extension_usages.push(usage);

        let mapping = BzlmodRepoMapping::for_module(&module, "root");

        assert_eq!(
            mapping
                .canonicalize_label("@rules_rs++rules_rust+rules_rust//rust:defs.bzl")
                .unwrap()
                .to_storage_string(),
            "@rules_rust//rust:defs.bzl"
        );
    }

    #[test]
    fn extension_generated_repo_mapping_shadows_owner_module_mapping() {
        let mut snapshot = crate::RepoMappingSnapshot::new();
        let mut owner_mapping = BTreeMap::new();
        owner_mapping.insert("dep".to_owned(), "owner_dep".to_owned());
        owner_mapping.insert("base".to_owned(), "base_canonical".to_owned());
        snapshot.insert("owner".to_owned(), owner_mapping);

        assert!(add_extension_generated_repo_mappings(
            &mut snapshot,
            "@owner//:ext.bzl%ext",
            "root",
            [
                ("dep".to_owned(), "owner++ext+dep".to_owned()),
                ("tool".to_owned(), "owner++ext+tool".to_owned()),
            ],
            None,
        ));

        let mapping = snapshot.get("owner++ext+tool").unwrap();
        assert_eq!(mapping.get("base").unwrap(), "base_canonical");
        assert_eq!(mapping.get("dep").unwrap(), "owner++ext+dep");
        assert_eq!(mapping.get("tool").unwrap(), "owner++ext+tool");
    }

    #[test]
    fn extension_generated_repo_mapping_applies_root_overrides_last() {
        let mut snapshot = crate::RepoMappingSnapshot::new();
        snapshot.insert("owner".to_owned(), BTreeMap::new());
        let mut overrides = BTreeMap::new();
        overrides.insert("generated".to_owned(), "actual_dep".to_owned());

        assert!(add_extension_generated_repo_mappings(
            &mut snapshot,
            "@owner//:ext.bzl%ext",
            "root",
            [("generated".to_owned(), "owner++ext+generated".to_owned())],
            Some(&overrides),
        ));

        let mapping = snapshot.get("owner++ext+generated").unwrap();
        assert_eq!(mapping.get("generated").unwrap(), "actual_dep");
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

    #[test]
    fn canonical_label_renderers_distinguish_bazel_and_legacy_forms() {
        let label = CanonicalLabel::new(CanonicalRepoName::new("rules_cc"), "cc:toolchain");

        assert_eq!(label.to_unambiguous_string(), "@@rules_cc//cc:toolchain");
        assert_eq!(label.to_legacy_storage_string(), "@rules_cc//cc:toolchain");
    }

    #[test]
    fn package_context_canonicalizes_current_repo_absolute_label() {
        let label =
            canonicalize_label_with_package_context("//tools:lock", "rules_rs", "ext", None)
                .unwrap();

        assert_eq!(label.to_unambiguous_string(), "@@rules_rs//tools:lock");
    }

    #[test]
    fn package_context_canonicalizes_package_relative_label() {
        let label = canonicalize_label_with_package_context(":lock", "rules_rs", "tools/ext", None)
            .unwrap();

        assert_eq!(label.to_unambiguous_string(), "@@rules_rs//tools/ext:lock");
    }

    #[test]
    fn package_context_keeps_unambiguous_canonical_repo() {
        let label = canonicalize_label_with_package_context(
            "@@rules_cc//cc:toolchain",
            "rules_rs",
            "ext",
            None,
        )
        .unwrap();

        assert_eq!(label.repo().as_str(), "rules_cc");
        assert_eq!(label.to_unambiguous_string(), "@@rules_cc//cc:toolchain");
    }

    #[test]
    fn package_context_maps_apparent_repo() {
        let mut module = parsed_module("owner");
        let mut dep = BazelDep::new("rules_cc".to_owned(), Version::empty());
        dep.repo_name = Some("cc_rules".to_owned());
        module.module.bazel_deps.push(dep);
        let mapping = BzlmodRepoMapping::for_module(&module, "root");

        let label = canonicalize_label_with_package_context(
            "@cc_rules//cc:toolchain",
            "owner",
            "ext",
            Some(&mapping),
        )
        .unwrap();

        assert_eq!(label.to_unambiguous_string(), "@@rules_cc//cc:toolchain");
    }

    #[test]
    fn package_context_uses_apparent_repo_resolver_for_shorthand_labels() {
        let label = canonicalize_label_with_package_context_and_repo_resolver(
            "@launcher",
            "owner",
            "ext",
            None,
            |repo| {
                (repo == "launcher")
                    .then(|| CanonicalRepoName::new("rules_python++python+launcher"))
            },
        )
        .unwrap();

        assert_eq!(
            label.to_unambiguous_string(),
            "@@rules_python++python+launcher//:launcher"
        );
    }

    #[test]
    fn package_context_supports_legacy_lockfile_repo_label_shape() {
        let label =
            canonicalize_label_with_package_context("rules_cc//cc:toolchain", "owner", "ext", None)
                .unwrap();

        assert_eq!(label.to_unambiguous_string(), "@@rules_cc//cc:toolchain");
    }

    #[test]
    fn package_context_supports_repo_shorthand() {
        let label =
            canonicalize_label_with_package_context("@rules_cc", "owner", "ext", None).unwrap();

        assert_eq!(label.to_unambiguous_string(), "@@rules_cc//:rules_cc");
    }

    #[test]
    fn package_context_rejects_explicit_empty_target_labels() {
        assert!(
            canonicalize_label_with_package_context("@rules_cc//:", "owner", "ext", None).is_none()
        );
        assert!(
            canonicalize_label_with_package_context("@@rules_cc//:", "owner", "ext", None)
                .is_none()
        );
        assert!(canonicalize_label_with_package_context("//:", "owner", "ext", None).is_none());
    }
}
