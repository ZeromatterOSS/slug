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
use std::collections::VecDeque;
use std::fmt;

use crate::BazelDep;
use crate::Directive;
use crate::ModuleFile;
use crate::ModuleHeader;

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

    fn from_header(header: &ModuleHeader) -> Self {
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
        let module = self
            .modules
            .values()
            .find(|module| module.canonical_repo == canonical_repo)?;
        Some(
            module
                .dependencies
                .iter()
                .map(|dep| (dep.apparent_repo_name.clone(), dep.canonical_repo.clone()))
                .collect(),
        )
    }
}

pub fn bazel_canonical_module_repo_name(module_name: &str) -> String {
    if module_name == "_main" {
        "_main".to_owned()
    } else {
        format!("{module_name}+")
    }
}

pub fn resolve_local_module_graph(
    root: &ModuleFile,
    local_modules: &BTreeMap<String, ModuleFile>,
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

    let mut queue: VecDeque<BazelDep> = bazel_deps(root).into_iter().collect();
    while let Some(dep) = queue.pop_front() {
        if let Some(existing) = discovered_by_name.get(&dep.name) {
            if existing.version != dep.version {
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
        if key.version != dep.version {
            return Err(format!(
                "local module {} declared version {}, but dependency requested {}",
                dep.name, key.version, dep.version
            ));
        }

        discovered_by_name.insert(dep.name.clone(), key.clone());
        sources.insert(
            key.clone(),
            ModuleSource::LocalPath {
                path: override_path.clone(),
            },
        );
        module_files.insert(key.clone(), module_file);
        queue.extend(bazel_deps(module_file));
    }

    let mut modules = BTreeMap::new();
    for (key, module_file) in module_files {
        let canonical_repo = if key == root_key {
            "_main".to_owned()
        } else {
            bazel_canonical_module_repo_name(&key.name)
        };
        let mut dependencies = Vec::new();
        for dep in bazel_deps(module_file) {
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

fn bazel_deps(module_file: &ModuleFile) -> Vec<BazelDep> {
    module_file
        .directives
        .iter()
        .filter_map(|directive| match directive {
            Directive::BazelDep(dep) => Some(dep.clone()),
            _ => None,
        })
        .collect()
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
