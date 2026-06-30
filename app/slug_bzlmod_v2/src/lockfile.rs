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

use serde_json::Value;

use crate::ModuleKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BazelLockfile {
    pub lock_file_version: u64,
    pub registry_file_hashes: BTreeMap<String, String>,
    pub selected_yanked_versions: BTreeMap<ModuleKey, String>,
    pub module_extensions: BTreeMap<String, BazelLockfileModuleExtension>,
    pub facts: BTreeMap<String, Value>,
    pub facts_versions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BazelLockfileModuleExtension {
    pub general: Option<BazelLockfileModuleExtensionGeneral>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BazelLockfileModuleExtensionGeneral {
    pub bzl_transitive_digest: Option<String>,
    pub usages_digest: Option<String>,
    pub recorded_inputs: Vec<Value>,
    pub generated_repo_specs: BTreeMap<String, BazelLockfileRepoSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BazelLockfileRepoSpec {
    pub repo_rule_id: String,
    pub attributes: BTreeMap<String, Value>,
}

pub fn parse_bazel_lockfile(content: &str) -> Result<BazelLockfile, String> {
    let value: Value = serde_json::from_str(content)
        .map_err(|err| format!("Unable to parse MODULE.bazel.lock: {err}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "MODULE.bazel.lock must be a JSON object".to_owned())?;

    let lock_file_version = object
        .get("lockFileVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| "MODULE.bazel.lock is missing numeric lockFileVersion".to_owned())?;

    Ok(BazelLockfile {
        lock_file_version,
        registry_file_hashes: optional_string_map(object, "registryFileHashes")?,
        selected_yanked_versions: parse_selected_yanked_versions(object)?,
        module_extensions: parse_module_extensions(object)?,
        facts: optional_value_map(object, "facts")?,
        facts_versions: optional_value_map(object, "factsVersions")?,
    })
}

pub fn validate_registry_file_hashes(
    lockfile: &BazelLockfile,
    observed_hashes: &BTreeMap<String, String>,
) -> Result<(), String> {
    for (url, expected_hash) in &lockfile.registry_file_hashes {
        match observed_hashes.get(url) {
            Some(actual_hash) if actual_hash == expected_hash => {}
            Some(actual_hash) => {
                return Err(format!(
                    "Failed to fetch registry file {url}: Checksum was {actual_hash} but wanted {expected_hash}"
                ));
            }
            None => {
                return Err(format!(
                    "Failed to fetch registry file {url}: missing observed registry file hash"
                ));
            }
        }
    }
    Ok(())
}

pub fn validate_module_extension_usage_digests(
    lockfile: &BazelLockfile,
    observed_usage_digests: &BTreeMap<String, String>,
) -> Result<(), String> {
    for (extension_id, extension) in &lockfile.module_extensions {
        let Some(general) = &extension.general else {
            continue;
        };
        let Some(expected_digest) = &general.usages_digest else {
            continue;
        };
        match observed_usage_digests.get(extension_id) {
            Some(actual_digest) if actual_digest == expected_digest => {}
            Some(_) | None => {
                return Err(format!(
                    "MODULE.bazel.lock is no longer up-to-date because the usages of the extension '{}' have changed. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile.",
                    bazel_display_extension_id(extension_id)
                ));
            }
        }
    }
    Ok(())
}

fn bazel_display_extension_id(extension_id: &str) -> String {
    if extension_id.starts_with("@@") {
        extension_id.to_owned()
    } else if let Some(rest) = extension_id.strip_prefix('@') {
        format!("@@{rest}")
    } else {
        format!("@@{extension_id}")
    }
}

fn optional_string_map(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<BTreeMap<String, String>, String> {
    let Some(value) = object.get(field) else {
        return Ok(BTreeMap::new());
    };
    let Value::Object(entries) = value else {
        return Err(format!("MODULE.bazel.lock field {field} must be an object"));
    };
    let mut result = BTreeMap::new();
    for (key, value) in entries {
        let Value::String(text) = value else {
            return Err(format!(
                "MODULE.bazel.lock field {field} entry {key} must be a string"
            ));
        };
        result.insert(key.clone(), text.clone());
    }
    Ok(result)
}

fn optional_value_map(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<BTreeMap<String, Value>, String> {
    let Some(value) = object.get(field) else {
        return Ok(BTreeMap::new());
    };
    let Value::Object(entries) = value else {
        return Err(format!("MODULE.bazel.lock field {field} must be an object"));
    };
    Ok(entries
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect())
}

fn parse_selected_yanked_versions(
    object: &serde_json::Map<String, Value>,
) -> Result<BTreeMap<ModuleKey, String>, String> {
    let raw = optional_string_map(object, "selectedYankedVersions")?;
    let mut result = BTreeMap::new();
    for (module_version, reason) in raw {
        let (module_name, version) = module_version.rsplit_once('@').ok_or_else(|| {
            format!(
                "MODULE.bazel.lock selectedYankedVersions key {module_version} must be module@version"
            )
        })?;
        if module_name.is_empty() || version.is_empty() {
            return Err(format!(
                "MODULE.bazel.lock selectedYankedVersions key {module_version} must be module@version"
            ));
        }
        result.insert(ModuleKey::new(module_name, version), reason);
    }
    Ok(result)
}

fn parse_module_extensions(
    object: &serde_json::Map<String, Value>,
) -> Result<BTreeMap<String, BazelLockfileModuleExtension>, String> {
    let Some(value) = object.get("moduleExtensions") else {
        return Ok(BTreeMap::new());
    };
    let Value::Object(entries) = value else {
        return Err("MODULE.bazel.lock field moduleExtensions must be an object".to_owned());
    };

    let mut result = BTreeMap::new();
    for (extension_id, value) in entries {
        let Value::Object(extension) = value else {
            return Err(format!(
                "MODULE.bazel.lock moduleExtensions entry {extension_id} must be an object"
            ));
        };
        let general = match extension.get("general") {
            Some(value) => Some(parse_module_extension_general(extension_id, value)?),
            None => None,
        };
        result.insert(
            extension_id.clone(),
            BazelLockfileModuleExtension { general },
        );
    }
    Ok(result)
}

fn parse_module_extension_general(
    extension_id: &str,
    value: &Value,
) -> Result<BazelLockfileModuleExtensionGeneral, String> {
    let Value::Object(general) = value else {
        return Err(format!(
            "MODULE.bazel.lock moduleExtensions entry {extension_id}.general must be an object"
        ));
    };
    let bzl_transitive_digest = optional_string(general, "bzlTransitiveDigest")?;
    let usages_digest = optional_string(general, "usagesDigest")?;
    let recorded_inputs = match general.get("recordedInputs") {
        Some(Value::Array(inputs)) => inputs.clone(),
        Some(_) => {
            return Err(format!(
                "MODULE.bazel.lock moduleExtensions entry {extension_id}.general.recordedInputs must be an array"
            ));
        }
        None => Vec::new(),
    };
    let generated_repo_specs =
        parse_generated_repo_specs(extension_id, general.get("generatedRepoSpecs"))?;

    Ok(BazelLockfileModuleExtensionGeneral {
        bzl_transitive_digest,
        usages_digest,
        recorded_inputs,
        generated_repo_specs,
    })
}

fn optional_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<String>, String> {
    match object.get(field) {
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("MODULE.bazel.lock field {field} must be a string")),
        None => Ok(None),
    }
}

fn parse_generated_repo_specs(
    extension_id: &str,
    value: Option<&Value>,
) -> Result<BTreeMap<String, BazelLockfileRepoSpec>, String> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let Value::Object(entries) = value else {
        return Err(format!(
            "MODULE.bazel.lock moduleExtensions entry {extension_id}.general.generatedRepoSpecs must be an object"
        ));
    };

    let mut result = BTreeMap::new();
    for (repo_name, value) in entries {
        let Value::Object(spec) = value else {
            return Err(format!(
                "MODULE.bazel.lock generatedRepoSpecs entry {repo_name} must be an object"
            ));
        };
        let repo_rule_id = spec
            .get("repoRuleId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "MODULE.bazel.lock generatedRepoSpecs entry {repo_name} is missing string repoRuleId"
                )
            })?
            .to_owned();
        let attributes = match spec.get("attributes") {
            Some(Value::Object(attributes)) => attributes
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            Some(_) => {
                return Err(format!(
                    "MODULE.bazel.lock generatedRepoSpecs entry {repo_name}.attributes must be an object"
                ));
            }
            None => BTreeMap::new(),
        };
        result.insert(
            repo_name.clone(),
            BazelLockfileRepoSpec {
                repo_rule_id,
                attributes,
            },
        );
    }
    Ok(result)
}
