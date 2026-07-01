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
use crate::dice::LockfileMode;

pub const BAZEL_9_LOCK_FILE_VERSION: u64 = 26;

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
    pub recorded_inputs: Vec<BazelLockfileRecordedInput>,
    pub generated_repo_specs: BTreeMap<String, BazelLockfileRepoSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BazelLockfileRepoSpec {
    pub repo_rule_id: String,
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BazelLockfileRecordedInput {
    Env { name: String, value: String },
    File { label: String, digest: String },
    Raw(Value),
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

pub fn validate_lockfile_version(
    lockfile: &BazelLockfile,
    supported_lock_file_version: u64,
) -> Result<(), String> {
    if lockfile.lock_file_version == supported_lock_file_version {
        return Ok(());
    }
    Err(
        "The version of MODULE.bazel.lock is not supported by this version of Bazel. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
            .to_owned(),
    )
}

pub fn render_bazel_lockfile(lockfile: &BazelLockfile) -> Result<String, String> {
    let mut fields = Vec::new();
    fields.push(format!(
        "  {}: {}",
        json_string("lockFileVersion")?,
        lockfile.lock_file_version
    ));
    fields.push(render_string_map_field(
        "registryFileHashes",
        &lockfile.registry_file_hashes,
    )?);
    fields.push(render_selected_yanked_versions_field(lockfile)?);
    fields.push(render_module_extensions_field(lockfile)?);
    fields.push(render_value_map_field("facts", &lockfile.facts)?);
    if !lockfile.facts_versions.is_empty() {
        fields.push(render_value_map_field(
            "factsVersions",
            &lockfile.facts_versions,
        )?);
    }

    let mut rendered = String::from("{\n");
    for (index, field) in fields.iter().enumerate() {
        rendered.push_str(field);
        if index + 1 != fields.len() {
            rendered.push(',');
        }
        rendered.push('\n');
    }
    rendered.push_str("}\n");
    Ok(rendered)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisibleLockfilePlan {
    Ignore,
    Keep,
    Write { content: String },
    Error { message: String },
}

pub fn plan_visible_lockfile(
    mode: &LockfileMode,
    existing_content: Option<&str>,
    desired: &BazelLockfile,
) -> Result<VisibleLockfilePlan, String> {
    match mode {
        LockfileMode::Off => Ok(VisibleLockfilePlan::Ignore),
        LockfileMode::Update | LockfileMode::Refresh => {
            let rendered = render_bazel_lockfile(desired)?;
            if existing_content == Some(rendered.as_str()) {
                Ok(VisibleLockfilePlan::Keep)
            } else {
                Ok(VisibleLockfilePlan::Write { content: rendered })
            }
        }
        LockfileMode::Error => plan_error_mode_visible_lockfile(existing_content, desired),
    }
}

fn plan_error_mode_visible_lockfile(
    existing_content: Option<&str>,
    desired: &BazelLockfile,
) -> Result<VisibleLockfilePlan, String> {
    let Some(existing_content) = existing_content else {
        return Ok(VisibleLockfilePlan::Error {
            message: "MODULE.bazel.lock is missing and --lockfile_mode=error does not permit updating it. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
                .to_owned(),
        });
    };
    let existing = match parse_bazel_lockfile(existing_content) {
        Ok(lockfile) => lockfile,
        Err(err) => return Ok(VisibleLockfilePlan::Error { message: err }),
    };
    if let Err(err) = validate_lockfile_version(&existing, BAZEL_9_LOCK_FILE_VERSION) {
        return Ok(VisibleLockfilePlan::Error { message: err });
    }
    if existing == *desired {
        Ok(VisibleLockfilePlan::Keep)
    } else {
        Ok(VisibleLockfilePlan::Error {
            message: "MODULE.bazel.lock is no longer up-to-date. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
                .to_owned(),
        })
    }
}

fn render_selected_yanked_versions_field(lockfile: &BazelLockfile) -> Result<String, String> {
    let selected = lockfile
        .selected_yanked_versions
        .iter()
        .map(|(module, reason)| (module.to_string(), reason.clone()))
        .collect::<BTreeMap<_, _>>();
    render_string_map_field("selectedYankedVersions", &selected)
}

fn render_string_map_field(field: &str, map: &BTreeMap<String, String>) -> Result<String, String> {
    let mut rendered = format!("  {}: {{", json_string(field)?);
    if map.is_empty() {
        rendered.push('}');
        return Ok(rendered);
    }
    rendered.push('\n');
    for (index, (key, value)) in map.iter().enumerate() {
        rendered.push_str("    ");
        rendered.push_str(&json_string(key)?);
        rendered.push_str(": ");
        rendered.push_str(&json_string(value)?);
        if index + 1 != map.len() {
            rendered.push(',');
        }
        rendered.push('\n');
    }
    rendered.push_str("  }");
    Ok(rendered)
}

fn render_value_map_field(field: &str, map: &BTreeMap<String, Value>) -> Result<String, String> {
    let mut rendered = format!("  {}: {{", json_string(field)?);
    if map.is_empty() {
        rendered.push('}');
        return Ok(rendered);
    }
    rendered.push('\n');
    for (index, (key, value)) in map.iter().enumerate() {
        rendered.push_str("    ");
        rendered.push_str(&json_string(key)?);
        rendered.push_str(": ");
        rendered.push_str(&render_json_value(value, "    ")?);
        if index + 1 != map.len() {
            rendered.push(',');
        }
        rendered.push('\n');
    }
    rendered.push_str("  }");
    Ok(rendered)
}

fn render_module_extensions_field(lockfile: &BazelLockfile) -> Result<String, String> {
    let mut rendered = format!("  {}: {{", json_string("moduleExtensions")?);
    if lockfile.module_extensions.is_empty() {
        rendered.push('}');
        return Ok(rendered);
    }
    rendered.push('\n');
    for (index, (extension_id, extension)) in lockfile.module_extensions.iter().enumerate() {
        rendered.push_str("    ");
        rendered.push_str(&json_string(extension_id)?);
        rendered.push_str(": ");
        rendered.push_str(&render_module_extension(extension)?);
        if index + 1 != lockfile.module_extensions.len() {
            rendered.push(',');
        }
        rendered.push('\n');
    }
    rendered.push_str("  }");
    Ok(rendered)
}

fn render_module_extension(extension: &BazelLockfileModuleExtension) -> Result<String, String> {
    let Some(general) = &extension.general else {
        return Ok("{}".to_owned());
    };
    let mut rendered = String::from("{\n");
    rendered.push_str("      ");
    rendered.push_str(&json_string("general")?);
    rendered.push_str(": ");
    rendered.push_str(&render_module_extension_general(general)?);
    rendered.push('\n');
    rendered.push_str("    }");
    Ok(rendered)
}

fn render_module_extension_general(
    general: &BazelLockfileModuleExtensionGeneral,
) -> Result<String, String> {
    let mut fields = Vec::new();
    if let Some(digest) = &general.bzl_transitive_digest {
        fields.push(format!(
            "        {}: {}",
            json_string("bzlTransitiveDigest")?,
            json_string(digest)?
        ));
    }
    if let Some(digest) = &general.usages_digest {
        fields.push(format!(
            "        {}: {}",
            json_string("usagesDigest")?,
            json_string(digest)?
        ));
    }
    fields.push(format!(
        "        {}: {}",
        json_string("recordedInputs")?,
        render_recorded_inputs(&general.recorded_inputs)?
    ));
    fields.push(format!(
        "        {}: {}",
        json_string("generatedRepoSpecs")?,
        render_generated_repo_specs(&general.generated_repo_specs)?
    ));

    let mut rendered = String::from("{\n");
    for (index, field) in fields.iter().enumerate() {
        rendered.push_str(field);
        if index + 1 != fields.len() {
            rendered.push(',');
        }
        rendered.push('\n');
    }
    rendered.push_str("      }");
    Ok(rendered)
}

fn render_recorded_inputs(inputs: &[BazelLockfileRecordedInput]) -> Result<String, String> {
    if inputs.is_empty() {
        return Ok("[]".to_owned());
    }
    let values = inputs
        .iter()
        .map(|input| match input {
            BazelLockfileRecordedInput::Env { name, value } => {
                Value::String(format!("ENV:{name} {value}"))
            }
            BazelLockfileRecordedInput::File { label, digest } => {
                Value::String(format!("FILE:{label} {digest}"))
            }
            BazelLockfileRecordedInput::Raw(value) => value.clone(),
        })
        .collect::<Vec<_>>();
    render_json_value(&Value::Array(values), "        ")
}

fn render_generated_repo_specs(
    specs: &BTreeMap<String, BazelLockfileRepoSpec>,
) -> Result<String, String> {
    let mut rendered = String::from("{");
    if specs.is_empty() {
        rendered.push('}');
        return Ok(rendered);
    }
    rendered.push('\n');
    for (index, (repo_name, spec)) in specs.iter().enumerate() {
        rendered.push_str("          ");
        rendered.push_str(&json_string(repo_name)?);
        rendered.push_str(": {\n");
        rendered.push_str("            ");
        rendered.push_str(&json_string("repoRuleId")?);
        rendered.push_str(": ");
        rendered.push_str(&json_string(&spec.repo_rule_id)?);
        rendered.push_str(",\n");
        rendered.push_str("            ");
        rendered.push_str(&json_string("attributes")?);
        rendered.push_str(": ");
        rendered.push_str(&render_json_object(&spec.attributes, "            ")?);
        rendered.push('\n');
        rendered.push_str("          }");
        if index + 1 != specs.len() {
            rendered.push(',');
        }
        rendered.push('\n');
    }
    rendered.push_str("        }");
    Ok(rendered)
}

fn render_json_object(
    map: &BTreeMap<String, Value>,
    continuation_indent: &str,
) -> Result<String, String> {
    let value = Value::Object(
        map.iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    );
    render_json_value(&value, continuation_indent)
}

fn render_json_value(value: &Value, continuation_indent: &str) -> Result<String, String> {
    let pretty = serde_json::to_string_pretty(value)
        .map_err(|err| format!("Unable to render MODULE.bazel.lock JSON value: {err}"))?;
    let mut lines = pretty.lines();
    let first = lines.next().unwrap_or("null").to_owned();
    let mut rendered = first;
    for line in lines {
        rendered.push('\n');
        rendered.push_str(continuation_indent);
        rendered.push_str(line);
    }
    Ok(rendered)
}

fn json_string(value: &str) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|err| format!("Unable to render MODULE.bazel.lock JSON string: {err}"))
}
pub fn validate_required_registry_file_hashes(
    lockfile: &BazelLockfile,
    required_urls: &[&str],
) -> Result<(), String> {
    for url in required_urls {
        if !lockfile.registry_file_hashes.contains_key(*url) {
            return Err(format!(
                "Missing checksum for registry file {url} not permitted with --lockfile_mode=error. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
            ));
        }
    }
    Ok(())
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

pub fn validate_module_extension_bzl_transitive_digests(
    lockfile: &BazelLockfile,
    observed_bzl_transitive_digests: &BTreeMap<String, String>,
) -> Result<(), String> {
    for (extension_id, extension) in &lockfile.module_extensions {
        let Some(general) = &extension.general else {
            continue;
        };
        let Some(expected_digest) = &general.bzl_transitive_digest else {
            continue;
        };
        match observed_bzl_transitive_digests.get(extension_id) {
            Some(actual_digest) if actual_digest == expected_digest => {}
            Some(_) | None => {
                return Err(format!(
                    "MODULE.bazel.lock is no longer up-to-date because the implementation of the extension '{}' or one of its transitive .bzl files has changed. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile.",
                    bazel_display_extension_id(extension_id)
                ));
            }
        }
    }
    Ok(())
}

pub fn validate_module_extension_recorded_env_inputs(
    lockfile: &BazelLockfile,
    observed_env_values: &BTreeMap<String, String>,
) -> Result<(), String> {
    for (extension_id, extension) in &lockfile.module_extensions {
        let Some(general) = &extension.general else {
            continue;
        };
        for input in &general.recorded_inputs {
            let BazelLockfileRecordedInput::Env { name, value } = input else {
                continue;
            };
            match observed_env_values.get(name) {
                Some(actual_value) if actual_value == value => {}
                Some(actual_value) => {
                    return Err(format!(
                        "MODULE.bazel.lock is no longer up-to-date because an input to the extension '{}' changed: environment variable {name} changed: '{value}' -> '{actual_value}'. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile.",
                        bazel_display_extension_id(extension_id)
                    ));
                }
                None => {
                    return Err(format!(
                        "MODULE.bazel.lock is no longer up-to-date because an input to the extension '{}' changed: environment variable {name} changed: '{value}' -> '<unset>'. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile.",
                        bazel_display_extension_id(extension_id)
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn validate_module_extension_recorded_file_inputs(
    lockfile: &BazelLockfile,
    observed_file_digests: &BTreeMap<String, String>,
) -> Result<(), String> {
    for (extension_id, extension) in &lockfile.module_extensions {
        let Some(general) = &extension.general else {
            continue;
        };
        for input in &general.recorded_inputs {
            let BazelLockfileRecordedInput::File { label, digest } = input else {
                continue;
            };
            match observed_file_digests.get(label) {
                Some(actual_digest) if actual_digest == digest => {}
                Some(_) | None => {
                    return Err(format!(
                        "MODULE.bazel.lock is no longer up-to-date because an input to the extension '{}' changed: file info or contents of {label} changed. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile.",
                        bazel_display_extension_id(extension_id)
                    ));
                }
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
        Some(Value::Array(inputs)) => parse_recorded_inputs(extension_id, inputs)?,
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

fn parse_recorded_inputs(
    extension_id: &str,
    inputs: &[Value],
) -> Result<Vec<BazelLockfileRecordedInput>, String> {
    let mut result = Vec::with_capacity(inputs.len());
    for input in inputs {
        match input {
            Value::String(text) if text.starts_with("ENV:") => {
                result.push(parse_recorded_env_input(extension_id, text)?);
            }
            Value::String(text) if text.starts_with("FILE:") => {
                result.push(parse_recorded_file_input(extension_id, text)?);
            }
            other => result.push(BazelLockfileRecordedInput::Raw(other.clone())),
        }
    }
    Ok(result)
}

fn parse_recorded_env_input(
    extension_id: &str,
    text: &str,
) -> Result<BazelLockfileRecordedInput, String> {
    let body = text
        .strip_prefix("ENV:")
        .expect("caller checked ENV prefix");
    let (name, value) = body.split_once(' ').ok_or_else(|| {
        format!(
            "MODULE.bazel.lock moduleExtensions entry {extension_id}.general.recordedInputs ENV entry must be 'ENV:<name> <value>'"
        )
    })?;
    if name.is_empty() {
        return Err(format!(
            "MODULE.bazel.lock moduleExtensions entry {extension_id}.general.recordedInputs ENV entry must be 'ENV:<name> <value>'"
        ));
    }
    Ok(BazelLockfileRecordedInput::Env {
        name: name.to_owned(),
        value: value.to_owned(),
    })
}

fn parse_recorded_file_input(
    extension_id: &str,
    text: &str,
) -> Result<BazelLockfileRecordedInput, String> {
    let body = text
        .strip_prefix("FILE:")
        .expect("caller checked FILE prefix");
    let (label, digest) = body.rsplit_once(' ').ok_or_else(|| {
        format!(
            "MODULE.bazel.lock moduleExtensions entry {extension_id}.general.recordedInputs FILE entry must be 'FILE:<label> <digest>'"
        )
    })?;
    if label.is_empty() || digest.is_empty() {
        return Err(format!(
            "MODULE.bazel.lock moduleExtensions entry {extension_id}.general.recordedInputs FILE entry must be 'FILE:<label> <digest>'"
        ));
    }
    Ok(BazelLockfileRecordedInput::File {
        label: label.to_owned(),
        digest: digest.to_owned(),
    })
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
