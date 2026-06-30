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
    })
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
