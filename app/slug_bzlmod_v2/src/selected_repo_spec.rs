/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select the license that applies to you.
 */

use std::collections::BTreeMap;
use std::fmt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use serde_json::Map;
use serde_json::Value;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathOutcome;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;
use url::Url;

use crate::OverrideAttributeKey;
use crate::OverrideAttributeValue;
use crate::RegistryFileError;
use crate::RegistryFileKey;
use crate::RegistryFileUrl;
use crate::RegistryFileValue;
use crate::RepoRuleId;
use crate::RepoSpec;
use crate::RootModuleOverride;
use crate::host_registry::HostRegistryFunctionError;
use crate::host_registry::HostRegistryFunctionKey;
use crate::host_registry::HostRegistryScheme;
use crate::host_registry::RegistryKnownFileHashesMode;
use crate::module_eval::HostEffectiveModuleOverride;
use crate::module_eval::HostEffectiveModuleOverrideError;
use crate::module_eval::HostEffectiveModuleOverrideKey;
use crate::selected_graph::HostGraphModuleKey;
use crate::selected_graph::HostGraphModuleSource;
use crate::selected_graph::HostSelectedModuleEntry;
use crate::selected_graph::HostSelectedModuleGraphError;
use crate::selected_graph::HostSelectedModuleGraphKey;
use crate::source_preparation::HostDiscoveredModuleProvenance;
use crate::source_preparation::RegistryModuleFileAttempt;
use crate::source_preparation::SourcePreparationNeeds;
use crate::source_preparation::SourcePreparationNeedsError;
use crate::source_preparation::SourcePreparationOutcome;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct SelectedRegistryPolicyIdentity {
    original_registry: CompactString,
    resolved_registry: CompactString,
    scheme: HostRegistryScheme,
    known_file_hashes_mode: RegistryKnownFileHashesMode,
    vendor_directory: Option<NormalizedAbsolutePath>,
    module_mirrors: Arc<[CompactString]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct RegistryFileObservation {
    url: RegistryFileUrl,
    value: RegistryFileValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostSelectedRegistryRepoSpec {
    module: HostGraphModuleKey,
    policy: SelectedRegistryPolicyIdentity,
    module_file_attempts: Arc<[RegistryModuleFileAttempt]>,
    source_json: RegistryFileObservation,
    registry_json: Option<RegistryFileObservation>,
    effective_override: HostEffectiveModuleOverride,
    repo_spec: RepoSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostSelectedRegistryRepoSpecs {
    entries: Arc<[HostSelectedRegistryRepoSpec]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostSelectedRegistryRepoSpecsError {
    Graph(HostSelectedModuleGraphError),
    GraphCompute(CompactString),
    RegistryPolicy {
        module: HostGraphModuleKey,
        error: HostRegistryFunctionError,
    },
    RegistryPolicyCompute {
        module: HostGraphModuleKey,
        message: CompactString,
    },
    RegistryFile {
        module: HostGraphModuleKey,
        url: RegistryFileUrl,
        error: RegistryFileError,
    },
    RegistryFileCompute {
        module: HostGraphModuleKey,
        url: RegistryFileUrl,
        message: CompactString,
    },
    MissingRegistryFile {
        module: HostGraphModuleKey,
        url: RegistryFileUrl,
    },
    Json {
        module: HostGraphModuleKey,
        file: CompactString,
        message: CompactString,
    },
    Projection {
        module: HostGraphModuleKey,
        message: CompactString,
    },
    EffectiveOverride {
        module: HostGraphModuleKey,
        error: HostEffectiveModuleOverrideError,
    },
    EffectiveOverrideCompute {
        module: HostGraphModuleKey,
        message: CompactString,
    },
    IncompatibleNeeds(SourcePreparationNeedsError),
}

impl fmt::Display for HostSelectedRegistryRepoSpecsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for HostSelectedRegistryRepoSpecsError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostSelectedRegistryRepoSpecsKey {
    workspace: NormalizedAbsolutePath,
}

impl HostSelectedRegistryRepoSpecsKey {
    #[allow(dead_code)]
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostSelectedRegistryRepoSpecsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-selected-registry-repo-specs:{}", self.workspace)
    }
}

type RepoSpecsOutcome = SourcePreparationOutcome<
    Arc<Result<HostSelectedRegistryRepoSpecs, HostSelectedRegistryRepoSpecsError>>,
>;

struct SourceJson {
    source_type: Option<String>,
    url: Option<String>,
    mirror_urls: Vec<String>,
    integrity: Option<String>,
    strip_prefix: Option<String>,
    patches: BTreeMap<String, String>,
    overlay: BTreeMap<String, String>,
    patch_strip: i32,
    archive_type: Option<String>,
    path: Option<String>,
    remote: Option<String>,
    commit: Option<String>,
    shallow_since: Option<String>,
    tag: Option<String>,
    init_submodules: bool,
    verbose: bool,
}
struct RegistryJson {
    mirrors: Vec<String>,
    module_base_path: Option<String>,
}

fn fail(
    module: &HostGraphModuleKey,
    message: impl Into<CompactString>,
) -> HostSelectedRegistryRepoSpecsError {
    HostSelectedRegistryRepoSpecsError::Projection {
        module: module.clone(),
        message: message.into(),
    }
}

fn json_object(
    module: &HostGraphModuleKey,
    file: &str,
    bytes: &[u8],
) -> Result<Map<String, Value>, HostSelectedRegistryRepoSpecsError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        HostSelectedRegistryRepoSpecsError::Json {
            module: module.clone(),
            file: file.into(),
            message: error.to_string().into(),
        }
    })?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| HostSelectedRegistryRepoSpecsError::Json {
            module: module.clone(),
            file: file.into(),
            message: "expected a JSON object".into(),
        })
}

fn json_string(
    module: &HostGraphModuleKey,
    object: &Map<String, Value>,
    name: &str,
) -> Result<Option<String>, HostSelectedRegistryRepoSpecsError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(fail(module, format!("field {name} must be a string"))),
    }
}

fn json_strings(
    module: &HostGraphModuleKey,
    object: &Map<String, Value>,
    name: &str,
) -> Result<Vec<String>, HostSelectedRegistryRepoSpecsError> {
    let Some(value) = object.get(name) else {
        return Ok(Vec::new());
    };
    let Value::Array(values) = value else {
        return Err(fail(module, format!("field {name} must be a string list")));
    };
    values
        .iter()
        .map(|value| match value {
            Value::String(value) => Ok(value.clone()),
            _ => Err(fail(module, format!("field {name} must be a string list"))),
        })
        .collect()
}

fn json_string_map(
    module: &HostGraphModuleKey,
    object: &Map<String, Value>,
    name: &str,
) -> Result<BTreeMap<String, String>, HostSelectedRegistryRepoSpecsError> {
    let Some(value) = object.get(name) else {
        return Ok(BTreeMap::new());
    };
    let Value::Object(values) = value else {
        return Err(fail(module, format!("field {name} must be a string map")));
    };
    values
        .iter()
        .map(|(key, value)| match value {
            Value::String(value) => Ok((key.clone(), value.clone())),
            _ => Err(fail(module, format!("field {name} must be a string map"))),
        })
        .collect()
}

fn json_bool(
    module: &HostGraphModuleKey,
    object: &Map<String, Value>,
    name: &str,
) -> Result<bool, HostSelectedRegistryRepoSpecsError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(fail(module, format!("field {name} must be a bool"))),
    }
}

fn json_i32(
    module: &HostGraphModuleKey,
    object: &Map<String, Value>,
    name: &str,
) -> Result<i32, HostSelectedRegistryRepoSpecsError> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(0),
        Some(Value::Number(value)) => value
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| fail(module, format!("field {name} must be a 32-bit integer"))),
        Some(_) => Err(fail(module, format!("field {name} must be an integer"))),
    }
}

fn parse_source_json(
    module: &HostGraphModuleKey,
    bytes: &[u8],
) -> Result<SourceJson, HostSelectedRegistryRepoSpecsError> {
    let object = json_object(module, "source.json", bytes)?;
    Ok(SourceJson {
        source_type: json_string(module, &object, "type")?,
        url: json_string(module, &object, "url")?,
        mirror_urls: json_strings(module, &object, "mirror_urls")?,
        integrity: json_string(module, &object, "integrity")?,
        strip_prefix: json_string(module, &object, "strip_prefix")?,
        patches: json_string_map(module, &object, "patches")?,
        overlay: json_string_map(module, &object, "overlay")?,
        patch_strip: json_i32(module, &object, "patch_strip")?,
        archive_type: json_string(module, &object, "archive_type")?,
        path: json_string(module, &object, "path")?,
        remote: json_string(module, &object, "remote")?,
        commit: json_string(module, &object, "commit")?,
        shallow_since: json_string(module, &object, "shallow_since")?,
        tag: json_string(module, &object, "tag")?,
        init_submodules: json_bool(module, &object, "init_submodules")?,
        verbose: json_bool(module, &object, "verbose")?,
    })
}

fn parse_registry_json(
    module: &HostGraphModuleKey,
    bytes: &[u8],
) -> Result<RegistryJson, HostSelectedRegistryRepoSpecsError> {
    let object = json_object(module, "bazel_registry.json", bytes)?;
    Ok(RegistryJson {
        mirrors: json_strings(module, &object, "mirrors")?,
        module_base_path: json_string(module, &object, "module_base_path")?,
    })
}

fn optional_registry_json(
    module: &HostGraphModuleKey,
    bytes: &[u8],
) -> Result<Option<RegistryJson>, HostSelectedRegistryRepoSpecsError> {
    if std::str::from_utf8(bytes).is_ok_and(|text| text.trim().is_empty()) {
        Ok(None)
    } else {
        parse_registry_json(module, bytes).map(Some)
    }
}

fn normalized_path(path: &Path) -> Result<CompactString, String> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err("local path escapes its root".to_owned());
                }
            }
            Component::Normal(component) => normalized.push(component),
        }
    }
    normalized
        .to_str()
        .map(CompactString::new)
        .ok_or_else(|| "local path is not valid Unicode".to_owned())
}

fn source_json_url(registry: &str, name: &str, version: &str) -> RegistryFileUrl {
    RegistryFileUrl::new(format!(
        "{}/modules/{name}/{version}/source.json",
        registry.trim_end_matches('/')
    ))
}
fn registry_json_url(registry: &str) -> RegistryFileUrl {
    RegistryFileUrl::new(format!(
        "{}/bazel_registry.json",
        registry.trim_end_matches('/')
    ))
}

async fn registry_file(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    module: &HostGraphModuleKey,
    url: RegistryFileUrl,
) -> Result<RegistryFileObservation, HostSelectedRegistryRepoSpecsError> {
    let value = ctx
        .compute(&RegistryFileKey {
            workspace: workspace.as_path().to_owned(),
            url: url.clone(),
        })
        .await
        .map_err(
            |error| HostSelectedRegistryRepoSpecsError::RegistryFileCompute {
                module: module.clone(),
                url: url.clone(),
                message: error.to_string().into(),
            },
        )?;
    let value = value.as_ref().clone().map_err(|error| {
        HostSelectedRegistryRepoSpecsError::RegistryFile {
            module: module.clone(),
            url: url.clone(),
            error,
        }
    })?;
    Ok(RegistryFileObservation { url, value })
}

fn found_bytes<'a>(
    module: &HostGraphModuleKey,
    observation: &'a RegistryFileObservation,
) -> Result<&'a [u8], HostSelectedRegistryRepoSpecsError> {
    match &observation.value {
        RegistryFileValue::Found { bytes, .. } => Ok(bytes),
        RegistryFileValue::NotFound { .. } => {
            Err(HostSelectedRegistryRepoSpecsError::MissingRegistryFile {
                module: module.clone(),
                url: observation.url.clone(),
            })
        }
    }
}

fn module_file_identity(
    module: &HostGraphModuleKey,
    attempts: &[RegistryModuleFileAttempt],
) -> Result<(CompactString, [u8; 32]), HostSelectedRegistryRepoSpecsError> {
    attempts
        .iter()
        .rev()
        .find_map(|attempt| {
            attempt
                .sha256
                .map(|hash| (attempt.url.as_str().into(), hash))
        })
        .ok_or_else(|| {
            fail(
                module,
                "selected registry module has no winning MODULE hash",
            )
        })
}

fn iterable(values: impl IntoIterator<Item = OverrideAttributeValue>) -> OverrideAttributeValue {
    OverrideAttributeValue::Iterable(values.into_iter().collect())
}
fn strings(values: impl IntoIterator<Item = impl Into<CompactString>>) -> OverrideAttributeValue {
    iterable(
        values
            .into_iter()
            .map(|value| OverrideAttributeValue::String(value.into())),
    )
}
fn attrs_map(
    values: impl IntoIterator<Item = (String, OverrideAttributeValue)>,
) -> OverrideAttributeValue {
    OverrideAttributeValue::Map(Arc::new(
        values
            .into_iter()
            .map(|(key, value)| (OverrideAttributeKey::String(key.into()), value))
            .collect(),
    ))
}
fn repo_spec(
    rule_label: &str,
    rule_name: &str,
    attributes: SmallMap<CompactString, OverrideAttributeValue>,
) -> RepoSpec {
    RepoSpec {
        rule_id: RepoRuleId {
            bzl_file: slug_identity_v2::CanonicalLabel::parse(rule_label)
                .expect("pinned repository label is canonical"),
            rule_name: rule_name.into(),
        },
        attributes: Arc::new(attributes),
    }
}
fn module_sri(hash: [u8; 32]) -> CompactString {
    format!("sha256-{}", BASE64.encode(hash)).into()
}

fn mirrored_url(mirror: &str, source: &Url) -> CompactString {
    let authority = source.host_str().map_or_else(String::new, |host| {
        source
            .port()
            .map_or_else(|| host.to_owned(), |port| format!("{host}:{port}"))
    });
    let query = source
        .query()
        .map_or_else(String::new, |query| format!("?{query}"));
    format!(
        "{}/{authority}{}{query}",
        mirror.trim_end_matches('/'),
        source.path()
    )
    .into()
}

fn archive_repo_spec(
    module: &HostGraphModuleKey,
    source: &SourceJson,
    registry: &str,
    module_url: CompactString,
    module_hash: [u8; 32],
    command_mirrors: &[CompactString],
    registry_json: Option<&RegistryJson>,
    name: &str,
    version: &str,
) -> Result<RepoSpec, HostSelectedRegistryRepoSpecsError> {
    let primary = source
        .url
        .as_deref()
        .ok_or_else(|| fail(module, "missing source url"))?;
    let primary_url = Url::parse(primary)
        .map_err(|error| fail(module, format!("invalid source url: {error}")))?;
    if !primary_url.has_host() {
        return Err(fail(module, "source url must be absolute"));
    }
    let integrity = source
        .integrity
        .as_deref()
        .ok_or_else(|| fail(module, "missing source integrity"))?;
    let mut mirrors = SmallSet::new();
    mirrors.extend(command_mirrors.iter().cloned());
    if let Some(registry_json) = registry_json {
        mirrors.extend(registry_json.mirrors.iter().map(CompactString::new));
    }
    let mut urls = mirrors
        .iter()
        .map(|mirror| mirrored_url(mirror, &primary_url))
        .collect::<Vec<_>>();
    urls.push(primary.into());
    urls.extend(source.mirror_urls.iter().map(CompactString::new));

    let patches = source.patches.iter().map(|(file, integrity)| {
        (
            format!("{registry}/modules/{name}/{version}/patches/{file}"),
            OverrideAttributeValue::String(integrity.into()),
        )
    });
    let overlay_urls = source.overlay.keys().map(|file| {
        (
            file.clone(),
            strings([format!(
                "{registry}/modules/{name}/{version}/overlay/{file}"
            )]),
        )
    });
    let overlay_integrity = source.overlay.iter().map(|(file, integrity)| {
        (
            file.clone(),
            OverrideAttributeValue::String(integrity.into()),
        )
    });
    let mut attrs = SmallMap::new();
    attrs.insert("urls".into(), strings(urls));
    attrs.insert(
        "integrity".into(),
        OverrideAttributeValue::String(integrity.into()),
    );
    attrs.insert(
        "strip_prefix".into(),
        OverrideAttributeValue::String(source.strip_prefix.as_deref().unwrap_or("").into()),
    );
    attrs.insert("remote_patches".into(), attrs_map(patches));
    attrs.insert("remote_file_urls".into(), attrs_map(overlay_urls));
    attrs.insert("remote_file_integrity".into(), attrs_map(overlay_integrity));
    attrs.insert("remote_module_file_urls".into(), strings([module_url]));
    attrs.insert(
        "remote_module_file_integrity".into(),
        OverrideAttributeValue::String(module_sri(module_hash)),
    );
    attrs.insert(
        "remote_patch_strip".into(),
        OverrideAttributeValue::Int(source.patch_strip),
    );
    if let Some(archive_type) = source
        .archive_type
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        attrs.insert(
            "type".into(),
            OverrideAttributeValue::String(archive_type.into()),
        );
    }
    Ok(repo_spec(
        "@@bazel_tools//tools/build_defs/repo:http.bzl",
        "http_archive",
        attrs,
    ))
}

fn local_repo_spec(
    module: &HostGraphModuleKey,
    source: &SourceJson,
    policy: &SelectedRegistryPolicyIdentity,
    registry_json: Option<&RegistryJson>,
) -> Result<RepoSpec, HostSelectedRegistryRepoSpecsError> {
    let path = source
        .path
        .as_deref()
        .ok_or_else(|| fail(module, "missing local_path path"))?;
    let path = if Path::new(path).is_absolute() {
        normalized_path(Path::new(path))
    } else {
        let base = registry_json
            .and_then(|json| json.module_base_path.as_deref())
            .ok_or_else(|| fail(module, "relative local_path requires module_base_path"))?;
        if std::path::Path::new(base).is_absolute() {
            normalized_path(&Path::new(base).join(path))
        } else if policy.scheme == HostRegistryScheme::File {
            let registry = Url::parse(&policy.resolved_registry)
                .map_err(|error| fail(module, format!("invalid file registry: {error}")))?
                .to_file_path()
                .map_err(|_| fail(module, "file registry has invalid path"))?;
            normalized_path(&registry.join(base).join(path))
        } else {
            return Err(fail(
                module,
                "relative module_base_path requires a file registry",
            ));
        }
    }
    .map_err(|error| fail(module, error))?;
    Ok(repo_spec(
        "@@bazel_tools//tools/build_defs/repo:local.bzl",
        "local_repository",
        SmallMap::from_iter([(
            CompactString::new("path"),
            OverrideAttributeValue::String(path),
        )]),
    ))
}

fn git_repo_spec(
    source: &SourceJson,
    registry: &str,
    module_url: CompactString,
    module_hash: [u8; 32],
    name: &str,
    version: &str,
) -> RepoSpec {
    let mut attrs = SmallMap::new();
    for (name, value) in [
        ("remote", source.remote.as_deref()),
        ("commit", source.commit.as_deref()),
        ("shallow_since", source.shallow_since.as_deref()),
        ("tag", source.tag.as_deref()),
        ("strip_prefix", source.strip_prefix.as_deref()),
    ] {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            attrs.insert(name.into(), OverrideAttributeValue::String(value.into()));
        }
    }
    attrs.insert(
        "init_submodules".into(),
        OverrideAttributeValue::Bool(source.init_submodules),
    );
    attrs.insert(
        "recursive_init_submodules".into(),
        OverrideAttributeValue::Bool(source.init_submodules),
    );
    attrs.insert(
        "verbose".into(),
        OverrideAttributeValue::Bool(source.verbose),
    );
    if !source.patches.is_empty() {
        attrs.insert(
            "remote_patches".into(),
            attrs_map(source.patches.iter().map(|(file, integrity)| {
                (
                    format!("{registry}/modules/{name}/{version}/patches/{file}"),
                    OverrideAttributeValue::String(integrity.into()),
                )
            })),
        );
    }
    attrs.insert("remote_module_file_urls".into(), strings([module_url]));
    attrs.insert(
        "remote_module_file_integrity".into(),
        OverrideAttributeValue::String(module_sri(module_hash)),
    );
    attrs.insert(
        "remote_patch_strip".into(),
        OverrideAttributeValue::Int(source.patch_strip),
    );
    repo_spec(
        "@@bazel_tools//tools/build_defs/repo:git.bzl",
        "git_repository",
        attrs,
    )
}

fn augment_override(
    module: &HostGraphModuleKey,
    mut spec: RepoSpec,
    effective: &HostEffectiveModuleOverride,
) -> Result<RepoSpec, HostSelectedRegistryRepoSpecsError> {
    match effective {
        HostEffectiveModuleOverride::Root {
            override_: RootModuleOverride::RegistrySingle(single),
        } => {
            if single.patches.is_empty() && single.patch_cmds.is_empty() && single.patch_strip == 0
            {
                return Ok(spec);
            }
            let mut attrs = (*spec.attributes).clone();
            attrs.insert(
                "patches".into(),
                iterable(
                    single
                        .patches
                        .iter()
                        .cloned()
                        .map(OverrideAttributeValue::Label),
                ),
            );
            attrs.insert(
                "patch_cmds".into(),
                strings(single.patch_cmds.iter().cloned()),
            );
            attrs.insert(
                "patch_args".into(),
                strings([format!("-p{}", single.patch_strip)]),
            );
            spec.attributes = Arc::new(attrs);
            Ok(spec)
        }
        HostEffectiveModuleOverride::Root {
            override_: RootModuleOverride::RegistryMultiple(_),
        }
        | HostEffectiveModuleOverride::None => Ok(spec),
        _ => Err(fail(
            module,
            "selected registry module has incompatible effective override",
        )),
    }
}

async fn compute_entry(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    entry: &HostSelectedModuleEntry,
) -> SourcePreparationOutcome<
    Result<Option<HostSelectedRegistryRepoSpec>, HostSelectedRegistryRepoSpecsError>,
> {
    let (name, version) = match &entry.key {
        HostGraphModuleKey::Root => return SourcePreparationOutcome::Complete(Ok(None)),
        HostGraphModuleKey::Module { name, version } => (name, version),
    };
    let HostGraphModuleSource::Discovered(discovered) = &entry.source else {
        return SourcePreparationOutcome::Complete(Err(fail(
            &entry.key,
            "nonroot graph entry has root source",
        )));
    };
    let HostDiscoveredModuleProvenance::Registry {
        selected_registry,
        module_file_attempts,
    } = &discovered.provenance
    else {
        return SourcePreparationOutcome::Complete(Ok(None));
    };
    let policy = match ctx
        .compute(&HostRegistryFunctionKey::new(
            workspace.dupe(),
            selected_registry.as_str(),
        ))
        .await
    {
        Ok(PathOutcome::Need(need)) => {
            return SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need));
        }
        Ok(PathOutcome::Complete(value)) => match value.as_ref().clone() {
            Ok(value) => value,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    HostSelectedRegistryRepoSpecsError::RegistryPolicy {
                        module: entry.key.clone(),
                        error: error.clone(),
                    },
                ));
            }
        },
        Err(error) => {
            return SourcePreparationOutcome::Complete(Err(
                HostSelectedRegistryRepoSpecsError::RegistryPolicyCompute {
                    module: entry.key.clone(),
                    message: error.to_string().into(),
                },
            ));
        }
    };
    let policy_identity = SelectedRegistryPolicyIdentity {
        original_registry: policy.original_registry().into(),
        resolved_registry: policy.resolved_registry().into(),
        scheme: policy.scheme(),
        known_file_hashes_mode: policy.known_file_hashes_mode(),
        vendor_directory: policy.vendor_directory().map(Dupe::dupe),
        module_mirrors: policy.module_mirrors().iter().cloned().collect(),
    };
    let version = version.normalized();
    let source_json = match registry_file(
        ctx,
        workspace,
        &entry.key,
        source_json_url(policy.resolved_registry(), name, version),
    )
    .await
    {
        Ok(value) => value,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    let source = match found_bytes(&entry.key, &source_json)
        .and_then(|bytes| parse_source_json(&entry.key, bytes))
    {
        Ok(value) => value,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    let source_type = source.source_type.as_deref().unwrap_or("archive");
    let registry_json = if matches!(source_type, "archive" | "local_path") {
        match registry_file(
            ctx,
            workspace,
            &entry.key,
            registry_json_url(policy.resolved_registry()),
        )
        .await
        {
            Ok(value) => Some(value),
            Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
        }
    } else {
        None
    };
    let parsed_registry_json = match registry_json.as_ref() {
        Some(observation) => match &observation.value {
            RegistryFileValue::NotFound { .. } => None,
            RegistryFileValue::Found { bytes, .. } => {
                match optional_registry_json(&entry.key, bytes) {
                    Ok(value) => value,
                    Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
                }
            }
        },
        None => None,
    };
    let (module_url, module_hash) = match module_file_identity(&entry.key, module_file_attempts) {
        Ok(value) => value,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    let projected = match source_type {
        "archive" => archive_repo_spec(
            &entry.key,
            &source,
            policy.resolved_registry(),
            module_url,
            module_hash,
            policy.module_mirrors(),
            parsed_registry_json.as_ref(),
            name,
            version,
        ),
        "local_path" => local_repo_spec(
            &entry.key,
            &source,
            &policy_identity,
            parsed_registry_json.as_ref(),
        ),
        "git_repository" => Ok(git_repo_spec(
            &source,
            policy.resolved_registry(),
            module_url,
            module_hash,
            name,
            version,
        )),
        other => Err(fail(&entry.key, format!("invalid source type {other:?}"))),
    };
    let mut repo_spec = match projected {
        Ok(value) => value,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    let effective = match ctx
        .compute(&HostEffectiveModuleOverrideKey::new(
            workspace.dupe(),
            name.clone(),
        ))
        .await
    {
        Ok(value) => match value.as_ref() {
            Ok(value) => value.clone(),
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    HostSelectedRegistryRepoSpecsError::EffectiveOverride {
                        module: entry.key.clone(),
                        error: error.clone(),
                    },
                ));
            }
        },
        Err(error) => {
            return SourcePreparationOutcome::Complete(Err(
                HostSelectedRegistryRepoSpecsError::EffectiveOverrideCompute {
                    module: entry.key.clone(),
                    message: error.to_string().into(),
                },
            ));
        }
    };
    repo_spec = match augment_override(&entry.key, repo_spec, &effective) {
        Ok(value) => value,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    SourcePreparationOutcome::Complete(Ok(Some(HostSelectedRegistryRepoSpec {
        module: entry.key.clone(),
        policy: policy_identity,
        module_file_attempts: module_file_attempts.clone(),
        source_json,
        registry_json,
        effective_override: effective,
        repo_spec,
    })))
}

#[async_trait]
impl Key for HostSelectedRegistryRepoSpecsKey {
    type Value = RepoSpecsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let graph = match ctx
            .compute(&HostSelectedModuleGraphKey::new(self.workspace.dupe()))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref().clone() {
                Ok(value) => value,
                Err(error) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(
                        HostSelectedRegistryRepoSpecsError::Graph(error.clone()),
                    )));
                }
            },
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    HostSelectedRegistryRepoSpecsError::GraphCompute(error.to_string().into()),
                )));
            }
        };
        let mut entries = Vec::new();
        let mut first_error = None;
        let mut needs: Option<SourcePreparationNeeds> = None;
        let mut incompatible = None;
        for entry in graph.resolved.iter() {
            match compute_entry(ctx, &self.workspace, entry).await {
                SourcePreparationOutcome::Complete(Ok(Some(entry))) => entries.push(entry),
                SourcePreparationOutcome::Complete(Ok(None)) => {}
                SourcePreparationOutcome::Complete(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
                SourcePreparationOutcome::Need(need) => {
                    needs = match needs.take() {
                        None => Some(need),
                        Some(current) => match current.try_union(&need) {
                            Ok(union) => Some(union),
                            Err(error) => {
                                if incompatible.is_none() {
                                    incompatible = Some(error);
                                }
                                Some(current)
                            }
                        },
                    };
                }
            }
        }
        if let Some(error) = first_error {
            return SourcePreparationOutcome::Complete(Arc::new(Err(error)));
        }
        if let Some(error) = incompatible {
            return SourcePreparationOutcome::Complete(Arc::new(Err(
                HostSelectedRegistryRepoSpecsError::IncompatibleNeeds(error),
            )));
        }
        if let Some(need) = needs {
            return SourcePreparationOutcome::Need(need);
        }
        SourcePreparationOutcome::Complete(Arc::new(Ok(HostSelectedRegistryRepoSpecs {
            entries: entries.into(),
        })))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostSelectedRepositoryMapping {
    context_repo: CanonicalRepoName,
    entries: Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostSelectedModuleRoute {
    entry: HostSelectedModuleEntry,
    canonical_repo: CanonicalRepoName,
    mapping: HostSelectedRepositoryMapping,
    registry_repo_spec: Option<HostSelectedRegistryRepoSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostSelectedModuleRoutes {
    entries: Arc<[HostSelectedModuleRoute]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostSelectedModuleRoutesError {
    Graph(HostSelectedModuleGraphError),
    GraphCompute(CompactString),
    RepoSpecs(HostSelectedRegistryRepoSpecsError),
    RepoSpecsCompute(CompactString),
    Invalid {
        module: HostGraphModuleKey,
        message: CompactString,
    },
    CanonicalCollision {
        canonical_repo: CanonicalRepoName,
        first: HostGraphModuleKey,
        second: HostGraphModuleKey,
    },
    RegistryMismatch {
        module: HostGraphModuleKey,
        message: CompactString,
    },
}

impl fmt::Display for HostSelectedModuleRoutesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HostSelectedModuleRoutesError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostSelectedModuleRoutesKey {
    workspace: NormalizedAbsolutePath,
}

impl HostSelectedModuleRoutesKey {
    fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostSelectedModuleRoutesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-selected-module-routes:{}", self.workspace)
    }
}

type RoutesOutcome =
    SourcePreparationOutcome<Arc<Result<HostSelectedModuleRoutes, HostSelectedModuleRoutesError>>>;

fn route_invalid(
    module: &HostGraphModuleKey,
    message: impl Into<CompactString>,
) -> HostSelectedModuleRoutesError {
    HostSelectedModuleRoutesError::Invalid {
        module: module.clone(),
        message: message.into(),
    }
}

fn canonical_repo(
    key: &HostGraphModuleKey,
    name_counts: &SmallMap<CompactString, usize>,
) -> Result<CanonicalRepoName, HostSelectedModuleRoutesError> {
    let spelling = match key {
        HostGraphModuleKey::Root => return Ok(CanonicalRepoName::root()),
        HostGraphModuleKey::Module { name, .. }
            if matches!(name.as_str(), "bazel_tools" | "platforms") =>
        {
            name.to_string()
        }
        HostGraphModuleKey::Module { name, version }
            if name_counts.get(name.as_str()).copied().unwrap_or_default() > 1 =>
        {
            if version.is_empty() {
                return Err(route_invalid(
                    key,
                    "multiple selected versions require a nonempty version",
                ));
            }
            format!("{}+{}", name, version.normalized())
        }
        HostGraphModuleKey::Module { name, .. } => format!("{name}+"),
    };
    CanonicalRepoName::new(spelling).map_err(|message| route_invalid(key, message))
}

fn insert_mapping(
    owner: &HostGraphModuleKey,
    mapping: &mut SmallMap<ApparentRepoName, CanonicalRepoName>,
    apparent: &str,
    canonical: CanonicalRepoName,
) -> Result<(), HostSelectedModuleRoutesError> {
    let apparent = if apparent.is_empty() {
        ApparentRepoName::root()
    } else {
        ApparentRepoName::new(apparent).map_err(|message| route_invalid(owner, message))?
    };
    if let Some(existing) = mapping.get(&apparent) {
        if existing != &canonical {
            return Err(route_invalid(
                owner,
                format!(
                    "apparent repository '{}' maps to both '{}' and '{}'",
                    apparent.as_str(),
                    existing.as_str(),
                    canonical.as_str()
                ),
            ));
        }
    } else {
        mapping.insert(apparent, canonical);
    }
    Ok(())
}

fn selected_routes(
    graph: &crate::selected_graph::HostSelectedModuleGraph,
    repo_specs: &HostSelectedRegistryRepoSpecs,
) -> Result<HostSelectedModuleRoutes, HostSelectedModuleRoutesError> {
    let canonicals = canonical_lookup(graph.resolved.iter().map(|entry| &entry.key))?;
    selected_routes_with_canonicals(graph, repo_specs, canonicals)
}

fn canonical_lookup<'a>(
    keys: impl Clone + Iterator<Item = &'a HostGraphModuleKey>,
) -> Result<SmallMap<HostGraphModuleKey, CanonicalRepoName>, HostSelectedModuleRoutesError> {
    let mut name_counts = SmallMap::<CompactString, usize>::new();
    for key in keys.clone() {
        if let HostGraphModuleKey::Module { name, .. } = key {
            *name_counts.entry(name.clone()).or_default() += 1;
        }
    }
    let mut canonicals = SmallMap::<HostGraphModuleKey, CanonicalRepoName>::new();
    let mut owners = SmallMap::<CanonicalRepoName, HostGraphModuleKey>::new();
    for key in keys {
        let canonical = canonical_repo(key, &name_counts)?;
        if let Some(first) = owners.get(&canonical) {
            return Err(HostSelectedModuleRoutesError::CanonicalCollision {
                canonical_repo: canonical,
                first: first.clone(),
                second: key.clone(),
            });
        }
        owners.insert(canonical.clone(), key.clone());
        canonicals.insert(key.clone(), canonical);
    }
    Ok(canonicals)
}

fn selected_routes_with_canonicals(
    graph: &crate::selected_graph::HostSelectedModuleGraph,
    repo_specs: &HostSelectedRegistryRepoSpecs,
    canonicals: SmallMap<HostGraphModuleKey, CanonicalRepoName>,
) -> Result<HostSelectedModuleRoutes, HostSelectedModuleRoutesError> {
    let mut selected_specs = SmallMap::<HostGraphModuleKey, HostSelectedRegistryRepoSpec>::new();
    for spec in repo_specs.entries.iter() {
        if selected_specs
            .insert(spec.module.clone(), spec.clone())
            .is_some()
        {
            return Err(HostSelectedModuleRoutesError::RegistryMismatch {
                module: spec.module.clone(),
                message: "duplicate selected registry RepoSpec".into(),
            });
        }
    }

    let mut routes = Vec::with_capacity(graph.resolved.len());
    for entry in graph.resolved.iter() {
        let canonical = canonicals
            .get(&entry.key)
            .expect("every selected entry has a canonical identity")
            .clone();
        let mut mapping = SmallMap::new();
        if matches!(entry.key, HostGraphModuleKey::Root) {
            insert_mapping(&entry.key, &mut mapping, "", CanonicalRepoName::root())?;
        }
        let self_name = match &entry.source {
            HostGraphModuleSource::Root(module) => module
                .header
                .as_ref()
                .map(|header| header.repo_name.as_deref().unwrap_or(header.name.as_str())),
            HostGraphModuleSource::Discovered(module) => {
                Some(module.module.base.repo_name.as_str())
            }
        };
        if let Some(name) = self_name.filter(|name| !name.is_empty()) {
            insert_mapping(&entry.key, &mut mapping, name, canonical.clone())?;
        }
        for dependency in entry.dependencies.iter() {
            let Some(apparent) = dependency.apparent_name.as_deref() else {
                continue;
            };
            let target = canonicals
                .get(&dependency.key)
                .ok_or_else(|| route_invalid(&entry.key, "selected dependency is absent"))?;
            insert_mapping(&entry.key, &mut mapping, apparent, target.clone())?;
        }

        let is_registry = matches!(
            &entry.source,
            HostGraphModuleSource::Discovered(module)
                if matches!(module.provenance, HostDiscoveredModuleProvenance::Registry { .. })
        );
        let registry_repo_spec = selected_specs.shift_remove(&entry.key);
        if is_registry != registry_repo_spec.is_some() {
            return Err(HostSelectedModuleRoutesError::RegistryMismatch {
                module: entry.key.clone(),
                message: if is_registry {
                    "selected registry module has no RepoSpec"
                } else {
                    "nonregistry selected module has a registry RepoSpec"
                }
                .into(),
            });
        }
        routes.push(HostSelectedModuleRoute {
            entry: entry.clone(),
            canonical_repo: canonical.clone(),
            mapping: HostSelectedRepositoryMapping {
                context_repo: canonical,
                entries: Arc::new(mapping),
            },
            registry_repo_spec,
        });
    }
    if let Some((module, _)) = selected_specs.into_iter().next() {
        return Err(HostSelectedModuleRoutesError::RegistryMismatch {
            module,
            message: "RepoSpec is not present in the selected graph".into(),
        });
    }
    Ok(HostSelectedModuleRoutes {
        entries: routes.into(),
    })
}

#[async_trait]
impl Key for HostSelectedModuleRoutesKey {
    type Value = RoutesOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let graph = match ctx
            .compute(&HostSelectedModuleGraphKey::new(self.workspace.dupe()))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(graph) => graph.clone(),
                Err(error) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(
                        HostSelectedModuleRoutesError::Graph(error.clone()),
                    )));
                }
            },
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    HostSelectedModuleRoutesError::GraphCompute(error.to_string().into()),
                )));
            }
        };
        let repo_specs = match ctx
            .compute(&HostSelectedRegistryRepoSpecsKey::new(
                self.workspace.dupe(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(repo_specs) => repo_specs.clone(),
                Err(error) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(
                        HostSelectedModuleRoutesError::RepoSpecs(error.clone()),
                    )));
                }
            },
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    HostSelectedModuleRoutesError::RepoSpecsCompute(error.to_string().into()),
                )));
            }
        };
        SourcePreparationOutcome::Complete(Arc::new(selected_routes(&graph, &repo_specs)))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use compact_str::CompactString;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::Key;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use starlark_map::sorted_map::SortedMap;

    use super::*;
    use crate::host_registry_inputs::HostModuleMirrorOccurrence;
    use crate::host_registry_inputs::HostModuleMirrorsInputKey;
    use crate::host_registry_inputs::HostRegistryRefreshToken;
    use crate::host_registry_inputs::HostRegistryRefreshTokenKey;
    use crate::host_registry_inputs::normalize_host_registry_inputs;
    use crate::interim_module::NonrootModuleBuilder;
    use crate::interim_module::NonrootModuleKey;
    use crate::module_eval::RegistrySingleOverride;
    use crate::module_eval::RootModuleHeader;
    use crate::module_eval::RootModuleRegistrations;
    use crate::module_version::BazelModuleVersion;
    use crate::source_preparation::HostDiscoveredModule;

    const WORKSPACE: &str = "/selected-repo-spec-test";
    const REGISTRY: &str = "https://registry.invalid";
    const LOCAL_MODULES: &[&str] = &[
        "local",
        "rules_license",
        "buildozer",
        "platforms",
        "zlib",
        "bazel_features",
        "protobuf",
        "rules_java",
        "rules_cc",
        "rules_python",
        "rules_shell",
        "apple_support",
        "rules_apple",
        "rules_swift",
        "abseil-cpp",
    ];

    struct TrackingRegistryIo {
        files: Mutex<BTreeMap<String, Arc<[u8]>>>,
        calls: Mutex<Vec<String>>,
    }

    impl TrackingRegistryIo {
        fn new(files: impl IntoIterator<Item = (&'static str, &'static [u8])>) -> Self {
            Self {
                files: Mutex::new(
                    files
                        .into_iter()
                        .map(|(url, bytes)| (url.to_owned(), Arc::from(bytes)))
                        .collect(),
                ),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn replace(&self, url: &str, bytes: &'static [u8]) {
            self.files
                .lock()
                .unwrap()
                .insert(url.to_owned(), Arc::from(bytes));
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl crate::RegistryIo for TrackingRegistryIo {
        async fn read_exact(
            &self,
            url: &RegistryFileUrl,
        ) -> Result<crate::RegistryIoOutcome, crate::RegistryTransportError> {
            self.calls.lock().unwrap().push(url.as_str().to_owned());
            Ok(self
                .files
                .lock()
                .unwrap()
                .get(url.as_str())
                .map_or(crate::RegistryIoOutcome::NotFound, |bytes| {
                    crate::RegistryIoOutcome::Found(bytes.clone())
                }))
        }
    }

    fn observation(path: &str, operation: PathObservationOperation) -> PathObservationDemand {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    }

    fn host_epoch() -> PathObservationEpoch {
        let lock = format!("{WORKSPACE}/MODULE.bazel.lock");
        let lstat = |path: &str, kind, id| {
            (
                observation(path, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    kind,
                    id,
                    id + 1,
                    id + 2,
                    id + 3,
                    0o755,
                ))),
            )
        };
        let mut observations = vec![
            lstat("/", PathNodeKind::Directory, 1),
            lstat(WORKSPACE, PathNodeKind::Directory, 2),
            lstat(&lock, PathNodeKind::RegularFile, 3),
            (
                observation(&lock, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    &br#"{"lockFileVersion":28,"registryFileHashes":{},
                    "selectedYankedVersions":{},"moduleExtensions":{},
                    "facts":{},"factsVersions":{}}"#[..],
                ))),
            ),
        ];
        for (index, name) in LOCAL_MODULES.iter().enumerate() {
            let root = format!("{WORKSPACE}/{name}");
            let module = format!("{root}/MODULE.bazel");
            let id = 10 + index as i64 * 2;
            observations.extend([
                lstat(&root, PathNodeKind::Directory, id),
                lstat(&module, PathNodeKind::RegularFile, id + 1),
                (
                    observation(&module, PathObservationOperation::FileBytes),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        format!("module(name='{name}')\n").into_bytes(),
                    ))),
                ),
                (
                    observation(
                        &format!("{root}/REPO.bazel"),
                        PathObservationOperation::Lstat,
                    ),
                    PathObservationResult::Lstat(PathOperationResult::Missing),
                ),
                (
                    observation(
                        &format!("{root}/.bazelignore"),
                        PathObservationOperation::Lstat,
                    ),
                    PathObservationResult::Lstat(PathOperationResult::Missing),
                ),
            ]);
        }
        PathObservationEpoch::new(observations).unwrap()
    }

    async fn real_transaction(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        mirrors: &[&str],
        include_epoch: bool,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel"),
                        WorkspaceFileValue::Present(Arc::new(root.to_owned())),
                    )])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel.lock"),
                        WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        crate::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Update,
        )
        .unwrap();
        crate::inject_registry_request_inputs(
            &mut updater,
            workspace.as_path(),
            crate::RegistryUrls::new([REGISTRY]),
            crate::RegistryRequestGeneration(generation),
        )
        .unwrap();
        crate::inject_root_package_policy_inputs(
            &mut updater,
            crate::RootPackagePolicyInputs::new(
                workspace.dupe(),
                Arc::from([workspace.dupe()]),
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        if include_epoch {
            updater
                .changed_to(vec![(PathObservationEpochKey, host_epoch())])
                .unwrap();
        } else {
            updater
                .changed_to(vec![(
                    PathObservationEpochKey,
                    PathObservationEpoch::new(std::iter::empty()).unwrap(),
                )])
                .unwrap();
        }
        updater
            .changed_to(vec![(
                HostRegistryRefreshTokenKey::new(workspace.dupe()),
                HostRegistryRefreshToken::new(generation),
            )])
            .unwrap();
        let mirror_input = normalize_host_registry_inputs(
            [REGISTRY],
            [HostModuleMirrorOccurrence::new(
                REGISTRY,
                mirrors
                    .iter()
                    .map(|mirror| CompactString::new(*mirror))
                    .collect::<Arc<[_]>>(),
            )],
        )
        .unwrap()
        .1;
        updater
            .changed_to(vec![(
                HostModuleMirrorsInputKey::new(workspace.dupe()),
                mirror_input,
            )])
            .unwrap();
        let materializations = LOCAL_MODULES.iter().filter_map(|name| {
            root.contains(&format!(
                "local_path_override(module_name='{name}', path='{name}')"
            ))
            .then(|| {
                let repo_spec = repo_spec(
                    "@@bazel_tools//tools/build_defs/repo:local.bzl",
                    "local_repository",
                    SmallMap::from_iter([(
                        CompactString::new("path"),
                        OverrideAttributeValue::String((*name).into()),
                    )]),
                );
                crate::RepositoryMaterializationEpochEntry {
                    request: Arc::new(crate::RepositoryMaterializationRequest {
                        id: crate::RepositoryMaterializationRequestId {
                            workspace: workspace.dupe(),
                            canonical_repo: CanonicalRepoName::new(format!("{name}+")).unwrap(),
                        },
                        repo_spec,
                        kind: crate::RepositoryMaterializationKind::Local {
                            logical_root: NormalizedAbsolutePath::new(format!(
                                "{WORKSPACE}/{name}"
                            ))
                            .unwrap(),
                        },
                    }),
                    result: crate::RepositoryMaterializationResult::Success(
                        crate::RepositoryMaterializationSuccess::Local,
                    ),
                }
            })
        });
        updater
            .changed_to(vec![(
                crate::RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                crate::RepositoryMaterializationResultEpoch::new(
                    workspace.dupe(),
                    materializations,
                )
                .unwrap(),
            )])
            .unwrap();
        updater.commit().await
    }

    async fn compute_real(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        mirrors: &[&str],
        include_epoch: bool,
    ) -> RepoSpecsOutcome {
        real_transaction(dice, root, generation, mirrors, include_epoch)
            .await
            .compute(&HostSelectedRegistryRepoSpecsKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    async fn compute_real_routes(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        mirrors: &[&str],
        include_epoch: bool,
    ) -> RoutesOutcome {
        real_transaction(dice, root, generation, mirrors, include_epoch)
            .await
            .compute(&HostSelectedModuleRoutesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    fn module() -> HostGraphModuleKey {
        HostGraphModuleKey::Module {
            name: "demo".into(),
            version: BazelModuleVersion::parse("1.2.3").unwrap(),
        }
    }

    fn route_key(name: &str, version: &str) -> HostGraphModuleKey {
        HostGraphModuleKey::Module {
            name: name.into(),
            version: BazelModuleVersion::parse(version).unwrap(),
        }
    }

    fn route_root(
        dependencies: impl IntoIterator<Item = (&'static str, HostGraphModuleKey)>,
        repo_name: Option<&str>,
    ) -> HostSelectedModuleEntry {
        let dependencies: Arc<[_]> = dependencies
            .into_iter()
            .map(
                |(apparent, key)| crate::selected_graph::HostGraphDependency {
                    apparent_name: Some(apparent.into()),
                    key,
                },
            )
            .collect();
        HostSelectedModuleEntry {
            key: HostGraphModuleKey::Root,
            source: HostGraphModuleSource::Root(Arc::new(
                crate::module_eval::EvaluatedRootModule {
                    header: Some(RootModuleHeader {
                        name: "root".into(),
                        version: Some("1".into()),
                        repo_name: repo_name.map(Into::into),
                    }),
                    dependencies: Arc::from([]),
                    registrations: RootModuleRegistrations::default(),
                },
            )),
            original_dependencies: dependencies.clone(),
            dependencies,
            nodep_dependencies: Arc::from([]),
        }
    }

    fn route_module(
        name: &str,
        version: &str,
        repo_name: &str,
        registry: bool,
    ) -> HostSelectedModuleEntry {
        let key = route_key(name, version);
        let module = NonrootModuleBuilder::new(
            NonrootModuleKey::new(name, version),
            name,
            version,
            repo_name,
        )
        .build()
        .unwrap();
        let provenance = if registry {
            HostDiscoveredModuleProvenance::Registry {
                selected_registry: crate::RegistryBaseUrl::new(REGISTRY),
                module_file_attempts: Arc::from([]),
            }
        } else {
            HostDiscoveredModuleProvenance::BuiltinBazelTools {
                route_identity: crate::BuiltinBazelToolsSnapshot::CURRENT.route_identity(),
                module_sha256: [0; 32],
            }
        };
        HostSelectedModuleEntry {
            key,
            source: HostGraphModuleSource::Discovered(Arc::new(HostDiscoveredModule {
                module,
                provenance,
            })),
            dependencies: Arc::from([]),
            original_dependencies: Arc::from([]),
            nodep_dependencies: Arc::from([]),
        }
    }

    fn route_graph(
        entries: impl IntoIterator<Item = HostSelectedModuleEntry>,
    ) -> crate::selected_graph::HostSelectedModuleGraph {
        let resolved: Arc<[_]> = entries.into_iter().collect();
        crate::selected_graph::HostSelectedModuleGraph {
            resolved: resolved.clone(),
            unpruned: resolved,
        }
    }

    fn route_spec(module: HostGraphModuleKey) -> HostSelectedRegistryRepoSpec {
        HostSelectedRegistryRepoSpec {
            module,
            policy: policy(REGISTRY, &[]),
            module_file_attempts: Arc::from([]),
            source_json: RegistryFileObservation {
                url: RegistryFileUrl::new("https://registry.invalid/source.json"),
                value: RegistryFileValue::NotFound {
                    source: crate::RegistryNotFoundSource::Io404,
                    recordable_remote_expectation: None,
                },
            },
            registry_json: None,
            effective_override: HostEffectiveModuleOverride::None,
            repo_spec: repo_spec(
                "@@bazel_tools//tools/build_defs/repo:http.bzl",
                "http_archive",
                SmallMap::new(),
            ),
        }
    }

    fn policy(registry: &str, mirrors: &[&str]) -> SelectedRegistryPolicyIdentity {
        SelectedRegistryPolicyIdentity {
            original_registry: registry.into(),
            resolved_registry: registry.into(),
            scheme: if registry.starts_with("file:") {
                HostRegistryScheme::File
            } else {
                HostRegistryScheme::Https
            },
            known_file_hashes_mode: RegistryKnownFileHashesMode::UseAndUpdate,
            vendor_directory: None,
            module_mirrors: mirrors.iter().map(|value| (*value).into()).collect(),
        }
    }

    fn source(json: &str) -> SourceJson {
        parse_source_json(&module(), json.as_bytes()).unwrap()
    }

    fn string_attr<'a>(spec: &'a RepoSpec, name: &str) -> &'a str {
        match spec.attributes.get(name).unwrap() {
            OverrideAttributeValue::String(value) => value,
            value => panic!("{name} was not a string: {value:?}"),
        }
    }

    fn list_attr<'a>(spec: &'a RepoSpec, name: &str) -> Vec<&'a str> {
        match spec.attributes.get(name).unwrap() {
            OverrideAttributeValue::Iterable(values) => values
                .iter()
                .map(|value| match value {
                    OverrideAttributeValue::String(value) => value.as_str(),
                    value => panic!("{name} contained non-string {value:?}"),
                })
                .collect(),
            value => panic!("{name} was not a list: {value:?}"),
        }
    }

    #[test]
    fn source_json_defaults_and_typed_failures() {
        let parsed = source(r#"{"url":"https://example.test/a","integrity":"sha256-x"}"#);
        assert_eq!(parsed.source_type, None);
        assert_eq!(parsed.patch_strip, 0);
        assert!(!parsed.init_submodules);
        assert!(parse_source_json(&module(), br#"{"patch_strip":2147483648}"#).is_err());
        assert!(parse_source_json(&module(), br#"{"mirror_urls":"bad"}"#).is_err());
        assert!(parse_source_json(&module(), br#"{"patches":{"x":1}}"#).is_err());
        assert!(parse_source_json(&module(), b"[]").is_err());
    }

    #[test]
    fn blank_registry_json_is_absent() {
        assert!(optional_registry_json(&module(), b"").unwrap().is_none());
        assert!(
            optional_registry_json(&module(), " \n\t\u{2003}".as_bytes())
                .unwrap()
                .is_none()
        );
        assert!(optional_registry_json(&module(), b"{}").unwrap().is_some());
        assert!(optional_registry_json(&module(), b"not json").is_err());
    }

    #[test]
    fn normalized_path_removes_dot_parent_and_repeated_separators() {
        assert_eq!(
            normalized_path(Path::new("/a//b/./c/../d")).unwrap(),
            "/a/b/d"
        );
        assert!(normalized_path(Path::new("../../escape")).is_err());
    }

    #[test]
    fn local_path_decodes_file_registry_and_normalizes() {
        let parsed = source(r#"{"type":"local_path","path":"pkg/./x/../repo"}"#);
        let registry = RegistryJson {
            mirrors: Vec::new(),
            module_base_path: Some("modules/../base".to_owned()),
        };
        let spec = local_repo_spec(
            &module(),
            &parsed,
            &policy("file:///tmp/registry%20root", &[]),
            Some(&registry),
        )
        .unwrap();
        assert_eq!(
            string_attr(&spec, "path"),
            "/tmp/registry root/base/pkg/repo"
        );
    }

    #[test]
    fn local_path_absolute_and_remote_relative_boundary() {
        let absolute = source(r#"{"type":"local_path","path":"/tmp/a/../repo"}"#);
        assert_eq!(
            string_attr(
                &local_repo_spec(
                    &module(),
                    &absolute,
                    &policy("https://registry.test", &[]),
                    None
                )
                .unwrap(),
                "path"
            ),
            "/tmp/repo"
        );
        let relative = source(r#"{"type":"local_path","path":"repo"}"#);
        let relative_base = RegistryJson {
            mirrors: Vec::new(),
            module_base_path: Some("base".to_owned()),
        };
        assert!(
            local_repo_spec(
                &module(),
                &relative,
                &policy("https://registry.test", &[]),
                Some(&relative_base)
            )
            .is_err()
        );
    }

    #[test]
    fn archive_projection_has_exact_mirror_and_module_order() {
        let parsed = source(
            r#"{
              "url":"https://origin.test/pkg/a.tgz?x=1",
              "mirror_urls":["https://fallback.test/a.tgz"],
              "integrity":"sha256-source",
              "strip_prefix":"pkg",
              "patches":{"p.diff":"sha256-p"},
              "overlay":{"MODULE.bazel":"sha256-o"},
              "patch_strip":2,
              "archive_type":"tar.gz"
            }"#,
        );
        let registry = RegistryJson {
            mirrors: vec![
                "https://command.test".to_owned(),
                "https://registry.test/mirror".to_owned(),
            ],
            module_base_path: None,
        };
        let spec = archive_repo_spec(
            &module(),
            &parsed,
            "https://registry.test",
            "https://registry.test/modules/demo/1.2.3/MODULE.bazel".into(),
            [0; 32],
            &["https://command.test".into()],
            Some(&registry),
            "demo",
            "1.2.3",
        )
        .unwrap();
        assert_eq!(
            list_attr(&spec, "urls"),
            vec![
                "https://command.test/origin.test/pkg/a.tgz?x=1",
                "https://registry.test/mirror/origin.test/pkg/a.tgz?x=1",
                "https://origin.test/pkg/a.tgz?x=1",
                "https://fallback.test/a.tgz",
            ]
        );
        assert_eq!(string_attr(&spec, "type"), "tar.gz");
        assert_eq!(
            string_attr(&spec, "remote_module_file_integrity"),
            format!("sha256-{}", BASE64.encode([0; 32]))
        );
        assert_eq!(
            spec.rule_id.bzl_file,
            CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl").unwrap()
        );
    }

    #[test]
    fn archive_rejects_missing_and_relative_primary() {
        assert!(
            archive_repo_spec(
                &module(),
                &source(r#"{"integrity":"sha256-x"}"#),
                "https://registry.test",
                "module".into(),
                [1; 32],
                &[],
                None,
                "demo",
                "1.2.3"
            )
            .is_err()
        );
        assert!(
            archive_repo_spec(
                &module(),
                &source(r#"{"url":"relative","integrity":"sha256-x"}"#),
                "https://registry.test",
                "module".into(),
                [1; 32],
                &[],
                None,
                "demo",
                "1.2.3"
            )
            .is_err()
        );
    }

    #[test]
    fn git_projection_preserves_defaults_and_remote_inputs() {
        let parsed = source(
            r#"{"type":"git_repository","remote":"https://git.test/r",
            "tag":"v1","init_submodules":true,"patches":{"p":"sha256-p"}}"#,
        );
        let spec = git_repo_spec(
            &parsed,
            "https://registry.test",
            "module".into(),
            [2; 32],
            "demo",
            "1.2.3",
        );
        assert_eq!(string_attr(&spec, "remote"), "https://git.test/r");
        assert_eq!(string_attr(&spec, "tag"), "v1");
        assert_eq!(
            spec.attributes.get("init_submodules"),
            Some(&OverrideAttributeValue::Bool(true))
        );
        assert_eq!(
            spec.attributes.get("recursive_init_submodules"),
            Some(&OverrideAttributeValue::Bool(true))
        );
        assert_eq!(
            spec.attributes.get("verbose"),
            Some(&OverrideAttributeValue::Bool(false))
        );
    }

    #[test]
    fn registry_single_augmentation_is_structural() {
        let base = git_repo_spec(
            &source(r#"{"type":"git_repository"}"#),
            "https://registry.test",
            "module".into(),
            [3; 32],
            "demo",
            "1.2.3",
        );
        let effective = HostEffectiveModuleOverride::Root {
            override_: RootModuleOverride::RegistrySingle(RegistrySingleOverride {
                version: "".into(),
                registry: "".into(),
                patches: Arc::from([
                    CanonicalLabel::parse("@@//:one.patch").unwrap(),
                    CanonicalLabel::parse("@@//:two.patch").unwrap(),
                ]),
                patch_cmds: Arc::from([CompactString::new("echo patch")]),
                patch_strip: 3,
            }),
        };
        let augmented = augment_override(&module(), base.clone(), &effective).unwrap();
        assert_eq!(list_attr(&augmented, "patch_args"), vec!["-p3"]);
        assert_eq!(list_attr(&augmented, "patch_cmds"), vec!["echo patch"]);
        assert_ne!(augmented, base);
        assert!(
            augment_override(
                &module(),
                base,
                &HostEffectiveModuleOverride::Command {
                    path: NormalizedAbsolutePath::new("/tmp").unwrap(),
                    override_: RootModuleOverride::NonRegistry(repo_spec(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                        "local_repository",
                        SmallMap::new(),
                    )),
                }
            )
            .is_err()
        );
    }

    #[test]
    fn module_hash_and_key_validity_are_exact() {
        let attempts = [
            RegistryModuleFileAttempt {
                url: RegistryFileUrl::new("https://a/MODULE.bazel"),
                sha256: None,
            },
            RegistryModuleFileAttempt {
                url: RegistryFileUrl::new("https://b/MODULE.bazel"),
                sha256: Some([4; 32]),
            },
        ];
        assert_eq!(
            module_file_identity(&module(), &attempts).unwrap(),
            ("https://b/MODULE.bazel".into(), [4; 32])
        );
        assert!(module_file_identity(&module(), &attempts[..1]).is_err());
        let complete =
            SourcePreparationOutcome::Complete(Arc::new(Ok(HostSelectedRegistryRepoSpecs {
                entries: Arc::from([]),
            })));
        assert!(HostSelectedRegistryRepoSpecsKey::validity(&complete));
        assert!(HostSelectedRegistryRepoSpecsKey::equality(
            &complete, &complete
        ));
    }

    #[test]
    fn canonical_name_table_is_exact() {
        let key = |name: &str, version: &str| HostGraphModuleKey::Module {
            name: name.into(),
            version: BazelModuleVersion::parse(version).unwrap(),
        };
        let mut unique = SmallMap::new();
        unique.insert("dep".into(), 1);
        assert_eq!(
            canonical_repo(&HostGraphModuleKey::Root, &unique)
                .unwrap()
                .as_str(),
            ""
        );
        assert_eq!(
            canonical_repo(&key("dep", "1+build"), &unique)
                .unwrap()
                .as_str(),
            "dep+"
        );
        assert_eq!(
            canonical_repo(&key("bazel_tools", ""), &unique)
                .unwrap()
                .as_str(),
            "bazel_tools"
        );
        assert_eq!(
            canonical_repo(&key("platforms", "1"), &unique)
                .unwrap()
                .as_str(),
            "platforms"
        );
        let mut multiple = SmallMap::new();
        multiple.insert("dep".into(), 2);
        assert_eq!(
            canonical_repo(&key("dep", "1.2+build"), &multiple)
                .unwrap()
                .as_str(),
            "dep+1.2"
        );
        assert!(canonical_repo(&key("dep", ""), &multiple).is_err());
        assert!(matches!(
            canonical_lookup(
                [key("platforms", "1"), key("platforms", "2")]
                    .iter()
            ),
            Err(HostSelectedModuleRoutesError::CanonicalCollision {
                canonical_repo,
                ..
            }) if canonical_repo.as_str() == "platforms"
        ));
    }

    #[test]
    fn pure_mvo_context_mapping_and_route_errors_are_typed() {
        let v1 = route_key("dep", "1");
        let v2 = route_key("dep", "2");
        let graph = route_graph([
            route_root(
                [("first", v1.clone()), ("second", v2.clone())],
                Some("root_self"),
            ),
            route_module("dep", "1", "dep", true),
            route_module("dep", "2", "dep", true),
        ]);
        let specs = HostSelectedRegistryRepoSpecs {
            entries: Arc::from([route_spec(v1.clone()), route_spec(v2.clone())]),
        };
        let routes = selected_routes(&graph, &specs).unwrap();
        assert_eq!(
            routes
                .entries
                .iter()
                .map(|route| route.canonical_repo.as_str())
                .collect::<Vec<_>>(),
            ["", "dep+1", "dep+2"]
        );
        let root = &routes.entries[0].mapping;
        assert_eq!(root.context_repo.as_str(), "");
        assert_eq!(root.entries.get("root_self").unwrap().as_str(), "");
        assert_eq!(root.entries.get("first").unwrap().as_str(), "dep+1");
        assert_eq!(root.entries.get("second").unwrap().as_str(), "dep+2");
        assert_eq!(routes.entries[1].mapping.context_repo.as_str(), "dep+1");
        assert_eq!(
            routes.entries[1]
                .mapping
                .entries
                .get("dep")
                .unwrap()
                .as_str(),
            "dep+1"
        );
        assert_eq!(routes.entries[2].mapping.context_repo.as_str(), "dep+2");
        assert_eq!(
            routes.entries[2]
                .mapping
                .entries
                .get("dep")
                .unwrap()
                .as_str(),
            "dep+2"
        );

        let conflict = route_graph([
            route_root([("same", v1.clone())], Some("same")),
            route_module("dep", "1", "dep", false),
        ]);
        assert!(matches!(
            selected_routes(
                &conflict,
                &HostSelectedRegistryRepoSpecs { entries: Arc::from([]) }
            ),
            Err(HostSelectedModuleRoutesError::Invalid { message, .. })
                if message.contains("maps to both")
        ));

        let absent = route_graph([route_root([("missing", v1.clone())], None)]);
        assert!(matches!(
            selected_routes(
                &absent,
                &HostSelectedRegistryRepoSpecs { entries: Arc::from([]) }
            ),
            Err(HostSelectedModuleRoutesError::Invalid { message, .. })
                if message == "selected dependency is absent"
        ));

        let registry_graph = route_graph([
            route_root([("dep", v1.clone())], None),
            route_module("dep", "1", "dep", true),
        ]);
        assert!(matches!(
            selected_routes(
                &registry_graph,
                &HostSelectedRegistryRepoSpecs { entries: Arc::from([]) }
            ),
            Err(HostSelectedModuleRoutesError::RegistryMismatch { message, .. })
                if message == "selected registry module has no RepoSpec"
        ));
        assert!(matches!(
            selected_routes(
                &registry_graph,
                &HostSelectedRegistryRepoSpecs {
                    entries: Arc::from([route_spec(v1.clone()), route_spec(v1.clone())])
                }
            ),
            Err(HostSelectedModuleRoutesError::RegistryMismatch { message, .. })
                if message == "duplicate selected registry RepoSpec"
        ));

        let extra = route_graph([route_root([], None)]);
        assert!(matches!(
            selected_routes(
                &extra,
                &HostSelectedRegistryRepoSpecs {
                    entries: Arc::from([route_spec(v1.clone())])
                }
            ),
            Err(HostSelectedModuleRoutesError::RegistryMismatch { message, .. })
                if message == "RepoSpec is not present in the selected graph"
        ));
        let nonregistry = route_graph([
            route_root([("dep", v1.clone())], None),
            route_module("dep", "1", "dep", false),
        ]);
        assert!(matches!(
            selected_routes(
                &nonregistry,
                &HostSelectedRegistryRepoSpecs {
                    entries: Arc::from([route_spec(v1)])
                }
            ),
            Err(HostSelectedModuleRoutesError::RegistryMismatch { message, .. })
                if message == "nonregistry selected module has a registry RepoSpec"
        ));
    }

    #[tokio::test]
    async fn real_aggregate_selected_only_lifecycle_and_reuse() {
        const MODULE_URL: &str = "https://registry.invalid/modules/dep/1/MODULE.bazel";
        const SOURCE_URL: &str = "https://registry.invalid/modules/dep/1/source.json";
        const REGISTRY_JSON_URL: &str = "https://registry.invalid/bazel_registry.json";
        const MODULE_A: &[u8] = b"module(name='dep', version='1')\n";
        const MODULE_B: &[u8] = b"module(name='dep', version='1')\n# changed hash\n";
        const SOURCE_A: &[u8] = br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#;
        const SOURCE_B: &[u8] = br#"{"url":"https://origin.test/b.tgz","integrity":"sha256-b"}"#;
        const REGISTRY_A: &[u8] = br#"{"mirrors":["https://registry-mirror-a.test"]}"#;
        const REGISTRY_B: &[u8] = br#"{"mirrors":["https://registry-mirror-b.test"]}"#;
        let io = Arc::new(TrackingRegistryIo::new([
            (MODULE_URL, MODULE_A),
            (SOURCE_URL, SOURCE_A),
            (REGISTRY_JSON_URL, REGISTRY_A),
        ]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.clone());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let root = "module(name='bazel_tools')\n\
                    single_version_override(module_name='dep', patch_strip=0)\n\
                    bazel_dep(name='dep', version='1')\n";

        let a = compute_real(&dice, root, 1, &["https://command-a.test"], true).await;
        let warm_a = compute_real(&dice, root, 1, &["https://command-a.test"], true).await;
        assert!(HostSelectedRegistryRepoSpecsKey::equality(&a, &warm_a));
        assert!(HostSelectedRegistryRepoSpecsKey::validity(&a));
        let calls_after_warm = io.calls();
        assert!(calls_after_warm.iter().any(|url| url == SOURCE_URL));
        assert!(calls_after_warm.iter().any(|url| url == REGISTRY_JSON_URL));
        assert_eq!(
            calls_after_warm
                .iter()
                .filter(|url| *url == SOURCE_URL)
                .count(),
            1,
            "warm aggregate must reuse the selected source observation"
        );

        io.replace(SOURCE_URL, SOURCE_B);
        let source_b = compute_real(&dice, root, 2, &["https://command-a.test"], true).await;
        assert!(!HostSelectedRegistryRepoSpecsKey::equality(&a, &source_b));
        io.replace(SOURCE_URL, SOURCE_A);
        let source_a = compute_real(&dice, root, 3, &["https://command-a.test"], true).await;
        assert!(HostSelectedRegistryRepoSpecsKey::equality(&a, &source_a));

        io.replace(REGISTRY_JSON_URL, REGISTRY_B);
        let registry_b = compute_real(&dice, root, 4, &["https://command-a.test"], true).await;
        assert!(!HostSelectedRegistryRepoSpecsKey::equality(&a, &registry_b));
        io.replace(REGISTRY_JSON_URL, REGISTRY_A);
        let registry_a = compute_real(&dice, root, 5, &["https://command-a.test"], true).await;
        assert!(HostSelectedRegistryRepoSpecsKey::equality(&a, &registry_a));

        io.replace(MODULE_URL, MODULE_B);
        let module_b = compute_real(&dice, root, 6, &["https://command-a.test"], true).await;
        assert!(!HostSelectedRegistryRepoSpecsKey::equality(&a, &module_b));
        io.replace(MODULE_URL, MODULE_A);
        let module_a = compute_real(&dice, root, 7, &["https://command-a.test"], true).await;
        assert!(HostSelectedRegistryRepoSpecsKey::equality(&a, &module_a));

        let mirror_b = compute_real(&dice, root, 8, &["https://command-b.test"], true).await;
        assert!(!HostSelectedRegistryRepoSpecsKey::equality(&a, &mirror_b));
        let mirror_a = compute_real(&dice, root, 9, &["https://command-a.test"], true).await;
        assert!(HostSelectedRegistryRepoSpecsKey::equality(&a, &mirror_a));

        let override_b = compute_real(
            &dice,
            &root.replace("patch_strip=0", "patch_strip=2"),
            10,
            &["https://command-a.test"],
            true,
        )
        .await;
        assert!(!HostSelectedRegistryRepoSpecsKey::equality(&a, &override_b));
        let override_a = compute_real(&dice, root, 11, &["https://command-a.test"], true).await;
        assert!(HostSelectedRegistryRepoSpecsKey::equality(&a, &override_a));
    }

    #[tokio::test]
    async fn real_aggregate_root_builtin_and_nonregistry_do_zero_registry_io() {
        let io = Arc::new(TrackingRegistryIo::new([]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.clone());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let mut root = "module(name='root')\n".to_owned();
        for name in LOCAL_MODULES {
            root.push_str(&format!(
                "local_path_override(module_name='{name}', path='{name}')\n\
                 bazel_dep(name='{name}', version='1')\n"
            ));
        }
        let value = compute_real(&dice, &root, 1, &[], true).await;
        assert!(matches!(
            value,
            SourcePreparationOutcome::Complete(value)
                if value.as_ref().as_ref().unwrap().entries.is_empty()
        ));
        let routes = compute_real_routes(&dice, &root, 1, &[], true).await;
        let SourcePreparationOutcome::Complete(routes) = routes else {
            panic!("selected local routes must complete");
        };
        let routes = routes.as_ref().as_ref().unwrap();
        assert_eq!(routes.entries.len(), 2 + LOCAL_MODULES.len());
        assert!(
            routes
                .entries
                .iter()
                .all(|route| route.registry_repo_spec.is_none())
        );
        assert!(routes.entries.iter().any(|route| {
            matches!(
                &route.entry.source,
                HostGraphModuleSource::Discovered(module)
                    if matches!(
                        &module.provenance,
                        HostDiscoveredModuleProvenance::NonRegistry { closure }
                            if closure.repo_spec().rule_id.rule_name == "local_repository"
                    )
            )
        }));
        assert!(io.calls().is_empty());
    }

    #[tokio::test]
    async fn real_routes_mapping_registry_lifecycle_and_reuse() {
        const MODULE_URL: &str = "https://registry.invalid/modules/dep/1/MODULE.bazel";
        const SOURCE_URL: &str = "https://registry.invalid/modules/dep/1/source.json";
        const SOURCE_A: &[u8] = br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#;
        const SOURCE_B: &[u8] = br#"{"url":"https://origin.test/b.tgz","integrity":"sha256-b"}"#;
        let io = Arc::new(TrackingRegistryIo::new([
            (MODULE_URL, b"module(name='dep', version='1')\n" as &[u8]),
            (SOURCE_URL, SOURCE_A),
            (
                "https://registry.invalid/bazel_registry.json",
                b"{}" as &[u8],
            ),
        ]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.clone());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let root = "module(name='bazel_tools', repo_name='root_self')\n\
                    bazel_dep(name='dep', version='1', repo_name='alias')\n";

        let a = compute_real_routes(&dice, root, 1, &[], true).await;
        let warm = compute_real_routes(&dice, root, 1, &[], true).await;
        assert!(HostSelectedModuleRoutesKey::equality(&a, &warm));
        assert!(HostSelectedModuleRoutesKey::validity(&a));
        let SourcePreparationOutcome::Complete(value) = &a else {
            panic!("registry routes must complete")
        };
        let routes = &value.as_ref().as_ref().unwrap().entries;
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].canonical_repo.as_str(), "");
        assert_eq!(routes[0].mapping.context_repo.as_str(), "");
        assert_eq!(
            routes[0].mapping.entries.get("alias").unwrap().as_str(),
            "dep+"
        );
        assert_eq!(
            routes[0].mapping.entries.get("root_self").unwrap().as_str(),
            ""
        );
        assert_eq!(routes[1].canonical_repo.as_str(), "dep+");
        assert_eq!(routes[1].mapping.context_repo.as_str(), "dep+");
        assert_eq!(
            routes[1].mapping.entries.get("dep").unwrap().as_str(),
            "dep+"
        );
        assert!(routes[0].registry_repo_spec.is_none());
        assert!(routes[1].registry_repo_spec.is_some());
        assert_eq!(
            io.calls().iter().filter(|url| *url == SOURCE_URL).count(),
            1,
            "warm routes reuse the selected source owner"
        );

        io.replace(SOURCE_URL, SOURCE_B);
        let b = compute_real_routes(&dice, root, 2, &[], true).await;
        assert!(!HostSelectedModuleRoutesKey::equality(&a, &b));
        io.replace(SOURCE_URL, SOURCE_A);
        let restored = compute_real_routes(&dice, root, 3, &[], true).await;
        assert!(HostSelectedModuleRoutesKey::equality(&a, &restored));
    }

    #[tokio::test]
    async fn real_aggregate_fetches_no_source_for_unselected_version() {
        const DEP_1_MODULE: &str = "https://registry.invalid/modules/dep/1/MODULE.bazel";
        const DEP_2_MODULE: &str = "https://registry.invalid/modules/dep/2/MODULE.bazel";
        const HOLDER_MODULE: &str = "https://registry.invalid/modules/holder/1/MODULE.bazel";
        const DEP_1_SOURCE: &str = "https://registry.invalid/modules/dep/1/source.json";
        const DEP_2_SOURCE: &str = "https://registry.invalid/modules/dep/2/source.json";
        const HOLDER_SOURCE: &str = "https://registry.invalid/modules/holder/1/source.json";
        let source = br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#;
        let io = Arc::new(TrackingRegistryIo::new([
            (DEP_1_MODULE, b"module(name='dep', version='1')\n" as &[u8]),
            (DEP_2_MODULE, b"module(name='dep', version='2')\n" as &[u8]),
            (
                HOLDER_MODULE,
                b"module(name='holder', version='1')\nbazel_dep(name='dep', version='2')\n"
                    as &[u8],
            ),
            (DEP_2_SOURCE, source as &[u8]),
            (HOLDER_SOURCE, source as &[u8]),
            (
                "https://registry.invalid/bazel_registry.json",
                b"{}" as &[u8],
            ),
        ]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.clone());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let value = compute_real(
            &dice,
            "module(name='bazel_tools')\n\
             bazel_dep(name='dep', version='1')\n\
             bazel_dep(name='holder', version='1')\n",
            1,
            &[],
            true,
        )
        .await;
        assert!(matches!(
            value,
            SourcePreparationOutcome::Complete(value)
                if value.as_ref().as_ref().unwrap().entries.len() == 2
        ));
        let calls = io.calls();
        assert!(!calls.iter().any(|url| url == DEP_1_SOURCE));
        assert!(calls.iter().any(|url| url == DEP_2_SOURCE));
        assert!(calls.iter().any(|url| url == HOLDER_SOURCE));
    }

    #[tokio::test]
    async fn real_aggregate_need_is_invalid_and_not_self_equal() {
        let io = Arc::new(TrackingRegistryIo::new([(
            "https://registry.invalid/modules/dep/1/MODULE.bazel",
            b"module(name='dep', version='1')\n" as &[u8],
        )]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io);
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let need = compute_real(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n",
            1,
            &[],
            false,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostSelectedRegistryRepoSpecsKey::validity(&need));
        assert!(!HostSelectedRegistryRepoSpecsKey::equality(&need, &need));
        let route_need = compute_real_routes(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n",
            2,
            &[],
            false,
        )
        .await;
        assert!(matches!(route_need, SourcePreparationOutcome::Need(_)));
        assert!(!HostSelectedModuleRoutesKey::validity(&route_need));
        assert!(!HostSelectedModuleRoutesKey::equality(
            &route_need,
            &route_need
        ));
    }

    #[tokio::test]
    async fn real_aggregate_completed_graph_error_beats_compatible_needs() {
        let io = Arc::new(TrackingRegistryIo::new([]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io);
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let value = compute_real(
            &dice,
            "module(name='bazel_tools')\n\
             local_path_override(module_name='local_a', path='a')\n\
             local_path_override(module_name='local_c', path='c')\n\
             bazel_dep(name='local_a', version='1')\n\
             bazel_dep(name='missing_b', version='1')\n\
             bazel_dep(name='local_c', version='1')\n",
            1,
            &[],
            true,
        )
        .await;
        assert!(matches!(
            value,
            SourcePreparationOutcome::Complete(error)
                if matches!(
                    error.as_ref(),
                    Err(HostSelectedRegistryRepoSpecsError::Graph(
                        HostSelectedModuleGraphError::DiscoveryLeaf {
                            module: HostGraphModuleKey::Module { name, .. },
                            ..
                        }
                    )) if name == "missing_b"
                )
        ));
        let routes = compute_real_routes(
            &dice,
            "module(name='bazel_tools')\n\
             local_path_override(module_name='local_a', path='a')\n\
             bazel_dep(name='local_a', version='1')\n\
             bazel_dep(name='missing_b', version='1')\n",
            2,
            &[],
            true,
        )
        .await;
        assert!(matches!(
            routes,
            SourcePreparationOutcome::Complete(error)
                if matches!(
                    error.as_ref(),
                    Err(HostSelectedModuleRoutesError::Graph(
                        HostSelectedModuleGraphError::DiscoveryLeaf {
                            module: HostGraphModuleKey::Module { name, .. },
                            ..
                        }
                    )) if name == "missing_b"
                )
        ));
    }
}
