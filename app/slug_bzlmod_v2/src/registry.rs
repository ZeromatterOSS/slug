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

    let mut selected_versions: BTreeMap<String, String> = BTreeMap::new();
    let mut queue: VecDeque<BazelDep> = bazel_deps(root).into_iter().collect();
    while let Some(dep) = queue.pop_front() {
        let requested_key = ModuleKey::new(dep.name.clone(), dep.version.clone());
        if !registry_modules.contains_key(&requested_key) {
            return Err(format!("registry module {requested_key} was not supplied"));
        }

        let should_update = selected_versions
            .get(&dep.name)
            .is_none_or(|selected| compare_versions(&dep.version, selected).is_gt());
        if !should_update {
            continue;
        }

        selected_versions.insert(dep.name.clone(), dep.version.clone());
        let selected = registry_modules
            .get(&requested_key)
            .ok_or_else(|| format!("registry module {requested_key} was not supplied"))?;
        queue.extend(bazel_deps(&selected.module_file));
    }

    let mut modules = BTreeMap::new();
    modules.insert(
        root_key.clone(),
        ResolvedModule {
            key: root_key.clone(),
            canonical_repo: "_main".to_owned(),
            source: ModuleSource::Root,
            dependencies: resolve_dependencies(root, &selected_versions)?,
        },
    );

    for (name, version) in &selected_versions {
        let key = ModuleKey::new(name.clone(), version.clone());
        let registry_module = registry_modules
            .get(&key)
            .ok_or_else(|| format!("selected registry module {key} was not supplied"))?;
        modules.insert(
            key.clone(),
            ResolvedModule {
                key: key.clone(),
                canonical_repo: bazel_canonical_module_repo_name(name),
                source: ModuleSource::Registry {
                    registry_url: registry_module.registry_url.clone(),
                },
                dependencies: resolve_dependencies(
                    &registry_module.module_file,
                    &selected_versions,
                )?,
            },
        );
    }

    Ok(ResolvedGraph {
        root: root_key,
        modules,
    })
}

fn resolve_dependencies(
    module_file: &ModuleFile,
    selected_versions: &BTreeMap<String, String>,
) -> Result<Vec<ResolvedDependency>, String> {
    let mut dependencies = Vec::new();
    for dep in bazel_deps(module_file) {
        let selected_version = selected_versions
            .get(&dep.name)
            .ok_or_else(|| format!("dependency {} was not resolved", dep.name))?;
        dependencies.push(ResolvedDependency {
            apparent_repo_name: dep.repo_name.clone().unwrap_or_else(|| dep.name.clone()),
            module: ModuleKey::new(dep.name.clone(), selected_version.clone()),
            canonical_repo: bazel_canonical_module_repo_name(&dep.name),
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
