/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::fmt;

use serde_json::Value;

use crate::BazelDep;
use crate::BzlmodRepoMappingDigest;
use crate::Directive;
use crate::ModuleFile;
use crate::ModuleHeader;
use crate::digest_repo_mapping_entries;
use crate::digest_repo_mappings;
use crate::expand_included_module_files;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleKey {
    pub name: String,
    pub version: String,
}

impl ModuleKey {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }

    pub(crate) fn from_header(header: &ModuleHeader) -> Self {
        Self::new(
            header.name.clone(),
            header.version.clone().unwrap_or_default(),
        )
    }
}

impl fmt::Display for ModuleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.version)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleSource {
    Root,
    LocalPath { path: String },
    Registry { registry_url: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevDependencyMode {
    IncludeRoot,
    IgnoreRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DependencyOwner {
    Root,
    NonRoot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDependency {
    pub apparent_repo_name: String,
    pub module: ModuleKey,
    pub canonical_repo: String,
    pub dev_dependency: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModule {
    pub key: ModuleKey,
    pub canonical_repo: String,
    pub source: ModuleSource,
    pub dependencies: Vec<ResolvedDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGraph {
    pub root: ModuleKey,
    pub modules: BTreeMap<ModuleKey, ResolvedModule>,
}

impl ResolvedGraph {
    pub fn module(&self, key: &ModuleKey) -> Option<&ResolvedModule> {
        self.modules.get(key)
    }

    pub fn repo_mapping_for(&self, canonical_repo: &str) -> Option<BTreeMap<String, String>> {
        let module = self.module_by_canonical_repo(canonical_repo)?;
        Some(module_dependency_mapping(module))
    }

    pub fn bazel_repo_mapping_for(&self, canonical_repo: &str) -> Option<BTreeMap<String, String>> {
        let module = self.module_by_canonical_repo(canonical_repo)?;
        Some(bazel_module_repo_mapping(module))
    }

    pub fn module_repo_mapping_digests(&self) -> Result<Vec<BzlmodRepoMappingDigest>, String> {
        let mut digests = Vec::new();
        for module in self.modules.values() {
            digests.push(digest_repo_mapping_entries(
                module.canonical_repo.clone(),
                &bazel_module_repo_mapping(module),
            )?);
        }
        digests.sort();
        Ok(digests)
    }

    pub fn module_repo_mapping_digest(&self) -> Result<String, String> {
        digest_repo_mappings(self.module_repo_mapping_digests()?)
    }

    pub fn extension_generated_repo_mapping(
        &self,
        generated_repo_canonical: impl Into<String>,
        generated_repo_apparent: impl Into<String>,
    ) -> Result<BTreeMap<String, String>, String> {
        let root = self
            .modules
            .get(&self.root)
            .ok_or_else(|| format!("root module {} is missing from resolved graph", self.root))?;
        let mut mapping = BTreeMap::new();
        mapping.insert(
            generated_repo_apparent.into(),
            generated_repo_canonical.into(),
        );
        mapping.insert(String::new(), String::new());
        mapping.insert(root.key.name.clone(), String::new());
        mapping.extend(module_dependency_mapping(root));
        mapping.insert("bazel_tools".to_owned(), "bazel_tools".to_owned());
        Ok(mapping)
    }

    pub fn extension_generated_repo_mapping_digest(
        &self,
        generated_repo_canonical: impl Into<String>,
        generated_repo_apparent: impl Into<String>,
    ) -> Result<BzlmodRepoMappingDigest, String> {
        let generated_repo_canonical = generated_repo_canonical.into();
        let mapping = self.extension_generated_repo_mapping(
            generated_repo_canonical.clone(),
            generated_repo_apparent,
        )?;
        digest_repo_mapping_entries(generated_repo_canonical, &mapping)
    }

    fn module_by_canonical_repo(&self, canonical_repo: &str) -> Option<&ResolvedModule> {
        self.modules
            .values()
            .find(|module| module.canonical_repo == canonical_repo)
    }
}

fn module_dependency_mapping(module: &ResolvedModule) -> BTreeMap<String, String> {
    module
        .dependencies
        .iter()
        .map(|dep| (dep.apparent_repo_name.clone(), dep.canonical_repo.clone()))
        .collect()
}

fn bazel_module_repo_mapping(module: &ResolvedModule) -> BTreeMap<String, String> {
    let mut mapping = module_dependency_mapping(module);
    if matches!(module.source, ModuleSource::Root) {
        mapping.insert(String::new(), String::new());
        mapping.insert(module.key.name.clone(), String::new());
    } else {
        mapping.insert(module.key.name.clone(), module.canonical_repo.clone());
    }
    mapping.insert("bazel_tools".to_owned(), "bazel_tools".to_owned());
    mapping
}

pub fn parse_bazel_dump_repo_mapping_json_lines(
    output: &str,
) -> Result<Vec<BTreeMap<String, String>>, String> {
    let mut mappings = Vec::new();
    for (line_number, line) in output.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).map_err(|err| {
            format!(
                "unable to parse dump_repo_mapping JSON line {}: {}",
                line_number + 1,
                err
            )
        })?;
        let object = value.as_object().ok_or_else(|| {
            format!(
                "dump_repo_mapping JSON line {} must be an object",
                line_number + 1
            )
        })?;
        let mut mapping = BTreeMap::new();
        for (key, value) in object {
            let Some(value) = value.as_str() else {
                return Err(format!(
                    "dump_repo_mapping JSON line {} entry {} must be a string",
                    line_number + 1,
                    key
                ));
            };
            mapping.insert(key.clone(), value.to_owned());
        }
        mappings.push(mapping);
    }
    Ok(mappings)
}
pub fn bazel_canonical_module_repo_name(module_name: &str) -> String {
    if module_name == "_main" {
        "_main".to_owned()
    } else {
        format!("{module_name}+")
    }
}

pub fn resolve_local_module_graph_with_includes(
    root: &ModuleFile,
    included_modules: &BTreeMap<String, ModuleFile>,
    local_modules: &BTreeMap<String, ModuleFile>,
) -> Result<ResolvedGraph, String> {
    resolve_local_module_graph_with_includes_and_dev_dependency_mode(
        root,
        included_modules,
        local_modules,
        DevDependencyMode::IncludeRoot,
    )
}

pub fn resolve_local_module_graph_with_includes_and_dev_dependency_mode(
    root: &ModuleFile,
    included_modules: &BTreeMap<String, ModuleFile>,
    local_modules: &BTreeMap<String, ModuleFile>,
    dev_dependency_mode: DevDependencyMode,
) -> Result<ResolvedGraph, String> {
    let expanded = expand_included_module_files(root, included_modules)?;
    resolve_local_module_graph_with_dev_dependency_mode(
        &expanded,
        local_modules,
        dev_dependency_mode,
    )
}

pub fn resolve_local_module_graph(
    root: &ModuleFile,
    local_modules: &BTreeMap<String, ModuleFile>,
) -> Result<ResolvedGraph, String> {
    resolve_local_module_graph_with_dev_dependency_mode(
        root,
        local_modules,
        DevDependencyMode::IncludeRoot,
    )
}

pub fn resolve_local_module_graph_with_dev_dependency_mode(
    root: &ModuleFile,
    local_modules: &BTreeMap<String, ModuleFile>,
    dev_dependency_mode: DevDependencyMode,
) -> Result<ResolvedGraph, String> {
    let root_header = root
        .module
        .as_ref()
        .ok_or_else(|| "root MODULE.bazel is missing module()".to_owned())?;
    let root_key = ModuleKey::from_header(root_header);
    let local_overrides = local_path_overrides(root)?;
    let mut discovered_by_name = BTreeMap::new();
    discovered_by_name.insert(root_key.name.clone(), root_key.clone());

    let mut module_files = BTreeMap::new();
    module_files.insert(root_key.clone(), root);

    let mut sources = BTreeMap::new();
    sources.insert(root_key.clone(), ModuleSource::Root);

    let mut queue: VecDeque<BazelDep> =
        active_bazel_deps(root, DependencyOwner::Root, dev_dependency_mode)
            .into_iter()
            .collect();
    while let Some(dep) = queue.pop_front() {
        if let Some(existing) = discovered_by_name.get(&dep.name) {
            if existing.version != dep.version && !local_overrides.contains_key(&dep.name) {
                return Err(format!(
                    "multiple versions for module {} are not resolved by the local-only graph substrate",
                    dep.name
                ));
            }
            continue;
        }

        let override_path = local_overrides
            .get(&dep.name)
            .ok_or_else(|| format!("module {} has no local_path_override", dep.name))?;
        let module_file = local_modules
            .get(&dep.name)
            .ok_or_else(|| format!("local module {} was not supplied", dep.name))?;
        let header = module_file
            .module
            .as_ref()
            .ok_or_else(|| format!("local module {} is missing module()", dep.name))?;
        if header.name != dep.name {
            return Err(format!(
                "local module {} declared module name {}",
                dep.name, header.name
            ));
        }
        let key = ModuleKey::from_header(header);

        discovered_by_name.insert(dep.name.clone(), key.clone());
        sources.insert(
            key.clone(),
            ModuleSource::LocalPath {
                path: override_path.clone(),
            },
        );
        module_files.insert(key.clone(), module_file);
        queue.extend(active_bazel_deps(
            module_file,
            DependencyOwner::NonRoot,
            dev_dependency_mode,
        ));
    }

    let mut known_module_names: BTreeSet<String> = discovered_by_name.keys().cloned().collect();
    for module_file in module_files.values() {
        known_module_names.extend(bazel_deps(module_file).into_iter().map(|dep| dep.name));
    }
    validate_root_overrides_have_targets(root, &known_module_names)?;

    let mut modules = BTreeMap::new();
    for (key, module_file) in module_files {
        let canonical_repo = if key == root_key {
            "_main".to_owned()
        } else {
            bazel_canonical_module_repo_name(&key.name)
        };
        let mut dependencies = Vec::new();
        let dependency_owner = if key == root_key {
            DependencyOwner::Root
        } else {
            DependencyOwner::NonRoot
        };
        for dep in active_bazel_deps(module_file, dependency_owner, dev_dependency_mode) {
            let dep_key = discovered_by_name
                .get(&dep.name)
                .ok_or_else(|| format!("dependency {} was not resolved", dep.name))?;
            dependencies.push(ResolvedDependency {
                apparent_repo_name: dep.repo_name.clone().unwrap_or_else(|| dep.name.clone()),
                module: dep_key.clone(),
                canonical_repo: bazel_canonical_module_repo_name(&dep.name),
                dev_dependency: dep.dev_dependency,
            });
        }
        modules.insert(
            key.clone(),
            ResolvedModule {
                key: key.clone(),
                canonical_repo,
                source: sources
                    .remove(&key)
                    .ok_or_else(|| format!("module source missing for {key}"))?,
                dependencies,
            },
        );
    }

    Ok(ResolvedGraph {
        root: root_key,
        modules,
    })
}

pub(crate) fn bazel_deps(module_file: &ModuleFile) -> Vec<BazelDep> {
    module_file
        .directives
        .iter()
        .filter_map(|directive| match directive {
            Directive::BazelDep(dep) => Some(dep.clone()),
            _ => None,
        })
        .collect()
}

pub(crate) fn validate_root_overrides_have_targets(
    root: &ModuleFile,
    known_module_names: &BTreeSet<String>,
) -> Result<(), String> {
    let mut missing = Vec::new();
    let mut seen = BTreeSet::new();

    for directive in &root.directives {
        let override_module = match directive {
            Directive::LocalPathOverride(override_) => Some(&override_.module_name),
            Directive::SingleVersionOverride(override_) => Some(&override_.module_name),
            Directive::MultipleVersionOverride(override_) => Some(&override_.module_name),
            Directive::ArchiveOverride(override_) => Some(&override_.module_name),
            Directive::GitOverride(override_) => Some(&override_.module_name),
            _ => None,
        };
        let Some(override_module) = override_module else {
            continue;
        };
        if !known_module_names.contains(override_module) && seen.insert(override_module.clone()) {
            missing.push(override_module.clone());
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "root module specifies overrides on nonexistent module(s): {}",
            missing.join(", ")
        ))
    }
}

pub(crate) fn active_bazel_deps(
    module_file: &ModuleFile,
    owner: DependencyOwner,
    dev_dependency_mode: DevDependencyMode,
) -> Vec<BazelDep> {
    bazel_deps(module_file)
        .into_iter()
        .filter(|dep| active_bazel_dep(dep, owner, dev_dependency_mode))
        .collect()
}

fn active_bazel_dep(
    dep: &BazelDep,
    owner: DependencyOwner,
    dev_dependency_mode: DevDependencyMode,
) -> bool {
    !dep.dev_dependency
        || (matches!(owner, DependencyOwner::Root)
            && matches!(dev_dependency_mode, DevDependencyMode::IncludeRoot))
}

fn local_path_overrides(module_file: &ModuleFile) -> Result<BTreeMap<String, String>, String> {
    let mut overrides = BTreeMap::new();
    for directive in &module_file.directives {
        let Directive::LocalPathOverride(override_) = directive else {
            continue;
        };
        if overrides
            .insert(override_.module_name.clone(), override_.path.clone())
            .is_some()
        {
            return Err(format!(
                "duplicate local_path_override for module {}",
                override_.module_name
            ));
        }
    }
    Ok(overrides)
}
