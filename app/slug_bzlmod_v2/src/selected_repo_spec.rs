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
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochError;
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
use crate::host_registry::HostRegistryFunctionObservationKey;
use crate::host_registry::HostRegistryFunctionValue;
use crate::host_registry::HostRegistryScheme;
use crate::host_registry::RegistryKnownFileHashesMode;
use crate::module_eval::HostEffectiveModuleOverride;
use crate::module_eval::HostEffectiveModuleOverrideError;
use crate::module_eval::HostEffectiveModuleOverrideKey;
use crate::module_eval::HostEffectiveModuleOverrideObservationKey;
use crate::module_eval::RootExtensionUsage;
use crate::module_eval::RootModuleFiles;
use crate::module_eval::RootModuleFilesKey;
use crate::module_eval::RootModuleFilesObservationKey;
use crate::registry_dice::RegistryFileObservationKey;
use crate::selected_graph::HostGraphModuleKey;
use crate::selected_graph::HostGraphModuleSource;
use crate::selected_graph::HostSelectedModuleEntry;
use crate::selected_graph::HostSelectedModuleGraph;
use crate::selected_graph::HostSelectedModuleGraphError;
use crate::selected_graph::HostSelectedModuleGraphKey;
use crate::selected_graph::HostSelectedModuleGraphObservationError;
use crate::selected_graph::HostSelectedModuleGraphObservationKey;
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

type RepoSpecsResult =
    Arc<Result<HostSelectedRegistryRepoSpecs, HostSelectedRegistryRepoSpecsError>>;
type RepoSpecsOutcome = SourcePreparationOutcome<RepoSpecsResult>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
pub(crate) struct HostSelectedRegistryRepoSpecsObservationKey(HostSelectedRegistryRepoSpecsKey);

#[allow(dead_code)]
impl HostSelectedRegistryRepoSpecsObservationKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self(HostSelectedRegistryRepoSpecsKey::new(workspace))
    }
}

impl fmt::Display for HostSelectedRegistryRepoSpecsObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)]
pub(crate) struct ObservedHostSelectedRegistryRepoSpecs {
    result: RepoSpecsResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedHostSelectedRegistryRepoSpecs {
    fn new(result: RepoSpecsResult, observations: PathObservationEpoch) -> Self {
        Self {
            result,
            observations,
        }
    }

    pub(crate) fn result(&self) -> &RepoSpecsResult {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum RepoSpecObservationStage {
    Graph,
    HostRegistry,
    SourceRegistryFile,
    RegistryMetadataFile,
    EffectiveOverride,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostSelectedRegistryRepoSpecsObservationError {
    Graph(HostSelectedModuleGraphObservationError),
    HostRegistry {
        module: Arc<HostGraphModuleKey>,
        error: ObservedPathFrontierError,
    },
    RegistryFile {
        module: Arc<HostGraphModuleKey>,
        url: RegistryFileUrl,
        error: ObservedPathFrontierError,
    },
    EffectiveOverride {
        module: Arc<HostGraphModuleKey>,
        error: ObservedPathFrontierError,
    },
    Merge {
        module: Option<Arc<HostGraphModuleKey>>,
        stage: RepoSpecObservationStage,
        error: ObservedPathFrontierError,
    },
}

#[derive(Clone, Copy)]
enum RepoSpecsMode {
    Legacy,
    Observed,
}

type RepoSpecsDriverOutcome = SourcePreparationOutcome<
    Result<(RepoSpecsResult, PathObservationEpoch), HostSelectedRegistryRepoSpecsObservationError>,
>;

enum RepoSpecChild<T, E, O = ObservedPathFrontierError> {
    Complete {
        result: Arc<Result<T, E>>,
        observations: PathObservationEpoch,
    },
    Need(SourcePreparationNeeds),
    Outer(O),
    Compute(CompactString),
}

enum RepoSpecEntryTerminal {
    Complete(Result<Option<HostSelectedRegistryRepoSpec>, HostSelectedRegistryRepoSpecsError>),
    Need(SourcePreparationNeeds),
    Outer(HostSelectedRegistryRepoSpecsObservationError),
}

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

fn merge_repo_spec_observations(
    prefix: &mut PathObservationEpoch,
    incoming: &PathObservationEpoch,
    module: Option<&HostGraphModuleKey>,
    stage: RepoSpecObservationStage,
) -> Result<(), HostSelectedRegistryRepoSpecsObservationError> {
    *prefix = PathObservationEpoch::from_shared(
        prefix
            .observations()
            .iter()
            .chain(incoming.observations())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(
        |error| HostSelectedRegistryRepoSpecsObservationError::Merge {
            module: module.map(|module| Arc::new(module.clone())),
            stage,
            error: ObservedPathFrontierError::from(error),
        },
    )?;
    Ok(())
}

async fn selected_graph_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
) -> RepoSpecChild<
    HostSelectedModuleGraph,
    HostSelectedModuleGraphError,
    HostSelectedModuleGraphObservationError,
> {
    match ctx
        .compute(&HostSelectedModuleGraphObservationKey::new(
            workspace.dupe(),
        ))
        .await
    {
        Err(error) => RepoSpecChild::Compute(error.to_string().into()),
        Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
        Ok(SourcePreparationOutcome::Complete(Err(error))) => RepoSpecChild::Outer(error),
        Ok(SourcePreparationOutcome::Complete(Ok(observed))) => RepoSpecChild::Complete {
            result: observed.result().dupe(),
            observations: observed.observations().dupe(),
        },
    }
}

async fn legacy_graph_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
) -> RepoSpecChild<
    HostSelectedModuleGraph,
    HostSelectedModuleGraphError,
    HostSelectedModuleGraphObservationError,
> {
    match ctx
        .compute(&HostSelectedModuleGraphKey::new(workspace.dupe()))
        .await
    {
        Err(error) => RepoSpecChild::Compute(error.to_string().into()),
        Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
        Ok(SourcePreparationOutcome::Complete(result)) => RepoSpecChild::Complete {
            result,
            observations: PathObservationEpoch::empty(),
        },
    }
}

async fn host_registry_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    registry: &str,
    mode: RepoSpecsMode,
) -> RepoSpecChild<HostRegistryFunctionValue, HostRegistryFunctionError> {
    match mode {
        RepoSpecsMode::Legacy => {
            match ctx
                .compute(&HostRegistryFunctionKey::new(workspace.dupe(), registry))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(PathOutcome::Need(need)) => {
                    RepoSpecChild::Need(SourcePreparationNeeds::path(need))
                }
                Ok(PathOutcome::Complete(result)) => RepoSpecChild::Complete {
                    result,
                    observations: PathObservationEpoch::empty(),
                },
            }
        }
        RepoSpecsMode::Observed => {
            match ctx
                .compute(&HostRegistryFunctionObservationKey::new(
                    workspace.dupe(),
                    registry,
                ))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(PathOutcome::Need(need)) => {
                    RepoSpecChild::Need(SourcePreparationNeeds::path(need))
                }
                Ok(PathOutcome::Complete(Err(error))) => RepoSpecChild::Outer(error),
                Ok(PathOutcome::Complete(Ok(observed))) => RepoSpecChild::Complete {
                    result: observed.result().dupe(),
                    observations: observed.observations().dupe(),
                },
            }
        }
    }
}

async fn registry_file_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    url: RegistryFileUrl,
    mode: RepoSpecsMode,
) -> RepoSpecChild<RegistryFileValue, RegistryFileError> {
    match mode {
        RepoSpecsMode::Legacy => {
            match ctx
                .compute(&RegistryFileKey {
                    workspace: workspace.as_path().to_owned(),
                    url,
                })
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(result) => RepoSpecChild::Complete {
                    result,
                    observations: PathObservationEpoch::empty(),
                },
            }
        }
        RepoSpecsMode::Observed => {
            match ctx
                .compute(&RegistryFileObservationKey::new(
                    workspace.as_path().to_owned(),
                    url,
                ))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
                Ok(SourcePreparationOutcome::Complete(Err(error))) => RepoSpecChild::Outer(error),
                Ok(SourcePreparationOutcome::Complete(Ok(observed))) => RepoSpecChild::Complete {
                    result: observed.result().dupe(),
                    observations: observed.observations().dupe(),
                },
            }
        }
    }
}

async fn effective_override_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    module_name: CompactString,
    mode: RepoSpecsMode,
) -> RepoSpecChild<HostEffectiveModuleOverride, HostEffectiveModuleOverrideError> {
    match mode {
        RepoSpecsMode::Legacy => {
            match ctx
                .compute(&HostEffectiveModuleOverrideKey::new(
                    workspace.dupe(),
                    module_name,
                ))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(result) => RepoSpecChild::Complete {
                    result,
                    observations: PathObservationEpoch::empty(),
                },
            }
        }
        RepoSpecsMode::Observed => {
            match ctx
                .compute(&HostEffectiveModuleOverrideObservationKey::new(
                    workspace.dupe(),
                    module_name,
                ))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
                Ok(SourcePreparationOutcome::Complete(Err(error))) => RepoSpecChild::Outer(error),
                Ok(SourcePreparationOutcome::Complete(Ok(observed))) => RepoSpecChild::Complete {
                    result: observed.result().dupe(),
                    observations: observed.observations().dupe(),
                },
            }
        }
    }
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

fn finish_host_registry_child(
    child: RepoSpecChild<HostRegistryFunctionValue, HostRegistryFunctionError>,
    module: &HostGraphModuleKey,
    observations: &mut PathObservationEpoch,
) -> Result<HostRegistryFunctionValue, RepoSpecEntryTerminal> {
    match child {
        RepoSpecChild::Compute(message) => Err(RepoSpecEntryTerminal::Complete(Err(
            HostSelectedRegistryRepoSpecsError::RegistryPolicyCompute {
                module: module.clone(),
                message,
            },
        ))),
        RepoSpecChild::Need(need) => Err(RepoSpecEntryTerminal::Need(need)),
        RepoSpecChild::Outer(error) => Err(RepoSpecEntryTerminal::Outer(
            HostSelectedRegistryRepoSpecsObservationError::HostRegistry {
                module: Arc::new(module.clone()),
                error,
            },
        )),
        RepoSpecChild::Complete {
            result,
            observations: incoming,
        } => {
            merge_repo_spec_observations(
                observations,
                &incoming,
                Some(module),
                RepoSpecObservationStage::HostRegistry,
            )
            .map_err(RepoSpecEntryTerminal::Outer)?;
            result.as_ref().clone().map_err(|error| {
                RepoSpecEntryTerminal::Complete(Err(
                    HostSelectedRegistryRepoSpecsError::RegistryPolicy {
                        module: module.clone(),
                        error,
                    },
                ))
            })
        }
    }
}

fn finish_registry_file_child(
    child: RepoSpecChild<RegistryFileValue, RegistryFileError>,
    module: &HostGraphModuleKey,
    url: RegistryFileUrl,
    stage: RepoSpecObservationStage,
    observations: &mut PathObservationEpoch,
) -> Result<RegistryFileObservation, RepoSpecEntryTerminal> {
    match child {
        RepoSpecChild::Compute(message) => Err(RepoSpecEntryTerminal::Complete(Err(
            HostSelectedRegistryRepoSpecsError::RegistryFileCompute {
                module: module.clone(),
                url,
                message,
            },
        ))),
        RepoSpecChild::Need(need) => Err(RepoSpecEntryTerminal::Need(need)),
        RepoSpecChild::Outer(error) => Err(RepoSpecEntryTerminal::Outer(
            HostSelectedRegistryRepoSpecsObservationError::RegistryFile {
                module: Arc::new(module.clone()),
                url,
                error,
            },
        )),
        RepoSpecChild::Complete {
            result,
            observations: incoming,
        } => {
            merge_repo_spec_observations(observations, &incoming, Some(module), stage)
                .map_err(RepoSpecEntryTerminal::Outer)?;
            result
                .as_ref()
                .clone()
                .map(|value| RegistryFileObservation {
                    url: url.dupe(),
                    value,
                })
                .map_err(|error| {
                    RepoSpecEntryTerminal::Complete(Err(
                        HostSelectedRegistryRepoSpecsError::RegistryFile {
                            module: module.clone(),
                            url,
                            error,
                        },
                    ))
                })
        }
    }
}

fn finish_effective_override_child(
    child: RepoSpecChild<HostEffectiveModuleOverride, HostEffectiveModuleOverrideError>,
    module: &HostGraphModuleKey,
    observations: &mut PathObservationEpoch,
) -> Result<HostEffectiveModuleOverride, RepoSpecEntryTerminal> {
    match child {
        RepoSpecChild::Compute(message) => Err(RepoSpecEntryTerminal::Complete(Err(
            HostSelectedRegistryRepoSpecsError::EffectiveOverrideCompute {
                module: module.clone(),
                message,
            },
        ))),
        RepoSpecChild::Need(need) => Err(RepoSpecEntryTerminal::Need(need)),
        RepoSpecChild::Outer(error) => Err(RepoSpecEntryTerminal::Outer(
            HostSelectedRegistryRepoSpecsObservationError::EffectiveOverride {
                module: Arc::new(module.clone()),
                error,
            },
        )),
        RepoSpecChild::Complete {
            result,
            observations: incoming,
        } => {
            merge_repo_spec_observations(
                observations,
                &incoming,
                Some(module),
                RepoSpecObservationStage::EffectiveOverride,
            )
            .map_err(RepoSpecEntryTerminal::Outer)?;
            result.as_ref().clone().map_err(|error| {
                RepoSpecEntryTerminal::Complete(Err(
                    HostSelectedRegistryRepoSpecsError::EffectiveOverride {
                        module: module.clone(),
                        error,
                    },
                ))
            })
        }
    }
}

async fn drive_repo_spec_entry(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    entry: &HostSelectedModuleEntry,
    mode: RepoSpecsMode,
    observations: &mut PathObservationEpoch,
) -> RepoSpecEntryTerminal {
    let (name, version) = match &entry.key {
        HostGraphModuleKey::Root => return RepoSpecEntryTerminal::Complete(Ok(None)),
        HostGraphModuleKey::Module { name, version } => (name, version),
    };
    let HostGraphModuleSource::Discovered(discovered) = &entry.source else {
        return RepoSpecEntryTerminal::Complete(Err(fail(
            &entry.key,
            "nonroot graph entry has root source",
        )));
    };
    let HostDiscoveredModuleProvenance::Registry {
        selected_registry,
        module_file_attempts,
    } = &discovered.provenance
    else {
        return RepoSpecEntryTerminal::Complete(Ok(None));
    };
    let policy = match finish_host_registry_child(
        host_registry_child(ctx, workspace, selected_registry.as_str(), mode).await,
        &entry.key,
        observations,
    ) {
        Ok(value) => value,
        Err(terminal) => return terminal,
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
    let source_url = source_json_url(policy.resolved_registry(), name, version);
    let source_json = match finish_registry_file_child(
        registry_file_child(ctx, workspace, source_url.clone(), mode).await,
        &entry.key,
        source_url,
        RepoSpecObservationStage::SourceRegistryFile,
        observations,
    ) {
        Ok(value) => value,
        Err(terminal) => return terminal,
    };
    let source = match found_bytes(&entry.key, &source_json)
        .and_then(|bytes| parse_source_json(&entry.key, bytes))
    {
        Ok(value) => value,
        Err(error) => return RepoSpecEntryTerminal::Complete(Err(error)),
    };
    let source_type = source.source_type.as_deref().unwrap_or("archive");
    let registry_json = if matches!(source_type, "archive" | "local_path") {
        let registry_url = registry_json_url(policy.resolved_registry());
        match finish_registry_file_child(
            registry_file_child(ctx, workspace, registry_url.clone(), mode).await,
            &entry.key,
            registry_url,
            RepoSpecObservationStage::RegistryMetadataFile,
            observations,
        ) {
            Ok(value) => Some(value),
            Err(terminal) => return terminal,
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
                    Err(error) => return RepoSpecEntryTerminal::Complete(Err(error)),
                }
            }
        },
        None => None,
    };
    let (module_url, module_hash) = match module_file_identity(&entry.key, module_file_attempts) {
        Ok(value) => value,
        Err(error) => return RepoSpecEntryTerminal::Complete(Err(error)),
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
        Err(error) => return RepoSpecEntryTerminal::Complete(Err(error)),
    };
    let effective = match finish_effective_override_child(
        effective_override_child(ctx, workspace, name.clone(), mode).await,
        &entry.key,
        observations,
    ) {
        Ok(value) => value,
        Err(terminal) => return terminal,
    };
    repo_spec = match augment_override(&entry.key, repo_spec, &effective) {
        Ok(value) => value,
        Err(error) => return RepoSpecEntryTerminal::Complete(Err(error)),
    };
    RepoSpecEntryTerminal::Complete(Ok(Some(HostSelectedRegistryRepoSpec {
        module: entry.key.clone(),
        policy: policy_identity,
        module_file_attempts: module_file_attempts.clone(),
        source_json,
        registry_json,
        effective_override: effective,
        repo_spec,
    })))
}

fn repo_specs_complete(
    result: Result<HostSelectedRegistryRepoSpecs, HostSelectedRegistryRepoSpecsError>,
    observations: PathObservationEpoch,
) -> RepoSpecsDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn finish_selected_graph_child(
    child: RepoSpecChild<
        HostSelectedModuleGraph,
        HostSelectedModuleGraphError,
        HostSelectedModuleGraphObservationError,
    >,
    observations: &mut PathObservationEpoch,
) -> Result<
    Arc<Result<HostSelectedModuleGraph, HostSelectedModuleGraphError>>,
    RepoSpecsDriverOutcome,
> {
    match child {
        RepoSpecChild::Compute(message) => Err(repo_specs_complete(
            Err(HostSelectedRegistryRepoSpecsError::GraphCompute(message)),
            observations.dupe(),
        )),
        RepoSpecChild::Need(need) => Err(SourcePreparationOutcome::Need(need)),
        RepoSpecChild::Outer(error) => Err(SourcePreparationOutcome::Complete(Err(
            HostSelectedRegistryRepoSpecsObservationError::Graph(error),
        ))),
        RepoSpecChild::Complete {
            result,
            observations: incoming,
        } => {
            merge_repo_spec_observations(
                observations,
                &incoming,
                None,
                RepoSpecObservationStage::Graph,
            )
            .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
            Ok(result)
        }
    }
}

#[derive(Default)]
struct RepoSpecsAccumulator {
    entries: Vec<HostSelectedRegistryRepoSpec>,
    first_outer: Option<HostSelectedRegistryRepoSpecsObservationError>,
    first_error: Option<HostSelectedRegistryRepoSpecsError>,
    needs: Option<SourcePreparationNeeds>,
    incompatible: Option<SourcePreparationNeedsError>,
}

impl RepoSpecsAccumulator {
    fn record(&mut self, terminal: RepoSpecEntryTerminal) {
        match terminal {
            RepoSpecEntryTerminal::Complete(Ok(Some(entry))) => self.entries.push(entry),
            RepoSpecEntryTerminal::Complete(Ok(None)) => {}
            RepoSpecEntryTerminal::Complete(Err(error)) => {
                if self.first_error.is_none() {
                    self.first_error = Some(error);
                }
            }
            RepoSpecEntryTerminal::Need(need) => {
                self.needs = match self.needs.take() {
                    None => Some(need),
                    Some(current) => match current.try_union(&need) {
                        Ok(union) => Some(union),
                        Err(error) => {
                            if self.incompatible.is_none() {
                                self.incompatible = Some(error);
                            }
                            Some(current)
                        }
                    },
                };
            }
            RepoSpecEntryTerminal::Outer(error) => {
                if self.first_outer.is_none() {
                    self.first_outer = Some(error);
                }
            }
        }
    }

    fn finish(self, observations: PathObservationEpoch) -> RepoSpecsDriverOutcome {
        if let Some(error) = self.first_outer {
            return SourcePreparationOutcome::Complete(Err(error));
        }
        if let Some(error) = self.first_error {
            return repo_specs_complete(Err(error), observations);
        }
        if let Some(error) = self.incompatible {
            return repo_specs_complete(
                Err(HostSelectedRegistryRepoSpecsError::IncompatibleNeeds(error)),
                observations,
            );
        }
        if let Some(need) = self.needs {
            return SourcePreparationOutcome::Need(need);
        }
        repo_specs_complete(
            Ok(HostSelectedRegistryRepoSpecs {
                entries: self.entries.into(),
            }),
            observations,
        )
    }
}

async fn drive_selected_registry_repo_specs(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRegistryRepoSpecsKey,
    mode: RepoSpecsMode,
) -> RepoSpecsDriverOutcome {
    let graph_child = match mode {
        RepoSpecsMode::Legacy => legacy_graph_child(ctx, &key.workspace).await,
        RepoSpecsMode::Observed => selected_graph_child(ctx, &key.workspace).await,
    };
    let mut observations = PathObservationEpoch::empty();
    let graph_result = match finish_selected_graph_child(graph_child, &mut observations) {
        Ok(result) => result,
        Err(terminal) => return terminal,
    };
    let graph = match graph_result.as_ref() {
        Ok(value) => value.clone(),
        Err(error) => {
            return repo_specs_complete(
                Err(HostSelectedRegistryRepoSpecsError::Graph(error.clone())),
                observations,
            );
        }
    };
    let mut accumulator = RepoSpecsAccumulator::default();
    for entry in graph.resolved.iter() {
        accumulator.record(
            drive_repo_spec_entry(ctx, &key.workspace, entry, mode, &mut observations).await,
        );
    }
    accumulator.finish(observations)
}

fn project_legacy_repo_specs(outcome: RepoSpecsDriverOutcome) -> RepoSpecsOutcome {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, _))) => {
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy selected repo specs have no observed frontier")
        }
    }
}

#[async_trait]
impl Key for HostSelectedRegistryRepoSpecsKey {
    type Value = RepoSpecsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_repo_specs(
            drive_selected_registry_repo_specs(ctx, self, RepoSpecsMode::Legacy).await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
#[async_trait]
impl Key for HostSelectedRegistryRepoSpecsObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostSelectedRegistryRepoSpecs,
            HostSelectedRegistryRepoSpecsObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_selected_registry_repo_specs(ctx, &self.0, RepoSpecsMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostSelectedRegistryRepoSpecs::new(
                    result,
                    observations,
                )))
            }
        }
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
    order: Arc<[ApparentRepoName]>,
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

type RoutesResult = Arc<Result<HostSelectedModuleRoutes, HostSelectedModuleRoutesError>>;
type RoutesOutcome = SourcePreparationOutcome<RoutesResult>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
struct HostSelectedModuleRoutesObservationKey(HostSelectedModuleRoutesKey);

#[allow(dead_code)]
impl HostSelectedModuleRoutesObservationKey {
    fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self(HostSelectedModuleRoutesKey::new(workspace))
    }
}

impl fmt::Display for HostSelectedModuleRoutesObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)]
struct ObservedHostSelectedModuleRoutes {
    result: RoutesResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedHostSelectedModuleRoutes {
    fn new(result: RoutesResult, observations: PathObservationEpoch) -> Self {
        Self {
            result,
            observations,
        }
    }

    fn result(&self) -> &RoutesResult {
        &self.result
    }

    fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
enum RouteObservationStage {
    Graph,
    RepoSpecs,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum HostSelectedModuleRoutesObservationError {
    Graph(HostSelectedModuleGraphObservationError),
    RepoSpecs(HostSelectedRegistryRepoSpecsObservationError),
    Merge {
        stage: RouteObservationStage,
        error: ObservedPathFrontierError,
    },
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum RoutesMode {
    Legacy,
    Observed,
}

type RoutesDriverOutcome = SourcePreparationOutcome<
    Result<(RoutesResult, PathObservationEpoch), HostSelectedModuleRoutesObservationError>,
>;

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
                order: mapping.keys().cloned().collect(),
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

fn route_merge_error(
    stage: RouteObservationStage,
    error: PathObservationEpochError,
) -> HostSelectedModuleRoutesObservationError {
    HostSelectedModuleRoutesObservationError::Merge {
        stage,
        error: ObservedPathFrontierError::from(error),
    }
}

fn merge_route_observations(
    prefix: &mut PathObservationEpoch,
    incoming: &PathObservationEpoch,
    stage: RouteObservationStage,
) -> Result<(), HostSelectedModuleRoutesObservationError> {
    *prefix = PathObservationEpoch::from_shared(
        prefix
            .observations()
            .iter()
            .chain(incoming.observations())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(|error| route_merge_error(stage, error))?;
    Ok(())
}

async fn route_repo_specs_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: RoutesMode,
) -> RepoSpecChild<
    HostSelectedRegistryRepoSpecs,
    HostSelectedRegistryRepoSpecsError,
    HostSelectedRegistryRepoSpecsObservationError,
> {
    match mode {
        RoutesMode::Legacy => {
            match ctx
                .compute(&HostSelectedRegistryRepoSpecsKey::new(workspace.dupe()))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
                Ok(SourcePreparationOutcome::Complete(result)) => RepoSpecChild::Complete {
                    result,
                    observations: PathObservationEpoch::empty(),
                },
            }
        }
        RoutesMode::Observed => {
            match ctx
                .compute(&HostSelectedRegistryRepoSpecsObservationKey::new(
                    workspace.dupe(),
                ))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
                Ok(SourcePreparationOutcome::Complete(Err(error))) => RepoSpecChild::Outer(error),
                Ok(SourcePreparationOutcome::Complete(Ok(observed))) => RepoSpecChild::Complete {
                    result: observed.result().dupe(),
                    observations: observed.observations().dupe(),
                },
            }
        }
    }
}

fn routes_complete(
    result: Result<HostSelectedModuleRoutes, HostSelectedModuleRoutesError>,
    observations: PathObservationEpoch,
) -> RoutesDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn finish_route_graph_child(
    child: RepoSpecChild<
        HostSelectedModuleGraph,
        HostSelectedModuleGraphError,
        HostSelectedModuleGraphObservationError,
    >,
    observations: &mut PathObservationEpoch,
) -> Result<Arc<Result<HostSelectedModuleGraph, HostSelectedModuleGraphError>>, RoutesDriverOutcome>
{
    match child {
        RepoSpecChild::Compute(message) => Err(routes_complete(
            Err(HostSelectedModuleRoutesError::GraphCompute(message)),
            observations.dupe(),
        )),
        RepoSpecChild::Need(need) => Err(SourcePreparationOutcome::Need(need)),
        RepoSpecChild::Outer(error) => Err(SourcePreparationOutcome::Complete(Err(
            HostSelectedModuleRoutesObservationError::Graph(error),
        ))),
        RepoSpecChild::Complete {
            result,
            observations: incoming,
        } => {
            merge_route_observations(observations, &incoming, RouteObservationStage::Graph)
                .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
            Ok(result)
        }
    }
}

fn finish_route_repo_specs_child(
    child: RepoSpecChild<
        HostSelectedRegistryRepoSpecs,
        HostSelectedRegistryRepoSpecsError,
        HostSelectedRegistryRepoSpecsObservationError,
    >,
    observations: &mut PathObservationEpoch,
) -> Result<RepoSpecsResult, RoutesDriverOutcome> {
    match child {
        RepoSpecChild::Compute(message) => Err(routes_complete(
            Err(HostSelectedModuleRoutesError::RepoSpecsCompute(message)),
            observations.dupe(),
        )),
        RepoSpecChild::Need(need) => Err(SourcePreparationOutcome::Need(need)),
        RepoSpecChild::Outer(error) => Err(SourcePreparationOutcome::Complete(Err(
            HostSelectedModuleRoutesObservationError::RepoSpecs(error),
        ))),
        RepoSpecChild::Complete {
            result,
            observations: incoming,
        } => {
            merge_route_observations(observations, &incoming, RouteObservationStage::RepoSpecs)
                .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
            Ok(result)
        }
    }
}

fn finish_route_graph_semantic<'a>(
    result: &'a Result<HostSelectedModuleGraph, HostSelectedModuleGraphError>,
    observations: PathObservationEpoch,
) -> Result<&'a HostSelectedModuleGraph, RoutesDriverOutcome> {
    result.as_ref().map_err(|error| {
        routes_complete(
            Err(HostSelectedModuleRoutesError::Graph(error.clone())),
            observations,
        )
    })
}

fn finish_route_repo_specs_semantic<'a>(
    result: &'a Result<HostSelectedRegistryRepoSpecs, HostSelectedRegistryRepoSpecsError>,
    observations: PathObservationEpoch,
) -> Result<&'a HostSelectedRegistryRepoSpecs, RoutesDriverOutcome> {
    result.as_ref().map_err(|error| {
        routes_complete(
            Err(HostSelectedModuleRoutesError::RepoSpecs(error.clone())),
            observations,
        )
    })
}

async fn drive_selected_module_routes(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedModuleRoutesKey,
    mode: RoutesMode,
) -> RoutesDriverOutcome {
    let graph_child = match mode {
        RoutesMode::Legacy => legacy_graph_child(ctx, &key.workspace).await,
        RoutesMode::Observed => selected_graph_child(ctx, &key.workspace).await,
    };
    let mut observations = PathObservationEpoch::empty();
    let graph_result = match finish_route_graph_child(graph_child, &mut observations) {
        Ok(result) => result,
        Err(terminal) => return terminal,
    };
    let graph = match finish_route_graph_semantic(&graph_result, observations.dupe()) {
        Ok(graph) => graph,
        Err(terminal) => return terminal,
    };
    let repo_specs = match finish_route_repo_specs_child(
        route_repo_specs_child(ctx, &key.workspace, mode).await,
        &mut observations,
    ) {
        Ok(result) => result,
        Err(terminal) => return terminal,
    };
    let repo_specs = match finish_route_repo_specs_semantic(&repo_specs, observations.dupe()) {
        Ok(repo_specs) => repo_specs,
        Err(terminal) => return terminal,
    };
    routes_complete(selected_routes(graph, repo_specs), observations)
}

fn project_legacy_routes(outcome: RoutesDriverOutcome) -> RoutesOutcome {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, _))) => {
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy selected routes have no observed frontier")
        }
    }
}

#[async_trait]
impl Key for HostSelectedModuleRoutesKey {
    type Value = RoutesOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_routes(drive_selected_module_routes(ctx, self, RoutesMode::Legacy).await)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostSelectedModuleRoutesObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostSelectedModuleRoutes, HostSelectedModuleRoutesObservationError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_selected_module_routes(ctx, &self.0, RoutesMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostSelectedModuleRoutes::new(
                    result,
                    observations,
                )))
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCanonicalSelectedModuleKind {
    Root,
    SelectedRegistry,
    SelectedNonregistry,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[doc(hidden)]
pub struct HostCanonicalSelectedModuleDefinition {
    routes: RetainedSelectedRoutes,
    ordinal: usize,
}

impl HostCanonicalSelectedModuleDefinition {
    pub fn view(&self) -> HostCanonicalSelectedModuleDefinitionView<'_> {
        let routes = self
            .routes
            .as_ref()
            .as_ref()
            .expect("a selected-module definition retains complete routes");
        let route = &routes.entries[self.ordinal];
        let kind = match &route.entry.source {
            HostGraphModuleSource::Root(_) => HostCanonicalSelectedModuleKind::Root,
            HostGraphModuleSource::Discovered(module) => match &module.provenance {
                HostDiscoveredModuleProvenance::Registry { .. } => {
                    HostCanonicalSelectedModuleKind::SelectedRegistry
                }
                HostDiscoveredModuleProvenance::NonRegistry { .. } => {
                    HostCanonicalSelectedModuleKind::SelectedNonregistry
                }
                HostDiscoveredModuleProvenance::BuiltinBazelTools { .. } => {
                    unreachable!("builtin selected routes fail before publication")
                }
            },
        };
        HostCanonicalSelectedModuleDefinitionView { kind, route }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum PrivateCanonicalSelectedModuleDefinitionError {
    Routes(RetainedSelectedRoutes, CanonicalRepoName),
    RoutesCompute(CompactString, CanonicalRepoName),
    Missing {
        predecessor: Arc<Result<HostSelectedModuleRoutes, HostSelectedModuleRoutesError>>,
        canonical_repo: CanonicalRepoName,
    },
    Duplicate {
        predecessor: Arc<Result<HostSelectedModuleRoutes, HostSelectedModuleRoutesError>>,
        canonical_repo: CanonicalRepoName,
        first_ordinal: usize,
        conflicting_ordinal: usize,
    },
    BuiltinDeferred {
        predecessor: Arc<Result<HostSelectedModuleRoutes, HostSelectedModuleRoutesError>>,
        ordinal: usize,
        canonical_repo: CanonicalRepoName,
    },
}

type RetainedSelectedRoutes = Arc<Result<HostSelectedModuleRoutes, HostSelectedModuleRoutesError>>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalSelectedModuleDefinitionError {
    inner: PrivateCanonicalSelectedModuleDefinitionError,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCanonicalSelectedModuleDefinitionErrorDisposition {
    Missing,
    Terminal,
}

impl HostCanonicalSelectedModuleDefinitionError {
    pub fn disposition(&self) -> HostCanonicalSelectedModuleDefinitionErrorDisposition {
        match &self.inner {
            PrivateCanonicalSelectedModuleDefinitionError::Missing { .. } => {
                HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing
            }
            _ => HostCanonicalSelectedModuleDefinitionErrorDisposition::Terminal,
        }
    }
}

impl fmt::Display for HostCanonicalSelectedModuleDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl std::error::Error for HostCanonicalSelectedModuleDefinitionError {}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCanonicalSelectedModuleIdentity<'a> {
    Root,
    Module {
        name: &'a str,
        normalized_version: &'a str,
    },
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct HostCanonicalSelectedModuleDefinitionView<'a> {
    kind: HostCanonicalSelectedModuleKind,
    route: &'a HostSelectedModuleRoute,
}

impl<'a> HostCanonicalSelectedModuleDefinitionView<'a> {
    pub fn kind(self) -> HostCanonicalSelectedModuleKind {
        self.kind
    }
    pub fn identity(self) -> HostCanonicalSelectedModuleIdentity<'a> {
        match &self.route.entry.key {
            HostGraphModuleKey::Root => HostCanonicalSelectedModuleIdentity::Root,
            HostGraphModuleKey::Module { name, version } => {
                HostCanonicalSelectedModuleIdentity::Module {
                    name,
                    normalized_version: version.normalized(),
                }
            }
        }
    }
    pub fn canonical_repo(self) -> &'a CanonicalRepoName {
        &self.route.canonical_repo
    }
    pub fn mapping_context(self) -> &'a CanonicalRepoName {
        &self.route.mapping.context_repo
    }
    pub fn mapping(self) -> HostCanonicalSelectedModuleMappingIter<'a> {
        HostCanonicalSelectedModuleMappingIter {
            order: self.route.mapping.order.iter(),
            entries: &self.route.mapping.entries,
        }
    }
    pub fn repo_spec(self) -> Option<&'a RepoSpec> {
        match &self.route.entry.source {
            HostGraphModuleSource::Root(_) => None,
            HostGraphModuleSource::Discovered(module) => match &module.provenance {
                HostDiscoveredModuleProvenance::Registry { .. } => self
                    .route
                    .registry_repo_spec
                    .as_ref()
                    .map(|spec| &spec.repo_spec),
                HostDiscoveredModuleProvenance::NonRegistry { closure } => {
                    Some(closure.repo_spec())
                }
                HostDiscoveredModuleProvenance::BuiltinBazelTools { .. } => {
                    unreachable!("builtin selected routes are not published")
                }
            },
        }
    }

    pub fn local_path_policy(self) -> Option<crate::HostRepositoryLocalPathPolicy> {
        match &self.route.entry.source {
            HostGraphModuleSource::Root(_) => None,
            HostGraphModuleSource::Discovered(module) => match &module.provenance {
                HostDiscoveredModuleProvenance::Registry { .. } => {
                    Some(crate::HostRepositoryLocalPathPolicy::LocalUnsupported)
                }
                HostDiscoveredModuleProvenance::NonRegistry { closure } => {
                    Some(closure.local_path_policy())
                }
                HostDiscoveredModuleProvenance::BuiltinBazelTools { .. } => {
                    unreachable!("builtin selected routes are not published")
                }
            },
        }
    }
}

#[doc(hidden)]
pub struct HostCanonicalSelectedModuleMappingIter<'a> {
    order: std::slice::Iter<'a, ApparentRepoName>,
    entries: &'a SmallMap<ApparentRepoName, CanonicalRepoName>,
}

impl<'a> Iterator for HostCanonicalSelectedModuleMappingIter<'a> {
    type Item = (&'a ApparentRepoName, &'a CanonicalRepoName);
    fn next(&mut self) -> Option<Self::Item> {
        self.order.next().map(|name| {
            (
                name,
                self.entries
                    .get(name)
                    .expect("retained mapping order is valid"),
            )
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.order.size_hint()
    }
}

impl ExactSizeIterator for HostCanonicalSelectedModuleMappingIter<'_> {}

enum CanonicalRouteMatch {
    Missing,
    Unique(usize),
    Duplicate {
        first_ordinal: usize,
        conflicting_ordinal: usize,
    },
}

fn find_canonical_route_ordinal<'a>(
    canonical_repo: &CanonicalRepoName,
    routes: impl Iterator<Item = &'a HostSelectedModuleRoute>,
) -> CanonicalRouteMatch {
    let mut first = None;
    let mut conflicting = None;
    for (ordinal, route) in routes.enumerate() {
        if &route.canonical_repo != canonical_repo {
            continue;
        }
        if first.is_none() {
            first = Some(ordinal);
        } else if conflicting.is_none() {
            conflicting = Some(ordinal);
        }
    }
    match (first, conflicting) {
        (None, _) => CanonicalRouteMatch::Missing,
        (Some(ordinal), None) => CanonicalRouteMatch::Unique(ordinal),
        (Some(first_ordinal), Some(conflicting_ordinal)) => CanonicalRouteMatch::Duplicate {
            first_ordinal,
            conflicting_ordinal,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[doc(hidden)]
pub struct HostCanonicalSelectedModuleDefinitionKey {
    workspace: NormalizedAbsolutePath,
    canonical_repo: CanonicalRepoName,
}

impl HostCanonicalSelectedModuleDefinitionKey {
    pub fn new(workspace: NormalizedAbsolutePath, canonical_repo: CanonicalRepoName) -> Self {
        Self {
            workspace,
            canonical_repo,
        }
    }
}

impl fmt::Display for HostCanonicalSelectedModuleDefinitionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-canonical-selected-module-definition:{}:{}",
            self.workspace, self.canonical_repo
        )
    }
}

#[doc(hidden)]
pub type HostCanonicalSelectedModuleDefinitionOutcome = SourcePreparationOutcome<
    Arc<Result<HostCanonicalSelectedModuleDefinition, HostCanonicalSelectedModuleDefinitionError>>,
>;

#[async_trait]
impl Key for HostCanonicalSelectedModuleDefinitionKey {
    type Value = HostCanonicalSelectedModuleDefinitionOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        compute_canonical_selected_module_definition(
            ctx,
            self,
            CanonicalSelectedModuleDefinitionMode::Legacy,
        )
        .await
        .map(|result| match result {
            Ok((result, observations)) => {
                debug_assert!(observations.observations().is_empty());
                result
            }
            Err(_) => {
                unreachable!("legacy selected definition has no observed frontier")
            }
        })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
struct HostCanonicalSelectedModuleDefinitionObservationKey(
    HostCanonicalSelectedModuleDefinitionKey,
);

#[rustfmt::skip]
#[allow(dead_code)]
impl HostCanonicalSelectedModuleDefinitionObservationKey {
    fn new(workspace: NormalizedAbsolutePath, canonical_repo: CanonicalRepoName) -> Self { Self(HostCanonicalSelectedModuleDefinitionKey::new(workspace, canonical_repo)) }
}

impl fmt::Display for HostCanonicalSelectedModuleDefinitionObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type SelectedDefinitionResult =
    Arc<Result<HostCanonicalSelectedModuleDefinition, HostCanonicalSelectedModuleDefinitionError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)]
struct ObservedHostCanonicalSelectedModuleDefinition {
    result: SelectedDefinitionResult,
    observations: PathObservationEpoch,
}

#[rustfmt::skip]
#[allow(dead_code)]
impl ObservedHostCanonicalSelectedModuleDefinition {
    fn result(&self) -> &SelectedDefinitionResult { &self.result }
    fn observations(&self) -> &PathObservationEpoch { &self.observations }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum HostCanonicalSelectedModuleDefinitionObservationError {
    Routes(HostSelectedModuleRoutesObservationError),
}

#[rustfmt::skip]
#[derive(Clone, Copy)]
enum CanonicalSelectedModuleDefinitionMode { Legacy, Observed }

type SelectedDefinitionDriverOutcome = SourcePreparationOutcome<
    Result<
        (SelectedDefinitionResult, PathObservationEpoch),
        HostCanonicalSelectedModuleDefinitionObservationError,
    >,
>;

fn complete_canonical_selected_definition_driver(
    child: RepoSpecChild<
        HostSelectedModuleRoutes,
        HostSelectedModuleRoutesError,
        HostSelectedModuleRoutesObservationError,
    >,
    key: &HostCanonicalSelectedModuleDefinitionKey,
) -> SelectedDefinitionDriverOutcome {
    let complete = |result, observations| {
        SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
    };
    let terminal = |inner, observations| {
        complete(
            Err(HostCanonicalSelectedModuleDefinitionError { inner }),
            observations,
        )
    };
    let (predecessor, observations) = match child {
        RepoSpecChild::Compute(message) => {
            return terminal(
                PrivateCanonicalSelectedModuleDefinitionError::RoutesCompute(
                    message,
                    key.canonical_repo.clone(),
                ),
                PathObservationEpoch::empty(),
            );
        }
        RepoSpecChild::Need(need) => return SourcePreparationOutcome::Need(need),
        RepoSpecChild::Outer(error) => {
            return SourcePreparationOutcome::Complete(Err(
                HostCanonicalSelectedModuleDefinitionObservationError::Routes(error),
            ));
        }
        RepoSpecChild::Complete {
            result,
            observations,
        } => (result, observations),
    };
    let Ok(routes) = predecessor.as_ref() else {
        return terminal(
            PrivateCanonicalSelectedModuleDefinitionError::Routes(
                predecessor,
                key.canonical_repo.clone(),
            ),
            observations,
        );
    };
    let ordinal = match find_canonical_route_ordinal(&key.canonical_repo, routes.entries.iter()) {
        CanonicalRouteMatch::Missing => {
            return terminal(
                PrivateCanonicalSelectedModuleDefinitionError::Missing {
                    predecessor,
                    canonical_repo: key.canonical_repo.clone(),
                },
                observations,
            );
        }
        CanonicalRouteMatch::Unique(ordinal) => ordinal,
        CanonicalRouteMatch::Duplicate {
            first_ordinal,
            conflicting_ordinal,
        } => {
            return terminal(
                PrivateCanonicalSelectedModuleDefinitionError::Duplicate {
                    predecessor,
                    canonical_repo: key.canonical_repo.clone(),
                    first_ordinal,
                    conflicting_ordinal,
                },
                observations,
            );
        }
    };
    if matches!(
        &routes.entries[ordinal].entry.source,
        HostGraphModuleSource::Discovered(module)
            if matches!(
                &module.provenance,
                HostDiscoveredModuleProvenance::BuiltinBazelTools { .. }
            )
    ) {
        return terminal(
            PrivateCanonicalSelectedModuleDefinitionError::BuiltinDeferred {
                predecessor,
                ordinal,
                canonical_repo: key.canonical_repo.clone(),
            },
            observations,
        );
    }
    complete(
        Ok(HostCanonicalSelectedModuleDefinition {
            routes: predecessor,
            ordinal,
        }),
        observations,
    )
}

async fn compute_canonical_selected_module_definition(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalSelectedModuleDefinitionKey,
    mode: CanonicalSelectedModuleDefinitionMode,
) -> SelectedDefinitionDriverOutcome {
    let child = match mode {
        CanonicalSelectedModuleDefinitionMode::Legacy => {
            match ctx
                .compute(&HostSelectedModuleRoutesKey::new(key.workspace.dupe()))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
                Ok(SourcePreparationOutcome::Complete(result)) => RepoSpecChild::Complete {
                    result,
                    observations: PathObservationEpoch::empty(),
                },
            }
        }
        CanonicalSelectedModuleDefinitionMode::Observed => {
            match ctx
                .compute(&HostSelectedModuleRoutesObservationKey::new(
                    key.workspace.dupe(),
                ))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
                Ok(SourcePreparationOutcome::Complete(Err(error))) => RepoSpecChild::Outer(error),
                Ok(SourcePreparationOutcome::Complete(Ok(observed))) => RepoSpecChild::Complete {
                    result: observed.result().dupe(),
                    observations: observed.observations().dupe(),
                },
            }
        }
    };
    complete_canonical_selected_definition_driver(child, key)
}

#[async_trait]
impl Key for HostCanonicalSelectedModuleDefinitionObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostCanonicalSelectedModuleDefinition,
            HostCanonicalSelectedModuleDefinitionObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        compute_canonical_selected_module_definition(
            ctx,
            &self.0,
            CanonicalSelectedModuleDefinitionMode::Observed,
        )
        .await
        .map(|result| {
            result.map(
                |(result, observations)| ObservedHostCanonicalSelectedModuleDefinition {
                    result,
                    observations,
                },
            )
        })
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

type ExtensionMappingsResult =
    Arc<Result<HostSelectedExtensionMappings, HostSelectedExtensionMappingsError>>;
type ExtensionMappingsOutcome = SourcePreparationOutcome<ExtensionMappingsResult>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
struct HostSelectedExtensionMappingsObservationKey(HostSelectedExtensionMappingsKey);

#[allow(dead_code)]
impl HostSelectedExtensionMappingsObservationKey {
    fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self(HostSelectedExtensionMappingsKey::new(workspace))
    }
}

impl fmt::Display for HostSelectedExtensionMappingsObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)]
struct ObservedHostSelectedExtensionMappings {
    result: ExtensionMappingsResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedHostSelectedExtensionMappings {
    fn result(&self) -> &ExtensionMappingsResult {
        &self.result
    }

    fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
enum ExtensionMappingsObservationStage {
    Routes,
    RootFiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum ExtensionMappingsObservationError {
    Routes(HostSelectedModuleRoutesObservationError),
    RootFiles(ObservedPathFrontierError),
    Merge {
        stage: ExtensionMappingsObservationStage,
        error: ObservedPathFrontierError,
    },
}

type ExtensionMappingsDriverOutcome = SourcePreparationOutcome<
    Result<(ExtensionMappingsResult, PathObservationEpoch), ExtensionMappingsObservationError>,
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
            order: entries.keys().cloned().collect(),
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
                order: entries.keys().cloned().collect(),
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

fn extension_mappings_complete(
    result: Result<HostSelectedExtensionMappings, HostSelectedExtensionMappingsError>,
    observations: PathObservationEpoch,
) -> ExtensionMappingsDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn extension_mapping_merge_error(
    stage: ExtensionMappingsObservationStage,
    error: PathObservationEpochError,
) -> ExtensionMappingsObservationError {
    ExtensionMappingsObservationError::Merge {
        stage,
        error: error.into(),
    }
}

fn merge_extension_mapping_observations(
    prefix: &mut PathObservationEpoch,
    incoming: &PathObservationEpoch,
    stage: ExtensionMappingsObservationStage,
) -> Result<(), ExtensionMappingsObservationError> {
    *prefix = PathObservationEpoch::from_shared(
        prefix
            .observations()
            .iter()
            .chain(incoming.observations())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(|error| extension_mapping_merge_error(stage, error))?;
    Ok(())
}

async fn extension_routes_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: RoutesMode,
) -> RepoSpecChild<
    HostSelectedModuleRoutes,
    HostSelectedModuleRoutesError,
    HostSelectedModuleRoutesObservationError,
> {
    match mode {
        RoutesMode::Legacy => {
            match ctx
                .compute(&HostSelectedModuleRoutesKey::new(workspace.dupe()))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
                Ok(SourcePreparationOutcome::Complete(result)) => RepoSpecChild::Complete {
                    result,
                    observations: PathObservationEpoch::empty(),
                },
            }
        }
        RoutesMode::Observed => {
            match ctx
                .compute(&HostSelectedModuleRoutesObservationKey::new(
                    workspace.dupe(),
                ))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
                Ok(SourcePreparationOutcome::Complete(Err(error))) => RepoSpecChild::Outer(error),
                Ok(SourcePreparationOutcome::Complete(Ok(observed))) => RepoSpecChild::Complete {
                    result: observed.result().dupe(),
                    observations: observed.observations().dupe(),
                },
            }
        }
    }
}

async fn extension_root_files_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: RoutesMode,
) -> RepoSpecChild<RootModuleFiles, CompactString> {
    match mode {
        RoutesMode::Legacy => {
            match ctx
                .compute(&RootModuleFilesKey {
                    workspace: workspace.as_path().to_owned(),
                })
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(result) => RepoSpecChild::Complete {
                    result,
                    observations: PathObservationEpoch::empty(),
                },
            }
        }
        RoutesMode::Observed => {
            match ctx
                .compute(&RootModuleFilesObservationKey::new(workspace.dupe()))
                .await
            {
                Err(error) => RepoSpecChild::Compute(error.to_string().into()),
                Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
                Ok(SourcePreparationOutcome::Complete(Err(error))) => RepoSpecChild::Outer(error),
                Ok(SourcePreparationOutcome::Complete(Ok(observed))) => RepoSpecChild::Complete {
                    result: observed.result().dupe(),
                    observations: observed.observations().dupe(),
                },
            }
        }
    }
}

fn finish_extension_routes_child(
    child: RepoSpecChild<
        HostSelectedModuleRoutes,
        HostSelectedModuleRoutesError,
        HostSelectedModuleRoutesObservationError,
    >,
    observations: &mut PathObservationEpoch,
) -> Result<RoutesResult, ExtensionMappingsDriverOutcome> {
    match child {
        RepoSpecChild::Compute(message) => Err(extension_mappings_complete(
            Err(HostSelectedExtensionMappingsError::RoutesCompute(message)),
            observations.dupe(),
        )),
        RepoSpecChild::Need(need) => Err(SourcePreparationOutcome::Need(need)),
        RepoSpecChild::Outer(error) => Err(SourcePreparationOutcome::Complete(Err(
            ExtensionMappingsObservationError::Routes(error),
        ))),
        RepoSpecChild::Complete {
            result,
            observations: incoming,
        } => {
            merge_extension_mapping_observations(
                observations,
                &incoming,
                ExtensionMappingsObservationStage::Routes,
            )
            .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
            Ok(result)
        }
    }
}

fn finish_extension_root_files_child(
    child: RepoSpecChild<RootModuleFiles, CompactString>,
    observations: &mut PathObservationEpoch,
) -> Result<Arc<Result<RootModuleFiles, CompactString>>, ExtensionMappingsDriverOutcome> {
    match child {
        RepoSpecChild::Compute(message) => Err(extension_mappings_complete(
            Err(HostSelectedExtensionMappingsError::RootFilesCompute(
                message,
            )),
            observations.dupe(),
        )),
        RepoSpecChild::Need(need) => Err(SourcePreparationOutcome::Need(need)),
        RepoSpecChild::Outer(error) => Err(SourcePreparationOutcome::Complete(Err(
            ExtensionMappingsObservationError::RootFiles(error),
        ))),
        RepoSpecChild::Complete {
            result,
            observations: incoming,
        } => {
            merge_extension_mapping_observations(
                observations,
                &incoming,
                ExtensionMappingsObservationStage::RootFiles,
            )
            .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
            Ok(result)
        }
    }
}

fn finish_extension_routes_semantic(
    result: &Result<HostSelectedModuleRoutes, HostSelectedModuleRoutesError>,
    observations: PathObservationEpoch,
) -> Result<Arc<HostSelectedModuleRoutes>, ExtensionMappingsDriverOutcome> {
    result
        .as_ref()
        .map(|routes| Arc::new(routes.clone()))
        .map_err(|error| {
            extension_mappings_complete(
                Err(HostSelectedExtensionMappingsError::Routes(error.clone())),
                observations,
            )
        })
}

fn finish_extension_root_files_semantic(
    result: &Result<RootModuleFiles, CompactString>,
    observations: PathObservationEpoch,
) -> Result<Arc<[RootExtensionUsage]>, ExtensionMappingsDriverOutcome> {
    result
        .as_ref()
        .map(|files| files.extension_usages.clone())
        .map_err(|error| {
            extension_mappings_complete(
                Err(HostSelectedExtensionMappingsError::RootFiles(error.clone())),
                observations,
            )
        })
}

async fn drive_selected_extension_mappings(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedExtensionMappingsKey,
    mode: RoutesMode,
) -> ExtensionMappingsDriverOutcome {
    let mut observations = PathObservationEpoch::empty();
    let routes = match finish_extension_routes_child(
        extension_routes_child(ctx, &key.workspace, mode).await,
        &mut observations,
    ) {
        Ok(result) => result,
        Err(terminal) => return terminal,
    };
    let routes = match finish_extension_routes_semantic(&routes, observations.dupe()) {
        Ok(routes) => routes,
        Err(terminal) => return terminal,
    };
    let root_files = match finish_extension_root_files_child(
        extension_root_files_child(ctx, &key.workspace, mode).await,
        &mut observations,
    ) {
        Ok(result) => result,
        Err(terminal) => return terminal,
    };
    let root_usages = match finish_extension_root_files_semantic(&root_files, observations.dupe()) {
        Ok(usages) => usages,
        Err(terminal) => return terminal,
    };
    extension_mappings_complete(
        selected_extension_mappings(routes, root_usages),
        observations,
    )
}

fn project_legacy_extension_mappings(
    outcome: ExtensionMappingsDriverOutcome,
) -> ExtensionMappingsOutcome {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, _))) => {
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy extension mappings have no observed frontier")
        }
    }
}

#[async_trait]
impl Key for HostSelectedExtensionMappingsKey {
    type Value = ExtensionMappingsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_extension_mappings(
            drive_selected_extension_mappings(ctx, self, RoutesMode::Legacy).await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostSelectedExtensionMappingsObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostSelectedExtensionMappings, ExtensionMappingsObservationError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_selected_extension_mappings(ctx, &self.0, RoutesMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostSelectedExtensionMappings {
                    result,
                    observations,
                }))
            }
        }
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

type DefinitionLoadRequestsResult = Arc<
    Result<
        HostSelectedExtensionDefinitionLoadRequests,
        HostSelectedExtensionDefinitionLoadRequestsError,
    >,
>;
type DefinitionLoadRequestsOutcome = SourcePreparationOutcome<DefinitionLoadRequestsResult>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedExtensionDefinitionLoadRequestsObservationKey(
    HostSelectedExtensionDefinitionLoadRequestsKey,
);

impl HostSelectedExtensionDefinitionLoadRequestsObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self(HostSelectedExtensionDefinitionLoadRequestsKey::new(
            workspace,
        ))
    }
}

impl fmt::Display for HostSelectedExtensionDefinitionLoadRequestsObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostSelectedExtensionDefinitionLoadRequests {
    result: DefinitionLoadRequestsResult,
    observations: PathObservationEpoch,
}

impl ObservedHostSelectedExtensionDefinitionLoadRequests {
    pub fn result(
        &self,
    ) -> &Arc<
        Result<
            HostSelectedExtensionDefinitionLoadRequests,
            HostSelectedExtensionDefinitionLoadRequestsError,
        >,
    > {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum DefinitionLoadRequestsObservationError {
    Mappings(ExtensionMappingsObservationError),
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct HostSelectedExtensionDefinitionLoadRequestsObservationError(
    DefinitionLoadRequestsObservationError,
);

type DefinitionLoadRequestsDriverOutcome = SourcePreparationOutcome<
    Result<
        (DefinitionLoadRequestsResult, PathObservationEpoch),
        DefinitionLoadRequestsObservationError,
    >,
>;

async fn definition_load_requests_mappings_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: RoutesMode,
) -> RepoSpecChild<
    HostSelectedExtensionMappings,
    HostSelectedExtensionMappingsError,
    ExtensionMappingsObservationError,
> {
    match mode {
        RoutesMode::Legacy => match ctx
            .compute(&HostSelectedExtensionMappingsKey::new(workspace.dupe()))
            .await
        {
            Err(error) => RepoSpecChild::Compute(error.to_string().into()),
            Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => RepoSpecChild::Complete {
                result,
                observations: PathObservationEpoch::empty(),
            },
        },
        RoutesMode::Observed => match ctx
            .compute(&HostSelectedExtensionMappingsObservationKey::new(
                workspace.dupe(),
            ))
            .await
        {
            Err(error) => RepoSpecChild::Compute(error.to_string().into()),
            Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => RepoSpecChild::Outer(error),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => RepoSpecChild::Complete {
                result: observed.result().dupe(),
                observations: observed.observations().dupe(),
            },
        },
    }
}

fn definition_load_requests_complete(
    result: Result<
        HostSelectedExtensionDefinitionLoadRequests,
        HostSelectedExtensionDefinitionLoadRequestsError,
    >,
    observations: PathObservationEpoch,
) -> DefinitionLoadRequestsDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn finish_definition_load_requests_mappings_child(
    child: RepoSpecChild<
        HostSelectedExtensionMappings,
        HostSelectedExtensionMappingsError,
        ExtensionMappingsObservationError,
    >,
) -> Result<(ExtensionMappingsResult, PathObservationEpoch), DefinitionLoadRequestsDriverOutcome> {
    match child {
        RepoSpecChild::Compute(message) => Err(definition_load_requests_complete(
            Err(HostSelectedExtensionDefinitionLoadRequestsError(
                HostSelectedExtensionDefinitionLoadRequestsErrorInner::MappingsCompute(message),
            )),
            PathObservationEpoch::empty(),
        )),
        RepoSpecChild::Need(need) => Err(SourcePreparationOutcome::Need(need)),
        RepoSpecChild::Outer(error) => Err(SourcePreparationOutcome::Complete(Err(
            DefinitionLoadRequestsObservationError::Mappings(error),
        ))),
        RepoSpecChild::Complete {
            result,
            observations,
        } => Ok((result, observations)),
    }
}

fn finish_definition_load_requests_mappings_semantic(
    result: &Result<HostSelectedExtensionMappings, HostSelectedExtensionMappingsError>,
    observations: PathObservationEpoch,
) -> Result<Arc<HostSelectedExtensionMappings>, DefinitionLoadRequestsDriverOutcome> {
    result
        .as_ref()
        .map(|mappings| Arc::new(mappings.clone()))
        .map_err(|error| {
            definition_load_requests_complete(
                Err(HostSelectedExtensionDefinitionLoadRequestsError(
                    HostSelectedExtensionDefinitionLoadRequestsErrorInner::Mappings(error.clone()),
                )),
                observations,
            )
        })
}

async fn drive_definition_load_requests(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedExtensionDefinitionLoadRequestsKey,
    mode: RoutesMode,
) -> DefinitionLoadRequestsDriverOutcome {
    let (mappings, observations) = match finish_definition_load_requests_mappings_child(
        definition_load_requests_mappings_child(ctx, &key.workspace, mode).await,
    ) {
        Ok(value) => value,
        Err(terminal) => return terminal,
    };
    let predecessor =
        match finish_definition_load_requests_mappings_semantic(&mappings, observations.dupe()) {
            Ok(value) => value,
            Err(terminal) => return terminal,
        };
    definition_load_requests_complete(
        selected_extension_definition_load_requests(key.workspace.dupe(), predecessor),
        observations,
    )
}

fn project_legacy_definition_load_requests(
    outcome: DefinitionLoadRequestsDriverOutcome,
) -> DefinitionLoadRequestsOutcome {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, _))) => {
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy definition-load requests have no observed frontier")
        }
    }
}

#[async_trait]
impl Key for HostSelectedExtensionDefinitionLoadRequestsKey {
    type Value = DefinitionLoadRequestsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_definition_load_requests(
            drive_definition_load_requests(ctx, self, RoutesMode::Legacy).await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostSelectedExtensionDefinitionLoadRequestsObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostSelectedExtensionDefinitionLoadRequests,
            HostSelectedExtensionDefinitionLoadRequestsObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_definition_load_requests(ctx, &self.0, RoutesMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionDefinitionLoadRequestsObservationError(error),
                ))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(
                    ObservedHostSelectedExtensionDefinitionLoadRequests {
                        result,
                        observations,
                    },
                ))
            }
        }
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

type EvaluationInputRequestsResult = Arc<
    Result<
        HostSelectedExtensionEvaluationInputRequests,
        HostSelectedExtensionEvaluationInputRequestsError,
    >,
>;
type EvaluationInputRequestsOutcome = SourcePreparationOutcome<EvaluationInputRequestsResult>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
enum EvaluationInputObservationStage {
    RootFiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum EvaluationInputRequestsObservationError {
    Requests(DefinitionLoadRequestsObservationError),
    RootFiles(ObservedPathFrontierError),
    Merge {
        stage: EvaluationInputObservationStage,
        error: ObservedPathFrontierError,
    },
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct HostSelectedExtensionEvaluationInputRequestsObservationError(
    EvaluationInputRequestsObservationError,
);

type EvaluationInputRequestsDriverOutcome = SourcePreparationOutcome<
    Result<
        (EvaluationInputRequestsResult, PathObservationEpoch),
        EvaluationInputRequestsObservationError,
    >,
>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedExtensionEvaluationInputRequestsObservationKey(
    HostSelectedExtensionEvaluationInputRequestsKey,
);

impl HostSelectedExtensionEvaluationInputRequestsObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self(HostSelectedExtensionEvaluationInputRequestsKey::new(
            workspace,
        ))
    }
}

impl fmt::Display for HostSelectedExtensionEvaluationInputRequestsObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostSelectedExtensionEvaluationInputRequests {
    result: EvaluationInputRequestsResult,
    observations: PathObservationEpoch,
}

impl ObservedHostSelectedExtensionEvaluationInputRequests {
    pub fn result(
        &self,
    ) -> &Arc<
        Result<
            HostSelectedExtensionEvaluationInputRequests,
            HostSelectedExtensionEvaluationInputRequestsError,
        >,
    > {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

fn evaluation_input_requests_complete(
    result: Result<
        HostSelectedExtensionEvaluationInputRequests,
        HostSelectedExtensionEvaluationInputRequestsError,
    >,
    observations: PathObservationEpoch,
) -> EvaluationInputRequestsDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

async fn evaluation_input_requests_request_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: RoutesMode,
) -> RepoSpecChild<
    HostSelectedExtensionDefinitionLoadRequests,
    HostSelectedExtensionDefinitionLoadRequestsError,
    HostSelectedExtensionDefinitionLoadRequestsObservationError,
> {
    match mode {
        RoutesMode::Legacy => match ctx
            .compute(&HostSelectedExtensionDefinitionLoadRequestsKey::new(
                workspace.dupe(),
            ))
            .await
        {
            Err(error) => RepoSpecChild::Compute(error.to_string().into()),
            Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => RepoSpecChild::Complete {
                result,
                observations: PathObservationEpoch::empty(),
            },
        },
        RoutesMode::Observed => match ctx
            .compute(
                &HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(workspace.dupe()),
            )
            .await
        {
            Err(error) => RepoSpecChild::Compute(error.to_string().into()),
            Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => RepoSpecChild::Outer(error),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => RepoSpecChild::Complete {
                result: observed.result().dupe(),
                observations: observed.observations().dupe(),
            },
        },
    }
}

fn finish_evaluation_input_requests_request_child(
    child: RepoSpecChild<
        HostSelectedExtensionDefinitionLoadRequests,
        HostSelectedExtensionDefinitionLoadRequestsError,
        HostSelectedExtensionDefinitionLoadRequestsObservationError,
    >,
) -> Result<
    (DefinitionLoadRequestsResult, PathObservationEpoch),
    EvaluationInputRequestsDriverOutcome,
> {
    match child {
        RepoSpecChild::Compute(message) => Err(evaluation_input_requests_complete(
            Err(HostSelectedExtensionEvaluationInputRequestsError::LoadRequestsCompute(message)),
            PathObservationEpoch::empty(),
        )),
        RepoSpecChild::Need(need) => Err(SourcePreparationOutcome::Need(need)),
        RepoSpecChild::Outer(HostSelectedExtensionDefinitionLoadRequestsObservationError(
            error,
        )) => Err(SourcePreparationOutcome::Complete(Err(
            EvaluationInputRequestsObservationError::Requests(error),
        ))),
        RepoSpecChild::Complete {
            result,
            observations,
        } => Ok((result, observations)),
    }
}

fn finish_evaluation_input_requests_request_semantic(
    result: &Result<
        HostSelectedExtensionDefinitionLoadRequests,
        HostSelectedExtensionDefinitionLoadRequestsError,
    >,
    observations: PathObservationEpoch,
) -> Result<Arc<HostSelectedExtensionDefinitionLoadRequests>, EvaluationInputRequestsDriverOutcome>
{
    result
        .as_ref()
        .map(|requests| Arc::new(requests.clone()))
        .map_err(|error| {
            evaluation_input_requests_complete(
                Err(HostSelectedExtensionEvaluationInputRequestsError::LoadRequests(error.clone())),
                observations,
            )
        })
}

async fn evaluation_input_requests_root_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: RoutesMode,
) -> RepoSpecChild<RootModuleFiles, CompactString, ObservedPathFrontierError> {
    match mode {
        RoutesMode::Legacy => match ctx
            .compute(&RootModuleFilesKey {
                workspace: workspace.as_path().to_owned(),
            })
            .await
        {
            Err(error) => RepoSpecChild::Compute(error.to_string().into()),
            Ok(result) => RepoSpecChild::Complete {
                result,
                observations: PathObservationEpoch::empty(),
            },
        },
        RoutesMode::Observed => match ctx
            .compute(&RootModuleFilesObservationKey::new(workspace.dupe()))
            .await
        {
            Err(error) => RepoSpecChild::Compute(error.to_string().into()),
            Ok(SourcePreparationOutcome::Need(need)) => RepoSpecChild::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => RepoSpecChild::Outer(error),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => RepoSpecChild::Complete {
                result: observed.result().dupe(),
                observations: observed.observations().dupe(),
            },
        },
    }
}

fn evaluation_input_merge_error(
    stage: EvaluationInputObservationStage,
    error: PathObservationEpochError,
) -> EvaluationInputRequestsObservationError {
    EvaluationInputRequestsObservationError::Merge {
        stage,
        error: error.into(),
    }
}

fn merge_evaluation_input_observations(
    prefix: &mut PathObservationEpoch,
    incoming: &PathObservationEpoch,
    stage: EvaluationInputObservationStage,
) -> Result<(), EvaluationInputRequestsObservationError> {
    *prefix = PathObservationEpoch::from_shared(
        prefix
            .observations()
            .iter()
            .chain(incoming.observations())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(|error| evaluation_input_merge_error(stage, error))?;
    Ok(())
}

fn finish_evaluation_input_requests_root_child(
    child: RepoSpecChild<RootModuleFiles, CompactString, ObservedPathFrontierError>,
    requests: Arc<HostSelectedExtensionDefinitionLoadRequests>,
    mut observations: PathObservationEpoch,
) -> Result<(Arc<RootModuleFiles>, PathObservationEpoch), EvaluationInputRequestsDriverOutcome> {
    let (result, incoming) = match child {
        RepoSpecChild::Compute(message) => {
            return Err(evaluation_input_requests_complete(
                Err(
                    HostSelectedExtensionEvaluationInputRequestsError::AfterRequests {
                        load_requests: requests,
                        request: None,
                        error: HostSelectedExtensionEvaluationInputError::RootFilesCompute(message),
                    },
                ),
                observations,
            ));
        }
        RepoSpecChild::Need(need) => return Err(SourcePreparationOutcome::Need(need)),
        RepoSpecChild::Outer(error) => {
            return Err(SourcePreparationOutcome::Complete(Err(
                EvaluationInputRequestsObservationError::RootFiles(error),
            )));
        }
        RepoSpecChild::Complete {
            result,
            observations,
        } => (result, observations),
    };
    if let Err(error) = merge_evaluation_input_observations(
        &mut observations,
        &incoming,
        EvaluationInputObservationStage::RootFiles,
    ) {
        return Err(SourcePreparationOutcome::Complete(Err(error)));
    }
    match result.as_ref() {
        Ok(root) => Ok((Arc::new(root.clone()), observations)),
        Err(error) => Err(evaluation_input_requests_complete(
            Err(
                HostSelectedExtensionEvaluationInputRequestsError::AfterRequests {
                    load_requests: requests,
                    request: None,
                    error: HostSelectedExtensionEvaluationInputError::RootFiles(error.clone()),
                },
            ),
            observations,
        )),
    }
}

async fn drive_evaluation_input_requests(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedExtensionEvaluationInputRequestsKey,
    mode: RoutesMode,
) -> EvaluationInputRequestsDriverOutcome {
    let (requests, observations) = match finish_evaluation_input_requests_request_child(
        evaluation_input_requests_request_child(ctx, &key.workspace, mode).await,
    ) {
        Ok(value) => value,
        Err(terminal) => return terminal,
    };
    let requests =
        match finish_evaluation_input_requests_request_semantic(&requests, observations.dupe()) {
            Ok(value) => value,
            Err(terminal) => return terminal,
        };
    let (root_files, observations) = match finish_evaluation_input_requests_root_child(
        evaluation_input_requests_root_child(ctx, &key.workspace, mode).await,
        requests.dupe(),
        observations,
    ) {
        Ok(value) => value,
        Err(terminal) => return terminal,
    };
    evaluation_input_requests_complete(
        selected_extension_evaluation_input_requests(requests, &root_files),
        observations,
    )
}

fn project_legacy_evaluation_input_requests(
    outcome: EvaluationInputRequestsDriverOutcome,
) -> EvaluationInputRequestsOutcome {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, _))) => {
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy evaluation-input requests have no observed frontier")
        }
    }
}

#[async_trait]
impl Key for HostSelectedExtensionEvaluationInputRequestsKey {
    type Value = EvaluationInputRequestsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_evaluation_input_requests(
            drive_evaluation_input_requests(ctx, self, RoutesMode::Legacy).await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostSelectedExtensionEvaluationInputRequestsObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostSelectedExtensionEvaluationInputRequests,
            HostSelectedExtensionEvaluationInputRequestsObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_evaluation_input_requests(ctx, &self.0, RoutesMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(
                Err(HostSelectedExtensionEvaluationInputRequestsObservationError(error)),
            ),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(
                    ObservedHostSelectedExtensionEvaluationInputRequests {
                        result,
                        observations,
                    },
                ))
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

type RetainedExtensionMappings =
    Arc<Result<HostSelectedExtensionMappings, HostSelectedExtensionMappingsError>>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRootRepositoryMapping {
    predecessor: RetainedExtensionMappings,
    root_ordinal: usize,
}

impl HostRootRepositoryMapping {
    pub fn view(&self) -> Option<HostRootRepositoryMappingView<'_>> {
        let predecessor = self.predecessor.as_ref().as_ref().ok()?;
        predecessor
            .mappings
            .get(self.root_ordinal)
            .map(|mapping| HostRootRepositoryMappingView { mapping })
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct HostRootRepositoryMappingView<'a> {
    mapping: &'a HostSelectedRepositoryMapping,
}

impl<'a> HostRootRepositoryMappingView<'a> {
    pub fn canonical_repo(self) -> &'a CanonicalRepoName {
        &self.mapping.context_repo
    }
    pub fn mapping_context(self) -> &'a CanonicalRepoName {
        &self.mapping.context_repo
    }
    pub fn mapping(self) -> HostRootRepositoryMappingIter<'a> {
        HostRootRepositoryMappingIter {
            order: self.mapping.order.iter(),
            entries: &self.mapping.entries,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct HostRootRepositoryMappingIter<'a> {
    order: std::slice::Iter<'a, ApparentRepoName>,
    entries: &'a SmallMap<ApparentRepoName, CanonicalRepoName>,
}

impl<'a> Iterator for HostRootRepositoryMappingIter<'a> {
    type Item = (&'a ApparentRepoName, &'a CanonicalRepoName);

    fn next(&mut self) -> Option<Self::Item> {
        let name = self.order.next()?;
        Some((name, self.entries.get(name).expect("valid mapping order")))
    }
}

impl ExactSizeIterator for HostRootRepositoryMappingIter<'_> {
    fn len(&self) -> usize {
        self.order.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum PrivateRootRepositoryMappingError {
    Predecessor(RetainedExtensionMappings),
    Compute(CompactString),
    Invalid {
        predecessor: RetainedExtensionMappings,
        reason: RootMappingInvalid,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RootMappingInvalid {
    Missing,
    Duplicate { first: usize, conflicting: usize },
    Context { ordinal: usize },
}

fn root_mapping_ordinal(
    mappings: &HostSelectedExtensionMappings,
) -> Result<usize, RootMappingInvalid> {
    let (mut root, mut conflicting) = (None, None);
    for (ordinal, route) in mappings.routes.entries.iter().enumerate() {
        if matches!(route.entry.key, HostGraphModuleKey::Root) {
            if root.is_none() {
                root = Some(ordinal);
            } else if conflicting.is_none() {
                conflicting = Some(ordinal);
            }
        }
    }
    let root = root.ok_or(RootMappingInvalid::Missing)?;
    if let Some(conflicting) = conflicting {
        return Err(RootMappingInvalid::Duplicate {
            first: root,
            conflicting,
        });
    }
    mappings
        .mappings
        .get(root)
        .filter(|mapping| mapping.context_repo == CanonicalRepoName::root())
        .map(|_| root)
        .ok_or(RootMappingInvalid::Context { ordinal: root })
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRootRepositoryMappingError {
    workspace: NormalizedAbsolutePath,
    inner: PrivateRootRepositoryMappingError,
}

impl fmt::Display for HostRootRepositoryMappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl std::error::Error for HostRootRepositoryMappingError {}

#[doc(hidden)]
pub type HostRootRepositoryMappingOutcome = SourcePreparationOutcome<
    Arc<Result<HostRootRepositoryMapping, HostRootRepositoryMappingError>>,
>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostRootRepositoryMappingKey {
    workspace: NormalizedAbsolutePath,
}

impl HostRootRepositoryMappingKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostRootRepositoryMappingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-root-repository-mapping:{}", self.workspace)
    }
}

#[async_trait]
impl Key for HostRootRepositoryMappingKey {
    type Value = HostRootRepositoryMappingOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let terminal = |inner| {
            SourcePreparationOutcome::Complete(Arc::new(Err(HostRootRepositoryMappingError {
                workspace: self.workspace.clone(),
                inner,
            })))
        };
        let predecessor = match ctx
            .compute(&HostSelectedExtensionMappingsKey::new(
                self.workspace.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => value,
            Err(error) => {
                return terminal(PrivateRootRepositoryMappingError::Compute(
                    error.to_string().into(),
                ));
            }
        };
        if predecessor.as_ref().is_err() {
            return terminal(PrivateRootRepositoryMappingError::Predecessor(predecessor));
        }
        let mappings = predecessor.as_ref().as_ref().expect("checked above");
        let root_ordinal = match root_mapping_ordinal(mappings) {
            Ok(root) => root,
            Err(reason) => {
                return terminal(PrivateRootRepositoryMappingError::Invalid {
                    predecessor,
                    reason,
                });
            }
        };
        SourcePreparationOutcome::Complete(Arc::new(Ok(HostRootRepositoryMapping {
            predecessor,
            root_ordinal,
        })))
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
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use async_trait::async_trait;
    use compact_str::CompactString;
    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::Key;
    use dice::RichActivation;
    use dice::UserComputationData;
    use dupe::Dupe;
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_events_v2::EventBatch;
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

    #[derive(Debug, PartialEq, Eq)]
    struct ExternalSelectedModuleSnapshot {
        kind: crate::HostCanonicalSelectedModuleKind,
        identity: Option<(String, String)>,
        canonical_repo: String,
        mapping_context: String,
        mapping: Vec<(String, String)>,
        repo_rule: Option<(String, String, Vec<String>)>,
        local_path_policy: Option<crate::HostRepositoryLocalPathPolicy>,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct ExternalRootMappingSnapshot {
        canonical_repo: String,
        mapping_context: String,
        mapping: Vec<(String, String)>,
    }

    fn external_root_mapping_snapshot(
        mapping: &crate::HostRootRepositoryMapping,
    ) -> ExternalRootMappingSnapshot {
        let view: crate::HostRootRepositoryMappingView<'_> = mapping.view().unwrap();
        let mut entries: crate::HostRootRepositoryMappingIter<'_> = view.mapping();
        let len = entries.len();
        let mapping = entries
            .by_ref()
            .map(|(name, target)| (name.as_str().to_owned(), target.as_str().to_owned()))
            .collect::<Vec<_>>();
        assert_eq!(mapping.len(), len);
        ExternalRootMappingSnapshot {
            canonical_repo: view.canonical_repo().as_str().to_owned(),
            mapping_context: view.mapping_context().as_str().to_owned(),
            mapping,
        }
    }

    fn external_style_snapshot(
        definition: &crate::HostCanonicalSelectedModuleDefinition,
    ) -> ExternalSelectedModuleSnapshot {
        let view: crate::HostCanonicalSelectedModuleDefinitionView<'_> = definition.view();
        let identity = match view.identity() {
            crate::HostCanonicalSelectedModuleIdentity::Root => None,
            crate::HostCanonicalSelectedModuleIdentity::Module {
                name,
                normalized_version,
            } => Some((name.to_owned(), normalized_version.to_owned())),
        };
        let mut mapping: crate::HostCanonicalSelectedModuleMappingIter<'_> = view.mapping();
        let mapping_len = mapping.len();
        let mapping = mapping
            .by_ref()
            .map(|(name, target)| (name.as_str().to_owned(), target.as_str().to_owned()))
            .collect::<Vec<_>>();
        assert_eq!(mapping.len(), mapping_len);
        let repo_rule = view.repo_spec().map(|spec| {
            (
                spec.rule_id.bzl_file.to_string(),
                spec.rule_id.rule_name.to_string(),
                spec.attributes.keys().map(ToString::to_string).collect(),
            )
        });
        ExternalSelectedModuleSnapshot {
            kind: view.kind(),
            identity,
            canonical_repo: view.canonical_repo().as_str().to_owned(),
            mapping_context: view.mapping_context().as_str().to_owned(),
            mapping,
            repo_rule,
            local_path_policy: view.local_path_policy(),
        }
    }

    fn assert_external_error<T: std::error::Error + Clone + Eq + Allocative>() {}

    fn external_disposition(
        error: &crate::HostCanonicalSelectedModuleDefinitionError,
    ) -> crate::HostCanonicalSelectedModuleDefinitionErrorDisposition {
        error.disposition()
    }
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

    #[derive(Debug, Clone)]
    struct RepoSpecActivation {
        key: String,
        kind: ActivationKind,
        batch: Option<EventBatch>,
    }

    #[derive(Debug, Default)]
    struct RepoSpecTracker {
        activations: Mutex<Vec<RepoSpecActivation>>,
        rows: Mutex<Vec<(String, Vec<String>)>>,
    }

    impl RepoSpecTracker {
        fn take(&self) -> (Vec<RepoSpecActivation>, Vec<(String, Vec<String>)>) {
            (
                std::mem::take(&mut *self.activations.lock().unwrap()),
                std::mem::take(&mut *self.rows.lock().unwrap()),
            )
        }
    }

    impl ActivationTracker for RepoSpecTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            self.rows
                .lock()
                .unwrap()
                .push((key.to_string(), deps.map(ToString::to_string).collect()));
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            self.activations.lock().unwrap().push(RepoSpecActivation {
                key: key.to_string(),
                kind: activation.kind(),
                batch: activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            });
        }
    }

    fn repo_spec_row<'a>(rows: &'a [(String, Vec<String>)], owner: &str) -> &'a [String] {
        &rows.iter().find(|(key, _)| key == owner).unwrap().1
    }

    fn assert_no_repo_spec_upper(rows: &[(String, Vec<String>)]) {
        let forbidden = "host-selected-module-routes: host-selected-extension- host-canonical-selected-module-definition: host-root-repository-mapping: host-generated-repository-definition: slug-command:";
        assert!(
            rows.iter()
                .all(|(owner, deps)| !forbidden.split(' ').any(|prefix| {
                    owner.starts_with(prefix) || deps.iter().any(|dep| dep.starts_with(prefix))
                }))
        );
    }
    #[derive(Default)]
    struct SelectedDefinitionTracker {
        selected: Mutex<Vec<(ActivationKind, bool)>>,
        root_mapping: Mutex<Vec<(ActivationKind, bool)>>,
        forbidden: Mutex<Vec<&'static str>>,
    }

    impl SelectedDefinitionTracker {
        fn take(&self) -> Vec<(ActivationKind, bool)> {
            std::mem::take(&mut *self.selected.lock().unwrap())
        }

        fn forbidden(&self) -> Vec<&'static str> {
            self.forbidden.lock().unwrap().clone()
        }

        fn take_root_mapping(&self) -> Vec<(ActivationKind, bool)> {
            std::mem::take(&mut *self.root_mapping.lock().unwrap())
        }
    }

    impl ActivationTracker for SelectedDefinitionTracker {
        fn key_activated(
            &self,
            _key: &DynKey,
            _deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            if key
                .downcast_ref::<HostCanonicalSelectedModuleDefinitionKey>()
                .is_some()
            {
                self.selected
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_none()));
            }
            if key.downcast_ref::<HostRootRepositoryMappingKey>().is_some() {
                self.root_mapping
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_none()));
            }
            let forbidden = if key
                .downcast_ref::<crate::RootRepositoryRouteKey>()
                .is_some()
            {
                Some("root route")
            } else if key
                .downcast_ref::<crate::RepositoryMaterializationKey>()
                .is_some()
            {
                Some("materialization")
            } else if key
                .downcast_ref::<crate::RepositorySourceFileKey>()
                .is_some()
            {
                Some("repository source")
            } else if key.downcast_ref::<RegistryFileKey>().is_some() {
                Some("registry")
            } else if key.downcast_ref::<PathObservationEpochKey>().is_some() {
                Some("filesystem")
            } else {
                None
            };
            if let Some(forbidden) = forbidden {
                self.forbidden.lock().unwrap().push(forbidden);
            }
        }
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

    struct CancelOnceRegistryIo {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl crate::RegistryIo for CancelOnceRegistryIo {
        async fn read_exact(
            &self,
            url: &RegistryFileUrl,
        ) -> Result<crate::RegistryIoOutcome, crate::RegistryTransportError> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                std::future::pending().await
            }
            let bytes: Option<&'static [u8]> = match url.as_str() {
                "https://registry.invalid/modules/dep/1/MODULE.bazel" => {
                    Some(b"module(name='dep', version='1')\n")
                }
                "https://registry.invalid/modules/dep/1/source.json" => {
                    Some(br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#)
                }
                "https://registry.invalid/bazel_registry.json" => None,
                _ => None,
            };
            Ok(bytes.map_or(crate::RegistryIoOutcome::NotFound, |bytes| {
                crate::RegistryIoOutcome::Found(Arc::from(bytes))
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

    fn host_epoch(root_module: &str, local_module: Option<&str>) -> PathObservationEpoch {
        let lock = format!("{WORKSPACE}/MODULE.bazel.lock");
        let module = format!("{WORKSPACE}/MODULE.bazel");
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
            lstat(&module, PathNodeKind::RegularFile, 4),
            (
                observation(&module, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    root_module.as_bytes(),
                ))),
            ),
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

    async fn real_transaction_with_tracker(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        mirrors: &[&str],
        include_epoch: bool,
        tracker: Option<Arc<RepoSpecTracker>>,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut data = UserComputationData {
            activation_tracker: tracker.map(|value| value as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
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
                .changed_to(vec![(
                    PathObservationEpochKey,
                    host_epoch(root, local_module),
                )])
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
                HostRegistryRefreshToken::new(1),
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
                let path = if name == "local" && root.contains("path='local/.'") {
                    "local/."
                } else {
                    name
                };
                (command_override
                    || root.contains(&format!(
                        "local_path_override(module_name='{name}', path='{path}')"
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
                                path.into()
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
                                    "{WORKSPACE}/{path}"
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

    async fn real_transaction(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        mirrors: &[&str],
        include_epoch: bool,
    ) -> dice::DiceTransaction {
        real_transaction_with_tracker(dice, root, generation, mirrors, include_epoch, None).await
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

    async fn compute_real_observed(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        mirrors: &[&str],
        include_epoch: bool,
        tracker: Option<Arc<RepoSpecTracker>>,
    ) -> <HostSelectedRegistryRepoSpecsObservationKey as Key>::Value {
        real_transaction_with_tracker(dice, root, generation, mirrors, include_epoch, tracker)
            .await
            .compute(&HostSelectedRegistryRepoSpecsObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    fn complete_observed_repo_specs(
        value: &<HostSelectedRegistryRepoSpecsObservationKey as Key>::Value,
    ) -> ObservedHostSelectedRegistryRepoSpecs {
        let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
            panic!("observed repo specs must complete: {value:?}");
        };
        observed.dupe()
    }

    fn assert_exact_repo_epoch(expected: &PathObservationEpoch, actual: &PathObservationEpoch) {
        assert_eq!(expected.observations().len(), actual.observations().len());
        for ((demand, result), (actual_demand, actual_result)) in
            expected.observations().iter().zip(actual.observations())
        {
            assert_eq!(demand, actual_demand);
            assert!(Arc::ptr_eq(result, actual_result), "{demand:?}");
        }
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

    async fn compute_real_observed_routes(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        include_epoch: bool,
        tracker: Option<Arc<RepoSpecTracker>>,
    ) -> <HostSelectedModuleRoutesObservationKey as Key>::Value {
        real_transaction_with_tracker(dice, root, generation, &[], include_epoch, tracker)
            .await
            .compute(&HostSelectedModuleRoutesObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    fn complete_observed_routes(
        value: &<HostSelectedModuleRoutesObservationKey as Key>::Value,
    ) -> ObservedHostSelectedModuleRoutes {
        let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
            panic!("observed routes must complete: {value:?}");
        };
        observed.dupe()
    }

    fn assert_no_route_upper(rows: &[(String, Vec<String>)]) {
        let forbidden = "host-canonical-selected-module-definition: host-root-repository-mapping: host-selected-extension- host-generated-repository-definition: slug-command:";
        assert!(
            rows.iter()
                .all(|(owner, deps)| !forbidden.split(' ').any(|prefix| {
                    owner.starts_with(prefix) || deps.iter().any(|dep| dep.starts_with(prefix))
                }))
        );
    }

    async fn compute_real_selected_definition(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        canonical_repo: &str,
        include_epoch: bool,
    ) -> crate::HostCanonicalSelectedModuleDefinitionOutcome {
        real_transaction(dice, root, generation, &[], include_epoch)
            .await
            .compute(&crate::HostCanonicalSelectedModuleDefinitionKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                if canonical_repo.is_empty() {
                    CanonicalRepoName::root()
                } else {
                    CanonicalRepoName::new(canonical_repo).unwrap()
                },
            ))
            .await
            .unwrap()
    }

    async fn compute_real_observed_selected_definition(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        canonical_repo: &str,
        include_epoch: bool,
        tracker: Option<Arc<RepoSpecTracker>>,
    ) -> <HostCanonicalSelectedModuleDefinitionObservationKey as Key>::Value {
        real_transaction_with_tracker(dice, root, generation, &[], include_epoch, tracker)
            .await
            .compute(&HostCanonicalSelectedModuleDefinitionObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                if canonical_repo.is_empty() {
                    CanonicalRepoName::root()
                } else {
                    CanonicalRepoName::new(canonical_repo).unwrap()
                },
            ))
            .await
            .unwrap()
    }

    fn complete_observed_selected_definition(
        value: &<HostCanonicalSelectedModuleDefinitionObservationKey as Key>::Value,
    ) -> ObservedHostCanonicalSelectedModuleDefinition {
        let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
            panic!("observed selected definition must complete: {value:?}");
        };
        observed.dupe()
    }

    fn assert_no_selected_definition_upper(rows: &[(String, Vec<String>)]) {
        let forbidden = "host-canonical-selected-module-definition: host-root-repository-mapping: host-selected-extension- host-generated-repository-definition: root-repository-route: repository-materialization: repository-source-file: slug-command: slug-bootstrap:";
        assert!(
            rows.iter().all(|(owner, deps)| {
                !forbidden.split(' ').any(|prefix| {
                    owner.starts_with(prefix) || deps.iter().any(|dep| dep.starts_with(prefix))
                })
            }),
            "unexpected selected-definition upper row: {rows:#?}"
        );
    }

    async fn observed_selected_state(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        canonical_repo: &str,
    ) -> (
        ObservedHostCanonicalSelectedModuleDefinition,
        HostCanonicalSelectedModuleDefinitionOutcome,
        PathObservationEpoch,
    ) {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let canonical = if canonical_repo.is_empty() {
            CanonicalRepoName::root()
        } else {
            CanonicalRepoName::new(canonical_repo).unwrap()
        };
        let mut transaction = real_transaction(dice, root, generation, &[], true).await;
        let observed = transaction
            .compute(&HostCanonicalSelectedModuleDefinitionObservationKey::new(
                workspace.dupe(),
                canonical.clone(),
            ))
            .await
            .unwrap();
        let observed = complete_observed_selected_definition(&observed);
        let legacy = transaction
            .compute(&HostCanonicalSelectedModuleDefinitionKey::new(
                workspace, canonical,
            ))
            .await
            .unwrap();
        let global = transaction.compute(&PathObservationEpochKey).await.unwrap();
        (observed, legacy, global)
    }

    fn assert_selected_epoch_subset(
        observed: &ObservedHostCanonicalSelectedModuleDefinition,
        global: &PathObservationEpoch,
    ) {
        for (demand, result) in observed.observations().observations() {
            assert_eq!(global.get(demand).unwrap().as_ref(), result.as_ref());
        }
    }

    fn selected_definition_hash(
        value: &HostCanonicalSelectedModuleDefinitionObservationKey,
    ) -> u64 {
        let mut state = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(value, &mut state);
        std::hash::Hasher::finish(&state)
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
    async fn compute_real_observed_extensions(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        include_epoch: bool,
        tracker: Option<Arc<RepoSpecTracker>>,
    ) -> <HostSelectedExtensionMappingsObservationKey as Key>::Value {
        real_transaction_with_tracker(dice, root, generation, &[], include_epoch, tracker)
            .await
            .compute(&HostSelectedExtensionMappingsObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    fn complete_observed_extensions(
        value: &<HostSelectedExtensionMappingsObservationKey as Key>::Value,
    ) -> ObservedHostSelectedExtensionMappings {
        let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
            panic!("observed extension mappings must complete: {value:?}");
        };
        observed.dupe()
    }

    fn assert_no_mapping_upper(rows: &[(String, Vec<String>)]) {
        let forbidden = "host-selected-extension-definition-load-requests: host-selected-extension-evaluation-inputs: host-root-repository-mapping: host-generated-repository-definition: host-canonical-selected-module-definition: slug-command:";
        assert!(rows.iter().all(|(owner, deps)| {
            !forbidden.split(' ').any(|prefix| {
                owner.starts_with(prefix) || deps.iter().any(|dep| dep.starts_with(prefix))
            })
        }));
    }

    async fn compute_real_root_mapping(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        include_epoch: bool,
    ) -> crate::HostRootRepositoryMappingOutcome {
        real_transaction(dice, root, generation, &[], include_epoch)
            .await
            .compute(&crate::HostRootRepositoryMappingKey::new(
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

    async fn compute_real_observed_definition_requests(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        include_epoch: bool,
        tracker: Option<Arc<RepoSpecTracker>>,
    ) -> <HostSelectedExtensionDefinitionLoadRequestsObservationKey as Key>::Value {
        real_transaction_with_tracker(dice, root, generation, &[], include_epoch, tracker)
            .await
            .compute(
                &HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                ),
            )
            .await
            .unwrap()
    }

    fn complete_observed_definition_requests(
        value: &<HostSelectedExtensionDefinitionLoadRequestsObservationKey as Key>::Value,
    ) -> ObservedHostSelectedExtensionDefinitionLoadRequests {
        let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
            panic!("observed definition requests must complete: {value:?}");
        };
        observed.dupe()
    }

    fn assert_no_definition_request_upper(rows: &[(String, Vec<String>)]) {
        let forbidden = "host-bzl-module: observed-host-bzl-module: host-loaded-module-extension-definitions: host-selected-extension-evaluation-inputs: host-prepared-module-extension-inputs: host-pure-module-extension-invocations: host-instantiated-module-extension-repositories: host-validated-module-extension-repositories: host-root-repository-mapping: host-canonical-selected-module-definition: host-generated-repository-definition: slug-command:";
        assert!(rows.iter().all(|(owner, deps)| {
            !forbidden.split(' ').any(|prefix| {
                owner.starts_with(prefix) || deps.iter().any(|dep| dep.starts_with(prefix))
            })
        }));
    }

    fn associate_definition_request_result(
        result: Result<
            HostSelectedExtensionDefinitionLoadRequests,
            HostSelectedExtensionDefinitionLoadRequestsError,
        >,
        epoch: &PathObservationEpoch,
    ) -> DefinitionLoadRequestsResult {
        let SourcePreparationOutcome::Complete(Ok((result, actual))) =
            definition_load_requests_complete(result, epoch.dupe())
        else {
            panic!("definition request association must complete");
        };
        assert_exact_repo_epoch(epoch, &actual);
        result
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

    async fn compute_real_observed_evaluation_inputs(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        include_epoch: bool,
        tracker: Option<Arc<RepoSpecTracker>>,
    ) -> <HostSelectedExtensionEvaluationInputRequestsObservationKey as Key>::Value {
        real_transaction_with_tracker(dice, root, generation, &[], include_epoch, tracker)
            .await
            .compute(
                &HostSelectedExtensionEvaluationInputRequestsObservationKey::new(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                ),
            )
            .await
            .unwrap()
    }

    fn complete_observed_evaluation_inputs(
        value: &<HostSelectedExtensionEvaluationInputRequestsObservationKey as Key>::Value,
    ) -> ObservedHostSelectedExtensionEvaluationInputRequests {
        let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
            panic!("observed evaluation inputs must complete: {value:?}");
        };
        observed.dupe()
    }

    async fn observed_evaluation_state(
        dice: &Arc<Dice>,
        root: &str,
        generation: u64,
        mode: crate::LockfileMode,
    ) -> (
        ObservedHostSelectedExtensionEvaluationInputRequests,
        ObservedHostSelectedExtensionDefinitionLoadRequests,
        crate::module_eval::ObservedRootModuleFiles,
    ) {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let _ = real_transaction(dice, root, generation, &[], true).await;
        let mut updater = dice.updater();
        crate::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            mode,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        let parent_value = transaction
            .compute(
                &HostSelectedExtensionEvaluationInputRequestsObservationKey::new(workspace.dupe()),
            )
            .await
            .unwrap();
        let parent = complete_observed_evaluation_inputs(&parent_value);
        let requests_value = transaction
            .compute(
                &HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(workspace.dupe()),
            )
            .await
            .unwrap();
        let requests = complete_observed_definition_requests(&requests_value);
        let root_value = transaction
            .compute(&RootModuleFilesObservationKey::new(workspace))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(root)) = root_value else {
            panic!("observed root files must complete: {root_value:?}");
        };
        (parent, requests, root)
    }

    fn assert_no_evaluation_input_upper(rows: &[(String, Vec<String>)]) {
        let forbidden = "host-bzl-module: observed-host-bzl-module: host-loaded-module-extension-definitions: host-prepared-module-extension-inputs: host-pure-module-extension-invocations: host-instantiated-module-extension-repositories: host-validated-module-extension-repositories: host-root-repository-mapping: host-canonical-selected-module-definition: host-generated-repository-definition: slug-command:";
        assert!(rows.iter().all(|(owner, deps)| {
            !forbidden.split(' ').any(|prefix| {
                owner.starts_with(prefix) || deps.iter().any(|dep| dep.starts_with(prefix))
            })
        }));
    }

    fn assert_evaluation_input_semantic_failure_prefixes(
        requests: Arc<HostSelectedExtensionDefinitionLoadRequests>,
        root: &crate::module_eval::RootModuleFiles,
        request_epoch: &PathObservationEpoch,
        root_epoch: &PathObservationEpoch,
    ) {
        let root_semantic = finish_evaluation_input_requests_root_child(
            RepoSpecChild::Complete {
                result: Arc::new(Err("root semantic".into())),
                observations: root_epoch.dupe(),
            },
            requests.dupe(),
            request_epoch.dupe(),
        );
        let Err(SourcePreparationOutcome::Complete(Ok((result, observations)))) = root_semantic
        else {
            panic!("root semantic failure must retain the full epoch");
        };
        assert!(matches!(
            result.as_ref(),
            Err(HostSelectedExtensionEvaluationInputRequestsError::AfterRequests {
                error: HostSelectedExtensionEvaluationInputError::RootFiles(message),
                ..
            }) if message == "root semantic"
        ));
        assert_exact_repo_epoch(root_epoch, &observations);

        let mut invalid_root = root.clone();
        invalid_root.extension_usages = Arc::from([]);
        let SourcePreparationOutcome::Complete(Ok((invalid, invalid_observations))) =
            evaluation_input_requests_complete(
                selected_extension_evaluation_input_requests(requests, &invalid_root),
                root_epoch.dupe(),
            )
        else {
            panic!("pure invalid projection must retain the full epoch");
        };
        assert!(matches!(
            invalid.as_ref(),
            Err(
                HostSelectedExtensionEvaluationInputRequestsError::AfterRequests {
                    error: HostSelectedExtensionEvaluationInputError::Invalid(_),
                    ..
                }
            )
        ));
        assert_exact_repo_epoch(root_epoch, &invalid_observations);
    }

    fn assert_evaluation_input_observation_equality(
        result: EvaluationInputRequestsResult,
        request_epoch: PathObservationEpoch,
        root_epoch: PathObservationEpoch,
        need: SourcePreparationNeeds,
    ) {
        let associated = |observations| {
            SourcePreparationOutcome::Complete(Ok(
                ObservedHostSelectedExtensionEvaluationInputRequests {
                    result: result.dupe(),
                    observations,
                },
            ))
        };
        assert!(
            !HostSelectedExtensionEvaluationInputRequestsObservationKey::equality(
                &associated(request_epoch),
                &associated(root_epoch),
            )
        );
        let need_value = SourcePreparationOutcome::Need(need);
        assert!(!HostSelectedExtensionEvaluationInputRequestsObservationKey::validity(&need_value));
        assert!(
            !HostSelectedExtensionEvaluationInputRequestsObservationKey::equality(
                &need_value,
                &need_value
            )
        );
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
    fn pure_canonical_selected_definition_exhausts_and_retains_identity() {
        let dep = route_key("dep", "1");
        let routes = selected_routes(
            &route_graph([
                route_root([("alias", dep.clone())], Some("root_self")),
                route_module("dep", "1", "dep", true),
            ]),
            &HostSelectedRegistryRepoSpecs {
                entries: Arc::from([route_spec(dep)]),
            },
        )
        .unwrap();
        let mut corrupted = routes.entries.to_vec();
        corrupted.push(corrupted[1].clone());
        let consumed = Cell::new(0);
        assert!(matches!(
            find_canonical_route_ordinal(
                &CanonicalRepoName::new("dep+").unwrap(),
                corrupted
                    .iter()
                    .inspect(|_| consumed.set(consumed.get() + 1)),
            ),
            CanonicalRouteMatch::Duplicate {
                first_ordinal: 1,
                conflicting_ordinal: 2,
            }
        ));
        assert_eq!(consumed.get(), corrupted.len());
        assert!(matches!(
            find_canonical_route_ordinal(
                &CanonicalRepoName::new("missing+").unwrap(),
                corrupted.iter(),
            ),
            CanonicalRouteMatch::Missing
        ));

        let outcome = |routes: HostSelectedModuleRoutes, ordinal| {
            SourcePreparationOutcome::Complete(Arc::new(Ok(
                HostCanonicalSelectedModuleDefinition {
                    routes: Arc::new(Ok(routes)),
                    ordinal,
                },
            )))
        };
        let a = outcome(routes.clone(), 0);
        let restored = outcome(routes.clone(), 0);
        assert!(HostCanonicalSelectedModuleDefinitionKey::equality(
            &a, &restored
        ));

        let mutate = |route: HostSelectedModuleRoute| {
            let mut entries = routes.entries.to_vec();
            entries[0] = route;
            HostSelectedModuleRoutes {
                entries: entries.into(),
            }
        };
        let mut route = routes.entries[0].clone();
        route.canonical_repo = CanonicalRepoName::new("changed+").unwrap();
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &a,
            &outcome(mutate(route), 0),
        ));

        let mut route = routes.entries[0].clone();
        route.mapping.context_repo = CanonicalRepoName::new("context+").unwrap();
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &a,
            &outcome(mutate(route), 0),
        ));

        let mut route = routes.entries[0].clone();
        let mut mapping = (*route.mapping.entries).clone();
        mapping.insert(
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new("other+").unwrap(),
        );
        route.mapping.entries = Arc::new(mapping);
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &a,
            &outcome(mutate(route), 0),
        ));

        let mut route = routes.entries[0].clone();
        let map_before = route.mapping.entries.clone();
        route.mapping.order = route.mapping.order.iter().rev().cloned().collect();
        assert_eq!(route.mapping.entries, map_before);
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &a,
            &outcome(mutate(route), 0),
        ));

        let mut reordered = routes.entries.to_vec();
        reordered.swap(0, 1);
        let reordered = outcome(
            HostSelectedModuleRoutes {
                entries: reordered.into(),
            },
            1,
        );
        let SourcePreparationOutcome::Complete(reordered_value) = &reordered else {
            unreachable!()
        };
        assert_eq!(
            reordered_value
                .as_ref()
                .as_ref()
                .unwrap()
                .view()
                .canonical_repo()
                .as_str(),
            ""
        );
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &a, &reordered
        ));

        let mut changed_spec = routes.clone();
        let mut rows = changed_spec.entries.to_vec();
        rows[1]
            .registry_repo_spec
            .as_mut()
            .unwrap()
            .repo_spec
            .rule_id
            .rule_name = "changed_rule".into();
        changed_spec.entries = rows.into();
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &a,
            &outcome(changed_spec, 0),
        ));
        let failed = Arc::new(Err(HostSelectedModuleRoutesError::Invalid {
            module: HostGraphModuleKey::Root,
            message: "failed".into(),
        }));
        let error = |name| {
            SourcePreparationOutcome::Complete(Arc::new(Err(
                HostCanonicalSelectedModuleDefinitionError {
                    inner: PrivateCanonicalSelectedModuleDefinitionError::Routes(
                        failed.clone(),
                        CanonicalRepoName::new(name).unwrap(),
                    ),
                },
            )))
        };
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &error("a+"),
            &error("b+")
        ));
        assert!(HostCanonicalSelectedModuleDefinitionKey::equality(
            &error("a+"),
            &error("a+")
        ));
        let predecessor = Arc::new(Ok(routes.clone()));
        let disposition =
            |inner| external_disposition(&HostCanonicalSelectedModuleDefinitionError { inner });
        assert_eq!(
            disposition(PrivateCanonicalSelectedModuleDefinitionError::Missing {
                predecessor: predecessor.clone(),
                canonical_repo: CanonicalRepoName::new("missing+").unwrap(),
            }),
            crate::HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing
        );
        for terminal in [
            PrivateCanonicalSelectedModuleDefinitionError::Routes(
                failed,
                CanonicalRepoName::new("route+").unwrap(),
            ),
            PrivateCanonicalSelectedModuleDefinitionError::RoutesCompute(
                "compute".into(),
                CanonicalRepoName::new("compute+").unwrap(),
            ),
            PrivateCanonicalSelectedModuleDefinitionError::Duplicate {
                predecessor: predecessor.clone(),
                canonical_repo: CanonicalRepoName::new("duplicate+").unwrap(),
                first_ordinal: 0,
                conflicting_ordinal: 1,
            },
            PrivateCanonicalSelectedModuleDefinitionError::BuiltinDeferred {
                predecessor,
                ordinal: 1,
                canonical_repo: CanonicalRepoName::new("bazel_tools").unwrap(),
            },
        ] {
            assert_eq!(
                disposition(terminal),
                crate::HostCanonicalSelectedModuleDefinitionErrorDisposition::Terminal
            );
        }
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
        let proof_demand = observation("/pure-request", PathObservationOperation::Lstat);
        let proof_result = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let proof_epoch =
            PathObservationEpoch::from_shared([(proof_demand, proof_result)]).unwrap();

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
            associate_definition_request_result(
                selected_extension_definition_load_requests(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                    Arc::new(invalid),
                ),
                &proof_epoch,
            )
            .as_ref(),
            Err(HostSelectedExtensionDefinitionLoadRequestsError(
                HostSelectedExtensionDefinitionLoadRequestsErrorInner::Invalid {
                    message,
                    ..
                },
            )) if message.contains("mismatched namespace ownership")
        ));
        let mut duplicate = namespaced.predecessor.as_ref().clone();
        let mut duplicate_usages = duplicate.usages.to_vec();
        duplicate_usages[1].unique_name = duplicate_usages[0].unique_name.clone();
        duplicate.usages = duplicate_usages.into();
        assert!(matches!(
            associate_definition_request_result(
                selected_extension_definition_load_requests(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                    Arc::new(duplicate),
                ),
                &proof_epoch,
            )
            .as_ref(),
            Err(HostSelectedExtensionDefinitionLoadRequestsError(
                HostSelectedExtensionDefinitionLoadRequestsErrorInner::Invalid {
                    message,
                    ..
                },
            )) if message.contains("duplicate ownership")
        ));
        let mut missing = namespaced.predecessor.as_ref().clone();
        missing.base_mappings = Arc::from([]);
        assert!(matches!(
            associate_definition_request_result(
                selected_extension_definition_load_requests(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                    Arc::new(missing),
                ),
                &proof_epoch,
            )
            .as_ref(),
            Err(HostSelectedExtensionDefinitionLoadRequestsError(
                HostSelectedExtensionDefinitionLoadRequestsErrorInner::InvalidContext(
                    message
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
            associate_definition_request_result(
                selected_extension_definition_load_requests(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                    Arc::new(unsupported),
                ),
                &proof_epoch,
            )
            .as_ref(),
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
                order: Arc::from([]),
                entries: Arc::new(SmallMap::new()),
            },
            mapping: HostSelectedRepositoryMapping {
                context_repo: CanonicalRepoName::new("root").unwrap(),
                order: Arc::from([]),
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
        let observed = compute_real_observed(&dice, &root, 1, &[], true, None).await;
        let observed = complete_observed_repo_specs(&observed);
        assert!(
            observed
                .result()
                .as_ref()
                .as_ref()
                .unwrap()
                .entries
                .is_empty()
                && !observed.observations().observations().is_empty()
        );
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
    async fn real_canonical_selected_definition_classifies_and_reuses() {
        const MODULE_URL: &str = "https://registry.invalid/modules/dep/1/MODULE.bazel";
        const MODULE_2_URL: &str = "https://registry.invalid/modules/dep/2/MODULE.bazel";
        const SOURCE_URL: &str = "https://registry.invalid/modules/dep/1/source.json";
        const SOURCE_2_URL: &str = "https://registry.invalid/modules/dep/2/source.json";
        const SOURCE_A: &[u8] = br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#;
        const SOURCE_B: &[u8] = br#"{"url":"https://origin.test/b.tgz","integrity":"sha256-b"}"#;
        let io = Arc::new(TrackingRegistryIo::new([
            (MODULE_URL, b"module(name='dep', version='1')\n" as &[u8]),
            (MODULE_2_URL, b"module(name='dep', version='2')\n" as &[u8]),
            (SOURCE_URL, SOURCE_A),
            (SOURCE_2_URL, SOURCE_A),
            (
                "https://registry.invalid/bazel_registry.json",
                b"{}" as &[u8],
            ),
        ]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.clone());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let mut root = "module(name='root', repo_name='root_self')\n\
                        bazel_dep(name='dep', version='1', repo_name='dep_alias')\n"
            .to_owned();
        for name in LOCAL_MODULES {
            root.push_str(&format!(
                "local_path_override(module_name='{name}', path='{name}')\n\
                 bazel_dep(name='{name}', version='1'{} )\n",
                if *name == "local" {
                    ", repo_name='local_alias'"
                } else {
                    ""
                }
            ));
        }

        let routes = compute_real_routes(&dice, &root, 1, &[], true).await;
        assert!(matches!(routes, SourcePreparationOutcome::Complete(_)));
        let calls_after_routes = io.calls();

        let root_value = compute_real_selected_definition(&dice, &root, 1, "", true).await;
        let SourcePreparationOutcome::Complete(root_value) = &root_value else {
            panic!("root definition must complete")
        };
        let root_value = root_value.as_ref().as_ref().unwrap();
        assert_external_error::<crate::HostCanonicalSelectedModuleDefinitionError>();
        let root_snapshot = external_style_snapshot(root_value);
        assert_eq!(root_snapshot.kind, HostCanonicalSelectedModuleKind::Root);
        assert_eq!(root_snapshot.identity, None);
        assert_eq!(root_snapshot.canonical_repo, "");
        assert_eq!(root_snapshot.mapping_context, "");
        let mut expected_root_mapping = vec![
            ("".to_owned(), "".to_owned()),
            ("root_self".to_owned(), "".to_owned()),
            ("dep_alias".to_owned(), "dep+".to_owned()),
        ];
        expected_root_mapping.extend(LOCAL_MODULES.iter().map(|name| {
            (
                if *name == "local" {
                    "local_alias"
                } else {
                    name
                }
                .to_string(),
                if matches!(*name, "platforms" | "bazel_tools") {
                    name.to_string()
                } else {
                    format!("{name}+")
                },
            )
        }));
        expected_root_mapping.push(("bazel_tools".into(), "bazel_tools".into()));
        assert_eq!(root_snapshot.mapping, expected_root_mapping);
        assert!(root_snapshot.repo_rule.is_none());
        assert_eq!(root_snapshot.local_path_policy, None);
        assert_eq!(io.calls(), calls_after_routes);

        let registry = compute_real_selected_definition(&dice, &root, 1, "dep+", true).await;
        let SourcePreparationOutcome::Complete(registry) = &registry else {
            panic!("registry definition must complete")
        };
        let registry = registry.as_ref().as_ref().unwrap();
        let registry_view = registry.view();
        assert_eq!(
            registry_view.kind(),
            HostCanonicalSelectedModuleKind::SelectedRegistry
        );
        assert!(matches!(
            registry_view.identity(),
            HostCanonicalSelectedModuleIdentity::Module { name, .. } if name == "dep"
        ));
        assert_eq!(
            registry_view.repo_spec().unwrap().rule_id.rule_name,
            "http_archive"
        );
        let registry_snapshot = external_style_snapshot(registry);
        assert_eq!(registry_snapshot.identity, Some(("dep".into(), "1".into())));
        assert_eq!(registry_snapshot.canonical_repo, "dep+");
        assert_eq!(registry_snapshot.mapping_context, "dep+");
        let (bzl_file, rule_name, attributes) = registry_snapshot.repo_rule.unwrap();
        assert!(bzl_file.ends_with("//tools/build_defs/repo:http.bzl"));
        assert_eq!(rule_name, "http_archive");
        assert!(!attributes.is_empty());
        assert_eq!(
            registry_snapshot.local_path_policy,
            Some(crate::HostRepositoryLocalPathPolicy::LocalUnsupported)
        );

        let local = compute_real_selected_definition(&dice, &root, 1, "local+", true).await;
        let SourcePreparationOutcome::Complete(local) = &local else {
            panic!("nonregistry definition must complete")
        };
        let local = local.as_ref().as_ref().unwrap();
        let local_snapshot = external_style_snapshot(local);
        assert_eq!(
            local_snapshot.kind,
            HostCanonicalSelectedModuleKind::SelectedNonregistry
        );
        assert_eq!(local_snapshot.identity, Some(("local".into(), "".into())));
        assert_eq!(local_snapshot.canonical_repo, "local+");
        assert_eq!(local_snapshot.repo_rule.unwrap().1, "local_repository");
        assert_eq!(
            local_snapshot.local_path_policy,
            Some(crate::HostRepositoryLocalPathPolicy::WorkspaceRelative)
        );

        let builtin = compute_real_selected_definition(&dice, &root, 1, "bazel_tools", true).await;
        assert!(matches!(
            builtin,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalSelectedModuleDefinitionError {
                        inner: PrivateCanonicalSelectedModuleDefinitionError::BuiltinDeferred {
                            ordinal,
                            canonical_repo,
                            predecessor,
                        }
                    }) if *ordinal > 0
                        && canonical_repo.as_str() == "bazel_tools"
                        && predecessor.as_ref().as_ref().unwrap().entries[*ordinal]
                            .canonical_repo.as_str() == "bazel_tools"
                )
        ));

        let missing = compute_real_selected_definition(&dice, &root, 1, "missing+", true).await;
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalSelectedModuleDefinitionError {
                        inner: PrivateCanonicalSelectedModuleDefinitionError::Missing {
                            canonical_repo,
                            ..
                        }
                    }) if canonical_repo.as_str() == "missing+"
                )
        ));

        let tracker = Arc::new(SelectedDefinitionTracker::default());
        let data = UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let mut transaction = dice.updater_with_data(data).commit().await;
        let warm = transaction
            .compute(&HostCanonicalSelectedModuleDefinitionKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::new("dep+").unwrap(),
            ))
            .await
            .unwrap();
        assert!(HostCanonicalSelectedModuleDefinitionKey::equality(
            &SourcePreparationOutcome::Complete(Arc::new(Ok(registry.clone()))),
            &warm,
        ));
        assert_eq!(tracker.take(), [(ActivationKind::Reused, true)]);
        assert!(tracker.forbidden().is_empty());
        assert_eq!(io.calls(), calls_after_routes);
        drop(transaction);

        io.replace(SOURCE_URL, SOURCE_B);
        let source_b = compute_real_selected_definition(&dice, &root, 2, "dep+", true).await;
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &SourcePreparationOutcome::Complete(Arc::new(Ok(registry.clone()))),
            &source_b,
        ));
        io.replace(SOURCE_URL, SOURCE_A);
        let restored = compute_real_selected_definition(&dice, &root, 3, "dep+", true).await;
        assert!(HostCanonicalSelectedModuleDefinitionKey::equality(
            &SourcePreparationOutcome::Complete(Arc::new(Ok(registry.clone()))),
            &restored,
        ));

        let mapping_b = compute_real_selected_definition(
            &dice,
            &root.replace("repo_name='dep_alias'", "repo_name='other_alias'"),
            4,
            "dep+",
            true,
        )
        .await;
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &restored, &mapping_b
        ));
        let mapping_a = compute_real_selected_definition(&dice, &root, 5, "dep+", true).await;
        assert!(HostCanonicalSelectedModuleDefinitionKey::equality(
            &restored, &mapping_a
        ));

        let version_b = compute_real_selected_definition(
            &dice,
            &root.replace("name='dep', version='1'", "name='dep', version='2'"),
            6,
            "dep+",
            true,
        )
        .await;
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &mapping_a, &version_b
        ));
        let version_a = compute_real_selected_definition(&dice, &root, 7, "dep+", true).await;
        assert!(HostCanonicalSelectedModuleDefinitionKey::equality(
            &mapping_a, &version_a
        ));

        let local_a = compute_real_selected_definition(&dice, &root, 7, "local+", true).await;
        let local_b = compute_real_selected_definition(
            &dice,
            &root.replace("path='local'", "path='local/.'"),
            8,
            "local+",
            true,
        )
        .await;
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &local_a, &local_b
        ));
        let local_restored =
            compute_real_selected_definition(&dice, &root, 9, "local+", true).await;
        assert!(HostCanonicalSelectedModuleDefinitionKey::equality(
            &local_a,
            &local_restored,
        ));

        let need = compute_real_selected_definition(&dice, &root, 10, "dep+", false).await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostCanonicalSelectedModuleDefinitionKey::validity(&need));
        assert!(!HostCanonicalSelectedModuleDefinitionKey::equality(
            &need, &need
        ));

        let graph_error = compute_real_selected_definition(
            &dice,
            "module(name='root')\nbazel_dep(name='missing', version='1')\n",
            7,
            "",
            true,
        )
        .await;
        assert!(matches!(
            graph_error,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalSelectedModuleDefinitionError {
                        inner: PrivateCanonicalSelectedModuleDefinitionError::Routes(
                            predecessor,
                            _
                        )
                    }) if predecessor.as_ref().is_err()
                )
        ));

        let command_root = format!("{root}# command_override_bazel_tools\n");
        let command =
            compute_real_selected_definition(&dice, &command_root, 11, "bazel_tools", true).await;
        let SourcePreparationOutcome::Complete(command) = &command else {
            panic!("command override definition must complete")
        };
        let command = external_style_snapshot(command.as_ref().as_ref().unwrap());
        assert_eq!(
            command.kind,
            HostCanonicalSelectedModuleKind::SelectedNonregistry
        );
        assert_eq!(
            command.local_path_policy,
            Some(crate::HostRepositoryLocalPathPolicy::CommandAbsolute)
        );
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
    async fn root_mapping_publication_is_exact_borrowed_and_structural() {
        let io = Arc::new(TrackingRegistryIo::new([]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.clone());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let source = |first: &str, reversed: bool, extension_order: bool, operation_order: bool| {
            let imports = if reversed {
                format!("second_alias='plain_b', first_alias='{first}'")
            } else {
                format!("first_alias='{first}', second_alias='plain_b'")
            };
            let extensions = if extension_order {
                format!(
                    "three=use_extension(\"//:three.bzl\", \"third\")\n\
                     use_repo(three, visible_alias=\"visible\")\n\
                     p=use_extension(\"//:ext.bzl\", \"extension\")\n\
                     use_repo(p, {imports}, overridden_alias=\"overridden\")"
                )
            } else {
                format!(
                    "p=use_extension(\"//:ext.bzl\", \"extension\")\n\
                     use_repo(p, {imports}, overridden_alias=\"overridden\")\n\
                     three=use_extension(\"//:three.bzl\", \"third\")\n\
                     use_repo(three, visible_alias=\"visible\")"
                )
            };
            let operations = if operation_order {
                "inject_repo(three, injected=\"replacement\")\noverride_repo(p, overridden=\"replacement\")"
            } else {
                "override_repo(p, overridden=\"replacement\")\ninject_repo(three, injected=\"replacement\")"
            };
            format!(
                r#"
module(name="bazel_tools", repo_name="root_self")
local_path_override(module_name='local', path='local')
bazel_dep(name='local', version='1', repo_name='local_alias')
{extensions}
{operations}
repo=use_repo_rule("//:repo.bzl", "simple_repo")
repo(name="replacement")
"#
            )
        };

        let baseline = source("plain_a", false, false, false);
        let predecessor = compute_real_extensions(&dice, &baseline, 80, true).await;
        assert!(matches!(predecessor, SourcePreparationOutcome::Complete(_)));
        let tracker = Arc::new(SelectedDefinitionTracker::default());
        let data = UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let key = crate::HostRootRepositoryMappingKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let a = dice
            .updater_with_data(data)
            .commit()
            .await
            .compute(&key)
            .await
            .unwrap();
        assert_eq!(
            tracker.take_root_mapping(),
            [(ActivationKind::Evaluated, true)]
        );
        let SourcePreparationOutcome::Complete(a_value) = &a else {
            panic!("root mapping must complete")
        };
        let a_value = a_value.as_ref().as_ref().unwrap();
        let snapshot = external_root_mapping_snapshot(a_value);
        assert_eq!(
            (
                snapshot.canonical_repo.as_str(),
                snapshot.mapping_context.as_str()
            ),
            ("", "")
        );
        assert_eq!(
            snapshot.mapping,
            [
                ("", ""),
                ("root_self", ""),
                ("local_alias", "local+"),
                ("first_alias", "+extension+plain_a"),
                ("second_alias", "+extension+plain_b"),
                ("overridden_alias", "+simple_repo+replacement"),
                ("visible_alias", "+third+visible"),
                ("replacement", "+simple_repo+replacement"),
            ]
            .map(|(name, target)| (name.to_owned(), target.to_owned()))
        );
        let view = a_value.view().unwrap();
        let (name, target) = view
            .mapping()
            .find(|(name, _)| name.as_str() == "first_alias")
            .unwrap();
        let retained = a_value.predecessor.as_ref().as_ref().unwrap();
        assert!(
            retained
                .overrides
                .iter()
                .any(|row| row.generated_name == "injected" && !row.must_exist)
        );
        assert!(snapshot.mapping.iter().all(|(name, _)| name != "injected"));
        let retained_mapping = &retained.mappings[a_value.root_ordinal];
        assert!(std::ptr::eq(
            target,
            retained_mapping.entries.get(name).unwrap()
        ));
        assert_external_error::<crate::HostRootRepositoryMappingError>();

        let data = UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let warm = dice
            .updater_with_data(data)
            .commit()
            .await
            .compute(&key)
            .await
            .unwrap();
        assert!(HostRootRepositoryMappingKey::equality(&a, &warm));
        assert_eq!(
            tracker.take_root_mapping(),
            [(ActivationKind::Reused, true)]
        );
        assert!(tracker.forbidden().is_empty());

        let target_b =
            compute_real_root_mapping(&dice, &source("plain_c", false, false, false), 81, true)
                .await;
        assert!(!HostRootRepositoryMappingKey::equality(&a, &target_b));
        let target_a = compute_real_root_mapping(&dice, &baseline, 82, true).await;
        assert!(HostRootRepositoryMappingKey::equality(&a, &target_a));
        let order_b =
            compute_real_root_mapping(&dice, &source("plain_a", true, false, false), 83, true)
                .await;
        let SourcePreparationOutcome::Complete(order_b_value) = &order_b else {
            panic!("reordered mapping must complete")
        };
        assert_eq!(
            external_root_mapping_snapshot(order_b_value.as_ref().as_ref().unwrap()).mapping[3..5]
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["second_alias", "first_alias"]
        );
        assert!(!HostRootRepositoryMappingKey::equality(&a, &order_b));
        let order_a = compute_real_root_mapping(&dice, &baseline, 84, true).await;
        assert!(HostRootRepositoryMappingKey::equality(&a, &order_a));

        for (generation, changed) in [
            (85, source("plain_a", false, true, false)),
            (87, source("plain_a", false, false, true)),
            (
                89,
                baseline.replace("injected=\"replacement\"", "other=\"replacement\""),
            ),
            (
                91,
                baseline.replace("repo_name=\"root_self\"", "repo_name=\"other_root\""),
            ),
        ] {
            let b = compute_real_root_mapping(&dice, &changed, generation, true).await;
            assert!(!HostRootRepositoryMappingKey::equality(&a, &b));
            let restored = compute_real_root_mapping(&dice, &baseline, generation + 1, true).await;
            assert!(HostRootRepositoryMappingKey::equality(&a, &restored));
        }

        let mut empty_source = "module(name='root', repo_name='root_self')\n".to_owned();
        for name in LOCAL_MODULES {
            empty_source.push_str(&format!(
                "local_path_override(module_name='{name}', path='{name}')\n\
                 bazel_dep(name='{name}', version='1')\n"
            ));
        }
        let empty = compute_real_root_mapping(&dice, &empty_source, 93, true).await;
        let SourcePreparationOutcome::Complete(empty) = empty else {
            panic!("empty-extension root mapping must complete")
        };
        let empty = external_root_mapping_snapshot(empty.as_ref().as_ref().unwrap());
        assert_eq!(
            &empty.mapping[..2],
            &[("".into(), "".into()), ("root_self".into(), "".into())]
        );
        assert!(
            empty
                .mapping
                .iter()
                .any(|(name, target)| name == "bazel_tools" && target == "bazel_tools")
        );
        assert!(
            empty
                .mapping
                .iter()
                .all(|(_, target)| !target.contains("+extension+"))
        );
        assert!(io.calls().is_empty());
    }

    #[tokio::test]
    async fn root_mapping_rejects_corruption_and_preserves_predecessor_terminals() {
        let io = Arc::new(TrackingRegistryIo::new([]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io);
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let outcome = compute_real_extensions(
            &dice,
            "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1')\n",
            90,
            true,
        )
        .await;
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("mapping predecessor must complete")
        };
        let original = value.as_ref().as_ref().unwrap();
        let root = root_mapping_ordinal(original).unwrap();

        let mut missing = original.clone();
        missing.routes = Arc::new(HostSelectedModuleRoutes {
            entries: missing
                .routes
                .entries
                .iter()
                .filter(|route| !matches!(route.entry.key, HostGraphModuleKey::Root))
                .cloned()
                .collect(),
        });
        assert_eq!(
            root_mapping_ordinal(&missing),
            Err(RootMappingInvalid::Missing)
        );

        let mut duplicate = original.clone();
        let mut duplicate_routes = duplicate.routes.entries.to_vec();
        duplicate_routes.push(duplicate_routes[root].clone());
        duplicate.routes = Arc::new(HostSelectedModuleRoutes {
            entries: duplicate_routes.into(),
        });
        assert_eq!(
            root_mapping_ordinal(&duplicate),
            Err(RootMappingInvalid::Duplicate {
                first: root,
                conflicting: duplicate.routes.entries.len() - 1,
            })
        );

        let mut context = original.clone();
        let mut mappings = context.mappings.to_vec();
        mappings[root].context_repo = CanonicalRepoName::new("wrong+").unwrap();
        context.mappings = mappings.into();
        assert_eq!(
            root_mapping_ordinal(&context),
            Err(RootMappingInvalid::Context { ordinal: root })
        );

        let need_io = Arc::new(TrackingRegistryIo::new([(
            "https://registry.invalid/modules/dep/1/MODULE.bazel",
            b"module(name='dep', version='1')\n" as &[u8],
        )]));
        let mut need_builder = Dice::builder();
        crate::install_registry_io(&mut need_builder, need_io);
        let need_dice = Arc::new(need_builder.build(DetectCycles::Enabled));
        let need = compute_real_root_mapping(
            &need_dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n",
            91,
            false,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostRootRepositoryMappingKey::validity(&need));
        assert!(!HostRootRepositoryMappingKey::equality(&need, &need));
        let error = compute_real_root_mapping(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='missing', version='1')\n",
            92,
            true,
        )
        .await;
        assert!(matches!(
            error,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    &value.as_ref().as_ref().unwrap_err().inner,
                    PrivateRootRepositoryMappingError::Predecessor(predecessor)
                        if predecessor.as_ref().is_err()
                )
        ));
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
        const ROOT: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n";
        let io = Arc::new(TrackingRegistryIo::new([(
            "https://registry.invalid/modules/dep/1/MODULE.bazel",
            b"module(name='dep', version='1')\n" as &[u8],
        )]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io);
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let need = compute_real(&dice, ROOT, 1, &[], false).await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostSelectedRegistryRepoSpecsKey::validity(&need));
        assert!(!HostSelectedRegistryRepoSpecsKey::equality(&need, &need));

        let tracker = Arc::new(RepoSpecTracker::default());
        let observed =
            compute_real_observed(&dice, ROOT, 1, &[], false, Some(tracker.dupe())).await;
        assert!(matches!(observed, SourcePreparationOutcome::Need(_)));
        assert!(!HostSelectedRegistryRepoSpecsObservationKey::validity(
            &observed
        ));
        assert!(!HostSelectedRegistryRepoSpecsObservationKey::equality(
            &observed, &observed
        ));
        let (activations, rows) = tracker.take();
        let owner = HostSelectedRegistryRepoSpecsObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        )
        .to_string();
        let parent = activations
            .iter()
            .find(|activation| activation.key == owner)
            .unwrap();
        assert!(parent.batch.is_none());
        assert_no_repo_spec_upper(&rows);

        let route_need = compute_real_routes(&dice, ROOT, 2, &[], false).await;
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

    #[test]
    fn observed_routes_identity_is_workspace_scoped() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostSelectedModuleRoutesObservationKey::new(workspace.dupe());
        let other = HostSelectedModuleRoutesObservationKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
        );
        assert_eq!(
            key.to_string(),
            "observed-host-selected-module-routes:\"/selected-repo-spec-test\""
        );
        assert_ne!(key, other);
        let hash = |value: &HostSelectedModuleRoutesObservationKey| {
            let mut state = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(value, &mut state);
            std::hash::Hasher::finish(&state)
        };
        assert_ne!(hash(&key), hash(&other));
    }

    #[test]
    fn observed_routes_production_terminals_preserve_prefixes() {
        let demand = observation("/graph", PathObservationOperation::Lstat);
        let first = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let graph_epoch =
            PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let repo_demand = observation("/repo-spec", PathObservationOperation::FileBytes);
        let repo_result = Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Present(Arc::from(&b"spec"[..])),
        ));
        let repo_epoch =
            PathObservationEpoch::from_shared([(repo_demand.dupe(), repo_result.dupe())]).unwrap();
        let need = || {
            SourcePreparationNeeds::path(slug_workspace_v2::NeedPathObservations::singleton(
                demand.dupe(),
            ))
        };
        let mismatch = || {
            ObservedPathFrontierError::from(PathObservationEpochError::OperationMismatch {
                demand: demand.dupe(),
                result_operation: PathObservationOperation::FileBytes,
            })
        };

        let mut prefix = PathObservationEpoch::empty();
        assert!(matches!(
            finish_route_graph_child(RepoSpecChild::Compute("graph-compute".into()), &mut prefix),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(
                    HostSelectedModuleRoutesError::GraphCompute(message)
                ) if message == "graph-compute") && observations.observations().is_empty()
        ));
        assert!(matches!(
            finish_route_graph_child(RepoSpecChild::Need(need()), &mut prefix),
            Err(SourcePreparationOutcome::Need(_))
        ));
        assert!(matches!(
            finish_route_graph_child(
                RepoSpecChild::Outer(HostSelectedModuleGraphObservationError::Merge(mismatch())),
                &mut prefix,
            ),
            Err(SourcePreparationOutcome::Complete(Err(
                HostSelectedModuleRoutesObservationError::Graph(_)
            )))
        ));
        let graph_result = Arc::new(Err(HostSelectedModuleGraphError::Input {
            owner: "graph".into(),
            message: "semantic".into(),
        }));
        assert!(Arc::ptr_eq(
            &finish_route_graph_child(
                RepoSpecChild::Complete {
                    result: graph_result.dupe(),
                    observations: graph_epoch.dupe(),
                },
                &mut prefix,
            )
            .unwrap(),
            &graph_result
        ));
        assert_exact_repo_epoch(&graph_epoch, &prefix);
        assert!(matches!(
            finish_route_graph_semantic(graph_result.as_ref(), graph_epoch.dupe()),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(
                    HostSelectedModuleRoutesError::Graph(_)
                )) && observations == graph_epoch
        ));

        assert!(matches!(
            finish_route_repo_specs_child(
                RepoSpecChild::Compute("repo-compute".into()),
                &mut prefix,
            ),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(
                    HostSelectedModuleRoutesError::RepoSpecsCompute(message)
                ) if message == "repo-compute")
                    && observations == graph_epoch
        ));
        assert!(matches!(
            finish_route_repo_specs_child(RepoSpecChild::Need(need()), &mut prefix),
            Err(SourcePreparationOutcome::Need(_))
        ));
        assert!(matches!(
            finish_route_repo_specs_child(
                RepoSpecChild::Outer(HostSelectedRegistryRepoSpecsObservationError::Merge {
                    module: None,
                    stage: RepoSpecObservationStage::Graph,
                    error: mismatch(),
                }),
                &mut prefix,
            ),
            Err(SourcePreparationOutcome::Complete(Err(
                HostSelectedModuleRoutesObservationError::RepoSpecs(_)
            )))
        ));
        let repo_specs_result = Arc::new(Err(fail(&module(), "semantic")));
        assert!(Arc::ptr_eq(
            &finish_route_repo_specs_child(
                RepoSpecChild::Complete {
                    result: repo_specs_result.dupe(),
                    observations: repo_epoch.dupe(),
                },
                &mut prefix,
            )
            .unwrap(),
            &repo_specs_result
        ));
        let merged = PathObservationEpoch::from_shared(
            graph_epoch
                .observations()
                .iter()
                .chain(repo_epoch.observations())
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap();
        assert_exact_repo_epoch(&merged, &prefix);
        assert!(matches!(
            finish_route_repo_specs_semantic(repo_specs_result.as_ref(), merged.dupe()),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(HostSelectedModuleRoutesError::RepoSpecs(_)))
                    && observations == merged
        ));
        assert!(matches!(
            routes_complete(Err(route_invalid(&module(), "mapping")), merged.dupe()),
            SourcePreparationOutcome::Complete(Ok((result, observations)))
                if matches!(result.as_ref(), Err(HostSelectedModuleRoutesError::Invalid { .. }))
                    && observations == merged
        ));
        let projected_arc = Arc::new(Err(route_invalid(&module(), "legacy")));
        let projected = project_legacy_routes(SourcePreparationOutcome::Complete(Ok((
            projected_arc.dupe(),
            merged.dupe(),
        ))));
        assert!(matches!(
            projected,
            SourcePreparationOutcome::Complete(result)
                if Arc::ptr_eq(&result, &projected_arc)
        ));
        let outer = SourcePreparationOutcome::Complete(Err(
            HostSelectedModuleRoutesObservationError::Graph(
                HostSelectedModuleGraphObservationError::Merge(mismatch()),
            ),
        ));
        assert!(HostSelectedModuleRoutesObservationKey::validity(&outer));
        assert!(HostSelectedModuleRoutesObservationKey::equality(
            &outer, &outer
        ));

        let equal =
            PathObservationEpoch::from_shared([(demand.dupe(), Arc::new(first.as_ref().clone()))])
                .unwrap();
        let mut earliest = graph_epoch.dupe();
        merge_route_observations(&mut earliest, &equal, RouteObservationStage::RepoSpecs).unwrap();
        assert!(Arc::ptr_eq(earliest.get(&demand).unwrap(), &first));
        let conflict = PathObservationEpoch::from_shared([(
            demand.dupe(),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 2, 3, 4, 0o644),
            ))),
        )])
        .unwrap();
        assert!(matches!(
            merge_route_observations(&mut earliest, &conflict, RouteObservationStage::RepoSpecs),
            Err(HostSelectedModuleRoutesObservationError::Merge {
                stage: RouteObservationStage::RepoSpecs,
                ..
            })
        ));
        let mut graph_conflict = graph_epoch.dupe();
        assert!(matches!(
            merge_route_observations(&mut graph_conflict, &conflict, RouteObservationStage::Graph),
            Err(HostSelectedModuleRoutesObservationError::Merge {
                stage: RouteObservationStage::Graph,
                ..
            })
        ));
        assert!(matches!(
            route_merge_error(
                RouteObservationStage::RepoSpecs,
                PathObservationEpochError::OperationMismatch {
                    demand: demand.dupe(),
                    result_operation: PathObservationOperation::FileBytes,
                }
            ),
            HostSelectedModuleRoutesObservationError::Merge {
                stage: RouteObservationStage::RepoSpecs,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn observed_routes_match_legacy_families_epochs_events_and_warm_reuse() {
        const ROOT: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n";
        const MODULE_URL: &str = "https://registry.invalid/modules/dep/1/MODULE.bazel";
        const SOURCE_URL: &str = "https://registry.invalid/modules/dep/1/source.json";
        let files = [
            (MODULE_URL, b"module(name='dep', version='1')\n".as_slice()),
            (
                SOURCE_URL,
                br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#.as_slice(),
            ),
        ];
        let tracker = Arc::new(RepoSpecTracker::default());
        let io = Arc::new(TrackingRegistryIo::new(files));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io);
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostSelectedModuleRoutesObservationKey::new(workspace.dupe());
        let mut transaction =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        let cold = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_routes(&cold);
        assert!(observed.result().as_ref().is_ok());
        let graph = transaction
            .compute(&HostSelectedModuleGraphObservationKey::new(
                workspace.dupe(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(graph)) = graph else {
            panic!("graph must complete")
        };
        let repo_specs = transaction
            .compute(&HostSelectedRegistryRepoSpecsObservationKey::new(
                workspace.dupe(),
            ))
            .await
            .unwrap();
        let repo_specs = complete_observed_repo_specs(&repo_specs);
        let expected = PathObservationEpoch::from_shared(
            graph
                .observations()
                .observations()
                .iter()
                .chain(repo_specs.observations().observations())
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap();
        assert_exact_repo_epoch(&expected, observed.observations());
        let (observed_activations, observed_rows) = tracker.take();
        assert_eq!(
            repo_spec_row(&observed_rows, &key.to_string()),
            [
                HostSelectedModuleGraphObservationKey::new(workspace.dupe()).to_string(),
                HostSelectedRegistryRepoSpecsObservationKey::new(workspace.dupe()).to_string(),
            ]
        );
        assert!(
            observed_activations
                .iter()
                .any(|entry| { entry.key == key.to_string() && entry.batch.is_none() })
        );
        let observed_events = observed_activations
            .iter()
            .filter_map(|entry| {
                entry.batch.dupe().map(|batch| {
                    (
                        entry
                            .key
                            .strip_prefix("observed-")
                            .unwrap_or(&entry.key)
                            .to_owned(),
                        batch,
                    )
                })
            })
            .collect::<Vec<_>>();

        let warm = transaction.compute(&key).await.unwrap();
        assert!(HostSelectedModuleRoutesObservationKey::equality(
            &cold, &warm
        ));
        assert!(tracker.take().0.iter().all(|entry| entry.batch.is_none()));

        let legacy_tracker = Arc::new(RepoSpecTracker::default());
        let legacy_io = Arc::new(TrackingRegistryIo::new(files));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, legacy_io);
        let legacy_dice = Arc::new(builder.build(DetectCycles::Enabled));
        let legacy_key = HostSelectedModuleRoutesKey::new(workspace.dupe());
        let mut legacy_transaction = real_transaction_with_tracker(
            &legacy_dice,
            ROOT,
            1,
            &[],
            true,
            Some(legacy_tracker.dupe()),
        )
        .await;
        let legacy = legacy_transaction.compute(&legacy_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy_result) = legacy else {
            panic!("legacy routes must complete")
        };
        assert_eq!(observed.result(), &legacy_result);
        let (legacy_activations, legacy_rows) = legacy_tracker.take();
        assert_eq!(
            repo_spec_row(&legacy_rows, &legacy_key.to_string()),
            [
                HostSelectedModuleGraphKey::new(workspace.dupe()).to_string(),
                HostSelectedRegistryRepoSpecsKey::new(workspace.dupe()).to_string(),
            ]
        );
        let legacy_events = legacy_activations
            .iter()
            .filter_map(|entry| entry.batch.dupe().map(|batch| (entry.key.clone(), batch)))
            .collect::<Vec<_>>();
        assert_eq!(
            observed_events
                .iter()
                .map(|(_, batch)| batch)
                .collect::<Vec<_>>(),
            legacy_events
                .iter()
                .map(|(_, batch)| batch)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            observed_events
                .iter()
                .map(|(owner, _)| owner.as_str())
                .collect::<Vec<_>>(),
            [
                "bzlmod-observed-host-root-module-file:\"/selected-repo-spec-test\"",
                "host-discovered-module:\"/selected-repo-spec-test\":dep@1",
            ]
        );
        assert_eq!(
            legacy_events
                .iter()
                .map(|(owner, _)| owner.as_str())
                .collect::<Vec<_>>(),
            [
                "root-module-evaluation:/selected-repo-spec-test",
                "host-discovered-module:\"/selected-repo-spec-test\":dep@1",
            ]
        );
        assert!(!observed_events.is_empty());
        assert_no_route_upper(&observed_rows);
        assert_no_route_upper(&legacy_rows);
        assert!(observed_rows.iter().all(|(owner, deps)| {
            let legacy = "host-selected-module-graph: host-selected-registry-repo-specs:";
            !legacy.split(' ').any(|prefix| {
                owner.starts_with(prefix) || deps.iter().any(|dep| dep.starts_with(prefix))
            })
        }));
        assert!(legacy_rows.iter().all(|(owner, deps)| {
            !owner.starts_with("observed-")
                && deps
                    .iter()
                    .all(|dependency| !dependency.starts_with("observed-"))
        }));
    }

    #[tokio::test]
    async fn observed_routes_need_is_carrierless_eventless_and_upper_silent() {
        const ROOT: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n";
        let io = Arc::new(TrackingRegistryIo::new([(
            "https://registry.invalid/modules/dep/1/MODULE.bazel",
            b"module(name='dep', version='1')\n" as &[u8],
        )]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io);
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepoSpecTracker::default());
        let value = compute_real_observed_routes(&dice, ROOT, 1, false, Some(tracker.dupe())).await;
        assert!(matches!(value, SourcePreparationOutcome::Need(_)));
        assert!(!HostSelectedModuleRoutesObservationKey::validity(&value));
        assert!(!HostSelectedModuleRoutesObservationKey::equality(
            &value, &value
        ));
        let (activations, rows) = tracker.take();
        let owner = HostSelectedModuleRoutesObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        )
        .to_string();
        assert!(
            activations
                .iter()
                .any(|entry| entry.key == owner && entry.batch.is_none())
        );
        assert_no_route_upper(&rows);
        let error = complete_observed_routes(
            &compute_real_observed_routes(&dice, ROOT, 2, true, Some(tracker.dupe())).await,
        );
        assert!(matches!(
            error.result().as_ref(),
            Err(HostSelectedModuleRoutesError::RepoSpecs(_))
        ));
        assert!(!error.observations().observations().is_empty());
        assert_no_route_upper(&tracker.take().1);
    }

    #[tokio::test]
    async fn observed_routes_restore_graph_and_repo_specs_with_held_carriers() {
        const MODULE_URL: &str = "https://registry.invalid/modules/dep/1/MODULE.bazel";
        const SOURCE_URL: &str = "https://registry.invalid/modules/dep/1/source.json";
        const ROOT_A: &str =
            "module(name='bazel_tools', repo_name='root_a')\nbazel_dep(name='dep', version='1')\n";
        const ROOT_B: &str =
            "module(name='bazel_tools', repo_name='root_b')\nbazel_dep(name='dep', version='1')\n";
        const SOURCE_A: &[u8] = br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#;
        const SOURCE_B: &[u8] = br#"{"url":"https://origin.test/b.tgz","integrity":"sha256-b"}"#;
        let io = Arc::new(TrackingRegistryIo::new([
            (MODULE_URL, b"module(name='dep', version='1')\n".as_slice()),
            (SOURCE_URL, SOURCE_A),
        ]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.dupe());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let a = complete_observed_routes(
            &compute_real_observed_routes(&dice, ROOT_A, 1, true, None).await,
        );
        let held_result = a.result().dupe();
        let held_epoch = a.observations().dupe();

        io.replace(SOURCE_URL, SOURCE_B);
        let repo_b = complete_observed_routes(
            &compute_real_observed_routes(&dice, ROOT_A, 2, true, None).await,
        );
        assert_ne!(repo_b.result(), a.result());
        assert_exact_repo_epoch(&held_epoch, repo_b.observations());
        io.replace(SOURCE_URL, SOURCE_A);
        let repo_a = complete_observed_routes(
            &compute_real_observed_routes(&dice, ROOT_A, 3, true, None).await,
        );
        assert_eq!(repo_a.result(), a.result());
        assert_exact_repo_epoch(&held_epoch, repo_a.observations());

        let graph_b = complete_observed_routes(
            &compute_real_observed_routes(&dice, ROOT_B, 3, true, None).await,
        );
        assert_ne!(graph_b.result(), a.result());
        let restored = complete_observed_routes(
            &compute_real_observed_routes(&dice, ROOT_A, 3, true, None).await,
        );
        assert_eq!(restored.result(), a.result());
        assert_eq!(restored.observations(), a.observations());
        let module_file = format!("{WORKSPACE}/MODULE.bazel");
        for (demand, result) in held_epoch.observations() {
            let restored_result = restored.observations().get(demand).unwrap();
            assert_eq!(result.as_ref(), restored_result.as_ref());
            if demand.path().as_path() != Path::new(&module_file) {
                assert!(Arc::ptr_eq(result, restored_result), "{demand:?}");
            }
        }
        assert_eq!(held_result.as_ref(), restored.result().as_ref());
    }

    #[tokio::test]
    async fn observed_routes_poll_drop_publishes_nothing_and_recovers() {
        const ROOT: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n";
        let io = Arc::new(CancelOnceRegistryIo {
            calls: AtomicUsize::new(0),
        });
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.dupe());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepoSpecTracker::default());
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostSelectedModuleRoutesObservationKey::new(workspace.dupe());
        let mut cancelled =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        tracker.take();
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while io.calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(future);
        drop(cancelled);
        let (cancelled_activations, cancelled_rows) = tracker.take();
        assert!(
            cancelled_rows
                .iter()
                .all(|(owner, _)| owner != &key.to_string())
        );
        assert!(
            cancelled_activations
                .iter()
                .all(|entry| entry.key != key.to_string())
        );
        assert_no_route_upper(&cancelled_rows);

        let mut recovered =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        let recovered_value = recovered.compute(&key).await.unwrap();
        let recovered_value = complete_observed_routes(&recovered_value);
        let graph = recovered
            .compute(&HostSelectedModuleGraphObservationKey::new(
                workspace.dupe(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(graph)) = graph else {
            panic!("recovered graph must complete")
        };
        let repo_specs = recovered
            .compute(&HostSelectedRegistryRepoSpecsObservationKey::new(workspace))
            .await
            .unwrap();
        let repo_specs = complete_observed_repo_specs(&repo_specs);
        let expected = PathObservationEpoch::from_shared(
            graph
                .observations()
                .observations()
                .iter()
                .chain(repo_specs.observations().observations())
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap();
        assert_exact_repo_epoch(&expected, recovered_value.observations());
        let (recovered_activations, recovered_rows) = tracker.take();
        assert!(
            recovered_activations
                .iter()
                .filter(|entry| entry.key == key.to_string())
                .all(|entry| entry.batch.is_none())
        );
        assert_no_route_upper(&recovered_rows);
    }

    #[test]
    fn observed_repo_specs_identity_merge_and_terminal_projection_are_exact() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostSelectedRegistryRepoSpecsObservationKey::new(workspace.dupe());
        let other = HostSelectedRegistryRepoSpecsObservationKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
        );
        assert_eq!(
            key.to_string(),
            "observed-host-selected-registry-repo-specs:\"/selected-repo-spec-test\""
        );
        assert_ne!(key, other);
        let hash = |key: &HostSelectedRegistryRepoSpecsObservationKey| {
            let mut state = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(key, &mut state);
            std::hash::Hasher::finish(&state)
        };
        assert_ne!(hash(&key), hash(&other));

        let demand = observation("/prefix", PathObservationOperation::Lstat);
        let first = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let mut prefix =
            PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let equal =
            PathObservationEpoch::from_shared([(demand.dupe(), Arc::new(first.as_ref().clone()))])
                .unwrap();
        merge_repo_spec_observations(
            &mut prefix,
            &equal,
            Some(&module()),
            RepoSpecObservationStage::SourceRegistryFile,
        )
        .unwrap();
        assert!(Arc::ptr_eq(prefix.get(&demand).unwrap(), &first));
        let conflict = PathObservationEpoch::from_shared([(
            demand.dupe(),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 2, 3, 4, 0o644),
            ))),
        )])
        .unwrap();
        assert!(matches!(
            merge_repo_spec_observations(
                &mut prefix,
                &conflict,
                Some(&module()),
                RepoSpecObservationStage::RegistryMetadataFile,
            ),
            Err(HostSelectedRegistryRepoSpecsObservationError::Merge {
                stage: RepoSpecObservationStage::RegistryMetadataFile,
                ..
            })
        ));

        let need = || {
            SourcePreparationNeeds::path(slug_workspace_v2::NeedPathObservations::singleton(
                demand.dupe(),
            ))
        };
        let mismatch = || {
            ObservedPathFrontierError::from(PathObservationEpochError::OperationMismatch {
                demand: demand.dupe(),
                result_operation: PathObservationOperation::FileBytes,
            })
        };
        let module = module();
        let mut empty = PathObservationEpoch::empty();
        assert!(matches!(
            finish_host_registry_child(RepoSpecChild::Need(need()), &module, &mut empty),
            Err(RepoSpecEntryTerminal::Need(_))
        ));
        assert!(matches!(
            finish_host_registry_child(RepoSpecChild::Outer(mismatch()), &module, &mut empty),
            Err(RepoSpecEntryTerminal::Outer(
                HostSelectedRegistryRepoSpecsObservationError::HostRegistry { .. }
            ))
        ));
        assert!(matches!(
            finish_registry_file_child(
                RepoSpecChild::Need(need()),
                &module,
                RegistryFileUrl::new("https://registry/source.json"),
                RepoSpecObservationStage::SourceRegistryFile,
                &mut empty,
            ),
            Err(RepoSpecEntryTerminal::Need(_))
        ));
        assert!(matches!(
            finish_registry_file_child(
                RepoSpecChild::Outer(mismatch()),
                &module,
                RegistryFileUrl::new("https://registry/source.json"),
                RepoSpecObservationStage::SourceRegistryFile,
                &mut empty,
            ),
            Err(RepoSpecEntryTerminal::Outer(
                HostSelectedRegistryRepoSpecsObservationError::RegistryFile { .. }
            ))
        ));
        assert!(matches!(
            finish_effective_override_child(RepoSpecChild::Need(need()), &module, &mut empty),
            Err(RepoSpecEntryTerminal::Need(_))
        ));
        assert!(matches!(
            finish_effective_override_child(RepoSpecChild::Outer(mismatch()), &module, &mut empty),
            Err(RepoSpecEntryTerminal::Outer(
                HostSelectedRegistryRepoSpecsObservationError::EffectiveOverride { .. }
            ))
        ));

        let result = Arc::new(Ok(HostSelectedRegistryRepoSpecs {
            entries: Arc::from([]),
        }));
        let observed = ObservedHostSelectedRegistryRepoSpecs::new(result.dupe(), prefix.dupe());
        let complete: <HostSelectedRegistryRepoSpecsObservationKey as Key>::Value =
            SourcePreparationOutcome::Complete(Ok(observed));
        assert!(HostSelectedRegistryRepoSpecsObservationKey::validity(
            &complete
        ));
        assert!(HostSelectedRegistryRepoSpecsObservationKey::equality(
            &complete, &complete
        ));
        let legacy = project_legacy_repo_specs(SourcePreparationOutcome::Complete(Ok((
            result.dupe(),
            prefix.dupe(),
        ))));
        let SourcePreparationOutcome::Complete(projected) = legacy else {
            panic!("legacy projection must complete");
        };
        assert!(Arc::ptr_eq(&projected, &result));
        let need_value: <HostSelectedRegistryRepoSpecsObservationKey as Key>::Value =
            SourcePreparationOutcome::Need(need());
        assert!(!HostSelectedRegistryRepoSpecsObservationKey::validity(
            &need_value
        ));
        assert!(!HostSelectedRegistryRepoSpecsObservationKey::equality(
            &need_value,
            &need_value
        ));
    }

    async fn assert_repo_spec_error_scan(
        engines: (&Arc<Dice>, &Arc<Dice>),
        ios: (&Arc<TrackingRegistryIo>, &Arc<TrackingRegistryIo>),
        trackers: (&Arc<RepoSpecTracker>, &Arc<RepoSpecTracker>),
        keys: (
            &HostSelectedRegistryRepoSpecsObservationKey,
            &HostSelectedRegistryRepoSpecsKey,
        ),
        rows: (&[String], &[String]),
        observed: &ObservedHostSelectedRegistryRepoSpecs,
        urls: (&str, &str),
        root: &str,
    ) {
        let (dice, legacy_dice) = engines;
        let (io, legacy_io) = ios;
        let (tracker, legacy_tracker) = trackers;
        let (observed_key, legacy_key) = keys;
        let (observed_row, legacy_row) = rows;
        let (bad_source_url, source_url) = urls;
        io.replace(bad_source_url, b"{");
        legacy_io.replace(bad_source_url, b"{");
        let before = io.calls().len();
        let error = compute_real_observed(dice, root, 2, &[], true, Some(tracker.dupe())).await;
        let error = complete_observed_repo_specs(&error);
        assert!(matches!(
            error.result().as_ref(),
            Err(HostSelectedRegistryRepoSpecsError::Json { module, file, .. })
                if module == &route_key("bad", "1") && file == "source.json"
        ));
        assert_exact_repo_epoch(observed.observations(), error.observations());
        assert!(io.calls()[before..].iter().any(|url| url == source_url));
        let (error_activations, error_rows) = tracker.take();
        let error_row = repo_spec_row(&error_rows, &observed_key.to_string());
        assert_eq!(
            error_row,
            [0, 1, 2, 5, 6, 7, 8].map(|index| observed_row[index].clone())
        );
        let mut transaction = real_transaction_with_tracker(
            legacy_dice,
            root,
            2,
            &[],
            true,
            Some(legacy_tracker.dupe()),
        )
        .await;
        let legacy_error = transaction.compute(legacy_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy_error) = legacy_error else {
            panic!("legacy error must complete")
        };
        assert_eq!(error.result(), &legacy_error);
        let (legacy_activations, legacy_rows) = legacy_tracker.take();
        let legacy_error_row = repo_spec_row(&legacy_rows, &legacy_key.to_string());
        assert_eq!(
            legacy_error_row,
            [0, 1, 2, 5, 6, 7, 8].map(|index| legacy_row[index].clone())
        );
        let batches = |activations: &[RepoSpecActivation]| {
            activations
                .iter()
                .filter_map(|entry| entry.batch.dupe())
                .collect::<Vec<_>>()
        };
        assert_eq!(batches(&error_activations), batches(&legacy_activations));
        assert_no_repo_spec_upper(&error_rows);
        assert_no_repo_spec_upper(&legacy_rows);
    }
    #[tokio::test]
    async fn observed_repo_specs_match_legacy_families_events_and_warm_reuse() {
        const BAD_MODULE_URL: &str = "https://registry.invalid/modules/bad/1/MODULE.bazel";
        const BAD_SOURCE_URL: &str = "https://registry.invalid/modules/bad/1/source.json";
        const MODULE_URL: &str = "https://registry.invalid/modules/dep/1/MODULE.bazel";
        const SOURCE_URL: &str = "https://registry.invalid/modules/dep/1/source.json";
        const REGISTRY_URL: &str = "https://registry.invalid/bazel_registry.json";
        const ROOT: &str = "module(name='bazel_tools')\n\
            local_path_override(module_name='local', path='local')\n\
            bazel_dep(name='local', version='1')\n\
            bazel_dep(name='bad', version='1')\n\
            bazel_dep(name='dep', version='1')\n";
        let files = [
            (
                BAD_MODULE_URL,
                b"module(name='bad', version='1')\n".as_slice(),
            ),
            (
                BAD_SOURCE_URL,
                br#"{"url":"https://origin.test/bad.tgz","integrity":"sha256-bad"}"#.as_slice(),
            ),
            (MODULE_URL, b"module(name='dep', version='1')\n".as_slice()),
            (
                SOURCE_URL,
                br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#.as_slice(),
            ),
            (REGISTRY_URL, br#"{"mirrors":[]}"#.as_slice()),
        ];
        let tracker = Arc::new(RepoSpecTracker::default());
        let io = Arc::new(TrackingRegistryIo::new(files));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.dupe());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let observed_key = HostSelectedRegistryRepoSpecsObservationKey::new(workspace.dupe());
        let mut transaction =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        let cold = transaction.compute(&observed_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = &cold else {
            panic!("observed repo specs must complete: {cold:?}");
        };
        assert!(observed.result().as_ref().is_ok());
        assert!(!observed.observations().observations().is_empty());
        let (observed_activations, observed_rows) = tracker.take();
        let observed_row = repo_spec_row(&observed_rows, &observed_key.to_string());
        let observed_file = |url| {
            RegistryFileObservationKey::new(
                workspace.as_path().to_owned(),
                RegistryFileUrl::new(url),
            )
            .to_string()
        };
        assert_eq!(
            observed_row,
            vec![
                HostSelectedModuleGraphObservationKey::new(workspace.dupe()).to_string(),
                HostRegistryFunctionObservationKey::new(workspace.dupe(), REGISTRY).to_string(),
                observed_file(BAD_SOURCE_URL),
                observed_file(REGISTRY_URL),
                HostEffectiveModuleOverrideObservationKey::new(workspace.dupe(), "bad".into(),)
                    .to_string(),
                HostRegistryFunctionObservationKey::new(workspace.dupe(), REGISTRY).to_string(),
                observed_file(SOURCE_URL),
                observed_file(REGISTRY_URL),
                HostEffectiveModuleOverrideObservationKey::new(workspace.dupe(), "dep".into(),)
                    .to_string(),
            ]
        );
        let observed_events = observed_activations
            .iter()
            .filter_map(|entry| entry.batch.dupe())
            .collect::<Vec<_>>();
        assert!(
            observed_activations
                .iter()
                .any(|entry| { entry.key == observed_key.to_string() && entry.batch.is_none() })
        );

        let warm = transaction.compute(&observed_key).await.unwrap();
        assert!(HostSelectedRegistryRepoSpecsObservationKey::equality(
            &cold, &warm
        ));
        assert!(
            tracker
                .take()
                .0
                .iter()
                .all(|activation| activation.batch.is_none())
        );

        let legacy_tracker = Arc::new(RepoSpecTracker::default());
        let legacy_io = Arc::new(TrackingRegistryIo::new(files));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, legacy_io.dupe());
        let legacy_dice = Arc::new(builder.build(DetectCycles::Enabled));
        let legacy_key = HostSelectedRegistryRepoSpecsKey::new(workspace.dupe());
        let mut legacy_transaction = real_transaction_with_tracker(
            &legacy_dice,
            ROOT,
            1,
            &[],
            true,
            Some(legacy_tracker.dupe()),
        )
        .await;
        let legacy = legacy_transaction.compute(&legacy_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy_result) = &legacy else {
            panic!("legacy repo specs must complete");
        };
        assert_eq!(observed.result(), legacy_result);
        let (legacy_activations, legacy_rows) = legacy_tracker.take();
        let legacy_row = repo_spec_row(&legacy_rows, &legacy_key.to_string());
        let legacy_file = |url| {
            RegistryFileKey {
                workspace: workspace.as_path().to_owned(),
                url: RegistryFileUrl::new(url),
            }
            .to_string()
        };
        assert_eq!(
            legacy_row,
            vec![
                HostSelectedModuleGraphKey::new(workspace.dupe()).to_string(),
                HostRegistryFunctionKey::new(workspace.dupe(), REGISTRY).to_string(),
                legacy_file(BAD_SOURCE_URL),
                legacy_file(REGISTRY_URL),
                HostEffectiveModuleOverrideKey::new(workspace.dupe(), "bad".into()).to_string(),
                HostRegistryFunctionKey::new(workspace.dupe(), REGISTRY).to_string(),
                legacy_file(SOURCE_URL),
                legacy_file(REGISTRY_URL),
                HostEffectiveModuleOverrideKey::new(workspace.dupe(), "dep".into()).to_string(),
            ]
        );
        let legacy_events = legacy_activations
            .iter()
            .filter_map(|entry| entry.batch.dupe())
            .collect::<Vec<_>>();
        assert_eq!(observed_events, legacy_events);
        assert!(!observed_events.is_empty());

        assert_repo_spec_error_scan(
            (&dice, &legacy_dice),
            (&io, &legacy_io),
            (&tracker, &legacy_tracker),
            (&observed_key, &legacy_key),
            (observed_row, legacy_row),
            observed,
            (BAD_SOURCE_URL, SOURCE_URL),
            ROOT,
        )
        .await;

        let git_io = Arc::new(TrackingRegistryIo::new([
            (MODULE_URL, b"module(name='dep', version='1')\n".as_slice()),
            (
                SOURCE_URL,
                br#"{"type":"git_repository","remote":"https://git.test/repo","commit":"abc"}"#
                    .as_slice(),
            ),
        ]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, git_io.dupe());
        let git_dice = Arc::new(builder.build(DetectCycles::Enabled));
        let git_tracker = Arc::new(RepoSpecTracker::default());
        let git_root = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n";
        let git_value =
            compute_real_observed(&git_dice, git_root, 1, &[], true, Some(git_tracker.dupe()))
                .await;
        assert!(
            complete_observed_repo_specs(&git_value)
                .result()
                .as_ref()
                .is_ok()
        );
        assert!(!git_io.calls().iter().any(|url| url == REGISTRY_URL));
        let git_rows = git_tracker.take().1;
        let git_row = repo_spec_row(&git_rows, &observed_key.to_string());
        assert_eq!(git_row.len(), 4);
        assert!(
            git_row
                .iter()
                .all(|dependency| dependency != &observed_file(REGISTRY_URL))
        );
        assert_no_repo_spec_upper(&observed_rows);
        assert_no_repo_spec_upper(&legacy_rows);
    }

    #[tokio::test]
    async fn observed_repo_specs_restore_each_child_input_with_held_carriers() {
        const MODULE_URL: &str = "https://registry.invalid/modules/dep/1/MODULE.bazel";
        const SOURCE_URL: &str = "https://registry.invalid/modules/dep/1/source.json";
        const REGISTRY_URL: &str = "https://registry.invalid/bazel_registry.json";
        const MODULE_A: &[u8] = b"module(name='dep', version='1')\n";
        const MODULE_B: &[u8] = b"module(name='dep', version='1')\n# graph-b\n";
        const SOURCE_A: &[u8] = br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#;
        const SOURCE_B: &[u8] = br#"{"url":"https://origin.test/b.tgz","integrity":"sha256-b"}"#;
        const REGISTRY_A: &[u8] = br#"{"mirrors":["https://a.test"]}"#;
        const REGISTRY_B: &[u8] = br#"{"mirrors":["https://b.test"]}"#;
        const ROOT: &str = "module(name='bazel_tools')\n\
            single_version_override(module_name='dep', patch_strip=0)\n\
            bazel_dep(name='dep', version='1')\n";
        let io = Arc::new(TrackingRegistryIo::new([
            (MODULE_URL, MODULE_A),
            (SOURCE_URL, SOURCE_A),
            (REGISTRY_URL, REGISTRY_A),
        ]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.dupe());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let a = complete_observed_repo_specs(
            &compute_real_observed(&dice, ROOT, 1, &["https://command-a.test"], true, None).await,
        );
        let held_result = a.result().dupe();
        let held_epoch = a.observations().dupe();
        let warm = complete_observed_repo_specs(
            &compute_real_observed(&dice, ROOT, 1, &["https://command-a.test"], true, None).await,
        );
        assert_eq!(warm, a);
        assert!(Arc::ptr_eq(warm.result(), a.result()));
        assert_exact_repo_epoch(&held_epoch, warm.observations());

        io.replace(SOURCE_URL, SOURCE_B);
        let source_b = complete_observed_repo_specs(
            &compute_real_observed(&dice, ROOT, 2, &["https://command-a.test"], true, None).await,
        );
        assert_ne!(source_b.result(), a.result());
        assert_exact_repo_epoch(&held_epoch, source_b.observations());
        io.replace(SOURCE_URL, SOURCE_A);
        let source_a = complete_observed_repo_specs(
            &compute_real_observed(&dice, ROOT, 3, &["https://command-a.test"], true, None).await,
        );
        assert_eq!(source_a.result(), a.result());
        assert_exact_repo_epoch(&held_epoch, source_a.observations());

        io.replace(REGISTRY_URL, REGISTRY_B);
        let registry_b = complete_observed_repo_specs(
            &compute_real_observed(&dice, ROOT, 4, &["https://command-a.test"], true, None).await,
        );
        assert_ne!(registry_b.result(), a.result());
        assert_exact_repo_epoch(&held_epoch, registry_b.observations());
        io.replace(REGISTRY_URL, REGISTRY_A);
        let registry_a = complete_observed_repo_specs(
            &compute_real_observed(&dice, ROOT, 5, &["https://command-a.test"], true, None).await,
        );
        assert_eq!(registry_a.result(), a.result());
        assert_exact_repo_epoch(&held_epoch, registry_a.observations());

        io.replace(MODULE_URL, MODULE_B);
        let graph_b = complete_observed_repo_specs(
            &compute_real_observed(&dice, ROOT, 6, &["https://command-a.test"], true, None).await,
        );
        assert_ne!(graph_b.result(), a.result());
        assert_exact_repo_epoch(&held_epoch, graph_b.observations());
        io.replace(MODULE_URL, MODULE_A);
        let graph_a = complete_observed_repo_specs(
            &compute_real_observed(&dice, ROOT, 7, &["https://command-a.test"], true, None).await,
        );
        assert_eq!(graph_a.result(), a.result());
        assert_exact_repo_epoch(&held_epoch, graph_a.observations());

        let policy_b = complete_observed_repo_specs(
            &compute_real_observed(&dice, ROOT, 7, &["https://command-b.test"], true, None).await,
        );
        assert_ne!(policy_b.result(), a.result());
        assert_exact_repo_epoch(&held_epoch, policy_b.observations());
        let policy_a = complete_observed_repo_specs(
            &compute_real_observed(&dice, ROOT, 7, &["https://command-a.test"], true, None).await,
        );
        assert_eq!(policy_a.result(), a.result());
        assert_exact_repo_epoch(&held_epoch, policy_a.observations());

        let override_b = complete_observed_repo_specs(
            &compute_real_observed(
                &dice,
                &ROOT.replace("patch_strip=0", "patch_strip=2"),
                7,
                &["https://command-a.test"],
                true,
                None,
            )
            .await,
        );
        assert_ne!(override_b.result(), a.result());
        assert_ne!(override_b.observations(), a.observations());
        let restored = complete_observed_repo_specs(
            &compute_real_observed(&dice, ROOT, 7, &["https://command-a.test"], true, None).await,
        );
        assert_eq!(restored.result(), a.result());
        assert_eq!(restored.observations(), a.observations());
        let root_module = format!("{WORKSPACE}/MODULE.bazel");
        for (demand, result) in held_epoch.observations() {
            assert_eq!(
                result.as_ref(),
                restored.observations().get(demand).unwrap().as_ref()
            );
            if demand.path().as_path() != Path::new(&root_module) {
                assert!(Arc::ptr_eq(
                    result,
                    restored.observations().get(demand).unwrap()
                ));
            }
        }
        assert_eq!(held_result.as_ref(), restored.result().as_ref());
    }

    #[test]
    fn repo_spec_production_finishers_preserve_exact_terminal_prefixes() {
        let demand = |path: &str| observation(path, PathObservationOperation::Lstat);
        let result = |inode| {
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 2, inode, 4, 0o644),
            )))
        };
        let first_demand = demand("/first");
        let first_result = result(3);
        let prefix =
            PathObservationEpoch::from_shared([(first_demand.dupe(), first_result.dupe())])
                .unwrap();
        let second_demand = demand("/second");
        let second_result = result(4);
        let incoming =
            PathObservationEpoch::from_shared([(second_demand.dupe(), second_result.dupe())])
                .unwrap();
        let module = module();
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();

        let mut graph_prefix = PathObservationEpoch::empty();
        let compute = finish_selected_graph_child(
            RepoSpecChild::Compute("graph-compute".into()),
            &mut graph_prefix,
        )
        .unwrap_err();
        assert!(matches!(
            compute,
            SourcePreparationOutcome::Complete(Ok((result, observations)))
                if matches!(
                    result.as_ref(),
                    Err(HostSelectedRegistryRepoSpecsError::GraphCompute(message))
                        if message == "graph-compute"
                ) && observations.observations().is_empty()
        ));
        let graph_need = SourcePreparationNeeds::path(
            slug_workspace_v2::NeedPathObservations::singleton(first_demand.dupe()),
        );
        assert!(matches!(
            finish_selected_graph_child(RepoSpecChild::Need(graph_need), &mut graph_prefix),
            Err(SourcePreparationOutcome::Need(_))
        ));
        assert!(matches!(
            finish_selected_graph_child(
                RepoSpecChild::Outer(HostSelectedModuleGraphObservationError::Merge(
                    ObservedPathFrontierError::from(PathObservationEpochError::DuplicateDemand(
                        first_demand.dupe()
                    ))
                )),
                &mut graph_prefix,
            ),
            Err(SourcePreparationOutcome::Complete(Err(
                HostSelectedRegistryRepoSpecsObservationError::Graph(_)
            )))
        ));
        let graph_result = Arc::new(Err(HostSelectedModuleGraphError::Input {
            owner: "graph".into(),
            message: "semantic".into(),
        }));
        let returned = finish_selected_graph_child(
            RepoSpecChild::Complete {
                result: graph_result.dupe(),
                observations: incoming.dupe(),
            },
            &mut graph_prefix,
        )
        .unwrap();
        assert!(Arc::ptr_eq(&returned, &graph_result));
        assert_exact_repo_epoch(&incoming, &graph_prefix);

        let mut policy_prefix = prefix.dupe();
        assert!(matches!(
            finish_host_registry_child(
                RepoSpecChild::Compute("policy-compute".into()),
                &module,
                &mut policy_prefix,
            ),
            Err(RepoSpecEntryTerminal::Complete(Err(
                HostSelectedRegistryRepoSpecsError::RegistryPolicyCompute {
                    message,
                    ..
                }
            ))) if message == "policy-compute"
        ));
        assert_exact_repo_epoch(&prefix, &policy_prefix);
        assert!(matches!(
            finish_host_registry_child(
                RepoSpecChild::Complete {
                    result: Arc::new(Err(HostRegistryFunctionError::LockfileModeInput {
                        workspace: workspace.dupe(),
                    })),
                    observations: incoming.dupe(),
                },
                &module,
                &mut policy_prefix,
            ),
            Err(RepoSpecEntryTerminal::Complete(Err(
                HostSelectedRegistryRepoSpecsError::RegistryPolicy { .. }
            )))
        ));
        assert!(Arc::ptr_eq(
            policy_prefix.get(&first_demand).unwrap(),
            &first_result
        ));
        assert!(Arc::ptr_eq(
            policy_prefix.get(&second_demand).unwrap(),
            &second_result
        ));

        let mut file_prefix = prefix.dupe();
        let url = RegistryFileUrl::new("https://registry.invalid/source.json");
        assert!(matches!(
            finish_registry_file_child(
                RepoSpecChild::Compute("file-compute".into()),
                &module,
                url.dupe(),
                RepoSpecObservationStage::SourceRegistryFile,
                &mut file_prefix,
            ),
            Err(RepoSpecEntryTerminal::Complete(Err(
                HostSelectedRegistryRepoSpecsError::RegistryFileCompute { message, .. }
            ))) if message == "file-compute"
        ));
        assert_exact_repo_epoch(&prefix, &file_prefix);
        assert!(matches!(
            finish_registry_file_child(
                RepoSpecChild::Complete {
                    result: Arc::new(Err(RegistryFileError::MissingIoCapability)),
                    observations: incoming.dupe(),
                },
                &module,
                url,
                RepoSpecObservationStage::SourceRegistryFile,
                &mut file_prefix,
            ),
            Err(RepoSpecEntryTerminal::Complete(Err(
                HostSelectedRegistryRepoSpecsError::RegistryFile { .. }
            )))
        ));
        assert!(Arc::ptr_eq(
            file_prefix.get(&first_demand).unwrap(),
            &first_result
        ));
        assert!(Arc::ptr_eq(
            file_prefix.get(&second_demand).unwrap(),
            &second_result
        ));

        let mut effective_prefix = prefix.dupe();
        assert!(matches!(
            finish_effective_override_child(
                RepoSpecChild::Compute("effective-compute".into()),
                &module,
                &mut effective_prefix,
            ),
            Err(RepoSpecEntryTerminal::Complete(Err(
                HostSelectedRegistryRepoSpecsError::EffectiveOverrideCompute { message, .. }
            ))) if message == "effective-compute"
        ));
        assert_exact_repo_epoch(&prefix, &effective_prefix);
        assert!(matches!(
            finish_effective_override_child(
                RepoSpecChild::Complete {
                    result: Arc::new(Err(HostEffectiveModuleOverrideError::CommandPolicy(
                        "effective".into()
                    ))),
                    observations: incoming,
                },
                &module,
                &mut effective_prefix,
            ),
            Err(RepoSpecEntryTerminal::Complete(Err(
                HostSelectedRegistryRepoSpecsError::EffectiveOverride { .. }
            )))
        ));
        assert!(Arc::ptr_eq(
            effective_prefix.get(&first_demand).unwrap(),
            &first_result
        ));
        assert!(Arc::ptr_eq(
            effective_prefix.get(&second_demand).unwrap(),
            &second_result
        ));
    }

    #[test]
    fn repo_spec_accumulator_preserves_full_scan_terminal_precedence() {
        let demand = |path: &str| {
            slug_workspace_v2::NeedPathObservations::singleton(observation(
                path,
                PathObservationOperation::Lstat,
            ))
        };
        let epoch_demand = observation("/prefix", PathObservationOperation::Lstat);
        let epoch_result = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let epoch = PathObservationEpoch::from_shared([(epoch_demand.dupe(), epoch_result.dupe())])
            .unwrap();
        let semantic = || {
            RepoSpecEntryTerminal::Complete(Err(HostSelectedRegistryRepoSpecsError::Projection {
                module: module(),
                message: "first-semantic".into(),
            }))
        };
        for position in 0..3 {
            let mut accumulator = RepoSpecsAccumulator::default();
            for index in 0..3 {
                accumulator.record(if index == position {
                    semantic()
                } else {
                    RepoSpecEntryTerminal::Complete(Ok(None))
                });
            }
            assert!(matches!(
                accumulator.finish(epoch.dupe()),
                SourcePreparationOutcome::Complete(Ok((result, observations)))
                    if matches!(
                        result.as_ref(),
                        Err(HostSelectedRegistryRepoSpecsError::Projection {
                            message,
                            ..
                        }) if message == "first-semantic"
                    ) && Arc::ptr_eq(observations.get(&epoch_demand).unwrap(), &epoch_result)
            ));
        }

        let mismatch = || {
            ObservedPathFrontierError::from(PathObservationEpochError::OperationMismatch {
                demand: epoch_demand.dupe(),
                result_operation: PathObservationOperation::FileBytes,
            })
        };
        let outer = |merge| {
            if merge {
                HostSelectedRegistryRepoSpecsObservationError::Merge {
                    module: Some(Arc::new(module())),
                    stage: RepoSpecObservationStage::HostRegistry,
                    error: mismatch(),
                }
            } else {
                HostSelectedRegistryRepoSpecsObservationError::HostRegistry {
                    module: Arc::new(module()),
                    error: mismatch(),
                }
            }
        };
        let terminal_class = |outcome: &RepoSpecsDriverOutcome| match outcome {
            SourcePreparationOutcome::Complete(Err(
                HostSelectedRegistryRepoSpecsObservationError::HostRegistry { .. },
            )) => 0,
            SourcePreparationOutcome::Complete(Err(
                HostSelectedRegistryRepoSpecsObservationError::Merge { .. },
            )) => 1,
            SourcePreparationOutcome::Need(_) => 2,
            _ => 3,
        };
        for kind in 0..3 {
            for position in 0..3 {
                let mut accumulator = RepoSpecsAccumulator::default();
                for index in 0..3 {
                    accumulator.record(if index != position {
                        RepoSpecEntryTerminal::Complete(Ok(None))
                    } else if kind == 2 {
                        RepoSpecEntryTerminal::Need(SourcePreparationNeeds::path(demand("/need")))
                    } else {
                        RepoSpecEntryTerminal::Outer(outer(kind == 1))
                    });
                }
                if kind == 2 {
                    accumulator.record(RepoSpecEntryTerminal::Need(SourcePreparationNeeds::path(
                        demand("/union"),
                    )));
                }
                let outcome = accumulator.finish(epoch.dupe());
                assert_eq!(terminal_class(&outcome), kind);
                assert!(
                    kind != 2
                        || matches!(&outcome, SourcePreparationOutcome::Need(needs)
                    if needs.path_observations().unwrap().demands().len() == 2)
                );
            }
        }

        let mut priority = RepoSpecsAccumulator::default();
        priority.record(RepoSpecEntryTerminal::Need(SourcePreparationNeeds::path(
            demand("/a"),
        )));
        priority.record(semantic());
        priority.record(RepoSpecEntryTerminal::Outer(outer(false)));
        priority.record(RepoSpecEntryTerminal::Outer(outer(true)));
        assert_eq!(terminal_class(&priority.finish(epoch.dupe())), 0);
        let mut semantic_first = RepoSpecsAccumulator::default();
        semantic_first.record(RepoSpecEntryTerminal::Need(SourcePreparationNeeds::path(
            demand("/a"),
        )));
        semantic_first.record(semantic());
        assert!(
            matches!(semantic_first.finish(epoch.dupe()), SourcePreparationOutcome::Complete(Ok((result, _)))
            if matches!(result.as_ref(), Err(HostSelectedRegistryRepoSpecsError::Projection { .. })))
        );

        let request = |path: &str| crate::RepositoryMaterializationRequest {
            id: crate::RepositoryMaterializationRequestId {
                workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
            },
            repo_spec: repo_spec(
                "@@bazel_tools//tools/build_defs/repo:local.bzl",
                "local_repository",
                SmallMap::new(),
            ),
            kind: crate::RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new(path).unwrap(),
            },
        };
        for (first, second) in [(0, 1), (0, 2), (1, 2)] {
            let mut incompatible = RepoSpecsAccumulator::default();
            for index in 0..3 {
                incompatible.record(if index == first || index == second {
                    RepoSpecEntryTerminal::Need(SourcePreparationNeeds::repository(request(
                        if index == first { "/a" } else { "/b" },
                    )))
                } else {
                    RepoSpecEntryTerminal::Complete(Ok(None))
                });
            }
            assert!(matches!(incompatible.finish(epoch.dupe()),
                SourcePreparationOutcome::Complete(Ok((result, observations)))
                    if matches!(result.as_ref(), Err(
                        HostSelectedRegistryRepoSpecsError::IncompatibleNeeds(
                            SourcePreparationNeedsError::ConflictingRepositoryRequest { .. }
                        ))) && Arc::ptr_eq(observations.get(&epoch_demand).unwrap(), &epoch_result)
            ));
        }
    }

    #[tokio::test]
    async fn observed_repo_specs_poll_drop_publishes_nothing_and_recovers() {
        const ROOT: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n";
        let io = Arc::new(CancelOnceRegistryIo {
            calls: AtomicUsize::new(0),
        });
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.dupe());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepoSpecTracker::default());
        let key = HostSelectedRegistryRepoSpecsObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let mut cancelled =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        tracker.take();
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while io.calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(future);
        drop(cancelled);
        let (cancelled_activations, cancelled_rows) = tracker.take();
        assert!(
            cancelled_rows
                .iter()
                .all(|(owner, _)| owner != &key.to_string())
        );
        assert!(
            cancelled_activations
                .iter()
                .all(|activation| activation.key != key.to_string())
        );

        let mut recovered =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        let recovered_value = recovered.compute(&key).await.unwrap();
        assert!(
            complete_observed_repo_specs(&recovered_value)
                .result()
                .as_ref()
                .is_ok()
        );
        assert!(io.calls.load(Ordering::SeqCst) >= 4);
        let (recovered_activations, recovered_rows) = tracker.take();
        assert!(
            recovered_rows
                .iter()
                .any(|(owner, _)| owner == &key.to_string())
        );
        assert!(
            recovered_activations
                .iter()
                .filter(|activation| activation.key == key.to_string())
                .all(|activation| activation.batch.is_none())
        );
        assert_no_repo_spec_upper(&cancelled_rows);
        assert_no_repo_spec_upper(&recovered_rows);
    }
    #[test]
    fn observed_extension_mappings_identity_terminals_and_merge_are_exact() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostSelectedExtensionMappingsObservationKey::new(workspace.dupe());
        let other = HostSelectedExtensionMappingsObservationKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
        );
        let hash = |value: &HostSelectedExtensionMappingsObservationKey| {
            let mut state = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(value, &mut state);
            std::hash::Hasher::finish(&state)
        };
        assert_ne!(key, other);
        assert_ne!(hash(&key), hash(&other));
        assert_eq!(
            key.to_string(),
            "observed-host-selected-extension-mappings:\"/selected-repo-spec-test\""
        );
        let demand = observation("/shared", PathObservationOperation::Lstat);
        let first = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let epoch = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let equal =
            PathObservationEpoch::from_shared([(demand.dupe(), Arc::new(first.as_ref().clone()))])
                .unwrap();
        let need = || {
            SourcePreparationNeeds::path(slug_workspace_v2::NeedPathObservations::singleton(
                demand.dupe(),
            ))
        };
        let mismatch = || {
            ObservedPathFrontierError::from(PathObservationEpochError::OperationMismatch {
                demand: demand.dupe(),
                result_operation: PathObservationOperation::FileBytes,
            })
        };
        let mut prefix = PathObservationEpoch::empty();
        assert!(matches!(
            finish_extension_routes_child(RepoSpecChild::Compute("routes".into()), &mut prefix),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(
                    HostSelectedExtensionMappingsError::RoutesCompute(message)
                ) if message == "routes") && observations.observations().is_empty()
        ));
        assert!(matches!(
            finish_extension_routes_child(RepoSpecChild::Need(need()), &mut prefix),
            Err(SourcePreparationOutcome::Need(_))
        ));
        assert!(matches!(
            finish_extension_routes_child(
                RepoSpecChild::Outer(HostSelectedModuleRoutesObservationError::Merge {
                    stage: RouteObservationStage::Graph,
                    error: mismatch(),
                }),
                &mut prefix,
            ),
            Err(SourcePreparationOutcome::Complete(Err(
                ExtensionMappingsObservationError::Routes(_)
            )))
        ));
        let routes = Arc::new(Err(route_invalid(&module(), "routes-semantic")));
        assert!(Arc::ptr_eq(
            &finish_extension_routes_child(
                RepoSpecChild::Complete {
                    result: routes.dupe(),
                    observations: epoch.dupe(),
                },
                &mut prefix,
            )
            .unwrap(),
            &routes
        ));
        assert!(matches!(
            finish_extension_routes_semantic(routes.as_ref(), prefix.dupe()),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(
                    HostSelectedExtensionMappingsError::Routes(_)
                )) && observations == epoch
        ));
        assert!(matches!(
            finish_extension_root_files_child(RepoSpecChild::Compute("root".into()), &mut prefix),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(
                    HostSelectedExtensionMappingsError::RootFilesCompute(message)
                ) if message == "root") && observations == epoch
        ));
        assert!(matches!(
            finish_extension_root_files_child(RepoSpecChild::Need(need()), &mut prefix),
            Err(SourcePreparationOutcome::Need(_))
        ));
        assert!(matches!(
            finish_extension_root_files_child(RepoSpecChild::Outer(mismatch()), &mut prefix),
            Err(SourcePreparationOutcome::Complete(Err(
                ExtensionMappingsObservationError::RootFiles(_)
            )))
        ));
        let root = Arc::new(Err(CompactString::new("root-semantic")));
        assert!(Arc::ptr_eq(
            &finish_extension_root_files_child(
                RepoSpecChild::Complete {
                    result: root.dupe(),
                    observations: equal,
                },
                &mut prefix,
            )
            .unwrap(),
            &root
        ));
        assert!(Arc::ptr_eq(prefix.get(&demand).unwrap(), &first));
        assert!(matches!(
            finish_extension_root_files_semantic(root.as_ref(), prefix.dupe()),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(
                    HostSelectedExtensionMappingsError::RootFiles(message)
                ) if message == "root-semantic") && observations == epoch
        ));
        let conflict = PathObservationEpoch::from_shared([(
            demand.dupe(),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 2, 3, 4, 0o644),
            ))),
        )])
        .unwrap();
        for stage in [
            ExtensionMappingsObservationStage::Routes,
            ExtensionMappingsObservationStage::RootFiles,
        ] {
            let mut current = epoch.dupe();
            assert!(matches!(
                merge_extension_mapping_observations(&mut current, &conflict, stage),
                Err(ExtensionMappingsObservationError::Merge {
                    stage: actual,
                    ..
                }) if actual == stage
            ));
        }
        for stage in [
            ExtensionMappingsObservationStage::Routes,
            ExtensionMappingsObservationStage::RootFiles,
        ] {
            assert!(matches!(
                extension_mapping_merge_error(
                    stage,
                    PathObservationEpochError::OperationMismatch {
                        demand: demand.dupe(),
                        result_operation: PathObservationOperation::FileBytes,
                    },
                ),
                ExtensionMappingsObservationError::Merge {
                    stage: actual,
                    ..
                } if actual == stage
            ));
        }
        let projected = Arc::new(Err(HostSelectedExtensionMappingsError::RootFiles(
            "legacy".into(),
        )));
        assert!(matches!(
            project_legacy_extension_mappings(SourcePreparationOutcome::Complete(Ok((
                projected.dupe(),
                epoch.dupe(),
            )))),
            SourcePreparationOutcome::Complete(result) if Arc::ptr_eq(&result, &projected)
        ));
        let outer = SourcePreparationOutcome::Complete(Err(
            ExtensionMappingsObservationError::RootFiles(mismatch()),
        ));
        assert!(HostSelectedExtensionMappingsObservationKey::validity(
            &outer
        ));
        assert!(HostSelectedExtensionMappingsObservationKey::equality(
            &outer, &outer
        ));
    }

    #[tokio::test]
    async fn observed_extension_mappings_match_families_epochs_events_and_warm() {
        const ROOT: &str = "module(name='bazel_tools')\ne=use_extension('//:e.bzl','e')\n";
        let tracker = Arc::new(RepoSpecTracker::default());
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostSelectedExtensionMappingsObservationKey::new(workspace.dupe());
        let mut transaction =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        let cold = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_extensions(&cold);
        let routes = complete_observed_routes(
            &transaction
                .compute(&HostSelectedModuleRoutesObservationKey::new(
                    workspace.dupe(),
                ))
                .await
                .unwrap(),
        );
        let root = transaction
            .compute(&RootModuleFilesObservationKey::new(workspace.dupe()))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(root)) = root else {
            panic!("root files must complete")
        };
        let expected = PathObservationEpoch::from_shared(
            routes
                .observations()
                .observations()
                .iter()
                .chain(root.observations().observations())
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap();
        assert_exact_repo_epoch(&expected, observed.observations());
        let (observed_activations, observed_rows) = tracker.take();
        assert_eq!(
            repo_spec_row(&observed_rows, &key.to_string()),
            [
                HostSelectedModuleRoutesObservationKey::new(workspace.dupe()).to_string(),
                RootModuleFilesObservationKey::new(workspace.dupe()).to_string(),
            ]
        );
        let observed_events = observed_activations
            .iter()
            .filter_map(|entry| entry.batch.dupe().map(|batch| (entry.key.clone(), batch)))
            .collect::<Vec<_>>();
        assert!(
            observed_activations
                .iter()
                .any(|entry| entry.key == key.to_string() && entry.batch.is_none())
        );
        assert!(HostSelectedExtensionMappingsObservationKey::equality(
            &cold,
            &transaction.compute(&key).await.unwrap()
        ));
        assert!(tracker.take().0.iter().all(|entry| entry.batch.is_none()));

        let legacy_tracker = Arc::new(RepoSpecTracker::default());
        let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let legacy_key = HostSelectedExtensionMappingsKey::new(workspace.dupe());
        let mut legacy_transaction = real_transaction_with_tracker(
            &legacy_dice,
            ROOT,
            1,
            &[],
            true,
            Some(legacy_tracker.dupe()),
        )
        .await;
        let SourcePreparationOutcome::Complete(legacy_result) =
            legacy_transaction.compute(&legacy_key).await.unwrap()
        else {
            panic!("legacy mappings must complete")
        };
        assert_eq!(observed.result(), &legacy_result);
        let (legacy_activations, legacy_rows) = legacy_tracker.take();
        assert_eq!(
            repo_spec_row(&legacy_rows, &legacy_key.to_string()),
            [
                HostSelectedModuleRoutesKey::new(workspace.dupe()).to_string(),
                RootModuleFilesKey {
                    workspace: workspace.as_path().to_owned(),
                }
                .to_string(),
            ]
        );
        let legacy_events = legacy_activations
            .iter()
            .filter_map(|entry| entry.batch.dupe().map(|batch| (entry.key.clone(), batch)))
            .collect::<Vec<_>>();
        assert_eq!(
            observed_events
                .iter()
                .map(|(_, batch)| batch)
                .collect::<Vec<_>>(),
            legacy_events
                .iter()
                .map(|(_, batch)| batch)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            observed_events
                .iter()
                .map(|(owner, _)| owner.as_str())
                .collect::<Vec<_>>(),
            ["bzlmod-observed-host-root-module-file:\"/selected-repo-spec-test\""]
        );
        assert_eq!(
            legacy_events
                .iter()
                .map(|(owner, _)| owner.as_str())
                .collect::<Vec<_>>(),
            ["root-module-evaluation:/selected-repo-spec-test"]
        );
        assert!(observed_rows.iter().all(|(owner, deps)| {
            let forbidden = "host-selected-module-routes: root-module-files:";
            !forbidden.split(' ').any(|prefix| {
                owner.starts_with(prefix) || deps.iter().any(|dep| dep.starts_with(prefix))
            })
        }));
        assert!(legacy_rows.iter().all(|(owner, deps)| {
            let forbidden =
                "observed- observed-root-module-files: bzlmod-observed-host-root-module-file:";
            !forbidden.split(' ').any(|prefix| {
                owner.starts_with(prefix) || deps.iter().any(|dep| dep.starts_with(prefix))
            })
        }));
        assert_no_mapping_upper(&observed_rows);
        assert_no_mapping_upper(&legacy_rows);

        let bad = complete_observed_extensions(
            &compute_real_observed_extensions(
                &dice,
                "module(name='bazel_tools')\ne=use_extension('@missing//:e.bzl','e')\n",
                2,
                true,
                Some(tracker.dupe()),
            )
            .await,
        );
        assert!(
            matches!(
                bad.result().as_ref(),
                Err(HostSelectedExtensionMappingsError::Invalid { .. })
            ),
            "{:?}",
            bad.result()
        );
        assert!(!bad.observations().observations().is_empty());
        assert_no_mapping_upper(&tracker.take().1);
    }

    #[tokio::test]
    async fn observed_extension_mappings_need_cancel_lifecycle_and_recovery_are_exact() {
        const ROOT: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n";
        let need_io = Arc::new(TrackingRegistryIo::new([(
            "https://registry.invalid/modules/dep/1/MODULE.bazel",
            b"module(name='dep', version='1')\n" as &[u8],
        )]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, need_io);
        let need_dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepoSpecTracker::default());
        let need =
            compute_real_observed_extensions(&need_dice, ROOT, 1, false, Some(tracker.dupe()))
                .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostSelectedExtensionMappingsObservationKey::validity(
            &need
        ));
        assert!(!HostSelectedExtensionMappingsObservationKey::equality(
            &need, &need
        ));
        let (need_activations, need_rows) = tracker.take();
        assert!(need_activations.iter().any(|entry| {
            entry
                .key
                .starts_with("observed-host-selected-extension-mappings:")
                && entry.batch.is_none()
        }));
        assert_no_mapping_upper(&need_rows);

        let io = Arc::new(CancelOnceRegistryIo {
            calls: AtomicUsize::new(0),
        });
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.dupe());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let key = HostSelectedExtensionMappingsObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let mut cancelled =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        tracker.take();
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while io.calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(future);
        drop(cancelled);
        let (cancelled_activations, cancelled_rows) = tracker.take();
        assert!(
            cancelled_rows
                .iter()
                .all(|(owner, _)| owner != &key.to_string())
        );
        assert!(
            cancelled_activations
                .iter()
                .all(|entry| entry.key != key.to_string())
        );
        assert_no_mapping_upper(&cancelled_rows);
        let recovered = complete_observed_extensions(
            &compute_real_observed_extensions(&dice, ROOT, 1, true, Some(tracker.dupe())).await,
        );
        assert!(recovered.result().as_ref().is_ok());
        assert!(!recovered.observations().observations().is_empty());
        assert_no_mapping_upper(&tracker.take().1);

        const A: &str =
            "module(name='bazel_tools',repo_name='root_a')\ne=use_extension('//:a.bzl','e')\n";
        const ROUTE_B: &str =
            "module(name='bazel_tools',repo_name='root_b')\ne=use_extension('//:a.bzl','e')\n";
        const USAGE_B: &str =
            "module(name='bazel_tools',repo_name='root_a')\ne=use_extension('//:b.bzl','e')\n";
        let lifecycle_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let a = complete_observed_extensions(
            &compute_real_observed_extensions(&lifecycle_dice, A, 1, true, None).await,
        );
        let held_result = a.result().dupe();
        let held_epoch = a.observations().dupe();
        for (source, generation) in [(ROUTE_B, 2), (A, 3), (USAGE_B, 4), (A, 5)] {
            let value = complete_observed_extensions(
                &compute_real_observed_extensions(&lifecycle_dice, source, generation, true, None)
                    .await,
            );
            assert_eq!(value.result() == a.result(), source == A);
        }
        let restored = complete_observed_extensions(
            &compute_real_observed_extensions(&lifecycle_dice, A, 6, true, None).await,
        );
        assert_eq!(held_result.as_ref(), restored.result().as_ref());
        let module_file = format!("{WORKSPACE}/MODULE.bazel");
        for (demand, result) in held_epoch.observations() {
            let restored_result = restored.observations().get(demand).unwrap();
            assert_eq!(result.as_ref(), restored_result.as_ref());
            if demand.path().as_path() != Path::new(&module_file) {
                assert!(Arc::ptr_eq(result, restored_result), "{demand:?}");
            }
        }
    }

    #[test]
    fn observed_definition_requests_identity_and_terminals_are_exact() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(workspace.dupe());
        let other = HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
        );
        let hash = |value: &HostSelectedExtensionDefinitionLoadRequestsObservationKey| {
            let mut state = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(value, &mut state);
            std::hash::Hasher::finish(&state)
        };
        assert_ne!(key, other);
        assert_ne!(hash(&key), hash(&other));
        assert_eq!(
            key.to_string(),
            "observed-host-selected-extension-definition-load-requests:\"/selected-repo-spec-test\""
        );

        let demand = observation("/mapping", PathObservationOperation::Lstat);
        let first = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let epoch = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let need = SourcePreparationNeeds::path(
            slug_workspace_v2::NeedPathObservations::singleton(demand.dupe()),
        );
        let mismatch = || {
            ExtensionMappingsObservationError::RootFiles(ObservedPathFrontierError::from(
                PathObservationEpochError::OperationMismatch {
                    demand: demand.dupe(),
                    result_operation: PathObservationOperation::FileBytes,
                },
            ))
        };
        assert!(matches!(
            finish_definition_load_requests_mappings_child(RepoSpecChild::Compute("dice".into())),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(
                    HostSelectedExtensionDefinitionLoadRequestsError(
                        HostSelectedExtensionDefinitionLoadRequestsErrorInner::MappingsCompute(
                            message
                        )
                    )
                ) if message == "dice") && observations.observations().is_empty()
        ));
        assert!(matches!(
            finish_definition_load_requests_mappings_child(RepoSpecChild::Need(need.dupe())),
            Err(SourcePreparationOutcome::Need(_))
        ));
        let outer = SourcePreparationOutcome::Complete(Err(
            HostSelectedExtensionDefinitionLoadRequestsObservationError(
                DefinitionLoadRequestsObservationError::Mappings(mismatch()),
            ),
        ));
        assert!(matches!(
            finish_definition_load_requests_mappings_child(RepoSpecChild::Outer(mismatch())),
            Err(SourcePreparationOutcome::Complete(Err(
                DefinitionLoadRequestsObservationError::Mappings(_)
            )))
        ));
        assert!(HostSelectedExtensionDefinitionLoadRequestsObservationKey::validity(&outer));
        assert!(
            HostSelectedExtensionDefinitionLoadRequestsObservationKey::equality(&outer, &outer)
        );

        let mappings = Arc::new(Err(extension_invalid(&module(), "mapping-semantic")));
        let (forwarded, forwarded_epoch) =
            finish_definition_load_requests_mappings_child(RepoSpecChild::Complete {
                result: mappings.dupe(),
                observations: epoch.dupe(),
            })
            .unwrap();
        assert!(Arc::ptr_eq(&forwarded, &mappings));
        assert!(Arc::ptr_eq(forwarded_epoch.get(&demand).unwrap(), &first));
        assert!(matches!(
            finish_definition_load_requests_mappings_semantic(
                mappings.as_ref(),
                forwarded_epoch.dupe(),
            ),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(
                    HostSelectedExtensionDefinitionLoadRequestsError(
                        HostSelectedExtensionDefinitionLoadRequestsErrorInner::Mappings(_)
                    )
                )) && observations == epoch
        ));

        let pure = definition_load_requests_complete(
            Err(HostSelectedExtensionDefinitionLoadRequestsError(
                HostSelectedExtensionDefinitionLoadRequestsErrorInner::InvalidContext(
                    "pure".into(),
                ),
            )),
            epoch.dupe(),
        );
        assert!(matches!(
            pure,
            SourcePreparationOutcome::Complete(Ok((result, observations)))
                if matches!(result.as_ref(), Err(
                    HostSelectedExtensionDefinitionLoadRequestsError(
                        HostSelectedExtensionDefinitionLoadRequestsErrorInner::InvalidContext(
                            message
                        )
                    )
                ) if message == "pure") && observations == epoch
        ));
        let projected = Arc::new(Err(HostSelectedExtensionDefinitionLoadRequestsError(
            HostSelectedExtensionDefinitionLoadRequestsErrorInner::InvalidContext("legacy".into()),
        )));
        assert!(matches!(
            project_legacy_definition_load_requests(SourcePreparationOutcome::Complete(Ok((
                projected.dupe(),
                epoch.dupe(),
            )))),
            SourcePreparationOutcome::Complete(result) if Arc::ptr_eq(&result, &projected)
        ));
        let changed_epoch = PathObservationEpoch::from_shared([(
            demand.dupe(),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 2, 3, 4, 0o644),
            ))),
        )])
        .unwrap();
        let associated = |observations| {
            SourcePreparationOutcome::Complete(Ok(
                ObservedHostSelectedExtensionDefinitionLoadRequests {
                    result: projected.dupe(),
                    observations,
                },
            ))
        };
        assert!(
            !HostSelectedExtensionDefinitionLoadRequestsObservationKey::equality(
                &associated(epoch.dupe()),
                &associated(changed_epoch),
            )
        );
        let need_value = SourcePreparationOutcome::Need(need);
        assert!(!HostSelectedExtensionDefinitionLoadRequestsObservationKey::validity(&need_value));
        assert!(
            !HostSelectedExtensionDefinitionLoadRequestsObservationKey::equality(
                &need_value,
                &need_value
            )
        );
    }

    #[tokio::test]
    async fn observed_definition_requests_match_families_epochs_events_and_errors() {
        const ROOT: &str = "module(name='bazel_tools')\ne=use_extension('//:e.bzl','e')\nuse_repo(e, repo='repo')\n";
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(workspace.dupe());
        let tracker = Arc::new(RepoSpecTracker::default());
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut transaction =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        let cold = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_definition_requests(&cold);
        let mappings = complete_observed_extensions(
            &transaction
                .compute(&HostSelectedExtensionMappingsObservationKey::new(
                    workspace.dupe(),
                ))
                .await
                .unwrap(),
        );
        assert_exact_repo_epoch(mappings.observations(), observed.observations());
        assert_eq!(
            observed
                .result()
                .as_ref()
                .as_ref()
                .unwrap()
                .predecessor
                .as_ref(),
            mappings.result().as_ref().as_ref().unwrap()
        );
        let (observed_activations, observed_rows) = tracker.take();
        assert_eq!(
            repo_spec_row(&observed_rows, &key.to_string()),
            [HostSelectedExtensionMappingsObservationKey::new(workspace.dupe()).to_string()]
        );
        assert!(
            observed_activations
                .iter()
                .any(|entry| entry.key == key.to_string() && entry.batch.is_none())
        );
        assert!(
            HostSelectedExtensionDefinitionLoadRequestsObservationKey::equality(
                &cold,
                &transaction.compute(&key).await.unwrap()
            )
        );
        assert!(tracker.take().0.iter().all(|entry| entry.batch.is_none()));

        let legacy_tracker = Arc::new(RepoSpecTracker::default());
        let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let legacy_key = HostSelectedExtensionDefinitionLoadRequestsKey::new(workspace.dupe());
        let mut legacy = real_transaction_with_tracker(
            &legacy_dice,
            ROOT,
            1,
            &[],
            true,
            Some(legacy_tracker.dupe()),
        )
        .await;
        let SourcePreparationOutcome::Complete(legacy_result) =
            legacy.compute(&legacy_key).await.unwrap()
        else {
            panic!("legacy definition requests must complete")
        };
        assert_eq!(observed.result(), &legacy_result);
        let (legacy_activations, legacy_rows) = legacy_tracker.take();
        assert_eq!(
            repo_spec_row(&legacy_rows, &legacy_key.to_string()),
            [HostSelectedExtensionMappingsKey::new(workspace.dupe()).to_string()]
        );
        let eventful = |entries: &[RepoSpecActivation]| {
            entries
                .iter()
                .filter_map(|entry| entry.batch.dupe().map(|batch| (entry.key.clone(), batch)))
                .collect::<Vec<_>>()
        };
        let observed_events = eventful(&observed_activations);
        let legacy_events = eventful(&legacy_activations);
        assert_eq!(
            observed_events
                .iter()
                .map(|(_, batch)| batch)
                .collect::<Vec<_>>(),
            legacy_events
                .iter()
                .map(|(_, batch)| batch)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            observed_events
                .iter()
                .map(|(owner, _)| owner.as_str())
                .collect::<Vec<_>>(),
            ["bzlmod-observed-host-root-module-file:\"/selected-repo-spec-test\""]
        );
        assert_eq!(
            legacy_events
                .iter()
                .map(|(owner, _)| owner.as_str())
                .collect::<Vec<_>>(),
            ["root-module-evaluation:/selected-repo-spec-test"]
        );
        assert!(observed_rows.iter().all(|(owner, deps)| {
            !owner.starts_with("host-selected-extension-mappings:")
                && deps
                    .iter()
                    .all(|dep| !dep.starts_with("host-selected-extension-mappings:"))
        }));
        assert!(legacy_rows.iter().all(|(owner, deps)| {
            !owner.starts_with("observed-host-selected-extension-mappings:")
                && deps
                    .iter()
                    .all(|dep| !dep.starts_with("observed-host-selected-extension-mappings:"))
        }));
        assert_no_definition_request_upper(&observed_rows);
        assert_no_definition_request_upper(&legacy_rows);

        let error = complete_observed_definition_requests(
            &compute_real_observed_definition_requests(
                &dice,
                "module(name='bazel_tools')\ne=use_extension('@missing//:e.bzl','e')\n",
                2,
                true,
                Some(tracker.dupe()),
            )
            .await,
        );
        assert!(matches!(
            error.result().as_ref(),
            Err(HostSelectedExtensionDefinitionLoadRequestsError(
                HostSelectedExtensionDefinitionLoadRequestsErrorInner::Mappings(_)
            ))
        ));
        assert!(!error.observations().observations().is_empty());
        assert_no_definition_request_upper(&tracker.take().1);
    }

    #[tokio::test]
    async fn observed_definition_requests_need_cancel_and_lifecycle_are_exact() {
        const NEED_ROOT: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n";
        let need_io = Arc::new(TrackingRegistryIo::new([(
            "https://registry.invalid/modules/dep/1/MODULE.bazel",
            b"module(name='dep', version='1')\n" as &[u8],
        )]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, need_io);
        let need_dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepoSpecTracker::default());
        let need = compute_real_observed_definition_requests(
            &need_dice,
            NEED_ROOT,
            1,
            false,
            Some(tracker.dupe()),
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostSelectedExtensionDefinitionLoadRequestsObservationKey::validity(&need));
        assert!(!HostSelectedExtensionDefinitionLoadRequestsObservationKey::equality(&need, &need));
        assert_no_definition_request_upper(&tracker.take().1);

        let io = Arc::new(CancelOnceRegistryIo {
            calls: AtomicUsize::new(0),
        });
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.dupe());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let key = HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let mut cancelled =
            real_transaction_with_tracker(&dice, NEED_ROOT, 1, &[], true, Some(tracker.dupe()))
                .await;
        tracker.take();
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while io.calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(future);
        drop(cancelled);
        let (cancelled_activations, cancelled_rows) = tracker.take();
        assert!(
            cancelled_rows
                .iter()
                .all(|(owner, _)| owner != &key.to_string())
        );
        assert!(
            cancelled_activations
                .iter()
                .all(|entry| entry.key != key.to_string())
        );
        assert_no_definition_request_upper(&cancelled_rows);
        let recovered = complete_observed_definition_requests(
            &compute_real_observed_definition_requests(
                &dice,
                NEED_ROOT,
                1,
                true,
                Some(tracker.dupe()),
            )
            .await,
        );
        assert!(recovered.result().as_ref().is_ok());
        assert!(!recovered.observations().observations().is_empty());
        assert_no_definition_request_upper(&tracker.take().1);

        const A: &str = "module(name='bazel_tools',repo_name='root')\na=use_extension('//:a.bzl','a')\nb=use_extension('//:b.bzl','b')\n";
        const B: &str = "module(name='bazel_tools',repo_name='root')\nb=use_extension('//:b.bzl','b')\na=use_extension('//:a.bzl','a')\n";
        let lifecycle_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let a = complete_observed_definition_requests(
            &compute_real_observed_definition_requests(&lifecycle_dice, A, 1, true, None).await,
        );
        let held_result = a.result().dupe();
        let held_epoch = a.observations().dupe();
        let b = complete_observed_definition_requests(
            &compute_real_observed_definition_requests(&lifecycle_dice, B, 2, true, None).await,
        );
        assert_ne!(a, b);
        let restored = complete_observed_definition_requests(
            &compute_real_observed_definition_requests(&lifecycle_dice, A, 3, true, None).await,
        );
        assert_eq!(a, restored);
        assert_eq!(held_result.as_ref(), restored.result().as_ref());
        let fixed_epoch = |result| {
            SourcePreparationOutcome::Complete(Ok(
                ObservedHostSelectedExtensionDefinitionLoadRequests {
                    result,
                    observations: held_epoch.dupe(),
                },
            ))
        };
        let pure_a = fixed_epoch(a.result().dupe());
        let pure_b = fixed_epoch(b.result().dupe());
        let pure_restored = fixed_epoch(restored.result().dupe());
        assert!(
            !HostSelectedExtensionDefinitionLoadRequestsObservationKey::equality(&pure_a, &pure_b)
        );
        assert!(
            HostSelectedExtensionDefinitionLoadRequestsObservationKey::equality(
                &pure_a,
                &pure_restored
            )
        );
        let module_file = format!("{WORKSPACE}/MODULE.bazel");
        for (demand, result) in held_epoch.observations() {
            let current = restored.observations().get(demand).unwrap();
            assert_eq!(result.as_ref(), current.as_ref());
            if demand.path().as_path() != Path::new(&module_file) {
                assert!(Arc::ptr_eq(result, current), "{demand:?}");
            }
        }
    }

    #[tokio::test]
    async fn observed_evaluation_inputs_identity_and_terminals_are_exact() {
        const ROOT: &str = "module(name='bazel_tools')\ne=use_extension('//:e.bzl','e')\nuse_repo(e, repo='repo')\n";
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostSelectedExtensionEvaluationInputRequestsObservationKey::new(workspace.dupe());
        let other = HostSelectedExtensionEvaluationInputRequestsObservationKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
        );
        let hash = |value: &HostSelectedExtensionEvaluationInputRequestsObservationKey| {
            let mut state = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(value, &mut state);
            std::hash::Hasher::finish(&state)
        };
        assert_ne!(key, other);
        assert_ne!(hash(&key), hash(&other));
        assert_eq!(
            key.to_string(),
            "observed-host-selected-extension-evaluation-inputs:\"/selected-repo-spec-test\""
        );

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut transaction = real_transaction(&dice, ROOT, 1, &[], true).await;
        let requests_observed = complete_observed_definition_requests(
            &transaction
                .compute(
                    &HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(
                        workspace.dupe(),
                    ),
                )
                .await
                .unwrap(),
        );
        let requests = requests_observed.result().dupe();
        let root_value = transaction
            .compute(&RootModuleFilesObservationKey::new(workspace.dupe()))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(root_observed)) = root_value else {
            panic!("observed root files must complete: {root_value:?}");
        };

        let request_demand = observation("/request", PathObservationOperation::Lstat);
        let root_demand = observation("/root", PathObservationOperation::FileBytes);
        let request_result = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let root_result = Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Missing,
        ));
        let request_epoch =
            PathObservationEpoch::from_shared([(request_demand.dupe(), request_result.dupe())])
                .unwrap();
        let root_epoch = PathObservationEpoch::from_shared([
            (request_demand.dupe(), request_result.dupe()),
            (root_demand.dupe(), root_result.dupe()),
        ])
        .unwrap();
        let need = SourcePreparationNeeds::path(
            slug_workspace_v2::NeedPathObservations::singleton(request_demand.dupe()),
        );
        assert!(matches!(
            finish_evaluation_input_requests_request_child(RepoSpecChild::Compute("request".into())),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(
                    HostSelectedExtensionEvaluationInputRequestsError::LoadRequestsCompute(message)
                ) if message == "request") && observations.observations().is_empty()
        ));
        assert!(matches!(
            finish_evaluation_input_requests_request_child(RepoSpecChild::Need(need.dupe())),
            Err(SourcePreparationOutcome::Need(_))
        ));
        let request_outer = HostSelectedExtensionDefinitionLoadRequestsObservationError(
            DefinitionLoadRequestsObservationError::Mappings(
                ExtensionMappingsObservationError::RootFiles(ObservedPathFrontierError::from(
                    PathObservationEpochError::OperationMismatch {
                        demand: request_demand.dupe(),
                        result_operation: PathObservationOperation::FileBytes,
                    },
                )),
            ),
        );
        assert!(matches!(
            finish_evaluation_input_requests_request_child(RepoSpecChild::Outer(request_outer)),
            Err(SourcePreparationOutcome::Complete(Err(
                EvaluationInputRequestsObservationError::Requests(_)
            )))
        ));
        let (forwarded, forwarded_epoch) =
            finish_evaluation_input_requests_request_child(RepoSpecChild::Complete {
                result: requests.dupe(),
                observations: request_epoch.dupe(),
            })
            .unwrap();
        assert!(Arc::ptr_eq(&forwarded, &requests));
        assert_exact_repo_epoch(&request_epoch, &forwarded_epoch);

        let requests_value =
            finish_evaluation_input_requests_request_semantic(&forwarded, forwarded_epoch.dupe())
                .unwrap();
        assert!(matches!(
            finish_evaluation_input_requests_root_child(
                RepoSpecChild::Compute("root".into()),
                requests_value.dupe(),
                forwarded_epoch.dupe(),
            ),
            Err(SourcePreparationOutcome::Complete(Ok((result, observations))))
                if matches!(result.as_ref(), Err(
                    HostSelectedExtensionEvaluationInputRequestsError::AfterRequests {
                        error: HostSelectedExtensionEvaluationInputError::RootFilesCompute(message),
                        ..
                    }
                ) if message == "root") && observations == request_epoch
        ));
        assert!(matches!(
            finish_evaluation_input_requests_root_child(
                RepoSpecChild::Need(need.dupe()),
                requests_value.dupe(),
                forwarded_epoch.dupe(),
            ),
            Err(SourcePreparationOutcome::Need(_))
        ));
        assert!(matches!(
            finish_evaluation_input_requests_root_child(
                RepoSpecChild::Outer(ObservedPathFrontierError::from(
                    PathObservationEpochError::OperationMismatch {
                        demand: root_demand.dupe(),
                        result_operation: PathObservationOperation::Lstat,
                    },
                )),
                requests_value.dupe(),
                forwarded_epoch.dupe(),
            ),
            Err(SourcePreparationOutcome::Complete(Err(
                EvaluationInputRequestsObservationError::RootFiles(_)
            )))
        ));
        assert_evaluation_input_semantic_failure_prefixes(
            requests_value.dupe(),
            root_observed.result().as_ref().as_ref().unwrap(),
            &forwarded_epoch,
            &root_epoch,
        );
        let (root, merged) = finish_evaluation_input_requests_root_child(
            RepoSpecChild::Complete {
                result: root_observed.result().dupe(),
                observations: root_epoch.dupe(),
            },
            requests_value.dupe(),
            forwarded_epoch,
        )
        .unwrap();
        assert_eq!(
            root.as_ref(),
            root_observed.result().as_ref().as_ref().unwrap()
        );
        assert_exact_repo_epoch(&root_epoch, &merged);
        assert!(Arc::ptr_eq(
            merged.get(&request_demand).unwrap(),
            &request_result
        ));

        let conflict = PathObservationEpoch::from_shared([(
            request_demand.dupe(),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::Directory, 1, 2, 3, 4, 0o755),
            ))),
        )])
        .unwrap();
        let mut prefix = request_epoch.dupe();
        assert!(matches!(
            merge_evaluation_input_observations(
                &mut prefix,
                &conflict,
                EvaluationInputObservationStage::RootFiles,
            ),
            Err(EvaluationInputRequestsObservationError::Merge {
                stage: EvaluationInputObservationStage::RootFiles,
                ..
            })
        ));
        assert!(matches!(
            evaluation_input_merge_error(
                EvaluationInputObservationStage::RootFiles,
                PathObservationEpochError::OperationMismatch {
                    demand: root_demand,
                    result_operation: PathObservationOperation::Lstat,
                },
            ),
            EvaluationInputRequestsObservationError::Merge {
                stage: EvaluationInputObservationStage::RootFiles,
                ..
            }
        ));

        let pure = Arc::new(selected_extension_evaluation_input_requests(
            requests_value,
            &root,
        ));
        assert_evaluation_input_observation_equality(pure, request_epoch, root_epoch, need);
    }

    #[tokio::test]
    async fn observed_evaluation_inputs_match_families_events_errors_and_warm() {
        const ROOT: &str = "module(name='bazel_tools')\ne=use_extension('//:e.bzl','e')\nuse_repo(e, repo='repo')\n";
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostSelectedExtensionEvaluationInputRequestsObservationKey::new(workspace.dupe());
        let tracker = Arc::new(RepoSpecTracker::default());
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut transaction =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        let cold = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_evaluation_inputs(&cold);
        let requests = complete_observed_definition_requests(
            &transaction
                .compute(
                    &HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(
                        workspace.dupe(),
                    ),
                )
                .await
                .unwrap(),
        );
        let root_value = transaction
            .compute(&RootModuleFilesObservationKey::new(workspace.dupe()))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(root)) = root_value else {
            panic!("observed root files must complete: {root_value:?}");
        };
        let mut expected = requests.observations().dupe();
        merge_evaluation_input_observations(
            &mut expected,
            root.observations(),
            EvaluationInputObservationStage::RootFiles,
        )
        .unwrap();
        assert_exact_repo_epoch(&expected, observed.observations());
        assert_eq!(
            observed.result().as_ref().as_ref().unwrap().parts().0,
            requests.result().as_ref().as_ref().unwrap()
        );

        let (observed_activations, observed_rows) = tracker.take();
        assert_eq!(
            repo_spec_row(&observed_rows, &key.to_string()),
            [
                HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(workspace.dupe())
                    .to_string(),
                RootModuleFilesObservationKey::new(workspace.dupe()).to_string(),
            ]
        );
        assert!(
            observed_activations
                .iter()
                .any(|entry| entry.key == key.to_string() && entry.batch.is_none())
        );
        assert!(
            HostSelectedExtensionEvaluationInputRequestsObservationKey::equality(
                &cold,
                &transaction.compute(&key).await.unwrap()
            )
        );
        assert!(tracker.take().0.iter().all(|entry| entry.batch.is_none()));

        let legacy_tracker = Arc::new(RepoSpecTracker::default());
        let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let legacy_key = HostSelectedExtensionEvaluationInputRequestsKey::new(workspace.dupe());
        let mut legacy = real_transaction_with_tracker(
            &legacy_dice,
            ROOT,
            1,
            &[],
            true,
            Some(legacy_tracker.dupe()),
        )
        .await;
        let SourcePreparationOutcome::Complete(legacy_result) =
            legacy.compute(&legacy_key).await.unwrap()
        else {
            panic!("legacy evaluation inputs must complete")
        };
        assert_eq!(observed.result(), &legacy_result);
        let (legacy_activations, legacy_rows) = legacy_tracker.take();
        assert_eq!(
            repo_spec_row(&legacy_rows, &legacy_key.to_string()),
            [
                HostSelectedExtensionDefinitionLoadRequestsKey::new(workspace.dupe()).to_string(),
                RootModuleFilesKey {
                    workspace: workspace.as_path().to_owned(),
                }
                .to_string(),
            ]
        );
        let eventful = |entries: &[RepoSpecActivation]| {
            entries
                .iter()
                .filter_map(|entry| entry.batch.dupe().map(|batch| (entry.key.clone(), batch)))
                .collect::<Vec<_>>()
        };
        let observed_events = eventful(&observed_activations);
        let legacy_events = eventful(&legacy_activations);
        assert_eq!(
            observed_events
                .iter()
                .map(|(_, batch)| batch)
                .collect::<Vec<_>>(),
            legacy_events
                .iter()
                .map(|(_, batch)| batch)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            observed_events
                .iter()
                .map(|(owner, _)| owner.as_str())
                .collect::<Vec<_>>(),
            ["bzlmod-observed-host-root-module-file:\"/selected-repo-spec-test\""]
        );
        assert_eq!(
            legacy_events
                .iter()
                .map(|(owner, _)| owner.as_str())
                .collect::<Vec<_>>(),
            ["root-module-evaluation:/selected-repo-spec-test"]
        );
        assert!(observed_rows.iter().all(|(owner, deps)| {
            !owner.starts_with("host-selected-extension-definition-load-requests:")
                && !owner.starts_with("root-module-files:")
                && deps.iter().all(|dep| {
                    !dep.starts_with("host-selected-extension-definition-load-requests:")
                        && !dep.starts_with("root-module-files:")
                })
        }));
        assert!(legacy_rows.iter().all(|(owner, deps)| {
            !owner.starts_with("observed-host-selected-extension-definition-load-requests:")
                && !owner.starts_with("observed-root-module-files:")
                && deps.iter().all(|dep| {
                    !dep.starts_with("observed-host-selected-extension-definition-load-requests:")
                        && !dep.starts_with("observed-root-module-files:")
                })
        }));
        assert_no_evaluation_input_upper(&observed_rows);
        assert_no_evaluation_input_upper(&legacy_rows);

        let error = complete_observed_evaluation_inputs(
            &compute_real_observed_evaluation_inputs(
                &dice,
                "module(name='bazel_tools')\ne=use_extension('@missing//:e.bzl','e')\n",
                2,
                true,
                Some(tracker.dupe()),
            )
            .await,
        );
        let request_error = complete_observed_definition_requests(
            &compute_real_observed_definition_requests(
                &dice,
                "module(name='bazel_tools')\ne=use_extension('@missing//:e.bzl','e')\n",
                2,
                true,
                None,
            )
            .await,
        );
        let request_terminal = finish_evaluation_input_requests_request_semantic(
            request_error.result().as_ref(),
            request_error.observations().dupe(),
        );
        let Err(SourcePreparationOutcome::Complete(Ok((request_result, request_epoch)))) =
            request_terminal
        else {
            panic!("request semantic failure must retain the request epoch");
        };
        assert!(matches!(
            error.result().as_ref(),
            Err(HostSelectedExtensionEvaluationInputRequestsError::LoadRequests(_))
        ));
        assert!(matches!(
            request_result.as_ref(),
            Err(HostSelectedExtensionEvaluationInputRequestsError::LoadRequests(_))
        ));
        assert_exact_repo_epoch(request_error.observations(), &request_epoch);
        assert_exact_repo_epoch(request_error.observations(), error.observations());
        let error_rows = tracker.take().1;
        assert_eq!(
            repo_spec_row(&error_rows, &key.to_string()),
            [
                HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(workspace)
                    .to_string()
            ]
        );
        assert_no_evaluation_input_upper(&error_rows);
    }

    #[tokio::test]
    async fn observed_evaluation_inputs_restore_each_revision_family() {
        const A: &str = "module(name='bazel_tools',repo_name='root',version='1')\na=use_extension('//:a.bzl','a')\na.tag(value='a')\nb=use_extension('//:b.bzl','b')\nb.tag(value='b')\n";
        const REQUEST_B: &str = "module(name='bazel_tools',repo_name='root',version='1')\nb=use_extension('//:b.bzl','b')\nb.tag(value='b')\na=use_extension('//:a.bzl','a')\na.tag(value='a')\n";
        const ROOT_B: &str = A;
        const PURE_B: &str = "module(name='bazel_tools',repo_name='root',version='2')\na=use_extension('//:a.bzl','a')\na.tag(value='a')\nb=use_extension('//:b.bzl','b')\nb.tag(value='b')\n";
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let update = crate::LockfileMode::Update;
        let (a, requests_a, root_a) = observed_evaluation_state(&dice, A, 1, update.clone()).await;
        let held_result = a.result().dupe();
        let held_epoch = a.observations().dupe();

        let (request_b, requests_b, _) =
            observed_evaluation_state(&dice, REQUEST_B, 2, update.clone()).await;
        assert_ne!(requests_a.result(), requests_b.result());
        assert_ne!(a.result(), request_b.result());
        let (request_restored, requests_restored, _) =
            observed_evaluation_state(&dice, A, 3, update.clone()).await;
        assert_eq!(requests_a, requests_restored);
        assert_eq!(a, request_restored);

        let (root_b, requests_root_b, root_files_b) =
            observed_evaluation_state(&dice, ROOT_B, 4, crate::LockfileMode::Off).await;
        assert_eq!(requests_a.result(), requests_root_b.result());
        assert_ne!(root_a.result(), root_files_b.result());
        assert_eq!(a.result(), root_b.result());
        assert_ne!(a.observations(), root_b.observations());
        let (root_restored, requests_root_restored, root_files_restored) =
            observed_evaluation_state(&dice, A, 5, update.clone()).await;
        assert_eq!(requests_a, requests_root_restored);
        assert_eq!(root_a, root_files_restored);
        assert_eq!(a, root_restored);

        let (pure_b, _, _) = observed_evaluation_state(&dice, PURE_B, 6, update.clone()).await;
        assert_ne!(a.result(), pure_b.result());
        let (restored, requests_final, root_final) =
            observed_evaluation_state(&dice, A, 7, update).await;
        assert_eq!(requests_a, requests_final);
        assert_eq!(root_a, root_final);
        assert_eq!(a, restored);
        let fixed_epoch = |result| {
            SourcePreparationOutcome::Complete(Ok(
                ObservedHostSelectedExtensionEvaluationInputRequests {
                    result,
                    observations: held_epoch.dupe(),
                },
            ))
        };
        assert!(
            !HostSelectedExtensionEvaluationInputRequestsObservationKey::equality(
                &fixed_epoch(a.result().dupe()),
                &fixed_epoch(pure_b.result().dupe()),
            )
        );
        assert!(
            HostSelectedExtensionEvaluationInputRequestsObservationKey::equality(
                &fixed_epoch(a.result().dupe()),
                &fixed_epoch(restored.result().dupe()),
            )
        );
        assert_eq!(held_result.as_ref(), restored.result().as_ref());
        let module_file = format!("{WORKSPACE}/MODULE.bazel");
        for (demand, result) in held_epoch.observations() {
            let current = restored.observations().get(demand).unwrap();
            assert_eq!(result.as_ref(), current.as_ref());
            if demand.path().as_path() != Path::new(&module_file) {
                assert!(Arc::ptr_eq(result, current), "{demand:?}");
            }
        }
    }

    #[tokio::test]
    async fn observed_evaluation_inputs_poll_drop_publishes_nothing_and_recovers() {
        const ROOT: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n";
        let io = Arc::new(CancelOnceRegistryIo {
            calls: AtomicUsize::new(0),
        });
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.dupe());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepoSpecTracker::default());
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostSelectedExtensionEvaluationInputRequestsObservationKey::new(workspace);
        let mut cancelled =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        tracker.take();
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while io.calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(future);
        drop(cancelled);
        let (cancelled_activations, cancelled_rows) = tracker.take();
        assert!(
            cancelled_rows
                .iter()
                .all(|(owner, _)| owner != &key.to_string())
        );
        assert!(
            cancelled_activations
                .iter()
                .all(|entry| entry.key != key.to_string())
        );
        assert_no_evaluation_input_upper(&cancelled_rows);

        let recovered = complete_observed_evaluation_inputs(
            &compute_real_observed_evaluation_inputs(&dice, ROOT, 1, true, Some(tracker.dupe()))
                .await,
        );
        assert!(recovered.result().as_ref().is_ok());
        assert!(!recovered.observations().observations().is_empty());
        let (recovered_activations, recovered_rows) = tracker.take();
        assert!(
            recovered_activations
                .iter()
                .filter(|entry| entry.key == key.to_string())
                .all(|entry| entry.batch.is_none())
        );
        assert_no_evaluation_input_upper(&recovered_rows);
    }

    #[test]
    fn observed_canonical_selected_definition_identity_scan_and_terminal_algebra() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let canonical = CanonicalRepoName::new("dep+").unwrap();
        let key = HostCanonicalSelectedModuleDefinitionObservationKey::new(
            workspace.dupe(),
            canonical.clone(),
        );
        let same = HostCanonicalSelectedModuleDefinitionObservationKey::new(
            workspace.dupe(),
            canonical.clone(),
        );
        let other = HostCanonicalSelectedModuleDefinitionObservationKey::new(
            workspace,
            CanonicalRepoName::new("other+").unwrap(),
        );
        assert_eq!(
            key.to_string(),
            "observed-host-canonical-selected-module-definition:\"/workspace\":@@dep+"
        );
        assert_eq!(key, same);
        assert_eq!(
            selected_definition_hash(&key),
            selected_definition_hash(&same)
        );
        assert_ne!(key, other);
        let demand = observation("/selected", PathObservationOperation::Lstat);
        let observation = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let epoch =
            PathObservationEpoch::from_shared([(demand.dupe(), observation.dupe())]).unwrap();
        let needs = || {
            SourcePreparationNeeds::path(slug_workspace_v2::NeedPathObservations::singleton(
                demand.dupe(),
            ))
        };
        assert!(matches!(
            complete_canonical_selected_definition_driver(RepoSpecChild::Need(needs()), &key.0),
            SourcePreparationOutcome::Need(_)
        ));
        let mismatch =
            ObservedPathFrontierError::from(PathObservationEpochError::OperationMismatch {
                demand: demand.dupe(),
                result_operation: PathObservationOperation::FileBytes,
            });
        let outer = complete_canonical_selected_definition_driver(
            RepoSpecChild::Outer(HostSelectedModuleRoutesObservationError::Graph(
                HostSelectedModuleGraphObservationError::Merge(mismatch),
            )),
            &key.0,
        );
        let outer: <HostCanonicalSelectedModuleDefinitionObservationKey as Key>::Value = match outer
        {
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            _ => panic!("observed route outer must remain carrierless"),
        };
        assert!(matches!(
            outer,
            SourcePreparationOutcome::Complete(Err(
                HostCanonicalSelectedModuleDefinitionObservationError::Routes(_)
            ))
        ));
        let compute = complete_canonical_selected_definition_driver(
            RepoSpecChild::Compute("dice".into()),
            &key.0,
        );
        assert!(matches!(
            compute,
            SourcePreparationOutcome::Complete(Ok((result, observations)))
                if matches!(result.as_ref(), Err(HostCanonicalSelectedModuleDefinitionError {
                    inner: PrivateCanonicalSelectedModuleDefinitionError::RoutesCompute(
                        message, canonical_repo
                    )
                }) if message == "dice" && canonical_repo == &canonical)
                    && observations.observations().is_empty()
        ));
        let dep = route_key("dep", "1");
        let routes = selected_routes(
            &route_graph([
                route_root([("dep", dep.clone())], None),
                route_module("dep", "1", "dep", true),
            ]),
            &HostSelectedRegistryRepoSpecs {
                entries: Arc::from([route_spec(dep)]),
            },
        )
        .unwrap();
        let finish = |result, key| {
            complete_canonical_selected_definition_driver(
                RepoSpecChild::Complete {
                    result,
                    observations: epoch.dupe(),
                },
                &key,
            )
        };
        let failed = Arc::new(Err(route_invalid(&module(), "routes")));
        let semantic = finish(failed.dupe(), key.0.clone());
        assert!(matches!(
            semantic,
            SourcePreparationOutcome::Complete(Ok((result, observations)))
                if matches!(result.as_ref(), Err(HostCanonicalSelectedModuleDefinitionError {
                    inner: PrivateCanonicalSelectedModuleDefinitionError::Routes(
                        predecessor, canonical_repo
                    )
                }) if Arc::ptr_eq(predecessor, &failed) && canonical_repo == &canonical)
                    && observations == epoch
        ));
        let missing_key = HostCanonicalSelectedModuleDefinitionKey::new(
            key.0.workspace.dupe(),
            CanonicalRepoName::new("missing+").unwrap(),
        );
        let missing = finish(Arc::new(Ok(routes.clone())), missing_key);
        assert!(
            matches!(missing, SourcePreparationOutcome::Complete(Ok((result, observations)))
            if matches!(result.as_ref(), Err(HostCanonicalSelectedModuleDefinitionError {
                inner: PrivateCanonicalSelectedModuleDefinitionError::Missing { .. }
            })) && observations == epoch)
        );

        let mut duplicate_routes = routes.clone();
        let mut entries = duplicate_routes.entries.to_vec();
        entries.push(entries[1].clone());
        duplicate_routes.entries = entries.into();
        let consumed = Cell::new(0);
        assert!(matches!(
            find_canonical_route_ordinal(
                &canonical,
                duplicate_routes
                    .entries
                    .iter()
                    .inspect(|_| consumed.set(consumed.get() + 1)),
            ),
            CanonicalRouteMatch::Duplicate {
                first_ordinal: 1,
                conflicting_ordinal: 2
            }
        ));
        assert_eq!(consumed.get(), duplicate_routes.entries.len());
        let duplicate = finish(Arc::new(Ok(duplicate_routes)), key.0.clone());
        assert!(
            matches!(duplicate, SourcePreparationOutcome::Complete(Ok((result, observations)))
            if matches!(result.as_ref(), Err(HostCanonicalSelectedModuleDefinitionError {
                inner: PrivateCanonicalSelectedModuleDefinitionError::Duplicate {
                    first_ordinal: 1, conflicting_ordinal: 2, ..
                }
            })) && observations == epoch)
        );

        let builtin_key = route_key("bazel_tools", "1");
        let builtin_routes = selected_routes(
            &route_graph([
                route_root([("bazel_tools", builtin_key)], None),
                route_module("bazel_tools", "1", "bazel_tools", false),
            ]),
            &HostSelectedRegistryRepoSpecs {
                entries: Arc::from([]),
            },
        )
        .unwrap();
        let builtin = finish(
            Arc::new(Ok(builtin_routes)),
            HostCanonicalSelectedModuleDefinitionKey::new(
                key.0.workspace.dupe(),
                CanonicalRepoName::new("bazel_tools").unwrap(),
            ),
        );
        assert!(
            matches!(builtin, SourcePreparationOutcome::Complete(Ok((result, observations)))
            if matches!(result.as_ref(), Err(HostCanonicalSelectedModuleDefinitionError {
                inner: PrivateCanonicalSelectedModuleDefinitionError::BuiltinDeferred { ordinal: 1, .. }
            })) && observations == epoch)
        );

        let success = finish(Arc::new(Ok(routes)), key.0.clone());
        let SourcePreparationOutcome::Complete(Ok((result, actual_epoch))) = success else {
            panic!("selected success must complete");
        };
        let observed = ObservedHostCanonicalSelectedModuleDefinition {
            result: result.dupe(),
            observations: actual_epoch,
        };
        assert!(Arc::ptr_eq(observed.result(), &result));
        assert_exact_repo_epoch(&epoch, observed.observations());
        let associated = SourcePreparationOutcome::Complete(Ok(observed));
        assert!(HostCanonicalSelectedModuleDefinitionObservationKey::validity(&associated));
        assert!(
            HostCanonicalSelectedModuleDefinitionObservationKey::equality(&associated, &associated)
        );
        let need_value = SourcePreparationOutcome::Need(needs());
        assert!(!HostCanonicalSelectedModuleDefinitionObservationKey::validity(&need_value));
        assert!(
            !HostCanonicalSelectedModuleDefinitionObservationKey::equality(
                &need_value,
                &need_value
            )
        );
    }

    #[tokio::test]
    async fn observed_canonical_selected_definition_real_order_events_and_parity() {
        const ROOT: &str = "module(name='bazel_tools')\n\
            bazel_dep(name='dep', version='1')\n";
        const LOCAL_ROOT: &str = "module(name='bazel_tools')\n\
            local_path_override(module_name='local', path='local')\n\
            bazel_dep(name='local', version='1')\n";
        const MODULE_URL: &str = "https://registry.invalid/modules/dep/1/MODULE.bazel";
        const SOURCE_URL: &str = "https://registry.invalid/modules/dep/1/source.json";
        let files = [
            (MODULE_URL, b"module(name='dep', version='1')\n".as_slice()),
            (
                SOURCE_URL,
                br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#.as_slice(),
            ),
        ];
        let tracker = Arc::new(RepoSpecTracker::default());
        let io = Arc::new(TrackingRegistryIo::new(files));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io);
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostCanonicalSelectedModuleDefinitionObservationKey::new(
            workspace.dupe(),
            CanonicalRepoName::new("dep+").unwrap(),
        );
        let mut transaction =
            real_transaction_with_tracker(&dice, ROOT, 1, &[], true, Some(tracker.dupe())).await;
        let cold = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_selected_definition(&cold);
        let (activations, rows) = tracker.take();
        assert_eq!(
            repo_spec_row(&rows, &key.to_string()),
            [HostSelectedModuleRoutesObservationKey::new(workspace.dupe()).to_string()]
        );
        assert_no_selected_definition_upper(&rows);
        assert!(
            activations
                .iter()
                .filter(|entry| entry.key == key.to_string())
                .all(|entry| entry.batch.is_none())
        );
        let observed_events = activations
            .iter()
            .filter_map(|entry| {
                entry.batch.dupe().map(|batch| {
                    (
                        entry
                            .key
                            .strip_prefix("observed-")
                            .unwrap_or(&entry.key)
                            .to_owned(),
                        batch,
                    )
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            observed_events
                .iter()
                .map(|(owner, _)| owner.as_str())
                .collect::<Vec<_>>(),
            [
                "bzlmod-observed-host-root-module-file:\"/selected-repo-spec-test\"",
                "host-discovered-module:\"/selected-repo-spec-test\":dep@1",
            ]
        );
        let warm = complete_observed_selected_definition(&transaction.compute(&key).await.unwrap());
        let warm_parent = tracker
            .take()
            .0
            .into_iter()
            .find(|entry| entry.key == key.to_string())
            .unwrap();
        assert_eq!(warm_parent.kind, ActivationKind::Reused);
        assert!(warm_parent.batch.is_none());
        assert!(Arc::ptr_eq(observed.result(), warm.result()));

        let legacy_tracker = Arc::new(RepoSpecTracker::default());
        let legacy_io = Arc::new(TrackingRegistryIo::new(files));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, legacy_io);
        let legacy_dice = Arc::new(builder.build(DetectCycles::Enabled));
        let legacy_key = HostCanonicalSelectedModuleDefinitionKey::new(
            workspace.dupe(),
            CanonicalRepoName::new("dep+").unwrap(),
        );
        let mut legacy_transaction = real_transaction_with_tracker(
            &legacy_dice,
            ROOT,
            1,
            &[],
            true,
            Some(legacy_tracker.dupe()),
        )
        .await;
        let legacy = legacy_transaction.compute(&legacy_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy_result) = &legacy else {
            panic!("legacy selected definition must complete");
        };
        assert_eq!(observed.result(), legacy_result);
        let (legacy_activations, legacy_rows) = legacy_tracker.take();
        assert_eq!(
            repo_spec_row(&legacy_rows, &legacy_key.to_string()),
            [HostSelectedModuleRoutesKey::new(workspace.dupe()).to_string()]
        );
        let legacy_events = legacy_activations
            .iter()
            .filter_map(|entry| entry.batch.dupe().map(|batch| (entry.key.clone(), batch)))
            .collect::<Vec<_>>();
        assert_eq!(
            legacy_events
                .iter()
                .map(|(owner, _)| owner.as_str())
                .collect::<Vec<_>>(),
            [
                "root-module-evaluation:/selected-repo-spec-test",
                "host-discovered-module:\"/selected-repo-spec-test\":dep@1",
            ]
        );
        assert_eq!(
            observed_events
                .iter()
                .map(|(_, batch)| batch)
                .collect::<Vec<_>>(),
            legacy_events
                .iter()
                .map(|(_, batch)| batch)
                .collect::<Vec<_>>()
        );

        let mut builtin_root = "module(name='root')\n".to_owned();
        for name in LOCAL_MODULES {
            builtin_root.push_str(&format!(
                "local_path_override(module_name='{name}', path='{name}')\n\
                 bazel_dep(name='{name}', version='1')\n"
            ));
        }
        for (root, canonical) in [
            (ROOT, ""),
            (ROOT, "dep+"),
            (LOCAL_ROOT, "local+"),
            (ROOT, "missing+"),
            (builtin_root.as_str(), "bazel_tools"),
        ] {
            let (observed, legacy, _) = observed_selected_state(&dice, root, 1, canonical).await;
            let SourcePreparationOutcome::Complete(legacy) = legacy else {
                panic!("family legacy must complete");
            };
            assert_eq!(observed.result(), &legacy, "{canonical}");
        }
        let route_error_root = "module(name='root')\nbazel_dep(name='absent', version='1')\n";
        let (observed_error, legacy_error, _) =
            observed_selected_state(&dice, route_error_root, 2, "").await;
        let SourcePreparationOutcome::Complete(legacy_error) = legacy_error else {
            panic!("route error must complete");
        };
        assert_eq!(observed_error.result(), &legacy_error);
        assert!(matches!(
            observed_error.result().as_ref(),
            Err(HostCanonicalSelectedModuleDefinitionError {
                inner: PrivateCanonicalSelectedModuleDefinitionError::Routes(..)
            })
        ));
        let need = compute_real_observed_selected_definition(
            &dice,
            ROOT,
            3,
            "dep+",
            false,
            Some(tracker.dupe()),
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostCanonicalSelectedModuleDefinitionObservationKey::validity(&need));
        let (need_activations, need_rows) = tracker.take();
        assert!(!need_activations.is_empty());
        assert_eq!(
            repo_spec_row(&need_rows, &key.to_string()),
            [HostSelectedModuleRoutesObservationKey::new(workspace).to_string()]
        );
        assert!(need_activations.iter().all(|entry| entry.batch.is_none()));
        assert_no_selected_definition_upper(&need_rows);
    }

    #[tokio::test]
    async fn observed_canonical_selected_definition_lifecycle_cancellation_and_nonactivation() {
        const MODULE_1: &str = "https://registry.invalid/modules/dep/1/MODULE.bazel";
        const MODULE_2: &str = "https://registry.invalid/modules/dep/2/MODULE.bazel";
        const EXTRA_MODULE: &str = "https://registry.invalid/modules/extra/1/MODULE.bazel";
        const SOURCE_1: &str = "https://registry.invalid/modules/dep/1/source.json";
        const SOURCE_2: &str = "https://registry.invalid/modules/dep/2/source.json";
        const EXTRA_SOURCE: &str = "https://registry.invalid/modules/extra/1/source.json";
        const SOURCE_A: &[u8] = br#"{"url":"https://origin.test/a.tgz","integrity":"sha256-a"}"#;
        const SOURCE_B: &[u8] = br#"{"url":"https://origin.test/b.tgz","integrity":"sha256-b"}"#;
        const ROOT: &str = "module(name='bazel_tools')\n\
            bazel_dep(name='dep', version='1', repo_name='dep_alias')\n\
            bazel_dep(name='extra', version='1', repo_name='extra_alias')\n\
            local_path_override(module_name='local', path='local')\n\
            bazel_dep(name='local', version='1')\n";
        let io = Arc::new(TrackingRegistryIo::new([
            (MODULE_1, b"module(name='dep', version='1')\n".as_slice()),
            (MODULE_2, b"module(name='dep', version='2')\n".as_slice()),
            (
                EXTRA_MODULE,
                b"module(name='extra', version='1')\n".as_slice(),
            ),
            (SOURCE_1, SOURCE_A),
            (SOURCE_2, SOURCE_A),
            (EXTRA_SOURCE, SOURCE_A),
        ]));
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, io.dupe());
        let dice = Arc::new(builder.build(DetectCycles::Enabled));

        let (base, _, global) = observed_selected_state(&dice, ROOT, 1, "dep+").await;
        assert_selected_epoch_subset(&base, &global);
        let held_result = base.result().dupe();
        let held_carrier = base.dupe();
        let held_epoch = base.observations().dupe();
        io.replace(SOURCE_1, SOURCE_B);
        let (source_b, _, source_global) = observed_selected_state(&dice, ROOT, 2, "dep+").await;
        assert_ne!(base.result(), source_b.result());
        assert_selected_epoch_subset(&source_b, &source_global);
        io.replace(SOURCE_1, SOURCE_A);
        let (source_a, _, source_a_global) = observed_selected_state(&dice, ROOT, 3, "dep+").await;
        assert_eq!(base.result(), source_a.result());
        assert_selected_epoch_subset(&source_a, &source_a_global);

        let version = ROOT.replace("name='dep', version='1'", "name='dep', version='2'");
        let order = "module(name='bazel_tools')\n\
            bazel_dep(name='extra', version='1', repo_name='extra_alias')\n\
            bazel_dep(name='dep', version='1', repo_name='dep_alias')\n\
            local_path_override(module_name='local', path='local')\n\
            bazel_dep(name='local', version='1')\n";
        let local = ROOT.replace("path='local'", "path='local/.'");
        for (index, changed_root) in [version.as_str(), order, local.as_str()]
            .into_iter()
            .enumerate()
        {
            let (a, _, a_global) =
                observed_selected_state(&dice, ROOT, 10 + index as u64 * 3, "dep+").await;
            let (b, _, b_global) =
                observed_selected_state(&dice, changed_root, 11 + index as u64 * 3, "dep+").await;
            let (restored, _, restored_global) =
                observed_selected_state(&dice, ROOT, 12 + index as u64 * 3, "dep+").await;
            assert_ne!(a.result(), b.result(), "axis {index}");
            assert_eq!(a.result(), restored.result(), "axis {index}");
            assert_selected_epoch_subset(&a, &a_global);
            assert_selected_epoch_subset(&b, &b_global);
            assert_selected_epoch_subset(&restored, &restored_global);
        }

        let metadata = format!("{ROOT}# metadata-only revision\n");
        let (metadata_a, legacy_a, metadata_global_a) =
            observed_selected_state(&dice, ROOT, 30, "dep+").await;
        let (metadata_b, legacy_b, metadata_global_b) =
            observed_selected_state(&dice, &metadata, 31, "dep+").await;
        assert_eq!(metadata_a.result(), metadata_b.result());
        assert!(HostCanonicalSelectedModuleDefinitionKey::equality(
            &legacy_a, &legacy_b,
        ));
        assert_ne!(metadata_a.observations(), metadata_b.observations());
        assert_selected_epoch_subset(&metadata_a, &metadata_global_a);
        assert_selected_epoch_subset(&metadata_b, &metadata_global_b);
        let associated = |observed| SourcePreparationOutcome::Complete(Ok(observed));
        assert!(
            !HostCanonicalSelectedModuleDefinitionObservationKey::equality(
                &associated(metadata_a),
                &associated(metadata_b),
            )
        );
        assert_eq!(held_result.as_ref(), held_carrier.result().as_ref());
        assert_exact_repo_epoch(&held_epoch, held_carrier.observations());

        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let warm_key = HostCanonicalSelectedModuleDefinitionObservationKey::new(
            workspace.dupe(),
            CanonicalRepoName::new("dep+").unwrap(),
        );
        let mut warm_transaction = real_transaction(&dice, ROOT, 40, &[], true).await;
        let warm_a = complete_observed_selected_definition(
            &warm_transaction.compute(&warm_key).await.unwrap(),
        );
        let warm_b = complete_observed_selected_definition(
            &warm_transaction.compute(&warm_key).await.unwrap(),
        );
        assert!(Arc::ptr_eq(warm_a.result(), warm_b.result()));

        let cancel_io = Arc::new(CancelOnceRegistryIo {
            calls: AtomicUsize::new(0),
        });
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, cancel_io.dupe());
        let cancel_dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepoSpecTracker::default());
        let cancel_key = HostCanonicalSelectedModuleDefinitionObservationKey::new(
            workspace,
            CanonicalRepoName::new("dep+").unwrap(),
        );
        let cancel_root = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1')\n";
        let mut cancelled = real_transaction_with_tracker(
            &cancel_dice,
            cancel_root,
            1,
            &[],
            true,
            Some(tracker.dupe()),
        )
        .await;
        tracker.take();
        let mut future = Box::pin(cancelled.compute(&cancel_key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while cancel_io.calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(future);
        drop(cancelled);
        let (cancelled_activations, cancelled_rows) = tracker.take();
        assert!(
            cancelled_rows
                .iter()
                .all(|(owner, _)| owner != &cancel_key.to_string())
        );
        assert!(
            cancelled_activations
                .iter()
                .all(|entry| entry.key != cancel_key.to_string())
        );
        assert_no_selected_definition_upper(&cancelled_rows);
        let mut recovered_transaction = real_transaction_with_tracker(
            &cancel_dice,
            cancel_root,
            1,
            &[],
            true,
            Some(tracker.dupe()),
        )
        .await;
        let recovered_value = recovered_transaction.compute(&cancel_key).await.unwrap();
        let recovered = complete_observed_selected_definition(&recovered_value);
        let (recovered_activations, recovered_rows) = tracker.take();
        assert!(
            recovered_activations
                .iter()
                .filter(|entry| entry.key == cancel_key.to_string())
                .all(|entry| entry.batch.is_none())
        );
        assert_no_selected_definition_upper(&recovered_rows);
        let recovered_global = recovered_transaction
            .compute(&PathObservationEpochKey)
            .await
            .unwrap();
        let legacy_key = HostCanonicalSelectedModuleDefinitionKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            CanonicalRepoName::new("dep+").unwrap(),
        );
        let legacy = recovered_transaction.compute(&legacy_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("recovered legacy control must complete");
        };
        let (clean, _, _) = observed_selected_state(&dice, cancel_root, 50, "dep+").await;
        assert_eq!(recovered.result(), &legacy);
        assert_eq!(recovered.result(), clean.result());
        assert_selected_epoch_subset(&recovered, &recovered_global);
        for source in [
            include_str!("lib.rs"),
            include_str!("../../slug_loading_v2/src/bzl_module.rs"),
            include_str!("../../slug_core_v2/src/runtime/generated_repository_definition.rs"),
        ] {
            assert!(!source.contains("HostCanonicalSelectedModuleDefinitionObservationKey"));
        }
    }
}
