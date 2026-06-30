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
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;

use serde_json::Value;

use crate::BazelDep;
use crate::Directive;
use crate::ModuleFile;
use crate::resolution::ModuleKey;
use crate::resolution::ModuleSource;
use crate::resolution::ResolvedDependency;
use crate::resolution::ResolvedGraph;
use crate::resolution::ResolvedModule;
use crate::resolution::bazel_canonical_module_repo_name;
use crate::resolution::bazel_deps;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryModule {
    pub registry_url: String,
    pub module_file: ModuleFile,
}

impl RegistryModule {
    pub fn new(registry_url: impl Into<String>, module_file: ModuleFile) -> Self {
        Self {
            registry_url: registry_url.into(),
            module_file,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryMetadata {
    pub homepage: Option<String>,
    pub repository: Vec<String>,
    pub versions: Vec<String>,
    pub yanked_versions: BTreeMap<String, String>,
}

impl RegistryMetadata {
    pub fn yanked_version_entries(&self, module_name: &str) -> BTreeMap<ModuleKey, String> {
        self.yanked_versions
            .iter()
            .map(|(version, reason)| (ModuleKey::new(module_name, version), reason.clone()))
            .collect()
    }
}

pub fn parse_registry_metadata_json(
    module_name: &str,
    content: &str,
) -> Result<RegistryMetadata, String> {
    let value: Value = serde_json::from_str(content).map_err(|err| {
        format!("Unable to parse json at url metadata.json for module {module_name}: {err}")
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("metadata.json for module {module_name} must be a JSON object"))?;

    let versions = metadata_string_list_field(module_name, object, "versions")?
        .ok_or_else(|| format!("metadata.json for module {module_name} is missing versions"))?;

    Ok(RegistryMetadata {
        homepage: metadata_optional_string_field(module_name, object, "homepage")?,
        repository: metadata_string_or_list_field(module_name, object, "repository")?
            .unwrap_or_default(),
        versions,
        yanked_versions: metadata_string_map_field(module_name, object, "yanked_versions")?
            .unwrap_or_default(),
    })
}

fn metadata_optional_string_field(
    module_name: &str,
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<String>, String> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!(
            "metadata.json field {name} for module {module_name} must be a string"
        )),
    }
}

fn metadata_string_or_list_field(
    module_name: &str,
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<Vec<String>>, String> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(vec![value.clone()])),
        Some(Value::Array(items)) => Ok(Some(metadata_items_to_strings(module_name, name, items)?)),
        Some(_) => Err(format!(
            "metadata.json field {name} for module {module_name} must be a string or list of strings"
        )),
    }
}

fn metadata_string_list_field(
    module_name: &str,
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<Vec<String>>, String> {
    let Some(value) = object.get(name) else {
        return Ok(None);
    };
    let Value::Array(items) = value else {
        return Err(format!(
            "metadata.json field {name} for module {module_name} must be a list of strings"
        ));
    };
    Ok(Some(metadata_items_to_strings(module_name, name, items)?))
}

fn metadata_items_to_strings(
    module_name: &str,
    name: &str,
    items: &[Value],
) -> Result<Vec<String>, String> {
    let mut strings = Vec::with_capacity(items.len());
    for item in items {
        let Value::String(value) = item else {
            return Err(format!(
                "metadata.json field {name} for module {module_name} must be a list of strings"
            ));
        };
        strings.push(value.clone());
    }
    Ok(strings)
}

fn metadata_string_map_field(
    module_name: &str,
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<BTreeMap<String, String>>, String> {
    let Some(value) = object.get(name) else {
        return Ok(None);
    };
    let Value::Object(entries) = value else {
        return Err(format!(
            "metadata.json field {name} for module {module_name} must be a string map"
        ));
    };

    let mut map = BTreeMap::new();
    for (key, value) in entries {
        let Value::String(value) = value else {
            return Err(format!(
                "metadata.json field {name} for module {module_name} must be a string map"
            ));
        };
        map.insert(key.clone(), value.clone());
    }
    Ok(Some(map))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrySourceSpec {
    pub urls: Vec<String>,
    pub integrity: String,
    pub source_type: Option<String>,
    pub strip_prefix: Option<String>,
    pub patches: BTreeMap<String, String>,
    pub patch_strip: Option<u64>,
}

pub fn parse_registry_source_json(
    module: &ModuleKey,
    content: &str,
) -> Result<RegistrySourceSpec, String> {
    let value: Value = serde_json::from_str(content).map_err(|err| {
        format!("Unable to parse json at url source.json for module {module}: {err}")
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("source.json for module {module} must be a JSON object"))?;

    let mut urls = Vec::new();
    if let Some(url) = optional_string_field(module, object, "url")? {
        urls.push(url);
    }
    if let Some(mut url_list) = optional_string_list_field(module, object, "urls")? {
        urls.append(&mut url_list);
    }
    if urls.is_empty() {
        return Err(format!("Missing source URL for module {module}"));
    }

    let integrity = optional_string_field(module, object, "integrity")?
        .ok_or_else(|| format!("Missing integrity for module {module}"))?;

    Ok(RegistrySourceSpec {
        urls,
        integrity,
        source_type: optional_string_field(module, object, "type")?,
        strip_prefix: optional_string_field(module, object, "strip_prefix")?,
        patches: optional_string_map_field(module, object, "patches")?.unwrap_or_default(),
        patch_strip: optional_u64_field(module, object, "patch_strip")?,
    })
}

fn optional_string_field(
    module: &ModuleKey,
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<String>, String> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!(
            "source.json field {name} for module {module} must be a string"
        )),
    }
}

fn optional_string_list_field(
    module: &ModuleKey,
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<Vec<String>>, String> {
    let Some(value) = object.get(name) else {
        return Ok(None);
    };
    let Value::Array(items) = value else {
        return Err(format!(
            "source.json field {name} for module {module} must be a list of strings"
        ));
    };

    let mut strings = Vec::with_capacity(items.len());
    for item in items {
        let Value::String(value) = item else {
            return Err(format!(
                "source.json field {name} for module {module} must be a list of strings"
            ));
        };
        strings.push(value.clone());
    }
    Ok(Some(strings))
}

fn optional_string_map_field(
    module: &ModuleKey,
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<BTreeMap<String, String>>, String> {
    let Some(value) = object.get(name) else {
        return Ok(None);
    };
    let Value::Object(entries) = value else {
        return Err(format!(
            "source.json field {name} for module {module} must be a string map"
        ));
    };

    let mut map = BTreeMap::new();
    for (key, value) in entries {
        let Value::String(value) = value else {
            return Err(format!(
                "source.json field {name} for module {module} must be a string map"
            ));
        };
        map.insert(key.clone(), value.clone());
    }
    Ok(Some(map))
}

fn optional_u64_field(
    module: &ModuleKey,
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<u64>, String> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_u64()
            .ok_or_else(|| {
                format!(
                    "source.json field {name} for module {module} must be a non-negative integer"
                )
            })
            .map(Some),
        Some(_) => Err(format!(
            "source.json field {name} for module {module} must be a non-negative integer"
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YankedVersionPolicy {
    Reject,
    AllowAll,
    AllowList(BTreeSet<ModuleKey>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedYankedVersion {
    pub module: ModuleKey,
    pub reason: String,
}

pub fn validate_yanked_versions(
    graph: &ResolvedGraph,
    yanked_versions: &BTreeMap<ModuleKey, String>,
    policy: &YankedVersionPolicy,
) -> Result<Vec<SelectedYankedVersion>, String> {
    let mut selected = Vec::new();
    for key in graph.modules.keys() {
        let Some(reason) = yanked_versions.get(key) else {
            continue;
        };
        if !policy.allows(key) {
            return Err(format!(
                "Yanked version detected in your resolved dependency graph: {key}, for the reason: {reason}"
            ));
        }
        selected.push(SelectedYankedVersion {
            module: key.clone(),
            reason: reason.clone(),
        });
    }
    Ok(selected)
}

impl YankedVersionPolicy {
    fn allows(&self, key: &ModuleKey) -> bool {
        match self {
            Self::Reject => false,
            Self::AllowAll => true,
            Self::AllowList(allowed) => allowed.contains(key),
        }
    }
}

pub fn resolve_registry_mvs(
    root: &ModuleFile,
    registry_modules: &BTreeMap<ModuleKey, RegistryModule>,
) -> Result<ResolvedGraph, String> {
    let root_header = root
        .module
        .as_ref()
        .ok_or_else(|| "root MODULE.bazel is missing module()".to_owned())?;
    let root_key = ModuleKey::from_header(root_header);
    let multiple_overrides = multiple_version_overrides(root)?;

    let mut selected_versions: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut queue: VecDeque<BazelDep> = bazel_deps(root).into_iter().collect();
    while let Some(dep) = queue.pop_front() {
        let requested_key = ModuleKey::new(dep.name.clone(), dep.version.clone());
        if !registry_modules.contains_key(&requested_key) {
            return Err(format!("registry module {requested_key} was not supplied"));
        }

        let changed = if let Some(allowed_versions) = multiple_overrides.get(&dep.name) {
            if !allowed_versions.contains(&dep.version) {
                return Err(format!(
                    "multiple_version_override for module {} does not allow requested version {}",
                    dep.name, dep.version
                ));
            }
            selected_versions
                .entry(dep.name.clone())
                .or_default()
                .insert(dep.version.clone())
        } else {
            let versions = selected_versions.entry(dep.name.clone()).or_default();
            let selected = versions.iter().next().cloned();
            if selected
                .as_deref()
                .is_none_or(|selected| compare_versions(&dep.version, selected).is_gt())
            {
                versions.clear();
                versions.insert(dep.version.clone());
                true
            } else {
                false
            }
        };

        if changed {
            let selected = registry_modules
                .get(&requested_key)
                .ok_or_else(|| format!("registry module {requested_key} was not supplied"))?;
            queue.extend(bazel_deps(&selected.module_file));
        }
    }

    let canonical_repos = canonical_repos_for_selected_versions(&selected_versions);
    let mut modules = BTreeMap::new();
    modules.insert(
        root_key.clone(),
        ResolvedModule {
            key: root_key.clone(),
            canonical_repo: "_main".to_owned(),
            source: ModuleSource::Root,
            dependencies: resolve_dependencies(root, &selected_versions, &canonical_repos)?,
        },
    );

    for (name, versions) in &selected_versions {
        for version in versions {
            let key = ModuleKey::new(name.clone(), version.clone());
            let registry_module = registry_modules
                .get(&key)
                .ok_or_else(|| format!("selected registry module {key} was not supplied"))?;
            modules.insert(
                key.clone(),
                ResolvedModule {
                    key: key.clone(),
                    canonical_repo: canonical_repos
                        .get(&key)
                        .ok_or_else(|| format!("canonical repo missing for {key}"))?
                        .clone(),
                    source: ModuleSource::Registry {
                        registry_url: registry_module.registry_url.clone(),
                    },
                    dependencies: resolve_dependencies(
                        &registry_module.module_file,
                        &selected_versions,
                        &canonical_repos,
                    )?,
                },
            );
        }
    }

    Ok(ResolvedGraph {
        root: root_key,
        modules,
    })
}

fn multiple_version_overrides(
    root: &ModuleFile,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut overrides = BTreeMap::new();
    for directive in &root.directives {
        let Directive::MultipleVersionOverride(override_) = directive else {
            continue;
        };
        if override_.versions.len() < 2 {
            return Err(format!(
                "multiple_version_override for module {} must specify at least two versions",
                override_.module_name
            ));
        }
        let versions: BTreeSet<String> = override_.versions.iter().cloned().collect();
        if versions.len() != override_.versions.len() {
            return Err(format!(
                "multiple_version_override for module {} contains duplicate versions",
                override_.module_name
            ));
        }
        if overrides
            .insert(override_.module_name.clone(), versions)
            .is_some()
        {
            return Err(format!(
                "duplicate multiple_version_override for module {}",
                override_.module_name
            ));
        }
    }
    Ok(overrides)
}

fn canonical_repos_for_selected_versions(
    selected_versions: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<ModuleKey, String> {
    let mut canonical_repos = BTreeMap::new();
    for (name, versions) in selected_versions {
        for version in versions {
            let repo = if versions.len() > 1 {
                format!("{name}+{version}")
            } else {
                bazel_canonical_module_repo_name(name)
            };
            canonical_repos.insert(ModuleKey::new(name.clone(), version.clone()), repo);
        }
    }
    canonical_repos
}

fn resolve_dependencies(
    module_file: &ModuleFile,
    selected_versions: &BTreeMap<String, BTreeSet<String>>,
    canonical_repos: &BTreeMap<ModuleKey, String>,
) -> Result<Vec<ResolvedDependency>, String> {
    let mut dependencies = Vec::new();
    for dep in bazel_deps(module_file) {
        let versions = selected_versions
            .get(&dep.name)
            .ok_or_else(|| format!("dependency {} was not resolved", dep.name))?;
        let selected_version = if versions.contains(&dep.version) {
            dep.version.clone()
        } else if versions.len() == 1 {
            versions.iter().next().unwrap().clone()
        } else {
            return Err(format!(
                "dependency {}@{} was not selected by multiple_version_override",
                dep.name, dep.version
            ));
        };
        let module = ModuleKey::new(dep.name.clone(), selected_version);
        let canonical_repo = canonical_repos
            .get(&module)
            .ok_or_else(|| format!("canonical repo missing for {module}"))?
            .clone();
        dependencies.push(ResolvedDependency {
            apparent_repo_name: dep.repo_name.clone().unwrap_or_else(|| dep.name.clone()),
            module,
            canonical_repo,
            dev_dependency: dep.dev_dependency,
        });
    }
    Ok(dependencies)
}

fn compare_versions(left: &str, right: &str) -> Ordering {
    let left_parts = version_parts(left);
    let right_parts = version_parts(right);
    for index in 0..left_parts.len().max(right_parts.len()) {
        match (left_parts.get(index), right_parts.get(index)) {
            (Some(left), Some(right)) => match compare_version_part(left, right) {
                Ordering::Equal => {}
                ordering => return ordering,
            },
            (Some(left), None) if is_zero_part(left) => {}
            (None, Some(right)) if is_zero_part(right) => {}
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => return Ordering::Equal,
        }
    }
    Ordering::Equal
}

fn version_parts(version: &str) -> Vec<&str> {
    version
        .split(|c: char| matches!(c, '.' | '-' | '_'))
        .filter(|part| !part.is_empty())
        .collect()
}

fn compare_version_part(left: &str, right: &str) -> Ordering {
    match (left.parse::<u64>(), right.parse::<u64>()) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

fn is_zero_part(part: &str) -> bool {
    part.parse::<u64>() == Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numeric_version_segments() {
        assert!(compare_versions("2.0.0", "1.0.0").is_gt());
        assert!(compare_versions("1.10.0", "1.2.0").is_gt());
        assert!(compare_versions("1.0.0", "1").is_eq());
    }
}
