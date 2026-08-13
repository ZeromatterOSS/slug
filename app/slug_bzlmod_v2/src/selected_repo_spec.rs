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
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathOutcome;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;
use url::Url;

use crate::NonrootRepoOverride;
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
use crate::module_eval::RootExtensionUsage;
use crate::module_eval::RootModuleFilesKey;
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
enum HostSelectedExtensionIsolation {
    Root {
        proxy: CompactString,
    },
    Module {
        module: HostGraphModuleKey,
        proxy: CompactString,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostSelectedExtensionId {
    bzl_file: CanonicalLabel,
    extension_name: CompactString,
    isolation: Option<HostSelectedExtensionIsolation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostSelectedExtensionUsage {
    owner: HostGraphModuleKey,
    id: HostSelectedExtensionId,
    unique_name: CanonicalRepoName,
    imports: Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    validation_imports: Arc<[HostSelectedExtensionDefinitionImport]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostSelectedExtensionOverride {
    id: HostSelectedExtensionId,
    generated_name: CompactString,
    replacement: CanonicalRepoName,
    must_exist: bool,
    location: crate::LogicalSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostSelectedExtensionMappings {
    routes: Arc<HostSelectedModuleRoutes>,
    root_usages: Arc<[RootExtensionUsage]>,
    usages: Arc<[HostSelectedExtensionUsage]>,
    overrides: Arc<[HostSelectedExtensionOverride]>,
    base_mappings: Arc<[HostSelectedRepositoryMapping]>,
    mappings: Arc<[HostSelectedRepositoryMapping]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostSelectedExtensionMappingsError {
    Routes(HostSelectedModuleRoutesError),
    RoutesCompute(CompactString),
    RootFiles(CompactString),
    RootFilesCompute(CompactString),
    Invalid {
        owner: HostGraphModuleKey,
        message: CompactString,
    },
}

impl fmt::Display for HostSelectedExtensionMappingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HostSelectedExtensionMappingsError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostSelectedExtensionMappingsKey {
    workspace: NormalizedAbsolutePath,
}

impl HostSelectedExtensionMappingsKey {
    fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostSelectedExtensionMappingsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-selected-extension-mappings:{}", self.workspace)
    }
}

type ExtensionMappingsOutcome = SourcePreparationOutcome<
    Arc<Result<HostSelectedExtensionMappings, HostSelectedExtensionMappingsError>>,
>;

fn extension_invalid(
    owner: &HostGraphModuleKey,
    message: impl Into<CompactString>,
) -> HostSelectedExtensionMappingsError {
    HostSelectedExtensionMappingsError::Invalid {
        owner: owner.clone(),
        message: message.into(),
    }
}

fn resolve_extension_label(
    owner: &HostGraphModuleKey,
    raw: &str,
    mapping: &HostSelectedRepositoryMapping,
) -> Result<CanonicalLabel, HostSelectedExtensionMappingsError> {
    if let Some(label) = raw.strip_prefix("//") {
        let spelling = if mapping.context_repo.is_root() {
            format!("@@//{label}")
        } else {
            format!("@@{}//{label}", mapping.context_repo.as_str())
        };
        return CanonicalLabel::parse(&spelling)
            .map_err(|message| extension_invalid(owner, message));
    }
    let rest = raw
        .strip_prefix('@')
        .ok_or_else(|| extension_invalid(owner, "extension label is not absolute"))?;
    let (apparent, label) = rest
        .split_once("//")
        .ok_or_else(|| extension_invalid(owner, "extension label has no package separator"))?;
    let canonical = if apparent.is_empty() {
        CanonicalRepoName::root()
    } else {
        let apparent =
            ApparentRepoName::new(apparent).map_err(|message| extension_invalid(owner, message))?;
        mapping.entries.get(&apparent).cloned().ok_or_else(|| {
            extension_invalid(
                owner,
                format!("extension label repository '{apparent}' is not visible"),
            )
        })?
    };
    let spelling = if canonical.is_root() {
        format!("@@//{label}")
    } else {
        format!("@@{}//{label}", canonical.as_str())
    };
    CanonicalLabel::parse(&spelling).map_err(|message| extension_invalid(owner, message))
}

fn generated_repo(
    owner: &HostGraphModuleKey,
    unique_name: &CanonicalRepoName,
    exported: &str,
) -> Result<CanonicalRepoName, HostSelectedExtensionMappingsError> {
    CanonicalRepoName::new(format!("{}+{exported}", unique_name.as_str()))
        .map_err(|message| extension_invalid(owner, message))
}

struct UsageInput<'a> {
    owner: &'a HostGraphModuleKey,
    bzl_label: &'a str,
    extension_name: &'a str,
    proxies: &'a [crate::NonrootExtensionProxy],
    overrides: &'a SmallMap<CompactString, NonrootRepoOverride>,
    isolation_proxy: Option<&'a str>,
    root: bool,
}

fn extension_unique_candidate(id: &HostSelectedExtensionId, collision: usize) -> String {
    let label = id.bzl_file.to_string();
    let repo = label
        .strip_prefix("@@")
        .and_then(|value| value.split_once("//"))
        .map(|(repo, _)| repo)
        .unwrap_or_default();
    let name = id
        .extension_name
        .split_ascii_whitespace()
        .next_back()
        .unwrap_or(id.extension_name.as_str());
    let collision = (collision > 1)
        .then(|| collision.to_string())
        .unwrap_or_default();
    match &id.isolation {
        None => format!("{repo}+{name}{collision}"),
        Some(HostSelectedExtensionIsolation::Root { proxy }) => {
            format!("{repo}+_{name}{collision}+++{proxy}")
        }
        Some(HostSelectedExtensionIsolation::Module { module, proxy }) => {
            let module = match module {
                HostGraphModuleKey::Root => String::new(),
                HostGraphModuleKey::Module { name, version } => {
                    format!("{name}+{}", version.normalized())
                }
            };
            format!("{repo}+_{name}{collision}+{module}+{proxy}")
        }
    }
}

fn selected_extension_mappings(
    routes: Arc<HostSelectedModuleRoutes>,
    root_usages: Arc<[RootExtensionUsage]>,
) -> Result<HostSelectedExtensionMappings, HostSelectedExtensionMappingsError> {
    let root_route = routes
        .entries
        .iter()
        .find(|route| matches!(route.entry.key, HostGraphModuleKey::Root))
        .ok_or_else(|| extension_invalid(&HostGraphModuleKey::Root, "root route is absent"))?;
    let mut inputs = Vec::new();
    for usage in root_usages.iter() {
        inputs.push(UsageInput {
            owner: &root_route.entry.key,
            bzl_label: usage.bzl_label.as_str(),
            extension_name: usage.extension_name.as_str(),
            proxies: &usage.proxies,
            overrides: &usage.repo_overrides,
            isolation_proxy: usage
                .isolation
                .as_ref()
                .map(|isolation| isolation.exported_proxy_name.as_str()),
            root: true,
        });
    }
    for route in routes.entries.iter() {
        let HostGraphModuleSource::Discovered(module) = &route.entry.source else {
            continue;
        };
        for usage in module.module.extension_usages.iter() {
            inputs.push(UsageInput {
                owner: &route.entry.key,
                bzl_label: usage.bzl_label.as_str(),
                extension_name: usage.extension_name.as_str(),
                proxies: &usage.proxies,
                overrides: &usage.repo_overrides,
                isolation_proxy: usage
                    .isolation
                    .as_ref()
                    .map(|isolation| isolation.exported_proxy_name.as_str()),
                root: false,
            });
        }
    }

    let route_by_owner = routes
        .entries
        .iter()
        .map(|route| (route.entry.key.clone(), route))
        .collect::<SmallMap<_, _>>();
    let mut names = SmallMap::<HostSelectedExtensionId, CanonicalRepoName>::new();
    let mut claimed = SmallSet::<CanonicalRepoName>::new();
    let mut usages = Vec::new();
    let mut pending_overrides = Vec::new();
    let mut no_overrides = routes
        .entries
        .iter()
        .map(|route| (*route.mapping.entries).clone())
        .collect::<Vec<_>>();

    for input in inputs {
        let route = route_by_owner
            .get(input.owner)
            .expect("every selected usage owner has a route");
        let id = HostSelectedExtensionId {
            bzl_file: resolve_extension_label(input.owner, input.bzl_label, &route.mapping)?,
            extension_name: input.extension_name.into(),
            isolation: input.isolation_proxy.map(|proxy| {
                if input.root {
                    HostSelectedExtensionIsolation::Root {
                        proxy: proxy.into(),
                    }
                } else {
                    HostSelectedExtensionIsolation::Module {
                        module: input.owner.clone(),
                        proxy: proxy.into(),
                    }
                }
            }),
        };
        let unique_name = if let Some(existing) = names.get(&id) {
            existing.clone()
        } else {
            let mut suffix = 1usize;
            let unique = loop {
                let spelling = extension_unique_candidate(&id, suffix);
                let candidate = CanonicalRepoName::new(spelling)
                    .map_err(|message| extension_invalid(input.owner, message))?;
                if claimed.insert(candidate.clone()) {
                    break candidate;
                }
                suffix += 1;
                if suffix == usize::MAX {
                    return Err(extension_invalid(
                        input.owner,
                        "extension name space exhausted",
                    ));
                }
            };
            names.insert(id.clone(), unique.clone());
            unique
        };
        let route_index = routes
            .entries
            .iter()
            .position(|candidate| candidate.entry.key == *input.owner)
            .expect("selected owner route index exists");
        let mut imports = SmallMap::new();
        let mut validation_imports = Vec::new();
        for proxy in input.proxies {
            if proxy.imports.local_order.len() != proxy.imports.local_to_exported.len() {
                return Err(extension_invalid(
                    input.owner,
                    "extension import order and mapping differ",
                ));
            }
            for (index, local) in proxy.imports.local_order.iter().enumerate() {
                if proxy.imports.local_order[..index].contains(local) {
                    return Err(extension_invalid(
                        input.owner,
                        "extension import order contains a duplicate local name",
                    ));
                }
                let exported = proxy.imports.local_to_exported.get(local).ok_or_else(|| {
                    extension_invalid(
                        input.owner,
                        "extension import order references a missing mapping",
                    )
                })?;
                let canonical = generated_repo(input.owner, &unique_name, exported)?;
                insert_mapping(
                    input.owner,
                    &mut no_overrides[route_index],
                    local,
                    canonical.clone(),
                )
                .map_err(|error| extension_invalid(input.owner, error.to_string()))?;
                let apparent = ApparentRepoName::new(local.as_str())
                    .map_err(|message| extension_invalid(input.owner, message))?;
                imports.insert(apparent, canonical);
                validation_imports.push(HostSelectedExtensionDefinitionImport {
                    local_name: local.clone(),
                    generated_name: exported.clone(),
                    location: proxy.location.clone(),
                });
            }
        }
        if input.root {
            for (generated, replacement) in input.overrides.iter() {
                pending_overrides.push((
                    id.clone(),
                    unique_name.clone(),
                    generated.clone(),
                    replacement.clone(),
                ));
            }
        }
        usages.push(HostSelectedExtensionUsage {
            owner: input.owner.clone(),
            id,
            unique_name,
            imports: Arc::new(imports),
            validation_imports: validation_imports.into(),
        });
    }
    drop(route_by_owner);

    let root_index = routes
        .entries
        .iter()
        .position(|route| matches!(route.entry.key, HostGraphModuleKey::Root))
        .expect("root route exists");
    let mut overrides = Vec::new();
    let mut substitutions = SmallMap::new();
    for (id, unique_name, generated, replacement) in pending_overrides {
        let apparent = ApparentRepoName::new(replacement.overriding_repo_name.as_str())
            .map_err(|message| extension_invalid(&HostGraphModuleKey::Root, message))?;
        let target = no_overrides[root_index]
            .get(&apparent)
            .cloned()
            .ok_or_else(|| {
                extension_invalid(
                    &HostGraphModuleKey::Root,
                    format!("override target '{apparent}' is not visible"),
                )
            })?;
        let source = generated_repo(&HostGraphModuleKey::Root, &unique_name, &generated)?;
        if substitutions.insert(source, target.clone()).is_some() {
            return Err(extension_invalid(
                &HostGraphModuleKey::Root,
                "generated repository is overridden more than once",
            ));
        }
        overrides.push(HostSelectedExtensionOverride {
            id,
            generated_name: generated,
            replacement: target,
            must_exist: replacement.must_exist,
            location: replacement.location,
        });
    }
    let base_mappings = routes
        .entries
        .iter()
        .zip(no_overrides.iter())
        .map(|(route, entries)| HostSelectedRepositoryMapping {
            context_repo: route.mapping.context_repo.clone(),
            entries: Arc::new(entries.clone()),
        })
        .collect::<Arc<_>>();
    let mappings = routes
        .entries
        .iter()
        .zip(no_overrides)
        .map(|(route, mut entries)| {
            for canonical in entries.values_mut() {
                if let Some(replacement) = substitutions.get(canonical) {
                    *canonical = replacement.clone();
                }
            }
            HostSelectedRepositoryMapping {
                context_repo: route.mapping.context_repo.clone(),
                entries: Arc::new(entries),
            }
        })
        .collect::<Arc<_>>();
    Ok(HostSelectedExtensionMappings {
        routes,
        root_usages,
        usages: usages.into(),
        overrides: overrides.into(),
        base_mappings,
        mappings,
    })
}

#[async_trait]
impl Key for HostSelectedExtensionMappingsKey {
    type Value = ExtensionMappingsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let routes = match ctx
            .compute(&HostSelectedModuleRoutesKey::new(self.workspace.dupe()))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(routes) => Arc::new(routes.clone()),
                Err(error) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(
                        HostSelectedExtensionMappingsError::Routes(error.clone()),
                    )));
                }
            },
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    HostSelectedExtensionMappingsError::RoutesCompute(error.to_string().into()),
                )));
            }
        };
        let root_usages = match ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.as_path().to_owned(),
            })
            .await
        {
            Ok(value) => match value.as_ref() {
                Ok(files) => files.extension_usages.clone(),
                Err(error) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(
                        HostSelectedExtensionMappingsError::RootFiles(error.clone()),
                    )));
                }
            },
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    HostSelectedExtensionMappingsError::RootFilesCompute(error.to_string().into()),
                )));
            }
        };
        SourcePreparationOutcome::Complete(Arc::new(selected_extension_mappings(
            routes,
            root_usages,
        )))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionDefinitionLoadRequest {
    bzl_file: CanonicalLabel,
    extension_name: CompactString,
    unique_name: CanonicalRepoName,
    base_mapping: HostSelectedRepositoryMapping,
    mapping: HostSelectedRepositoryMapping,
    imports: Arc<[HostSelectedExtensionDefinitionImport]>,
    overrides: Arc<[HostSelectedExtensionDefinitionOverride]>,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionDefinitionImport {
    local_name: CompactString,
    generated_name: CompactString,
    location: crate::LogicalSpan,
}

impl HostSelectedExtensionDefinitionImport {
    pub fn parts(&self) -> (&str, &str, &crate::LogicalSpan) {
        (&self.local_name, &self.generated_name, &self.location)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionDefinitionOverride {
    generated_name: CompactString,
    replacement: CanonicalRepoName,
    must_exist: bool,
    location: crate::LogicalSpan,
}

impl HostSelectedExtensionDefinitionOverride {
    pub fn parts(&self) -> (&str, &CanonicalRepoName, bool) {
        (&self.generated_name, &self.replacement, self.must_exist)
    }

    pub fn location(&self) -> &crate::LogicalSpan {
        &self.location
    }
}

impl HostSelectedExtensionDefinitionLoadRequest {
    pub fn parts(
        &self,
    ) -> (
        &CanonicalLabel,
        &str,
        &CanonicalRepoName,
        &SmallMap<ApparentRepoName, CanonicalRepoName>,
    ) {
        (
            &self.bzl_file,
            &self.extension_name,
            &self.mapping.context_repo,
            &self.mapping.entries,
        )
    }

    pub fn namespace_parts(
        &self,
    ) -> (
        &CanonicalRepoName,
        &CanonicalRepoName,
        &SmallMap<ApparentRepoName, CanonicalRepoName>,
        &[HostSelectedExtensionDefinitionOverride],
    ) {
        (
            &self.unique_name,
            &self.base_mapping.context_repo,
            &self.base_mapping.entries,
            &self.overrides,
        )
    }

    pub fn validation_parts(
        &self,
    ) -> (
        &[HostSelectedExtensionDefinitionImport],
        &[HostSelectedExtensionDefinitionOverride],
    ) {
        (&self.imports, &self.overrides)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionDefinitionLoadRequests {
    workspace: NormalizedAbsolutePath,
    predecessor: Arc<HostSelectedExtensionMappings>,
    requests: Arc<[HostSelectedExtensionDefinitionLoadRequest]>,
}

impl HostSelectedExtensionDefinitionLoadRequests {
    pub fn parts(
        &self,
    ) -> (
        &NormalizedAbsolutePath,
        &[HostSelectedExtensionDefinitionLoadRequest],
    ) {
        (&self.workspace, &self.requests)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostSelectedExtensionDefinitionLoadRequestsErrorInner {
    Mappings(HostSelectedExtensionMappingsError),
    MappingsCompute(CompactString),
    Unsupported {
        owner: HostGraphModuleKey,
        id: HostSelectedExtensionId,
    },
    Invalid {
        id: HostSelectedExtensionId,
        message: CompactString,
    },
    InvalidContext(CompactString),
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionDefinitionLoadRequestsError(
    HostSelectedExtensionDefinitionLoadRequestsErrorInner,
);

impl fmt::Display for HostSelectedExtensionDefinitionLoadRequestsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::error::Error for HostSelectedExtensionDefinitionLoadRequestsError {}

fn selected_extension_definition_load_requests(
    workspace: NormalizedAbsolutePath,
    predecessor: Arc<HostSelectedExtensionMappings>,
) -> Result<
    HostSelectedExtensionDefinitionLoadRequests,
    HostSelectedExtensionDefinitionLoadRequestsError,
> {
    let root_index = predecessor
        .routes
        .entries
        .iter()
        .position(|route| matches!(route.entry.key, HostGraphModuleKey::Root))
        .ok_or_else(|| {
            HostSelectedExtensionDefinitionLoadRequestsError(
                HostSelectedExtensionDefinitionLoadRequestsErrorInner::InvalidContext(
                    "selected extension root route is absent".into(),
                ),
            )
        })?;
    let root_base_mapping = predecessor.base_mappings.get(root_index).ok_or_else(|| {
        HostSelectedExtensionDefinitionLoadRequestsError(
            HostSelectedExtensionDefinitionLoadRequestsErrorInner::InvalidContext(
                "selected extension root base mapping is absent".into(),
            ),
        )
    })?;
    let root_mapping = predecessor.mappings.get(root_index).ok_or_else(|| {
        HostSelectedExtensionDefinitionLoadRequestsError(
            HostSelectedExtensionDefinitionLoadRequestsErrorInner::InvalidContext(
                "selected extension root final mapping is absent".into(),
            ),
        )
    })?;
    let unsupported = predecessor.usages.iter().find(|usage| {
        !matches!(usage.owner, HostGraphModuleKey::Root)
            || usage.id.isolation.is_some()
            || !usage.id.bzl_file.package().repo().is_root()
            || usage.id.extension_name.split_ascii_whitespace().count() != 1
    });
    if let Some(usage) = unsupported {
        return Err(HostSelectedExtensionDefinitionLoadRequestsError(
            HostSelectedExtensionDefinitionLoadRequestsErrorInner::Unsupported {
                owner: usage.owner.clone(),
                id: usage.id.clone(),
            },
        ));
    }
    let mut seen = SmallMap::new();
    let mut namespace_owners = SmallMap::new();
    let mut requests = Vec::new();
    for usage in predecessor.usages.iter() {
        if let Some(owner) = namespace_owners.get(&usage.unique_name) {
            if owner != &usage.id {
                return Err(HostSelectedExtensionDefinitionLoadRequestsError(
                    HostSelectedExtensionDefinitionLoadRequestsErrorInner::Invalid {
                        id: usage.id.clone(),
                        message: "selected extension namespace has duplicate ownership".into(),
                    },
                ));
            }
        } else {
            namespace_owners.insert(usage.unique_name.clone(), usage.id.clone());
        }
        if let Some(unique_name) = seen.get(&usage.id) {
            if unique_name != &usage.unique_name {
                return Err(HostSelectedExtensionDefinitionLoadRequestsError(
                    HostSelectedExtensionDefinitionLoadRequestsErrorInner::Invalid {
                        id: usage.id.clone(),
                        message: "selected extension has mismatched namespace ownership".into(),
                    },
                ));
            }
            continue;
        }
        seen.insert(usage.id.clone(), usage.unique_name.clone());
        let imports = predecessor
            .usages
            .iter()
            .filter(|candidate| candidate.id == usage.id)
            .flat_map(|candidate| candidate.validation_imports.iter().cloned())
            .collect::<Arc<_>>();
        let overrides = predecessor
            .overrides
            .iter()
            .filter(|candidate| candidate.id == usage.id)
            .map(|candidate| HostSelectedExtensionDefinitionOverride {
                generated_name: candidate.generated_name.clone(),
                replacement: candidate.replacement.clone(),
                must_exist: candidate.must_exist,
                location: candidate.location.clone(),
            })
            .collect::<Arc<_>>();
        requests.push(HostSelectedExtensionDefinitionLoadRequest {
            bzl_file: usage.id.bzl_file.clone(),
            extension_name: usage.id.extension_name.clone(),
            unique_name: usage.unique_name.clone(),
            base_mapping: root_base_mapping.clone(),
            mapping: root_mapping.clone(),
            imports,
            overrides,
        });
    }
    Ok(HostSelectedExtensionDefinitionLoadRequests {
        workspace,
        predecessor,
        requests: requests.into(),
    })
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedExtensionDefinitionLoadRequestsKey {
    workspace: NormalizedAbsolutePath,
}

impl HostSelectedExtensionDefinitionLoadRequestsKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostSelectedExtensionDefinitionLoadRequestsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-selected-extension-definition-load-requests:{}",
            self.workspace
        )
    }
}

type DefinitionLoadRequestsOutcome = SourcePreparationOutcome<
    Arc<
        Result<
            HostSelectedExtensionDefinitionLoadRequests,
            HostSelectedExtensionDefinitionLoadRequestsError,
        >,
    >,
>;

#[async_trait]
impl Key for HostSelectedExtensionDefinitionLoadRequestsKey {
    type Value = DefinitionLoadRequestsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let predecessor = match ctx
            .compute(&HostSelectedExtensionMappingsKey::new(
                self.workspace.dupe(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => Arc::new(value.clone()),
                Err(error) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(
                        HostSelectedExtensionDefinitionLoadRequestsError(
                            HostSelectedExtensionDefinitionLoadRequestsErrorInner::Mappings(
                                error.clone(),
                            ),
                        ),
                    )));
                }
            },
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    HostSelectedExtensionDefinitionLoadRequestsError(
                        HostSelectedExtensionDefinitionLoadRequestsErrorInner::MappingsCompute(
                            error.to_string().into(),
                        ),
                    ),
                )));
            }
        };
        SourcePreparationOutcome::Complete(Arc::new(selected_extension_definition_load_requests(
            self.workspace.dupe(),
            predecessor,
        )))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionEvaluationInput {
    load_request: HostSelectedExtensionDefinitionLoadRequest,
    canonical_repo: CanonicalRepoName,
    name: CompactString,
    version: CompactString,
    tags: Arc<[crate::NonrootExtensionTag]>,
}

impl HostSelectedExtensionEvaluationInput {
    pub fn parts(
        &self,
    ) -> (
        &HostSelectedExtensionDefinitionLoadRequest,
        &CanonicalRepoName,
        &str,
        &str,
        bool,
        &[crate::NonrootExtensionTag],
    ) {
        (
            &self.load_request,
            &self.canonical_repo,
            &self.name,
            &self.version,
            true,
            &self.tags,
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionEvaluationInputRequests {
    load_requests: Arc<HostSelectedExtensionDefinitionLoadRequests>,
    requests: Arc<[HostSelectedExtensionEvaluationInput]>,
}

impl HostSelectedExtensionEvaluationInputRequests {
    pub fn parts(
        &self,
    ) -> (
        &HostSelectedExtensionDefinitionLoadRequests,
        &[HostSelectedExtensionEvaluationInput],
    ) {
        (&self.load_requests, &self.requests)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostSelectedExtensionEvaluationInputRequestsError {
    LoadRequests(HostSelectedExtensionDefinitionLoadRequestsError),
    LoadRequestsCompute(CompactString),
    AfterRequests {
        load_requests: Arc<HostSelectedExtensionDefinitionLoadRequests>,
        request: Option<HostSelectedExtensionDefinitionLoadRequest>,
        error: HostSelectedExtensionEvaluationInputError,
    },
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostSelectedExtensionEvaluationInputError {
    RootFiles(CompactString),
    RootFilesCompute(CompactString),
    Invalid(CompactString),
}

fn matching_root_extension_tags(
    usages: &[crate::module_eval::RootExtensionUsage],
    request: &HostSelectedExtensionDefinitionLoadRequest,
) -> Option<Arc<[crate::NonrootExtensionTag]>> {
    let mut matched = false;
    let mut tags = Vec::new();
    for usage in usages {
        if usage.isolation.is_none()
            && usage.extension_name == request.extension_name
            && resolve_extension_label(
                &HostGraphModuleKey::Root,
                usage.bzl_label.as_str(),
                &request.mapping,
            )
            .is_ok_and(|label| label == request.bzl_file)
        {
            matched = true;
            tags.extend(usage.tags.iter().cloned());
        }
    }
    matched.then(|| tags.into())
}

fn selected_extension_evaluation_input_requests(
    load_requests: Arc<HostSelectedExtensionDefinitionLoadRequests>,
    root_files: &crate::module_eval::RootModuleFiles,
) -> Result<
    HostSelectedExtensionEvaluationInputRequests,
    HostSelectedExtensionEvaluationInputRequestsError,
> {
    let invalid = |request: Option<&HostSelectedExtensionDefinitionLoadRequest>, message| {
        HostSelectedExtensionEvaluationInputRequestsError::AfterRequests {
            load_requests: load_requests.clone(),
            request: request.cloned(),
            error: HostSelectedExtensionEvaluationInputError::Invalid(message),
        }
    };
    let root_route = load_requests
        .predecessor
        .routes
        .entries
        .iter()
        .find(|route| matches!(route.entry.key, HostGraphModuleKey::Root))
        .ok_or_else(|| invalid(None, "selected root route is absent".into()))?;
    let header = root_files
        .module
        .header
        .as_ref()
        .ok_or_else(|| invalid(None, "root module header is absent".into()))?;
    if header.name.is_empty() {
        return Err(invalid(None, "root module name is empty".into()));
    }
    let version = crate::module_version::BazelModuleVersion::parse(
        header.version.as_deref().unwrap_or_default(),
    )
    .map_err(|error| invalid(None, error.to_string().into()))?;
    let requests = load_requests
        .requests
        .iter()
        .map(|request| {
            let tags = matching_root_extension_tags(&root_files.extension_usages, request)
                .ok_or_else(|| {
                    invalid(
                        Some(request),
                        "definition request has no matching root usage".into(),
                    )
                })?;
            Ok(HostSelectedExtensionEvaluationInput {
                load_request: request.clone(),
                canonical_repo: root_route.canonical_repo.clone(),
                name: header.name.clone(),
                version: version.normalized().into(),
                tags,
            })
        })
        .collect::<Result<Arc<_>, _>>()?;
    Ok(HostSelectedExtensionEvaluationInputRequests {
        load_requests,
        requests,
    })
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedExtensionEvaluationInputRequestsKey {
    workspace: NormalizedAbsolutePath,
}

impl HostSelectedExtensionEvaluationInputRequestsKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostSelectedExtensionEvaluationInputRequestsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-selected-extension-evaluation-inputs:{}",
            self.workspace
        )
    }
}

#[async_trait]
impl Key for HostSelectedExtensionEvaluationInputRequestsKey {
    type Value = SourcePreparationOutcome<
        Arc<
            Result<
                HostSelectedExtensionEvaluationInputRequests,
                HostSelectedExtensionEvaluationInputRequestsError,
            >,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let load_requests = match ctx
            .compute(&HostSelectedExtensionDefinitionLoadRequestsKey::new(
                self.workspace.dupe(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => Arc::new(value.clone()),
                Err(error) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(
                        HostSelectedExtensionEvaluationInputRequestsError::LoadRequests(
                            error.clone(),
                        ),
                    )));
                }
            },
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    HostSelectedExtensionEvaluationInputRequestsError::LoadRequestsCompute(
                        error.to_string().into(),
                    ),
                )));
            }
        };
        let root_files = match ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.as_path().to_owned(),
            })
            .await
        {
            Ok(value) => match value.as_ref() {
                Ok(value) => value.clone(),
                Err(error) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(
                        HostSelectedExtensionEvaluationInputRequestsError::AfterRequests {
                            load_requests,
                            request: None,
                            error: HostSelectedExtensionEvaluationInputError::RootFiles(
                                error.clone(),
                            ),
                        },
                    )));
                }
            },
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    HostSelectedExtensionEvaluationInputRequestsError::AfterRequests {
                        load_requests,
                        request: None,
                        error: HostSelectedExtensionEvaluationInputError::RootFilesCompute(
                            error.to_string().into(),
                        ),
                    },
                )));
            }
        };
        SourcePreparationOutcome::Complete(Arc::new(selected_extension_evaluation_input_requests(
            load_requests,
            &root_files,
        )))
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

    fn host_epoch(local_module: Option<&str>) -> PathObservationEpoch {
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
        for (index, name) in LOCAL_MODULES
            .iter()
            .copied()
            .chain(std::iter::once("bazel_tools"))
            .enumerate()
        {
            let root = format!("{WORKSPACE}/{name}");
            let module = format!("{root}/MODULE.bazel");
            let id = 10 + index as i64 * 2;
            observations.extend([
                lstat(&root, PathNodeKind::Directory, id),
                lstat(&module, PathNodeKind::RegularFile, id + 1),
                (
                    observation(&module, PathObservationOperation::FileBytes),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        if name == "local" {
                            local_module.unwrap_or(
                                "module(name='local')\np=use_extension('//:local.bzl','shared')\nuse_repo(p,'generated')\n",
                            ).as_bytes().to_vec()
                        } else {
                            format!("module(name='{name}')\n").into_bytes()
                        },
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
        let command_builtin_override = root.contains("# command_override_bazel_tools");
        let command_policy = if command_builtin_override {
            let override_value = format!("bazel_tools={WORKSPACE}/bazel_tools");
            crate::BzlmodCommandPolicyKey::from_flags_with_module_overrides(
                None,
                false,
                workspace.as_path(),
                [override_value.as_str()],
            )
            .unwrap()
        } else {
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap()
        };
        crate::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            command_policy,
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
            let local_module = if root.contains("# ordered_nonroot_imports") {
                Some(
                    "module(name='local')\np=use_extension('//:local.bzl','shared')\nuse_repo(p, first='generated_one', second='generated_two')\n",
                )
            } else if root.contains("# reversed_nonroot_imports") {
                Some(
                    "module(name='local')\np=use_extension('//:local.bzl','shared')\nuse_repo(p, second='generated_two', first='generated_one')\n",
                )
            } else {
                None
            };
            updater
                .changed_to(vec![(PathObservationEpochKey, host_epoch(local_module))])
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
        let materializations = LOCAL_MODULES
            .iter()
            .copied()
            .chain(command_builtin_override.then_some("bazel_tools"))
            .filter_map(|name| {
                let command_override = command_builtin_override && name == "bazel_tools";
                (command_override
                    || root.contains(&format!(
                        "local_path_override(module_name='{name}', path='{name}')"
                    )))
                .then(|| {
                    let repo_spec = repo_spec(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                        "local_repository",
                        SmallMap::from_iter([(
                            CompactString::new("path"),
                            OverrideAttributeValue::String(if command_override {
                                format!("{WORKSPACE}/{name}").into()
                            } else {
                                name.into()
                            }),
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

    async fn compute_real_extensions(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        include_epoch: bool,
    ) -> ExtensionMappingsOutcome {
        real_transaction(dice, root, generation, &[], include_epoch)
            .await
            .compute(&HostSelectedExtensionMappingsKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    async fn compute_real_definition_requests(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        include_epoch: bool,
    ) -> <HostSelectedExtensionDefinitionLoadRequestsKey as Key>::Value {
        real_transaction(dice, root, generation, &[], include_epoch)
            .await
            .compute(&HostSelectedExtensionDefinitionLoadRequestsKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    async fn compute_real_evaluation_inputs(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        include_epoch: bool,
    ) -> <HostSelectedExtensionEvaluationInputRequestsKey as Key>::Value {
        real_transaction(dice, root, generation, &[], include_epoch)
            .await
            .compute(&HostSelectedExtensionEvaluationInputRequestsKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    async fn compute_real_root_files(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
    ) -> Arc<Result<crate::module_eval::RootModuleFiles, CompactString>> {
        real_transaction(dice, root, generation, &[], true)
            .await
            .compute(&RootModuleFilesKey {
                workspace: Path::new(WORKSPACE).to_owned(),
            })
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

    fn test_span() -> crate::LogicalSpan {
        crate::LogicalSpan {
            file: crate::LogicalModuleFileId::new("/MODULE.bazel"),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 2,
        }
    }

    fn test_proxy(
        proxy: &str,
        imports: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> crate::NonrootExtensionProxy {
        crate::NonrootExtensionProxy {
            proxy_name: proxy.into(),
            containing_file: crate::LogicalModuleFileId::new("/MODULE.bazel"),
            dev_dependency: false,
            location: test_span(),
            imports: crate::NonrootRepoImports::from_local_to_exported(
                imports
                    .into_iter()
                    .map(|(local, exported)| {
                        (CompactString::from(local), CompactString::from(exported))
                    })
                    .collect::<SmallMap<_, _>>(),
            )
            .unwrap(),
        }
    }

    fn root_usage(
        label: &str,
        name: &str,
        proxy: crate::NonrootExtensionProxy,
        isolated: bool,
        overrides: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> crate::module_eval::RootExtensionUsage {
        crate::module_eval::RootExtensionUsage {
            bzl_label: label.into(),
            extension_name: name.into(),
            proxies: Arc::from([proxy.clone()]),
            tags: Arc::from([]),
            repo_overrides: Arc::new(
                overrides
                    .into_iter()
                    .map(|(generated, replacement)| {
                        (
                            CompactString::from(generated),
                            crate::NonrootRepoOverride {
                                overriding_repo_name: replacement.into(),
                                must_exist: true,
                                location: test_span(),
                            },
                        )
                    })
                    .collect::<SmallMap<_, _>>(),
            ),
            isolation: isolated.then(|| crate::module_eval::RootExtensionIsolationKey {
                exported_proxy_name: proxy.proxy_name,
            }),
        }
    }

    fn nonroot_usage(
        owner: &HostGraphModuleKey,
        label: &str,
        name: &str,
        proxy: crate::NonrootExtensionProxy,
        isolated: bool,
    ) -> crate::NonrootExtensionUsage {
        let HostGraphModuleKey::Module {
            name: module_name,
            version,
        } = owner
        else {
            panic!("nonroot usage owner must be a module")
        };
        crate::NonrootExtensionUsage {
            bzl_label: label.into(),
            extension_name: name.into(),
            proxies: Arc::from([proxy.clone()]),
            tags: Arc::from([]),
            repo_overrides: Arc::new(SmallMap::new()),
            isolation: isolated.then(|| crate::NonrootExtensionIsolationKey {
                module: NonrootModuleKey::new(module_name.clone(), version.normalized()),
                exported_proxy_name: proxy.proxy_name,
            }),
        }
    }

    fn route_module_with_usages(
        name: &str,
        version: &str,
        repo_name: &str,
        usages: Arc<[crate::NonrootExtensionUsage]>,
    ) -> HostSelectedModuleEntry {
        let mut entry = route_module(name, version, repo_name, false);
        let HostGraphModuleSource::Discovered(module) = &entry.source else {
            unreachable!()
        };
        let mut discovered = (**module).clone();
        discovered.module.extension_usages = usages;
        entry.source = HostGraphModuleSource::Discovered(Arc::new(discovered));
        entry
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

    #[test]
    fn pure_extension_ids_group_isolate_collide_and_map_mvo_contexts() {
        let v1 = route_key("dep", "1");
        let v2 = route_key("dep", "2");
        let dep1_usage = nonroot_usage(
            &v1,
            "@dep//:ext.bzl",
            "shared",
            test_proxy("dep_one", [("dep_one", "one")]),
            false,
        );
        let dep2_usage = nonroot_usage(
            &v2,
            "@dep//:ext.bzl",
            "shared",
            test_proxy("dep_two", [("dep_two", "two")]),
            true,
        );
        let graph = route_graph([
            route_root([("dep_one", v1.clone()), ("dep_two", v2.clone())], None),
            route_module_with_usages("dep", "1", "dep", Arc::from([dep1_usage])),
            route_module_with_usages("dep", "2", "dep", Arc::from([dep2_usage])),
        ]);
        let routes = Arc::new(
            selected_routes(
                &graph,
                &HostSelectedRegistryRepoSpecs {
                    entries: Arc::from([]),
                },
            )
            .unwrap(),
        );
        let roots = Arc::from([
            root_usage(
                "@root//:same.bzl",
                "shared",
                test_proxy("first", [("first", "one")]),
                false,
                [],
            ),
            root_usage(
                "@root//:same.bzl",
                "shared",
                test_proxy("second", [("second", "two")]),
                false,
                [],
            ),
            root_usage(
                "@root//:other.bzl",
                "shared",
                test_proxy("collision", [("collision", "three")]),
                false,
                [],
            ),
            root_usage(
                "@root//:iso.bzl",
                "shared",
                test_proxy("root_iso", [("root_iso", "four")]),
                true,
                [],
            ),
            root_usage(
                "@root//:iso_collision.bzl",
                "shared",
                test_proxy("root_iso", [("root_iso_two", "five")]),
                true,
                [],
            ),
            root_usage(
                "//:MODULE.bazel",
                "//:repo.bzl simple_repo",
                test_proxy("", [("innate", "innate")]),
                false,
                [],
            ),
        ]);
        let value = selected_extension_mappings(routes, roots).unwrap();

        assert_eq!(value.usages.len(), 8);
        assert_eq!(value.usages[0].unique_name.as_str(), "+shared");
        assert_eq!(value.usages[1].unique_name.as_str(), "+shared");
        assert_eq!(value.usages[2].unique_name.as_str(), "+shared2");
        assert_eq!(value.usages[3].unique_name.as_str(), "+_shared+++root_iso");
        assert_eq!(value.usages[4].unique_name.as_str(), "+_shared2+++root_iso");
        assert_eq!(value.usages[5].unique_name.as_str(), "+simple_repo");
        assert_eq!(value.usages[6].unique_name.as_str(), "dep+1+shared");
        assert_eq!(
            value.usages[7].unique_name.as_str(),
            "dep+2+_shared+dep+2+dep_two"
        );
        assert_ne!(value.usages[6].id, value.usages[7].id);
        assert_eq!(
            value.mappings[1].entries.get("dep_one").unwrap().as_str(),
            "dep+1+shared+one"
        );
        assert_eq!(
            value.mappings[2].entries.get("dep_two").unwrap().as_str(),
            "dep+2+_shared+dep+2+dep_two+two"
        );

        let empty = selected_extension_mappings(
            Arc::new(
                selected_routes(
                    &route_graph([route_root([], None)]),
                    &HostSelectedRegistryRepoSpecs {
                        entries: Arc::from([]),
                    },
                )
                .unwrap(),
            ),
            Arc::from([]),
        )
        .unwrap();
        assert!(empty.usages.is_empty());
        assert_eq!(empty.mappings.len(), 1);
    }

    #[test]
    fn pure_extension_projection_errors_are_typed_and_source_ordered() {
        let routes = || {
            Arc::new(
                selected_routes(
                    &route_graph([route_root([], None)]),
                    &HostSelectedRegistryRepoSpecs {
                        entries: Arc::from([]),
                    },
                )
                .unwrap(),
            )
        };
        let error = |usages| selected_extension_mappings(routes(), usages).unwrap_err();

        assert!(matches!(
            error(Arc::from([root_usage(
                "relative.bzl",
                "ext",
                test_proxy("p", []),
                false,
                [],
            )])),
            HostSelectedExtensionMappingsError::Invalid { ref message, .. }
                if message == "extension label is not absolute"
        ));
        assert!(matches!(
            error(Arc::from([root_usage(
                "@missing//:ext.bzl",
                "ext",
                test_proxy("p", []),
                false,
                [],
            )])),
            HostSelectedExtensionMappingsError::Invalid { message, .. }
                if message.contains("is not visible")
        ));
        assert!(matches!(
            error(Arc::from([
                root_usage(
                    "@root//:one.bzl",
                    "ext",
                    test_proxy("p", [("same", "one")]),
                    false,
                    [],
                ),
                root_usage(
                    "@root//:two.bzl",
                    "ext",
                    test_proxy("q", [("same", "two")]),
                    false,
                    [],
                ),
            ])),
            HostSelectedExtensionMappingsError::Invalid { message, .. }
                if message.contains("maps to both")
        ));
        let missing_target = error(Arc::from([root_usage(
            "@root//:one.bzl",
            "ext",
            test_proxy("p", []),
            false,
            [("generated", "missing")],
        )]));
        assert!(
            matches!(
                missing_target,
                HostSelectedExtensionMappingsError::Invalid { ref message, .. }
                    if message.contains("override target") && message.contains("not visible")
            ),
            "{missing_target:?}"
        );
        assert!(matches!(
            error(Arc::from([
                root_usage(
                    "@root//:one.bzl",
                    "ext",
                    test_proxy("p", []),
                    false,
                    [("generated", "root")],
                ),
                root_usage(
                    "@root//:one.bzl",
                    "ext",
                    test_proxy("q", []),
                    false,
                    [("generated", "root")],
                ),
            ])),
            HostSelectedExtensionMappingsError::Invalid { message, .. }
                if message == "generated repository is overridden more than once"
        ));
        assert!(matches!(
            selected_extension_mappings(
                Arc::new(HostSelectedModuleRoutes {
                    entries: Arc::from([]),
                }),
                Arc::from([]),
            ),
            Err(HostSelectedExtensionMappingsError::Invalid {
                owner: HostGraphModuleKey::Root,
                message,
            }) if message == "root route is absent"
        ));

        let mut missing = test_proxy("missing", [("one", "generated")]);
        missing.imports.local_order = Arc::from([CompactString::from("absent")]);
        assert!(matches!(
            error(Arc::from([root_usage(
                "@root//:one.bzl",
                "ext",
                missing,
                false,
                [],
            )])),
            HostSelectedExtensionMappingsError::Invalid { message, .. }
                if message == "extension import order references a missing mapping"
        ));

        let mut duplicate = test_proxy(
            "duplicate",
            [("one", "generated_one"), ("two", "generated_two")],
        );
        duplicate.imports.local_order =
            Arc::from([CompactString::from("one"), CompactString::from("one")]);
        assert!(matches!(
            error(Arc::from([root_usage(
                "@root//:one.bzl",
                "ext",
                duplicate,
                false,
                [],
            )])),
            HostSelectedExtensionMappingsError::Invalid { message, .. }
                if message == "extension import order contains a duplicate local name"
        ));
    }

    #[test]
    fn pure_definition_requests_deduplicate_and_fail_closed() {
        let routes = Arc::new(
            selected_routes(
                &route_graph([route_root([], Some("root_self"))]),
                &HostSelectedRegistryRepoSpecs {
                    entries: Arc::from([]),
                },
            )
            .unwrap(),
        );
        let ordinary = selected_extension_mappings(
            routes.clone(),
            Arc::from([
                root_usage(
                    "//:ext.bzl",
                    "extension",
                    test_proxy("one", [("one", "generated")]),
                    false,
                    [],
                ),
                root_usage(
                    "//:ext.bzl",
                    "extension",
                    test_proxy("two", [("two", "generated")]),
                    false,
                    [],
                ),
            ]),
        )
        .unwrap();
        let value = selected_extension_definition_load_requests(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            Arc::new(ordinary),
        )
        .unwrap();
        assert_eq!(value.requests.len(), 1);
        assert_eq!(value.requests[0].extension_name, "extension");
        assert_eq!(value.requests[0].bzl_file.to_string(), "@@//:ext.bzl");
        assert_eq!(value.requests[0].mapping.context_repo.as_str(), "");
        assert!(value.requests[0].mapping.entries.contains_key("one"));
        assert!(value.requests[0].mapping.entries.contains_key("two"));
        assert_eq!(value.requests[0].unique_name.as_str(), "+extension");
        assert!(value.requests[0].overrides.is_empty());
        assert_eq!(
            value.requests[0]
                .validation_parts()
                .0
                .iter()
                .map(HostSelectedExtensionDefinitionImport::parts)
                .map(|(local, generated, _)| (local, generated))
                .collect::<Vec<_>>(),
            [("one", "generated"), ("two", "generated")]
        );
        assert!(
            value.requests[0]
                .validation_parts()
                .0
                .iter()
                .all(|import| import.parts().2 == &test_span())
        );

        let namespaced = selected_extension_mappings(
            Arc::new(
                selected_routes(
                    &route_graph([route_root([], Some("root_self"))]),
                    &HostSelectedRegistryRepoSpecs {
                        entries: Arc::from([]),
                    },
                )
                .unwrap(),
            ),
            Arc::from([
                root_usage(
                    "//:one.bzl",
                    "shared",
                    test_proxy("one", [("target", "replacement")]),
                    false,
                    [],
                ),
                root_usage(
                    "//:two.bzl",
                    "shared",
                    test_proxy("two", [("overridden_alias", "generated")]),
                    false,
                    [("generated", "target"), ("other", "target")],
                ),
            ]),
        )
        .unwrap();
        let namespaced = selected_extension_definition_load_requests(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            Arc::new(namespaced),
        )
        .unwrap();
        assert_eq!(
            namespaced
                .requests
                .iter()
                .map(|request| request.unique_name.as_str())
                .collect::<Vec<_>>(),
            ["+shared", "+shared2"]
        );
        let (_, _, base, overrides) = namespaced.requests[1].namespace_parts();
        assert_eq!(
            base.get("overridden_alias").unwrap().as_str(),
            "+shared2+generated"
        );
        assert_eq!(
            namespaced.requests[1]
                .parts()
                .3
                .get("overridden_alias")
                .unwrap()
                .as_str(),
            "+shared+replacement"
        );
        assert_eq!(
            overrides
                .iter()
                .map(HostSelectedExtensionDefinitionOverride::parts)
                .collect::<Vec<_>>(),
            [
                (
                    "generated",
                    &CanonicalRepoName::new("+shared+replacement").unwrap(),
                    true
                ),
                (
                    "other",
                    &CanonicalRepoName::new("+shared+replacement").unwrap(),
                    true
                ),
            ]
        );
        assert!(
            overrides
                .iter()
                .all(|override_value| override_value.location() == &test_span())
        );

        let mut invalid = namespaced.predecessor.as_ref().clone();
        let mut invalid_usages = invalid.usages.to_vec();
        let mut mismatched = invalid_usages[0].clone();
        mismatched.unique_name = CanonicalRepoName::new("+wrong").unwrap();
        invalid_usages.push(mismatched);
        invalid.usages = invalid_usages.into();
        assert!(matches!(
            selected_extension_definition_load_requests(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                Arc::new(invalid),
            ),
            Err(HostSelectedExtensionDefinitionLoadRequestsError(
                HostSelectedExtensionDefinitionLoadRequestsErrorInner::Invalid {
                    ref message,
                    ..
                },
            )) if message.contains("mismatched namespace ownership")
        ));
        let mut duplicate = namespaced.predecessor.as_ref().clone();
        let mut duplicate_usages = duplicate.usages.to_vec();
        duplicate_usages[1].unique_name = duplicate_usages[0].unique_name.clone();
        duplicate.usages = duplicate_usages.into();
        assert!(matches!(
            selected_extension_definition_load_requests(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                Arc::new(duplicate),
            ),
            Err(HostSelectedExtensionDefinitionLoadRequestsError(
                HostSelectedExtensionDefinitionLoadRequestsErrorInner::Invalid {
                    ref message,
                    ..
                },
            )) if message.contains("duplicate ownership")
        ));
        let mut missing = namespaced.predecessor.as_ref().clone();
        missing.base_mappings = Arc::from([]);
        assert!(matches!(
            selected_extension_definition_load_requests(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                Arc::new(missing),
            ),
            Err(HostSelectedExtensionDefinitionLoadRequestsError(
                HostSelectedExtensionDefinitionLoadRequestsErrorInner::InvalidContext(
                    ref message
                ),
            )) if message.contains("base mapping is absent")
        ));

        let unsupported = selected_extension_mappings(
            routes,
            Arc::from([root_usage(
                "//:ext.bzl",
                "extension",
                test_proxy("isolated", []),
                true,
                [],
            )]),
        )
        .unwrap();
        assert!(matches!(
            selected_extension_definition_load_requests(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                Arc::new(unsupported),
            ),
            Err(HostSelectedExtensionDefinitionLoadRequestsError(
                HostSelectedExtensionDefinitionLoadRequestsErrorInner::Unsupported {
                    owner: HostGraphModuleKey::Root,
                    id: HostSelectedExtensionId {
                        isolation: Some(_),
                        ..
                    },
                }
            ))
        ));
    }

    #[test]
    fn pure_evaluation_inputs_aggregate_matching_root_usages_in_source_order() {
        let request = HostSelectedExtensionDefinitionLoadRequest {
            bzl_file: CanonicalLabel::parse("@@root//:ext.bzl").unwrap(),
            extension_name: "ext".into(),
            unique_name: CanonicalRepoName::new("root+ext").unwrap(),
            base_mapping: HostSelectedRepositoryMapping {
                context_repo: CanonicalRepoName::new("root").unwrap(),
                entries: Arc::new(SmallMap::new()),
            },
            mapping: HostSelectedRepositoryMapping {
                context_repo: CanonicalRepoName::new("root").unwrap(),
                entries: Arc::new(SmallMap::new()),
            },
            imports: Arc::from([]),
            overrides: Arc::from([]),
        };
        let tag = |name: &str, value: crate::NonrootAttributeValue| crate::NonrootExtensionTag {
            tag_class: name.into(),
            attributes: Arc::new(SmallMap::from_iter([(CompactString::from("raw"), value)])),
            dev_dependency: false,
            location: test_span(),
        };
        let mut first = root_usage("//:ext.bzl", "ext", test_proxy("a", []), false, []);
        first.tags = Arc::from([
            tag("one", crate::NonrootAttributeValue::None),
            tag("two", crate::NonrootAttributeValue::String("raw".into())),
        ]);
        let empty = root_usage("//:ext.bzl", "ext", test_proxy("empty", []), false, []);
        let mut last = root_usage("//:ext.bzl", "ext", test_proxy("b", []), false, []);
        last.tags = Arc::from([tag("three", crate::NonrootAttributeValue::Bool(true))]);
        let tags = matching_root_extension_tags(&[first, empty.clone(), last], &request).unwrap();
        assert_eq!(
            tags.iter()
                .map(|tag| tag.tag_class.as_str())
                .collect::<Vec<_>>(),
            ["one", "two", "three"]
        );
        assert!(
            matches!(tags[1].attributes.get("raw"), Some(crate::NonrootAttributeValue::String(value)) if value == "raw")
        );
        assert!(
            matching_root_extension_tags(&[empty], &request)
                .unwrap()
                .is_empty()
        );
        assert!(
            matching_root_extension_tags(
                &[
                    root_usage("//:other.bzl", "ext", test_proxy("other", []), false, []),
                    root_usage("//:ext.bzl", "other", test_proxy("name", []), false, []),
                    root_usage("//:ext.bzl", "ext", test_proxy("isolated", []), true, [])
                ],
                &request,
            )
            .is_none()
        );
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
    async fn real_selected_extensions_use_two_phase_mapping_and_restore_a_b_a() {
        let io = Arc::new(TrackingRegistryIo::new([]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.clone());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let source = |name: &str| {
            format!(
                r#"
module(name = "bazel_tools", repo_name = "root_self")
p = use_extension("//:ext.bzl", "extension")
use_repo(p, root_alias = "{name}", overridden_alias = "overridden")
override_repo(p, overridden = "replacement")
isolated = use_extension("//:ext.bzl", "extension", isolate = True)
use_repo(isolated, isolated_alias = "isolated")
repo = use_repo_rule("//:repo.bzl", "simple_repo")
repo(name = "replacement")
"#
            )
        };

        let a = compute_real_extensions(&dice, &source("plain_a"), 1, true).await;
        let warm = compute_real_extensions(&dice, &source("plain_a"), 1, true).await;
        assert!(HostSelectedExtensionMappingsKey::equality(&a, &warm));
        assert!(HostSelectedExtensionMappingsKey::validity(&a));
        let SourcePreparationOutcome::Complete(a_value) = &a else {
            panic!("selected extension projection must complete")
        };
        let a_value = a_value.as_ref().as_ref().unwrap();
        assert_eq!(a_value.usages.len(), 3);
        assert_eq!(a_value.usages[0].unique_name.as_str(), "+extension");
        assert_eq!(
            a_value.usages[1].unique_name.as_str(),
            "+_extension+++isolated"
        );
        assert_eq!(a_value.usages[2].unique_name.as_str(), "+simple_repo");
        let root = &a_value.mappings[0].entries;
        assert_eq!(
            root.get("root_alias").unwrap().as_str(),
            "+extension+plain_a"
        );
        assert_eq!(
            root.get("replacement").unwrap().as_str(),
            "+simple_repo+replacement"
        );
        assert_eq!(
            root.get("overridden_alias").unwrap().as_str(),
            "+simple_repo+replacement"
        );
        assert_eq!(
            root.get("isolated_alias").unwrap().as_str(),
            "+_extension+++isolated+isolated"
        );
        assert_eq!(a_value.overrides.len(), 1);
        assert!(a_value.overrides[0].must_exist);

        let b = compute_real_extensions(&dice, &source("plain_b"), 2, true).await;
        assert!(!HostSelectedExtensionMappingsKey::equality(&a, &b));
        let restored = compute_real_extensions(&dice, &source("plain_a"), 3, true).await;
        assert!(HostSelectedExtensionMappingsKey::equality(&a, &restored));
        assert!(io.calls().is_empty());
    }

    #[tokio::test]
    async fn real_definition_requests_reuse_reorder_and_restore() {
        let io = Arc::new(TrackingRegistryIo::new([]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.clone());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let source = |reversed: bool| {
            let usages = if reversed {
                "b=use_extension('//:b.bzl','b')\nb.beta(value='two')\nuse_repo(b,b_alias='repo')\n\
                 a=use_extension('//:a.bzl','a')\na.alpha(value='one')\nuse_repo(a,a_alias='repo')"
            } else {
                "a=use_extension('//:a.bzl','a')\na.alpha(value='one')\nuse_repo(a,a_alias='repo')\n\
                 b=use_extension('//:b.bzl','b')\nb.beta(value='two')\nuse_repo(b,b_alias='repo')"
            };
            format!("module(name='bazel_tools', repo_name='root_self')\n{usages}\n")
        };

        let a = compute_real_definition_requests(&dice, &source(false), 40, true).await;
        let warm = compute_real_definition_requests(&dice, &source(false), 40, true).await;
        assert!(HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a, &warm
        ));
        assert!(HostSelectedExtensionDefinitionLoadRequestsKey::validity(&a));
        let SourcePreparationOutcome::Complete(a_value) = &a else {
            panic!("definition requests must complete")
        };
        let a_value = a_value.as_ref().as_ref().unwrap();
        assert_eq!(a_value.requests.len(), 2);
        assert_eq!(a_value.requests[0].extension_name, "a");
        assert_eq!(a_value.requests[1].extension_name, "b");
        assert_eq!(a_value.workspace.as_path(), Path::new(WORKSPACE));
        assert_eq!(
            a_value.requests[0]
                .mapping
                .entries
                .get("root_self")
                .unwrap()
                .as_str(),
            ""
        );

        let b = compute_real_definition_requests(&dice, &source(true), 41, true).await;
        let SourcePreparationOutcome::Complete(b_value) = &b else {
            panic!("reordered requests must complete")
        };
        assert_eq!(
            b_value.as_ref().as_ref().unwrap().requests[0].extension_name,
            "b"
        );
        assert!(!HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a, &b
        ));
        let restored = compute_real_definition_requests(&dice, &source(false), 42, true).await;
        assert!(HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a, &restored
        ));
        let absent = compute_real_definition_requests(
            &dice,
            "module(name='bazel_tools', repo_name='root_self')\n",
            43,
            true,
        )
        .await;
        let SourcePreparationOutcome::Complete(absent_value) = &absent else {
            panic!("absent requests must complete")
        };
        assert!(absent_value.as_ref().as_ref().unwrap().requests.is_empty());
        assert!(!HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a, &absent
        ));
        let changed = compute_real_definition_requests(
            &dice,
            "module(name='bazel_tools', repo_name='root_self')\n\
             c=use_extension('//:changed.bzl','changed')\n\
             use_repo(c,changed_alias='repo')\n",
            44,
            true,
        )
        .await;
        let SourcePreparationOutcome::Complete(changed_value) = &changed else {
            panic!("changed requests must complete")
        };
        let changed_request = &changed_value.as_ref().as_ref().unwrap().requests[0];
        assert_eq!(changed_request.parts().0.to_string(), "@@//:changed.bzl");
        assert_eq!(changed_request.parts().1, "changed");
        assert!(changed_request.parts().3.contains_key("changed_alias"));
        assert!(!HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a, &changed
        ));
        let restored_again =
            compute_real_definition_requests(&dice, &source(false), 45, true).await;
        assert!(HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a,
            &restored_again
        ));
        assert!(io.calls().is_empty());

        let inputs = compute_real_evaluation_inputs(&dice, &source(false), 46, true).await;
        let warm_inputs = compute_real_evaluation_inputs(&dice, &source(false), 46, true).await;
        assert!(HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &inputs,
            &warm_inputs
        ));
        let SourcePreparationOutcome::Complete(inputs_value) = &inputs else {
            panic!("evaluation inputs must complete")
        };
        let inputs_value = inputs_value.as_ref().as_ref().unwrap();
        assert_eq!(inputs_value.parts().1.len(), 2);
        let (load, canonical, name, version, is_root, tags) = inputs_value.parts().1[0].parts();
        assert_eq!(load.parts().1, "a");
        assert_eq!(canonical.as_str(), "");
        assert_eq!((name, version, is_root), ("bazel_tools", "", true));
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].tag_class, "alpha");
        assert!(matches!(
            tags[0].attributes.get("value"),
            Some(crate::NonrootAttributeValue::String(value)) if value == "one"
        ));
        let reordered = compute_real_evaluation_inputs(&dice, &source(true), 47, true).await;
        assert!(!HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &inputs, &reordered
        ));
        let restored_inputs = compute_real_evaluation_inputs(&dice, &source(false), 48, true).await;
        assert!(HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &inputs,
            &restored_inputs
        ));

        let versioned = compute_real_evaluation_inputs(
            &dice,
            "module(name='bazel_tools',version='1.2+ignored')\n\
             a=use_extension('//:a.bzl','a')\na.alpha(value='one')\n",
            49,
            true,
        )
        .await;
        let SourcePreparationOutcome::Complete(versioned_value) = &versioned else {
            panic!("versioned root input must complete")
        };
        let (_, canonical, name, version, is_root, _) =
            versioned_value.as_ref().as_ref().unwrap().parts().1[0].parts();
        assert_eq!(canonical.as_str(), "");
        assert_eq!((name, version, is_root), ("bazel_tools", "1.2", true));
        assert!(!HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &inputs, &versioned
        ));
        let restored_fields = compute_real_evaluation_inputs(&dice, &source(false), 50, true).await;
        assert!(HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &inputs,
            &restored_fields
        ));
    }

    #[tokio::test]
    async fn real_definition_requests_retain_namespace_overrides_and_restore() {
        let io = Arc::new(TrackingRegistryIo::new([]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.clone());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let variant = |target: &str,
                       local: &str,
                       exported: &str,
                       move_override: bool,
                       inject: bool,
                       overridden: &str| {
            format!(
                r#"
module(name = "bazel_tools", repo_name = "root_self")
one = use_extension("//:one.bzl", "shared")
use_repo(one, target = "replacement", other_target = "other")
two = use_extension("//:two.bzl", "shared")
use_repo(two, {local} = "{exported}")
{}{operation}(two, {overridden} = "{target}")
three = use_extension("//:three.bzl", "third")
inject_repo(three, injected = "target")
"#,
                if move_override { "\n" } else { "" },
                operation = if inject {
                    "inject_repo"
                } else {
                    "override_repo"
                },
            )
        };
        let source = |target: &str| {
            variant(
                target,
                "overridden_alias",
                "generated",
                false,
                false,
                "generated",
            )
        };

        let a = compute_real_definition_requests(&dice, &source("target"), 70, true).await;
        let warm = compute_real_definition_requests(&dice, &source("target"), 70, true).await;
        assert!(HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a, &warm
        ));
        let SourcePreparationOutcome::Complete(a_value) = &a else {
            panic!("namespace requests must complete")
        };
        let a_value = a_value.as_ref().as_ref().unwrap();
        assert_eq!(
            a_value
                .requests
                .iter()
                .map(|request| request.unique_name.as_str())
                .collect::<Vec<_>>(),
            ["+shared", "+shared2", "+third"]
        );
        let (_, _, base, overrides) = a_value.requests[1].namespace_parts();
        assert_eq!(
            base.get("overridden_alias").unwrap().as_str(),
            "+shared2+generated"
        );
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].parts().0, "generated");
        assert_eq!(overrides[0].parts().1.as_str(), "+shared+replacement");
        assert!(overrides[0].parts().2);
        assert_eq!(overrides[0].location().start_line, 7);
        assert_eq!(
            a_value.requests[0]
                .validation_parts()
                .0
                .iter()
                .map(HostSelectedExtensionDefinitionImport::parts)
                .map(|(local, generated, location)| { (local, generated, location.start_line) })
                .collect::<Vec<_>>(),
            [("target", "replacement", 3), ("other_target", "other", 3)]
        );
        let (local, generated, location) = a_value.requests[1].validation_parts().0[0].parts();
        assert_eq!((local, generated), ("overridden_alias", "generated"));
        assert_eq!(
            location.file,
            crate::LogicalModuleFileId::new("/selected-repo-spec-test/MODULE.bazel")
        );
        assert_eq!((location.start_line, location.start_column), (5, 20));
        let third = a_value.requests[2].namespace_parts().3;
        assert_eq!(third.len(), 1);
        assert_eq!(third[0].parts().0, "injected");
        assert!(!third[0].parts().2);
        assert_eq!(third[0].location().start_line, 9);

        let b = compute_real_definition_requests(&dice, &source("other_target"), 71, true).await;
        let SourcePreparationOutcome::Complete(b_value) = &b else {
            panic!("changed namespace requests must complete")
        };
        assert_eq!(
            b_value.as_ref().as_ref().unwrap().requests[1]
                .namespace_parts()
                .3[0]
                .parts()
                .1
                .as_str(),
            "+shared+other"
        );
        assert!(!HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a, &b
        ));
        let restored = compute_real_definition_requests(&dice, &source("target"), 72, true).await;
        assert!(HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a, &restored
        ));

        for (index, changed_source) in [
            variant(
                "target",
                "changed_alias",
                "generated",
                false,
                false,
                "generated",
            ),
            variant(
                "target",
                "overridden_alias",
                "changed_generated",
                false,
                false,
                "generated",
            ),
            variant(
                "target",
                "overridden_alias",
                "generated",
                true,
                false,
                "generated",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let changed = compute_real_definition_requests(
                &dice,
                &changed_source,
                80 + index as u64 * 2,
                true,
            )
            .await;
            let SourcePreparationOutcome::Complete(changed_value) = &changed else {
                panic!("field variant must complete")
            };
            let changed_value = changed_value.as_ref().as_ref().unwrap();
            match index {
                0 => assert_eq!(
                    changed_value.requests[1].validation_parts().0[0].parts().0,
                    "changed_alias"
                ),
                1 => assert_eq!(
                    changed_value.requests[1].validation_parts().0[0].parts().1,
                    "changed_generated"
                ),
                2 => assert_eq!(
                    changed_value.requests[1].namespace_parts().3[0]
                        .location()
                        .start_line,
                    8
                ),
                _ => unreachable!(),
            }
            assert!(!HostSelectedExtensionDefinitionLoadRequestsKey::equality(
                &a, &changed
            ));
            let restored_field = compute_real_definition_requests(
                &dice,
                &source("target"),
                81 + index as u64 * 2,
                true,
            )
            .await;
            assert!(HostSelectedExtensionDefinitionLoadRequestsKey::equality(
                &a,
                &restored_field
            ));
        }

        let polarity_a_source = variant(
            "target",
            "overridden_alias",
            "generated",
            false,
            false,
            "unimported",
        );
        let polarity_b_source = variant(
            "target",
            "overridden_alias",
            "generated",
            false,
            true,
            "unimported",
        );
        let polarity_a =
            compute_real_definition_requests(&dice, &polarity_a_source, 90, true).await;
        let polarity_b =
            compute_real_definition_requests(&dice, &polarity_b_source, 91, true).await;
        let SourcePreparationOutcome::Complete(polarity_b_value) = &polarity_b else {
            panic!("injection polarity variant must complete")
        };
        assert!(
            !polarity_b_value.as_ref().as_ref().unwrap().requests[1]
                .namespace_parts()
                .3[0]
                .parts()
                .2
        );
        assert!(!HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &polarity_a,
            &polarity_b,
        ));
        let polarity_restored =
            compute_real_definition_requests(&dice, &polarity_a_source, 92, true).await;
        assert!(HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &polarity_a,
            &polarity_restored,
        ));
        let reordered_source = source("target").replace(
            "target = \"replacement\", other_target = \"other\"",
            "other_target = \"other\", target = \"replacement\"",
        );
        let reordered = compute_real_definition_requests(&dice, &reordered_source, 73, true).await;
        assert!(!HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a, &reordered
        ));
        let moved_source = source("target").replace("two = use_extension", "\ntwo = use_extension");
        let moved = compute_real_definition_requests(&dice, &moved_source, 74, true).await;
        assert!(!HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a, &moved
        ));
        let restored_again =
            compute_real_definition_requests(&dice, &source("target"), 75, true).await;
        assert!(HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &a,
            &restored_again
        ));
        assert!(io.calls().is_empty());
    }

    #[tokio::test]
    async fn real_evaluation_inputs_retain_fields_and_exclude_unrelated_root_state() {
        let io = Arc::new(TrackingRegistryIo::new([]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.clone());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let source = |name: &str, dev: bool, blank_before_tag: bool| {
            format!(
                "module(name='{name}')\n\
                 # command_override_bazel_tools\n\
                 a=use_extension('//:a.bzl','a'{})\n{}a.alpha(value='one')\n",
                if dev { ", dev_dependency=True" } else { "" },
                if blank_before_tag { "\n" } else { "" },
            )
        };
        let base_source = source("root_a", false, false);
        let a = compute_real_evaluation_inputs(&dice, &base_source, 60, true).await;
        let SourcePreparationOutcome::Complete(a_value) = &a else {
            panic!("root evaluation inputs must complete")
        };
        let a_value = a_value.as_ref().as_ref().unwrap();
        let a_input = a_value.parts().1[0].clone();
        assert_eq!(a_input.parts().2, "root_a");
        assert!(!a_input.parts().5[0].dev_dependency);

        let renamed =
            compute_real_evaluation_inputs(&dice, &source("root_b", false, false), 61, true).await;
        let SourcePreparationOutcome::Complete(renamed_value) = &renamed else {
            panic!("renamed root evaluation input must complete")
        };
        assert_eq!(
            renamed_value.as_ref().as_ref().unwrap().parts().1[0]
                .parts()
                .2,
            "root_b"
        );
        assert!(!HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &a, &renamed
        ));
        let renamed_restored = compute_real_evaluation_inputs(&dice, &base_source, 62, true).await;
        assert!(HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &a,
            &renamed_restored
        ));

        let dev =
            compute_real_evaluation_inputs(&dice, &source("root_a", true, false), 63, true).await;
        let SourcePreparationOutcome::Complete(dev_value) = &dev else {
            panic!("dev evaluation input must complete")
        };
        assert!(dev_value.as_ref().as_ref().unwrap().parts().1[0].parts().5[0].dev_dependency);
        assert!(!HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &a, &dev
        ));
        let dev_restored = compute_real_evaluation_inputs(&dice, &base_source, 64, true).await;
        assert!(HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &a,
            &dev_restored
        ));

        let moved =
            compute_real_evaluation_inputs(&dice, &source("root_a", false, true), 65, true).await;
        let SourcePreparationOutcome::Complete(moved_value) = &moved else {
            panic!("moved evaluation input must complete")
        };
        let moved_tag = &moved_value.as_ref().as_ref().unwrap().parts().1[0]
            .parts()
            .5[0];
        assert_ne!(a_input.parts().5[0].location, moved_tag.location);
        assert!(!HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &a, &moved
        ));
        let moved_restored = compute_real_evaluation_inputs(&dice, &base_source, 66, true).await;
        assert!(HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &a,
            &moved_restored
        ));

        let load_requests = Arc::new(a_value.parts().0.clone());
        let root_files = compute_real_root_files(&dice, &base_source, 67)
            .await
            .as_ref()
            .as_ref()
            .unwrap()
            .clone();
        let projected =
            selected_extension_evaluation_input_requests(load_requests.clone(), &root_files)
                .unwrap();
        for suffix in [
            "bazel_dep(name='rules_license', version='1')\nlocal_path_override(module_name='rules_license', path='rules_license')\n",
            "register_toolchains('//:toolchain')\n",
            "single_version_override(module_name='unused', version='1')\n",
        ] {
            let changed =
                compute_real_root_files(&dice, &format!("{base_source}{suffix}"), 68).await;
            let changed = changed.as_ref().as_ref().unwrap();
            assert_eq!(
                projected.requests,
                selected_extension_evaluation_input_requests(load_requests.clone(), changed)
                    .unwrap()
                    .requests
            );
        }
        let mut path_and_lockfile = root_files.clone();
        path_and_lockfile.module_file_paths =
            Arc::from([Path::new("/unrelated/MODULE.bazel").to_owned()]);
        path_and_lockfile.visible_lockfile = crate::VisibleLockfileRead::Ignored;
        assert_eq!(
            projected.requests,
            selected_extension_evaluation_input_requests(
                load_requests.clone(),
                &path_and_lockfile,
            )
            .unwrap()
            .requests
        );

        let module_view = |input: &HostSelectedExtensionEvaluationInput| {
            (
                input.canonical_repo.clone(),
                input.name.clone(),
                input.version.clone(),
                input.tags.clone(),
            )
        };
        let mut mapping_changed = a_input.clone();
        mapping_changed.load_request.mapping.context_repo =
            CanonicalRepoName::new("changed+").unwrap();
        assert_ne!(a_input, mapping_changed);
        assert_eq!(module_view(&a_input), module_view(&mapping_changed));
        let mut canonical_changed = a_input.clone();
        canonical_changed.canonical_repo = CanonicalRepoName::new("changed+").unwrap();
        assert_ne!(a_input, canonical_changed);
        assert_eq!(a_input.parts().4, canonical_changed.parts().4);
        canonical_changed.canonical_repo = a_input.canonical_repo.clone();
        assert_eq!(a_input, canonical_changed);

        let mut unmatched_root_files = root_files;
        unmatched_root_files.extension_usages = Arc::from([]);
        let error_a = selected_extension_evaluation_input_requests(
            load_requests.clone(),
            &unmatched_root_files,
        )
        .unwrap_err();
        let mut changed_load_requests = load_requests.as_ref().clone();
        let mut changed_request = changed_load_requests.requests[0].clone();
        changed_request.mapping.context_repo = CanonicalRepoName::new("changed+").unwrap();
        changed_load_requests.requests = Arc::from([changed_request]);
        let error_b = selected_extension_evaluation_input_requests(
            Arc::new(changed_load_requests),
            &unmatched_root_files,
        )
        .unwrap_err();
        assert_ne!(error_a, error_b);
        assert_eq!(
            error_a,
            selected_extension_evaluation_input_requests(load_requests, &unmatched_root_files)
                .unwrap_err()
        );
        assert!(io.calls().is_empty());
    }

    #[tokio::test]
    async fn real_selected_extensions_retain_nonroot_and_collision_order() {
        let io = Arc::new(TrackingRegistryIo::new([]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.clone());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let source = |reversed: bool| {
            let usages = if reversed {
                "b=use_extension('//:b.bzl','shared')\nuse_repo(b,b_alias='b')\n\
                 a=use_extension('//:a.bzl','shared')\nuse_repo(a,a_alias='a')"
            } else {
                "a=use_extension('//:a.bzl','shared')\nuse_repo(a,a_alias='a')\n\
                 b=use_extension('//:b.bzl','shared')\nuse_repo(b,b_alias='b')"
            };
            format!(
                "module(name='bazel_tools', repo_name='root_self')\n\
                 local_path_override(module_name='local', path='local')\n\
                 bazel_dep(name='local', version='1')\n{usages}\n"
            )
        };

        let a = compute_real_extensions(&dice, &source(false), 20, true).await;
        let SourcePreparationOutcome::Complete(a_value) = &a else {
            panic!("selected extension projection must complete")
        };
        let a_value = a_value.as_ref().as_ref().unwrap();
        assert_eq!(a_value.usages.len(), 3);
        assert!(a_value.usages[0].id.bzl_file.to_string().contains(":a.bzl"));
        assert_eq!(a_value.usages[0].unique_name.as_str(), "+shared");
        assert!(a_value.usages[1].id.bzl_file.to_string().contains(":b.bzl"));
        assert_eq!(a_value.usages[1].unique_name.as_str(), "+shared2");
        assert_eq!(a_value.usages[2].unique_name.as_str(), "local++shared");
        let local_mapping = a_value
            .mappings
            .iter()
            .find(|mapping| mapping.context_repo.as_str() == "local+")
            .unwrap();
        assert_eq!(
            local_mapping.entries.get("generated").unwrap().as_str(),
            "local++shared+generated"
        );

        let b = compute_real_extensions(&dice, &source(true), 21, true).await;
        let SourcePreparationOutcome::Complete(b_value) = &b else {
            panic!("reordered extension projection must complete")
        };
        let b_value = b_value.as_ref().as_ref().unwrap();
        assert!(b_value.usages[0].id.bzl_file.to_string().contains(":b.bzl"));
        assert_eq!(b_value.usages[0].unique_name.as_str(), "+shared");
        assert!(b_value.usages[1].id.bzl_file.to_string().contains(":a.bzl"));
        assert_eq!(b_value.usages[1].unique_name.as_str(), "+shared2");
        assert!(!HostSelectedExtensionMappingsKey::equality(&a, &b));

        let restored = compute_real_extensions(&dice, &source(false), 22, true).await;
        assert!(HostSelectedExtensionMappingsKey::equality(&a, &restored));

        let ordered_source = format!("{}\n# ordered_nonroot_imports\n", source(false));
        let ordered = compute_real_extensions(&dice, &ordered_source, 23, true).await;
        let SourcePreparationOutcome::Complete(ordered_value) = &ordered else {
            panic!("ordered nonroot imports must complete")
        };
        assert_eq!(
            ordered_value.as_ref().as_ref().unwrap().usages[2]
                .validation_imports
                .iter()
                .map(|import| (import.local_name.as_str(), import.generated_name.as_str()))
                .collect::<Vec<_>>(),
            [("first", "generated_one"), ("second", "generated_two"),]
        );
        let reversed_source = format!("{}\n# reversed_nonroot_imports\n", source(false));
        let reversed = compute_real_extensions(&dice, &reversed_source, 24, true).await;
        let SourcePreparationOutcome::Complete(reversed_value) = &reversed else {
            panic!("reversed nonroot imports must complete")
        };
        assert_eq!(
            reversed_value.as_ref().as_ref().unwrap().usages[2]
                .validation_imports
                .iter()
                .map(|import| import.local_name.as_str())
                .collect::<Vec<_>>(),
            ["second", "first"]
        );
        assert!(!HostSelectedExtensionMappingsKey::equality(
            &ordered, &reversed
        ));
        let restored_nonroot = compute_real_extensions(&dice, &ordered_source, 25, true).await;
        assert!(HostSelectedExtensionMappingsKey::equality(
            &ordered,
            &restored_nonroot
        ));
        assert!(io.calls().is_empty());
    }

    #[tokio::test]
    async fn real_selected_extension_route_need_and_error_remain_typed() {
        let io = Arc::new(TrackingRegistryIo::new([(
            "https://registry.invalid/modules/dep/1/MODULE.bazel",
            b"module(name='dep', version='1')\n" as &[u8],
        )]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io);
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let need = compute_real_extensions(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n",
            30,
            false,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostSelectedExtensionMappingsKey::validity(&need));
        assert!(!HostSelectedExtensionMappingsKey::equality(&need, &need));

        let error = compute_real_extensions(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='missing', version='1')\n",
            31,
            true,
        )
        .await;
        assert!(matches!(
            error,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostSelectedExtensionMappingsError::Routes(
                        HostSelectedModuleRoutesError::Graph(
                            HostSelectedModuleGraphError::DiscoveryLeaf {
                                module: HostGraphModuleKey::Module { name, .. },
                                ..
                            }
                        )
                    )) if name == "missing"
                )
        ));
    }

    #[tokio::test]
    async fn real_definition_request_need_and_error_preserve_predecessor() {
        let io = Arc::new(TrackingRegistryIo::new([(
            "https://registry.invalid/modules/dep/1/MODULE.bazel",
            b"module(name='dep', version='1')\n" as &[u8],
        )]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io);
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let need = compute_real_definition_requests(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n",
            50,
            false,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostSelectedExtensionDefinitionLoadRequestsKey::validity(
            &need
        ));
        assert!(!HostSelectedExtensionDefinitionLoadRequestsKey::equality(
            &need, &need
        ));
        let input_need = compute_real_evaluation_inputs(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n",
            52,
            false,
        )
        .await;
        assert!(matches!(input_need, SourcePreparationOutcome::Need(_)));
        assert!(!HostSelectedExtensionEvaluationInputRequestsKey::validity(
            &input_need
        ));
        assert!(!HostSelectedExtensionEvaluationInputRequestsKey::equality(
            &input_need,
            &input_need
        ));

        let error = compute_real_definition_requests(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='missing', version='1')\n",
            51,
            true,
        )
        .await;
        assert!(matches!(
            error,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostSelectedExtensionDefinitionLoadRequestsError(
                        HostSelectedExtensionDefinitionLoadRequestsErrorInner::Mappings(
                            HostSelectedExtensionMappingsError::Routes(
                                HostSelectedModuleRoutesError::Graph(
                                    HostSelectedModuleGraphError::DiscoveryLeaf {
                                        module: HostGraphModuleKey::Module { name, .. },
                                        ..
                                    }
                                )
                            )
                        )
                    )) if name == "missing"
                )
        ));
        let input_error = compute_real_evaluation_inputs(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='missing', version='1')\n",
            53,
            true,
        )
        .await;
        assert!(matches!(
            input_error,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostSelectedExtensionEvaluationInputRequestsError::LoadRequests(_))
                )
        ));
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
